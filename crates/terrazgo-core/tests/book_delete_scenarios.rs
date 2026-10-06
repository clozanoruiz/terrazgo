// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deleting a book with its records, across a laptop and three phones
//! (docs/sync.md → How it is tested — a laptop and three phones, slice 10).
//!
//! **The devices**: a laptop L, where the group's files meet, and phones P, Q
//! and R — R away for the whole sequence, receiving everything at the end.
//! **The transport**: real bundle files through `write_bundle` and
//! `apply_bundle`, so the completeness and identity checks run as in the app.
//! Every situation runs twice — with files carrying the whole log, and with the
//! laptop's one reply to both phones trimmed to what both had seen, the
//! session rule — and with the laptop writing something unrelated after each
//! round: each transport can hide a divergence the other shows.
//!
//! **What every situation ends in**: the same synced tables on all four
//! devices, the same review queue and the same list of records in a removed
//! book, and no conflict nobody caused.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{new_farm, new_plot, new_season, sowing_state};
use rusqlite::Connection;
use terrazgo_core::bundle;
use terrazgo_core::date::{now_ms, today_utc};
use terrazgo_core::merge::{CORE_ROW_CAPTIONS, heads, resolve};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_core::sync::VersionVector;

const L: &str = "0192f3a4-0000-7000-8000-00000000000a";
const P: &str = "0192f3a4-0000-7000-8000-00000000000b";
const Q: &str = "0192f3a4-0000-7000-8000-00000000000c";
const R: &str = "0192f3a4-0000-7000-8000-00000000000d";

/// How files travel in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Files {
    /// Every file carries its sender's whole log.
    Whole,
    /// A phone sends what the laptop lacks; the laptop's one reply answers
    /// every file it imported in the session.
    Trimmed,
}

const BOTH: [Files; 2] = [Files::Whole, Files::Trimmed];

struct Device {
    conn: Connection,
    id: &'static str,
}

impl Device {
    fn new(id: &'static str) -> Self {
        let conn = terrazgo_core::open_in_memory().unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        Device { conn, id }
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

    fn apply(&mut self, bytes: &[u8]) -> bundle::ImportSummary {
        let parsed = bundle::read_bundle(bytes).unwrap();
        bundle::apply_bundle(&mut self.conn, &parsed, now_ms())
            .unwrap_or_else(|refusal| panic!("{} refused a file: {refusal:?}", self.id))
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
}

/// A laptop and three phones in the laptop's group, all holding one farm with
/// a book of two sowings and a crop, and a twin of the book.
struct Group {
    l: Device,
    p: Device,
    q: Device,
    r: Device,
    farm: String,
    plot: String,
    book: String,
    twin: String,
    sowings: [String; 2],
    crop: String,
    unrelated: usize,
}

fn group() -> Group {
    let mut l = Device::new(L);
    let mut p = Device::new(P);
    let mut q = Device::new(Q);
    let mut r = Device::new(R);
    let group_id = terrazgo_core::sync::ensure_sync_group(&l.conn).unwrap();
    for phone in [&p, &q, &r] {
        terrazgo_core::sync::join_sync_group(&phone.conn, &group_id).unwrap();
    }
    let farm = repo::insert_farm(&mut l.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut l.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let book =
        repo::insert_season(&mut l.conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let twin = repo::insert_season(
        &mut l.conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap();
    let sowings = [
        sow(&mut l, &farm.id, &plot.id, &book.id, "one"),
        sow(&mut l, &farm.id, &plot.id, &book.id, "two"),
    ];
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
    let whole = l.file(&VersionVector::default());
    for phone in [&mut p, &mut q, &mut r] {
        phone.apply(&whole);
    }
    Group {
        l,
        p,
        q,
        r,
        farm: farm.id,
        plot: plot.id,
        book: book.id,
        twin: twin.id,
        sowings,
        crop,
        unrelated: 0,
    }
}

fn sow(device: &mut Device, farm: &str, plot: &str, book: &str, notes: &str) -> String {
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
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

/// Which phones take part in a round.
#[derive(Debug, Clone, Copy)]
enum Phones {
    /// P and Q.
    Both,
    /// P alone: Q has not heard from anyone yet.
    OnlyP,
}

impl Group {
    fn sow_on(&mut self, who: char, notes: &str) -> String {
        let (farm, plot, book) = (self.farm.clone(), self.plot.clone(), self.book.clone());
        sow(self.device(who), &farm, &plot, &book, notes)
    }

    fn device(&mut self, who: char) -> &mut Device {
        match who {
            'L' => &mut self.l,
            'P' => &mut self.p,
            'Q' => &mut self.q,
            _ => &mut self.r,
        }
    }

    /// Two rounds of the phones taking part sending to the laptop, the laptop
    /// writing something unrelated, and its one reply going back to each.
    fn exchange(&mut self, files: Files, phones: Phones) {
        for _ in 0..2 {
            let mut manifests = Vec::new();
            let senders: &mut [&mut Device] = match phones {
                Phones::Both => &mut [&mut self.p, &mut self.q],
                Phones::OnlyP => &mut [&mut self.p],
            };
            for phone in senders.iter() {
                let file = phone.file_for(&self.l, files);
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
            for phone in senders.iter_mut() {
                phone.apply(&reply);
            }
        }
    }

    /// Everyone meets: P and Q exchange with the laptop until nothing moves,
    /// and R, away all along, hears from the laptop at last.
    fn finish(&mut self, files: Files) {
        self.exchange(files, Phones::Both);
        let from_r = self.r.file_for(&self.l, files);
        self.l.apply(&from_r);
        let reply = self.l.file_for(&self.r, files);
        self.r.apply(&reply);
    }

    fn all(&self) -> [&Device; 4] {
        [&self.l, &self.p, &self.q, &self.r]
    }

    /// The same tables, the same queue and the same stranded records on every
    /// device, R included.
    fn assert_converged(&self, situation: &str) {
        let tables = self.l.tables();
        // The control: the comparison reads the tables this file writes to,
        // so it cannot pass by comparing nothing with nothing.
        for written in ["season", "sowing_record", "sowing_plot", "crop", "plot"] {
            assert!(
                tables
                    .iter()
                    .any(|(table, rows)| table == written && !rows.is_empty()),
                "{situation}: the comparison must read {written}"
            );
        }
        for device in [&self.p, &self.q, &self.r] {
            assert_eq!(
                device.tables(),
                tables,
                "{situation}: {} holds different tables from the laptop",
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

    fn book_removed(&self, device: &Device) -> bool {
        device.removed("season", &self.book)
    }

    fn delete_book(&mut self, who: char) -> usize {
        let book = self.book.clone();
        repo::delete_book(&mut self.device(who).conn, &book, &[], None).unwrap()
    }

    fn restore_book(&mut self, who: char) -> repo::RestoredBook {
        let book = self.book.clone();
        repo::restore_book(&mut self.device(who).conn, &book, &today_utc(), None).unwrap()
    }

    /// Keep, on `who`, the version of `table`/`id` that `device` wrote.
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
}

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

#[test]
fn s1_a_record_written_into_the_book_elsewhere_is_listed_everywhere() {
    for files in BOTH {
        let mut g = group();
        assert_eq!(g.delete_book('L'), 3);
        let late = g.sow_on('P', "written offline");
        g.finish(files);

        for device in g.all() {
            assert!(g.book_removed(device), "{files:?}");
            assert_eq!(
                device.strays(),
                vec![late.clone()],
                "{files:?}: {}",
                device.id
            );
            assert!(
                device.queue().is_empty(),
                "{files:?}: nobody caused a conflict"
            );
            for sowing in &g.sowings {
                assert!(device.removed("sowing_record", sowing));
            }
        }
        g.assert_converged(&format!("s1 {files:?}"));
    }
}

#[test]
fn s2_a_record_corrected_elsewhere_is_a_conflict_and_either_answer_converges() {
    for files in BOTH {
        for keep_correction in [true, false] {
            let mut g = group();
            g.delete_book('L');
            let corrected = g.sowings[0].clone();
            let plot = g.plot.clone();
            repo::update_sowing_record(
                &mut g.p.conn,
                &corrected,
                sowing_state(Some("corrected"), &[&plot]),
                None,
            )
            .unwrap();
            g.finish(files);
            for device in g.all() {
                assert_eq!(
                    device.queue(),
                    vec![("sowing_record".to_owned(), corrected.clone())],
                    "{files:?}: the correction against the removal, on {}",
                    device.id
                );
            }

            let writer = if keep_correction { P } else { L };
            g.keep_version_of('L', "sowing_record", &corrected, writer);
            g.finish(files);
            for device in g.all() {
                assert!(device.queue().is_empty());
                if keep_correction {
                    assert_eq!(
                        device.strays(),
                        vec![corrected.clone()],
                        "{files:?}: kept as the correction, a record in a removed book"
                    );
                } else {
                    assert!(device.strays().is_empty());
                    assert!(device.removed("sowing_record", &corrected));
                }
            }
            g.assert_converged(&format!("s2 {files:?} keep_correction={keep_correction}"));
        }
    }
}

#[test]
fn s3_the_same_book_deleted_on_two_devices_agrees_and_comes_back_whole() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.delete_book('P');
        g.finish(files);
        for device in g.all() {
            assert!(g.book_removed(device));
            assert!(device.queue().is_empty(), "{files:?}: the removals agree");
            assert!(device.strays().is_empty());
        }
        g.assert_converged(&format!("s3 {files:?} deleted"));

        // Brought back on a third device, which holds both deletions.
        let back = g.restore_book('Q');
        assert_eq!(back.records, 3, "{files:?}: either deletion brings it back");
        g.finish(files);
        for device in g.all() {
            assert!(!g.book_removed(device));
            assert!(!device.removed("crop", &g.crop));
            for sowing in &g.sowings {
                assert!(!device.removed("sowing_record", sowing));
            }
            assert!(device.queue().is_empty());
        }
        g.assert_converged(&format!("s3 {files:?} restored"));
    }
}

#[test]
fn s3c_two_deletions_of_one_book_agree_though_one_device_had_corrected_a_record() {
    // The phone corrects a sowing, then deletes the book; the laptop deletes
    // it too. The two removals of that sowing differ in what it held — and
    // both say removed, which is all any screen shows. *Found by the audit
    // (2026-10-03)*: listed, the pair blocked bringing the book back, and the
    // choice — which changed nothing visible — made the record a removal of
    // its own, which bringing the book back then left behind. Removed is
    // removed: nothing is listed, and the record comes back as the later
    // deletion left it (docs/sync.md → Versions that say the same thing are
    // not listed).
    for files in BOTH {
        for later_on in ['L', 'P'] {
            let mut g = group();
            let (plot, sowing) = (g.plot.clone(), g.sowings[1].clone());
            let held = |device: &Device| -> Option<String> {
                device
                    .conn
                    .query_row(
                        "SELECT notes FROM sowing_record WHERE id = ?1",
                        [&sowing],
                        |r| r.get(0),
                    )
                    .unwrap()
            };
            let before = held(&g.l);
            repo::update_sowing_record(
                &mut g.p.conn,
                &sowing,
                sowing_state(Some("corregido en el móvil"), &[&plot]),
                None,
            )
            .unwrap();
            let earlier_on = if later_on == 'L' { 'P' } else { 'L' };
            g.delete_book(earlier_on);
            // The clock moves on, so the second deletion is the later one.
            std::thread::sleep(std::time::Duration::from_millis(2));
            g.delete_book(later_on);
            g.finish(files);
            let situation = format!("{files:?}, the later deletion on {later_on}");
            assert_eq!(
                heads(&g.l.conn, "sowing_record", &sowing).unwrap().len(),
                2,
                "{situation}: the control — two versions of the sowing"
            );
            for device in g.all() {
                assert!(device.queue().is_empty(), "{situation}: on {}", device.id);
            }
            g.assert_converged(&format!("s3c {situation} deleted"));

            let back = g.restore_book('Q');
            assert_eq!(back.records, 3, "{situation}: everything comes back");
            g.finish(files);
            let expected = if later_on == 'P' {
                Some("corregido en el móvil".to_owned())
            } else {
                before.clone()
            };
            for device in g.all() {
                assert!(!device.removed("sowing_record", &sowing), "{situation}");
                assert_eq!(held(device), expected, "{situation}: on {}", device.id);
                assert!(device.queue().is_empty(), "{situation}");
            }
            g.assert_converged(&format!("s3c {situation} restored"));
        }
    }
}

#[test]
fn s3b_a_record_removed_on_its_own_while_the_book_went_stays_removed() {
    // The phone's removal of one record is a person's word about THAT record,
    // and the book's deletion does not get to undo it.
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        let alone = g.sowings[0].clone();
        repo::soft_delete_sowing_record(&mut g.p.conn, &alone, None).unwrap();
        g.finish(files);
        for device in g.all() {
            assert!(device.queue().is_empty(), "{files:?}: both say removed");
        }

        let back = g.restore_book('L');
        assert_eq!(back.records, 2, "{files:?}: the crop and the other sowing");
        g.finish(files);
        for device in g.all() {
            assert!(device.removed("sowing_record", &alone));
            assert!(!device.removed("sowing_record", &g.sowings[1]));
            assert!(!device.removed("crop", &g.crop));
        }
        g.assert_converged(&format!("s3b {files:?}"));
    }
}

#[test]
fn s4_a_book_brought_back_while_another_device_recorded_into_it_holds_everything() {
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.restore_book('L');
        let written = g.sow_on('Q', "never heard of either");
        g.finish(files);
        for device in g.all() {
            assert!(!g.book_removed(device));
            assert!(!device.removed("sowing_record", &written));
            for sowing in &g.sowings {
                assert!(!device.removed("sowing_record", sowing));
            }
            assert!(
                device.queue().is_empty(),
                "{files:?}: nobody caused a conflict"
            );
            assert!(device.strays().is_empty());
        }
        g.assert_converged(&format!("s4 {files:?}"));
    }
}

#[test]
fn s5_a_book_brought_back_on_a_phone_while_another_still_writes_in_it() {
    // Q never heard of the deletion: it records a new sowing (no conflict) and
    // corrects one the deletion removed and P's restore brought back — two
    // people writing one record, which is a conflict like any other.
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        g.exchange(files, Phones::OnlyP);
        assert!(g.book_removed(&g.p));
        g.restore_book('P');
        let written = g.sow_on('Q', "still recording");
        let corrected = g.sowings[0].clone();
        let plot = g.plot.clone();
        repo::update_sowing_record(
            &mut g.q.conn,
            &corrected,
            sowing_state(Some("corrected on Q"), &[&plot]),
            None,
        )
        .unwrap();
        g.finish(files);
        for device in g.all() {
            assert!(!g.book_removed(device));
            assert!(!device.removed("sowing_record", &written));
            assert!(!device.removed("sowing_record", &g.sowings[1]));
            assert_eq!(
                device.queue(),
                vec![("sowing_record".to_owned(), corrected.clone())],
                "{files:?}: P's restore against Q's correction, on {}",
                device.id
            );
        }
        g.assert_converged(&format!("s5 {files:?}"));
    }
}

#[test]
fn s6_a_record_removed_before_the_book_went_stays_removed_on_every_device() {
    for files in BOTH {
        let mut g = group();
        let before = g.sowings[0].clone();
        repo::soft_delete_sowing_record(&mut g.l.conn, &before, None).unwrap();
        g.exchange(files, Phones::Both);
        assert_eq!(g.delete_book('P'), 2);
        g.exchange(files, Phones::Both);
        assert_eq!(g.restore_book('Q').records, 2);
        g.finish(files);
        for device in g.all() {
            assert!(device.removed("sowing_record", &before), "{files:?}");
            assert!(!device.removed("sowing_record", &g.sowings[1]));
            assert!(!device.removed("crop", &g.crop));
        }
        g.assert_converged(&format!("s6 {files:?}"));
    }
}

#[test]
fn s7_a_book_deleted_while_another_device_merges_it_is_a_conflict_per_record() {
    // Two people doing opposite things to one book offline: each record is a
    // conflict, the merge's version against the removal, and nothing is lost
    // whichever is kept. The cost is one decision per record (docs/sync.md →
    // Deleting a book with its records).
    for files in BOTH {
        let mut g = group();
        g.delete_book('L');
        let (twin, book) = (g.twin.clone(), g.book.clone());
        repo::merge_books(&mut g.p.conn, &twin, &book, &[], None).unwrap();
        g.finish(files);
        for device in g.all() {
            assert!(g.book_removed(device), "both removed the book, and agree");
            let mut expected: Vec<(String, String)> = g
                .sowings
                .iter()
                .map(|id| ("sowing_record".to_owned(), id.clone()))
                .chain([("crop".to_owned(), g.crop.clone())])
                .collect();
            expected.sort();
            assert_eq!(device.queue(), expected, "{files:?}: one per record");
        }
        g.assert_converged(&format!("s7 {files:?}"));
    }
}

#[test]
fn s10_a_book_renamed_elsewhere_while_it_was_deleted_is_offered_back_everywhere() {
    // Added when the design was run before building: the rename, later on the
    // clock, brings the book back live and empty, and nothing listed its
    // records. Its page now offers them back, on every device.
    for files in BOTH {
        for keep_rename in [true, false] {
            let mut g = group();
            g.delete_book('L');
            let season = repo::get_season(&g.p.conn, &g.book).unwrap();
            let book = g.book.clone();
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
            g.finish(files);
            for device in g.all() {
                assert_eq!(device.queue(), vec![("season".to_owned(), book.clone())]);
                let offered = repo::removed_with_book(&device.conn, &book, &today_utc())
                    .unwrap()
                    .expect("the rename is live, and the records are offered back");
                let total: usize = offered.records.iter().map(|count| count.count).sum();
                assert_eq!(total, 3, "{files:?}: on {}", device.id);
            }

            let writer = if keep_rename { P } else { L };
            g.keep_version_of('L', "season", &book, writer);
            if keep_rename {
                assert_eq!(g.restore_book('L').records, 3);
            }
            g.finish(files);
            for device in g.all() {
                assert!(device.queue().is_empty());
                if keep_rename {
                    assert!(!g.book_removed(device));
                    assert!(!device.removed("crop", &g.crop));
                    assert!(
                        repo::removed_with_book(&device.conn, &book, &today_utc())
                            .unwrap()
                            .is_none()
                    );
                } else {
                    // Kept as the deletion: the book is in the list of removed
                    // books, with what went with it.
                    assert!(g.book_removed(device));
                    let listed = repo::list_removed_books(&device.conn, &today_utc()).unwrap();
                    assert_eq!(listed.len(), 1);
                    assert_eq!(listed[0].removal.removed_on, L);
                }
            }
            g.assert_converged(&format!("s10 {files:?} keep_rename={keep_rename}"));
        }
    }
}

#[test]
fn the_comparison_sees_a_device_that_differs() {
    // The control for every situation above: a write that has not travelled
    // is a difference the comparison reports.
    let mut g = group();
    g.delete_book('L');
    assert_ne!(g.l.tables(), g.r.tables());
    g.finish(Files::Whole);
    g.assert_converged("control");
}
