// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Backup export/import-validation tests (docs/architecture.md testing strategy #1:
//! regulatory records must survive a lost device, so this is compliance
//! logic — written test-first from the requirements).
//!
//! Requirements pinned here:
//!   * an export taken while the app runs is a consistent, self-contained
//!     snapshot: it passes integrity_check, carries the same schema version
//!     and the same data;
//!   * exporting over an existing file replaces it (the save dialog already
//!     confirmed the overwrite);
//!   * import validation rejects files that are not Terrazgo backups and
//!     backups from a NEWER schema than the app supports (downgrades lose
//!     data); OLDER backups are accepted — reopening migrates them forward.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::backup::{export_backup, validate_backup};
use terrazgo_core::models::NewFarm;
use terrazgo_core::repository as repo;

/// Unique per-test temp path; tests clean up behind themselves.
fn temp_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "terrazgo-backup-test-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A migrated file-based database with one farm in it.
fn seeded_db(path: &PathBuf) -> Connection {
    let mut conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    terrazgo_core::migrations().to_latest(&mut conn).unwrap();
    repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    conn
}

fn user_version(conn: &Connection) -> i64 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn export_produces_a_consistent_snapshot_with_data_and_version() {
    let source_path = temp_path("export-src.db");
    let dest = temp_path("export-dest.db");
    let conn = seeded_db(&source_path);

    let summary = export_backup(&conn, &dest).unwrap();

    // The snapshot opens on its own (no -wal/-shm sidecars needed) and is intact.
    let copy = Connection::open(&dest).unwrap();
    let integrity: String = copy
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");

    // Same schema version and same data as the live database.
    assert_eq!(user_version(&copy), user_version(&conn));
    let farms: i64 = copy
        .query_row("SELECT COUNT(*) FROM farm", [], |r| r.get(0))
        .unwrap();
    assert_eq!(farms, 1);

    // The summary reports what the UI shows.
    assert_eq!(summary.schema_version, user_version(&conn));
    assert_eq!(summary.size_bytes, std::fs::metadata(&dest).unwrap().len());
    assert!(summary.size_bytes > 0);

    drop(copy);
    std::fs::remove_file(&source_path).unwrap();
    std::fs::remove_file(&dest).unwrap();
}

#[test]
fn export_replaces_an_existing_destination_file() {
    let source_path = temp_path("overwrite-src.db");
    let dest = temp_path("overwrite-dest.db");
    let conn = seeded_db(&source_path);
    std::fs::write(&dest, b"stale previous backup").unwrap();

    export_backup(&conn, &dest).unwrap();

    let copy = Connection::open(&dest).unwrap();
    let farms: i64 = copy
        .query_row("SELECT COUNT(*) FROM farm", [], |r| r.get(0))
        .unwrap();
    assert_eq!(farms, 1);

    drop(copy);
    std::fs::remove_file(&source_path).unwrap();
    std::fs::remove_file(&dest).unwrap();
}

#[test]
fn validate_accepts_a_fresh_export_and_reports_its_version() {
    let source_path = temp_path("validate-src.db");
    let dest = temp_path("validate-dest.db");
    let conn = seeded_db(&source_path);
    let current = user_version(&conn);

    export_backup(&conn, &dest).unwrap();
    let info = validate_backup(&dest, current).unwrap();
    assert_eq!(info.schema_version, current);

    // An OLDER backup (lower user_version) is also accepted: reopening the
    // swapped file runs the migration runner, which brings it forward.
    let copy = Connection::open(&dest).unwrap();
    copy.pragma_update(None, "user_version", current - 1)
        .unwrap();
    drop(copy);
    let info = validate_backup(&dest, current).unwrap();
    assert_eq!(info.schema_version, current - 1);

    std::fs::remove_file(&source_path).unwrap();
    std::fs::remove_file(&dest).unwrap();
}

#[test]
fn validate_rejects_a_backup_from_a_newer_schema() {
    let source_path = temp_path("newer-src.db");
    let dest = temp_path("newer-dest.db");
    let conn = seeded_db(&source_path);
    let current = user_version(&conn);

    export_backup(&conn, &dest).unwrap();
    let copy = Connection::open(&dest).unwrap();
    copy.pragma_update(None, "user_version", current + 1)
        .unwrap();
    drop(copy);

    let result = validate_backup(&dest, current);
    assert!(
        matches!(result, Err(CoreError::Invalid("backup_newer_schema"))),
        "importing a newer-schema backup would downgrade and lose data"
    );

    std::fs::remove_file(&source_path).unwrap();
    std::fs::remove_file(&dest).unwrap();
}

#[test]
fn validate_rejects_files_that_are_not_terrazgo_backups() {
    // Garbage bytes: not SQLite at all.
    let garbage = temp_path("garbage.bin");
    std::fs::write(&garbage, b"definitely not a database").unwrap();
    assert!(matches!(
        validate_backup(&garbage, 99),
        Err(CoreError::Invalid("backup_invalid"))
    ));
    std::fs::remove_file(&garbage).unwrap();

    // A valid but empty SQLite file (user_version 0): not created by Terrazgo.
    let empty = temp_path("empty.db");
    Connection::open(&empty)
        .unwrap()
        .execute_batch("CREATE TABLE t (x)")
        .unwrap();
    assert!(matches!(
        validate_backup(&empty, 99),
        Err(CoreError::Invalid("backup_invalid"))
    ));
    std::fs::remove_file(&empty).unwrap();

    // A missing file.
    let missing = temp_path("missing.db");
    assert!(validate_backup(&missing, 99).is_err());
}
