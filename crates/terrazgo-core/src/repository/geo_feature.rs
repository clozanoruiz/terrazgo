// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Geometry attached to core entities (plot/farm boundaries), with full audit
//! logging.
//!
//! Save is replace-within-source: at most one ACTIVE row exists per
//! (subject, role, source) — enforced by the schema's partial unique indexes —
//! so saving soft-deletes the previous row and inserts the new one in the same
//! transaction. Rows from different sources coexist (a manually drawn boundary
//! next to a provider-fetched one is the discrepancy-display case, not a
//! conflict). Soft delete keeps history: fetched geometry cannot be re-derived
//! offline, so replaced rows stay provable and sync like any user data.

use crate::audit::{log_delete, log_insert};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::geojson::validate_boundary_geometry;
use crate::models::{GeoFeature, NewGeoFeature};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// Save a geometry for its subject, replacing any active row with the same
/// (subject, role, source). Validates the exclusive arc, the subject's
/// existence and the GeoJSON geometry before touching the database.
pub fn save_geo_feature(
    conn: &mut Connection,
    new: NewGeoFeature,
    actor: Option<&str>,
) -> Result<GeoFeature> {
    validate_arc(&new)?;
    validate_boundary_geometry(&new.geometry)?;
    let tx = conn.transaction()?;
    ensure_subject_exists(&tx, &new)?;
    replace_active(&tx, &new, actor)?;

    let now = now_utc_iso();
    let feature = GeoFeature {
        id: Uuid::now_v7().to_string(),
        plot_id: new.plot_id,
        farm_id: new.farm_id,
        role: new.role,
        geometry: new.geometry,
        source: new.source,
        campaign: new.campaign,
        official_area_ha: new.official_area_ha,
        properties: new.properties,
        fetched_at: new.fetched_at,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO geo_feature
           (id, plot_id, farm_id, role, geometry, source, campaign, official_area_ha,
            properties, fetched_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            feature.id,
            feature.plot_id,
            feature.farm_id,
            feature.role,
            feature.geometry,
            feature.source,
            feature.campaign,
            feature.official_area_ha,
            feature.properties,
            feature.fetched_at,
            feature.created_at,
            feature.updated_at
        ],
    )?;
    log_insert(&tx, "geo_feature", &feature.id, None, actor, &feature)?;
    tx.commit()?;
    Ok(feature)
}

/// Active geometries of one farm: its own (farm-arc) rows plus those of its
/// active plots — one call feeds the whole map for a farm.
pub fn list_geo_features_for_farm(conn: &Connection, farm_id: &str) -> Result<Vec<GeoFeature>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM geo_feature
         WHERE deleted_at IS NULL
           AND (farm_id = ?1
                OR plot_id IN (SELECT id FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL))
         ORDER BY id",
    )?;
    let features = stmt
        .query_map([farm_id], map_geo_feature)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(features)
}

/// Soft delete one geometry row (e.g. the user discards a drawn boundary).
pub fn soft_delete_geo_feature(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM geo_feature WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_geo_feature,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    soft_delete_row(&tx, &before, actor)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Exactly one subject id must be set (defense-in-depth above the schema CHECK,
/// so the caller gets a stable machine code instead of a raw constraint error).
fn validate_arc(new: &NewGeoFeature) -> Result<()> {
    match (new.plot_id.is_some(), new.farm_id.is_some()) {
        (true, false) | (false, true) => Ok(()),
        (false, false) => Err(CoreError::Invalid("geo_subject_missing")),
        (true, true) => Err(CoreError::Invalid("geo_subject_ambiguous")),
    }
}

/// The subject row must exist and be active — a geometry for a deleted plot
/// would be invisible everywhere yet still sync.
fn ensure_subject_exists(tx: &Transaction, new: &NewGeoFeature) -> Result<()> {
    let (sql, id) = match (&new.plot_id, &new.farm_id) {
        (Some(plot_id), None) => (
            "SELECT 1 FROM plot WHERE id = ?1 AND deleted_at IS NULL",
            plot_id,
        ),
        (None, Some(farm_id)) => (
            "SELECT 1 FROM farm WHERE id = ?1 AND deleted_at IS NULL",
            farm_id,
        ),
        // validate_arc already rejected these shapes.
        _ => return Err(CoreError::Invalid("geo_subject_missing")),
    };
    tx.query_row(sql, [id], |_| Ok(()))
        .optional()?
        .ok_or(CoreError::NotFound)
}

/// Soft-delete the currently active row for this (subject, role, source), if any.
fn replace_active(tx: &Transaction, new: &NewGeoFeature, actor: Option<&str>) -> Result<()> {
    let current = tx
        .query_row(
            "SELECT * FROM geo_feature
             WHERE deleted_at IS NULL AND role = ?1 AND source = ?2
               AND ((?3 IS NOT NULL AND plot_id = ?3) OR (?4 IS NOT NULL AND farm_id = ?4))",
            params![new.role, new.source, new.plot_id, new.farm_id],
            map_geo_feature,
        )
        .optional()?;
    if let Some(before) = current {
        soft_delete_row(tx, &before, actor)?;
    }
    Ok(())
}

fn soft_delete_row(tx: &Transaction, before: &GeoFeature, actor: Option<&str>) -> Result<()> {
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE geo_feature SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![before.id, now],
    )?;
    log_delete(
        tx,
        "geo_feature",
        &before.id,
        None,
        actor,
        before,
        Some(&after),
    )?;
    Ok(())
}

fn map_geo_feature(row: &Row) -> rusqlite::Result<GeoFeature> {
    Ok(GeoFeature {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        farm_id: row.get("farm_id")?,
        role: row.get("role")?,
        geometry: row.get("geometry")?,
        source: row.get("source")?,
        campaign: row.get("campaign")?,
        official_area_ha: row.get("official_area_ha")?,
        properties: row.get("properties")?,
        fetched_at: row.get("fetched_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
