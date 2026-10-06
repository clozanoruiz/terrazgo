// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What merging two books costs, on the heaviest register — treatments.
//! Ignored, like the other measurements in this repo: an answer on demand, not
//! an assertion (docs/sync.md → The merge).
//!
//! ```
//! cargo test -p module-phytosanitary --release --test book_merge_cost -- --ignored --nocapture
//! ```
//!
//! Four figures per size: the merge on the device that makes it, what it adds
//! to that device's log, the file that carries it, and applying that file on a
//! device that holds both books already.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use common::treatment::{Fixture, add_es_authorisation, base_fixture, sample_treatment};
use module_phytosanitary::models::NewTreatmentPlot;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;
use terrazgo_core::bundle;
use terrazgo_core::models::{NewPlot, NewSeason};
use terrazgo_core::sync::VersionVector;

/// A farm whose second book holds `records` treatments — the one that goes —
/// and what recording them added to the log, for scale.
fn two_books(records: usize) -> (Connection, Fixture, String, i64) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
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
    let absorbed = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: fx.farm_id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2026 bis".into()),
        },
        None,
    )
    .unwrap()
    .id;
    let before = log_bytes(&conn);
    for _ in 0..records {
        let mut new = sample_treatment(&fx, None, Some(21));
        new.season_id = absorbed.clone();
        let plots = vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }];
        repo::insert_treatment_record(&mut conn, new, plots, None).unwrap();
    }
    let recorded = log_bytes(&conn) - before;
    (conn, fx, absorbed, recorded)
}

/// The bytes the log and its indexes occupy.
fn log_bytes(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT SUM(pgsize) FROM dbstat
         WHERE name = 'record_change' OR name LIKE 'idx_record_change%'
            OR name LIKE 'sqlite_autoindex_record_change%'",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

fn bundle_for(conn: &Connection, seen: &VersionVector) -> Vec<u8> {
    let device = terrazgo_core::sync::installed_device(conn).unwrap();
    let mut bytes = Vec::new();
    bundle::write_bundle(conn, &device, seen, &mut bytes).unwrap();
    bytes
}

fn median(mut runs: Vec<Duration>) -> Duration {
    runs.sort();
    runs[runs.len() / 2]
}

fn kib(bytes: i64) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0)
}

#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_merging_two_books_costs() {
    println!();
    println!(
        "| Records moved | Recording them logged | Merge | Log grows by | File | Applying the file |"
    );
    println!("| --- | --- | --- | --- | --- | --- |");
    for records in [500, 4_000] {
        let mut merged = Vec::new();
        let mut applied = Vec::new();
        let mut grown = 0;
        let mut file = 0;
        let mut recorded = 0;
        for _ in 0..3 {
            let (mut conn, fx, absorbed, logged) = two_books(records);
            recorded = logged;
            // Another device holding both books already, as a file carried
            // them there.
            let group = terrazgo_core::sync::ensure_sync_group(&conn).unwrap();
            let mut other = module_phytosanitary::open_in_memory().unwrap();
            terrazgo_core::sync::join_sync_group(&other, &group).unwrap();
            let everything =
                bundle::read_bundle(&bundle_for(&conn, &VersionVector::default())[..]).unwrap();
            bundle::apply_bundle(&mut other, &everything, terrazgo_core::date::now_ms()).unwrap();

            let seen = bundle::seen_by(&conn).unwrap();
            let before = log_bytes(&conn);
            let started = Instant::now();
            terrazgo_core::repository::merge_books(&mut conn, &fx.season_id, &absorbed, &[], None)
                .unwrap();
            merged.push(started.elapsed());
            grown = log_bytes(&conn) - before;

            let bytes = bundle_for(&conn, &seen);
            file = i64::try_from(bytes.len()).unwrap();
            let parsed = bundle::read_bundle(&bytes[..]).unwrap();
            let started = Instant::now();
            bundle::apply_bundle(&mut other, &parsed, terrazgo_core::date::now_ms()).unwrap();
            applied.push(started.elapsed());
        }
        println!(
            "| {records} | {} | {:.0?} | {} | {} | {:.0?} |",
            kib(recorded),
            median(merged),
            kib(grown),
            kib(file),
            median(applied)
        );
    }
    println!();
}

/// What deleting a book with its records costs, and bringing it back — held to
/// the merge's budget: no slower than merging two books of the same size
/// (docs/sync.md → What it costs a farmer in time). Run beside the merge's
/// measurement, `--test-threads=1`, so the two are read on one machine at one
/// moment.
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_deleting_a_book_and_bringing_it_back_costs() {
    println!();
    println!(
        "| Records | Deleting | Log grows by | File | Applying it | Bringing back | Log grows by | File | Applying it |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for records in [500, 4_000] {
        let (mut deleted, mut applied_deletion) = (Vec::new(), Vec::new());
        let (mut restored, mut applied_restore) = (Vec::new(), Vec::new());
        let (mut grown, mut file, mut regrown, mut refile) = (0, 0, 0, 0);
        for _ in 0..3 {
            let (mut conn, _fx, book, _) = two_books(records);
            let group = terrazgo_core::sync::ensure_sync_group(&conn).unwrap();
            let mut other = module_phytosanitary::open_in_memory().unwrap();
            terrazgo_core::sync::join_sync_group(&other, &group).unwrap();
            let everything =
                bundle::read_bundle(&bundle_for(&conn, &VersionVector::default())[..]).unwrap();
            bundle::apply_bundle(&mut other, &everything, terrazgo_core::date::now_ms()).unwrap();

            let seen = bundle::seen_by(&conn).unwrap();
            let before = log_bytes(&conn);
            let started = Instant::now();
            let gone = terrazgo_core::repository::delete_book(&mut conn, &book, &[], None).unwrap();
            deleted.push(started.elapsed());
            assert_eq!(gone, records);
            grown = log_bytes(&conn) - before;
            let bytes = bundle_for(&conn, &seen);
            file = i64::try_from(bytes.len()).unwrap();
            let parsed = bundle::read_bundle(&bytes[..]).unwrap();
            let started = Instant::now();
            bundle::apply_bundle(&mut other, &parsed, terrazgo_core::date::now_ms()).unwrap();
            applied_deletion.push(started.elapsed());

            let seen = bundle::seen_by(&conn).unwrap();
            let before = log_bytes(&conn);
            let started = Instant::now();
            let back = terrazgo_core::repository::restore_book(
                &mut conn,
                &book,
                &terrazgo_core::date::today_utc(),
                None,
            )
            .unwrap();
            restored.push(started.elapsed());
            assert_eq!(back.records, records);
            regrown = log_bytes(&conn) - before;
            let bytes = bundle_for(&conn, &seen);
            refile = i64::try_from(bytes.len()).unwrap();
            let parsed = bundle::read_bundle(&bytes[..]).unwrap();
            let started = Instant::now();
            bundle::apply_bundle(&mut other, &parsed, terrazgo_core::date::now_ms()).unwrap();
            applied_restore.push(started.elapsed());
        }
        println!(
            "| {records} | {:.0?} | {} | {} | {:.0?} | {:.0?} | {} | {} | {:.0?} |",
            median(deleted),
            kib(grown),
            kib(file),
            median(applied_deletion),
            median(restored),
            kib(regrown),
            kib(refile),
            median(applied_restore)
        );
    }
    println!();
}

/// What the purge costs: erasing a book deleted with its treatments, once it is
/// due, and another device applying the file that carries the erasure — held
/// to deleting the same book (docs/sync.md → What it costs a farmer in time).
/// In memory, so it reads the work and not the disk; `secure_delete` and the
/// checkpoint are measured on a file by the probe the doc cites.
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_erasing_a_deleted_book_costs() {
    println!();
    println!("| Records | Erasing | Log shrinks by | File | Applying it |");
    println!("| --- | --- | --- | --- | --- |");
    let due = terrazgo_core::date::add_days(
        &terrazgo_core::date::today_utc(),
        terrazgo_core::repository::REMOVED_BOOK_DAYS + 1,
    )
    .unwrap();
    for records in [500, 4_000] {
        let (mut erased, mut applied) = (Vec::new(), Vec::new());
        let (mut shrunk, mut file) = (0, 0);
        for _ in 0..3 {
            let (mut conn, _fx, book, _) = two_books(records);
            terrazgo_core::repository::delete_book(&mut conn, &book, &[], None).unwrap();
            let group = terrazgo_core::sync::ensure_sync_group(&conn).unwrap();
            let mut other = module_phytosanitary::open_in_memory().unwrap();
            terrazgo_core::sync::join_sync_group(&other, &group).unwrap();
            let everything =
                bundle::read_bundle(&bundle_for(&conn, &VersionVector::default())[..]).unwrap();
            bundle::apply_bundle(&mut other, &everything, terrazgo_core::date::now_ms()).unwrap();

            let seen = bundle::seen_by(&conn).unwrap();
            let before = log_bytes(&conn);
            let started = Instant::now();
            let gone = terrazgo_core::repository::purge_due(&mut conn, &due, None).unwrap();
            erased.push(started.elapsed());
            assert_eq!(
                (gone.registers, gone.books),
                (records, 1),
                "every treatment; the book went with them, for the farmer"
            );
            shrunk = before - log_bytes(&conn);
            let bytes = bundle_for(&conn, &seen);
            file = i64::try_from(bytes.len()).unwrap();
            let parsed = bundle::read_bundle(&bytes[..]).unwrap();
            let started = Instant::now();
            let summary =
                bundle::apply_bundle(&mut other, &parsed, terrazgo_core::date::now_ms()).unwrap();
            applied.push(started.elapsed());
            assert_eq!((summary.purged.books, summary.purged.records), (1, records));
        }
        println!(
            "| {records} | {:.0?} | {} | {} | {:.0?} |",
            median(erased),
            kib(shrunk),
            kib(file),
            median(applied)
        );
    }
    println!();
}

/// The fold read on `day`, and how long it took.
fn read_fold(
    conn: &Connection,
    day: &str,
) -> (Duration, Vec<terrazgo_core::repository::RemovedBook>) {
    let started = Instant::now();
    let listed = terrazgo_core::repository::list_removed_books(conn, day).unwrap();
    (started.elapsed(), listed)
}

/// What the fold under the record-book list reads of a book deleted with its
/// treatments (docs/sync.md → What it costs a farmer in time): inside its
/// thirty days, counting what would come back; past them, saying what its
/// erasure waits for — a device not yet heard from, the common case, and
/// nothing, which also asks what still points at it.
#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_the_fold_reads_of_a_deleted_book() {
    println!();
    println!("| Records | Inside its thirty days | Waiting for a device | Due, not yet erased |");
    println!("| --- | --- | --- | --- |");
    let today = terrazgo_core::date::today_utc();
    let due =
        terrazgo_core::date::add_days(&today, terrazgo_core::repository::REMOVED_BOOK_DAYS + 1)
            .unwrap();
    for records in [500, 4_000] {
        let (mut inside, mut waiting, mut ready) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..3 {
            let (mut conn, _fx, book, _) = two_books(records);
            // A phone in the group, heard from before the deletion and not since.
            let group = terrazgo_core::sync::ensure_sync_group(&conn).unwrap();
            let mut phone = module_phytosanitary::open_in_memory().unwrap();
            terrazgo_core::sync::join_sync_group(&phone, &group).unwrap();
            terrazgo_core::repository::register_this_device(&mut phone, None).unwrap();
            let from_phone =
                bundle::read_bundle(&bundle_for(&phone, &VersionVector::default())[..]).unwrap();
            bundle::apply_bundle(&mut conn, &from_phone, terrazgo_core::date::now_ms()).unwrap();
            terrazgo_core::repository::delete_book(&mut conn, &book, &[], None).unwrap();

            let (took, listed) = read_fold(&conn, &today);
            assert_eq!(listed[0].removal.records[0].count, records);
            inside.push(took);
            let (took, listed) = read_fold(&conn, &due);
            assert!(matches!(
                listed[0].erasing,
                Some(terrazgo_core::repository::Erasing::Devices { .. })
            ));
            waiting.push(took);

            let phone_id = terrazgo_core::sync::installed_device(&phone).unwrap();
            terrazgo_core::repository::retire_sync_peer(&mut conn, &phone_id, true, None).unwrap();
            let (took, listed) = read_fold(&conn, &due);
            assert_eq!(
                listed[0].erasing,
                Some(terrazgo_core::repository::Erasing::Due { from: None })
            );
            ready.push(took);
        }
        println!(
            "| {records} | {:.0?} | {:.0?} | {:.0?} |",
            median(inside),
            median(waiting),
            median(ready)
        );
    }
    println!();
}
