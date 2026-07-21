// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Farm and plot CRUD (the land entities), with full audit logging.
//!
//! Soft-delete only: farms and plots are referenced by regulatory treatment
//! records, so rows are never removed — `deleted_at` hides them from lists and
//! pickers while history keeps resolving. The ES extension rows are the one
//! exception: removing a SIGPAC/REGA reference hard-deletes the extension row
//! (logged with a null after-image), the parent row is untouched.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{
    Farm, FarmDetail, FarmEsExtension, FarmEsFields, NewFarm, NewPlot, Plot, PlotDetail,
    PlotEsExtension, PlotEsFields, UpdateFarm, UpdatePlot,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Farm
// ---------------------------------------------------------------------------

pub fn insert_farm(conn: &mut Connection, new: NewFarm, actor: Option<&str>) -> Result<Farm> {
    validate_name(&new.name)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let farm = Farm {
        id: Uuid::now_v7().to_string(),
        name: new.name,
        owner_name: new.owner_name,
        owner_tax_id: new.owner_tax_id,
        location_text: None, // not on the create form yet; editable via update_farm
        latitude: None,
        longitude: None,
        country_code: new.country_code,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO farm
           (id, name, owner_name, owner_tax_id, location_text, latitude, longitude, country_code, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            farm.id, farm.name, farm.owner_name, farm.owner_tax_id, farm.location_text, farm.latitude,
            farm.longitude, farm.country_code, farm.created_at, farm.updated_at
        ],
    )?;
    log_insert(&tx, "farm", &farm.id, None, actor, &farm)?;
    if let Some(es) = new.es {
        insert_farm_extension(&tx, &farm.id, &es, actor)?;
    }
    tx.commit()?;
    Ok(farm)
}

/// Active farms, newest first (UUIDv7 ids are insertion-ordered).
pub fn list_farms(conn: &Connection) -> Result<Vec<Farm>> {
    let mut stmt = conn.prepare("SELECT * FROM farm WHERE deleted_at IS NULL ORDER BY id DESC")?;
    let farms = stmt
        .query_map([], map_farm)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(farms)
}

/// One active farm with its ES extension (what the edit form needs).
pub fn get_farm(conn: &Connection, id: &str) -> Result<FarmDetail> {
    let farm = conn
        .query_row(
            "SELECT * FROM farm WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_farm,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let es = get_farm_extension(conn, id)?;
    Ok(FarmDetail { farm, es })
}

/// Full-row update; the submitted state replaces the stored one. Logs complete
/// before/after images for the farm and for any extension transition.
pub fn update_farm(
    conn: &mut Connection,
    id: &str,
    update: UpdateFarm,
    actor: Option<&str>,
) -> Result<FarmDetail> {
    validate_name(&update.name)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM farm WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_farm,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.name = update.name;
    after.owner_name = update.owner_name;
    after.owner_tax_id = update.owner_tax_id;
    after.location_text = update.location_text;
    after.latitude = update.latitude;
    after.longitude = update.longitude;
    after.country_code = update.country_code;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE farm SET name = ?2, owner_name = ?3, owner_tax_id = ?4, location_text = ?5,
                         latitude = ?6, longitude = ?7, country_code = ?8, updated_at = ?9
         WHERE id = ?1",
        params![
            id,
            after.name,
            after.owner_name,
            after.owner_tax_id,
            after.location_text,
            after.latitude,
            after.longitude,
            after.country_code,
            after.updated_at
        ],
    )?;
    log_update(&tx, "farm", id, None, actor, &before, &after)?;

    let es = reconcile_farm_extension(&tx, id, update.es, actor)?;
    tx.commit()?;
    Ok(FarmDetail { farm: after, es })
}

/// Soft delete: the row stays (treatment history must keep resolving), it just
/// leaves every list and picker. The extension row is kept with it.
pub fn soft_delete_farm(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM farm WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_farm,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE farm SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "farm", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Plot
// ---------------------------------------------------------------------------

pub fn insert_plot(conn: &mut Connection, new: NewPlot, actor: Option<&str>) -> Result<Plot> {
    validate_name(&new.name)?;
    validate_area(new.area_ha)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let plot = Plot {
        id: Uuid::now_v7().to_string(),
        farm_id: new.farm_id,
        name: new.name,
        area_ha: new.area_ha,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO plot (id, farm_id, name, area_ha, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            plot.id,
            plot.farm_id,
            plot.name,
            plot.area_ha,
            plot.created_at,
            plot.updated_at
        ],
    )?;
    log_insert(&tx, "plot", &plot.id, None, actor, &plot)?;
    if let Some(es) = new.es {
        insert_plot_extension(&tx, &plot.id, &es, actor)?;
    }
    tx.commit()?;
    Ok(plot)
}

/// Active plots of one farm, with their SIGPAC extensions, insertion order.
pub fn list_plots(conn: &Connection, farm_id: &str) -> Result<Vec<PlotDetail>> {
    let mut stmt =
        conn.prepare("SELECT * FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL ORDER BY id")?;
    let plots = stmt
        .query_map([farm_id], map_plot)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    plots
        .into_iter()
        .map(|plot| {
            let es = get_plot_extension(conn, &plot.id)?;
            Ok(PlotDetail { plot, es })
        })
        .collect()
}

/// Full-row update. `farm_id` is immutable by design (see `UpdatePlot`).
pub fn update_plot(
    conn: &mut Connection,
    id: &str,
    update: UpdatePlot,
    actor: Option<&str>,
) -> Result<PlotDetail> {
    validate_name(&update.name)?;
    validate_area(update.area_ha)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM plot WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_plot,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.name = update.name;
    after.area_ha = update.area_ha;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE plot SET name = ?2, area_ha = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, after.name, after.area_ha, after.updated_at],
    )?;
    log_update(&tx, "plot", id, None, actor, &before, &after)?;

    let es = reconcile_plot_extension(&tx, id, update.es, actor)?;
    tx.commit()?;
    Ok(PlotDetail { plot: after, es })
}

pub fn soft_delete_plot(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM plot WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_plot,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE plot SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "plot", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Extension plumbing
// ---------------------------------------------------------------------------

fn insert_farm_extension(
    tx: &Transaction,
    farm_id: &str,
    es: &FarmEsFields,
    actor: Option<&str>,
) -> Result<FarmEsExtension> {
    let ext = FarmEsExtension {
        farm_id: farm_id.to_string(),
        rega_code: es.rega_code.clone(),
        rea_code: es.rea_code.clone(),
        province_code: es.province_code.clone(),
    };
    tx.execute(
        "INSERT INTO farm_es_extension (farm_id, rega_code, rea_code, province_code) VALUES (?1, ?2, ?3, ?4)",
        params![ext.farm_id, ext.rega_code, ext.rea_code, ext.province_code],
    )?;
    log_insert(tx, "farm_es_extension", farm_id, None, actor, &ext)?;
    Ok(ext)
}

fn get_farm_extension(conn: &Connection, farm_id: &str) -> Result<Option<FarmEsExtension>> {
    Ok(conn
        .query_row(
            "SELECT * FROM farm_es_extension WHERE farm_id = ?1",
            [farm_id],
            map_farm_extension,
        )
        .optional()?)
}

/// Bring the extension row in line with the submitted state, logging the
/// transition (insert / update / hard delete with null after-image).
fn reconcile_farm_extension(
    tx: &Transaction,
    farm_id: &str,
    desired: Option<FarmEsFields>,
    actor: Option<&str>,
) -> Result<Option<FarmEsExtension>> {
    let current = {
        // Same query as get_farm_extension, but on the open transaction.
        tx.query_row(
            "SELECT * FROM farm_es_extension WHERE farm_id = ?1",
            [farm_id],
            map_farm_extension,
        )
        .optional()?
    };
    match (current, desired) {
        (None, None) => Ok(None),
        (None, Some(es)) => Ok(Some(insert_farm_extension(tx, farm_id, &es, actor)?)),
        (Some(before), None) => {
            tx.execute(
                "DELETE FROM farm_es_extension WHERE farm_id = ?1",
                [farm_id],
            )?;
            log_delete(tx, "farm_es_extension", farm_id, None, actor, &before, None)?;
            Ok(None)
        }
        (Some(before), Some(es)) => {
            let after = FarmEsExtension {
                farm_id: farm_id.to_string(),
                rega_code: es.rega_code,
                rea_code: es.rea_code,
                province_code: es.province_code,
            };
            tx.execute(
                "UPDATE farm_es_extension SET rega_code = ?2, rea_code = ?3, province_code = ?4 WHERE farm_id = ?1",
                params![farm_id, after.rega_code, after.rea_code, after.province_code],
            )?;
            log_update(
                tx,
                "farm_es_extension",
                farm_id,
                None,
                actor,
                &before,
                &after,
            )?;
            Ok(Some(after))
        }
    }
}

fn insert_plot_extension(
    tx: &Transaction,
    plot_id: &str,
    es: &PlotEsFields,
    actor: Option<&str>,
) -> Result<PlotEsExtension> {
    let ext = PlotEsExtension {
        plot_id: plot_id.to_string(),
        sigpac_province: es.sigpac_province.clone(),
        sigpac_municipality: es.sigpac_municipality.clone(),
        sigpac_aggregate: es.sigpac_aggregate.clone(),
        sigpac_zone: es.sigpac_zone.clone(),
        sigpac_polygon: es.sigpac_polygon.clone(),
        sigpac_parcel: es.sigpac_parcel.clone(),
        sigpac_enclosure: es.sigpac_enclosure.clone(),
    };
    tx.execute(
        "INSERT INTO plot_es_extension
           (plot_id, sigpac_province, sigpac_municipality, sigpac_aggregate, sigpac_zone,
            sigpac_polygon, sigpac_parcel, sigpac_enclosure)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            ext.plot_id,
            ext.sigpac_province,
            ext.sigpac_municipality,
            ext.sigpac_aggregate,
            ext.sigpac_zone,
            ext.sigpac_polygon,
            ext.sigpac_parcel,
            ext.sigpac_enclosure
        ],
    )?;
    log_insert(tx, "plot_es_extension", plot_id, None, actor, &ext)?;
    Ok(ext)
}

fn get_plot_extension(conn: &Connection, plot_id: &str) -> Result<Option<PlotEsExtension>> {
    Ok(conn
        .query_row(
            "SELECT * FROM plot_es_extension WHERE plot_id = ?1",
            [plot_id],
            map_plot_extension,
        )
        .optional()?)
}

fn reconcile_plot_extension(
    tx: &Transaction,
    plot_id: &str,
    desired: Option<PlotEsFields>,
    actor: Option<&str>,
) -> Result<Option<PlotEsExtension>> {
    let current = tx
        .query_row(
            "SELECT * FROM plot_es_extension WHERE plot_id = ?1",
            [plot_id],
            map_plot_extension,
        )
        .optional()?;
    match (current, desired) {
        (None, None) => Ok(None),
        (None, Some(es)) => Ok(Some(insert_plot_extension(tx, plot_id, &es, actor)?)),
        (Some(before), None) => {
            tx.execute(
                "DELETE FROM plot_es_extension WHERE plot_id = ?1",
                [plot_id],
            )?;
            log_delete(tx, "plot_es_extension", plot_id, None, actor, &before, None)?;
            Ok(None)
        }
        (Some(before), Some(es)) => {
            let after = PlotEsExtension {
                plot_id: plot_id.to_string(),
                sigpac_province: es.sigpac_province,
                sigpac_municipality: es.sigpac_municipality,
                sigpac_aggregate: es.sigpac_aggregate,
                sigpac_zone: es.sigpac_zone,
                sigpac_polygon: es.sigpac_polygon,
                sigpac_parcel: es.sigpac_parcel,
                sigpac_enclosure: es.sigpac_enclosure,
            };
            tx.execute(
                "UPDATE plot_es_extension
                 SET sigpac_province = ?2, sigpac_municipality = ?3, sigpac_aggregate = ?4,
                     sigpac_zone = ?5, sigpac_polygon = ?6, sigpac_parcel = ?7, sigpac_enclosure = ?8
                 WHERE plot_id = ?1",
                params![
                    plot_id, after.sigpac_province, after.sigpac_municipality, after.sigpac_aggregate,
                    after.sigpac_zone, after.sigpac_polygon, after.sigpac_parcel, after.sigpac_enclosure
                ],
            )?;
            log_update(
                tx,
                "plot_es_extension",
                plot_id,
                None,
                actor,
                &before,
                &after,
            )?;
            Ok(Some(after))
        }
    }
}

// ---------------------------------------------------------------------------
// Validation + row mappers
// ---------------------------------------------------------------------------

fn validate_area(area_ha: Option<f64>) -> Result<()> {
    if let Some(area) = area_ha
        && area <= 0.0
    {
        return Err(CoreError::Invalid("nonpositive_area"));
    }
    Ok(())
}

fn map_farm(row: &Row) -> rusqlite::Result<Farm> {
    Ok(Farm {
        id: row.get("id")?,
        name: row.get("name")?,
        owner_name: row.get("owner_name")?,
        owner_tax_id: row.get("owner_tax_id")?,
        location_text: row.get("location_text")?,
        latitude: row.get("latitude")?,
        longitude: row.get("longitude")?,
        country_code: row.get("country_code")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_plot(row: &Row) -> rusqlite::Result<Plot> {
    Ok(Plot {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        name: row.get("name")?,
        area_ha: row.get("area_ha")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_farm_extension(row: &Row) -> rusqlite::Result<FarmEsExtension> {
    Ok(FarmEsExtension {
        farm_id: row.get("farm_id")?,
        rega_code: row.get("rega_code")?,
        rea_code: row.get("rea_code")?,
        province_code: row.get("province_code")?,
    })
}

fn map_plot_extension(row: &Row) -> rusqlite::Result<PlotEsExtension> {
    Ok(PlotEsExtension {
        plot_id: row.get("plot_id")?,
        sigpac_province: row.get("sigpac_province")?,
        sigpac_municipality: row.get("sigpac_municipality")?,
        sigpac_aggregate: row.get("sigpac_aggregate")?,
        sigpac_zone: row.get("sigpac_zone")?,
        sigpac_polygon: row.get("sigpac_polygon")?,
        sigpac_parcel: row.get("sigpac_parcel")?,
        sigpac_enclosure: row.get("sigpac_enclosure")?,
    })
}
