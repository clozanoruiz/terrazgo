// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a write costs as the record book grows — the write-side twin of
//! `query_scope.rs`'s measurement, and like it a recipe rather than a gate
//! (docs/maintenance.md §9).
//!
//! Every write that logs pays for the sync stamp: the change set opened with
//! the transaction, a version vector per register, the log row with its four
//! indexes, and the check of that row against the aggregate map. This prints
//! what a new record, a correction and a withdrawal cost, twice over:
//!
//! * **in memory** — the CPU, the part the code decides;
//! * **on a WAL file at the app's durability** — what a farmer waits for, where
//!   one disk flush per save dominates.
//!
//! Each on a fresh book and on one already holding 20 000 treatment records
//! (about 124 000 log rows), because the question worth asking of every write
//! path is whether its cost grows with the history. **The history is written
//! through the real repositories**, never with synthetic SQL: a log bulk-filled
//! with made-up keys once put every new key at one edge of one huge index and
//! reported deletions 140% slower than they are.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Instant;

use common::treatment::{add_es_authorisation, base_fixture, correction_of, sample_treatment};
use module_phytosanitary::models::*;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;

fn three_plots(conn: &mut Connection, farm_id: &str) -> Vec<String> {
    (0..3)
        .map(|i| {
            repo::insert_plot(
                conn,
                NewPlot {
                    farm_id: farm_id.into(),
                    name: format!("Parcela {i}"),
                    area_ha: Some(3.0),
                    es: None,
                },
                None,
            )
            .unwrap()
            .id
        })
        .collect()
}

fn treated(plots: &[String]) -> Vec<NewTreatmentPlot> {
    plots
        .iter()
        .map(|plot_id| NewTreatmentPlot {
            plot_id: plot_id.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        })
        .collect()
}

fn median_us(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Median microseconds of `n` calls of `write`, each timed on its own so one
/// stall (a checkpoint, a page split) cannot move the answer.
fn time_each(n: usize, mut write: impl FnMut(usize)) -> f64 {
    median_us(
        (0..n)
            .map(|i| {
                let start = Instant::now();
                write(i);
                start.elapsed().as_secs_f64() * 1e6
            })
            .collect(),
    )
}

/// `(insert, correction, withdrawal)` in microseconds, on a book already
/// holding `history` treatment records.
fn measure(mut conn: Connection, history: usize, on_disk: bool) -> (f64, f64, f64) {
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plots = three_plots(&mut conn, &fx.farm_id);

    // The history. Durability is lifted while filling only — one flush per
    // record would make the fill take minutes and measures nothing.
    if on_disk {
        conn.pragma_update(None, "synchronous", "OFF").unwrap();
    }
    let ids: Vec<String> = (0..history)
        .map(|_| {
            repo::insert_treatment_record(
                &mut conn,
                sample_treatment(&fx, None, Some(14)),
                treated(&plots),
                None,
            )
            .unwrap()
            .id
        })
        .collect();
    if on_disk {
        conn.pragma_update(None, "synchronous", "FULL").unwrap();
    }

    // Fewer samples on disk, where each one waits for a flush.
    let n = if on_disk { 150 } else { 500 };

    let insert = time_each(n, |_| {
        repo::insert_treatment_record(
            &mut conn,
            sample_treatment(&fx, None, Some(14)),
            treated(&plots),
            None,
        )
        .unwrap();
    });

    // Corrections and withdrawals reach back into the history — every third
    // record for each, never the same one — which is where a write lands in
    // the middle of the log's indexes rather than at their end.
    let mut corrections = ids
        .iter()
        .step_by(3)
        .take(n)
        .map(|id| {
            let stored = repo::get_treatment_record(&conn, id).unwrap();
            let mut update = correction_of(&stored.record, treated(&plots[..2]));
            update.notes = Some("dosis corregida".into());
            (id.clone(), update)
        })
        .collect::<Vec<_>>()
        .into_iter();
    let correction = time_each(n, |_| {
        let (id, update) = corrections.next().unwrap();
        repo::update_treatment_record(&mut conn, &id, update, None).unwrap();
    });

    let mut withdrawals = ids.iter().skip(1).step_by(3);
    let withdrawal = time_each(n, |_| {
        repo::soft_delete_treatment_record(&mut conn, withdrawals.next().unwrap(), None).unwrap();
    });

    (insert, correction, withdrawal)
}

/// Not a test: it asserts nothing and prints a table.
///
/// ```text
/// cargo test -p module-phytosanitary --release --test write_cost -- --ignored --nocapture
/// ```
///
/// To compare against another revision, build both with `--no-run` into
/// separate target directories and run the two binaries interleaved
/// (docs/maintenance.md §9).
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn measure_a_write_on_a_fresh_book_and_on_a_long_one() {
    let dir = std::env::temp_dir().join(format!("terrazgo-write-cost-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    println!("\n| book | where | new record | correction | withdrawal |");
    println!("| --- | --- | --- | --- | --- |");
    for history in [2_000, 20_000] {
        let (i, c, w) = measure(
            module_phytosanitary::open_in_memory().unwrap(),
            history,
            false,
        );
        println!("| {history} records | memory | {i:.0} µs | {c:.0} µs | {w:.0} µs |");

        let path = dir.join(format!("book-{history}.db"));
        let (i, c, w) = measure(module_phytosanitary::open(&path).unwrap(), history, true);
        println!(
            "| {history} records | WAL file | {:.1} ms | {:.1} ms | {:.1} ms |",
            i / 1000.0,
            c / 1000.0,
            w / 1000.0
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
