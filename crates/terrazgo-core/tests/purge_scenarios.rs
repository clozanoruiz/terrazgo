// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The purge, across a laptop and three phones (docs/sync.md → How it is
//! tested — a laptop and three phones, slice 11).
//!
//! **The devices**: a laptop L, where the group's files meet, and phones P, Q
//! and R. **The transport**: real bundle files through `write_bundle` and
//! `apply_bundle`, so the completeness and identity checks run as in the app.
//! Every situation runs twice — with files carrying the whole log, and with the
//! laptop's one reply to the phones trimmed to what all of them had seen, the
//! session rule — and with the laptop writing something unrelated after each
//! round: a whole-log file re-sends history a purge took away, and a trimmed
//! one does not, so each can hide a divergence the other shows.
//!
//! **Each device purges as the app does**: when it starts, and after every
//! file it imports, by its own calendar. A month passing is every device's
//! calendar moving on.
//!
//! **What every situation ends in**: the same synced tables on every device,
//! the same review queue and records in a removed book, the same LOG — the
//! purge leaves every device holding one history, not only one set of tables —
//! and no conflict nobody caused.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use common::{new_farm, new_plot, new_season, sowing_state};
use rusqlite::Connection;
use terrazgo_core::bundle;
use terrazgo_core::date::{add_days, now_ms, today_utc};
use terrazgo_core::merge::{CORE_ROW_CAPTIONS, heads, resolve};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_core::sync::VersionVector;

const L: &str = "0192f3a4-0000-7000-8000-00000000000a";
const P: &str = "0192f3a4-0000-7000-8000-00000000000b";
const Q: &str = "0192f3a4-0000-7000-8000-00000000000c";
const R: &str = "0192f3a4-0000-7000-8000-00000000000d";
/// A phone that joins once everything else has happened.
const N: &str = "0192f3a4-0000-7000-8000-00000000000e";

/// How files travel in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Files {
    /// Every file carries its sender's whole log.
    Whole,
    /// A phone sends what the laptop lacks; the laptop's one reply answers
    /// every file it imported in the round.
    Trimmed,
}

const BOTH: [Files; 2] = [Files::Whole, Files::Trimmed];

/// A day by which every removal made today is due: the day after the last
/// day the fold offers it back.
fn a_month_on() -> String {
    add_days(&today_utc(), repo::REMOVED_BOOK_DAYS + 1).unwrap()
}

struct Device {
    conn: Connection,
    id: &'static str,
    /// Its calendar. Moved on to make a month pass, or held back for a
    /// device whose clock is behind.
    today: String,
    /// What it erased since the test last asked.
    purged: usize,
    /// The changes its imports discarded since the test last asked, by the
    /// device that made them.
    discarded: Vec<(String, usize)>,
}

impl Device {
    fn new(id: &'static str) -> Self {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        repo::register_this_device(&mut conn, None).unwrap();
        Device {
            conn,
            id,
            today: today_utc(),
            purged: 0,
            discarded: Vec::new(),
        }
    }

    fn file(&self, seen: &VersionVector) -> Vec<u8> {
        let mut bytes = Vec::new();
        bundle::write_bundle(&self.conn, self.id, seen, &mut bytes).unwrap();
        bytes
    }

    /// A file for `to`: the whole log, or what `to` lacks.
    fn file_for(&self, to: &Device, files: Files) -> Vec<u8> {
        match files {
            Files::Whole => self.file(&VersionVector::default()),
            Files::Trimmed => self.file(&bundle::seen_by(&to.conn).unwrap()),
        }
    }

    /// Import a file, then purge what is due — as the app does after every
    /// import.
    fn apply(&mut self, bytes: &[u8]) -> bundle::ImportSummary {
        let parsed = bundle::read_bundle(bytes).unwrap();
        let summary = bundle::apply_bundle(&mut self.conn, &parsed, now_ms())
            .unwrap_or_else(|refusal| panic!("{} refused a file: {refusal:?}", self.id));
        self.purged += summary.purged.registers;
        for change in &summary.discarded {
            self.discarded.push((change.device.clone(), change.changes));
        }
        self.start();
        summary
    }

    /// What the app does when it starts: purge what is due by this device's
    /// calendar.
    fn start(&mut self) -> usize {
        let today = self.today.clone();
        let erased = repo::purge_due(&mut self.conn, &today, None)
            .unwrap()
            .registers;
        self.purged += erased;
        erased
    }

    fn row_exists(&self, table: &str, id: &str) -> bool {
        self.conn
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
                [id],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn removed(&self, table: &str, id: &str) -> bool {
        self.conn
            .query_row(
                &format!("SELECT deleted_at IS NOT NULL FROM {table} WHERE id = ?1"),
                [id],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// Whether the log holds anything of the register.
    fn logged(&self, table: &str, id: &str) -> bool {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM record_change WHERE root_table = ?1 AND root_id = ?2)",
                [table, id],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn queue(&self) -> Vec<(String, String)> {
        self.conn
            .prepare("SELECT root_table, root_id FROM sync_conflict ORDER BY root_table, root_id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn strays(&self) -> Vec<String> {
        let mut ids: Vec<String> = repo::list_stray_records(&self.conn, CORE_ROW_CAPTIONS)
            .unwrap()
            .into_iter()
            .flat_map(|book| book.records.into_iter().map(|record| record.id))
            .collect();
        ids.sort();
        ids
    }

    /// The log as a set of rows, each named everywhere alike.
    fn log(&self) -> BTreeSet<(String, i64, String, String)> {
        self.conn
            .prepare("SELECT origin_device, origin_seq, entity_table, entity_id FROM record_change")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    /// Every synced table, row by row, every column — sorted, since devices
    /// insert in different orders.
    fn tables(&self) -> Vec<(String, Vec<String>)> {
        let names: Vec<String> = self
            .conn
            .prepare(
                "SELECT table_name FROM temp.sync_shape WHERE role <> 'local' ORDER BY table_name",
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
                let columns = stmt.column_count();
                let mut rows: Vec<String> = stmt
                    .query_map([], |row| {
                        Ok((0..columns)
                            .map(|index| {
                                format!(
                                    "{:?}",
                                    row.get::<_, rusqlite::types::Value>(index).unwrap()
                                )
                            })
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

    /// How many versions a register has.
    fn versions(&self, table: &str, id: &str) -> usize {
        heads(&self.conn, table, id).unwrap().len()
    }

    /// The highest change-set number this device has written.
    fn own_last_seq(&self) -> i64 {
        bundle::seen_by(&self.conn).unwrap().get(self.id)
    }
}

/// A copy of a device's database, as a backup is one: written with
/// `VACUUM INTO`, and kept as a file until it is opened.
struct Copy(std::path::PathBuf);

impl Copy {
    fn of(device: &Device) -> Self {
        let path = std::env::temp_dir().join(format!("terrazgo-purge-{}.db", uuid::Uuid::now_v7()));
        device
            .conn
            .execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
        Copy(path)
    }

    /// Open it on another phone, which — as a backup import does — is a new
    /// replica with an id of its own, taking over from the one it was copied
    /// from.
    fn open_as(&self, id: &'static str, copied_from: &str) -> Device {
        let mut conn = Connection::open(&self.0).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        terrazgo_core::db::harden(&conn).unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        terrazgo_core::sync::install_shape(&conn, &[terrazgo_core::sync::CORE_SYNC_SHAPE]).unwrap();
        repo::register_this_device(&mut conn, None).unwrap();
        repo::succeed_sync_peer(&mut conn, copied_from, None).unwrap();
        Device {
            conn,
            id,
            today: a_month_on(),
            purged: 0,
            discarded: Vec::new(),
        }
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A laptop and three phones in the laptop's group, all holding one farm with
/// a book of two sowings — the first on the book's crop — and a twin of the
/// book.
struct Group {
    l: Device,
    p: Device,
    q: Device,
    r: Device,
    farm: String,
    plot: String,
    book: String,
    sowings: [String; 2],
    crop: String,
    unrelated: usize,
}

fn group() -> Group {
    let mut l = Device::new(L);
    let p = Device::new(P);
    let q = Device::new(Q);
    let r = Device::new(R);
    let group_id = terrazgo_core::sync::ensure_sync_group(&l.conn).unwrap();
    for phone in [&p, &q, &r] {
        terrazgo_core::sync::join_sync_group(&phone.conn, &group_id).unwrap();
    }
    let farm = repo::insert_farm(&mut l.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut l.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let book =
        repo::insert_season(&mut l.conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut l.conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: book.id.clone(),
            species_name: "Trigo blando".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(2.0),
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap()
    .id;
    let sowings = [
        sow(&mut l, &farm.id, &plot.id, &book.id, Some(&crop), "one"),
        sow(&mut l, &farm.id, &plot.id, &book.id, None, "two"),
    ];
    let mut g = Group {
        l,
        p,
        q,
        r,
        farm: farm.id,
        plot: plot.id,
        book: book.id,
        sowings,
        crop,
        unrelated: 0,
    };
    // Everyone starts from the same history, and knows that everyone does.
    g.everyone(Files::Whole);
    g
}

fn sow(
    device: &mut Device,
    farm: &str,
    plot: &str,
    book: &str,
    crop: Option<&str>,
    notes: &str,
) -> String {
    repo::insert_sowing_record(
        &mut device.conn,
        NewSowingRecord {
            season_id: book.into(),
            farm_id: farm.into(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: Some(notes.into()),
            plots: vec![NewSowingPlot {
                plot_id: plot.into(),
                crop_id: crop.map(str::to_owned),
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

impl Group {
    fn device(&mut self, who: char) -> &mut Device {
        match who {
            'L' => &mut self.l,
            'P' => &mut self.p,
            'Q' => &mut self.q,
            _ => &mut self.r,
        }
    }

    fn all(&self) -> [&Device; 4] {
        [&self.l, &self.p, &self.q, &self.r]
    }

    /// One round: each named phone sends to the laptop, the laptop writes
    /// something unrelated, and its one reply goes back to each.
    fn round(&mut self, files: Files, phones: &str) {
        let mut manifests = Vec::new();
        for who in phones.chars() {
            let file = {
                let phone = match who {
                    'P' => &self.p,
                    'Q' => &self.q,
                    _ => &self.r,
                };
                phone.file_for(&self.l, files)
            };
            manifests.push(self.l.apply(&file).peer_seen);
        }
        self.unrelated += 1;
        repo::insert_plot(
            &mut self.l.conn,
            new_plot(&self.farm, &format!("Unrelated {}", self.unrelated)),
            None,
        )
        .unwrap();
        let reply = match files {
            Files::Whole => self.l.file(&VersionVector::default()),
            Files::Trimmed => self.l.file(&VersionVector::common_ancestor_of(&manifests)),
        };
        for who in phones.chars() {
            self.device(who).apply(&reply);
        }
    }

    /// Two rounds with every phone: enough for whatever was written to reach
    /// every device, and for every device to hear that every other holds it.
    fn everyone(&mut self, files: Files) {
        self.round(files, "PQR");
        self.round(files, "PQR");
    }

    /// Every device's calendar moves on a month.
    fn month_later(&mut self) {
        let later = a_month_on();
        for who in ['L', 'P', 'Q', 'R'] {
            self.device(who).today = later.clone();
        }
    }

    /// Everyone meets until nothing is left to move, R included.
    fn finish(&mut self, files: Files) {
        self.everyone(files);
        self.everyone(files);
    }

    fn assert_converged(&self, situation: &str) {
        let tables = self.l.tables();
        // The control: the comparison reads the tables this file writes to,
        // so it cannot pass by comparing nothing with nothing.
        for written in ["farm", "plot", "sync_peer"] {
            assert!(
                tables
                    .iter()
                    .any(|(table, rows)| table == written && !rows.is_empty()),
                "{situation}: the comparison must read {written}"
            );
        }
        let log = self.l.log();
        for device in [&self.p, &self.q, &self.r] {
            assert_eq!(
                device.tables(),
                tables,
                "{situation}: {} holds different tables from the laptop",
                device.id
            );
            assert_eq!(
                device.log(),
                log,
                "{situation}: {} holds a different log from the laptop",
                device.id
            );
            assert_eq!(device.queue(), self.l.queue(), "{situation}: {}", device.id);
            assert_eq!(
                device.strays(),
                self.l.strays(),
                "{situation}: {}",
                device.id
            );
        }
    }

    fn delete_book(&mut self, who: char) -> usize {
        let book = self.book.clone();
        repo::delete_book(&mut self.device(who).conn, &book, &[], None).unwrap()
    }

    fn restore_book(&mut self, who: char) -> repo::RestoredBook {
        let book = self.book.clone();
        let device = self.device(who);
        let today = device.today.clone();
        repo::restore_book(&mut device.conn, &book, &today, None).unwrap()
    }

    /// What a purge leaves of the book on `device`: its sowings gone from the
    /// tables and the log, and the book and its crop kept, removed — records
    /// point at both, and a version arriving later may too (docs/sync.md →
    /// What can go: what nothing else can point at).
    fn book_gone(&self, device: &Device) -> bool {
        let gone =
            |table: &str, id: &str| !device.row_exists(table, id) && !device.logged(table, id);
        let kept =
            |table: &str, id: &str| device.row_exists(table, id) && device.removed(table, id);
        kept("season", &self.book)
            && kept("crop", &self.crop)
            && self
                .sowings
                .iter()
                .all(|sowing| gone("sowing_record", sowing))
    }

    /// Whether the book's sowings are gone from `device`'s tables and log —
    /// what the purge erased, however a late restore tried to bring it back.
    fn sowings_gone(&self, device: &Device) -> bool {
        self.sowings.iter().all(|sowing| {
            !device.row_exists("sowing_record", sowing) && !device.logged("sowing_record", sowing)
        })
    }

    /// Keep, on `who`, the version of `table`/`id` that `writer` wrote.
    fn keep_version_of(&mut self, who: char, table: &str, id: &str, writer: &str) {
        let conn = &mut self.device(who).conn;
        let current = heads(conn, table, id).unwrap();
        let kept = current
            .iter()
            .find(|head| head.device == writer)
            .unwrap_or_else(|| panic!("{writer} wrote no version of {table} {id}"))
            .clone();
        resolve(conn, table, id, &kept.device, kept.seq, None).unwrap();
    }

    fn correct_first_sowing(&mut self, who: char, notes: &str) {
        self.correct_sowing(who, 0, notes);
    }

    /// Correct a sowing's notes, keeping its plot. The second sowing names no
    /// crop, so its plot row is left exactly as it was.
    fn correct_sowing(&mut self, who: char, which: usize, notes: &str) {
        let (sowing, plot) = (self.sowings[which].clone(), self.plot.clone());
        repo::update_sowing_record(
            &mut self.device(who).conn,
            &sowing,
            sowing_state(Some(notes), &[&plot]),
            None,
        )
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// The matrix: the six failures the naive purge showed
// ---------------------------------------------------------------------------

#[test]
fn s1_a_device_joins_from_a_laptop_that_has_purged() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        assert!(g.l.start() > 0, "{files:?}: due, and erased");
        assert!(g.book_gone(&g.l));

        // A new phone, holding nothing, joins from the laptop's whole log —
        // whose numbers now have holes where the book was.
        let mut n = Device::new(N);
        let group_id = terrazgo_core::sync::sync_group(&g.l.conn).unwrap().unwrap();
        terrazgo_core::sync::join_sync_group(&n.conn, &group_id).unwrap();
        n.apply(&g.l.file(&VersionVector::default()));
        let from_n = n.file_for(&g.l, files);
        g.l.apply(&from_n);
        assert_eq!(n.tables(), g.l.tables(), "{files:?}");
        assert_eq!(n.log(), g.l.log(), "{files:?}");

        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
        }
        g.assert_converged(&format!("s1 {files:?}"));
    }
}

#[test]
fn s1b_a_device_whose_own_last_change_set_was_purged_never_reuses_its_number() {
    for files in BOTH {
        let mut g = group();
        // P's deletion is the last thing P ever wrote.
        g.delete_book('P');
        let deletion = g.p.own_last_seq();
        g.everyone(files);
        g.month_later();
        g.l.start();
        // The laptop's reply carries the purge to P, which erases its own
        // last change set.
        g.round(files, "P");
        assert!(g.book_gone(&g.p));
        repo::insert_plot(&mut g.p.conn, new_plot(&g.farm, "Written after"), None).unwrap();
        assert!(
            g.p.own_last_seq() > deletion,
            "{files:?}: a number names one change set, for ever"
        );
        // Q still holds P's deletion and has not heard of the purge; P's new
        // change set reaches it with the purge, and neither is mistaken for
        // the other.
        g.finish(files);
        g.assert_converged(&format!("s1b {files:?}"));
    }
}

#[test]
fn s2_a_device_that_has_not_purged_cannot_bring_the_history_back() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.l.start();
        // P has not heard of the purge, and sends everything it holds.
        let everything = g.p.file(&VersionVector::default());
        g.l.apply(&everything);
        assert!(g.book_gone(&g.l), "{files:?}: the history stays erased");
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
        }
        g.assert_converged(&format!("s2 {files:?}"));
    }
}

#[test]
fn s3_a_correction_made_before_the_removal_arrived_holds_the_purge_back() {
    for files in BOTH {
        for keep_correction in [true, false] {
            let mut g = group();
            g.delete_book('L');
            g.correct_first_sowing('P', "corrected offline");
            g.everyone(files);
            g.month_later();
            g.finish(files);
            let corrected = g.sowings[0].clone();
            for device in g.all() {
                assert_eq!(
                    device.queue(),
                    vec![("sowing_record".to_owned(), corrected.clone())],
                    "{files:?}: waiting for a person, so nothing of it is erased"
                );
                assert!(device.row_exists("sowing_record", &corrected));
                assert!(device.row_exists("season", &g.book));
            }

            let writer = if keep_correction { P } else { L };
            if !keep_correction {
                // Kept today: the removal a person keeps is a removal made
                // today, and every calendar says today again.
                for who in ['L', 'P', 'Q', 'R'] {
                    g.device(who).today = today_utc();
                }
            }
            g.keep_version_of('L', "sowing_record", &corrected, writer);
            g.finish(files);
            for device in g.all() {
                assert!(device.queue().is_empty());
                if keep_correction {
                    // Live in a removed book: the book cannot go, the rest of
                    // what went with it can.
                    assert_eq!(device.strays(), vec![corrected.clone()]);
                    assert!(device.row_exists("season", &g.book));
                    assert!(!device.row_exists("sowing_record", &g.sowings[1]));
                } else {
                    // The kept removal is a removal made today, so it waits
                    // its own month.
                    assert!(device.removed("sowing_record", &corrected));
                }
            }
            g.assert_converged(&format!("s3 {files:?} keep_correction={keep_correction}"));
            if !keep_correction {
                g.month_later();
                g.finish(files);
                for device in g.all() {
                    assert!(
                        g.book_gone(device),
                        "{files:?}: a month on, everything goes"
                    );
                }
                g.assert_converged(&format!("s3 {files:?} a month on"));
            }
        }
    }
}

#[test]
fn s4_a_book_brought_back_while_the_purge_ran_comes_back_without_what_went() {
    // P's calendar is behind: for it the book can still come back, for the
    // laptop it is due. The purge wins (docs/sync.md → An import meets the
    // purge): the book and its crop, which are never erased, come back; the
    // sowings, erased, do not — and the laptop says it discarded P's changes.
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.p.today = today_utc();
        g.l.start();
        assert!(g.book_gone(&g.l));
        assert_eq!(g.restore_book('P').records, 3, "{files:?}: the control");
        g.finish(files);
        for device in g.all() {
            assert!(
                !device.removed("season", &g.book),
                "{files:?}: {}",
                device.id
            );
            assert!(!device.removed("crop", &g.crop));
            assert!(g.sowings_gone(device), "{files:?}: {}", device.id);
            assert!(device.queue().is_empty());
        }
        assert!(
            g.l.discarded.iter().any(|(device, _)| device == P),
            "{files:?}: P's restore of what went is discarded, and said so"
        );
        g.assert_converged(&format!("s4 {files:?}"));
    }
}

#[test]
fn s5_the_same_campaign_opened_again_is_one_book_without_a_conflict() {
    // Opened on the laptop, which purged the book's records, and — in the
    // other run — on R, which has not had the purge yet. The book was renamed
    // on P first, so the version that deleted it names P as well as the
    // laptop. The book itself is never erased, so opening it again is written
    // on top of its whole history, on either device. (A register keyed by what
    // it is and erased for good is stamped on top of its marker: a declaration
    // made again, in module-phytosanitary's `book_delete.rs`.)
    for files in BOTH {
        for opened_on in ['L', 'R'] {
            let mut g = group();
            let book = g.book.clone();
            let season = repo::get_season(&g.p.conn, &book).unwrap();
            repo::update_season(
                &mut g.p.conn,
                &book,
                UpdateSeason {
                    starts_on: season.starts_on,
                    ends_on: season.ends_on,
                    custom_label: Some("Campaña del trigo".into()),
                },
                None,
            )
            .unwrap();
            g.everyone(files);
            g.delete_book('L');
            g.everyone(files);
            g.month_later();
            g.l.start();
            let farm = g.farm.clone();
            let again = repo::insert_season(
                &mut g.device(opened_on).conn,
                new_season(&farm, 2026, "2025/2026"),
                None,
            )
            .unwrap();
            assert_eq!(again.id, g.book, "the campaign's id is derived");
            g.finish(files);
            for device in g.all() {
                assert!(!device.removed("season", &g.book), "{files:?} {opened_on}");
                assert!(
                    device.queue().is_empty(),
                    "{files:?} {opened_on}: one book, newer than its deletion, on {}",
                    device.id
                );
                assert!(!device.row_exists("sowing_record", &g.sowings[0]));
            }
            g.assert_converged(&format!("s5 {files:?} opened on {opened_on}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Knowledge that runs behind
// ---------------------------------------------------------------------------

#[test]
fn s7_a_phone_purges_on_what_it_heard_of_another_through_the_laptop() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        // P has never exchanged a file with Q.
        assert!(
            g.p.start() > 0,
            "{files:?}: P knows, through L, that Q holds it"
        );
        assert!(g.book_gone(&g.p));
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device));
        }
        g.assert_converged(&format!("s7 {files:?}"));
    }
}

#[test]
fn s8_two_devices_purge_at_once() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        assert!(g.l.start() > 0);
        assert!(g.p.start() > 0);
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device));
            assert!(
                device.queue().is_empty(),
                "{files:?}: two markers never conflict"
            );
        }
        g.assert_converged(&format!("s8 {files:?}"));
    }
}

#[test]
fn s9_a_record_brought_back_while_another_phone_purges_it_goes_with_the_purge() {
    // Q's calendar is behind and it brings the book back; P purges it on what
    // it heard through the laptop. Q's restore never saw the purge, so what
    // the purge erased stays erased — on Q too, once the purge reaches it.
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.q.today = today_utc();
        assert!(g.p.start() > 0);
        assert_eq!(g.restore_book('Q').records, 3, "{files:?}: the control");
        g.finish(files);
        for device in g.all() {
            assert!(
                !device.removed("season", &g.book),
                "{files:?}: {}",
                device.id
            );
            assert!(g.sowings_gone(device), "{files:?}: {}", device.id);
        }
        g.assert_converged(&format!("s9 {files:?}"));
    }
}

#[test]
fn s10_a_copy_restored_from_before_the_deletion_loses_the_book_too() {
    for files in BOTH {
        let mut g = group();
        // Q's database as it was, before anything was deleted.
        let before = Copy::of(&g.q);
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.l.start();
        g.finish(files);

        // The copy comes back on a phone, which a restore makes a new replica,
        // and meets the laptop — holding the book live, older than its removal.
        let mut restored = before.open_as(N, Q);
        assert!(restored.row_exists("sowing_record", &g.sowings[0]));
        let from_restored = restored.file_for(&g.l, files);
        g.l.apply(&from_restored);
        let reply = g.l.file_for(&restored, files);
        restored.apply(&reply);
        assert!(
            g.book_gone(&restored),
            "{files:?}: what it held is older than the removal"
        );
        assert!(
            restored.discarded.is_empty(),
            "nothing it held was a change"
        );
        g.finish(files);
        let reply = g.l.file_for(&restored, files);
        restored.apply(&reply);
        assert_eq!(restored.tables(), g.l.tables(), "{files:?}");
        assert_eq!(restored.log(), g.l.log(), "{files:?}");
        g.assert_converged(&format!("s10 {files:?}"));
    }
}

#[test]
fn s11_a_phone_joins_after_the_purge_from_a_backup_copy() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.l.start();
        g.finish(files);

        // A copy of the laptop's database, opened as a new phone.
        let copy = Copy::of(&g.l);
        let mut joined = copy.open_as(N, L);
        let from_joined = joined.file_for(&g.l, files);
        g.l.apply(&from_joined);
        let reply = g.l.file_for(&joined, files);
        joined.apply(&reply);
        g.finish(files);
        let reply = g.l.file_for(&joined, files);
        joined.apply(&reply);
        assert_eq!(joined.tables(), g.l.tables(), "{files:?}");
        assert_eq!(joined.log(), g.l.log(), "{files:?}");
        g.assert_converged(&format!("s11 {files:?}"));
    }
}

#[test]
fn s12_a_retired_phone_comes_back_with_a_change_to_an_erased_record() {
    for files in BOTH {
        let mut g = group();
        // R, away, corrects a sowing and adds a plot; the laptop retires it.
        g.correct_first_sowing('R', "corrected on a phone nobody expects back");
        let farm = g.farm.clone();
        let unrelated =
            repo::insert_plot(&mut g.r.conn, new_plot(&farm, "Added on R"), None).unwrap();
        repo::rename_sync_peer(&mut g.l.conn, R, Some("Móvil de Rosa"), None).unwrap();
        repo::retire_sync_peer(&mut g.l.conn, R, true, None).unwrap();
        g.delete_book('L');
        g.round(files, "PQ");
        g.round(files, "PQ");
        g.month_later();
        assert!(
            g.l.start() > 0,
            "{files:?}: a retired phone is not waited for"
        );

        // R comes back.
        let from_r = g.r.file_for(&g.l, files);
        let summary = g.l.apply(&from_r);
        assert_eq!(
            summary.discarded[0].device_label.as_deref(),
            Some("Móvil de Rosa"),
            "{files:?}: named as people call it, for the message"
        );
        assert_eq!(
            g.l.discarded,
            vec![(R.to_owned(), 1)],
            "{files:?}: the correction is discarded, and the import says so"
        );
        assert!(
            g.l.row_exists("plot", &unrelated.id),
            "the rest of it applies"
        );
        assert!(g.book_gone(&g.l));
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
            assert!(device.row_exists("plot", &unrelated.id));
        }
        g.assert_converged(&format!("s12 {files:?}"));
    }
}

#[test]
fn s13_a_phone_not_yet_told_of_a_retirement_waits_for_the_retired_one() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.round(files, "PQ");
        g.round(files, "PQ");
        // R never had the deletion; the laptop retires it, and P has not been
        // told.
        repo::retire_sync_peer(&mut g.l.conn, R, true, None).unwrap();
        g.month_later();
        assert_eq!(g.p.start(), 0, "{files:?}: P still waits for R");
        assert!(g.l.start() > 0, "{files:?}: the laptop does not");
        g.round(files, "PQ");
        assert!(g.book_gone(&g.p), "{files:?}: told, P erases too");
        g.finish(files);
        g.assert_converged(&format!("s13 {files:?}"));
    }
}

#[test]
fn s14_a_removal_younger_than_thirty_days_is_never_erased() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.everyone(files);
        let last_day = add_days(&today_utc(), repo::REMOVED_BOOK_DAYS).unwrap();
        for who in ['L', 'P', 'Q', 'R'] {
            g.device(who).today = last_day.clone();
        }
        assert_eq!(g.l.start(), 0, "{files:?}: the fold still offers it back");
        g.finish(files);
        for device in g.all() {
            assert!(device.removed("season", &g.book));
        }
        g.month_later();
        assert!(g.l.start() > 0);
        g.finish(files);
        g.assert_converged(&format!("s14 {files:?}"));
    }
}

#[test]
fn s14b_a_removing_device_with_a_wrong_clock_moves_only_the_wait() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        // The laptop's clock was ten days ahead when it deleted the book.
        let ahead = add_days(&today_utc(), 10).unwrap();
        let seq = g.l.own_last_seq();
        g.l.conn
            .execute(
                "UPDATE record_change SET changed_at = ?1 || substr(changed_at, 11)
                 WHERE origin_device = ?2 AND origin_seq = ?3",
                rusqlite::params![ahead, L, seq],
            )
            .unwrap();
        g.everyone(files);
        g.month_later();
        assert_eq!(g.l.start(), 0, "{files:?}: due ten days later");
        let later = add_days(&a_month_on(), 10).unwrap();
        for who in ['L', 'P', 'Q', 'R'] {
            g.device(who).today = later.clone();
        }
        assert!(g.l.start() > 0);
        g.finish(files);
        g.assert_converged(&format!("s14b {files:?}"));
    }
}

#[test]
fn s15_a_book_deleted_with_its_records_goes_from_every_device() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('Q');
        g.everyone(files);
        g.month_later();
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
            assert!(
                device.row_exists("plot", &g.plot),
                "the plot is never erased"
            );
        }
        g.assert_converged(&format!("s15 {files:?}"));
    }
}

// ---------------------------------------------------------------------------
// Found reading the design against the engine (2026-10-02)
// ---------------------------------------------------------------------------

#[test]
fn s16_a_correction_kept_over_a_removal_where_the_purge_had_not_arrived_goes_with_it() {
    // R, retired, corrected the second sowing's notes while the laptop removed
    // the book. Q hears from R before it hears of the purge, and a person on Q
    // keeps R's correction — which would bring back a record the laptop has
    // erased. The choice never saw the purge, so it is discarded wherever the
    // purge is, and the record goes from Q too.
    for files in BOTH {
        let mut g = group();
        g.correct_sowing('R', 1, "kept by a person");
        repo::retire_sync_peer(&mut g.l.conn, R, true, None).unwrap();
        g.delete_book('L');
        g.round(files, "PQ");
        g.round(files, "PQ");
        g.month_later();
        g.q.today = today_utc();
        // R's file reaches Q directly, as a memory stick would carry it.
        let from_r = g.r.file(&VersionVector::default());
        g.q.apply(&from_r);
        let sowing = g.sowings[1].clone();
        assert_eq!(
            g.q.queue(),
            vec![("sowing_record".to_owned(), sowing.clone())],
            "{files:?}: the control"
        );
        g.l.start();
        assert!(g.book_gone(&g.l));
        g.keep_version_of('Q', "sowing_record", &sowing, R);
        g.finish(files);
        for device in g.all() {
            assert!(
                !device.row_exists("sowing_record", &sowing)
                    && !device.logged("sowing_record", &sowing),
                "{files:?}: gone on {}",
                device.id
            );
        }
        assert!(
            g.l.discarded.iter().any(|(device, _)| device == Q),
            "{files:?}: Q's choice is discarded, and said so"
        );
        g.assert_converged(&format!("s16 {files:?}"));
    }
}

#[test]
fn s17_a_late_change_that_reaches_the_laptop_first_never_stops_its_files() {
    // P purges; R, retired, reaches the laptop before P's purge does. The
    // laptop holds R's change until the purge arrives, then discards it — and
    // every device goes on taking the laptop's files.
    for files in BOTH {
        let mut g = group();
        g.correct_first_sowing('R', "a late change");
        repo::retire_sync_peer(&mut g.l.conn, R, true, None).unwrap();
        g.delete_book('L');
        g.round(files, "PQ");
        g.round(files, "PQ");
        // The laptop's calendar has not moved; P's has.
        g.p.today = a_month_on();
        assert!(g.p.start() > 0);
        let from_r = g.r.file_for(&g.l, files);
        g.l.apply(&from_r);
        assert!(
            !g.l.queue().is_empty(),
            "{files:?}: a conflict on the laptop, for now"
        );
        g.round(files, "PQ");
        assert!(g.book_gone(&g.l), "{files:?}: the purge reached the laptop");
        assert_eq!(g.l.discarded, vec![(R.to_owned(), 1)]);
        g.month_later();
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
        }
        g.assert_converged(&format!("s17 {files:?}"));
    }
}

#[test]
fn s19_a_book_brought_back_late_keeps_the_crop_removed_on_its_own() {
    // The crop was removed on its own before the book went, so bringing the
    // book back leaves it removed — and since crops are never erased, it is
    // there, removed, on every device, while the sowings the late restore
    // would have brought back went with the purge.
    for files in BOTH {
        let mut g = group();
        let crop = g.crop.clone();
        repo::soft_delete_crop(&mut g.l.conn, &crop, None).unwrap();
        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        g.p.today = today_utc();
        g.l.start();
        assert!(g.book_gone(&g.l));
        assert_eq!(g.restore_book('P').records, 2, "{files:?}: the two sowings");
        g.finish(files);
        for device in g.all() {
            assert!(g.sowings_gone(device), "{files:?}: {}", device.id);
            assert!(
                device.removed("crop", &g.crop),
                "{files:?}: the crop is there, still removed, on {}",
                device.id
            );
        }
        g.assert_converged(&format!("s19 {files:?}"));
    }
}

#[test]
fn s21_a_record_removed_on_its_own_goes_a_month_on() {
    for files in BOTH {
        let mut g = group();
        let gone = g.sowings[1].clone();
        repo::soft_delete_sowing_record(&mut g.q.conn, &gone, None).unwrap();
        g.everyone(files);
        g.month_later();
        g.finish(files);
        for device in g.all() {
            assert!(!device.row_exists("sowing_record", &gone), "{files:?}");
            assert!(!device.logged("sowing_record", &gone));
            assert!(!device.removed("season", &g.book), "the book stays");
            assert!(!device.removed("sowing_record", &g.sowings[0]));
            assert!(!device.removed("crop", &g.crop));
        }
        g.assert_converged(&format!("s21 {files:?}"));
    }
}

// ---------------------------------------------------------------------------
// Found building the screens (2026-10-02)
// ---------------------------------------------------------------------------

#[test]
fn s22_a_book_deleted_on_two_devices_apart_goes_from_every_device() {
    // L and P each delete the book before hearing of the other, while Q
    // removes the second sowing on its own: every register of the book ends
    // with two removals — the second sowing with three — that agree, so
    // nothing is listed, and each removal is a version of its own.
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.delete_book('P');
        let second = g.sowings[1].clone();
        repo::soft_delete_sowing_record(&mut g.q.conn, &second, None).unwrap();
        g.everyone(files);
        assert_eq!(g.l.versions("season", &g.book), 2, "{files:?}");
        assert_eq!(g.l.versions("sowing_record", &second), 3, "{files:?}");
        for device in g.all() {
            assert!(device.queue().is_empty(), "{files:?}: they agree");
        }
        g.month_later();
        g.finish(files);
        for device in g.all() {
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
            assert!(
                device.discarded.is_empty(),
                "{files:?}: each removal was history the marker had seen, on {}",
                device.id
            );
        }
        g.assert_converged(&format!("s22 {files:?}"));
    }
}

#[test]
fn s22b_each_of_two_removals_is_waited_for_everywhere() {
    // P deletes the book, then L, apart. Q hears only of L's deletion and,
    // its calendar still inside the thirty days, brings the book back. The
    // laptop, a month on, knows every phone holds its own deletion — but not
    // P's: erasing then would discard Q's restore, written on top of one
    // removal and not the other.
    for files in BOTH {
        let mut g = group();
        g.delete_book('P');
        g.delete_book('L');
        g.round(files, "QR");
        g.round(files, "QR");
        assert_eq!(g.restore_book('Q').records, 3, "{files:?}");
        g.l.today = a_month_on();
        g.round(files, "P");
        g.round(files, "P");
        assert_eq!(g.l.purged, 0, "{files:?}: Q and R lack P's deletion");
        g.round(files, "Q");
        g.finish(files);
        for device in g.all() {
            assert!(
                device.discarded.is_empty(),
                "{files:?}: Q's restore is nobody's late change, on {}",
                device.id
            );
            assert!(
                device
                    .queue()
                    .contains(&("season".to_owned(), g.book.clone())),
                "{files:?}: brought back against a deletion — for a person, on {}",
                device.id
            );
        }
        g.assert_converged(&format!("s22b {files:?}"));
    }
}

#[test]
fn s23_a_merged_book_whose_removed_crop_a_moved_sowing_names_goes_everywhere() {
    // The crop was removed while the first sowing still named it, and the book
    // was then merged into its twin. The crop goes with the sowing — on L, and
    // in the second run on P too, merging the same pair the same way at the
    // same time (P4 in Merging two books) — so nothing names the book that went.
    for files in BOTH {
        for both in [false, true] {
            let mut g = group();
            let twin = repo::insert_season(
                &mut g.l.conn,
                NewSeason {
                    farm_id: g.farm.clone(),
                    starts_on: "2025-09-15".into(),
                    ends_on: "2026-08-31".into(),
                    custom_label: Some("2025/2026 bis".into()),
                },
                None,
            )
            .unwrap()
            .id;
            let crop = g.crop.clone();
            repo::soft_delete_crop(&mut g.l.conn, &crop, None).unwrap();
            g.everyone(files);
            let book = g.book.clone();
            repo::merge_books(&mut g.l.conn, &twin, &book, &[], None).unwrap();
            if both {
                repo::merge_books(&mut g.p.conn, &twin, &book, &[], None).unwrap();
            }
            g.everyone(files);
            for device in g.all() {
                assert!(device.queue().is_empty(), "{files:?} {both}: {}", device.id);
            }
            g.month_later();
            g.finish(files);
            for device in g.all() {
                let situation = format!("{files:?} {both}: {}", device.id);
                assert!(device.removed("season", &g.book), "{situation}");
                let filed: String = device
                    .conn
                    .query_row("SELECT season_id FROM crop WHERE id = ?1", [&g.crop], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(filed, twin, "{situation}: the crop went with the sowing");
                assert!(device.removed("crop", &g.crop), "{situation}");
                assert!(
                    !device.removed("sowing_record", &g.sowings[0]),
                    "{situation}"
                );
                assert!(device.discarded.is_empty(), "{situation}");
            }
            g.assert_converged(&format!("s23 {files:?} {both}"));
        }
    }
}

#[test]
fn s24_a_phone_not_heard_from_records_into_a_book_deleted_and_purged() {
    // N joins from the laptop's file and sends nothing back, so no device
    // waits for it; it records into the book while the laptop deletes the
    // book and, a month on, purges what was in it. *Found by the audit
    // (2026-10-03)*: erased, the book N's record names made each device refuse
    // the other's files for good. Kept, the record arrives as a record in a
    // removed book, on every device (docs/sync.md → What can go).
    for files in BOTH {
        let mut g = group();
        let mut n = Device::new(N);
        let group_id = terrazgo_core::sync::sync_group(&g.l.conn).unwrap().unwrap();
        terrazgo_core::sync::join_sync_group(&n.conn, &group_id).unwrap();
        n.apply(&g.l.file(&VersionVector::default()));
        let (farm, plot, book) = (g.farm.clone(), g.plot.clone(), g.book.clone());
        let late = sow(&mut n, &farm, &plot, &book, None, "recorded on N");

        g.delete_book('L');
        g.everyone(files);
        g.month_later();
        assert!(g.l.start() > 0, "{files:?}: nobody waits for N");
        g.finish(files);

        // N's file at last, and the laptop's answer: neither is refused.
        n.today = a_month_on();
        let from_n = n.file_for(&g.l, files);
        g.l.apply(&from_n);
        g.finish(files);
        let reply = g.l.file_for(&n, files);
        n.apply(&reply);
        for device in g.all().into_iter().chain([&n]) {
            assert_eq!(
                device.strays(),
                vec![late.clone()],
                "{files:?}: a record in a removed book, on {}",
                device.id
            );
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
        }
        assert_eq!(n.tables(), g.l.tables(), "{files:?}");
        assert_eq!(n.log(), g.l.log(), "{files:?}");
        g.assert_converged(&format!("s24 {files:?}"));
    }
}

#[test]
fn s25_a_phone_that_holds_a_removal_edits_a_record_moved_out_of_that_book() {
    // P records into the book while L deletes it; everyone hears of both, so
    // P's record is in a removed book everywhere, and every device knows every
    // other holds the deletion. Q moves the record into another book; before
    // that reaches P, P corrects the record — still filed, on P, in the
    // removed book. *Found by the audit (2026-10-03)*: L, holding the move,
    // erased the book, and P's correction then named it, and its files were
    // refused for good. Kept, the correction is a version like any other.
    for files in BOTH {
        let mut g = group();
        let other =
            repo::insert_season(&mut g.l.conn, new_season(&g.farm, 2027, "2026/2027"), None)
                .unwrap()
                .id;
        g.everyone(files);
        let (farm, plot, book) = (g.farm.clone(), g.plot.clone(), g.book.clone());
        let late = sow(&mut g.p, &farm, &plot, &book, None, "recorded on P");
        g.delete_book('L');
        g.everyone(files);
        assert_eq!(g.l.strays(), vec![late.clone()], "{files:?}: the control");

        repo::move_stray_records(&mut g.q.conn, &book, &other, None).unwrap();
        g.round(files, "Q");
        // Later than the move, so the correction is the version that shows.
        std::thread::sleep(std::time::Duration::from_millis(2));
        repo::update_sowing_record(
            &mut g.p.conn,
            &late,
            sowing_state(Some("corrected on P"), &[&plot]),
            None,
        )
        .unwrap();
        g.month_later();
        assert!(g.l.start() > 0, "{files:?}: the book's own sowings go");
        g.round(files, "P");
        g.finish(files);
        for device in g.all() {
            assert!(
                device
                    .queue()
                    .contains(&("sowing_record".to_owned(), late.clone())),
                "{files:?}: the correction and the move, for a person, on {}",
                device.id
            );
            assert!(g.book_gone(device), "{files:?}: {}", device.id);
        }
        g.assert_converged(&format!("s25 {files:?}"));
    }
}

#[test]
fn s26_a_choice_made_where_the_purge_had_not_arrived_goes_with_it() {
    // N, not heard from, corrects a sowing onto a second plot just after P
    // deletes the book, then hears of the deletion and keeps it — a choice
    // that drops N's second plot. P, meanwhile,
    // brings the book back, and the laptop erases the sowing a month on, then
    // P edits it on top of its restore. *Found by the 10 000-history soak
    // (2026-10-04)*: the laptop discarded N's correction but kept N's choice,
    // written on top of the removal; undoing the choice when P's edit beat it
    // put back N's second plot, a row only the discarded correction ever held,
    // and the laptop refused P's file on `UNIQUE (sowing_record_id, plot_id)`.
    // The purge wins: the choice, the restore and the edit never saw it, so
    // they go wherever it is, and no device holds part of the sowing's history
    // (docs/sync.md → An import meets the purge).
    for files in BOTH {
        let mut g = group();
        let second = repo::insert_plot(&mut g.l.conn, new_plot(&g.farm, "La Loma"), None)
            .unwrap()
            .id;
        g.everyone(files);
        let mut n = Device::new(N);
        let group_id = terrazgo_core::sync::sync_group(&g.l.conn).unwrap().unwrap();
        terrazgo_core::sync::join_sync_group(&n.conn, &group_id).unwrap();
        n.apply(&g.l.file(&VersionVector::default()));
        let (sowing, plot) = (g.sowings[1].clone(), g.plot.clone());

        g.delete_book('P');
        g.everyone(files);
        // Later than the deletion, so N's correction is what N shows.
        std::thread::sleep(std::time::Duration::from_millis(2));
        repo::update_sowing_record(
            &mut n.conn,
            &sowing,
            sowing_state(Some("on two plots"), &[&plot, &second]),
            None,
        )
        .unwrap();
        g.restore_book('P');
        n.apply(&g.l.file(&VersionVector::default()));
        assert_eq!(
            n.versions("sowing_record", &sowing),
            2,
            "{files:?}: the control"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        let deletion = heads(&n.conn, "sowing_record", &sowing)
            .unwrap()
            .into_iter()
            .find(|head| head.device == P)
            .unwrap();
        resolve(&mut n.conn, "sowing_record", &sowing, P, deletion.seq, None).unwrap();

        g.month_later();
        assert!(g.l.start() > 0, "{files:?}: nobody waits for N");
        // P's restore reaches the laptop only by way of N.
        let from_p = g.p.file(&VersionVector::default());
        n.apply(&from_p);
        n.today = a_month_on();
        let from_n = n.file_for(&g.l, files);
        g.l.apply(&from_n);

        // Later than N's choice, so P's edit beats it.
        std::thread::sleep(std::time::Duration::from_millis(2));
        repo::update_sowing_record(
            &mut g.p.conn,
            &sowing,
            sowing_state(Some("moved on P"), &[&second]),
            None,
        )
        .unwrap();
        let from_p = g.p.file_for(&g.l, files);
        g.l.apply(&from_p);

        // Everyone meets; then N's file at last, and the laptop's answer.
        g.finish(files);
        let from_n = n.file_for(&g.l, files);
        g.l.apply(&from_n);
        g.finish(files);
        let reply = g.l.file_for(&n, files);
        n.apply(&reply);
        assert_eq!(n.tables(), g.l.tables(), "{files:?}");
        assert_eq!(n.log(), g.l.log(), "{files:?}");
        for device in g.all().into_iter().chain([&n]) {
            assert!(
                !device.row_exists("sowing_record", &sowing)
                    && !device.logged("sowing_record", &sowing),
                "{files:?}: the sowing is gone on {}",
                device.id
            );
        }
        g.assert_converged(&format!("s26 {files:?}"));
    }
}

#[test]
fn the_comparison_sees_a_device_that_differs() {
    // The control for every situation above: an erasure that has not
    // travelled is a difference the comparison reports, in the tables and in
    // the log.
    let mut g = group();
    g.delete_book('L');
    g.everyone(Files::Whole);
    g.month_later();
    g.l.start();
    assert_ne!(g.l.tables(), g.r.tables());
    assert_ne!(g.l.log(), g.r.log());
    g.finish(Files::Whole);
    g.assert_converged("control");
}
