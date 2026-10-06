// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deleting a book with its records, and bringing it back (docs/sync.md →
//! Deleting a book with its records) — the act itself, on one device and two.
//!
//! What it does across a laptop and three phones exchanging real files is
//! `book_delete_scenarios.rs`; this file pins the act: what one deletion
//! writes, what it refuses, what a restore brings back and what it leaves, the
//! thirty days, and what the list and the page line say and cost.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::CoreError;
use terrazgo_core::date::{add_days, today_utc};
use terrazgo_core::duplicates::SOWING_DUPLICATES;
use terrazgo_core::merge::resolve;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::query_cost;

/// One farm, one plot, one book named by its dates.
struct Book {
    farm: String,
    plot: String,
    book: String,
}

fn book(device: &mut Device) -> Book {
    let farm = repo::insert_farm(&mut device.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut device.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let book = repo::insert_season(
        &mut device.conn,
        new_season(&farm.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    Book {
        farm: farm.id,
        plot: plot.id,
        book: book.id,
    }
}

/// The same book on two devices, from shared history.
fn shared_book(a: &mut Device, b: &mut Device) -> Book {
    let book = book(a);
    sync_both(a, b);
    book
}

fn sow(device: &mut Device, book: &Book, notes: &str) -> String {
    repo::insert_sowing_record(
        &mut device.conn,
        NewSowingRecord {
            season_id: book.book.clone(),
            farm_id: book.farm.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: Some(notes.into()),
            plots: vec![NewSowingPlot {
                plot_id: book.plot.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn crop(device: &mut Device, book: &Book) -> String {
    repo::insert_crop(
        &mut device.conn,
        NewCrop {
            plot_id: book.plot.clone(),
            season_id: book.book.clone(),
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
    .id
}

fn load(device: &mut Device, book: &Book) -> String {
    repo::insert_harvest_record(
        &mut device.conn,
        NewHarvestRecord {
            season_id: book.book.clone(),
            farm_id: book.farm.clone(),
            harvested_on: "2026-07-01".into(),
            product_name: "Trigo blando".into(),
            plant_product_code: Some("1".into()),
            quantity_value: Some(12_000.0),
            quantity_unit_code: Some("kg".into()),
            delivery_note_ref: None,
            lot_number: None,
            buyer_name: "Cooperativa del Páramo".into(),
            buyer_tax_id: None,
            buyer_address: None,
            buyer_registry_number: None,
            notes: None,
            plots: vec![NewHarvestPlot {
                plot_id: book.plot.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn removed(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        &format!("SELECT deleted_at IS NOT NULL FROM {table} WHERE id = ?1"),
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

fn queue(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM sync_conflict", [], |r| r.get(0))
        .unwrap()
}

/// This device's newest change set, as `(device, seq)`.
fn newest_change_set(conn: &Connection) -> (String, i64) {
    conn.query_row(
        "SELECT origin_device, origin_seq FROM record_change ORDER BY hlc DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// What one change set logged, in the order it wrote it.
fn logged_by(conn: &Connection, set: &(String, i64)) -> Vec<(String, String, String)> {
    conn.prepare(
        "SELECT entity_table, entity_id, operation FROM record_change
         WHERE origin_device = ?1 AND origin_seq = ?2 ORDER BY id",
    )
    .unwrap()
    .query_map(rusqlite::params![set.0, set.1], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

fn restore(device: &mut Device, book: &str) -> Result<repo::RestoredBook, CoreError> {
    repo::restore_book(&mut device.conn, book, &today_utc(), None)
}

// ---------------------------------------------------------------------------
// The deletion
// ---------------------------------------------------------------------------

#[test]
fn deleting_a_book_removes_every_live_record_and_the_book_in_one_change_set() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let planted = crop(&mut laptop, &book);
    let sown = sow(&mut laptop, &book, "sown");
    let loaded = load(&mut laptop, &book);
    let taken_back = sow(&mut laptop, &book, "removed before");
    repo::soft_delete_sowing_record(&mut laptop.conn, &taken_back, None).unwrap();

    let gone = repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();

    assert_eq!(gone, 3, "the crop, the live sowing and the load");
    for (table, id) in [
        ("crop", &planted),
        ("sowing_record", &sown),
        ("harvest_record", &loaded),
    ] {
        assert!(
            removed(&laptop.conn, table, id),
            "{table} went with its book"
        );
    }
    assert!(matches!(
        repo::get_season(&laptop.conn, &book.book),
        Err(CoreError::NotFound)
    ));
    let written = logged_by(&laptop.conn, &newest_change_set(&laptop.conn));
    let mut what: Vec<(&str, &str)> = written
        .iter()
        .map(|(table, _, operation)| (table.as_str(), operation.as_str()))
        .collect();
    what.sort_unstable();
    assert_eq!(
        what,
        vec![
            ("crop", "delete"),
            ("harvest_record", "delete"),
            ("season", "delete"),
            ("sowing_record", "delete"),
        ],
        "one change set: the records' own rows and the book — never their plots, \
         and never the sowing removed before"
    );
    assert!(
        repo::list_stray_records(&laptop.conn, terrazgo_core::merge::CORE_ROW_CAPTIONS)
            .unwrap()
            .is_empty(),
        "nothing is left live in the book"
    );
}

#[test]
fn the_confirmation_counts_the_live_records_only() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    assert_eq!(
        repo::count_book_records(&laptop.conn, &book.book).unwrap(),
        0
    );
    crop(&mut laptop, &book);
    sow(&mut laptop, &book, "one");
    let gone = sow(&mut laptop, &book, "two");
    repo::soft_delete_sowing_record(&mut laptop.conn, &gone, None).unwrap();
    assert_eq!(
        repo::count_book_records(&laptop.conn, &book.book).unwrap(),
        2
    );
}

#[test]
fn a_deletion_waits_while_a_conflict_in_the_book_waits() {
    // The deletion is a write, and a write merges every version it saw: the
    // waiting version would leave the queue unseen.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let book = shared_book(&mut phone, &mut laptop);
    let record = sow(&mut phone, &book, "original");
    sync_both(&mut phone, &mut laptop);
    let plot = book.plot.clone();
    for (device, notes) in [(&mut phone, "phone"), (&mut laptop, "laptop")] {
        repo::update_sowing_record(
            &mut device.conn,
            &record,
            sowing_state(Some(notes), &[&plot]),
            None,
        )
        .unwrap();
    }
    sync_both(&mut phone, &mut laptop);
    assert_eq!(queue(&laptop.conn), 1);

    assert!(matches!(
        repo::delete_book(&mut laptop.conn, &book.book, &[], None),
        Err(CoreError::Invalid("book_delete_conflicts_waiting"))
    ));
    assert_eq!(queue(&laptop.conn), 1, "still waiting, for a person");

    let heads = laptop.heads_of("sowing_record", &record);
    resolve(
        &mut laptop.conn,
        "sowing_record",
        &record,
        &heads[0].device,
        heads[0].seq,
        None,
    )
    .unwrap();
    assert_eq!(
        repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap(),
        1
    );
}

#[test]
fn a_deletion_waits_while_the_book_holds_a_pair_removed_twice_over() {
    // Once the book is gone the pair leaves the list of duplicates with it,
    // and the operation is in the book zero times with nothing to say so.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let book = shared_book(&mut phone, &mut laptop);
    let first = sow(&mut phone, &book, "same");
    let second = sow(&mut phone, &book, "same");
    sync_both(&mut phone, &mut laptop);
    for (device, kept, removed) in [
        (&mut phone, &first, &second),
        (&mut laptop, &second, &first),
    ] {
        repo::keep_duplicate(
            &mut device.conn,
            &SOWING_DUPLICATES,
            kept,
            removed,
            None,
            repo::soft_delete_sowing_record_tx,
        )
        .unwrap();
    }
    sync_both(&mut phone, &mut laptop);

    assert!(matches!(
        repo::delete_book(&mut laptop.conn, &book.book, &[SOWING_DUPLICATES], None),
        Err(CoreError::Invalid("book_delete_removals_waiting"))
    ));
    repo::restore_removed_duplicate(&mut laptop.conn, &SOWING_DUPLICATES, &first, None).unwrap();
    assert_eq!(
        repo::delete_book(&mut laptop.conn, &book.book, &[SOWING_DUPLICATES], None).unwrap(),
        1
    );
}

#[test]
fn a_removed_book_or_a_removed_farms_book_is_not_found() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    assert!(matches!(
        repo::delete_book(&mut laptop.conn, &book.book, &[], None),
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        repo::count_book_records(&laptop.conn, &book.book),
        Err(CoreError::NotFound)
    ));
}

// ---------------------------------------------------------------------------
// Bringing it back
// ---------------------------------------------------------------------------

#[test]
fn bringing_a_book_back_restores_what_went_with_it_and_nothing_else() {
    // Situation 6: a record removed on its own before the book went stays
    // removed when the book comes back.
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let planted = crop(&mut laptop, &book);
    let sown = sow(&mut laptop, &book, "sown");
    let taken_back = sow(&mut laptop, &book, "removed before");
    repo::soft_delete_sowing_record(&mut laptop.conn, &taken_back, None).unwrap();
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();

    let back = restore(&mut laptop, &book.book).unwrap();

    assert_eq!(back.records, 2);
    assert!(back.season.deleted_at.is_none());
    assert_eq!(back.season.label, "2025/2026", "as it was");
    assert!(!removed(&laptop.conn, "crop", &planted));
    assert!(!removed(&laptop.conn, "sowing_record", &sown));
    assert!(removed(&laptop.conn, "sowing_record", &taken_back));
    assert_eq!(
        repo::count_book_records(&laptop.conn, &book.book).unwrap(),
        2
    );
    assert!(
        repo::removed_with_book(&laptop.conn, &book.book, &today_utc())
            .unwrap()
            .is_none(),
        "nothing left to offer back"
    );
}

#[test]
fn bringing_back_writes_only_what_the_deletion_wrote() {
    // The deletion wrote the record's own row, so undoing it writes that row
    // with its removal cleared, and nothing of what the deletion left alone:
    // every device the change set reaches holds the record's history, its
    // plots included (docs/sync.md → Bringing a register back writes only what
    // changes). The record still comes back whole, here and on the phone.
    let mut laptop = Device::new(A);
    let mut phone = Device::new(B);
    let book = shared_book(&mut laptop, &mut phone);
    let second_plot =
        repo::insert_plot(&mut laptop.conn, new_plot(&book.farm, "La Loma"), None).unwrap();
    let sown = sow(&mut laptop, &book, "two plots");
    repo::update_sowing_record(
        &mut laptop.conn,
        &sown,
        sowing_state(Some("two plots"), &[&book.plot, &second_plot.id]),
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    sync_both(&mut laptop, &mut phone);
    assert!(removed(&phone.conn, "sowing_record", &sown), "the control");

    restore(&mut laptop, &book.book).unwrap();

    let written = logged_by(&laptop.conn, &newest_change_set(&laptop.conn));
    let rows: Vec<(&str, &str, &str)> = written
        .iter()
        .map(|(table, id, operation)| (table.as_str(), id.as_str(), operation.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("season", book.book.as_str(), "update"),
            ("sowing_record", sown.as_str(), "update"),
        ],
        "the book, then the record's own row — none of its plots"
    );
    let (_, before, after) = last_change(&laptop.conn, "sowing_record", &sown);
    assert!(before["deleted_at"].is_string());
    assert_eq!(after["deleted_at"], Value::Null);
    assert_eq!(after["notes"], "two plots");

    sync_both(&mut laptop, &mut phone);
    for device in [&laptop, &phone] {
        assert!(
            !removed(&device.conn, "sowing_record", &sown),
            "{}",
            device.id
        );
        let plots: i64 = device
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sowing_plot WHERE sowing_record_id = ?1",
                [&sown],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plots, 2, "{}: the record with both of its plots", device.id);
        assert_eq!(queue(&device.conn), 0, "{}", device.id);
    }
}

#[test]
fn bringing_back_leaves_a_crop_removed_on_its_own_as_it_was() {
    // A sowing brought back can name a crop removed before the book went. The
    // crop stays removed and nothing is written for it: a crop is never erased
    // (docs/sync.md → What can go), so every device still holds what the
    // sowing names.
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let planted = crop(&mut laptop, &book);
    let sown = repo::insert_sowing_record(
        &mut laptop.conn,
        NewSowingRecord {
            season_id: book.book.clone(),
            farm_id: book.farm.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: None,
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: book.plot.clone(),
                crop_id: Some(planted.clone()),
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id;
    repo::soft_delete_crop(&mut laptop.conn, &planted, None).unwrap();
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();

    let back = restore(&mut laptop, &book.book).unwrap();

    assert_eq!(back.records, 1, "the sowing alone");
    assert!(!removed(&laptop.conn, "sowing_record", &sown));
    assert!(removed(&laptop.conn, "crop", &planted));
    let crop_sets: i64 = laptop
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT origin_seq) FROM record_change
             WHERE root_table = 'crop' AND root_id = ?1",
            [&planted],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(crop_sets, 2, "created and removed, and nothing since");
}

#[test]
fn bringing_back_waits_while_a_conflict_in_the_book_waits() {
    // Situation 2's other half: the phone corrected a record while the laptop
    // deleted its book. Bringing the book back would settle that conflict with
    // nobody looking.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let book = shared_book(&mut phone, &mut laptop);
    let record = sow(&mut phone, &book, "original");
    sync_both(&mut phone, &mut laptop);
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(Some("corrected"), &[&book.plot]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(queue(&laptop.conn), 1);

    assert!(matches!(
        restore(&mut laptop, &book.book),
        Err(CoreError::Invalid("book_restore_conflicts_waiting"))
    ));
}

#[test]
fn a_book_renamed_elsewhere_while_it_was_deleted_offers_its_records_back() {
    // Found by running the design before building it: the rename, later on the
    // clock, brings the book back live and empty on every device, and nothing
    // listed its records — a record lost from view.
    let mut laptop = Device::new(A);
    let mut phone = Device::new(B);
    let book = shared_book(&mut laptop, &mut phone);
    let first = sow(&mut laptop, &book, "one");
    let second = crop(&mut laptop, &book);
    sync_both(&mut laptop, &mut phone);
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    let season = repo::get_season(&phone.conn, &book.book).unwrap();
    repo::update_season(
        &mut phone.conn,
        &book.book,
        UpdateSeason {
            starts_on: season.starts_on,
            ends_on: season.ends_on,
            custom_label: Some("Campaña del trigo".into()),
        },
        None,
    )
    .unwrap();
    sync_both(&mut laptop, &mut phone);

    // The book's own register is in conflict — removed against renamed — and a
    // person settles it first; the page line answers either way.
    for device in [&laptop, &phone] {
        assert!(device.conflicted("season", &book.book));
        assert!(repo::get_season(&device.conn, &book.book).is_ok(), "live");
        let offered = repo::removed_with_book(&device.conn, &book.book, &today_utc())
            .unwrap()
            .expect("its records are offered back");
        let mut counts: Vec<(String, usize)> = offered
            .records
            .into_iter()
            .map(|count| (count.table, count.count))
            .collect();
        counts.sort();
        assert_eq!(
            counts,
            vec![("crop".to_owned(), 1), ("sowing_record".to_owned(), 1)]
        );
        assert_eq!(offered.removed_on, A, "made on the laptop");
    }
    assert!(matches!(
        restore(&mut phone, &book.book),
        Err(CoreError::Invalid("book_restore_conflicts_waiting"))
    ));
    let heads = phone.heads_of("season", &book.book);
    let rename = heads.iter().find(|head| head.device == B).unwrap();
    resolve(
        &mut phone.conn,
        "season",
        &book.book,
        &rename.device,
        rename.seq,
        None,
    )
    .unwrap();

    let back = restore(&mut phone, &book.book).unwrap();
    assert_eq!(back.records, 2);
    assert_eq!(back.season.label, "Campaña del trigo");
    sync_both(&mut laptop, &mut phone);
    for device in [&laptop, &phone] {
        assert!(!removed(&device.conn, "sowing_record", &first));
        assert!(!removed(&device.conn, "crop", &second));
        assert!(
            repo::removed_with_book(&device.conn, &book.book, &today_utc())
                .unwrap()
                .is_none()
        );
        assert_eq!(queue(&device.conn), 0);
    }
}

#[test]
fn a_book_opened_again_comes_back_empty_and_offers_back_what_went_with_it() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let sown = sow(&mut laptop, &book, "sown");
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    let season = repo::insert_season(
        &mut laptop.conn,
        new_season(&book.farm, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    assert_eq!(season.id, book.book, "the same campaign, the same book");
    assert_eq!(
        repo::count_book_records(&laptop.conn, &book.book).unwrap(),
        0
    );
    let offered = repo::removed_with_book(&laptop.conn, &book.book, &today_utc())
        .unwrap()
        .expect("what went with it is offered back");
    assert_eq!(offered.records.len(), 1);

    let back = restore(&mut laptop, &book.book).unwrap();
    assert_eq!(back.records, 1);
    assert!(!removed(&laptop.conn, "sowing_record", &sown));
}

#[test]
fn a_deletion_is_offered_back_for_thirty_days_and_no_longer() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let sown = sow(&mut laptop, &book, "sown");
    repo::delete_book(&mut laptop.conn, &book.book, &[], None).unwrap();
    let today = today_utc();
    let last_day = add_days(&today, repo::REMOVED_BOOK_DAYS).unwrap();
    let too_late = add_days(&today, repo::REMOVED_BOOK_DAYS + 1).unwrap();

    assert_eq!(
        repo::list_removed_books(&laptop.conn, &last_day)
            .unwrap()
            .len(),
        1
    );
    // Past them it stays listed until the purge takes it, saying what that
    // waits for — but nothing is offered back.
    let listed = repo::list_removed_books(&laptop.conn, &too_late).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(
        listed[0].removal.records.is_empty() && listed[0].erasing.is_some(),
        "thirty days after it went, the book is no longer offered back"
    );

    // Asked for past the thirty days — from the list of records in a removed
    // book, which is not a list of deletions — the book comes back alone.
    let back = repo::restore_book(&mut laptop.conn, &book.book, &too_late, None).unwrap();
    assert_eq!(back.records, 0);
    assert!(back.season.deleted_at.is_none());
    assert!(removed(&laptop.conn, "sowing_record", &sown));
}

#[test]
fn the_removed_books_list_says_when_who_where_and_how_much() {
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let person = repo::insert_user_profile(
        &mut laptop.conn,
        NewUserProfile {
            display_name: "Ana".into(),
            operator_id: None,
        },
        None,
    )
    .unwrap();
    repo::register_this_device(&mut laptop.conn, None).unwrap();
    repo::rename_sync_peer(&mut laptop.conn, A, Some("Portátil"), None).unwrap();
    crop(&mut laptop, &book);
    sow(&mut laptop, &book, "one");
    sow(&mut laptop, &book, "two");
    let older = repo::insert_season(
        &mut laptop.conn,
        new_season(&book.farm, 2025, "2024/2025"),
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &older.id, &[], None).unwrap();
    repo::delete_book(&mut laptop.conn, &book.book, &[], Some(&person.id)).unwrap();
    // Another farm's removed book, and then the farm: it leaves every list.
    let gone = repo::insert_farm(&mut laptop.conn, new_farm("El Soto"), None).unwrap();
    let its_book = repo::insert_season(
        &mut laptop.conn,
        new_season(&gone.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &its_book.id, &[], None).unwrap();
    repo::soft_delete_farm(&mut laptop.conn, &gone.id, None).unwrap();

    let listed = repo::list_removed_books(&laptop.conn, &today_utc()).unwrap();

    let ids: Vec<&str> = listed
        .iter()
        .map(|entry| entry.season.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![book.book.as_str(), older.id.as_str()],
        "latest first"
    );
    let entry = &listed[0];
    assert_eq!(entry.farm_name, "Los Llanos");
    assert_eq!(entry.season.label, "2025/2026");
    assert_eq!(entry.removal.removed_on, A);
    assert_eq!(entry.removal.device_label.as_deref(), Some("Portátil"));
    assert_eq!(
        entry.removal.removed_by.as_deref(),
        Some(person.id.as_str())
    );
    assert_eq!(entry.removal.author_name.as_deref(), Some("Ana"));
    assert!(entry.removal.removed_at.starts_with(&today_utc()));
    assert_eq!(
        entry.removal.restorable_until,
        add_days(&today_utc(), repo::REMOVED_BOOK_DAYS).unwrap(),
        "the last day the list offers it back"
    );
    let counts: Vec<(&str, usize)> = entry
        .removal
        .records
        .iter()
        .map(|count| (count.table.as_str(), count.count))
        .collect();
    assert_eq!(counts, vec![("crop", 1), ("sowing_record", 2)]);
    assert!(listed[1].removal.records.is_empty(), "an empty book went");
}

#[test]
fn a_book_removed_by_a_merge_has_nothing_to_bring_back_but_itself() {
    // Its records went into the book that stayed: they were moved, not
    // removed, and bringing the book back leaves them where they are.
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    let twin = repo::insert_season(
        &mut laptop.conn,
        NewSeason {
            farm_id: book.farm.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap();
    let sown = repo::insert_sowing_record(
        &mut laptop.conn,
        NewSowingRecord {
            season_id: twin.id.clone(),
            farm_id: book.farm.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: None,
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: book.plot.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id;
    repo::merge_books(&mut laptop.conn, &book.book, &twin.id, &[], None).unwrap();

    let listed = repo::list_removed_books(&laptop.conn, &today_utc()).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].removal.records.is_empty());
    let back = restore(&mut laptop, &twin.id).unwrap();
    assert_eq!(back.records, 0);
    let in_book: String = laptop
        .conn
        .query_row(
            "SELECT season_id FROM sowing_record WHERE id = ?1",
            [&sown],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_book, book.book, "still in the book it went into");
}

#[test]
fn a_restored_duplicate_writes_only_what_its_removal_wrote() {
    // The other way back from a removal undoes it the same way: the record's
    // own row, its plot untouched — and it comes back whole on both devices.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let book = shared_book(&mut phone, &mut laptop);
    let first = sow(&mut phone, &book, "same");
    let second = sow(&mut phone, &book, "same");
    sync_both(&mut phone, &mut laptop);
    for (device, kept, removed) in [
        (&mut phone, &first, &second),
        (&mut laptop, &second, &first),
    ] {
        repo::keep_duplicate(
            &mut device.conn,
            &SOWING_DUPLICATES,
            kept,
            removed,
            None,
            repo::soft_delete_sowing_record_tx,
        )
        .unwrap();
    }
    sync_both(&mut phone, &mut laptop);

    repo::restore_removed_duplicate(&mut laptop.conn, &SOWING_DUPLICATES, &first, None).unwrap();

    let written = logged_by(&laptop.conn, &newest_change_set(&laptop.conn));
    let rows: Vec<(&str, &str)> = written
        .iter()
        .map(|(table, id, _)| (table.as_str(), id.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![("sowing_record", first.as_str())],
        "the record's own row"
    );
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert!(
            !removed(&device.conn, "sowing_record", &first),
            "{}",
            device.id
        );
        let plots: i64 = device
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sowing_plot WHERE sowing_record_id = ?1",
                [&first],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plots, 1, "{}: and its plot", device.id);
    }
}

// ---------------------------------------------------------------------------
// What the list and the page line cost
// ---------------------------------------------------------------------------

#[test]
fn the_list_and_the_page_line_do_not_grow_with_the_books_in_use() {
    // Both run on screens a farmer opens every day — the list of books and a
    // book's page — so what they read must grow with what was deleted, never
    // with what is kept. One database, measured before and after it grows: two
    // built apart differ in every random id, and SQLite's visiting order with
    // them.
    let mut laptop = Device::new(A);
    let book = book(&mut laptop);
    sow(&mut laptop, &book, "kept");
    let gone = repo::insert_season(
        &mut laptop.conn,
        new_season(&book.farm, 2025, "2024/2025"),
        None,
    )
    .unwrap();
    let gone_book = Book {
        farm: book.farm.clone(),
        plot: book.plot.clone(),
        book: gone.id.clone(),
    };
    sow(&mut laptop, &gone_book, "went with it");
    repo::delete_book(&mut laptop.conn, &gone.id, &[], None).unwrap();
    let today = today_utc();
    let measure = |device: &mut Device| {
        let (listed, list) = query_cost(&mut device.conn, |conn| {
            repo::list_removed_books(conn, &today).unwrap()
        });
        assert_eq!(listed.len(), 1);
        let (offered, line) = query_cost(&mut device.conn, |conn| {
            repo::removed_with_book(conn, &book.book, &today).unwrap()
        });
        assert!(offered.is_none());
        (list, line)
    };
    let (small_list, small_line) = measure(&mut laptop);

    for year in 0..30 {
        let year = 1990 + year;
        let older = repo::insert_season(
            &mut laptop.conn,
            new_season(&book.farm, year, &format!("{}/{year}", year - 1)),
            None,
        )
        .unwrap();
        let older = Book {
            farm: book.farm.clone(),
            plot: book.plot.clone(),
            book: older.id,
        };
        for n in 0..5 {
            sow(&mut laptop, &older, &format!("r{n}"));
        }
    }
    let (large_list, large_line) = measure(&mut laptop);

    assert_eq!(large_list.statements, small_list.statements);
    assert_eq!(large_line.statements, small_line.statements);
    // A range read on an index reads one entry past its range unless the range
    // is the last in it — a book added behind may sort after one that was —
    // so a fetch may cost one more row, and never more than that.
    let fetches = small_list.statements;
    assert!(
        large_list.rows.saturating_sub(small_list.rows) <= fetches,
        "the list read the books in use: {small_list:?} → {large_list:?}"
    );
    assert!(
        large_line.rows.saturating_sub(small_line.rows) <= small_line.statements,
        "the line read the books in use: {small_line:?} → {large_line:?}"
    );
}
