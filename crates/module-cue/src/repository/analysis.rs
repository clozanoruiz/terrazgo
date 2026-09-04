// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 4 — the register of analyses.
//!
//! Metadata only. The register records that an analysis was made and where its
//! bulletin can be found; the bulletin itself stays in the farmer's folder,
//! which art. 16.3 obliges keeping for three years. The app has no attachment
//! capability, and giving it one has backup, sync and mobile-storage
//! consequences that belong to their own decision.
//!
//! Fully correctable, for the treated-seed reason: the record holds no snapshot
//! of another row's identity, so there is nothing a later edit elsewhere could
//! rewrite. The per-plot crop snapshots are the frozen *printed* crop,
//! re-resolved when a correction restates the plots.
//!
//! Unlike 3.2–3.5 this register has no "APLICA TRATAMIENTO: SÍ/NO" line — it is
//! model-recommended (art. 16.3's conservation duty) rather than conditional —
//! so no `register_declaration` code backs it and nothing is withdrawn here.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::date::now_utc_iso;
use crate::error::{CueError, Result};
use crate::models::{
    AnalysisPlot, AnalysisRecord, AnalysisRecordDetail, AnalysisRecordType, AnalysisSubstance,
    NewAnalysisPlot, NewAnalysisRecord, SoilParameters, UpdateAnalysisRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use terrazgo_core::sql::children_by_parent;
use uuid::Uuid;

pub fn insert_analysis_record(
    conn: &mut Connection,
    new: NewAnalysisRecord,
    actor: Option<&str>,
) -> Result<AnalysisRecordDetail> {
    validate_soil(&new.soil)?;
    let tx = conn.transaction()?;
    validate_material(&tx, &new.material_kind_code)?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;

    let now = now_utc_iso();
    let record = AnalysisRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        sampled_on: new.sampled_on,
        material_kind_code: new.material_kind_code,
        bulletin_number: blank_to_none(new.bulletin_number),
        lab_name: blank_to_none(new.lab_name),
        lab_address: blank_to_none(new.lab_address),
        lab_tax_id: blank_to_none(new.lab_tax_id),
        substances_detected: blank_to_none(new.substances_detected),
        soil: new.soil,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO analysis_record (
            id, season_id, farm_id, sampled_on, material_kind_code, bulletin_number,
            lab_name, lab_address, lab_tax_id, substances_detected,
            soil_ph, soil_organic_matter_pct, soil_available_p_mg_kg,
            soil_available_k_mg_kg, soil_total_n_pct, soil_conductivity_ds_m,
            soil_sand_pct, soil_silt_pct, soil_clay_pct,
            notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
            ?17, ?18, ?19, ?20, ?21, ?22
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.sampled_on,
            record.material_kind_code,
            record.bulletin_number,
            record.lab_name,
            record.lab_address,
            record.lab_tax_id,
            record.substances_detected,
            record.soil.ph,
            record.soil.organic_matter_pct,
            record.soil.available_p_mg_kg,
            record.soil.available_k_mg_kg,
            record.soil.total_n_pct,
            record.soil.conductivity_ds_m,
            record.soil.sand_pct,
            record.soil.silt_pct,
            record.soil.clay_pct,
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
    let type_rows = reconcile_types(&tx, &record, &new.analysis_type_codes, actor)?;
    let substance_rows = reconcile_substances(&tx, &record, &new.substance_codes, actor)?;

    log_insert(
        &tx,
        "analysis_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(AnalysisRecordDetail {
        record,
        plots: plot_rows,
        types: type_rows,
        substances: substance_rows,
    })
}

/// Full-row correction, plus the sampled plots reconciled from the submitted
/// state: rows that stayed are updated in place (so the audit trail reads as a
/// correction, not a replacement), rows that went are removed, new ones are
/// inserted — each logged on its own.
pub fn update_analysis_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateAnalysisRecord,
    actor: Option<&str>,
) -> Result<AnalysisRecordDetail> {
    validate_soil(&update.soil)?;
    let tx = conn.transaction()?;
    validate_material(&tx, &update.material_kind_code)?;
    let before = tx
        .query_row(
            "SELECT * FROM analysis_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;

    let mut after = before.clone();
    after.sampled_on = update.sampled_on;
    after.material_kind_code = update.material_kind_code;
    after.bulletin_number = blank_to_none(update.bulletin_number);
    after.lab_name = blank_to_none(update.lab_name);
    after.lab_address = blank_to_none(update.lab_address);
    after.lab_tax_id = blank_to_none(update.lab_tax_id);
    after.substances_detected = blank_to_none(update.substances_detected);
    after.soil = update.soil;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE analysis_record SET
            sampled_on = ?2, material_kind_code = ?3, bulletin_number = ?4, lab_name = ?5,
            lab_address = ?6, lab_tax_id = ?7, substances_detected = ?8,
            soil_ph = ?9, soil_organic_matter_pct = ?10, soil_available_p_mg_kg = ?11,
            soil_available_k_mg_kg = ?12, soil_total_n_pct = ?13,
            soil_conductivity_ds_m = ?14, soil_sand_pct = ?15, soil_silt_pct = ?16,
            soil_clay_pct = ?17, notes = ?18, updated_at = ?19
         WHERE id = ?1",
        params![
            id,
            after.sampled_on,
            after.material_kind_code,
            after.bulletin_number,
            after.lab_name,
            after.lab_address,
            after.lab_tax_id,
            after.substances_detected,
            after.soil.ph,
            after.soil.organic_matter_pct,
            after.soil.available_p_mg_kg,
            after.soil.available_k_mg_kg,
            after.soil.total_n_pct,
            after.soil.conductivity_ds_m,
            after.soil.sand_pct,
            after.soil.silt_pct,
            after.soil.clay_pct,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "analysis_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    let type_rows = reconcile_types(&tx, &after, &update.analysis_type_codes, actor)?;
    let substance_rows = reconcile_substances(&tx, &after, &update.substance_codes, actor)?;
    tx.commit()?;
    Ok(AnalysisRecordDetail {
        record: after,
        plots: plot_rows,
        types: type_rows,
        substances: substance_rows,
    })
}

pub fn soft_delete_analysis_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM analysis_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE analysis_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "analysis_record",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_analysis_record(conn: &Connection, id: &str) -> Result<AnalysisRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM analysis_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let detail = detail_of(conn, record)?;
    Ok(detail)
}

/// Oldest first, the order a record book reads in.
/// Every record of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
pub fn list_analysis_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<AnalysisRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM analysis_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY sampled_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_detail_of(conn, records)
}

pub fn list_analysis_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<AnalysisRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM analysis_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY sampled_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_detail_of(conn, records)
}

/// [`detail_of`] for a whole list, in three child statements rather than three
/// per record. The single-record path keeps its point queries.
fn all_detail_of(
    conn: &Connection,
    records: Vec<AnalysisRecord>,
) -> Result<Vec<AnalysisRecordDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM analysis_plot WHERE analysis_record_id IN ({ids})
         ORDER BY analysis_record_id, id",
        &ids,
        map_plot,
        |p| p.analysis_record_id.clone(),
    )?;
    let mut types = children_by_parent(
        conn,
        "SELECT * FROM analysis_record_type WHERE analysis_record_id IN ({ids})
         ORDER BY analysis_record_id, id",
        &ids,
        map_type,
        |t| t.analysis_record_id.clone(),
    )?;
    let mut substances = children_by_parent(
        conn,
        "SELECT * FROM analysis_substance WHERE analysis_record_id IN ({ids})
         ORDER BY analysis_record_id, id",
        &ids,
        map_substance,
        |s| s.analysis_record_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| AnalysisRecordDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            types: types.remove(&record.id).unwrap_or_default(),
            substances: substances.remove(&record.id).unwrap_or_default(),
            record,
        })
        .collect())
}

/// A record plus its three child lists — what the book, the form and the
/// documents all read.
fn detail_of(conn: &Connection, record: AnalysisRecord) -> Result<AnalysisRecordDetail> {
    let plots = plots_of(conn, &record.id)?;
    let types = types_of(conn, &record.id)?;
    let substances = substances_of(conn, &record.id)?;
    Ok(AnalysisRecordDetail {
        record,
        plots,
        types,
        substances,
    })
}

/// Whether any analysis hangs off this season — one arm of the guard the shell
/// chains before deleting a season. Soft-deleted records count, like
/// `season_has_treatments`: their audit history is only reachable through the
/// season they belong to.
pub(super) fn season_has_analyses(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM analysis_record WHERE season_id = ?1)",
        [season_id],
        |row| row.get(0),
    )?;
    Ok(held)
}

// --- reconcile -------------------------------------------------------------

/// Reconcile the sampled plots against the submitted state — the 3-way match
/// the extension tables use, one plot at a time.
fn reconcile_plots(
    tx: &Transaction,
    record: &AnalysisRecord,
    desired: Vec<NewAnalysisPlot>,
    actor: Option<&str>,
) -> Result<Vec<AnalysisPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. These
    // are pure children — they live and die with the analysis, and soft-deleting
    // them would leave the register pointing at parcels nobody sampled.
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute("DELETE FROM analysis_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "analysis_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&AnalysisPlot>,
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
                        "UPDATE analysis_plot
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
                        "analysis_plot",
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

/// Reconcile the kinds of analysis against the submitted state. These rows
/// carry nothing but their code, so there is no "corrected in place" case the
/// plots have: a different code is a different row, logged as its own insert and
/// its own delete.
fn reconcile_types(
    tx: &Transaction,
    record: &AnalysisRecord,
    desired: &[String],
    actor: Option<&str>,
) -> Result<Vec<AnalysisRecordType>> {
    let wanted = deduped(desired);
    for code in &wanted {
        validate_type(tx, code)?;
    }
    let current = types_of_tx(tx, &record.id)?;
    for existing in &current {
        if !wanted.contains(&existing.analysis_type_code) {
            tx.execute(
                "DELETE FROM analysis_record_type WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "analysis_record_type",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&AnalysisRecordType>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for code in wanted {
        match current.iter().find(|c| c.analysis_type_code == code) {
            Some(existing) => rows.push(existing.clone()),
            None => {
                let row = AnalysisRecordType {
                    id: Uuid::now_v7().to_string(),
                    analysis_record_id: record.id.clone(),
                    analysis_type_code: code,
                };
                tx.execute(
                    "INSERT INTO analysis_record_type (id, analysis_record_id, analysis_type_code)
                     VALUES (?1, ?2, ?3)",
                    params![row.id, row.analysis_record_id, row.analysis_type_code],
                )?;
                log_insert(
                    tx,
                    "analysis_record_type",
                    &row.id,
                    Some(&record.season_id),
                    actor,
                    &row,
                )?;
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

/// Reconcile the substances found, same shape as the types above — except that
/// the code is NOT validated against anything. SUST_ACTIVAS ships with the app
/// and a laboratory's bulletin does not wait for our next release, so an
/// unresolvable code is stored and simply prints itself (the upsert-never-delete
/// reasoning behind `treatment_problem.problem_code`).
fn reconcile_substances(
    tx: &Transaction,
    record: &AnalysisRecord,
    desired: &[String],
    actor: Option<&str>,
) -> Result<Vec<AnalysisSubstance>> {
    let wanted = deduped(desired);
    let current = substances_of_tx(tx, &record.id)?;
    for existing in &current {
        if !wanted.contains(&existing.substance_code) {
            tx.execute(
                "DELETE FROM analysis_substance WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "analysis_substance",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&AnalysisSubstance>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for code in wanted {
        match current.iter().find(|c| c.substance_code == code) {
            Some(existing) => rows.push(existing.clone()),
            None => {
                let row = AnalysisSubstance {
                    id: Uuid::now_v7().to_string(),
                    analysis_record_id: record.id.clone(),
                    substance_code: code,
                };
                tx.execute(
                    "INSERT INTO analysis_substance (id, analysis_record_id, substance_code)
                     VALUES (?1, ?2, ?3)",
                    params![row.id, row.analysis_record_id, row.substance_code],
                )?;
                log_insert(
                    tx,
                    "analysis_substance",
                    &row.id,
                    Some(&record.season_id),
                    actor,
                    &row,
                )?;
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

/// Trimmed, blank-free and duplicate-free, in submitted order: a form that
/// lists a code twice means one finding, not an error the UNIQUE index should
/// report.
fn deduped(codes: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    codes
        .iter()
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty() && seen.insert(code.clone()))
        .collect()
}

fn insert_plot_row(
    tx: &Transaction,
    analysis_record_id: &str,
    season_id: &str,
    plot: NewAnalysisPlot,
    actor: Option<&str>,
) -> Result<AnalysisPlot> {
    let (crop_name, variety) = crop_snapshot(tx, plot.crop_id.as_deref())?;
    let row = AnalysisPlot {
        id: Uuid::now_v7().to_string(),
        analysis_record_id: analysis_record_id.to_string(),
        plot_id: plot.plot_id,
        crop_id: plot.crop_id,
        crop_name_snapshot: crop_name,
        variety_snapshot: variety,
    };
    tx.execute(
        "INSERT INTO analysis_plot (
            id, analysis_record_id, plot_id, crop_id, crop_name_snapshot, variety_snapshot
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.analysis_record_id,
            row.plot_id,
            row.crop_id,
            row.crop_name_snapshot,
            row.variety_snapshot
        ],
    )?;
    log_insert(tx, "analysis_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

/// Freeze the crop as it reads today, so a later rename cannot rewrite what the
/// printed book said was sampled.
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
            .map_err(no_rows_to_not_found),
        None => Ok((None, None)),
    }
}

// --- validation ------------------------------------------------------------

/// The material analysed is the one field the SIEX twin makes mandatory besides
/// the date, and the model prints it as a closed three-value list.
fn validate_material(tx: &Transaction, code: &str) -> Result<()> {
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM analysis_material WHERE code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(CueError::Invalid("unknown_analysis_material"));
    }
    Ok(())
}

/// The kinds of analysis, unlike the substances, ARE a closed list we own — a
/// code outside it could not be exported and would print as nothing.
fn validate_type(tx: &Transaction, code: &str) -> Result<()> {
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM analysis_type WHERE code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(CueError::Invalid("unknown_analysis_type"));
    }
    Ok(())
}

/// Every sampled plot must exist and be on this farm. Duplicates are folded —
/// the UNIQUE index would reject them anyway, and a form that lists a plot
/// twice means one sample, not an error.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewAnalysisPlot],
) -> Result<Vec<NewAnalysisPlot>> {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
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
            .map_err(no_rows_to_not_found)?;
        if plot_farm != farm_id {
            return Err(CueError::PlotNotOnFarm {
                plot_id: plot.plot_id.clone(),
                farm_id: farm_id.to_string(),
            });
        }
        kept.push(NewAnalysisPlot {
            plot_id: plot.plot_id.clone(),
            crop_id: plot.crop_id.clone(),
        });
    }
    if kept.is_empty() {
        return Err(CueError::Invalid("no_plots"));
    }
    Ok(kept)
}

/// Anexo III A.3's figures. Every one optional — the minimums bind only a year
/// after MAPA publishes its guides, and a bulletin reports what was asked for —
/// but a STATED figure has to be possible.
///
/// pH is bounded by the scale itself; percentages by being percentages; the
/// rest merely non-negative, since a concentration cannot be. And the three
/// texture fractions must sum to 100 when all three are given: they are
/// fractions of one whole, so 30/30/30 is a bulletin transcribed wrong, not a
/// soil. A tolerance of one point absorbs a lab's rounding.
fn validate_soil(soil: &SoilParameters) -> Result<()> {
    let sane = |value: Option<f64>, max: f64| !matches!(value, Some(v) if !v.is_finite() || v < 0.0 || v > max);
    if !sane(soil.ph, 14.0) {
        return Err(CueError::Invalid("invalid_soil_ph"));
    }
    for percentage in [
        soil.organic_matter_pct,
        soil.total_n_pct,
        soil.sand_pct,
        soil.silt_pct,
        soil.clay_pct,
    ] {
        if !sane(percentage, 100.0) {
            return Err(CueError::Invalid("invalid_soil_percentage"));
        }
    }
    for concentration in [
        soil.available_p_mg_kg,
        soil.available_k_mg_kg,
        soil.conductivity_ds_m,
    ] {
        if !sane(concentration, f64::MAX) {
            return Err(CueError::Invalid("invalid_soil_value"));
        }
    }
    if let (Some(sand), Some(silt), Some(clay)) = (soil.sand_pct, soil.silt_pct, soil.clay_pct)
        && (sand + silt + clay - 100.0).abs() > 1.0
    {
        return Err(CueError::Invalid("invalid_soil_texture"));
    }
    Ok(())
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

fn plots_of(conn: &Connection, analysis_record_id: &str) -> Result<Vec<AnalysisPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM analysis_plot WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, analysis_record_id: &str) -> Result<Vec<AnalysisPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM analysis_plot WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn types_of(conn: &Connection, analysis_record_id: &str) -> Result<Vec<AnalysisRecordType>> {
    let mut stmt = conn
        .prepare("SELECT * FROM analysis_record_type WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_type)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn types_of_tx(tx: &Transaction, analysis_record_id: &str) -> Result<Vec<AnalysisRecordType>> {
    let mut stmt =
        tx.prepare("SELECT * FROM analysis_record_type WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_type)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn substances_of(conn: &Connection, analysis_record_id: &str) -> Result<Vec<AnalysisSubstance>> {
    let mut stmt =
        conn.prepare("SELECT * FROM analysis_substance WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_substance)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn substances_of_tx(tx: &Transaction, analysis_record_id: &str) -> Result<Vec<AnalysisSubstance>> {
    let mut stmt =
        tx.prepare("SELECT * FROM analysis_substance WHERE analysis_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([analysis_record_id], map_substance)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_type(row: &Row) -> rusqlite::Result<AnalysisRecordType> {
    Ok(AnalysisRecordType {
        id: row.get("id")?,
        analysis_record_id: row.get("analysis_record_id")?,
        analysis_type_code: row.get("analysis_type_code")?,
    })
}

fn map_substance(row: &Row) -> rusqlite::Result<AnalysisSubstance> {
    Ok(AnalysisSubstance {
        id: row.get("id")?,
        analysis_record_id: row.get("analysis_record_id")?,
        substance_code: row.get("substance_code")?,
    })
}

fn map_record(row: &Row) -> rusqlite::Result<AnalysisRecord> {
    Ok(AnalysisRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        sampled_on: row.get("sampled_on")?,
        material_kind_code: row.get("material_kind_code")?,
        bulletin_number: row.get("bulletin_number")?,
        lab_name: row.get("lab_name")?,
        lab_address: row.get("lab_address")?,
        lab_tax_id: row.get("lab_tax_id")?,
        substances_detected: row.get("substances_detected")?,
        soil: SoilParameters {
            ph: row.get("soil_ph")?,
            organic_matter_pct: row.get("soil_organic_matter_pct")?,
            available_p_mg_kg: row.get("soil_available_p_mg_kg")?,
            available_k_mg_kg: row.get("soil_available_k_mg_kg")?,
            total_n_pct: row.get("soil_total_n_pct")?,
            conductivity_ds_m: row.get("soil_conductivity_ds_m")?,
            sand_pct: row.get("soil_sand_pct")?,
            silt_pct: row.get("soil_silt_pct")?,
            clay_pct: row.get("soil_clay_pct")?,
        },
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row) -> rusqlite::Result<AnalysisPlot> {
    Ok(AnalysisPlot {
        id: row.get("id")?,
        analysis_record_id: row.get("analysis_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        crop_name_snapshot: row.get("crop_name_snapshot")?,
        variety_snapshot: row.get("variety_snapshot")?,
    })
}
