// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tauri commands: thin wrappers over the `terrazgo_core` and `module_cue`
//! repositories, plus the error mapping for the command boundary. Logic stays
//! in the crates and is tested there (docs/architecture.md → Testing strategy #4).

use anyhow::anyhow;
use module_cue::alerts::AlertConfig;
use module_cue::repository;
use terrazgo_core::date::today_utc;

use crate::state;
use crate::state::AppState;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::MutexGuard;
use tauri::State;

// One file per crate the commands wrap. The parent keeps the boundary
// machinery below and re-exports each child, so `generate_handler!`'s
// `commands::<name>` paths do not depend on which file a command lives in
// — moving one between domains is not an API change.
mod app;
mod core;
mod cue;
mod fertilisation;
mod geo;
mod recordbook;
mod sigpac;

pub use app::*;
pub use core::*;
pub use cue::*;
pub use fertilisation::*;
pub use geo::*;
pub use recordbook::*;
pub use sigpac::*;

/// Serializable error for the command boundary. Tauri requires command errors
/// to implement `Serialize`; `CueError`/`anyhow::Error` do not.
///
/// Serialized as `{ code, params, message }`: `code` is a stable machine
/// string the frontend maps to an `error.<code>` i18n key, `params` carries
/// the values its `{placeholders}` interpolate, and `message` is the full
/// `{:#}` Display chain (message + causes) — the untranslated fallback for
/// codes without a dictionary entry and the debugging trail for `internal`.
pub struct CommandError(anyhow::Error);

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let (code, params) = classify(&self.0);
        let mut s = serializer.serialize_struct("CommandError", 3)?;
        s.serialize_field("code", &code)?;
        s.serialize_field("params", &params)?;
        s.serialize_field("message", &format!("{:#}", self.0))?;
        s.end()
    }
}

/// Map a boundary error to its (code, interpolation params) pair.
///
/// `anyhow::Error` keeps the concrete type it was built from, so the domain
/// errors are recovered here by downcast — the commands themselves stay on the
/// blanket `?` conversion and never name error variants. Anything that is not
/// a domain error (SQLite, migration, poisoned mutex, …) is `internal`: the
/// frontend has no dictionary entry for it and shows the raw message instead.
///
/// The tables every registered module contributes to the backup shape probe,
/// composed the same way `composed_migrations()` composes their migration
/// steps — and for the same reason: core owns the probe but may never name a
/// module's tables, so each module declares its own and the shell asks the
/// registry. Registering a module is what adds its tables; there is no second
/// list to keep in step.
///
/// Public so the shell's backup tests can assert the composition rather than
/// re-deriving it.
pub fn module_backup_shape() -> Vec<terrazgo_core::backup::TableShape> {
    crate::registry::registered_modules()
        .iter()
        .flat_map(|module| module.backup_shape().iter().copied())
        .collect()
}

/// Public for the i18n contract test (`tests/i18n_contract.rs`), which checks
/// that every emitted code has an `error.<code>` key in every locale dictionary.
pub fn classify(err: &anyhow::Error) -> (String, serde_json::Value) {
    use terrazgo_core::Classify;

    // One line per error type that can reach a command; each crate maps its own
    // variants (see `terrazgo_core::Classify`), so the codes live beside the
    // enums that emit them and a new variant is a compile error where it was
    // added. The chain itself is irreducible: `anyhow::downcast_ref` needs a
    // concrete type, so the boundary has to name the types it knows.
    if let Some(e) = err.downcast_ref::<terrazgo_core::CoreError>() {
        return e.classify();
    }
    if let Some(e) = err.downcast_ref::<module_cue::CueError>() {
        return e.classify();
    }
    if let Some(e) = err.downcast_ref::<module_fertilisation::FertilisationError>() {
        return e.classify();
    }
    if let Some(e) = err.downcast_ref::<terrazgo_recordbook::RecordbookError>() {
        return e.classify();
    }
    if let Some(e) = err.downcast_ref::<terrazgo_geo::GeoError>() {
        return e.classify();
    }

    // Anything else — a rusqlite error escaping a helper, a plain anyhow
    // message — is not something a user can act on.
    ("internal".into(), serde_json::json!({}))
}

// Blanket conversion so `?` maps any error (`CueError`, `rusqlite::Error`,
// plain `anyhow::Error`, …) into `CommandError` at the boundary. Legal only
// because `CommandError` itself is not `Into<anyhow::Error>` — otherwise this
// would overlap with the standard library's reflexive `From<T> for T`.
impl<E: Into<anyhow::Error>> From<E> for CommandError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// Lock the shared connection. A poisoned mutex (a panic while another command
/// held the lock) is unrecoverable for that connection — surface it as an error
/// rather than `unwrap()` (no unwrap/expect outside tests).
fn lock_conn<'a>(state: &'a State<'_, AppState>) -> CmdResult<MutexGuard<'a, Connection>> {
    state
        .conn
        .lock()
        .map_err(|_| CommandError(anyhow!("database connection mutex is poisoned")))
}

/// Same poisoned-mutex reasoning as [`lock_conn`], for the geo cache.
fn lock_geo<'a>(geo: &'a State<'_, state::GeoState>) -> CmdResult<MutexGuard<'a, Connection>> {
    geo.conn
        .lock()
        .map_err(|_| CommandError(anyhow!("geo cache mutex is poisoned")))
}

/// Re-derive the alert set after a write, whatever domain the write was in.
///
/// The alert engine is module-cue's, and modules never call each other — so
/// chaining it after a core, sigpac or fertilisation write is the SHELL's job,
/// which is why this sits here beside the locks rather than in `cue.rs`.
fn reconcile_alerts(conn: &mut Connection) -> Result<(), CommandError> {
    repository::refresh_alerts(conn, &today_utc(), &AlertConfig::default())?;
    Ok(())
}

/// The device's active profile id — the author stamp every write command
/// passes to the repositories (`record_change.actor`). `None` = no active
/// profile; the id is read fresh per command so a Settings change applies
/// immediately. The settings lock is released before any other lock is
/// taken, so the ordering can never deadlock.
fn active_actor(settings: &State<'_, state::SettingsState>) -> CmdResult<Option<String>> {
    Ok(settings
        .settings
        .lock()
        .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?
        .active_user_id
        .clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialized(err: CommandError) -> serde_json::Value {
        serde_json::to_value(&err).unwrap()
    }

    #[test]
    fn domain_error_maps_to_code_and_params() {
        let err = CommandError::from(module_cue::CueError::CountryMismatch {
            provided: "fr".into(),
            farm: "es".into(),
        });
        let value = serialized(err);
        assert_eq!(value["code"], "country_mismatch");
        assert_eq!(value["params"]["provided"], "fr");
        assert_eq!(value["params"]["farm"], "es");
        assert!(value["message"].as_str().unwrap().contains("fr"));
    }

    #[test]
    fn core_invalid_code_becomes_key_suffix() {
        let err = CommandError::from(terrazgo_core::CoreError::Invalid("empty_name"));
        assert_eq!(serialized(err)["code"], "invalid.empty_name");
    }

    #[test]
    fn non_domain_error_is_internal_with_message() {
        let err = CommandError(anyhow!("mutex is poisoned"));
        let value = serialized(err);
        assert_eq!(value["code"], "internal");
        assert_eq!(value["message"], "mutex is poisoned");
    }
}
