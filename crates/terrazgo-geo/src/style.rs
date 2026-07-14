// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MapLibre style building. The webview must only ever see `geo://` URLs
//! (production CSP stays `default-src 'self'` + the custom scheme), so
//! upstream styles are rewritten: every tile/glyph/sprite reference is mapped
//! onto the protocol paths served by [`crate::fetch`], and TileJSON
//! indirections are resolved server-side.
//!
//! `base` is the platform form of the protocol origin (Linux/macOS
//! `geo://localhost/`, Windows/Android `http://geo.localhost/`) — the
//! frontend computes it and passes it in, keeping this crate platform-blind.

use crate::error::{GeoError, Result};
use crate::fetch::{self, Fetched};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::Mutex;

const OFM_NE2_PREFIX: &str = "https://tiles.openfreemap.org/natural_earth/ne2sr/";
const OFM_TILEJSON_URL: &str = "https://tiles.openfreemap.org/planet";
const OFM_SPRITES_PREFIX: &str = "https://tiles.openfreemap.org/sprites/";
const OFM_ATTRIBUTION: &str = "© OpenFreeMap contributors, data © OpenStreetMap";

/// The OpenFreeMap "liberty" style, rewritten onto `geo://`. Fetched through
/// the cache, so once seen it also works offline.
pub fn openfreemap_style(cache: &Mutex<Connection>, base: &str) -> Result<String> {
    let raw = fetch::resource(cache, "ofm-style", "")?;
    let mut style: Value = serde_json::from_slice(&raw.data)?;
    rewrite_openfreemap_style(&mut style, base, |url| {
        let Fetched { data, .. } = fetch::cached_resource(
            cache,
            "tilejson/openfreemap",
            url,
            "application/json",
            false,
        )?;
        Ok(serde_json::from_slice(&data)?)
    })?;
    Ok(style.to_string())
}

/// Minimal raster style for the PNOA orthophoto (built locally — no upstream
/// style exists; raster styles need no glyphs or sprites).
pub fn pnoa_style(base: &str) -> String {
    json!({
        "version": 8,
        "name": "PNOA",
        "sources": {
            "pnoa": {
                "type": "raster",
                "tiles": [format!("{base}tiles/pnoa/{{z}}/{{x}}/{{y}}")],
                "tileSize": 256,
                "maxzoom": 19,
                "attribution": "PNOA cedido por © Instituto Geográfico Nacional"
            }
        },
        "layers": [
            { "id": "pnoa", "type": "raster", "source": "pnoa" }
        ]
    })
    .to_string()
}

/// Rewrite an OpenFreeMap style in place. Fails loudly (`style_unsupported`)
/// on any source it does not recognise: an unrewritten external URL would be
/// silently blocked by the CSP and appear as blank tiles — a hard error at
/// build time is diagnosable, that is not.
fn rewrite_openfreemap_style(
    style: &mut Value,
    base: &str,
    resolve_tilejson: impl Fn(&str) -> Result<Value>,
) -> Result<()> {
    style["glyphs"] = json!(format!("{base}res/ofm-fonts/{{fontstack}}/{{range}}.pbf"));

    if let Some(sprite) = style.get("sprite").and_then(Value::as_str) {
        let rest = sprite
            .strip_prefix(OFM_SPRITES_PREFIX)
            .ok_or(GeoError::Invalid("style_unsupported"))?;
        style["sprite"] = json!(format!("{base}res/ofm-sprites/{rest}"));
    }

    let sources = style
        .get_mut("sources")
        .and_then(Value::as_object_mut)
        .ok_or(GeoError::Invalid("style_unsupported"))?;

    for source in sources.values_mut() {
        let kind = source.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "raster" => {
                let first_tile = source["tiles"].get(0).and_then(Value::as_str).unwrap_or("");
                if !first_tile.starts_with(OFM_NE2_PREFIX) {
                    return Err(GeoError::Invalid("style_unsupported"));
                }
                source["tiles"] = json!([format!("{base}tiles/openfreemap-ne2/{{z}}/{{x}}/{{y}}")]);
                source["attribution"] = json!(OFM_ATTRIBUTION);
            }
            "vector" => {
                let url = source.get("url").and_then(Value::as_str).unwrap_or("");
                if url != OFM_TILEJSON_URL {
                    return Err(GeoError::Invalid("style_unsupported"));
                }
                let tilejson = resolve_tilejson(url)?;
                if let Some(obj) = source.as_object_mut() {
                    obj.remove("url");
                    obj.insert(
                        "tiles".into(),
                        json!([format!("{base}tiles/openfreemap/{{z}}/{{x}}/{{y}}")]),
                    );
                    obj.insert("minzoom".into(), tilejson["minzoom"].clone());
                    obj.insert("maxzoom".into(), tilejson["maxzoom"].clone());
                    obj.insert("attribution".into(), json!(OFM_ATTRIBUTION));
                }
            }
            _ => return Err(GeoError::Invalid("style_unsupported")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the shape of the real liberty style (fetched 2026-07-07):
    /// glyph/sprite templates plus one raster (direct tiles) and one vector
    /// (TileJSON url) source.
    fn fixture_style() -> Value {
        json!({
            "version": 8,
            "glyphs": "https://tiles.openfreemap.org/fonts/{fontstack}/{range}.pbf",
            "sprite": "https://tiles.openfreemap.org/sprites/ofm_f384/ofm",
            "sources": {
                "ne2_shaded": {
                    "type": "raster",
                    "maxzoom": 6,
                    "tileSize": 256,
                    "tiles": ["https://tiles.openfreemap.org/natural_earth/ne2sr/{z}/{x}/{y}.png"]
                },
                "openmaptiles": {
                    "type": "vector",
                    "url": "https://tiles.openfreemap.org/planet"
                }
            },
            "layers": []
        })
    }

    fn fake_tilejson(_url: &str) -> Result<Value> {
        Ok(json!({
            "tiles": ["https://tiles.openfreemap.org/planet/20260621_080001_pt/{z}/{x}/{y}.pbf"],
            "minzoom": 0,
            "maxzoom": 14
        }))
    }

    #[test]
    fn rewrites_every_external_reference_onto_the_protocol() {
        let mut style = fixture_style();
        rewrite_openfreemap_style(&mut style, "geo://localhost/", fake_tilejson).unwrap();

        assert_eq!(
            style["glyphs"],
            "geo://localhost/res/ofm-fonts/{fontstack}/{range}.pbf"
        );
        assert_eq!(
            style["sprite"],
            "geo://localhost/res/ofm-sprites/ofm_f384/ofm"
        );
        assert_eq!(
            style["sources"]["ne2_shaded"]["tiles"][0],
            "geo://localhost/tiles/openfreemap-ne2/{z}/{x}/{y}"
        );
        let omt = &style["sources"]["openmaptiles"];
        assert!(omt.get("url").is_none());
        assert_eq!(
            omt["tiles"][0],
            "geo://localhost/tiles/openfreemap/{z}/{x}/{y}"
        );
        assert_eq!(omt["maxzoom"], 14);

        // Nothing in the final style may point at an external origin.
        let rendered = style.to_string();
        assert!(!rendered.contains("https://"));
    }

    #[test]
    fn windows_style_base_works_too() {
        let mut style = fixture_style();
        rewrite_openfreemap_style(&mut style, "http://geo.localhost/", fake_tilejson).unwrap();
        assert_eq!(
            style["sources"]["openmaptiles"]["tiles"][0],
            "http://geo.localhost/tiles/openfreemap/{z}/{x}/{y}"
        );
    }

    #[test]
    fn unknown_source_fails_loudly() {
        let mut style = fixture_style();
        style["sources"]["rogue"] = json!({
            "type": "vector",
            "url": "https://evil.example.com/tiles"
        });
        assert!(matches!(
            rewrite_openfreemap_style(&mut style, "geo://localhost/", fake_tilejson),
            Err(GeoError::Invalid("style_unsupported"))
        ));
    }

    #[test]
    fn pnoa_style_is_self_contained() {
        let style: Value = serde_json::from_str(&pnoa_style("geo://localhost/")).unwrap();
        assert_eq!(
            style["sources"]["pnoa"]["tiles"][0],
            "geo://localhost/tiles/pnoa/{z}/{x}/{y}"
        );
        assert!(!style.to_string().contains("https://"));
    }
}
