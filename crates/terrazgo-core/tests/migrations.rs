// SPDX-License-Identifier: AGPL-3.0-or-later

//! Migration tests for the CORE-owned steps in isolation (docs/architecture.md
//! testing strategy #3): apply cleanly to a fresh database AND on top of the previous
//! version. The composed global sequence is tested in src-tauri.

use rusqlite::Connection;
use terrazgo_core::db::migrations;

#[test]
fn migration_definitions_are_valid() {
    migrations()
        .validate()
        .expect("core migration set should validate");
}

#[test]
fn applies_cleanly_to_fresh_database() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    let countries: i64 = conn
        .query_row("SELECT count(*) FROM country", [], |r| r.get(0))
        .unwrap();
    assert!(countries >= 1, "country reference data should be seeded");

    let farms: i64 = conn
        .query_row("SELECT count(*) FROM farm", [], |r| r.get(0))
        .unwrap();
    assert_eq!(farms, 0);
}

#[test]
fn applies_cleanly_on_top_of_previous_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    let m = migrations();

    // Stop at v1 (core DDL only): tables exist, seed data not yet applied.
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
fn foreign_keys_are_enforced() {
    let conn = terrazgo_core::open_in_memory().unwrap();

    // Inserting a plot with a non-existent farm_id must be rejected.
    let result = conn.execute(
        "INSERT INTO plot (id, farm_id, name, created_at, updated_at)
         VALUES ('p1', 'no-such-farm', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    );
    assert!(result.is_err(), "foreign key violation should fail");
}
