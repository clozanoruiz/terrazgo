// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shutdown contract, through the shell's own opener.
//!
//! SQLite in WAL mode keeps a `-wal` and a `-shm` file beside the database, and
//! deletes them when the last connection closes cleanly. Nothing else does: a
//! checkpoint empties the write-ahead log but leaves both files, and Tauri never
//! drops managed state (the platform event loop ends the process with
//! `std::process::exit`), so the close has to be explicit at `RunEvent::Exit`.
//!
//! `terrazgo_core::db::Database` owns that behaviour and tests it directly. What
//! is tested here is that the APP's database — opened by `db::open_app_db`, with
//! the composed migration sequence applied — actually loses its sidecars and
//! keeps its data, which is the thing that was wrong.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use terrazgo::state::AppState;
use terrazgo_core::db::Database;
use terrazgo_testkit::files::TempFile;

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut file = path.as_os_str().to_owned();
    file.push(suffix);
    PathBuf::from(file)
}

/// The app database as the shell opens it, plus one farm so the write-ahead
/// log has something in it.
fn app_state(path: &Path) -> AppState {
    let conn = terrazgo::db::open_app_db(path).unwrap();
    let schema_version = terrazgo::db::schema_version(&conn).unwrap();
    let db = Database::new(conn).unwrap();
    {
        let mut guard = db.lock().unwrap();
        terrazgo_core::repository::insert_farm(
            guard.conn_mut().unwrap(),
            terrazgo_core::models::NewFarm {
                name: "Finca La Vega".into(),
                owner_name: None,
                owner_tax_id: None,
                country_code: "es".into(),
                es: None,
            },
            None,
        )
        .unwrap();
    }
    AppState {
        db,
        db_path: path.to_path_buf(),
        schema_version,
    }
}

fn farm_names(path: &Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn.prepare("SELECT name FROM farm ORDER BY name").unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap()
}

#[test]
fn closing_the_app_database_deletes_its_wal_sidecars() {
    let file = TempFile::reserve("shutdown-app.db");
    let state = app_state(file.path());

    // open_app_db sets journal_mode = WAL, so both sidecars exist while the
    // app is running. That much is the intended state, not the defect.
    assert!(sidecar(file.path(), "-wal").exists());
    assert!(sidecar(file.path(), "-shm").exists());

    state.db.close().unwrap();

    assert!(
        !sidecar(file.path(), "-wal").exists(),
        "the write-ahead log must not outlive the app"
    );
    assert!(!sidecar(file.path(), "-shm").exists());
}

#[test]
fn the_data_survives_the_close_and_reopens() {
    let file = TempFile::reserve("shutdown-roundtrip.db");
    let state = app_state(file.path());
    state.db.close().unwrap();

    // The write lived in the WAL until the close folded it back in. Reading it
    // from a fresh connection is what proves the close checkpointed rather
    // than discarded.
    assert_eq!(farm_names(file.path()), vec!["Finca La Vega".to_string()]);

    // And the app can open the same file again, migrations and all.
    let reopened = app_state(file.path());
    assert_eq!(reopened.schema_version, state.schema_version);
    reopened.db.close().unwrap();
}

#[test]
fn commands_get_an_error_rather_than_a_panic_after_shutdown() {
    let file = TempFile::reserve("shutdown-after-close.db");
    let state = app_state(file.path());
    state.db.close().unwrap();

    // The window between the shutdown hook and the process exiting is narrow,
    // but a command landing in it must be told the database is closed.
    let guard = state.db.lock().unwrap();
    assert!(matches!(
        guard.conn(),
        Err(terrazgo_core::Unavailable::Closed)
    ));
}
