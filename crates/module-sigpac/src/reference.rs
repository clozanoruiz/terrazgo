// SPDX-License-Identifier: AGPL-3.0-or-later

//! The 7-part SIGPAC reference: provincia / municipio / agregado / zona /
//! polígono / parcela / recinto. This is exactly what `plot_es_extension`
//! stores as seven TEXT columns and what the Nube de SIGPAC `recinfo`
//! endpoint takes as its URL path.

use serde::Serialize;
use serde_json::{Map, Value};
use terrazgo_geo::{GeoError, Result};

/// A validated SIGPAC reference. All parts are numeric in SIGPAC itself;
/// `aggregate` and `zone` are usually 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SigpacRef {
    pub province: u32,
    pub municipality: u32,
    pub aggregate: u32,
    pub zone: u32,
    pub polygon: u32,
    pub parcel: u32,
    pub enclosure: u32,
}

impl SigpacRef {
    /// Parse the seven parts as the user typed them (the `plot_es_extension`
    /// columns, in storage order). Rejects non-numeric or out-of-range parts
    /// with the stable `sigpac_ref_invalid` code; whether the reference
    /// EXISTS is the service's call (an unknown one comes back as `None`
    /// from the client, not as an error here).
    pub fn from_parts(parts: [&str; 7]) -> Result<Self> {
        // `map` on the array runs the closure per element; `?` inside a
        // closure can't propagate to the outer function, so parse to a
        // Result per part and collect them below.
        let numbers = parts.map(|part| {
            part.trim()
                .parse::<u32>()
                .map_err(|_| GeoError::Invalid("sigpac_ref_invalid"))
        });
        let [
            province,
            municipality,
            aggregate,
            zone,
            polygon,
            parcel,
            enclosure,
        ] = numbers;
        let reference = SigpacRef {
            province: province?,
            municipality: municipality?,
            aggregate: aggregate?,
            zone: zone?,
            polygon: polygon?,
            parcel: parcel?,
            enclosure: enclosure?,
        };
        // Spanish provinces are 1–52 (INE codes; 51/52 are Ceuta/Melilla).
        if !(1..=52).contains(&reference.province) {
            return Err(GeoError::Invalid("sigpac_ref_invalid"));
        }
        Ok(reference)
    }

    /// The reference read back from a service response's `properties`
    /// (provincia, municipio, … come back as JSON numbers).
    pub fn from_properties(properties: &Map<String, Value>) -> Result<Self> {
        let number = |key: &str| -> Result<u32> {
            properties
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or(GeoError::Invalid("sigpac_response_invalid"))
        };
        Ok(SigpacRef {
            province: number("provincia")?,
            municipality: number("municipio")?,
            aggregate: number("agregado")?,
            zone: number("zona")?,
            polygon: number("poligono")?,
            parcel: number("parcela")?,
            enclosure: number("recinto")?,
        })
    }

    /// The slash-joined form the consultas endpoints take as URL path —
    /// `34/10/0/0/604/5021/13`.
    pub fn to_path(&self) -> String {
        format!(
            "{}/{}/{}/{}/{}/{}/{}",
            self.province,
            self.municipality,
            self.aggregate,
            self.zone,
            self.polygon,
            self.parcel,
            self.enclosure
        )
    }
}
