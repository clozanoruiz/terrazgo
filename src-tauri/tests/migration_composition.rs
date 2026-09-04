// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tests for the core's composed migration runner and registry — the "single
//! global version sequence" contract (docs/architecture.md → Migrations: one global
//! sequence; testing strategy #3), plus the demo seeding the shell exposes.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo::db::{composed_migrations, core_migrations};
use terrazgo::registry::registered_modules;

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn fresh_migrated_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

#[test]
fn composed_migration_definitions_are_valid() {
    composed_migrations()
        .validate()
        .expect("composed migration set should validate");
}

#[test]
fn the_record_book_composes_the_same_schema_the_shell_does() {
    // `terrazgo_recordbook::db::migrations()` hand-repeats the registry: core's
    // steps followed by each contributing module's. Its doc comment has claimed
    // since 2026-08-07 that a contract test pinned the two together — and until
    // 2026-08-18, when a third module joined, no such test existed. That is the
    // shape of finding the app has been bitten by twice (a stated guard that was
    // only a comment), so here it is for real.
    //
    // What it catches: a module registered in the shell but forgotten in the
    // book's composition. Nothing else would. The book would keep compiling and
    // its own tests would keep passing, because a section only fails once it
    // reads a table — so the gap would surface as "no such table" in a later
    // seam, or, worse, as a section quietly missing from a legal document.
    //
    // The shell is the only crate that can see both sides: the book does not
    // depend on src-tauri, and it must not — the arrow points the other way.
    let mut from_shell = Connection::open_in_memory().unwrap();
    composed_migrations().to_latest(&mut from_shell).unwrap();

    let mut from_book = Connection::open_in_memory().unwrap();
    terrazgo_recordbook::db::migrations()
        .to_latest(&mut from_book)
        .unwrap();

    assert_eq!(
        schema_objects(&from_shell),
        schema_objects(&from_book),
        "the record book's migration composition has drifted from the module \
         registry — add the missing module to terrazgo-recordbook's db::migrations \
         (and to its Cargo.toml), or remove the one that left"
    );
}

#[test]
fn the_siex_export_composes_the_same_schema_the_shell_does() {
    // The second consumer crate, pinned on exactly the same terms and for the
    // same failure: `terrazgo_siex::db::migrations()` hand-repeats the registry,
    // and a module registered in the shell but forgotten there would compile,
    // pass that crate's own tests, and then fail as "no such table" the first
    // time a block read it — or, worse, emit a descriptor silently missing a
    // register the authority expects.
    //
    // Written WITH the crate rather than after it, because the record book's
    // equivalent went eleven days as a doc comment describing a test that did
    // not exist.
    let mut from_shell = Connection::open_in_memory().unwrap();
    composed_migrations().to_latest(&mut from_shell).unwrap();

    let mut from_export = Connection::open_in_memory().unwrap();
    terrazgo_siex::db::migrations()
        .to_latest(&mut from_export)
        .unwrap();

    assert_eq!(
        schema_objects(&from_shell),
        schema_objects(&from_export),
        "the SIEX export's migration composition has drifted from the module \
         registry — add the missing module to terrazgo-siex's db::migrations \
         (and to its Cargo.toml), or remove the one that left"
    );
}

/// Every table, index, view and trigger a database carries, with its DDL —
/// the comparable fingerprint of a composed schema.
fn schema_objects(conn: &Connection) -> Vec<(String, String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT type, name, IFNULL(sql, '') FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn registry_is_populated_with_unique_names() {
    let modules = registered_modules();
    assert!(
        !modules.is_empty(),
        "at least the CUE module must be registered"
    );

    let mut names: Vec<&str> = modules.iter().map(|m| m.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), modules.len(), "module names must be unique");
}

#[test]
fn applies_cleanly_to_fresh_database() {
    let conn = fresh_migrated_db();

    // Module SQL arrived through the registry → composed runner wiring:
    // schema present and reference data seeded.
    assert!(count(&conn, "SELECT COUNT(*) FROM country") >= 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM treatment_record"), 0);
}

#[test]
fn applies_cleanly_from_previous_version() {
    // Upgrade path (testing strategy #3) through the COMPOSED sequence:
    // global v1 is the core's DDL (no seeds yet), to_latest then applies the rest
    // (core seed, cue DDL, cue seed).
    let mut conn = Connection::open_in_memory().unwrap();
    let migrations = composed_migrations();

    migrations.to_version(&mut conn, 1).unwrap();
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM country"),
        0,
        "v1 is DDL only"
    );

    migrations.to_latest(&mut conn).unwrap();
    assert!(
        count(&conn, "SELECT COUNT(*) FROM country") >= 1,
        "seeds applied"
    );
}

#[test]
fn global_version_accounts_for_core_and_all_modules() {
    let conn = fresh_migrated_db();

    // Derived from the registry, never pinned to a literal: a hardcoded count
    // beneath this line would assert nothing the derivation does not already
    // say, and would fail the build every time a migration is added (the
    // `status.len() == 47` lesson, 2026-08-11).
    let expected: usize = core_migrations().len()
        + registered_modules()
            .iter()
            .map(|m| m.migrations().len())
            .sum::<usize>();
    assert!(expected > 0, "the composed sequence cannot be empty");

    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        user_version as usize, expected,
        "user_version is the GLOBAL sequence position"
    );
}

#[test]
fn demo_seed_is_guarded_and_drives_alerts() {
    let mut conn = fresh_migrated_db();

    let first = module_cue::demo::seed_demo(&mut conn).unwrap();
    assert!(first.seeded);
    // Three since 2026-08-09: two product applications and one purely
    // non-chemical actuation (model 3.1 bis).
    assert_eq!(first.treatment_ids.len(), 3);

    // Los Alcores ships with real SIGPAC data (vendored recinfo response for
    // 47:182:0:0:7:14:1): one active sigpac boundary carrying the official
    // area, distinct from the declared plot.area_ha (8.75).
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM geo_feature WHERE source = 'sigpac' AND deleted_at IS NULL"
        ),
        1
    );
    let official: f64 = conn
        .query_row(
            "SELECT g.official_area_ha FROM geo_feature g
             JOIN plot p ON p.id = g.plot_id
             WHERE g.source = 'sigpac' AND p.name = 'Los Alcores'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!((official - 8.897).abs() < 1e-9);

    // Second call must refuse and change nothing.
    let rows_before = count(&conn, "SELECT COUNT(*) FROM record_change");
    let second = module_cue::demo::seed_demo(&mut conn).unwrap();
    assert!(!second.seeded);
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM record_change"),
        rows_before
    );

    // Pinned "today" — demo dates: PHI window 2026-05-25..2026-06-24 (open),
    // ITV due 2026-07-01 with 30-day lead (active from 2026-06-01), licence
    // expiry 2026-08-15 with 60-day lead (active from 2026-06-16 only).
    let config = module_cue::alerts::AlertConfig::defaults();

    module_cue::repository::refresh_alerts(&mut conn, "2026-06-12", &config).unwrap();
    let codes = active_alert_codes(&conn);
    assert_eq!(codes, vec!["itv_expiry", "phi_window"]);

    module_cue::repository::refresh_alerts(&mut conn, "2026-06-20", &config).unwrap();
    let codes = active_alert_codes(&conn);
    assert_eq!(codes, vec!["itv_expiry", "licence_expiry", "phi_window"]);
}

fn active_alert_codes(conn: &Connection) -> Vec<String> {
    let mut codes: Vec<String> = module_cue::repository::list_active_alerts(conn)
        .unwrap()
        .into_iter()
        .map(|a| a.alert_type_code)
        .collect();
    codes.sort_unstable();
    codes
}
