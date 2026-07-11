// SPDX-License-Identifier: AGPL-3.0-or-later

//! Base-map and overlay source registry — data, not code (the `nav.js`
//! philosophy). Adding a source is a new entry here.
//!
//! Service-selection rule (2026-07-07): when a provider offers
//! several services for the same data, pick the most modern and
//! bandwidth-frugal — vector tiles (MVT) > WMTS > WMS; WMS only as last
//! resort. Hence PNOA over WMTS here, not SIGPAC's WMS orthophoto — and
//! SIGPAC's recinto boundaries over MVT, not its WMS rendering.

/// One tile source the `geo://tiles/{id}/{z}/{x}/{y}` protocol path can serve.
pub struct TileSource {
    pub id: &'static str,
    /// Upstream URL template with `{z}`/`{x}`/`{y}` placeholders, or `None`
    /// when the template is resolved at runtime from a TileJSON document
    /// (see `tilejson_url`).
    pub url_template: Option<&'static str>,
    /// TileJSON document to resolve the current template from (OpenFreeMap
    /// publishes dated snapshot paths that rotate — the cache re-resolves on
    /// a 404, see `fetch`).
    pub tilejson_url: Option<&'static str>,
    /// Content type served when the upstream response does not say.
    pub content_type: &'static str,
    /// Zoom levels the upstream actually serves; outside them the protocol
    /// answers 404 and MapLibre falls back to over/underzoom per its source
    /// spec (the frontend mirrors these bounds there).
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// Attribution the style builder (base maps) or the overlay source spec
    /// (frontend) injects so MapLibre's control shows it.
    pub attribution: &'static str,
    /// Campaign-keyed cache rows (`{id}@{campaign}`): the upstream URL always
    /// serves the *current* SIGPAC campaign with no year in the template
    /// (checked 2026-07-11), so the cache must version itself or tiles from
    /// different campaigns would silently mix after the ~February rollover.
    pub campaign_keyed: bool,
    /// Upstream answers 404 for tiles with no features (SIGPAC MVT, verified
    /// 2026-07-11). Cache and serve them as empty payloads: an empty body is
    /// a valid empty vector tile, re-fetching known-empty countryside on
    /// every pan is impolite, and offline they must not read as errors.
    pub empty_on_404: bool,
}

/// A non-tile resource family the `geo://res/{prefix}/{rest}` path can serve.
/// Only allowlisted prefixes are proxied — the webview cannot reach arbitrary
/// hosts through the protocol.
pub struct ResourceBase {
    pub prefix: &'static str,
    /// Upstream base; `{rest}` (may be empty) is appended after it.
    pub base_url: &'static str,
    pub content_type: &'static str,
}

pub const TILE_SOURCES: &[TileSource] = &[
    TileSource {
        id: "openfreemap",
        url_template: None,
        tilejson_url: Some("https://tiles.openfreemap.org/planet"),
        content_type: "application/x-protobuf",
        min_zoom: 0,
        max_zoom: 14,
        attribution: "© OpenFreeMap contributors, data © OpenStreetMap",
        campaign_keyed: false,
        empty_on_404: false,
    },
    // The liberty style's low-zoom shaded-relief backdrop.
    TileSource {
        id: "openfreemap-ne2",
        url_template: Some("https://tiles.openfreemap.org/natural_earth/ne2sr/{z}/{x}/{y}.png"),
        tilejson_url: None,
        content_type: "image/png",
        min_zoom: 0,
        max_zoom: 6,
        attribution: "© OpenFreeMap contributors",
        campaign_keyed: false,
        empty_on_404: false,
    },
    // IGN's PNOA orthophoto over WMTS (KVP GetTile, GoogleMapsCompatible
    // matrix set = standard XYZ addressing; TILEROW is the XYZ y).
    TileSource {
        id: "pnoa",
        url_template: Some(
            "https://www.ign.es/wmts/pnoa-ma?service=WMTS&request=GetTile&version=1.0.0\
             &layer=OI.OrthoimageCoverage&style=default&format=image/jpeg\
             &tilematrixset=GoogleMapsCompatible&tilematrix={z}&tilerow={y}&tilecol={x}",
        ),
        tilejson_url: None,
        content_type: "image/jpeg",
        min_zoom: 0,
        max_zoom: 19,
        attribution: "PNOA cedido por © Instituto Geográfico Nacional",
        campaign_keyed: false,
        empty_on_404: false,
    },
    // SIGPAC recinto boundaries, Nube de SIGPAC MVT service (pbf z12–15,
    // EPSG:3857; single source-layer "recinto", inspected 2026-07-11). The
    // fixed URL always serves the current campaign — hence campaign_keyed —
    // and tiles with no recintos answer 404 — hence empty_on_404.
    TileSource {
        id: "sigpac-recintos",
        url_template: Some("https://sigpac-hubcloud.es/mvt/recinto@3857@pbf/{z}/{x}/{y}.pbf"),
        tilejson_url: None,
        content_type: "application/x-protobuf",
        min_zoom: 12,
        max_zoom: 15,
        attribution: "SIGPAC © FEGA (CC BY 4.0)",
        campaign_keyed: true,
        empty_on_404: true,
    },
];

pub const RESOURCE_BASES: &[ResourceBase] = &[
    ResourceBase {
        prefix: "ofm-style",
        base_url: "https://tiles.openfreemap.org/styles/liberty",
        content_type: "application/json",
    },
    ResourceBase {
        prefix: "ofm-fonts",
        base_url: "https://tiles.openfreemap.org/fonts/",
        content_type: "application/x-protobuf",
    },
    ResourceBase {
        prefix: "ofm-sprites",
        base_url: "https://tiles.openfreemap.org/sprites/",
        content_type: "application/octet-stream",
    },
];

pub fn tile_source(id: &str) -> Option<&'static TileSource> {
    TILE_SOURCES.iter().find(|s| s.id == id)
}

pub fn resource_base(prefix: &str) -> Option<&'static ResourceBase> {
    RESOURCE_BASES.iter().find(|r| r.prefix == prefix)
}
