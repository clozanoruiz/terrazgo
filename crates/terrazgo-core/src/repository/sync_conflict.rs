// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The registers waiting for a person to choose between two versions
//! (docs/sync.md → Conflicts as the person sees them).
//!
//! `sync_conflict` is a notepad over the log rather than a record of its own:
//! `merge::settle` writes a row per losing branch as it materialises a register
//! and clears them all when one head is left, and any audited write to a
//! conflicted register clears them too. So this file only reads, and the queue
//! it reads is small by construction — a row lives from the import that
//! detected it to the moment somebody decides.

use std::collections::HashMap;

use rusqlite::{Connection, Row};
use serde::Serialize;

use crate::error::Result;
use crate::merge::RowCaption;
use crate::sql::children_by_parent;

/// One device's part in a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictDevice {
    pub device: String,
    /// What people call it (`sync_peer.label`), where anybody has named it.
    pub label: Option<String>,
    pub live: bool,
}

/// One register waiting for a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictEntry {
    pub root_table: String,
    pub root_id: String,
    pub season_id: Option<String>,
    /// The campaign's name, and its farm's, for a queue read across books —
    /// two farms may each keep a "2025/2026".
    pub season_label: Option<String>,
    pub farm_name: Option<String>,
    /// What to call the register — the live row's name column, where its table
    /// has one. `None` for a register whose table names nothing a person would
    /// recognise, or whose live version is a slot rather than a row: the screen
    /// falls back to the register's kind.
    pub caption: Option<String>,
    /// When the conflict was first detected, oldest of the branches' rows.
    pub detected_at: String,
    /// Every version waiting, live first. Two is the ordinary case.
    pub devices: Vec<ConflictDevice>,
}

/// The queue, newest first.
///
/// Database-wide rather than per book, because a conflict is about a register
/// and not every register is in one: a plot, an operator and a machine belong
/// to the holding. The season is carried on each row so a screen can say which
/// book a conflicted record is in.
const CONFLICT_QUEUE_SQL: &str = "SELECT root_table, root_id, season_id, live_device,
            other_device, detected_at
     FROM sync_conflict
     ORDER BY detected_at DESC, root_table, root_id, other_device";

/// Every register waiting for a person, with the names to show for each.
///
/// One row of `sync_conflict` per losing branch, so a three-way conflict is two
/// rows and one entry. `captions` is the composed naming map — core's half plus
/// each module's, joined by the shell.
pub fn list_sync_conflicts(
    conn: &Connection,
    captions: &[RowCaption],
) -> Result<Vec<ConflictEntry>> {
    let mut stmt = conn.prepare(CONFLICT_QUEUE_SQL)?;
    let mut rows = stmt.query([])?;
    let mut entries: Vec<ConflictEntry> = Vec::new();
    while let Some(row) = rows.next()? {
        let root_table: String = row.get("root_table")?;
        let root_id: String = row.get("root_id")?;
        let live_device: String = row.get("live_device")?;
        let other_device: String = row.get("other_device")?;
        // One register's rows are adjacent: they share `detected_at` — one
        // settle stamps every losing branch with one instant — as well as the
        // two columns after it in the sort. So the group is closed by the last
        // entry not matching, with no map to build and no second pass.
        let started = entries
            .last()
            .is_some_and(|held| held.root_table == root_table && held.root_id == root_id);
        if !started {
            entries.push(ConflictEntry {
                root_table,
                root_id,
                season_id: row.get("season_id")?,
                season_label: None,
                farm_name: None,
                caption: None,
                detected_at: row.get("detected_at")?,
                devices: vec![ConflictDevice {
                    device: live_device,
                    label: None,
                    live: true,
                }],
            });
        }
        if let Some(entry) = entries.last_mut() {
            entry.devices.push(ConflictDevice {
                device: other_device,
                label: None,
                live: false,
            });
        }
    }
    name_devices(conn, &mut entries)?;
    name_registers(conn, &mut entries, captions)?;
    name_seasons(conn, &mut entries)?;
    Ok(entries)
}

/// Every device's label, in one statement rather than one per version.
fn name_devices(conn: &Connection, entries: &mut [ConflictEntry]) -> Result<()> {
    let mut ids: Vec<String> = entries
        .iter()
        .flat_map(|entry| entry.devices.iter().map(|device| device.device.clone()))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let labels = labels_of(conn, "sync_peer", "label", &ids)?;
    for entry in entries {
        for device in &mut entry.devices {
            device.label = labels.get(&device.device).cloned();
        }
    }
    Ok(())
}

/// Each conflicted register's name, one statement per table involved.
///
/// The live version is what the tables hold, so the name comes from the table
/// rather than from the log — a seek per register would otherwise be a read of
/// its whole history.
fn name_registers(
    conn: &Connection,
    entries: &mut [ConflictEntry],
    captions: &[RowCaption],
) -> Result<()> {
    let mut tables: Vec<String> = entries
        .iter()
        .map(|entry| entry.root_table.clone())
        .collect();
    tables.sort_unstable();
    tables.dedup();
    for table in tables {
        let Some(caption) = captions.iter().find(|entry| entry.table == table) else {
            continue;
        };
        let mut ids: Vec<String> = entries
            .iter()
            .filter(|entry| entry.root_table == table)
            .map(|entry| entry.root_id.clone())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let names = labels_of(conn, caption.table, caption.column, &ids)?;
        for entry in entries.iter_mut().filter(|entry| entry.root_table == table) {
            entry.caption = names.get(&entry.root_id).cloned();
        }
    }
    Ok(())
}

/// The campaign each conflict is filed under, and its farm, in one statement —
/// and none when no conflict is in a book.
fn name_seasons(conn: &Connection, entries: &mut [ConflictEntry]) -> Result<()> {
    let mut ids: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.season_id.clone())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let books = super::season::book_names(conn, &ids)?;
    for entry in entries {
        let book = entry.season_id.as_ref().and_then(|id| books.get(id));
        entry.season_label = book.map(|book| book.label.clone());
        entry.farm_name = book.map(|book| book.farm.clone());
    }
    Ok(())
}

/// One column of many rows of one table, by id.
///
/// The SQL names a table and a column, so it varies per table and is built with
/// plain `prepare` — the shared statement cache is keyed by SQL text and is for
/// the statements every write runs. `children_by_parent` does the chunking, so
/// a queue of any size is a statement per 500 rows.
fn labels_of(
    conn: &Connection,
    table: &str,
    column: &str,
    ids: &[String],
) -> Result<HashMap<String, String>> {
    let sql = format!("SELECT id, {column} AS caption FROM {table} WHERE id IN ({{ids}})");
    let found = children_by_parent(
        conn,
        &sql,
        ids,
        |row: &Row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
        |(id, _)| id.clone(),
    )?;
    Ok(found
        .into_values()
        .flatten()
        .filter_map(|(id, caption)| caption.map(|caption| (id, caption)))
        .collect())
}
