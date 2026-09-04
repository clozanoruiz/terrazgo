// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Season (campaña agrícola) CRUD.
//!
//! Deletion is deliberately narrow: only an EMPTY season may be soft-deleted,
//! because every record-book view is season-scoped — hiding a season that owns
//! records would hide the records with it. Retiring a season that holds real
//! data is what the orthogonal `status` column ('active' | 'archived') is for.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewSeason, Season, UpdateSeason};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

pub fn insert_season(conn: &mut Connection, new: NewSeason, actor: Option<&str>) -> Result<Season> {
    validate_name(&new.label)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let season = Season {
        id: Uuid::now_v7().to_string(),
        campaign_year: new.campaign_year,
        label: new.label,
        starts_on: new.starts_on,
        ends_on: new.ends_on,
        status: "active".to_string(),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO season (id, campaign_year, label, starts_on, ends_on, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            season.id, season.campaign_year, season.label, season.starts_on,
            season.ends_on, season.status, season.created_at, season.updated_at
        ],
    )?;
    log_insert(&tx, "season", &season.id, Some(&season.id), actor, &season)?;
    tx.commit()?;
    Ok(season)
}

/// Every live season, newest campaign first — the season selector default is the
/// most recent one.
pub fn list_seasons(conn: &Connection) -> Result<Vec<Season>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM season WHERE deleted_at IS NULL ORDER BY campaign_year DESC, id DESC",
    )?;
    let seasons = stmt
        .query_map([], map_season)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(seasons)
}

/// Full-row update; the submitted state replaces the stored one. Safe at any
/// time: treatment records reference the season by id, and the printed cuaderno
/// takes the campaign label from the season row precisely so a correction here
/// reaches the document.
pub fn update_season(
    conn: &mut Connection,
    id: &str,
    update: UpdateSeason,
    actor: Option<&str>,
) -> Result<Season> {
    validate_name(&update.label)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM season WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_season,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.campaign_year = update.campaign_year;
    after.label = update.label;
    after.starts_on = update.starts_on;
    after.ends_on = update.ends_on;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE season SET campaign_year = ?2, label = ?3, starts_on = ?4, ends_on = ?5,
                           updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            after.campaign_year,
            after.label,
            after.starts_on,
            after.ends_on,
            after.updated_at
        ],
    )?;
    log_update(&tx, "season", id, Some(id), actor, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete, for a season created by mistake. Refuses while the season still
/// holds crops (`invalid.season_in_use`) — see the module doc for why.
///
/// This guard covers only the core side — crops, the sowing register and the
/// commercialised harvest. The CUE registers (treatments, the non-field ones, treated seed,
/// analyses) also hang off a season, but they belong to module-cue and core may
/// never reference a module table, so the shell command checks that half before
/// calling here (the same chaining the zone-flag command does).
pub fn soft_delete_season(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    if super::crop::season_has_crops(conn, id)?
        || super::sowing::season_has_sowings(conn, id)?
        || super::harvest::season_has_harvests(conn, id)?
    {
        return Err(CoreError::Invalid("season_in_use"));
    }
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM season WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_season,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE season SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "season", id, Some(id), actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

fn map_season(row: &Row) -> rusqlite::Result<Season> {
    Ok(Season {
        id: row.get("id")?,
        campaign_year: row.get("campaign_year")?,
        label: row.get("label")?,
        starts_on: row.get("starts_on")?,
        ends_on: row.get("ends_on")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
