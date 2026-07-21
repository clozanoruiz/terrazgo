// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The composed SIGPAC operations the shell's commands wrap: lookup (by
//! reference or point, with the dedup check), and verify-and-store for an
//! existing plot. Composition lives here, not in commands, so it is testable
//! offline against a pre-seeded cache (docs/architecture.md → Testing strategy #4).

use crate::client;
use crate::models::RecintoInfo;
use crate::reference::SigpacRef;
use crate::storage::{self, PlotMatch};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use terrazgo_core::models::{GeoFeature, ZoneFlag};
use terrazgo_geo::{GeoError, Result};

/// A recinto looked up for entry/prefill, with the plots that already carry
/// its reference — the UI offers "attach to existing" over duplicating.
#[derive(Debug, Serialize)]
pub struct RecintoLookup {
    pub recinto: RecintoInfo,
    pub matching_plots: Vec<PlotMatch>,
}

/// A verified plot: what SIGPAC said, the `geo_feature` row it was stored as
/// (replacing this source's previous row — history soft-deleted), and the
/// zone-check results. The boundary is the primary outcome: if the zone
/// checks fail AFTER it stored (network flake, campaign listing down),
/// `zone_flags` is `None` and `zone_check_error` says why — the caller
/// surfaces "zones unchecked, retry", never loses the stored boundary.
#[derive(Debug, Serialize)]
pub struct PlotVerification {
    pub recinto: RecintoInfo,
    pub feature: GeoFeature,
    pub zone_flags: Option<Vec<ZoneFlag>>,
    pub zone_check_error: Option<String>,
}

/// Door A while typing: look a reference up for form prefill. Stores nothing
/// (the plot may not exist yet); `Ok(None)` = SIGPAC does not know the ref.
pub fn lookup_reference(
    app: &Connection,
    cache: &Mutex<Connection>,
    parts: &[String],
    refresh: bool,
) -> Result<Option<RecintoLookup>> {
    let parts: [&str; 7] = parts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| GeoError::Invalid("sigpac_ref_invalid"))?;
    let reference = SigpacRef::from_parts(parts)?;
    lookup(app, cache, |cache| {
        client::recinto_by_reference(cache, &reference, refresh)
    })
}

/// Door B: the recinto under a map click (later: the GPS position).
pub fn lookup_point(
    app: &Connection,
    cache: &Mutex<Connection>,
    lon: f64,
    lat: f64,
) -> Result<Option<RecintoLookup>> {
    lookup(app, cache, |cache| {
        client::recinto_by_point(cache, lon, lat)
    })
}

/// Verify an existing plot against SIGPAC using its stored reference:
/// persist the official boundary, then run the zone checks (folded into
/// verification per the 2026-07-08 decision — one tap covers both).
/// `Ok(None)` = reference unknown to SIGPAC (typo or outdated) — nothing is
/// stored, the plot is untouched.
pub fn verify_plot(
    app: &mut Connection,
    cache: &Mutex<Connection>,
    plot_id: &str,
    refresh: bool,
    actor: Option<&str>,
) -> Result<Option<PlotVerification>> {
    let reference = storage::plot_reference(app, plot_id)?;
    let Some(recinto) = client::recinto_by_reference(cache, &reference, refresh)? else {
        return Ok(None);
    };
    let feature = storage::save_recinto_boundary(app, plot_id, &recinto, actor)?;
    let (zone_flags, zone_check_error) =
        match check_zones(app, cache, plot_id, &reference, refresh, actor) {
            Ok(flags) => (Some(flags), None),
            // The boundary is already stored; a zone failure must not undo that.
            Err(err) => (None, Some(format!("{err}"))),
        };
    Ok(Some(PlotVerification {
        recinto,
        feature,
        zone_flags,
        zone_check_error,
    }))
}

/// The three zone-layer checks for one recinto, stored replace-within-campaign.
fn check_zones(
    app: &mut Connection,
    cache: &Mutex<Connection>,
    plot_id: &str,
    reference: &SigpacRef,
    refresh: bool,
    actor: Option<&str>,
) -> Result<Vec<ZoneFlag>> {
    let campaign = client::current_campaign(cache, refresh)?;
    let mut results = Vec::with_capacity(client::ZONE_LAYERS.len());
    for (zone_type_code, layer) in client::ZONE_LAYERS {
        let intersection = client::zone_intersection(cache, reference, layer, refresh)?;
        results.push((*zone_type_code, intersection));
    }
    storage::save_zone_checks(app, plot_id, campaign, results, actor)
}

/// Shared lookup shape: fetch, then attach the dedup matches.
fn lookup<F>(app: &Connection, cache: &Mutex<Connection>, fetch: F) -> Result<Option<RecintoLookup>>
where
    F: FnOnce(&Mutex<Connection>) -> Result<Option<RecintoInfo>>,
{
    let Some(recinto) = fetch(cache)? else {
        return Ok(None);
    };
    let matching_plots = storage::find_plots_with_reference(app, &recinto.reference)?;
    Ok(Some(RecintoLookup {
        recinto,
        matching_plots,
    }))
}
