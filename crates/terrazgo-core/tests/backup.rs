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
//!     data); OLDER backups are accepted — reopening migrates them forward;
//!   * the pre-import safety copies are pruned to a bounded number, oldest
//!     first, and nothing else in the directory is touched.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{TempDir, TempFile};

use std::path::Path;

use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::backup::{
    KEEP_PRE_IMPORT_COPIES, PRE_IMPORT_PREFIX, export_backup, prune_pre_import_copies,
    validate_backup,
};
use terrazgo_core::models::NewFarm;
use terrazgo_core::repository as repo;

/// A migrated file-based database with one farm in it.
fn seeded_db(path: &Path) -> Connection {
    let mut conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    terrazgo_core::migrations().to_latest(&mut conn).unwrap();
    terrazgo_core::sync::install_device(&conn, &terrazgo_core::sync::mint_device_id()).unwrap();
    terrazgo_core::sync::install_shape(&conn, &[terrazgo_core::sync::CORE_SYNC_SHAPE]).unwrap();
    repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
            ..NewFarm::default()
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
    let source_path = TempFile::reserve("export-src.db");
    let dest = TempFile::reserve("export-dest.db");
    let conn = seeded_db(source_path.path());

    let summary = export_backup(&conn, dest.path()).unwrap();

    // The snapshot opens on its own (no -wal/-shm sidecars needed) and is intact.
    let copy = Connection::open(dest.path()).unwrap();
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
    assert_eq!(
        summary.size_bytes,
        std::fs::metadata(dest.path()).unwrap().len()
    );
    assert!(summary.size_bytes > 0);

    drop(copy);
}

#[test]
fn a_snapshot_carries_the_log_but_never_the_device_that_wrote_it() {
    // The device identity is connection state on purpose (docs/sync.md →
    // Device identity): a backup restored on another machine must not let that
    // machine write under this one's name. The log rows, which say who wrote
    // what, travel with the data as they must.
    let source_path = TempFile::reserve("export-identity-src.db");
    let dest = TempFile::reserve("export-identity-dest.db");
    let conn = seeded_db(source_path.path());
    let device = terrazgo_core::sync::installed_device(&conn).unwrap();

    export_backup(&conn, dest.path()).unwrap();

    let copy = Connection::open(dest.path()).unwrap();
    assert!(matches!(
        terrazgo_core::sync::installed_device(&copy),
        Err(CoreError::Stamp("no_device_identity"))
    ));
    let logged_by_source: i64 = copy
        .query_row(
            "SELECT COUNT(*) FROM record_change WHERE origin_device = ?1",
            [&device],
            |r| r.get(0),
        )
        .unwrap();
    assert!(logged_by_source > 0);
}

#[test]
fn export_replaces_an_existing_destination_file() {
    let source_path = TempFile::reserve("overwrite-src.db");
    let dest = TempFile::reserve("overwrite-dest.db");
    let conn = seeded_db(source_path.path());
    std::fs::write(dest.path(), b"stale previous backup").unwrap();

    export_backup(&conn, dest.path()).unwrap();

    let copy = Connection::open(dest.path()).unwrap();
    let farms: i64 = copy
        .query_row("SELECT COUNT(*) FROM farm", [], |r| r.get(0))
        .unwrap();
    assert_eq!(farms, 1);

    drop(copy);
}

#[test]
fn validate_accepts_a_fresh_export_and_reports_its_version() {
    let source_path = TempFile::reserve("validate-src.db");
    let dest = TempFile::reserve("validate-dest.db");
    let conn = seeded_db(source_path.path());
    let current = user_version(&conn);

    export_backup(&conn, dest.path()).unwrap();
    let info = validate_backup(dest.path(), current, &[]).unwrap();
    assert_eq!(info.schema_version, current);

    // An OLDER backup (lower user_version) is also accepted: reopening the
    // swapped file runs the migration runner, which brings it forward.
    let copy = Connection::open(dest.path()).unwrap();
    copy.pragma_update(None, "user_version", current - 1)
        .unwrap();
    drop(copy);
    let info = validate_backup(dest.path(), current, &[]).unwrap();
    assert_eq!(info.schema_version, current - 1);
}

#[test]
fn validate_rejects_a_backup_from_a_newer_schema() {
    let source_path = TempFile::reserve("newer-src.db");
    let dest = TempFile::reserve("newer-dest.db");
    let conn = seeded_db(source_path.path());
    let current = user_version(&conn);

    export_backup(&conn, dest.path()).unwrap();
    let copy = Connection::open(dest.path()).unwrap();
    copy.pragma_update(None, "user_version", current + 1)
        .unwrap();
    drop(copy);

    let result = validate_backup(dest.path(), current, &[]);
    assert!(
        matches!(result, Err(CoreError::Invalid("backup_newer_schema"))),
        "importing a newer-schema backup would downgrade and lose data"
    );
}

#[test]
fn validate_rejects_files_that_are_not_terrazgo_backups() {
    // Garbage bytes: not SQLite at all.
    let garbage = TempFile::reserve("garbage.bin");
    std::fs::write(garbage.path(), b"definitely not a database").unwrap();
    assert!(matches!(
        validate_backup(garbage.path(), 99, &[]),
        Err(CoreError::Invalid("backup_invalid"))
    ));

    // A valid but empty SQLite file (user_version 0): not created by Terrazgo.
    let empty = TempFile::reserve("empty.db");
    Connection::open(empty.path())
        .unwrap()
        .execute_batch("CREATE TABLE t (x)")
        .unwrap();
    assert!(matches!(
        validate_backup(empty.path(), 99, &[]),
        Err(CoreError::Invalid("backup_invalid"))
    ));

    // A missing file.
    let missing = TempFile::reserve("missing.db");
    assert!(validate_backup(missing.path(), 99, &[]).is_err());
}

/// The reason the shape probe exists (docs/cuaderno-print.md → Capture design):
/// while the project is pre-release, migration files are edited in place, which
/// adds columns WITHOUT bumping the migration count. `user_version` alone
/// therefore cannot tell a backup taken before an edit from one taken after —
/// the stale file would import "successfully" and then fail with `no such
/// column` on the first query, far from the cause. Comparing the shape rejects
/// it at the door instead.
#[test]
fn validate_rejects_a_current_version_backup_missing_columns() {
    let source_path = TempFile::reserve("stale-shape-src.db");
    let dest = TempFile::reserve("stale-shape-dest.db");
    let conn = seeded_db(source_path.path());
    let current = user_version(&conn);
    export_backup(&conn, dest.path()).unwrap();

    // A fresh export passes.
    assert!(validate_backup(dest.path(), current, &[]).is_ok());

    // Simulate the backup of an older squashed schema: same user_version, one
    // column short. (Dropping is how a real pre-edit file differs from this one.)
    let copy = Connection::open(dest.path()).unwrap();
    copy.execute("ALTER TABLE operator DROP COLUMN tax_id", [])
        .unwrap();
    drop(copy);

    let result = validate_backup(dest.path(), current, &[]);
    assert!(
        matches!(result, Err(CoreError::Invalid("backup_invalid"))),
        "a same-version backup with an older shape must be refused, not imported"
    );
}

/// Every pre-release schema edit must join the fingerprint, or the probe stops
/// catching the file it was written for. This pins the crop provenance columns
/// added for the SIGPAC declared-crops import.
#[test]
fn validate_rejects_a_backup_taken_before_the_crop_provenance_columns() {
    let source_path = TempFile::reserve("stale-crop-src.db");
    let dest = TempFile::reserve("stale-crop-dest.db");
    let conn = seeded_db(source_path.path());
    let current = user_version(&conn);
    export_backup(&conn, dest.path()).unwrap();

    let copy = Connection::open(dest.path()).unwrap();
    copy.execute("ALTER TABLE crop DROP COLUMN source", [])
        .unwrap();
    drop(copy);

    assert!(matches!(
        validate_backup(dest.path(), current, &[]),
        Err(CoreError::Invalid("backup_invalid"))
    ));
}

/// The shape probe applies only at the current version. An OLDER backup is
/// exempt on purpose — it is migrated forward on reopen, which is exactly what
/// makes it importable.
#[test]
fn an_older_backup_is_not_shape_checked() {
    let source_path = TempFile::reserve("older-shape-src.db");
    let dest = TempFile::reserve("older-shape-dest.db");
    let conn = seeded_db(source_path.path());
    let current = user_version(&conn);
    export_backup(&conn, dest.path()).unwrap();

    let copy = Connection::open(dest.path()).unwrap();
    copy.execute("ALTER TABLE operator DROP COLUMN tax_id", [])
        .unwrap();
    drop(copy);

    // Same file, but the app now knows a later version: the backup is "older".
    let info =
        validate_backup(dest.path(), current + 1, &[]).expect("older backups migrate forward");
    assert_eq!(info.schema_version, current);
}

// --- pre-import safety copies -------------------------------------------
//
// The copies are full databases and an import is routine once a phone mirrors
// onto a desktop, so the directory has to be bounded. Ordering is by filename
// because the stamp inside it is an ISO instant: lexicographic order IS
// chronological order, and it survives a copy that resets the mtime.

/// A pre-import copy name for a given instant stamp, in the shape the shell
/// writes: `pre-import-20260702T101500Z.db`.
fn copy_name(stamp: &str) -> String {
    format!("{PRE_IMPORT_PREFIX}{stamp}.db")
}

#[test]
fn pruning_removes_the_oldest_copies_and_keeps_the_newest() {
    let dir = TempDir::create("prune-oldest");
    for stamp in [
        "20260701T090000Z",
        "20260702T090000Z",
        "20260703T090000Z",
        "20260704T090000Z",
        "20260705T090000Z",
    ] {
        dir.write(&copy_name(stamp), b"a database");
    }

    let removed = prune_pre_import_copies(dir.path(), 3).unwrap();

    assert_eq!(removed, 2);
    assert_eq!(
        dir.names(),
        vec![
            copy_name("20260703T090000Z"),
            copy_name("20260704T090000Z"),
            copy_name("20260705T090000Z"),
        ],
        "the three most recent survive, the two oldest go"
    );
}

#[test]
fn pruning_a_directory_already_within_the_limit_removes_nothing() {
    let dir = TempDir::create("prune-under-limit");
    dir.write(&copy_name("20260701T090000Z"), b"a database");
    dir.write(&copy_name("20260702T090000Z"), b"a database");

    assert_eq!(
        prune_pre_import_copies(dir.path(), KEEP_PRE_IMPORT_COPIES).unwrap(),
        0
    );
    assert_eq!(dir.names().len(), 2);
}

#[test]
fn pruning_never_touches_anything_but_pre_import_copies() {
    let dir = TempDir::create("prune-foreign-files");
    // Enough copies to force a removal, so the foreign files are exposed to a
    // prune that is actually deleting rather than one that returns early.
    for stamp in ["20260701T090000Z", "20260702T090000Z", "20260703T090000Z"] {
        dir.write(&copy_name(stamp), b"a database");
    }
    dir.write("terrazgo.db", b"not a safety copy");
    dir.write("holiday-photo.jpg", b"definitely not");
    dir.write("pre-import.db", b"close, but carries no stamp");

    let removed = prune_pre_import_copies(dir.path(), 1).unwrap();

    assert_eq!(removed, 2);
    assert_eq!(
        dir.names(),
        vec![
            "holiday-photo.jpg".to_string(),
            copy_name("20260703T090000Z"),
            "pre-import.db".to_string(),
            "terrazgo.db".to_string(),
        ],
        "only the two oldest stamped copies were removed"
    );
}

#[test]
fn pruning_a_directory_that_does_not_exist_is_not_an_error() {
    let dir = TempDir::create("prune-missing");
    let missing = dir.path().join("backups");

    // The first import is what creates the directory, so a prune before it has
    // nothing to do and must not fail the import that called it.
    assert_eq!(
        prune_pre_import_copies(&missing, KEEP_PRE_IMPORT_COPIES).unwrap(),
        0
    );
}

// ---------------------------------------------------------------------------
// What an import would discard
// ---------------------------------------------------------------------------
//
// An import is a MIRROR, not a merge: it replaces everything. The screen
// therefore asks what this book holds that the file does not, and these pin the
// number it is given — a warning that counts wrong is worse than a generic one.

/// Rename the farm `seeded_db` made, which is one change set per call.
fn edit(conn: &mut Connection) {
    let farm = repo::list_farms(conn).unwrap().remove(0);
    repo::update_farm(
        conn,
        &farm.id,
        terrazgo_core::models::UpdateFarm {
            name: format!("Finca {}", terrazgo_core::date::now_ms()),
            owner_name: None,
            owner_tax_id: None,
            location_text: None,
            address: None,
            postal_code: None,
            phone_fixed: None,
            phone_mobile: None,
            email: None,
            opened_on: None,
            latitude: None,
            longitude: None,
            es: None,
            representative: None,
        },
        None,
    )
    .unwrap();
}

#[test]
fn a_backup_of_this_book_discards_nothing() {
    let dir = TempDir::create("discard-none");
    let live = dir.path().join("live.db");
    let conn = seeded_db(&live);
    let snapshot = dir.path().join("snapshot.db");
    export_backup(&conn, &snapshot).unwrap();

    let device = terrazgo_core::sync::installed_device(&conn).unwrap();
    let discarded = terrazgo_core::backup::discarded_by_import(&conn, &snapshot, &device)
        .unwrap()
        .expect("both sides carry the change stamp");
    assert_eq!(
        (discarded.change_sets, discarded.own_change_sets),
        (0, 0),
        "a snapshot of this book holds everything this book holds"
    );
}

#[test]
fn changes_made_after_the_snapshot_are_counted_and_attributed() {
    let dir = TempDir::create("discard-some");
    let live = dir.path().join("live.db");
    let mut conn = seeded_db(&live);
    let snapshot = dir.path().join("snapshot.db");
    export_backup(&conn, &snapshot).unwrap();

    // Two writes here, and one that arrived from somewhere else — the
    // distinction the warning turns on: a change set written on another device
    // comes back at the next sync, one written here exists nowhere else.
    edit(&mut conn);
    edit(&mut conn);
    let device = terrazgo_core::sync::installed_device(&conn).unwrap();
    conn.execute(
        "INSERT INTO record_change
           (id, entity_table, entity_id, operation, changed_at, payload,
            root_table, root_id, origin_device, origin_seq, version_vector, hlc)
         VALUES ('from-a-peer', 'farm', 'f1', 'update', '2026-09-22T10:00:00Z', '{}',
                 'farm', 'f1', '0192f3a4-0000-7000-8000-0000000000b2', 7, '{}', 99)",
        [],
    )
    .unwrap();

    let discarded = terrazgo_core::backup::discarded_by_import(&conn, &snapshot, &device)
        .unwrap()
        .unwrap();
    assert_eq!(
        discarded.change_sets, 3,
        "two written here and one received"
    );
    assert_eq!(
        discarded.own_change_sets, 2,
        "only the ones this device wrote are on no other device"
    );
}

#[test]
fn a_newer_backup_discards_nothing_from_an_older_book() {
    // The direction that matters for a restore: the file holds everything the
    // live book does and more, so importing it takes nothing away.
    let dir = TempDir::create("discard-newer");
    let live = dir.path().join("live.db");
    let mut conn = seeded_db(&live);
    let early = dir.path().join("early.db");
    export_backup(&conn, &early).unwrap();
    edit(&mut conn);
    let late = dir.path().join("late.db");
    export_backup(&conn, &late).unwrap();

    let device = terrazgo_core::sync::installed_device(&conn).unwrap();
    let discarded = terrazgo_core::backup::discarded_by_import(&conn, &late, &device)
        .unwrap()
        .unwrap();
    assert_eq!(discarded.change_sets, 0);
    // And the older one, against the same book, does discard the later edit.
    let older = terrazgo_core::backup::discarded_by_import(&conn, &early, &device)
        .unwrap()
        .unwrap();
    assert_eq!(older.change_sets, 1);
}

#[test]
fn a_backup_too_old_to_count_against_says_so_rather_than_guessing() {
    // A log written before the change stamp existed has no change sets to
    // compare. The screen falls back to the plain warning, which is what it
    // always said — and never to a number it cannot know.
    let dir = TempDir::create("discard-unknown");
    let live = dir.path().join("live.db");
    let conn = seeded_db(&live);

    // A log of the old shape. Built rather than derived, because the column
    // cannot be dropped from a real snapshot — the UNIQUE that names a change
    // set is built on it, which is itself the point: this shape is from before
    // any of that existed.
    let snapshot = dir.path().join("old.db");
    let old = Connection::open(&snapshot).unwrap();
    old.execute_batch(
        "CREATE TABLE record_change (id TEXT PRIMARY KEY, entity_table TEXT, payload TEXT);",
    )
    .unwrap();
    drop(old);

    let device = terrazgo_core::sync::installed_device(&conn).unwrap();
    assert_eq!(
        terrazgo_core::backup::discarded_by_import(&conn, &snapshot, &device).unwrap(),
        None
    );
}
