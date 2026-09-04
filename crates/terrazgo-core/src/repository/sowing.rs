// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sowing and planting — how a crop began.
//!
//! In core rather than in a module, and for the same reason as
//! [`super::harvest`]: it is harvest's mirror image, the two bracket a crop,
//! and crop planning, costs and analytics will all want it. Core therefore
//! holds the crop's three brackets — `crop` (what is grown), `sowing_record`
//! (how it began), `harvest_record` (what left).
//!
//! It is a register in its own right, and it also feeds two pages of the
//! record book's third decree: model 9.2's "Siembra" column (RD 1048/2022
//! art. 31) and model 9.3's "Fecha de siembra en seco" and "Fecha de
//! inundación" (art. 45.2). **Neither of those makes it an eco-scheme table**:
//! core may not reference `module-ecoscheme`'s practice lookup, and a sowing is
//! a farm event under no decree in particular. What marks one as a *cultivo
//! bajo agua* is `flooded_on`, a core-native fact.
//!
//! Fully correctable, like harvest and unlike `treatment_record`: the record
//! holds no snapshot of another row's identity, so nothing edited elsewhere can
//! rewrite it. The per-plot crop snapshots are the frozen crop, re-resolved
//! when a correction restates the plots — restating them is the same act as
//! re-entering the row.

use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{
    NewSowingPlot, NewSowingRecord, SowingPlot, SowingRecord, SowingRecordDetail,
    UpdateSowingRecord,
};
use crate::sql::children_by_parent;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::collections::HashSet;
use uuid::Uuid;

pub fn insert_sowing_record(
    conn: &mut Connection,
    new: NewSowingRecord,
    actor: Option<&str>,
) -> Result<SowingRecordDetail> {
    validate_interval(&new.sown_on, new.sowing_end_date.as_deref())?;
    validate_flooding(&new.sown_on, new.flooded_on.as_deref())?;
    validate_seed_quantity(new.seed_quantity_kg)?;

    let tx = conn.transaction()?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;

    let now = now_utc_iso();
    let record = SowingRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        kind_code: new.kind_code,
        sown_on: new.sown_on,
        sowing_end_date: new.sowing_end_date,
        flooded_on: new.flooded_on,
        seed_quantity_kg: new.seed_quantity_kg,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO sowing_record (
            id, season_id, farm_id, kind_code, sown_on, sowing_end_date,
            flooded_on, seed_quantity_kg, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.kind_code,
            record.sown_on,
            record.sowing_end_date,
            record.flooded_on,
            record.seed_quantity_kg,
            record.notes,
            record.created_at,
            record.updated_at
        ],
    )?;

    let mut plot_rows = Vec::new();
    for plot in plots {
        plot_rows.push(insert_plot_row(
            &tx,
            &record.id,
            &record.season_id,
            plot,
            actor,
        )?);
    }

    log_insert(
        &tx,
        "sowing_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(SowingRecordDetail {
        record,
        plots: plot_rows,
    })
}

/// Full-row correction, plots reconciled from the submitted state.
///
/// **This is the ordinary way `flooded_on` gets filled.** A rice grower dry-sows
/// in April and floods in May: the sowing is annotated within a month of the
/// sowing, and the flooding date joins it a month later — one act, one row,
/// corrected. That is why art. 45.2's dates can share a record at all.
pub fn update_sowing_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateSowingRecord,
    actor: Option<&str>,
) -> Result<SowingRecordDetail> {
    validate_interval(&update.sown_on, update.sowing_end_date.as_deref())?;
    validate_flooding(&update.sown_on, update.flooded_on.as_deref())?;
    validate_seed_quantity(update.seed_quantity_kg)?;

    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM sowing_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;

    let mut after = before.clone();
    after.kind_code = update.kind_code;
    after.sown_on = update.sown_on;
    after.sowing_end_date = update.sowing_end_date;
    after.flooded_on = update.flooded_on;
    after.seed_quantity_kg = update.seed_quantity_kg;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE sowing_record SET
            kind_code = ?2, sown_on = ?3, sowing_end_date = ?4, flooded_on = ?5,
            seed_quantity_kg = ?6, notes = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            after.kind_code,
            after.sown_on,
            after.sowing_end_date,
            after.flooded_on,
            after.seed_quantity_kg,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "sowing_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    tx.commit()?;
    Ok(SowingRecordDetail {
        record: after,
        plots: plot_rows,
    })
}

pub fn soft_delete_sowing_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM sowing_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE sowing_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(
        &tx,
        "sowing_record",
        id,
        Some(&before.season_id),
        actor,
        &before,
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_sowing_record(conn: &Connection, id: &str) -> Result<SowingRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM sowing_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let plots = plots_of(conn, &record.id)?;
    Ok(SowingRecordDetail { record, plots })
}

/// Every sowing of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
pub fn list_sowing_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<SowingRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM sowing_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY sown_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Oldest first, the order a record book reads in.
pub fn list_sowing_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<SowingRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM sowing_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY sown_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any sowing hangs off this season — part of the guard
/// `soft_delete_season` uses. Soft-deleted records count, like the harvests':
/// their audit history is only reachable through the season.
pub(super) fn season_has_sowings(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sowing_record WHERE season_id = ?1)",
        [season_id],
        |row| row.get(0),
    )?;
    Ok(held)
}

// --- reconcile -------------------------------------------------------------

fn reconcile_plots(
    tx: &Transaction,
    record: &SowingRecord,
    desired: Vec<NewSowingPlot>,
    actor: Option<&str>,
) -> Result<Vec<SowingPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. Pure
    // children — they live and die with the sowing.
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute("DELETE FROM sowing_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "sowing_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&SowingPlot>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current.iter().find(|c| c.plot_id == want.plot_id) {
            // Still there: corrected in place, keeping its identity. The crop
            // snapshot is re-resolved, because restating the plot is the same
            // act as re-entering it.
            Some(existing) => {
                if existing.crop_id != want.crop_id {
                    let (crop_name, variety) = crop_snapshot(tx, want.crop_id.as_deref())?;
                    let mut after = existing.clone();
                    after.crop_id = want.crop_id;
                    after.crop_name_snapshot = crop_name;
                    after.variety_snapshot = variety;
                    tx.execute(
                        "UPDATE sowing_plot
                         SET crop_id = ?2, crop_name_snapshot = ?3, variety_snapshot = ?4
                         WHERE id = ?1",
                        params![
                            existing.id,
                            after.crop_id,
                            after.crop_name_snapshot,
                            after.variety_snapshot
                        ],
                    )?;
                    log_update(
                        tx,
                        "sowing_plot",
                        &existing.id,
                        Some(&record.season_id),
                        actor,
                        existing,
                        &after,
                    )?;
                    rows.push(after);
                } else {
                    rows.push(existing.clone());
                }
            }
            None => rows.push(insert_plot_row(
                tx,
                &record.id,
                &record.season_id,
                want,
                actor,
            )?),
        }
    }
    Ok(rows)
}

fn insert_plot_row(
    tx: &Transaction,
    sowing_record_id: &str,
    season_id: &str,
    plot: NewSowingPlot,
    actor: Option<&str>,
) -> Result<SowingPlot> {
    let (crop_name, variety) = crop_snapshot(tx, plot.crop_id.as_deref())?;
    let row = SowingPlot {
        id: Uuid::now_v7().to_string(),
        sowing_record_id: sowing_record_id.to_string(),
        plot_id: plot.plot_id,
        crop_id: plot.crop_id,
        crop_name_snapshot: crop_name,
        variety_snapshot: variety,
    };
    tx.execute(
        "INSERT INTO sowing_plot (
            id, sowing_record_id, plot_id, crop_id, crop_name_snapshot, variety_snapshot
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.sowing_record_id,
            row.plot_id,
            row.crop_id,
            row.crop_name_snapshot,
            row.variety_snapshot
        ],
    )?;
    log_insert(tx, "sowing_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

/// Freeze the crop as it reads today, so a later rename cannot rewrite what the
/// book said was sown.
fn crop_snapshot(
    tx: &Transaction,
    crop_id: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    match crop_id {
        Some(id) => tx
            .query_row(
                "SELECT species_name, variety FROM crop WHERE id = ?1",
                [id],
                |r| Ok((Some(r.get::<_, String>(0)?), r.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or(CoreError::NotFound),
        None => Ok((None, None)),
    }
}

// --- validation ------------------------------------------------------------

/// A sowing must not end before it starts.
fn validate_interval(start: &str, end: Option<&str>) -> Result<()> {
    match end {
        None => Ok(()),
        // ISO 8601 date-only strings compare lexicographically.
        Some(end) if end >= start => Ok(()),
        Some(_) => Err(CoreError::Invalid("invalid_date_interval")),
    }
}

/// A field is flooded after it is sown, never before — model 9.3 prints the two
/// dates in that order and art. 45.2 names "siembra" ahead of "inundación",
/// because the register is about *siembra en seco*: the seed goes into dry
/// ground and the water follows.
fn validate_flooding(sown_on: &str, flooded_on: Option<&str>) -> Result<()> {
    match flooded_on {
        None => Ok(()),
        Some(flooded) if flooded >= sown_on => Ok(()),
        Some(_) => Err(CoreError::Invalid("flooded_before_sown")),
    }
}

/// Kilograms of seed, when stated. Zero is not a sowing and a negative weight
/// is a typo; an unstated amount stays `None`, because the decree asks for
/// dates and this column exists only because the twin requires it.
fn validate_seed_quantity(value: Option<f64>) -> Result<()> {
    match value {
        None => Ok(()),
        Some(kg) if kg > 0.0 => Ok(()),
        Some(_) => Err(CoreError::Invalid("invalid_seed_quantity")),
    }
}

/// Every sown plot must exist and be on this farm. Duplicates are folded — the
/// UNIQUE index would reject them anyway, and a form listing a plot twice means
/// one sowing, not an error.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewSowingPlot],
) -> Result<Vec<NewSowingPlot>> {
    let mut seen = HashSet::new();
    let mut kept: Vec<NewSowingPlot> = Vec::new();
    for plot in plots {
        if !seen.insert(plot.plot_id.clone()) {
            continue;
        }
        let plot_farm: String = tx
            .query_row(
                "SELECT farm_id FROM plot WHERE id = ?1",
                [&plot.plot_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(CoreError::NotFound)?;
        if plot_farm != farm_id {
            return Err(CoreError::Invalid("plot_not_on_farm"));
        }
        kept.push(plot.clone());
    }
    if kept.is_empty() {
        return Err(CoreError::Invalid("no_plots"));
    }
    Ok(kept)
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

/// [`with_details`]-style hydration for a whole list, in ONE child statement
/// rather than one per record. The single-record paths keep their point
/// queries: a `WHERE id = ?` has nothing to hoist.
fn all_with_details(
    conn: &Connection,
    records: Vec<SowingRecord>,
) -> Result<Vec<SowingRecordDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM sowing_plot WHERE sowing_record_id IN ({ids}) ORDER BY sowing_record_id, id",
        &ids,
        map_plot,
        |p| p.sowing_record_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| SowingRecordDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            record,
        })
        .collect())
}

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<SowingPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM sowing_plot WHERE sowing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<SowingPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM sowing_plot WHERE sowing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<SowingRecord> {
    Ok(SowingRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        kind_code: row.get("kind_code")?,
        sown_on: row.get("sown_on")?,
        sowing_end_date: row.get("sowing_end_date")?,
        flooded_on: row.get("flooded_on")?,
        seed_quantity_kg: row.get("seed_quantity_kg")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<SowingPlot> {
    Ok(SowingPlot {
        id: row.get("id")?,
        sowing_record_id: row.get("sowing_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        crop_name_snapshot: row.get("crop_name_snapshot")?,
        variety_snapshot: row.get("variety_snapshot")?,
    })
}
