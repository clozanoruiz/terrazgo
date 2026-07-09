// SPDX-License-Identifier: AGPL-3.0-or-later

//! The geo cache database (`geo-cache.db`) — its own file, its own (tiny)
//! migration sequence, deliberately NOT part of the app database or the
//! composed global migration runner: everything in it is derived and
//! re-fetchable, so it must never bloat `VACUUM INTO` backups or the
//! `record_change` log. Deleting the file loses nothing but warm caches.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use std::path::Path;

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!(
        "../migrations/0001_cache_schema.sql"
    ))])
}

/// Open (creating if needed) and migrate the cache database.
pub fn open_cache(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    apply_pragmas_and_migrate(&mut conn)?;
    Ok(conn)
}

/// In-memory cache database for tests.
pub fn open_cache_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    apply_pragmas_and_migrate(&mut conn)?;
    Ok(conn)
}

fn apply_pragmas_and_migrate(conn: &mut Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "wal")?;
    migrations().to_latest(conn)?;
    Ok(())
}
