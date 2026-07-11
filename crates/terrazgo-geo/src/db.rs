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

/// Ceiling for cached tile payload bytes — the knob `enforce_tile_cache_cap`
/// enforces. 512 MiB holds full-depth orthophoto for a farm plus generous
/// base-map browsing; revisit at the mobile milestone, where device storage
/// is the real constraint, and promote to a user setting once the core
/// settings module exists.
pub const TILE_CACHE_MAX_BYTES: i64 = 512 * 1024 * 1024;

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!(
        "../migrations/0001_cache_schema.sql"
    ))])
}

/// Open (creating if needed) and migrate the cache database.
///
/// Pre-release the schema file is squashed in place (the same regime as the
/// app database), which leaves already-deployed caches at the same
/// `user_version` with an older shape — undetectable by the migration
/// runner. Unlike the app database, this one is derived and disposable, so
/// the guard is recreation: if the cache fails to open, migrate, or match
/// the current schema, delete the file (and WAL sidecars) and start fresh.
/// The cost is a cold cache, nothing else.
pub fn open_cache(path: &Path) -> Result<Connection> {
    match try_open(path) {
        Ok(conn) => Ok(conn),
        Err(_) => {
            for suffix in ["", "-wal", "-shm"] {
                let mut file = path.as_os_str().to_owned();
                file.push(suffix);
                let _ = std::fs::remove_file(std::path::Path::new(&file));
            }
            try_open(path)
        }
    }
}

fn try_open(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    apply_pragmas_and_migrate(&mut conn)?;
    // Schema probe: preparing against the newest column detects a stale
    // pre-release cache (same user_version, older squashed schema).
    conn.prepare("SELECT last_used_at FROM tile LIMIT 0")?;
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

/// Evict least-recently-used tiles until their payloads fit `max_bytes`,
/// then reclaim the file space (`VACUUM`) if anything was deleted. Returns
/// the number of evicted rows.
///
/// Scope is deliberately the `tile` table only. `resource` rows stay
/// uncapped: styles/glyphs/sprites are small and bounded, and the SIGPAC
/// lookup/zone responses in there are what keeps a verified plot verifiable
/// offline — silently evicting those would break that promise for kilobytes
/// of savings. Tiles are pure display data; the map redraws them on the
/// next online visit.
///
/// Runs off the startup path (a `VACUUM` on a maxed-out cache takes
/// seconds); callers hold the cache lock, so tile serving simply waits.
pub fn enforce_tile_cache_cap(conn: &Connection, max_bytes: i64) -> Result<usize> {
    // Newest-first running total; everything past the cap goes. The row that
    // crosses the boundary is evicted too — under, never over, the cap.
    let evicted = conn.execute(
        "DELETE FROM tile WHERE rowid IN (
             SELECT rowid FROM (
                 SELECT rowid, SUM(LENGTH(data)) OVER (
                     ORDER BY last_used_at DESC, rowid DESC
                 ) AS cumulative FROM tile
             ) WHERE cumulative > ?1
         )",
        [max_bytes],
    )?;
    if evicted > 0 {
        // DELETE alone only frees pages inside the file; VACUUM returns the
        // space to the filesystem, which is the point of the cap.
        conn.execute_batch("VACUUM")?;
    }
    Ok(evicted)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn seed_tile(conn: &Connection, source: &str, z: u8, bytes: usize, used_at: &str) {
        conn.execute(
            "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at)
             VALUES (?1, ?2, 0, 0, ?3, 'image/jpeg', ?4, ?4)",
            rusqlite::params![source, z, vec![0u8; bytes], used_at],
        )
        .unwrap();
    }

    #[test]
    fn cap_evicts_least_recently_used_first_and_only_past_the_cap() {
        let conn = open_cache_in_memory().unwrap();
        seed_tile(&conn, "pnoa", 10, 100, "2026-07-01T00:00:00Z"); // oldest
        seed_tile(&conn, "pnoa", 11, 100, "2026-07-05T00:00:00Z");
        seed_tile(&conn, "pnoa", 12, 100, "2026-07-10T00:00:00Z"); // newest

        // 250-byte cap: the two newest (200 bytes) fit; the oldest crosses it.
        assert_eq!(enforce_tile_cache_cap(&conn, 250).unwrap(), 1);
        let zooms: Vec<u8> = conn
            .prepare("SELECT z FROM tile ORDER BY z")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(zooms, vec![11, 12], "oldest-used tile evicted first");

        // Under the cap: a no-op.
        assert_eq!(enforce_tile_cache_cap(&conn, 250).unwrap(), 0);
    }

    #[test]
    fn stale_pre_release_cache_is_recreated_on_open() {
        // A deployed cache whose 0001 predates the squash: same
        // user_version, no last_used_at column.
        let dir = std::env::temp_dir().join(format!("terrazgo-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("geo-cache.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE tile (
                     source TEXT NOT NULL, z INTEGER NOT NULL,
                     x INTEGER NOT NULL, y INTEGER NOT NULL,
                     data BLOB NOT NULL, content_type TEXT NOT NULL,
                     fetched_at TEXT NOT NULL,
                     PRIMARY KEY (source, z, x, y));
                 CREATE TABLE resource (
                     key TEXT PRIMARY KEY, data BLOB NOT NULL,
                     content_type TEXT NOT NULL, fetched_at TEXT NOT NULL);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }

        // open_cache detects the shape mismatch, recreates, and the fresh
        // cache has the current schema (probe column present, usable).
        let conn = open_cache(&path).unwrap();
        conn.execute(
            "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at)
             VALUES ('pnoa', 13, 1, 1, x'FF', 'image/jpeg',
                     '2026-07-11T00:00:00Z', '2026-07-11T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
