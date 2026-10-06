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
//!      counting as one, which is how most of them already satisfy it;
//!   3. a **foreign key pointing at a book or a register of one** needs an index
//!      leading with its columns, holding removed rows as well as live ones:
//!      the purge erases a record only together with everything that names it,
//!      and SQLite asks "does anything still name it" through that column —
//!      without an index, by reading the whole table for every row it erases;
//!   4. a **register of a book** (a table carrying `season_id` and `deleted_at`)
//!      needs a partial index over its removed rows, which is what the purge
//!      looks through, so finding what is due never grows with what is kept
//!      (docs/sync.md → The purge, as settled).
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
use terrazgo_lib::db::composed_migrations;

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

/// Every foreign key of `table`, as (the table it points at, its columns in
/// the key's order). A composite key — a register's `(season_id, farm_id)` —
/// is one entry.
fn foreign_keys(conn: &Connection, table: &str) -> Vec<(String, Vec<String>)> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, \"table\", \"from\" FROM pragma_foreign_key_list('{table}')
             ORDER BY id, seq"
        ))
        .unwrap();
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let mut keys: Vec<(i64, String, Vec<String>)> = Vec::new();
    for (id, target, column) in rows {
        match keys.last_mut() {
            Some((last, _, columns)) if *last == id => columns.push(column),
            _ => keys.push((id, target, vec![column])),
        }
    }
    keys.into_iter()
        .map(|(_, target, columns)| (target, columns))
        .collect()
}

/// Every index on `table` that holds REMOVED rows as well as live ones, as the
/// columns it leads with. A partial index qualifies only when its condition is
/// that its own leading column is set — which `col = ?` implies, so SQLite
/// uses it for exactly the lookup rule 3 is about. One that leaves removed rows
/// out cannot say what a removed row points at.
fn indexes_holding_removed_rows(conn: &Connection, table: &str) -> Vec<Vec<String>> {
    let mut list = conn
        .prepare(&format!(
            "SELECT il.name, il.partial, s.sql FROM pragma_index_list('{table}') AS il
             LEFT JOIN sqlite_schema AS s ON s.name = il.name"
        ))
        .unwrap();
    let found: Vec<(String, bool, Option<String>)> = list
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    found
        .into_iter()
        .filter_map(|(name, partial, sql)| {
            let columns: Vec<String> = conn
                .prepare(&format!(
                    "SELECT name FROM pragma_index_info('{name}') ORDER BY seqno"
                ))
                .unwrap()
                .query_map([], |r| {
                    Ok(r.get::<_, Option<String>>(0)?.unwrap_or_default())
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            if !partial {
                return Some(columns);
            }
            let condition = sql
                .as_deref()
                .and_then(|sql| {
                    sql.to_lowercase()
                        .split_once(" where ")
                        .map(|(_, c)| c.to_owned())
                })
                .map(|condition| condition.split_whitespace().collect::<Vec<_>>().join(" "))?;
            let lead = columns.first()?;
            (condition == format!("{lead} is not null")).then_some(columns)
        })
        .collect()
}

/// Whether an index leads with exactly `key`'s columns, in either order.
fn leads_with_key(index: &[String], key: &[String]) -> bool {
    index.len() >= key.len() && index[..key.len()].iter().all(|column| key.contains(column))
}

/// Whether `table` has a partial index over its removed rows — rule 4.
fn indexes_its_removed_rows(conn: &Connection, table: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT s.sql FROM pragma_index_list('{table}') AS il
             JOIN sqlite_schema AS s ON s.name = il.name
             WHERE il.partial = 1"
        ))
        .unwrap();
    let conditions: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    conditions.iter().any(|sql| {
        sql.to_lowercase()
            .split_once(" where ")
            .map(|(_, condition)| condition.split_whitespace().collect::<Vec<_>>().join(" "))
            .is_some_and(|condition| condition == "deleted_at is not null")
    })
}

/// The tables whose rows belong to a book: the books themselves, and every
/// table carrying `season_id` and `deleted_at`.
fn book_tables(conn: &Connection) -> Vec<String> {
    tables(conn)
        .into_iter()
        .filter(|table| {
            let cols = columns(conn, table);
            table == "season" || (cols.contains("season_id") && cols.contains("deleted_at"))
        })
        .collect()
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
fn every_link_into_a_book_is_indexed_with_its_removed_rows() {
    let conn = composed_schema();
    let books = book_tables(&conn);
    let mut checked = 0;
    let mut missing = Vec::new();

    for table in tables(&conn) {
        let indexes = indexes_holding_removed_rows(&conn, &table);
        for (target, key) in foreign_keys(&conn, &table) {
            if !books.contains(&target) {
                continue;
            }
            checked += 1;
            if !indexes.iter().any(|index| leads_with_key(index, &key)) {
                missing.push(format!("{table}({}) -> {target}", key.join(", ")));
            }
        }
    }

    assert!(
        checked >= 30,
        "only {checked} links into a book found — the rule is looking at the \
         wrong schema"
    );
    assert!(
        missing.is_empty(),
        "these columns point at a book or a register of one but no index holding \
         removed rows leads with them, so erasing what they point at reads the \
         whole table for every row (docs/sync.md → The purge, as settled): {missing:?}"
    );
}

#[test]
fn every_register_of_a_book_indexes_its_removed_rows() {
    let conn = composed_schema();
    let books = book_tables(&conn);
    assert!(
        books.len() >= 15,
        "only {} tables of a book found — the rule is looking at the wrong schema",
        books.len()
    );
    let missing: Vec<&String> = books
        .iter()
        .filter(|table| !indexes_its_removed_rows(&conn, table))
        .collect();
    assert!(
        missing.is_empty(),
        "these registers of a book have no index over their removed rows, so \
         finding what the purge may erase reads every row they keep \
         (docs/sync.md → The purge, as settled): {missing:?}"
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

    // Rules 3 and 4: a link indexed over live rows only, and a register with
    // no index over its removed ones.
    conn.execute_batch(
        "CREATE TABLE season (id TEXT PRIMARY KEY, deleted_at TEXT);
         CREATE TABLE bare_record (
             id TEXT PRIMARY KEY, season_id TEXT REFERENCES season(id), deleted_at TEXT);
         CREATE TABLE bare_link (
             id TEXT PRIMARY KEY, bare_record_id TEXT REFERENCES bare_record(id),
             deleted_at TEXT);
         CREATE INDEX idx_bare_link_live ON bare_link(bare_record_id)
             WHERE deleted_at IS NULL;
         CREATE INDEX idx_bare_record_season ON bare_record(season_id);",
    )
    .unwrap();
    let key = vec!["bare_record_id".to_owned()];
    assert!(
        !indexes_holding_removed_rows(&conn, "bare_link")
            .iter()
            .any(|index| leads_with_key(index, &key)),
        "an index over live rows only must not satisfy rule 3"
    );
    assert!(
        indexes_holding_removed_rows(&conn, "bare_record")
            .iter()
            .any(|index| leads_with_key(index, &["season_id".to_owned()])),
        "and a whole index does"
    );
    assert!(
        !indexes_its_removed_rows(&conn, "bare_record"),
        "a register with no index over its removed rows must not satisfy rule 4"
    );
}
