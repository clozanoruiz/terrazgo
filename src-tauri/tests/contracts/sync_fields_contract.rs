// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The conflict review's field map, checked against the schema and the
//! dictionaries (docs/sync.md → Conflicts as the person sees them).
//!
//! `src/lib/syncFields.js` is a second description of the schema: it says what
//! each column of a register is CALLED, so a farmer choosing between two
//! versions reads "Dosis" rather than `dose_value`. The backend compares the
//! versions generically and knows nothing about it, and nothing else in the
//! app reads it — so without this test it would drift silently, and the first
//! sign would be a raw column name in front of a farmer mid-decision.
//!
//! Three checks, in the order they would fail:
//!
//! 1. **Every table and column it names exists.** A typo never matches a line,
//!    so the field it was meant to label silently keeps its column name.
//! 2. **Every register it claims is complete.** A table listed in the map
//!    whose schema grows a column the map does not name fails here, which is
//!    the moment to name it — "the SIEX exporter moves in parallel" applied to
//!    the review screen.
//! 3. **Every i18n key it names exists in every locale.** `t()` falls back to
//!    the key itself, so a mistyped key prints `sync.field.dose` on screen.
//!
//! A register absent from the map is NOT a failure: its columns fall back to
//! their own names, which is plain rather than broken, and that is what lets a
//! module add a register without a screen waiting on it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use rusqlite::Connection;
use terrazgo_core::sync::SyncRole;
use terrazgo_lib::db::composed_migrations;
use terrazgo_lib::registry::{composed_row_captions, composed_sync_shape};

use super::i18n_contract::{dictionaries, repo_root};

/// Columns no comparison ever names, whatever register they are on.
///
/// The first three never differ ABOUT anything: `id` is the key the two
/// versions are matched by, and the housekeeping stamps differ on almost every
/// pair of branches and mean nothing by it. `deleted_at` and `season_id` are
/// named once in the map's COMMON block instead of thirty times — the book a
/// version files a record in is compared since books can be merged
/// (docs/sync.md → Merging two books). `farm_id` places the register rather
/// than describing it — a record does not move between holdings — and a
/// child's own link to its register is exempted below, off the aggregate map
/// rather than by a list here.
const NEVER_COMPARED: &[&str] = &[
    "id",
    "created_at",
    "updated_at",
    "deleted_at",
    "season_id",
    "farm_id",
];

/// Columns that place a row inside something larger, beyond the two above and
/// the child links the aggregate map declares. Each is the row's OWNER, so it
/// is the question "which register is this" rather than a field of it.
const PLACEMENT: &[&str] = &[
    // A water point, a zone flag and a declaration all belong to their plot,
    // and each is a register of its own rather than a child of `plot`.
    "plot_water_point.plot_id",
    "plot_zone_flag.plot_id",
    "plot_water_declaration.plot_id",
    // A link row's own farm: the other half, the advisor, IS named.
    "farm_advisor.farm_id",
];

/// The map, as `{table: {column: key}}`, read out of the JS.
///
/// A tiny hand-rolled reader like `i18n_contract`'s, and for the same reason:
/// the test stays self-contained Rust with no Node invocation. It reads the
/// two forms the file uses — `column: "key"` and `column: coded("key",
/// "prefix")` — and stops at the end of the SYNC_FIELDS object.
fn sync_fields() -> BTreeMap<String, BTreeMap<String, String>> {
    let source = fs::read_to_string(repo_root().join("src/lib/syncFields.js")).unwrap();
    let body = source
        .split_once("export const SYNC_FIELDS = {")
        .expect("syncFields.js declares SYNC_FIELDS")
        .1;
    let body = body
        .split_once("\n};")
        .expect("SYNC_FIELDS is closed at the start of a line")
        .0;

    let mut map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut table: Option<String> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if line == "}," {
            table = None;
            continue;
        }
        if let Some(name) = line.strip_suffix(": {") {
            table = Some(name.trim().to_owned());
            map.entry(name.trim().to_owned()).or_default();
            continue;
        }
        let Some(table) = table.as_deref() else {
            continue;
        };
        let Some((column, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_end_matches(',');
        // `coded("treatment.unit", "unit")` names its key first.
        let key = value
            .strip_prefix("coded(")
            .unwrap_or(value)
            .split('"')
            .nth(1);
        if let Some(key) = key {
            map.entry(table.to_owned())
                .or_default()
                .insert(column.trim().to_owned(), key.to_owned());
        }
    }
    assert!(map.len() > 20, "the map reader found almost nothing");
    map
}

/// The composed schema's tables and their columns.
fn schema() -> BTreeMap<String, BTreeSet<String>> {
    let mut conn = Connection::open_in_memory().unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    tables
        .into_iter()
        .map(|table| {
            let columns = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>("name"))
                .unwrap()
                .collect::<rusqlite::Result<BTreeSet<_>>>()
                .unwrap();
            (table, columns)
        })
        .collect()
}

/// Whether the map is allowed not to name this column.
fn exempt(table: &str, column: &str) -> bool {
    if NEVER_COMPARED.contains(&column) || PLACEMENT.contains(&format!("{table}.{column}").as_str())
    {
        return true;
    }
    // A child's link to its register, off the aggregate map rather than a list.
    composed_sync_shape().iter().any(|entry| {
        entry.table == table && matches!(entry.role, SyncRole::Child { fk, .. } if fk == column)
    })
}

#[test]
fn every_table_and_column_the_field_map_names_exists() {
    let schema = schema();
    let mut missing = Vec::new();
    for (table, columns) in sync_fields() {
        let Some(known) = schema.get(&table) else {
            missing.push(table.clone());
            continue;
        };
        for column in columns.keys() {
            if !known.contains(column) {
                missing.push(format!("{table}.{column}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "src/lib/syncFields.js names what the schema does not have — a label \
         that can never appear: {missing:?}"
    );
}

#[test]
fn a_register_the_field_map_covers_is_covered_whole() {
    let schema = schema();
    let mut unnamed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (table, named) in sync_fields() {
        let Some(columns) = schema.get(&table) else {
            continue; // the test above says so
        };
        for column in columns {
            if named.contains_key(column) || exempt(&table, column) {
                continue;
            }
            unnamed
                .entry(table.clone())
                .or_default()
                .push(column.clone());
        }
    }
    assert!(
        unnamed.is_empty(),
        "these registers are in src/lib/syncFields.js, so the conflict review \
         claims to name every field of them — and these columns have no label, \
         so a farmer choosing between two versions would read the column name. \
         Add them to the map, or take the register out of it: {unnamed:?}"
    );
}

#[test]
fn every_label_the_field_map_names_is_in_every_dictionary() {
    let dictionaries = dictionaries();
    let mut keys: BTreeSet<String> = sync_fields()
        .values()
        .flat_map(|columns| columns.values().cloned())
        .collect();
    // The COMMON block sits outside SYNC_FIELDS and is as much a label as the
    // rest; it is one key, so it is named here rather than parsed.
    keys.insert("sync.field.deleted_at".to_owned());

    let mut missing = Vec::new();
    for (locale, dictionary) in &dictionaries {
        for key in &keys {
            if !dictionary.contains_key(key) {
                missing.push(format!("{locale}: {key}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "t() falls back to the key itself, so each of these prints a dotted \
         identifier on screen: {missing:?}"
    );
}

#[test]
fn every_register_a_conflict_can_name_has_a_kind_in_every_dictionary() {
    // The queue heads each entry with its register's KIND (`entity.<table>`),
    // and falls back to the table name when there is no entry — which is a
    // schema identifier in front of a farmer. Every table that can be a
    // register is therefore named, in every language.
    let dictionaries = dictionaries();
    let registers: Vec<&str> = composed_sync_shape()
        .iter()
        .filter(|entry| matches!(entry.role, SyncRole::Root | SyncRole::Slot { .. }))
        .map(|entry| entry.table)
        .collect();
    let mut missing = Vec::new();
    for (locale, dictionary) in &dictionaries {
        for table in &registers {
            if !dictionary.contains_key(&format!("entity.{table}")) {
                missing.push(format!("{locale}: entity.{table}"));
            }
        }
    }
    assert!(missing.is_empty(), "unnamed register kinds: {missing:?}");
}

#[test]
fn every_table_the_naming_map_reads_has_that_column() {
    // The other half of the same job, in Rust: `RowCaption` says which column
    // names a row, and the review reads it with `SELECT <column> FROM <table>`
    // — spliced, so a stale entry is a query that fails at runtime inside a
    // dialog rather than anything the compiler sees.
    let schema = schema();
    let mut wrong = Vec::new();
    for caption in composed_row_captions() {
        match schema.get(caption.table) {
            None => wrong.push(format!("{} (no such table)", caption.table)),
            Some(columns) if !columns.contains(caption.column) => {
                wrong.push(format!("{}.{}", caption.table, caption.column));
            }
            Some(_) => {}
        }
    }
    assert!(
        wrong.is_empty(),
        "the conflict review names rows by these columns and the schema has no \
         such column: {wrong:?}"
    );
}
