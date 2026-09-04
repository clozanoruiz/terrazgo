// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Migration tests (docs/architecture.md testing strategy #3): every migration
//! must apply cleanly to a fresh database AND on top of the previous version.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_ecoscheme::db::migrations;
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

    // RD 1048/2022's six register-level annotation duties (docs/cuaderno-print.md
    // → "The eco-scheme registers this book does not carry" lists them by
    // article: 30.2 ter, 31 + 31.4.d, 45.2, 42, 43, anexo IV).
    let practices: i64 = conn
        .query_row("SELECT count(*) FROM eco_practice", [], |r| r.get(0))
        .unwrap();
    assert_eq!(practices, 6);

    // Fifteen owned kinds over TIPO_LABOR's fourteen codes: model 9.4 prints
    // Siega and Desbrozado as separate columns where the catalogue has one
    // "Desbroce y siega" (see siex.rs).
    let kinds: i64 = conn
        .query_row("SELECT count(*) FROM cultural_operation_kind", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(kinds, 15);
}

#[test]
fn the_registers_that_link_back_to_a_cover_resolve_it_forward() {
    // `grazing_record` and `cultural_operation` declare `soil_cover_id` before
    // `soil_cover` is created, because the registers stay in the model's own
    // order. SQLite resolves a foreign key by name when a row is WRITTEN rather
    // than when the table is declared, so the forward reference is legal — but
    // it fails at insert time if the target is ever renamed or dropped, which
    // no schema test would otherwise catch.
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrations().to_latest(&mut conn).unwrap();

    for (table, parent) in [
        ("soil_cover", None),
        ("soil_cover_plot", Some("soil_cover")),
        ("grazing_record", Some("soil_cover")),
        ("cultural_operation", Some("soil_cover")),
    ] {
        let exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "{table} should exist");

        if let Some(parent) = parent {
            let keys: Vec<String> = conn
                .prepare(&format!("PRAGMA foreign_key_list({table})"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>("table"))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(
                keys.iter().any(|t| t == parent),
                "{table} should reference {parent}, found {keys:?}"
            );
        }
    }

    // The check SQLite itself does, over the whole schema.
    let violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
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
        .query_row("SELECT count(*) FROM eco_practice", [], |r| r.get(0))
        .unwrap();
    assert_eq!(unseeded, 0, "v3 has the schema but no seeds");

    // Upgrade v3 -> latest (applies 0002 on an existing v3 database).
    m.to_latest(&mut conn).unwrap();
    let practices: i64 = conn
        .query_row("SELECT count(*) FROM eco_practice", [], |r| r.get(0))
        .unwrap();
    assert_eq!(practices, 6);
}

#[test]
fn the_module_runs_on_core_alone() {
    // This crate depends on terrazgo-core and on no other module. Its test
    // migration set is core + itself, so the day some code here starts relying
    // on another module's table, this fails instead of passing quietly because
    // the app happens to register all of them.
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    for table in [
        "treatment_record",
        "irrigation_record",
        "fertilisation_record",
    ] {
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            found, 0,
            "module-ecoscheme must not depend on another module's schema ({table})"
        );
    }
}

#[test]
fn every_lookup_row_carries_an_i18n_key_under_its_own_namespace() {
    // No user-facing strings in the schema (docs/architecture.md → i18n): a
    // lookup row carries a key, and the dictionaries carry the words.
    //
    // The namespace half is what nothing else checks. The frontend resolves
    // these through `tCode(prefix, code)`, which builds `<prefix>.<code>` and
    // falls back to printing the raw code when the key is missing — so a row
    // seeded with a key that does not follow its own table's shape would
    // render a machine code in the UI, silently and in every locale.
    let mut conn = Connection::open_in_memory().unwrap();
    migrations().to_latest(&mut conn).unwrap();

    for table in ["eco_practice", "cultural_operation_kind"] {
        let mut stmt = conn
            .prepare(&format!("SELECT code, i18n_key FROM {table}"))
            .unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(!rows.is_empty());
        for (code, key) in rows {
            assert_eq!(
                key,
                format!("{table}.{code}"),
                "{table}.{code} must key its dictionary entry by its own code"
            );
        }
    }
}
