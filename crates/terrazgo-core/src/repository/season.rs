// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Season (campaña agrícola) CRUD. A season row is one holding's campaign, and
//! so one record book, named by its dates unless the farmer names it.
//!
//! Deleting a book deletes what is in it, and is `book_removal`'s: every
//! record-book view is season-scoped, so a book removed with records left live
//! in it would hide them (docs/sync.md → Deleting a book with its records).

use crate::audit::{WriteTx, begin, log_delete, log_insert, log_update};
use crate::date::{now_utc_iso, parse_date};
use crate::error::{CoreError, Result};
use crate::models::{NewSeason, Season, SeasonPage, UpdateSeason};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// The namespace every derived season id is minted under (RFC 9562 §6.6).
///
/// An arbitrary constant, fixed on 2026-09-20 and never to be changed: it is
/// half of what every season id in every database is made of, so a different
/// value would give the same campaign a different id and stop two devices
/// agreeing — which is the entire point of deriving it.
const SEASON_NAMESPACE: Uuid = Uuid::from_u128(0x7e44_a2c0_5f31_4b8e_9d62_1c0a_7b3f_5e28);

/// A campaign's id: the same on every device that records the same campaign.
///
/// **The one id in the schema that is not `Uuid::now_v7()`**, and the reason is
/// the merge (docs/sync.md → Seasons created on two devices). Two phones both
/// starting "2026/2027" offline would otherwise write two unrelated books whose
/// records could never be brought together; derived, they write one register
/// twice, which version vectors and the clock already know how to resolve.
///
/// The parts are joined with `|`, which cannot occur in a UUID or an ISO date,
/// so no two different campaigns can hash the same bytes.
///
/// **Mint with it; never look up with it.** `update_season` may move
/// `starts_on`, and the id does not follow — ids are immutable, which is what
/// keeps every record pointing at the same book. So an id derived from today's
/// dates need not be the id the row actually has.
pub fn season_id(farm_id: &str, starts_on: &str, ends_on: &str) -> String {
    let name = format!("{farm_id}|{starts_on}|{ends_on}");
    Uuid::new_v5(&SEASON_NAMESPACE, name.as_bytes()).to_string()
}

/// Create a campaign's book — or bring back the one that was deleted.
///
/// Because [`season_id`] is derived, a campaign has ONE id on this farm forever,
/// and a soft-deleted book still holds it. Creating it again is therefore not a
/// second book but the same one returning: its row is revived in place, with
/// whatever name and bounds this call gives it, and logged as the update to
/// that register that it is. **It comes back empty**: what was deleted with it
/// stays removed, and its page offers it back for as long as a deleted book's
/// records can be brought back (`super::removed_with_book`).
///
/// Minting a fresh id instead would be the one thing that must not happen: two
/// devices would then hold two books for one campaign with no way to tell they
/// were the same, which is the whole failure deriving the id exists to prevent.
/// A book of these dates that is still LIVE is a different matter and refused
/// (`season_dates_taken`).
pub fn insert_season(conn: &mut Connection, new: NewSeason, actor: Option<&str>) -> Result<Season> {
    let custom_label = trimmed(new.custom_label);
    let label = season_label(&new.starts_on, &new.ends_on, custom_label.as_deref())?;
    let id = season_id(&new.farm_id, &new.starts_on, &new.ends_on);
    let tx = begin(conn, actor)?;
    ensure_label_free(&tx, &new.farm_id, &label, None)?;
    if let Some(buried) = season_by_id(&tx, &id)? {
        if buried.deleted_at.is_none() {
            return Err(CoreError::Invalid("season_dates_taken"));
        }
        let revived = revive_season(&tx, buried, label, custom_label, new.starts_on, new.ends_on)?;
        tx.commit()?;
        return Ok(revived);
    }
    let now = now_utc_iso();
    let season = Season {
        id,
        farm_id: new.farm_id,
        label,
        custom_label,
        starts_on: new.starts_on,
        ends_on: new.ends_on,
        status: "active".to_string(),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO season (id, farm_id, label, custom_label, starts_on, ends_on, status,
                             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            season.id,
            season.farm_id,
            season.label,
            season.custom_label,
            season.starts_on,
            season.ends_on,
            season.status,
            season.created_at,
            season.updated_at
        ],
    )?;
    let stamp = tx.register("season", &season.id, Some(&season.id))?;
    log_insert(&tx, &stamp, "season", &season.id, &season)?;
    tx.commit()?;
    Ok(season)
}

/// The most books one page of the list may carry, whatever the caller asks.
pub const SEASON_PAGE_MAX: i64 = 500;

/// One page of the record book list: the live seasons of live farms, the latest
/// ending first, then by farm name, and the total across every page. A deleted
/// farm's books leave the list with it, as its entry in every farm picker
/// already does; their rows are untouched.
///
/// **Paged because it is the one list that grows with farms × years**: a
/// holding adds a book or more a year, and a cooperative adds one per member.
/// Measured 2026-09-16 in headless Chrome under a 6× CPU throttle, the list
/// painted 100 books in ~55 ms and 4 000 in ~3 s — the rows rendered are the
/// cost (docs/data-model.md → "Indexes and query scope"). `limit` is clamped to
/// 1..=[`SEASON_PAGE_MAX`] and a negative `offset` reads as 0.
///
/// The page is chosen over the narrow sort keys and only then joined back to
/// its rows: sorting whole rows made the last page of 40 000 books three times
/// slower. The name order is SQLite's BINARY one, so a page boundary may fall
/// differently from the collated order the view shows within a page; a page is
/// re-collated on screen, a set of pages cannot be.
pub fn list_seasons(conn: &Connection, limit: i64, offset: i64) -> Result<SeasonPage> {
    let limit = limit.clamp(1, SEASON_PAGE_MAX);
    let offset = offset.max(0);
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM season
         JOIN farm ON farm.id = season.farm_id
         WHERE season.deleted_at IS NULL AND farm.deleted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT season.* FROM (
             SELECT season.id AS id, season.ends_on AS ends_on, farm.name AS name
             FROM season
             JOIN farm ON farm.id = season.farm_id
             WHERE season.deleted_at IS NULL AND farm.deleted_at IS NULL
             ORDER BY season.ends_on DESC, farm.name, season.id
             LIMIT ?1 OFFSET ?2
         ) AS page
         JOIN season ON season.id = page.id
         ORDER BY page.ends_on DESC, page.name, page.id",
    )?;
    let seasons = stmt
        .query_map(params![limit, offset], map_season)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SeasonPage { total, seasons })
}

/// One live season of a live farm — the record book a page was opened on.
/// `NotFound` for a deleted season or a deleted farm's, the same population
/// [`list_seasons`] offers.
pub fn get_season(conn: &Connection, id: &str) -> Result<Season> {
    conn.query_row(
        "SELECT season.* FROM season
         JOIN farm ON farm.id = season.farm_id
         WHERE season.id = ?1 AND season.deleted_at IS NULL AND farm.deleted_at IS NULL",
        [id],
        map_season,
    )
    .optional()?
    .ok_or(CoreError::NotFound)
}

/// What a list read across books calls a book: its name, and its farm's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookName {
    pub label: String,
    pub farm: String,
}

/// The names of many books at once, by id — removed books included, since a
/// list may name a record left in one. One statement per 500 books, and none
/// for none.
///
/// The farm comes with the name because a book's name is only unique within
/// its farm: two farms each keep a "2025/2026", and a list of duplicates or
/// conflicts read across every book has to say whose it is.
pub(crate) fn book_names(
    conn: &Connection,
    ids: &[String],
) -> Result<std::collections::HashMap<String, BookName>> {
    let found = crate::sql::children_by_parent(
        conn,
        "SELECT season.id, season.label, farm.name FROM season
         JOIN farm ON farm.id = season.farm_id
         WHERE season.id IN ({ids})",
        ids,
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                BookName {
                    label: row.get(1)?,
                    farm: row.get(2)?,
                },
            ))
        },
        |(id, _)| id.clone(),
    )?;
    Ok(found
        .into_iter()
        .filter_map(|(id, mut rows)| rows.pop().map(|(_, name)| (id, name)))
        .collect())
}

/// A farm's live books, the latest ending first — what a screen offers when a
/// person picks any one of the farm's books: the one records left in a removed
/// book go back into (docs/sync.md → Records in a removed book). Empty for a
/// removed farm, whose books leave every list with it.
///
/// Not paged, unlike [`list_seasons`]: a farm adds a book or two a year, so
/// this is one farm's campaigns and never the whole database's. A seek on
/// `idx_season_farm`, so it reads the same however many other farms there are.
pub fn list_farm_seasons(conn: &Connection, farm_id: &str) -> Result<Vec<Season>> {
    let mut stmt = conn.prepare(
        "SELECT season.* FROM season
         JOIN farm ON farm.id = season.farm_id
         WHERE season.farm_id = ?1 AND season.deleted_at IS NULL AND farm.deleted_at IS NULL
         ORDER BY season.ends_on DESC, season.id",
    )?;
    let seasons = stmt
        .query_map([farm_id], map_season)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(seasons)
}

/// Full-row update; the submitted state replaces the stored one, and the name is
/// derived again, so correcting a date renames a book the dates name. Safe at
/// any time: records reference the season by id, and the printed cuaderno takes
/// its name from the season row precisely so a correction here reaches the
/// document.
pub fn update_season(
    conn: &mut Connection,
    id: &str,
    update: UpdateSeason,
    actor: Option<&str>,
) -> Result<Season> {
    let custom_label = trimmed(update.custom_label);
    let label = season_label(&update.starts_on, &update.ends_on, custom_label.as_deref())?;
    let tx = begin(conn, actor)?;
    let before = tx
        .query_row(
            "SELECT * FROM season WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_season,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    ensure_label_free(&tx, &before.farm_id, &label, Some(id))?;

    let mut after = before.clone();
    after.label = label;
    after.custom_label = custom_label;
    after.starts_on = update.starts_on;
    after.ends_on = update.ends_on;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE season SET label = ?2, custom_label = ?3, starts_on = ?4, ends_on = ?5,
                           updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            after.label,
            after.custom_label,
            after.starts_on,
            after.ends_on,
            after.updated_at
        ],
    )?;
    let stamp = tx.register("season", id, Some(id))?;
    log_update(&tx, &stamp, "season", id, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// The book's own row removed, inside a caller's change set — for deleting a
/// book with its records, and for merging two books, which moves one's records
/// and deletes it (docs/sync.md → Deleting a book with its records, Merging two
/// books). Checks nothing about what the book holds: that is the caller's.
pub(crate) fn soft_delete_season_tx(tx: &WriteTx, id: &str) -> Result<()> {
    let before = tx
        .query_row(
            "SELECT * FROM season WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_season,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE season SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    let stamp = tx.register("season", id, Some(id))?;
    log_delete(tx, &stamp, "season", id, &before, Some(&after))?;
    Ok(())
}

/// Which of two books a merge keeps unless the person picks the other: the one
/// named by its dates when exactly one of them is, otherwise the older.
///
/// **The same answer on every device**, from what both books' rows say, so two
/// people merging one pair on two devices pre-selected alike and move the
/// records the same way. Two opposite choices would delete both books
/// (docs/sync.md → Merging two books, P5). Named by its dates first because
/// that is the book a collision did NOT rename: the other is the "bis" someone
/// typed to get the import through.
pub fn kept_by_default<'a>(first: &'a Season, second: &'a Season) -> &'a Season {
    match (first.custom_label.is_none(), second.custom_label.is_none()) {
        (true, false) => first,
        (false, true) => second,
        _ => {
            let age = |season: &Season| (season.created_at.clone(), season.id.clone());
            if age(second) < age(first) {
                second
            } else {
                first
            }
        }
    }
}

/// The name a book goes by: the farmer's own when they gave one, otherwise its
/// dates' years — "2025/2026" for a campaign that spans the new year, "2026" for
/// one that does not. Refuses dates that are not `YYYY-MM-DD`
/// (`InvalidDate`) and an end before the start (`invalid_date_interval`, the
/// registers' own code for it).
///
/// Public because it is the whole of the naming rule, and so the thing to test:
/// the insert and the update only store what it answers.
pub fn season_label(starts_on: &str, ends_on: &str, custom_label: Option<&str>) -> Result<String> {
    let start = parse_date(starts_on)?;
    let end = parse_date(ends_on)?;
    if end < start {
        return Err(CoreError::Invalid("invalid_date_interval"));
    }
    if let Some(label) = custom_label {
        return Ok(label.to_string());
    }
    Ok(if start.year() == end.year() {
        end.year().to_string()
    } else {
        format!("{}/{}", start.year(), end.year())
    })
}

/// A name typed as spaces is no name: the dates name the book.
fn trimmed(label: Option<String>) -> Option<String> {
    label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
}

/// One live book per farm per name. The partial unique index on `season` would
/// refuse the row anyway; checking first is what turns that into a refusal the
/// form can explain. `except_id` is the season being corrected, which may of
/// course keep its own name.
fn ensure_label_free(
    tx: &Transaction,
    farm_id: &str,
    label: &str,
    except_id: Option<&str>,
) -> Result<()> {
    if book_using_label(tx, farm_id, label, except_id)?.is_some() {
        return Err(CoreError::Invalid("season_name_taken"));
    }
    Ok(())
}

/// The live book of `farm_id` already going by `label`, if there is one.
///
/// Two callers, for two different reasons, which is why it is a query and not
/// just a guard. [`ensure_label_free`] asks before a farmer's own write, so the
/// form can explain itself. **The merge asks before applying a book that came
/// from another device** — because `(farm_id, label)` is UNIQUE and two devices
/// can reach the same name independently, so the collision has to be refused
/// with something a person can act on rather than as a constraint failure
/// (docs/sync.md → Seasons created on two devices).
pub fn book_using_label(
    conn: &Connection,
    farm_id: &str,
    label: &str,
    except_id: Option<&str>,
) -> Result<Option<Season>> {
    // `id IS NOT ?3` rather than `!=`: with no season to except, the parameter
    // is NULL, and `id != NULL` would match nothing.
    Ok(conn
        .query_row(
            "SELECT * FROM season
             WHERE farm_id = ?1 AND label = ?2 AND deleted_at IS NULL AND id IS NOT ?3",
            params![farm_id, label, except_id],
            map_season,
        )
        .optional()?)
}

/// A book by id whatever its state, deleted ones included — which `get_season`
/// deliberately will not do. Only [`insert_season`] needs it, to tell a live
/// book of these dates from a buried one.
fn season_by_id(tx: &Transaction, id: &str) -> Result<Option<Season>> {
    Ok(tx
        .query_row("SELECT * FROM season WHERE id = ?1", [id], map_season)
        .optional()?)
}

/// Bring a soft-deleted book back, with the name and bounds the farmer just
/// gave it. Logged as an update, because that is what it is: the register
/// existed, and its version vector carries on from where it stopped.
pub(super) fn revive_season(
    tx: &WriteTx,
    before: Season,
    label: String,
    custom_label: Option<String>,
    starts_on: String,
    ends_on: String,
) -> Result<Season> {
    let mut after = before.clone();
    after.label = label;
    after.custom_label = custom_label;
    after.starts_on = starts_on;
    after.ends_on = ends_on;
    after.status = "active".to_string();
    after.deleted_at = None;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE season SET label = ?2, custom_label = ?3, starts_on = ?4, ends_on = ?5,
                           status = ?6, deleted_at = NULL, updated_at = ?7
         WHERE id = ?1",
        params![
            after.id,
            after.label,
            after.custom_label,
            after.starts_on,
            after.ends_on,
            after.status,
            after.updated_at
        ],
    )?;
    let stamp = tx.register("season", &after.id, Some(&after.id))?;
    log_update(tx, &stamp, "season", &after.id, &before, &after)?;
    Ok(after)
}

pub(super) fn map_season(row: &Row) -> rusqlite::Result<Season> {
    Ok(Season {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        label: row.get("label")?,
        custom_label: row.get("custom_label")?,
        starts_on: row.get("starts_on")?,
        ends_on: row.get("ends_on")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- the derived id ----------------------------------------------------
    //
    // docs/sync.md → "Seasons created on two devices": two devices creating one
    // campaign must mint the SAME id, so that the merge sees one register
    // written twice rather than two books to reconcile.

    const FARM: &str = "0192f3a4-0000-7000-8000-00000000000f";
    const OTHER_FARM: &str = "0192f3a4-0000-7000-8000-0000000000aa";

    #[test]
    fn one_campaign_gets_one_id_on_every_device() {
        assert_eq!(
            season_id(FARM, "2026-09-01", "2027-08-31"),
            season_id(FARM, "2026-09-01", "2027-08-31")
        );
    }

    #[test]
    fn the_same_campaign_of_another_farm_is_another_book() {
        assert_ne!(
            season_id(FARM, "2026-09-01", "2027-08-31"),
            season_id(OTHER_FARM, "2026-09-01", "2027-08-31")
        );
    }

    #[test]
    fn different_bounds_are_different_books_however_close() {
        // One day apart is a different id — which is exactly the gap the
        // rename-and-report path exists to cover, since both are "2026/2027".
        assert_ne!(
            season_id(FARM, "2026-09-01", "2027-08-31"),
            season_id(FARM, "2026-09-02", "2027-08-31")
        );
        assert_ne!(
            season_id(FARM, "2026-09-01", "2027-08-31"),
            season_id(FARM, "2026-09-01", "2027-08-30")
        );
    }

    #[test]
    fn the_name_the_farmer_gives_it_does_not_change_the_id() {
        // The id is the campaign; the label is what it is called. Two devices
        // naming one campaign differently still write one register.
        let id = season_id(FARM, "2026-09-01", "2027-08-31");
        assert_eq!(id, season_id(FARM, "2026-09-01", "2027-08-31"));
        assert_eq!(
            season_label("2026-09-01", "2027-08-31", None).unwrap(),
            "2026/2027"
        );
        assert_eq!(
            season_label("2026-09-01", "2027-08-31", Some("Campaña buena")).unwrap(),
            "Campaña buena"
        );
    }

    #[test]
    fn the_id_is_a_uuid_in_the_form_every_other_id_uses() {
        let id = season_id(FARM, "2026-09-01", "2027-08-31");
        assert_eq!(id.len(), 36, "hyphenated, like every id in this schema");
        assert_eq!(Uuid::parse_str(&id).unwrap().to_string(), id);
        assert_eq!(
            Uuid::parse_str(&id).unwrap().get_version_num(),
            5,
            "name-based, not the v7 the rest of the schema mints"
        );
    }

    #[test]
    fn the_fields_cannot_be_slid_past_each_other() {
        // A separator that cannot occur inside a farm id or an ISO date: without
        // one, ("ab", "c") and ("a", "bc") would hash the same bytes.
        assert_ne!(
            season_id("a", "2026-09-01", "2027-08-31"),
            season_id("a2026-09-01", "", "2027-08-31")
        );
    }

    /// The naming rule, which is the spec for every book the app names itself.
    #[test]
    fn a_campaign_spanning_the_new_year_is_named_by_both_years() {
        assert_eq!(
            season_label("2025-09-01", "2026-08-31", None).unwrap(),
            "2025/2026"
        );
    }

    #[test]
    fn a_campaign_within_one_year_is_named_by_that_year() {
        assert_eq!(
            season_label("2026-01-01", "2026-12-31", None).unwrap(),
            "2026"
        );
        assert_eq!(
            season_label("2026-03-01", "2026-06-30", None).unwrap(),
            "2026"
        );
    }

    #[test]
    fn a_campaign_of_a_single_day_is_still_a_campaign() {
        assert_eq!(
            season_label("2026-05-01", "2026-05-01", None).unwrap(),
            "2026"
        );
    }

    #[test]
    fn a_campaign_longer_than_a_year_names_its_first_and_last_years() {
        assert_eq!(
            season_label("2024-10-01", "2026-03-31", None).unwrap(),
            "2024/2026"
        );
    }

    #[test]
    fn the_farmers_own_name_wins_over_the_dates() {
        assert_eq!(
            season_label("2026-03-01", "2026-06-30", Some("2026 primavera")).unwrap(),
            "2026 primavera"
        );
    }

    #[test]
    fn an_end_before_the_start_is_refused() {
        assert!(matches!(
            season_label("2026-08-31", "2025-09-01", None),
            Err(CoreError::Invalid("invalid_date_interval"))
        ));
        // Refused even with a name: the dates are still the campaign's bounds.
        assert!(matches!(
            season_label("2026-08-31", "2025-09-01", Some("Mal")),
            Err(CoreError::Invalid("invalid_date_interval"))
        ));
    }

    #[test]
    fn a_date_that_is_not_a_date_is_refused() {
        assert!(matches!(
            season_label("2026-02-30", "2026-08-31", None),
            Err(CoreError::InvalidDate(_))
        ));
        assert!(matches!(
            season_label("", "2026-08-31", None),
            Err(CoreError::InvalidDate(_))
        ));
    }

    #[test]
    fn a_name_of_spaces_is_no_name() {
        assert_eq!(trimmed(Some("   ".into())), None);
        assert_eq!(trimmed(Some("  2026 bis ".into())), Some("2026 bis".into()));
        assert_eq!(trimmed(None), None);
    }
}
