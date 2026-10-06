// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Merging two books that turned out to be one campaign, and the records left
//! in a book that no longer exists (docs/sync.md → Merging two books that
//! turned out to be one campaign).
//!
//! Two devices that start one campaign on different dates keep two books; once
//! both are on one device, [`merge_books`] makes them one. Every live record of
//! the book that goes is re-pointed into the book the person chose to keep, in
//! one change set, and the book that goes is deleted with what it still holds:
//! the records removed from it before.
//!
//! **Nothing here is per register.** Which tables hang off a book is read off
//! the installed aggregate map ([`crate::sync::book_tables`]), and each row's
//! image is its register's live version read off the log — the way
//! `merge::resolve` builds the version it writes. The tables then follow
//! through `settle`, as after a resolution, so they stay a pure function of the
//! log. A register a module adds is moved the day it exists, with nothing
//! written for it here.
//!
//! **A live record in a deleted book is shown nowhere else** — no book page, no
//! print, no export — and a merge is not the only way to leave one there: a
//! book deleted on one device while another records into it, a record written
//! elsewhere before the merge arrived, a conflict resolved to the version that
//! states the old book. [`list_stray_records`] finds them when read, and
//! [`move_stray_records`] or [`super::restore_book`] puts them back in a book.

use std::collections::BTreeMap;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::audit::{WriteTx, begin, write_change};
use crate::date::{now_utc_iso, parse_date};
use crate::duplicates::DuplicatePolicy;
use crate::error::{CoreError, Result};
use crate::merge::{RowCaption, branch_states, heads, live_head, settle};
use crate::models::Season;
use crate::sync::{BookRows, BookTable, book_tables};

use super::season::{get_season, map_season, soft_delete_season_tx};

/// Whether anything filed under a book is waiting in the review queue. A seek
/// on `idx_sync_conflict_season`, against a table that is empty on almost every
/// device almost always.
const BOOK_CONFLICTS_SQL: &str = "SELECT EXISTS(SELECT 1 FROM sync_conflict WHERE season_id = ?1)";

/// A removed book of a live farm, and the farm's name.
const REMOVED_BOOK_SQL: &str = "SELECT season.*, farm.name AS farm_name FROM season
     JOIN farm ON farm.id = season.farm_id
     WHERE season.id = ?1 AND season.deleted_at IS NOT NULL AND farm.deleted_at IS NULL";

/// A farm's live books whose campaign shares a day with `?2`..`?3` — what a
/// record left in a removed book could go back into. A seek on
/// `idx_season_farm`; the dates are ISO, so they compare as text.
const OVERLAPPING_BOOKS_SQL: &str = "SELECT * FROM season
     WHERE farm_id = ?1 AND deleted_at IS NULL AND starts_on <= ?3 AND ends_on >= ?2";

/// What a merge did.
#[derive(Debug, Clone, Serialize)]
pub struct BookMerge {
    /// The book that stays, as it stands.
    pub kept: Season,
    /// How many live records were re-pointed into it.
    pub moved: usize,
}

/// Make two books of one farm one: every live record of `absorbed_id` moves
/// into `kept_id`, and `absorbed_id` is deleted — one change set.
///
/// **Which book stays is the person's**; [`super::kept_by_default`] is what the
/// screen pre-selects, alike on every device. Choosing the book rather than a
/// name is what keeps any name from being reused while the other book could
/// still come back on a device that has not seen the merge (docs/sync.md →
/// The merge).
///
/// **Live records move; removed ones stay** in the absorbed book and are
/// removed with it — **except what a moved record names**, which goes with it,
/// still removed (`super::carry::carry_what_they_name`,
/// corrected 2026-10-02): left behind, a sowing naming a crop removed before
/// the merge would name a row of a removed book for good, and that book could
/// never be erased. A slot keyed by its book — a register declared empty — is
/// withdrawn from the absorbed book and stated in the kept one when the kept
/// one states nothing for it.
///
/// Refused while anything in the absorbed book waits for a person, because
/// the merge would decide it with nobody looking:
///
///   * a conflict (`book_merge_conflicts_waiting`) — the move is a write, and a
///     write merges every version it saw;
///   * a pair removed twice over (`book_merge_removals_waiting`) — once its
///     book is gone, the pair leaves the list of duplicates with it.
///
/// Both have their answer on the Status view. Two books of different farms
/// (`book_merge_other_farm`), or a book with itself (`book_merge_same_book`),
/// are refused too; a removed book is `NotFound`, as everywhere.
pub fn merge_books(
    conn: &mut Connection,
    kept_id: &str,
    absorbed_id: &str,
    policies: &[DuplicatePolicy],
    actor: Option<&str>,
) -> Result<BookMerge> {
    if kept_id == absorbed_id {
        return Err(CoreError::Invalid("book_merge_same_book"));
    }
    let kept = get_season(conn, kept_id)?;
    let absorbed = get_season(conn, absorbed_id)?;
    if kept.farm_id != absorbed.farm_id {
        return Err(CoreError::Invalid("book_merge_other_farm"));
    }
    if conflicts_waiting(conn, &absorbed.id)? {
        return Err(CoreError::Invalid("book_merge_conflicts_waiting"));
    }
    if super::duplicate::book_has_both_removed(conn, policies, &absorbed.id)? {
        return Err(CoreError::Invalid("book_merge_removals_waiting"));
    }
    let tables = book_tables(conn)?;

    let tx = begin(conn, actor)?;
    let moved = move_rows(&tx, &tables, &absorbed, &kept.id)?;
    soft_delete_season_tx(&tx, &absorbed.id)?;
    tx.commit()?;
    Ok(BookMerge { kept, moved })
}

/// The books a merge screen offers for `season_id`: the farm's other live
/// books whose campaign shares at least a day with its campaign, the latest
/// ending first.
///
/// **Only those that overlap**, because two books of one campaign always do —
/// two books both named "2025/2026" by their dates both hold the turn of the
/// year — while a farm's consecutive campaigns never do. So a farm with years
/// of books is offered no merge on every page, and the offer appearing is
/// itself the sign that a book has a twin. A book whose dates were typed
/// wrong is corrected first, like any book. The overlap is the one
/// [`list_stray_records`] suggests a book by, read by the same statement.
pub fn merge_candidates(conn: &Connection, season_id: &str) -> Result<Vec<Season>> {
    let season = get_season(conn, season_id)?;
    let mut books = overlapping_books(conn, &season)?;
    books.retain(|book| book.id != season.id);
    books.sort_by(|left, right| {
        right
            .ends_on
            .cmp(&left.ends_on)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(books)
}

/// Records left live in a removed book, grouped by the book.
#[derive(Debug, Clone, Serialize)]
pub struct StrayBook {
    /// The removed book, as it was when it went.
    pub season: Season,
    pub farm_name: String,
    pub records: Vec<StrayRecord>,
    /// The farm's live book whose campaign overlaps the removed one's most —
    /// after a merge, the book it went into. `None` when no live book overlaps
    /// it: the person picks one, or brings the book back.
    pub suggested: Option<Season>,
}

/// One live record in a removed book.
#[derive(Debug, Clone, Serialize)]
pub struct StrayRecord {
    /// Its register's table, which the screen names.
    pub table: String,
    pub id: String,
    /// What the register is called — its `RowCaption` column, a treatment's
    /// day — where its table has one.
    pub caption: Option<String>,
}

/// Every live record in a removed book of a live farm (docs/sync.md → Records
/// in a removed book), the books ordered by farm and then latest campaign
/// first.
///
/// **Worked out when read, stored nowhere**, like the alerts and the
/// duplicates. One statement per table hanging off a book, each starting from
/// the removed books — `idx_season_removed`, empty on almost every device —
/// and seeking the table on its book index; then one statement per removed
/// book that has records, to name it and read its farm's live books.
pub fn list_stray_records(conn: &Connection, captions: &[RowCaption]) -> Result<Vec<StrayBook>> {
    let mut by_book: BTreeMap<String, Vec<StrayRecord>> = BTreeMap::new();
    for table in book_tables(conn)? {
        let caption = captions
            .iter()
            .find(|caption| caption.table == table.table)
            .map(|caption| caption.column);
        let mut stmt = conn.prepare(&table.stray_sql(caption))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let season_id: String = row.get(0)?;
            by_book.entry(season_id).or_default().push(StrayRecord {
                table: table.table.clone(),
                id: row.get(1)?,
                caption: text_of(row.get_ref(2)?),
            });
        }
    }
    let mut books = Vec::with_capacity(by_book.len());
    for (season_id, mut records) in by_book {
        records.sort_by(|left, right| (&left.table, &left.id).cmp(&(&right.table, &right.id)));
        let Some((season, farm_name)) = removed_book(conn, &season_id)? else {
            continue;
        };
        let suggested = suggested_book(conn, &season)?;
        books.push(StrayBook {
            season,
            farm_name,
            records,
            suggested,
        });
    }
    books.sort_by(|left, right| {
        left.farm_name
            .cmp(&right.farm_name)
            .then_with(|| right.season.ends_on.cmp(&left.season.ends_on))
            .then_with(|| left.season.id.cmp(&right.season.id))
    });
    Ok(books)
}

/// How many live records sit in removed books — what an import says arrived
/// there, the list's own statements counted rather than named.
pub fn count_stray_records(conn: &Connection) -> Result<usize> {
    let mut total = 0usize;
    for table in book_tables(conn)? {
        let sql = format!("SELECT COUNT(*) FROM ({})", table.stray_sql(None));
        let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
        total += usize::try_from(count).unwrap_or_default();
    }
    Ok(total)
}

/// Move every live record of the removed book `from_id` into the live book
/// `into_id` of the same farm — one change set, the merge's own move.
///
/// Refused while anything filed under the removed book waits in the review
/// queue (`stray_conflicts_waiting`): a record edited elsewhere while its book
/// was merged is exactly such a conflict, and moving it would decide it with
/// nobody looking. Its review shows which book each version files it in.
/// Returns how many records moved.
pub fn move_stray_records(
    conn: &mut Connection,
    from_id: &str,
    into_id: &str,
    actor: Option<&str>,
) -> Result<usize> {
    let (from, _) = removed_book(conn, from_id)?.ok_or(CoreError::NotFound)?;
    let into = get_season(conn, into_id)?;
    if into.farm_id != from.farm_id {
        return Err(CoreError::Invalid("book_merge_other_farm"));
    }
    if conflicts_waiting(conn, &from.id)? {
        return Err(CoreError::Invalid("stray_conflicts_waiting"));
    }
    let tables = book_tables(conn)?;

    let tx = begin(conn, actor)?;
    let moved = move_rows(&tx, &tables, &from, &into.id)?;
    tx.commit()?;
    Ok(moved)
}

// ---------------------------------------------------------------------------
// The move
// ---------------------------------------------------------------------------

/// Re-point `from`'s live rows into `into`, inside the caller's change set,
/// and carry with them the removed rows of `from` they name
/// (`super::carry::carry_what_they_name`). Returns how many records moved; a
/// slot's rows are not records, and neither is what was carried.
fn move_rows(tx: &WriteTx, tables: &[BookTable], from: &Season, into: &str) -> Result<usize> {
    let mut moved: Vec<(String, String)> = Vec::new();
    for table in tables {
        match &table.slot {
            None => {
                for id in book_rows(tx, table, from, BookRows::Live)? {
                    rewrite(tx, &table.table, &id, &id, into, "update", |image| {
                        set(image, "season_id", json!(into));
                    })?;
                    moved.push((table.table.clone(), id));
                }
            }
            // A withdrawn row of a slot keyed by its book stays where it is:
            // it states nothing, and moving it would be entering another
            // register.
            Some(columns) => {
                for id in book_rows(tx, table, from, BookRows::Live)? {
                    carry_slot_row(tx, &table.table, columns, &id, from, into)?;
                }
            }
        }
    }
    super::carry::carry_what_they_name(tx, &moved, from, into)?;
    Ok(moved.len())
}

/// The ids of one table's rows in `book`, on the table's book index.
pub(super) fn book_rows(
    conn: &Connection,
    table: &BookTable,
    book: &Season,
    rows: BookRows,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&table.rows_sql(rows))?;
    let found = if table.farm_scoped {
        stmt.query_map(params![book.id, book.farm_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    } else {
        stmt.query_map([&book.id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    };
    Ok(found)
}

/// Withdraw one live row of a slot keyed by its book from `from`, and state it
/// in `into` when `into` states nothing in that slot.
///
/// The row cannot simply be re-pointed: its book is part of its slot, so a row
/// in another book is in another register. The new row gets an id of its own —
/// one id in two registers' logs would let a rewind of one overwrite the other.
/// It is stated even beside records that contradict it: every screen and the
/// printed book let records win over a declaration, the state two devices can
/// already reach offline (docs/sync.md → The merge).
fn carry_slot_row(
    tx: &WriteTx,
    table: &str,
    columns: &[String],
    id: &str,
    from: &Season,
    into: &str,
) -> Result<()> {
    let from_slot = logged_register(tx, table, id)?;
    let now = now_utc_iso();
    let before = rewrite(tx, table, id, &from_slot, &from.id, "delete", |image| {
        set(image, "deleted_at", json!(now));
    })?;

    let row_id = Uuid::now_v7().to_string();
    let mut stated = before;
    set(&mut stated, "id", json!(row_id));
    set(&mut stated, "season_id", json!(into));
    for stamp in ["created_at", "updated_at"] {
        if stated.get(stamp).is_some() {
            set(&mut stated, stamp, json!(now));
        }
    }
    if stated.get("deleted_at").is_some() {
        set(&mut stated, "deleted_at", Value::Null);
    }
    let values = columns
        .iter()
        .map(|column| {
            stated.get(column).cloned().ok_or_else(|| {
                CoreError::ShapeViolation(format!("{table}'s logged image has no {column}"))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let into_slot = crate::sync::slot_id(&values);

    let current = heads(tx, table, &into_slot)?;
    let was_live = live_head(&current).cloned();
    if let Some(live) = &was_live
        && states_a_row(tx, table, &into_slot, live)?
    {
        return Ok(());
    }
    let stamp = tx.register(table, &into_slot, Some(into))?;
    write_change(
        tx,
        &stamp,
        table,
        &row_id,
        "insert",
        json!({ "before": Value::Null, "after": stated }),
    )?;
    settle(tx, table, &into_slot, was_live.as_ref())?;
    Ok(())
}

/// Whether a slot's live version holds a row still standing.
fn states_a_row(tx: &WriteTx, table: &str, slot: &str, live: &crate::merge::Head) -> Result<bool> {
    let states = branch_states(tx, table, slot, std::slice::from_ref(live))?;
    Ok(states.iter().any(|state| {
        state
            .values()
            .flatten()
            .any(|image| image.get("deleted_at").is_some_and(Value::is_null))
    }))
}

/// Rewrite one row of a register as `edit` leaves its live image, log it as
/// `operation`, and bring the tables in line through `settle` —
/// `merge::resolve`'s way of writing, so the tables stay a pure function of the
/// log. Returns the image as it was.
///
/// The image comes off the log rather than the table: the log is where every
/// value has the type its register's struct gave it, which a row read back
/// from SQLite cannot say (a boolean is an integer there).
pub(super) fn rewrite(
    tx: &WriteTx,
    table: &str,
    id: &str,
    root_id: &str,
    season: &str,
    operation: &str,
    edit: impl FnOnce(&mut Value),
) -> Result<Value> {
    let current = heads(tx, table, root_id)?;
    let live = live_head(&current)
        .cloned()
        .ok_or_else(|| unlogged(table, id))?;
    let before = branch_states(tx, table, root_id, std::slice::from_ref(&live))?
        .into_iter()
        .next()
        .and_then(|mut state| state.remove(&(table.to_owned(), id.to_owned())))
        .flatten()
        .ok_or_else(|| unlogged(table, id))?;
    let mut after = before.clone();
    edit(&mut after);
    if after.get("updated_at").is_some() {
        set(&mut after, "updated_at", json!(now_utc_iso()));
    }
    let stamp = tx.register(table, root_id, Some(season))?;
    write_change(
        tx,
        &stamp,
        table,
        id,
        operation,
        json!({ "before": before, "after": after }),
    )?;
    settle(tx, table, root_id, Some(&live))?;
    Ok(before)
}

/// Set one column of a logged image. An image is always an object — the log
/// holds whole rows — and indexing one that was not would panic, where the
/// applier refuses it with a reason.
pub(super) fn set(image: &mut Value, column: &str, value: Value) {
    if let Some(object) = image.as_object_mut() {
        object.insert(column.to_owned(), value);
    }
}

/// The register a row was last logged under — for a slot, the key its values
/// make, read off the log rather than rebuilt from a row SQLite hands back.
/// A seek on `idx_record_change_entity`.
pub(super) fn logged_register(conn: &Connection, table: &str, id: &str) -> Result<String> {
    crate::sql::cached_statement(conn, LOGGED_REGISTER_SQL)?
        .query_row([table, id], |row| row.get(0))
        .optional()?
        .ok_or_else(|| unlogged(table, id))
}

const LOGGED_REGISTER_SQL: &str = "SELECT root_id FROM record_change
     WHERE entity_table = ?1 AND entity_id = ?2
     ORDER BY hlc DESC, id DESC
     LIMIT 1";

/// A row the tables hold and the log does not: every write logs, so this is a
/// defect, and moving the row without its history would invent one.
fn unlogged(table: &str, id: &str) -> CoreError {
    CoreError::ShapeViolation(format!(
        "{table} {id} is in the tables but not in the log — every write logs"
    ))
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Whether anything filed under a book waits in the review queue.
pub(super) fn conflicts_waiting(conn: &Connection, season_id: &str) -> Result<bool> {
    Ok(crate::sql::cached_statement(conn, BOOK_CONFLICTS_SQL)?
        .query_row([season_id], |row| row.get(0))?)
}

/// A removed book of a live farm, and the farm's name; `None` otherwise.
fn removed_book(conn: &Connection, id: &str) -> Result<Option<(Season, String)>> {
    Ok(crate::sql::cached_statement(conn, REMOVED_BOOK_SQL)?
        .query_row([id], |row| Ok((map_season(row)?, row.get("farm_name")?)))
        .optional()?)
}

/// The farm's live book whose campaign overlaps `removed`'s most; ties to the
/// one ending later, then by id, so every device suggests the same one.
fn suggested_book(conn: &Connection, removed: &Season) -> Result<Option<Season>> {
    let mut best: Option<(i64, Season)> = None;
    for book in overlapping_books(conn, removed)? {
        let Some(days) = overlap_days(removed, &book)? else {
            continue;
        };
        let better = match &best {
            None => true,
            Some((held, kept)) => {
                (days, &book.ends_on, &book.id) > (*held, &kept.ends_on, &kept.id)
            }
        };
        if better {
            best = Some((days, book));
        }
    }
    Ok(best.map(|(_, book)| book))
}

/// The farm's live books whose campaign shares a day with `season`'s —
/// `season` itself among them when it is live.
fn overlapping_books(conn: &Connection, season: &Season) -> Result<Vec<Season>> {
    Ok(crate::sql::cached_statement(conn, OVERLAPPING_BOOKS_SQL)?
        .query_map(
            params![season.farm_id, season.starts_on, season.ends_on],
            map_season,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// How many days two campaigns share, both ends counted; `None` when they
/// share none.
fn overlap_days(a: &Season, b: &Season) -> Result<Option<i64>> {
    let start = parse_date(&a.starts_on)?.max(parse_date(&b.starts_on)?);
    let end = parse_date(&a.ends_on)?.min(parse_date(&b.ends_on)?);
    if end < start {
        return Ok(None);
    }
    let span = end
        .since(start)
        .map_err(|_| CoreError::InvalidDate(a.ends_on.clone()))?;
    Ok(Some(i64::from(span.get_days()) + 1))
}

/// A caption as text, whatever SQLite stored it as.
pub(super) fn text_of(value: ValueRef) -> Option<String> {
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(number) => Some(number.to_string()),
        ValueRef::Real(number) => Some(number.to_string()),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
    }
}
