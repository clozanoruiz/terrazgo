// SPDX-License-Identifier: AGPL-3.0-or-later

//! Minimal GeoJSON *geometry* validation (RFC 7946) for boundary geometries.
//!
//! The `geo_feature` write path must reject malformed geometry at insert time —
//! invariants live next to the write they guard, like `validate_name`. This is
//! pure `serde_json` parsing (no geo crates, no I/O), which is why it sits in
//! core, the bottom of the dependency stack: `terrazgo-geo` and the shell reuse
//! it through their existing dependency on this crate.
//!
//! Accepted: a bare GeoJSON geometry object of type `Polygon` or `MultiPolygon`,
//! coordinates in EPSG:4326 lon/lat order. Features/FeatureCollections are NOT
//! accepted here — file import normalises them to bare geometries first.

use crate::error::{CoreError, Result};
use serde_json::Value;

/// Stable machine code rendered by the frontend as `error.invalid.geometry_invalid`.
const INVALID: CoreError = CoreError::Invalid("geometry_invalid");

/// Validate a GeoJSON geometry string (Polygon/MultiPolygon, closed rings of
/// ≥ 4 positions, longitude ∈ [-180, 180], latitude ∈ [-90, 90]).
pub fn validate_boundary_geometry(geometry: &str) -> Result<()> {
    let value: Value = serde_json::from_str(geometry).map_err(|_| INVALID)?;
    validate_boundary_geometry_value(&value)
}

/// Same validation for an already-parsed JSON value (the import path has one).
pub fn validate_boundary_geometry_value(value: &Value) -> Result<()> {
    let obj = value.as_object().ok_or(INVALID)?;
    let coordinates = obj.get("coordinates").ok_or(INVALID)?;
    match obj.get("type").and_then(Value::as_str) {
        Some("Polygon") => validate_polygon(coordinates),
        Some("MultiPolygon") => {
            let polygons = coordinates.as_array().ok_or(INVALID)?;
            if polygons.is_empty() {
                return Err(INVALID);
            }
            polygons.iter().try_for_each(validate_polygon)
        }
        _ => Err(INVALID),
    }
}

fn validate_polygon(coordinates: &Value) -> Result<()> {
    let rings = coordinates.as_array().ok_or(INVALID)?;
    if rings.is_empty() {
        return Err(INVALID);
    }
    rings.iter().try_for_each(validate_ring)
}

/// A linear ring: ≥ 4 positions, closed (first == last, per RFC 7946 §3.1.6).
fn validate_ring(ring: &Value) -> Result<()> {
    let positions = ring.as_array().ok_or(INVALID)?;
    if positions.len() < 4 {
        return Err(INVALID);
    }
    for position in positions {
        validate_position(position)?;
    }
    let first = lon_lat(&positions[0])?;
    let last = lon_lat(&positions[positions.len() - 1])?;
    if first != last {
        return Err(INVALID);
    }
    Ok(())
}

/// A position: [lon, lat] or [lon, lat, elevation], with lon/lat in range.
fn validate_position(position: &Value) -> Result<()> {
    let coords = position.as_array().ok_or(INVALID)?;
    if !(2..=3).contains(&coords.len()) {
        return Err(INVALID);
    }
    let (lon, lat) = lon_lat(position)?;
    if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
        return Err(INVALID);
    }
    // The optional third element only needs to be numeric.
    if let Some(elevation) = coords.get(2)
        && elevation.as_f64().is_none()
    {
        return Err(INVALID);
    }
    Ok(())
}

fn lon_lat(position: &Value) -> Result<(f64, f64)> {
    let coords = position.as_array().ok_or(INVALID)?;
    let lon = coords.first().and_then(Value::as_f64).ok_or(INVALID)?;
    let lat = coords.get(1).and_then(Value::as_f64).ok_or(INVALID)?;
    Ok((lon, lat))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_invalid(geometry: &str) {
        match validate_boundary_geometry(geometry) {
            Err(CoreError::Invalid("geometry_invalid")) => {}
            other => panic!("expected Invalid(geometry_invalid), got {other:?}"),
        }
    }

    // A small square near Valladolid (dev/testing region), lon/lat order.
    const SQUARE: &str = r#"{"type":"Polygon","coordinates":[
        [[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.66],[-4.72,41.65]]
    ]}"#;

    #[test]
    fn accepts_valid_polygon() {
        assert!(validate_boundary_geometry(SQUARE).is_ok());
    }

    #[test]
    fn accepts_polygon_with_hole() {
        let geometry = r#"{"type":"Polygon","coordinates":[
            [[-4.72,41.65],[-4.70,41.65],[-4.70,41.67],[-4.72,41.67],[-4.72,41.65]],
            [[-4.715,41.655],[-4.705,41.655],[-4.705,41.665],[-4.715,41.665],[-4.715,41.655]]
        ]}"#;
        assert!(validate_boundary_geometry(geometry).is_ok());
    }

    #[test]
    fn accepts_multipolygon_and_3d_positions() {
        let geometry = r#"{"type":"MultiPolygon","coordinates":[
            [[[-4.72,41.65,701.2],[-4.71,41.65,700.0],[-4.71,41.66,699.8],[-4.72,41.65,701.2]]],
            [[[-4.60,41.60],[-4.59,41.60],[-4.59,41.61],[-4.60,41.60]]]
        ]}"#;
        assert!(validate_boundary_geometry(geometry).is_ok());
    }

    #[test]
    fn rejects_non_polygon_geometry_types() {
        assert_invalid(r#"{"type":"Point","coordinates":[-4.72,41.65]}"#);
        assert_invalid(r#"{"type":"LineString","coordinates":[[-4.72,41.65],[-4.71,41.65]]}"#);
    }

    #[test]
    fn rejects_feature_wrappers() {
        // Features must be unwrapped by the import layer before reaching here.
        let feature = format!(r#"{{"type":"Feature","properties":{{}},"geometry":{SQUARE}}}"#);
        assert_invalid(&feature);
        let collection = format!(
            r#"{{"type":"FeatureCollection","features":[{{"type":"Feature","geometry":{SQUARE}}}]}}"#
        );
        assert_invalid(&collection);
    }

    #[test]
    fn rejects_unclosed_ring() {
        assert_invalid(
            r#"{"type":"Polygon","coordinates":[
                [[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.66]]
            ]}"#,
        );
    }

    #[test]
    fn rejects_ring_with_fewer_than_four_positions() {
        assert_invalid(
            r#"{"type":"Polygon","coordinates":[
                [[-4.72,41.65],[-4.71,41.65],[-4.72,41.65]]
            ]}"#,
        );
    }

    #[test]
    fn rejects_out_of_range_coordinates() {
        // Longitude beyond ±180 — the classic symptom of projected (UTM) input,
        // e.g. an EPSG:25830 GPKG passed through without reprojection.
        assert_invalid(
            r#"{"type":"Polygon","coordinates":[
                [[355000.0,4612000.0],[355100.0,4612000.0],[355100.0,4612100.0],[355000.0,4612000.0]]
            ]}"#,
        );
        // Latitude beyond ±90 (lon/lat order swapped by mistake would hit this too).
        assert_invalid(
            r#"{"type":"Polygon","coordinates":[
                [[41.65,-94.72],[41.66,-94.72],[41.66,-94.71],[41.65,-94.72]]
            ]}"#,
        );
    }

    #[test]
    fn rejects_malformed_structures() {
        assert_invalid("not json at all");
        assert_invalid(r#"{"type":"Polygon"}"#); // missing coordinates
        assert_invalid(r#"{"type":"Polygon","coordinates":[]}"#); // no rings
        assert_invalid(r#"{"type":"MultiPolygon","coordinates":[]}"#); // no polygons
        assert_invalid(
            r#"{"type":"Polygon","coordinates":[[["a",41.65],["b",41.65],["c",41.66],["a",41.65]]]}"#,
        ); // non-numeric
        assert_invalid(r#"{"type":"Polygon","coordinates":[[[-4.72],[-4.71],[-4.71],[-4.72]]]}"#); // 1-element positions
    }
}
