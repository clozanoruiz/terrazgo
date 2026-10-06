// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The duplicate rules held against the schema (docs/sync.md → Duplicate
//! suspects).
//!
//! A rule is a constant naming columns, and nothing in the compiler or the
//! database checks a column named in a string. So this file does, from the
//! composed schema — the whole of it, which only the shell can see:
//!
//!   * **every register of the book is listed.** A table the aggregate map
//!     calls a register of its own, carrying a `season_id`, must be on
//!     `duplicates::DUPLICATE_REGISTERS` — with rules, or declared never
//!     compared and why. A rule, not an allowlist: a register a new crate
//!     adds fails here the day it exists, until somebody decides;
//!   * every column, child table and parent link a rule names exists, and each
//!     child set it compares is part of the register;
//!   * **every fetch is a seek, never a scan** — the list's and the one run
//!     right after a save. The list is worked out whenever it is read, and the
//!     check after every save, so a register whose index is missing would give
//!     the right answer at a cost that grows with every record the farm ever
//!     kept — which only the query planner can show;
//!   * every dated rule of one register compares the same period, which is
//!     what lets one fetch serve all of them;
//!   * **every register with a rule is asked about right after its form
//!     saves** — the screen's half of the rule, found in `src/`.

// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use terrazgo_core::duplicates::{
    Scope, When, both_removed_sql, candidates_sql, overlap_sql, record_candidates_sql,
    record_overlap_sql,
};
use terrazgo_core::sync::SyncRole;
use terrazgo_lib::db::composed_migrations;
use terrazgo_lib::duplicates::DUPLICATE_REGISTERS;
use terrazgo_lib::registry::composed_sync_shape;

fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

fn columns(conn: &Connection, table: &str) -> BTreeSet<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info(?1)")
        .unwrap();
    stmt.query_map([table], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .unwrap()
}

/// The registers of the book: tables the aggregate map calls registers of
/// their own that carry a `season_id`.
fn registers_of_the_book(conn: &Connection) -> BTreeSet<&'static str> {
    composed_sync_shape()
        .into_iter()
        .filter(|entry| entry.role == SyncRole::Root)
        .filter(|entry| columns(conn, entry.table).contains("season_id"))
        .map(|entry| entry.table)
        .collect()
}

fn listed() -> Vec<&'static str> {
    DUPLICATE_REGISTERS
        .iter()
        .map(|register| register.policy.table)
        .collect()
}

#[test]
fn every_register_of_the_book_is_listed_for_duplicates() {
    let conn = composed_schema();
    let listed: BTreeSet<&str> = listed().into_iter().collect();
    let book = registers_of_the_book(&conn);
    assert!(
        book.len() >= 13,
        "the rule found only {} registers of the book — did the aggregate map change shape?",
        book.len()
    );
    for table in &book {
        assert!(
            listed.contains(table),
            "`{table}` is a register of the book and is not on \
             terrazgo_lib::duplicates::DUPLICATE_REGISTERS. Declare when two of its \
             records are one operation recorded twice — or `Detection::Never`, with \
             why — in the crate that owns it, and list it there \
             (docs/sync.md → Duplicate suspects)."
        );
    }
    for table in &listed {
        assert!(
            book.contains(table),
            "`{table}` is listed for duplicates but is not a register of the book: \
             a verdict names a register by its own id, and a rule reads its book"
        );
    }
}

#[test]
fn no_register_is_listed_twice() {
    let mut seen = BTreeSet::new();
    for table in listed() {
        assert!(seen.insert(table), "`{table}` is listed twice");
    }
}

#[test]
fn every_column_a_rule_names_exists_and_every_child_set_is_the_registers() {
    let conn = composed_schema();
    let shape = composed_sync_shape();
    for register in DUPLICATE_REGISTERS {
        let table = register.policy.table;
        let own = columns(&conn, table);
        let has = |column: &str, what: &str| {
            assert!(
                own.contains(column),
                "`{table}`'s duplicate rule names {what} `{column}`, which it does not have"
            );
        };
        for rule in register.policy.rules() {
            match rule.when {
                When::SameBook => has("season_id", "the book column"),
                When::Within { period, .. } => {
                    has("farm_id", "the farm column");
                    has(period.first_day(), "the day column");
                    if let Some(last) = period.last_day() {
                        has(last, "the last-day column");
                    }
                }
            }
            for same in rule.same {
                let (terrazgo_core::duplicates::Same::Value(column)
                | terrazgo_core::duplicates::Same::Stated(column)) = *same;
                has(column, "the compared column");
            }
            for overlap in rule.overlaps {
                let child = columns(&conn, overlap.table);
                assert!(
                    child.contains(overlap.parent) && child.contains(overlap.column),
                    "`{table}`'s duplicate rule compares `{}.{}` by `{}`, which that table \
                     does not have",
                    overlap.table,
                    overlap.column,
                    overlap.parent
                );
                let role = shape
                    .iter()
                    .find(|entry| entry.table == overlap.table)
                    .map(|entry| entry.role);
                assert_eq!(
                    role,
                    Some(SyncRole::Child {
                        root: table,
                        fk: overlap.parent
                    }),
                    "`{table}`'s duplicate rule compares `{}`, which is not one of its \
                     children through `{}`",
                    overlap.table,
                    overlap.parent
                );
            }
        }
    }
}

#[test]
fn every_dated_rule_of_a_register_compares_one_period() {
    // One fetch serves every rule of a register, and a dated fetch reaches as
    // far around the records in scope as the register's period and its widest
    // slack say (`DuplicatePolicy::period`). Two rules of one register reading
    // two different pairs of date columns would need two fetches.
    for register in DUPLICATE_REGISTERS {
        let periods: BTreeSet<String> = register
            .policy
            .rules()
            .iter()
            .filter_map(|rule| match rule.when {
                When::Within { period, .. } => Some(format!("{period:?}")),
                When::SameBook => None,
            })
            .collect();
        assert!(
            periods.len() <= 1,
            "`{}`'s dated rules compare different periods: {periods:?}",
            register.policy.table
        );
    }
}

/// The steps a fetch may read whole, because none is a table:
///
///   * the CTE a dated fetch builds first — one row per farm in scope, holding
///     the span of days around its records in scope. Reading it whole is
///     reading the farms, not the records;
///   * a book page's scope, `SELECT ?1` — one constant row naming the book;
///   * the CTE a record's dated fetch builds first — the one record asked
///     about, its farm, book and days.
const NOT_A_TABLE: [&str; 3] = ["SCAN span", "SCAN CONSTANT ROW", "SCAN r"];

/// The plan's steps that read a table whole.
fn scans(conn: &Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    let details = stmt
        .query_map(["2026-06-11"], |row| row.get::<_, String>("detail"))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    details
        .into_iter()
        .filter(|detail| detail.starts_with("SCAN ") && !NOT_A_TABLE.contains(&detail.as_str()))
        .collect()
}

#[test]
fn every_fetch_seeks_the_records_it_compares() {
    let conn = composed_schema();
    // The control: the instrument does see a scan when there is one.
    assert!(
        !scans(
            &conn,
            "SELECT x.id FROM treatment_record x WHERE x.notes = ?1"
        )
        .is_empty(),
        "the plan reader must be able to fail"
    );
    for register in DUPLICATE_REGISTERS {
        let policy = &register.policy;
        let table = policy.table;
        for scope in [
            Scope::Current {
                today: "2026-06-11",
            },
            Scope::Book { season_id: "s" },
        ] {
            let mut queries = vec![
                ("records", candidates_sql(policy, scope)),
                ("removed twice", both_removed_sql(table, scope)),
            ];
            for overlap in policy.overlaps() {
                queries.push((overlap.table, overlap_sql(policy, &overlap, scope)));
            }
            for (what, sql) in queries {
                let found = scans(&conn, &sql);
                assert!(
                    found.is_empty(),
                    "`{table}`'s {what} fetch reads a table whole ({found:?}) under \
                     {scope:?}: the list is worked out on every read, so its cost \
                     would grow with every record the farm ever kept. A dated rule \
                     needs `(farm_id, <first day>)` indexed."
                );
            }
        }
        // What is asked right after every save: the records one record could
        // pair with.
        let mut queries = vec![("records", record_candidates_sql(policy))];
        for overlap in policy.overlaps() {
            queries.push((overlap.table, record_overlap_sql(policy, &overlap)));
        }
        for (what, sql) in queries {
            let found = scans(&conn, &sql);
            assert!(
                found.is_empty(),
                "`{table}`'s {what} fetch after a save reads a table whole ({found:?}): it \
                 runs after every save, so its cost would grow with every record the \
                 farm ever kept"
            );
        }
    }
}

/// Every `.svelte` and `.js` file under `src/`, as text — tests excepted: a
/// test names a register without saving one, and would satisfy the check below
/// for a form that never calls it.
fn frontend_sources() -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if (name.ends_with(".svelte") || name.ends_with(".js"))
                    && !name.ends_with(".test.js")
                {
                    out.push(path);
                }
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .join("src");
    let mut paths = Vec::new();
    walk(&src, &mut paths);
    assert!(
        paths.len() > 20,
        "expected to walk the frontend, found {} files — has src/ moved?",
        paths.len()
    );
    paths
        .into_iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect()
}

#[test]
fn every_register_with_a_rule_is_checked_after_its_form_saves() {
    // A form asks its register's rule right after it saves, by naming what it
    // saved to the book page's check (src/lib/savedCheck.js). A form that
    // forgot would lose nothing — the list is the guarantee, and the pair shows
    // there — but the person who just typed it would not be told while they
    // still remember whether it was the same operation.
    let text = frontend_sources().join("\n");
    let call = |table: &str| format!("checkSaved(\"{table}\"");
    // The control: the search can come back empty.
    assert!(!text.contains(&call("no_such_register")));
    for register in DUPLICATE_REGISTERS {
        let table = register.policy.table;
        if register.policy.rules().is_empty() {
            continue;
        }
        assert!(
            text.contains(&call(table)),
            "`{table}` has a duplicate rule, and no form under src/ calls \
             `checkSaved(\"{table}\", id)` after saving one — add it after the \
             save, where the form closes (docs/sync.md → The same rule, right after the form saves)"
        );
    }
}
