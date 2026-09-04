// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The composed schema must define no views and no triggers.
//!
//! Every connection the app opens is hardened (`terrazgo_core::db::harden`),
//! and two of those flags — `ENABLE_VIEW` and `ENABLE_TRIGGER` — turn off
//! *using* views and triggers, not *creating* them. So a migration that adds
//! one would apply perfectly cleanly, pass its own migration tests, ship, and
//! then fail at read time with "access to view X prohibited", a long way from
//! the cause.
//!
//! This is the guard that turns that into a CI failure with an explanation. It
//! is not a claim that views and triggers are bad — it is the pairing that
//! makes the hardening safe to leave on. If a register ever genuinely needs
//! one, the decision is to relax the flag deliberately (and note it in
//! docs/architecture.md), not to discover the restriction in production.
//!
//! Foreign-key actions are NOT affected by the trigger flag — the schema's
//! `ON DELETE CASCADE` clauses keep working, pinned by a test beside `harden`
//! itself.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo::db::composed_migrations;

/// Every object of `kind` in the composed schema, by name.
fn objects_of_kind(kind: &str) -> Vec<String> {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = ?1 ORDER BY name")
        .unwrap();
    stmt.query_map([kind], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn the_composed_schema_defines_no_views() {
    let views = objects_of_kind("view");
    assert!(
        views.is_empty(),
        "the schema now defines view(s) {views:?}, but every connection runs \
         with ENABLE_VIEW=false, so reading one fails at runtime rather than \
         here. Either express it as a query where it is used (the house \
         pattern — 3.1 bis is a filtered view of treatment_record written in \
         Rust, not SQL), or relax the flag in terrazgo_core::db::harden on \
         purpose and say so in docs/architecture.md."
    );
}

#[test]
fn the_composed_schema_defines_no_triggers() {
    let triggers = objects_of_kind("trigger");
    assert!(
        triggers.is_empty(),
        "the schema now defines trigger(s) {triggers:?}, but every connection \
         runs with ENABLE_TRIGGER=false, so it will never fire. Note that the \
         audit trail is deliberately NOT a trigger: record_change stores \
         complete row images of the domain struct and an actor that is not a \
         column, neither of which a trigger can see."
    );
}
