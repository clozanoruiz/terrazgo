// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alerts: what every crate that raises one shares. Pure — no database here.
//!
//! An alert is a condition the farmer should act on that holds TODAY: an open
//! plazo de seguridad, a licence about to expire, a plot inside a nitrate zone.
//! **Nothing stores alerts.** Each crate that raises them returns the ones
//! holding now, in the standard shape [`RaisedAlert`], and
//! [`crate::repository::list_alerts`] assembles the list from those and from
//! what people did about them (`alert_acknowledgement`, the one part that is
//! stored, because no device can re-derive it). A list worked out when it is
//! read cannot be stale, and there is no copy a forgotten refresh could leave
//! wrong (docs/data-model.md → "Alerts: the settled design").
//!
//! **Core knows no kind but its own.** A crate declares each kind it raises as
//! a constant — [`DatedKind`] or [`StandingKind`] — and core never lists
//! another crate's kinds, so a module adds an alert without touching core.
//! Core's own kinds are the three zone alerts below: their rule, "the latest
//! check says inside", belongs to no one domain.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;

use crate::date::parse_date;
use crate::error::{CoreError, Result};
use crate::models::AlertAcknowledgement;

/// What core needs to know about one kind of alert: its code, the table its
/// subjects are in, and whether its condition is standing (holds until the data
/// changes, with no date to end on).
///
/// A crate never builds one directly: it declares a [`DatedKind`] or a
/// [`StandingKind`] and lists them with `.kind()`. The code is the key of the
/// kind's text in every dictionary (`alert.type.<code>`), and must be unique
/// across every crate that raises alerts; the subject table must be one that
/// syncs — the shell's contract test checks all three.
///
/// **The subject's table is the kind's, never the caller's.** A plazo is
/// always about a treatment and a carné always about an operator, so the table
/// is declared once, beside the code, and every alert and every act of the kind
/// takes it from here. An act then cannot be filed under a table its kind is
/// never about — which, with the table taken from the screen, nothing stopped:
/// the column is a code the database cannot check (docs/data-model.md →
/// "Alerts: the settled design").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlertKind {
    code: &'static str,
    subject_table: &'static str,
    standing: bool,
}

impl AlertKind {
    /// The code this kind is sent and stored as (`"phi_window"`).
    pub const fn code(self) -> &'static str {
        self.code
    }

    /// The table every alert of this kind is about (`"treatment_record"`).
    pub const fn subject_table(self) -> &'static str {
        self.subject_table
    }

    /// Whether the condition holds indefinitely rather than ending on a date.
    pub const fn is_standing(self) -> bool {
        self.standing
    }
}

/// A kind whose alerts end on a date: a plazo lapses, a licence is renewed.
/// Every alert of it carries that date.
///
/// **Why two wrapper types rather than one kind with a `standing` flag.** The
/// pairing that matters is "a dated alert always has its date, a standing one
/// never has one", and a flag passed next to an optional date is two values a
/// caller can get out of step. Wrapping the kind in one of two types lets
/// [`RaisedAlert::dated`] accept only a `DatedKind` and require the date, and
/// [`RaisedAlert::standing`] accept only a `StandingKind` and take none — so
/// the compiler refuses the wrong pairing instead of a test having to catch it.
/// (The pattern is called a *newtype*: a struct with one field, there to give
/// a value a type of its own.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatedKind(AlertKind);

impl DatedKind {
    /// A kind coded `code` whose alerts are about rows of `subject_table`.
    pub const fn new(code: &'static str, subject_table: &'static str) -> Self {
        Self(AlertKind {
            code,
            subject_table,
            standing: false,
        })
    }

    /// The kind itself, for a crate's list of the kinds it declares.
    pub const fn kind(self) -> AlertKind {
        self.0
    }
}

/// A kind whose condition holds until the data says otherwise: a plot is in a
/// nitrate zone for as long as the zone says so, and no date clears it. Its
/// alerts carry no due date — printing one would announce a deadline nobody
/// set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandingKind(AlertKind);

impl StandingKind {
    /// A kind coded `code` whose alerts are about rows of `subject_table`.
    pub const fn new(code: &'static str, subject_table: &'static str) -> Self {
        Self(AlertKind {
            code,
            subject_table,
            standing: true,
        })
    }

    /// The kind itself, for a crate's list of the kinds it declares.
    pub const fn kind(self) -> AlertKind {
        self.0
    }
}

/// One condition holding now, as the crate that raised it reports it — the
/// standard shape every crate fills.
///
/// The fields are private so the pairing [`DatedKind`] explains cannot be
/// undone after construction; the getters below read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaisedAlert {
    kind: AlertKind,
    subject_id: String,
    subject_label: Option<String>,
    due_date: Option<String>,
}

impl RaisedAlert {
    /// An alert that ends on `due_date` (`YYYY-MM-DD`): the plazo's end, the
    /// licence's expiry, the ITV's due date.
    ///
    /// `subject_id` says which row it is about — the one a farmer would open to
    /// deal with it, in the table the kind names. With the kind it is also what
    /// an act on the alert is filed under, so it must be the same every time
    /// the condition is raised.
    pub fn dated(kind: DatedKind, subject_id: String, due_date: String) -> Self {
        Self {
            kind: kind.kind(),
            subject_id,
            subject_label: None,
            due_date: Some(due_date),
        }
    }

    /// An alert whose condition holds until the data changes.
    pub fn standing(kind: StandingKind, subject_id: String) -> Self {
        Self {
            kind: kind.kind(),
            subject_id,
            subject_label: None,
            due_date: None,
        }
    }

    /// The same condition, rebuilt from what the screen sends back when the
    /// farmer acts on it — a kind the shell looked up by code, the subject the
    /// card was about, and the date it showed. The subject's table is the
    /// kind's, so the screen does not send one.
    ///
    /// The only place the pairing is checked at run time, because this is the
    /// only input that does not come from a crate's own constants: a standing
    /// kind with a date, or a dated one without, is refused rather than filed
    /// under a deadline no listed alert could ever match.
    pub(crate) fn acted_on(
        kind: AlertKind,
        subject_id: &str,
        due_date: Option<&str>,
    ) -> Result<Self> {
        if kind.is_standing() != due_date.is_none() {
            return Err(CoreError::Invalid("alert_deadline_mismatch"));
        }
        if let Some(date) = due_date {
            parse_date(date)?;
        }
        Ok(Self {
            kind,
            subject_id: subject_id.to_owned(),
            subject_label: None,
            due_date: due_date.map(str::to_owned),
        })
    }

    /// Name the subject as a farmer knows it: the plot, the person, the machine.
    /// `None` leaves the screen to fall back to naming the kind of thing.
    pub fn set_subject_label(&mut self, label: Option<String>) {
        self.subject_label = label;
    }

    pub fn kind(&self) -> AlertKind {
        self.kind
    }

    /// The table the subject is in — its kind's.
    pub fn subject_table(&self) -> &'static str {
        self.kind.subject_table()
    }

    pub fn subject_id(&self) -> &str {
        &self.subject_id
    }

    pub fn subject_label(&self) -> Option<&str> {
        self.subject_label.as_deref()
    }

    pub fn due_date(&self) -> Option<&str> {
        self.due_date.as_deref()
    }

    /// This alert as the list shows it on `today`, standing where `status`
    /// says.
    pub(crate) fn into_listed(
        self,
        status: AlertStatus,
        acknowledged_at: Option<String>,
        today: &str,
    ) -> Alert {
        Alert {
            overdue: is_overdue(self.due_date.as_deref(), today),
            alert_type_code: self.kind.code(),
            subject_table: self.kind.subject_table(),
            subject_id: self.subject_id,
            subject_label: self.subject_label,
            due_date: self.due_date,
            standing: self.kind.is_standing(),
            status,
            acknowledged_at,
        }
    }

    /// Whether `act` was made about this condition AND this deadline.
    ///
    /// **The deadline is what stops an old dismissal silencing a new
    /// occurrence.** An act outlives the alert it was made on, and the subject
    /// comes back: a renewed licence nears its next expiry under the same
    /// operator, an ITV falls due again for the same machine. So an act names
    /// the deadline it saw and applies only while the alert still has it. A
    /// corrected date is a different deadline too, which brings the alert back
    /// — a plazo moved two weeks later is exactly what should not stay hidden.
    /// A standing alert has no date, and an act on one names none, so it holds
    /// whenever the condition does — a plot leaving a zone and entering it
    /// again comes back as it was left (docs/sync.md → Alert acknowledgements
    /// roam).
    ///
    /// The table is compared although this build's kind decides it: an act can
    /// arrive by sync from a build whose kind of the same code was about
    /// another table, and such an act was never about this alert.
    fn is_about(&self, act: &AlertAcknowledgement) -> bool {
        act.alert_type_code == self.kind.code()
            && act.subject_table == self.kind.subject_table()
            && act.subject_id == self.subject_id
            && act.due_date == self.due_date
    }
}

/// A record a crate could not check: one of its values could not be read, so
/// whether it raises an alert is unknown.
///
/// **Reported, named, and the rest still worked out.** The alternative — the
/// crate failing whole — would hide every other alert it raises behind one bad
/// row, and could only say "something could not be checked". This names the
/// record and the value, so the Status view can tell the farmer what to open
/// and what to correct. Stored nowhere, like the alerts themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UncheckedRecord {
    /// The kind of alert that could not be worked out for this record.
    pub alert_type_code: &'static str,
    /// The kind's subject table, which is where the record is.
    pub subject_table: &'static str,
    pub subject_id: String,
    /// The record as a farmer knows it, filled like a raised alert's.
    pub subject_label: Option<String>,
    /// The value that could not be read, exactly as stored.
    pub value: String,
}

impl UncheckedRecord {
    pub fn new(kind: AlertKind, subject_id: String, value: String) -> Self {
        Self {
            alert_type_code: kind.code(),
            subject_table: kind.subject_table(),
            subject_id,
            subject_label: None,
            value,
        }
    }
}

/// What a crate that raises alerts reports: the alerts holding now, and the
/// records it could not check. The standard answer every crate's
/// `current_alerts` gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlertReport {
    pub raised: Vec<RaisedAlert>,
    pub unchecked: Vec<UncheckedRecord>,
}

impl AlertReport {
    /// Another crate's report added to this one — how the shell gathers them.
    pub fn absorb(&mut self, other: AlertReport) {
        self.raised.extend(other.raised);
        self.unchecked.extend(other.unchecked);
    }

    /// The unchecked records in the order the Status view lists them: by kind,
    /// then by the record's name, then by id, so two reads list them alike.
    pub fn unchecked_in_listing_order(&self) -> Vec<UncheckedRecord> {
        let mut records = self.unchecked.clone();
        records.sort_by(|a, b| {
            a.alert_type_code
                .cmp(b.alert_type_code)
                .then_with(|| a.subject_label.cmp(&b.subject_label))
                .then_with(|| a.subject_id.cmp(&b.subject_id))
        });
        records
    }
}

/// Where an alert stands with the people who have seen it.
///
/// **Declared weakest first, and the order is the rule.** `derive(PartialOrd,
/// Ord)` on an enum orders its variants as they are written, so `Active <
/// Acknowledged < Dismissed`, and "the strongest act wins" is simply `max`.
/// That is how two devices acting on one alert without seeing each other reach
/// the same answer — a dismissal beats an acknowledgement whichever was made
/// first — with no clock and no person involved (docs/sync.md → Alert
/// acknowledgements roam).
///
/// Serialised as its code (`"acknowledged"`), which is both what the Status
/// view compares against and what `alert_acknowledgement.status` stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Dismissed,
}

impl AlertStatus {
    /// The code this status is stored and sent as.
    pub fn code(self) -> &'static str {
        match self {
            AlertStatus::Active => "active",
            AlertStatus::Acknowledged => "acknowledged",
            AlertStatus::Dismissed => "dismissed",
        }
    }

    /// The status a stored code names, or `None` for a code that names none.
    pub fn from_code(code: &str) -> Option<AlertStatus> {
        match code {
            "active" => Some(AlertStatus::Active),
            "acknowledged" => Some(AlertStatus::Acknowledged),
            "dismissed" => Some(AlertStatus::Dismissed),
            _ => None,
        }
    }
}

/// One alert as the Status view shows it: the condition, and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Alert {
    pub alert_type_code: &'static str,
    pub subject_table: &'static str,
    pub subject_id: String,
    /// The subject as a farmer knows it ("Los Alcores", not "plot"); `None`
    /// when the crate could not name it, and the view names the kind instead.
    pub subject_label: Option<String>,
    /// The date the condition turns on; `None` exactly when `standing`.
    pub due_date: Option<String>,
    /// Whether the condition holds with no date to end on. The view phrases a
    /// standing alert without a date; the classification is the kind's, in
    /// Rust beside its rule, never a list the view keeps.
    pub standing: bool,
    /// Whether the date has already passed while the condition still holds —
    /// an expired carné, an overdue ITV. The most urgent state, and one the
    /// card must not word as upcoming ([`is_overdue`]).
    pub overdue: bool,
    /// `Active` or `Acknowledged`: a dismissed alert is not listed.
    pub status: AlertStatus,
    /// When somebody first acted on this alert's current deadline, on any
    /// device.
    pub acknowledged_at: Option<String>,
}

/// Whether an alert due on `due_date` is overdue on `today` (both
/// `YYYY-MM-DD`): the date has passed and the alert is still raised.
///
/// **Strictly after**, because the date itself is still inside: a carné that
/// expires on 15 August is valid that day, and an ITV due on 1 July can be
/// passed on the 1st. A PHI window can never be overdue — it stops being raised
/// on its end date — and a standing alert has no date to pass.
///
/// Compared as text, which is exact for the ISO form every rule raises: its
/// fields run from the largest unit to the smallest, at fixed width.
///
/// ```
/// use terrazgo_core::alerts::is_overdue;
///
/// assert!(!is_overdue(Some("2026-08-15"), "2026-08-15")); // the last valid day
/// assert!(is_overdue(Some("2026-08-15"), "2026-08-16"));
/// assert!(!is_overdue(None, "2026-08-16")); // standing: no date to pass
/// ```
pub fn is_overdue(due_date: Option<&str>, today: &str) -> bool {
    due_date.is_some_and(|due| due < today)
}

/// Where an alert stands, given the acts that might be about it.
///
/// `acts` may include acts on other conditions or other deadlines — only the
/// ones [`RaisedAlert::is_about`] accepts count. Returns the strongest of those
/// and when the alert was first seen, the earliest of them. Nobody having acted
/// leaves it `Active` and unseen.
pub fn alert_status(
    alert: &RaisedAlert,
    acts: &[AlertAcknowledgement],
) -> (AlertStatus, Option<String>) {
    let about_this = acts.iter().filter(|act| alert.is_about(act));
    let status = about_this
        .clone()
        .map(|act| act.status)
        .max()
        .unwrap_or(AlertStatus::Active);
    // ISO 8601 UTC text sorts chronologically, so the smallest string is the
    // earliest instant.
    let first_seen = about_this.map(|act| act.created_at.clone()).min();
    (status, first_seen)
}

/// One alert per condition, whatever the crates handed in.
///
/// A condition is a kind and a subject — the kind names the subject's table. A
/// crate raising the same condition twice is a defect in that crate, but it
/// must not reach the screen as two cards — the view keys each card by its
/// condition, and two cards under one key is an error there, not a duplicate.
/// Of two, the one due soonest is kept, being the more urgent reading; the
/// order they arrived in decides a tie, so the answer never depends on hashing.
pub(crate) fn one_per_condition(raised: Vec<RaisedAlert>) -> Vec<RaisedAlert> {
    let mut kept: Vec<RaisedAlert> = Vec::with_capacity(raised.len());
    let mut position: HashMap<(AlertKind, String), usize> = HashMap::new();
    for alert in raised {
        let key = (alert.kind, alert.subject_id.clone());
        match position.get(&key) {
            Some(&at) => {
                // `Option<String>` orders `None` first, but a kind is either
                // dated or standing, so two alerts of one kind are both `Some`
                // or both `None` — this compares dates, or equals.
                if alert.due_date < kept[at].due_date {
                    kept[at] = alert;
                }
            }
            None => {
                position.insert(key, kept.len());
                kept.push(alert);
            }
        }
    }
    kept
}

/// The order the Status view lists alerts in: every dated alert first, soonest
/// deadline at the top — those are the ones with a clock running — then the
/// standing conditions. Ties go by kind, then by subject, so the order never
/// depends on which crate reported first. (One kind is one subject table, so
/// the table decides nothing the kind has not.)
pub(crate) fn listing_order(a: &Alert, b: &Alert) -> Ordering {
    a.standing
        .cmp(&b.standing)
        .then_with(|| a.due_date.cmp(&b.due_date))
        .then_with(|| a.alert_type_code.cmp(b.alert_type_code))
        .then_with(|| a.subject_id.cmp(&b.subject_id))
}

// --- core's own kinds: the zone alerts --------------------------------------

/// A plot inside a nitrate-vulnerable zone.
pub const NITRATE_ZONE: StandingKind = StandingKind::new("nitrate_zone", "plot");
/// A plot inside a phytosanitary restriction zone.
pub const PHYTO_ZONE: StandingKind = StandingKind::new("phyto_zone", "plot");
/// A plot inside Red Natura 2000.
pub const NATURA_ZONE: StandingKind = StandingKind::new("natura_zone", "plot");

/// Every kind core raises — its entry in the shell's list of alert crates.
pub const ALERT_KINDS: &[AlertKind] = &[NITRATE_ZONE.kind(), PHYTO_ZONE.kind(), NATURA_ZONE.kind()];

/// The alert a zone kind raises, or `None` for a zone kind no alert is mapped
/// to: a future country's zone codes simply raise nothing until one is added,
/// and never break the list for everyone else.
///
/// ```
/// use terrazgo_core::alerts::{zone_alert_kind, NITRATE_ZONE};
///
/// assert_eq!(zone_alert_kind("nitrate_vulnerable"), Some(NITRATE_ZONE));
/// assert_eq!(zone_alert_kind("zone_humide"), None);
/// ```
pub fn zone_alert_kind(zone_type_code: &str) -> Option<StandingKind> {
    match zone_type_code {
        "nitrate_vulnerable" => Some(NITRATE_ZONE),
        "phytosanitary_restriction" => Some(PHYTO_ZONE),
        "natura_2000" => Some(NATURA_ZONE),
        _ => None,
    }
}

/// A plot's latest check saying `inside` is a standing condition: no date
/// window, and it clears only when a newer check says `outside` or the plot is
/// deleted.
pub fn zone_alert_is_active(status: &str) -> bool {
    status == "inside"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const EXPIRY: DatedKind = DatedKind::new("licence_expiry", "operator");
    const OTHER_EXPIRY: DatedKind = DatedKind::new("itv_expiry", "machinery");

    fn dated(subject: &str, due: &str) -> RaisedAlert {
        RaisedAlert::dated(EXPIRY, subject.into(), due.into())
    }

    fn zone(subject: &str) -> RaisedAlert {
        RaisedAlert::standing(NITRATE_ZONE, subject.into())
    }

    fn act(alert: &RaisedAlert, status: AlertStatus, at: &str) -> AlertAcknowledgement {
        AlertAcknowledgement {
            id: format!("act-{at}"),
            alert_type_code: alert.kind().code().into(),
            subject_table: alert.subject_table().into(),
            subject_id: alert.subject_id().into(),
            due_date: alert.due_date().map(str::to_owned),
            status,
            created_at: at.into(),
        }
    }

    fn listed(alert: &RaisedAlert) -> Alert {
        Alert {
            alert_type_code: alert.kind().code(),
            subject_table: alert.subject_table(),
            subject_id: alert.subject_id().into(),
            subject_label: None,
            due_date: alert.due_date().map(str::to_owned),
            standing: alert.kind().is_standing(),
            overdue: false,
            status: AlertStatus::Active,
            acknowledged_at: None,
        }
    }

    // --- kinds and the standard shape -----------------------------------------

    #[test]
    fn a_dated_kind_is_not_standing_and_a_standing_kind_is() {
        assert_eq!(EXPIRY.kind().code(), "licence_expiry");
        assert!(!EXPIRY.kind().is_standing());
        assert_eq!(NITRATE_ZONE.kind().code(), "nitrate_zone");
        assert!(NITRATE_ZONE.kind().is_standing());
    }

    #[test]
    fn a_dated_alert_carries_its_date_and_a_standing_one_none() {
        let licence = dated("op-1", "2026-07-15");
        assert_eq!(licence.due_date(), Some("2026-07-15"));
        assert_eq!(licence.kind(), EXPIRY.kind());
        assert_eq!(licence.subject_table(), "operator");
        assert_eq!(licence.subject_id(), "op-1");
        assert_eq!(
            licence.subject_label(),
            None,
            "unnamed until its crate names it"
        );

        let zoned = zone("plot-1");
        assert_eq!(zoned.due_date(), None);
        assert!(zoned.kind().is_standing());
    }

    #[test]
    fn a_subject_label_is_set_and_can_be_cleared() {
        let mut alert = zone("plot-1");
        alert.set_subject_label(Some("Los Alcores".into()));
        assert_eq!(alert.subject_label(), Some("Los Alcores"));
        alert.set_subject_label(None);
        assert_eq!(alert.subject_label(), None);
    }

    #[test]
    fn what_the_screen_sends_back_is_rebuilt_when_it_pairs_up() {
        let back = RaisedAlert::acted_on(EXPIRY.kind(), "op-1", Some("2026-07-15")).unwrap();
        assert_eq!(back, dated("op-1", "2026-07-15"));
        let back = RaisedAlert::acted_on(NITRATE_ZONE.kind(), "plot-1", None).unwrap();
        assert_eq!(back, zone("plot-1"));
    }

    #[test]
    fn the_subject_table_is_always_the_kinds() {
        // Raised, rebuilt from what the screen sends back, listed, or unchecked:
        // nothing but the kind supplies the table, so an act cannot be filed
        // under a table its kind is never about.
        assert_eq!(EXPIRY.kind().subject_table(), "operator");
        assert_eq!(dated("op-1", "2026-07-15").subject_table(), "operator");
        let back = RaisedAlert::acted_on(OTHER_EXPIRY.kind(), "m-1", Some("2026-07-15")).unwrap();
        assert_eq!(back.subject_table(), "machinery");
        assert_eq!(zone("plot-1").subject_table(), "plot");
        let shown = zone("plot-1").into_listed(AlertStatus::Active, None, "2026-06-11");
        assert_eq!(shown.subject_table, "plot");
        let record = UncheckedRecord::new(OTHER_EXPIRY.kind(), "m-1".into(), "x".into());
        assert_eq!(record.subject_table, "machinery");
    }

    #[test]
    fn a_standing_kind_sent_back_with_a_date_is_refused() {
        // Filed under a date, the act could never match the standing alert,
        // whose deadline is none — the farmer's tap would do nothing, silently.
        assert!(matches!(
            RaisedAlert::acted_on(NITRATE_ZONE.kind(), "plot-1", Some("2026-12-31")),
            Err(CoreError::Invalid("alert_deadline_mismatch"))
        ));
    }

    #[test]
    fn a_dated_kind_sent_back_without_its_date_is_refused() {
        // Without its date an act on a licence would hold for every renewal
        // after it — the 2026 dismissal silencing the 2031 expiry.
        assert!(matches!(
            RaisedAlert::acted_on(EXPIRY.kind(), "op-1", None),
            Err(CoreError::Invalid("alert_deadline_mismatch"))
        ));
    }

    #[test]
    fn a_malformed_date_sent_back_is_refused() {
        assert!(matches!(
            RaisedAlert::acted_on(EXPIRY.kind(), "op-1", Some("15/07/2026")),
            Err(CoreError::InvalidDate(_))
        ));
    }

    // --- where an alert stands (docs/sync.md → Alert acknowledgements roam) -----

    #[test]
    fn nobody_having_acted_leaves_an_alert_active() {
        assert_eq!(
            alert_status(&dated("op-1", "2026-07-15"), &[]),
            (AlertStatus::Active, None)
        );
    }

    #[test]
    fn an_acknowledgement_marks_the_alert_seen_from_when_it_was_made() {
        let alert = dated("op-1", "2026-07-15");
        let acts = [act(
            &alert,
            AlertStatus::Acknowledged,
            "2026-06-11T08:00:00Z",
        )];
        assert_eq!(
            alert_status(&alert, &acts),
            (
                AlertStatus::Acknowledged,
                Some("2026-06-11T08:00:00Z".to_string())
            )
        );
    }

    #[test]
    fn a_dismissal_beats_an_acknowledgement_whichever_came_first() {
        // Two devices acting on one alert without seeing each other: the
        // strongest act wins, exactly, with no clock involved.
        let alert = dated("op-1", "2026-07-15");
        let dismissed_first = [
            act(&alert, AlertStatus::Dismissed, "2026-06-11T08:00:00Z"),
            act(&alert, AlertStatus::Acknowledged, "2026-06-12T08:00:00Z"),
        ];
        let acknowledged_first = [
            act(&alert, AlertStatus::Acknowledged, "2026-06-11T08:00:00Z"),
            act(&alert, AlertStatus::Dismissed, "2026-06-12T08:00:00Z"),
        ];
        for acts in [dismissed_first, acknowledged_first] {
            assert_eq!(alert_status(&alert, &acts).0, AlertStatus::Dismissed);
        }
    }

    #[test]
    fn the_first_act_says_when_an_alert_was_first_seen() {
        let alert = dated("op-1", "2026-07-15");
        let acts = [
            act(&alert, AlertStatus::Acknowledged, "2026-06-12T08:00:00Z"),
            act(&alert, AlertStatus::Dismissed, "2026-06-11T09:30:00Z"),
        ];
        assert_eq!(
            alert_status(&alert, &acts).1,
            Some("2026-06-11T09:30:00Z".to_string())
        );
    }

    #[test]
    fn an_act_about_another_deadline_says_nothing_about_this_one() {
        // A licence dismissed in 2026 and renewed: when the renewed licence
        // nears ITS expiry, the old dismissal must not silence the new alert.
        let old = dated("op-1", "2026-07-15");
        let acts = [act(&old, AlertStatus::Dismissed, "2026-06-11T08:00:00Z")];
        assert_eq!(
            alert_status(&dated("op-1", "2031-07-15"), &acts),
            (AlertStatus::Active, None)
        );
    }

    #[test]
    fn an_act_about_another_subject_or_kind_says_nothing_about_this_one() {
        let alert = dated("op-1", "2026-07-15");
        let other_subject = dated("op-2", "2026-07-15");
        let other_kind = RaisedAlert::dated(OTHER_EXPIRY, "op-1".into(), "2026-07-15".into());
        // Synced from a build whose kind of this code was about machines: the
        // same code, subject and date, and still never about this operator.
        let mut other_table = act(&alert, AlertStatus::Dismissed, "2026-06-11T08:00:00Z");
        other_table.subject_table = "machinery".into();
        let acts = [
            act(
                &other_subject,
                AlertStatus::Dismissed,
                "2026-06-11T08:00:00Z",
            ),
            act(&other_kind, AlertStatus::Dismissed, "2026-06-11T08:00:00Z"),
            other_table,
        ];
        assert_eq!(alert_status(&alert, &acts), (AlertStatus::Active, None));
    }

    #[test]
    fn an_act_on_a_standing_alert_names_no_date_and_holds() {
        let alert = zone("plot-1");
        let acts = [act(&alert, AlertStatus::Dismissed, "2026-06-11T08:00:00Z")];
        assert_eq!(acts[0].due_date, None);
        assert_eq!(alert_status(&alert, &acts).0, AlertStatus::Dismissed);
    }

    #[test]
    fn the_status_order_is_the_precedence() {
        // `alert_status` takes the maximum, so the declaration order of the
        // enum IS the rule. Pinned here so a reordering cannot pass silently.
        assert!(AlertStatus::Active < AlertStatus::Acknowledged);
        assert!(AlertStatus::Acknowledged < AlertStatus::Dismissed);
    }

    #[test]
    fn a_status_reads_back_from_the_code_it_is_stored_as() {
        for status in [
            AlertStatus::Active,
            AlertStatus::Acknowledged,
            AlertStatus::Dismissed,
        ] {
            assert_eq!(AlertStatus::from_code(status.code()), Some(status));
            // The same spelling serde puts in the log payload, which is what a
            // receiving device writes into the column.
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::Value::from(status.code())
            );
        }
        assert_eq!(AlertStatus::from_code("snoozed"), None);
    }

    // --- one per condition, and the listing order -------------------------------

    #[test]
    fn a_condition_raised_twice_is_listed_once_keeping_the_soonest_deadline() {
        let kept = one_per_condition(vec![
            dated("op-1", "2026-09-01"),
            dated("op-1", "2026-07-15"),
            dated("op-1", "2026-08-01"),
        ]);
        assert_eq!(kept, vec![dated("op-1", "2026-07-15")]);
    }

    #[test]
    fn of_two_identical_raisings_the_first_is_kept() {
        let mut named = zone("plot-1");
        named.set_subject_label(Some("Los Alcores".into()));
        let kept = one_per_condition(vec![named.clone(), zone("plot-1")]);
        assert_eq!(kept, vec![named]);
    }

    #[test]
    fn distinct_conditions_are_all_kept_in_the_order_they_came() {
        let other_kind = RaisedAlert::dated(OTHER_EXPIRY, "op-1".into(), "2026-07-15".into());
        let raised = vec![
            dated("op-1", "2026-07-15"),
            dated("op-2", "2026-07-15"),
            other_kind,
            zone("plot-1"),
            RaisedAlert::standing(PHYTO_ZONE, "plot-1".into()),
        ];
        assert_eq!(one_per_condition(raised.clone()), raised);
    }

    #[test]
    fn dated_alerts_come_first_soonest_at_the_top_then_the_standing_ones() {
        let mut alerts = [
            listed(&zone("plot-1")),
            listed(&dated("op-1", "2026-09-01")),
            listed(&RaisedAlert::standing(PHYTO_ZONE, "plot-0".into())),
            listed(&dated("op-2", "2026-07-15")),
        ];
        alerts.sort_by(listing_order);
        let order: Vec<(&str, &str)> = alerts
            .iter()
            .map(|a| (a.alert_type_code, a.subject_id.as_str()))
            .collect();
        assert_eq!(
            order,
            [
                ("licence_expiry", "op-2"),
                ("licence_expiry", "op-1"),
                ("nitrate_zone", "plot-1"),
                ("phyto_zone", "plot-0"),
            ]
        );
    }

    #[test]
    fn a_tie_is_broken_by_kind_then_subject_never_by_arrival() {
        let expiry = listed(&dated("op-2", "2026-07-15"));
        let earlier_subject = listed(&dated("op-1", "2026-07-15"));
        let itv = listed(&RaisedAlert::dated(
            OTHER_EXPIRY,
            "m-1".into(),
            "2026-07-15".into(),
        ));
        for mut alerts in [
            vec![expiry.clone(), earlier_subject.clone(), itv.clone()],
            vec![itv.clone(), expiry.clone(), earlier_subject.clone()],
        ] {
            alerts.sort_by(listing_order);
            assert_eq!(
                alerts,
                vec![itv.clone(), earlier_subject.clone(), expiry.clone()]
            );
        }
    }

    #[test]
    fn an_alert_is_overdue_only_once_its_date_has_passed() {
        assert!(!is_overdue(Some("2026-08-15"), "2026-08-14"));
        assert!(
            !is_overdue(Some("2026-08-15"), "2026-08-15"),
            "the date itself is the last day inside"
        );
        assert!(is_overdue(Some("2026-08-15"), "2026-08-16"));
        // Across a month and a year boundary, where text order must still be
        // date order.
        assert!(is_overdue(Some("2026-09-30"), "2026-10-01"));
        assert!(is_overdue(Some("2026-12-31"), "2027-01-01"));
        assert!(!is_overdue(Some("2027-01-01"), "2026-12-31"));
    }

    #[test]
    fn a_standing_alert_is_never_overdue() {
        assert!(!is_overdue(None, "2099-12-31"));
        let listed = zone("plot-1").into_listed(AlertStatus::Active, None, "2099-12-31");
        assert!(!listed.overdue);
    }

    #[test]
    fn the_listed_alert_carries_whether_it_is_overdue() {
        let due = dated("op-1", "2026-08-15");
        assert!(
            !due.clone()
                .into_listed(AlertStatus::Active, None, "2026-08-15")
                .overdue
        );
        assert!(
            due.into_listed(AlertStatus::Active, None, "2026-08-16")
                .overdue
        );
    }

    // --- unchecked records -----------------------------------------------------------

    fn unchecked(code: DatedKind, subject: &str, label: Option<&str>) -> UncheckedRecord {
        let mut record = UncheckedRecord::new(code.kind(), subject.into(), "15/08/2026".into());
        record.subject_label = label.map(str::to_owned);
        record
    }

    #[test]
    fn an_unchecked_record_names_its_kind_and_keeps_the_value_as_stored() {
        let record = UncheckedRecord::new(EXPIRY.kind(), "op-1".into(), " 15/08/2026".into());
        assert_eq!(record.alert_type_code, "licence_expiry");
        assert_eq!(record.subject_table, "operator");
        assert_eq!(
            record.value, " 15/08/2026",
            "verbatim, so the farmer finds it"
        );
        assert_eq!(record.subject_label, None);
    }

    #[test]
    fn reports_absorb_each_other_keeping_both_halves() {
        let mut gathered = AlertReport {
            raised: vec![dated("op-1", "2026-07-15")],
            unchecked: vec![unchecked(EXPIRY, "op-2", None)],
        };
        gathered.absorb(AlertReport {
            raised: vec![zone("plot-1")],
            unchecked: vec![unchecked(OTHER_EXPIRY, "m-1", None)],
        });
        assert_eq!(gathered.raised.len(), 2);
        assert_eq!(gathered.unchecked.len(), 2);
        gathered.absorb(AlertReport::default());
        assert_eq!((gathered.raised.len(), gathered.unchecked.len()), (2, 2));
    }

    #[test]
    fn unchecked_records_list_by_kind_then_name_then_id() {
        let report = AlertReport {
            raised: vec![],
            unchecked: vec![
                unchecked(EXPIRY, "op-3", Some("Zoe")),
                unchecked(OTHER_EXPIRY, "m-1", Some("Atomizador")),
                unchecked(EXPIRY, "op-2", Some("Ana")),
                unchecked(EXPIRY, "op-1", Some("Ana")),
            ],
        };
        let listed = report.unchecked_in_listing_order();
        let order: Vec<&str> = listed.iter().map(|r| r.subject_id.as_str()).collect();
        // "itv_expiry" sorts before "licence_expiry".
        assert_eq!(order, ["m-1", "op-1", "op-2", "op-3"]);
    }

    // --- core's own kinds ----------------------------------------------------------

    #[test]
    fn zone_kinds_map_the_known_codes_and_ignore_unknown_ones() {
        assert_eq!(zone_alert_kind("nitrate_vulnerable"), Some(NITRATE_ZONE));
        assert_eq!(
            zone_alert_kind("phytosanitary_restriction"),
            Some(PHYTO_ZONE)
        );
        assert_eq!(zone_alert_kind("natura_2000"), Some(NATURA_ZONE));
        // Forward compatibility: an unmapped zone type raises nothing.
        assert_eq!(zone_alert_kind("fr_some_future_zone"), None);
    }

    #[test]
    fn every_zone_type_core_seeds_raises_an_alert_listed_in_its_kinds() {
        // The three codes core's 0002 seeds into `zone_type`; a fourth added
        // there without a mapping here would be a zone nobody is warned about.
        for zone_type in [
            "nitrate_vulnerable",
            "phytosanitary_restriction",
            "natura_2000",
        ] {
            let kind = zone_alert_kind(zone_type).expect("a seeded zone kind maps");
            assert!(
                ALERT_KINDS.contains(&kind.kind()),
                "{zone_type} raises {}, which core's ALERT_KINDS does not list",
                kind.kind().code()
            );
        }
    }

    #[test]
    fn core_declares_each_of_its_kinds_once() {
        let codes: HashSet<&str> = ALERT_KINDS.iter().map(|kind| kind.code()).collect();
        assert_eq!(codes.len(), ALERT_KINDS.len());
    }

    #[test]
    fn zone_alerts_are_active_only_when_inside() {
        assert!(zone_alert_is_active("inside"));
        assert!(!zone_alert_is_active("outside"));
    }
}
