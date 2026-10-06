// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What people say about an alert, carried between devices by real bundles
//! (docs/sync.md → Alert acknowledgements roam).
//!
//! The alerts themselves never travel, and are never stored: each device works
//! out its own from the registers it holds, whenever its list is read. What
//! travels is the act — seen, or hidden — and the properties pinned here are
//! the ones the design turns on:
//!
//!   * an act made on one device is the alert's state on every device the
//!     moment it arrives, because the state is read through the acts;
//!   * two devices acting on one alert without seeing each other reach the
//!     same answer — dismissed beats acknowledged — and **nobody is asked**:
//!     the acts are two registers, so there is no conflict to queue;
//!   * an act arrives as exactly the row that was written, which is the
//!     payload↔column contract the generic applier rests on.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_phytosanitary::alerts::{AlertConfig, LICENCE_EXPIRY};
use module_phytosanitary::models::NewOperator;
use module_phytosanitary::open_in_memory;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;
use terrazgo_core::alerts::{Alert, AlertStatus};
use terrazgo_core::models::UpdateOperator;
use terrazgo_testkit::sync::send;

/// Inside the operator's 60-day licence lead: the licence expires 2026-07-15.
const TODAY: &str = "2026-06-11";

/// Two devices holding one book: an operator whose licence is about to expire,
/// written on the first and carried to the second. Each is a replica with its
/// own device id, as `open_in_memory` mints one.
fn two_devices() -> (Connection, Connection) {
    let mut phone = open_in_memory().unwrap();
    let mut laptop = open_in_memory().unwrap();
    repo::insert_operator(
        &mut phone,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("CL-12345".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2026-07-15".into()),
        },
        None,
    )
    .unwrap();

    let group = terrazgo_core::sync::ensure_sync_group(&phone).unwrap();
    terrazgo_core::sync::join_sync_group(&laptop, &group).unwrap();
    send(&phone, &mut laptop);
    (phone, laptop)
}

/// This device's licence alert on `today`, if it is listed at all — worked out
/// from its own registers, the way its Status view would.
fn licence_alert_on(conn: &Connection, today: &str) -> Option<Alert> {
    let raised = repo::current_alerts(conn, today, &AlertConfig::defaults())
        .unwrap()
        .raised;
    terrazgo_core::repository::list_alerts(conn, raised, today)
        .unwrap()
        .into_iter()
        .find(|a| a.alert_type_code == "licence_expiry")
}

fn licence_alert(conn: &Connection) -> Option<Alert> {
    licence_alert_on(conn, TODAY)
}

/// Act on an alert as a card showing it would.
fn act(conn: &mut Connection, alert: &Alert, dismiss: bool) {
    let record = if dismiss {
        terrazgo_core::repository::dismiss_alert
    } else {
        terrazgo_core::repository::acknowledge_alert
    };
    record(
        conn,
        LICENCE_EXPIRY.kind(),
        &alert.subject_id,
        alert.due_date.as_deref(),
        None,
    )
    .unwrap();
}

fn conflicts(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM sync_conflict", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn each_device_works_out_the_same_alert_from_the_shared_book() {
    let (phone, laptop) = two_devices();
    let on_phone = licence_alert(&phone).expect("the phone raises the alert");
    let on_laptop = licence_alert(&laptop).expect("and so does the laptop");
    assert_eq!(
        on_phone, on_laptop,
        "the same registers, the same day: the same alert, with nothing sent"
    );
    assert_eq!(on_laptop.status, AlertStatus::Active);
}

#[test]
fn an_alert_seen_on_one_device_is_seen_on_the_other() {
    let (mut phone, mut laptop) = two_devices();
    let alert = licence_alert(&phone).unwrap();
    act(&mut phone, &alert, false);

    send(&phone, &mut laptop);
    // The state is read through the acts, so the one that arrived is the state
    // the moment it is in the tables.
    let on_laptop = licence_alert(&laptop).expect("seen, not hidden");
    assert_eq!(on_laptop.status, AlertStatus::Acknowledged);
    assert_eq!(
        on_laptop.acknowledged_at,
        licence_alert(&phone).unwrap().acknowledged_at,
        "seen when the phone saw it"
    );
}

#[test]
fn a_dismissal_on_one_device_beats_an_acknowledgement_on_another_and_nobody_is_asked() {
    let (mut phone, mut laptop) = two_devices();
    // Both act before either has heard from the other.
    let on_phone = licence_alert(&phone).unwrap();
    let on_laptop = licence_alert(&laptop).unwrap();
    act(&mut phone, &on_phone, false);
    act(&mut laptop, &on_laptop, true);

    send(&phone, &mut laptop);
    send(&laptop, &mut phone);

    assert!(licence_alert(&phone).is_none(), "hidden on the phone");
    assert!(licence_alert(&laptop).is_none(), "and on the laptop");
    assert_eq!(
        (conflicts(&phone), conflicts(&laptop)),
        (0, 0),
        "two acts are two registers, so there is nothing for the review queue"
    );
}

#[test]
fn an_act_arrives_as_exactly_the_row_that_was_written() {
    // The payload↔column contract, for this table: the receiving device
    // writes the row from the logged image alone, through the generic applier.
    let (mut phone, mut laptop) = two_devices();
    let alert = licence_alert(&phone).unwrap();
    act(&mut phone, &alert, true);
    send(&phone, &mut laptop);

    let rows = |conn: &Connection| -> Vec<Vec<Option<String>>> {
        conn.prepare(
            "SELECT id, alert_type_code, subject_table, subject_id, due_date, status, created_at
             FROM alert_acknowledgement ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| (0..7).map(|i| r.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    };
    let written = rows(&phone);
    assert_eq!(written.len(), 1);
    assert_eq!(rows(&laptop), written);
}

#[test]
fn a_renewed_licence_alerts_again_on_every_device() {
    // A dismissal names the deadline it saw. Renewing the licence on one device
    // gives the next expiry a new one, and when it nears, the alert is new on
    // both — the old dismissal, which travelled, does not silence it.
    let (mut phone, mut laptop) = two_devices();
    let alert = licence_alert(&phone).unwrap();
    act(&mut phone, &alert, true);
    send(&phone, &mut laptop);
    assert!(licence_alert(&laptop).is_none(), "the dismissal arrived");

    // Renewed on the phone, as an ordinary audited correction, so it travels.
    let operator_id: String = phone
        .query_row("SELECT id FROM operator", [], |r| r.get(0))
        .unwrap();
    terrazgo_core::repository::update_operator(
        &mut phone,
        &operator_id,
        UpdateOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("CL-12345".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2031-07-15".into()),
        },
        None,
    )
    .unwrap();
    send(&phone, &mut laptop);

    // Five years on, inside the renewed licence's lead.
    for device in [&phone, &laptop] {
        let alert = licence_alert_on(device, "2031-06-01").expect("the next expiry alerts");
        assert_eq!(alert.due_date.as_deref(), Some("2031-07-15"));
        assert_eq!(alert.status, AlertStatus::Active, "and nobody has seen it");
    }
}
