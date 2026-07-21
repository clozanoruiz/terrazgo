// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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

/// Device-local app settings (`settings.json` in the app data dir), loaded
/// once at startup. The mutex guards the in-memory copy commands read; the
/// file is the durable one, and every change writes the file first (via
/// `terrazgo_core::settings::save_settings`, atomic) and the copy second.
/// Deliberately not in any database: different lifecycle from farm data —
/// no audit trail, no sync, excluded from backups.
pub struct SettingsState {
    pub settings: Mutex<terrazgo_core::settings::AppSettings>,
    pub path: PathBuf,
}

/// Marker managed as the LAST statement of the setup hook. Exists for the
/// `app_ready` command: on Android the webview loads in parallel with setup,
/// so the frontend can invoke commands before `.manage()` has run — any
/// command taking `State<...>` then fails with Tauri's raw "state not
/// managed" error. The frontend polls `app_ready` (which has no `State`
/// parameter, so it works at any time) before mounting the app. Desktop
/// never races — its window is created only after setup returns — so the
/// first poll answers `true` there.
pub struct SetupComplete;
