// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The alerts this module raises (docs/architecture.md testing strategy #2):
//! `current_alerts` against an in-memory database, read the way the Status
//! view reads them — raised here, assembled by core.
//!
//! Nothing stores an alert (docs/data-model.md → "Alerts: the settled design"),
//! so what these tests pin is what the registers say on a given day: which
//! conditions hold, under which lead times, named how — and that an act made on
//! a phytosanitary alert means the deadline it saw. The pure window and expiry
//! rules are unit-tested in `src/alerts.rs`; the list and the acts themselves
//! in core's `tests/alerts.rs`; the acts between devices in `alert_roaming.rs`.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_phytosanitary::PhytosanitaryError;
use module_phytosanitary::alerts::{ALERT_KINDS, AlertConfig};
use module_phytosanitary::models::*;
use module_phytosanitary::open_in_memory;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;
use terrazgo_core::alerts::{Alert, AlertKind, AlertReport, AlertStatus, RaisedAlert};
use terrazgo_core::repository as core_repo;

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
    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
            ..NewFarm::default()
        },
        None,
    )
    .unwrap()
    .id;

    let season_id = repo::insert_season(
        conn,
        NewSeason {
            farm_id: farm_id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2026".into()),
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
            tax_id: None,
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
            acquired_on: None,
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
            application_end_date: None,
            drying_date: None,
            application_time: None,
            product_id: Some(product_id),
            country_code: None,
            dose_value: Some(1.0),
            dose_unit_code: Some("l_ha".into()),
            total_quantity_value: None,
            total_quantity_unit_code: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "disease".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: None,
            target_organism: None,
            operator_id: operator_id.clone(),
            machinery_id: Some(machinery_id.clone()),
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            measure_basic_substance_code: None,
            phi_days_used: None, // falls back to the product's 21-day PHI
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
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

/// What this module reports on `today` under `config`.
fn report(conn: &Connection, today: &str, config: &AlertConfig) -> AlertReport {
    repo::current_alerts(conn, today, config).unwrap()
}

/// The alerts it raises.
fn raised(conn: &Connection, today: &str, config: &AlertConfig) -> Vec<RaisedAlert> {
    report(conn, today, config).raised
}

/// The list as the Status view gets it — this module's alerts, assembled by
/// core — under `config`.
fn listed_with(conn: &Connection, today: &str, config: &AlertConfig) -> Vec<Alert> {
    core_repo::list_alerts(conn, raised(conn, today, config), today).unwrap()
}

/// The same, under the default lead times.
fn listed(conn: &Connection, today: &str) -> Vec<Alert> {
    listed_with(conn, today, &AlertConfig::defaults())
}

fn alert_for<'a>(alerts: &'a [Alert], type_code: &str) -> &'a Alert {
    alerts
        .iter()
        .find(|a| a.alert_type_code == type_code)
        .unwrap_or_else(|| panic!("expected a {type_code} alert"))
}

fn has(alerts: &[Alert], type_code: &str) -> bool {
    alerts.iter().any(|a| a.alert_type_code == type_code)
}

/// What the shell does with the code a card sends back.
fn kind_of(alert: &Alert) -> AlertKind {
    ALERT_KINDS
        .iter()
        .copied()
        .find(|kind| kind.code() == alert.alert_type_code)
        .expect("one of this module's kinds")
}

/// Hide an alert as the card showed it.
fn dismiss(conn: &mut Connection, alert: &Alert) {
    core_repo::dismiss_alert(
        conn,
        kind_of(alert),
        &alert.subject_id,
        alert.due_date.as_deref(),
        None,
    )
    .unwrap();
}

/// Mark an alert as seen, as the card showed it.
fn acknowledge(conn: &mut Connection, alert: &Alert, actor: Option<&str>) {
    core_repo::acknowledge_alert(
        conn,
        kind_of(alert),
        &alert.subject_id,
        alert.due_date.as_deref(),
        actor,
    )
    .unwrap();
}

fn log_rows(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM record_change", [], |r| r.get(0))
        .unwrap()
}

/// A second treatment on the fixture's farm, over new plots with these names.
fn treatment_over(conn: &mut Connection, fx: &Fixture, plot_names: &[&str]) -> String {
    let (farm_id, season_id, product_id): (String, String, String) = conn
        .query_row(
            "SELECT farm_id, season_id, product_id FROM treatment_record WHERE id = ?1",
            [&fx.treatment_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let plots: Vec<NewTreatmentPlot> = plot_names
        .iter()
        .map(|name| {
            let plot_id = repo::insert_plot(
                conn,
                NewPlot {
                    farm_id: farm_id.clone(),
                    name: (*name).into(),
                    area_ha: Some(1.0),
                    es: None,
                },
                None,
            )
            .unwrap()
            .id;
            NewTreatmentPlot {
                plot_id,
                crop_id: None,
                surface_treated_ha: 1.0,
                growth_stage_code: None,
            }
        })
        .collect();
    repo::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id,
            farm_id,
            application_date: "2026-06-05".into(),
            application_end_date: None,
            drying_date: None,
            application_time: None,
            product_id: Some(product_id),
            country_code: None,
            dose_value: Some(1.0),
            dose_unit_code: Some("l_ha".into()),
            total_quantity_value: None,
            total_quantity_unit_code: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: None,
            target_organism: None,
            operator_id: fx.operator_id.clone(),
            machinery_id: None,
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            measure_basic_substance_code: None,
            phi_days_used: Some(14),
            notes: None,
        },
        plots,
        None,
    )
    .unwrap()
    .id
}

// --- what is raised -------------------------------------------------------------

#[test]
fn the_three_conditions_each_raise_an_alert() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let alerts = listed(&conn, TODAY);
    assert_eq!(alerts.len(), 3);

    let phi = alert_for(&alerts, "phi_window");
    assert_eq!(phi.subject_table, "treatment_record");
    assert_eq!(phi.subject_id, fx.treatment_id);
    assert_eq!(phi.due_date.as_deref(), Some("2026-06-22")); // 2026-06-01 + 21 (PHI per product label)

    let licence = alert_for(&alerts, "licence_expiry");
    assert_eq!(licence.subject_table, "operator");
    assert_eq!(licence.subject_id, fx.operator_id);
    assert_eq!(licence.due_date.as_deref(), Some("2026-07-15"));

    let itv = alert_for(&alerts, "itv_expiry");
    assert_eq!(itv.subject_table, "machinery");
    assert_eq!(itv.subject_id, fx.machinery_id);
    assert_eq!(itv.due_date.as_deref(), Some("2026-07-01"));

    // Soonest due date first: PHI (06-22), then ITV (07-01), then licence (07-15).
    let due: Vec<_> = alerts
        .iter()
        .map(|a| a.due_date.as_deref().unwrap())
        .collect();
    assert_eq!(due, ["2026-06-22", "2026-07-01", "2026-07-15"]);

    // All three end by themselves, so each due date is a real deadline the
    // screen may print.
    assert!(
        alerts.iter().all(|a| !a.standing),
        "a plazo, a licence and an ITV all lapse"
    );
    assert!(alerts.iter().all(|a| a.status == AlertStatus::Active));
    assert!(
        alerts.iter().all(|a| !a.overdue),
        "every date is still ahead on TODAY"
    );

    // Each alert names its subject: "which plot", "which operator", "which
    // machine" is the question a farmer has, and (subject_table, subject_id)
    // could only answer the kind.
    assert_eq!(phi.subject_label.as_deref(), Some("Parcela 1"));
    assert_eq!(licence.subject_label.as_deref(), Some("Carlos Pérez"));
    assert_eq!(itv.subject_label.as_deref(), Some("Atomizador"));
}

#[test]
fn every_kind_raised_is_one_this_module_declares() {
    // The shell looks a card's code up among the declared kinds, and the
    // contract test words only those — a kind raised but not declared would
    // list with no text, and could not be acted on.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let alerts = raised(&conn, TODAY, &AlertConfig::defaults());
    assert_eq!(
        alerts.len(),
        3,
        "every kind is raised, so every kind is checked"
    );
    for alert in &alerts {
        assert!(
            ALERT_KINDS.contains(&alert.kind()),
            "{} is raised but not in ALERT_KINDS",
            alert.kind().code()
        );
    }
}

#[test]
fn conditions_outside_their_window_raise_nothing() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);

    // On 2026-01-15 nothing is live yet: the treatment hasn't happened and both
    // expiry dates are far beyond their lead windows.
    assert!(listed(&conn, "2026-01-15").is_empty());
}

/// The lead times are a user setting (2026-08-26), and most tests here pass the
/// defaults — so nothing would prove the rules actually READ the config rather
/// than reaching for module-phytosanitary's own numbers.
///
/// Fixture dates against `TODAY` (2026-06-11): the licence expires 2026-07-15
/// (34 days out) and the ITV falls due 2026-07-01 (20 days out). So a 10-day
/// lead is too short to reach either, and the defaults are long enough for both.
#[test]
fn a_narrower_lead_time_closes_the_window_the_defaults_leave_open() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);

    let short = AlertConfig {
        licence_lead_days: 10,
        itv_lead_days: 10,
    };
    assert!(
        listed_with(&conn, TODAY, &short)
            .iter()
            .all(|a| a.alert_type_code == "phi_window"),
        "a 10-day lead cannot reach either expiry, so only the PHI alert should stand"
    );

    // The same day with the defaults raises both — so the difference is the
    // config and nothing else.
    assert_eq!(listed(&conn, TODAY).len(), 3);
}

#[test]
fn the_lead_time_decides_the_first_day_an_expiry_alerts() {
    // The window opens exactly `lead_days` before the date: a lead of 34 days
    // reaches the licence expiring 34 days after TODAY, a lead of 33 does not.
    // The same at the ITV's 20 days. An off-by-one here is a farmer warned a
    // day late, or not at all.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let at = |licence, itv| AlertConfig {
        licence_lead_days: licence,
        itv_lead_days: itv,
    };

    let reached = listed_with(&conn, TODAY, &at(34, 20));
    assert!(has(&reached, "licence_expiry"));
    assert!(has(&reached, "itv_expiry"));

    let short_by_one = listed_with(&conn, TODAY, &at(33, 19));
    assert!(!has(&short_by_one, "licence_expiry"));
    assert!(!has(&short_by_one, "itv_expiry"));
}

#[test]
fn each_lead_time_governs_only_its_own_alert() {
    // Two knobs, not one: shortening the licence lead must not disturb the ITV.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);

    let alerts = listed_with(
        &conn,
        TODAY,
        &AlertConfig {
            licence_lead_days: 10,
            ..AlertConfig::defaults()
        },
    );
    assert!(
        !has(&alerts, "licence_expiry"),
        "the shortened licence lead should have closed its window"
    );
    assert!(
        has(&alerts, "itv_expiry"),
        "the ITV lead was left at its default and must still alert"
    );
}

#[test]
fn operator_without_expiry_date_raises_nothing() {
    let mut conn = open_in_memory().unwrap();
    repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Sin Carné".into(),
            tax_id: None,
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();

    assert!(listed(&conn, TODAY).is_empty());
}

#[test]
fn a_multi_plot_treatment_is_one_phi_alert_naming_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let second = treatment_over(&mut conn, &fx, &["A", "B"]);

    let alerts = listed(&conn, TODAY);
    let phi: Vec<&Alert> = alerts
        .iter()
        .filter(|a| a.alert_type_code == "phi_window")
        .collect();
    assert_eq!(
        phi.len(),
        2,
        "one alert per treatment record, regardless of plot count"
    );
    // The one alert names every plot the treatment covered — a plazo is about
    // which ground cannot be harvested yet.
    let two_plots = phi.iter().find(|a| a.subject_id == second).unwrap();
    assert_eq!(two_plots.subject_label.as_deref(), Some("A, B"));
}

#[test]
fn a_treatment_over_more_plots_than_fit_names_the_first_of_them() {
    // Forty parcels in one pass is a cooperative's ordinary day and forty names
    // are not a label, so the line stops after three and trails off.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let record_id = treatment_over(&mut conn, &fx, &["A", "B", "C", "D", "E"]);

    let alerts = listed(&conn, TODAY);
    let alert = alerts
        .iter()
        .find(|a| a.subject_id == record_id)
        .expect("the five-plot treatment has a PHI alert");
    assert_eq!(alert.subject_label.as_deref(), Some("A, B, C, …"));
}

// --- nothing is stored ------------------------------------------------------------

#[test]
fn working_out_the_alerts_writes_nothing() {
    // The design's whole point: a read that changed the database would be a
    // copy to keep current again.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let before = log_rows(&conn);
    let changes_before = conn.total_changes();

    assert_eq!(raised(&conn, TODAY, &AlertConfig::defaults()).len(), 3);

    assert_eq!(log_rows(&conn), before, "nothing logged");
    assert_eq!(
        conn.total_changes(),
        changes_before,
        "nothing written at all"
    );
    let alert_tables: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name IN ('alert', 'alert_type')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(alert_tables, 0, "and there is nowhere to write them");
}

#[test]
fn the_same_registers_raise_the_same_alerts() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let config = AlertConfig::defaults();
    assert_eq!(report(&conn, TODAY, &config), report(&conn, TODAY, &config));
}

#[test]
fn a_lapsed_phi_window_raises_nothing() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    assert_eq!(listed(&conn, TODAY).len(), 3);

    // On the PHI end date (2026-06-22) harvest is allowed again: the alert lapses.
    let alerts = listed(&conn, "2026-06-22");
    assert!(!has(&alerts, "phi_window"));
    assert_eq!(alerts.len(), 2, "the expiry alerts are still live");
}

#[test]
fn an_expired_licence_keeps_alerting_until_it_is_renewed() {
    // An expired carné is the most urgent state, not a resolved one — and it
    // is listed as overdue, so the card words it as expired.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let alerts = listed(&conn, "2026-09-01");
    let licence = alert_for(&alerts, "licence_expiry");
    assert!(licence.overdue);
    assert_eq!(licence.due_date.as_deref(), Some("2026-07-15"));
}

#[test]
fn an_itv_is_overdue_from_the_day_after_it_falls_due() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    // Due 2026-07-01: on the day it can still be passed.
    assert!(!alert_for(&listed(&conn, "2026-07-01"), "itv_expiry").overdue);
    assert!(alert_for(&listed(&conn, "2026-07-02"), "itv_expiry").overdue);
}

#[test]
fn a_phi_window_is_never_listed_as_overdue() {
    // It stops being raised on its end date, so there is no day on which it
    // could be past it.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    for day in ["2026-06-01", "2026-06-21"] {
        assert!(!alert_for(&listed(&conn, day), "phi_window").overdue);
    }
    assert!(!has(&listed(&conn, "2026-06-22"), "phi_window"));
}

#[test]
fn a_renewed_licence_raises_nothing() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Renewal: the expiry date moves out beyond the lead window. Direct SQL —
    // the rule must react to the data, not to which API wrote it.
    conn.execute(
        "UPDATE operator SET licence_expiry_date = '2031-07-15' WHERE id = ?1",
        [&fx.operator_id],
    )
    .unwrap();

    assert!(!has(&listed(&conn, TODAY), "licence_expiry"));
}

#[test]
fn deleted_subjects_raise_nothing() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    for table in ["machinery", "operator"] {
        conn.execute(
            &format!("UPDATE {table} SET deleted_at = '2026-06-11T08:00:00Z' WHERE id = ?1"),
            [if table == "machinery" {
                &fx.machinery_id
            } else {
                &fx.operator_id
            }],
        )
        .unwrap();
    }
    repo::soft_delete_treatment_record(&mut conn, &fx.treatment_id, None).unwrap();

    assert!(
        listed(&conn, TODAY).is_empty(),
        "a withdrawn treatment, a removed operator and a retired machine raise nothing"
    );
}

/// Damage one stored date behind the app's back — the path a bad value really
/// takes now that every form refuses one: a row synced or restored from a copy
/// that let it through.
fn damage(conn: &Connection, sql: &str, id: &str) {
    assert_eq!(conn.execute(sql, [id]).unwrap(), 1);
}

#[test]
fn an_unreadable_licence_date_is_reported_by_name_and_the_rest_still_raise() {
    // Compliance logic must not silently drop a record — and one bad row must
    // not hide every other alert either, which is what failing the call did.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    damage(
        &conn,
        "UPDATE operator SET licence_expiry_date = '15/07/2026' WHERE id = ?1",
        &fx.operator_id,
    );

    let report = report(&conn, TODAY, &AlertConfig::defaults());
    assert_eq!(report.unchecked.len(), 1);
    let unchecked = &report.unchecked[0];
    assert_eq!(unchecked.alert_type_code, "licence_expiry");
    assert_eq!(unchecked.subject_table, "operator");
    assert_eq!(unchecked.subject_id, fx.operator_id);
    assert_eq!(unchecked.subject_label.as_deref(), Some("Carlos Pérez"));
    assert_eq!(unchecked.value, "15/07/2026", "the value as stored");

    let still: Vec<&str> = report.raised.iter().map(|a| a.kind().code()).collect();
    assert_eq!(
        still,
        ["phi_window", "itv_expiry"],
        "the PHI window and the ITV are still worked out"
    );
}

#[test]
fn an_unreadable_itv_or_treatment_date_is_reported_for_its_own_record() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    damage(
        &conn,
        "UPDATE machinery SET next_inspection_due_date = '1 de julio' WHERE id = ?1",
        &fx.machinery_id,
    );
    // The application date: the PHI end stays well formed, so the row is still
    // a candidate, and it is the window rule that cannot read it.
    damage(
        &conn,
        "UPDATE treatment_record SET application_date = '2026-6-1' WHERE id = ?1",
        &fx.treatment_id,
    );

    let report = report(&conn, TODAY, &AlertConfig::defaults());
    let mut unchecked: Vec<(&str, &str, Option<&str>, &str)> = report
        .unchecked
        .iter()
        .map(|u| {
            (
                u.alert_type_code,
                u.subject_id.as_str(),
                u.subject_label.as_deref(),
                u.value.as_str(),
            )
        })
        .collect();
    unchecked.sort();
    let mut expected = vec![
        (
            "itv_expiry",
            fx.machinery_id.as_str(),
            Some("Atomizador"),
            "1 de julio",
        ),
        (
            "phi_window",
            fx.treatment_id.as_str(),
            Some("Parcela 1"),
            "2026-6-1",
        ),
    ];
    expected.sort();
    assert_eq!(unchecked, expected);
    let still: Vec<&str> = report.raised.iter().map(|a| a.kind().code()).collect();
    assert_eq!(still, ["licence_expiry"], "the licence is still checked");
}

#[test]
fn a_failure_that_is_not_one_records_still_fails_the_call() {
    // A database that cannot answer is not a record to correct: the call
    // fails, and the shell names this module's alerts as unavailable.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    conn.execute_batch("ALTER TABLE operator RENAME COLUMN licence_expiry_date TO gone")
        .unwrap();
    assert!(matches!(
        repo::current_alerts(&conn, TODAY, &AlertConfig::defaults()),
        Err(PhytosanitaryError::Sqlite(_))
    ));
}

// --- acts on this module's alerts --------------------------------------------------

#[test]
fn a_dismissed_alert_stays_hidden_while_its_deadline_holds() {
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let alerts = listed(&conn, TODAY);
    dismiss(&mut conn, alert_for(&alerts, "licence_expiry"));

    for day in [TODAY, "2026-07-01", "2026-09-01"] {
        assert!(
            !has(&listed(&conn, day), "licence_expiry"),
            "the dismissed licence came back on {day}"
        );
    }
    assert_eq!(listed(&conn, TODAY).len(), 2, "the others are untouched");
}

#[test]
fn a_dismissal_holds_when_the_window_closes_and_reopens() {
    // A shorter lead time closes the window; widening it again brings back the
    // same deadline, which was already dismissed.
    let mut conn = open_in_memory().unwrap();
    fixture(&mut conn);
    let alerts = listed(&conn, TODAY);
    dismiss(&mut conn, alert_for(&alerts, "licence_expiry"));

    let short = AlertConfig {
        licence_lead_days: 10,
        ..AlertConfig::defaults()
    };
    assert!(!has(&listed_with(&conn, TODAY, &short), "licence_expiry"));
    assert!(
        !has(&listed(&conn, TODAY), "licence_expiry"),
        "the same deadline came back, and it was already dismissed"
    );
}

#[test]
fn a_renewed_licence_alerts_again_when_its_next_expiry_nears() {
    // The condition's identity (licence_expiry, operator, X) comes back at
    // every renewal cycle, and a dismissal of the 2026 expiry must not silence
    // the 2031 one.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let alerts = listed(&conn, TODAY);
    dismiss(&mut conn, alert_for(&alerts, "licence_expiry"));

    conn.execute(
        "UPDATE operator SET licence_expiry_date = '2031-07-15' WHERE id = ?1",
        [&fx.operator_id],
    )
    .unwrap();
    // Five years on, inside the renewed licence's 60-day lead.
    let later = listed(&conn, "2031-06-01");
    let licence = alert_for(&later, "licence_expiry");
    assert_eq!(licence.due_date.as_deref(), Some("2031-07-15"));
    assert_eq!(
        licence.status,
        AlertStatus::Active,
        "a new deadline, unseen"
    );
}

#[test]
fn a_corrected_itv_date_is_a_deadline_nobody_has_seen() {
    // An act names the deadline it saw, so a corrected date comes back unseen —
    // which also means a plazo moved two weeks later cannot stay hidden
    // (docs/sync.md → Alert acknowledgements roam).
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let alerts = listed(&conn, TODAY);
    acknowledge(&mut conn, alert_for(&alerts, "itv_expiry"), None);
    assert_eq!(
        alert_for(&listed(&conn, TODAY), "itv_expiry").status,
        AlertStatus::Acknowledged
    );

    // Corrected, and still inside the 30-day lead.
    conn.execute(
        "UPDATE machinery SET next_inspection_due_date = '2026-07-05' WHERE id = ?1",
        [&fx.machinery_id],
    )
    .unwrap();

    let refreshed = listed(&conn, TODAY);
    let itv = alert_for(&refreshed, "itv_expiry");
    assert_eq!(itv.due_date.as_deref(), Some("2026-07-05"));
    assert_eq!(itv.status, AlertStatus::Active);
    assert_eq!(itv.acknowledged_at, None);
}

#[test]
fn an_act_on_a_phi_alert_is_filed_under_its_treatment_and_deadline() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let alerts = listed(&conn, TODAY);
    acknowledge(
        &mut conn,
        alert_for(&alerts, "phi_window"),
        Some("profile-1"),
    );

    let act_id: String = conn
        .query_row("SELECT id FROM alert_acknowledgement", [], |r| r.get(0))
        .unwrap();
    let (operation, _before, after) =
        terrazgo_testkit::last_change(&conn, "alert_acknowledgement", &act_id);
    assert_eq!(operation, "insert");
    // The full row image, which is what a receiving device writes the row from.
    assert_eq!(
        after,
        serde_json::json!({
            "id": act_id,
            "alert_type_code": "phi_window",
            "subject_table": "treatment_record",
            "subject_id": fx.treatment_id,
            "due_date": "2026-06-22",
            "status": AlertStatus::Acknowledged.code(),
            "created_at": after["created_at"],
        })
    );
    let actor: Option<String> = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1",
            [&act_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor.as_deref(), Some("profile-1"), "and says who acted");
}

// ---------------------------------------------------------------------------
// Non-chemical actuations and the plazo de seguridad
//
// RD 1311/2012 art. 10.1 asks professionals to prefer non-chemical methods, so
// the register has to be able to hold an actuation with no product — and a
// measure imposes no waiting period before harvest. These two tests pin the
// rule in BOTH directions on purpose: the failure that would matter is not a
// spurious alert but a MISSING one, and the earlier shape of the candidate
// query (`phi_end_date` read as a non-null String) would have failed the whole
// call, leaving this module's alerts unavailable altogether.
// ---------------------------------------------------------------------------

/// Insert a purely non-chemical actuation on the fixture's farm/season:
/// pheromone diffusers, no product, no dose, no plazo.
fn insert_non_chemical(conn: &mut Connection, fx: &Fixture) -> String {
    let (season_id, farm_id): (String, String) = conn
        .query_row(
            "SELECT season_id, farm_id FROM treatment_record WHERE id = ?1",
            [&fx.treatment_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let plot_id: String = conn
        .query_row(
            "SELECT plot_id FROM treatment_plot WHERE treatment_record_id = ?1",
            [&fx.treatment_id],
            |r| r.get(0),
        )
        .unwrap();
    repo::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id,
            farm_id,
            // Same day as the fixture's chemical treatment, so any PHI window
            // this wrongly opened would be live on TODAY and the test would
            // see it.
            application_date: "2026-06-01".into(),
            application_end_date: None,
            drying_date: None,
            application_time: None,
            product_id: None,
            country_code: None,
            dose_value: None,
            dose_unit_code: None,
            total_quantity_value: None,
            total_quantity_unit_code: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: None,
            target_organism: None,
            operator_id: fx.operator_id.clone(),
            machinery_id: None,
            advisor_id: None,
            measure_code: Some("15".into()), // feromonas y atrayentes
            measure_intensity_value: Some(4.0),
            measure_intensity_unit_code: Some("diffusers_ha".into()),
            measure_registration_number: None,
            measure_basic_substance_code: None,
            phi_days_used: None,
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap()
    .id
}

#[test]
fn a_non_chemical_actuation_stores_no_plazo_de_seguridad() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let id = insert_non_chemical(&mut conn, &fx);

    let (phi_days, phi_end, dose, product): (
        Option<i64>,
        Option<String>,
        Option<f64>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT phi_days_used, phi_end_date, dose_value, product_id
             FROM treatment_record WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    // The whole chemical block is absent together — the table CHECK refuses
    // any partial combination, so this is the only shape it can take.
    assert_eq!(phi_days, None);
    assert_eq!(phi_end, None);
    assert_eq!(dose, None);
    assert_eq!(product, None);
}

#[test]
fn a_non_chemical_actuation_raises_no_phi_alert_and_leaves_the_others_alone() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    // Baseline: the chemical fixture's PHI window is live on TODAY.
    let before = listed(&conn, TODAY);
    let phi_before: Vec<&str> = before
        .iter()
        .filter(|a| a.alert_type_code == "phi_window")
        .map(|a| a.subject_id.as_str())
        .collect();
    assert_eq!(
        phi_before,
        vec![fx.treatment_id.as_str()],
        "the chemical treatment's window is the baseline"
    );

    let non_chemical = insert_non_chemical(&mut conn, &fx);
    let after = listed(&conn, TODAY);

    // The measure raises nothing of its own...
    assert!(
        !after.iter().any(|a| a.subject_id == non_chemical),
        "a measure imposes no plazo de seguridad, so it opens no window"
    );
    // ...and, the point of the test, it does not take the existing alerts with
    // it: the licence and ITV alerts are worked out in the same call.
    assert_eq!(
        after.len(),
        before.len(),
        "an actuation with no product must not disturb the rest of the alerts"
    );
    assert!(
        after
            .iter()
            .any(|a| a.alert_type_code == "phi_window" && a.subject_id == fx.treatment_id),
        "the chemical treatment's own window is still open"
    );
}

#[test]
fn phi_status_ignores_actuations_with_no_product() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let (farm_id, plot_id): (String, String) = conn
        .query_row(
            "SELECT tr.farm_id, tp.plot_id FROM treatment_record tr
             JOIN treatment_plot tp ON tp.treatment_record_id = tr.id
             WHERE tr.id = ?1",
            [&fx.treatment_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    insert_non_chemical(&mut conn, &fx);

    // The map overlay reads the same windows. The plot is in PHI because of
    // the CHEMICAL treatment; the measure neither adds a window nor breaks the
    // query that finds them.
    let status =
        repo::phi_status_for_farm(&conn, &farm_id, TODAY, repo::default_phi_horizon_days())
            .unwrap();
    let plot = status.iter().find(|s| s.plot_id == plot_id).unwrap();
    assert!(plot.in_phi);
    assert_eq!(plot.phi_until.as_deref(), Some("2026-06-22"));
}
