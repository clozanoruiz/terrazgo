// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the duplicate list costs to read — a recipe rather than a gate, like
//! `write_cost.rs` beside it (docs/maintenance.md §9).
//!
//! The list is worked out whenever it is read and stored nowhere
//! (docs/sync.md → Duplicate suspects), so its cost is paid on every visit to
//! the Status view and to a book's page. This prints it for a medium farm and
//! for a cooperative's year kept as one farm, each with ten campaigns behind
//! it, the history written through the real repository and one record in a
//! hundred of the current campaign recorded twice — so the list has pairs to
//! name, which is the part that grows with what it finds.
//!
//! It also prints what a form pays right after it saves: the pairs one record
//! is in (`list_saved_duplicates`), asked about one of those copies.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Instant;

use common::scale::{Scale, scaled_farm};
use common::treatment::{Fixture, sample_treatment};
use module_phytosanitary::duplicates::{
    ANALYSIS_DUPLICATES, NON_FIELD_DUPLICATES, SEED_TREATMENT_DUPLICATES, TREATMENT_DUPLICATES,
};
use module_phytosanitary::models::NewTreatmentPlot;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;
use terrazgo_core::duplicates::{
    CROP_DUPLICATES, DuplicatePolicy, HARVEST_DUPLICATES, SOWING_DUPLICATES, Scope,
};

/// Inside the latest campaign the scaled farm builds.
const TODAY: &str = "2026-06-30";

/// Every rule this crate can reach: core's and this module's.
const POLICIES: [DuplicatePolicy; 7] = [
    CROP_DUPLICATES,
    SOWING_DUPLICATES,
    HARVEST_DUPLICATES,
    TREATMENT_DUPLICATES,
    NON_FIELD_DUPLICATES,
    SEED_TREATMENT_DUPLICATES,
    ANALYSIS_DUPLICATES,
];

fn median_ms(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Median milliseconds of `n` reads.
fn time_reads(n: usize, mut read: impl FnMut() -> usize) -> (f64, usize) {
    let mut found = 0;
    let samples = (0..n)
        .map(|_| {
            let start = Instant::now();
            found = read();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    (median_ms(samples), found)
}

/// The farm, with one treatment in a hundred of its latest campaign recorded a
/// second time — same day, same product, same plots — and the id of the last
/// copy, which has a pair.
fn farm(records_per_season: usize, plots: usize) -> (Connection, String, String) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    let scaled = scaled_farm(
        &mut conn,
        &Scale {
            records_per_season,
            plots,
            ..Scale::default()
        },
    );
    let latest = scaled.latest_season().to_owned();
    let fx = fixture_of(&conn, &scaled.farm_id, &latest);
    let book = repo::list_treatment_records(&conn, &latest, &scaled.farm_id).unwrap();
    let mut copy_id = String::new();
    for original in book.iter().step_by(100) {
        let mut copy = sample_treatment(&fx, None, Some(21));
        copy.season_id = latest.clone();
        copy.application_date = original.record.application_date.clone();
        let plots = original
            .plots
            .iter()
            .map(|plot| NewTreatmentPlot {
                plot_id: plot.plot_id.clone(),
                crop_id: None,
                surface_treated_ha: 1.0,
                growth_stage_code: None,
            })
            .collect();
        copy_id = repo::insert_treatment_record(&mut conn, copy, plots, None)
            .unwrap()
            .id;
    }
    (conn, latest, copy_id)
}

/// The scaled farm's product and operator, read back rather than rebuilt: a
/// fresh fixture would add a second farm.
fn fixture_of(conn: &Connection, farm_id: &str, season_id: &str) -> Fixture {
    let (operator_id, product_id): (String, String) = conn
        .query_row(
            "SELECT operator_id, product_id FROM treatment_record WHERE farm_id = ?1 LIMIT 1",
            [farm_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    Fixture {
        season_id: season_id.to_owned(),
        farm_id: farm_id.to_owned(),
        operator_id,
        product_id,
    }
}

/// Not a test: it asserts nothing and prints a table.
///
/// ```text
/// cargo test -p module-phytosanitary --release --test duplicate_cost -- --ignored --nocapture
/// ```
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn measure_the_duplicate_list_on_a_medium_farm_and_a_cooperatives_year() {
    let captions = module_phytosanitary::ROW_CAPTIONS;
    println!(
        "\n| farm | treatments in the current books | pairs | Status view | the book's page | after a save |"
    );
    println!("| --- | --- | --- | --- | --- | --- |");
    // The scaled farm sprays one product everywhere and walks its plots in
    // order, two per spray. On 120 plots at 4 000 sprays a campaign it comes
    // back to a plot every 60 sprays — under two days — so nearly every spray
    // has a genuine suspect: the worst case, measured on purpose. On 400 plots
    // the planted copies are the only pairs, which is the ordinary case for a
    // farm that size.
    for (name, per_season, plots) in [
        ("medium", 400, 120),
        ("a cooperative's year", 4_000, 400),
        (
            "the same, one product on each plot every other day",
            4_000,
            120,
        ),
    ] {
        let (conn, latest, copy) = farm(per_season, plots);
        let (status, pairs) = time_reads(40, || {
            terrazgo_core::repository::list_duplicates(
                &conn,
                &POLICIES,
                captions,
                Scope::Current { today: TODAY },
            )
            .unwrap()
            .suspects
            .len()
        });
        let (page, _) = time_reads(40, || {
            terrazgo_core::repository::list_duplicates(
                &conn,
                &POLICIES,
                captions,
                Scope::Book { season_id: &latest },
            )
            .unwrap()
            .suspects
            .len()
        });
        let (save, found) = time_reads(40, || {
            terrazgo_core::repository::list_saved_duplicates(
                &conn,
                &POLICIES,
                captions,
                "treatment_record",
                &copy,
            )
            .unwrap()
            .suspects
            .len()
        });
        assert!(found > 0, "the copy asked about has a pair");
        let current: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM treatment_record r JOIN season s ON s.id = r.season_id
                 WHERE s.ends_on >= date(?1, '-1 year')",
                [TODAY],
                |r| r.get(0),
            )
            .unwrap();
        println!("| {name} | {current} | {pairs} | {status:.1} ms | {page:.1} ms | {save:.2} ms |");
    }
}
