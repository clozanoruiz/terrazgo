// SPDX-License-Identifier: AGPL-3.0-or-later

//! Treatment record CRUD — the central regulatory entity.

use super::audit::{log_insert, write_change};
use super::no_rows_to_not_found;
use crate::date::{add_days, now_utc_iso};
use crate::error::{CueError, Result};
use crate::models::{
    NewTreatmentPlot, NewTreatmentRecord, TreatmentPlot, TreatmentRecord, TreatmentRecordWithPlots,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use uuid::Uuid;

/// Insert a treatment record together with its treated plots, in one transaction.
/// Resolves and freezes the legal snapshots, computes the PHI end date, and logs
/// every inserted row to `record_change`.
pub fn insert_treatment_record(
    conn: &mut Connection,
    new: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
) -> Result<TreatmentRecord> {
    let tx = conn.transaction()?;

    // --- derive and validate the country from the farm ---------------------
    // The record belongs to one farm; its country is the source of truth (NOT NULL in
    // SQL, so it always exists). An explicit country_code is accepted only if it
    // matches (no silent override).
    let country_code: String = tx
        .query_row(
            "SELECT country_code FROM farm WHERE id = ?1",
            [&new.farm_id],
            |r| r.get(0),
        )
        .map_err(no_rows_to_not_found)?;
    if let Some(provided) = &new.country_code
        && provided != &country_code
    {
        return Err(CueError::CountryMismatch {
            provided: provided.clone(),
            farm: country_code,
        });
    }

    // Every treated plot must belong to the record's farm.
    for p in &plots {
        let plot_farm: String = tx
            .query_row(
                "SELECT farm_id FROM plot WHERE id = ?1",
                [&p.plot_id],
                |r| r.get(0),
            )
            .map_err(no_rows_to_not_found)?;
        if plot_farm != new.farm_id {
            return Err(CueError::PlotNotOnFarm {
                plot_id: p.plot_id.clone(),
                farm_id: new.farm_id.clone(),
            });
        }
    }

    // --- resolve legal snapshots from the referenced rows ------------------
    let (product_name, default_phi): (String, Option<i64>) = tx
        .query_row(
            "SELECT commercial_name, default_phi_days FROM product WHERE id = ?1",
            [&new.product_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(no_rows_to_not_found)?;

    let phi_days = new
        .phi_days_used
        .or(default_phi)
        .ok_or(CueError::MissingPhiDays)?;

    // Pick the authorisation number for the record's country (latest by validity).
    let authorisation_number: String = tx
        .query_row(
            "SELECT authorisation_number FROM product_authorisation
             WHERE product_id = ?1 AND country_code = ?2
             ORDER BY valid_from DESC LIMIT 1",
            params![new.product_id, country_code],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| CueError::AuthorisationMissing {
            product_id: new.product_id.clone(),
            country: country_code.clone(),
        })?;

    let active_substances_snapshot = active_substances_snapshot(&tx, &new.product_id)?;

    let (operator_name, operator_licence): (String, Option<String>) = tx
        .query_row(
            "SELECT full_name, licence_number FROM operator WHERE id = ?1",
            [&new.operator_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(no_rows_to_not_found)?;

    let (machinery_roma, machinery_reganip): (Option<String>, Option<String>) =
        match &new.machinery_id {
            Some(mid) => tx
                .query_row(
                    "SELECT roma_number, reganip_number
                     FROM machinery_es_extension WHERE machinery_id = ?1",
                    [mid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
                .unwrap_or((None, None)),
            None => (None, None),
        };

    // --- build and insert the record --------------------------------------
    let now = now_utc_iso();
    let record = TreatmentRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id.clone(),
        farm_id: new.farm_id,
        application_date: new.application_date.clone(),
        product_id: new.product_id,
        country_code,
        dose_value: new.dose_value,
        dose_unit_code: new.dose_unit_code,
        reason_category_code: new.reason_category_code,
        target_organism: new.target_organism,
        operator_id: new.operator_id,
        machinery_id: new.machinery_id,
        phi_days_used: phi_days,
        phi_end_date: add_days(&new.application_date, phi_days)?,
        product_name_snapshot: product_name,
        authorisation_number_snapshot: Some(authorisation_number),
        active_substances_snapshot,
        operator_name_snapshot: operator_name,
        operator_licence_snapshot: operator_licence,
        machinery_roma_snapshot: machinery_roma,
        machinery_reganip_snapshot: machinery_reganip,
        notes: new.notes,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };

    tx.execute(
        "INSERT INTO treatment_record (
            id, season_id, farm_id, application_date, product_id, country_code, dose_value, dose_unit_code,
            reason_category_code, target_organism, operator_id, machinery_id, phi_days_used, phi_end_date,
            product_name_snapshot, authorisation_number_snapshot, active_substances_snapshot,
            operator_name_snapshot, operator_licence_snapshot, machinery_roma_snapshot,
            machinery_reganip_snapshot, notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
         )",
        params![
            record.id, record.season_id, record.farm_id, record.application_date, record.product_id, record.country_code,
            record.dose_value, record.dose_unit_code, record.reason_category_code, record.target_organism,
            record.operator_id, record.machinery_id, record.phi_days_used, record.phi_end_date,
            record.product_name_snapshot, record.authorisation_number_snapshot, record.active_substances_snapshot,
            record.operator_name_snapshot, record.operator_licence_snapshot, record.machinery_roma_snapshot,
            record.machinery_reganip_snapshot, record.notes, record.created_at, record.updated_at
        ],
    )?;

    // --- the treated plots (multi-plot in one entry) ----------------------
    for p in plots {
        let (crop_name, variety): (Option<String>, Option<String>) = match &p.crop_id {
            Some(cid) => tx
                .query_row(
                    "SELECT species_name, variety FROM crop WHERE id = ?1",
                    [cid],
                    |r| Ok((Some(r.get::<_, String>(0)?), r.get::<_, Option<String>>(1)?)),
                )
                .map_err(no_rows_to_not_found)?,
            None => (None, None),
        };
        let tp = TreatmentPlot {
            id: Uuid::now_v7().to_string(),
            treatment_record_id: record.id.clone(),
            plot_id: p.plot_id,
            crop_id: p.crop_id,
            surface_treated_ha: p.surface_treated_ha,
            crop_name_snapshot: crop_name,
            variety_snapshot: variety,
        };
        tx.execute(
            "INSERT INTO treatment_plot
               (id, treatment_record_id, plot_id, crop_id, surface_treated_ha, crop_name_snapshot, variety_snapshot)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                tp.id, tp.treatment_record_id, tp.plot_id, tp.crop_id,
                tp.surface_treated_ha, tp.crop_name_snapshot, tp.variety_snapshot
            ],
        )?;
        log_insert(&tx, "treatment_plot", &tp.id, Some(&record.season_id), &tp)?;
    }

    log_insert(
        &tx,
        "treatment_record",
        &record.id,
        Some(&record.season_id),
        &record,
    )?;
    tx.commit()?;
    Ok(record)
}

/// Fetch a treatment record with all of its treated plots.
pub fn get_treatment_record(conn: &Connection, id: &str) -> Result<TreatmentRecordWithPlots> {
    let record = conn
        .query_row(
            "SELECT * FROM treatment_record WHERE id = ?1",
            [id],
            map_treatment_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let plots = plots_of(conn, id)?;
    Ok(TreatmentRecordWithPlots { record, plots })
}

/// Active treatment records of one farm in one season, newest application
/// first, each with its treated plots — the record-book list view.
pub fn list_treatment_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<TreatmentRecordWithPlots>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM treatment_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY application_date DESC, id DESC",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_treatment_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|record| {
            let plots = plots_of(conn, &record.id)?;
            Ok(TreatmentRecordWithPlots { record, plots })
        })
        .collect()
}

fn plots_of(conn: &Connection, treatment_record_id: &str) -> Result<Vec<TreatmentPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM treatment_plot WHERE treatment_record_id = ?1 ORDER BY id")?;
    let plots = stmt
        .query_map([treatment_record_id], map_treatment_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(plots)
}

/// Soft-delete a regulatory record (official records are never hard-deleted).
/// Both the before- and after-images in the audit log are complete rows.
pub fn soft_delete_treatment_record(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM treatment_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_treatment_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE treatment_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "treatment_record",
        id,
        Some(&before.season_id),
        "delete",
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn active_substances_snapshot(tx: &Transaction, product_id: &str) -> Result<Option<String>> {
    let mut stmt = tx.prepare(
        "SELECT a.name, pas.concentration_value, pas.concentration_unit_code
         FROM product_active_substance pas
         JOIN active_substance a ON a.id = pas.active_substance_id
         WHERE pas.product_id = ?1
         ORDER BY a.name",
    )?;
    let rows = stmt.query_map([product_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<f64>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut parts = Vec::new();
    for row in rows {
        let (name, value, unit) = row?;
        match (value, unit) {
            (Some(v), Some(u)) => parts.push(format!("{name} {v} {u}")),
            _ => parts.push(name),
        }
    }
    Ok((!parts.is_empty()).then(|| parts.join("; ")))
}

fn map_treatment_record(row: &Row) -> rusqlite::Result<TreatmentRecord> {
    Ok(TreatmentRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        application_date: row.get("application_date")?,
        product_id: row.get("product_id")?,
        country_code: row.get("country_code")?,
        dose_value: row.get("dose_value")?,
        dose_unit_code: row.get("dose_unit_code")?,
        reason_category_code: row.get("reason_category_code")?,
        target_organism: row.get("target_organism")?,
        operator_id: row.get("operator_id")?,
        machinery_id: row.get("machinery_id")?,
        phi_days_used: row.get("phi_days_used")?,
        phi_end_date: row.get("phi_end_date")?,
        product_name_snapshot: row.get("product_name_snapshot")?,
        authorisation_number_snapshot: row.get("authorisation_number_snapshot")?,
        active_substances_snapshot: row.get("active_substances_snapshot")?,
        operator_name_snapshot: row.get("operator_name_snapshot")?,
        operator_licence_snapshot: row.get("operator_licence_snapshot")?,
        machinery_roma_snapshot: row.get("machinery_roma_snapshot")?,
        machinery_reganip_snapshot: row.get("machinery_reganip_snapshot")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_treatment_plot(row: &Row) -> rusqlite::Result<TreatmentPlot> {
    Ok(TreatmentPlot {
        id: row.get("id")?,
        treatment_record_id: row.get("treatment_record_id")?,
        plot_id: row.get("plot_id")?,
        crop_id: row.get("crop_id")?,
        surface_treated_ha: row.get("surface_treated_ha")?,
        crop_name_snapshot: row.get("crop_name_snapshot")?,
        variety_snapshot: row.get("variety_snapshot")?,
    })
}
