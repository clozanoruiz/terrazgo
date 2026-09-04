// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 6 — the fertilisation register.
//!
//! RD 1051/2022 art. 5.d makes this binding since 1 January 2026, recorded
//! within one month of each operation, and redirects the field list to
//! RD 1311/2012 Anexo III Parte I sección C — which is wider than the printed
//! model. Each column below cites its letter.
//!
//! Unlike irrigation, this register DOES snapshot another row: the material's
//! name and printed richness freeze at write time, so correcting the registry
//! never rewrites a legal record. The record stays fully correctable all the
//! same — the snapshot is what makes that safe, not what forbids it.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::error::{FertilisationError, Result};
use crate::models::{
    FertilisationPlot, FertilisationRecord, FertilisationRecordDetail, NewFertilisationPlot,
    NewFertilisationRecord, UpdateFertilisationRecord,
};
use crate::siex::NO_PRACTICES_CODE;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::sql::children_by_parent;
use uuid::Uuid;

/// The four rates Anexo III C.j's "por hectárea" can be stated in. The column
/// has a foreign key to `unit`, but that only says the code is a unit at all —
/// the dose of a fertiliser is a rate, so a bare kilogram would answer a
/// different question (the `list_units` split seam 1 of slice 8 made).
const DOSE_UNITS: [&str; 4] = ["kg_ha", "l_ha", "t_ha", "m3_ha"];

/// The three `MACRONUTRIENTES` codes the printed model's "Riqueza N/P/K" cell
/// asks for: N total, P₂O₅ total and K₂O. C.h wants eight values and the
/// registry row carries all of them; these three are what section 6 prints, so
/// these three are what a record freezes.
const RICHNESS_N: &str = "1";
const RICHNESS_P2O5: &str = "6";
const RICHNESS_K2O: &str = "9";

pub fn insert_fertilisation_record(
    conn: &mut Connection,
    new: NewFertilisationRecord,
    actor: Option<&str>,
) -> Result<FertilisationRecordDetail> {
    validate_interval(&new.applied_on, new.application_end_date.as_deref())?;
    validate_dose(new.dose_value, &new.dose_unit_code)?;
    validate_yields(new.yield_estimated_kg_ha, new.yield_final_kg_ha)?;

    let tx = conn.transaction()?;
    validate_codes(
        &tx,
        &new.fertilisation_type_code,
        &new.application_method_code,
    )?;
    validate_machinery(&tx, new.machinery_id.as_deref(), &new.farm_id)?;
    validate_fertigation_link(
        &tx,
        new.irrigation_record_id.as_deref(),
        &new.application_method_code,
        &new.farm_id,
        &new.season_id,
    )?;
    let snapshot = material_snapshot(&tx, &new.fertiliser_material_id)?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;
    let practices = validated_practices(&new.practices)?;

    let now = now_utc_iso();
    let record = FertilisationRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        applied_on: new.applied_on,
        application_end_date: new.application_end_date,
        fertilisation_type_code: new.fertilisation_type_code,
        application_method_code: new.application_method_code,
        dose_value: new.dose_value,
        dose_unit_code: new.dose_unit_code,
        fertiliser_material_id: new.fertiliser_material_id,
        material_name_snapshot: snapshot.name,
        material_code_snapshot: snapshot.material_code,
        richness_n_snapshot: snapshot.n,
        richness_p2o5_snapshot: snapshot.p2o5,
        richness_k2o_snapshot: snapshot.k2o,
        sludge_application: new.sludge_application,
        sustainable_input_management: new.sustainable_input_management,
        machinery_id: blank_to_none(new.machinery_id),
        irrigation_record_id: blank_to_none(new.irrigation_record_id),
        service_company: blank_to_none(new.service_company),
        service_regfer_number: blank_to_none(new.service_regfer_number),
        delivery_note_ref: blank_to_none(new.delivery_note_ref),
        yield_estimated_kg_ha: new.yield_estimated_kg_ha,
        yield_final_kg_ha: new.yield_final_kg_ha,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        INSERT_SQL,
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.applied_on,
            record.application_end_date,
            record.fertilisation_type_code,
            record.application_method_code,
            record.dose_value,
            record.dose_unit_code,
            record.fertiliser_material_id,
            record.material_name_snapshot,
            record.material_code_snapshot,
            record.richness_n_snapshot,
            record.richness_p2o5_snapshot,
            record.richness_k2o_snapshot,
            record.sludge_application,
            record.sustainable_input_management,
            record.machinery_id,
            record.irrigation_record_id,
            record.service_company,
            record.service_regfer_number,
            record.delivery_note_ref,
            record.yield_estimated_kg_ha,
            record.yield_final_kg_ha,
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
    for practice in &practices {
        insert_practice_row(&tx, &record.id, &record.season_id, practice, actor)?;
    }

    log_insert(
        &tx,
        "fertilisation_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(FertilisationRecordDetail {
        record,
        plots: plot_rows,
        practices,
    })
}

/// Full-row correction, plots and good practices reconciled from the submitted
/// state.
///
/// Changing the material **re-takes the snapshot**: a record naming one
/// fertiliser while printing another's richness would be worse than either
/// version of the mistake it was meant to fix. Leaving the material alone keeps
/// what the record already printed, even if the registry entry has been
/// corrected in between — the freeze is what protects a printed legal value
/// from an edit made elsewhere, and a correction may only change what it names
/// (the rule `treatment_record` follows, 2026-08-10).
pub fn update_fertilisation_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateFertilisationRecord,
    actor: Option<&str>,
) -> Result<FertilisationRecordDetail> {
    validate_interval(&update.applied_on, update.application_end_date.as_deref())?;
    validate_dose(update.dose_value, &update.dose_unit_code)?;
    validate_yields(update.yield_estimated_kg_ha, update.yield_final_kg_ha)?;

    let tx = conn.transaction()?;
    validate_codes(
        &tx,
        &update.fertilisation_type_code,
        &update.application_method_code,
    )?;
    let before = tx
        .query_row(
            "SELECT * FROM fertilisation_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    validate_machinery(&tx, update.machinery_id.as_deref(), &before.farm_id)?;
    // Against the SUBMITTED method, not the stored one: a correction that turns
    // a fertigation into a broadcast must drop the link in the same edit.
    validate_fertigation_link(
        &tx,
        update.irrigation_record_id.as_deref(),
        &update.application_method_code,
        &before.farm_id,
        &before.season_id,
    )?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;
    let practices = validated_practices(&update.practices)?;

    let mut after = before.clone();
    after.applied_on = update.applied_on;
    after.application_end_date = update.application_end_date;
    after.fertilisation_type_code = update.fertilisation_type_code;
    after.application_method_code = update.application_method_code;
    after.dose_value = update.dose_value;
    after.dose_unit_code = update.dose_unit_code;
    if update.fertiliser_material_id != before.fertiliser_material_id {
        let snapshot = material_snapshot(&tx, &update.fertiliser_material_id)?;
        after.material_name_snapshot = snapshot.name;
        after.material_code_snapshot = snapshot.material_code;
        after.richness_n_snapshot = snapshot.n;
        after.richness_p2o5_snapshot = snapshot.p2o5;
        after.richness_k2o_snapshot = snapshot.k2o;
    }
    after.fertiliser_material_id = update.fertiliser_material_id;
    after.sludge_application = update.sludge_application;
    after.sustainable_input_management = update.sustainable_input_management;
    after.machinery_id = blank_to_none(update.machinery_id);
    after.irrigation_record_id = blank_to_none(update.irrigation_record_id);
    after.service_company = blank_to_none(update.service_company);
    after.service_regfer_number = blank_to_none(update.service_regfer_number);
    after.delivery_note_ref = blank_to_none(update.delivery_note_ref);
    after.yield_estimated_kg_ha = update.yield_estimated_kg_ha;
    after.yield_final_kg_ha = update.yield_final_kg_ha;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE fertilisation_record SET
            applied_on = ?2, application_end_date = ?3, fertilisation_type_code = ?4,
            application_method_code = ?5, dose_value = ?6, dose_unit_code = ?7,
            fertiliser_material_id = ?8, material_name_snapshot = ?9,
            material_code_snapshot = ?10, richness_n_snapshot = ?11,
            richness_p2o5_snapshot = ?12, richness_k2o_snapshot = ?13,
            sludge_application = ?14, sustainable_input_management = ?15,
            machinery_id = ?16, irrigation_record_id = ?17, service_company = ?18,
            service_regfer_number = ?19, delivery_note_ref = ?20,
            yield_estimated_kg_ha = ?21, yield_final_kg_ha = ?22, notes = ?23,
            updated_at = ?24
         WHERE id = ?1",
        params![
            id,
            after.applied_on,
            after.application_end_date,
            after.fertilisation_type_code,
            after.application_method_code,
            after.dose_value,
            after.dose_unit_code,
            after.fertiliser_material_id,
            after.material_name_snapshot,
            after.material_code_snapshot,
            after.richness_n_snapshot,
            after.richness_p2o5_snapshot,
            after.richness_k2o_snapshot,
            after.sludge_application,
            after.sustainable_input_management,
            after.machinery_id,
            after.irrigation_record_id,
            after.service_company,
            after.service_regfer_number,
            after.delivery_note_ref,
            after.yield_estimated_kg_ha,
            after.yield_final_kg_ha,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "fertilisation_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    reconcile_practices(&tx, &after, &practices, actor)?;
    tx.commit()?;
    Ok(FertilisationRecordDetail {
        record: after,
        plots: plot_rows,
        practices,
    })
}

pub fn soft_delete_fertilisation_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM fertilisation_record WHERE id = ?1 AND deleted_at IS NULL",
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
        "UPDATE fertilisation_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "fertilisation_record",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_fertilisation_record(conn: &Connection, id: &str) -> Result<FertilisationRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM fertilisation_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    let practices = practices_of(conn, &record.id)?;
    Ok(FertilisationRecordDetail {
        record,
        plots,
        practices,
    })
}

/// Oldest first, the order a record book reads in.
/// Every fertilisation record of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
pub fn list_fertilisation_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<FertilisationRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM fertilisation_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY applied_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

pub fn list_fertilisation_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<FertilisationRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM fertilisation_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY applied_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any fertilisation record hangs off this season. Soft-deleted rows
/// count: their audit history is only reachable through the season they belong
/// to.
pub(super) fn season_has_fertilisation(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM fertilisation_record WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- reconciliation --------------------------------------------------------

fn reconcile_plots(
    tx: &Transaction,
    record: &FertilisationRecord,
    desired: Vec<NewFertilisationPlot>,
    actor: Option<&str>,
) -> Result<Vec<FertilisationPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute(
                "DELETE FROM fertilisation_plot WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "fertilisation_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&FertilisationPlot>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current.iter().find(|c| c.plot_id == want.plot_id) {
            Some(existing) => {
                if existing.fertilised_area_ha != want.fertilised_area_ha
                    || existing.crop_id != want.crop_id
                {
                    let mut after = existing.clone();
                    after.fertilised_area_ha = want.fertilised_area_ha;
                    after.crop_id = want.crop_id;
                    tx.execute(
                        "UPDATE fertilisation_plot SET crop_id = ?2, fertilised_area_ha = ?3
                         WHERE id = ?1",
                        params![existing.id, after.crop_id, after.fertilised_area_ha],
                    )?;
                    log_update(
                        tx,
                        "fertilisation_plot",
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

/// Good practices carry no attributes of their own, so there is nothing to
/// correct in place — a practice is either claimed or it is not. Both
/// directions are logged, so the trail keeps saying what the farmer once
/// declared they did.
fn reconcile_practices(
    tx: &Transaction,
    record: &FertilisationRecord,
    desired: &[String],
    actor: Option<&str>,
) -> Result<()> {
    let current = practice_rows_tx(tx, &record.id)?;
    for (row_id, code) in &current {
        if !desired.iter().any(|d| d == code) {
            tx.execute("DELETE FROM fertilisation_practice WHERE id = ?1", [row_id])?;
            let gone = practice_image(row_id, &record.id, code);
            log_delete(
                tx,
                "fertilisation_practice",
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
            insert_practice_row(tx, &record.id, &record.season_id, code, actor)?;
        }
    }
    Ok(())
}

fn insert_plot_row(
    tx: &Transaction,
    fertilisation_record_id: &str,
    season_id: &str,
    plot: NewFertilisationPlot,
    actor: Option<&str>,
) -> Result<FertilisationPlot> {
    let row = FertilisationPlot {
        id: Uuid::now_v7().to_string(),
        fertilisation_record_id: fertilisation_record_id.to_string(),
        plot_id: plot.plot_id,
        crop_id: plot.crop_id,
        fertilised_area_ha: plot.fertilised_area_ha,
    };
    tx.execute(
        "INSERT INTO fertilisation_plot (
            id, fertilisation_record_id, plot_id, crop_id, fertilised_area_ha
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.id,
            row.fertilisation_record_id,
            row.plot_id,
            row.crop_id,
            row.fertilised_area_ha
        ],
    )?;
    log_insert(
        tx,
        "fertilisation_plot",
        &row.id,
        Some(season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

fn insert_practice_row(
    tx: &Transaction,
    fertilisation_record_id: &str,
    season_id: &str,
    practice_code: &str,
    actor: Option<&str>,
) -> Result<()> {
    let id = Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO fertilisation_practice (id, fertilisation_record_id, practice_code)
         VALUES (?1, ?2, ?3)",
        params![id, fertilisation_record_id, practice_code],
    )?;
    log_insert(
        tx,
        "fertilisation_practice",
        &id,
        Some(season_id),
        actor,
        &practice_image(&id, fertilisation_record_id, practice_code),
    )?;
    Ok(())
}

fn practice_image(
    id: &str,
    fertilisation_record_id: &str,
    practice_code: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "fertilisation_record_id": fertilisation_record_id,
        "practice_code": practice_code,
    })
}

// --- the material snapshot -------------------------------------------------

struct Snapshot {
    name: String,
    material_code: String,
    n: Option<f64>,
    p2o5: Option<f64>,
    k2o: Option<f64>,
}

/// Freeze what section 6 prints about the material at write time.
///
/// A blank stays blank: a material whose label states no potassium gets `None`
/// in that cell, never `0.0`. Zero is a claim ("this contains no K₂O") and a
/// spreadsheet would go on to add it up.
fn material_snapshot(tx: &Transaction, material_id: &str) -> Result<Snapshot> {
    let (name, material_code): (String, String) = tx
        .query_row(
            "SELECT name, material_code FROM fertiliser_material
             WHERE id = ?1 AND deleted_at IS NULL",
            [material_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(no_rows_to_not_found)?;

    let mut stmt = tx.prepare(
        "SELECT nutrient_code, percentage FROM fertiliser_material_nutrient
         WHERE fertiliser_material_id = ?1 AND kind_code = 'macro'",
    )?;
    let mut snapshot = Snapshot {
        name,
        material_code,
        n: None,
        p2o5: None,
        k2o: None,
    };
    let rows = stmt
        .query_map([material_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (code, percentage) in rows {
        match code.as_str() {
            RICHNESS_N => snapshot.n = Some(percentage),
            RICHNESS_P2O5 => snapshot.p2o5 = Some(percentage),
            RICHNESS_K2O => snapshot.k2o = Some(percentage),
            _ => {}
        }
    }
    Ok(snapshot)
}

// --- validation ------------------------------------------------------------

/// An interval must not end before it starts. A single-day application leaves
/// the end NULL rather than repeating the start, so a serializer can tell "one
/// day" from "a period that happened to be one day long".
fn validate_interval(start: &str, end: Option<&str>) -> Result<()> {
    match end {
        None => Ok(()),
        Some(end) if end >= start => Ok(()),
        Some(_) => Err(FertilisationError::Invalid("invalid_date_interval")),
    }
}

fn validate_dose(value: f64, unit_code: &str) -> Result<()> {
    // NaN must be rejected explicitly: every comparison against it is false.
    if value.is_nan() || value <= 0.0 {
        return Err(FertilisationError::Invalid("invalid_dose"));
    }
    if !DOSE_UNITS.contains(&unit_code) {
        return Err(FertilisationError::Invalid("invalid_dose_unit"));
    }
    Ok(())
}

/// The model's two "Producción (kg/ha)" columns. Both optional — an estimate is
/// made before the season and a final figure only exists after the harvest — but
/// a stated yield cannot be negative.
fn validate_yields(estimated: Option<f64>, final_yield: Option<f64>) -> Result<()> {
    for value in [estimated, final_yield].into_iter().flatten() {
        if value < 0.0 || value.is_nan() {
            return Err(FertilisationError::Invalid("invalid_yield"));
        }
    }
    Ok(())
}

fn validate_codes(
    tx: &Transaction,
    fertilisation_type: &str,
    application_method: &str,
) -> Result<()> {
    let known_type: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM fertilisation_type WHERE code = ?1)",
        [fertilisation_type],
        |r| r.get(0),
    )?;
    if !known_type {
        return Err(FertilisationError::Invalid("unknown_fertilisation_type"));
    }
    let known_method: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM application_method WHERE code = ?1)",
        [application_method],
        |r| r.get(0),
    )?;
    if !known_method {
        return Err(FertilisationError::Invalid("unknown_application_method"));
    }
    Ok(())
}

/// C.g's machine is optional, but a named one must belong to this holding —
/// the same rule a treated plot obeys, and for the same reason: a record that
/// names another farm's spreader states something that did not happen.
fn validate_machinery(tx: &Transaction, machinery_id: Option<&str>, farm_id: &str) -> Result<()> {
    let Some(machinery_id) = machinery_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(());
    };
    let owner: String = tx
        .query_row(
            "SELECT farm_id FROM machinery WHERE id = ?1 AND deleted_at IS NULL",
            [machinery_id],
            |r| r.get(0),
        )
        .map_err(no_rows_to_not_found)?;
    if owner != farm_id {
        return Err(FertilisationError::Invalid("machinery_not_on_farm"));
    }
    Ok(())
}

/// The linked watering must exist, be live, be on the SAME farm and campaign,
/// and — the part that makes the link mean something — the application must
/// actually be a fertigation.
///
/// The link exists so `Fertilizacion.Fertirrigacion` can be built from the
/// register that holds the water (the decree splits one act across arts. 5.d
/// and 5.e). On any other application method it would assert a fertigation that
/// did not happen, which is why the method decides rather than the farmer.
/// `is_fertigation` is read from the lookup rather than matched on the code, so
/// the rule follows the data.
fn validate_fertigation_link(
    tx: &Transaction,
    irrigation_record_id: Option<&str>,
    application_method_code: &str,
    farm_id: &str,
    season_id: &str,
) -> Result<()> {
    let Some(id) = irrigation_record_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Ok(());
    };
    let is_fertigation: bool = tx
        .query_row(
            "SELECT is_fertigation FROM application_method WHERE code = ?1",
            [application_method_code],
            |r| r.get(0),
        )
        .map_err(no_rows_to_not_found)?;
    if !is_fertigation {
        return Err(FertilisationError::Invalid("link_needs_fertigation"));
    }
    let found: Option<(String, String)> = tx
        .query_row(
            "SELECT farm_id, season_id FROM irrigation_record
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match found {
        Some((owner, campaign)) if owner == farm_id && campaign == season_id => Ok(()),
        Some(_) => Err(FertilisationError::Invalid("irrigation_not_on_farm")),
        None => Err(FertilisationError::NotFound),
    }
}

/// Every fertilised plot must exist and be on this farm. Duplicates fold — the
/// UNIQUE index would reject them anyway, and a form listing a plot twice means
/// one application.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewFertilisationPlot],
) -> Result<Vec<NewFertilisationPlot>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for plot in plots {
        if !seen.insert(plot.plot_id.clone()) {
            continue;
        }
        if let Some(area) = plot.fertilised_area_ha
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
        kept.push(NewFertilisationPlot {
            plot_id: plot.plot_id.clone(),
            crop_id: plot.crop_id.clone(),
            fertilised_area_ha: plot.fertilised_area_ha,
        });
    }
    if kept.is_empty() {
        return Err(FertilisationError::Invalid("no_plots"));
    }
    Ok(kept)
}

/// `BUENAS_PRACTICAS_AMBITOS` codes, stored verbatim and NOT validated against
/// the catalogue: the file is keyed on (code, ámbito) and the same integer
/// means a different practice in each of the three, so this table's own
/// existence is what fixes the ámbito — there is no ámbito-blind membership
/// test that would mean anything.
///
/// Duplicates fold, and the result is sorted numerically so a freshly written
/// record and one read back list its practices identically.
///
/// One pair IS refused. Code "0" is spelled "No realiza buenas prácticas", so a
/// record holding it beside any other code says both that no practice was
/// carried out and which ones were. That is not an unknown code — the objection
/// the picker-narrows-never-the-repository rule exists for, since a catalogue
/// grows between releases and refusing an unlisted code would make a lawful
/// practice unrecordable. This is a contradictory pair of KNOWN codes whose
/// meanings cannot drift, and a register that can hold it exports it.
fn validated_practices(codes: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for code in codes {
        let code = code.trim();
        if code.is_empty() {
            return Err(FertilisationError::Invalid("empty_practice_code"));
        }
        if !seen.insert(code.to_string()) {
            continue;
        }
        kept.push(code.to_string());
    }
    if kept.len() > 1 && kept.iter().any(|code| code == NO_PRACTICES_CODE) {
        return Err(FertilisationError::Invalid("practices_contradict_none"));
    }
    // Provider codes are integers published in a deliberate order; a code that
    // does not parse sorts last rather than derailing the comparison.
    kept.sort_by_key(|code| {
        (
            code.parse::<i64>().is_err(),
            code.parse::<i64>().ok(),
            code.clone(),
        )
    });
    Ok(kept)
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

const INSERT_SQL: &str = "INSERT INTO fertilisation_record (
        id, season_id, farm_id, applied_on, application_end_date,
        fertilisation_type_code, application_method_code, dose_value, dose_unit_code,
        fertiliser_material_id, material_name_snapshot, material_code_snapshot,
        richness_n_snapshot, richness_p2o5_snapshot, richness_k2o_snapshot,
        sludge_application, sustainable_input_management, machinery_id,
        irrigation_record_id, service_company, service_regfer_number,
        delivery_note_ref, yield_estimated_kg_ha, yield_final_kg_ha, notes,
        created_at, updated_at
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
        ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
     )";

/// Hydration for a whole list, in two child statements rather than two per
/// record. The single-record paths keep their point queries.
///
/// The practices query keeps its numeric ordering: the codes are catalogue
/// integers stored as text, so ordering by the column would print 10 before 2.
fn all_with_details(
    conn: &Connection,
    records: Vec<FertilisationRecord>,
) -> Result<Vec<FertilisationRecordDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM fertilisation_plot WHERE fertilisation_record_id IN ({ids})
         ORDER BY fertilisation_record_id, id",
        &ids,
        map_plot,
        |p| p.fertilisation_record_id.clone(),
    )?;
    let mut practices = children_by_parent(
        conn,
        "SELECT fertilisation_record_id, practice_code FROM fertilisation_practice
         WHERE fertilisation_record_id IN ({ids})
         ORDER BY fertilisation_record_id, CAST(practice_code AS INTEGER), practice_code",
        &ids,
        |row| {
            Ok((
                row.get::<_, String>("fertilisation_record_id")?,
                row.get::<_, String>("practice_code")?,
            ))
        },
        |(record_id, _)| record_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| FertilisationRecordDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            practices: practices
                .remove(&record.id)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, code)| code)
                .collect(),
            record,
        })
        .collect())
}

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<FertilisationPlot>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM fertilisation_plot WHERE fertilisation_record_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<FertilisationPlot>> {
    let mut stmt = tx.prepare(
        "SELECT * FROM fertilisation_plot WHERE fertilisation_record_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Sorted the way `validated_practices` sorts, so what comes back from the
/// database matches what the insert returned.
fn practices_of(conn: &Connection, record_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT practice_code FROM fertilisation_practice
         WHERE fertilisation_record_id = ?1
         ORDER BY CAST(practice_code AS INTEGER), practice_code",
    )?;
    let rows = stmt
        .query_map([record_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(rows)
}

fn practice_rows_tx(tx: &Transaction, record_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = tx.prepare(
        "SELECT id, practice_code FROM fertilisation_practice
         WHERE fertilisation_record_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<FertilisationRecord> {
    Ok(FertilisationRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        applied_on: row.get("applied_on")?,
        application_end_date: row.get("application_end_date")?,
        fertilisation_type_code: row.get("fertilisation_type_code")?,
        application_method_code: row.get("application_method_code")?,
        dose_value: row.get("dose_value")?,
        dose_unit_code: row.get("dose_unit_code")?,
        fertiliser_material_id: row.get("fertiliser_material_id")?,
        material_name_snapshot: row.get("material_name_snapshot")?,
        material_code_snapshot: row.get("material_code_snapshot")?,
        richness_n_snapshot: row.get("richness_n_snapshot")?,
        richness_p2o5_snapshot: row.get("richness_p2o5_snapshot")?,
        richness_k2o_snapshot: row.get("richness_k2o_snapshot")?,
        sludge_application: row.get("sludge_application")?,
        sustainable_input_management: row.get("sustainable_input_management")?,
        irrigation_record_id: row.get("irrigation_record_id")?,
        machinery_id: row.get("machinery_id")?,
        service_company: row.get("service_company")?,
        service_regfer_number: row.get("service_regfer_number")?,
        delivery_note_ref: row.get("delivery_note_ref")?,
        yield_estimated_kg_ha: row.get("yield_estimated_kg_ha")?,
        yield_final_kg_ha: row.get("yield_final_kg_ha")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<FertilisationPlot> {
    Ok(FertilisationPlot {
        id: row.get("id")?,
        fertilisation_record_id: row.get("fertilisation_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        fertilised_area_ha: row.get("fertilised_area_ha")?,
    })
}
