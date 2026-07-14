// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Zone-flag storage (`plot_zone_flag`), with full audit logging.
//!
//! Replace-within-campaign: a re-check soft-deletes the active row for the
//! same (plot, zone type, campaign, source) and inserts the new result in the
//! same transaction — the partial unique index enforces it by construction.
//! A new campaign appends instead, so "was this plot flagged in 2027?" stays
//! answerable forever. Flags come from a provider query and cannot be
//! re-derived offline, so they are user data: `record_change`-logged, synced,
//! in backups (the 2026-07-05 decision, unlike alerts).

use crate::audit::{log_delete, log_insert};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewZoneFlag, ZoneFlag};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// Store one provider check's results for a plot: every checked zone type in
/// one transaction, replacing this (campaign, source)'s previous active rows.
/// The plot must exist and be active; `status` must be 'inside' or 'outside'.
pub fn replace_zone_flags(
    conn: &mut Connection,
    plot_id: &str,
    campaign: i64,
    source: &str,
    flags: Vec<NewZoneFlag>,
) -> Result<Vec<ZoneFlag>> {
    for flag in &flags {
        if flag.status != "inside" && flag.status != "outside" {
            return Err(CoreError::Invalid("zone_status_invalid"));
        }
    }
    let tx = conn.transaction()?;
    tx.query_row(
        "SELECT 1 FROM plot WHERE id = ?1 AND deleted_at IS NULL",
        [plot_id],
        |_| Ok(()),
    )
    .optional()?
    .ok_or(CoreError::NotFound)?;

    let now = now_utc_iso();
    let mut stored = Vec::with_capacity(flags.len());
    for flag in flags {
        replace_active(&tx, plot_id, &flag.zone_type_code, campaign, source)?;
        let row = ZoneFlag {
            id: Uuid::now_v7().to_string(),
            plot_id: plot_id.to_string(),
            zone_type_code: flag.zone_type_code,
            campaign,
            status: flag.status,
            coverage_pct: flag.coverage_pct,
            detail: flag.detail,
            source: source.to_string(),
            checked_at: now.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            deleted_at: None,
        };
        tx.execute(
            "INSERT INTO plot_zone_flag
               (id, plot_id, zone_type_code, campaign, status, coverage_pct, detail,
                source, checked_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.id,
                row.plot_id,
                row.zone_type_code,
                row.campaign,
                row.status,
                row.coverage_pct,
                row.detail,
                row.source,
                row.checked_at,
                row.created_at,
                row.updated_at
            ],
        )?;
        log_insert(&tx, "plot_zone_flag", &row.id, None, &row)?;
        stored.push(row);
    }
    tx.commit()?;
    Ok(stored)
}

/// Active flags of one farm's active plots, newest campaign first — one call
/// feeds the plot cards' zone chips and the alert engine's candidates.
pub fn list_zone_flags_for_farm(conn: &Connection, farm_id: &str) -> Result<Vec<ZoneFlag>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM plot_zone_flag
         WHERE deleted_at IS NULL
           AND plot_id IN (SELECT id FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL)
         ORDER BY campaign DESC, plot_id, zone_type_code",
    )?;
    let flags = stmt
        .query_map([farm_id], map_zone_flag)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(flags)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn replace_active(
    tx: &Transaction,
    plot_id: &str,
    zone_type_code: &str,
    campaign: i64,
    source: &str,
) -> Result<()> {
    let current = tx
        .query_row(
            "SELECT * FROM plot_zone_flag
             WHERE deleted_at IS NULL AND plot_id = ?1 AND zone_type_code = ?2
               AND campaign = ?3 AND source = ?4",
            params![plot_id, zone_type_code, campaign, source],
            map_zone_flag,
        )
        .optional()?;
    if let Some(before) = current {
        let now = now_utc_iso();
        let mut after = before.clone();
        after.deleted_at = Some(now.clone());
        after.updated_at = now.clone();
        tx.execute(
            "UPDATE plot_zone_flag SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![before.id, now],
        )?;
        log_delete(
            tx,
            "plot_zone_flag",
            &before.id,
            None,
            &before,
            Some(&after),
        )?;
    }
    Ok(())
}

fn map_zone_flag(row: &Row) -> rusqlite::Result<ZoneFlag> {
    Ok(ZoneFlag {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        zone_type_code: row.get("zone_type_code")?,
        campaign: row.get("campaign")?,
        status: row.get("status")?,
        coverage_pct: row.get("coverage_pct")?,
        detail: row.get("detail")?,
        source: row.get("source")?,
        checked_at: row.get("checked_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
