// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reads and writes against the app database (core tables): the plot's stored
//! SIGPAC reference, persisting a fetched recinto as a `geo_feature` row, and
//! the dedup query behind "this recinto is already one of your plots".
//!
//! All writes go through core's `save_geo_feature` — replace-within-source
//! semantics, audit logging and GeoJSON validation come from there; this file
//! only shapes SIGPAC data into core inputs.

use crate::models::{RecintoInfo, ZoneIntersection};
use crate::reference::SigpacRef;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::models::{GeoFeature, NewGeoFeature, NewZoneFlag, ZoneFlag};
use terrazgo_core::repository::{replace_zone_flags, save_geo_feature};
use terrazgo_geo::{GeoError, Result};

/// The `geo_feature.source` tag for provider-fetched SIGPAC geometry. A row
/// with this source coexists with `manual`/`import` rows on the same plot —
/// that pairing is the discrepancy display, not a conflict.
pub const SOURCE: &str = "sigpac";

/// An active plot whose stored SIGPAC reference equals a looked-up recinto —
/// the UI offers "attach to this plot" instead of creating a duplicate.
#[derive(Debug, Serialize)]
pub struct PlotMatch {
    pub plot_id: String,
    pub plot_name: String,
    pub farm_id: String,
    pub farm_name: String,
}

/// The SIGPAC reference stored on a plot's ES extension. `NotFound` if the
/// plot does not exist (or is deleted); `sigpac_ref_missing` if it has no
/// extension or any of the seven parts is empty — the form must be completed
/// before verification makes sense.
pub fn plot_reference(conn: &Connection, plot_id: &str) -> Result<SigpacRef> {
    conn.query_row(
        "SELECT 1 FROM plot WHERE id = ?1 AND deleted_at IS NULL",
        [plot_id],
        |_| Ok(()),
    )
    .optional()?
    .ok_or(GeoError::NotFound)?;

    let parts: Option<[Option<String>; 7]> = conn
        .query_row(
            "SELECT sigpac_province, sigpac_municipality, sigpac_aggregate, sigpac_zone,
                    sigpac_polygon, sigpac_parcel, sigpac_enclosure
             FROM plot_es_extension WHERE plot_id = ?1",
            [plot_id],
            |row| {
                Ok([
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ])
            },
        )
        .optional()?;
    let parts = parts.ok_or(GeoError::Invalid("sigpac_ref_missing"))?;
    reference_from_columns(&parts).ok_or(GeoError::Invalid("sigpac_ref_missing"))?
}

/// Persist a fetched recinto as the plot's SIGPAC boundary. The full
/// attribute set lands source-tagged in `properties`; the official surface
/// goes to `official_area_ha` and NEVER touches `plot.area_ha` (the user's
/// declared value — the difference is the discrepancy display). `campaign`
/// stays NULL for now: the consultas endpoints serve the current campaign
/// without naming it; tagging arrives with the code-lists service.
pub fn save_recinto_boundary(
    conn: &mut Connection,
    plot_id: &str,
    recinto: &RecintoInfo,
) -> Result<GeoFeature> {
    let properties = serde_json::to_string(&recinto.properties)?;
    Ok(save_geo_feature(
        conn,
        NewGeoFeature {
            plot_id: Some(plot_id.to_string()),
            farm_id: None,
            role: "boundary".into(),
            geometry: recinto.geometry.to_string(),
            source: SOURCE.into(),
            campaign: None,
            official_area_ha: recinto.surface_ha(),
            properties: Some(properties),
            fetched_at: Some(now_utc_iso()),
        },
    )?)
}

/// Persist one check run's zone results (all layers, one transaction) via
/// core's replace-within-campaign semantics. `None` intersection = outside —
/// stored as proof the check ran and was clear.
pub fn save_zone_checks(
    conn: &mut Connection,
    plot_id: &str,
    campaign: i64,
    results: Vec<(&'static str, Option<ZoneIntersection>)>,
) -> Result<Vec<ZoneFlag>> {
    let flags = results
        .into_iter()
        .map(|(zone_type_code, intersection)| match intersection {
            Some(zone) => NewZoneFlag {
                zone_type_code: zone_type_code.into(),
                status: "inside".into(),
                coverage_pct: Some(zone.surface_tpc),
                detail: zone.descripcion,
            },
            None => NewZoneFlag {
                zone_type_code: zone_type_code.into(),
                status: "outside".into(),
                coverage_pct: None,
                detail: None,
            },
        })
        .collect();
    Ok(replace_zone_flags(conn, plot_id, campaign, SOURCE, flags)?)
}

/// Every active plot whose stored reference equals `reference`. Comparison is
/// numeric ("05" equals "5" — the parts are TEXT columns typed by hand), done
/// in Rust over the handful of extension rows a farm installation holds.
pub fn find_plots_with_reference(
    conn: &Connection,
    reference: &SigpacRef,
) -> Result<Vec<PlotMatch>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, f.id, f.name,
                e.sigpac_province, e.sigpac_municipality, e.sigpac_aggregate, e.sigpac_zone,
                e.sigpac_polygon, e.sigpac_parcel, e.sigpac_enclosure
         FROM plot p
         JOIN farm f ON f.id = p.farm_id AND f.deleted_at IS NULL
         JOIN plot_es_extension e ON e.plot_id = p.id
         WHERE p.deleted_at IS NULL
         ORDER BY p.id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let parts: [Option<String>; 7] = [
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ];
            Ok((
                PlotMatch {
                    plot_id: row.get(0)?,
                    plot_name: row.get(1)?,
                    farm_id: row.get(2)?,
                    farm_name: row.get(3)?,
                },
                parts,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows
        .into_iter()
        .filter(|(_, parts)| {
            // A malformed or incomplete stored ref simply never matches.
            matches!(reference_from_columns(parts), Some(Ok(stored)) if stored == *reference)
        })
        .map(|(plot, _)| plot)
        .collect())
}

/// The seven extension columns as a `SigpacRef`: `None` when any part is
/// missing/blank, `Some(Err(_))` when present but not parseable.
fn reference_from_columns(parts: &[Option<String>; 7]) -> Option<Result<SigpacRef>> {
    let filled: Vec<&str> = parts
        .iter()
        .filter_map(|part| part.as_deref())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let complete: [&str; 7] = filled.try_into().ok()?;
    Some(SigpacRef::from_parts(complete))
}
