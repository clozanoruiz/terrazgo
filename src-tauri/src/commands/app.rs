// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell's own surface: readiness, platform, status, settings,
//! reference-catalogue maintenance and backups.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError, lock_conn, lock_geo, module_backup_shape};
use crate::state;
use crate::state::AppState;
use anyhow::anyhow;
use module_cue::alerts::AlertConfig;
use module_cue::repository;
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use terrazgo_core::catalogue::CatalogueStatus;
use terrazgo_core::catalogue::RefreshReport;
use terrazgo_core::date::today_utc;
use terrazgo_core::settings::AppSettings;

#[derive(Serialize)]
pub struct AppStatus {
    pub db_path: String,
    pub schema_version: usize,
    pub app_version: &'static str,
}

/// Readiness probe for the startup race on Android (see
/// `state::SetupComplete`). Deliberately takes `AppHandle`, never `State`:
/// it must be callable before setup has managed anything.
#[tauri::command]
pub fn app_ready(app: tauri::AppHandle) -> bool {
    use tauri::Manager;
    app.try_state::<state::SetupComplete>().is_some()
}

/// Compile-time platform truth for the frontend: mobile builds carry the
/// geolocation plugin, desktop builds never do. The frontend must gate the
/// GPS controls on this, NOT on probing a plugin command — the plugin
/// rejects `check_permissions` when the device's location services are off,
/// so a rejection does not mean absence (learned on-device, 2026-07-23).
#[tauri::command]
pub fn is_mobile() -> bool {
    cfg!(mobile)
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> CmdResult<AppStatus> {
    Ok(AppStatus {
        db_path: state.db_path.display().to_string(),
        schema_version: state.schema_version,
        app_version: env!("CARGO_PKG_VERSION"),
    })
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Settings plus the code-owned defaults the UI needs to render "unset"
/// meaningfully: an unset cache cap displays the default value, not a blank,
/// and the frontend must not hardcode a copy of the constant.
#[derive(Serialize)]
pub struct SettingsInfo {
    pub settings: AppSettings,
    pub tile_cache_default_bytes: i64,
}

fn settings_info(settings: AppSettings) -> SettingsInfo {
    SettingsInfo {
        settings,
        tile_cache_default_bytes: terrazgo_geo::db::TILE_CACHE_MAX_BYTES,
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, state::SettingsState>) -> CmdResult<SettingsInfo> {
    let guard = state
        .settings
        .lock()
        .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
    Ok(settings_info(guard.clone()))
}

/// Replace the settings wholesale — the Settings form is the source of truth,
/// like the farm/plot full-row updates. Validation belongs to each setting's
/// owning crate (the cache cap is range-checked by terrazgo-geo); the file is
/// written before the in-memory copy so a failed save never leaves them
/// disagreeing. The new cap is enforced immediately: shrinking the cache must
/// visibly act, not wait for the next launch.
///
/// `async` because that enforcement can VACUUM a multi-hundred-MB file
/// (seconds); the body stays synchronous — no `.await`, so holding the state
/// guards is safe.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, state::SettingsState>,
    geo: State<'_, state::GeoState>,
    settings: AppSettings,
) -> CmdResult<SettingsInfo> {
    if let Some(bytes) = settings.tile_cache_max_bytes {
        terrazgo_geo::db::validate_tile_cache_cap(bytes)?;
    }

    terrazgo_core::settings::save_settings(&state.path, &settings)?;
    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
        *guard = settings.clone();
    }

    let cap = settings
        .tile_cache_max_bytes
        .unwrap_or(terrazgo_geo::db::TILE_CACHE_MAX_BYTES);
    let conn = lock_geo(&geo)?;
    terrazgo_geo::db::enforce_tile_cache_cap(&conn, cap)?;

    Ok(settings_info(settings))
}

/// Empty the tile cache, keeping `resource` rows (styles, glyphs, SIGPAC
/// lookup/zone responses — a verified plot stays verifiable offline). Returns
/// the number of tiles dropped, for the notification. `async` for the VACUUM,
/// same reasoning as `update_settings`.
#[tauri::command]
pub async fn clear_tile_cache(geo: State<'_, state::GeoState>) -> CmdResult<usize> {
    let conn = lock_geo(&geo)?;
    Ok(terrazgo_geo::db::clear_tile_cache(&conn)?)
}

// ---------------------------------------------------------------------------
// Reference catalogues
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn catalogue_status(state: State<'_, AppState>) -> CmdResult<Vec<CatalogueStatus>> {
    let conn = lock_conn(&state)?;
    Ok(terrazgo_core::catalogue::catalogue_status(&conn)?)
}

/// Fetch every vendored catalogue from the provider and adopt the ones that
/// pass validation, reporting per file.
///
/// `async` because it does network work over dozens of files (the
/// long-running-command rule); the body stays synchronous, so holding the
/// connection guard is safe.
///
/// Two ordering rules, both deliberate. **The connection lock is taken per
/// file, after that file's bytes have arrived** — never across the network,
/// so the rest of the app keeps answering while a refresh runs (the geo-cache
/// precedent). And **a failure is always a per-file refusal**, never an early
/// return: one retired idTabla or one truncated download must not deny the
/// user the other 46 catalogues' updates.
#[tauri::command]
pub async fn refresh_catalogues(state: State<'_, AppState>) -> CmdResult<Vec<RefreshReport>> {
    let mut reports = Vec::new();
    for id in terrazgo_core::catalogue::vendored_ids() {
        let bytes = match crate::catalogues::fetch_catalogue(id) {
            Ok(bytes) => bytes,
            Err(refusal) => {
                reports.push(refusal);
                continue;
            }
        };
        let mut conn = lock_conn(&state)?;
        reports.push(terrazgo_core::catalogue::refresh_catalogue(
            &mut conn, id, &bytes,
        )?);
    }
    Ok(reports)
}

// ---------------------------------------------------------------------------
// Backup export / import
// ---------------------------------------------------------------------------

/// Export a verified snapshot of the live database to `dest_path` (chosen by
/// the user in the save dialog, so overwriting is already confirmed).
///
/// `async` because sync commands run on the main thread and freeze the window
/// while they work; `VACUUM INTO` + verification scale with database size, so
/// this must run on the async runtime's pool instead. The body stays fully
/// synchronous — it blocks a worker thread, never the UI.
#[tauri::command]
pub async fn export_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dest_path: String,
) -> CmdResult<terrazgo_core::backup::BackupSummary> {
    let conn = lock_conn(&state)?;
    match crate::user_files::stage_dest(&app, &dest_path)? {
        None => Ok(terrazgo_core::backup::export_backup(
            &conn,
            Path::new(&dest_path),
        )?),
        // Android content URI: `VACUUM INTO` needs a real filesystem path, so
        // the verified snapshot lands in a private staging file and is then
        // streamed to the user's chosen document.
        Some(staging) => {
            let summary = terrazgo_core::backup::export_backup(&conn, staging.path())?;
            crate::user_files::copy_to_user_file(&app, staging.path(), &dest_path)?;
            Ok(terrazgo_core::backup::BackupSummary {
                path: dest_path,
                ..summary
            })
        }
    }
}

#[derive(Serialize)]
pub struct ImportSummary {
    /// Schema version found in the imported file (before forward migration).
    pub schema_version_found: i64,
    /// Where the pre-import safety copy of the previous database was written.
    pub safety_backup_path: String,
}

/// Replace the live database with a backup file.
///
/// Order is the safety argument: (1) validate the file (integrity + schema
/// version — newer-than-app is rejected, older migrates forward on reopen);
/// (2) export a safety copy of the CURRENT database next to it; (3) close the
/// live connection (parking an in-memory placeholder in the mutex), copy the
/// backup over the live path, reopen through the composed migration runner and
/// refresh alerts. If reopening fails midway the placeholder stays parked —
/// commands error until restart — but the previous data is already safe in the
/// pre-import copy.
/// `async` for the same reason as `export_backup`: validate + safety copy +
/// file swap take time proportional to database size and must not block the
/// main thread (no `.await` inside, so holding the mutex guard is safe).
#[tauri::command]
pub async fn import_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
) -> CmdResult<ImportSummary> {
    // Android content URI: staged into a private copy so validation and the
    // file swap below work on a real path. Plain paths pass through as-is.
    let src = crate::user_files::stage_user_source(&app, &src_path)?;
    let mut guard = lock_conn(&state)?;

    // The live db is always at the latest composed version, so it IS the
    // ceiling of what this build supports.
    let live_version: i64 = guard.pragma_query_value(None, "user_version", |r| r.get(0))?;
    // The shape probe spans core AND every registered module, the same way the
    // migration sequence does — core cannot name a module's tables itself.
    let info =
        terrazgo_core::backup::validate_backup(src.path(), live_version, &module_backup_shape())?;

    let backups_dir = state
        .db_path
        .parent()
        .ok_or_else(|| CommandError(anyhow!("database path has no parent directory")))?
        .join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    // ISO instant with the filename-hostile characters stripped: 20260702T101500Z.
    let stamp: String = today_utc_instant().replace(['-', ':'], "");
    let safety_path = backups_dir.join(format!("pre-import-{stamp}.db"));
    terrazgo_core::backup::export_backup(&guard, &safety_path)?;

    // Swap: park a placeholder so the old connection drops (closing the file
    // and checkpointing its WAL) before the copy lands on the same path.
    let placeholder = Connection::open_in_memory()?;
    drop(std::mem::replace(&mut *guard, placeholder));
    for suffix in ["-wal", "-shm"] {
        let sidecar = state.db_path.display().to_string() + suffix;
        if Path::new(&sidecar).exists() {
            std::fs::remove_file(&sidecar)?;
        }
    }
    std::fs::copy(src.path(), &state.db_path)?;

    let mut conn = crate::db::open_app_db(&state.db_path)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    *guard = conn;

    Ok(ImportSummary {
        schema_version_found: info.schema_version,
        safety_backup_path: safety_path.display().to_string(),
    })
}

/// Full UTC instant (not just the date) for unique backup filenames.
fn today_utc_instant() -> String {
    terrazgo_core::date::now_utc_iso()
}
