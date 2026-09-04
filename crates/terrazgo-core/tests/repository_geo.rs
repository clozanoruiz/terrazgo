// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The geometry and the flags hung off a plot: `geo_feature`'s exclusive-arc
//! storage, `plot_zone_flag` (where a stored 'outside' is proof-of-check rather
//! than an absence), and the water points of model 2.2 — including the stored
//! negative, "this plot has none", which is an answer and not a silence.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// Geo features (exclusive-arc geometry storage)
// ---------------------------------------------------------------------------

const SQUARE: &str = r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.66],[-4.72,41.65]]]}"#;
const SQUARE_B: &str = r#"{"type":"Polygon","coordinates":[[[-4.62,41.55],[-4.61,41.55],[-4.61,41.56],[-4.62,41.56],[-4.62,41.55]]]}"#;

fn boundary_for_plot(plot_id: &str, source: &str, geometry: &str) -> NewGeoFeature {
    NewGeoFeature {
        plot_id: Some(plot_id.into()),
        farm_id: None,
        role: "boundary".into(),
        geometry: geometry.into(),
        source: source.into(),
        campaign: None,
        official_area_ha: None,
        properties: None,
        fetched_at: None,
    }
}

#[test]
fn save_geo_feature_inserts_and_logs_complete_image() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let feature = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    assert_eq!(feature.plot_id.as_deref(), Some(plot.id.as_str()));
    assert!(feature.farm_id.is_none());

    let (op, before, after) = last_change(&conn, "geo_feature", &feature.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: every column present, not a subset.
    assert_eq!(after["role"], "boundary");
    assert_eq!(after["source"], "manual");
    assert_eq!(after["geometry"], SQUARE);
    assert!(after.get("created_at").is_some());
    assert!(after.get("official_area_ha").is_some());

    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, feature.id);
}

#[test]
fn save_geo_feature_replaces_active_row_within_same_source() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let first = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    let second = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE_B),
        None,
    )
    .unwrap();

    // Replacement soft-deletes the first row (history kept), with full images.
    let (op, before, after) = last_change(&conn, "geo_feature", &first.id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(!after["deleted_at"].is_null());
    assert_eq!(after["geometry"], SQUARE);

    // Only the new row is active; the old row still exists physically.
    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, second.id);
    assert_eq!(listed[0].geometry, SQUARE_B);
    let raw: i64 = conn
        .query_row("SELECT COUNT(*) FROM geo_feature", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, 2);
}

#[test]
fn geo_feature_sources_coexist() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "import", SQUARE_B),
        None,
    )
    .unwrap();

    // A manual boundary and an imported one are both active (discrepancy
    // display case), because replacement is scoped to (subject, role, source).
    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn geo_feature_farm_arc_saves_and_lists() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();

    let feature = repo::save_geo_feature(
        &mut conn,
        NewGeoFeature {
            plot_id: None,
            farm_id: Some(farm.id.clone()),
            role: "boundary".into(),
            geometry: SQUARE.into(),
            source: "manual".into(),
            campaign: None,
            official_area_ha: None,
            properties: None,
            fetched_at: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(feature.farm_id.as_deref(), Some(farm.id.as_str()));

    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
}

#[test]
fn geo_feature_arc_validation_rejects_bad_shapes() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let mut no_subject = boundary_for_plot(&plot.id, "manual", SQUARE);
    no_subject.plot_id = None;
    assert!(matches!(
        repo::save_geo_feature(&mut conn, no_subject, None),
        Err(CoreError::Invalid("geo_subject_missing"))
    ));

    let mut both_subjects = boundary_for_plot(&plot.id, "manual", SQUARE);
    both_subjects.farm_id = Some(farm.id.clone());
    assert!(matches!(
        repo::save_geo_feature(&mut conn, both_subjects, None),
        Err(CoreError::Invalid("geo_subject_ambiguous"))
    ));
}

#[test]
fn geo_feature_requires_active_subject() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    // Unknown plot id.
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot("no-such-plot", "manual", SQUARE),
            None
        ),
        Err(CoreError::NotFound)
    ));

    // Soft-deleted plot: hidden subjects don't take geometry.
    repo::soft_delete_plot(&mut conn, &plot.id, None).unwrap();
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot(&plot.id, "manual", SQUARE),
            None
        ),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn geo_feature_rejects_invalid_geometry() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let unclosed = r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.70,41.60]]]}"#;
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot(&plot.id, "manual", unclosed),
            None
        ),
        Err(CoreError::Invalid("geometry_invalid"))
    ));
}

#[test]
fn soft_delete_geo_feature_hides_row_and_logs() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();
    let feature = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();

    repo::soft_delete_geo_feature(&mut conn, &feature.id, None).unwrap();

    assert!(
        repo::list_geo_features_for_farm(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
    let (op, _, after) = last_change(&conn, "geo_feature", &feature.id);
    assert_eq!(op, "delete");
    assert!(!after["deleted_at"].is_null());

    // Second delete: already hidden.
    assert!(matches!(
        repo::soft_delete_geo_feature(&mut conn, &feature.id, None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn list_geo_features_is_scoped_to_the_farm() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("B"), None).unwrap();
    let plot_a = repo::insert_plot(&mut conn, new_plot(&farm_a.id, "Recinto A"), None).unwrap();
    let plot_b = repo::insert_plot(&mut conn, new_plot(&farm_b.id, "Recinto B"), None).unwrap();

    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot_a.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot_b.id, "manual", SQUARE_B),
        None,
    )
    .unwrap();

    let listed = repo::list_geo_features_for_farm(&conn, &farm_a.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].plot_id.as_deref(), Some(plot_a.id.as_str()));
}

// ---------------------------------------------------------------------------
// Zone flags (plot_zone_flag)
// ---------------------------------------------------------------------------

fn zone_flag(zone: &str, status: &str, pct: Option<f64>) -> NewZoneFlag {
    NewZoneFlag {
        zone_type_code: zone.into(),
        status: status.into(),
        coverage_pct: pct,
        detail: None,
    }
}

#[test]
fn replace_zone_flags_stores_results_and_logs_inserts() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    let stored = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![
            zone_flag("nitrate_vulnerable", "inside", Some(100.0)),
            zone_flag("phytosanitary_restriction", "inside", Some(99.9)),
            // Negative results are stored too: proof the check ran and was clear.
            zone_flag("natura_2000", "outside", None),
        ],
        None,
    )
    .unwrap();
    assert_eq!(stored.len(), 3);
    assert!(
        stored
            .iter()
            .all(|f| f.campaign == 2026 && f.source == "sigpac")
    );
    let natura = stored
        .iter()
        .find(|f| f.zone_type_code == "natura_2000")
        .unwrap();
    assert_eq!(natura.status, "outside");
    assert_eq!(natura.coverage_pct, None);

    // Complete after-images in the audit log (sync delta contract).
    let (op, _, after) = last_change(&conn, "plot_zone_flag", &stored[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["plot_id"], plot.id);
    assert_eq!(after["campaign"], 2026);
    assert_eq!(after["status"], "inside");
}

#[test]
fn recheck_replaces_within_campaign_and_appends_across_campaigns() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    let first = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "outside", None)],
        None,
    )
    .unwrap();
    // Re-check the SAME campaign: the zone declaration changed → replace.
    let second = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();
    // A NEW campaign appends; the 2026 history stays provable.
    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2027,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();

    // Both campaigns' rows are live in the TABLE — that is what "appends across
    // campaigns" means, and it is asserted here rather than through the listing
    // because the listing answers a different question: where the plot stands
    // now, one row per (plot, zone type).
    let live: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plot_zone_flag WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(live, 2, "one live row per campaign");

    let standing = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(standing.len(), 1, "the plot stands one way, not two");
    assert_eq!(
        standing[0].campaign, 2027,
        "the newest check is the standing"
    );
    assert_eq!(standing[0].status, "inside");

    // The replaced 2026 row is soft-deleted with a delete log, not erased.
    let (op, before, after) = last_change(&conn, "plot_zone_flag", &first[0].id);
    assert_eq!(op, "delete");
    assert_eq!(before["status"], "outside");
    assert!(after["deleted_at"].is_string());
    assert_ne!(first[0].id, second[0].id);
}

#[test]
fn zone_flags_validate_status_and_plot() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    assert!(matches!(
        repo::replace_zone_flags(
            &mut conn,
            &plot.id,
            2026,
            "sigpac",
            vec![zone_flag("nitrate_vulnerable", "maybe", None)],
            None,
        ),
        Err(CoreError::Invalid("zone_status_invalid"))
    ));
    assert!(matches!(
        repo::replace_zone_flags(&mut conn, "missing-plot", 2026, "sigpac", vec![], None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn zone_flag_listing_is_scoped_to_the_farms_active_plots() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Mine"), None).unwrap();
    let other = repo::insert_farm(&mut conn, new_farm("Other"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();
    let foreign = repo::insert_plot(&mut conn, new_plot(&other.id, "P2"), None).unwrap();

    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("natura_2000", "inside", Some(12.5))],
        None,
    )
    .unwrap();
    repo::replace_zone_flags(
        &mut conn,
        &foreign.id,
        2026,
        "sigpac",
        vec![zone_flag("natura_2000", "inside", Some(50.0))],
        None,
    )
    .unwrap();

    let flags = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].plot_id, plot.id);

    // Deleting the plot hides its flags from the listing.
    repo::soft_delete_plot(&mut conn, &plot.id, None).unwrap();
    assert!(
        repo::list_zone_flags_for_farm(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
}

// --- the current standing (2026-08-24) --------------------------------------
//
// `plot_zone_flag` appends across campaigns, so it grows by (plots × zone
// kinds) every year while every reader asks a question of fixed size: where
// does this plot stand today? The reduction is the repository's, so the plot
// card, the map layer and the alert engine cannot answer it three different
// ways.

#[test]
fn the_standing_is_resolved_per_plot_and_zone_kind_not_once_for_the_holding() {
    // The trap a single MAX(campaign) over the holding would fall into: a plot
    // nobody re-verified this year would silently lose its chip.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let recent = repo::insert_plot(&mut conn, new_plot(&farm.id, "Reciente"), None).unwrap();
    let stale = repo::insert_plot(&mut conn, new_plot(&farm.id, "Antigua"), None).unwrap();

    repo::replace_zone_flags(
        &mut conn,
        &stale.id,
        2020,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();
    repo::replace_zone_flags(
        &mut conn,
        &recent.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();

    let standing = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(standing.len(), 2, "both plots still stand somewhere");
    let old = standing.iter().find(|f| f.plot_id == stale.id).unwrap();
    assert_eq!(old.campaign, 2020, "a stale check is still the standing");
}

#[test]
fn a_zone_kind_keeps_its_own_standing_when_a_later_check_covered_another() {
    // A 2027 check for Natura must not hide the 2026 nitrate answer: the
    // partition is (plot, zone type), not the plot.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();
    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2027,
        "sigpac",
        vec![zone_flag("natura_2000", "inside", Some(30.0))],
        None,
    )
    .unwrap();

    let standing = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(standing.len(), 2);
    let nitrate = standing
        .iter()
        .find(|f| f.zone_type_code == "nitrate_vulnerable")
        .unwrap();
    assert_eq!(nitrate.campaign, 2026);
}

#[test]
fn within_one_campaign_a_provider_saying_inside_decides_the_standing() {
    // Two sources, same campaign, disagreeing. 'inside' wins: it is the answer
    // that carries a duty, and it is what the alert engine already concluded on
    // its own before this reduction existed.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "outside", None)],
        None,
    )
    .unwrap();
    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "regional",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(80.0))],
        None,
    )
    .unwrap();

    let standing = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(standing.len(), 1, "one standing per (plot, zone type)");
    assert_eq!(standing[0].status, "inside");
    assert_eq!(standing[0].source, "regional");
}

#[test]
fn the_holding_wide_listing_reduces_the_same_way_and_spans_farms() {
    // What the alert engine reads: every farm, same one-row-per-pair rule.
    let mut conn = db();
    let mine = repo::insert_farm(&mut conn, new_farm("Mine"), None).unwrap();
    let other = repo::insert_farm(&mut conn, new_farm("Other"), None).unwrap();
    let a = repo::insert_plot(&mut conn, new_plot(&mine.id, "P1"), None).unwrap();
    let b = repo::insert_plot(&mut conn, new_plot(&other.id, "P2"), None).unwrap();

    for plot in [&a, &b] {
        for campaign in [2025, 2026] {
            repo::replace_zone_flags(
                &mut conn,
                &plot.id,
                campaign,
                "sigpac",
                vec![zone_flag("natura_2000", "inside", Some(10.0))],
                None,
            )
            .unwrap();
        }
    }

    let standing = repo::list_latest_zone_flags(&conn).unwrap();
    assert_eq!(standing.len(), 2, "one row per plot, both farms");
    assert!(standing.iter().all(|f| f.campaign == 2026));
}

// ---------------------------------------------------------------------------
// Water points (model 2.2's water half, Anexo III A.1.f–g)
// ---------------------------------------------------------------------------

/// A farm with two plots — enough to prove the lists are farm-scoped and the
/// declaration is per plot.
struct WaterFixture {
    farm_id: String,
    plot_id: String,
    other_plot_id: String,
}

fn water_fixture(conn: &mut Connection) -> WaterFixture {
    let farm = repo::insert_farm(conn, new_farm("Finca del agua"), None).unwrap();
    let plot = repo::insert_plot(conn, new_plot(&farm.id, "La Vega"), None).unwrap();
    let other = repo::insert_plot(conn, new_plot(&farm.id, "El Soto"), None).unwrap();
    WaterFixture {
        farm_id: farm.id,
        plot_id: plot.id,
        other_plot_id: other.id,
    }
}

fn new_water_point(plot_id: &str) -> NewWaterPoint {
    NewWaterPoint {
        plot_id: plot_id.into(),
        denomination: "Pozo del norte".into(),
        inside_plot: true,
        distance_m: None,
        latitude: None,
        longitude: None,
    }
}

#[test]
fn insert_water_point_round_trips_and_logs_a_full_image() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut new = new_water_point(&fx.plot_id);
    new.inside_plot = false;
    new.distance_m = Some(120.0);
    new.latitude = Some(41.65234);
    new.longitude = Some(-4.72891);
    let saved = repo::insert_water_point(&mut conn, new, None).unwrap();

    assert_eq!(saved.denomination, "Pozo del norte");
    assert!(!saved.inside_plot);
    assert_eq!(saved.distance_m, Some(120.0));

    let points = repo::list_water_points(&conn, &fx.farm_id).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].id, saved.id);
    assert_eq!(points[0].latitude, Some(41.65234));

    let (op, before, after) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image, not a field subset: the log is the sync delta source.
    assert_eq!(after["denomination"], "Pozo del norte");
    assert_eq!(after["inside_plot"], false);
    assert_eq!(after["distance_m"], 120.0);
    assert_eq!(after["longitude"], -4.72891);
}

/// Anexo III A.1.g asks for the distance when the point lies outside the plot,
/// and it is knowledge the farmer already has — so it is required, unlike the
/// values that are only observed later (efficacy, total quantity used).
#[test]
fn a_point_outside_the_plot_must_state_its_distance() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut outside = new_water_point(&fx.plot_id);
    outside.inside_plot = false;
    assert!(matches!(
        repo::insert_water_point(&mut conn, outside, None).unwrap_err(),
        CoreError::Invalid("missing_distance")
    ));

    // Zero is not a distance to something outside the plot.
    let mut zero = new_water_point(&fx.plot_id);
    zero.inside_plot = false;
    zero.distance_m = Some(0.0);
    assert!(matches!(
        repo::insert_water_point(&mut conn, zero, None).unwrap_err(),
        CoreError::Invalid("missing_distance")
    ));
}

/// A distance beside "included in the plot: YES" contradicts the cell next to
/// it — a wrong answer, not a missing one.
#[test]
fn a_point_inside_the_plot_cannot_carry_a_distance() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut inside = new_water_point(&fx.plot_id);
    inside.distance_m = Some(15.0);
    assert!(matches!(
        repo::insert_water_point(&mut conn, inside, None).unwrap_err(),
        CoreError::Invalid("water_point_distance_inside")
    ));
}

#[test]
fn water_point_coordinates_are_both_or_neither_and_in_range() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut half = new_water_point(&fx.plot_id);
    half.latitude = Some(41.65);
    assert!(matches!(
        repo::insert_water_point(&mut conn, half, None).unwrap_err(),
        CoreError::Invalid("water_point_coordinates_invalid")
    ));

    let mut off_globe = new_water_point(&fx.plot_id);
    off_globe.latitude = Some(91.0);
    off_globe.longitude = Some(-4.7);
    assert!(matches!(
        repo::insert_water_point(&mut conn, off_globe, None).unwrap_err(),
        CoreError::Invalid("water_point_coordinates_invalid")
    ));

    // Stating neither is the normal case: the model marks the column voluntary.
    let bare = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();
    assert_eq!((bare.latitude, bare.longitude), (None, None));
}

#[test]
fn water_point_validation_rejects_a_blank_denomination() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let mut blank = new_water_point(&fx.plot_id);
    blank.denomination = "   ".into();
    assert!(matches!(
        repo::insert_water_point(&mut conn, blank, None).unwrap_err(),
        CoreError::Invalid("empty_name")
    ));
}

/// Fully correctable, unlike the treatment registers: the row freezes no
/// snapshot of another row, so there is nothing an edit could rewrite.
#[test]
fn update_water_point_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let saved = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    let after = repo::update_water_point(
        &mut conn,
        &saved.id,
        UpdateWaterPoint {
            denomination: "  Sondeo municipal  ".into(),
            inside_plot: false,
            distance_m: Some(240.5),
            latitude: None,
            longitude: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(after.denomination, "Sondeo municipal");
    assert!(!after.inside_plot);
    assert_eq!(after.distance_m, Some(240.5));
    // The plot it belongs to is not part of the update.
    assert_eq!(after.plot_id, saved.plot_id);

    let (op, before, logged) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "update");
    assert_eq!(before["denomination"], "Pozo del norte");
    assert_eq!(before["inside_plot"], true);
    assert_eq!(logged["denomination"], "Sondeo municipal");
    assert_eq!(logged["distance_m"], 240.5);
}

#[test]
fn soft_delete_water_point_hides_it_and_logs_both_images() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let saved = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    repo::soft_delete_water_point(&mut conn, &saved.id, None).unwrap();
    assert!(
        repo::list_water_points(&conn, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    let (op, before, after) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "delete");
    assert_eq!(before["denomination"], "Pozo del norte");
    assert!(after["deleted_at"].is_string());

    assert!(matches!(
        repo::soft_delete_water_point(&mut conn, &saved.id, None).unwrap_err(),
        CoreError::NotFound
    ));
}

#[test]
fn water_points_are_listed_per_farm_and_skip_deleted_plots() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let elsewhere = repo::insert_farm(&mut conn, new_farm("Otra finca"), None).unwrap();
    let far_plot = repo::insert_plot(&mut conn, new_plot(&elsewhere.id, "Lejos"), None)
        .unwrap()
        .id;

    repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();
    repo::insert_water_point(&mut conn, new_water_point(&far_plot), None).unwrap();
    let on_other =
        repo::insert_water_point(&mut conn, new_water_point(&fx.other_plot_id), None).unwrap();

    assert_eq!(
        repo::list_water_points(&conn, &fx.farm_id).unwrap().len(),
        2
    );

    // A point leaves the book with its plot; its audit history stays reachable.
    repo::soft_delete_plot(&mut conn, &fx.other_plot_id, None).unwrap();
    let left = repo::list_water_points(&conn, &fx.farm_id).unwrap();
    assert_eq!(left.len(), 1);
    assert!(left.iter().all(|p| p.id != on_other.id));
}

#[test]
fn a_water_point_needs_an_active_plot() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    repo::soft_delete_plot(&mut conn, &fx.plot_id, None).unwrap();
    assert!(matches!(
        repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap_err(),
        CoreError::NotFound
    ));
}

// --- the stored negative ----------------------------------------------------

#[test]
fn declaring_a_plot_free_of_water_points_round_trips_and_is_logged() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let declared = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();
    assert_eq!(declared.declared_on, "2026-05-12");

    let standing = repo::list_water_declarations(&conn, &fx.farm_id).unwrap();
    assert_eq!(standing.len(), 1);
    assert_eq!(standing[0].plot_id, fx.plot_id);

    let (op, before, after) = last_change(&conn, "plot_water_declaration", &declared.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    assert_eq!(after["declared_on"], "2026-05-12");

    // Restating updates the standing row rather than printing the plot twice.
    let again = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-06-01", None).unwrap();
    assert_eq!(again.id, declared.id);
    assert_eq!(
        repo::list_water_declarations(&conn, &fx.farm_id)
            .unwrap()
            .len(),
        1
    );
}

/// First direction of the invariant: the rows and the "nothing here" contradict
/// each other, and the rows are the stronger statement.
#[test]
fn a_plot_holding_water_points_cannot_be_declared_free_of_them() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let point = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    assert!(matches!(
        repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap_err(),
        CoreError::Invalid("plot_has_water_points")
    ));

    // The declaration is per plot: its neighbour is unaffected.
    assert!(repo::set_water_declaration(&mut conn, &fx.other_plot_id, "2026-05-12", None).is_ok());

    // Removing the point re-opens the question.
    repo::soft_delete_water_point(&mut conn, &point.id, None).unwrap();
    assert!(repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).is_ok());
}

/// Second direction: a stale "no captaciones" printing beside a contradicting
/// row would forge proof-of-check, so the record withdraws it as it lands.
#[test]
fn recording_a_water_point_withdraws_a_standing_declaration() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let declared = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();
    repo::set_water_declaration(&mut conn, &fx.other_plot_id, "2026-05-12", None).unwrap();

    repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    let standing = repo::list_water_declarations(&conn, &fx.farm_id).unwrap();
    assert_eq!(
        standing.len(),
        1,
        "only this plot's declaration is withdrawn"
    );
    assert_eq!(standing[0].plot_id, fx.other_plot_id);

    // Withdrawal is a soft delete: the trail keeps saying what was declared.
    let (op, before, after) = last_change(&conn, "plot_water_declaration", &declared.id);
    assert_eq!(op, "delete");
    assert_eq!(before["declared_on"], "2026-05-12");
    assert!(after["deleted_at"].is_string());
}

#[test]
fn clearing_a_declaration_is_a_soft_delete_and_restating_mints_a_new_row() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let first = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();

    repo::clear_water_declaration(&mut conn, &fx.plot_id, None).unwrap();
    assert!(
        repo::list_water_declarations(&conn, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    // Clearing nothing is not an error — the panel toggles freely.
    assert!(repo::clear_water_declaration(&mut conn, &fx.plot_id, None).is_ok());

    let second = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-07-03", None).unwrap();
    assert_ne!(
        second.id, first.id,
        "a withdrawn declaration is not resurrected"
    );
}

#[test]
fn a_declaration_needs_an_active_plot() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    repo::soft_delete_plot(&mut conn, &fx.plot_id, None).unwrap();
    assert!(matches!(
        repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap_err(),
        CoreError::NotFound
    ));
}
