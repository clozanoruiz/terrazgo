// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model sections 3.3, 3.4 and 3.5 in one table, plus the stored
//! "APLICA TRATAMIENTO: NO" that heads each conditional register.
//!
//! Shaped after `treatment.rs` on purpose: the SIEX twins
//! (`TratamientosPostCosecha`, `TratamientosEdifInstalaciones`) require coded
//! problems, coded justifications, a named applicator and an observed efficacy,
//! none of which the printed model shows. Capturing to the stricter shape means
//! a future un-parking of the export needs no migration.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use super::treatment::validated_reasons;
use crate::date::now_utc_iso;
use crate::error::{CueError, Result};
use crate::models::{
    NewNonFieldTreatment, NewTreatmentProblem, NonFieldTreatment, NonFieldTreatmentDetail,
    NonFieldTreatmentJustification, NonFieldTreatmentProblem, RegisterDeclaration,
    UpdateNonFieldTreatment,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use uuid::Uuid;

/// What each subject is measured in. The model's own footnotes: produce in
/// tonnes (3.3), premises and vehicles in cubic metres (3.4, 3.5). Recording a
/// warehouse in tonnes is not a slip, it is a different claim.
fn subject_unit(kind: &str) -> Option<&'static str> {
    match kind {
        "postharvest" => Some("t"),
        "storage_premises" | "transport" => Some("m3"),
        _ => None,
    }
}

/// Which `register_kind` a subject's records belong to. The two vocabularies
/// coincide today (the register list additionally carries seed treatment), but
/// they answer different questions, so the mapping is explicit.
fn register_of(subject_kind: &str) -> &str {
    subject_kind
}

/// Insert one non-field treatment with its coded problems and justifications,
/// in a single transaction: derives the country from the farm, freezes the
/// legal snapshots, and withdraws any standing "nothing to declare" for the
/// register it lands in.
pub fn insert_non_field_treatment(
    conn: &mut Connection,
    mut new: NewNonFieldTreatment,
    actor: Option<&str>,
) -> Result<NonFieldTreatmentDetail> {
    let tx = conn.transaction()?;

    // --- country, derived from the farm (the treatment_record rule) --------
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

    // --- what was treated --------------------------------------------------
    let subject_description = new.subject_description.trim().to_string();
    if subject_description.is_empty() {
        return Err(CueError::Invalid("empty_subject"));
    }
    let expected_unit =
        subject_unit(&new.subject_kind_code).ok_or(CueError::Invalid("unknown_subject_kind"))?;
    match (&new.treated_quantity_value, &new.treated_quantity_unit_code) {
        (None, None) => {}
        (Some(value), Some(unit)) if *value > 0.0 && unit == expected_unit => {}
        _ => return Err(CueError::Invalid("quantity_unit_mismatch")),
    }

    // --- how much product was used ("Cantidad utilizada, kg o l") ----------
    match (&new.product_quantity_value, &new.product_quantity_unit_code) {
        (None, None) => {}
        (Some(value), Some(unit)) if *value > 0.0 && (unit == "kg" || unit == "l") => {}
        _ => return Err(CueError::Invalid("invalid_product_quantity")),
    }

    // --- coded problems + IPM justifications, ≥1 of each -------------------
    let (problems, justifications) = validated_reasons(
        &tx,
        &country_code,
        std::mem::take(&mut new.problems),
        std::mem::take(&mut new.justifications),
    )?;

    // --- legal snapshots ---------------------------------------------------
    let (product_name, authorisation_number) =
        product_snapshot(&tx, &new.product_id, &country_code)?;
    let (operator_name, operator_licence) = operator_snapshot(&tx, &new.operator_id)?;
    let (machinery_roma, machinery_reganip) = machinery_snapshot(&tx, new.machinery_id.as_deref())?;
    // Anexo III Parte I B.d, which reaches these registers through B.b and B.f.
    let (advisor_name, advisor_registration) =
        super::advisor_snapshot(&tx, new.advisor_id.as_deref())?;

    // --- build and insert --------------------------------------------------
    let now = now_utc_iso();
    let record = NonFieldTreatment {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id.clone(),
        farm_id: new.farm_id.clone(),
        country_code,
        subject_kind_code: new.subject_kind_code,
        treated_on: new.treated_on,
        subject_description,
        subject_product_code: new.subject_product_code,
        treated_quantity_value: new.treated_quantity_value,
        treated_quantity_unit_code: new.treated_quantity_unit_code,
        product_id: new.product_id,
        product_quantity_value: new.product_quantity_value,
        product_quantity_unit_code: new.product_quantity_unit_code,
        operator_id: new.operator_id,
        machinery_id: new.machinery_id,
        advisor_id: new.advisor_id,
        advisor_name_snapshot: advisor_name,
        advisor_registration_snapshot: advisor_registration,
        efficacy_code: new.efficacy_code,
        product_name_snapshot: product_name,
        authorisation_number_snapshot: authorisation_number,
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
        "INSERT INTO non_field_treatment (
            id, season_id, farm_id, country_code, subject_kind_code, treated_on,
            subject_description, subject_product_code, treated_quantity_value,
            treated_quantity_unit_code, product_id, product_quantity_value,
            product_quantity_unit_code, operator_id, machinery_id, advisor_id,
            advisor_name_snapshot, advisor_registration_snapshot, efficacy_code,
            product_name_snapshot, authorisation_number_snapshot, operator_name_snapshot,
            operator_licence_snapshot, machinery_roma_snapshot, machinery_reganip_snapshot,
            notes, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28
         )",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.country_code,
            record.subject_kind_code,
            record.treated_on,
            record.subject_description,
            record.subject_product_code,
            record.treated_quantity_value,
            record.treated_quantity_unit_code,
            record.product_id,
            record.product_quantity_value,
            record.product_quantity_unit_code,
            record.operator_id,
            record.machinery_id,
            record.advisor_id,
            record.advisor_name_snapshot,
            record.advisor_registration_snapshot,
            record.efficacy_code,
            record.product_name_snapshot,
            record.authorisation_number_snapshot,
            record.operator_name_snapshot,
            record.operator_licence_snapshot,
            record.machinery_roma_snapshot,
            record.machinery_reganip_snapshot,
            record.notes,
            record.created_at,
            record.updated_at
        ],
    )?;

    // --- junction rows, each logged under its own id -----------------------
    let mut problem_rows = Vec::new();
    for p in problems {
        problem_rows.push(insert_problem_row(&tx, &record, p, actor)?);
    }
    let mut justification_rows = Vec::new();
    for code in justifications {
        justification_rows.push(insert_justification_row(&tx, &record, code, actor)?);
    }

    log_insert(
        &tx,
        "non_field_treatment",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;

    // A record contradicts any standing "nothing to declare" for its register.
    // The record is the stronger statement, so the declaration is withdrawn
    // here rather than left to print beside it.
    withdraw_declaration_tx(
        &tx,
        &record.farm_id,
        &record.season_id,
        register_of(&record.subject_kind_code),
        actor,
    )?;

    tx.commit()?;
    Ok(NonFieldTreatmentDetail {
        record,
        problems: problem_rows,
        justifications: justification_rows,
    })
}

/// Correct a non-field treatment, on the same terms as a field one: the
/// submitted state replaces the stored one, the coded problems and
/// justifications are reconciled from it, and a snapshot is re-taken only when
/// the row it froze is a different row. Correcting the date of an actuation
/// therefore cannot move the product name it printed.
///
/// The subject KIND is not correctable here — see [`UpdateNonFieldTreatment`].
pub fn update_non_field_treatment(
    conn: &mut Connection,
    id: &str,
    update: UpdateNonFieldTreatment,
    actor: Option<&str>,
) -> Result<NonFieldTreatmentDetail> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM non_field_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;

    let subject_description = update.subject_description.trim().to_string();
    if subject_description.is_empty() {
        return Err(CueError::Invalid("empty_subject"));
    }
    // The kind is frozen, so what the quantity must be measured in is too:
    // recording a warehouse in tonnes stays a different claim, not a slip.
    let expected_unit =
        subject_unit(&before.subject_kind_code).ok_or(CueError::Invalid("unknown_subject_kind"))?;
    match (
        &update.treated_quantity_value,
        &update.treated_quantity_unit_code,
    ) {
        (None, None) => {}
        (Some(value), Some(unit)) if *value > 0.0 && unit == expected_unit => {}
        _ => return Err(CueError::Invalid("quantity_unit_mismatch")),
    }
    match (
        &update.product_quantity_value,
        &update.product_quantity_unit_code,
    ) {
        (None, None) => {}
        (Some(value), Some(unit)) if *value > 0.0 && (unit == "kg" || unit == "l") => {}
        _ => return Err(CueError::Invalid("invalid_product_quantity")),
    }

    let (problems, justifications) = validated_reasons(
        &tx,
        &before.country_code,
        update.problems,
        update.justifications,
    )?;

    let mut after = before.clone();
    after.treated_on = update.treated_on;
    after.subject_description = subject_description;
    after.subject_product_code = update.subject_product_code;
    after.treated_quantity_value = update.treated_quantity_value;
    after.treated_quantity_unit_code = update.treated_quantity_unit_code;
    after.product_quantity_value = update.product_quantity_value;
    after.product_quantity_unit_code = update.product_quantity_unit_code;
    after.notes = update.notes;
    after.updated_at = now_utc_iso();

    if update.product_id != before.product_id {
        let (name, authorisation) =
            product_snapshot(&tx, &update.product_id, &before.country_code)?;
        after.product_name_snapshot = name;
        after.authorisation_number_snapshot = authorisation;
    }
    after.product_id = update.product_id;

    if update.operator_id != before.operator_id {
        let (name, licence) = operator_snapshot(&tx, &update.operator_id)?;
        after.operator_name_snapshot = name;
        after.operator_licence_snapshot = licence;
    }
    after.operator_id = update.operator_id;

    if update.machinery_id != before.machinery_id {
        let (roma, reganip) = machinery_snapshot(&tx, update.machinery_id.as_deref())?;
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
        "UPDATE non_field_treatment SET
            treated_on = ?2, subject_description = ?3, subject_product_code = ?4,
            treated_quantity_value = ?5, treated_quantity_unit_code = ?6, product_id = ?7,
            product_quantity_value = ?8, product_quantity_unit_code = ?9, operator_id = ?10,
            machinery_id = ?11, advisor_id = ?12, advisor_name_snapshot = ?13,
            advisor_registration_snapshot = ?14, product_name_snapshot = ?15,
            authorisation_number_snapshot = ?16, operator_name_snapshot = ?17,
            operator_licence_snapshot = ?18, machinery_roma_snapshot = ?19,
            machinery_reganip_snapshot = ?20, notes = ?21, updated_at = ?22
         WHERE id = ?1",
        params![
            id,
            after.treated_on,
            after.subject_description,
            after.subject_product_code,
            after.treated_quantity_value,
            after.treated_quantity_unit_code,
            after.product_id,
            after.product_quantity_value,
            after.product_quantity_unit_code,
            after.operator_id,
            after.machinery_id,
            after.advisor_id,
            after.advisor_name_snapshot,
            after.advisor_registration_snapshot,
            after.product_name_snapshot,
            after.authorisation_number_snapshot,
            after.operator_name_snapshot,
            after.operator_licence_snapshot,
            after.machinery_roma_snapshot,
            after.machinery_reganip_snapshot,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "non_field_treatment",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    reconcile_problems(&tx, &after, problems, actor)?;
    reconcile_justifications(&tx, &after, justifications, actor)?;
    tx.commit()?;
    with_details(conn, after)
}

/// What the product prints on the record. The authorisation number is optional
/// here, unlike a field treatment's: these registers are not Anexo III B.g's
/// crop-treatment case, and the model leaves the cell hand-fillable.
fn product_snapshot(
    tx: &Transaction,
    product_id: &str,
    country_code: &str,
) -> Result<(String, Option<String>)> {
    let name: String = tx
        .query_row(
            "SELECT commercial_name FROM product WHERE id = ?1",
            [product_id],
            |r| r.get(0),
        )
        .map_err(no_rows_to_not_found)?;
    let authorisation: Option<String> = tx
        .query_row(
            "SELECT authorisation_number FROM product_authorisation
             WHERE product_id = ?1 AND country_code = ?2
             ORDER BY authorisation_number LIMIT 1",
            params![product_id, country_code],
            |r| r.get(0),
        )
        .optional()?;
    Ok((name, authorisation))
}

fn operator_snapshot(tx: &Transaction, operator_id: &str) -> Result<(String, Option<String>)> {
    tx.query_row(
        "SELECT full_name, licence_number FROM operator WHERE id = ?1",
        [operator_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(no_rows_to_not_found)
}

fn machinery_snapshot(
    tx: &Transaction,
    machinery_id: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    let Some(id) = machinery_id else {
        return Ok((None, None));
    };
    Ok(tx
        .query_row(
            "SELECT roma_number, reganip_number FROM machinery_es_extension
             WHERE machinery_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((None, None)))
}

fn insert_problem_row(
    tx: &Transaction,
    record: &NonFieldTreatment,
    want: NewTreatmentProblem,
    actor: Option<&str>,
) -> Result<NonFieldTreatmentProblem> {
    let row = NonFieldTreatmentProblem {
        id: Uuid::now_v7().to_string(),
        non_field_treatment_id: record.id.clone(),
        reason_category_code: want.reason_category_code,
        problem_code: want.problem_code,
    };
    tx.execute(
        "INSERT INTO non_field_treatment_problem
            (id, non_field_treatment_id, reason_category_code, problem_code)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            row.id,
            row.non_field_treatment_id,
            row.reason_category_code,
            row.problem_code
        ],
    )?;
    log_insert(
        tx,
        "non_field_treatment_problem",
        &row.id,
        Some(&record.season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

fn insert_justification_row(
    tx: &Transaction,
    record: &NonFieldTreatment,
    code: String,
    actor: Option<&str>,
) -> Result<NonFieldTreatmentJustification> {
    let row = NonFieldTreatmentJustification {
        id: Uuid::now_v7().to_string(),
        non_field_treatment_id: record.id.clone(),
        justification_code: code,
    };
    tx.execute(
        "INSERT INTO non_field_treatment_justification
            (id, non_field_treatment_id, justification_code)
         VALUES (?1, ?2, ?3)",
        params![row.id, row.non_field_treatment_id, row.justification_code],
    )?;
    log_insert(
        tx,
        "non_field_treatment_justification",
        &row.id,
        Some(&record.season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

/// The coded problems, reconciled from the submitted state. They carry no
/// snapshot, so a claim that is gone is simply deleted; survivors keep their
/// row id so their audit history stays one thread.
fn reconcile_problems(
    tx: &Transaction,
    record: &NonFieldTreatment,
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
                "DELETE FROM non_field_treatment_problem WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "non_field_treatment_problem",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&NonFieldTreatmentProblem>,
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
    record: &NonFieldTreatment,
    desired: Vec<String>,
    actor: Option<&str>,
) -> Result<()> {
    let current = justifications_of(tx, &record.id)?;
    for existing in &current {
        if !desired.contains(&existing.justification_code) {
            tx.execute(
                "DELETE FROM non_field_treatment_justification WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "non_field_treatment_justification",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&NonFieldTreatmentJustification>,
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

pub fn get_non_field_treatment(conn: &Connection, id: &str) -> Result<NonFieldTreatmentDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM non_field_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    with_details(conn, record)
}

/// The register as the book prints it: oldest first within each section, which
/// is how a record book reads.
pub fn list_non_field_treatments(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<NonFieldTreatmentDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM non_field_treatment
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY treated_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    records
        .into_iter()
        .map(|record| with_details(conn, record))
        .collect()
}

fn problems_of(conn: &Connection, record_id: &str) -> Result<Vec<NonFieldTreatmentProblem>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM non_field_treatment_problem WHERE non_field_treatment_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], |row| {
            Ok(NonFieldTreatmentProblem {
                id: row.get("id")?,
                non_field_treatment_id: row.get("non_field_treatment_id")?,
                reason_category_code: row.get("reason_category_code")?,
                problem_code: row.get("problem_code")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn justifications_of(
    conn: &Connection,
    record_id: &str,
) -> Result<Vec<NonFieldTreatmentJustification>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM non_field_treatment_justification
         WHERE non_field_treatment_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], |row| {
            Ok(NonFieldTreatmentJustification {
                id: row.get("id")?,
                non_field_treatment_id: row.get("non_field_treatment_id")?,
                justification_code: row.get("justification_code")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn with_details(conn: &Connection, record: NonFieldTreatment) -> Result<NonFieldTreatmentDetail> {
    let problems = problems_of(conn, &record.id)?;
    let justifications = justifications_of(conn, &record.id)?;
    Ok(NonFieldTreatmentDetail {
        record,
        problems,
        justifications,
    })
}

/// The one edit these records allow, for the same reason field treatments do:
/// efficacy is observed after the fact and cannot be demanded at insert.
pub fn set_non_field_efficacy(
    conn: &mut Connection,
    id: &str,
    efficacy_code: Option<String>,
    actor: Option<&str>,
) -> Result<NonFieldTreatment> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM non_field_treatment WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let mut after = before.clone();
    after.efficacy_code = efficacy_code;
    after.updated_at = now_utc_iso();
    tx.execute(
        "UPDATE non_field_treatment SET efficacy_code = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, after.efficacy_code, after.updated_at],
    )?;
    log_update(
        &tx,
        "non_field_treatment",
        id,
        Some(&before.season_id),
        actor,
        &before,
        &after,
    )?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete, like every other regulatory record: the row stays, both audit
/// images are complete.
pub fn soft_delete_non_field_treatment(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM non_field_treatment WHERE id = ?1 AND deleted_at IS NULL",
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
        "UPDATE non_field_treatment SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "non_field_treatment",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

fn map_record(row: &Row) -> rusqlite::Result<NonFieldTreatment> {
    Ok(NonFieldTreatment {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        country_code: row.get("country_code")?,
        subject_kind_code: row.get("subject_kind_code")?,
        treated_on: row.get("treated_on")?,
        subject_description: row.get("subject_description")?,
        subject_product_code: row.get("subject_product_code")?,
        treated_quantity_value: row.get("treated_quantity_value")?,
        treated_quantity_unit_code: row.get("treated_quantity_unit_code")?,
        product_id: row.get("product_id")?,
        product_quantity_value: row.get("product_quantity_value")?,
        product_quantity_unit_code: row.get("product_quantity_unit_code")?,
        operator_id: row.get("operator_id")?,
        machinery_id: row.get("machinery_id")?,
        advisor_id: row.get("advisor_id")?,
        advisor_name_snapshot: row.get("advisor_name_snapshot")?,
        advisor_registration_snapshot: row.get("advisor_registration_snapshot")?,
        efficacy_code: row.get("efficacy_code")?,
        product_name_snapshot: row.get("product_name_snapshot")?,
        authorisation_number_snapshot: row.get("authorisation_number_snapshot")?,
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

// ---------------------------------------------------------------------------
// "APLICA TRATAMIENTO: NO" — the stored negative
// ---------------------------------------------------------------------------

/// Record that a register held nothing this campaign. SÍ is derivable from rows
/// existing; NO is not — an empty register is indistinguishable from an
/// unfilled one, and only one of those is evidence the farmer checked. Same
/// philosophy as `plot_zone_flag`'s stored 'outside' result.
///
/// Restating updates the standing row rather than adding a second, so table
/// 3.x never prints the same declaration twice.
pub fn set_register_declaration(
    conn: &mut Connection,
    farm_id: &str,
    season_id: &str,
    register_code: &str,
    declared_on: &str,
    actor: Option<&str>,
) -> Result<RegisterDeclaration> {
    let tx = conn.transaction()?;

    // Declaring a register empty while it holds records would put a false
    // statement in a legal document. Which table answers that depends on the
    // register: three of them are backed by `non_field_treatment`, the seed one
    // by its own table.
    if register_has_rows(&tx, farm_id, season_id, register_code)? {
        return Err(CueError::Invalid("register_has_rows"));
    }

    let now = now_utc_iso();
    let standing = tx
        .query_row(
            "SELECT * FROM register_declaration
             WHERE farm_id = ?1 AND season_id = ?2 AND register_code = ?3
               AND deleted_at IS NULL",
            params![farm_id, season_id, register_code],
            map_declaration,
        )
        .optional()?;

    let declaration = match standing {
        Some(before) => {
            let mut after = before.clone();
            after.declared_on = declared_on.to_string();
            after.updated_at = now;
            tx.execute(
                "UPDATE register_declaration SET declared_on = ?2, updated_at = ?3 WHERE id = ?1",
                params![after.id, after.declared_on, after.updated_at],
            )?;
            log_update(
                &tx,
                "register_declaration",
                &after.id,
                Some(season_id),
                actor,
                &before,
                &after,
            )?;
            after
        }
        None => {
            let row = RegisterDeclaration {
                id: Uuid::now_v7().to_string(),
                farm_id: farm_id.to_string(),
                season_id: season_id.to_string(),
                register_code: register_code.to_string(),
                declared_on: declared_on.to_string(),
                created_at: now.clone(),
                updated_at: now,
                deleted_at: None,
            };
            tx.execute(
                "INSERT INTO register_declaration
                    (id, farm_id, season_id, register_code, declared_on, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row.id,
                    row.farm_id,
                    row.season_id,
                    row.register_code,
                    row.declared_on,
                    row.created_at,
                    row.updated_at
                ],
            )?;
            log_insert(
                &tx,
                "register_declaration",
                &row.id,
                Some(season_id),
                actor,
                &row,
            )?;
            row
        }
    };

    tx.commit()?;
    Ok(declaration)
}

/// Take back a declaration made in error. Soft delete, so the audit trail keeps
/// saying the farmer once declared it.
pub fn clear_register_declaration(
    conn: &mut Connection,
    farm_id: &str,
    season_id: &str,
    register_code: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    withdraw_declaration_tx(&tx, farm_id, season_id, register_code, actor)?;
    tx.commit()?;
    Ok(())
}

/// Whether any non-field treatment hangs off this season — one arm of the guard
/// the shell chains before deleting a season. Soft-deleted records count, like
/// `season_has_treatments`: their audit history is only reachable through the
/// season they belong to.
pub(super) fn season_has_non_field_treatments(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM non_field_treatment WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

/// Whether a register currently holds live records. The `register_kind` list is
/// wider than `non_field_subject_kind`: seed treatment (3.2) is a register too,
/// backed by its own table, so the lookup dispatches on the code rather than
/// assuming one home.
fn register_has_rows(
    tx: &Transaction,
    farm_id: &str,
    season_id: &str,
    register_code: &str,
) -> Result<bool> {
    if register_code == super::seed_treatment::REGISTER {
        return super::seed_treatment::register_has_rows(tx, farm_id, season_id);
    }
    let held: bool = tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM non_field_treatment
             WHERE farm_id = ?1 AND season_id = ?2 AND subject_kind_code = ?3
               AND deleted_at IS NULL
         )",
        params![farm_id, season_id, register_code],
        |r| r.get(0),
    )?;
    Ok(held)
}

/// Withdraw the standing declaration for one register, if there is one. Shared
/// by the explicit clear, by a non-field insert that contradicts it, and by a
/// sowing (`seed_treatment.rs`) — hence crate-visible.
pub(super) fn withdraw_declaration_tx(
    tx: &Transaction,
    farm_id: &str,
    season_id: &str,
    register_code: &str,
    actor: Option<&str>,
) -> Result<()> {
    let Some(before) = tx
        .query_row(
            "SELECT * FROM register_declaration
             WHERE farm_id = ?1 AND season_id = ?2 AND register_code = ?3
               AND deleted_at IS NULL",
            params![farm_id, season_id, register_code],
            map_declaration,
        )
        .optional()?
    else {
        return Ok(());
    };

    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE register_declaration SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![before.id, now],
    )?;
    log_delete(
        tx,
        "register_declaration",
        &before.id,
        Some(season_id),
        actor,
        &before,
        Some(&after),
    )?;
    Ok(())
}

pub fn list_register_declarations(
    conn: &Connection,
    farm_id: &str,
    season_id: &str,
) -> Result<Vec<RegisterDeclaration>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM register_declaration
         WHERE farm_id = ?1 AND season_id = ?2 AND deleted_at IS NULL
         ORDER BY register_code",
    )?;
    let rows = stmt
        .query_map(params![farm_id, season_id], map_declaration)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_declaration(row: &Row) -> rusqlite::Result<RegisterDeclaration> {
    Ok(RegisterDeclaration {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        season_id: row.get("season_id")?,
        register_code: row.get("register_code")?,
        declared_on: row.get("declared_on")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
