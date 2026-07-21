// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert reconciliation tests (docs/architecture.md testing strategy #2): `refresh_alerts` and the
//! acknowledgement functions against an in-memory database.
//!
//! The pure window/expiry rules are unit-tested in `src/alerts.rs`; these tests cover
//! the reconciliation semantics: idempotency, status preservation, lapse/renewal/soft-
//! delete cleanup, and due-date drift correction.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::alerts::AlertConfig;
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;

/// Reference day for every test. The fixture dates are chosen around it:
///   * treatment applied 2026-06-01, PHI 21 → end 2026-06-22 (window live today);
///   * operator licence expires 2026-07-15 (34 days out, inside the 60-day lead);
///   * machinery ITV due 2026-07-01 (20 days out, inside the 30-day lead).
const TODAY: &str = "2026-06-11";

struct Fixture {
    treatment_id: String,
    operator_id: String,
    machinery_id: String,
}

fn fixture(conn: &mut Connection) -> Fixture {
    let season_id = repo::insert_season(
        conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap()
    .id;

    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let plot_id = repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.clone(),
            name: "Parcela 1".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            licence_number: Some("CL-12345".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2026-07-15".into()),
        },
        None,
    )
    .unwrap()
    .id;

    let machinery_id = repo::insert_machinery(
        conn,
        NewMachinery {
            farm_id: farm_id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            last_inspection_date: Some("2023-07-01".into()),
            next_inspection_due_date: Some("2026-07-01".into()),
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap()
    .id;

    let product_id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: "Fungitop".into(),
            holder: None,
            formulation_type_code: Some("sc".into()),
            default_phi_days: Some(21),
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
        None,
    )
    .unwrap();

    let treatment_id = repo::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id,
            farm_id,
            application_date: "2026-06-01".into(),
            product_id,
            country_code: None,
            dose_value: 1.0,
            dose_unit_code: "l_ha".into(),
            problems: vec![NewTreatmentProblem {
                reason_category_code: "disease".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: None,
            target_organism: None,
            operator_id: operator_id.clone(),
            machinery_id: Some(machinery_id.clone()),
            phi_days_used: None, // falls back to the product's 21-day PHI
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id,
            crop_id: None,
            surface_treated_ha: 3.0,
        }],
        None,
    )
    .unwrap()
    .id;

    Fixture {
        treatment_id,
        operator_id,
        machinery_id,
    }
}

fn alert_for<'a>(alerts: &'a [Alert], type_code: &str) -> &'a Alert {
    alerts
        .iter()
        .find(|a| a.alert_type_code == type_code)
        .unwrap_or_else(|| panic!("expected a {type_code} alert"))
}

// --- generation ---------------------------------------------------------------

#[test]
fn refresh_creates_alerts_for_all_three_conditions() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert_eq!(alerts.len(), 3);

    let phi = alert_for(&alerts, "phi_window");
    assert_eq!(phi.subject_table, "treatment_record");
    assert_eq!(phi.subject_id, fx.treatment_id);
    assert_eq!(phi.due_date.as_deref(), Some("2026-06-22")); // 2026-06-01 + 21 (PHI per product label)
    assert_eq!(phi.lead_days_used, None);
    assert!(
        phi.season_id.is_some(),
        "PHI alerts inherit the treatment's season"
    );

    let licence = alert_for(&alerts, "licence_expiry");
    assert_eq!(licence.subject_id, fx.operator_id);
    assert_eq!(licence.due_date.as_deref(), Some("2026-07-15"));
    assert_eq!(licence.lead_days_used, Some(60));

    let itv = alert_for(&alerts, "itv_expiry");
    assert_eq!(itv.subject_id, fx.machinery_id);
    assert_eq!(itv.due_date.as_deref(), Some("2026-07-01"));
    assert_eq!(itv.lead_days_used, Some(30));

    // Soonest due date first: ITV (07-01) before licence (07-15); PHI (06-22) first.
    let due: Vec<_> = alerts
        .iter()
        .map(|a| a.due_date.as_deref().unwrap())
        .collect();
    assert_eq!(due, ["2026-06-22", "2026-07-01", "2026-07-15"]);
}

#[test]
fn conditions_outside_their_window_produce_no_alerts() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);

    // On 2026-01-15 nothing is live yet: the treatment hasn't happened and both
    // expiry dates are far beyond their lead windows.
    repo::refresh_alerts(&mut conn, "2026-01-15", &AlertConfig::default()).unwrap();
    assert!(repo::list_active_alerts(&conn).unwrap().is_empty());
}

#[test]
fn operator_without_expiry_date_produces_no_alert() {
    let mut conn = open_in_memory().unwrap();
    repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Sin Carné".into(),
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert!(repo::list_active_alerts(&conn).unwrap().is_empty());
}

#[test]
fn multi_plot_treatment_yields_a_single_phi_alert() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Add a second plot to the same farm and a second treatment over both plots.
    let farm_id: String = conn
        .query_row(
            "SELECT farm_id FROM treatment_record WHERE id = ?1",
            [&fx.treatment_id],
            |r| r.get(0),
        )
        .unwrap();
    let plot_a = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm_id.clone(),
            name: "A".into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot_b = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm_id.clone(),
            name: "B".into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let (season_id, product_id): (String, String) = conn
        .query_row(
            "SELECT season_id, product_id FROM treatment_record WHERE id = ?1",
            [&fx.treatment_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        NewTreatmentRecord {
            season_id,
            farm_id,
            application_date: "2026-06-05".into(),
            product_id,
            country_code: None,
            dose_value: 1.0,
            dose_unit_code: "l_ha".into(),
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: None,
            target_organism: None,
            operator_id: fx.operator_id.clone(),
            machinery_id: None,
            phi_days_used: Some(14),
            notes: None,
        },
        vec![
            NewTreatmentPlot {
                plot_id: plot_a,
                crop_id: None,
                surface_treated_ha: 2.0,
            },
            NewTreatmentPlot {
                plot_id: plot_b,
                crop_id: None,
                surface_treated_ha: 2.0,
            },
        ],
        None,
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let phi_alerts: i64 = conn
        .query_row(
            "SELECT count(*) FROM alert WHERE alert_type_code = 'phi_window'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        phi_alerts, 2,
        "one alert per treatment record, regardless of plot count"
    );
}

// --- reconciliation semantics ---------------------------------------------------

#[test]
fn refresh_is_idempotent() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let first = repo::list_active_alerts(&conn).unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let second = repo::list_active_alerts(&conn).unwrap();

    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a.id, b.id, "rows must be kept, not recreated");
        assert_eq!(a.created_at, b.created_at);
        assert_eq!(
            a.updated_at, b.updated_at,
            "an unchanged alert must not be rewritten"
        );
    }
}

#[test]
fn dismissed_alert_survives_refresh_and_stays_hidden() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    let alerts = repo::list_active_alerts(&conn).unwrap();
    let licence_id = alert_for(&alerts, "licence_expiry").id.clone();
    repo::dismiss_alert(&mut conn, &licence_id).unwrap();

    // Condition still holds → the row survives, status untouched by the refresh.
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    let visible = repo::list_active_alerts(&conn).unwrap();
    assert!(
        visible.iter().all(|a| a.id != licence_id),
        "dismissed alerts are hidden"
    );
    let status: String = conn
        .query_row(
            "SELECT status FROM alert WHERE id = ?1",
            [&licence_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        status, "dismissed",
        "refresh must never resurrect a dismissal"
    );
}

#[test]
fn lapsed_phi_window_removes_the_alert() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert_eq!(repo::list_active_alerts(&conn).unwrap().len(), 3);

    // On the PHI end date (2026-06-22) harvest is allowed again: the alert lapses.
    repo::refresh_alerts(&mut conn, "2026-06-22", &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert!(alerts.iter().all(|a| a.alert_type_code != "phi_window"));
    assert_eq!(alerts.len(), 2, "the expiry alerts are still live");
}

#[test]
fn licence_renewal_removes_the_alert() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    // Renewal: the expiry date moves out beyond the lead window. (Direct SQL — there is
    // no update_operator yet; the reconciliation must react to the data, not the API.)
    conn.execute(
        "UPDATE operator SET licence_expiry_date = '2031-07-15' WHERE id = ?1",
        [&fx.operator_id],
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert!(alerts.iter().all(|a| a.alert_type_code != "licence_expiry"));
}

#[test]
fn soft_deleted_subject_removes_the_alert() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    conn.execute(
        "UPDATE machinery SET deleted_at = '2026-06-11T08:00:00Z' WHERE id = ?1",
        [&fx.machinery_id],
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert!(alerts.iter().all(|a| a.alert_type_code != "itv_expiry"));
}

#[test]
fn drifted_due_date_is_corrected_in_place() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    let alerts = repo::list_active_alerts(&conn).unwrap();
    let itv = alert_for(&alerts, "itv_expiry");
    let (itv_id, itv_created_at) = (itv.id.clone(), itv.created_at.clone());
    repo::acknowledge_alert(&mut conn, &itv_id).unwrap();

    // The ITV date is corrected but stays inside the 30-day lead window.
    conn.execute(
        "UPDATE machinery SET next_inspection_due_date = '2026-07-05' WHERE id = ?1",
        [&fx.machinery_id],
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let refreshed = repo::list_active_alerts(&conn).unwrap();
    let itv = alert_for(&refreshed, "itv_expiry");
    assert_eq!(itv.id, itv_id, "the same row is corrected, not replaced");
    assert_eq!(itv.created_at, itv_created_at);
    assert_eq!(
        itv.due_date.as_deref(),
        Some("2026-07-05"),
        "derived due_date must not drift"
    );
    assert_eq!(
        itv.status, "acknowledged",
        "status is preserved through the correction"
    );
}

// --- acknowledgement state ------------------------------------------------------

#[test]
fn acknowledge_marks_the_alert_but_keeps_it_listed() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    let phi_id = alert_for(&repo::list_active_alerts(&conn).unwrap(), "phi_window")
        .id
        .clone();
    repo::acknowledge_alert(&mut conn, &phi_id).unwrap();

    let alerts = repo::list_active_alerts(&conn).unwrap();
    let phi = alerts
        .iter()
        .find(|a| a.id == phi_id)
        .expect("acknowledged alerts stay visible");
    assert_eq!(phi.status, "acknowledged");
    assert!(phi.acknowledged_at.is_some());
}

#[test]
fn acknowledge_and_dismiss_unknown_alert_fail_with_not_found() {
    let mut conn = open_in_memory().unwrap();
    let missing = "01890000-0000-7000-8000-000000000000";
    assert!(matches!(
        repo::acknowledge_alert(&mut conn, missing),
        Err(module_cue::CueError::NotFound)
    ));
    assert!(matches!(
        repo::dismiss_alert(&mut conn, missing),
        Err(module_cue::CueError::NotFound)
    ));
}

#[test]
fn alert_rows_are_not_audit_logged() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();

    // Derived state is excluded from the audit/sync log by design (2026-06-11 decision).
    let logged: i64 = conn
        .query_row(
            "SELECT count(*) FROM record_change WHERE entity_table = 'alert'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 0);
}

// ---------------------------------------------------------------------------
// Zone-flag alerts (P4, 2026-07-08). Source table: core's plot_zone_flag.
// Identity is the PLOT (standing condition), not the flag row: a dismissal
// survives re-checks and campaign rollovers; due_date drift-corrects to the
// latest campaign's year end.
// ---------------------------------------------------------------------------

use terrazgo_core::models::NewZoneFlag;
use terrazgo_core::repository::{replace_zone_flags, soft_delete_plot};

fn zoned_plot(conn: &mut Connection) -> String {
    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Zonas".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id,
            name: "P1".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

fn flag(zone: &str, status: &str) -> NewZoneFlag {
    NewZoneFlag {
        zone_type_code: zone.into(),
        status: status.into(),
        coverage_pct: (status == "inside").then_some(100.0),
        detail: None,
    }
}

#[test]
fn inside_flags_alert_per_zone_type_and_outside_ones_do_not() {
    let mut conn = open_in_memory().unwrap();
    let plot_id = zoned_plot(&mut conn);
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2026,
        "sigpac",
        vec![
            flag("nitrate_vulnerable", "inside"),
            flag("phytosanitary_restriction", "inside"),
            flag("natura_2000", "outside"),
        ],
        None,
    )
    .unwrap();

    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert_eq!(alerts.len(), 2);

    let nitrate = alert_for(&alerts, "nitrate_zone");
    assert_eq!(nitrate.subject_table, "plot");
    assert_eq!(nitrate.subject_id, plot_id);
    // Standing condition for the campaign: due date = campaign year end.
    assert_eq!(nitrate.due_date.as_deref(), Some("2026-12-31"));
    alert_for(&alerts, "phyto_zone");
    assert!(!alerts.iter().any(|a| a.alert_type_code == "natura_zone"));
}

#[test]
fn only_the_latest_campaign_counts() {
    let mut conn = open_in_memory().unwrap();
    let plot_id = zoned_plot(&mut conn);
    // Flagged in 2026, then cleared by the 2027 check: no alert survives.
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2026,
        "sigpac",
        vec![flag("nitrate_vulnerable", "inside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert_eq!(repo::list_active_alerts(&conn).unwrap().len(), 1);

    replace_zone_flags(
        &mut conn,
        &plot_id,
        2027,
        "sigpac",
        vec![flag("nitrate_vulnerable", "outside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert!(repo::list_active_alerts(&conn).unwrap().is_empty());

    // Flagged again in 2028: the alert returns with the new campaign's date.
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2028,
        "sigpac",
        vec![flag("nitrate_vulnerable", "inside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    assert_eq!(
        alert_for(&alerts, "nitrate_zone").due_date.as_deref(),
        Some("2028-12-31")
    );
}

#[test]
fn dismissed_zone_alert_survives_rechecks_and_rollover() {
    let mut conn = open_in_memory().unwrap();
    let plot_id = zoned_plot(&mut conn);
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2026,
        "sigpac",
        vec![flag("nitrate_vulnerable", "inside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    let alerts = repo::list_active_alerts(&conn).unwrap();
    repo::dismiss_alert(&mut conn, &alert_for(&alerts, "nitrate_zone").id).unwrap();

    // Re-check (replaces the flag row) and roll the campaign: the condition
    // still holds, identity is the plot — the dismissal must NOT resurrect.
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2027,
        "sigpac",
        vec![flag("nitrate_vulnerable", "inside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert!(
        !repo::list_active_alerts(&conn)
            .unwrap()
            .iter()
            .any(|a| a.alert_type_code == "nitrate_zone")
    );
}

#[test]
fn zone_alerts_lapse_with_the_plot() {
    let mut conn = open_in_memory().unwrap();
    let plot_id = zoned_plot(&mut conn);
    replace_zone_flags(
        &mut conn,
        &plot_id,
        2026,
        "sigpac",
        vec![flag("nitrate_vulnerable", "inside")],
        None,
    )
    .unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert_eq!(repo::list_active_alerts(&conn).unwrap().len(), 1);

    soft_delete_plot(&mut conn, &plot_id, None).unwrap();
    repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::default()).unwrap();
    assert!(repo::list_active_alerts(&conn).unwrap().is_empty());
}
