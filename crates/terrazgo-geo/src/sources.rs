// SPDX-License-Identifier: AGPL-3.0-or-later

//! Base-map source and resource registry — data, not code (the `nav.js`
//! philosophy). Adding a source is a new entry here; SIGPAC's MVT recintos
//! slot in as another entry when `module-sigpac` arrives.
//!
//! Service-selection rule (2026-07-07): when a provider offers
//! several services for the same data, pick the most modern and
//! bandwidth-frugal — vector tiles (MVT) > WMTS > WMS; WMS only as last
//! resort. Hence PNOA over WMTS here, not SIGPAC's WMS orthophoto.

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
    pub max_zoom: u8,
    /// Attribution the style builder injects so MapLibre's control shows it.
    pub attribution: &'static str,
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
        max_zoom: 14,
        attribution: "© OpenFreeMap contributors, data © OpenStreetMap",
    },
    // The liberty style's low-zoom shaded-relief backdrop.
    TileSource {
        id: "openfreemap-ne2",
        url_template: Some("https://tiles.openfreemap.org/natural_earth/ne2sr/{z}/{x}/{y}.png"),
        tilejson_url: None,
        content_type: "image/png",
        max_zoom: 6,
        attribution: "© OpenFreeMap contributors",
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
        max_zoom: 19,
        attribution: "PNOA cedido por © Instituto Geográfico Nacional",
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
