// SPDX-License-Identifier: AGPL-3.0-or-later

//! Offline tests for the SIGPAC reference and client. Fixtures under
//! `tests/fixtures/` are REAL Nube de SIGPAC responses harvested 2026-07-08
//! (recinto 34/10/0/0/604/5021/13, Palencia) — no test touches the network:
//! the client is exercised through a pre-seeded in-memory geo cache.

// Clippy's `allow-unwrap-in-tests` only covers `#[test]` fns, not shared
// helpers — the file-level allow is the workspace convention.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_sigpac::client::{recinfo_cache_key, recinto_by_reference};
use module_sigpac::models::parse_recinto_response;
use module_sigpac::reference::SigpacRef;
use rusqlite::Connection;
use std::sync::Mutex;
use terrazgo_geo::GeoError;
use terrazgo_geo::db::open_cache_in_memory;

const RECINFO: &[u8] = include_bytes!("fixtures/recinfo.geojson");
const RECINFO_NOT_FOUND: &[u8] = include_bytes!("fixtures/recinfo-notfound.geojson");
const RECINFO_BY_POINT: &[u8] = include_bytes!("fixtures/recinfobypoint.geojson");

fn palencia_ref() -> SigpacRef {
    SigpacRef::from_parts(["34", "10", "0", "0", "604", "5021", "13"]).unwrap()
}

#[test]
fn reference_parses_and_round_trips_to_path() {
    let reference = palencia_ref();
    assert_eq!(reference.province, 34);
    assert_eq!(reference.enclosure, 13);
    assert_eq!(reference.to_path(), "34/10/0/0/604/5021/13");
    // Whitespace from form inputs is tolerated.
    let padded = SigpacRef::from_parts([" 34", "10 ", "0", "0", "604", "5021", "13"]).unwrap();
    assert_eq!(padded, reference);
}

#[test]
fn reference_rejects_non_numeric_and_bad_province() {
    let bad = [
        ["", "10", "0", "0", "604", "5021", "13"],
        ["34", "diez", "0", "0", "604", "5021", "13"],
        ["34", "10", "0", "0", "604", "5021", "-1"],
        // INE province codes are 1–52.
        ["0", "10", "0", "0", "604", "5021", "13"],
        ["53", "10", "0", "0", "604", "5021", "13"],
    ];
    for parts in bad {
        assert!(
            matches!(
                SigpacRef::from_parts(parts),
                Err(GeoError::Invalid("sigpac_ref_invalid"))
            ),
            "expected rejection for {parts:?}"
        );
    }
}

#[test]
fn recinfo_fixture_parses_to_recinto_info() {
    let recinto = parse_recinto_response(RECINFO).unwrap().unwrap();
    assert_eq!(recinto.reference, palencia_ref());
    // Attributes verified against the live service 2026-07-08: `superficie`
    // is hectares (the intersection endpoint reported the same recinto as
    // 288465 m² = 100%).
    assert_eq!(recinto.surface_ha(), Some(28.8465));
    assert_eq!(recinto.land_use(), Some("PA"));
    assert_eq!(recinto.geometry["type"], "Polygon");
    // The full attribute set survives untyped for geo_feature.properties.
    assert!(recinto.properties.contains_key("coef_regadio"));
    assert!(recinto.properties.contains_key("pendiente_media"));
}

#[test]
fn unknown_reference_is_none_not_error() {
    // The service answers HTTP 200 with an empty FeatureCollection for an
    // unknown reference — never 404 (live-tested 2026-07-08).
    assert!(parse_recinto_response(RECINFO_NOT_FOUND).unwrap().is_none());
}

#[test]
fn by_point_response_parses_with_the_same_shape() {
    let recinto = parse_recinto_response(RECINFO_BY_POINT).unwrap().unwrap();
    assert_eq!(recinto.reference, palencia_ref());
    assert!(recinto.geometry.is_object());
}

#[test]
fn malformed_response_is_a_stable_error() {
    assert!(matches!(
        parse_recinto_response(br#"{"unexpected": true}"#),
        Err(GeoError::Invalid("sigpac_response_invalid"))
    ));
}

#[test]
fn client_serves_a_cached_lookup_without_network() {
    let cache = Mutex::new(open_cache_in_memory().unwrap());
    let reference = palencia_ref();
    seed_resource(&cache, &recinfo_cache_key(&reference), RECINFO);

    // This test has no network; a cache hit must be enough.
    let recinto = recinto_by_reference(&cache, &reference, false)
        .unwrap()
        .unwrap();
    assert_eq!(recinto.reference, reference);
    assert_eq!(recinto.land_use(), Some("PA"));
}

fn seed_resource(cache: &Mutex<Connection>, key: &str, data: &[u8]) {
    cache
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO resource (key, data, content_type, fetched_at)
             VALUES (?1, ?2, 'application/json', '2026-07-08T00:00:00Z')",
            rusqlite::params![key, data],
        )
        .unwrap();
}

// --- zone intersections + campaign (P4, fixtures harvested 2026-07-08) -----

const NITRATOS: &[u8] = include_bytes!("fixtures/intersection-nitratos.json");
const FITOSANITARIOS: &[u8] = include_bytes!("fixtures/intersection-fitosanitarios.json");
const RED_NATURA: &[u8] = include_bytes!("fixtures/intersection-red-natura.json");
const CAMPAIGNS: &[u8] = include_bytes!("fixtures/geopackages-listing.html");

#[test]
fn intersection_fixtures_parse_inside_and_outside() {
    use module_sigpac::models::parse_intersection_response;

    // 100% inside the nitrate-vulnerable zone (live service, 2026-07-08).
    let nitratos = parse_intersection_response(NITRATOS).unwrap().unwrap();
    assert_eq!(nitratos.surface_tpc, 100.0);
    assert_eq!(nitratos.descripcion, None);

    // Phyto layer carries a description the UI shows verbatim.
    let phyto = parse_intersection_response(FITOSANITARIOS)
        .unwrap()
        .unwrap();
    assert!(phyto.surface_tpc > 99.0);
    assert_eq!(phyto.descripcion.as_deref(), Some("Zona periférica"));

    // `[]` = outside the layer: a storable negative, not an error.
    assert!(parse_intersection_response(RED_NATURA).unwrap().is_none());

    assert!(matches!(
        parse_intersection_response(br#"{"not":"an array"}"#),
        Err(GeoError::Invalid("sigpac_response_invalid"))
    ));
}

#[test]
fn current_campaign_reads_the_max_year_from_the_listing() {
    use module_sigpac::client::current_campaign;

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    seed_resource(&cache, "sigpac/campaigns", CAMPAIGNS);
    // The harvested listing names 2025/ and 2026/ → current campaign 2026.
    assert_eq!(current_campaign(&cache, false).unwrap(), 2026);

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    seed_resource(&cache, "sigpac/campaigns", b"<html>no years here</html>");
    assert!(matches!(
        current_campaign(&cache, false),
        Err(GeoError::Invalid("sigpac_response_invalid"))
    ));
}
