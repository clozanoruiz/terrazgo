// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Crop CRUD: the crop on a plot in a given season ("crop at time of treatment"
//! in CUE links here; future crop-planning modules will too).
//!
//! Soft-delete only, like every other referenced entity. Past treatment records
//! are safe from both edits and deletes — `treatment_plot` freezes the crop name
//! and variety in its own snapshot columns at write time.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{Crop, NewCrop, UpdateCrop};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

/// Provenance of a hand-typed crop — what `crop.source` defaults to.
pub const SOURCE_USER: &str = "user";

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
        area_ha: new.area_ha,
        irrigation_code: new.irrigation_code,
        growing_environment_code: new.growing_environment_code,
        gip_system_code: new.gip_system_code,
        crop_code: new.crop_code,
        source: new.source.unwrap_or_else(|| SOURCE_USER.to_string()),
        source_campaign: new.source_campaign,
        declared_area_ha: new.declared_area_ha,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO crop
           (id, plot_id, season_id, species_name, variety, production_system_code,
            area_ha, irrigation_code, growing_environment_code, gip_system_code,
            crop_code, source, source_campaign, declared_area_ha,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            crop.id,
            crop.plot_id,
            crop.season_id,
            crop.species_name,
            crop.variety,
            crop.production_system_code,
            crop.area_ha,
            crop.irrigation_code,
            crop.growing_environment_code,
            crop.gip_system_code,
            crop.crop_code,
            crop.source,
            crop.source_campaign,
            crop.declared_area_ha,
            crop.created_at,
            crop.updated_at
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
         ORDER BY crop.id",
    )?;
    let crops = stmt
        .query_map(params![season_id, farm_id], map_crop)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(crops)
}

/// One crop by id, WITHDRAWN ONES INCLUDED — the SIEX export.
///
/// **The soft-delete filter is left off deliberately, and that is why this has a
/// name rather than being a query at each caller.** Crop deletion is always
/// allowed (`treatment_plot` and its siblings froze the name and variety they
/// print), so a record written years ago routinely names a crop that is no
/// longer live — and the descriptor still has to state that crop's PRODUCTOS
/// code. Spelling the query inline four times expressed that intent only by the
/// absence of a `WHERE`, which the next reader to tidy one of them would not
/// see.
///
/// `find_` rather than `get_`, the [`super::find_export_alias`] convention: a
/// missing row is `None`, not an error. Every `crop_id` column that reaches here
/// carries a real foreign key, so that case is unreachable in practice and the
/// caller simply names no crop code.
pub fn find_crop_for_export(conn: &Connection, id: &str) -> Result<Option<Crop>> {
    let crop = conn
        .query_row("SELECT * FROM crop WHERE id = ?1", [id], map_crop)
        .optional()?;
    Ok(crop)
}

/// Live crops on ONE plot in one season — the DGC units that plot carries.
///
/// A narrower [`list_crops`], and it exists because several readers have to
/// answer "which crop was on this plot" for a register that stores only the
/// plot: the eco-scheme junctions carry no `crop_id`, because no printed page of
/// model section 9 asks for one, while the SIEX exchange unit is a plot+crop
/// pair — a field FEGA itself describes as *"campo calculado"*.
///
/// It returns every match rather than an `Option` on purpose. **A plot carrying
/// two crops is two units, and which of them a caller may assume is the
/// caller's rule, not this one's**: the export refuses such a plot by name
/// rather than choosing, and a future reader might reasonably split instead.
/// Soft-deleted crops are excluded — a withdrawn crop is not a unit, and
/// counting one would make a plot look ambiguous over a row the farmer has
/// already retracted.
pub fn crops_on_plot(conn: &Connection, plot_id: &str, season_id: &str) -> Result<Vec<Crop>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM crop
         WHERE plot_id = ?1 AND season_id = ?2 AND deleted_at IS NULL
         ORDER BY id",
    )?;
    let crops = stmt
        .query_map(params![plot_id, season_id], map_crop)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(crops)
}

/// Full-row update; the submitted state replaces the stored one. The crop stays
/// on its plot and in its season, and its provenance survives a form that does
/// not carry it (see `UpdateCrop`).
pub fn update_crop(
    conn: &mut Connection,
    id: &str,
    update: UpdateCrop,
    actor: Option<&str>,
) -> Result<Crop> {
    validate_name(&update.species_name)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM crop WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_crop,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.species_name = update.species_name;
    after.variety = update.variety;
    after.production_system_code = update.production_system_code;
    after.area_ha = update.area_ha;
    after.irrigation_code = update.irrigation_code;
    after.growing_environment_code = update.growing_environment_code;
    after.gip_system_code = update.gip_system_code;
    after.crop_code = update.crop_code;
    // Provenance is set-if-present: the manual edit form does not carry these,
    // and losing "this row came from the 2025 declaration" to an unrelated typo
    // fix would erase a fact nothing else records.
    after.source = update.source.unwrap_or(after.source);
    after.source_campaign = update.source_campaign.or(after.source_campaign);
    after.declared_area_ha = update.declared_area_ha.or(after.declared_area_ha);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE crop SET species_name = ?2, variety = ?3, production_system_code = ?4,
                         area_ha = ?5, irrigation_code = ?6, growing_environment_code = ?7,
                         gip_system_code = ?8, crop_code = ?9, source = ?10,
                         source_campaign = ?11, declared_area_ha = ?12, updated_at = ?13
         WHERE id = ?1",
        params![
            id,
            after.species_name,
            after.variety,
            after.production_system_code,
            after.area_ha,
            after.irrigation_code,
            after.growing_environment_code,
            after.gip_system_code,
            after.crop_code,
            after.source,
            after.source_campaign,
            after.declared_area_ha,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "crop",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete: the row stays so `treatment_plot.crop_id` keeps resolving, it
/// just leaves the crop list and the treatment form's per-plot crop picker.
/// Always allowed (the farm/plot precedent) — treatments printed in the cuaderno
/// read their crop from the snapshot columns, not from this row.
pub fn soft_delete_crop(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM crop WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_crop,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE crop SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(
        &tx,
        "crop",
        id,
        Some(&before.season_id),
        actor,
        &before,
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}

/// Whether any crop still lives in this season — the guard
/// `soft_delete_season` uses (only an empty season may be deleted).
pub(super) fn season_has_crops(conn: &Connection, season_id: &str) -> Result<bool> {
    // EXISTS rather than COUNT(*): the subquery stops at the first matching row,
    // where a count has to visit every one of a campaign's crops to answer a
    // yes/no. With `idx_crop_season_plot` it is a single index seek.
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM crop WHERE season_id = ?1 AND deleted_at IS NULL)",
        [season_id],
        |row| row.get(0),
    )?;
    Ok(held)
}

fn map_crop(row: &Row) -> rusqlite::Result<Crop> {
    Ok(Crop {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        season_id: row.get("season_id")?,
        species_name: row.get("species_name")?,
        variety: row.get("variety")?,
        production_system_code: row.get("production_system_code")?,
        area_ha: row.get("area_ha")?,
        irrigation_code: row.get("irrigation_code")?,
        growing_environment_code: row.get("growing_environment_code")?,
        gip_system_code: row.get("gip_system_code")?,
        crop_code: row.get("crop_code")?,
        source: row.get("source")?,
        source_campaign: row.get("source_campaign")?,
        declared_area_ha: row.get("declared_area_ha")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
