// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Date maths shared app-wide (PHI calculation, alert logic), backed by the `jiff` crate.
//!
//! Compliance-critical date arithmetic lives on a battle-tested crate rather than bespoke
//! code; the public surface (`add_days`, `now_utc_iso`) is unchanged from the earlier
//! hand-rolled implementation. Moved here from module-phytosanitary (2026-06-12) — the maths was
//! never phytosanitary-specific.

use crate::error::{CoreError, Result};
use jiff::{Timestamp, ToSpan, civil::Date};

/// Parse a `YYYY-MM-DD` string into a calendar date, mapping failure to `InvalidDate`.
/// Public because module crates build their own date rules on it (e.g. the phytosanitary module's alert rules).
///
/// ```
/// use terrazgo_core::date::parse_date;
///
/// assert!(parse_date("2028-02-29").is_ok()); // 2028 is a leap year
/// assert!(parse_date("2026-02-29").is_err()); // 2026 is not
/// // A date-only field is exactly ten characters; anything else is a typo,
/// // never a partially-understood value.
/// assert!(parse_date("2026-02-28T00:00:00Z").is_err());
/// ```
pub fn parse_date(date: &str) -> Result<Date> {
    date.parse()
        .map_err(|_| CoreError::InvalidDate(date.to_string()))
}

/// Refuse any recorded date that is not `YYYY-MM-DD` — the write-side guard for
/// a date some rule reads later. A date not recorded (`None`) always passes.
///
/// **At the write, not only where a rule reads it**: a date nobody checked on
/// the way in is found by the first rule that reads it — the alert list, days
/// later, on a screen that is not the form holding the mistake. Refused here,
/// the farmer sees "Fecha no válida" on the form they are filling.
///
/// ```
/// use terrazgo_core::date::validate_dates;
///
/// assert!(validate_dates(&[Some("2026-08-15"), None]).is_ok());
/// assert!(validate_dates(&[None, Some("15/08/2026")]).is_err());
/// ```
pub fn validate_dates(dates: &[Option<&str>]) -> Result<()> {
    for date in dates.iter().flatten() {
        parse_date(date)?;
    }
    Ok(())
}

/// Add `days` to a `YYYY-MM-DD` date, returning a `YYYY-MM-DD` date.
/// Month/year/leap-day/campaign-boundary crossings are handled by `jiff`.
///
/// This is how a PHI end date is derived: RD 1311/2012 makes the plazo de
/// seguridad a number of days from the application, and the resulting date is
/// the FIRST day harvest is allowed — not the last day it is forbidden.
///
/// ```
/// use terrazgo_core::date::add_days;
///
/// // A 21-day PHI applied on 12 March: harvest is allowed from 2 April.
/// assert_eq!(add_days("2026-03-12", 21).unwrap(), "2026-04-02");
///
/// // Leap day and campaign boundaries are `jiff`'s problem, not ours.
/// assert_eq!(add_days("2028-02-28", 1).unwrap(), "2028-02-29");
/// assert_eq!(add_days("2026-02-28", 1).unwrap(), "2026-03-01");
/// assert_eq!(add_days("2026-12-31", 1).unwrap(), "2027-01-01");
/// ```
pub fn add_days(date: &str, days: i64) -> Result<String> {
    let result = parse_date(date)?
        .checked_add(days.days())
        .map_err(|_| CoreError::InvalidDate(date.to_string()))?;
    Ok(result.to_string())
}

/// Current UTC instant as `YYYY-MM-DDTHH:MM:SSZ` (the storage format for timestamps).
pub fn now_utc_iso() -> String {
    // `strftime` on a `Timestamp` formats in UTC and truncates to whole seconds.
    Timestamp::now().strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Current instant in milliseconds since the Unix epoch — the reading the
/// hybrid logical clock is built from, and the one an import measures an
/// incoming stamp against.
///
/// **Not a timestamp to store or show.** Everything a person or an inspection
/// reads is [`now_utc_iso`]; this is for the two places that do arithmetic on
/// instants, and it is a function here rather than a `Timestamp::now()` at each
/// of them so that "what time is it" has one answer in this crate. The callers
/// that compare against it take it as a parameter, which is what lets a wrong
/// clock be tested without one.
pub fn now_ms() -> i64 {
    Timestamp::now().as_millisecond()
}

/// Current UTC date as `YYYY-MM-DD` — the "today" the alert rules compare
/// date-only fields against. Date-only, UTC: a treatment recorded at 23:30 local
/// must not flip alert state depending on the device's timezone.
pub fn today_utc() -> String {
    Timestamp::now().strftime("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // PHI end-date calculation is a compliance rule (docs/architecture.md testing strategy #1),
    // so these are written as specs of the expected regulatory behaviour.

    #[test]
    fn add_days_simple_within_month() {
        // PHI of 21 days on a treatment applied 2026-06-10 → re-entry/harvest from 2026-07-01.
        assert_eq!(add_days("2026-06-10", 21).unwrap(), "2026-07-01");
    }

    #[test]
    fn add_days_zero_is_identity() {
        assert_eq!(add_days("2026-06-10", 0).unwrap(), "2026-06-10");
    }

    #[test]
    fn add_days_crosses_leap_day() {
        // 2024 is a leap year: 28 Feb + 1 = 29 Feb, + 2 = 1 Mar.
        assert_eq!(add_days("2024-02-28", 1).unwrap(), "2024-02-29");
        assert_eq!(add_days("2024-02-28", 2).unwrap(), "2024-03-01");
    }

    #[test]
    fn add_days_non_leap_february() {
        // 2023 is not a leap year: 28 Feb + 1 = 1 Mar.
        assert_eq!(add_days("2023-02-28", 1).unwrap(), "2023-03-01");
    }

    #[test]
    fn add_days_crosses_campaign_boundary() {
        // PHI spanning the new year (campaign boundary): 20 Dec 2025 + 21 = 10 Jan 2026.
        assert_eq!(add_days("2025-12-20", 21).unwrap(), "2026-01-10");
    }

    #[test]
    fn add_days_century_leap_rule() {
        // 2000 is a leap year (divisible by 400); 28 Feb + 1 = 29 Feb.
        assert_eq!(add_days("2000-02-28", 1).unwrap(), "2000-02-29");
    }

    #[test]
    fn today_utc_is_a_valid_date_only_string() {
        let today = today_utc();
        // Shape: parseable as YYYY-MM-DD by the same parser the alert logic uses.
        assert!(parse_date(&today).is_ok(), "today_utc() returned {today}");
        assert_eq!(today.len(), 10);
    }

    #[test]
    fn rejects_malformed_date() {
        assert!(matches!(
            add_days("2026/06/10", 1),
            Err(CoreError::InvalidDate(_))
        ));
        assert!(matches!(
            add_days("2026-13-01", 1),
            Err(CoreError::InvalidDate(_))
        ));
    }
}
