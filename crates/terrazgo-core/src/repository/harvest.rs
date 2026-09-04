// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commercialised harvest — model section 5.
//!
//! In core rather than in the CUE module: what leaves the holding and to whom
//! is whole-farm data the costs and analytics modules will want, and modules
//! never depend on each other.
//!
//! Fully correctable, like the treated-seed register and unlike
//! `treatment_record`: the record holds no snapshot of another row's identity,
//! so there is nothing a later edit elsewhere could rewrite — which is exactly
//! the condition the snapshot columns exist to handle. The per-plot crop
//! snapshots are the frozen *printed* crop, re-resolved when a correction
//! restates the plots, because restating them is the same act as re-entering
//! the row.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{
    HarvestPlot, HarvestRecord, HarvestRecordDetail, NewHarvestPlot, NewHarvestRecord,
    UpdateHarvestRecord,
};
use crate::sql::children_by_parent;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

pub fn insert_harvest_record(
    conn: &mut Connection,
    new: NewHarvestRecord,
    actor: Option<&str>,
) -> Result<HarvestRecordDetail> {
    validate_name(&new.product_name)?;
    let buyer_name = validated_buyer(&new.buyer_name)?;
    validate_quantity(new.quantity_value, new.quantity_unit_code.as_deref())?;

    let tx = conn.transaction()?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;

    let now = now_utc_iso();
    let record = HarvestRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        harvested_on: new.harvested_on,
        product_name: new.product_name.trim().to_string(),
        plant_product_code: new.plant_product_code,
        quantity_value: new.quantity_value,
        quantity_unit_code: new.quantity_unit_code,
        delivery_note_ref: blank_to_none(new.delivery_note_ref),
        lot_number: blank_to_none(new.lot_number),
        buyer_name,
        buyer_tax_id: blank_to_none(new.buyer_tax_id),
        buyer_address: blank_to_none(new.buyer_address),
        buyer_registry_number: blank_to_none(new.buyer_registry_number),
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO harvest_record (
            id, season_id, farm_id, harvested_on, product_name, plant_product_code,
            quantity_value, quantity_unit_code, delivery_note_ref, lot_number,
            buyer_name, buyer_tax_id, buyer_address, buyer_registry_number,
            notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.harvested_on,
            record.product_name,
            record.plant_product_code,
            record.quantity_value,
            record.quantity_unit_code,
            record.delivery_note_ref,
            record.lot_number,
            record.buyer_name,
            record.buyer_tax_id,
            record.buyer_address,
            record.buyer_registry_number,
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
        "harvest_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(HarvestRecordDetail {
        record,
        plots: plot_rows,
    })
}

/// Full-row correction, plus the origin plots reconciled from the submitted
/// state: rows that stayed are updated in place (so the audit trail reads as a
/// correction, not a replacement), rows that went are removed, new ones are
/// inserted — each logged on its own.
pub fn update_harvest_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateHarvestRecord,
    actor: Option<&str>,
) -> Result<HarvestRecordDetail> {
    validate_name(&update.product_name)?;
    let buyer_name = validated_buyer(&update.buyer_name)?;
    validate_quantity(update.quantity_value, update.quantity_unit_code.as_deref())?;

    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM harvest_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;

    let mut after = before.clone();
    after.harvested_on = update.harvested_on;
    after.product_name = update.product_name.trim().to_string();
    after.plant_product_code = update.plant_product_code;
    after.quantity_value = update.quantity_value;
    after.quantity_unit_code = update.quantity_unit_code;
    after.delivery_note_ref = blank_to_none(update.delivery_note_ref);
    after.lot_number = blank_to_none(update.lot_number);
    after.buyer_name = buyer_name;
    after.buyer_tax_id = blank_to_none(update.buyer_tax_id);
    after.buyer_address = blank_to_none(update.buyer_address);
    after.buyer_registry_number = blank_to_none(update.buyer_registry_number);
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE harvest_record SET
            harvested_on = ?2, product_name = ?3, plant_product_code = ?4, quantity_value = ?5,
            quantity_unit_code = ?6, delivery_note_ref = ?7, lot_number = ?8,
            buyer_name = ?9, buyer_tax_id = ?10, buyer_address = ?11,
            buyer_registry_number = ?12, notes = ?13, updated_at = ?14
         WHERE id = ?1",
        params![
            id,
            after.harvested_on,
            after.product_name,
            after.plant_product_code,
            after.quantity_value,
            after.quantity_unit_code,
            after.delivery_note_ref,
            after.lot_number,
            after.buyer_name,
            after.buyer_tax_id,
            after.buyer_address,
            after.buyer_registry_number,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "harvest_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    tx.commit()?;
    Ok(HarvestRecordDetail {
        record: after,
        plots: plot_rows,
    })
}

pub fn soft_delete_harvest_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM harvest_record WHERE id = ?1 AND deleted_at IS NULL",
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
        "UPDATE harvest_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(
        &tx,
        "harvest_record",
        id,
        Some(&before.season_id),
        actor,
        &before,
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_harvest_record(conn: &Connection, id: &str) -> Result<HarvestRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM harvest_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let plots = plots_of(conn, &record.id)?;
    Ok(HarvestRecordDetail { record, plots })
}

/// Every harvest of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
pub fn list_harvest_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<HarvestRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM harvest_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY harvested_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Oldest first, the order a record book reads in.
pub fn list_harvest_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<HarvestRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM harvest_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY harvested_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any harvest hangs off this season — half the guard
/// `soft_delete_season` uses. The module-owned registers are the other half,
/// chained by the shell (core may never query a module's table).
///
/// Soft-deleted records count, like `season_has_treatments`: their audit
/// history is only reachable through the season they belong to, so hiding the
/// season would hide them for good.
pub(super) fn season_has_harvests(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM harvest_record WHERE season_id = ?1)",
        [season_id],
        |row| row.get(0),
    )?;
    Ok(held)
}

// --- reconcile -------------------------------------------------------------

/// Reconcile the origin plots against the submitted state — the 3-way match the
/// extension tables use, one plot at a time.
fn reconcile_plots(
    tx: &Transaction,
    record: &HarvestRecord,
    desired: Vec<NewHarvestPlot>,
    actor: Option<&str>,
) -> Result<Vec<HarvestPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. These
    // are pure children — they live and die with the sale, and soft-deleting
    // them would leave section 5 pointing at parcels nobody harvested.
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute("DELETE FROM harvest_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "harvest_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&HarvestPlot>,
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
                let (crop_name, variety) = crop_snapshot(tx, want.crop_id.as_deref())?;
                if existing.crop_id != want.crop_id {
                    let mut after = existing.clone();
                    after.crop_id = want.crop_id;
                    after.crop_name_snapshot = crop_name;
                    after.variety_snapshot = variety;
                    tx.execute(
                        "UPDATE harvest_plot
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
                        "harvest_plot",
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
    harvest_record_id: &str,
    season_id: &str,
    plot: NewHarvestPlot,
    actor: Option<&str>,
) -> Result<HarvestPlot> {
    let (crop_name, variety) = crop_snapshot(tx, plot.crop_id.as_deref())?;
    let row = HarvestPlot {
        id: Uuid::now_v7().to_string(),
        harvest_record_id: harvest_record_id.to_string(),
        plot_id: plot.plot_id,
        crop_id: plot.crop_id,
        crop_name_snapshot: crop_name,
        variety_snapshot: variety,
    };
    tx.execute(
        "INSERT INTO harvest_plot (
            id, harvest_record_id, plot_id, crop_id, crop_name_snapshot, variety_snapshot
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.harvest_record_id,
            row.plot_id,
            row.crop_id,
            row.crop_name_snapshot,
            row.variety_snapshot
        ],
    )?;
    log_insert(tx, "harvest_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

/// Freeze the crop as it reads today, so a later rename cannot rewrite what the
/// printed book said was harvested.
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

fn validated_buyer(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Invalid("empty_buyer_name"));
    }
    Ok(trimmed.to_string())
}

/// A quantity is a value AND its unit or neither — an amount with no unit is
/// not a statement. The set is {kg, t}: what the model measures a sold harvest
/// in.
///
/// The column has carried a foreign key to `unit` since that table moved into
/// core (2026-08-07), but the key and this check answer different questions:
/// the key says the code is a unit at all, this says it is one a harvest can
/// be weighed in. Cubic metres would satisfy the first and be nonsense here.
fn validate_quantity(value: Option<f64>, unit: Option<&str>) -> Result<()> {
    match (value, unit) {
        (None, None) => Ok(()),
        (Some(v), Some(u)) if v > 0.0 && (u == "kg" || u == "t") => Ok(()),
        _ => Err(CoreError::Invalid("invalid_harvest_quantity")),
    }
}

/// Every origin plot must exist and be on this farm. Duplicates are folded —
/// the UNIQUE index would reject them anyway, and a form that lists a plot
/// twice means one origin, not an error.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewHarvestPlot],
) -> Result<Vec<NewHarvestPlot>> {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    for plot in plots {
        if !seen.insert(plot.plot_id.clone()) {
            continue;
        }
        let plot_farm: Option<String> = tx
            .query_row(
                "SELECT farm_id FROM plot WHERE id = ?1",
                [&plot.plot_id],
                |r| r.get(0),
            )
            .optional()?;
        match plot_farm {
            Some(owner) if owner == farm_id => {}
            Some(_) => return Err(CoreError::Invalid("plot_not_on_farm")),
            None => return Err(CoreError::NotFound),
        }
        kept.push(NewHarvestPlot {
            plot_id: plot.plot_id.clone(),
            crop_id: plot.crop_id.clone(),
        });
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
    records: Vec<HarvestRecord>,
) -> Result<Vec<HarvestRecordDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM harvest_plot WHERE harvest_record_id IN ({ids}) ORDER BY harvest_record_id, id",
        &ids,
        map_plot,
        |p| p.harvest_record_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| HarvestRecordDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            record,
        })
        .collect())
}

fn plots_of(conn: &Connection, harvest_record_id: &str) -> Result<Vec<HarvestPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM harvest_plot WHERE harvest_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([harvest_record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, harvest_record_id: &str) -> Result<Vec<HarvestPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM harvest_plot WHERE harvest_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([harvest_record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row) -> rusqlite::Result<HarvestRecord> {
    Ok(HarvestRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        harvested_on: row.get("harvested_on")?,
        product_name: row.get("product_name")?,
        plant_product_code: row.get("plant_product_code")?,
        quantity_value: row.get("quantity_value")?,
        quantity_unit_code: row.get("quantity_unit_code")?,
        delivery_note_ref: row.get("delivery_note_ref")?,
        lot_number: row.get("lot_number")?,
        buyer_name: row.get("buyer_name")?,
        buyer_tax_id: row.get("buyer_tax_id")?,
        buyer_address: row.get("buyer_address")?,
        buyer_registry_number: row.get("buyer_registry_number")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row) -> rusqlite::Result<HarvestPlot> {
    Ok(HarvestPlot {
        id: row.get("id")?,
        harvest_record_id: row.get("harvest_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        crop_name_snapshot: row.get("crop_name_snapshot")?,
        variety_snapshot: row.get("variety_snapshot")?,
    })
}
