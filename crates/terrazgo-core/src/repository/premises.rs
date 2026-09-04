// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The premises registry: the places and vehicles models 3.4 and 3.5 treat.
//!
//! Soft-delete only, like every other registry entity — a treatment that named
//! a store must keep resolving it, and the audit history has to survive.
//!
//! This layer knows nothing about the register that uses it. Which kind of
//! premises a `non_field_treatment` may name is module-cue's rule, because only
//! that crate holds the register's own vocabulary; core owns the thing, not the
//! use.
//!
//! The Spanish registry identifiers (the cadastral reference and the REA
//! installation code) live in an extension row reconciled from the submitted
//! state, hard-deleted when both are removed — the machinery/farm/plot
//! contract.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewPremises, Premises, PremisesDetail, PremisesEsExtension, UpdatePremises};
use crate::sql::children_by_parent;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

pub fn insert_premises(
    conn: &mut Connection,
    new: NewPremises,
    actor: Option<&str>,
) -> Result<PremisesDetail> {
    validate_name(&new.name)?;
    validate_volume(new.volume_m3)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let premises = Premises {
        id: Uuid::now_v7().to_string(),
        farm_id: new.farm_id,
        kind_code: new.kind_code,
        name: new.name.trim().to_string(),
        address: blank_to_none(new.address),
        vehicle_model: blank_to_none(new.vehicle_model),
        plate: blank_to_none(new.plate),
        class_code: blank_to_none(new.class_code),
        volume_m3: new.volume_m3,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO premises
           (id, farm_id, kind_code, name, address, vehicle_model, plate,
            class_code, volume_m3, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            premises.id,
            premises.farm_id,
            premises.kind_code,
            premises.name,
            premises.address,
            premises.vehicle_model,
            premises.plate,
            premises.class_code,
            premises.volume_m3,
            premises.notes,
            premises.created_at,
            premises.updated_at
        ],
    )?;
    log_insert(&tx, "premises", &premises.id, None, actor, &premises)?;

    let cadastral_reference = normalise_reference(new.cadastral_reference);
    let rea_installation_code = blank_to_none(new.rea_installation_code);
    let es = if cadastral_reference.is_some() || rea_installation_code.is_some() {
        Some(insert_extension(
            &tx,
            &premises.id,
            cadastral_reference,
            rea_installation_code,
            actor,
        )?)
    } else {
        None
    };

    tx.commit()?;
    Ok(PremisesDetail { premises, es })
}

/// Full-row correction. Carries no `farm_id`: re-homing a premises would take
/// every treatment that names it to another holding, the `plot.farm_id`
/// precedent.
pub fn update_premises(
    conn: &mut Connection,
    id: &str,
    update: UpdatePremises,
    actor: Option<&str>,
) -> Result<PremisesDetail> {
    validate_name(&update.name)?;
    validate_volume(update.volume_m3)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM premises WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_premises,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.kind_code = update.kind_code;
    after.name = update.name.trim().to_string();
    after.address = blank_to_none(update.address);
    after.vehicle_model = blank_to_none(update.vehicle_model);
    after.plate = blank_to_none(update.plate);
    after.class_code = blank_to_none(update.class_code);
    after.volume_m3 = update.volume_m3;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE premises SET
            kind_code = ?2, name = ?3, address = ?4, vehicle_model = ?5,
            plate = ?6, class_code = ?7, volume_m3 = ?8, notes = ?9,
            updated_at = ?10
         WHERE id = ?1",
        params![
            id,
            after.kind_code,
            after.name,
            after.address,
            after.vehicle_model,
            after.plate,
            after.class_code,
            after.volume_m3,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(&tx, "premises", id, None, actor, &before, &after)?;

    let es = reconcile_extension(
        &tx,
        id,
        normalise_reference(update.cadastral_reference),
        blank_to_none(update.rea_installation_code),
        actor,
    )?;
    tx.commit()?;
    Ok(PremisesDetail {
        premises: after,
        es,
    })
}

pub fn soft_delete_premises(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM premises WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_premises,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE premises SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "premises", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

/// The live premises of one holding, oldest first.
pub fn list_premises(conn: &Connection, farm_id: &str) -> Result<Vec<Premises>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM premises
         WHERE farm_id = ?1 AND deleted_at IS NULL
         ORDER BY created_at, id",
    )?;
    let rows = stmt
        .query_map([farm_id], map_premises)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The live premises of one holding with their Spanish extensions — the
/// registry list and edit form. [`list_premises`] stays extension-less for the
/// register's own picker, which needs only kind, name and the printed parts.
pub fn list_premises_details(conn: &Connection, farm_id: &str) -> Result<Vec<PremisesDetail>> {
    let rows = list_premises(conn, farm_id)?;
    let ids: Vec<String> = rows.iter().map(|p| p.id.clone()).collect();
    let mut extensions = children_by_parent(
        conn,
        "SELECT premises_id, cadastral_reference, rea_installation_code
         FROM premises_es_extension WHERE premises_id IN ({ids})
         ORDER BY premises_id",
        &ids,
        map_extension,
        |es| es.premises_id.clone(),
    )?;
    Ok(rows
        .into_iter()
        .map(|premises| PremisesDetail {
            es: extensions
                .remove(&premises.id)
                .and_then(|rows| rows.into_iter().next()),
            premises,
        })
        .collect())
}

/// One premises by id, soft-deleted included. The register resolves a named
/// row to compose its printed description, and a record must keep resolving
/// the store it named even after the farmer retires it from the picker.
pub fn get_premises(conn: &Connection, id: &str) -> Result<Premises> {
    conn.query_row("SELECT * FROM premises WHERE id = ?1", [id], map_premises)
        .optional()?
        .ok_or(CoreError::NotFound)
}

/// One premises with its extension, soft-deleted included — what the SIEX
/// export reads to fill `Edificaciones[].IdEdificacion`.
pub fn get_premises_detail(conn: &Connection, id: &str) -> Result<PremisesDetail> {
    let premises = get_premises(conn, id)?;
    let es = get_extension(conn, &premises.id)?;
    Ok(PremisesDetail { premises, es })
}

// ---------------------------------------------------------------------------
// Extension plumbing (mirrors the machinery/farm/plot extension helpers)
// ---------------------------------------------------------------------------

fn insert_extension(
    tx: &Transaction,
    premises_id: &str,
    cadastral_reference: Option<String>,
    rea_installation_code: Option<String>,
    actor: Option<&str>,
) -> Result<PremisesEsExtension> {
    let ext = PremisesEsExtension {
        premises_id: premises_id.to_string(),
        cadastral_reference,
        rea_installation_code,
    };
    tx.execute(
        "INSERT INTO premises_es_extension
           (premises_id, cadastral_reference, rea_installation_code)
         VALUES (?1, ?2, ?3)",
        params![
            ext.premises_id,
            ext.cadastral_reference,
            ext.rea_installation_code
        ],
    )?;
    log_insert(tx, "premises_es_extension", premises_id, None, actor, &ext)?;
    Ok(ext)
}

fn get_extension(conn: &Connection, premises_id: &str) -> Result<Option<PremisesEsExtension>> {
    Ok(conn
        .query_row(
            "SELECT premises_id, cadastral_reference, rea_installation_code
             FROM premises_es_extension WHERE premises_id = ?1",
            [premises_id],
            map_extension,
        )
        .optional()?)
}

/// Bring the extension row in line with the submitted state, logging the
/// transition (insert / update / hard delete with a null after-image). The row
/// exists while at least one registry identifier is present.
fn reconcile_extension(
    tx: &Transaction,
    premises_id: &str,
    cadastral_reference: Option<String>,
    rea_installation_code: Option<String>,
    actor: Option<&str>,
) -> Result<Option<PremisesEsExtension>> {
    let current = tx
        .query_row(
            "SELECT premises_id, cadastral_reference, rea_installation_code
             FROM premises_es_extension WHERE premises_id = ?1",
            [premises_id],
            map_extension,
        )
        .optional()?;
    let wanted = cadastral_reference.is_some() || rea_installation_code.is_some();
    match (current, wanted) {
        (None, false) => Ok(None),
        (None, true) => Ok(Some(insert_extension(
            tx,
            premises_id,
            cadastral_reference,
            rea_installation_code,
            actor,
        )?)),
        (Some(before), false) => {
            tx.execute(
                "DELETE FROM premises_es_extension WHERE premises_id = ?1",
                [premises_id],
            )?;
            log_delete(
                tx,
                "premises_es_extension",
                premises_id,
                None,
                actor,
                &before,
                None,
            )?;
            Ok(None)
        }
        (Some(before), true) => {
            let after = PremisesEsExtension {
                premises_id: premises_id.to_string(),
                cadastral_reference,
                rea_installation_code,
            };
            tx.execute(
                "UPDATE premises_es_extension
                 SET cadastral_reference = ?2, rea_installation_code = ?3
                 WHERE premises_id = ?1",
                params![
                    premises_id,
                    after.cadastral_reference,
                    after.rea_installation_code
                ],
            )?;
            log_update(
                tx,
                "premises_es_extension",
                premises_id,
                None,
                actor,
                &before,
                &after,
            )?;
            Ok(Some(after))
        }
    }
}

fn map_extension(row: &Row<'_>) -> rusqlite::Result<PremisesEsExtension> {
    Ok(PremisesEsExtension {
        premises_id: row.get("premises_id")?,
        cadastral_reference: row.get("cadastral_reference")?,
        rea_installation_code: row.get("rea_installation_code")?,
    })
}

/// A stated capacity has to be a real one: zero or negative is a typo, not a
/// building. Absent stays fine — no decree asks for the capacity.
fn validate_volume(volume_m3: Option<f64>) -> Result<()> {
    match volume_m3 {
        Some(v) if v <= 0.0 => Err(CoreError::Invalid("nonpositive_volume")),
        _ => Ok(()),
    }
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
}

/// A cadastral reference is a canonical upper-case code, so it is stored as
/// one: two spellings of the same reference would defeat the identification
/// this registry exists to give (Anexo III Parte I B.b). Its SHAPE is not
/// checked — the precedent for an external registry identifier is
/// `roma_number`, `rea_code` and `licence_number`, none of which is
/// pattern-validated, and refusing a reference the app merely fails to
/// recognise would block a lawful registry row.
fn normalise_reference(value: Option<String>) -> Option<String> {
    blank_to_none(value).map(|v| v.to_uppercase())
}

fn map_premises(row: &Row<'_>) -> rusqlite::Result<Premises> {
    Ok(Premises {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        kind_code: row.get("kind_code")?,
        name: row.get("name")?,
        address: row.get("address")?,
        vehicle_model: row.get("vehicle_model")?,
        plate: row.get("plate")?,
        class_code: row.get("class_code")?,
        volume_m3: row.get("volume_m3")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
