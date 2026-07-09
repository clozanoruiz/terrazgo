// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core-owned migrations, embedded in the binary.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

/// The ordered migration steps the core contributes to the single global sequence.
/// They run FIRST — before every module's steps — so module tables may reference
/// core tables (`farm`, `plot`, `country`, `record_change`). SQL is embedded with
/// `include_str!` so the binary needs no files at runtime (offline-first).
///
/// Pre-release the set is squashed freely (dev databases are recreated). The moment
/// any database holds real data, the composed global sequence becomes append-only
/// as a whole (docs/architecture.md → Migrations: one global sequence).
pub fn migration_set() -> Vec<M<'static>> {
    vec![
        M::up(include_str!("../migrations/0001_core_schema.sql")),
        M::up(include_str!("../migrations/0002_seed_countries.sql")),
    ]
}

/// The core's migrations as a runnable set, for this crate's own tests. The app
/// composes `migration_set()` with every module's into the global sequence instead.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(migration_set())
}

/// Open an in-memory database with foreign keys enforced and the CORE migrations
/// applied — enough for testing the core repository in isolation. The app opens
/// its database through the shell's composed runner.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}
