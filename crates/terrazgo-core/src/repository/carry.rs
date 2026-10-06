// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a moved record names goes with it ([`carry_what_they_name`], added
//! 2026-10-02): a merge or a stray move re-points live records into another
//! book, and the removed rows of the book they leave that they name go with
//! them, still removed, filed in their new book (docs/sync.md → The merge).

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use crate::audit::{WriteTx, write_change};
use crate::date::now_utc_iso;
use crate::error::Result;
use crate::merge::{branch_states, heads, live_head, settle};

/// Carry, with the records a merge or a stray move has just re-pointed from
/// book `from` into book `into`, the removed rows of `from` that they name —
/// and what those name there in turn — each register filed in `into`, as it
/// stands and still removed. In the caller's change set, after the move.
///
/// *Corrected 2026-10-02* (docs/sync.md → The merge): a merge left every
/// removed record in the book that went, and a moved sowing could name a crop
/// removed before the merge. It then named a row of a removed book for good, so
/// that book could never be erased, and the sowing's own form — which lists its
/// book's crops — could not show what it named.
///
/// Only rows the moved records name travel; a removed record nothing moved
/// names stays where it is, as before. One of two records removed by opposite
/// duplicate verdicts stays too: writing on top of its removal would take
/// away bringing one of the pair back, which needs the removal to be its
/// newest version. A slot's row is never carried — its book is part of its
/// key — and neither is the book itself.
pub(crate) fn carry_what_they_name(
    tx: &WriteTx,
    moved: &[(String, String)],
    from: &crate::models::Season,
    into: &str,
) -> Result<()> {
    // From the book's removed records rather than from what moved: a book
    // usually holds none, found on each table's index over its removed rows,
    // so a merge carrying nothing pays one statement per table. Read the other
    // way — every moved record's version, for what it names — it cost a third
    // more on every merge (measured 2026-10-02).
    let mut left: Vec<(String, String)> = Vec::new();
    for table in crate::sync::book_tables(tx)? {
        if table.slot.is_some() {
            continue;
        }
        for id in super::book_merge::book_rows(tx, &table, from, crate::sync::BookRows::Removed)? {
            left.push((table.table.clone(), id));
        }
    }
    if left.is_empty() {
        return Ok(());
    }
    let targets: BTreeSet<String> = left.iter().map(|(table, _)| table.clone()).collect();
    let references = super::purge::references_to(tx, &targets)?;
    let mut carried: BTreeSet<(String, String)> = moved.iter().cloned().collect();
    let mut rows_named = tx.prepare(crate::merge::REGISTER_ROWS_NAMED_SQL)?;
    let mut row_register = tx.prepare(super::purge::ROW_REGISTER_SQL)?;
    // Until a pass carries nothing: a carried record can name another.
    loop {
        let mut carrying: Vec<(String, String)> = Vec::new();
        for register in &left {
            if carried.contains(register) {
                continue;
            }
            let rows: Vec<(String, String)> = rows_named
                .query_map([&register.0, &register.1], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            let mut named = false;
            'rows: for (row_table, row_id) in &rows {
                for reference in references.get(row_table).into_iter().flatten() {
                    for pointing in super::purge::pointing_at(tx, reference, row_table, row_id)? {
                        let by: Option<(String, String)> = row_register
                            .query_row([&reference.table, &pointing], |row| {
                                Ok((row.get(0)?, row.get(1)?))
                            })
                            .optional()?;
                        if by.is_some_and(|by| by != *register && carried.contains(&by)) {
                            named = true;
                            break 'rows;
                        }
                    }
                }
            }
            if named && !super::purge::removed_twice_over(tx, &register.0, &register.1)? {
                carrying.push(register.clone());
            }
        }
        if carrying.is_empty() {
            return Ok(());
        }
        for (table, id) in carrying {
            file_in(tx, &table, &id, into)?;
            carried.insert((table, id));
        }
    }
}

/// File a register in book `into`, in the caller's change set: each row of its
/// live version that states another book states `into`, with a fresh
/// `updated_at`, and nothing else is written — the rows belonging to it follow
/// their register on every device, which holds its history (docs/sync.md →
/// Bringing a register back writes only what changes).
fn file_in(tx: &WriteTx, root_table: &str, root_id: &str, into: &str) -> Result<()> {
    let current = heads(tx, root_table, root_id)?;
    let Some(live) = live_head(&current).cloned() else {
        return Ok(());
    };
    let state = branch_states(tx, root_table, root_id, std::slice::from_ref(&live))?
        .into_iter()
        .next()
        .unwrap_or_default();
    let now = now_utc_iso();
    let mut rows: Vec<(&(String, String), &Value)> = state
        .iter()
        .filter_map(|(key, image)| image.as_ref().map(|image| (key, image)))
        .filter(|(_, image)| {
            image
                .get("season_id")
                .is_some_and(|season| season.as_str() != Some(into))
        })
        .collect();
    if rows.is_empty() {
        return Ok(());
    }
    rows.sort_by_key(|((table, id), _)| (table != root_table, table.clone(), id.clone()));
    let stamp = tx.register(root_table, root_id, Some(into))?;
    for ((table, id), image) in rows {
        let mut stated = image.clone();
        if let Some(object) = stated.as_object_mut() {
            object.insert("season_id".to_owned(), json!(into));
            if object.contains_key("updated_at") {
                object.insert("updated_at".to_owned(), json!(now));
            }
        }
        write_change(
            tx,
            &stamp,
            table,
            id,
            "update",
            json!({ "before": image, "after": stated }),
        )?;
    }
    settle(tx, root_table, root_id, Some(&live))?;
    Ok(())
}
