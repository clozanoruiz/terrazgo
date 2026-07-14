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

    let expected: usize = core_migrations().len()
        + registered_modules()
            .iter()
            .map(|m| m.migrations().len())
            .sum::<usize>();
    // Today: 2 core + 2 cue. If this fails after adding a migration, the
    // composed sequence and this expectation must move together.
    assert_eq!(expected, 4);

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
    assert_eq!(first.treatment_ids.len(), 2);

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
    let config = module_cue::alerts::AlertConfig::default();

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
