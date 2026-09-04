// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Does the cost of a question grow with the record book?
//!
//! The behaviour tests elsewhere in this directory ask whether an answer is
//! right. These ask whether getting it stays affordable after twenty campaigns,
//! which is a different property and one no correctness test can fail on:
//! an N+1 returns the right rows, and a query that reads the whole history to
//! answer a question about today returns the right answer too.
//!
//! Two shapes are pinned here, and `terrazgo_testkit::query_cost` reports both:
//!
//!   * **statements** — the rows multiply, the statement count stands still;
//!   * **rows** — a query answering a question about *today* must not make
//!     SQLite produce twenty campaigns of rows to do it. This is the half no
//!     index can fix, and the half a statement count calls fine.
//!
//! The timing table that motivated all of this is not a test — timings are not
//! assertions — so it lives in the `#[ignore]`d measurement at the bottom.
//! `docs/maintenance.md` says how to run it. It is a measurement rather than a
//! disabled test: nothing here is being skipped to make the suite pass.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::scale::{Scale, ScaledFarm, scaled_farm};
use module_cue::alerts::AlertConfig;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;
use terrazgo_testkit::query_cost;

/// A day inside the last campaign's applications, so a handful of PHI windows
/// are open and every earlier campaign's are long closed — the ratio a real
/// holding presents.
const TODAY: &str = "2026-06-20";

fn built(scale: &Scale) -> (Connection, ScaledFarm) {
    let mut conn = open_in_memory().unwrap();
    let farm = scaled_farm(&mut conn, scale);
    (conn, farm)
}

// --- statement counts -------------------------------------------------------

/// Run `list` over `Scale::small` and again over four times the records,
/// returning `(few, many, statements_few, statements_many)`.
fn at_both_scales<T>(
    list: impl Fn(&Connection, &str, &str) -> Vec<T>,
) -> (Vec<T>, Vec<T>, usize, usize) {
    let (mut conn, farm) = built(&Scale::small());
    let (few, few_cost) = query_cost(&mut conn, |conn| {
        list(conn, farm.latest_season(), &farm.farm_id)
    });

    let (mut conn, farm) = built(&Scale::small_times_four());
    let (many, many_cost) = query_cost(&mut conn, |conn| {
        list(conn, farm.latest_season(), &farm.farm_id)
    });

    (few, many, few_cost.statements, many_cost.statements)
}

#[test]
fn listing_a_season_costs_the_same_number_of_statements_at_four_times_the_records() {
    let (few, many, few_statements, many_statements) = at_both_scales(|conn, season, farm| {
        repo::list_treatment_records(conn, season, farm).unwrap()
    });

    assert_eq!(few.len(), 4);
    assert_eq!(many.len(), 16, "four times the records");
    assert_eq!(
        few_statements, many_statements,
        "the rows multiplied and the statement count must not: {few_statements} then \
         {many_statements}. A per-record child query is the usual cause."
    );
}

#[test]
fn the_export_listing_is_hoisted_too() {
    // Its twin reads soft-deleted parents as well, and the SIEX exporter is the
    // one caller that walks a whole campaign in one go.
    let (few, many, few_statements, many_statements) = at_both_scales(|conn, season, farm| {
        repo::list_treatment_records_for_export(conn, season, farm).unwrap()
    });

    assert_eq!((few.len(), many.len()), (4, 16));
    assert_eq!(few_statements, many_statements);
}

#[test]
fn a_hoisted_listing_returns_exactly_what_the_per_record_path_returns() {
    // The hoist is only safe if it is invisible in the answer. Every record's
    // children must match what `get_treatment_record` resolves one at a time —
    // same rows, same order, same parents.
    let (conn, farm) = built(&Scale::small_times_four());
    let listed = repo::list_treatment_records(&conn, farm.latest_season(), &farm.farm_id).unwrap();
    assert_eq!(listed.len(), 16);

    for entry in &listed {
        let one = repo::get_treatment_record(&conn, &entry.record.id).unwrap();
        assert_eq!(one.record.id, entry.record.id);
        assert_eq!(
            one.plots.iter().map(|p| &p.id).collect::<Vec<_>>(),
            entry.plots.iter().map(|p| &p.id).collect::<Vec<_>>(),
        );
        assert_eq!(
            one.problems.iter().map(|p| &p.id).collect::<Vec<_>>(),
            entry.problems.iter().map(|p| &p.id).collect::<Vec<_>>(),
        );
        assert_eq!(
            one.justifications.iter().map(|j| &j.id).collect::<Vec<_>>(),
            entry
                .justifications
                .iter()
                .map(|j| &j.id)
                .collect::<Vec<_>>(),
        );
        assert!(
            entry
                .plots
                .iter()
                .all(|p| p.treatment_record_id == entry.record.id),
            "a hoisted child was handed to the wrong parent"
        );
    }
}

// --- result-set scope -------------------------------------------------------

#[test]
fn the_phi_tint_does_not_read_campaigns_nobody_asked_about() {
    // Ten campaigns of history; the map asks one question about today. Each
    // campaign treats a different set of plots, so an unscoped query answers
    // with every plot ever treated and a scoped one with the recent few.
    let scale = Scale {
        seasons: 10,
        plots: 40,
        records_per_season: 4,
        plots_per_record: 2,
    };
    let (mut conn, farm) = built(&scale);

    let (rows, cost) = query_cost(&mut conn, |conn| {
        repo::phi_status_for_farm(conn, &farm.farm_id, TODAY, repo::default_phi_horizon_days())
            .unwrap()
    });

    assert!(
        cost.rows <= scale.records_per_season * scale.plots_per_record,
        "the tint read {} rows to answer a question about today, over {} \
         records of history",
        cost.rows,
        scale.total_records()
    );
    assert!(
        rows.len() < farm.plot_ids.len(),
        "every plot came back ({} of {}), so the answer is the whole history \
         rather than the recent past",
        rows.len(),
        farm.plot_ids.len()
    );
}

#[test]
fn the_phi_tint_stays_bounded_at_the_widest_horizon_a_farmer_can_choose() {
    // The horizon became a device setting on 2026-08-26, and it IS the WHERE
    // clause that keeps the test above honest. So the guarantee has to hold at
    // the value a farmer can actually leave it on, not only at the default —
    // otherwise the ceiling is decoration and the scoping is one Settings
    // visit away from being undone.
    let scale = Scale {
        seasons: 10,
        plots: 40,
        records_per_season: 4,
        plots_per_record: 2,
    };
    let (mut conn, farm) = built(&scale);

    let (rows, capped) = query_cost(&mut conn, |conn| {
        repo::phi_status_for_farm(conn, &farm.farm_id, TODAY, repo::MAX_PHI_HORIZON_DAYS).unwrap()
    });

    // The control: what the same query costs with no effective horizon at all,
    // which is the state this whole mechanism replaced. Deliberately a value no
    // farmer can choose — validation happens at the settings boundary, so the
    // repository will still answer for it, and that is what makes it a control.
    let (all_rows, uncapped) = query_cost(&mut conn, |conn| {
        repo::phi_status_for_farm(conn, &farm.farm_id, TODAY, 100 * 365).unwrap()
    });

    // Measured against the control rather than against arithmetic about
    // campaigns: 730 days from a June "today" spans two full campaigns plus the
    // tail of a third (the fixture applies March-June), so a hand-computed
    // "two campaigns' worth" is off by exactly that tail. What the ceiling has
    // to guarantee is not a particular number, it is that the read stays a
    // fraction of the whole book — 18 rows against 80 when last measured.
    assert!(
        capped.rows * 2 < uncapped.rows,
        "at the maximum horizon the tint read {} rows against the unbounded \
         {}, over {} records of history — the ceiling is not bounding anything",
        capped.rows,
        uncapped.rows,
        scale.total_records()
    );
    assert!(
        rows.len() < all_rows.len(),
        "the widest horizon returned as many plots as the unbounded read ({} \
         of {}), which is the answer the horizon exists to prevent",
        rows.len(),
        all_rows.len()
    );
}

#[test]
fn the_alert_refresh_does_not_read_the_whole_treatment_table() {
    // Ten campaigns behind one question about today. The refresh runs a fixed
    // handful of STATEMENTS either way — which is exactly why the statement
    // count cannot see this defect, and the row count can.
    let scale = Scale {
        seasons: 10,
        plots: 12,
        records_per_season: 12,
        plots_per_record: 1,
    };
    let (mut conn, _farm) = built(&scale);

    let (_, cost) = query_cost(&mut conn, |conn| {
        repo::refresh_alerts(conn, TODAY, &AlertConfig::defaults()).unwrap()
    });

    assert!(
        cost.rows < scale.total_records(),
        "the refresh produced {} rows over {} records — it is deriving today's \
         alerts from the whole history",
        cost.rows,
        scale.total_records()
    );
}

// --- the measurement --------------------------------------------------------

/// Not a test: it asserts nothing and prints the table
/// `docs/data-model.md` → "Indexes and query scope" records.
///
/// ```text
/// cargo test -p module-cue --release --test query_scope -- --ignored --nocapture
/// ```
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn measure_the_hot_paths_across_ten_fifteen_and_twenty_seasons() {
    println!("\n| treatments | map PHI tint | alert refresh |");
    println!("| --- | --- | --- |");
    for seasons in [10, 15, 20] {
        let scale = Scale {
            seasons,
            ..Scale::default()
        };
        let (mut conn, farm) = built(&scale);

        let start = std::time::Instant::now();
        repo::phi_status_for_farm(
            &conn,
            &farm.farm_id,
            TODAY,
            repo::default_phi_horizon_days(),
        )
        .unwrap();
        let tint = start.elapsed();

        let start = std::time::Instant::now();
        repo::refresh_alerts(&mut conn, TODAY, &AlertConfig::defaults()).unwrap();
        let refresh = start.elapsed();

        println!(
            "| {} ({} seasons) | {:.1} ms | {:.1} ms |",
            scale.total_records(),
            seasons,
            tint.as_secs_f64() * 1000.0,
            refresh.as_secs_f64() * 1000.0,
        );
    }
}
