// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Water abstraction points near a plot (`plot_water_point`) and the stored
//! negative that says a plot has none (`plot_water_declaration`) — the water
//! half of the printed model's section 2.2, Anexo III A.1.f–g.
//!
//! Fully correctable, unlike the treatment registers: these rows freeze no
//! snapshot of another row, which is the condition `*_snapshot` columns exist
//! to handle. Where there is nothing frozen, immutability buys nothing.
//!
//! The declaration invariant runs BOTH ways, as in the CUE module's
//! `register_declaration`: declaring a plot empty while it holds points is
//! refused, and recording a point on a declared plot withdraws the declaration
//! in the same transaction. The record is the stronger statement, and a stale
//! "no captaciones" printing beside a contradicting row would forge
//! proof-of-check.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewWaterPoint, UpdateWaterPoint, WaterDeclaration, WaterPoint};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// Record an abstraction point on a plot, withdrawing any standing "this plot
/// has none" in the same transaction.
pub fn insert_water_point(
    conn: &mut Connection,
    new: NewWaterPoint,
    actor: Option<&str>,
) -> Result<WaterPoint> {
    validate_name(&new.denomination)?;
    let distance_m = validated_distance(new.inside_plot, new.distance_m)?;
    let (latitude, longitude) = validated_coordinates(new.latitude, new.longitude)?;

    let tx = conn.transaction()?;
    require_active_plot(&tx, &new.plot_id)?;
    // A point contradicts any standing "nothing here" for its plot.
    withdraw_declaration_tx(&tx, &new.plot_id, actor)?;

    let now = now_utc_iso();
    let point = WaterPoint {
        id: Uuid::now_v7().to_string(),
        plot_id: new.plot_id,
        denomination: new.denomination.trim().to_string(),
        inside_plot: new.inside_plot,
        distance_m,
        latitude,
        longitude,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO plot_water_point
           (id, plot_id, denomination, inside_plot, distance_m, latitude, longitude,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            point.id,
            point.plot_id,
            point.denomination,
            point.inside_plot,
            point.distance_m,
            point.latitude,
            point.longitude,
            point.created_at,
            point.updated_at
        ],
    )?;
    log_insert(&tx, "plot_water_point", &point.id, None, actor, &point)?;
    tx.commit()?;
    Ok(point)
}

/// Every active water point of a farm's active plots, ordered as section 2.2
/// prints them (by plot, then by the order they were recorded).
pub fn list_water_points(conn: &Connection, farm_id: &str) -> Result<Vec<WaterPoint>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM plot_water_point
         WHERE deleted_at IS NULL
           AND plot_id IN (SELECT id FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL)
         ORDER BY plot_id, created_at, id",
    )?;
    let points = stmt
        .query_map([farm_id], map_point)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(points)
}

/// Full-row update; the submitted state replaces the stored one. No `plot_id`
/// — see `UpdateWaterPoint`.
pub fn update_water_point(
    conn: &mut Connection,
    id: &str,
    update: UpdateWaterPoint,
    actor: Option<&str>,
) -> Result<WaterPoint> {
    validate_name(&update.denomination)?;
    let distance_m = validated_distance(update.inside_plot, update.distance_m)?;
    let (latitude, longitude) = validated_coordinates(update.latitude, update.longitude)?;

    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM plot_water_point WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_point,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.denomination = update.denomination.trim().to_string();
    after.inside_plot = update.inside_plot;
    after.distance_m = distance_m;
    after.latitude = latitude;
    after.longitude = longitude;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE plot_water_point
            SET denomination = ?2, inside_plot = ?3, distance_m = ?4,
                latitude = ?5, longitude = ?6, updated_at = ?7
          WHERE id = ?1",
        params![
            id,
            after.denomination,
            after.inside_plot,
            after.distance_m,
            after.latitude,
            after.longitude,
            after.updated_at
        ],
    )?;
    log_update(&tx, "plot_water_point", id, None, actor, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete: a past campaign's printed book named this point, so the row
/// stays reachable through its audit history.
pub fn soft_delete_water_point(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM plot_water_point WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_point,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE plot_water_point SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(
        &tx,
        "plot_water_point",
        id,
        None,
        actor,
        &before,
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}

/// Standing "this plot has no abstraction point" declarations of a farm.
pub fn list_water_declarations(conn: &Connection, farm_id: &str) -> Result<Vec<WaterDeclaration>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM plot_water_declaration
         WHERE deleted_at IS NULL
           AND plot_id IN (SELECT id FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL)
         ORDER BY plot_id",
    )?;
    let declarations = stmt
        .query_map([farm_id], map_declaration)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(declarations)
}

/// State that a plot has no abstraction point, or restate the date of a
/// standing declaration. Refused while the plot holds points: the two
/// statements contradict each other, and the rows are the stronger one.
pub fn set_water_declaration(
    conn: &mut Connection,
    plot_id: &str,
    declared_on: &str,
    actor: Option<&str>,
) -> Result<WaterDeclaration> {
    let tx = conn.transaction()?;
    require_active_plot(&tx, plot_id)?;

    let points: i64 = tx.query_row(
        "SELECT COUNT(*) FROM plot_water_point WHERE plot_id = ?1 AND deleted_at IS NULL",
        [plot_id],
        |row| row.get(0),
    )?;
    if points > 0 {
        return Err(CoreError::Invalid("plot_has_water_points"));
    }

    let standing = tx
        .query_row(
            "SELECT * FROM plot_water_declaration WHERE plot_id = ?1 AND deleted_at IS NULL",
            [plot_id],
            map_declaration,
        )
        .optional()?;
    let now = now_utc_iso();

    let declaration = match standing {
        Some(before) => {
            let mut after = before.clone();
            after.declared_on = declared_on.to_string();
            after.updated_at = now;
            tx.execute(
                "UPDATE plot_water_declaration SET declared_on = ?2, updated_at = ?3 WHERE id = ?1",
                params![after.id, after.declared_on, after.updated_at],
            )?;
            log_update(
                &tx,
                "plot_water_declaration",
                &after.id,
                None,
                actor,
                &before,
                &after,
            )?;
            after
        }
        None => {
            let row = WaterDeclaration {
                id: Uuid::now_v7().to_string(),
                plot_id: plot_id.to_string(),
                declared_on: declared_on.to_string(),
                created_at: now.clone(),
                updated_at: now,
                deleted_at: None,
            };
            tx.execute(
                "INSERT INTO plot_water_declaration
                   (id, plot_id, declared_on, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.id,
                    row.plot_id,
                    row.declared_on,
                    row.created_at,
                    row.updated_at
                ],
            )?;
            log_insert(&tx, "plot_water_declaration", &row.id, None, actor, &row)?;
            row
        }
    };
    tx.commit()?;
    Ok(declaration)
}

/// Take back a declaration made in error. Soft delete, so the audit trail keeps
/// saying the farmer once declared it.
pub fn clear_water_declaration(
    conn: &mut Connection,
    plot_id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    withdraw_declaration_tx(&tx, plot_id, actor)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Withdraw the standing declaration for one plot, if there is one. Shared by
/// the explicit clear and by `insert_water_point`, which must withdraw inside
/// its own transaction so the record and the retraction land together.
fn withdraw_declaration_tx(tx: &Transaction, plot_id: &str, actor: Option<&str>) -> Result<()> {
    let Some(before) = tx
        .query_row(
            "SELECT * FROM plot_water_declaration WHERE plot_id = ?1 AND deleted_at IS NULL",
            [plot_id],
            map_declaration,
        )
        .optional()?
    else {
        return Ok(());
    };
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE plot_water_declaration SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![before.id, now],
    )?;
    log_delete(
        tx,
        "plot_water_declaration",
        &before.id,
        None,
        actor,
        &before,
        Some(&after),
    )?;
    Ok(())
}

fn require_active_plot(tx: &Transaction, plot_id: &str) -> Result<()> {
    tx.query_row(
        "SELECT 1 FROM plot WHERE id = ?1 AND deleted_at IS NULL",
        [plot_id],
        |_| Ok(()),
    )
    .optional()?
    .ok_or(CoreError::NotFound)
}

/// A.1.g asks for the distance when the point lies outside the plot, and it is
/// knowledge the farmer already has (unlike efficacy, which is observed later),
/// so it is required rather than nullable. Inside the plot it must be absent:
/// a distance there contradicts the answer to the column beside it.
fn validated_distance(inside_plot: bool, distance_m: Option<f64>) -> Result<Option<f64>> {
    match (inside_plot, distance_m) {
        (true, Some(_)) => Err(CoreError::Invalid("water_point_distance_inside")),
        (true, None) => Ok(None),
        (false, None) => Err(CoreError::Invalid("missing_distance")),
        (false, Some(distance)) if distance <= 0.0 || !distance.is_finite() => {
            Err(CoreError::Invalid("missing_distance"))
        }
        (false, some) => Ok(some),
    }
}

/// Voluntary, but half a coordinate locates nothing — both or neither.
fn validated_coordinates(
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<(Option<f64>, Option<f64>)> {
    match (latitude, longitude) {
        (None, None) => Ok((None, None)),
        (Some(lat), Some(lon))
            if lat.is_finite()
                && lon.is_finite()
                && (-90.0..=90.0).contains(&lat)
                && (-180.0..=180.0).contains(&lon) =>
        {
            Ok((Some(lat), Some(lon)))
        }
        _ => Err(CoreError::Invalid("water_point_coordinates_invalid")),
    }
}

fn map_point(row: &Row) -> rusqlite::Result<WaterPoint> {
    Ok(WaterPoint {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        denomination: row.get("denomination")?,
        inside_plot: row.get("inside_plot")?,
        distance_m: row.get("distance_m")?,
        latitude: row.get("latitude")?,
        longitude: row.get("longitude")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_declaration(row: &Row) -> rusqlite::Result<WaterDeclaration> {
    Ok(WaterDeclaration {
        id: row.get("id")?,
        plot_id: row.get("plot_id")?,
        declared_on: row.get("declared_on")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
