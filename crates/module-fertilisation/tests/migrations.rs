// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Migration tests (docs/architecture.md testing strategy #3): every migration
//! must apply cleanly to a fresh database AND on top of the previous version.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_fertilisation::db::migrations;
use rusqlite::Connection;

#[test]
fn migration_definitions_are_valid() {
    migrations()
        .validate()
        .expect("migration set should validate");
}

#[test]
fn applies_cleanly_to_fresh_database() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    let records: i64 = conn
        .query_row("SELECT count(*) FROM irrigation_record", [], |r| r.get(0))
        .unwrap();
    assert_eq!(records, 0);

    // Both lookups seeded, at their catalogue sizes.
    let methods: i64 = conn
        .query_row("SELECT count(*) FROM irrigation_method", [], |r| r.get(0))
        .unwrap();
    assert_eq!(methods, 8, "SIST_RIEGO has eight values");
    let origins: i64 = conn
        .query_row("SELECT count(*) FROM water_origin", [], |r| r.get(0))
        .unwrap();
    assert_eq!(origins, 6, "ORIGEN_AGUA_RIEGO has six values");
}

#[test]
fn applies_cleanly_on_top_of_previous_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    let m = migrations();

    // v3 is this module's own DDL: core's two steps run first in the test
    // composition, then 0001, then 0002. So at v3 the tables exist and the
    // lookups are not yet seeded.
    m.to_version(&mut conn, 3).unwrap();
    let unseeded: i64 = conn
        .query_row("SELECT count(*) FROM irrigation_method", [], |r| r.get(0))
        .unwrap();
    assert_eq!(unseeded, 0, "v3 has the schema but no seeds");

    // Upgrade v3 -> latest (applies 0002 on an existing v3 database).
    m.to_latest(&mut conn).unwrap();
    let methods: i64 = conn
        .query_row("SELECT count(*) FROM irrigation_method", [], |r| r.get(0))
        .unwrap();
    assert_eq!(methods, 8);
}

#[test]
fn the_module_runs_on_core_alone() {
    // This crate depends on terrazgo-core and on no other module. Its test
    // migration set is core + itself, so the day some code here starts
    // relying on a module-cue table, this fails instead of passing quietly
    // because the app happens to register both.
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    let treatment_tables: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type = 'table' AND name = 'treatment_record'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        treatment_tables, 0,
        "module-fertilisation must not depend on module-cue's schema"
    );
}

#[test]
fn the_volume_unit_foreign_key_resolves_against_core() {
    // `unit` moved into core on 2026-08-07 precisely so this key could exist:
    // a module may never reference another module's table, and both modules
    // record amounts.
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    let m3_ha: i64 = conn
        .query_row("SELECT count(*) FROM unit WHERE code = 'm3_ha'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(m3_ha, 1);
}
