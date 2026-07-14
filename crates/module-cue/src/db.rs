// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Database setup: pragmas + versioned migrations embedded in the binary.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

/// The ordered migration steps this module contributes to the core's single global
/// sequence. The core's registry collects these from every module and concatenates
/// them into one `Migrations` set — numbering and execution are owned by the core
/// (docs/architecture.md → Migrations: one global sequence). SQL is embedded with
/// `include_str!` so the binary needs no files at runtime (offline-first).
///
/// Pre-release the set is exactly two files — 0001 (DDL) and 0002 (seed DML) — and is
/// squashed freely, recreating dev databases. The moment any database holds real data,
/// this list becomes append-only (docs/architecture.md → Migrations: one global sequence).
pub fn migration_set() -> Vec<M<'static>> {
    vec![
        M::up(include_str!("../migrations/0001_schema.sql")),
        M::up(include_str!("../migrations/0002_seed_reference.sql")),
    ]
}

/// A runnable set for the library's own tests and the demo example: the CORE's
/// steps followed by this module's, mirroring the shell's composed global
/// sequence (CUE tables reference core tables — farm, plot, country — so the
/// module's SQL cannot run on its own). The app itself never calls this.
pub fn migrations() -> Migrations<'static> {
    let mut steps = terrazgo_core::migration_set();
    steps.extend(migration_set());
    Migrations::new(steps)
}

/// Open an in-memory database with foreign keys enforced and all migrations applied.
/// Used by the repository tests. The app opens its database through the core's
/// composed runner instead.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

/// Open (or create) a file-backed database: WAL mode + foreign keys + migrations.
/// WAL only applies to file databases, so it lives here rather than in `open_in_memory`.
/// Like `open_in_memory`, this is for the library's tests and example — not the app.
pub fn open(path: impl AsRef<std::path::Path>) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}
