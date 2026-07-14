// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert reconciliation and acknowledgement state.
//!
//! `refresh_alerts` is the single writer of alert rows. Each run derives the candidate
//! set from the source tables (treatments, operators, machinery) with the pure rules in
//! [`crate::alerts`], then reconciles the `alert` table against it:
//!   * missing candidates are inserted as 'active';
//!   * existing rows get drifted `due_date` / `lead_days_used` corrected, but their
//!     `status` is NEVER touched — a dismissal cannot be resurrected by a refresh;
//!   * rows whose condition no longer holds are deleted (lapsed, renewed, or the
//!     subject was soft-deleted).
//!
//! The function is idempotent, so callers may over-call it freely (app start, after
//! writes, day rollover). Alert rows are derived state: they are deliberately NOT
//! logged to `record_change` and never sync — each device re-derives its own.

use std::collections::HashMap;

use super::no_rows_to_not_found;
use crate::alerts::{
    AlertConfig, expiry_alert_is_active, phi_window_is_active, zone_alert_is_active,
    zone_alert_type,
};
use crate::date::now_utc_iso;
use crate::error::Result;
use crate::models::Alert;
use rusqlite::{Connection, Transaction, params};
use uuid::Uuid;

/// One condition that should currently have an alert row.
struct Candidate {
    alert_type_code: &'static str,
    season_id: Option<String>,
    subject_table: &'static str,
    subject_id: String,
    due_date: String,
    lead_days_used: Option<i64>,
}

/// Recompute every alert condition as of `today` (a `YYYY-MM-DD` date) and reconcile
/// the `alert` table against the result. See the module docs for the exact semantics.
pub fn refresh_alerts(conn: &mut Connection, today: &str, config: &AlertConfig) -> Result<()> {
    let tx = conn.transaction()?;
    let candidates = collect_candidates(&tx, today, config)?;
    reconcile(&tx, candidates)?;
    tx.commit()?;
    Ok(())
}

/// Alerts the UI should surface: 'active' and 'acknowledged' (still visible, subdued),
/// soonest due date first. 'dismissed' rows are hidden; refresh removes them for good
/// once their condition lapses.
pub fn list_active_alerts(conn: &Connection) -> Result<Vec<Alert>> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type_code, season_id, subject_table, subject_id, due_date,
                lead_days_used, status, acknowledged_at, created_at, updated_at
         FROM alert
         WHERE status <> 'dismissed'
         ORDER BY due_date, alert_type_code, subject_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Alert {
            id: r.get(0)?,
            alert_type_code: r.get(1)?,
            season_id: r.get(2)?,
            subject_table: r.get(3)?,
            subject_id: r.get(4)?,
            due_date: r.get(5)?,
            lead_days_used: r.get(6)?,
            status: r.get(7)?,
            acknowledged_at: r.get(8)?,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Mark an alert as seen. It stays in `list_active_alerts` (subdued in the UI).
pub fn acknowledge_alert(conn: &mut Connection, alert_id: &str) -> Result<()> {
    let now = now_utc_iso();
    set_status(conn, alert_id, "acknowledged", Some(&now), &now)
}

/// Hide an alert while its condition holds; refresh deletes it once the condition lapses.
pub fn dismiss_alert(conn: &mut Connection, alert_id: &str) -> Result<()> {
    let now = now_utc_iso();
    set_status(conn, alert_id, "dismissed", None, &now)
}

fn set_status(
    conn: &Connection,
    alert_id: &str,
    status: &str,
    acknowledged_at: Option<&str>,
    now: &str,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE alert
         SET status = ?1, acknowledged_at = coalesce(?2, acknowledged_at), updated_at = ?3
         WHERE id = ?4",
        params![status, acknowledged_at, now, alert_id],
    )?;
    if changed == 0 {
        return Err(no_rows_to_not_found(rusqlite::Error::QueryReturnedNoRows));
    }
    Ok(())
}

fn collect_candidates(
    tx: &Transaction,
    today: &str,
    config: &AlertConfig,
) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();

    // PHI windows: one alert per live treatment record (multi-plot treatments are a
    // single record, hence a single alert).
    let mut stmt = tx.prepare(
        "SELECT id, season_id, application_date, phi_end_date
         FROM treatment_record WHERE deleted_at IS NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (id, season_id, application_date, phi_end_date) = row?;
        if phi_window_is_active(&application_date, &phi_end_date, today)? {
            out.push(Candidate {
                alert_type_code: "phi_window",
                season_id: Some(season_id),
                subject_table: "treatment_record",
                subject_id: id,
                due_date: phi_end_date,
                lead_days_used: None,
            });
        }
    }

    // Subjects with no date on file produce no alert (nothing to derive); expiry alerts
    // are not season-bound, so season_id stays NULL.
    expiry_candidates(
        tx,
        &mut out,
        "SELECT id, licence_expiry_date FROM operator
         WHERE deleted_at IS NULL AND licence_expiry_date IS NOT NULL",
        "licence_expiry",
        "operator",
        config.licence_lead_days,
        today,
    )?;
    expiry_candidates(
        tx,
        &mut out,
        "SELECT id, next_inspection_due_date FROM machinery
         WHERE deleted_at IS NULL AND next_inspection_due_date IS NOT NULL",
        "itv_expiry",
        "machinery",
        config.itv_lead_days,
        today,
    )?;

    zone_candidates(tx, &mut out)?;

    Ok(out)
}

/// Zone flags (core's `plot_zone_flag`, filled by provider modules): one
/// alert per (plot, zone kind) whose LATEST campaign check says 'inside' — a
/// standing condition, so the subject is the plot (a dismissal survives
/// re-checks and rollovers) and the due date is the campaign's year end
/// (drift-corrected by reconcile at rollover). Older campaigns are history,
/// never alert sources.
fn zone_candidates(tx: &Transaction, out: &mut Vec<Candidate>) -> Result<()> {
    let mut stmt = tx.prepare(
        "SELECT f.plot_id, f.zone_type_code, f.status, f.campaign
         FROM plot_zone_flag f
         JOIN plot p ON p.id = f.plot_id AND p.deleted_at IS NULL
         WHERE f.deleted_at IS NULL
           AND f.campaign = (SELECT MAX(f2.campaign) FROM plot_zone_flag f2
                             WHERE f2.deleted_at IS NULL
                               AND f2.plot_id = f.plot_id
                               AND f2.zone_type_code = f.zone_type_code)",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    // Multiple sources could one day flag the same (plot, kind) in the same
    // campaign; alert identity is (type, plot), so emit each key once.
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let (plot_id, zone_type_code, status, campaign) = row?;
        let Some(alert_type_code) = zone_alert_type(&zone_type_code) else {
            continue;
        };
        if zone_alert_is_active(&status) && seen.insert((alert_type_code, plot_id.clone())) {
            out.push(Candidate {
                alert_type_code,
                season_id: None,
                subject_table: "plot",
                subject_id: plot_id,
                due_date: format!("{campaign}-12-31"),
                lead_days_used: None,
            });
        }
    }
    Ok(())
}

fn expiry_candidates(
    tx: &Transaction,
    out: &mut Vec<Candidate>,
    sql: &str,
    alert_type_code: &'static str,
    subject_table: &'static str,
    lead_days: i64,
    today: &str,
) -> Result<()> {
    let mut stmt = tx.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, expiry_date) = row?;
        if expiry_alert_is_active(&expiry_date, today, lead_days)? {
            out.push(Candidate {
                alert_type_code,
                season_id: None,
                subject_table,
                subject_id: id,
                due_date: expiry_date,
                lead_days_used: Some(lead_days),
            });
        }
    }
    Ok(())
}

fn reconcile(tx: &Transaction, candidates: Vec<Candidate>) -> Result<()> {
    // Existing rows keyed by the condition identity (the table's UNIQUE constraint).
    type ConditionKey = (String, String, String);
    let mut existing: HashMap<ConditionKey, (String, Option<String>, Option<i64>)> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT alert_type_code, subject_table, subject_id, id, due_date, lead_days_used
             FROM alert",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                (r.get(0)?, r.get(1)?, r.get(2)?),
                (r.get(3)?, r.get(4)?, r.get(5)?),
            ))
        })?;
        for row in rows {
            let (key, value) = row?;
            existing.insert(key, value);
        }
    }

    let now = now_utc_iso();
    for c in candidates {
        let key = (
            c.alert_type_code.to_string(),
            c.subject_table.to_string(),
            c.subject_id.clone(),
        );
        match existing.remove(&key) {
            // Already alerted: correct drifted derived fields, leave status alone.
            Some((alert_id, due_date, lead_days)) => {
                if due_date.as_deref() != Some(&c.due_date) || lead_days != c.lead_days_used {
                    tx.execute(
                        "UPDATE alert SET due_date = ?1, lead_days_used = ?2, updated_at = ?3
                         WHERE id = ?4",
                        params![c.due_date, c.lead_days_used, now, alert_id],
                    )?;
                }
            }
            None => {
                tx.execute(
                    "INSERT INTO alert
                       (id, alert_type_code, season_id, subject_table, subject_id,
                        due_date, lead_days_used, status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?8)",
                    params![
                        Uuid::now_v7().to_string(),
                        c.alert_type_code,
                        c.season_id,
                        c.subject_table,
                        c.subject_id,
                        c.due_date,
                        c.lead_days_used,
                        now,
                    ],
                )?;
            }
        }
    }

    // Whatever was not re-derived has no live condition any more: lapsed window,
    // renewed licence/ITV, or soft-deleted subject. Derived + non-regulatory → delete.
    for (alert_id, _, _) in existing.into_values() {
        tx.execute("DELETE FROM alert WHERE id = ?1", [alert_id])?;
    }
    Ok(())
}
