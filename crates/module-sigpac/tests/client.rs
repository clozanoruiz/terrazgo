// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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
    seed_resource_at(cache, key, data, "2026-07-08T00:00:00Z");
}

/// Seed with an explicit fetch time. The declared-crops fallback re-asks an
/// EMPTY current-campaign answer that was stored on an earlier day, so any
/// test about a trusted empty must seed it as today's — otherwise the test
/// silently depends on the machine having network.
fn seed_resource_at(cache: &Mutex<Connection>, key: &str, data: &[u8], fetched_at: &str) {
    cache
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO resource (key, data, content_type, fetched_at)
             VALUES (?1, ?2, 'application/json', ?3)",
            rusqlite::params![key, data, fetched_at],
        )
        .unwrap();
}

/// Today, as the cache writes it.
fn today_stamp() -> String {
    format!("{}T00:00:00Z", terrazgo_core::date::today_utc())
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

// --- declared crops: OGC API Features `cultivo_declarado` -------------------
// Fixtures harvested live 2026-08-03: recinto 47/163/0/0/11/40/1 (Valladolid,
// campaign 2025) and 47/219/0/0/11/28/2, which declares a secondary crop.

const DECLARED: &[u8] = include_bytes!("fixtures/cultivo-declarado.json");
const DECLARED_EMPTY: &[u8] = include_bytes!("fixtures/cultivo-declarado-empty.json");
const DECLARED_SECONDARY: &[u8] = include_bytes!("fixtures/cultivo-declarado-secondary.json");

fn valladolid_ref() -> SigpacRef {
    SigpacRef::from_parts(["47", "163", "0", "0", "11", "40", "1"]).unwrap()
}

#[test]
fn declared_crops_fixture_parses_the_declaration_line() {
    use module_sigpac::models::parse_declared_crops_response;

    let lines = parse_declared_crops_response(DECLARED).unwrap();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    // PRODUCTOS code 5 = CEBADA in the vendored FEGA catalogue.
    assert_eq!(line.product_code(), Some(5));
    assert_eq!(line.secondary_product_code(), None);
    // "S" = secano (live-observed; "R" = regadío, see the secondary fixture).
    assert_eq!(line.exploitation_system(), Some("S"));
    // parc_supcult is in SQUARE METRES: 296800 m² = 29,68 ha.
    assert_eq!(line.cultivated_area_ha(), Some(29.68));
}

#[test]
fn declared_crops_read_the_secondary_crop_and_regadio() {
    use module_sigpac::models::parse_declared_crops_response;

    let lines = parse_declared_crops_response(DECLARED_SECONDARY).unwrap();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    // Codes 4 = MAÍZ (main) and 6 = CENTENO (secondary): a second crop on the
    // same recinto, not a correction of the first.
    assert_eq!(line.product_code(), Some(4));
    assert_eq!(line.secondary_product_code(), Some(6));
    assert_eq!(line.exploitation_system(), Some("R"));
    assert_eq!(line.cultivated_area_ha(), Some(4.17));
}

/// Nothing declared is a real answer the service gives as HTTP 200 with
/// `numberMatched: 0` — never a 404, so no status special-casing is needed.
#[test]
fn declared_crops_empty_collection_is_an_empty_list_not_an_error() {
    use module_sigpac::models::parse_declared_crops_response;

    assert!(
        parse_declared_crops_response(DECLARED_EMPTY)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        parse_declared_crops_response(br#"{"no":"features"}"#),
        Err(GeoError::Invalid("sigpac_response_invalid"))
    ));
}

#[test]
fn declared_crops_cache_key_carries_the_campaign() {
    use module_sigpac::client::declared_crops_cache_key;

    assert_eq!(
        declared_crops_cache_key(2025, &valladolid_ref()),
        "sigpac/cultivos/2025/47/163/0/0/11/40/1"
    );
    // A different campaign is a different row, so a rollover adds rather than
    // overwrites and last year's answer stays available as the fallback.
    assert_ne!(
        declared_crops_cache_key(2025, &valladolid_ref()),
        declared_crops_cache_key(2026, &valladolid_ref())
    );
}

/// The service runs one campaign behind, so the fallback is the normal path:
/// the current campaign answers nothing and the previous one carries the
/// declaration. No network here — the current campaign's empty is seeded as
/// TODAY's, which is exactly the answer the day rule trusts.
#[test]
fn fallback_serves_the_previous_campaign_and_labels_it() {
    use module_sigpac::client::{declared_crops_cache_key, declared_crops_with_fallback};

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    let reference = valladolid_ref();
    seed_resource_at(
        &cache,
        &declared_crops_cache_key(2026, &reference),
        DECLARED_EMPTY,
        &today_stamp(),
    );
    seed_resource(
        &cache,
        &declared_crops_cache_key(2025, &reference),
        DECLARED,
    );

    let answer = declared_crops_with_fallback(&cache, &reference, 2026, false)
        .unwrap()
        .unwrap();
    // The campaign that answered, not the one that was asked for first — every
    // proposal has to be able to say which year's declaration it repeats.
    assert_eq!(answer.campaign, 2025);
    assert_eq!(answer.lines.len(), 1);
    assert_eq!(answer.lines[0].product_code(), Some(5));
}

/// A stored non-empty answer for the current campaign is final: it is served
/// without asking upstream, which is what makes a loaded farm work offline.
#[test]
fn fallback_trusts_a_stored_current_campaign_answer() {
    use module_sigpac::client::{declared_crops_cache_key, declared_crops_with_fallback};

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    let reference = valladolid_ref();
    seed_resource(
        &cache,
        &declared_crops_cache_key(2026, &reference),
        DECLARED,
    );

    let answer = declared_crops_with_fallback(&cache, &reference, 2026, false)
        .unwrap()
        .unwrap();
    assert_eq!(answer.campaign, 2026);
}

/// "Nothing declared" is only reported when both campaigns actually answered:
/// the previous campaign's cached empty is authoritative (closed dataset) and
/// the current one's was asked today, so both are real answers rather than
/// gaps — and no network is involved in establishing that.
#[test]
fn fallback_reports_none_when_both_campaigns_answered_empty() {
    use module_sigpac::client::{declared_crops_cache_key, declared_crops_with_fallback};

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    let reference = valladolid_ref();
    seed_resource_at(
        &cache,
        &declared_crops_cache_key(2026, &reference),
        DECLARED_EMPTY,
        &today_stamp(),
    );
    seed_resource(
        &cache,
        &declared_crops_cache_key(2025, &reference),
        DECLARED_EMPTY,
    );

    assert!(
        declared_crops_with_fallback(&cache, &reference, 2026, false)
            .unwrap()
            .is_none()
    );
}

/// The read-only cache probe the fallback needs: it must never fetch, so that
/// "stored but empty" can be told apart from "never asked".
#[test]
fn cached_probe_reads_without_fetching() {
    use terrazgo_geo::fetch;

    let cache = Mutex::new(open_cache_in_memory().unwrap());
    assert!(
        fetch::cached(&cache, "sigpac/cultivos/2025/x")
            .unwrap()
            .is_none()
    );

    seed_resource(&cache, "sigpac/cultivos/2025/x", DECLARED_EMPTY);
    let hit = fetch::cached(&cache, "sigpac/cultivos/2025/x")
        .unwrap()
        .unwrap();
    assert_eq!(hit.data, DECLARED_EMPTY);
}
