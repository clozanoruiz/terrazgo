// SPDX-License-Identifier: AGPL-3.0-or-later

//! Operator (aplicador) CRUD. Soft-delete only: operators are referenced by
//! treatment records, so rows are hidden, never removed. Past records are safe
//! from edits either way — they snapshot the operator's name and licence.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewOperator, Operator, UpdateOperator};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

pub fn insert_operator(conn: &mut Connection, new: NewOperator) -> Result<Operator> {
    validate_name(&new.full_name)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let operator = Operator {
        id: Uuid::now_v7().to_string(),
        full_name: new.full_name,
        licence_number: new.licence_number,
        licence_level_code: new.licence_level_code,
        licence_expiry_date: new.licence_expiry_date,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO operator
           (id, full_name, licence_number, licence_level_code, licence_expiry_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            operator.id, operator.full_name, operator.licence_number,
            operator.licence_level_code, operator.licence_expiry_date,
            operator.created_at, operator.updated_at
        ],
    )?;
    log_insert(&tx, "operator", &operator.id, None, &operator)?;
    tx.commit()?;
    Ok(operator)
}

/// Active operators, for the treatment form's operator selector. Operators are
/// not farm-scoped: the same applicator may work several farms.
pub fn list_operators(conn: &Connection) -> Result<Vec<Operator>> {
    let mut stmt =
        conn.prepare("SELECT * FROM operator WHERE deleted_at IS NULL ORDER BY full_name, id")?;
    let operators = stmt
        .query_map([], map_operator)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(operators)
}

/// Full-row update; the submitted state replaces the stored one.
pub fn update_operator(
    conn: &mut Connection,
    id: &str,
    update: UpdateOperator,
) -> Result<Operator> {
    validate_name(&update.full_name)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM operator WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_operator,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.full_name = update.full_name;
    after.licence_number = update.licence_number;
    after.licence_level_code = update.licence_level_code;
    after.licence_expiry_date = update.licence_expiry_date;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE operator SET full_name = ?2, licence_number = ?3, licence_level_code = ?4,
                             licence_expiry_date = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            after.full_name,
            after.licence_number,
            after.licence_level_code,
            after.licence_expiry_date,
            after.updated_at
        ],
    )?;
    log_update(&tx, "operator", id, None, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete: the row stays (treatment history must keep resolving), it just
/// leaves every list and picker. The operator's licence-expiry alert lapses on
/// the next `refresh_alerts` — the reconciler skips soft-deleted subjects.
pub fn soft_delete_operator(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM operator WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_operator,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE operator SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "operator", id, None, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

fn map_operator(row: &Row) -> rusqlite::Result<Operator> {
    Ok(Operator {
        id: row.get("id")?,
        full_name: row.get("full_name")?,
        licence_number: row.get("licence_number")?,
        licence_level_code: row.get("licence_level_code")?,
        licence_expiry_date: row.get("licence_expiry_date")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
