// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alerts in core: the zone alerts core raises, the list assembled from any
//! crate's alerts, and the acts people record on them — including between two
//! devices (docs/data-model.md → "Alerts: the settled design"; docs/sync.md →
//! Alert acknowledgements roam).
//!
//! Nothing stores an alert, so every test here reads the list the way the app
//! does: raise what holds now, then assemble. A kind core has never heard of —
//! the PHI window below — goes through the same list, which is the promise that
//! lets a module add an alert without touching core.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::alerts::{
    Alert, AlertStatus, DatedKind, NATURA_ZONE, NITRATE_ZONE, PHYTO_ZONE, RaisedAlert,
};
use terrazgo_core::models::NewZoneFlag;
use terrazgo_core::repository as repo;

/// The day every list here is read on. Zone alerts do not depend on it; the
/// PHI windows below are all due after it, so none is overdue unless a test
/// says so.
const TODAY: &str = "2026-06-11";

/// A kind declared the way a module declares one, and known to core only
/// through the alerts that carry it.
const PHI_WINDOW: DatedKind = DatedKind::new("phi_window", "treatment_record");

fn phi(subject: &str, due: &str) -> RaisedAlert {
    RaisedAlert::dated(PHI_WINDOW, subject.into(), due.into())
}

fn flag(zone: &str, status: &str) -> NewZoneFlag {
    NewZoneFlag {
        zone_type_code: zone.into(),
        status: status.into(),
        coverage_pct: (status == "inside").then_some(40.0),
        detail: None,
    }
}

fn check(conn: &mut Connection, plot_id: &str, campaign: i64, flags: Vec<NewZoneFlag>) {
    repo::replace_zone_flags(conn, plot_id, campaign, "sigpac", flags, None).unwrap();
}

/// A farm with two plots, the first named "Los Alcores".
fn land(conn: &mut Connection) -> (String, String, String) {
    let farm = repo::insert_farm(conn, new_farm("Finca La Vega"), None).unwrap();
    let alcores = repo::insert_plot(conn, new_plot(&farm.id, "Los Alcores"), None).unwrap();
    let soto = repo::insert_plot(conn, new_plot(&farm.id, "El Soto"), None).unwrap();
    (farm.id, alcores.id, soto.id)
}

/// The list as the app assembles it from core's alerts and any `others`.
fn listed(conn: &Connection, others: Vec<RaisedAlert>) -> Vec<Alert> {
    let mut raised = repo::current_alerts(conn).unwrap().raised;
    raised.extend(others);
    repo::list_alerts(conn, raised, TODAY).unwrap()
}

/// `(kind, subject)` for each listed alert, in listed order.
fn conditions(alerts: &[Alert]) -> Vec<(&str, &str)> {
    alerts
        .iter()
        .map(|a| (a.alert_type_code, a.subject_id.as_str()))
        .collect()
}

fn acts(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM alert_acknowledgement", [], |r| {
        r.get(0)
    })
    .unwrap()
}

fn log_rows(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap()
}

fn acknowledge(conn: &mut Connection, alert: &Alert) -> Result<(), CoreError> {
    repo::acknowledge_alert(
        conn,
        kind_of(alert),
        &alert.subject_id,
        alert.due_date.as_deref(),
        None,
    )
}

fn dismiss(conn: &mut Connection, alert: &Alert) -> Result<(), CoreError> {
    repo::dismiss_alert(
        conn,
        kind_of(alert),
        &alert.subject_id,
        alert.due_date.as_deref(),
        None,
    )
}

/// What the shell does with the code a card sends back: find the kind among
/// the ones the alert crates declare.
fn kind_of(alert: &Alert) -> terrazgo_core::alerts::AlertKind {
    [
        NITRATE_ZONE.kind(),
        PHYTO_ZONE.kind(),
        NATURA_ZONE.kind(),
        PHI_WINDOW.kind(),
    ]
    .into_iter()
    .find(|kind| kind.code() == alert.alert_type_code)
    .expect("a kind this test declares")
}

// --- the zone alerts core raises ---------------------------------------------------

#[test]
fn each_inside_flag_raises_a_standing_alert_named_after_its_plot() {
    let mut conn = db();
    let (_, alcores, soto) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![
            flag("nitrate_vulnerable", "inside"),
            flag("phytosanitary_restriction", "inside"),
            flag("natura_2000", "outside"),
        ],
    );
    check(&mut conn, &soto, 2026, vec![flag("natura_2000", "inside")]);

    let alerts = listed(&conn, vec![]);
    let mut seen = conditions(&alerts);
    seen.sort();
    let mut expected = vec![
        ("nitrate_zone", alcores.as_str()),
        ("phyto_zone", alcores.as_str()),
        ("natura_zone", soto.as_str()),
    ];
    expected.sort();
    assert_eq!(seen, expected, "an outside answer raises nothing");

    for alert in &alerts {
        assert!(alert.standing);
        assert_eq!(
            alert.due_date, None,
            "a standing alert announces no deadline"
        );
        assert_eq!(alert.subject_table, "plot");
        assert_eq!(alert.status, AlertStatus::Active);
        assert_eq!(alert.acknowledged_at, None);
    }
    let alcores_label = alerts
        .iter()
        .find(|a| a.subject_id == alcores)
        .and_then(|a| a.subject_label.as_deref());
    assert_eq!(alcores_label, Some("Los Alcores"));
}

#[test]
fn only_the_latest_campaign_counts() {
    let mut conn = db();
    let (_, alcores, soto) = land(&mut conn);
    // Los Alcores left the zone; El Soto entered it.
    check(
        &mut conn,
        &alcores,
        2025,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "outside")],
    );
    check(
        &mut conn,
        &soto,
        2025,
        vec![flag("nitrate_vulnerable", "outside")],
    );
    check(
        &mut conn,
        &soto,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );

    assert_eq!(
        conditions(&listed(&conn, vec![])),
        vec![("nitrate_zone", soto.as_str())]
    );
}

#[test]
fn a_deleted_plot_takes_its_zone_alerts_with_it_at_once() {
    // With the alerts stored, deleting a plot refreshed nothing, and its zone
    // alert stayed listed until some other save ran. Worked out when read,
    // there is nothing to go stale.
    let mut conn = db();
    let (_, alcores, soto) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    check(
        &mut conn,
        &soto,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    assert_eq!(listed(&conn, vec![]).len(), 2);

    repo::soft_delete_plot(&mut conn, &alcores, None).unwrap();

    assert_eq!(
        conditions(&listed(&conn, vec![])),
        vec![("nitrate_zone", soto.as_str())]
    );
}

#[test]
fn a_zone_kind_with_no_alert_mapped_raises_nothing_and_breaks_nothing() {
    // A future country's zone codes raise nothing until a mapping is added.
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    conn.execute(
        "INSERT INTO zone_type (code, i18n_key) VALUES ('zone_humide', 'zone_type.zone_humide')",
        [],
    )
    .unwrap();
    check(
        &mut conn,
        &alcores,
        2026,
        vec![
            flag("zone_humide", "inside"),
            flag("nitrate_vulnerable", "inside"),
        ],
    );

    assert_eq!(
        conditions(&listed(&conn, vec![])),
        vec![("nitrate_zone", alcores.as_str())]
    );
}

#[test]
fn zone_alerts_span_every_farm() {
    // The Status view is database-wide, like the review queue.
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    let other = repo::insert_farm(&mut conn, new_farm("Los Llanos"), None).unwrap();
    let llanos = repo::insert_plot(&mut conn, new_plot(&other.id, "La Vega"), None).unwrap();
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    check(
        &mut conn,
        &llanos.id,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );

    assert_eq!(listed(&conn, vec![]).len(), 2);
}

#[test]
fn nothing_raised_lists_nothing() {
    let mut conn = db();
    land(&mut conn);
    assert_eq!(repo::current_alerts(&conn).unwrap(), Default::default());
    assert!(repo::list_alerts(&conn, vec![], TODAY).unwrap().is_empty());
}

// --- the list, from any crate's alerts ---------------------------------------------

#[test]
fn a_kind_core_never_heard_of_is_listed_and_acted_on_like_its_own() {
    // The standard shape is the whole contract: no table of kinds, no
    // registration in core.
    let mut conn = db();
    let alerts = listed(&conn, vec![phi("tr-1", "2026-07-01")]);
    assert_eq!(conditions(&alerts), vec![("phi_window", "tr-1")]);
    assert_eq!(alerts[0].due_date.as_deref(), Some("2026-07-01"));
    assert!(!alerts[0].standing);

    acknowledge(&mut conn, &alerts[0]).unwrap();
    let alerts = listed(&conn, vec![phi("tr-1", "2026-07-01")]);
    assert_eq!(alerts[0].status, AlertStatus::Acknowledged);
}

#[test]
fn a_crate_s_subject_label_reaches_the_list() {
    let conn = db();
    let mut named = phi("tr-1", "2026-07-01");
    named.set_subject_label(Some("Los Alcores, El Soto".into()));
    let alerts = listed(&conn, vec![named, phi("tr-2", "2026-07-02")]);
    assert_eq!(
        alerts[0].subject_label.as_deref(),
        Some("Los Alcores, El Soto")
    );
    assert_eq!(alerts[1].subject_label, None, "unnamed stays unnamed");
}

#[test]
fn an_acknowledged_alert_stays_listed_and_says_when_it_was_seen() {
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    let alert = listed(&conn, vec![]).remove(0);

    acknowledge(&mut conn, &alert).unwrap();

    let alerts = listed(&conn, vec![]);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].status, AlertStatus::Acknowledged);
    assert!(alerts[0].acknowledged_at.is_some());
}

#[test]
fn a_dismissed_alert_leaves_the_list_and_the_others_stay() {
    let mut conn = db();
    let (_, alcores, soto) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    check(
        &mut conn,
        &soto,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    let alerts = listed(&conn, vec![phi("tr-1", "2026-07-01")]);
    let alcores_alert = alerts.iter().find(|a| a.subject_id == alcores).unwrap();

    dismiss(&mut conn, alcores_alert).unwrap();

    let still = listed(&conn, vec![phi("tr-1", "2026-07-01")]);
    let mut left = conditions(&still);
    left.sort();
    let mut expected = vec![("nitrate_zone", soto.as_str()), ("phi_window", "tr-1")];
    expected.sort();
    assert_eq!(left, expected);
}

#[test]
fn an_act_on_one_kind_of_a_plot_leaves_its_other_kinds_alone() {
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![
            flag("nitrate_vulnerable", "inside"),
            flag("phytosanitary_restriction", "inside"),
        ],
    );
    let alerts = listed(&conn, vec![]);
    let nitrate = alerts
        .iter()
        .find(|a| a.alert_type_code == "nitrate_zone")
        .unwrap();

    dismiss(&mut conn, nitrate).unwrap();

    assert_eq!(
        conditions(&listed(&conn, vec![])),
        vec![("phyto_zone", alcores.as_str())]
    );
}

#[test]
fn a_dismissed_deadline_does_not_hide_the_next_one() {
    // A plazo dismissed, then corrected to end later — or a licence dismissed
    // and renewed. Either way the deadline is different, so the alert is new.
    let mut conn = db();
    let first = listed(&conn, vec![phi("tr-1", "2026-07-01")]).remove(0);
    dismiss(&mut conn, &first).unwrap();
    assert!(listed(&conn, vec![phi("tr-1", "2026-07-01")]).is_empty());

    let moved = listed(&conn, vec![phi("tr-1", "2026-07-15")]);
    assert_eq!(
        moved.len(),
        1,
        "a deadline moved later must not stay hidden"
    );
    assert_eq!(moved[0].status, AlertStatus::Active);
    assert_eq!(moved[0].acknowledged_at, None);
}

#[test]
fn a_dismissed_zone_alert_stays_hidden_when_the_plot_leaves_and_reenters() {
    // A standing act names no date, so it holds for as long as the condition
    // does — and across a campaign where it did not hold.
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2025,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    let alert = listed(&conn, vec![]).remove(0);
    dismiss(&mut conn, &alert).unwrap();

    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "outside")],
    );
    assert!(listed(&conn, vec![]).is_empty());
    check(
        &mut conn,
        &alcores,
        2027,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    assert!(
        listed(&conn, vec![]).is_empty(),
        "re-entering the zone brought a dismissed alert back"
    );
}

#[test]
fn a_condition_raised_twice_is_listed_once() {
    let conn = db();
    let alerts = listed(
        &conn,
        vec![phi("tr-1", "2026-07-09"), phi("tr-1", "2026-07-01")],
    );
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].due_date.as_deref(), Some("2026-07-01"));
}

#[test]
fn deadlines_come_first_soonest_at_the_top_then_the_standing_conditions() {
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    let alerts = listed(
        &conn,
        vec![phi("tr-late", "2026-09-01"), phi("tr-soon", "2026-07-01")],
    );
    assert_eq!(
        conditions(&alerts),
        vec![
            ("phi_window", "tr-soon"),
            ("phi_window", "tr-late"),
            ("nitrate_zone", alcores.as_str()),
        ]
    );
}

#[test]
fn an_alert_whose_date_has_passed_is_listed_as_overdue() {
    // An expired carné is the most urgent state there is, and the card must
    // not word it as upcoming. The date itself is still inside.
    const EXPIRY: DatedKind = DatedKind::new("licence_expiry", "operator");
    let mut conn = db();
    let (_, alcores, _) = land(&mut conn);
    check(
        &mut conn,
        &alcores,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    let expiring =
        |subject: &str, due: &str| RaisedAlert::dated(EXPIRY, subject.into(), due.into());

    let mut raised = repo::current_alerts(&conn).unwrap().raised;
    raised.extend([
        expiring("op-past", "2026-06-10"),
        expiring("op-today", TODAY),
        expiring("op-later", "2026-07-15"),
    ]);
    let alerts = repo::list_alerts(&conn, raised, TODAY).unwrap();
    let overdue: Vec<(&str, bool)> = alerts
        .iter()
        .map(|a| (a.subject_id.as_str(), a.overdue))
        .collect();
    assert_eq!(
        overdue,
        vec![
            ("op-past", true),
            ("op-today", false),
            ("op-later", false),
            (alcores.as_str(), false),
        ],
        "only the passed date is overdue, and it lists first, being the most urgent"
    );
}

// --- the acts ------------------------------------------------------------------------

#[test]
fn an_act_that_changes_nothing_is_not_written() {
    let mut conn = db();
    let alert = listed(&conn, vec![phi("tr-1", "2026-07-01")]).remove(0);

    acknowledge(&mut conn, &alert).unwrap();
    let logged = log_rows(&conn);
    acknowledge(&mut conn, &alert).unwrap();
    assert_eq!(acts(&conn), 1, "a second acknowledgement is a double tap");
    assert_eq!(log_rows(&conn), logged, "and it logs nothing either");

    dismiss(&mut conn, &alert).unwrap();
    assert_eq!(
        acts(&conn),
        2,
        "dismissing what was only seen is a real act"
    );

    acknowledge(&mut conn, &alert).unwrap();
    dismiss(&mut conn, &alert).unwrap();
    assert_eq!(acts(&conn), 2, "nothing is stronger than a dismissal");
}

#[test]
fn an_act_is_logged_as_a_register_of_its_own() {
    let mut conn = db();
    let alert = listed(&conn, vec![phi("tr-1", "2026-07-01")]).remove(0);
    repo::acknowledge_alert(
        &mut conn,
        PHI_WINDOW.kind(),
        "tr-1",
        Some("2026-07-01"),
        Some("profile-1"),
    )
    .unwrap();
    let id: String = conn
        .query_row("SELECT id FROM alert_acknowledgement", [], |r| r.get(0))
        .unwrap();

    let (operation, before, after) = last_change(&conn, "alert_acknowledgement", &id);
    assert_eq!(operation, "insert");
    assert!(before.is_null());
    assert_eq!(after["alert_type_code"], alert.alert_type_code);
    assert_eq!(after["subject_table"], "treatment_record");
    assert_eq!(after["subject_id"], "tr-1");
    assert_eq!(after["due_date"], "2026-07-01");
    assert_eq!(after["status"], "acknowledged");

    let stamped = terrazgo_testkit::last_stamp(&conn, "alert_acknowledgement", &id);
    assert_eq!(
        stamped.register,
        ("alert_acknowledgement".to_string(), id.clone()),
        "each act is its own register, so two acts can never conflict"
    );
    let (season, actor): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT season_id, actor FROM record_change WHERE entity_id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(season, None, "an act is about a condition, not a campaign");
    assert_eq!(actor.as_deref(), Some("profile-1"));
}

#[test]
fn a_standing_alert_sent_back_with_a_date_is_refused_and_writes_nothing() {
    let mut conn = db();
    let before = log_rows(&conn);
    let refused = repo::dismiss_alert(
        &mut conn,
        NITRATE_ZONE.kind(),
        "plot-1",
        Some("2026-12-31"),
        None,
    );
    assert!(matches!(
        refused,
        Err(CoreError::Invalid("alert_deadline_mismatch"))
    ));
    assert_eq!(acts(&conn), 0);
    assert_eq!(log_rows(&conn), before);
}

#[test]
fn a_dated_alert_sent_back_without_its_date_is_refused_and_writes_nothing() {
    let mut conn = db();
    let refused = repo::acknowledge_alert(&mut conn, PHI_WINDOW.kind(), "tr-1", None, None);
    assert!(matches!(
        refused,
        Err(CoreError::Invalid("alert_deadline_mismatch"))
    ));
    assert_eq!(acts(&conn), 0);
}

#[test]
fn a_malformed_date_sent_back_is_refused() {
    let mut conn = db();
    let refused = repo::acknowledge_alert(
        &mut conn,
        PHI_WINDOW.kind(),
        "tr-1",
        Some("01/07/2026"),
        None,
    );
    assert!(matches!(refused, Err(CoreError::InvalidDate(_))));
    assert_eq!(acts(&conn), 0);
}

#[test]
fn an_act_records_what_was_shown_even_if_it_changed_meanwhile() {
    // The card showed 1 July; by the tap the plazo had been corrected to 15
    // July. The act is filed under what the person saw, so the corrected
    // alert — which they have not seen — stays active.
    let mut conn = db();
    let shown = listed(&conn, vec![phi("tr-1", "2026-07-01")]).remove(0);
    dismiss(&mut conn, &shown).unwrap();
    assert_eq!(acts(&conn), 1, "recorded without re-checking");

    let now = listed(&conn, vec![phi("tr-1", "2026-07-15")]);
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].status, AlertStatus::Active);
}

#[test]
fn a_damaged_status_is_an_error_rather_than_nobody_having_acted() {
    let conn = db();
    conn.execute_batch(
        "PRAGMA ignore_check_constraints = ON;
         INSERT INTO alert_acknowledgement
           (id, alert_type_code, subject_table, subject_id, due_date, status, created_at)
         VALUES ('a1', 'phi_window', 'treatment_record', 'tr-1', '2026-07-01', 'snoozed',
                 '2026-06-11T08:00:00Z');
         PRAGMA ignore_check_constraints = OFF;",
    )
    .unwrap();
    assert!(repo::list_alerts(&conn, vec![phi("tr-1", "2026-07-01")], TODAY).is_err());
}

// --- two devices -----------------------------------------------------------------------

/// A plot inside a nitrate zone on `a`, carried to `b`.
fn zoned_plot_on_both(a: &mut Device, b: &mut Device) -> String {
    let farm = repo::insert_farm(&mut a.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut a.conn, new_plot(&farm.id, "Los Alcores"), None).unwrap();
    check(
        &mut a.conn,
        &plot.id,
        2026,
        vec![flag("nitrate_vulnerable", "inside")],
    );
    sync(a, b);
    plot.id
}

#[test]
fn an_alert_hidden_on_one_device_is_hidden_on_the_other() {
    let mut a = Device::new(A);
    let mut b = Device::new(B);
    let plot = zoned_plot_on_both(&mut a, &mut b);
    assert_eq!(
        conditions(&listed(&b.conn, vec![])),
        vec![("nitrate_zone", plot.as_str())],
        "each device works out the alert from the synced flag"
    );

    let alert = listed(&a.conn, vec![]).remove(0);
    dismiss(&mut a.conn, &alert).unwrap();
    sync(&a, &mut b);

    assert!(listed(&b.conn, vec![]).is_empty());
}

#[test]
fn two_devices_acting_on_one_alert_offline_agree_without_a_conflict() {
    let mut a = Device::new(A);
    let mut b = Device::new(B);
    zoned_plot_on_both(&mut a, &mut b);
    // B sees it, A hides it, neither knowing of the other.
    let on_b = listed(&b.conn, vec![]).remove(0);
    acknowledge(&mut b.conn, &on_b).unwrap();
    let on_a = listed(&a.conn, vec![]).remove(0);
    dismiss(&mut a.conn, &on_a).unwrap();

    sync_both(&mut a, &mut b);

    for device in [&a, &b] {
        assert!(
            listed(&device.conn, vec![]).is_empty(),
            "the dismissal is the stronger act on {}",
            device.id
        );
        assert_eq!(acts(&device.conn), 2, "both acts arrived on {}", device.id);
        let conflicts: i64 = device
            .conn
            .query_row("SELECT COUNT(*) FROM sync_conflict", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            conflicts, 0,
            "two acts are two registers, never two versions"
        );
    }
}
