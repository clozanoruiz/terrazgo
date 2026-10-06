// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The purge on one device and two: each condition under which a removal goes
//! for good, or stays (docs/sync.md → The purge, as settled). The four-device
//! situations are `purge_scenarios.rs`.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{A, B, Device, new_farm, new_plot, new_season, sowing_state, sync_both};
use rusqlite::Connection;
use terrazgo_core::bundle;
use terrazgo_core::date::{add_days, now_ms, today_utc};
use terrazgo_core::duplicates::SOWING_DUPLICATES;
use terrazgo_core::merge::{heads, resolve};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::sync::{send, whole_log};

/// A day on which a removal made today is due: the day after the last day the
/// fold offers it back.
fn a_month_on() -> String {
    add_days(&today_utc(), repo::REMOVED_BOOK_DAYS + 1).unwrap()
}

/// The last day a removal made today can still be brought back.
fn last_day() -> String {
    add_days(&today_utc(), repo::REMOVED_BOOK_DAYS).unwrap()
}

struct Book {
    farm: String,
    plot: String,
    book: String,
    crop: String,
    sowings: [String; 2],
}

/// A farm with a book holding a crop and two sowings, the first naming the
/// crop.
fn book(conn: &mut Connection) -> Book {
    let farm = repo::insert_farm(conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let book = repo::insert_season(conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let crop = repo::insert_crop(
        conn,
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
        sow(
            conn,
            &farm.id,
            &plot.id,
            &book.id,
            Some(&crop),
            "one",
            "2026-04-10",
        ),
        sow(
            conn,
            &farm.id,
            &plot.id,
            &book.id,
            None,
            "two",
            "2026-05-20",
        ),
    ];
    Book {
        farm: farm.id,
        plot: plot.id,
        book: book.id,
        crop,
        sowings,
    }
}

fn sow(
    conn: &mut Connection,
    farm: &str,
    plot: &str,
    book: &str,
    crop: Option<&str>,
    notes: &str,
    day: &str,
) -> String {
    repo::insert_sowing_record(
        conn,
        NewSowingRecord {
            season_id: book.into(),
            farm_id: farm.into(),
            kind_code: "sowing".into(),
            sown_on: day.into(),
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

fn exists(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

fn logged(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM record_change WHERE root_table = ?1 AND root_id = ?2)",
        [table, id],
        |r| r.get(0),
    )
    .unwrap()
}

fn gone(conn: &Connection, table: &str, id: &str) -> bool {
    !exists(conn, table, id) && !logged(conn, table, id)
}

fn markers(conn: &Connection) -> Vec<(String, String)> {
    conn.prepare("SELECT root_table, root_id FROM purged_register ORDER BY root_table, root_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn purge(conn: &mut Connection, today: &str) -> usize {
    repo::purge_due(conn, today, None).unwrap().registers
}

// ---------------------------------------------------------------------------
// When a register goes
// ---------------------------------------------------------------------------

#[test]
fn nothing_removed_is_nothing_erased() {
    let mut conn = common::db();
    let b = book(&mut conn);
    assert_eq!(purge(&mut conn, &a_month_on()), 0);
    assert!(exists(&conn, "season", &b.book));
    assert!(markers(&conn).is_empty());
}

/// Whether a row is in its table, removed.
fn removed(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1 AND deleted_at IS NOT NULL)"),
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn a_deleted_book_s_records_go_the_day_after_the_fold_stops_offering_it() {
    let mut conn = common::db();
    let b = book(&mut conn);
    assert_eq!(repo::delete_book(&mut conn, &b.book, &[], None).unwrap(), 3);

    assert_eq!(
        purge(&mut conn, &last_day()),
        0,
        "the last day it can be brought back, it stays"
    );

    let erased = repo::purge_due(&mut conn, &a_month_on(), None).unwrap();
    assert_eq!(
        (erased.registers, erased.books, erased.records),
        (2, 1, 2),
        "two sowings; and the book went, for the farmer"
    );
    for sowing in &b.sowings {
        assert!(gone(&conn, "sowing_record", sowing));
    }
    let plots: i64 = conn
        .query_row("SELECT COUNT(*) FROM sowing_plot", [], |r| r.get(0))
        .unwrap();
    assert_eq!(plots, 0, "a record's children go with it");
    // What a record can point at is never erased, so nothing arriving later
    // can name what went (docs/sync.md → What can go): it stays, removed.
    assert!(removed(&conn, "season", &b.book), "the book stays, removed");
    assert!(removed(&conn, "crop", &b.crop), "and so does its crop");
    assert!(exists(&conn, "plot", &b.plot), "the land is never erased");
    assert!(exists(&conn, "farm", &b.farm));
    let mut expected = vec![
        ("sowing_record".to_owned(), b.sowings[0].clone()),
        ("sowing_record".to_owned(), b.sowings[1].clone()),
    ];
    expected.sort();
    assert_eq!(markers(&conn), expected, "a marker per register");
    assert_eq!(purge(&mut conn, &a_month_on()), 0, "and nothing twice");
}

#[test]
fn what_a_record_can_point_at_is_never_erased() {
    // Read off the schema at core's: every record names its book, and sowings
    // name crops. At the composed schema the shell holds the whole list.
    let conn = common::db();
    let tables = terrazgo_core::sync::book_tables(&conn).unwrap();
    let kept: Vec<&str> = tables
        .iter()
        .filter(|table| !table.erasable)
        .map(|table| table.table.as_str())
        .collect();
    assert_eq!(kept, vec!["crop"]);
    assert!(
        tables
            .iter()
            .any(|table| table.table == "sowing_record" && table.erasable),
        "nothing at core's schema names a sowing"
    );
}

#[test]
fn a_record_removed_on_its_own_goes_and_its_book_stays() {
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::soft_delete_sowing_record(&mut conn, &b.sowings[1], None).unwrap();
    assert_eq!(purge(&mut conn, &a_month_on()), 1);
    assert!(gone(&conn, "sowing_record", &b.sowings[1]));
    assert!(exists(&conn, "season", &b.book));
    assert!(exists(&conn, "sowing_record", &b.sowings[0]));
}

#[test]
fn a_removed_crop_stays_even_once_nothing_names_it() {
    // A sowing on a device not heard from yet could still name it.
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::soft_delete_crop(&mut conn, &b.crop, None).unwrap();
    assert_eq!(purge(&mut conn, &a_month_on()), 0);

    repo::soft_delete_sowing_record(&mut conn, &b.sowings[0], None).unwrap();
    assert_eq!(purge(&mut conn, &a_month_on()), 1, "the sowing alone");
    assert!(gone(&conn, "sowing_record", &b.sowings[0]));
    assert!(removed(&conn, "crop", &b.crop));
    assert!(logged(&conn, "crop", &b.crop), "with its history");
}

#[test]
fn a_record_with_an_export_alias_stays() {
    // The authority may hold it: only the exporter's submission log can say it
    // was withdrawn there, and until that log exists, it stays.
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::ensure_export_alias(&mut conn, "siex", "sowing_record", &b.sowings[1], "", None).unwrap();
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();
    purge(&mut conn, &a_month_on());
    assert!(exists(&conn, "sowing_record", &b.sowings[1]));
    assert!(
        exists(&conn, "season", &b.book),
        "and its book, which it is in"
    );
    assert!(gone(&conn, "sowing_record", &b.sowings[0]));
    assert!(removed(&conn, "crop", &b.crop));
}

#[test]
fn two_opposite_duplicate_removals_wait_for_a_person() {
    // Each device kept the copy the other removed: the operation is in the
    // book zero times, and erasing both would make that permanent.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let b = book(&mut laptop.conn);
    let twin = sow(
        &mut laptop.conn,
        &b.farm,
        &b.plot,
        &b.book,
        None,
        "two",
        "2026-05-20",
    );
    sync_both(&mut phone, &mut laptop);
    let (first, second) = (b.sowings[1].clone(), twin);
    repo::keep_duplicate(
        &mut phone.conn,
        &SOWING_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_sowing_record_tx,
    )
    .unwrap();
    repo::keep_duplicate(
        &mut laptop.conn,
        &SOWING_DUPLICATES,
        &second,
        &first,
        None,
        repo::soft_delete_sowing_record_tx,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(purge(&mut laptop.conn, &a_month_on()), 0);
    assert!(exists(&laptop.conn, "sowing_record", &first));
    assert!(exists(&laptop.conn, "sowing_record", &second));
}

#[test]
fn a_copy_two_people_removed_alike_goes() {
    // Both kept the same copy: the other was removed twice, apart — two
    // versions that agree, which nobody is asked about. It goes as one.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let b = book(&mut laptop.conn);
    let twin = sow(
        &mut laptop.conn,
        &b.farm,
        &b.plot,
        &b.book,
        None,
        "two",
        "2026-05-20",
    );
    sync_both(&mut phone, &mut laptop);
    let (kept, removed) = (b.sowings[1].clone(), twin);
    for device in [&mut phone, &mut laptop] {
        repo::keep_duplicate(
            &mut device.conn,
            &SOWING_DUPLICATES,
            &kept,
            &removed,
            None,
            repo::soft_delete_sowing_record_tx,
        )
        .unwrap();
    }
    sync_both(&mut phone, &mut laptop);
    assert_eq!(
        heads(&laptop.conn, "sowing_record", &removed)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(purge(&mut laptop.conn, &a_month_on()), 1);
    assert!(gone(&laptop.conn, "sowing_record", &removed));
    assert!(exists(&laptop.conn, "sowing_record", &kept));
}

// ---------------------------------------------------------------------------
// Every device holds the removal
// ---------------------------------------------------------------------------

/// Two devices in one group, each registered, exchanging real files.
fn two_in_a_group() -> (Connection, Connection) {
    let mut laptop = common::db();
    let mut phone = common::db();
    repo::register_this_device(&mut laptop, None).unwrap();
    repo::register_this_device(&mut phone, None).unwrap();
    let group = terrazgo_core::sync::ensure_sync_group(&laptop).unwrap();
    terrazgo_core::sync::join_sync_group(&phone, &group).unwrap();
    exchange(&mut laptop, &mut phone);
    (laptop, phone)
}

/// Each sends the other its whole log.
fn exchange(left: &mut Connection, right: &mut Connection) {
    send(left, right);
    send(right, left);
}

#[test]
fn a_device_not_known_to_hold_the_removal_is_waited_for() {
    let (mut laptop, mut phone) = two_in_a_group();
    let b = book(&mut laptop);
    exchange(&mut laptop, &mut phone);
    repo::delete_book(&mut laptop, &b.book, &[], None).unwrap();
    assert_eq!(
        purge(&mut laptop, &a_month_on()),
        0,
        "the phone has not had the deletion"
    );

    // The phone receives it; the laptop does not know yet.
    let from_laptop = whole_log(&laptop);
    bundle::apply_bundle(&mut phone, &from_laptop, now_ms()).unwrap();
    assert_eq!(
        purge(&mut laptop, &a_month_on()),
        0,
        "knowledge runs behind"
    );

    // The phone's next file says it holds the deletion.
    let from_phone = whole_log(&phone);
    bundle::apply_bundle(&mut laptop, &from_phone, now_ms()).unwrap();
    assert_eq!(purge(&mut laptop, &a_month_on()), 2);

    // And the laptop's next file erases it on the phone, counted as the
    // message counts it.
    let from_laptop = whole_log(&laptop);
    let summary = bundle::apply_bundle(&mut phone, &from_laptop, now_ms()).unwrap();
    assert_eq!(
        (
            summary.purged.registers,
            summary.purged.books,
            summary.purged.records
        ),
        (2, 1, 2)
    );
    assert!(removed(&phone, "season", &b.book));
    for sowing in &b.sowings {
        assert!(gone(&phone, "sowing_record", sowing));
    }
}

#[test]
fn a_retired_device_is_not_waited_for() {
    let (mut laptop, mut phone) = two_in_a_group();
    let b = book(&mut laptop);
    exchange(&mut laptop, &mut phone);
    let phone_id = terrazgo_core::sync::installed_device(&phone).unwrap();
    repo::delete_book(&mut laptop, &b.book, &[], None).unwrap();
    repo::retire_sync_peer(&mut laptop, &phone_id, true, None).unwrap();
    assert_eq!(purge(&mut laptop, &a_month_on()), 2);
}

#[test]
fn a_refused_file_teaches_nothing() {
    // Knowledge is recorded only once a file applies: a file refused before
    // it applies must not let a device believe the sender holds anything.
    let (mut laptop, mut phone) = two_in_a_group();
    let b = book(&mut laptop);
    exchange(&mut laptop, &mut phone);
    repo::delete_book(&mut laptop, &b.book, &[], None).unwrap();
    let from_laptop = whole_log(&laptop);
    bundle::apply_bundle(&mut phone, &from_laptop, now_ms()).unwrap();
    // The phone's file, trimmed for a device holding more than the laptop
    // does: refused here.
    let phone_id = terrazgo_core::sync::installed_device(&phone).unwrap();
    let mut ahead = bundle::seen_by(&laptop).unwrap();
    ahead.observe("0192f3a4-0000-7000-8000-0000000000ff", 3);
    let mut bytes = Vec::new();
    bundle::write_bundle(&phone, &phone_id, &ahead, &mut bytes).unwrap();
    let refused = bundle::apply_bundle(
        &mut laptop,
        &bundle::read_bundle(&bytes[..]).unwrap(),
        now_ms(),
    );
    assert!(matches!(
        refused,
        Err(terrazgo_core::CoreError::Invalid("bundle_skips_changes"))
    ));
    assert_eq!(purge(&mut laptop, &a_month_on()), 0);
}

// ---------------------------------------------------------------------------
// The marker
// ---------------------------------------------------------------------------

#[test]
fn a_campaign_opened_again_after_the_purge_is_a_book_with_nothing_in_it() {
    // The book was never erased, so its id — its farm and dates — brings it
    // back, under the name just given: to the farmer, a new book. What stayed
    // of the old one stays removed. A register keyed by what it is and erased
    // for good is stamped on top of its marker instead
    // (module-phytosanitary's `book_delete.rs`, a declaration made again).
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();
    purge(&mut conn, &a_month_on());

    let again =
        repo::insert_season(&mut conn, new_season(&b.farm, 2026, "otra vez"), None).unwrap();
    assert_eq!(again.id, b.book);
    assert_eq!(again.label, "otra vez");
    assert_eq!(repo::count_book_records(&conn, &b.book).unwrap(), 0);
    assert!(removed(&conn, "crop", &b.crop));
    assert!(
        repo::removed_with_book(&conn, &b.book, &a_month_on())
            .unwrap()
            .is_none(),
        "and nothing is offered back past the thirty days"
    );
}

// ---------------------------------------------------------------------------
// Erased, not compacted
// ---------------------------------------------------------------------------

#[test]
fn an_erased_record_is_gone_from_the_database_file() {
    // secure_delete overwrites what the purge frees, and the checkpoint empties
    // the write-ahead log: a distinctive note is nowhere in either file
    // afterwards, without the file being compacted.
    let dir = common::TempDir::create("purge-erase");
    let path = dir.path().join("farm.db");
    let mut conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    terrazgo_core::db::harden(&conn).unwrap();
    terrazgo_core::migrations().to_latest(&mut conn).unwrap();
    terrazgo_core::sync::install_device(&conn, A).unwrap();
    terrazgo_core::sync::install_shape(&conn, &[terrazgo_core::sync::CORE_SYNC_SHAPE]).unwrap();
    let b = book(&mut conn);
    let marker = "ERASE-ME-7f3a91c2";
    let noted = sow(
        &mut conn,
        &b.farm,
        &b.plot,
        &b.book,
        None,
        marker,
        "2026-06-01",
    );
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();

    let on_disk = |path: &std::path::Path| -> bool {
        let wal = path.with_extension("db-wal");
        let read = |p: &std::path::Path| std::fs::read(p).unwrap_or_default();
        let needle = marker.as_bytes();
        [read(path), read(&wal)]
            .iter()
            .any(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
    };
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
    assert!(
        on_disk(&path),
        "the control: before the purge it is in the file"
    );

    purge(&mut conn, &a_month_on());
    assert!(gone(&conn, "sowing_record", &noted));
    assert!(!on_disk(&path), "after it, it is nowhere in the file");
}

// ---------------------------------------------------------------------------
// What a book past its date waits for — the fold's line
// ---------------------------------------------------------------------------

/// The fold's entry for a book, read on `day`.
fn fold_entry(conn: &Connection, book: &str, day: &str) -> Option<repo::RemovedBook> {
    repo::list_removed_books(conn, day)
        .unwrap()
        .into_iter()
        .find(|entry| entry.season.id == book)
}

/// What the fold says a book past its date waits for, read on `day`.
fn waiting(conn: &Connection, book: &str, day: &str) -> repo::Erasing {
    fold_entry(conn, book, day)
        .expect("listed")
        .erasing
        .expect("past its date")
}

#[test]
fn a_book_inside_its_thirty_days_is_offered_back_and_says_nothing_of_erasing() {
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();
    let entry = fold_entry(&conn, &b.book, &last_day()).unwrap();
    assert!(entry.erasing.is_none());
    assert_eq!(
        entry.removal.records.len(),
        2,
        "crops and sowings, to bring back"
    );
}

#[test]
fn a_book_past_its_date_names_the_device_it_waits_for() {
    let (mut laptop, mut phone) = two_in_a_group();
    let phone_id = terrazgo_core::sync::installed_device(&phone).unwrap();
    repo::rename_sync_peer(&mut laptop, &phone_id, Some("Móvil de Juan"), None).unwrap();
    let b = book(&mut laptop);
    exchange(&mut laptop, &mut phone);
    repo::delete_book(&mut laptop, &b.book, &[], None).unwrap();

    let entry = fold_entry(&laptop, &b.book, &a_month_on()).unwrap();
    assert!(
        entry.removal.records.is_empty(),
        "nothing is offered back past the date"
    );
    assert_eq!(
        entry.erasing,
        Some(repo::Erasing::Devices {
            devices: vec![repo::WaitedDevice {
                device: phone_id.clone(),
                label: Some("Móvil de Juan".into()),
            }],
        })
    );
    assert_eq!(
        purge(&mut laptop, &a_month_on()),
        0,
        "the line says the truth"
    );

    // Retiring the phone ends the wait: due, and then gone.
    repo::retire_sync_peer(&mut laptop, &phone_id, true, None).unwrap();
    assert_eq!(
        waiting(&laptop, &b.book, &a_month_on()),
        repo::Erasing::Due { from: None }
    );
    assert_eq!(purge(&mut laptop, &a_month_on()), 2);
    assert!(
        fold_entry(&laptop, &b.book, &a_month_on()).is_none(),
        "gone, for the farmer"
    );
}

#[test]
fn a_book_past_its_date_holding_a_live_record_waits_on_the_status_view() {
    // Recorded into on the phone while the laptop deleted it: a record in a
    // removed book, which the Status view lists.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let b = book(&mut laptop.conn);
    sync_both(&mut laptop, &mut phone);
    sow(
        &mut phone.conn,
        &b.farm,
        &b.plot,
        &b.book,
        None,
        "recorded on the phone",
        "2026-06-01",
    );
    repo::delete_book(&mut laptop.conn, &b.book, &[], None).unwrap();
    sync_both(&mut laptop, &mut phone);
    assert_eq!(
        waiting(&laptop.conn, &b.book, &a_month_on()),
        repo::Erasing::Status
    );
    purge(&mut laptop.conn, &a_month_on());
    assert_eq!(
        waiting(&laptop.conn, &b.book, &a_month_on()),
        repo::Erasing::Status,
        "listed while the record waits there"
    );
}

#[test]
fn a_book_past_its_date_with_a_conflict_in_it_waits_on_the_status_view() {
    // Corrected on the phone, then the book deleted on the laptop: the
    // deletion shows and the correction waits for a person.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let b = book(&mut laptop.conn);
    sync_both(&mut laptop, &mut phone);
    repo::update_sowing_record(
        &mut phone.conn,
        &b.sowings[1],
        sowing_state(Some("corrected"), &[&b.plot]),
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &b.book, &[], None).unwrap();
    sync_both(&mut laptop, &mut phone);
    assert!(
        laptop.conflicted("sowing_record", &b.sowings[1]),
        "the control: the correction waits in the review"
    );
    assert_eq!(
        waiting(&laptop.conn, &b.book, &a_month_on()),
        repo::Erasing::Status
    );
}

#[test]
fn a_book_past_its_date_waits_for_a_removal_made_after_its_own() {
    // A record of the book removed on its own reads as ten days later than
    // the book's deletion — a clock that was behind when the book went, as in
    // s14b. The book is past its date, and goes when that record does.
    let mut conn = common::db();
    let b = book(&mut conn);
    let earlier = add_days(&today_utc(), -10).unwrap();
    repo::soft_delete_sowing_record(&mut conn, &b.sowings[1], None).unwrap();
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();
    let deletion = heads(&conn, "season", &b.book).unwrap().remove(0);
    conn.execute(
        "UPDATE record_change SET changed_at = ?1 || substr(changed_at, 11)
         WHERE origin_device = ?2 AND origin_seq = ?3",
        rusqlite::params![earlier, deletion.device, deletion.seq],
    )
    .unwrap();

    let day = add_days(&earlier, repo::REMOVED_BOOK_DAYS + 1).unwrap();
    assert_eq!(
        waiting(&conn, &b.book, &day),
        repo::Erasing::Due {
            from: Some(add_days(&today_utc(), repo::REMOVED_BOOK_DAYS + 1).unwrap())
        }
    );
}

#[test]
fn a_book_past_its_date_with_nothing_left_that_can_go_leaves_the_fold() {
    // The laptop merged the book into another, the crop going live with its
    // sowing, while the phone removed the crop. Kept as the removal in the
    // review, the crop is back in the book that went — and the sowing that
    // names it is in the other. A crop is never erased, so nothing breaks:
    // nothing in the book can go, so to the farmer it has gone.
    let mut laptop = Device::new(B);
    let mut phone = Device::new(A);
    let b = book(&mut laptop.conn);
    let other = repo::insert_season(
        &mut laptop.conn,
        new_season(&b.farm, 2027, "2026/2027"),
        None,
    )
    .unwrap()
    .id;
    sync_both(&mut laptop, &mut phone);
    repo::soft_delete_crop(&mut phone.conn, &b.crop, None).unwrap();
    repo::merge_books(&mut laptop.conn, &other, &b.book, &[], None).unwrap();
    sync_both(&mut laptop, &mut phone);
    let removal = heads(&laptop.conn, "crop", &b.crop)
        .unwrap()
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut laptop.conn,
        "crop",
        &b.crop,
        &removal.device,
        removal.seq,
        None,
    )
    .unwrap();
    let filed: String = laptop
        .conn
        .query_row("SELECT season_id FROM crop WHERE id = ?1", [&b.crop], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        filed, b.book,
        "the control: the crop is in the book that went"
    );

    assert!(fold_entry(&laptop.conn, &b.book, &a_month_on()).is_none());
    assert_eq!(purge(&mut laptop.conn, &a_month_on()), 0);
    assert!(removed(&laptop.conn, "crop", &b.crop));
}

#[test]
fn a_correction_naming_a_merged_book_waits_in_the_review_and_the_book_leaves_the_fold() {
    // The phone corrected the sowing that names the removed crop while the
    // laptop merged its book away: the move shows, and the correction — still
    // naming the removed book and its crop — waits in the review. Nothing in
    // that book can go, so the fold does not list it; the review does.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let b = book(&mut laptop.conn);
    let other = repo::insert_season(
        &mut laptop.conn,
        new_season(&b.farm, 2027, "2026/2027"),
        None,
    )
    .unwrap()
    .id;
    repo::soft_delete_crop(&mut laptop.conn, &b.crop, None).unwrap();
    sync_both(&mut laptop, &mut phone);
    let mut corrected = sowing_state(Some("corrected"), &[&b.plot]);
    corrected.plots[0].crop_id = Some(b.crop.clone());
    repo::update_sowing_record(&mut phone.conn, &b.sowings[0], corrected, None).unwrap();
    repo::merge_books(&mut laptop.conn, &other, &b.book, &[], None).unwrap();
    sync_both(&mut laptop, &mut phone);
    assert!(
        laptop.conflicted("sowing_record", &b.sowings[0]),
        "the control: the correction waits in the review"
    );
    assert!(fold_entry(&laptop.conn, &b.book, &a_month_on()).is_none());
    purge(&mut laptop.conn, &a_month_on());
    assert!(
        laptop.conflicted("sowing_record", &b.sowings[0]),
        "and keeping it still finds its book and crop"
    );
}

#[test]
fn a_book_due_and_not_yet_erased_goes_at_the_next_start() {
    let mut conn = common::db();
    let b = book(&mut conn);
    repo::delete_book(&mut conn, &b.book, &[], None).unwrap();
    assert_eq!(
        waiting(&conn, &b.book, &a_month_on()),
        repo::Erasing::Due { from: None }
    );
    purge(&mut conn, &a_month_on());
    assert!(fold_entry(&conn, &b.book, &a_month_on()).is_none());
}
