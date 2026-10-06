// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How big a sync actually is, against real treatment records at real scale.
//!
//! docs/sync.md carried a table of provisional arithmetic and said it should be
//! **replaced by a measurement when the transport is built**. This is that
//! measurement, and it lives in this crate rather than in core because this is
//! where the scaled-data builder is: a treatment record with its treated plots,
//! problems and justifications is the widest register in the schema, so it is
//! the one worth sizing.
//!
//! ```
//! cargo test -p module-phytosanitary --release --test bundle_size -- --ignored --nocapture --test-threads=1
//! ```
//!
//! One thread, or the two measurements' tables print into each other.
//!
//! Ignored like the other measurements here: an answer on demand, not an
//! assertion. What it prints is the markdown tables the doc records — the size
//! of a sync, and what acting on alerts adds to it and to the database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::scale::{Scale, scaled_farm};
use common::treatment::{add_es_authorisation, base_fixture, sample_treatment};
use module_phytosanitary::alerts::PHI_WINDOW;
use module_phytosanitary::models::NewTreatmentPlot;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;
use terrazgo_core::audit;
use terrazgo_core::bundle;
use terrazgo_core::models::NewPlot;
use terrazgo_core::sync::VersionVector;

/// A bundle of everything `seen` has not got, and what it cost to say it.
fn bundle_of(conn: &Connection, seen: &VersionVector) -> (usize, usize, usize) {
    let mut bytes = Vec::new();
    let device = terrazgo_core::sync::installed_device(conn).unwrap();
    let summary = bundle::write_bundle(conn, &device, seen, &mut bytes).unwrap();
    (summary.change_sets, summary.rows, bytes.len())
}

fn kib(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn row(label: &str, sets: usize, rows: usize, gzipped: usize) {
    println!(
        "| {label} | {sets} | {rows} | {} | {} |",
        kib(gzipped),
        kib(gzipped / sets.max(1)),
    );
}

/// A whole farm's log, which is what a device that has never synced — or has
/// been away long enough — hands over.
fn whole_log(label: &str, scale: &Scale) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    scaled_farm(&mut conn, scale);
    let (sets, rows, gzipped) = bundle_of(&conn, &VersionVector::default());
    row(label, sets, rows, gzipped);
}

/// A day's work on a holding that already exists — the everyday exchange, and
/// the one the design's table was really about.
///
/// The size of what is already there does not enter into it: the export selects
/// by `origin_seq` past what the peer has seen, so a delta costs what the delta
/// is. Measured on a season-sized holding anyway, so the claim is shown rather
/// than asserted.
fn a_days_work(label: &str, records: usize) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    let established = Scale {
        seasons: 1,
        plots: 40,
        records_per_season: 400,
        plots_per_record: 2,
    };
    scaled_farm(&mut conn, &established);

    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "La Vega".into(),
            area_ha: Some(2.5),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    // What the peer had when it last synced. Taken AFTER the holding and this
    // operator's own setup, so what the delta measures is the day's records and
    // nothing else.
    let seen = bundle::seen_by(&conn).unwrap();
    for _ in 0..records {
        let mut new = sample_treatment(&fx, None, Some(21));
        new.season_id = fx.season_id.clone();
        let plots = vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }];
        repo::insert_treatment_record(&mut conn, new, plots, None).unwrap();
    }

    let (sets, rows, gzipped) = bundle_of(&conn, &seen);
    row(label, sets, rows, gzipped);
}

#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn how_big_is_a_sync() {
    println!();
    println!("| Exchange | Change sets | Rows | Gzipped | Per change set |");
    println!("| --- | --- | --- | --- | --- |");

    a_days_work("A day, one operator (8 records)", 8);
    a_days_work("A day, three operators (30 records)", 30);
    whole_log(
        "Whole log: a season, medium holding (400 records)",
        &Scale {
            seasons: 1,
            plots: 40,
            records_per_season: 400,
            plots_per_record: 2,
        },
    );
    whole_log(
        "Whole log: a cooperative's year (4000 records)",
        &Scale {
            seasons: 1,
            plots: 120,
            records_per_season: 4000,
            plots_per_record: 3,
        },
    );
    whole_log(
        "Whole log: ten seasons (4000 records)",
        &Scale {
            seasons: 10,
            plots: 120,
            records_per_season: 400,
            plots_per_record: 2,
        },
    );
    println!();
}

// ---------------------------------------------------------------------------
// What acting on alerts costs (docs/sync.md → Alert acknowledgements roam)
// ---------------------------------------------------------------------------

/// Bytes on disk, split three ways: the acts table and its indexes, the log and
/// its indexes, and everything else. `dbstat` counts whole pages, so this is
/// what the file holds, slack included.
#[derive(Clone, Copy)]
struct Footprint {
    acts: i64,
    log: i64,
    rest: i64,
}

impl Footprint {
    fn total(self) -> i64 {
        self.acts + self.log + self.rest
    }
}

fn footprint(conn: &Connection) -> Footprint {
    let by_object: BTreeMap<String, i64> = conn
        .prepare("SELECT name, SUM(pgsize) FROM dbstat GROUP BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut f = Footprint {
        acts: 0,
        log: 0,
        rest: 0,
    };
    for (name, bytes) in by_object {
        if name.contains("alert_acknowledgement") {
            f.acts += bytes;
        } else if name.contains("record_change") {
            f.log += bytes;
        } else {
            f.rest += bytes;
        }
    }
    f
}

fn whole_log_bytes(conn: &Connection) -> i64 {
    bundle_of(conn, &VersionVector::default()).2 as i64
}

/// Every PHI window the holding has ever opened, as the condition an act on
/// its alert is filed under: the treatment, and the plazo's end. Nothing stores
/// the alerts themselves, so acting on all of them at once needs no more than
/// this — which is the ceiling a farmer alone can reach.
fn every_phi_alert(conn: &Connection) -> Vec<(String, String)> {
    conn.prepare(
        "SELECT id, phi_end_date FROM treatment_record
         WHERE deleted_at IS NULL AND phi_end_date IS NOT NULL",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

/// Seen, or hidden: one act on every alert in `alerts`, as the Status view
/// records it.
fn act_on_all(conn: &mut Connection, alerts: &[(String, String)], dismiss: bool) {
    let record = if dismiss {
        terrazgo_core::repository::dismiss_alert
    } else {
        terrazgo_core::repository::acknowledge_alert
    };
    for (treatment_id, due) in alerts {
        record(conn, PHI_WINDOW.kind(), treatment_id, Some(due), None).unwrap();
    }
}

fn medium() -> Scale {
    Scale {
        seasons: 1,
        plots: 40,
        records_per_season: 400,
        plots_per_record: 2,
    }
}

fn pct(part: i64, whole: i64) -> String {
    format!("{:+.0}%", 100.0 * part as f64 / whole as f64)
}

/// One act, split by where its bytes land; then what pruning the superseded
/// ones would do.
fn per_act() {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    scaled_farm(&mut conn, &medium());
    // There are no alert rows to size on either side: nothing stores an alert,
    // only the acts on them.
    let before = footprint(&conn);
    let seen = bundle::seen_by(&conn).unwrap();
    let whole_before = whole_log_bytes(&conn);

    let alerts = every_phi_alert(&conn);
    act_on_all(&mut conn, &alerts, false);
    act_on_all(&mut conn, &alerts, true);
    let n = 2 * alerts.len() as i64;
    let after = footprint(&conn);
    let delta = bundle_of(&conn, &seen).2 as i64;
    let whole_after = whole_log_bytes(&conn);

    println!("| Where one act's bytes land | Per act |");
    println!("| --- | --- |");
    println!(
        "| the acts table and its indexes | {} B |",
        (after.acts - before.acts) / n
    );
    println!(
        "| the log and its indexes | {} B |",
        (after.log - before.log) / n
    );
    println!(
        "| **the database** | **{} B** |",
        (after.total() - before.total()) / n
    );
    println!("| a day's delta, gzipped | {} B |", delta / n);
    println!(
        "| a whole-log bundle, gzipped | {} B |",
        (whole_after - whole_before) / n
    );
    println!();

    // Prune the "seen" rows every dismissal made redundant, as the cheapest
    // pruner there could be: logged hard deletes, one transaction for the lot.
    // Compacted before and after, so freed pages are not counted as kept.
    conn.execute_batch("VACUUM").unwrap();
    let kept = footprint(&conn);
    let superseded: Vec<(String, serde_json::Value)> = conn
        .prepare(
            "SELECT id, alert_type_code, subject_table, subject_id, due_date, status, created_at
             FROM alert_acknowledgement WHERE status = 'acknowledged'",
        )
        .unwrap()
        .query_map([], |r| {
            let id: String = r.get(0)?;
            Ok((
                id.clone(),
                serde_json::json!({
                    "id": id,
                    "alert_type_code": r.get::<_, String>(1)?,
                    "subject_table": r.get::<_, String>(2)?,
                    "subject_id": r.get::<_, String>(3)?,
                    "due_date": r.get::<_, Option<String>>(4)?,
                    "status": r.get::<_, String>(5)?,
                    "created_at": r.get::<_, String>(6)?,
                }),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    {
        let tx = audit::begin(&mut conn, None).unwrap();
        for (id, row) in &superseded {
            tx.execute("DELETE FROM alert_acknowledgement WHERE id = ?1", [id])
                .unwrap();
            let stamp = tx.register("alert_acknowledgement", id, None).unwrap();
            audit::log_delete(&tx, &stamp, "alert_acknowledgement", id, row, None).unwrap();
        }
        tx.commit().unwrap();
    }
    conn.execute_batch("VACUUM").unwrap();
    let pruned = footprint(&conn);
    let count = superseded.len() as i64;
    println!("| Pruning {count} superseded acts | Per act pruned |");
    println!("| --- | --- |");
    println!(
        "| the acts table and its indexes | {:+} B |",
        (pruned.acts - kept.acts) / count
    );
    println!(
        "| the log and its indexes | {:+} B |",
        (pruned.log - kept.log) / count
    );
    println!(
        "| **the database** | **{:+} B** |",
        (pruned.total() - kept.total()) / count
    );
    println!();
}

/// What a holding's acts add at the ceiling a farmer alone can reach: every
/// PHI alert it ever raised seen, then seen and hidden. One device — the real
/// repository writes no act that changes nothing, so a second act per alert
/// needs a second, stronger one.
fn holding(label: &str, scale: &Scale) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    scaled_farm(&mut conn, scale);
    let alerts = every_phi_alert(&conn);
    let book = footprint(&conn).total();
    let book_bundle = whole_log_bytes(&conn);

    act_on_all(&mut conn, &alerts, false);
    let seen = (footprint(&conn).total(), whole_log_bytes(&conn));

    act_on_all(&mut conn, &alerts, true);
    let hidden = (footprint(&conn).total(), whole_log_bytes(&conn));

    println!(
        "| {label} | {} | {} / {} | {} / {} | {} / {} |",
        alerts.len(),
        mb(book),
        kib(book_bundle as usize),
        pct(seen.0 - book, book),
        pct(seen.1 - book_bundle, book_bundle),
        pct(hidden.0 - book, book),
        pct(hidden.1 - book_bundle, book_bundle),
    );
}

fn mb(bytes: i64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_acting_on_alerts_costs() {
    println!();
    per_act();
    println!(
        "| Holding | PHI alerts | The book: database / whole-log bundle | Every alert seen | Seen, then hidden |"
    );
    println!("| --- | --- | --- | --- | --- |");
    holding("A season, medium holding (400 records)", &medium());
    holding(
        "A cooperative's year (4000 records)",
        &Scale {
            seasons: 1,
            plots: 120,
            records_per_season: 4000,
            plots_per_record: 3,
        },
    );
    println!();
}
