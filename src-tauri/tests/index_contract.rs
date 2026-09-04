// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The index house pattern, read off the composed schema.
//!
//! Two rules the compiler cannot check and no correctness test can fail on,
//! because a missing index changes nothing about the answer — only about what
//! it costs, and only once the table is big:
//!
//!   1. a **register** (a table carrying both `season_id` and `farm_id`) is read
//!      one campaign of one holding at a time, so it needs an index leading with
//!      those two columns;
//!   2. a **junction** (a child whose foreign key cascades, so it lives and dies
//!      with its parent) is always read through that parent, so it needs an
//!      index leading with that column — a `UNIQUE (parent_id, …)` constraint
//!      counting as one, which is how most of them already satisfy it.
//!
//! **There is no list to maintain here: the schema IS the expectation.** A
//! register added next year is checked the day it exists, which is the whole
//! reason this is a test rather than a paragraph — `treatment_record` sat off
//! the pattern for months and nothing noticed until an audit went looking.
//!
//! It lives in the shell because the shell is the only crate that can see the
//! whole schema. `terrazgo-core` may depend on no module, so the same test there
//! would check core's six tables and be blind to `treatment_record` and to every
//! eco-scheme and fertilisation register. `composed_migrations()` is where core's
//! steps and every module's exist in one sequence.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rusqlite::Connection;
use terrazgo::db::composed_migrations;

fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

/// User tables, in schema order. `sqlite_*` internals and the SQLite-managed
/// sequence table are not ours to have an opinion about.
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

/// Every index on `table`, as the ordered list of columns each one leads with.
/// Both `CREATE INDEX` and the implicit indexes behind `UNIQUE` constraints —
/// the planner does not distinguish them and neither does this rule.
///
/// An expression index yields a NULL column name; those entries are kept as
/// empty strings so they can never accidentally match a required column.
fn index_columns(conn: &Connection, table: &str) -> Vec<Vec<String>> {
    let mut list = conn
        .prepare(&format!("SELECT name FROM pragma_index_list('{table}')"))
        .unwrap();
    let names: Vec<String> = list
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    names
        .iter()
        .map(|index| {
            let mut info = conn
                .prepare(&format!(
                    "SELECT name FROM pragma_index_info('{index}') ORDER BY seqno"
                ))
                .unwrap();
            info.query_map([], |r| {
                Ok(r.get::<_, Option<String>>(0)?.unwrap_or_default())
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        })
        .collect()
}

/// The parents this table's rows cascade from — a foreign key with
/// `ON DELETE CASCADE` is the schema saying "this row lives and dies with that
/// one", which is exactly the child that is always read through its parent.
fn cascading_parents(conn: &Connection, table: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT \"from\" FROM pragma_foreign_key_list('{table}')
             WHERE on_delete = 'CASCADE'"
        ))
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// Rule 1's predicate. Either order: the book reads a campaign of a holding, and
/// which of the two columns leads is a per-register choice with no consequence
/// — `register_declaration` leads with the farm, the rest with the season.
fn leads_with_campaign_and_holding(index: &[String]) -> bool {
    let pair = (
        index.first().map(String::as_str),
        index.get(1).map(String::as_str),
    );
    matches!(
        pair,
        (Some("season_id"), Some("farm_id")) | (Some("farm_id"), Some("season_id"))
    )
}

#[test]
fn every_register_is_indexed_by_campaign_and_holding() {
    let conn = composed_schema();
    let mut checked = 0;
    let mut missing = Vec::new();

    for table in tables(&conn) {
        let cols = columns(&conn, &table);
        if !(cols.contains("season_id") && cols.contains("farm_id")) {
            continue;
        }
        checked += 1;
        let served = index_columns(&conn, &table)
            .iter()
            .any(|index| leads_with_campaign_and_holding(index));
        if !served {
            missing.push(table);
        }
    }

    assert!(
        checked >= 13,
        "only {checked} registers found — the rule is looking at the wrong schema"
    );
    assert!(
        missing.is_empty(),
        "these registers carry season_id and farm_id but no index leading with \
         both, so listing one campaign searches a holding's whole history: {missing:?}"
    );
}

#[test]
fn every_cascading_child_is_indexed_by_its_parent() {
    let conn = composed_schema();
    let mut checked = 0;
    let mut missing = Vec::new();

    for table in tables(&conn) {
        for parent_column in cascading_parents(&conn, &table) {
            checked += 1;
            let served = index_columns(&conn, &table)
                .iter()
                .any(|index| index.first() == Some(&parent_column));
            if !served {
                missing.push(format!("{table}.{parent_column}"));
            }
        }
    }

    assert!(
        checked >= 20,
        "only {checked} cascading children found — the rule is looking at the \
         wrong schema"
    );
    assert!(
        missing.is_empty(),
        "these children cascade from a parent but are not indexed by it, so \
         hydrating a list of parents scans them whole: {missing:?}"
    );
}

#[test]
fn the_rules_are_capable_of_failing() {
    // A guard on the guard. Both rules above pass by finding an index, so a
    // bug that made `index_columns` return everything would leave them green
    // and useless. This builds a schema that breaks each rule on purpose and
    // checks the predicates reject it.
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE bare_register (
             id TEXT PRIMARY KEY, season_id TEXT NOT NULL, farm_id TEXT NOT NULL);
         CREATE INDEX idx_bare_register_farm ON bare_register(farm_id);
         CREATE TABLE bare_child (
             id TEXT PRIMARY KEY,
             bare_register_id TEXT NOT NULL REFERENCES bare_register(id) ON DELETE CASCADE);",
    )
    .unwrap();

    let cols = columns(&conn, "bare_register");
    assert!(cols.contains("season_id") && cols.contains("farm_id"));
    let served = index_columns(&conn, "bare_register")
        .iter()
        .any(|index| leads_with_campaign_and_holding(index));
    assert!(!served, "a single-column index must not satisfy rule 1");

    let parents = cascading_parents(&conn, "bare_child");
    assert_eq!(parents, ["bare_register_id"]);
    let served = index_columns(&conn, "bare_child")
        .iter()
        .any(|index| index.first() == Some(&parents[0]));
    assert!(
        !served,
        "an unindexed cascading child must not satisfy rule 2"
    );
}
