// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Append-only `record_change` helpers — the audit trail AND future sync delta
//! source. Public: every crate that writes synced user data (the core repository,
//! module-cue's, future modules') logs through these, inside the same transaction
//! as the write itself.

use crate::date::now_utc_iso;
use crate::error::Result;
use rusqlite::{Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// Append one row to the append-only `record_change` log. `payload` is the full
/// `{"before": ..., "after": ...}` document for the change.
pub fn write_change(
    tx: &Transaction,
    entity_table: &str,
    entity_id: &str,
    season_id: Option<&str>,
    operation: &str,
    payload: Value,
) -> Result<()> {
    tx.execute(
        "INSERT INTO record_change
           (id, entity_table, entity_id, season_id, operation, changed_at, actor, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            Uuid::now_v7().to_string(),
            entity_table,
            entity_id,
            season_id,
            operation,
            now_utc_iso(),
            Option::<String>::None, // actor: filled once multi-device sync exists
            payload.to_string(),
        ],
    )?;
    Ok(())
}

/// Log an insert: `before` is null, `after` is the serialized new row.
///
/// `after` must be the complete domain struct, never a hand-picked subset: the log
/// doubles as the sync delta source, and a receiving device has to be able to
/// materialise the row from this payload alone.
pub fn log_insert<T: Serialize>(
    tx: &Transaction,
    table: &str,
    id: &str,
    season_id: Option<&str>,
    after: &T,
) -> Result<()> {
    write_change(
        tx,
        table,
        id,
        season_id,
        "insert",
        json!({ "before": Value::Null, "after": serde_json::to_value(after)? }),
    )
}

/// Log an update: complete before- and after-images of the row.
pub fn log_update<T: Serialize>(
    tx: &Transaction,
    table: &str,
    id: &str,
    season_id: Option<&str>,
    before: &T,
    after: &T,
) -> Result<()> {
    write_change(
        tx,
        table,
        id,
        season_id,
        "update",
        json!({ "before": serde_json::to_value(before)?, "after": serde_json::to_value(after)? }),
    )
}

/// Log a delete. For a soft delete `after` is the row with `deleted_at` set
/// (both images complete); for a hard delete (extension rows only — regulatory
/// records are never hard-deleted) `after` is `None`, serialized as null.
pub fn log_delete<T: Serialize>(
    tx: &Transaction,
    table: &str,
    id: &str,
    season_id: Option<&str>,
    before: &T,
    after: Option<&T>,
) -> Result<()> {
    let after = match after {
        Some(row) => serde_json::to_value(row)?,
        None => Value::Null,
    };
    write_change(
        tx,
        table,
        id,
        season_id,
        "delete",
        json!({ "before": serde_json::to_value(before)?, "after": after }),
    )
}
