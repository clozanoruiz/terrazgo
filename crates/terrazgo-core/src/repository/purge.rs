// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The purge: what was deleted from a book goes for good, thirty days on, once
//! every device has the deletion (docs/sync.md → The purge, as settled).
//!
//! **Nobody asks for it.** The app runs [`purge_due`] when it starts and after
//! every import — the two moments something can become due: the thirty days
//! pass, or the last device's knowledge arrives.
//!
//! **Nothing here is per register.** What can go is read off the installed
//! aggregate map: the registers of a book whose table nothing else points at
//! (`sync::book_tables`, `BookTable::erasable`) — never a book, a crop, a
//! sowing or anything else a record names, so no version arriving later can
//! name what went (docs/sync.md → What can go: what nothing else can point
//! at). Each register's rows come off its log. A register a module adds next
//! year is purged the day it exists, held to that by the index contract (an
//! index over its removed rows).
//!
//! **What goes is erased, not compacted**: `secure_delete` overwrites what it
//! frees as it frees it, and a checkpoint empties the write-ahead log, so the
//! rows are gone from the file without rewriting it. The file shrinks at the
//! maintenance check. Copies made before are beyond its reach.
//!
//! **It leaves a marker per register**: one `purged_register` row naming it and
//! the version that removed it, logged like any register so every other device
//! erases it too, and read by every stamp afterwards (`audit::WriteTx::register`).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;

use crate::audit::{begin, log_insert};
use crate::date::{add_days, now_utc_iso};
use crate::error::Result;
use crate::merge::{
    Applier, REGISTER_ROWS_NAMED_SQL, branch_states, heads, quote_ident, state_is_removed,
};
use crate::sync::{VersionVector, book_tables, held_through, installed_device, known, raise_held};

use crate::models::Season;
use crate::sync::{BookRows, BookTable};

use super::book_merge::{book_rows, logged_register};
use super::book_removal::REMOVED_BOOK_DAYS;
use super::season::map_season;

/// What one purge erased — here, or on another device and then here by an
/// import.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PurgeSummary {
    /// Registers erased, each with its rows and its log.
    pub registers: usize,
    /// What a screen counts: the books that went for the farmer — removed,
    /// with the last of what can go of them gone now — and the records that
    /// were in books. A slot's row, such as a book's declaration, is neither:
    /// nobody entered it as a record. The book itself is never erased (What
    /// can go), so it counts when its records are.
    pub books: usize,
    pub records: usize,
}

impl PurgeSummary {
    /// What a list of erased registers, `(root_table, root_id)`, counts as,
    /// given the books they were in (`books_of`, read before the erasure).
    pub(crate) fn of<'a>(
        conn: &Connection,
        erased: impl IntoIterator<Item = &'a (String, String)>,
        books: &BTreeSet<String>,
    ) -> Result<PurgeSummary> {
        let tables = book_tables(conn)?;
        let records: BTreeSet<&str> = tables
            .iter()
            .filter(|table| table.slot.is_none())
            .map(|table| table.table.as_str())
            .collect();
        let mut summary = PurgeSummary::default();
        for (table, _) in erased {
            summary.registers += 1;
            if records.contains(table.as_str()) {
                summary.records += 1;
            }
        }
        for book in books {
            if book_went(conn, &tables, book)? {
                summary.books += 1;
            }
        }
        Ok(summary)
    }
}

/// The books the given registers are filed in, as their logs state it — what
/// [`PurgeSummary::of`] counts once the registers are gone. A seek per register
/// on `idx_record_change_root`; read before the erasure takes the log.
pub(crate) fn books_of<'a>(
    conn: &Connection,
    registers: impl IntoIterator<Item = &'a (String, String)>,
) -> Result<BTreeSet<String>> {
    let mut stmt = conn.prepare(REGISTER_BOOKS_SQL)?;
    let mut books = BTreeSet::new();
    for (table, id) in registers {
        let found = stmt
            .query_map([table, id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        books.extend(found);
    }
    Ok(books)
}

const REGISTER_BOOKS_SQL: &str = "SELECT DISTINCT season_id FROM record_change
     WHERE root_table = ?1 AND root_id = ?2 AND season_id IS NOT NULL";

/// Whether a book has gone for the farmer: it is removed, and nothing that can
/// go of it is left — no removed register of an erasable table filed in it.
fn book_went(conn: &Connection, tables: &[BookTable], book: &str) -> Result<bool> {
    let Some(season) = conn
        .query_row("SELECT * FROM season WHERE id = ?1", [book], map_season)
        .optional()?
    else {
        return Ok(false);
    };
    if season.deleted_at.is_none() {
        return Ok(false);
    }
    for table in tables.iter().filter(|table| table.erasable) {
        if !book_rows(conn, table, &season, BookRows::Removed)?.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One register erased for good, as its marker row is logged
/// (`purged_register`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PurgedRegister {
    id: String,
    root_table: String,
    root_id: String,
    /// The register's version vector at its removal, JSON.
    removal: String,
    purged_at: String,
}

/// Erase, for good, every register of a book that is due by `today`
/// (docs/sync.md → When a register goes):
///
///   * its table is one nothing else points at ([`BookTable::erasable`]) — so
///     nothing that stays, and nothing arriving later, can name what goes;
///   * every version it has is a removal — and it is not one of two opposite
///     duplicate removals;
///   * it was removed more than [`REMOVED_BOOK_DAYS`] days before `today`, by
///     each removal's own `changed_at`;
///   * every active device holds each removal, as far as this device knows;
///   * no `export_alias` names it.
///
/// One change set: the rows and the log of every register that goes, and a
/// marker per register. Nothing due is the ordinary case, and costs one
/// statement per erasable register of a book, each reading only removed rows.
pub fn purge_due(conn: &mut Connection, today: &str, actor: Option<&str>) -> Result<PurgeSummary> {
    let due_before = add_days(today, -REMOVED_BOOK_DAYS)?;
    let due = due_registers(conn, &due_before)?;
    if due.is_empty() {
        return Ok(PurgeSummary::default());
    }
    erase(conn, &due, actor)
}

/// A register that could go, and what removed it: the version vector the
/// marker names — every version it has, joined, where two devices removed it
/// apart.
struct Due {
    removal: VersionVector,
}

/// The removed rows of one table that were removed before a day, on the
/// table's index over its removed rows — nothing live is read.
fn removed_before_sql(table: &str) -> String {
    format!(
        "SELECT id FROM {} WHERE deleted_at IS NOT NULL AND deleted_at < ?1 ORDER BY id",
        quote_ident(table)
    )
}

/// When a change set was made, by the clock of the device that made it. A seek
/// on the leading pair of `record_change`'s UNIQUE.
const SET_MADE_AT_SQL: &str =
    "SELECT changed_at FROM record_change WHERE origin_device = ?1 AND origin_seq = ?2 LIMIT 1";

/// Whether an export alias names a record — which the authority may hold, and
/// which only the exporter's submission log can release. A seek on
/// `idx_export_alias_entity`.
const ALIASED_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM export_alias WHERE entity_table = ?1 AND entity_id = ?2)";

/// The devices this one waits for: every device of the group nobody retired,
/// itself apart.
const ACTIVE_PEERS_SQL: &str = "SELECT id FROM sync_peer WHERE deleted_at IS NULL AND id <> ?1";

/// Every erasable register of a book that is due by `due_before`.
fn due_registers(conn: &Connection, due_before: &str) -> Result<BTreeMap<(String, String), Due>> {
    let me = installed_device(conn)?;
    let peers: Vec<String> = conn
        .prepare(ACTIVE_PEERS_SQL)?
        .query_map([&me], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let knowledge = known(conn)?;

    // Never the books themselves: every record points at its book.
    let tables: Vec<(String, bool)> = book_tables(conn)?
        .into_iter()
        .filter(|table| table.erasable)
        .map(|table| (table.table, table.slot.is_some()))
        .collect();

    let mut made_at = conn.prepare(SET_MADE_AT_SQL)?;
    let mut due = BTreeMap::new();
    for (table, slot) in tables {
        let ids: Vec<String> = conn
            .prepare(&removed_before_sql(&table))?
            .query_map([due_before], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut twice_over = if slot || ids.is_empty() {
            None
        } else {
            Some(conn.prepare(&removed_twice_over_sql(&table))?)
        };
        let mut asked: BTreeSet<String> = BTreeSet::new();
        for id in ids {
            let register = if slot {
                logged_register(conn, &table, &id)?
            } else {
                id
            };
            if !asked.insert(register.clone()) {
                continue;
            }
            let asked = Asked {
                due_before,
                peers: &peers,
                knowledge: &knowledge,
            };
            let removal = due_removal(
                conn,
                &mut made_at,
                twice_over.as_mut(),
                &table,
                &register,
                &asked,
            )?;
            if let Some(removal) = removal {
                due.insert((table.clone(), register), Due { removal });
            }
        }
    }
    Ok(due)
}

/// What every register is asked against: the day its removal must predate,
/// the devices that must hold it, and what this device knows they hold.
struct Asked<'a> {
    due_before: &'a str,
    peers: &'a [String],
    knowledge: &'a BTreeMap<String, VersionVector>,
}

/// What removed a register, if the register is due: removed in every version
/// it has, the newest removal long enough ago, each held by every active
/// device, and asked about by no person and no authority. `twice_over` is
/// [`removed_twice_over_sql`] prepared for the register's table — `None` for a
/// slot's, which no duplicate verdict or export alias names.
///
/// **Every version, not one.** Two devices removing one register apart — the
/// same book deleted on both, the same copy of a duplicate kept on both —
/// leave two versions that agree, which the review never lists and nothing
/// writes on top of again. *Corrected 2026-10-02*: requiring one version left
/// such a register in the file for good. What erases it is what erases one
/// version: the marker names both, joined, so a device holding either drops it
/// as history the removal had seen.
fn due_removal(
    conn: &Connection,
    made_at: &mut rusqlite::Statement,
    twice_over: Option<&mut rusqlite::Statement>,
    table: &str,
    register: &str,
    asked: &Asked,
) -> Result<Option<VersionVector>> {
    let current = heads(conn, table, register)?;
    if current.is_empty() {
        return Ok(None);
    }
    // Removed in the versions it has, not only in the table: the table is the
    // live one, but the versions are what every other device compares against.
    // Two that are both removals agree, so neither is waiting for a person.
    let states = branch_states(conn, table, register, &current)?;
    if states.len() != current.len()
        || !states
            .iter()
            .all(|state| state_is_removed(state, table, register))
    {
        return Ok(None);
    }
    let mut removal = VersionVector::default();
    for head in &current {
        let made: Option<String> = made_at
            .query_row(params![head.device, head.seq], |row| row.get(0))
            .optional()?;
        if !made.is_some_and(|made| made.as_str() < asked.due_before) {
            return Ok(None);
        }
        let everywhere = asked.peers.iter().all(|peer| {
            asked
                .knowledge
                .get(peer)
                .is_some_and(|seen| seen.has_seen(&head.device, head.seq))
        });
        if !everywhere {
            return Ok(None);
        }
        removal.merge(&head.vector);
    }
    if let Some(twice_over) = twice_over {
        let aliased: bool = conn.query_row(ALIASED_SQL, [table, register], |row| row.get(0))?;
        let twice: bool = twice_over.query_row([register, table], |row| row.get(0))?;
        if aliased || twice {
            return Ok(None);
        }
    }
    Ok(Some(removal))
}

/// Whether a removed record is one of two copies of one operation removed by
/// opposite verdicts — *keep A* on one device, *keep B* on another — which
/// waits on the Status view for a person to bring one back. Erasing it would
/// leave the operation in the book zero times, for good. Seeks on
/// `idx_duplicate_verdict_kept` and the pair; bound as `(id, table)`.
///
/// Built per table, so prepared once per table by the loop that runs it — once
/// per register, the preparing was three quarters of what the fold's line cost
/// on a book of 4 000 treatments (measured 2026-10-02).
fn removed_twice_over_sql(table: &str) -> String {
    format!(
        "SELECT EXISTS(
           SELECT 1 FROM duplicate_verdict v1
           JOIN duplicate_verdict v2
             ON v2.first_id = v1.first_id AND v2.second_id = v1.second_id
            AND v2.verdict = 'duplicate' AND v2.kept_id <> v1.kept_id
           JOIN {} other ON other.id = v2.kept_id AND other.deleted_at IS NOT NULL
           WHERE v1.kept_id = ?1 AND v1.verdict = 'duplicate' AND v1.subject_table = ?2)",
        quote_ident(table)
    )
}

/// Whether one removed record is one of two opposite duplicate removals — the
/// one-off form of [`removed_twice_over_sql`], for a caller asking of a single
/// record (a move deciding whether to carry it).
pub(super) fn removed_twice_over(conn: &Connection, table: &str, id: &str) -> Result<bool> {
    Ok(
        conn.query_row(&removed_twice_over_sql(table), [id, table], |row| {
            row.get(0)
        })?,
    )
}

// ---------------------------------------------------------------------------
// What still points at them
// ---------------------------------------------------------------------------

/// A foreign key pointing at a table: the table it is in, and its columns
/// beside the columns they name.
pub(super) struct Reference {
    pub(super) table: String,
    pub(super) from: Vec<String>,
    pub(super) to: Vec<String>,
}

/// Every foreign key pointing at each of `targets`, read off the schema — less
/// the ones from a register's own children, which go with it by definition.
pub(super) fn references_to(
    conn: &Connection,
    targets: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<Reference>>> {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut found: BTreeMap<String, Vec<Reference>> = BTreeMap::new();
    for table in tables {
        let own_parent: Option<(String, String)> = conn
            .query_row(
                "SELECT root, fk FROM temp.sync_shape WHERE table_name = ?1 AND role = 'child'",
                [&table],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let mut stmt = conn.prepare(
            "SELECT id, \"table\", \"from\", \"to\" FROM pragma_foreign_key_list(?1) ORDER BY id, seq",
        )?;
        let rows: Vec<(i64, String, String, Option<String>)> = stmt
            .query_map([&table], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut keys: Vec<(i64, String, Vec<String>, Vec<String>)> = Vec::new();
        for (id, target, from, to) in rows {
            let to = to.unwrap_or_else(|| "id".to_owned());
            match keys.last_mut() {
                Some((last, _, froms, tos)) if *last == id => {
                    froms.push(from);
                    tos.push(to);
                }
                _ => keys.push((id, target, vec![from], vec![to])),
            }
        }
        for (_, target, from, to) in keys {
            if !targets.contains(&target) {
                continue;
            }
            let own_child = own_parent
                .as_ref()
                .is_some_and(|(root, fk)| root == &target && from == [fk.clone()]);
            if own_child {
                continue;
            }
            found.entry(target).or_default().push(Reference {
                table: table.clone(),
                from,
                to,
            });
        }
    }
    Ok(found)
}

/// The register a row was last logged under, as `(root_table, root_id)`. A
/// seek on `idx_record_change_entity`.
pub(super) const ROW_REGISTER_SQL: &str = "SELECT root_table, root_id FROM record_change
     WHERE entity_table = ?1 AND entity_id = ?2
     ORDER BY hlc DESC, id DESC LIMIT 1";

/// The ids of the rows of `reference.table` that point at one row — through
/// the index the contract holds every such column to. The row being pointed at
/// is read for the columns the key names, which for every register but the
/// book's own composite is its `id`.
pub(super) fn pointing_at(
    conn: &Connection,
    reference: &Reference,
    row_table: &str,
    row_id: &str,
) -> Result<Vec<String>> {
    let named = reference
        .to
        .iter()
        .map(|column| quote_ident(column))
        .collect::<Vec<_>>()
        .join(", ");
    let values: Option<Vec<rusqlite::types::Value>> = conn
        .query_row(
            &format!(
                "SELECT {named} FROM {} WHERE id = ?1",
                quote_ident(row_table)
            ),
            [row_id],
            |row| {
                (0..reference.to.len())
                    .map(|index| row.get::<_, rusqlite::types::Value>(index))
                    .collect()
            },
        )
        .optional()?;
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    let matching = reference
        .from
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{} = ?{}", quote_ident(column), index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT id FROM {} WHERE {matching}",
        quote_ident(&reference.table)
    );
    // Built per reference, so prepared plainly: text that varies would crowd
    // the shared cache (`sql::cached_statement`).
    let ids = conn
        .prepare(&sql)?
        .query_map(rusqlite::params_from_iter(values.iter()), |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Erasing
// ---------------------------------------------------------------------------

/// Erase every register in `due` — its rows and its log — and write a marker
/// for each, in one change set; then empty the write-ahead log.
fn erase(
    conn: &mut Connection,
    due: &BTreeMap<(String, String), Due>,
    actor: Option<&str>,
) -> Result<PurgeSummary> {
    // Overwrite what is freed as it is freed: the erasure is real without the
    // file being rewritten. Only for the purge, so everyday work is unchanged.
    conn.pragma_update(None, "secure_delete", true)?;
    let erased = erase_in_one_change_set(conn, due, actor);
    conn.pragma_update(None, "secure_delete", false)?;
    let erased = erased?;
    // Pages that held the rows are zeroed in the log of pages too, but earlier
    // images of them stay in the write-ahead log until it is emptied.
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(erased)
}

fn erase_in_one_change_set(
    conn: &mut Connection,
    due: &BTreeMap<(String, String), Due>,
    actor: Option<&str>,
) -> Result<PurgeSummary> {
    let tx = begin(conn, actor)?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    // Read while the logs still say which book each register is filed in.
    let books = books_of(&tx, due.keys())?;

    // Every device whose change sets go: what this database has held of it is
    // raised to the log's own highest first, so no number is ever named twice.
    let mut devices: BTreeSet<String> = BTreeSet::new();
    {
        let mut writers = tx.prepare(
            "SELECT DISTINCT origin_device FROM record_change WHERE root_table = ?1 AND root_id = ?2",
        )?;
        for (table, id) in due.keys() {
            let found = writers
                .query_map([table, id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            devices.extend(found);
        }
    }
    let mut held = VersionVector::default();
    for device in &devices {
        held.observe(device, held_through(&tx, device)?);
    }
    raise_held(&tx, &held)?;

    {
        let mut rows_named = tx.prepare(REGISTER_ROWS_NAMED_SQL)?;
        let mut applier = Applier::new(&tx);
        for (table, id) in due.keys() {
            let named: Vec<(String, String)> = rows_named
                .query_map([table, id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (entity_table, entity_id) in &named {
                applier.delete(entity_table, entity_id)?;
            }
        }
    }
    {
        let mut log =
            tx.prepare("DELETE FROM record_change WHERE root_table = ?1 AND root_id = ?2")?;
        let mut waiting =
            tx.prepare("DELETE FROM sync_conflict WHERE root_table = ?1 AND root_id = ?2")?;
        for (table, id) in due.keys() {
            log.execute([table, id])?;
            waiting.execute([table, id])?;
        }
    }

    let now = now_utc_iso();
    for ((table, id), register) in due {
        let marker = PurgedRegister {
            id: Uuid::now_v7().to_string(),
            root_table: table.clone(),
            root_id: id.clone(),
            removal: register.removal.to_json()?,
            purged_at: now.clone(),
        };
        tx.execute(
            "INSERT INTO purged_register (id, root_table, root_id, removal, purged_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                marker.id,
                marker.root_table,
                marker.root_id,
                marker.removal,
                marker.purged_at
            ],
        )?;
        let stamp = tx.register("purged_register", &marker.id, None)?;
        log_insert(&tx, &stamp, "purged_register", &marker.id, &marker)?;
    }
    let summary = PurgeSummary::of(&tx, due.keys(), &books)?;
    tx.commit()?;
    Ok(summary)
}

// ---------------------------------------------------------------------------
// What a book past its date waits for
// ---------------------------------------------------------------------------

/// Why a book whose thirty days have passed is still here — what the fold
/// under the record-book list says of it (docs/sync.md → Deleting a book with
/// its records → Offered back for thirty days).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "waiting", rename_all = "snake_case")]
pub enum Erasing {
    /// For these devices to be known to hold its deletion, or the deletion of
    /// something in it. Each one syncing ends its part of the wait, and so
    /// does retiring it.
    Devices { devices: Vec<WaitedDevice> },
    /// For a person, on the Status view: a record still live in it, a conflict
    /// over something in it, two opposite duplicate removals.
    Status,
    /// For nothing but the calendar: it goes at the first start or import on
    /// or after `from` — `None` once that day has come.
    Due { from: Option<String> },
}

/// A device a book waits for, and what people call it where this device knows
/// a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitedDevice {
    pub device: String,
    pub label: Option<String>,
}

/// Why `book`, removed and past its thirty days by `today`, has not gone — the
/// purge's own conditions, asked of what can go of it (docs/sync.md → What can
/// go), and answered with the first a person can act on: the devices, then the
/// Status view, then the calendar. One read of each removed register's
/// versions — what the fold already reads of a book inside its thirty days to
/// count what would come back.
///
/// **`None` once nothing that can go of it is left**: to the farmer the book
/// has gone, and the fold stops listing it. What stays of it — the book itself,
/// its crops and sowings, which records point at — is shown nowhere. A record
/// still live in it keeps it listed, as waiting on the Status view.
///
/// An export alias would hold a record too, and nothing here says so: nothing
/// mints one while the export has no button, and what the line would say then
/// is for the exporter's submission log to decide (docs/siex-export.md).
pub fn book_erasure(conn: &Connection, book: &Season, today: &str) -> Result<Option<Erasing>> {
    let me = installed_device(conn)?;
    let peers: Vec<String> = conn
        .prepare(ACTIVE_PEERS_SQL)?
        .query_map([&me], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let knowledge = known(conn)?;

    // A row still live in it is a record in a removed book, which the Status
    // view lists with what to do about it.
    let mut status = false;
    // Each erasable table's removed registers in the book.
    let mut by_table: Vec<(String, bool, Vec<String>)> = Vec::new();
    for table in book_tables(conn)? {
        if !book_rows(conn, &table, book, BookRows::Live)?.is_empty() {
            status = true;
        }
        if !table.erasable {
            continue;
        }
        let slot = table.slot.is_some();
        let mut registers: BTreeSet<String> = BTreeSet::new();
        for id in book_rows(conn, &table, book, BookRows::Removed)? {
            registers.insert(if slot {
                logged_register(conn, &table.table, &id)?
            } else {
                id
            });
        }
        if !registers.is_empty() {
            by_table.push((table.table, slot, registers.into_iter().collect()));
        }
    }
    if by_table.is_empty() {
        return Ok(status.then_some(Erasing::Status));
    }

    let mut made_at = conn.prepare(SET_MADE_AT_SQL)?;
    let mut asked: BTreeSet<(String, i64)> = BTreeSet::new();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    let mut newest: Option<String> = None;
    for (table, slot, registers) in &by_table {
        let mut twice_over = if *slot {
            None
        } else {
            Some(conn.prepare(&removed_twice_over_sql(table))?)
        };
        for register in registers {
            let current = heads(conn, table, register)?;
            // One version is what the table holds, and the table says removed;
            // several are compared as the purge compares them.
            if current.len() > 1 {
                let states = branch_states(conn, table, register, &current)?;
                if !states
                    .iter()
                    .all(|state| state_is_removed(state, table, register))
                {
                    status = true;
                    continue;
                }
            }
            for head in &current {
                if !asked.insert((head.device.clone(), head.seq)) {
                    continue;
                }
                let made: Option<String> = made_at
                    .query_row(params![head.device, head.seq], |row| row.get(0))
                    .optional()?;
                if made > newest {
                    newest = made;
                }
                for peer in &peers {
                    let holds = knowledge
                        .get(peer)
                        .is_some_and(|seen| seen.has_seen(&head.device, head.seq));
                    if !holds {
                        missing.insert(peer.clone());
                    }
                }
            }
            if let Some(twice_over) = twice_over.as_mut()
                && twice_over.query_row([register, table], |row| row.get::<_, bool>(0))?
            {
                status = true;
            }
        }
    }

    if !missing.is_empty() {
        let mut label = conn.prepare("SELECT label FROM sync_peer WHERE id = ?1")?;
        let mut devices = Vec::with_capacity(missing.len());
        for device in missing {
            let found: Option<String> = label
                .query_row([&device], |row| row.get(0))
                .optional()?
                .flatten();
            devices.push(WaitedDevice {
                device,
                label: found,
            });
        }
        return Ok(Some(Erasing::Devices { devices }));
    }
    if status {
        return Ok(Some(Erasing::Status));
    }
    // Due the day after the newest removal's thirty days, by the clock of the
    // device that made it — as `purge_due` counts.
    if let Some(newest) = &newest {
        let from = add_days(newest.get(..10).unwrap_or(newest), REMOVED_BOOK_DAYS + 1)?;
        if from.as_str() > today {
            return Ok(Some(Erasing::Due { from: Some(from) }));
        }
    }
    Ok(Some(Erasing::Due { from: None }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::StatementStatus;

    #[test]
    fn what_is_due_is_found_on_the_removed_rows_alone() {
        // The control and the claim side by side: a table of live rows, read
        // for the removed ones by the purge's own statement, steps through no
        // row of it — while a plain filter on the same column scans it whole.
        let conn = crate::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO farm (id, name, country_code, created_at, updated_at)
             VALUES ('f', 'Los Llanos', 'es', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO season (id, farm_id, label, starts_on, ends_on, status, created_at, updated_at)
             VALUES ('s', 'f', '2025/2026', '2025-09-01', '2026-08-31', 'active',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 2000)
             INSERT INTO sowing_record (id, season_id, farm_id, kind_code, sown_on,
                                        created_at, updated_at)
             SELECT printf('r%d', i), 's', 'f', 'sowing', '2026-04-10',
                    '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z' FROM n;",
        )
        .unwrap();
        let steps = |sql: &str| {
            let mut stmt = conn.prepare(sql).unwrap();
            let mut rows = stmt.query(["2026-12-01"]).unwrap();
            while rows.next().unwrap().is_some() {}
            drop(rows);
            stmt.get_status(StatementStatus::FullscanStep)
        };
        assert!(
            steps("SELECT id FROM sowing_record WHERE sown_on < ?1") > 1000,
            "the counter must be able to fail"
        );
        assert_eq!(steps(&removed_before_sql("sowing_record")), 0);
    }
}
