// SPDX-License-Identifier: AGPL-3.0-or-later

//! Boundary-file import tests (docs/architecture.md testing strategy #1 — import feeds
//! regulatory plot data, so formats and rejects are pinned test-first).
//!
//! The GeoPackage fixture is built from scratch with rusqlite (GPKG *is*
//! SQLite): metadata tables per the OGC GeoPackage spec, geometry blobs as
//! GP-header + little-endian WKB.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use serde_json::Value;
use std::path::{Path, PathBuf};
use terrazgo_geo::GeoError;
use terrazgo_geo::import::{list_boundary_file, read_boundary_geometry};

/// Unique temp path per test (std temp dir; no tempfile dev-dependency).
fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("terrazgo-geo-test-{}-{name}", std::process::id()))
}

struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_temp(name: &str, contents: &str) -> TempFile {
    let path = temp_path(name);
    std::fs::write(&path, contents).unwrap();
    TempFile(path)
}

// ---------------------------------------------------------------------------
// GeoJSON
// ---------------------------------------------------------------------------

const COLLECTION: &str = r#"{
  "type": "FeatureCollection",
  "features": [
    { "type": "Feature",
      "properties": { "name": "Recinto grande", "uso": "TA" },
      "geometry": { "type": "Polygon", "coordinates":
        [[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.66],[-4.72,41.65]]] } },
    { "type": "Feature",
      "properties": { "name": "Un pozo" },
      "geometry": { "type": "Point", "coordinates": [-4.715,41.655] } },
    { "type": "Feature",
      "properties": { "NOMBRE": "El otro" },
      "geometry": { "type": "Polygon", "coordinates":
        [[[-4.62,41.55],[-4.61,41.55],[-4.61,41.56],[-4.62,41.56],[-4.62,41.55]]] } }
  ]
}"#;

#[test]
fn geojson_collection_lists_only_polygons_with_stable_ids() {
    let file = write_temp("collection.geojson", COLLECTION);
    let entries = list_boundary_file(&file.0).unwrap();

    // The Point feature is skipped, not an error; indexes stay document-based.
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "geojson:0");
    assert_eq!(entries[0].name.as_deref(), Some("Recinto grande"));
    assert_eq!(entries[0].properties["uso"], "TA");
    assert_eq!(entries[1].id, "geojson:2");
    assert_eq!(entries[1].name.as_deref(), Some("El otro")); // NOMBRE, any case

    let geometry = read_boundary_geometry(&file.0, "geojson:2").unwrap();
    let value: Value = serde_json::from_str(&geometry).unwrap();
    assert_eq!(value["type"], "Polygon");
    assert_eq!(value["coordinates"][0][0][0], -4.62);
}

#[test]
fn geojson_bare_geometry_and_single_feature_work() {
    let bare = write_temp(
        "bare.json",
        r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.65]]]}"#,
    );
    let entries = list_boundary_file(&bare.0).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(read_boundary_geometry(&bare.0, "geojson:0").is_ok());

    let feature = write_temp(
        "feature.json",
        r#"{"type":"Feature","properties":{"name":"Solo"},"geometry":
            {"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.65]]]}}"#,
    );
    let entries = list_boundary_file(&feature.0).unwrap();
    assert_eq!(entries[0].name.as_deref(), Some("Solo"));
}

#[test]
fn unsupported_and_empty_files_are_rejected_with_stable_codes() {
    let garbage = write_temp("garbage.bin", "not geodata of any kind");
    assert!(matches!(
        list_boundary_file(&garbage.0),
        Err(GeoError::Invalid("boundary_file_unsupported"))
    ));

    let empty = write_temp(
        "empty.geojson",
        r#"{"type":"FeatureCollection","features":[]}"#,
    );
    assert!(matches!(
        list_boundary_file(&empty.0),
        Err(GeoError::Invalid("boundary_file_empty"))
    ));

    // Points only: nothing usable as a boundary.
    let points = write_temp(
        "points.geojson",
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[-4.7,41.6]}}]}"#,
    );
    assert!(matches!(
        list_boundary_file(&points.0),
        Err(GeoError::Invalid("boundary_file_empty"))
    ));
}

// ---------------------------------------------------------------------------
// GeoPackage
// ---------------------------------------------------------------------------

/// A GPKG geometry blob: "GP" magic, version 0, flags 0x01 (little-endian,
/// no envelope), srs_id, then standard little-endian WKB for a polygon.
fn gpkg_polygon_blob(srs_id: i32, ring: &[(f64, f64)]) -> Vec<u8> {
    let mut blob = vec![0x47, 0x50, 0x00, 0x01];
    blob.extend_from_slice(&srs_id.to_le_bytes());
    blob.push(0x01); // WKB little-endian
    blob.extend_from_slice(&3u32.to_le_bytes()); // Polygon
    blob.extend_from_slice(&1u32.to_le_bytes()); // one ring
    blob.extend_from_slice(&(ring.len() as u32).to_le_bytes());
    for (x, y) in ring {
        blob.extend_from_slice(&x.to_le_bytes());
        blob.extend_from_slice(&y.to_le_bytes());
    }
    blob
}

const RING_A: &[(f64, f64)] = &[
    (-4.72, 41.65),
    (-4.71, 41.65),
    (-4.71, 41.66),
    (-4.72, 41.66),
    (-4.72, 41.65),
];

fn build_gpkg(path: &Path, srs_id: i32) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE gpkg_contents (
             table_name TEXT PRIMARY KEY, data_type TEXT NOT NULL,
             identifier TEXT, srs_id INTEGER);
         CREATE TABLE gpkg_geometry_columns (
             table_name TEXT PRIMARY KEY, column_name TEXT NOT NULL,
             geometry_type_name TEXT, srs_id INTEGER, z INTEGER, m INTEGER);
         CREATE TABLE recinto (
             fid INTEGER PRIMARY KEY, provincia INTEGER, municipio INTEGER,
             poligono INTEGER, parcela INTEGER, geom BLOB);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO gpkg_contents VALUES ('recinto', 'features', 'recintos', ?1)",
        [srs_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO gpkg_geometry_columns VALUES ('recinto', 'geom', 'POLYGON', ?1, 0, 0)",
        [srs_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recinto VALUES (1, 47, 186, 5, 23, ?1)",
        [gpkg_polygon_blob(srs_id, RING_A)],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO recinto VALUES (2, 47, 186, 5, 24, ?1)",
        [gpkg_polygon_blob(
            srs_id,
            &[
                (-4.62, 41.55),
                (-4.61, 41.55),
                (-4.61, 41.56),
                (-4.62, 41.55),
            ],
        )],
    )
    .unwrap();
}

#[test]
fn gpkg_lists_features_with_attributes_and_reads_geometry() {
    let path = temp_path("recintos-4258.gpkg");
    let file = TempFile(path.clone());
    build_gpkg(&path, 4258); // ETRS89 geographic — the SIGPAC case

    let entries = list_boundary_file(&file.0).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "gpkg:recinto:1");
    // Attributes surface for UI filtering (SIGPAC ref columns findable).
    assert_eq!(entries[0].properties["provincia"], 47);
    assert_eq!(entries[0].properties["parcela"], 23);
    // The geometry blob is not leaked into properties.
    assert!(entries[0].properties.get("geom").is_none());

    let geometry = read_boundary_geometry(&file.0, "gpkg:recinto:1").unwrap();
    let value: Value = serde_json::from_str(&geometry).unwrap();
    assert_eq!(value["type"], "Polygon");
    let ring = value["coordinates"][0].as_array().unwrap();
    assert_eq!(ring.len(), 5);
    assert_eq!(ring[0][0].as_f64().unwrap(), -4.72);
    assert_eq!(ring[0][1].as_f64().unwrap(), 41.65);
}

#[test]
fn gpkg_with_regcan95_is_accepted() {
    let path = temp_path("recintos-4081.gpkg");
    let file = TempFile(path.clone());
    // REGCAN95 geographic — Canary SIGPAC files. The EPSG-registered
    // transformation REGCAN95 → WGS84 is 0,0,0 (both ITRF-based), so
    // identity is the correct handling, not an approximation.
    build_gpkg(&path, 4081);

    let entries = list_boundary_file(&file.0).unwrap();
    assert_eq!(entries.len(), 2);
    let geometry = read_boundary_geometry(&file.0, "gpkg:recinto:1").unwrap();
    let value: Value = serde_json::from_str(&geometry).unwrap();
    assert_eq!(value["type"], "Polygon");
}

#[test]
fn gpkg_with_projected_srs_is_rejected() {
    let path = temp_path("recintos-25830.gpkg");
    let file = TempFile(path.clone());
    build_gpkg(&path, 25830); // ETRS89 / UTM 30N — the proj4rs contingency

    assert!(matches!(
        list_boundary_file(&file.0),
        Err(GeoError::Invalid("gpkg_unsupported_srs"))
    ));
}

#[test]
fn gpkg_unknown_entry_id_is_not_found() {
    let path = temp_path("recintos-lookup.gpkg");
    let file = TempFile(path.clone());
    build_gpkg(&path, 4326);

    assert!(matches!(
        read_boundary_geometry(&file.0, "gpkg:recinto:99"),
        Err(GeoError::NotFound)
    ));
    assert!(matches!(
        read_boundary_geometry(&file.0, "nonsense"),
        Err(GeoError::NotFound)
    ));
}
