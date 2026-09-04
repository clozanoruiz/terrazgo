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

/// Lead times for the expiry alerts: how far ahead of an operator licence's or a
/// machine's ITV expiry date the alert opens. **Not regulatory values** — no decree
/// says when to start warning — and since 2026-08-26 the farmer can override both in
/// Settings, because the right answer is something they know and the app does not: a
/// carné de aplicador is renewed on a training course with limited dates, and how far
/// ahead one has to book varies by province and season.
///
/// Deliberately **not** `Default`. The values below are module-cue's own, for its
/// tests; the shell must resolve from device settings through [`AlertConfig::from_overrides`],
/// and `AlertConfig::default()` is exactly the reflex that would silently ignore a
/// farmer's choice while compiling perfectly. Removing the trait makes that a compile
/// error instead of a wrong lead time nothing on screen would reveal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertConfig {
    pub licence_lead_days: i64,
    pub itv_lead_days: i64,
}

/// Smallest and largest lead time a user may choose. Zero is excluded because a lead
/// time of nothing is a licence that first alerts on the day it expires — indistinguishable
/// from not being warned; the ceiling is a little over a year, past which the alert is
/// permanent and stops meaning anything.
pub const MIN_LEAD_DAYS: i64 = 1;
pub const MAX_LEAD_DAYS: i64 = 400;

/// Range-check a user-supplied lead time. The rule's owner owns its validation, the
/// way `terrazgo_geo::db::validate_tile_cache_cap` does for the tile cache.
pub fn validate_lead_days(days: i64) -> Result<()> {
    if !(MIN_LEAD_DAYS..=MAX_LEAD_DAYS).contains(&days) {
        return Err(CueError::Invalid("lead_days_out_of_range"));
    }
    Ok(())
}

impl AlertConfig {
    /// module-cue's own lead times (60 days licence / 30 days ITV, per the alerts
    /// design of 2026-06-11). **The shell must not call this** — it resolves from
    /// device settings via [`from_overrides`](Self::from_overrides).
    pub fn defaults() -> Self {
        Self {
            licence_lead_days: 60,
            itv_lead_days: 30,
        }
    }

    /// Build from device settings, each `None` following the default above.
    ///
    /// `None` means "the user never chose", not "the default at the time of writing" —
    /// so changing a default here reaches every user who left the setting alone, which
    /// is the same contract `AppSettings`' optional fields carry.
    pub fn from_overrides(licence_lead_days: Option<i64>, itv_lead_days: Option<i64>) -> Self {
        let defaults = Self::defaults();
        Self {
            licence_lead_days: licence_lead_days.unwrap_or(defaults.licence_lead_days),
            itv_lead_days: itv_lead_days.unwrap_or(defaults.itv_lead_days),
        }
    }
}

/// PHI window rule. `phi_end_date` (= application date + PHI days, per RD 1311/2012 and
/// the product label) is the FIRST day harvest is allowed again, so the alert is active
/// on `[application_date, phi_end_date)` — inclusive start, exclusive end.
///
/// ```
/// use module_cue::alerts::phi_window_is_active;
///
/// // A 21-day PHI applied on 12 March: the window runs to 1 April inclusive.
/// let (applied, ends) = ("2026-03-12", "2026-04-02");
/// assert!(phi_window_is_active(applied, ends, "2026-03-12").unwrap()); // day of
/// assert!(phi_window_is_active(applied, ends, "2026-04-01").unwrap()); // last day
///
/// // The end date is the first day harvest is ALLOWED, so the alert is over.
/// assert!(!phi_window_is_active(applied, ends, "2026-04-02").unwrap());
/// assert!(!phi_window_is_active(applied, ends, "2026-03-11").unwrap());
/// ```
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
///
/// ```
/// use module_cue::alerts::zone_alert_type;
///
/// assert_eq!(zone_alert_type("nitrate_vulnerable"), Some("nitrate_zone"));
///
/// // An unmapped zone kind raises nothing rather than failing: a future
/// // country's codes must not break alert refresh for everyone else.
/// assert_eq!(zone_alert_type("zone_humide"), None);
/// ```
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
///
/// ```
/// use module_cue::alerts::expiry_alert_is_active;
///
/// // A licence expiring 1 March, warned about 60 days ahead (the AlertConfig
/// // default — a lead time, not a regulatory figure).
/// // The window opens on 31 December, exactly 60 days before.
/// let expiry = "2027-03-01";
/// assert!(!expiry_alert_is_active(expiry, "2026-12-30", 60).unwrap());
/// assert!(expiry_alert_is_active(expiry, "2026-12-31", 60).unwrap());
///
/// // Past the date it does not resolve itself: an expired licence is the
/// // most urgent state there is.
/// assert!(expiry_alert_is_active(expiry, "2028-06-01", 60).unwrap());
/// ```
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
        let config = AlertConfig::defaults();
        assert_eq!(config.licence_lead_days, 60);
        assert_eq!(config.itv_lead_days, 30);
    }

    #[test]
    fn an_unset_override_follows_the_default_rather_than_a_captured_value() {
        // The whole point of storing `None` instead of 60: a user who never
        // touched the setting must move when the default moves.
        assert_eq!(
            AlertConfig::from_overrides(None, None),
            AlertConfig::defaults()
        );
    }

    #[test]
    fn each_override_applies_independently() {
        let config = AlertConfig::from_overrides(Some(90), None);
        assert_eq!(config.licence_lead_days, 90);
        assert_eq!(
            config.itv_lead_days,
            AlertConfig::defaults().itv_lead_days,
            "overriding one lead time must not disturb the other"
        );
    }

    #[test]
    fn lead_times_are_accepted_across_the_whole_offered_range() {
        assert!(validate_lead_days(MIN_LEAD_DAYS).is_ok());
        assert!(validate_lead_days(MAX_LEAD_DAYS).is_ok());
        assert!(validate_lead_days(60).is_ok());
    }

    #[test]
    fn lead_times_outside_the_range_are_refused() {
        // Zero would first alert on the expiry day itself, which is not a
        // warning; the ceiling keeps a permanent alert from being reachable.
        for days in [MIN_LEAD_DAYS - 1, MAX_LEAD_DAYS + 1, -30] {
            assert!(
                matches!(
                    validate_lead_days(days),
                    Err(CueError::Invalid("lead_days_out_of_range"))
                ),
                "{days} should have been refused"
            );
        }
    }
}
