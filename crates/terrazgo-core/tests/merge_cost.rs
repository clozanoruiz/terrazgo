// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What an import costs the merge layer, in the three shapes that pull in
//! different directions. Ignored, like the other measurements in this repo: it
//! is an answer on demand, not an assertion.
//!
//! ```
//! cargo test -p terrazgo-core --release --test merge_cost -- --ignored --nocapture
//! ```
//!
//! It times the two halves of a delivery separately, because only one of them
//! is the merge's: **carrying** the log rows in (plain inserts, the transport's
//! cost) and **settling** each register the rows touched.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use common::*;
use rusqlite::Connection;
use terrazgo_core::merge::{Head, heads, live_head, settle};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

const LOG_COLUMNS: &str = "id, entity_table, entity_id, season_id, operation, changed_at, actor,
     payload, root_table, root_id, origin_device, origin_seq, version_vector, hlc";

fn device(id: &str) -> Connection {
    let conn = terrazgo_core::open_in_memory().unwrap();
    terrazgo_core::sync::install_device(&conn, id).unwrap();
    conn
}

/// Every log row of `from`, as values ready to re-insert.
fn carried(from: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    from.prepare(&format!(
        "SELECT {LOG_COLUMNS} FROM record_change ORDER BY hlc, id"
    ))
    .unwrap()
    .query_map([], |row| {
        (0..14)
            .map(|index| row.get::<_, rusqlite::types::Value>(index))
            .collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

/// The registers a set of carried rows touches, in the order an importer walks
/// them.
fn touched(rows: &[Vec<rusqlite::types::Value>]) -> BTreeSet<(String, String)> {
    rows.iter()
        .map(|row| {
            let text = |index: usize| match &row[index] {
                rusqlite::types::Value::Text(value) => value.clone(),
                other => panic!("expected text, got {other:?}"),
            };
            (text(8), text(9))
        })
        .collect()
}

/// One delivery into a fresh device: carry the rows, then settle each register.
/// Returns the two halves.
fn deliver(rows: &[Vec<rusqlite::types::Value>], to: &mut Connection) -> (Duration, Duration) {
    let registers = touched(rows);
    let was_live: Vec<Option<Head>> = registers
        .iter()
        .map(|(table, id)| live_head(&heads(to, table, id).unwrap()).cloned())
        .collect();

    let tx = to.transaction().unwrap();
    tx.execute_batch("PRAGMA defer_foreign_keys = ON").unwrap();

    let carrying = Instant::now();
    {
        let mut insert = tx
            .prepare(&format!(
                "INSERT INTO record_change ({LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"
            ))
            .unwrap();
        for row in rows {
            insert
                .execute(rusqlite::params_from_iter(row.iter()))
                .unwrap();
        }
    }
    let carried_in = carrying.elapsed();

    let settling = Instant::now();
    for ((table, id), before) in registers.iter().zip(was_live) {
        settle(&tx, table, id, before.as_ref()).unwrap();
    }
    let settled = settling.elapsed();

    tx.commit().unwrap();
    (carried_in, settled)
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// Run one delivery `times` over, each into a device built by `build`.
fn measure(
    label: &str,
    rows: &[Vec<rusqlite::types::Value>],
    times: usize,
    build: impl Fn() -> Connection,
) {
    let mut carrying = Vec::new();
    let mut settling = Vec::new();
    for _ in 0..times {
        let mut receiver = build();
        let (carried_in, settled) = deliver(rows, &mut receiver);
        carrying.push(carried_in);
        settling.push(settled);
    }
    println!(
        "{label:38} carry {:>9.3?}   settle {:>9.3?}   rows {}   registers {}",
        median(carrying),
        median(settling),
        rows.len(),
        touched(rows).len()
    );
}

/// A holding with `records` sowing records of three plots each — the everyday
/// import: many registers, each written once.
fn a_campaign(records: usize) -> Connection {
    let mut conn = device("0192f3a4-0000-7000-8000-00000000000a");
    let farm = repo::insert_farm(&mut conn, new_farm("Los Llanos"), None).unwrap();
    let season =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let plots: Vec<String> = ["El Prado", "La Loma", "Las Eras"]
        .iter()
        .map(|name| {
            repo::insert_plot(&mut conn, new_plot(&farm.id, name), None)
                .unwrap()
                .id
        })
        .collect();
    for _ in 0..records {
        repo::insert_sowing_record(
            &mut conn,
            NewSowingRecord {
                season_id: season.id.clone(),
                farm_id: farm.id.clone(),
                kind_code: "sowing".into(),
                sown_on: "2026-04-10".into(),
                sowing_end_date: None,
                flooded_on: None,
                seed_quantity_kg: Some(180.0),
                notes: None,
                plots: plots
                    .iter()
                    .map(|plot_id| NewSowingPlot {
                        plot_id: plot_id.clone(),
                        crop_id: None,
                    })
                    .collect(),
            },
            None,
        )
        .unwrap();
    }
    conn
}

/// One register corrected `edits` times — the shape where the two changes pull
/// against each other: more sets to compare, fewer statements to run.
fn a_long_history(edits: usize) -> Connection {
    let mut conn = device("0192f3a4-0000-7000-8000-00000000000a");
    let farm = repo::insert_farm(&mut conn, new_farm("Los Llanos"), None).unwrap();
    for round in 0..edits {
        repo::update_farm(
            &mut conn,
            &farm.id,
            UpdateFarm {
                name: format!("La Vega {round}"),
                owner_name: None,
                owner_tax_id: None,
                location_text: None,
                address: None,
                postal_code: None,
                phone_fixed: None,
                phone_mobile: None,
                email: None,
                opened_on: None,
                latitude: None,
                longitude: None,
                es: None,
                representative: None,
            },
            None,
        )
        .unwrap();
    }
    conn
}

#[test]
#[ignore = "measurement, not an assertion; see docs/maintenance.md"]
fn what_an_import_costs() {
    println!();
    for records in [50, 300] {
        let source = a_campaign(records);
        let rows = carried(&source);
        measure(&format!("a campaign, {records} records"), &rows, 5, || {
            device("0192f3a4-0000-7000-8000-00000000000b")
        });
    }

    for edits in [10, 60, 200] {
        let source = a_long_history(edits);
        let rows = carried(&source);
        measure(&format!("one register, {edits} edits"), &rows, 5, || {
            device("0192f3a4-0000-7000-8000-00000000000b")
        });
    }

    // The same campaign replayed over tables that already hold its rows, which
    // is what the applier does on every `ON CONFLICT DO UPDATE` rather than on
    // an insert. Reached by clearing the receiver's LOG and carrying the same
    // rows again — a device restored from a snapshot older than the bundle.
    let source = a_campaign(300);
    let rows = carried(&source);
    measure("a campaign, rows already present", &rows, 5, || {
        let mut receiver = device("0192f3a4-0000-7000-8000-00000000000b");
        deliver(&rows, &mut receiver);
        receiver
            .execute("DELETE FROM record_change", [])
            .expect("clear the log so the same rows carry again");
        receiver
    });
    println!();
}
