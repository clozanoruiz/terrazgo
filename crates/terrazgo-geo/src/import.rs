// SPDX-License-Identifier: AGPL-3.0-or-later

//! Boundary-file import: turn a file the user already has (GeoJSON export,
//! Nube de SIGPAC GeoPackage download, …) into GeoJSON geometries ready for
//! `geo_feature`. User-supplied map data is first-class
//! (2026-07-07): this path works fully offline.
//!
//! Two-step API, because a municipality-sized GeoPackage holds thousands of
//! recintos: [`list_boundary_file`] returns light entries (id + name +
//! attributes, no geometry) the UI can filter; [`read_boundary_geometry`]
//! loads the one geometry the user picked.
//!
//! Formats: GeoJSON (bare geometry, Feature, or FeatureCollection) and
//! GeoPackage (it is SQLite — read with rusqlite; geometry blobs decoded via
//! geozero's GPKG WKB dialect). KML/KMZ deliberately absent — dropped from
//! the roadmap 2026-07-08: the visor exports GeoJSON/GML/Shapefile, no KML.
//! Coordinates must be geographic: EPSG 4326, 4258 (ETRS89 ≡ WGS84 at cm
//! level for our purpose) or 4081 (REGCAN95, ditto). Projected GeoPackages
//! (UTM, LCC, LAEA) are rejected with `gpkg_unsupported_srs`; the agreed
//! future contingency is a proj4rs-backed EPSG registry (25828–31, 4083,
//! 3034, 3035, 32628–31 — decision 2026-07-08, deferred until needed).

use crate::error::{GeoError, Result};
use geozero::GeozeroGeometry;
use geozero::geojson::GeoJsonWriter;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::Path;
use terrazgo_core::geojson::validate_boundary_geometry_value;

/// Hard cap on listed entries — beyond this the UI filter itself becomes
/// useless and the payload to the webview unreasonable (a full province
/// download; the user should fetch the municipality file instead).
const MAX_ENTRIES: usize = 50_000;

/// SRS ids accepted as geographic lon/lat. 4081 is REGCAN95 (Canary SIGPAC
/// files): its EPSG-registered transformation to WGS84 is 0,0,0, so identity
/// is the correct handling. 0/-1 are GPKG's "undefined"; range validation
/// catches projected coordinates hiding behind them.
const GEOGRAPHIC_SRS: &[i64] = &[4326, 4258, 4081, 0, -1];

/// One selectable boundary candidate. `id` is stable within the file
/// (`geojson:<index>` or `gpkg:<table>:<rowid>`); `properties` carries the
/// feature's attributes so the UI can label and filter (SIGPAC ref columns
/// make recintos findable).
#[derive(Debug, Serialize)]
pub struct BoundaryEntry {
    pub id: String,
    pub name: Option<String>,
    pub properties: Value,
}

/// List the polygonal features of a boundary file (no geometries returned).
pub fn list_boundary_file(path: &Path) -> Result<Vec<BoundaryEntry>> {
    let entries = if is_sqlite_file(path)? {
        list_gpkg(path)?
    } else {
        list_geojson(path)?
    };
    if entries.is_empty() {
        return Err(GeoError::Invalid("boundary_file_empty"));
    }
    Ok(entries)
}

/// Load one candidate's geometry as a validated GeoJSON geometry string.
pub fn read_boundary_geometry(path: &Path, entry_id: &str) -> Result<String> {
    let geometry = if let Some(index) = entry_id.strip_prefix("geojson:") {
        let index: usize = index.parse().map_err(|_| GeoError::NotFound)?;
        geojson_geometries(path)?
            .into_iter()
            .nth(index)
            .ok_or(GeoError::NotFound)?
    } else if let Some(rest) = entry_id.strip_prefix("gpkg:") {
        let (table, rowid) = rest.rsplit_once(':').ok_or(GeoError::NotFound)?;
        let rowid: i64 = rowid.parse().map_err(|_| GeoError::NotFound)?;
        read_gpkg_geometry(path, table, rowid)?
    } else {
        return Err(GeoError::NotFound);
    };
    let value: Value = serde_json::from_str(&geometry)?;
    validate_boundary_geometry_value(&value)?;
    Ok(geometry)
}

// ---------------------------------------------------------------------------
// GeoJSON
// ---------------------------------------------------------------------------

/// The features of a GeoJSON document as (properties, geometry) pairs, in
/// document order. Indexes are the stable ids, so this must enumerate
/// identically in `list` and `read`.
fn geojson_features(path: &Path) -> Result<Vec<(Value, Value)>> {
    let text = std::fs::read_to_string(path)?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|_| GeoError::Invalid("boundary_file_unsupported"))?;
    let features = match doc.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => doc
            .get("features")
            .and_then(Value::as_array)
            .ok_or(GeoError::Invalid("boundary_file_unsupported"))?
            .clone(),
        Some("Feature") => vec![doc],
        Some("Polygon" | "MultiPolygon") => {
            return Ok(vec![(Value::Null, doc)]);
        }
        _ => return Err(GeoError::Invalid("boundary_file_unsupported")),
    };
    if features.len() > MAX_ENTRIES {
        return Err(GeoError::Invalid("boundary_file_too_large"));
    }
    Ok(features
        .into_iter()
        .map(|mut f| {
            let geometry = f
                .get_mut("geometry")
                .map(Value::take)
                .unwrap_or(Value::Null);
            let properties = f
                .get_mut("properties")
                .map(Value::take)
                .unwrap_or(Value::Null);
            (properties, geometry)
        })
        .collect())
}

fn list_geojson(path: &Path) -> Result<Vec<BoundaryEntry>> {
    Ok(geojson_features(path)?
        .into_iter()
        .enumerate()
        // Only polygonal, valid geometries are offered; other feature types
        // in a mixed file are skipped, not an error.
        .filter(|(_, (_, geometry))| validate_boundary_geometry_value(geometry).is_ok())
        .map(|(index, (properties, _))| BoundaryEntry {
            id: format!("geojson:{index}"),
            name: name_from_properties(&properties),
            properties,
        })
        .collect())
}

/// Geometries in the same enumeration (and therefore index) order as
/// `list_geojson` — including the skipped ones is what keeps indexes stable,
/// so filtering happens only on the list side.
fn geojson_geometries(path: &Path) -> Result<Vec<String>> {
    Ok(geojson_features(path)?
        .into_iter()
        .map(|(_, geometry)| geometry.to_string())
        .collect())
}

// ---------------------------------------------------------------------------
// GeoPackage
// ---------------------------------------------------------------------------

fn is_sqlite_file(path: &Path) -> Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(bytes.starts_with(b"SQLite format 3\0"))
}

fn open_gpkg(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

/// The feature tables of a GeoPackage with a geographic SRS, as
/// (table, geometry_column) pairs.
fn gpkg_feature_tables(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT c.table_name, g.column_name, g.srs_id
             FROM gpkg_contents c
             JOIN gpkg_geometry_columns g ON g.table_name = c.table_name
             WHERE c.data_type = 'features'
             ORDER BY c.table_name",
        )
        .map_err(|_| GeoError::Invalid("boundary_file_unsupported"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Err(GeoError::Invalid("boundary_file_unsupported"));
    }
    let geographic: Vec<(String, String)> = rows
        .iter()
        .filter(|(_, _, srs)| GEOGRAPHIC_SRS.contains(srs))
        .map(|(t, c, _)| (t.clone(), c.clone()))
        .collect();
    if geographic.is_empty() {
        // Every feature table is projected (UTM province packs) — the named
        // proj4rs contingency; reject clearly rather than emit garbage.
        return Err(GeoError::Invalid("gpkg_unsupported_srs"));
    }
    Ok(geographic)
}

/// Table/column identifiers come from GPKG metadata tables and are quoted
/// into SQL — refuse names that would escape the quoting.
fn quote_identifier(name: &str) -> Result<String> {
    if name.contains('"') {
        return Err(GeoError::Invalid("boundary_file_unsupported"));
    }
    Ok(format!("\"{name}\""))
}

fn list_gpkg(path: &Path) -> Result<Vec<BoundaryEntry>> {
    let conn = open_gpkg(path)?;
    let mut entries = Vec::new();
    for (table, geom_column) in gpkg_feature_tables(&conn)? {
        let quoted_table = quote_identifier(&table)?;
        let mut stmt = conn.prepare(&format!("SELECT rowid, * FROM {quoted_table}"))?;
        let column_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let mut properties = Map::new();
            for (i, column) in column_names.iter().enumerate().skip(1) {
                if *column == geom_column {
                    continue;
                }
                let value = match row.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => Value::Null,
                    rusqlite::types::ValueRef::Integer(v) => Value::from(v),
                    rusqlite::types::ValueRef::Real(v) => Value::from(v),
                    rusqlite::types::ValueRef::Text(v) => {
                        Value::from(String::from_utf8_lossy(v).into_owned())
                    }
                    rusqlite::types::ValueRef::Blob(_) => continue,
                };
                properties.insert(column.clone(), value);
            }
            let properties = Value::Object(properties);
            entries.push(BoundaryEntry {
                id: format!("gpkg:{table}:{rowid}"),
                name: name_from_properties(&properties),
                properties,
            });
            if entries.len() > MAX_ENTRIES {
                return Err(GeoError::Invalid("boundary_file_too_large"));
            }
        }
    }
    Ok(entries)
}

fn read_gpkg_geometry(path: &Path, table: &str, rowid: i64) -> Result<String> {
    let conn = open_gpkg(path)?;
    let geom_column = gpkg_feature_tables(&conn)?
        .into_iter()
        .find(|(t, _)| t == table)
        .map(|(_, c)| c)
        .ok_or(GeoError::NotFound)?;
    let quoted_table = quote_identifier(table)?;
    let quoted_column = quote_identifier(&geom_column)?;
    let blob: Vec<u8> = conn
        .query_row(
            &format!("SELECT {quoted_column} FROM {quoted_table} WHERE rowid = ?1"),
            [rowid],
            |r| r.get(0),
        )
        .map_err(|_| GeoError::NotFound)?;
    gpkg_blob_to_geojson(&blob)
}

/// Decode one GPKG geometry blob (GP header + WKB) to a GeoJSON geometry
/// string via geozero.
fn gpkg_blob_to_geojson(blob: &[u8]) -> Result<String> {
    let mut out: Vec<u8> = Vec::new();
    let mut writer = GeoJsonWriter::new(&mut out);
    geozero::wkb::GpkgWkb(blob.to_vec())
        .process_geom(&mut writer)
        .map_err(|_| GeoError::Invalid("geometry_invalid"))?;
    String::from_utf8(out).map_err(|_| GeoError::Invalid("geometry_invalid"))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Best-effort display name from feature attributes ("name"/"nombre" in any
/// capitalisation) — a hint for the picker, never required.
fn name_from_properties(properties: &Value) -> Option<String> {
    let object = properties.as_object()?;
    object.iter().find_map(|(key, value)| {
        let key = key.to_lowercase();
        if key == "name" || key == "nombre" {
            value.as_str().map(str::to_string)
        } else {
            None
        }
    })
}
