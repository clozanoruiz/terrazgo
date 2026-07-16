// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Offline tests for the composed SIGPAC operations: verify-and-store,
//! lookups with dedup. The app database is in-memory with core migrations
//! applied; the geo cache is in-memory pre-seeded with the harvested real
//! responses — no test touches the network.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_sigpac::service::{lookup_point, lookup_reference, verify_plot};
use module_sigpac::storage::SOURCE;
use rusqlite::Connection;
use std::sync::Mutex;
use terrazgo_core::models::{NewFarm, NewPlot, PlotEsFields};
use terrazgo_core::repository::{insert_farm, insert_plot};
use terrazgo_geo::GeoError;
use terrazgo_geo::db::open_cache_in_memory;

const RECINFO: &[u8] = include_bytes!("fixtures/recinfo.geojson");
const RECINFO_NOT_FOUND: &[u8] = include_bytes!("fixtures/recinfo-notfound.geojson");
const RECINFO_BY_POINT: &[u8] = include_bytes!("fixtures/recinfobypoint.geojson");
const NITRATOS: &[u8] = include_bytes!("fixtures/intersection-nitratos.json");
const FITOSANITARIOS: &[u8] = include_bytes!("fixtures/intersection-fitosanitarios.json");
const RED_NATURA: &[u8] = include_bytes!("fixtures/intersection-red-natura.json");
const CAMPAIGNS: &[u8] = include_bytes!("fixtures/geopackages-listing.html");

const PALENCIA_KEY: &str = "sigpac/recinfo/34/10/0/0/604/5021/13";
const PALENCIA_PATH: &str = "34/10/0/0/604/5021/13";

/// Everything `verify_plot` touches on the happy path: recinfo, the campaign
/// listing and the three zone layers — verify must NEVER reach the network
/// from a test.
fn full_palencia_entries() -> Vec<(String, &'static [u8])> {
    vec![
        (PALENCIA_KEY.into(), RECINFO),
        ("sigpac/campaigns".into(), CAMPAIGNS),
        (
            format!("sigpac/intersection/nitratos/{PALENCIA_PATH}"),
            NITRATOS,
        ),
        (
            format!("sigpac/intersection/fitosanitarios/{PALENCIA_PATH}"),
            FITOSANITARIOS,
        ),
        (
            format!("sigpac/intersection/red_natura/{PALENCIA_PATH}"),
            RED_NATURA,
        ),
    ]
}

fn app_db() -> Connection {
    terrazgo_core::db::open_in_memory().unwrap()
}

fn seeded_cache<K: AsRef<str>>(entries: &[(K, &[u8])]) -> Mutex<Connection> {
    let cache = open_cache_in_memory().unwrap();
    for (key, data) in entries {
        cache
            .execute(
                "INSERT INTO resource (key, data, content_type, fetched_at)
                 VALUES (?1, ?2, 'application/json', '2026-07-08T00:00:00Z')",
                rusqlite::params![key.as_ref(), data],
            )
            .unwrap();
    }
    Mutex::new(cache)
}

/// Farm + plot carrying the fixture recinto's reference. `parts` lets tests
/// exercise the numeric normalisation ("034" must match provincia 34).
fn plot_with_reference(conn: &mut Connection, parts: [&str; 7]) -> String {
    let farm = insert_farm(
        conn,
        NewFarm {
            name: "La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
    )
    .unwrap();
    let plot = insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id,
            name: "El Páramo".into(),
            area_ha: Some(28.0),
            es: Some(PlotEsFields {
                sigpac_province: Some(parts[0].into()),
                sigpac_municipality: Some(parts[1].into()),
                sigpac_aggregate: Some(parts[2].into()),
                sigpac_zone: Some(parts[3].into()),
                sigpac_polygon: Some(parts[4].into()),
                sigpac_parcel: Some(parts[5].into()),
                sigpac_enclosure: Some(parts[6].into()),
            }),
        },
    )
    .unwrap();
    plot.id
}

fn geo_feature_rows(conn: &Connection, plot_id: &str) -> (i64, i64) {
    let active = conn
        .query_row(
            "SELECT COUNT(*) FROM geo_feature
             WHERE plot_id = ?1 AND source = 'sigpac' AND deleted_at IS NULL",
            [plot_id],
            |r| r.get(0),
        )
        .unwrap();
    let total = conn
        .query_row(
            "SELECT COUNT(*) FROM geo_feature WHERE plot_id = ?1 AND source = 'sigpac'",
            [plot_id],
            |r| r.get(0),
        )
        .unwrap();
    (active, total)
}

#[test]
fn verify_plot_stores_the_official_boundary() {
    let mut app = app_db();
    let cache = seeded_cache(&full_palencia_entries());
    let plot_id = plot_with_reference(&mut app, ["34", "10", "0", "0", "604", "5021", "13"]);

    let verification = verify_plot(&mut app, &cache, &plot_id, false)
        .unwrap()
        .expect("recinto exists in SIGPAC");

    let feature = &verification.feature;
    assert_eq!(feature.source, SOURCE);
    assert_eq!(feature.plot_id.as_deref(), Some(plot_id.as_str()));
    // Official surface is stored alongside, never onto plot.area_ha.
    assert_eq!(feature.official_area_ha, Some(28.8465));
    let declared: f64 = app
        .query_row("SELECT area_ha FROM plot WHERE id = ?1", [&plot_id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(declared, 28.0);
    // The full attribute set survives in properties, source-tagged.
    let properties: serde_json::Value =
        serde_json::from_str(feature.properties.as_deref().unwrap()).unwrap();
    assert_eq!(properties["uso_sigpac"], "PA");
    assert!(feature.fetched_at.is_some());
    // The write is audit-logged like any user data.
    let logged: i64 = app
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table = 'geo_feature' AND entity_id = ?1 AND operation = 'insert'",
            [&feature.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 1);

    // Zone checks folded into verification (decision 2026-07-08): all three
    // layers stored, campaign from the provider listing (2025+2026 → 2026).
    // Expected values are the live service's answers for this recinto,
    // harvested 2026-07-08: 100% nitrate-vulnerable, phyto "Zona periférica",
    // outside Natura 2000.
    assert!(verification.zone_check_error.is_none());
    let flags = verification.zone_flags.as_ref().expect("zones checked");
    assert_eq!(flags.len(), 3);
    assert!(
        flags
            .iter()
            .all(|f| f.campaign == 2026 && f.source == "sigpac")
    );
    let by_code = |code: &str| flags.iter().find(|f| f.zone_type_code == code).unwrap();
    assert_eq!(by_code("nitrate_vulnerable").status, "inside");
    assert_eq!(by_code("nitrate_vulnerable").coverage_pct, Some(100.0));
    let phyto = by_code("phytosanitary_restriction");
    assert_eq!(phyto.status, "inside");
    assert_eq!(phyto.detail.as_deref(), Some("Zona periférica"));
    assert_eq!(by_code("natura_2000").status, "outside");
}

#[test]
fn zone_failure_keeps_the_stored_boundary() {
    let mut app = app_db();
    // recinfo resolves, but the campaign listing is unusable (no year dirs) —
    // deterministic zone failure with no network attempt.
    let cache = seeded_cache(&[
        (PALENCIA_KEY.to_string(), RECINFO),
        (
            "sigpac/campaigns".to_string(),
            b"<html>maintenance</html>" as &[u8],
        ),
    ]);
    let plot_id = plot_with_reference(&mut app, ["34", "10", "0", "0", "604", "5021", "13"]);

    let verification = verify_plot(&mut app, &cache, &plot_id, false)
        .unwrap()
        .expect("recinto exists");
    // Boundary stored; zones honestly reported unchecked.
    assert_eq!(verification.feature.source, "sigpac");
    assert!(verification.zone_flags.is_none());
    assert!(verification.zone_check_error.is_some());
    assert_eq!(geo_feature_rows(&app, &plot_id), (1, 1));
    let zone_rows: i64 = app
        .query_row("SELECT COUNT(*) FROM plot_zone_flag", [], |r| r.get(0))
        .unwrap();
    assert_eq!(zone_rows, 0);
}

#[test]
fn re_verification_replaces_within_source_keeping_history() {
    let mut app = app_db();
    let cache = seeded_cache(&full_palencia_entries());
    let plot_id = plot_with_reference(&mut app, ["34", "10", "0", "0", "604", "5021", "13"]);

    let first = verify_plot(&mut app, &cache, &plot_id, false)
        .unwrap()
        .unwrap();
    let second = verify_plot(&mut app, &cache, &plot_id, false)
        .unwrap()
        .unwrap();
    assert_ne!(first.feature.id, second.feature.id);

    // One active row per (plot, boundary, sigpac); the replaced row is
    // soft-deleted — fetched geometry stays provable.
    assert_eq!(geo_feature_rows(&app, &plot_id), (1, 2));
}

#[test]
fn verify_plot_needs_an_existing_plot_with_a_complete_reference() {
    let mut app = app_db();
    let cache = seeded_cache::<&str>(&[]);

    assert!(matches!(
        verify_plot(&mut app, &cache, "no-such-plot", false),
        Err(GeoError::NotFound)
    ));

    let farm = insert_farm(
        &mut app,
        NewFarm {
            name: "Sin referencia".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
    )
    .unwrap();
    let bare_plot = insert_plot(
        &mut app,
        NewPlot {
            farm_id: farm.id,
            name: "Sin SIGPAC".into(),
            area_ha: None,
            es: None,
        },
    )
    .unwrap();
    assert!(matches!(
        verify_plot(&mut app, &cache, &bare_plot.id, false),
        Err(GeoError::Invalid("sigpac_ref_missing"))
    ));
}

#[test]
fn unknown_reference_stores_nothing() {
    let mut app = app_db();
    // The service's "no such recinto" is HTTP 200 + empty FeatureCollection.
    let cache = seeded_cache(&[("sigpac/recinfo/34/999/0/0/1/1/1", RECINFO_NOT_FOUND)]);
    let plot_id = plot_with_reference(&mut app, ["34", "999", "0", "0", "1", "1", "1"]);

    assert!(
        verify_plot(&mut app, &cache, &plot_id, false)
            .unwrap()
            .is_none()
    );
    assert_eq!(geo_feature_rows(&app, &plot_id), (0, 0));
}

#[test]
fn lookup_reference_reports_plots_already_carrying_the_ref() {
    let mut app = app_db();
    let cache = seeded_cache(&[(PALENCIA_KEY, RECINFO)]);
    // Zero-padded parts as a user might type them: matching is numeric.
    let plot_id = plot_with_reference(&mut app, ["034", "010", "0", "0", "604", "5021", "13"]);

    let parts: Vec<String> = ["34", "10", "0", "0", "604", "5021", "13"]
        .into_iter()
        .map(String::from)
        .collect();
    let lookup = lookup_reference(&app, &cache, &parts, false)
        .unwrap()
        .expect("recinto exists");

    assert_eq!(lookup.recinto.land_use(), Some("PA"));
    assert_eq!(lookup.matching_plots.len(), 1);
    assert_eq!(lookup.matching_plots[0].plot_id, plot_id);
    assert_eq!(lookup.matching_plots[0].plot_name, "El Páramo");
    assert_eq!(lookup.matching_plots[0].farm_name, "La Vega");
}

#[test]
fn lookup_reference_rejects_wrong_arity() {
    let app = app_db();
    let cache = seeded_cache::<&str>(&[]);
    let six: Vec<String> = ["34", "10", "0", "0", "604", "5021"]
        .into_iter()
        .map(String::from)
        .collect();
    assert!(matches!(
        lookup_reference(&app, &cache, &six, false),
        Err(GeoError::Invalid("sigpac_ref_invalid"))
    ));
}

#[test]
fn lookup_point_matches_plots_by_the_returned_reference() {
    let mut app = app_db();
    let cache = seeded_cache(&[("sigpac/recinfobypoint/-4.77/41.85", RECINFO_BY_POINT)]);
    let plot_id = plot_with_reference(&mut app, ["34", "10", "0", "0", "604", "5021", "13"]);

    let lookup = lookup_point(&app, &cache, -4.77, 41.85)
        .unwrap()
        .expect("a recinto under the point");
    assert_eq!(lookup.recinto.reference.to_path(), "34/10/0/0/604/5021/13");
    assert_eq!(lookup.matching_plots.len(), 1);
    assert_eq!(lookup.matching_plots[0].plot_id, plot_id);
}
