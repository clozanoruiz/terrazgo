// SPDX-License-Identifier: AGPL-3.0-or-later

//! Season (campaña agrícola) inserts and lists.

use super::validate_name;
use crate::audit::log_insert;
use crate::date::now_utc_iso;
use crate::error::Result;
use crate::models::{NewSeason, Season};
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

pub fn insert_season(conn: &mut Connection, new: NewSeason) -> Result<Season> {
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
    };
    tx.execute(
        "INSERT INTO season (id, campaign_year, label, starts_on, ends_on, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            season.id, season.campaign_year, season.label, season.starts_on,
            season.ends_on, season.status, season.created_at, season.updated_at
        ],
    )?;
    log_insert(&tx, "season", &season.id, Some(&season.id), &season)?;
    tx.commit()?;
    Ok(season)
}

/// Every season, newest campaign first — the season selector default is the
/// most recent one. Seasons are never soft-deleted (every record references one).
pub fn list_seasons(conn: &Connection) -> Result<Vec<Season>> {
    let mut stmt = conn.prepare("SELECT * FROM season ORDER BY campaign_year DESC, id DESC")?;
    let seasons = stmt
        .query_map([], map_season)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(seasons)
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
    })
}
