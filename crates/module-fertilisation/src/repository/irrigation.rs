// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 8 — the irrigation register.
//!
//! RD 1051/2022 art. 5.e puts irrigation doses and dates inside the SAME
//! cuaderno duty as fertilisation, on the same one-month deadline, and the
//! binding field list is RD 1311/2012 Anexo III Parte I sección C (letters a,
//! b and l). This is the *record*; scheduling and water balance belong to the
//! future Irrigation module.
//!
//! **Fully correctable from the start**, unlike `treatment_record` was: this
//! table holds no snapshot of another row's identity, so there is nothing a
//! later edit elsewhere could rewrite underneath it — exactly the condition
//! `seed_treatment` established.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::error::{FertilisationError, Result};
use crate::models::{
    IrrigationPlot, IrrigationRecord, IrrigationRecordDetail, NewIrrigationPlot,
    NewIrrigationRecord, UpdateIrrigationRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use uuid::Uuid;

/// The two units a volume of irrigation water can carry. The column has a
/// foreign key to `unit`, but that only says the code is a unit at all — kW
/// would satisfy it. Anexo III C.l asks for cubic metres per hectare, and a
/// meter reading is an absolute volume, so those are the two.
const VOLUME_UNITS: [&str; 2] = ["m3_ha", "m3"];

pub fn insert_irrigation_record(
    conn: &mut Connection,
    new: NewIrrigationRecord,
    actor: Option<&str>,
) -> Result<IrrigationRecordDetail> {
    validate_interval(&new.irrigated_on, new.irrigation_end_date.as_deref())?;
    validate_volume(new.volume_value, &new.volume_unit_code)?;
    validate_water_quality(new.water_nitric_n_mg_l, new.water_soluble_p2o5_mg_l)?;

    let tx = conn.transaction()?;
    validate_method(&tx, &new.irrigation_method_code)?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;
    let origins = validated_origins(&tx, &new.water_origins)?;

    let now = now_utc_iso();
    let record = IrrigationRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        irrigated_on: new.irrigated_on,
        irrigation_end_date: new.irrigation_end_date,
        irrigation_method_code: new.irrigation_method_code,
        volume_value: new.volume_value,
        volume_unit_code: new.volume_unit_code,
        water_nitric_n_mg_l: new.water_nitric_n_mg_l,
        water_soluble_p2o5_mg_l: new.water_soluble_p2o5_mg_l,
        energy_type_code: blank_to_none(new.energy_type_code),
        meter_number: blank_to_none(new.meter_number),
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO irrigation_record (
            id, season_id, farm_id, irrigated_on, irrigation_end_date,
            irrigation_method_code, volume_value, volume_unit_code,
            water_nitric_n_mg_l, water_soluble_p2o5_mg_l, energy_type_code,
            meter_number, notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.irrigated_on,
            record.irrigation_end_date,
            record.irrigation_method_code,
            record.volume_value,
            record.volume_unit_code,
            record.water_nitric_n_mg_l,
            record.water_soluble_p2o5_mg_l,
            record.energy_type_code,
            record.meter_number,
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
    for origin in &origins {
        insert_origin_row(&tx, &record.id, &record.season_id, origin, actor)?;
    }

    log_insert(
        &tx,
        "irrigation_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(IrrigationRecordDetail {
        record,
        plots: plot_rows,
        water_origins: origins,
    })
}

/// Full-row correction, with plots and water origins reconciled from the
/// submitted state: rows that stayed are updated in place (so the audit trail
/// reads as a correction, not a replacement), rows that went are removed, new
/// ones are inserted — each logged on its own.
pub fn update_irrigation_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateIrrigationRecord,
    actor: Option<&str>,
) -> Result<IrrigationRecordDetail> {
    validate_interval(&update.irrigated_on, update.irrigation_end_date.as_deref())?;
    validate_volume(update.volume_value, &update.volume_unit_code)?;
    validate_water_quality(update.water_nitric_n_mg_l, update.water_soluble_p2o5_mg_l)?;

    let tx = conn.transaction()?;
    validate_method(&tx, &update.irrigation_method_code)?;
    let before = tx
        .query_row(
            "SELECT * FROM irrigation_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;
    let origins = validated_origins(&tx, &update.water_origins)?;

    let mut after = before.clone();
    after.irrigated_on = update.irrigated_on;
    after.irrigation_end_date = update.irrigation_end_date;
    after.irrigation_method_code = update.irrigation_method_code;
    after.volume_value = update.volume_value;
    after.volume_unit_code = update.volume_unit_code;
    after.water_nitric_n_mg_l = update.water_nitric_n_mg_l;
    after.water_soluble_p2o5_mg_l = update.water_soluble_p2o5_mg_l;
    after.energy_type_code = blank_to_none(update.energy_type_code);
    after.meter_number = blank_to_none(update.meter_number);
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE irrigation_record SET
            irrigated_on = ?2, irrigation_end_date = ?3, irrigation_method_code = ?4,
            volume_value = ?5, volume_unit_code = ?6, water_nitric_n_mg_l = ?7,
            water_soluble_p2o5_mg_l = ?8, energy_type_code = ?9, meter_number = ?10,
            notes = ?11, updated_at = ?12
         WHERE id = ?1",
        params![
            id,
            after.irrigated_on,
            after.irrigation_end_date,
            after.irrigation_method_code,
            after.volume_value,
            after.volume_unit_code,
            after.water_nitric_n_mg_l,
            after.water_soluble_p2o5_mg_l,
            after.energy_type_code,
            after.meter_number,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "irrigation_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    reconcile_origins(&tx, &after, &origins, actor)?;
    tx.commit()?;
    Ok(IrrigationRecordDetail {
        record: after,
        plots: plot_rows,
        water_origins: origins,
    })
}

pub fn soft_delete_irrigation_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM irrigation_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE irrigation_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "irrigation_record",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_irrigation_record(conn: &Connection, id: &str) -> Result<IrrigationRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM irrigation_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    let water_origins = origins_of(conn, &record.id)?;
    Ok(IrrigationRecordDetail {
        record,
        plots,
        water_origins,
    })
}

/// Oldest first, the order a record book reads in.
pub fn list_irrigation_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<IrrigationRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM irrigation_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY irrigated_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|record| {
            let plots = plots_of(conn, &record.id)?;
            let water_origins = origins_of(conn, &record.id)?;
            Ok(IrrigationRecordDetail {
                record,
                plots,
                water_origins,
            })
        })
        .collect()
}

/// Whether any irrigation hangs off this season — half of the module's arm of
/// the guard the shell chains before deleting a season (see
/// `repository::season_has_records`). Soft-deleted records count, as they do in
/// module-cue: their audit history is only reachable through the season they
/// belong to.
pub(super) fn season_has_irrigation(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM irrigation_record WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- reconciliation --------------------------------------------------------

fn reconcile_plots(
    tx: &Transaction,
    record: &IrrigationRecord,
    desired: Vec<NewIrrigationPlot>,
    actor: Option<&str>,
) -> Result<Vec<IrrigationPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. These
    // are pure children — they live and die with the record, and soft-deleting
    // them would leave the register printing surfaces nobody irrigated.
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute("DELETE FROM irrigation_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "irrigation_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&IrrigationPlot>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current.iter().find(|c| c.plot_id == want.plot_id) {
            // Still there: corrected in place, keeping its identity.
            Some(existing) => {
                if existing.irrigated_area_ha != want.irrigated_area_ha
                    || existing.crop_id != want.crop_id
                {
                    let mut after = existing.clone();
                    after.irrigated_area_ha = want.irrigated_area_ha;
                    after.crop_id = want.crop_id;
                    tx.execute(
                        "UPDATE irrigation_plot SET crop_id = ?2, irrigated_area_ha = ?3
                         WHERE id = ?1",
                        params![existing.id, after.crop_id, after.irrigated_area_ha],
                    )?;
                    log_update(
                        tx,
                        "irrigation_plot",
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

/// Water origins carry no attributes of their own, so there is nothing to
/// correct in place: a code is either claimed or it is not. Removals and
/// additions are both logged, so the trail still says what the farmer once
/// stated the water came from.
fn reconcile_origins(
    tx: &Transaction,
    record: &IrrigationRecord,
    desired: &[String],
    actor: Option<&str>,
) -> Result<()> {
    let current = origin_rows_tx(tx, &record.id)?;
    for (row_id, code) in &current {
        if !desired.iter().any(|d| d == code) {
            tx.execute(
                "DELETE FROM irrigation_water_origin WHERE id = ?1",
                [row_id],
            )?;
            let gone = origin_image(row_id, &record.id, code);
            log_delete(
                tx,
                "irrigation_water_origin",
                row_id,
                Some(&record.season_id),
                actor,
                &gone,
                None::<&serde_json::Value>,
            )?;
        }
    }
    for code in desired {
        if !current.iter().any(|(_, existing)| existing == code) {
            insert_origin_row(tx, &record.id, &record.season_id, code, actor)?;
        }
    }
    Ok(())
}

fn insert_plot_row(
    tx: &Transaction,
    irrigation_record_id: &str,
    season_id: &str,
    plot: NewIrrigationPlot,
    actor: Option<&str>,
) -> Result<IrrigationPlot> {
    let row = IrrigationPlot {
        id: Uuid::now_v7().to_string(),
        irrigation_record_id: irrigation_record_id.to_string(),
        plot_id: plot.plot_id,
        crop_id: plot.crop_id,
        irrigated_area_ha: plot.irrigated_area_ha,
    };
    tx.execute(
        "INSERT INTO irrigation_plot (
            id, irrigation_record_id, plot_id, crop_id, irrigated_area_ha
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.id,
            row.irrigation_record_id,
            row.plot_id,
            row.crop_id,
            row.irrigated_area_ha
        ],
    )?;
    log_insert(tx, "irrigation_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

fn insert_origin_row(
    tx: &Transaction,
    irrigation_record_id: &str,
    season_id: &str,
    origin_code: &str,
    actor: Option<&str>,
) -> Result<()> {
    let id = Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO irrigation_water_origin (id, irrigation_record_id, origin_code)
         VALUES (?1, ?2, ?3)",
        params![id, irrigation_record_id, origin_code],
    )?;
    log_insert(
        tx,
        "irrigation_water_origin",
        &id,
        Some(season_id),
        actor,
        &origin_image(&id, irrigation_record_id, origin_code),
    )?;
    Ok(())
}

/// The complete row image `record_change` requires — the junction has no model
/// struct of its own, so it is built here rather than shipping a type whose
/// only purpose is the log.
fn origin_image(id: &str, irrigation_record_id: &str, origin_code: &str) -> serde_json::Value {
    json!({
        "id": id,
        "irrigation_record_id": irrigation_record_id,
        "origin_code": origin_code,
    })
}

// --- validation ------------------------------------------------------------

/// An interval must not end before it starts. A single-day irrigation leaves
/// the end NULL rather than repeating the start, so a serializer can tell "one
/// day" from "a period that happened to be one day long".
fn validate_interval(start: &str, end: Option<&str>) -> Result<()> {
    match end {
        None => Ok(()),
        // ISO 8601 date-only strings compare lexicographically, which is why
        // the whole app stores them this way.
        Some(end) if end >= start => Ok(()),
        Some(_) => Err(FertilisationError::Invalid("invalid_date_interval")),
    }
}

fn validate_volume(value: f64, unit_code: &str) -> Result<()> {
    // NaN must be rejected too, hence the explicit comparison rather than
    // `value <= 0.0`: every comparison against NaN is false.
    if value.is_nan() || value <= 0.0 {
        return Err(FertilisationError::Invalid("invalid_irrigation_volume"));
    }
    if !VOLUME_UNITS.contains(&unit_code) {
        return Err(FertilisationError::Invalid("invalid_volume_unit"));
    }
    Ok(())
}

/// Anexo III C.l's two water-quality figures. Both optional (art. 17.2 asks
/// for them only when a basin authority or irrigators' community supplies
/// them), but a stated concentration cannot be negative — that is a typo, not
/// a measurement.
fn validate_water_quality(nitric_n: Option<f64>, soluble_p2o5: Option<f64>) -> Result<()> {
    for value in [nitric_n, soluble_p2o5].into_iter().flatten() {
        if value < 0.0 || value.is_nan() {
            return Err(FertilisationError::Invalid("invalid_water_quality"));
        }
    }
    Ok(())
}

fn validate_method(tx: &Transaction, code: &str) -> Result<()> {
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM irrigation_method WHERE code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(FertilisationError::Invalid("unknown_irrigation_method"));
    }
    Ok(())
}

/// Every irrigated plot must exist and be on this farm. Duplicates are folded
/// — the UNIQUE index would reject them anyway, and a form that lists a plot
/// twice means one irrigation, not an error.
///
/// The surface is nullable here where a sowing's is required: the model prints
/// the column, but naming the plot already says what was watered, and an
/// invented hectare figure is worse than a blank cell.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewIrrigationPlot],
) -> Result<Vec<NewIrrigationPlot>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for plot in plots {
        if !seen.insert(plot.plot_id.clone()) {
            continue;
        }
        if let Some(area) = plot.irrigated_area_ha
            && (area <= 0.0 || area.is_nan())
        {
            return Err(FertilisationError::Invalid("nonpositive_area"));
        }
        let plot_farm: String = tx
            .query_row(
                "SELECT farm_id FROM plot WHERE id = ?1",
                [&plot.plot_id],
                |r| r.get(0),
            )
            .map_err(no_rows_to_not_found)?;
        if plot_farm != farm_id {
            return Err(FertilisationError::PlotNotOnFarm {
                plot_id: plot.plot_id.clone(),
                farm_id: farm_id.to_string(),
            });
        }
        kept.push(NewIrrigationPlot {
            plot_id: plot.plot_id.clone(),
            crop_id: plot.crop_id.clone(),
            irrigated_area_ha: plot.irrigated_area_ha,
        });
    }
    if kept.is_empty() {
        return Err(FertilisationError::Invalid("no_plots"));
    }
    Ok(kept)
}

/// Water origins are optional (the twin's `OrigenAgua` is), but a stated one
/// must be a code the export can speak. Duplicates fold, like plots.
///
/// The result is sorted into CATALOGUE order, not submitted order, so that a
/// freshly inserted record and one read back from the database list its
/// sources identically — otherwise the same book would print them one way
/// before a reload and another way after.
fn validated_origins(tx: &Transaction, codes: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for code in codes {
        if !seen.insert(code.clone()) {
            continue;
        }
        let rank: Option<i64> = tx
            .query_row(
                "SELECT rowid FROM water_origin WHERE code = ?1",
                [code],
                |r| r.get(0),
            )
            .optional()?;
        let Some(rank) = rank else {
            return Err(FertilisationError::Invalid("unknown_water_origin"));
        };
        kept.push((rank, code.clone()));
    }
    kept.sort_by_key(|(rank, _)| *rank);
    Ok(kept.into_iter().map(|(_, code)| code).collect())
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<IrrigationPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM irrigation_plot WHERE irrigation_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<IrrigationPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM irrigation_plot WHERE irrigation_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Ordered by the seeded catalogue order, so a book lists sources the way the
/// provider does rather than by insertion accident.
fn origins_of(conn: &Connection, record_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT o.origin_code FROM irrigation_water_origin o
         JOIN water_origin w ON w.code = o.origin_code
         WHERE o.irrigation_record_id = ?1
         ORDER BY w.rowid",
    )?;
    let rows = stmt
        .query_map([record_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(rows)
}

fn origin_rows_tx(tx: &Transaction, record_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = tx.prepare(
        "SELECT id, origin_code FROM irrigation_water_origin
         WHERE irrigation_record_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<IrrigationRecord> {
    Ok(IrrigationRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        irrigated_on: row.get("irrigated_on")?,
        irrigation_end_date: row.get("irrigation_end_date")?,
        irrigation_method_code: row.get("irrigation_method_code")?,
        volume_value: row.get("volume_value")?,
        volume_unit_code: row.get("volume_unit_code")?,
        water_nitric_n_mg_l: row.get("water_nitric_n_mg_l")?,
        water_soluble_p2o5_mg_l: row.get("water_soluble_p2o5_mg_l")?,
        energy_type_code: row.get("energy_type_code")?,
        meter_number: row.get("meter_number")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<IrrigationPlot> {
    Ok(IrrigationPlot {
        id: row.get("id")?,
        irrigation_record_id: row.get("irrigation_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        irrigated_area_ha: row.get("irrigated_area_ha")?,
    })
}
