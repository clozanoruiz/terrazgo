// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integer export aliases (design in docs/siex-export.md → gap 1).
//!
//! Regulatory exchange formats key activities on small integers (SIEX
//! `IdAjena*`: number(10)), so each exported record gets a monotonic alias
//! minted at first export and frozen forever — re-exporting must reuse it, or
//! the authority sees a new activity instead of an update. Aliases are synced
//! user data (they cannot be re-derived and must survive backups), so inserts
//! are logged to `record_change`. Known limit, recorded in the design doc:
//! two devices exporting independently before syncing could mint colliding
//! integers — a sync-stage-2 design item; today one device exports.

use super::audit::log_insert;
use crate::error::Result;
use crate::models::ExportAlias;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

/// Look up the alias for `(target, entity_table, entity_id, split_key)`
/// WITHOUT minting. `None` means "never exported" — deletion entries must
/// reference only what the authority actually received, so a miss is a signal
/// to emit nothing, not to assign a number.
pub fn find_export_alias(
    conn: &Connection,
    target: &str,
    entity_table: &str,
    entity_id: &str,
    split_key: &str,
) -> Result<Option<i64>> {
    let alias = conn
        .query_row(
            "SELECT alias FROM export_alias
             WHERE target = ?1 AND entity_table = ?2 AND entity_id = ?3 AND split_key = ?4",
            params![target, entity_table, entity_id, split_key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(alias)
}

/// Return the alias for `(target, entity_table, entity_id, split_key)`,
/// minting the next integer (MAX+1 per target, starting at 1) when the tuple
/// has never been exported. Race-free behind the app's connection mutex; the
/// read and the insert share one transaction regardless.
pub fn ensure_export_alias(
    conn: &mut Connection,
    target: &str,
    entity_table: &str,
    entity_id: &str,
    split_key: &str,
    actor: Option<&str>,
) -> Result<i64> {
    let tx = conn.transaction()?;
    let existing: Option<i64> = tx
        .query_row(
            "SELECT alias FROM export_alias
             WHERE target = ?1 AND entity_table = ?2 AND entity_id = ?3 AND split_key = ?4",
            params![target, entity_table, entity_id, split_key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(alias) = existing {
        return Ok(alias);
    }

    let next: i64 = tx.query_row(
        "SELECT COALESCE(MAX(alias), 0) + 1 FROM export_alias WHERE target = ?1",
        [target],
        |r| r.get(0),
    )?;
    let row = ExportAlias {
        id: Uuid::now_v7().to_string(),
        target: target.to_string(),
        entity_table: entity_table.to_string(),
        entity_id: entity_id.to_string(),
        split_key: split_key.to_string(),
        alias: next,
        created_at: terrazgo_core::date::now_utc_iso(),
    };
    tx.execute(
        "INSERT INTO export_alias (id, target, entity_table, entity_id, split_key, alias, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.target,
            row.entity_table,
            row.entity_id,
            row.split_key,
            row.alias,
            row.created_at
        ],
    )?;
    log_insert(&tx, "export_alias", &row.id, None, actor, &row)?;
    tx.commit()?;
    Ok(next)
}
