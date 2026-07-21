// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Crop inserts and lists: the crop on a plot in a given season ("crop at time
//! of treatment" in CUE links here; future crop-planning modules will too).

use super::validate_name;
use crate::audit::log_insert;
use crate::date::now_utc_iso;
use crate::error::Result;
use crate::models::{Crop, NewCrop};
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

pub fn insert_crop(conn: &mut Connection, new: NewCrop, actor: Option<&str>) -> Result<Crop> {
    validate_name(&new.species_name)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let crop = Crop {
        id: Uuid::now_v7().to_string(),
        plot_id: new.plot_id,
        season_id: new.season_id,
        species_name: new.species_name,
        variety: new.variety,
        production_system_code: new.production_system_code,
        sown_on: new.sown_on,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO crop
           (id, plot_id, season_id, species_name, variety, production_system_code, sown_on, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            crop.id, crop.plot_id, crop.season_id, crop.species_name, crop.variety,
            crop.production_system_code, crop.sown_on, crop.created_at, crop.updated_at
        ],
    )?;
    log_insert(&tx, "crop", &crop.id, Some(&crop.season_id), actor, &crop)?;
    tx.commit()?;
    Ok(crop)
}

/// Active crops on a farm's plots in one season — what the treatment form
/// offers as "crop on this plot" per treated-plot row.
pub fn list_crops(conn: &Connection, season_id: &str, farm_id: &str) -> Result<Vec<Crop>> {
    let mut stmt = conn.prepare(
        "SELECT crop.* FROM crop
         JOIN plot ON plot.id = crop.plot_id
         WHERE crop.season_id = ?1 AND plot.farm_id = ?2 AND crop.deleted_at IS NULL
         ORDER BY crop.species_name, crop.id",
    )?;
    let crops = stmt
        .query_map(params![season_id, farm_id], map_crop)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(crops)
}

fn map_crop(row: &Row) -> rusqlite::Result<Crop> {
    Ok(Crop {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        season_id: row.get("season_id")?,
        species_name: row.get("species_name")?,
        variety: row.get("variety")?,
        production_system_code: row.get("production_system_code")?,
        sown_on: row.get("sown_on")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
