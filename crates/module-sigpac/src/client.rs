// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recinto lookups against the Nube de SIGPAC consultas service, riding on
//! terrazgo-geo's cache-through fetch: a response seen once is served from
//! `geo-cache.db` forever after, so a verified plot stays verifiable offline.
//! `refresh` bypasses the cache read (user-triggered re-verification, e.g. at
//! campaign rollover) while still storing the new payload.

use crate::models::{
    RecintoInfo, ZoneIntersection, parse_intersection_response, parse_recinto_response,
};
use crate::reference::SigpacRef;
use rusqlite::Connection;
use std::sync::Mutex;
use terrazgo_geo::{Result, fetch};

/// The consultas base. Only this Rust code builds SIGPAC URLs — the webview
/// never sees the host (it talks `geo://`; production CSP stays closed).
const BASE_URL: &str = "https://sigpac-hubcloud.es/servicioconsultassigpac/query";
const INTERSECTION_URL: &str = "https://sigpac-hubcloud.es/servicioconsultassigpac/intersection";

/// The current SIGPAC campaign year. Moved into terrazgo-geo (2026-07-11)
/// because campaign-keyed tile caching (the MVT recinto overlay) needs it
/// below the module tier; re-exported so this crate's callers keep one entry
/// point for everything SIGPAC.
pub use terrazgo_geo::fetch::current_campaign;

/// The zone layers Terrazgo checks, as (zone_type code, service layer name).
/// Order is the storage/display order.
pub const ZONE_LAYERS: &[(&str, &str)] = &[
    ("nitrate_vulnerable", "nitratos"),
    ("phytosanitary_restriction", "fitosanitarios"),
    ("natura_2000", "red_natura"),
];

/// Cache key for a by-reference lookup in geo-cache.db's `resource` table.
/// Public so tests (and future cache maintenance) address the same row the
/// client writes.
pub fn recinfo_cache_key(reference: &SigpacRef) -> String {
    format!("sigpac/recinfo/{}", reference.to_path())
}

/// Look one recinto up by its 7-part reference. `Ok(None)` means SIGPAC does
/// not know the reference — the caller's "typo or outdated ref" signal.
pub fn recinto_by_reference(
    cache: &Mutex<Connection>,
    reference: &SigpacRef,
    refresh: bool,
) -> Result<Option<RecintoInfo>> {
    let url = format!("{BASE_URL}/recinfo/{}.geojson", reference.to_path());
    let fetched = fetch::cached_resource(
        cache,
        &recinfo_cache_key(reference),
        &url,
        "application/json",
        refresh,
    )?;
    parse_recinto_response(&fetched.data)
}

/// One zone-layer intersection for a recinto. `Ok(None)` = outside the layer
/// (the service answers `[]`). `layer` is the service name from [`ZONE_LAYERS`].
pub fn zone_intersection(
    cache: &Mutex<Connection>,
    reference: &SigpacRef,
    layer: &str,
    refresh: bool,
) -> Result<Option<ZoneIntersection>> {
    let key = format!("sigpac/intersection/{layer}/{}", reference.to_path());
    let url = format!("{INTERSECTION_URL}/{layer}/{}.json", reference.to_path());
    let fetched = fetch::cached_resource(cache, &key, &url, "application/json", refresh)?;
    parse_intersection_response(&fetched.data)
}

/// Look up the recinto under a geographic point (map click today, GPS
/// position later). Coordinates are cached verbatim — a repeated click on
/// the same stored feature works offline; arbitrary new points need network.
pub fn recinto_by_point(
    cache: &Mutex<Connection>,
    lon: f64,
    lat: f64,
) -> Result<Option<RecintoInfo>> {
    let key = format!("sigpac/recinfobypoint/{lon}/{lat}");
    let url = format!("{BASE_URL}/recinfobypoint/4326/{lon}/{lat}.geojson");
    let fetched = fetch::cached_resource(cache, &key, &url, "application/json", false)?;
    parse_recinto_response(&fetched.data)
}
