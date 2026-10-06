// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The alerts this module raises, worked out from the registers when the list
//! is read.
//!
//! **Nothing is stored and nothing is written.** [`current_alerts`] applies the
//! rules in [`crate::alerts`] to the treatments, operators and machines as they
//! stand and returns the conditions holding on `today`, in core's standard
//! shape. Core assembles the list from them and from what people did about
//! them (`terrazgo_core::repository::list_alerts`) — so a lapsed window, a
//! renewed licence or a deleted machine simply stops being raised, and there is
//! no copy for a forgotten refresh to leave wrong (docs/data-model.md → "Alerts:
//! the settled design").

use std::collections::{BTreeSet, HashMap};

use crate::alerts::{
    AlertConfig, ITV_EXPIRY, LICENCE_EXPIRY, PHI_WINDOW, expiry_alert_is_active,
    phi_window_is_active,
};
use crate::error::{PhytosanitaryError, Result};
use rusqlite::Connection;
use terrazgo_core::alerts::{AlertReport, DatedKind, RaisedAlert, UncheckedRecord};
use terrazgo_core::sql::children_by_parent;

/// The alerts this module raises that hold on `today` (a `YYYY-MM-DD` date),
/// each named as a farmer would know its subject: the treated plots, the
/// person, the machine.
///
/// A fixed handful of statements whatever the size of the holding — the three
/// conditions, then one name lookup per subject kind present.
///
/// **A date that cannot be read is reported, never skipped and never fatal.**
/// Compliance logic must not silently drop a record, so the record comes back
/// as unchecked — named, with the value as stored — for the Status view to tell
/// the farmer what to correct; and one bad row must not hide the others, so
/// every other record is still worked out. Anything else that goes wrong (the
/// database itself) still fails the call, and the shell names this module's
/// alerts as unavailable while listing everyone else's.
pub fn current_alerts(conn: &Connection, today: &str, config: &AlertConfig) -> Result<AlertReport> {
    let mut report = AlertReport::default();
    phi_windows(conn, today, &mut report)?;
    expiries(
        conn,
        "SELECT id, licence_expiry_date FROM operator
         WHERE deleted_at IS NULL AND licence_expiry_date IS NOT NULL",
        LICENCE_EXPIRY,
        config.licence_lead_days,
        today,
        &mut report,
    )?;
    expiries(
        conn,
        "SELECT id, next_inspection_due_date FROM machinery
         WHERE deleted_at IS NULL AND next_inspection_due_date IS NOT NULL",
        ITV_EXPIRY,
        config.itv_lead_days,
        today,
        &mut report,
    )?;
    name_subjects(conn, &mut report)?;
    Ok(report)
}

/// Where one record stands after its rule ran: an alert raised or not, or a
/// date that could not be read — which becomes an unchecked record rather than
/// an error. Any other error is the database's, and is passed up.
fn record_outcome(
    outcome: Result<bool>,
    report: &mut AlertReport,
    raise: impl FnOnce() -> RaisedAlert,
    unchecked: impl FnOnce(String) -> UncheckedRecord,
) -> Result<()> {
    match outcome {
        Ok(true) => report.raised.push(raise()),
        Ok(false) => {}
        Err(PhytosanitaryError::InvalidDate(value)) => report.unchecked.push(unchecked(value)),
        Err(other) => return Err(other),
    }
    Ok(())
}

/// PHI windows: one alert per live treatment record — a treatment over several
/// plots is one record, so one alert — due on its `phi_end_date`.
///
/// A record with no product has no plazo de seguridad, so there is no window
/// to open — filtered in SQL because that is where the rule belongs, and
/// because reading a NULL into a `String` would fail the whole call.
///
/// `phi_end_date >= today` is a candidate BOUND, not the window rule: a window
/// is `[application_date, phi_end_date)`, so one that has ended cannot contain
/// today and there is nothing to derive from it. Every row it excludes is one
/// [`phi_window_is_active`] would have rejected a moment later, which is what
/// makes it safe to narrow here — the rule itself stays in one place. Without
/// the bound this reads every treatment the holding has ever recorded, each
/// time the list is read; `idx_treatment_record_phi` serves it.
fn phi_windows(conn: &Connection, today: &str, report: &mut AlertReport) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, application_date, phi_end_date
         FROM treatment_record
         WHERE deleted_at IS NULL AND phi_end_date IS NOT NULL AND phi_end_date >= ?1",
    )?;
    let rows = stmt.query_map([today], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (id, application_date, phi_end_date) = row?;
        let outcome = phi_window_is_active(&application_date, &phi_end_date, today);
        record_outcome(
            outcome,
            report,
            || RaisedAlert::dated(PHI_WINDOW, id.clone(), phi_end_date),
            |value| UncheckedRecord::new(PHI_WINDOW.kind(), id.clone(), value),
        )?;
    }
    Ok(())
}

/// Expiry alerts (a carné, an ITV): active from `lead_days` before the date and
/// for as long as it stays unrenewed. A subject with no date on file raises
/// nothing — there is nothing to derive.
fn expiries(
    conn: &Connection,
    sql: &str,
    kind: DatedKind,
    lead_days: i64,
    today: &str,
    report: &mut AlertReport,
) -> Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, expiry_date) = row?;
        let outcome = expiry_alert_is_active(&expiry_date, today, lead_days);
        record_outcome(
            outcome,
            report,
            || RaisedAlert::dated(kind, id.clone(), expiry_date),
            |value| UncheckedRecord::new(kind.kind(), id.clone(), value),
        )?;
    }
    Ok(())
}

/// Name what each alert — and each record that could not be checked — is
/// about: the plots, the person or the machine the farmer would recognise.
/// `subject_table` alone can only say "an operator", and which one is the
/// question a farmer reading an alert actually has; for an unchecked record it
/// is also the one to open and correct.
///
/// **One statement per subject KIND, never one per alert** — three at the very
/// most, and only for the kinds present, since `children_by_parent` runs nothing
/// for an empty id list.
fn name_subjects(conn: &Connection, report: &mut AlertReport) -> Result<()> {
    let ids_of = |table: &str| -> Vec<String> {
        let raised = report
            .raised
            .iter()
            .map(|a| (a.subject_table(), a.subject_id()));
        let unchecked = report
            .unchecked
            .iter()
            .map(|u| (u.subject_table, u.subject_id.as_str()));
        raised
            .chain(unchecked)
            .filter(|(subject_table, _)| *subject_table == table)
            .map(|(_, id)| id.to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let operators = names(
        conn,
        "SELECT id, full_name FROM operator WHERE id IN ({ids})",
        &ids_of("operator"),
    )?;
    let machines = names(
        conn,
        "SELECT id, name FROM machinery WHERE id IN ({ids})",
        &ids_of("machinery"),
    )?;
    let treatments = treated_plots(conn, &ids_of("treatment_record"))?;

    let name_of = |subject_table: &str, id: &str| -> Option<String> {
        match subject_table {
            "operator" => operators.get(id),
            "machinery" => machines.get(id),
            "treatment_record" => treatments.get(id),
            _ => None,
        }
        .cloned()
    };
    // The closure's borrow of `report` ends at its last call above, which is
    // what lets the same report be walked mutably here.
    for alert in report.raised.iter_mut() {
        let name = name_of(alert.subject_table(), alert.subject_id());
        alert.set_subject_label(name);
    }
    for record in report.unchecked.iter_mut() {
        record.subject_label = name_of(record.subject_table, &record.subject_id);
    }
    Ok(())
}

/// One name per id, from a caller-written query selecting `(id, name)`.
///
/// The SQL stays at the call site the way `children_by_parent` requires: a
/// helper that assembled it from a table name would be choosing the column a
/// register displays, which is the caller's decision and not plumbing's.
fn names(conn: &Connection, sql: &str, ids: &[String]) -> Result<HashMap<String, String>> {
    let grouped = children_by_parent(
        conn,
        sql,
        ids,
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        |(id, _)| id.clone(),
    )?;
    Ok(grouped
        .into_iter()
        .filter_map(|(id, rows)| rows.into_iter().next().map(|(_, name)| (id, name)))
        .collect())
}

/// The plots one treatment covered, as a single line — a PHI window is about the
/// ground that cannot be harvested yet, and a treatment may cover several parcels.
///
/// Capped at [`PLOTS_NAMED`], because a holding that sprays forty parcels in one
/// pass would otherwise put forty names on a card. The comma is a separator
/// rather than a translated string; if this ever needs a language's own list
/// phrasing ("La Vega y El Soto"), the names travel to the view and it does the
/// joining.
fn treated_plots(conn: &Connection, ids: &[String]) -> Result<HashMap<String, String>> {
    let grouped = children_by_parent(
        conn,
        "SELECT tp.treatment_record_id, p.name
         FROM treatment_plot tp
         JOIN plot p ON p.id = tp.plot_id
         WHERE tp.treatment_record_id IN ({ids})
         ORDER BY tp.treatment_record_id, p.name",
        ids,
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        |(parent, _)| parent.clone(),
    )?;
    Ok(grouped
        .into_iter()
        .map(|(id, rows)| {
            let mut names: Vec<String> = rows.into_iter().map(|(_, name)| name).collect();
            let over = names.len() > PLOTS_NAMED;
            names.truncate(PLOTS_NAMED);
            let joined = names.join(", ");
            (
                id,
                if over {
                    format!("{joined}, …")
                } else {
                    joined
                },
            )
        })
        .collect())
}

/// How many treated plots a PHI alert names before it trails off.
const PLOTS_NAMED: usize = 3;
