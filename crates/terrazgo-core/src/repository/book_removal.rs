// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deleting a book with what is in it, and bringing it back (docs/sync.md →
//! Deleting a book with its records).
//!
//! What a farmer means by deleting a book is deleting what is in it, so
//! [`delete_book`] removes every live record of the book and the book itself,
//! in one change set. **Nothing here is per register**: the tables come off
//! the installed aggregate map ([`crate::sync::book_tables`]) and each row's
//! image off the log, as a merge moves them — a register a module adds is
//! deleted with its book the day it exists.
//!
//! [`restore_book`] brings back **what was removed with the book, and only
//! that**: every record whose current version is a change set that also
//! removed this book. A record removed on its own before the book went stays
//! removed. The test is on each record rather than on the book, because the
//! book can be live again while its records are not — renamed on another
//! device while this one deleted it, or opened again under the same dates — and
//! the records are brought back from the book's page then.
//!
//! **A deletion is offered back for thirty days** ([`REMOVED_BOOK_DAYS`]),
//! counted from the day it was made on the device that made it: the list of
//! removed books, the line on a live book's page and the act itself all look
//! that far back and no further for what to bring back. Past that, the purge
//! erases the book; one it has not erased yet stays in the list, offering
//! nothing back and saying what it waits for ([`super::book_erasure`]).

use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::json;

use crate::audit::begin;
use crate::date::{add_days, now_utc_iso};
use crate::duplicates::DuplicatePolicy;
use crate::error::{CoreError, Result};
use crate::merge::{Head, heads, live_head};
use crate::models::Season;
use crate::sync::{BookRows, BookTable, Hlc, book_tables};

use super::book_merge::{book_rows, conflicts_waiting, logged_register, rewrite, set};
use super::purge::{Erasing, book_erasure};
use super::season::{
    book_using_label, get_season, map_season, revive_season, soft_delete_season_tx,
};

/// How long a deleted book can be brought back, in days: the list of removed
/// books shows it, and bringing it back restores its records, for this long
/// after the day it was deleted.
pub const REMOVED_BOOK_DAYS: i64 = 30;

/// A book of a live farm, removed or not.
const BOOK_OF_LIVE_FARM_SQL: &str = "SELECT season.* FROM season
     JOIN farm ON farm.id = season.farm_id
     WHERE season.id = ?1 AND farm.deleted_at IS NULL";

/// The removed books of live farms, and each farm's name. A scan of
/// `idx_season_removed`, which holds the removed books and nothing else.
const REMOVED_BOOKS_SQL: &str = "SELECT season.*, farm.name AS farm_name FROM season
     JOIN farm ON farm.id = season.farm_id
     WHERE season.deleted_at IS NOT NULL AND farm.deleted_at IS NULL";

/// The change sets that deleted a book since a day, newest first, with who
/// made each and where. A seek on `idx_record_change_root`, over the book's own
/// few log rows.
const BOOK_REMOVALS_SQL: &str = "SELECT origin_device, origin_seq, actor, changed_at, hlc
     FROM record_change
     WHERE root_table = 'season' AND root_id = ?1 AND operation = 'delete' AND changed_at >= ?2
     ORDER BY hlc DESC, id DESC";

/// The latest change set that deleted a book, however long ago. A seek on
/// `idx_record_change_root`, as above.
const LATEST_BOOK_REMOVAL_SQL: &str = "SELECT origin_device, origin_seq, actor, changed_at, hlc
     FROM record_change
     WHERE root_table = 'season' AND root_id = ?1 AND operation = 'delete'
     ORDER BY hlc DESC, id DESC LIMIT 1";

const DEVICE_LABEL_SQL: &str = "SELECT label FROM sync_peer WHERE id = ?1";

const AUTHOR_NAME_SQL: &str = "SELECT display_name FROM user_profile WHERE id = ?1";

/// Delete a book with every live record in it — one change set. Returns how
/// many records went; the rows of a slot keyed by the book — a register
/// declared empty — are withdrawn too, and are not records.
///
/// Each row's image comes off its register's live version in the log, as a
/// merge moves it, so a register a module adds is deleted with nothing written
/// for it here. Children are untouched, as deleting one record leaves them.
///
/// Refused while something in the book waits for a person, because the
/// deletion would settle it with nobody looking — the merge's rule:
///
///   * a conflict (`book_delete_conflicts_waiting`);
///   * a pair removed twice over (`book_delete_removals_waiting`): once the
///     book is gone, the pair leaves the list of duplicates with it.
///
/// A removed book, or one of a removed farm, is `NotFound`.
pub fn delete_book(
    conn: &mut Connection,
    season_id: &str,
    policies: &[DuplicatePolicy],
    actor: Option<&str>,
) -> Result<usize> {
    let book = get_season(conn, season_id)?;
    if conflicts_waiting(conn, &book.id)? {
        return Err(CoreError::Invalid("book_delete_conflicts_waiting"));
    }
    if super::duplicate::book_has_both_removed(conn, policies, &book.id)? {
        return Err(CoreError::Invalid("book_delete_removals_waiting"));
    }
    let tables = book_tables(conn)?;

    let tx = begin(conn, actor)?;
    let now = now_utc_iso();
    let mut removed = 0usize;
    for table in &tables {
        for id in book_rows(&tx, table, &book, BookRows::Live)? {
            let register = match &table.slot {
                None => id.clone(),
                Some(_) => logged_register(&tx, &table.table, &id)?,
            };
            rewrite(
                &tx,
                &table.table,
                &id,
                &register,
                &book.id,
                "delete",
                |image| {
                    set(image, "deleted_at", json!(now));
                },
            )?;
            if table.slot.is_none() {
                removed += 1;
            }
        }
    }
    soft_delete_season_tx(&tx, &book.id)?;
    tx.commit()?;
    Ok(removed)
}

/// How many live records a book holds — what the confirmation before deleting
/// it names. One count per table hanging off a book, on the table's book index.
pub fn count_book_records(conn: &Connection, season_id: &str) -> Result<usize> {
    let book = get_season(conn, season_id)?;
    let mut total = 0usize;
    for table in book_tables(conn)? {
        if table.slot.is_some() {
            continue;
        }
        let sql = format!("SELECT COUNT(*) FROM ({})", table.rows_sql(BookRows::Live));
        let count: i64 = if table.farm_scoped {
            conn.query_row(&sql, params![book.id, book.farm_id], |row| row.get(0))?
        } else {
            conn.query_row(&sql, [&book.id], |row| row.get(0))?
        };
        total += usize::try_from(count).unwrap_or_default();
    }
    Ok(total)
}

/// What bringing a book back did.
#[derive(Debug, Clone, Serialize)]
pub struct RestoredBook {
    /// The book, live.
    pub season: Season,
    /// How many records came back with it.
    pub records: usize,
}

/// Bring a book back with what was removed with it (docs/sync.md → Deleting a
/// book with its records): the book, if it is removed, and every record of it
/// whose current version is a change set that also deleted this book, in the
/// last [`REMOVED_BOOK_DAYS`] days before `today`. One change set; for each
/// record, what the deletion wrote is put back as it was (`super::undo`).
///
/// **A record removed on its own stays removed** — before the book went, or by
/// a person choosing the removal in a review since. A record written since in
/// any way is not at the deletion any more, and is left as it is.
///
/// **A live book is brought back too**, meaning its records: a book renamed on
/// another device while this one deleted it comes back live and empty, and its
/// page offers what went with it ([`removed_with_book`]).
///
/// Refused while a conflict waits in the book (`book_restore_conflicts_waiting`)
/// — the restore is a write, and a write merges every version it saw — and,
/// for a removed book, while another live book of the farm goes by its name
/// (`season_restore_name_taken`): renaming that one is the person's call, and
/// a removed book cannot be renamed. A book of a removed farm stays removed
/// with it (`NotFound`).
pub fn restore_book(
    conn: &mut Connection,
    season_id: &str,
    today: &str,
    actor: Option<&str>,
) -> Result<RestoredBook> {
    let book = conn
        .query_row(BOOK_OF_LIVE_FARM_SQL, [season_id], map_season)
        .optional()?
        .ok_or(CoreError::NotFound)?;
    if conflicts_waiting(conn, &book.id)? {
        return Err(CoreError::Invalid("book_restore_conflicts_waiting"));
    }
    let removed = book.deleted_at.is_some();
    if removed && book_using_label(conn, &book.farm_id, &book.label, Some(&book.id))?.is_some() {
        return Err(CoreError::Invalid("season_restore_name_taken"));
    }
    let tables = book_tables(conn)?;
    let removals = book_removals(conn, &book.id, today)?;
    let restorable = restorable(conn, &tables, &book, &removals)?;
    if !removed && restorable.is_empty() {
        return Ok(RestoredBook {
            season: book,
            records: 0,
        });
    }

    let tx = begin(conn, actor)?;
    let season = if removed {
        let (label, custom_label) = (book.label.clone(), book.custom_label.clone());
        let (starts_on, ends_on) = (book.starts_on.clone(), book.ends_on.clone());
        revive_season(&tx, book, label, custom_label, starts_on, ends_on)?
    } else {
        book
    };
    for register in &restorable {
        super::undo::undo(
            &tx,
            &register.table,
            &register.root_id,
            Some(&season.id),
            &register.removal,
        )?;
    }
    tx.commit()?;
    Ok(RestoredBook {
        season,
        records: restorable.iter().filter(|register| register.record).count(),
    })
}

/// Who deleted a book, when and where, and what a restore would bring back.
#[derive(Debug, Clone, Serialize)]
pub struct BookRemoval {
    /// When the latest deletion was made, by the clock of the device it was
    /// made on.
    pub removed_at: String,
    /// The last day it can be brought back: [`REMOVED_BOOK_DAYS`] after the
    /// day it was made. Worked out here so a screen says the date without
    /// keeping a second copy of the number.
    pub restorable_until: String,
    /// The device it was made on, and its name where this device knows one.
    pub removed_on: String,
    pub device_label: Option<String>,
    /// The profile that made it, and its name where this device knows one.
    pub removed_by: Option<String>,
    pub author_name: Option<String>,
    /// The records a restore would bring back, per register. A deletion made
    /// by a merge removed none: the records went into the book that stayed.
    pub records: Vec<RecordCount>,
}

/// How many records of one register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordCount {
    /// The register's table, which the screen names.
    pub table: String,
    pub count: usize,
}

/// A removed book, as the list of them shows it.
#[derive(Debug, Clone, Serialize)]
pub struct RemovedBook {
    pub season: Season,
    pub farm_name: String,
    pub removal: BookRemoval,
    /// `None` while it can be brought back. Past its last day it cannot, and
    /// this says why it has not gone for good yet.
    pub erasing: Option<Erasing>,
}

/// The removed books of live farms, the latest deletion first: the ones
/// deleted in the last [`REMOVED_BOOK_DAYS`] days before `today`, which a
/// person can still bring back (docs/sync.md → Deleting a book with its
/// records) — and the ones past that and still here, each with what its
/// erasure waits for (docs/sync.md → The purge, as settled).
///
/// Worked out when read, stored nowhere: the removed books are a scan of a
/// partial index holding nothing else, empty on almost every device, and each
/// book's deletion is a seek on its own log.
pub fn list_removed_books(conn: &Connection, today: &str) -> Result<Vec<RemovedBook>> {
    let tables = book_tables(conn)?;
    let mut stmt = conn.prepare(REMOVED_BOOKS_SQL)?;
    let books = stmt
        .query_map([], |row| Ok((map_season(row)?, row.get("farm_name")?)))?
        .collect::<rusqlite::Result<Vec<(Season, String)>>>()?;
    let mut found = Vec::new();
    for (season, farm_name) in books {
        let removals = book_removals(conn, &season.id, today)?;
        if let Some(latest) = removals.first() {
            let removal = describe(conn, &tables, &season, &removals, latest)?;
            found.push((
                latest.hlc,
                RemovedBook {
                    season,
                    farm_name,
                    removal,
                    erasing: None,
                },
            ));
            continue;
        }
        let Some(latest) = conn
            .query_row(LATEST_BOOK_REMOVAL_SQL, [&season.id], map_removal)
            .optional()?
        else {
            continue;
        };
        // Past its thirty days, a book is listed only while something that can
        // go of it waits: once that has gone, to the farmer the book has.
        let Some(erasing) = book_erasure(conn, &season, today)? else {
            continue;
        };
        let erasing = Some(erasing);
        let removal = who_and_when(conn, &latest, Vec::new())?;
        found.push((
            latest.hlc,
            RemovedBook {
                season,
                farm_name,
                removal,
                erasing,
            },
        ));
    }
    found.sort_by(|(left_hlc, left), (right_hlc, right)| {
        right_hlc
            .cmp(left_hlc)
            .then_with(|| left.season.id.cmp(&right.season.id))
    });
    Ok(found.into_iter().map(|(_, book)| book).collect())
}

/// For a live book, what was removed with it in the last
/// [`REMOVED_BOOK_DAYS`] days before `today` and is removed still — what its
/// page offers to bring back. `None` when nothing is: a book never deleted
/// pays one seek on its own log to say so.
pub fn removed_with_book(
    conn: &Connection,
    season_id: &str,
    today: &str,
) -> Result<Option<BookRemoval>> {
    let book = get_season(conn, season_id)?;
    let removals = book_removals(conn, &book.id, today)?;
    if removals.is_empty() {
        return Ok(None);
    }
    let Some(latest) = removals.first() else {
        return Ok(None);
    };
    let tables = book_tables(conn)?;
    let removal = describe(conn, &tables, &book, &removals, latest)?;
    Ok(Some(removal).filter(|removal| !removal.records.is_empty()))
}

// ---------------------------------------------------------------------------
// What was removed with a book
// ---------------------------------------------------------------------------

/// One change set that deleted a book, and who made it where.
struct Removal {
    device: String,
    seq: i64,
    actor: Option<String>,
    at: String,
    /// Its clock, which orders deletions where their instants — whole seconds
    /// — tie.
    hlc: Hlc,
}

/// The change sets that deleted `season_id` on or after the first day of the
/// window ending `today`, newest first.
fn book_removals(conn: &Connection, season_id: &str, today: &str) -> Result<Vec<Removal>> {
    let since = add_days(today, -REMOVED_BOOK_DAYS)?;
    let removals = conn
        .prepare(BOOK_REMOVALS_SQL)?
        .query_map(params![season_id, since], map_removal)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(removals)
}

fn map_removal(row: &rusqlite::Row) -> rusqlite::Result<Removal> {
    Ok(Removal {
        device: row.get(0)?,
        seq: row.get(1)?,
        actor: row.get(2)?,
        at: row.get(3)?,
        hlc: row.get(4)?,
    })
}

/// A register a restore brings back: the version to undo, and whether it is a
/// record rather than a slot's declaration.
struct Restorable {
    table: String,
    root_id: String,
    removal: Head,
    record: bool,
}

/// The registers of `book` that were removed with it and are removed still:
/// every one of their current versions is a change set in `removals`.
///
/// Every current version, not only the live one: two people deleting one book
/// offline write two versions that agree, and either brings it back, but a
/// removal of the record on its own made at the same time is a person's word
/// about that record, and the record stays removed.
fn restorable(
    conn: &Connection,
    tables: &[BookTable],
    book: &Season,
    removals: &[Removal],
) -> Result<Vec<Restorable>> {
    let deleting: HashSet<(&str, i64)> = removals
        .iter()
        .map(|removal| (removal.device.as_str(), removal.seq))
        .collect();
    let mut found = Vec::new();
    if deleting.is_empty() {
        return Ok(found);
    }
    for table in tables {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for id in book_rows(conn, table, book, BookRows::Removed)? {
            let root_id = match &table.slot {
                None => id,
                Some(_) => logged_register(conn, &table.table, &id)?,
            };
            if !seen.insert(root_id.clone()) {
                continue;
            }
            let current = heads(conn, &table.table, &root_id)?;
            let removed_with_book = !current.is_empty()
                && current
                    .iter()
                    .all(|head| deleting.contains(&(head.device.as_str(), head.seq)));
            if !removed_with_book {
                continue;
            }
            let Some(removal) = live_head(&current).cloned() else {
                continue;
            };
            found.push(Restorable {
                table: table.table.clone(),
                root_id,
                removal,
                record: table.slot.is_none(),
            });
        }
    }
    Ok(found)
}

/// Who deleted `book` most recently — `latest`, the first of `removals` — and
/// what a restore would bring back.
fn describe(
    conn: &Connection,
    tables: &[BookTable],
    book: &Season,
    removals: &[Removal],
    latest: &Removal,
) -> Result<BookRemoval> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for register in restorable(conn, tables, book, removals)? {
        if register.record {
            *counts.entry(register.table).or_default() += 1;
        }
    }
    let mut records: Vec<RecordCount> = counts
        .into_iter()
        .map(|(table, count)| RecordCount { table, count })
        .collect();
    records.sort_by(|left, right| left.table.cmp(&right.table));
    who_and_when(conn, latest, records)
}

/// A deletion as the screens say it: when, where and by whom, and until when
/// it can be undone — beside `records`, what undoing it would bring back.
fn who_and_when(
    conn: &Connection,
    latest: &Removal,
    records: Vec<RecordCount>,
) -> Result<BookRemoval> {
    let device_label: Option<String> = conn
        .query_row(DEVICE_LABEL_SQL, [&latest.device], |row| row.get(0))
        .optional()?
        .flatten();
    let author_name: Option<String> = match &latest.actor {
        Some(actor) => conn
            .query_row(AUTHOR_NAME_SQL, [actor], |row| row.get(0))
            .optional()?,
        None => None,
    };
    let removed_on_day = latest.at.get(..10).unwrap_or(&latest.at);
    Ok(BookRemoval {
        restorable_until: add_days(removed_on_day, REMOVED_BOOK_DAYS)?,
        removed_at: latest.at.clone(),
        removed_on: latest.device.clone(),
        device_label,
        removed_by: latest.actor.clone(),
        author_name,
        records,
    })
}
