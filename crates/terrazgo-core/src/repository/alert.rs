// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alerts: core's own, the list the Status view shows, and what people did
//! about them.
//!
//! **Nothing here stores an alert.** Each crate that raises alerts returns the
//! ones holding now — [`current_alerts`] is core's — and [`list_alerts`]
//! assembles the list from them whenever it is read. The one stored part is
//! `alert_acknowledgement`: what a person said, one row per act, written by
//! [`acknowledge_alert`] and [`dismiss_alert`] and synced, since no device can
//! re-derive it. A listed alert's status is read through those acts
//! ([`crate::alerts::alert_status`]) and stored nowhere.

use std::collections::BTreeSet;

use crate::alerts::{
    Alert, AlertKind, AlertReport, AlertStatus, RaisedAlert, alert_status, listing_order,
    one_per_condition, zone_alert_is_active, zone_alert_kind,
};
use crate::audit::{begin, log_insert};
use crate::date::now_utc_iso;
use crate::error::Result;
use crate::models::AlertAcknowledgement;
use crate::sql::children_by_parent;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

/// Every act on the subjects the listed alerts are about, whatever kind or
/// deadline each act names — [`alert_status`] picks out the ones that apply.
///
/// **Keyed on the subject, and made to be.** Acts accumulate for as long as the
/// farmer uses the app, and nearly all of them are about deadlines long gone,
/// so the read must seek from the alerts holding now into this table and never
/// scan it. `idx_alert_acknowledgement_subject` leads with `subject_id`, so each
/// id in the list is one seek, and what is read is bounded by the acts on
/// today's subjects rather than by every act ever made. The in-crate test below
/// counts full-scan steps to hold it there.
const SUBJECT_ACTS_SQL: &str =
    "SELECT id, alert_type_code, subject_table, subject_id, due_date, status, created_at
     FROM alert_acknowledgement
     WHERE subject_id IN ({ids})
     ORDER BY subject_id, id";

/// Every act on one condition, whatever deadline each was about.
const CONDITION_ACTS_SQL: &str =
    "SELECT id, alert_type_code, subject_table, subject_id, due_date, status, created_at
     FROM alert_acknowledgement
     WHERE subject_id = ?1 AND alert_type_code = ?2 AND subject_table = ?3";

/// Read a status out of `alert_acknowledgement.status`. The column's CHECK
/// allows only the two codes an act can carry, so an unknown one is a damaged
/// row, and saying so beats reading it as "nobody acted".
impl FromSql for AlertStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let code = value.as_str()?;
        AlertStatus::from_code(code)
            .ok_or_else(|| FromSqlError::Other(format!("unknown alert status {code:?}").into()))
    }
}

fn map_act(row: &Row<'_>) -> rusqlite::Result<AlertAcknowledgement> {
    Ok(AlertAcknowledgement {
        id: row.get("id")?,
        alert_type_code: row.get("alert_type_code")?,
        subject_table: row.get("subject_table")?,
        subject_id: row.get("subject_id")?,
        due_date: row.get("due_date")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
    })
}

/// Core's own alerts holding now: one per (plot, zone kind) whose latest check
/// says the plot is inside, named after the plot.
///
/// "Latest per (plot, zone kind)" is [`super::list_latest_zone_flags`]' rule,
/// which the plot cards read too, so a chip and an alert on the same plot
/// cannot disagree. A deleted plot's flags are not in it, so its alerts go the
/// moment it is deleted. A standing condition, so the subject is the plot: a
/// dismissal survives re-checks and new campaigns.
///
/// Two statements whatever the number of plots: the flags, and the names.
///
/// Never an unchecked record: the one value the rule reads, a flag's status,
/// can only be `inside` or `outside` — the table's CHECK says so.
pub fn current_alerts(conn: &Connection) -> Result<AlertReport> {
    let mut raised = Vec::new();
    for flag in super::list_latest_zone_flags(conn)? {
        let Some(kind) = zone_alert_kind(&flag.zone_type_code) else {
            continue;
        };
        if zone_alert_is_active(&flag.status) {
            raised.push(RaisedAlert::standing(kind, flag.plot_id));
        }
    }
    name_plots(conn, &mut raised)?;
    Ok(AlertReport {
        raised,
        unchecked: Vec::new(),
    })
}

/// Name each alert after its plot — what a farmer reading it actually asks.
fn name_plots(conn: &Connection, alerts: &mut [RaisedAlert]) -> Result<()> {
    let ids: Vec<String> = alerts
        .iter()
        .map(|alert| alert.subject_id().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let names = children_by_parent(
        conn,
        "SELECT id, name FROM plot WHERE id IN ({ids})",
        &ids,
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        |(id, _)| id.clone(),
    )?;
    for alert in alerts.iter_mut() {
        let name = names
            .get(alert.subject_id())
            .and_then(|rows| rows.first())
            .map(|(_, name)| name.clone());
        alert.set_subject_label(name);
    }
    Ok(())
}

/// The list the Status view shows on `today` (`YYYY-MM-DD`): every alert
/// `raised` — by core and by each crate that raises alerts — with where it
/// stands and whether its date has passed, the dismissed ones left out, dated
/// alerts first by deadline and standing ones after
/// ([`crate::alerts::listing_order`]).
///
/// A condition raised twice is listed once ([`crate::alerts::one_per_condition`]).
///
/// One statement for the acts whatever the number of alerts (one per 500
/// subjects), and none at all when nothing is raised.
///
/// **The dismissed ones are dropped here rather than in the SQL**, because the
/// rule that decides it is [`alert_status`]: which acts apply turns on the
/// alert's current deadline, which only the crate that raised it knows.
pub fn list_alerts(conn: &Connection, raised: Vec<RaisedAlert>, today: &str) -> Result<Vec<Alert>> {
    let raised = one_per_condition(raised);
    let subjects: Vec<String> = raised
        .iter()
        .map(|alert| alert.subject_id().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let acts = children_by_parent(conn, SUBJECT_ACTS_SQL, &subjects, map_act, |act| {
        act.subject_id.clone()
    })?;

    let mut alerts = Vec::with_capacity(raised.len());
    for alert in raised {
        let on_subject = acts
            .get(alert.subject_id())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let (status, first_seen) = alert_status(&alert, on_subject);
        if status == AlertStatus::Dismissed {
            continue;
        }
        alerts.push(alert.into_listed(status, first_seen, today));
    }
    alerts.sort_by(listing_order);
    Ok(alerts)
}

/// Mark an alert as seen. It stays listed (subdued on screen), on every device
/// once they have synced.
///
/// The alert is named the way the screen showed it: its kind (which the shell
/// looks up by code among the kinds the alert crates declare, and which names
/// the subject's table), its subject, and the deadline on the card — `None` for
/// a standing alert, and anything else for one is refused, as is a dated alert
/// without its date. It is **not** re-checked against the alerts holding now:
/// if the condition changed in the meantime, the act names a deadline no listed
/// alert has and affects nothing, which is true to what the person saw.
pub fn acknowledge_alert(
    conn: &mut Connection,
    kind: AlertKind,
    subject_id: &str,
    due_date: Option<&str>,
    actor: Option<&str>,
) -> Result<()> {
    record_act(
        conn,
        RaisedAlert::acted_on(kind, subject_id, due_date)?,
        AlertStatus::Acknowledged,
        actor,
    )
}

/// Hide an alert while its deadline holds, on every device. It stays hidden if
/// the same deadline comes back; a new one — a renewed licence nearing its next
/// expiry, a corrected plazo — is a new alert. A standing alert stays hidden
/// whenever its condition holds, a plot entering a zone again included. Named
/// as for [`acknowledge_alert`].
pub fn dismiss_alert(
    conn: &mut Connection,
    kind: AlertKind,
    subject_id: &str,
    due_date: Option<&str>,
    actor: Option<&str>,
) -> Result<()> {
    record_act(
        conn,
        RaisedAlert::acted_on(kind, subject_id, due_date)?,
        AlertStatus::Dismissed,
        actor,
    )
}

/// Record one act on an alert, as an audited write of its own.
///
/// **An act that would change nothing is not written.** Acknowledging what is
/// already acknowledged or dismissed, or dismissing what is dismissed, is a
/// double tap or a screen that had not reloaded, and a row for it would only be
/// carried to every device and kept there. The transaction is then dropped
/// without a commit, which rolls it back, and `begin` consumed no change-set
/// number. Two devices that both act before syncing still write two rows —
/// neither could know — and [`alert_status`] takes the stronger.
fn record_act(
    conn: &mut Connection,
    alert: RaisedAlert,
    status: AlertStatus,
    actor: Option<&str>,
) -> Result<()> {
    let tx = begin(conn, actor)?;
    let acts = tx
        .prepare(CONDITION_ACTS_SQL)?
        .query_map(
            params![
                alert.subject_id(),
                alert.kind().code(),
                alert.subject_table()
            ],
            map_act,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (current, _) = alert_status(&alert, &acts);
    if current >= status {
        return Ok(());
    }

    let act = AlertAcknowledgement {
        id: Uuid::now_v7().to_string(),
        alert_type_code: alert.kind().code().to_owned(),
        subject_table: alert.subject_table().to_owned(),
        subject_id: alert.subject_id().to_owned(),
        due_date: alert.due_date().map(str::to_owned),
        status,
        created_at: now_utc_iso(),
    };
    tx.execute(
        "INSERT INTO alert_acknowledgement
           (id, alert_type_code, subject_table, subject_id, due_date, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            act.id,
            act.alert_type_code,
            act.subject_table,
            act.subject_id,
            act.due_date,
            act.status.code(),
            act.created_at,
        ],
    )?;
    // A register of its own, so no act is ever a second version of another. No
    // season: an act is about a condition, and a licence has none.
    let stamp = tx.register("alert_acknowledgement", &act.id, None)?;
    log_insert(&tx, &stamp, "alert_acknowledgement", &act.id, &act)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::StatementStatus;

    /// `live` subjects with one act each, plus `gone` acts about subjects no
    /// alert names any more — the history a holding accumulates. Written
    /// straight into the table: what is measured is a read, and the rows need
    /// no log for it.
    fn acts_with_history(live: usize, gone: usize) -> Connection {
        let conn = crate::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {live})
             INSERT INTO alert_acknowledgement
               (id, alert_type_code, subject_table, subject_id, due_date, status, created_at)
             SELECT printf('seen-%d', i), 'phi_window', 'treatment_record',
                    printf('live-%d', i), '2026-07-01', 'acknowledged', '2026-06-11T09:00:00Z'
             FROM n;"
        ))
        .unwrap();
        if gone > 0 {
            conn.execute_batch(&format!(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {gone})
                 INSERT INTO alert_acknowledgement
                   (id, alert_type_code, subject_table, subject_id, due_date, status, created_at)
                 SELECT printf('old-%d', i), 'phi_window', 'treatment_record',
                        printf('gone-%d', i), '2024-07-01', 'dismissed', '2024-06-11T09:00:00Z'
                 FROM n;"
            ))
            .unwrap();
        }
        conn
    }

    /// The subject-acts query for `live` subjects, with its `{ids}` spelled out
    /// the way `children_by_parent` spells them.
    fn subject_acts_for(live: usize) -> (String, Vec<String>) {
        let ids: Vec<String> = (1..=live).map(|i| format!("live-{i}")).collect();
        let placeholders: Vec<String> = (1..=live).map(|n| format!("?{n}")).collect();
        (
            SUBJECT_ACTS_SQL.replace("{ids}", &placeholders.join(", ")),
            ids,
        )
    }

    /// Run a query to the end and report how many rows SQLite stepped through in
    /// full table scans to answer it — the instrument `crate::audit`'s tests use
    /// for the log.
    fn full_scan_steps(conn: &Connection, sql: &str, ids: &[String]) -> i32 {
        let mut stmt = conn.prepare(sql).unwrap();
        let mut rows = stmt.query(rusqlite::params_from_iter(ids)).unwrap();
        let mut read = 0;
        while rows.next().unwrap().is_some() {
            read += 1;
        }
        assert_eq!(read, ids.len(), "one act per live subject comes back");
        drop(rows);
        stmt.get_status(StatementStatus::FullscanStep)
    }

    #[test]
    fn listing_reads_the_acts_on_todays_subjects_and_never_the_history() {
        // Acts are never pruned, so this table grows for as long as the farmer
        // uses the app while the alerts holding today stay few. A read that
        // scanned the table would give the same answer and cost the whole
        // history on every listing — which only a count of what was scanned
        // can see.
        let years = acts_with_history(20, 5000);

        // The control: the instrument does see a scan of this table.
        {
            let mut stmt = years
                .prepare("SELECT MAX(created_at) FROM alert_acknowledgement")
                .unwrap();
            stmt.query_row([], |_| Ok(())).unwrap();
            assert!(
                stmt.get_status(StatementStatus::FullscanStep) > 1000,
                "the counter must be able to fail"
            );
        }

        let (sql, ids) = subject_acts_for(20);
        assert_eq!(
            full_scan_steps(&years, &sql, &ids),
            0,
            "listing scanned acts instead of seeking the subjects it was asked about"
        );
    }
}
