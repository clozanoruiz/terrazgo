// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Random histories over three devices, each ending where a device that
//! replays the whole log from nothing ends (docs/sync.md → How it is tested).
//!
//! **The scenario files test what someone thought of; this tests what nobody
//! did.** A laptop and two phones make random edits, removals, book deletions
//! and restores, merges of a twin book, moves of records left in a removed
//! book, resolutions of whatever conflicts arise and purges — each as if a
//! month had passed for that device, racing what the others do — and exchange
//! real files through `write_bundle`/`apply_bundle`, whole or trimmed to what
//! the receiver holds. After a full exchange, every device must hold the same
//! tables, review queue and records in a removed book as a fresh device
//! joining from one of them, and the same log as each other. A refused file or
//! a resolution that fails is a finding too: nothing in such a history should
//! be refused.
//!
//! *Added by the audit of the arc (2026-10-03)*: this sweep found that two
//! devices could end with one log and different tables, that a resolution
//! could fail on a raw `UNIQUE` error, and that a purge could erase a book a
//! version arriving later still named, so each device refused the other's
//! files for good — all missed by every scenario above it.
//!
//! **The seeds are fixed; the clock is not.** Which version goes live depends
//! on each write's stamp, and stamps come from the wall clock, so one seed can
//! take a different path on another run. The property holds for every path, so
//! a failure is a defect whichever run finds it; the message carries the seed
//! and every act, to reproduce it with the timing that showed it.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::ops::RangeInclusive;

use common::{new_farm, new_plot, new_season, sowing_state};
use rusqlite::Connection;
use terrazgo_core::bundle;
use terrazgo_core::date::{add_days, now_ms, today_utc};
use terrazgo_core::merge::{CORE_ROW_CAPTIONS, heads, resolve};
use terrazgo_core::models::{NewSeason, NewSowingPlot, NewSowingRecord};
use terrazgo_core::repository as repo;
use terrazgo_core::sync::VersionVector;

/// The laptop, two phones, and the device that joins at the end.
const DEVICES: [&str; 4] = [
    "0192f3a4-0000-7000-8000-00000000000a",
    "0192f3a4-0000-7000-8000-00000000000b",
    "0192f3a4-0000-7000-8000-00000000000c",
    "0192f3a4-0000-7000-8000-00000000000d",
];

/// Acts per history in the sweep the suite runs.
const STEPS: usize = 40;

/// A small random-number generator (xorshift64), so a seed names one sequence
/// of choices on every machine with no dependency for it.
struct Choices(u64);

impl Choices {
    fn from_seed(seed: u64) -> Self {
        // Spread small seeds across the state; xorshift may never hold 0.
        Choices(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).unwrap()
    }
}

/// One simulated device: a real database at core's schema, with its own id.
struct Device {
    conn: Connection,
    id: &'static str,
}

impl Device {
    fn new(id: &'static str) -> Self {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        repo::register_this_device(&mut conn, None).unwrap();
        Device { conn, id }
    }

    /// A file of everything `seen` lacks.
    fn file(&self, seen: &VersionVector) -> Vec<u8> {
        let mut bytes = Vec::new();
        bundle::write_bundle(&self.conn, self.id, seen, &mut bytes).unwrap();
        bytes
    }

    fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        let parsed = bundle::read_bundle(bytes).map_err(|err| format!("{err:?}"))?;
        bundle::apply_bundle(&mut self.conn, &parsed, now_ms())
            .map(|_| ())
            .map_err(|err| format!("{err:?}"))
    }

    /// Every synced table, row by row, sorted — devices insert in different
    /// orders. `sync_peer` is left out: the joining device has written its own
    /// row, which the others have not heard of.
    fn tables(&self) -> Vec<(String, Vec<String>)> {
        let names: Vec<String> = self
            .conn
            .prepare(
                "SELECT table_name FROM temp.sync_shape
                 WHERE role <> 'local' AND table_name <> 'sync_peer' ORDER BY table_name",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        names
            .into_iter()
            .map(|table| {
                let mut stmt = self
                    .conn
                    .prepare(&format!("SELECT * FROM {table}"))
                    .unwrap();
                let width = stmt.column_count();
                let mut rows: Vec<String> = stmt
                    .query_map([], |row| {
                        Ok((0..width)
                            .map(|index| format!("{:?}", row.get_ref(index).unwrap()))
                            .collect::<Vec<_>>()
                            .join("|"))
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                rows.sort();
                (table, rows)
            })
            .collect()
    }

    /// The review queue, as each device derives it from its log.
    fn queue(&self) -> Vec<String> {
        self.conn
            .prepare(
                "SELECT root_table || '/' || root_id || ' ' || live_device || '/' || live_seq
                        || ' ' || other_device || '/' || other_seq
                 FROM sync_conflict ORDER BY 1",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    /// The records in a removed book.
    fn strays(&self) -> Vec<String> {
        let mut ids: Vec<String> = repo::list_stray_records(&self.conn, CORE_ROW_CAPTIONS)
            .unwrap()
            .into_iter()
            .flat_map(|book| book.records.into_iter().map(|record| record.id))
            .collect();
        ids.sort();
        ids
    }

    /// The log, as a list of change sets and the rows each names.
    fn log(&self) -> Vec<String> {
        self.conn
            .prepare(
                "SELECT origin_device || '/' || origin_seq || ' ' || entity_table || ' ' || entity_id
                 FROM record_change ORDER BY 1",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    /// What every device must agree on.
    fn picture(&self) -> Picture {
        Picture {
            tables: self.tables(),
            queue: self.queue(),
            strays: self.strays(),
        }
    }
}

/// What a device holds that another must hold too.
#[derive(Debug, PartialEq, Eq)]
struct Picture {
    tables: Vec<(String, Vec<String>)>,
    queue: Vec<String>,
    strays: Vec<String>,
}

/// Two of the devices at once, one to read and one to write.
fn pair(devices: &mut [Device], from: usize, to: usize) -> (&Device, &mut Device) {
    if from < to {
        let (left, right) = devices.split_at_mut(to);
        (&left[from], &mut right[0])
    } else {
        let (left, right) = devices.split_at_mut(from);
        (&right[0], &mut left[to])
    }
}

/// A farm on the laptop with three plots, a book and its twin — the same
/// campaign opened on another date, as two devices do — and three sowings,
/// carried to both phones.
struct Start {
    devices: Vec<Device>,
    group: String,
    plots: [String; 3],
    books: [String; 2],
    sowings: Vec<String>,
}

fn start() -> Start {
    let mut devices = vec![
        Device::new(DEVICES[0]),
        Device::new(DEVICES[1]),
        Device::new(DEVICES[2]),
    ];
    let group = terrazgo_core::sync::ensure_sync_group(&devices[0].conn).unwrap();
    for phone in &devices[1..] {
        terrazgo_core::sync::join_sync_group(&phone.conn, &group).unwrap();
    }
    let laptop = &mut devices[0].conn;
    let farm = repo::insert_farm(laptop, new_farm("Los Llanos"), None).unwrap();
    let plots = ["El Prado", "La Loma", "El Cerro"].map(|name| {
        repo::insert_plot(laptop, new_plot(&farm.id, name), None)
            .unwrap()
            .id
    });
    let book = repo::insert_season(laptop, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let twin = repo::insert_season(
        laptop,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-10-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap();
    let mut sowings = Vec::new();
    for (index, season) in [&book.id, &book.id, &twin.id].into_iter().enumerate() {
        let sowing = repo::insert_sowing_record(
            laptop,
            NewSowingRecord {
                season_id: season.clone(),
                farm_id: farm.id.clone(),
                kind_code: "sowing".into(),
                sown_on: "2026-04-10".into(),
                sowing_end_date: None,
                flooded_on: None,
                seed_quantity_kg: Some(180.0),
                notes: None,
                plots: vec![NewSowingPlot {
                    plot_id: plots[index].clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
        sowings.push(sowing.record.id);
    }
    let whole = devices[0].file(&VersionVector::default());
    for phone in &mut devices[1..] {
        phone.apply(&whole).unwrap();
    }
    Start {
        devices,
        group,
        plots,
        books: [book.id, twin.id],
        sowings,
    }
}

/// One random history, then a full exchange and a device joining. `Err`
/// carries what went wrong and every act that led there.
fn history(seed: u64, steps: usize) -> Result<(), String> {
    let mut choose = Choices::from_seed(seed);
    let Start {
        mut devices,
        group,
        plots,
        books,
        sowings,
    } = start();
    let today = today_utc();
    let a_month_on = add_days(&today, repo::REMOVED_BOOK_DAYS + 1).unwrap();
    let mut acts: Vec<String> = Vec::new();
    let failed =
        |what: String, acts: &[String]| format!("seed {seed}: {what}\n{}", acts.join("\n"));

    for _ in 0..steps {
        let who = choose.below(3);
        let conn = &mut devices[who].conn;
        match choose.below(100) {
            // Correct a sowing: its note and which plots it covers.
            0..=29 => {
                let sowing = &sowings[choose.below(sowings.len())];
                let note = ["a", "b", ""][choose.below(3)];
                let mask = 1 + choose.below(7);
                let covered: Vec<&str> = (0..3)
                    .filter(|bit| mask & (1 << bit) != 0)
                    .map(|bit| plots[bit].as_str())
                    .collect();
                let note = (!note.is_empty()).then_some(note);
                let done =
                    repo::update_sowing_record(conn, sowing, sowing_state(note, &covered), None);
                acts.push(format!("{who} corrects a sowing: {:?}", done.err()));
            }
            30..=35 => {
                let sowing = &sowings[choose.below(sowings.len())];
                let done = repo::soft_delete_sowing_record(conn, sowing, None);
                acts.push(format!("{who} removes a sowing: {:?}", done.err()));
            }
            36..=41 => {
                let book = &books[choose.below(2)];
                let done = repo::delete_book(conn, book, &[], None);
                acts.push(format!(
                    "{who} deletes book {}: {:?}",
                    choose_name(book, &books),
                    done.err()
                ));
            }
            42..=47 => {
                let book = &books[choose.below(2)];
                let done = repo::restore_book(conn, book, &today, None);
                acts.push(format!(
                    "{who} brings back book {}: {:?}",
                    choose_name(book, &books),
                    done.map(|restored| restored.records)
                ));
            }
            48..=51 => {
                let kept = choose.below(2);
                let done = repo::merge_books(conn, &books[kept], &books[1 - kept], &[], None);
                acts.push(format!(
                    "{who} merges, keeping book {kept}: {:?}",
                    done.err()
                ));
            }
            52..=54 => {
                let strays = repo::list_stray_records(conn, CORE_ROW_CAPTIONS).unwrap();
                if let Some(stray) = strays.first() {
                    let into = stray
                        .suggested
                        .as_ref()
                        .map_or_else(|| books[0].clone(), |book| book.id.clone());
                    let done = repo::move_stray_records(conn, &stray.season.id, &into, None);
                    acts.push(format!(
                        "{who} moves records out of a removed book: {:?}",
                        done.err()
                    ));
                }
            }
            // Resolve whatever waits, keeping any of its versions. A
            // resolution has nothing to refuse here, so a failure is a finding.
            55..=64 => {
                let waiting: Vec<(String, String)> = conn
                    .prepare("SELECT DISTINCT root_table, root_id FROM sync_conflict ORDER BY 1, 2")
                    .unwrap()
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                if !waiting.is_empty() {
                    let (table, id) = &waiting[choose.below(waiting.len())];
                    let versions = heads(conn, table, id).unwrap();
                    let kept = &versions[choose.below(versions.len())];
                    if let Err(err) = resolve(conn, table, id, &kept.device, kept.seq, None) {
                        return Err(failed(
                            format!("{who} could not resolve {table}: {err:?}"),
                            &acts,
                        ));
                    }
                    acts.push(format!(
                        "{who} resolves a {table}, keeping {}/{}",
                        kept.device, kept.seq
                    ));
                }
            }
            // Erase what is due, as if a month had passed for this device
            // alone — racing restores, resolutions and late writes elsewhere.
            65..=68 => {
                let erased = repo::purge_due(conn, &a_month_on, None)
                    .map_err(|err| failed(format!("{who} could not purge: {err:?}"), &acts))?;
                acts.push(format!("{who} purges: {} erased", erased.registers));
            }
            // A file to another device — whole, or trimmed to what it holds.
            // Nothing in these histories should ever be refused.
            _ => {
                let to = (who + 1 + choose.below(2)) % 3;
                let trimmed = choose.below(2) == 0;
                let (from, receiver) = pair(&mut devices, who, to);
                let seen = if trimmed {
                    bundle::seen_by(&receiver.conn).unwrap()
                } else {
                    VersionVector::default()
                };
                let bytes = from.file(&seen);
                let kind = if trimmed { "trimmed" } else { "whole" };
                if let Err(err) = receiver.apply(&bytes) {
                    return Err(failed(
                        format!("{to} refused {who}'s {kind} file: {err}"),
                        &acts,
                    ));
                }
                acts.push(format!("{who} sends {to} a {kind} file"));
            }
        }
    }

    // Everything everywhere: two rounds, so what one device learns in the
    // first reaches the others in the second; then twice more with every
    // device purging first, so what is due goes everywhere.
    for round in 0..4 {
        if round >= 2 {
            for (who, device) in devices.iter_mut().enumerate() {
                repo::purge_due(&mut device.conn, &a_month_on, None)
                    .map_err(|err| failed(format!("the final purge on {who}: {err:?}"), &acts))?;
            }
        }
        for from in 0..3 {
            for to in (0..3).filter(|to| *to != from) {
                let (sender, receiver) = pair(&mut devices, from, to);
                let bytes = sender.file(&VersionVector::default());
                if let Err(err) = receiver.apply(&bytes) {
                    return Err(failed(
                        format!("the final exchange: {to} refused {from}: {err}"),
                        &acts,
                    ));
                }
            }
        }
    }
    let mut joining = Device::new(DEVICES[3]);
    terrazgo_core::sync::join_sync_group(&joining.conn, &group).unwrap();
    joining
        .apply(&devices[0].file(&VersionVector::default()))
        .map_err(|err| failed(format!("a device joining was refused: {err}"), &acts))?;

    let expected = joining.picture();
    for (index, device) in devices.iter().enumerate() {
        let held = device.picture();
        if held != expected {
            let mut differences = Vec::new();
            for ((table, rows), (_, want)) in held.tables.iter().zip(&expected.tables) {
                if rows != want {
                    differences.push(format!("{table}:\n  holds {rows:?}\n  fresh {want:?}"));
                }
            }
            if held.queue != expected.queue {
                differences.push(format!(
                    "queue: holds {:?}, fresh {:?}",
                    held.queue, expected.queue
                ));
            }
            if held.strays != expected.strays {
                differences.push(format!(
                    "strays: holds {:?}, fresh {:?}",
                    held.strays, expected.strays
                ));
            }
            return Err(failed(
                format!(
                    "device {index} differs from a fresh replay:\n{}",
                    differences.join("\n")
                ),
                &acts,
            ));
        }
        if device.log() != devices[0].log() {
            return Err(failed(
                format!("device {index} holds a different log"),
                &acts,
            ));
        }
    }
    Ok(())
}

/// Which of the two books, as an act names it.
fn choose_name(book: &str, books: &[String; 2]) -> usize {
    usize::from(book != books[0])
}

/// Run every seed of `seeds`, and fail naming the first that went wrong and
/// how many did.
fn sweep(seeds: RangeInclusive<u64>, steps: usize) {
    let mut failures: Vec<(u64, String)> = Vec::new();
    for seed in seeds.clone() {
        if let Err(why) = history(seed, steps) {
            failures.push((seed, why));
        }
    }
    if let Some((_, first)) = failures.first() {
        let failed: Vec<u64> = failures.iter().map(|(seed, _)| *seed).collect();
        panic!(
            "{} of {} histories did not converge (seeds {failed:?}); the first:\n{first}",
            failures.len(),
            seeds.count()
        );
    }
}

// Four ranges, so the test runner spreads them across threads.

#[test]
fn random_histories_converge_seeds_1_to_8() {
    sweep(1..=8, STEPS);
}

#[test]
fn random_histories_converge_seeds_9_to_16() {
    sweep(9..=16, STEPS);
}

#[test]
fn random_histories_converge_seeds_17_to_24() {
    sweep(17..=24, STEPS);
}

#[test]
fn random_histories_converge_seeds_25_to_32() {
    sweep(25..=32, STEPS);
}

/// The wide sweep, for release runs and after a change to the merge:
/// `cargo test -p terrazgo-core --release --test random_histories -- --ignored`.
#[test]
#[ignore = "a wide sweep, minutes long; run after changing the merge layer"]
fn a_thousand_longer_histories_converge() {
    sweep(1..=1_000, 60);
}
