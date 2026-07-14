// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Migration tests (docs/architecture.md testing strategy #3): every migration must apply cleanly
//! to a fresh database AND on top of the previous version.

use module_cue::db::migrations;
use rusqlite::Connection;

#[test]
fn migration_definitions_are_valid() {
    // rusqlite_migration checks the up/down set is internally consistent.
    migrations()
        .validate()
        .expect("migration set should validate");
}

#[test]
fn applies_cleanly_to_fresh_database() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    // Schema exists and the seed migration populated reference data.
    let countries: i64 = conn
        .query_row("SELECT count(*) FROM country", [], |r| r.get(0))
        .unwrap();
    assert!(countries >= 1, "reference data should be seeded");

    // A representative core table exists and is empty.
    let treatments: i64 = conn
        .query_row("SELECT count(*) FROM treatment_record", [], |r| r.get(0))
        .unwrap();
    assert_eq!(treatments, 0);
}

#[test]
fn applies_cleanly_on_top_of_previous_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    let m = migrations();

    // Stop at v1 (the core's DDL — migrations() composes core steps before cue's):
    // the country table exists, reference data not yet seeded.
    m.to_version(&mut conn, 1).unwrap();
    let countries_v1: i64 = conn
        .query_row("SELECT count(*) FROM country", [], |r| r.get(0))
        .unwrap();
    assert_eq!(countries_v1, 0, "v1 has the schema but no seeds");

    // Upgrade v1 -> latest (applies 0002 on an existing v1 database).
    m.to_latest(&mut conn).unwrap();
    let countries_v2: i64 = conn
        .query_row("SELECT count(*) FROM country", [], |r| r.get(0))
        .unwrap();
    assert!(countries_v2 >= 1, "upgrade should seed reference data");
}

#[test]
fn farm_without_country_is_rejected_by_the_schema() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    // country_code is NOT NULL: the database itself must reject a country-less farm,
    // even for writes that bypass the repository (external scripts, future sync).
    let result = conn.execute(
        "INSERT INTO farm (id, name, created_at, updated_at)
         VALUES ('f1', 'No country', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    );
    assert!(
        result.is_err(),
        "farm without country_code must be rejected by NOT NULL"
    );
}

#[test]
fn duplicate_alert_for_the_same_condition_is_rejected_by_the_schema() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    // UNIQUE (alert_type_code, subject_table, subject_id): the reconciling refresh
    // relies on the database itself guaranteeing one alert per condition.
    let insert = "INSERT INTO alert
                    (id, alert_type_code, subject_table, subject_id, created_at, updated_at)
                  VALUES (?1, 'phi_window', 'treatment_record', 't1',
                          '2026-06-11T00:00:00Z', '2026-06-11T00:00:00Z')";
    conn.execute(insert, ["a1"]).unwrap();
    let duplicate = conn.execute(insert, ["a2"]);
    assert!(
        duplicate.is_err(),
        "second alert for the same condition must violate UNIQUE"
    );
}

#[test]
fn foreign_keys_are_enforced() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrations().to_latest(&mut conn).unwrap();

    // Inserting a plot with a non-existent farm_id must be rejected.
    let result = conn.execute(
        "INSERT INTO plot (id, farm_id, name, created_at, updated_at)
         VALUES ('p1', 'no-such-farm', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    );
    assert!(result.is_err(), "foreign key violation should fail");
}
