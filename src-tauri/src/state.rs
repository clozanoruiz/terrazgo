// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared app state, managed by Tauri and injected into commands.

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

/// State every command can reach via `tauri::State<AppState>`.
///
/// The `Mutex` is mandatory, not ceremony: Tauri runs commands on a thread
/// pool and anything in managed state must be `Send + Sync`, but a SQLite
/// `Connection` is `!Sync` (it must never be used from two threads at once).
/// The mutex serialises all database access through the one connection —
/// exactly right for a single-user desktop app. If a long-running query ever
/// blocks the UI, the upgrade path is a connection pool (r2d2), not removing
/// the lock.
pub struct AppState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
    pub schema_version: usize,
}

/// The geo cache database (`geo-cache.db`), managed separately from
/// [`AppState`]: different file, different lifecycle (derived, re-fetchable,
/// excluded from backups), and a different consumer — the `geo://` protocol
/// handler and the map-style command, not the entity CRUD.
///
/// Same `Mutex<Connection>` reasoning as above. Tile bursts stay parallel
/// because `terrazgo_geo::fetch` never holds this lock across network I/O
/// (its documented performance contract).
pub struct GeoState {
    pub conn: Mutex<Connection>,
}
