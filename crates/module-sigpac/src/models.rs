// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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

/// One line of the PAC graphical declaration (`cultivo_declarado`) for a
/// recinto: what the holder declared growing there in a given campaign.
///
/// `properties` keeps the full attribute set untyped, like [`RecintoInfo`] —
/// the declaration carries aid lines and expediente identity the app has no
/// use for today. Note what is NOT here: `exp_ano`. The service accepts it as
/// a filter but omits it from item responses (live-probed 2026-08-02), so the
/// campaign a line belongs to is the campaign that was asked for, and it
/// travels in [`DeclaredCampaign`] rather than being read back per feature.
#[derive(Debug, Clone, Serialize)]
pub struct DeclaredCrop {
    pub properties: Map<String, Value>,
}

impl DeclaredCrop {
    /// Declared crop code (`parc_producto`), a FEGA PRODUCTOS catalogue code.
    pub fn product_code(&self) -> Option<i64> {
        self.properties.get("parc_producto").and_then(Value::as_i64)
    }

    /// Secondary crop code (`cultsecun_producto`) when the line declares one —
    /// a second crop on the same recinto, not a replacement for the first.
    pub fn secondary_product_code(&self) -> Option<i64> {
        self.properties
            .get("cultsecun_producto")
            .and_then(Value::as_i64)
    }

    /// Exploitation system (`parc_sistexp`): `"S"` secano, `"R"` regadío —
    /// both observed live 2026-08-03. It says whether the crop is irrigated,
    /// never by which system, so it maps to `rainfed` or to nothing.
    pub fn exploitation_system(&self) -> Option<&str> {
        self.properties.get("parc_sistexp").and_then(Value::as_str)
    }

    /// Declared cultivated surface in hectares. `parc_supcult` is in **square
    /// metres** (296800 = 29,68 ha — the same m² trap as the MVT layer).
    pub fn cultivated_area_ha(&self) -> Option<f64> {
        self.properties
            .get("parc_supcult")
            .and_then(Value::as_f64)
            .map(|m2| m2 / 10_000.0)
    }
}

/// Declaration lines together with the campaign that actually answered for
/// them — the service serves one campaign behind, so which year a proposal
/// speaks for is part of the answer, never an assumption.
#[derive(Debug, Clone, Serialize)]
pub struct DeclaredCampaign {
    pub campaign: i64,
    pub lines: Vec<DeclaredCrop>,
}

/// Parse an OGC API Features `cultivo_declarado` items response.
///
/// Unlike [`parse_recinto_response`], **every** feature is kept: one recinto
/// can carry several declaration lines. An empty FeatureCollection is a real
/// answer — "nothing declared here" — and the service returns it as HTTP 200
/// with `numberMatched: 0`, never a 404 (live-probed 2026-08-02).
pub fn parse_declared_crops_response(bytes: &[u8]) -> Result<Vec<DeclaredCrop>> {
    let document: Value = serde_json::from_slice(bytes)?;
    let features = document
        .get("features")
        .and_then(Value::as_array)
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    features
        .iter()
        .map(|feature| {
            let properties = feature
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
            Ok(DeclaredCrop { properties })
        })
        .collect()
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
