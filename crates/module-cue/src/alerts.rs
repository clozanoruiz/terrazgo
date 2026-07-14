// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert rules: pure, date-only compliance logic (docs/architecture.md testing strategy #1, test-first).
//!
//! No database access here — these functions decide whether a condition holds on a given
//! day. The repository's `refresh_alerts` reconciles their decisions into the `alert`
//! table. `today` is always a parameter (never read from the clock) so the rules are
//! deterministic and testable.

use crate::date::parse_date;
use crate::error::{CueError, Result};
use jiff::ToSpan;

/// Lead times for the expiry alerts. These are user convenience, not regulatory values;
/// the future core settings UI will let the user override them.
#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub licence_lead_days: i64,
    pub itv_lead_days: i64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            licence_lead_days: 60,
            itv_lead_days: 30,
        }
    }
}

/// PHI window rule. `phi_end_date` (= application date + PHI days, per RD 1311/2012 and
/// the product label) is the FIRST day harvest is allowed again, so the alert is active
/// on `[application_date, phi_end_date)` — inclusive start, exclusive end.
pub fn phi_window_is_active(
    application_date: &str,
    phi_end_date: &str,
    today: &str,
) -> Result<bool> {
    let start = parse_date(application_date)?;
    let end = parse_date(phi_end_date)?;
    let today = parse_date(today)?;
    Ok(today >= start && today < end)
}

/// Zone-flag rule (P4, 2026-07-08): a plot's latest-campaign check saying
/// 'inside' is a standing condition — no date window; it clears only when a
/// newer check says 'outside' or the plot is deleted.
pub fn zone_alert_is_active(status: &str) -> bool {
    status == "inside"
}

/// The alert type raised for a zone kind. `None` for zone types the alert
/// engine does not know (a future country's codes simply raise nothing until
/// a mapping is added — never an error).
pub fn zone_alert_type(zone_type_code: &str) -> Option<&'static str> {
    match zone_type_code {
        "nitrate_vulnerable" => Some("nitrate_zone"),
        "phytosanitary_restriction" => Some("phyto_zone"),
        "natura_2000" => Some("natura_zone"),
        _ => None,
    }
}

/// Expiry rule (operator licence, machinery ITV): active from `expiry_date - lead_days`
/// onward, and it STAYS active once the date has passed — an expired licence or overdue
/// inspection is the most urgent state, not a resolved one. It only clears when the
/// source row's date changes (renewal) or the subject is deleted.
pub fn expiry_alert_is_active(expiry_date: &str, today: &str, lead_days: i64) -> Result<bool> {
    let expiry = parse_date(expiry_date)?;
    let today = parse_date(today)?;
    // The day the lead window opens; `checked_sub` can only fail on out-of-range dates.
    let window_opens = expiry
        .checked_sub(lead_days.days())
        .map_err(|_| CueError::InvalidDate(expiry_date.to_string()))?;
    Ok(today >= window_opens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CueError;

    // --- PHI window (plazo de seguridad, RD 1311/2012) ------------------------
    // The window semantics match date.rs: PHI 21 applied 2026-06-10 → harvest allowed
    // from 2026-07-01, so the alert must be live up to and including 2026-06-30.

    #[test]
    fn phi_active_on_the_application_day() {
        assert!(phi_window_is_active("2026-06-10", "2026-07-01", "2026-06-10").unwrap());
    }

    #[test]
    fn phi_active_mid_window() {
        assert!(phi_window_is_active("2026-06-10", "2026-07-01", "2026-06-20").unwrap());
    }

    #[test]
    fn phi_active_on_the_last_restricted_day() {
        assert!(phi_window_is_active("2026-06-10", "2026-07-01", "2026-06-30").unwrap());
    }

    #[test]
    fn phi_inactive_on_the_end_date_itself() {
        // phi_end_date is the first day harvest is allowed → no alert that day.
        assert!(!phi_window_is_active("2026-06-10", "2026-07-01", "2026-07-01").unwrap());
    }

    #[test]
    fn phi_inactive_before_the_application_date() {
        // A record entered ahead of the actual application must not alert early.
        assert!(!phi_window_is_active("2026-06-10", "2026-07-01", "2026-06-09").unwrap());
    }

    #[test]
    fn phi_window_spans_a_leap_day() {
        // 2024 is a leap year; the window must be live on 29 Feb.
        assert!(phi_window_is_active("2024-02-20", "2024-03-05", "2024-02-29").unwrap());
    }

    #[test]
    fn phi_window_spans_the_campaign_year_boundary() {
        assert!(phi_window_is_active("2025-12-20", "2026-01-10", "2026-01-05").unwrap());
        assert!(!phi_window_is_active("2025-12-20", "2026-01-10", "2026-01-10").unwrap());
    }

    #[test]
    fn phi_rejects_malformed_dates() {
        // Compliance logic must fail loudly, never silently skip a record.
        assert!(matches!(
            phi_window_is_active("2026/06/10", "2026-07-01", "2026-06-15"),
            Err(CueError::InvalidDate(_))
        ));
        assert!(matches!(
            phi_window_is_active("2026-06-10", "2026-07-01", "not-a-date"),
            Err(CueError::InvalidDate(_))
        ));
    }

    // --- expiry alerts (operator licence / machinery ITV) ---------------------

    #[test]
    fn expiry_inactive_the_day_before_the_lead_window() {
        // Expiry 2026-08-01, lead 60 → window opens 2026-06-02.
        assert!(!expiry_alert_is_active("2026-08-01", "2026-06-01", 60).unwrap());
    }

    #[test]
    fn expiry_active_on_the_first_day_of_the_lead_window() {
        assert!(expiry_alert_is_active("2026-08-01", "2026-06-02", 60).unwrap());
    }

    #[test]
    fn expiry_active_on_the_expiry_day() {
        assert!(expiry_alert_is_active("2026-08-01", "2026-08-01", 60).unwrap());
    }

    #[test]
    fn expiry_stays_active_after_the_date_has_passed() {
        // An expired licence is the most urgent state — it must not self-resolve.
        assert!(expiry_alert_is_active("2026-08-01", "2027-01-15", 60).unwrap());
    }

    #[test]
    fn expiry_lead_window_crosses_the_year_boundary() {
        // Expiry 2026-01-15, lead 30 → window opens 2025-12-16.
        assert!(!expiry_alert_is_active("2026-01-15", "2025-12-15", 30).unwrap());
        assert!(expiry_alert_is_active("2026-01-15", "2025-12-16", 30).unwrap());
    }

    #[test]
    fn expiry_with_zero_lead_alerts_only_from_the_expiry_day() {
        assert!(!expiry_alert_is_active("2026-08-01", "2026-07-31", 0).unwrap());
        assert!(expiry_alert_is_active("2026-08-01", "2026-08-01", 0).unwrap());
    }

    #[test]
    fn expiry_rejects_malformed_dates() {
        assert!(matches!(
            expiry_alert_is_active("01/08/2026", "2026-06-15", 60),
            Err(CueError::InvalidDate(_))
        ));
    }

    // --- zone flags (nitrate/phyto/Natura, P4) ---------------------------------

    #[test]
    fn zone_alert_active_only_when_inside() {
        assert!(zone_alert_is_active("inside"));
        assert!(!zone_alert_is_active("outside"));
    }

    #[test]
    fn zone_alert_types_map_known_codes_and_ignore_unknown_ones() {
        assert_eq!(zone_alert_type("nitrate_vulnerable"), Some("nitrate_zone"));
        assert_eq!(
            zone_alert_type("phytosanitary_restriction"),
            Some("phyto_zone")
        );
        assert_eq!(zone_alert_type("natura_2000"), Some("natura_zone"));
        // Forward compatibility: an unmapped zone type raises nothing.
        assert_eq!(zone_alert_type("fr_some_future_zone"), None);
    }

    #[test]
    fn default_config_lead_times() {
        // 60 days licence / 30 days ITV, per the alerts design (2026-06-11).
        let config = AlertConfig::default();
        assert_eq!(config.licence_lead_days, 60);
        assert_eq!(config.itv_lead_days, 30);
    }
}
