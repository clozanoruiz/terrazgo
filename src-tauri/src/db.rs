// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell-owned database runner: composes every module's migration steps into
//! the single global version sequence and opens the app database.
//!
//! Errors here are `anyhow` by design — src-tauri *is* the Tauri command
//! boundary, not a reusable library crate (thiserror stays in the crates,
//! anyhow at the boundary). If the core ever becomes a shared crate, promote to `thiserror`.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::registry;

/// Migration steps owned by the core: the `terrazgo-core` crate keeps the SQL
/// (farm, plot, country, record_change), exactly as module crates keep theirs.
/// Public so tests can pin the global version count.
pub fn core_migrations() -> Vec<M<'static>> {
    terrazgo_core::migration_set()
}

/// The single global migration sequence: core steps first, then each registered
/// module's steps in registry order. The resulting version numbers are GLOBAL —
/// cue's two migrations are global v1 and v2 today.
///
/// Pre-release, reordering/squashing is allowed and dev databases are recreated;
/// the moment any database holds real data, this composed sequence becomes
/// append-only as a whole: new migrations join at the global tail regardless of
/// which crate owns the SQL (docs/architecture.md → Migrations: one global sequence).
pub fn composed_migrations() -> Migrations<'static> {
    let mut steps = core_migrations();
    for module in registry::registered_modules() {
        steps.extend(module.migrations());
    }
    Migrations::new(steps)
}

/// Open (or create) the app database: WAL mode + foreign keys + the composed
/// global migrations. Mirrors `module_cue::db::open`, which stays library-only.
pub fn open_app_db(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    composed_migrations()
        .to_latest(&mut conn)
        .context("applying the global migration sequence")?;
    Ok(conn)
}

/// Current global schema version — the SQLite `user_version` pragma, which
/// `rusqlite_migration` maintains as it applies steps.
pub fn schema_version(conn: &Connection) -> Result<usize> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    usize::try_from(version).context("negative user_version")
}
