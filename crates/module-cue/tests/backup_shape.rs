// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The module's contribution to the backup shape probe.
//!
//! `terrazgo_core::backup::validate_backup` checks a fingerprint of the
//! squashed schema, because pre-release the migration files are edited in place
//! and `user_version` therefore cannot tell a backup taken before an edit from
//! one taken after. Core owns its own tables; a module's tables reach the probe
//! through `BACKUP_SHAPE`, the same composition the migration runner does — and
//! these tests are what keep that list honest as `0001` keeps changing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TempFile;

use rusqlite::Connection;
use std::path::Path;
use terrazgo_core::backup::{export_backup, validate_backup};
use terrazgo_core::error::CoreError;

/// A database at the composed (core + module) schema, i.e. what the app runs.
fn composed_db(path: &Path) -> Connection {
    module_cue::open(path).unwrap()
}

fn user_version(conn: &Connection) -> i64 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn a_fresh_composed_backup_satisfies_the_module_shape() {
    let source_path = TempFile::reserve("fresh-src.db");
    let dest = TempFile::reserve("fresh-dest.db");
    let conn = composed_db(source_path.path());
    let current = user_version(&conn);

    export_backup(&conn, dest.path()).unwrap();
    let info = validate_backup(dest.path(), current, module_cue::BACKUP_SHAPE).unwrap();
    assert_eq!(info.schema_version, current);

    drop(conn);
}

/// The file this slice's `0001` edit creates: same `user_version`, missing the
/// interval and total-quantity columns of Anexo III Parte I B. Left unchecked
/// it would import "successfully" and then fail with `no such column` on the
/// first treatment query, far from the cause.
#[test]
fn a_backup_taken_before_the_interval_and_total_columns_is_refused() {
    for column in [
        "application_end_date",
        "total_quantity_value",
        "total_quantity_unit_code",
    ] {
        let source_path = TempFile::reserve(&format!("stale-src-{column}.db"));
        let dest = TempFile::reserve(&format!("stale-dest-{column}.db"));
        let conn = composed_db(source_path.path());
        let current = user_version(&conn);
        export_backup(&conn, dest.path()).unwrap();

        let copy = Connection::open(dest.path()).unwrap();
        copy.execute(
            &format!("ALTER TABLE treatment_record DROP COLUMN {column}"),
            [],
        )
        .unwrap();
        drop(copy);

        assert!(
            matches!(
                validate_backup(dest.path(), current, module_cue::BACKUP_SHAPE),
                Err(CoreError::Invalid("backup_invalid"))
            ),
            "a same-version backup without treatment_record.{column} must be refused"
        );

        drop(conn);
    }
}

/// Without the module's contribution the same stale file passes: core alone
/// cannot see a module's tables. This is what the `module_shape` argument buys,
/// and why every module schema edit has to extend `BACKUP_SHAPE`.
#[test]
fn the_core_fingerprint_alone_cannot_catch_a_module_table() {
    let source_path = TempFile::reserve("core-only-src.db");
    let dest = TempFile::reserve("core-only-dest.db");
    let conn = composed_db(source_path.path());
    let current = user_version(&conn);
    export_backup(&conn, dest.path()).unwrap();

    let copy = Connection::open(dest.path()).unwrap();
    copy.execute(
        "ALTER TABLE treatment_record DROP COLUMN application_end_date",
        [],
    )
    .unwrap();
    drop(copy);

    assert!(validate_backup(dest.path(), current, &[]).is_ok());
    assert!(validate_backup(dest.path(), current, module_cue::BACKUP_SHAPE).is_err());

    drop(conn);
}
