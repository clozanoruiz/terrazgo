// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model 9.1 — the extensive-grazing register.
//!
//! RD 1048/2022 art. 30.2 ter obliges an annotation when the grazing dates
//! differ from those declared in the solicitud única, within one month of the
//! new date — and the printed model counts that month from the END of grazing.
//! SIEX twin: `Pastoreo`.
//!
//! **Fully correctable from the start**, like every register built since
//! `seed_treatment`: this table holds no snapshot of another row's identity, so
//! there is nothing a later edit elsewhere could rewrite underneath it.
//!
//! As in the cultural-operation register, each write comes in two forms: a
//! `pub fn` opening its own transaction and a `pub(super) fn …_tx` joining one
//! already open, which is how the cover register writes model 9.4's Pastoreo
//! column through this code instead of a copy of it.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::{no_rows_to_not_found, validated_cover_link};
use crate::error::{EcoschemeError, Result};
use crate::models::{
    GrazingAnimal, GrazingPlot, GrazingRecord, GrazingRecordDetail, NewGrazingRecord,
    UpdateGrazingRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::sql::children_by_parent;
use uuid::Uuid;

/// The duties a grazing can evidence.
///
/// P1 is the obvious one (art. 30.2 ter). P2 is here because art. 31 lists
/// *pastoreo* among the maintenance activities whose date must be annotated,
/// and anexo IV because a comunal pasture's maintenance is grazed as much as
/// mown. **P6 joined them with the cover register**: art. 42.1.c counts
/// pastoreo as one of three ways a live cover is maintained, and model 9.4
/// prints it as a column — so a grazing over a cover is a `plant_cover`
/// grazing carrying a `soil_cover_id`, not a P1 one.
///
/// `inert_cover` and `flooded_biodiversity` stay out: art. 43 asks for no
/// maintenance at all, and a flooded crop is not grazed.
const GRAZING_PRACTICES: [&str; 4] = [
    "extensive_grazing",
    "sustainable_mowing",
    "communal_pasture",
    "plant_cover",
];

pub fn insert_grazing_record(
    conn: &mut Connection,
    new: NewGrazingRecord,
    actor: Option<&str>,
) -> Result<GrazingRecordDetail> {
    let tx = conn.transaction()?;
    let detail = insert_grazing_record_tx(&tx, new, actor)?;
    tx.commit()?;
    Ok(detail)
}

/// The insert itself, inside a transaction the caller owns.
pub(super) fn insert_grazing_record_tx(
    tx: &Transaction,
    new: NewGrazingRecord,
    actor: Option<&str>,
) -> Result<GrazingRecordDetail> {
    validate_interval(&new.started_on, new.ended_on.as_deref())?;
    validate_practice(&new.practice_code)?;

    let plot_ids = validated_plots(tx, &new.farm_id, &new.plot_ids)?;
    let animals = validated_animals(&new.animals)?;
    let soil_cover_id = validated_cover_link(
        tx,
        new.soil_cover_id.as_deref(),
        &new.farm_id,
        &new.practice_code,
    )?;

    let now = now_utc_iso();
    let record = GrazingRecord {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        practice_code: new.practice_code,
        plot_group_ref: blank_to_none(new.plot_group_ref),
        soil_cover_id,
        started_on: new.started_on,
        ended_on: new.ended_on,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO grazing_record (
            id, season_id, farm_id, practice_code, plot_group_ref, soil_cover_id,
            started_on, ended_on, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            record.id,
            record.season_id,
            record.farm_id,
            record.practice_code,
            record.plot_group_ref,
            record.soil_cover_id,
            record.started_on,
            record.ended_on,
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
    let mut animal_rows = Vec::new();
    for animal in animals {
        animal_rows.push(insert_animal_row(
            tx,
            &record.id,
            &record.season_id,
            animal,
            actor,
        )?);
    }

    log_insert(
        tx,
        "grazing_record",
        &record.id,
        Some(&record.season_id),
        actor,
        &record,
    )?;
    Ok(GrazingRecordDetail {
        record,
        plots: plot_rows,
        animals: animal_rows,
    })
}

/// Full-row correction, with plots and animals reconciled from the submitted
/// state: rows that stayed are updated in place (so the audit trail reads as a
/// correction, not a replacement), rows that went are removed, new ones are
/// inserted — each logged on its own.
pub fn update_grazing_record(
    conn: &mut Connection,
    id: &str,
    update: UpdateGrazingRecord,
    actor: Option<&str>,
) -> Result<GrazingRecordDetail> {
    let tx = conn.transaction()?;
    let detail = update_grazing_record_tx(&tx, id, update, actor)?;
    tx.commit()?;
    Ok(detail)
}

/// The correction itself, inside a transaction the caller owns.
pub(super) fn update_grazing_record_tx(
    tx: &Transaction,
    id: &str,
    update: UpdateGrazingRecord,
    actor: Option<&str>,
) -> Result<GrazingRecordDetail> {
    validate_interval(&update.started_on, update.ended_on.as_deref())?;
    validate_practice(&update.practice_code)?;

    let before = tx
        .query_row(
            "SELECT * FROM grazing_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .optional()?
        .ok_or(EcoschemeError::NotFound)?;
    let plot_ids = validated_plots(tx, &before.farm_id, &update.plot_ids)?;
    let animals = validated_animals(&update.animals)?;
    let soil_cover_id = validated_cover_link(
        tx,
        update.soil_cover_id.as_deref(),
        &before.farm_id,
        &update.practice_code,
    )?;

    let mut after = before.clone();
    after.practice_code = update.practice_code;
    after.plot_group_ref = blank_to_none(update.plot_group_ref);
    after.soil_cover_id = soil_cover_id;
    after.started_on = update.started_on;
    after.ended_on = update.ended_on;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE grazing_record SET
            practice_code = ?2, plot_group_ref = ?3, soil_cover_id = ?4,
            started_on = ?5, ended_on = ?6, notes = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            after.practice_code,
            after.plot_group_ref,
            after.soil_cover_id,
            after.started_on,
            after.ended_on,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        tx,
        "grazing_record",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;

    let plot_rows = reconcile_plots(tx, &after, &plot_ids, actor)?;
    let animal_rows = reconcile_animals(tx, &after, animals, actor)?;
    Ok(GrazingRecordDetail {
        record: after,
        plots: plot_rows,
        animals: animal_rows,
    })
}

pub fn soft_delete_grazing_record(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    soft_delete_grazing_record_tx(&tx, id, actor)?;
    tx.commit()?;
    Ok(())
}

/// The withdrawal itself, inside a transaction the caller owns.
pub(super) fn soft_delete_grazing_record_tx(
    tx: &Transaction,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let before = tx
        .query_row(
            "SELECT * FROM grazing_record WHERE id = ?1 AND deleted_at IS NULL",
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
        "UPDATE grazing_record SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        tx,
        "grazing_record",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    Ok(())
}

pub fn get_grazing_record(conn: &Connection, id: &str) -> Result<GrazingRecordDetail> {
    let record = conn
        .query_row(
            "SELECT * FROM grazing_record WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_record,
        )
        .map_err(no_rows_to_not_found)?;
    let plots = plots_of(conn, &record.id)?;
    let animals = animals_of(conn, &record.id)?;
    Ok(GrazingRecordDetail {
        record,
        plots,
        animals,
    })
}

/// Oldest first, the order a record book reads in.
pub fn list_grazing_records(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<GrazingRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM grazing_record
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY started_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Every grazing of this farm+season INCLUDING the soft-deleted ones — the SIEX
/// export, which turns a withdrawn record into a `Borrar` entry under the alias
/// it was first exported with. Its name is the guard: a caller that is not
/// building an export and wants deleted rows is almost certainly mistaken.
pub fn list_grazing_records_for_export(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<GrazingRecordDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM grazing_record
         WHERE season_id = ?1 AND farm_id = ?2
         ORDER BY started_on, id",
    )?;
    let records = stmt
        .query_map(params![season_id, farm_id], map_record)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    all_with_details(conn, records)
}

/// Whether any grazing hangs off this season — this module's arm of the guard
/// the shell chains before deleting one. Soft-deleted records count, as they do
/// in the other modules: their audit history is only reachable through the
/// season they belong to.
pub(super) fn season_has_grazing(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM grazing_record WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- reconciliation --------------------------------------------------------

/// Plots carry no attributes of their own here — unlike an irrigation's, a
/// grazing plot has no surface column, because the model asks for the parcel
/// reference and not for a grazed area. So a plot is either named or it is not,
/// and there is nothing to correct in place.
fn reconcile_plots(
    tx: &Transaction,
    record: &GrazingRecord,
    desired: &[String],
    actor: Option<&str>,
) -> Result<Vec<GrazingPlot>> {
    let current = plots_of_tx(tx, &record.id)?;

    // Gone: hard-deleted with a null after-image, like an extension row. These
    // are pure children — they live and die with the record, and soft-deleting
    // them would leave the register printing plots nobody grazed.
    for existing in &current {
        if !desired.iter().any(|plot_id| plot_id == &existing.plot_id) {
            tx.execute("DELETE FROM grazing_plot WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "grazing_plot",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&GrazingPlot>,
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

/// Animals ARE corrected in place, because a line carries a head count that a
/// farmer can get wrong.
///
/// The identity is (rega, species) — the pair the UNIQUE index uses — so
/// correcting "40 sheep" to "45 sheep" updates one row and reads as a
/// correction, while changing the species is a different line: a different
/// animal grazed, not a mistyped number. **`animal_count` must be part of the
/// equality test below**; a field left out of one is silently discarded while
/// the command reports success (the 2026-08-12 `reconcile_plots` trap).
fn reconcile_animals(
    tx: &Transaction,
    record: &GrazingRecord,
    desired: Vec<GrazingAnimal>,
    actor: Option<&str>,
) -> Result<Vec<GrazingAnimal>> {
    let current = animals_of_tx(tx, &record.id)?;

    for existing in &current {
        if !desired
            .iter()
            .any(|d| d.rega_code == existing.rega_code && d.species_code == existing.species_code)
        {
            tx.execute("DELETE FROM grazing_animal WHERE id = ?1", [&existing.id])?;
            log_delete(
                tx,
                "grazing_animal",
                &existing.id,
                Some(&record.season_id),
                actor,
                existing,
                None::<&GrazingAnimal>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current
            .iter()
            .find(|c| c.rega_code == want.rega_code && c.species_code == want.species_code)
        {
            Some(existing) => {
                if existing.animal_count != want.animal_count {
                    let mut after = existing.clone();
                    after.animal_count = want.animal_count;
                    tx.execute(
                        "UPDATE grazing_animal SET animal_count = ?2 WHERE id = ?1",
                        params![existing.id, after.animal_count],
                    )?;
                    log_update(
                        tx,
                        "grazing_animal",
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
            None => rows.push(insert_animal_row(
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
    grazing_record_id: &str,
    season_id: &str,
    plot_id: &str,
    actor: Option<&str>,
) -> Result<GrazingPlot> {
    let row = GrazingPlot {
        id: Uuid::now_v7().to_string(),
        grazing_record_id: grazing_record_id.to_string(),
        plot_id: plot_id.to_string(),
    };
    tx.execute(
        "INSERT INTO grazing_plot (id, grazing_record_id, plot_id) VALUES (?1, ?2, ?3)",
        params![row.id, row.grazing_record_id, row.plot_id],
    )?;
    log_insert(tx, "grazing_plot", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

fn insert_animal_row(
    tx: &Transaction,
    grazing_record_id: &str,
    season_id: &str,
    animal: GrazingAnimal,
    actor: Option<&str>,
) -> Result<GrazingAnimal> {
    let row = GrazingAnimal {
        id: Uuid::now_v7().to_string(),
        grazing_record_id: grazing_record_id.to_string(),
        species_code: animal.species_code,
        rega_code: animal.rega_code,
        animal_count: animal.animal_count,
    };
    tx.execute(
        "INSERT INTO grazing_animal (
            id, grazing_record_id, species_code, rega_code, animal_count
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.id,
            row.grazing_record_id,
            row.species_code,
            row.rega_code,
            row.animal_count
        ],
    )?;
    log_insert(tx, "grazing_animal", &row.id, Some(season_id), actor, &row)?;
    Ok(row)
}

// --- validation ------------------------------------------------------------

/// A grazing must not end before it starts. An open grazing leaves the end
/// NULL rather than repeating the start, because the annotation deadline runs
/// from the end and "still grazing" is a different statement from "grazed for
/// one day".
fn validate_interval(start: &str, end: Option<&str>) -> Result<()> {
    match end {
        None => Ok(()),
        // ISO 8601 date-only strings compare lexicographically, which is why
        // the whole app stores them this way.
        Some(end) if end >= start => Ok(()),
        Some(_) => Err(EcoschemeError::Invalid("invalid_date_interval")),
    }
}

/// A grazing evidences one of three duties (see [`GRAZING_PRACTICES`]).
/// Recorded against a cover practice it would be a different register, and the
/// printed page it lands on depends on this code.
fn validate_practice(code: &str) -> Result<()> {
    if !GRAZING_PRACTICES.contains(&code) {
        return Err(EcoschemeError::Invalid("practice_not_grazing"));
    }
    Ok(())
}

/// Every grazed plot must exist and be on this farm. Duplicates are folded —
/// the UNIQUE index would reject them anyway, and a form listing a plot twice
/// means one grazing, not an error.
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

/// At least one animal line, each naming a species, a REGA and a positive head
/// count.
///
/// The species code is NOT validated against `ESPECIE_ANIMAL`: it is a
/// provider catalogue that grows between releases, and the two-tier rule holds
/// a closed list the decree enumerates to validation while leaving a
/// commercial registry to the picker. The REGA is likewise unvalidated — its
/// format is the livestock registry's business, and a third party's code is a
/// claim this farm cannot check.
fn validated_animals(animals: &[GrazingAnimal]) -> Result<Vec<GrazingAnimal>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for animal in animals {
        let species_code = animal.species_code.trim();
        let rega_code = animal.rega_code.trim();
        if species_code.is_empty() || rega_code.is_empty() {
            return Err(EcoschemeError::Invalid("incomplete_animal_line"));
        }
        if animal.animal_count <= 0 {
            return Err(EcoschemeError::Invalid("nonpositive_animal_count"));
        }
        // The pair the UNIQUE index keys on. A repeated pair is a form that
        // listed the same animals twice, which folds rather than errors — but
        // the LAST count wins, because that is what the farmer typed most
        // recently.
        if !seen.insert((rega_code.to_string(), species_code.to_string())) {
            if let Some(existing) = kept.iter_mut().find(|k: &&mut GrazingAnimal| {
                k.rega_code == rega_code && k.species_code == species_code
            }) {
                existing.animal_count = animal.animal_count;
            }
            continue;
        }
        kept.push(GrazingAnimal {
            id: String::new(),
            grazing_record_id: String::new(),
            species_code: species_code.to_string(),
            rega_code: rega_code.to_string(),
            animal_count: animal.animal_count,
        });
    }
    if kept.is_empty() {
        return Err(EcoschemeError::Invalid("no_animals"));
    }
    Ok(kept)
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

/// Hydration for a whole list, in two child statements rather than two per
/// record. The single-record paths keep their point queries.
fn all_with_details(
    conn: &Connection,
    records: Vec<GrazingRecord>,
) -> Result<Vec<GrazingRecordDetail>> {
    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let mut plots = children_by_parent(
        conn,
        "SELECT * FROM grazing_plot WHERE grazing_record_id IN ({ids})
         ORDER BY grazing_record_id, id",
        &ids,
        map_plot,
        |p| p.grazing_record_id.clone(),
    )?;
    let mut animals = children_by_parent(
        conn,
        "SELECT * FROM grazing_animal WHERE grazing_record_id IN ({ids})
         ORDER BY grazing_record_id, id",
        &ids,
        map_animal,
        |a| a.grazing_record_id.clone(),
    )?;
    Ok(records
        .into_iter()
        .map(|record| GrazingRecordDetail {
            plots: plots.remove(&record.id).unwrap_or_default(),
            animals: animals.remove(&record.id).unwrap_or_default(),
            record,
        })
        .collect())
}

fn plots_of(conn: &Connection, record_id: &str) -> Result<Vec<GrazingPlot>> {
    let mut stmt =
        conn.prepare("SELECT * FROM grazing_plot WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn plots_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<GrazingPlot>> {
    let mut stmt =
        tx.prepare("SELECT * FROM grazing_plot WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn animals_of(conn: &Connection, record_id: &str) -> Result<Vec<GrazingAnimal>> {
    let mut stmt =
        conn.prepare("SELECT * FROM grazing_animal WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_animal)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn animals_of_tx(tx: &Transaction, record_id: &str) -> Result<Vec<GrazingAnimal>> {
    let mut stmt =
        tx.prepare("SELECT * FROM grazing_animal WHERE grazing_record_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([record_id], map_animal)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_record(row: &Row<'_>) -> rusqlite::Result<GrazingRecord> {
    Ok(GrazingRecord {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        practice_code: row.get("practice_code")?,
        plot_group_ref: row.get("plot_group_ref")?,
        soil_cover_id: row.get("soil_cover_id")?,
        started_on: row.get("started_on")?,
        ended_on: row.get("ended_on")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row<'_>) -> rusqlite::Result<GrazingPlot> {
    Ok(GrazingPlot {
        id: row.get("id")?,
        grazing_record_id: row.get("grazing_record_id")?,
        plot_id: row.get("plot_id")?,
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
