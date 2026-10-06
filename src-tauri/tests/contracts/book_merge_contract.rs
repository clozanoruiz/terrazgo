// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Merging two books moves every table hanging off one, and deleting a book
//! removes every one of them, read off the composed schema (docs/sync.md →
//! Merging two books that turned out to be one campaign, Deleting a book with
//! its records).
//!
//! The merge, the deletion, bringing a book back and the list of records left
//! in a removed book are generic: they find the tables to read in the
//! installed aggregate map (`terrazgo_core::sync::book_tables`), so a register
//! a module adds is moved and deleted the day it exists. This test is what
//! keeps "the day it exists" true:
//!
//!   1. **every table carrying `season_id` is found** — or is one of the two
//!      that name a book without belonging to one: the log, and the review
//!      queue derived from it. A table the map left out would stay behind,
//!      live, in a book that is deleted, and nothing would show it again;
//!   2. **every one of them has `deleted_at`**, which the move, the deletion
//!      and the list all read;
//!   3. **every statement they run seeks its table** — the list runs on every
//!      visit to the Status view, and a scan there grows with every record the
//!      holdings ever kept;
//!   4. **the purge erases only what nothing else can point at**, and the
//!      tables it keeps are the ones docs/sync.md names (What can go): a
//!      module adding a reference changes the list, and the doc with it.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rusqlite::Connection;
use terrazgo_core::sync::{BookRows, BookTable, book_tables, install_shape};
use terrazgo_lib::db::composed_migrations;
use terrazgo_lib::registry::{composed_row_captions, composed_sync_shape};

/// The composed schema with the composed map installed, as the app opens it.
fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    let shape = composed_sync_shape();
    install_shape(&conn, &[shape.as_slice()]).unwrap();
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

/// Tables that name a book without being in one: the log, which outlives every
/// row it describes, and the review queue, derived from the log on each device.
const NAMES_A_BOOK_ONLY: [&str; 2] = ["record_change", "sync_conflict"];

#[test]
fn every_table_carrying_a_book_is_moved_with_it() {
    let conn = composed_schema();
    let carrying: BTreeSet<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
        .into_iter()
        .filter(|table| columns(&conn, table).contains("season_id"))
        .filter(|table| !NAMES_A_BOOK_ONLY.contains(&table.as_str()))
        .collect();
    let found: BTreeSet<String> = book_tables(&conn)
        .unwrap()
        .into_iter()
        .map(|table| table.table)
        .collect();
    assert_eq!(
        found, carrying,
        "every table with a `season_id` must be a register or a slot of the aggregate \
         map, so merging two books moves it: one left out stays in the book that is \
         deleted, where nothing shows it again"
    );
    // The book's fourteen registers and its declarations — named, so the
    // comparison above cannot pass on two empty sets.
    for expected in [
        "treatment_record",
        "crop",
        "grazing_record",
        "register_declaration",
    ] {
        assert!(found.contains(expected), "{expected} hangs off a book");
    }
}

#[test]
fn every_table_hanging_off_a_book_says_whether_a_row_is_removed() {
    let conn = composed_schema();
    for table in book_tables(&conn).unwrap() {
        assert!(
            columns(&conn, &table.table).contains("deleted_at"),
            "{}: a deletion removes live rows, a restore reads removed ones and the \
             list reads live ones, so a table hanging off a book needs `deleted_at`",
            table.table
        );
    }
}

/// The plan's steps, with every parameter bound to a dummy value.
fn plan(conn: &Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    for index in 1..=stmt.parameter_count() {
        stmt.raw_bind_parameter(index, "x").unwrap();
    }
    let mut rows = stmt.raw_query();
    let mut details = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        details.push(row.get::<_, String>("detail").unwrap());
    }
    details
}

fn scans(conn: &Connection, sql: &str) -> Vec<String> {
    plan(conn, sql)
        .into_iter()
        .filter(|detail| detail.starts_with("SCAN "))
        .collect()
}

fn caption_of<'a>(
    captions: &'a [terrazgo_core::merge::RowCaption],
    table: &BookTable,
) -> Option<&'a str> {
    captions
        .iter()
        .find(|caption| caption.table == table.table)
        .map(|caption| caption.column)
}

#[test]
fn every_statement_the_merge_the_deletion_and_the_list_run_seeks_its_table() {
    let conn = composed_schema();
    // The control: the instrument does see a scan when there is one.
    assert!(
        !scans(&conn, "SELECT id FROM treatment_record WHERE notes = ?1").is_empty(),
        "the plan reader must be able to fail"
    );
    let captions = composed_row_captions();
    for table in book_tables(&conn).unwrap() {
        for rows in [BookRows::Every, BookRows::Live, BookRows::Removed] {
            let found = scans(&conn, &table.rows_sql(rows));
            assert!(
                found.is_empty(),
                "{}: a book's rows are read whole ({found:?}) — index `season_id`{}",
                table.table,
                if table.farm_scoped {
                    " with `farm_id`"
                } else {
                    ""
                }
            );
        }
        // The list starts from the removed books, which is a scan of a partial
        // index holding nothing but them; everything else must be a seek.
        let sql = table.stray_sql(caption_of(&captions, &table));
        let steps = plan(&conn, &sql);
        assert!(
            steps.iter().any(|step| step.contains("idx_season_removed")),
            "{}: the list must start from the removed books ({steps:?})",
            table.table
        );
        let found: Vec<&String> = steps
            .iter()
            .filter(|step| step.starts_with("SCAN ") && !step.contains("idx_season_removed"))
            .collect();
        assert!(
            found.is_empty(),
            "{}: the list of records in a removed book reads a table whole ({found:?}); it \
             runs on every visit to the Status view",
            table.table
        );
    }
}

#[test]
fn the_purge_keeps_exactly_what_records_can_point_at() {
    // docs/sync.md → What can go: what nothing else can point at. Read off
    // the composed schema's foreign keys when the map is installed; held here
    // to the list the doc gives, so the two cannot part.
    let conn = composed_schema();
    let tables = book_tables(&conn).unwrap();
    let kept: Vec<&str> = tables
        .iter()
        .filter(|table| !table.erasable)
        .map(|table| table.table.as_str())
        .collect();
    assert_eq!(
        kept,
        ["crop", "irrigation_record", "soil_cover", "sowing_record"],
        "the registers of a book records point at (the book itself is not in \
         book_tables, and is never erased either)"
    );
    // The control: the rest of the book can go.
    for table in ["treatment_record", "fertilisation_record", "harvest_record"] {
        assert!(
            tables
                .iter()
                .any(|found| found.table == table && found.erasable),
            "{table} can go"
        );
    }
    let book_pointed_at: bool = conn
        .query_row(
            "SELECT pointed_at FROM temp.sync_shape WHERE table_name = 'season'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(book_pointed_at, "every record points at its book");
}
