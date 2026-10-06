// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The crates that raise alerts, and the one place their alerts are gathered.
//!
//! Nothing stores an alert (docs/data-model.md → "Alerts: the settled design"):
//! each crate on [`ALERT_CRATES`] works out the ones holding now, and core
//! assembles the list from them. The list of crates is the shell's because only
//! the shell sees every crate — core may never name a module, and a module may
//! never name another.
//!
//! **A crate that raises alerts is added here, once**, with the kinds it
//! declares. `tests/contracts/alert_kinds_contract.rs` holds the list honest: no
//! code declared twice, and every kind's text in every dictionary — in both
//! directions, so a kind with text but no crate on this list fails too.

use rusqlite::Connection;
use serde::Serialize;
use terrazgo_core::alerts::{AlertKind, AlertReport};
use terrazgo_core::settings::AppSettings;

/// One crate that raises alerts.
pub struct AlertCrate {
    /// Stable name: the key of the crate's own text (`alert.source.<name>`),
    /// which the Status view shows when this crate's alerts could not be worked
    /// out.
    pub name: &'static str,
    /// Every kind the crate declares: what the contract test checks, and where
    /// the code a card sends back is looked up.
    pub kinds: &'static [AlertKind],
    /// The crate's report for `today` (`YYYY-MM-DD`), under this device's
    /// settings: its alerts holding, and the records it could not check.
    ///
    /// A plain function rather than a trait: each entry's function adapts its
    /// crate to one signature, and a list of `fn` values is data the compiler
    /// checks like any other constant.
    pub current: fn(&Connection, &str, &AppSettings) -> anyhow::Result<AlertReport>,
}

/// Every crate that raises alerts, core first.
pub const ALERT_CRATES: &[AlertCrate] = &[
    AlertCrate {
        name: "core",
        kinds: terrazgo_core::alerts::ALERT_KINDS,
        current: core_alerts,
    },
    AlertCrate {
        name: "phytosanitary",
        kinds: module_phytosanitary::alerts::ALERT_KINDS,
        current: phytosanitary_alerts,
    },
];

/// Core's own: the zone alerts, which need neither the date nor a setting.
fn core_alerts(
    conn: &Connection,
    _today: &str,
    _settings: &AppSettings,
) -> anyhow::Result<AlertReport> {
    Ok(terrazgo_core::repository::current_alerts(conn)?)
}

/// module-phytosanitary's: PHI windows, carnés and ITVs.
///
/// **The one place the shell builds an `AlertConfig`.** module-phytosanitary
/// deliberately gives it no `Default`, so there is no second way to conjure one:
/// the lead times come from this device's settings here, and an unset field
/// follows the module's own default — a farmer who never opened Settings tracks
/// the code.
fn phytosanitary_alerts(
    conn: &Connection,
    today: &str,
    settings: &AppSettings,
) -> anyhow::Result<AlertReport> {
    let config = module_phytosanitary::alerts::AlertConfig::from_overrides(
        settings.licence_lead_days,
        settings.itv_lead_days,
    );
    Ok(module_phytosanitary::repository::current_alerts(
        conn, today, &config,
    )?)
}

/// What gathering found: every crate's report merged, and the crates that
/// could not report at all.
#[derive(Debug, Default)]
pub struct Gathered {
    pub report: AlertReport,
    /// In [`ALERT_CRATES`] order.
    pub unavailable: Vec<Unavailable>,
}

/// A crate whose alerts could not be worked out at all — not one record it
/// could not read (that is an unchecked record, and names itself), but the
/// crate failing whole: the database, or a defect.
///
/// The farmer cannot fix this one, so the Status view says what they can do —
/// update the app, report it — and carries `detail` for them to pass on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unavailable {
    /// The crate's name on [`ALERT_CRATES`] (`alert.source.<name>`).
    pub source: &'static str,
    /// The error with its causes, untranslated: what a report needs.
    pub detail: String,
}

/// Every crate's alerts holding on `today`.
///
/// **A crate that fails is reported, never fatal.** Whatever broke it must not
/// hide another crate's alerts — a zone alert is still true when the PHI
/// windows could not be worked out — so the failing crate is named in
/// [`Gathered::unavailable`], with the error's detail, for the screen to say so;
/// the log gets the same. A record a crate merely could not read is not a
/// failure: it arrives in the report as unchecked, and the rest of that crate's
/// alerts with it.
pub fn gather(conn: &Connection, today: &str, settings: &AppSettings) -> Gathered {
    gather_from(ALERT_CRATES, conn, today, settings)
}

/// [`gather`] over a given list — the same loop, which is what lets a test hand
/// it a crate that fails.
fn gather_from(
    crates: &[AlertCrate],
    conn: &Connection,
    today: &str,
    settings: &AppSettings,
) -> Gathered {
    let mut gathered = Gathered::default();
    for alert_crate in crates {
        match (alert_crate.current)(conn, today, settings) {
            Ok(report) => gathered.report.absorb(report),
            Err(err) => {
                let detail = format!("{err:#}");
                eprintln!(
                    "warning: the {} alerts could not be worked out: {detail}",
                    alert_crate.name
                );
                gathered.unavailable.push(Unavailable {
                    source: alert_crate.name,
                    detail,
                });
            }
        }
    }
    gathered
}

/// The kind a card's code names, among every kind the alert crates declare —
/// `None` for a code no crate on the list declares.
pub fn kind_by_code(code: &str) -> Option<AlertKind> {
    ALERT_CRATES
        .iter()
        .flat_map(|alert_crate| alert_crate.kinds.iter().copied())
        .find(|kind| kind.code() == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrazgo_core::alerts::{DatedKind, RaisedAlert, UncheckedRecord};

    const TEST_KIND: DatedKind = DatedKind::new("test_expiry", "operator");

    /// One alert raised and one record it could not read — a crate that
    /// worked, whose report carries both halves.
    fn raises_one(
        _conn: &Connection,
        _today: &str,
        _settings: &AppSettings,
    ) -> anyhow::Result<AlertReport> {
        Ok(AlertReport {
            raised: vec![RaisedAlert::dated(
                TEST_KIND,
                "op-1".into(),
                "2026-07-15".into(),
            )],
            unchecked: vec![UncheckedRecord::new(
                TEST_KIND.kind(),
                "op-2".into(),
                "15/07/2026".into(),
            )],
        })
    }

    fn fails(
        _conn: &Connection,
        _today: &str,
        _settings: &AppSettings,
    ) -> anyhow::Result<AlertReport> {
        Err(anyhow::anyhow!("no such table: treatment_record"))
    }

    fn crate_that(
        name: &'static str,
        current: fn(&Connection, &str, &AppSettings) -> anyhow::Result<AlertReport>,
    ) -> AlertCrate {
        AlertCrate {
            name,
            kinds: &[],
            current,
        }
    }

    #[test]
    fn a_crate_that_fails_is_named_with_its_detail_and_the_others_still_report() {
        let conn = terrazgo_core::open_in_memory().unwrap();
        let crates = [
            crate_that("first", fails),
            crate_that("second", raises_one),
            crate_that("third", fails),
        ];
        let gathered = gather_from(&crates, &conn, "2026-06-11", &AppSettings::default());
        assert_eq!(
            gathered.report.raised.len(),
            1,
            "the working crate's alert is kept"
        );
        assert_eq!(
            gathered.report.unchecked.len(),
            1,
            "and its unchecked record, which is not a failure"
        );
        let names: Vec<&str> = gathered.unavailable.iter().map(|u| u.source).collect();
        assert_eq!(names, vec!["first", "third"]);
        assert!(
            gathered
                .unavailable
                .iter()
                .all(|u| u.detail == "no such table: treatment_record"),
            "each carries the error's detail for the farmer to pass on"
        );
    }

    #[test]
    fn nothing_failing_names_nothing() {
        // Core's schema and phyto's: every crate on the list reads its own.
        let conn = module_phytosanitary::open_in_memory().unwrap();
        let gathered = gather(&conn, "2026-06-11", &AppSettings::default());
        assert_eq!(
            gathered.report,
            AlertReport::default(),
            "an empty database raises nothing"
        );
        assert!(gathered.unavailable.is_empty());
    }

    #[test]
    fn every_declared_kind_is_found_by_its_code_and_an_unknown_code_is_not() {
        for alert_crate in ALERT_CRATES {
            for kind in alert_crate.kinds {
                assert_eq!(kind_by_code(kind.code()), Some(*kind));
            }
        }
        assert_eq!(kind_by_code("snow_warning"), None);
    }

    #[test]
    fn the_lead_times_come_from_this_devices_settings() {
        // An operator whose carné expires 34 days after "today": inside the
        // default 60-day lead, outside a 10-day one. The difference can only be
        // the setting reaching the rule.
        let mut conn = module_phytosanitary::open_in_memory().unwrap();
        terrazgo_core::repository::insert_operator(
            &mut conn,
            terrazgo_core::models::NewOperator {
                full_name: "Carlos Pérez".into(),
                tax_id: None,
                licence_number: Some("CL-12345".into()),
                licence_level_code: Some("qualified".into()),
                licence_expiry_date: Some("2026-07-15".into()),
            },
            None,
        )
        .unwrap();
        let raised = |settings: &AppSettings| {
            phytosanitary_alerts(&conn, "2026-06-11", settings)
                .unwrap()
                .raised
                .len()
        };
        let short = AppSettings {
            licence_lead_days: Some(10),
            ..AppSettings::default()
        };
        assert_eq!(raised(&AppSettings::default()), 1);
        assert_eq!(raised(&short), 0);
    }
}
