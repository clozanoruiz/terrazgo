// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a SIGPAC lookup returns, plus the response parsing shared by the
//! by-reference and by-point endpoints (both answer a GeoJSON
//! FeatureCollection with the same attribute set).

use crate::reference::SigpacRef;
use serde::Serialize;
use serde_json::{Map, Value};
use terrazgo_geo::{GeoError, Result};

/// One recinto as SIGPAC describes it. `geometry` is the GeoJSON geometry
/// exactly as returned (geographic lon/lat, ETRS89 ≡ WGS84 for our purpose);
/// `properties` keeps the full attribute set source-tagged and untyped — the
/// same shape `geo_feature.properties` stores, so a field is promoted to a
/// typed accessor only when the app actually reads it.
#[derive(Debug, Serialize)]
pub struct RecintoInfo {
    pub reference: SigpacRef,
    pub geometry: Value,
    pub properties: Map<String, Value>,
}

impl RecintoInfo {
    /// Official surface in hectares (`superficie` — verified against the
    /// intersection endpoints' m² figures, 2026-07-08).
    pub fn surface_ha(&self) -> Option<f64> {
        self.properties.get("superficie").and_then(Value::as_f64)
    }

    /// SIGPAC land-use code (`uso_sigpac`, e.g. `TA` tierra arable,
    /// `PA` pasto arbustivo) — a schema code, translated at display time.
    pub fn land_use(&self) -> Option<&str> {
        self.properties.get("uso_sigpac").and_then(Value::as_str)
    }
}

/// One zone-layer intersection as the service reports it: percentage of the
/// recinto inside the zone, plus an optional description ("Zona periférica").
#[derive(Debug, Clone, Serialize)]
pub struct ZoneIntersection {
    pub surface_tpc: f64,
    pub descripcion: Option<String>,
}

/// Parse an `intersection/{layer}` response. `[]` means the recinto does not
/// intersect the layer — a real, storable "outside" result, not an error.
pub fn parse_intersection_response(bytes: &[u8]) -> Result<Option<ZoneIntersection>> {
    let document: Value = serde_json::from_slice(bytes)?;
    let rows = document
        .as_array()
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let surface_tpc = row
        .get("surface_tpc")
        .and_then(Value::as_f64)
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    let descripcion = row
        .get("descripcion")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(Some(ZoneIntersection {
        surface_tpc,
        descripcion,
    }))
}

/// Parse a consultas `.geojson` response. An empty FeatureCollection is the
/// service's "no such recinto" (it never answers 404 — live-tested
/// 2026-07-08), hence `Ok(None)`. A recinto is a single feature; the
/// endpoints never return more than one.
pub fn parse_recinto_response(bytes: &[u8]) -> Result<Option<RecintoInfo>> {
    let document: Value = serde_json::from_slice(bytes)?;
    let features = document
        .get("features")
        .and_then(Value::as_array)
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    let Some(feature) = features.first() else {
        return Ok(None);
    };
    let geometry = feature
        .get("geometry")
        .filter(|geometry| geometry.is_object())
        .cloned()
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    let properties = feature
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    let reference = SigpacRef::from_properties(&properties)?;
    Ok(Some(RecintoInfo {
        reference,
        geometry,
        properties,
    }))
}
