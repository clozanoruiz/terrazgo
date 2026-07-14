// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Machinery CRUD, including the Spanish registry extension row (ROMA for
//! mobile machinery, REGANIP for aircraft and fixed installations). Soft-delete
//! only, like farm/plot/operator: treatment history must keep resolving. The
//! extension row is the exception — reconciled from the submitted state and
//! hard-deleted when both registry numbers are removed (same contract as the
//! farm/plot extensions).

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{
    Machinery, MachineryDetail, MachineryEsExtension, NewMachinery, UpdateMachinery,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// Insert machinery, writing the ROMA/REGANIP numbers to the Spanish extension when
/// either is present. The core row and the extension row are separate synced tables,
/// so each gets its own `record_change` entry.
pub fn insert_machinery(conn: &mut Connection, new: NewMachinery) -> Result<Machinery> {
    validate_name(&new.name)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let machinery = Machinery {
        id: Uuid::now_v7().to_string(),
        farm_id: new.farm_id,
        name: new.name,
        kind: new.kind,
        last_inspection_date: new.last_inspection_date,
        next_inspection_due_date: new.next_inspection_due_date,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO machinery
           (id, farm_id, name, type, last_inspection_date, next_inspection_due_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            machinery.id, machinery.farm_id, machinery.name, machinery.kind,
            machinery.last_inspection_date, machinery.next_inspection_due_date,
            machinery.created_at, machinery.updated_at
        ],
    )?;
    log_insert(&tx, "machinery", &machinery.id, None, &machinery)?;

    if new.roma_number.is_some() || new.reganip_number.is_some() {
        insert_extension(&tx, &machinery.id, new.roma_number, new.reganip_number)?;
    }

    tx.commit()?;
    Ok(machinery)
}

/// Active machinery on one farm, for the treatment form's machinery selector.
pub fn list_machinery(conn: &Connection, farm_id: &str) -> Result<Vec<Machinery>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM machinery WHERE farm_id = ?1 AND deleted_at IS NULL ORDER BY name, id",
    )?;
    let machinery = stmt
        .query_map([farm_id], map_machinery)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(machinery)
}

/// Active machinery of one farm with its Spanish extension — the registry list
/// and edit form. `list_machinery` stays extension-less for the treatment form.
pub fn list_machinery_details(conn: &Connection, farm_id: &str) -> Result<Vec<MachineryDetail>> {
    let machinery = list_machinery(conn, farm_id)?;
    machinery
        .into_iter()
        .map(|machinery| {
            let es = get_extension(conn, &machinery.id)?;
            Ok(MachineryDetail { machinery, es })
        })
        .collect()
}

/// Full-row update; `farm_id` is immutable by design (see `UpdateMachinery`).
/// The extension row is reconciled from the submitted registry numbers.
pub fn update_machinery(
    conn: &mut Connection,
    id: &str,
    update: UpdateMachinery,
) -> Result<MachineryDetail> {
    validate_name(&update.name)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM machinery WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_machinery,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.name = update.name;
    after.kind = update.kind;
    after.last_inspection_date = update.last_inspection_date;
    after.next_inspection_due_date = update.next_inspection_due_date;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE machinery SET name = ?2, type = ?3, last_inspection_date = ?4,
                              next_inspection_due_date = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            after.name,
            after.kind,
            after.last_inspection_date,
            after.next_inspection_due_date,
            after.updated_at
        ],
    )?;
    log_update(&tx, "machinery", id, None, &before, &after)?;

    let es = reconcile_extension(&tx, id, update.roma_number, update.reganip_number)?;
    tx.commit()?;
    Ok(MachineryDetail {
        machinery: after,
        es,
    })
}

/// Soft delete: the row stays (treatment history must keep resolving), it just
/// leaves every list and picker. The ITV alert lapses on the next
/// `refresh_alerts` — the reconciler skips soft-deleted subjects.
pub fn soft_delete_machinery(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM machinery WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_machinery,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE machinery SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "machinery", id, None, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Extension plumbing (mirrors the farm/plot extension helpers)
// ---------------------------------------------------------------------------

fn insert_extension(
    tx: &Transaction,
    machinery_id: &str,
    roma_number: Option<String>,
    reganip_number: Option<String>,
) -> Result<MachineryEsExtension> {
    let ext = MachineryEsExtension {
        machinery_id: machinery_id.to_string(),
        roma_number,
        reganip_number,
    };
    tx.execute(
        "INSERT INTO machinery_es_extension (machinery_id, roma_number, reganip_number)
         VALUES (?1, ?2, ?3)",
        params![ext.machinery_id, ext.roma_number, ext.reganip_number],
    )?;
    log_insert(tx, "machinery_es_extension", machinery_id, None, &ext)?;
    Ok(ext)
}

fn get_extension(conn: &Connection, machinery_id: &str) -> Result<Option<MachineryEsExtension>> {
    Ok(conn
        .query_row(
            "SELECT machinery_id, roma_number, reganip_number
             FROM machinery_es_extension WHERE machinery_id = ?1",
            [machinery_id],
            map_extension,
        )
        .optional()?)
}

/// Bring the extension row in line with the submitted state, logging the
/// transition (insert / update / hard delete with null after-image). The row
/// exists while at least one registry number is present.
fn reconcile_extension(
    tx: &Transaction,
    machinery_id: &str,
    roma_number: Option<String>,
    reganip_number: Option<String>,
) -> Result<Option<MachineryEsExtension>> {
    let current = tx
        .query_row(
            "SELECT machinery_id, roma_number, reganip_number
             FROM machinery_es_extension WHERE machinery_id = ?1",
            [machinery_id],
            map_extension,
        )
        .optional()?;
    let wanted = roma_number.is_some() || reganip_number.is_some();
    match (current, wanted) {
        (None, false) => Ok(None),
        (None, true) => Ok(Some(insert_extension(
            tx,
            machinery_id,
            roma_number,
            reganip_number,
        )?)),
        (Some(before), false) => {
            tx.execute(
                "DELETE FROM machinery_es_extension WHERE machinery_id = ?1",
                [machinery_id],
            )?;
            log_delete(
                tx,
                "machinery_es_extension",
                machinery_id,
                None,
                &before,
                None,
            )?;
            Ok(None)
        }
        (Some(before), true) => {
            let after = MachineryEsExtension {
                machinery_id: machinery_id.to_string(),
                roma_number,
                reganip_number,
            };
            tx.execute(
                "UPDATE machinery_es_extension SET roma_number = ?2, reganip_number = ?3
                 WHERE machinery_id = ?1",
                params![machinery_id, after.roma_number, after.reganip_number],
            )?;
            log_update(
                tx,
                "machinery_es_extension",
                machinery_id,
                None,
                &before,
                &after,
            )?;
            Ok(Some(after))
        }
    }
}

fn map_extension(row: &Row) -> rusqlite::Result<MachineryEsExtension> {
    Ok(MachineryEsExtension {
        machinery_id: row.get("machinery_id")?,
        roma_number: row.get("roma_number")?,
        reganip_number: row.get("reganip_number")?,
    })
}

fn map_machinery(row: &Row) -> rusqlite::Result<Machinery> {
    Ok(Machinery {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        name: row.get("name")?,
        kind: row.get("type")?,
        last_inspection_date: row.get("last_inspection_date")?,
        next_inspection_due_date: row.get("next_inspection_due_date")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
