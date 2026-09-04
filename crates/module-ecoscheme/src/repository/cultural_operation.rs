// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model 9.2 and the book's "9.6" — the register of what was done on the land.
//!
//! The decree's widest register, and the clearest case for deriving a table
//! from the decree rather than the form. RD 1048/2022 art. 31 asks for *"la
//! fecha y las actividades realizadas"* on a P2 plot and art. 31.4.d for *"las
//! labores de siega realizadas"*, both within a month; **anexo IV asks for the
//! same annotation on each pasto comunal plot and the printed model gives it no
//! page at all**. Two further duties land here — art. 45.2's nivelación and
//! caballones dates, which model 9.3 omits from its own columns, and art.
//! 42.1.c's maintenance of a cover, which model 9.4 prints as its Siega and
//! Desbrozado columns.
//!
//! SIEX twin: `LaboresCulturales`.
//!
//! **Fully correctable from the start**, like every register built since
//! `seed_treatment`: nothing here is a snapshot of another row's identity, so
//! no later edit elsewhere can rewrite it underneath.
//!
//! Each write comes in two forms: a `pub fn` that opens its own transaction,
//! and a `pub(super) fn …_tx` that joins one already open. The cover register
//! writes its maintenance lines through the second, so a cover and the siega
//! that maintains it land together or not at all — and, more to the point, so
//! there is exactly ONE place where an operation is validated and audited,
//! however many forms can create one.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::{no_rows_to_not_found, validated_cover_link};
use crate::error::{EcoschemeError, Result};
use crate::models::{
    CulturalOperation, CulturalOperationDetail, CulturalOperationPlot, NewCulturalOperation,
    UpdateCulturalOperation,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::sql::children_by_parent;
use uuid::Uuid;

/// The duties a cultural operation can evidence — every practice except
/// `extensive_grazing`.
///
/// P1's art. 30.2 ter duty is about the grazing DATES and nothing else, and a
/// grazing is its own register (9.1) with its own table; an operation filed
/// against it would print on no page. The other five each name an activity
/// carried out on the land:
///
///   * `sustainable_mowing` — art. 31 / 31.4.d, model 9.2.
///   * `communal_pasture` — anexo IV, the duty with no printed page.
///   * `flooded_biodiversity` — art. 45.2's nivelación and caballones, the two
///     dates model 9.3 leaves out (seam 3 prints them).
///   * `plant_cover` — art. 42.1.c's maintenance of a live cover (seam 4 links
///     the row back to the cover it maintained).
///   * `inert_cover` — art. 43.1.a's evidence chain: the poda whose residue
///     was triturated onto the ground is what brought the cover into being.
///
/// The list is at its FINAL value already, so seams 3 and 4 add pages rather
/// than reopening validation. Which of them a form offers is the form's
/// business: this is what the decree admits.
const OPERATION_PRACTICES: [&str; 5] = [
    "sustainable_mowing",
    "communal_pasture",
    "flooded_biodiversity",
    "plant_cover",
    "inert_cover",
];

pub fn insert_cultural_operation(
    conn: &mut Connection,
    new: NewCulturalOperation,
    actor: Option<&str>,
) -> Result<CulturalOperationDetail> {
    let tx = conn.transaction()?;
    let detail = insert_cultural_operation_tx(&tx, new, actor)?;
    tx.commit()?;
    Ok(detail)
}

/// The insert itself, inside a transaction the caller owns.
pub(super) fn insert_cultural_operation_tx(
    tx: &Transaction,
    new: NewCulturalOperation,
    actor: Option<&str>,
) -> Result<CulturalOperationDetail> {
    validate_interval(&new.performed_on, new.performed_end_date.as_deref())?;
    validate_practice(&new.practice_code)?;

    let plot_ids = validated_plots(tx, &new.farm_id, &new.plot_ids)?;
    let soil_cover_id = validated_cover_link(
        tx,
        new.soil_cover_id.as_deref(),
        &new.farm_id,
        &new.practice_code,
    )?;

    let now = now_utc_iso();
    let record = CulturalOperation {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        practice_code: new.practice_code,
        operation_kind_code: new.operation_kind_code,
        performed_on: new.performed_on,
        performed_end_date: new.performed_end_date,
        activity_description: blank_to_none(new.activity_description),
        residue_destination_code: blank_to_none(new.residue_destination_code),
        soil_cover_id,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO cultural_operation (
            id, season_id, farm_id, practice_code, operation_kind_code, performed_on,
            performed_end_date, activity_description, residue_destination_code,
            soil_cover_id, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.practice_code,
            record.operation_kind_code,
            record.performed_on,
            record.performed_end_date,
            record.activity_description,
            record.residue_destination_code,
            record.soil_cover_id,
            record.notes,
            record.created_at,
            record.updated_at
        ],
    )?;

    let mut plot_rows = Vec::new();
    for plot_id in plot_ids {
        plot_rows.push(insert_plot_row(
            tx,
            &record.id,
            &record.season_id,
            &plot_id,
            actor,
        )?);
    }

    log_insert(
        tx,
        "cultural_operation",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    Ok(CulturalOperationDetail {
        record,
        plots: plot_rows,
    })
}

/// Full-row correction, with the plots reconciled from the submitted state.
pub fn update_cultural_operation(
    conn: &mut Connection,
    id: &str,
    update: UpdateCulturalOperation,
    actor: Option<&str>,
) -> Result<CulturalOperationDetail> {
    let tx = conn.transaction()?;
    let detail = update_cultural_operation_tx(&tx, id, update, actor)?;
    tx.commit()?;
    Ok(detail)
}

/// The correction itself, inside a transaction the caller owns.
pub(super) fn update_cultural_operation_tx(
    tx: &Transaction,
    id: &str,
    update: UpdateCulturalOperation,
    actor: Option<&str>,
) -> Result<CulturalOperationDetail> {
    validate_interval(&update.performed_on, update.performed_end_date.as_deref())?;
    validate_practice(&update.practice_code)?;

    let before = tx
        .query_row(
            "SELECT * FROM cultural_operation WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(EcoschemeError::NotFound)?;
    let plot_ids = validated_plots(tx, &before.farm_id, &update.plot_ids)?;
    let soil_cover_id = validated_cover_link(
        tx,
        update.soil_cover_id.as_deref(),
        &before.farm_id,
        &update.practice_code,
    )?;

    let mut after = before.clone();
    after.practice_code = update.practice_code;
    after.operation_kind_code = update.operation_kind_code;
    after.performed_on = update.performed_on;
    after.performed_end_date = update.performed_end_date;
    after.activity_description = blank_to_none(update.activity_description);
    after.residue_destination_code = blank_to_none(update.residue_destination_code);
    after.soil_cover_id = soil_cover_id;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE cultural_operation SET
            practice_code = ?2, operation_kind_code = ?3, performed_on = ?4,
            performed_end_date = ?5, activity_description = ?6,
            residue_destination_code = ?7, soil_cover_id = ?8, notes = ?9,
            updated_at = ?10
         WHERE id = ?1",
        params![
            id,
            after.practice_code,
            after.operation_kind_code,
            after.performed_on,
            after.performed_end_date,
            after.activity_description,
            after.residue_destination_code,
            after.soil_cover_id,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        tx,
        "cultural_operation",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(tx, &after, &plot_ids, actor)?;
    Ok(CulturalOperationDetail {
        record: after,
        plots: plot_rows,
    })
}

pub fn soft_delete_cultural_operation(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    soft_delete_cultural_operation_tx(&tx, id, actor)?;
    tx.commit()?;
    Ok(())
}

/// The withdrawal itself, inside a transaction the caller owns. Used by the
/// cover register when a maintenance line stops being sent.
pub(super) fn soft_delete_cultural_operation_tx(
    tx: &Transaction,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let before = tx
        .query_row(
            "SELECT * FROM cultural_operation WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(EcoschemeError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE cultural_operation SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        tx,
        "cultural_operation",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    Ok(())
}

pub fn get_cultural_operation(conn: &Connection, id: &str) -> Result<CulturalOperationDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM cultural_operation WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    Ok(CulturalOperationDetail { record, plots })
}

/// Oldest first, the order a record book reads in.
pub fn list_cultural_operations(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<CulturalOperationDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cultural_operation
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY performed_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Every cultural operation of this farm+season INCLUDING the soft-deleted ones
/// — the SIEX export, which turns a withdrawn record into a `Borrar` entry under
/// the alias it was first exported with. Its name is the guard: a caller that is
/// not building an export and wants deleted rows is almost certainly mistaken.
pub fn list_cultural_operations_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<CulturalOperationDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cultural_operation
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY performed_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any cultural operation hangs off this season — this module's second
/// arm of the guard the shell chains before deleting one. Soft-deleted records
/// count: their audit history is only reachable through the season.
pub(super) fn season_has_operations(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cultural_operation WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- reconciliation --------------------------------------------------------

/// Plots carry no attributes of their own here — model 9.2 prints the plot's
/// own SIGPAC surface from the parcel register, not a worked area — so a plot
/// is either named or it is not, and there is nothing to correct in place.
fn reconcile_plots(
    tx: &Transaction,
    record: &CulturalOperation,
    desired: &[String],
    actor: Option<&str>,
) -> Result<Vec<CulturalOperationPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. Pure
    // children — they live and die with the record.
    for existing in &current {
        if !desired.iter().any(|plot_id| plot_id == &existing.plot_id) {
            tx.execute(
                "DELETE FROM cultural_operation_plot WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "cultural_operation_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&CulturalOperationPlot>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for plot_id in desired {
        match current.iter().find(|c| &c.plot_id == plot_id) {
            Some(existing) => rows.push(existing.clone()),
            None => rows.push(insert_plot_row(
                tx,
                &record.id,
                &record.season_id,
                plot_id,
                actor,
            )?),
        }
    }
    Ok(rows)
}

fn insert_plot_row(
    tx: &Transaction,
    cultural_operation_id: &str,
    season_id: &str,
    plot_id: &str,
    actor: Option<&str>,
) -> Result<CulturalOperationPlot> {
    let row = CulturalOperationPlot {
        id: Uuid::now_v7().to_string(),
        cultural_operation_id: cultural_operation_id.to_string(),
        plot_id: plot_id.to_string(),
    };
    tx.execute(
        "INSERT INTO cultural_operation_plot (id, cultural_operation_id, plot_id)
         VALUES (?1, ?2, ?3)",
        params![row.id, row.cultural_operation_id, row.plot_id],
    )?;
    log_insert(
        tx,
        "cultural_operation_plot",
        &row.id,
        Some(season_id),
        actor,
        &row,
    )?;
    Ok(row)
}

// --- validation ------------------------------------------------------------

/// An operation must not end before it starts. A single-day operation leaves
/// the end NULL rather than repeating the start, because the twin distinguishes
/// the two and a repeated date would claim an interval nobody stated.
fn validate_interval(start: &str, end: Option<&str>) -> Result<()> {
    match end {
        None => Ok(()),
        // ISO 8601 date-only strings compare lexicographically, which is why
        // the whole app stores them this way.
        Some(end) if end >= start => Ok(()),
        Some(_) => Err(EcoschemeError::Invalid("invalid_date_interval")),
    }
}

/// An operation evidences one of five duties (see [`OPERATION_PRACTICES`]).
/// Recorded against `extensive_grazing` it would print on no page of section 9,
/// because art. 30.2 ter's register is the grazing itself.
fn validate_practice(code: &str) -> Result<()> {
    if !OPERATION_PRACTICES.contains(&code) {
        return Err(EcoschemeError::Invalid("practice_not_operation"));
    }
    Ok(())
}

/// Every worked plot must exist and be on this farm. Duplicates are folded —
/// the UNIQUE index would reject them anyway, and a form listing a plot twice
/// means one operation, not an error.
fn validated_plots(tx: &Transaction, farm_id: &str, plot_ids: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for plot_id in plot_ids {
        if !seen.insert(plot_id.clone()) {
            continue;
        }
        let plot_farm: String = tx
            .query_row("SELECT farm_id FROM plot WHERE id = ?1", [plot_id], |r| {
                r.get(0)
            })
            .map_err(no_rows_to_not_found)?;
        if plot_farm != farm_id {
            return Err(EcoschemeError::PlotNotOnFarm {
                plot_id: plot_id.clone(),
                farm_id: farm_id.to_string(),
            });
        }
        kept.push(plot_id.clone());
    }
    if kept.is_empty() {
        return Err(EcoschemeError::Invalid("no_plots"));
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
    records: Vec<CulturalOperation>,
) -> Result<Vec<CulturalOperationDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM cultural_operation_plot WHERE cultural_operation_id IN ({ids}) ORDER BY cultural_operation_id, id",
        &ids,
        map_plot,
        |p| p.cultural_operation_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| CulturalOperationDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            record,
        })
        .collect())
}

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<CulturalOperationPlot>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM cultural_operation_plot WHERE cultural_operation_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<CulturalOperationPlot>> {
    let mut stmt = tx.prepare(
        "SELECT * FROM cultural_operation_plot WHERE cultural_operation_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<CulturalOperation> {
    Ok(CulturalOperation {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        practice_code: row.get("practice_code")?,
        operation_kind_code: row.get("operation_kind_code")?,
        performed_on: row.get("performed_on")?,
        performed_end_date: row.get("performed_end_date")?,
        activity_description: row.get("activity_description")?,
        residue_destination_code: row.get("residue_destination_code")?,
        soil_cover_id: row.get("soil_cover_id")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<CulturalOperationPlot> {
    Ok(CulturalOperationPlot {
        id: row.get("id")?,
        cultural_operation_id: row.get("cultural_operation_id")?,
        plot_id: row.get("plot_id")?,
    })
}
