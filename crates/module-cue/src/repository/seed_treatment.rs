// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 3.2 — sowings made with seed the supplier already treated.
//!
//! Two departures from `treatment.rs`, both deliberate:
//!
//! * the product is **free capture**. Treated seed arrives in a sack whose
//!   label names a product the farmer never bought as such, so requiring a
//!   registry row first would block a lawful record; an optional `product_id`
//!   links the two when they do coincide.
//! * the record is **fully correctable**. It holds no snapshot of another row,
//!   so nothing a later edit elsewhere could rewrite — which is exactly the
//!   condition the snapshot columns exist to handle, absent here.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::date::now_utc_iso;
use crate::error::{CueError, Result};
use crate::models::{
    NewSeedTreatment, NewSeedTreatmentPlot, SeedTreatment, SeedTreatmentDetail, SeedTreatmentPlot,
    UpdateSeedTreatment,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use uuid::Uuid;

/// The `register_kind` code this table backs; `register_declaration`'s guard
/// has to look here rather than at `non_field_treatment`.
pub(super) const REGISTER: &str = "seed_treatment";

pub fn insert_seed_treatment(
    conn: &mut Connection,
    new: NewSeedTreatment,
    actor: Option<&str>,
) -> Result<SeedTreatmentDetail> {
    let species_name = validated_species(&new.species_name)?;
    let product_name = validated_product(&new.product_name)?;
    validate_seed_quantity(new.seed_quantity_kg)?;

    let tx = conn.transaction()?;
    validate_treatment_kind(&tx, new.treatment_kind_code.as_deref())?;
    let plots = validated_plots(&tx, &new.farm_id, &new.plots)?;

    let now = now_utc_iso();
    let record = SeedTreatment {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        sown_on: new.sown_on,
        species_name,
        variety: blank_to_none(new.variety),
        crop_code: new.crop_code,
        seed_quantity_kg: new.seed_quantity_kg,
        seed_lot: blank_to_none(new.seed_lot),
        treatment_kind_code: new.treatment_kind_code,
        product_name,
        product_registration_number: blank_to_none(new.product_registration_number),
        product_active_substance: blank_to_none(new.product_active_substance),
        product_id: new.product_id,
        efficacy_code: new.efficacy_code,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO seed_treatment (
            id, season_id, farm_id, sown_on, species_name, variety, crop_code,
            seed_quantity_kg, seed_lot, treatment_kind_code, product_name,
            product_registration_number, product_active_substance, product_id,
            efficacy_code, notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.sown_on,
            record.species_name,
            record.variety,
            record.crop_code,
            record.seed_quantity_kg,
            record.seed_lot,
            record.treatment_kind_code,
            record.product_name,
            record.product_registration_number,
            record.product_active_substance,
            record.product_id,
            record.efficacy_code,
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
        "seed_treatment",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;

    // A sowing contradicts a standing "no treated seed this campaign".
    super::non_field_treatment::withdraw_declaration_tx(
        &tx,
        &record.farm_id,
        &record.season_id,
        REGISTER,
        actor,
    )?;

    tx.commit()?;
    Ok(SeedTreatmentDetail {
        record,
        plots: plot_rows,
    })
}

/// Full-row correction, plus the sown plots reconciled from the submitted
/// state: rows that stayed are updated in place (so the audit trail reads as a
/// correction, not a replacement), rows that went are removed, new ones are
/// inserted — each logged on its own.
pub fn update_seed_treatment(
    conn: &mut Connection,
    id: &str,
    update: UpdateSeedTreatment,
    actor: Option<&str>,
) -> Result<SeedTreatmentDetail> {
    let species_name = validated_species(&update.species_name)?;
    let product_name = validated_product(&update.product_name)?;
    validate_seed_quantity(update.seed_quantity_kg)?;

    let tx = conn.transaction()?;
    validate_treatment_kind(&tx, update.treatment_kind_code.as_deref())?;
    let before = tx
        .query_row(
            "SELECT * FROM seed_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let plots = validated_plots(&tx, &before.farm_id, &update.plots)?;

    let mut after = before.clone();
    after.sown_on = update.sown_on;
    after.species_name = species_name;
    after.variety = blank_to_none(update.variety);
    after.crop_code = update.crop_code;
    after.seed_quantity_kg = update.seed_quantity_kg;
    after.seed_lot = blank_to_none(update.seed_lot);
    after.treatment_kind_code = update.treatment_kind_code;
    after.product_name = product_name;
    after.product_registration_number = blank_to_none(update.product_registration_number);
    after.product_active_substance = blank_to_none(update.product_active_substance);
    after.product_id = update.product_id;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE seed_treatment SET
            sown_on = ?2, species_name = ?3, variety = ?4, crop_code = ?5,
            seed_quantity_kg = ?6, seed_lot = ?7, treatment_kind_code = ?8,
            product_name = ?9, product_registration_number = ?10,
            product_active_substance = ?11, product_id = ?12, notes = ?13,
            updated_at = ?14
         WHERE id = ?1",
        params![
            id,
            after.sown_on,
            after.species_name,
            after.variety,
            after.crop_code,
            after.seed_quantity_kg,
            after.seed_lot,
            after.treatment_kind_code,
            after.product_name,
            after.product_registration_number,
            after.product_active_substance,
            after.product_id,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "seed_treatment",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, plots, actor)?;
    tx.commit()?;
    Ok(SeedTreatmentDetail {
        record: after,
        plots: plot_rows,
    })
}

/// Reconcile the sown plots against the submitted state — the 3-way match the
/// extension tables use, one plot at a time.
fn reconcile_plots(
    tx: &Transaction,
    record: &SeedTreatment,
    desired: Vec<NewSeedTreatmentPlot>,
    actor: Option<&str>,
) -> Result<Vec<SeedTreatmentPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. These
    // are pure children — they live and die with the sowing, and soft-deleting
    // them would leave the register printing surfaces nobody sowed.
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute(
                "DELETE FROM seed_treatment_plot WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "seed_treatment_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&SeedTreatmentPlot>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current.iter().find(|c| c.plot_id == want.plot_id) {
            // Still there: corrected in place, keeping its identity.
            Some(existing) => {
                if existing.surface_sown_ha != want.surface_sown_ha {
                    let mut after = existing.clone();
                    after.surface_sown_ha = want.surface_sown_ha;
                    tx.execute(
                        "UPDATE seed_treatment_plot SET surface_sown_ha = ?2 WHERE id = ?1",
                        params![existing.id, after.surface_sown_ha],
                    )?;
                    log_update(
                        tx,
                        "seed_treatment_plot",
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
    seed_treatment_id: &str,
    season_id: &str,
    plot: NewSeedTreatmentPlot,
    actor: Option<&str>,
) -> Result<SeedTreatmentPlot> {
    let row = SeedTreatmentPlot {
        id: Uuid::now_v7().to_string(),
        seed_treatment_id: seed_treatment_id.to_string(),
        plot_id: plot.plot_id,
        surface_sown_ha: plot.surface_sown_ha,
    };
    tx.execute(
        "INSERT INTO seed_treatment_plot (id, seed_treatment_id, plot_id, surface_sown_ha)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            row.id,
            row.seed_treatment_id,
            row.plot_id,
            row.surface_sown_ha
        ],
    )?;
    log_insert(
        tx,
        "seed_treatment_plot",
        &row.id,
        Some(season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

/// The one edit an otherwise complete record still needs after the fact:
/// whether the treated seed worked is only visible once the crop is up.
pub fn set_seed_treatment_efficacy(
    conn: &mut Connection,
    id: &str,
    efficacy_code: Option<String>,
    actor: Option<&str>,
) -> Result<SeedTreatment> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM seed_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let mut after = before.clone();
    after.efficacy_code = efficacy_code;
    after.updated_at = now_utc_iso();
    tx.execute(
        "UPDATE seed_treatment SET efficacy_code = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, after.efficacy_code, after.updated_at],
    )?;
    log_update(
        &tx,
        "seed_treatment",
        id,
        Some(&before.season_id),
        actor,
        &before,
        &after,
    )?;
    tx.commit()?;
    Ok(after)
}

pub fn soft_delete_seed_treatment(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM seed_treatment WHERE id = ?1 AND deleted_at IS NULL",
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
        "UPDATE seed_treatment SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "seed_treatment",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_seed_treatment(conn: &Connection, id: &str) -> Result<SeedTreatmentDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM seed_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    Ok(SeedTreatmentDetail { record, plots })
}

/// Oldest first, the order a record book reads in.
pub fn list_seed_treatments(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<SeedTreatmentDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM seed_treatment
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY sown_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|record| {
            let plots = plots_of(conn, &record.id)?;
            Ok(SeedTreatmentDetail { record, plots })
        })
        .collect()
}

/// Whether any sowing hangs off this season — one arm of the guard the shell
/// chains before deleting a season. Soft-deleted records count, like
/// `season_has_treatments`: their audit history is only reachable through the
/// season they belong to.
pub(super) fn season_has_sowings(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM seed_treatment WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

/// Whether this register holds live sowings — what the declaration guard asks.
pub(super) fn register_has_rows(conn: &Connection, farm_id: &str, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM seed_treatment
             WHERE farm_id = ?1 AND season_id = ?2 AND deleted_at IS NULL
         )",
        params![farm_id, season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- validation ------------------------------------------------------------

fn validated_species(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CueError::Invalid("empty_name"));
    }
    Ok(trimmed.to_string())
}

fn validated_product(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CueError::Invalid("empty_product_name"));
    }
    Ok(trimmed.to_string())
}

/// Where the seed was treated. Optional — the printed model has no such column
/// — but a stated kind must be one the export can speak.
fn validate_treatment_kind(tx: &Transaction, code: Option<&str>) -> Result<()> {
    let Some(code) = code else { return Ok(()) };
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM seed_treatment_kind WHERE code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(CueError::Invalid("unknown_seed_treatment_kind"));
    }
    Ok(())
}

fn validate_seed_quantity(value: Option<f64>) -> Result<()> {
    match value {
        None => Ok(()),
        Some(kg) if kg > 0.0 => Ok(()),
        Some(_) => Err(CueError::Invalid("invalid_seed_quantity")),
    }
}

/// Every sown plot must exist, be on this farm, and carry a real surface.
/// Duplicates are folded — the UNIQUE index would reject them anyway, and a
/// form that lists a plot twice means one sowing, not an error.
fn validated_plots(
    tx: &Transaction,
    farm_id: &str,
    plots: &[NewSeedTreatmentPlot],
) -> Result<Vec<NewSeedTreatmentPlot>> {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    for plot in plots {
        if !seen.insert(plot.plot_id.clone()) {
            continue;
        }
        if plot.surface_sown_ha <= 0.0 || plot.surface_sown_ha.is_nan() {
            return Err(CueError::Invalid("nonpositive_area"));
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
        kept.push(NewSeedTreatmentPlot {
            plot_id: plot.plot_id.clone(),
            surface_sown_ha: plot.surface_sown_ha,
        });
    }
    if kept.is_empty() {
        return Err(CueError::Invalid("no_plots"));
    }
    Ok(kept)
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

fn plots_of(conn: &Connection, seed_treatment_id: &str) -> Result<Vec<SeedTreatmentPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM seed_treatment_plot WHERE seed_treatment_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([seed_treatment_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, seed_treatment_id: &str) -> Result<Vec<SeedTreatmentPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM seed_treatment_plot WHERE seed_treatment_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([seed_treatment_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row) -> rusqlite::Result<SeedTreatment> {
    Ok(SeedTreatment {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        sown_on: row.get("sown_on")?,
        species_name: row.get("species_name")?,
        variety: row.get("variety")?,
        crop_code: row.get("crop_code")?,
        seed_quantity_kg: row.get("seed_quantity_kg")?,
        seed_lot: row.get("seed_lot")?,
        treatment_kind_code: row.get("treatment_kind_code")?,
        product_name: row.get("product_name")?,
        product_registration_number: row.get("product_registration_number")?,
        product_active_substance: row.get("product_active_substance")?,
        product_id: row.get("product_id")?,
        efficacy_code: row.get("efficacy_code")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row) -> rusqlite::Result<SeedTreatmentPlot> {
    Ok(SeedTreatmentPlot {
        id: row.get("id")?,
        seed_treatment_id: row.get("seed_treatment_id")?,
        plot_id: row.get("plot_id")?,
        surface_sown_ha: row.get("surface_sown_ha")?,
    })
}
