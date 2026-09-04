// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Models 9.4 and 9.5 — the register of covers.
//!
//! RD 1048/2022 art. 42 governs a live cover of spontaneous or sown vegetation
//! (P6, model 9.4) and art. 43 an inert one of triturated pruning residue (P7,
//! model 9.5). One table for both: the two articles ask for the same three
//! things, the exchange format gives them one block (`DatosCubierta`), and
//! `practice_code` is what separates the two printed pages — exactly as it
//! separates model 9.2 from the book's own "9.6".
//!
//! **Art. 42 is three annotations on three different deadlines**, which the
//! printed model collapses into one row and which this register therefore
//! splits:
//!
//!   * 42.1.a / 43.1.a — the establishment date, within a month. It is
//!     `established_on`, and it is what brings a row into existence.
//!   * 42.1.e / 43.1.b — the two widths, due within the month before a later
//!     period ends, so they are entered afterwards, together, or not at all.
//!   * 42.1.c — the maintenance, on a third date, stored as rows in the
//!     registers those activities already belong to.
//!
//! That last point is the one worth reading twice. A siega is a
//! `cultural_operation` and a pastoreo is a `grazing_record`, whichever land
//! they happen on, and the exchange format agrees — it hangs its cover-activity
//! booleans off `LaboresCulturales`, not off `DatosCubierta`. So this register
//! owns no maintenance table. It writes maintenance through the very functions
//! the 9.2 and 9.1 forms use, in the transaction it already holds, which is
//! what keeps one register from validating what the other does not.
//!
//! **Fully correctable**, like every register in this module.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use super::{cultural_operation, grazing};
use crate::error::{EcoschemeError, Result};
use crate::models::{
    CoverMaintenanceLine, GRAZING_MAINTENANCE, GrazingAnimal, NewCulturalOperation,
    NewGrazingRecord, NewSoilCover, SoilCover, SoilCoverDetail, SoilCoverPlot,
    UpdateCulturalOperation, UpdateGrazingRecord, UpdateSoilCover,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::sql::children_by_parent;
use uuid::Uuid;

/// The two duties a cover can evidence. Nothing else establishes one: P1 is a
/// grazing, P2 a mown plot, P5 a flooded crop, and anexo IV a comunal pasture.
const COVER_PRACTICES: [&str; 2] = ["plant_cover", "inert_cover"];

/// The kinds model 9.4 prints as its three maintenance columns.
///
/// `mowing` and `brush_cutting` are `cultural_operation_kind` codes — they are
/// two of ours over `TIPO_LABOR`'s single "Desbroce y siega" precisely because
/// this page prints them as separate columns. [`GRAZING_MAINTENANCE`] is not a
/// kind at all; it names the third column, which is a grazing record.
const MAINTENANCE_KINDS: [&str; 3] = ["mowing", "brush_cutting", GRAZING_MAINTENANCE];

pub fn insert_soil_cover(
    conn: &mut Connection,
    new: NewSoilCover,
    actor: Option<&str>,
) -> Result<SoilCoverDetail> {
    validate_practice(&new.practice_code)?;
    validate_widths(
        new.width_m,
        new.free_canopy_width_m,
        new.widths_stated_on.as_deref(),
    )?;
    validate_maintenance(&new.practice_code, &new.maintenance)?;

    let tx = conn.transaction()?;
    let plot_ids = validated_plots(&tx, &new.farm_id, &new.plot_ids)?;

    let now = now_utc_iso();
    let record = SoilCover {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        practice_code: new.practice_code,
        cover_type_code: new.cover_type_code,
        established_on: new.established_on,
        width_m: new.width_m,
        free_canopy_width_m: new.free_canopy_width_m,
        widths_stated_on: blank_to_none(new.widths_stated_on),
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO soil_cover (
            id, season_id, farm_id, practice_code, cover_type_code, established_on,
            width_m, free_canopy_width_m, widths_stated_on, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.practice_code,
            record.cover_type_code,
            record.established_on,
            record.width_m,
            record.free_canopy_width_m,
            record.widths_stated_on,
            record.notes,
            record.created_at,
            record.updated_at
        ],
    )?;

    let mut plot_rows = Vec::new();
    for plot_id in &plot_ids {
        plot_rows.push(insert_plot_row(
            &tx,
            &record.id,
            &record.season_id,
            plot_id,
            actor,
        )?);
    }

    log_insert(
        &tx,
        "soil_cover",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;

    // The maintenance lands in the SAME transaction as the cover it maintains,
    // so a book never holds a cover whose third annotation half-saved.
    for line in &new.maintenance {
        insert_maintenance_line(&tx, &record, &plot_ids, line, actor)?;
    }

    let maintenance = maintenance_of_tx(&tx, &record.id)?;
    tx.commit()?;
    Ok(SoilCoverDetail {
        record,
        plots: plot_rows,
        maintenance,
    })
}

/// Full-row correction, with plots and maintenance reconciled from the
/// submitted state.
pub fn update_soil_cover(
    conn: &mut Connection,
    id: &str,
    update: UpdateSoilCover,
    actor: Option<&str>,
) -> Result<SoilCoverDetail> {
    validate_practice(&update.practice_code)?;
    validate_widths(
        update.width_m,
        update.free_canopy_width_m,
        update.widths_stated_on.as_deref(),
    )?;
    validate_maintenance(&update.practice_code, &update.maintenance)?;

    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM soil_cover WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(EcoschemeError::NotFound)?;
    let plot_ids = validated_plots(&tx, &before.farm_id, &update.plot_ids)?;

    let mut after = before.clone();
    after.practice_code = update.practice_code;
    after.cover_type_code = update.cover_type_code;
    after.established_on = update.established_on;
    after.width_m = update.width_m;
    after.free_canopy_width_m = update.free_canopy_width_m;
    after.widths_stated_on = blank_to_none(update.widths_stated_on);
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE soil_cover SET
            practice_code = ?2, cover_type_code = ?3, established_on = ?4,
            width_m = ?5, free_canopy_width_m = ?6, widths_stated_on = ?7,
            notes = ?8, updated_at = ?9
         WHERE id = ?1",
        params![
            id,
            after.practice_code,
            after.cover_type_code,
            after.established_on,
            after.width_m,
            after.free_canopy_width_m,
            after.widths_stated_on,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "soil_cover",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(&tx, &after, &plot_ids, actor)?;
    reconcile_maintenance(&tx, &after, &plot_ids, &update.maintenance, actor)?;

    let maintenance = maintenance_of_tx(&tx, id)?;
    tx.commit()?;
    Ok(SoilCoverDetail {
        record: after,
        plots: plot_rows,
        maintenance,
    })
}

/// Withdrawing a cover withdraws the maintenance recorded against it.
///
/// Those rows are art. 42.1.c's annotation *of this cover* — they exist as its
/// third deadline and print in its columns — so a cover withdrawn as a mistake
/// leaves no siega behind pointing at nothing. Each withdrawal is its own
/// audited soft delete, so nothing is lost: the history of every line survives
/// exactly as it would have if it had been withdrawn on its own.
pub fn soft_delete_soil_cover(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM soil_cover WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(EcoschemeError::NotFound)?;

    for line in maintenance_of_tx(&tx, id)? {
        withdraw_maintenance_line(&tx, &line, actor)?;
    }

    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE soil_cover SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "soil_cover",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_soil_cover(conn: &Connection, id: &str) -> Result<SoilCoverDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM soil_cover WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    let maintenance = maintenance_of(conn, &record.id)?;
    Ok(SoilCoverDetail {
        record,
        plots,
        maintenance,
    })
}

/// Oldest first, the order a record book reads in.
pub fn list_soil_covers(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<SoilCoverDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM soil_cover
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY established_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// One cover by id, WITHDRAWN ONES INCLUDED — the SIEX export.
///
/// A maintenance record (art. 42.1.c) names the cover it maintained, and the
/// export restates that cover's type on every DGC of the entry. Two reasons the
/// ordinary getter cannot serve it: it filters soft-deleted rows, while a
/// deletion entry must still resolve the cover it named; and the link is
/// validated against the farm but **not** against the season, so a caller cannot
/// safely resolve it from a season's own list either.
///
/// Returns the record alone: the plots and maintenance lines of the cover say
/// nothing about the record that points at it.
pub fn get_soil_cover_for_export(conn: &Connection, id: &str) -> Result<SoilCover> {
    conn.query_row("SELECT * FROM soil_cover WHERE id = ?1", [id], map_record)
        .map_err(no_rows_to_not_found)
}

/// Every cover of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
///
/// The maintenance lines come along because the detail struct carries them, and
/// the export reads none of them: each is a `cultural_operation` or a
/// `grazing_record` in its own right and travels as its own `LaboresCulturales`
/// or `Pastoreo` entry, carrying the cover through `Cubiertas` rather than
/// through `DatosCubierta`.
pub fn list_soil_covers_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<SoilCoverDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM soil_cover
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY established_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any cover hangs off this season — the third arm of the module's
/// season guard. Soft-deleted rows count: their audit history is only reachable
/// through the season.
pub(super) fn season_has_covers(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM soil_cover WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- maintenance -----------------------------------------------------------

/// Write one maintenance line through the register that owns it.
///
/// Everything but the kind and the dates is inherited from the cover: the farm,
/// the season, the practice and the plots. That is not a shortcut — it is what
/// makes the sub-form safe. A maintenance annotation is about *this* cover, so
/// a line cannot name a plot the cover was never established over, and cannot
/// claim a different duty from the one it maintains.
fn insert_maintenance_line(
    tx: &Transaction,
    cover: &SoilCover,
    plot_ids: &[String],
    line: &CoverMaintenanceLine,
    actor: Option<&str>,
) -> Result<()> {
    if line.kind_code == GRAZING_MAINTENANCE {
        grazing::insert_grazing_record_tx(
            tx,
            NewGrazingRecord {
                season_id: cover.season_id.clone(),
                farm_id: cover.farm_id.clone(),
                practice_code: cover.practice_code.clone(),
                plot_group_ref: None,
                soil_cover_id: Some(cover.id.clone()),
                started_on: line.performed_on.clone(),
                ended_on: line.performed_end_date.clone(),
                notes: None,
                plot_ids: plot_ids.to_vec(),
                animals: line.animals.clone(),
            },
            actor,
        )?;
    } else {
        cultural_operation::insert_cultural_operation_tx(
            tx,
            NewCulturalOperation {
                season_id: cover.season_id.clone(),
                farm_id: cover.farm_id.clone(),
                practice_code: cover.practice_code.clone(),
                operation_kind_code: line.kind_code.clone(),
                performed_on: line.performed_on.clone(),
                performed_end_date: line.performed_end_date.clone(),
                activity_description: None,
                residue_destination_code: None,
                soil_cover_id: Some(cover.id.clone()),
                notes: None,
                plot_ids: plot_ids.to_vec(),
            },
            actor,
        )?;
    }
    Ok(())
}

/// Correct one line in place, keeping its id and its audit trail — the
/// `reconcile_animals` rule: a correction must read as a correction, never as a
/// withdrawal plus a new record.
fn update_maintenance_line(
    tx: &Transaction,
    cover: &SoilCover,
    plot_ids: &[String],
    line: &CoverMaintenanceLine,
    actor: Option<&str>,
) -> Result<()> {
    if line.kind_code == GRAZING_MAINTENANCE {
        grazing::update_grazing_record_tx(
            tx,
            &line.id,
            UpdateGrazingRecord {
                practice_code: cover.practice_code.clone(),
                plot_group_ref: None,
                soil_cover_id: Some(cover.id.clone()),
                started_on: line.performed_on.clone(),
                ended_on: line.performed_end_date.clone(),
                notes: None,
                plot_ids: plot_ids.to_vec(),
                animals: line.animals.clone(),
            },
            actor,
        )?;
    } else {
        cultural_operation::update_cultural_operation_tx(
            tx,
            &line.id,
            UpdateCulturalOperation {
                practice_code: cover.practice_code.clone(),
                operation_kind_code: line.kind_code.clone(),
                performed_on: line.performed_on.clone(),
                performed_end_date: line.performed_end_date.clone(),
                activity_description: None,
                residue_destination_code: None,
                soil_cover_id: Some(cover.id.clone()),
                notes: None,
                plot_ids: plot_ids.to_vec(),
            },
            actor,
        )?;
    }
    Ok(())
}

fn withdraw_maintenance_line(
    tx: &Transaction,
    line: &CoverMaintenanceLine,
    actor: Option<&str>,
) -> Result<()> {
    if line.kind_code == GRAZING_MAINTENANCE {
        grazing::soft_delete_grazing_record_tx(tx, &line.id, actor)
    } else {
        cultural_operation::soft_delete_cultural_operation_tx(tx, &line.id, actor)
    }
}

/// Reconcile the maintenance from the submitted state: a line carrying an id is
/// corrected, one without is created, one no longer sent is withdrawn.
///
/// A line that *changes* between the two registers — a siega corrected to a
/// pastoreo — is a withdrawal and a new record rather than an in-place edit,
/// because the two live in different tables and no row can move between them.
/// That is the honest audit trail for it: the annotation said one activity and
/// now says another.
fn reconcile_maintenance(
    tx: &Transaction,
    cover: &SoilCover,
    plot_ids: &[String],
    desired: &[CoverMaintenanceLine],
    actor: Option<&str>,
) -> Result<()> {
    let current = maintenance_of_tx(tx, &cover.id)?;

    for existing in &current {
        let kept = desired
            .iter()
            .any(|line| line.id == existing.id && is_grazing(line) == is_grazing(existing));
        if !kept {
            withdraw_maintenance_line(tx, existing, actor)?;
        }
    }

    for line in desired {
        let matched = current
            .iter()
            .any(|existing| existing.id == line.id && is_grazing(existing) == is_grazing(line));
        if matched {
            update_maintenance_line(tx, cover, plot_ids, line, actor)?;
        } else {
            insert_maintenance_line(tx, cover, plot_ids, line, actor)?;
        }
    }
    Ok(())
}

fn is_grazing(line: &CoverMaintenanceLine) -> bool {
    line.kind_code == GRAZING_MAINTENANCE
}

/// The cover's maintenance, gathered from the two registers that hold it and
/// ordered by date whichever table each line came from.
fn maintenance_of(conn: &Connection, cover_id: &str) -> Result<Vec<CoverMaintenanceLine>> {
    let mut lines = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT id, operation_kind_code, performed_on, performed_end_date
         FROM cultural_operation
         WHERE soil_cover_id = ?1 AND deleted_at IS NULL",
    )?;
    for row in stmt.query_map([cover_id], map_operation_line)? {
        lines.push(row?);
    }

    let mut stmt = conn.prepare(
        "SELECT id, started_on, ended_on FROM grazing_record
         WHERE soil_cover_id = ?1 AND deleted_at IS NULL",
    )?;
    let grazings = stmt
        .query_map([cover_id], map_grazing_line)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for mut line in grazings {
        line.animals = animals_of(conn, &line.id)?;
        lines.push(line);
    }

    sort_lines(&mut lines);
    Ok(lines)
}

fn maintenance_of_tx(tx: &Transaction, cover_id: &str) -> Result<Vec<CoverMaintenanceLine>> {
    let mut lines = Vec::new();

    let mut stmt = tx.prepare(
        "SELECT id, operation_kind_code, performed_on, performed_end_date
         FROM cultural_operation
         WHERE soil_cover_id = ?1 AND deleted_at IS NULL",
    )?;
    for row in stmt.query_map([cover_id], map_operation_line)? {
        lines.push(row?);
    }

    let mut stmt = tx.prepare(
        "SELECT id, started_on, ended_on FROM grazing_record
         WHERE soil_cover_id = ?1 AND deleted_at IS NULL",
    )?;
    let grazings = stmt
        .query_map([cover_id], map_grazing_line)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for mut line in grazings {
        line.animals = animals_of_tx(tx, &line.id)?;
        lines.push(line);
    }

    sort_lines(&mut lines);
    Ok(lines)
}

fn sort_lines(lines: &mut [CoverMaintenanceLine]) {
    lines.sort_by(|a, b| {
        a.performed_on
            .cmp(&b.performed_on)
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn animals_of(conn: &Connection, grazing_record_id: &str) -> Result<Vec<GrazingAnimal>> {
    let mut stmt =
        conn.prepare("SELECT * FROM grazing_animal WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([grazing_record_id], map_animal)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn animals_of_tx(tx: &Transaction, grazing_record_id: &str) -> Result<Vec<GrazingAnimal>> {
    let mut stmt =
        tx.prepare("SELECT * FROM grazing_animal WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([grazing_record_id], map_animal)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// --- reconciliation --------------------------------------------------------

/// Plots carry no attributes of their own here: a cover's extent is stated by
/// its two widths, which is what both articles ask for.
fn reconcile_plots(
    tx: &Transaction,
    record: &SoilCover,
    desired: &[String],
    actor: Option<&str>,
) -> Result<Vec<SoilCoverPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    for existing in &current {
        if !desired.iter().any(|plot_id| plot_id == &existing.plot_id) {
            tx.execute("DELETE FROM soil_cover_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "soil_cover_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&SoilCoverPlot>,
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
    soil_cover_id: &str,
    season_id: &str,
    plot_id: &str,
    actor: Option<&str>,
) -> Result<SoilCoverPlot> {
    let row = SoilCoverPlot {
        id: Uuid::now_v7().to_string(),
        soil_cover_id: soil_cover_id.to_string(),
        plot_id: plot_id.to_string(),
    };
    tx.execute(
        "INSERT INTO soil_cover_plot (id, soil_cover_id, plot_id) VALUES (?1, ?2, ?3)",
        params![row.id, row.soil_cover_id, row.plot_id],
    )?;
    log_insert(tx, "soil_cover_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

// --- validation ------------------------------------------------------------

/// A cover evidences one of two duties (see [`COVER_PRACTICES`]).
fn validate_practice(code: &str) -> Result<()> {
    if !COVER_PRACTICES.contains(&code) {
        return Err(EcoschemeError::Invalid("practice_not_cover"));
    }
    Ok(())
}

/// Art. 42.1.e and 43.1.b ask for **both** widths, as one annotation on one
/// deadline. So the three columns move together: two widths and the date they
/// were stated, or none of them.
///
/// The `plot_water_point.distance_m` pairing, and for the same reason — one
/// width without the other is a *wrong* answer rather than a missing one, and a
/// width with no statement date cannot answer the question the deadline asks.
/// A stated width must also be positive: a cover 0 m wide is not a cover.
fn validate_widths(
    width_m: Option<f64>,
    free_canopy_width_m: Option<f64>,
    widths_stated_on: Option<&str>,
) -> Result<()> {
    let stated_on = widths_stated_on.map(str::trim).filter(|d| !d.is_empty());
    let present = [
        width_m.is_some(),
        free_canopy_width_m.is_some(),
        stated_on.is_some(),
    ];
    if present.iter().any(|p| *p) && !present.iter().all(|p| *p) {
        return Err(EcoschemeError::Invalid("incomplete_widths"));
    }
    for width in [width_m, free_canopy_width_m].into_iter().flatten() {
        if width <= 0.0 {
            return Err(EcoschemeError::Invalid("nonpositive_width"));
        }
    }
    Ok(())
}

/// The maintenance lines the cover form may send.
///
/// Two rules, both from the decree rather than from taste. **Art. 43 asks for
/// no maintenance at all** — model 9.5 has no such columns, and an inert cover
/// of triturated pruning residue is not mown — so a line against one would
/// print nowhere. And only the three kinds model 9.4 prints are accepted: a
/// pruning or a rolling is a cultural operation in its own right, recorded on
/// the 9.2 register where it prints.
fn validate_maintenance(practice_code: &str, lines: &[CoverMaintenanceLine]) -> Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    if practice_code == "inert_cover" {
        return Err(EcoschemeError::Invalid("maintenance_on_an_inert_cover"));
    }
    for line in lines {
        if !MAINTENANCE_KINDS.contains(&line.kind_code.as_str()) {
            return Err(EcoschemeError::Invalid("not_a_maintenance_kind"));
        }
        // Animals belong to a grazing and to nothing else. Silently dropping
        // them would let a form lose the head count while reporting success.
        if line.kind_code != GRAZING_MAINTENANCE && !line.animals.is_empty() {
            return Err(EcoschemeError::Invalid("animals_on_a_non_grazing_line"));
        }
    }
    Ok(())
}

/// Every covered plot must exist and be on this farm. Duplicates fold, as
/// everywhere in this module.
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

/// Hydration for a whole list, one child statement for the plots.
///
/// The MAINTENANCE stays per cover, and that is deliberate rather than a
/// remainder: art. 42.1.c's third annotation is assembled from two other
/// registers plus their own children, so hoisting it would mean four more
/// grouped reads to save a handful of statements over a list bounded by the
/// covers a holding establishes in one campaign. The single-record paths keep
/// their point queries for the same reason.
fn all_with_details(conn: &Connection, records: Vec<SoilCover>) -> Result<Vec<SoilCoverDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM soil_cover_plot WHERE soil_cover_id IN ({ids})
         ORDER BY soil_cover_id, id",
        &ids,
        map_plot,
        |p| p.soil_cover_id.clone(),
    )?;
    records
        .into_iter()
        .map(|record| {
            let maintenance = maintenance_of(conn, &record.id)?;
            Ok(SoilCoverDetail {
                plots: plots.remove(&record.id).unwrap_or_default(),
                maintenance,
                record,
            })
        })
        .collect()
}

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<SoilCoverPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM soil_cover_plot WHERE soil_cover_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<SoilCoverPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM soil_cover_plot WHERE soil_cover_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<SoilCover> {
    Ok(SoilCover {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        practice_code: row.get("practice_code")?,
        cover_type_code: row.get("cover_type_code")?,
        established_on: row.get("established_on")?,
        width_m: row.get("width_m")?,
        free_canopy_width_m: row.get("free_canopy_width_m")?,
        widths_stated_on: row.get("widths_stated_on")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<SoilCoverPlot> {
    Ok(SoilCoverPlot {
        id: row.get("id")?,
        soil_cover_id: row.get("soil_cover_id")?,
        plot_id: row.get("plot_id")?,
    })
}

fn map_operation_line(row: &Row<'_>) -> rusqlite::Result<CoverMaintenanceLine> {
    Ok(CoverMaintenanceLine {
        id: row.get("id")?,
        kind_code: row.get("operation_kind_code")?,
        performed_on: row.get("performed_on")?,
        performed_end_date: row.get("performed_end_date")?,
        animals: Vec::new(),
    })
}

fn map_grazing_line(row: &Row<'_>) -> rusqlite::Result<CoverMaintenanceLine> {
    Ok(CoverMaintenanceLine {
        id: row.get("id")?,
        kind_code: GRAZING_MAINTENANCE.to_string(),
        performed_on: row.get("started_on")?,
        performed_end_date: row.get("ended_on")?,
        animals: Vec::new(),
    })
}

fn map_animal(row: &Row<'_>) -> rusqlite::Result<GrazingAnimal> {
    Ok(GrazingAnimal {
        id: row.get("id")?,
        grazing_record_id: row.get("grazing_record_id")?,
        species_code: row.get("species_code")?,
        rega_code: row.get("rega_code")?,
        animal_count: row.get("animal_count")?,
    })
}
