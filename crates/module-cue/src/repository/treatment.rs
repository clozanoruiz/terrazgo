// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Treatment record CRUD — the central regulatory entity.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::alerts::phi_window_is_active;
use crate::date::{add_days, now_utc_iso};
use crate::error::{CueError, Result};
use crate::models::{
    NewTreatmentPlot, NewTreatmentProblem, NewTreatmentRecord, PlotPhiStatus,
    TreatmentJustification, TreatmentPlot, TreatmentProblem, TreatmentRecord,
    TreatmentRecordWithPlots, UpdateTreatmentRecord,
};
use crate::siex;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

/// The chemical half of an actuation, resolved as a unit. It exists so the
/// six columns that describe a product application are carried together
/// through the insert: the table CHECK refuses any partial combination, and
/// building them separately would let a bug produce one.
struct ChemicalBlock {
    dose_value: f64,
    dose_unit_code: String,
    phi_days: i64,
    product_name: String,
    authorisation_number: String,
    active_substances: Option<String>,
}

/// The non-chemical measure and its intensity (the model's "Alternativas no
/// químicas de intervención"). `TIPO_MEDIDA_FITOSANITARIA` is a closed list of
/// fourteen entries the authority publishes in full, so an unresolvable code
/// is a mistake rather than a snapshot lagging a registry — the `MAT_FERTI`
/// side of the two-tier rule, not the `DETALLE_MATERIAL_FERT` side. With no
/// catalogue imported there is nothing to check against and the code stands.
fn validate_measure_fields(
    tx: &Transaction,
    measure_code: Option<&str>,
    intensity_value: Option<f64>,
    intensity_unit_code: Option<&str>,
) -> Result<()> {
    if let Some(code) = measure_code
        && let Some(false) = super::resolve_in_catalogue(tx, "TIPO_MEDIDA_FITOSANITARIA", code)?
    {
        return Err(CueError::Invalid("unknown_measure_code"));
    }
    // "Intensidad de la medida (Nº de trampas, nº de difusores, etc.)": a value
    // and its unit, or neither. A count is not a dose and not an amount of
    // product, which is what the dedicated dimension records.
    match (intensity_value, intensity_unit_code) {
        (None, None) => {}
        (Some(value), Some(unit)) if value > 0.0 => {
            let dimension: Option<String> = tx
                .query_row("SELECT dimension FROM unit WHERE code = ?1", [unit], |r| {
                    r.get(0)
                })
                .optional()?;
            if dimension.as_deref() != Some("intensity") {
                return Err(CueError::Invalid("invalid_intensity"));
            }
        }
        _ => return Err(CueError::Invalid("invalid_intensity")),
    }
    // An intensity without a measure describes nothing.
    if measure_code.is_none() && intensity_value.is_some() {
        return Err(CueError::Invalid("invalid_intensity"));
    }
    Ok(())
}

/// Anexo III Parte I B.i's total: value and unit travel together, the unit must
/// measure an amount rather than a rate, and a total of zero is not a treatment.
fn validate_total_quantity(
    tx: &Transaction,
    value: Option<f64>,
    unit_code: Option<&str>,
) -> Result<()> {
    match (value, unit_code) {
        (None, None) => Ok(()),
        (Some(value), Some(unit)) if value > 0.0 => {
            let dimension: Option<String> = tx
                .query_row("SELECT dimension FROM unit WHERE code = ?1", [unit], |r| {
                    r.get(0)
                })
                .optional()?;
            if dimension.as_deref() != Some("quantity") {
                return Err(CueError::Invalid("invalid_total_quantity"));
            }
            Ok(())
        }
        _ => Err(CueError::Invalid("invalid_total_quantity")),
    }
}

/// The start hour Reglamento (UE) 2023/564's annex asks for where relevant:
/// absent, or a local wall-clock `HH:MM` on a 24-hour clock.
///
/// Checked rather than stored as typed, unlike efficacy or a total quantity —
/// those are observations a farmer may not have yet, whereas an hour is either
/// well formed or unreadable, and "7pm" or "25:00" printed in a legal register
/// states nothing an inspector could use.
fn validate_application_time(time: Option<&str>) -> Result<()> {
    let Some(time) = time else { return Ok(()) };
    let well_formed = || {
        // Exactly "HH:MM": two digits, a colon, two digits, and an hour and a
        // minute that exist. `parse` alone would accept "1:5" and "07:5".
        let (hours, minutes) = time.split_once(':')?;
        // Two ASCII DIGITS each, checked before parsing: `parse` alone would
        // accept "1:5" on length and "+7:30" on content, since Rust's integer
        // parsing allows a leading sign.
        let two_digits = |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit());
        if !two_digits(hours) || !two_digits(minutes) {
            return None;
        }
        let hours: u8 = hours.parse().ok()?;
        let minutes: u8 = minutes.parse().ok()?;
        (hours < 24 && minutes < 60).then_some(())
    };
    well_formed().ok_or(CueError::Invalid("application_time"))
}

/// The treated crop's growth stage: absent, or a code the `EST_FENOLOGICO`
/// catalogue carries.
///
/// Validated, unlike `analysis_substance.substance_code` — that one speaks a
/// laboratory registry which grows between our snapshots, while the BBCH
/// monograph's principal stages are ten and closed, so an unrecognised code
/// here is a bug and not a newer catalogue (the `MAT_FERTI` case).
///
/// A catalogue that was never imported has no opinion, per
/// [`super::resolve_in_catalogue`]: reference data must never be what stands
/// between a farmer and a lawful record.
fn validate_growth_stage(tx: &Transaction, code: Option<&str>) -> Result<()> {
    if let Some(code) = code
        && let Some(false) = super::resolve_in_catalogue(tx, "EST_FENOLOGICO", code)?
    {
        return Err(CueError::Invalid("growth_stage_unknown"));
    }
    Ok(())
}

/// The coded reason for treatment and the IPM justifications: both required
/// (they are known when treating, unlike efficacy), duplicates from the form
/// folded rather than rejected.
pub(super) fn validated_reasons(
    tx: &Transaction,
    country_code: &str,
    mut problems: Vec<NewTreatmentProblem>,
    mut justifications: Vec<String>,
) -> Result<(Vec<NewTreatmentProblem>, Vec<String>)> {
    let mut seen = HashSet::new();
    problems.retain(|p| seen.insert((p.reason_category_code.clone(), p.problem_code.clone())));
    let mut seen = HashSet::new();
    justifications.retain(|j| seen.insert(j.clone()));
    if problems.is_empty() {
        return Err(CueError::Invalid("no_problems"));
    }
    if justifications.is_empty() {
        return Err(CueError::Invalid("no_justifications"));
    }
    for p in &problems {
        validate_problem_code(tx, country_code, &p.reason_category_code, &p.problem_code)?;
    }
    Ok((problems, justifications))
}

/// `product.default_phi_days`, and proof the product exists.
fn product_default_phi(tx: &Transaction, product_id: &str) -> Result<Option<i64>> {
    tx.query_row(
        "SELECT default_phi_days FROM product WHERE id = ?1",
        [product_id],
        |r| r.get(0),
    )
    .map_err(no_rows_to_not_found)
}

/// What a product prints on the record, read from the registry as it stands
/// now. Called at insert, and on correction only when the product CHANGED.
struct ChemicalSnapshots {
    product_name: String,
    authorisation_number: String,
    active_substances: Option<String>,
}

fn resolve_chemical_snapshots(
    tx: &Transaction,
    product_id: &str,
    country_code: &str,
) -> Result<ChemicalSnapshots> {
    let product_name: String = tx
        .query_row(
            "SELECT commercial_name FROM product WHERE id = ?1",
            [product_id],
            |r| r.get(0),
        )
        .map_err(no_rows_to_not_found)?;
    // The authorisation for the record's country (latest by validity).
    let authorisation_number: String = tx
        .query_row(
            "SELECT authorisation_number FROM product_authorisation
             WHERE product_id = ?1 AND country_code = ?2
             ORDER BY valid_from DESC LIMIT 1",
            params![product_id, country_code],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| CueError::AuthorisationMissing {
            product_id: product_id.to_string(),
            country: country_code.to_string(),
        })?;
    Ok(ChemicalSnapshots {
        product_name,
        authorisation_number,
        active_substances: active_substances_snapshot(tx, product_id)?,
    })
}

/// Insert a treatment record together with its treated plots, in one transaction.
/// Resolves and freezes the legal snapshots, computes the PHI end date, and logs
/// every inserted row to `record_change`.
pub fn insert_treatment_record(
    conn: &mut Connection,
    mut new: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
    actor: Option<&str>,
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

    // --- what the actuation actually WAS ------------------------------------
    // RD 1311/2012 art. 10.1 asks professionals to prefer non-chemical methods,
    // and the SIEX twin follows it: TratamFito requires an applicator, a
    // problem, justifications and an efficacy, but NOT ProductosFito. So a
    // record may carry a product, a non-chemical measure, or both — never
    // neither.
    if new.product_id.is_none() && new.measure_code.is_none() {
        return Err(CueError::Invalid("treatment_without_actuation"));
    }
    validate_measure_fields(
        &tx,
        new.measure_code.as_deref(),
        new.measure_intensity_value,
        new.measure_intensity_unit_code.as_deref(),
    )?;

    // --- the coded reason for treatment + IPM justifications ---------------
    let (problems, justifications) = validated_reasons(
        &tx,
        &country_code,
        std::mem::take(&mut new.problems),
        std::mem::take(&mut new.justifications),
    )?;

    // --- the actuation interval (Anexo III Parte I B) ----------------------
    // The date may be a range. An end before the start is not a correction
    // this layer may guess at.
    if let Some(end) = &new.application_end_date
        && end.as_str() < new.application_date.as_str()
    {
        return Err(CueError::Invalid("end_date_before_start"));
    }
    validate_application_time(new.application_time.as_deref())?;

    validate_total_quantity(
        &tx,
        new.total_quantity_value,
        new.total_quantity_unit_code.as_deref(),
    )?;

    // --- resolve legal snapshots from the referenced rows ------------------
    // The chemical block is resolved as a unit or not at all: a product brings
    // its dose, its authorisation, its substances and its plazo de seguridad
    // with it, and a purely non-chemical actuation has none of them.
    let chemical = match &new.product_id {
        None => {
            // A dose with nothing to dose is a form the farmer half-filled,
            // not a record we may quietly discard half of.
            if new.dose_value.is_some() || new.dose_unit_code.is_some() {
                return Err(CueError::Invalid("dose_without_product"));
            }
            None
        }
        Some(product_id) => {
            let (Some(dose_value), Some(dose_unit_code)) =
                (new.dose_value, new.dose_unit_code.clone())
            else {
                return Err(CueError::Invalid("product_without_dose"));
            };

            let phi_days = new
                .phi_days_used
                .or(product_default_phi(&tx, product_id)?)
                .ok_or(CueError::MissingPhiDays)?;
            let snapshots = resolve_chemical_snapshots(&tx, product_id, &country_code)?;

            Some(ChemicalBlock {
                dose_value,
                dose_unit_code,
                phi_days,
                product_name: snapshots.product_name,
                authorisation_number: snapshots.authorisation_number,
                active_substances: snapshots.active_substances,
            })
        }
    };

    // --- the advisor, when this actuation was an advised one ---------------
    // Anexo III Parte I B.d: "identificación del aplicador y, en su caso, del
    // asesor". Snapshotted like the applicator so a later correction to the
    // advisor's registry entry never rewrites what a past record printed.
    let (advisor_name, advisor_registration) =
        super::advisor_snapshot(&tx, new.advisor_id.as_deref())?;

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
        application_end_date: new.application_end_date.clone(),
        application_time: new.application_time,
        product_id: new.product_id.clone(),
        country_code,
        dose_value: chemical.as_ref().map(|c| c.dose_value),
        dose_unit_code: chemical.as_ref().map(|c| c.dose_unit_code.clone()),
        total_quantity_value: new.total_quantity_value,
        total_quantity_unit_code: new.total_quantity_unit_code,
        target_organism: new.target_organism,
        efficacy_code: new.efficacy_code,
        operator_id: new.operator_id,
        machinery_id: new.machinery_id,
        advisor_id: new.advisor_id,
        advisor_name_snapshot: advisor_name,
        advisor_registration_snapshot: advisor_registration,
        measure_code: new.measure_code,
        measure_intensity_value: new.measure_intensity_value,
        measure_intensity_unit_code: new.measure_intensity_unit_code,
        measure_registration_number: new.measure_registration_number,
        phi_days_used: chemical.as_ref().map(|c| c.phi_days),
        // The plazo de seguridad is counted from the LAST application, so an
        // interval pushes the end date out; a single-day treatment is the
        // degenerate case of the same rule. No product, no plazo — a measure
        // imposes no waiting period before harvest.
        phi_end_date: match &chemical {
            Some(c) => Some(add_days(
                new.application_end_date
                    .as_deref()
                    .unwrap_or(&new.application_date),
                c.phi_days,
            )?),
            None => None,
        },
        product_name_snapshot: chemical.as_ref().map(|c| c.product_name.clone()),
        authorisation_number_snapshot: chemical.as_ref().map(|c| c.authorisation_number.clone()),
        active_substances_snapshot: chemical.as_ref().and_then(|c| c.active_substances.clone()),
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
            id, season_id, farm_id, application_date, application_end_date, application_time,
            product_id, country_code,
            dose_value, dose_unit_code, total_quantity_value, total_quantity_unit_code,
            target_organism, efficacy_code, operator_id, machinery_id,
            advisor_id, advisor_name_snapshot, advisor_registration_snapshot,
            measure_code, measure_intensity_value, measure_intensity_unit_code,
            measure_registration_number, phi_days_used, phi_end_date,
            product_name_snapshot, authorisation_number_snapshot, active_substances_snapshot,
            operator_name_snapshot, operator_licence_snapshot, machinery_roma_snapshot,
            machinery_reganip_snapshot, notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27,
            ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.application_date,
            record.application_end_date,
            record.application_time,
            record.product_id,
            record.country_code,
            record.dose_value,
            record.dose_unit_code,
            record.total_quantity_value,
            record.total_quantity_unit_code,
            record.target_organism,
            record.efficacy_code,
            record.operator_id,
            record.machinery_id,
            record.advisor_id,
            record.advisor_name_snapshot,
            record.advisor_registration_snapshot,
            record.measure_code,
            record.measure_intensity_value,
            record.measure_intensity_unit_code,
            record.measure_registration_number,
            record.phi_days_used,
            record.phi_end_date,
            record.product_name_snapshot,
            record.authorisation_number_snapshot,
            record.active_substances_snapshot,
            record.operator_name_snapshot,
            record.operator_licence_snapshot,
            record.machinery_roma_snapshot,
            record.machinery_reganip_snapshot,
            record.notes,
            record.created_at,
            record.updated_at
        ],
    )?;

    // --- the coded problems + justifications (junction rows) ---------------
    for p in problems {
        insert_problem_row(&tx, &record, p, actor)?;
    }
    for code in justifications {
        insert_justification_row(&tx, &record, code, actor)?;
    }

    // --- the treated plots (multi-plot in one entry) ----------------------
    for p in plots {
        insert_plot_row(&tx, &record, p, actor)?;
    }

    log_insert(
        &tx,
        "treatment_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    tx.commit()?;
    Ok(record)
}

/// Correct a treatment record: the submitted state replaces the stored one,
/// plots, problems and justifications reconciled from it.
///
/// **Snapshots are re-taken only when their FK changes.** Picking a different
/// product re-freezes the product's printed values, because a record naming one
/// product while printing another's registration number would be worse than the
/// mistake it was meant to fix; leaving the product alone keeps what the record
/// already printed, because a registry row corrected later must never rewrite a
/// past entry (the rule `*_snapshot` columns exist for). Correcting a date thus
/// cannot move a single printed value the correction did not name.
///
/// The plazo de seguridad is re-derived, from the interval's END when there is
/// one: an interval only ever pushes the end date out.
pub fn update_treatment_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateTreatmentRecord,
    actor: Option<&str>,
) -> Result<TreatmentRecordWithPlots> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM treatment_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_treatment_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;

    // Every treated plot must still belong to the record's own farm.
    for p in &update.plots {
        let plot_farm: String = tx
            .query_row(
                "SELECT farm_id FROM plot WHERE id = ?1",
                [&p.plot_id],
                |r| r.get(0),
            )
            .map_err(no_rows_to_not_found)?;
        if plot_farm != before.farm_id {
            return Err(CueError::PlotNotOnFarm {
                plot_id: p.plot_id.clone(),
                farm_id: before.farm_id.clone(),
            });
        }
    }

    if update.product_id.is_none() && update.measure_code.is_none() {
        return Err(CueError::Invalid("treatment_without_actuation"));
    }
    validate_measure_fields(
        &tx,
        update.measure_code.as_deref(),
        update.measure_intensity_value,
        update.measure_intensity_unit_code.as_deref(),
    )?;

    let (problems, justifications) = validated_reasons(
        &tx,
        &before.country_code,
        update.problems,
        update.justifications,
    )?;

    if let Some(end) = &update.application_end_date
        && end.as_str() < update.application_date.as_str()
    {
        return Err(CueError::Invalid("end_date_before_start"));
    }
    validate_application_time(update.application_time.as_deref())?;
    validate_total_quantity(
        &tx,
        update.total_quantity_value,
        update.total_quantity_unit_code.as_deref(),
    )?;

    let mut after = before.clone();
    after.application_date = update.application_date;
    after.application_end_date = update.application_end_date;
    after.application_time = update.application_time;
    after.total_quantity_value = update.total_quantity_value;
    after.total_quantity_unit_code = update.total_quantity_unit_code;
    after.target_organism = update.target_organism;
    after.measure_code = update.measure_code;
    after.measure_intensity_value = update.measure_intensity_value;
    after.measure_intensity_unit_code = update.measure_intensity_unit_code;
    after.measure_registration_number = update.measure_registration_number;
    after.notes = update.notes;
    after.updated_at = now_utc_iso();

    // --- the chemical block, still all-or-nothing --------------------------
    match &update.product_id {
        None => {
            if update.dose_value.is_some() || update.dose_unit_code.is_some() {
                return Err(CueError::Invalid("dose_without_product"));
            }
            // A correction that removes the product removes the plazo with it:
            // a non-chemical actuation imposes no waiting period.
            after.product_id = None;
            after.dose_value = None;
            after.dose_unit_code = None;
            after.phi_days_used = None;
            after.phi_end_date = None;
            after.product_name_snapshot = None;
            after.authorisation_number_snapshot = None;
            after.active_substances_snapshot = None;
        }
        Some(product_id) => {
            let (Some(dose_value), Some(dose_unit_code)) =
                (update.dose_value, update.dose_unit_code.clone())
            else {
                return Err(CueError::Invalid("product_without_dose"));
            };
            let default_phi: Option<i64> = tx
                .query_row(
                    "SELECT default_phi_days FROM product WHERE id = ?1",
                    [product_id],
                    |r| r.get(0),
                )
                .map_err(no_rows_to_not_found)?;

            if before.product_id.as_deref() != Some(product_id.as_str()) {
                // A different product: everything it prints is re-frozen from
                // the registry as it stands now.
                let fresh = resolve_chemical_snapshots(&tx, product_id, &before.country_code)?;
                after.product_name_snapshot = Some(fresh.product_name);
                after.authorisation_number_snapshot = Some(fresh.authorisation_number);
                after.active_substances_snapshot = fresh.active_substances;
            }
            after.product_id = Some(product_id.clone());
            after.dose_value = Some(dose_value);
            after.dose_unit_code = Some(dose_unit_code);
            let phi_days = update
                .phi_days_used
                .or(before.phi_days_used)
                .or(default_phi)
                .ok_or(CueError::MissingPhiDays)?;
            after.phi_days_used = Some(phi_days);
            after.phi_end_date = Some(add_days(
                after
                    .application_end_date
                    .as_deref()
                    .unwrap_or(&after.application_date),
                phi_days,
            )?);
        }
    }

    // --- the identifications (Anexo III Parte I B.d) -----------------------
    if update.operator_id != before.operator_id {
        let (name, licence): (String, Option<String>) = tx
            .query_row(
                "SELECT full_name, licence_number FROM operator WHERE id = ?1",
                [&update.operator_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(no_rows_to_not_found)?;
        after.operator_name_snapshot = name;
        after.operator_licence_snapshot = licence;
    }
    after.operator_id = update.operator_id;

    if update.machinery_id != before.machinery_id {
        let (roma, reganip) = match &update.machinery_id {
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
        after.machinery_roma_snapshot = roma;
        after.machinery_reganip_snapshot = reganip;
    }
    after.machinery_id = update.machinery_id;

    if update.advisor_id != before.advisor_id {
        let (name, registration) = super::advisor_snapshot(&tx, update.advisor_id.as_deref())?;
        after.advisor_name_snapshot = name;
        after.advisor_registration_snapshot = registration;
    }
    after.advisor_id = update.advisor_id;

    tx.execute(
        "UPDATE treatment_record SET
            application_date = ?2, application_end_date = ?3, product_id = ?4,
            dose_value = ?5, dose_unit_code = ?6, total_quantity_value = ?7,
            total_quantity_unit_code = ?8, target_organism = ?9, operator_id = ?10,
            machinery_id = ?11, advisor_id = ?12, advisor_name_snapshot = ?13,
            advisor_registration_snapshot = ?14, measure_code = ?15,
            measure_intensity_value = ?16, measure_intensity_unit_code = ?17,
            measure_registration_number = ?18, phi_days_used = ?19, phi_end_date = ?20,
            product_name_snapshot = ?21, authorisation_number_snapshot = ?22,
            active_substances_snapshot = ?23, operator_name_snapshot = ?24,
            operator_licence_snapshot = ?25, machinery_roma_snapshot = ?26,
            machinery_reganip_snapshot = ?27, notes = ?28, updated_at = ?29,
            application_time = ?30
         WHERE id = ?1",
        params![
            id,
            after.application_date,
            after.application_end_date,
            after.product_id,
            after.dose_value,
            after.dose_unit_code,
            after.total_quantity_value,
            after.total_quantity_unit_code,
            after.target_organism,
            after.operator_id,
            after.machinery_id,
            after.advisor_id,
            after.advisor_name_snapshot,
            after.advisor_registration_snapshot,
            after.measure_code,
            after.measure_intensity_value,
            after.measure_intensity_unit_code,
            after.measure_registration_number,
            after.phi_days_used,
            after.phi_end_date,
            after.product_name_snapshot,
            after.authorisation_number_snapshot,
            after.active_substances_snapshot,
            after.operator_name_snapshot,
            after.operator_licence_snapshot,
            after.machinery_roma_snapshot,
            after.machinery_reganip_snapshot,
            after.notes,
            after.updated_at,
            after.application_time
        ],
    )?;
    log_update(
        &tx,
        "treatment_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    reconcile_plots(&tx, &after, update.plots, actor)?;
    reconcile_problems(&tx, &after, problems, actor)?;
    reconcile_justifications(&tx, &after, justifications, actor)?;
    tx.commit()?;
    with_details(conn, after)
}

/// The crop's printed pair, frozen at write time — species and variety as they
/// stand when the record is written, so renaming a crop later cannot rewrite
/// what a past record said was growing there.
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

fn insert_plot_row(
    tx: &Transaction,
    record: &TreatmentRecord,
    want: NewTreatmentPlot,
    actor: Option<&str>,
) -> Result<TreatmentPlot> {
    let (crop_name, variety) = crop_snapshot(tx, want.crop_id.as_deref())?;
    validate_growth_stage(tx, want.growth_stage_code.as_deref())?;
    let row = TreatmentPlot {
        id: Uuid::now_v7().to_string(),
        treatment_record_id: record.id.clone(),
        plot_id: want.plot_id,
        crop_id: want.crop_id,
        surface_treated_ha: want.surface_treated_ha,
        crop_name_snapshot: crop_name,
        variety_snapshot: variety,
        growth_stage_code: want.growth_stage_code,
    };
    tx.execute(
        "INSERT INTO treatment_plot
           (id, treatment_record_id, plot_id, crop_id, surface_treated_ha,
            crop_name_snapshot, variety_snapshot, growth_stage_code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.id,
            row.treatment_record_id,
            row.plot_id,
            row.crop_id,
            row.surface_treated_ha,
            row.crop_name_snapshot,
            row.variety_snapshot,
            row.growth_stage_code
        ],
    )?;
    log_insert(
        tx,
        "treatment_plot",
        &row.id,
        Some(&record.season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

fn insert_problem_row(
    tx: &Transaction,
    record: &TreatmentRecord,
    want: NewTreatmentProblem,
    actor: Option<&str>,
) -> Result<()> {
    let row = TreatmentProblem {
        id: Uuid::now_v7().to_string(),
        treatment_record_id: record.id.clone(),
        reason_category_code: want.reason_category_code,
        problem_code: want.problem_code,
    };
    tx.execute(
        "INSERT INTO treatment_problem
           (id, treatment_record_id, reason_category_code, problem_code)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            row.id,
            row.treatment_record_id,
            row.reason_category_code,
            row.problem_code
        ],
    )?;
    log_insert(
        tx,
        "treatment_problem",
        &row.id,
        Some(&record.season_id),
        actor,
        &row,
    )?;
    Ok(())
}

fn insert_justification_row(
    tx: &Transaction,
    record: &TreatmentRecord,
    code: String,
    actor: Option<&str>,
) -> Result<()> {
    let row = TreatmentJustification {
        id: Uuid::now_v7().to_string(),
        treatment_record_id: record.id.clone(),
        justification_code: code,
    };
    tx.execute(
        "INSERT INTO treatment_justification (id, treatment_record_id, justification_code)
         VALUES (?1, ?2, ?3)",
        params![row.id, row.treatment_record_id, row.justification_code],
    )?;
    log_insert(
        tx,
        "treatment_justification",
        &row.id,
        Some(&record.season_id),
        actor,
        &row,
    )?;
    Ok(())
}

/// The treated plots, reconciled from the submitted state: rows that are gone
/// are deleted, new ones inserted, survivors updated in place.
///
/// Survivors keep their row id — so a plot's audit history stays one thread —
/// and keep their crop snapshot unless the crop itself changed. The snapshot is
/// what protects the book from a crop renamed later, and a correction to the
/// treated surface is no reason to re-take it.
fn reconcile_plots(
    tx: &Transaction,
    record: &TreatmentRecord,
    desired: Vec<NewTreatmentPlot>,
    actor: Option<&str>,
) -> Result<()> {
    let current = plots_of(tx, &record.id)?;
    for existing in &current {
        if !desired.iter().any(|d| d.plot_id == existing.plot_id) {
            tx.execute("DELETE FROM treatment_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "treatment_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&TreatmentPlot>,
            )?;
        }
    }
    for want in desired {
        match current.iter().find(|c| c.plot_id == want.plot_id) {
            Some(existing) => {
                // Every correctable field of the row has to be in this test.
                // One left out is not a visible bug but a silent one: the row
                // is skipped, nothing is written, and the command reports
                // success on a correction it discarded.
                if existing.surface_treated_ha == want.surface_treated_ha
                    && existing.crop_id == want.crop_id
                    && existing.growth_stage_code == want.growth_stage_code
                {
                    continue;
                }
                validate_growth_stage(tx, want.growth_stage_code.as_deref())?;
                let mut after = existing.clone();
                after.surface_treated_ha = want.surface_treated_ha;
                after.growth_stage_code = want.growth_stage_code;
                if existing.crop_id != want.crop_id {
                    let (crop_name, variety) = crop_snapshot(tx, want.crop_id.as_deref())?;
                    after.crop_id = want.crop_id;
                    after.crop_name_snapshot = crop_name;
                    after.variety_snapshot = variety;
                }
                tx.execute(
                    "UPDATE treatment_plot SET crop_id = ?2, surface_treated_ha = ?3,
                        crop_name_snapshot = ?4, variety_snapshot = ?5,
                        growth_stage_code = ?6
                     WHERE id = ?1",
                    params![
                        after.id,
                        after.crop_id,
                        after.surface_treated_ha,
                        after.crop_name_snapshot,
                        after.variety_snapshot,
                        after.growth_stage_code
                    ],
                )?;
                log_update(
                    tx,
                    "treatment_plot",
                    &after.id,
                    Some(&record.season_id),
                    actor,
                    existing,
                    &after,
                )?;
            }
            None => {
                insert_plot_row(tx, record, want, actor)?;
            }
        }
    }
    Ok(())
}

/// The coded problems, reconciled from the submitted state. They carry no
/// snapshot, so a row that is no longer claimed is simply gone.
fn reconcile_problems(
    tx: &Transaction,
    record: &TreatmentRecord,
    desired: Vec<NewTreatmentProblem>,
    actor: Option<&str>,
) -> Result<()> {
    let current = problems_of(tx, &record.id)?;
    for existing in &current {
        if !desired.iter().any(|d| {
            d.reason_category_code == existing.reason_category_code
                && d.problem_code == existing.problem_code
        }) {
            tx.execute(
                "DELETE FROM treatment_problem WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "treatment_problem",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&TreatmentProblem>,
            )?;
        }
    }
    for want in desired {
        if current.iter().any(|c| {
            c.reason_category_code == want.reason_category_code
                && c.problem_code == want.problem_code
        }) {
            continue;
        }
        insert_problem_row(tx, record, want, actor)?;
    }
    Ok(())
}

fn reconcile_justifications(
    tx: &Transaction,
    record: &TreatmentRecord,
    desired: Vec<String>,
    actor: Option<&str>,
) -> Result<()> {
    let current = justifications_of(tx, &record.id)?;
    for existing in &current {
        if !desired.contains(&existing.justification_code) {
            tx.execute(
                "DELETE FROM treatment_justification WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "treatment_justification",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&TreatmentJustification>,
            )?;
        }
    }
    for want in desired {
        if current.iter().any(|c| c.justification_code == want) {
            continue;
        }
        insert_justification_row(tx, record, want, actor)?;
    }
    Ok(())
}

/// Fetch a treatment record with its treated plots, problems and justifications.
pub fn get_treatment_record(conn: &Connection, id: &str) -> Result<TreatmentRecordWithPlots> {
    let record = conn
        .query_row(
            "SELECT * FROM treatment_record WHERE id = ?1",
            [id],
            map_treatment_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    with_details(conn, record)
}

fn with_details(conn: &Connection, record: TreatmentRecord) -> Result<TreatmentRecordWithPlots> {
    let plots = plots_of(conn, &record.id)?;
    let problems = problems_of(conn, &record.id)?;
    let justifications = justifications_of(conn, &record.id)?;
    Ok(TreatmentRecordWithPlots {
        record,
        plots,
        problems,
        justifications,
    })
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
        .map(|record| with_details(conn, record))
        .collect()
}

/// Every record of one farm+season in application order, soft-deleted ones
/// INCLUDED — the SIEX exporter emits deletion entries (`Borrar`) for records
/// that were exported before being deleted, so it must see them. Everything
/// else reads through `list_treatment_records`, which filters them out.
pub(crate) fn list_treatment_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<TreatmentRecordWithPlots>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM treatment_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY application_date ASC, id ASC",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_treatment_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|record| with_details(conn, record))
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

fn problems_of(conn: &Connection, treatment_record_id: &str) -> Result<Vec<TreatmentProblem>> {
    let mut stmt =
        conn.prepare("SELECT * FROM treatment_problem WHERE treatment_record_id = ?1 ORDER BY id")?;
    let problems = stmt
        .query_map([treatment_record_id], |row| {
            Ok(TreatmentProblem {
                id: row.get("id")?,
                treatment_record_id: row.get("treatment_record_id")?,
                reason_category_code: row.get("reason_category_code")?,
                problem_code: row.get("problem_code")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(problems)
}

fn justifications_of(
    conn: &Connection,
    treatment_record_id: &str,
) -> Result<Vec<TreatmentJustification>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM treatment_justification WHERE treatment_record_id = ?1 ORDER BY id",
    )?;
    let justifications = stmt
        .query_map([treatment_record_id], |row| {
            Ok(TreatmentJustification {
                id: row.get("id")?,
                treatment_record_id: row.get("treatment_record_id")?,
                justification_code: row.get("justification_code")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(justifications)
}

/// The one edit a stored treatment record allows: recording (or correcting)
/// the observed efficacy, which is assessed after application and so cannot be
/// demanded at insert time. Everything else on the record stays immutable —
/// it is a legal document. Logged as an update with complete row images.
pub fn set_treatment_efficacy(
    conn: &mut Connection,
    id: &str,
    efficacy_code: Option<String>,
    actor: Option<&str>,
) -> Result<TreatmentRecord> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM treatment_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_treatment_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let mut after = before.clone();
    after.efficacy_code = efficacy_code;
    after.updated_at = now_utc_iso();
    tx.execute(
        "UPDATE treatment_record SET efficacy_code = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, after.efficacy_code, after.updated_at],
    )?;
    log_update(
        &tx,
        "treatment_record",
        id,
        Some(&before.season_id),
        actor,
        &before,
        &after,
    )?;
    tx.commit()?;
    Ok(after)
}

/// Insert-time net for catalogue-coded problems: when the record's country
/// maps the category to a reference catalogue AND that catalogue has been
/// imported (the app imports the vendored snapshot at startup, so in a running
/// app it always is), the code must exist there. Retired codes stay
/// acceptable — providers baja-date codes rather than delete them, and a
/// late-entered record may legitimately reference one. Without an imported
/// catalogue there is nothing to check against and the code is stored as
/// given; the export's schema-validated tests are the second net.
pub(super) fn validate_problem_code(
    tx: &Transaction,
    country: &str,
    category: &str,
    code: &str,
) -> Result<()> {
    let Some(catalogue_id) = siex::problem_catalogue(country, category) else {
        return Ok(());
    };
    match super::resolve_in_catalogue(tx, catalogue_id, code)? {
        Some(false) => Err(CueError::Invalid("unknown_problem_code")),
        _ => Ok(()),
    }
}

/// Per-plot PHI standing across one farm's active treatment records, every
/// season included — the PHI binds the plot physically, not the campaign the
/// record was filed under. Plots with no active treatments (or soft-deleted
/// plots) are absent. Window rule per `alerts::phi_window_is_active`:
/// `[application_date, phi_end_date)`, the end date being the first day
/// harvest is allowed again.
pub fn phi_status_for_farm(
    conn: &Connection,
    farm_id: &str,
    today: &str,
) -> Result<Vec<PlotPhiStatus>> {
    let mut stmt = conn.prepare(
        "SELECT tp.plot_id, tr.application_date, tr.phi_end_date
         FROM treatment_plot tp
         JOIN treatment_record tr ON tr.id = tp.treatment_record_id
         WHERE tr.farm_id = ?1 AND tr.deleted_at IS NULL
           AND tr.phi_end_date IS NOT NULL
           AND tp.plot_id IN (SELECT id FROM plot WHERE deleted_at IS NULL)
         ORDER BY tp.plot_id",
    )?;
    let windows = stmt
        .query_map([farm_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Rows arrive grouped by plot; fold each plot's windows into one status,
    // keeping the latest end date among the windows that contain today.
    let mut statuses: Vec<PlotPhiStatus> = Vec::new();
    for (plot_id, application_date, phi_end_date) in windows {
        let active = phi_window_is_active(&application_date, &phi_end_date, today)?;
        if statuses.last().map(|s| s.plot_id.as_str()) != Some(plot_id.as_str()) {
            statuses.push(PlotPhiStatus {
                plot_id,
                in_phi: false,
                phi_until: None,
            });
        }
        if active
            && let Some(status) = statuses.last_mut()
            && status.phi_until.as_deref() < Some(phi_end_date.as_str())
        {
            status.in_phi = true;
            status.phi_until = Some(phi_end_date);
        }
    }
    Ok(statuses)
}

/// Whether this season still holds treatment records (soft-deleted ones count:
/// their audit history is only reachable through the season they belong to).
///
/// Exists for the shell's `delete_season` guard. Core owns the season row but
/// may never reference `treatment_record`, so it checks the crop half itself and
/// the shell chains this call for the module half.
pub fn season_has_treatments(conn: &Connection, season_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM treatment_record WHERE season_id = ?1",
        [season_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Crops this farm's treatment records already point at, in one season.
///
/// Exists for the SIGPAC declared-crops import, which may only propose
/// overwriting a crop nothing has been applied to yet. Past records are safe
/// from crop edits either way — `treatment_plot` froze the species and variety
/// at write time — but a crop that backs this season's treatments is also what
/// the record book's section 2.1 states beside them, and rewriting one from a
/// third party's declaration would make the two disagree.
///
/// Soft-deleted treatments are excluded: they have left the book, so the crop
/// under them is free again. Called by the shell, which chains it into the
/// module-sigpac proposal — the two modules never call each other.
pub fn crop_ids_with_treatments(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT tp.crop_id FROM treatment_plot tp
         JOIN treatment_record tr ON tr.id = tp.treatment_record_id
         JOIN plot p ON p.id = tp.plot_id
         WHERE tr.season_id = ?1 AND p.farm_id = ?2
           AND tr.deleted_at IS NULL AND tp.crop_id IS NOT NULL",
    )?;
    let ids = stmt
        .query_map(params![season_id, farm_id], |row| row.get(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(ids)
}

/// Soft-delete a regulatory record (official records are never hard-deleted).
/// Both the before- and after-images in the audit log are complete rows.
pub fn soft_delete_treatment_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
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
        actor,
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
        application_end_date: row.get("application_end_date")?,
        application_time: row.get("application_time")?,
        product_id: row.get("product_id")?,
        country_code: row.get("country_code")?,
        dose_value: row.get("dose_value")?,
        dose_unit_code: row.get("dose_unit_code")?,
        total_quantity_value: row.get("total_quantity_value")?,
        total_quantity_unit_code: row.get("total_quantity_unit_code")?,
        target_organism: row.get("target_organism")?,
        efficacy_code: row.get("efficacy_code")?,
        operator_id: row.get("operator_id")?,
        machinery_id: row.get("machinery_id")?,
        advisor_id: row.get("advisor_id")?,
        advisor_name_snapshot: row.get("advisor_name_snapshot")?,
        advisor_registration_snapshot: row.get("advisor_registration_snapshot")?,
        measure_code: row.get("measure_code")?,
        measure_intensity_value: row.get("measure_intensity_value")?,
        measure_intensity_unit_code: row.get("measure_intensity_unit_code")?,
        measure_registration_number: row.get("measure_registration_number")?,
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
        growth_stage_code: row.get("growth_stage_code")?,
    })
}
