// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A register's season is its own farm's, read off the composed schema.
//!
//! A season row is one holding's campaign (docs/data-model.md), and a register
//! names both its farm and its season, so the two must agree: a record filed
//! under another holding's season would print in neither book. The schema says
//! so with a composite foreign key onto `season(id, farm_id)`, and this test is
//! what keeps it saying so:
//!
//!   1. **every table carrying `season_id` and `farm_id` declares that key** —
//!      the schema is the expectation, so a register added next year is checked
//!      the day it exists, the `index_contract.rs` shape;
//!   2. **every foreign key in the schema has a parent SQLite can use.** A
//!      composite key needs a full UNIQUE index over exactly its parent columns,
//!      and when it has none the schema still creates cleanly — the mistake
//!      surfaces as "foreign key mismatch" at the first insert, in whichever
//!      register happens to write first. `PRAGMA foreign_key_check` asks the
//!      question of an empty database, which is when it is cheap to answer.
//!
//! `crop` carries `season_id` without `farm_id` (it reaches its farm through its
//! plot), so rule 1 cannot see it; `insert_crop` checks it instead, and core's
//! repository tests pin that.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rusqlite::Connection;
use terrazgo_lib::db::composed_migrations;

fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

fn tables(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn columns(conn: &Connection, table: &str) -> BTreeSet<String> {
    let mut stmt = conn
        .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .unwrap()
}

/// Each foreign key on `table` that points at `season`, as its set of
/// (child column, parent column) pairs. A composite key is several rows of
/// `pragma_foreign_key_list` sharing one `id`.
fn keys_onto_season(conn: &Connection, table: &str) -> Vec<BTreeSet<(String, String)>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, \"from\", \"to\" FROM pragma_foreign_key_list('{table}')
             WHERE \"table\" = 'season'
             ORDER BY id, seq"
        ))
        .unwrap();
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    let mut keys: Vec<(i64, BTreeSet<(String, String)>)> = Vec::new();
    for (id, from, to) in rows {
        match keys.last_mut() {
            Some((last, pairs)) if *last == id => {
                pairs.insert((from, to));
            }
            _ => keys.push((id, BTreeSet::from([(from, to)]))),
        }
    }
    keys.into_iter().map(|(_, pairs)| pairs).collect()
}

/// `PRAGMA foreign_key_check` over the whole schema. The error — "foreign key
/// mismatch" when a key has no usable parent index — is raised when the pragma
/// first steps, so it is returned rather than unwrapped here.
fn foreign_key_check(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Rule 1's predicate: the key maps `season_id` to `id` AND `farm_id` to
/// `farm_id`, and nothing else.
fn holds_season_to_its_farm(key: &BTreeSet<(String, String)>) -> bool {
    *key == BTreeSet::from([
        ("farm_id".to_string(), "farm_id".to_string()),
        ("season_id".to_string(), "id".to_string()),
    ])
}

#[test]
fn every_register_holds_its_season_to_its_own_farm() {
    let conn = composed_schema();
    let mut checked = 0;
    let mut missing = Vec::new();

    for table in tables(&conn) {
        let cols = columns(&conn, &table);
        if !(cols.contains("season_id") && cols.contains("farm_id")) {
            continue;
        }
        checked += 1;
        if !keys_onto_season(&conn, &table)
            .iter()
            .any(holds_season_to_its_farm)
        {
            missing.push(table);
        }
    }

    assert!(
        checked >= 13,
        "only {checked} registers found — the rule is looking at the wrong schema"
    );
    assert!(
        missing.is_empty(),
        "these registers carry season_id and farm_id but no foreign key onto \
         season(id, farm_id), so a record can be filed under another holding's \
         season and print in neither book: {missing:?}"
    );
}

#[test]
fn every_foreign_key_has_a_parent_sqlite_can_use() {
    let conn = composed_schema();
    // On an empty database there are no rows to report, so the only way this
    // fails is the mismatch the rule exists for.
    match foreign_key_check(&conn) {
        Ok(violations) => assert!(violations.is_empty(), "{violations:?}"),
        Err(e) => panic!("a foreign key has no parent index SQLite can use: {e}"),
    }
}

#[test]
fn the_rules_are_capable_of_failing() {
    // A guard on the guard, as in index_contract.rs: a register whose season key
    // forgets the farm must fail rule 1, and a composite key onto a PARTIAL
    // unique index — the tempting mistake, since `season` already carries one —
    // must fail rule 2.
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE season (id TEXT PRIMARY KEY, farm_id TEXT NOT NULL, deleted_at TEXT);
         CREATE UNIQUE INDEX idx_season_live ON season(farm_id, id) WHERE deleted_at IS NULL;
         CREATE TABLE loose_register (
             id TEXT PRIMARY KEY,
             season_id TEXT NOT NULL REFERENCES season(id),
             farm_id TEXT NOT NULL);
         CREATE TABLE mismatched_register (
             id TEXT PRIMARY KEY,
             season_id TEXT NOT NULL,
             farm_id TEXT NOT NULL,
             FOREIGN KEY (season_id, farm_id) REFERENCES season(id, farm_id));",
    )
    .unwrap();

    let loose = keys_onto_season(&conn, "loose_register");
    assert_eq!(loose.len(), 1);
    assert!(
        !holds_season_to_its_farm(&loose[0]),
        "a key on season_id alone must not satisfy rule 1"
    );
    let composite = keys_onto_season(&conn, "mismatched_register");
    assert!(
        holds_season_to_its_farm(&composite[0]),
        "the composite key is read as one key, not two"
    );

    assert!(
        foreign_key_check(&conn).is_err(),
        "a composite key onto a partial index must fail rule 2"
    );
}
