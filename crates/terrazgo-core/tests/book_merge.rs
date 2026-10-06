// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Merging two books that turned out to be one campaign, and the records left
//! in a removed book — against simulated devices (docs/sync.md → Merging two
//! books that turned out to be one campaign).
//!
//! The cases are the ones the design as first written failed, found by running
//! it before anything was built (P0–P6 in that section), and the race that
//! sank a name choice (the absorbed book edited elsewhere while it is merged).
//! Each is asserted here as what the corrected design does instead.
//!
//! Every device is a real database with its own id and every write goes through
//! the real repositories; only the transport is simulated (`common::devices`).
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::date::today_utc;
use terrazgo_core::duplicates::{DuplicatePolicy, HARVEST_DUPLICATES, SOWING_DUPLICATES};
use terrazgo_core::merge::{CORE_ROW_CAPTIONS, resolve, review};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::query_cost;

const POLICIES: [DuplicatePolicy; 2] = [SOWING_DUPLICATES, HARVEST_DUPLICATES];

/// One farm, a plot, and two books of one campaign that could not share a
/// name: the one named by its dates, created first, and the "bis" a collision
/// made somebody type.
struct Books {
    farm: String,
    plot: String,
    dated: String,
    bis: String,
}

fn two_books(device: &mut Device) -> Books {
    let farm = repo::insert_farm(&mut device.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut device.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let dated = repo::insert_season(
        &mut device.conn,
        new_season(&farm.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    let bis = repo::insert_season(
        &mut device.conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap();
    Books {
        farm: farm.id,
        plot: plot.id,
        dated: dated.id,
        bis: bis.id,
    }
}

/// The same two books on two devices, from shared history.
fn shared_books(a: &mut Device, b: &mut Device) -> Books {
    let books = two_books(a);
    sync_both(a, b);
    books
}

fn sow(device: &mut Device, books: &Books, season: &str, notes: &str) -> String {
    repo::insert_sowing_record(
        &mut device.conn,
        NewSowingRecord {
            season_id: season.into(),
            farm_id: books.farm.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: Some(notes.into()),
            plots: vec![NewSowingPlot {
                plot_id: books.plot.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn crop(device: &mut Device, books: &Books, season: &str) -> String {
    repo::insert_crop(
        &mut device.conn,
        NewCrop {
            plot_id: books.plot.clone(),
            season_id: season.into(),
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

fn load(device: &mut Device, books: &Books, season: &str) -> String {
    repo::insert_harvest_record(
        &mut device.conn,
        NewHarvestRecord {
            season_id: season.into(),
            farm_id: books.farm.clone(),
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
                plot_id: books.plot.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn merge(device: &mut Device, kept: &str, absorbed: &str) -> Result<repo::BookMerge, CoreError> {
    repo::merge_books(&mut device.conn, kept, absorbed, &POLICIES, None)
}

/// The book a row of `table` is in, and whether that book is removed.
fn book_of(conn: &Connection, table: &str, id: &str) -> (String, bool) {
    conn.query_row(
        &format!(
            "SELECT t.season_id, s.deleted_at IS NOT NULL FROM {table} t
             JOIN season s ON s.id = t.season_id WHERE t.id = ?1"
        ),
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
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

fn live_books(conn: &Connection) -> Vec<String> {
    repo::list_seasons(conn, 500, 0)
        .unwrap()
        .seasons
        .into_iter()
        .map(|season| season.id)
        .collect()
}

fn strays(conn: &Connection) -> Vec<repo::StrayBook> {
    repo::list_stray_records(conn, CORE_ROW_CAPTIONS).unwrap()
}

fn stray_ids(conn: &Connection) -> Vec<String> {
    strays(conn)
        .into_iter()
        .flat_map(|book| book.records.into_iter().map(|record| record.id))
        .collect()
}

// ---------------------------------------------------------------------------
// The merge
// ---------------------------------------------------------------------------

#[test]
fn merging_moves_every_live_record_of_the_book_that_goes_and_deletes_it() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    let kept_record = sow(&mut laptop, &books, &books.dated, "in the kept book");
    let sown = sow(&mut laptop, &books, &books.bis, "sown");
    let taken_back = sow(&mut laptop, &books, &books.bis, "removed");
    repo::soft_delete_sowing_record(&mut laptop.conn, &taken_back, None).unwrap();
    let planted = crop(&mut laptop, &books, &books.bis);
    let loaded = load(&mut laptop, &books, &books.bis);

    let merged = merge(&mut laptop, &books.dated, &books.bis).unwrap();

    assert_eq!(merged.kept.id, books.dated);
    assert_eq!(merged.moved, 3, "the live sowing, the crop, the load");
    for (table, id) in [
        ("sowing_record", &sown),
        ("crop", &planted),
        ("harvest_record", &loaded),
    ] {
        assert_eq!(
            book_of(&laptop.conn, table, id),
            (books.dated.clone(), false),
            "{table} {id} is in the kept book now"
        );
    }
    // A removed record nothing moved names stays in the book that went,
    // removed with it (docs/sync.md → The merge).
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &taken_back),
        (books.bis.clone(), true)
    );
    assert!(removed(&laptop.conn, "sowing_record", &taken_back));
    assert_eq!(live_books(&laptop.conn), vec![books.dated.clone()]);
    assert!(matches!(
        repo::get_season(&laptop.conn, &books.bis),
        Err(CoreError::NotFound)
    ));
    let listed: Vec<String> = repo::list_sowing_records(&laptop.conn, &books.dated, &books.farm)
        .unwrap()
        .into_iter()
        .map(|detail| detail.record.id)
        .collect();
    assert!(listed.contains(&kept_record) && listed.contains(&sown));
    assert!(stray_ids(&laptop.conn).is_empty(), "nothing is left behind");
}

#[test]
fn a_merge_is_one_change_set_and_rewrites_no_child_row() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    sow(&mut laptop, &books, &books.bis, "one");
    sow(&mut laptop, &books, &books.bis, "two");
    let before: i64 = laptop
        .conn
        .query_row("SELECT MAX(origin_seq) FROM record_change", [], |r| {
            r.get(0)
        })
        .unwrap();

    merge(&mut laptop, &books.dated, &books.bis).unwrap();

    let written: Vec<(i64, String, String)> = laptop
        .conn
        .prepare(
            "SELECT origin_seq, entity_table, operation FROM record_change
             WHERE origin_seq > ?1 ORDER BY id",
        )
        .unwrap()
        .query_map([before], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(
        written.iter().all(|(seq, ..)| *seq == before + 1),
        "one change set: {written:?}"
    );
    let tables: Vec<(&str, &str)> = written
        .iter()
        .map(|(_, table, operation)| (table.as_str(), operation.as_str()))
        .collect();
    assert_eq!(
        tables,
        vec![
            ("sowing_record", "update"),
            ("sowing_record", "update"),
            ("season", "delete"),
        ],
        "the records' own rows and the book — their plots are theirs, not the book's"
    );
}

#[test]
fn the_merged_book_frees_its_name() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    merge(&mut laptop, &books.bis, &books.dated).unwrap();
    // The kept book can now take the name the other went by.
    let kept = repo::get_season(&laptop.conn, &books.bis).unwrap();
    repo::update_season(
        &mut laptop.conn,
        &books.bis,
        UpdateSeason {
            starts_on: kept.starts_on,
            ends_on: kept.ends_on,
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(
        repo::get_season(&laptop.conn, &books.bis).unwrap().label,
        "2025/2026"
    );
}

#[test]
fn a_merge_is_refused_for_one_book_two_farms_or_a_removed_book() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    assert!(matches!(
        merge(&mut laptop, &books.dated, &books.dated),
        Err(CoreError::Invalid("book_merge_same_book"))
    ));

    let other = repo::insert_farm(&mut laptop.conn, new_farm("El Soto"), None).unwrap();
    let elsewhere = repo::insert_season(
        &mut laptop.conn,
        new_season(&other.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    assert!(matches!(
        merge(&mut laptop, &books.dated, &elsewhere.id),
        Err(CoreError::Invalid("book_merge_other_farm"))
    ));

    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    assert!(matches!(
        merge(&mut laptop, &books.dated, &books.bis),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn a_book_holding_only_removed_records_can_be_deleted_or_merged() {
    // P6: every "book in use" guard but the crops' counted removed records, so
    // this book could not be deleted. A deletion takes what is in a book now,
    // and finds nothing live here; a merge moves nothing, and the removed
    // record stays in the book that goes.
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    let gone = sow(&mut laptop, &books, &books.bis, "removed");
    repo::soft_delete_sowing_record(&mut laptop.conn, &gone, None).unwrap();
    assert_eq!(
        repo::count_book_records(&laptop.conn, &books.bis).unwrap(),
        0
    );

    let merged = merge(&mut laptop, &books.dated, &books.bis).unwrap();
    assert_eq!(merged.moved, 0);
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &gone),
        (books.bis.clone(), true)
    );
    assert!(
        stray_ids(&laptop.conn).is_empty(),
        "removed, so not stranded"
    );

    let mut phone = Device::new(B);
    let books = two_books(&mut phone);
    let gone = sow(&mut phone, &books, &books.bis, "removed");
    repo::soft_delete_sowing_record(&mut phone.conn, &gone, None).unwrap();
    assert_eq!(
        repo::delete_book(&mut phone.conn, &books.bis, &[], None).unwrap(),
        0
    );
    assert!(matches!(
        repo::get_season(&phone.conn, &books.bis),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn a_merge_waits_while_a_conflict_in_the_book_that_goes_waits() {
    // P3: a move is a write, and a write merges every version it saw — the
    // waiting version would vanish from the queue unseen.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let record = sow(&mut phone, &books, &books.bis, "original");
    sync_both(&mut phone, &mut laptop);
    let plot = books.plot.clone();
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(Some("phone"), &[&plot]),
        None,
    )
    .unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("laptop"), &[&plot]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(queue(&laptop.conn), 1);

    assert!(matches!(
        merge(&mut laptop, &books.dated, &books.bis),
        Err(CoreError::Invalid("book_merge_conflicts_waiting"))
    ));
    assert_eq!(queue(&laptop.conn), 1, "still waiting, for a person");

    // A conflict in the book that STAYS is not written by the merge, so it
    // does not hold it up — merging the other way round is refused, this way
    // is not.
    assert!(merge(&mut laptop, &books.bis, &books.dated).is_ok());
    assert_eq!(queue(&laptop.conn), 1, "and it is still waiting");
}

#[test]
fn a_merge_waits_while_the_book_holds_a_pair_removed_twice_over() {
    // The book that goes takes the pair out of the list of duplicates, which
    // reads live books only — the operation would be in the book zero times
    // with nothing left to say so.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let first = load(&mut phone, &books, &books.bis);
    let second = load(&mut phone, &books, &books.bis);
    sync_both(&mut phone, &mut laptop);
    repo::keep_duplicate(
        &mut phone.conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    repo::keep_duplicate(
        &mut laptop.conn,
        &HARVEST_DUPLICATES,
        &second,
        &first,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    assert!(matches!(
        merge(&mut laptop, &books.dated, &books.bis),
        Err(CoreError::Invalid("book_merge_removals_waiting"))
    ));
    repo::restore_removed_duplicate(&mut laptop.conn, &HARVEST_DUPLICATES, &first, None).unwrap();
    assert!(merge(&mut laptop, &books.dated, &books.bis).is_ok());
}

// ---------------------------------------------------------------------------
// Which book stays by default
// ---------------------------------------------------------------------------

fn season(id: &str, created_at: &str, custom_label: Option<&str>) -> Season {
    Season {
        id: id.into(),
        farm_id: "f".into(),
        label: custom_label.unwrap_or("2025/2026").into(),
        custom_label: custom_label.map(str::to_owned),
        starts_on: "2025-09-01".into(),
        ends_on: "2026-08-31".into(),
        status: "active".into(),
        created_at: created_at.into(),
        updated_at: created_at.into(),
        deleted_at: None,
    }
}

#[test]
fn a_book_is_offered_a_merge_only_with_the_books_its_campaign_overlaps() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    // The campaigns either side, touching the dated book's ends but sharing
    // no day with it: a farm's consecutive years.
    let before = repo::insert_season(
        &mut laptop.conn,
        new_season(&books.farm, 2025, "2024/2025"),
        None,
    )
    .unwrap();
    let after = repo::insert_season(
        &mut laptop.conn,
        new_season(&books.farm, 2027, "2026/2027"),
        None,
    )
    .unwrap();
    // A twin that went, and another farm's book of the same dates.
    let gone = repo::insert_season(
        &mut laptop.conn,
        NewSeason {
            farm_id: books.farm.clone(),
            starts_on: "2025-10-01".into(),
            ends_on: "2026-09-30".into(),
            custom_label: Some("2025/2026 ter".into()),
        },
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &gone.id, &[], None).unwrap();
    let neighbour = repo::insert_farm(&mut laptop.conn, new_farm("El Soto"), None).unwrap();
    repo::insert_season(
        &mut laptop.conn,
        new_season(&neighbour.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();

    let offered = |conn: &Connection, id: &str| -> Vec<String> {
        repo::merge_candidates(conn, id)
            .unwrap()
            .into_iter()
            .map(|book| book.id)
            .collect()
    };
    assert_eq!(offered(&laptop.conn, &books.dated), vec![books.bis.clone()]);
    assert_eq!(offered(&laptop.conn, &books.bis), vec![books.dated.clone()]);
    assert!(
        offered(&laptop.conn, &before.id).is_empty(),
        "consecutive campaigns are not one"
    );
    assert!(offered(&laptop.conn, &after.id).is_empty());
    assert!(matches!(
        repo::merge_candidates(&laptop.conn, &gone.id),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn the_book_named_by_its_dates_is_kept_by_default() {
    // The one a collision did NOT rename, whichever is older.
    let bis = season("a", "2026-01-01T00:00:00Z", Some("2025/2026 bis"));
    let dated = season("b", "2026-02-01T00:00:00Z", None);
    assert_eq!(repo::kept_by_default(&bis, &dated).id, "b");
    assert_eq!(repo::kept_by_default(&dated, &bis).id, "b");
}

#[test]
fn otherwise_the_older_book_is_kept_by_default_and_ties_go_by_id() {
    let older = season("b", "2026-01-01T00:00:00Z", Some("Primera"));
    let newer = season("a", "2026-02-01T00:00:00Z", Some("Segunda"));
    assert_eq!(repo::kept_by_default(&older, &newer).id, "b");
    assert_eq!(repo::kept_by_default(&newer, &older).id, "b");
    let one = season("a", "2026-01-01T00:00:00Z", None);
    let two = season("b", "2026-01-01T00:00:00Z", None);
    assert_eq!(repo::kept_by_default(&one, &two).id, "a");
    assert_eq!(repo::kept_by_default(&two, &one).id, "a");
}

// ---------------------------------------------------------------------------
// Records in a removed book
// ---------------------------------------------------------------------------

#[test]
fn a_record_written_into_a_book_deleted_elsewhere_is_listed_and_moved_back() {
    // P0 — reachable with no merge at all: an empty book deleted on one device
    // while the other records into it.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    let written = sow(&mut phone, &books, &books.bis, "on the phone");
    sync_both(&mut phone, &mut laptop);

    for device in [&phone, &laptop] {
        assert_eq!(
            book_of(&device.conn, "sowing_record", &written),
            (books.bis.clone(), true),
            "live, in a removed book"
        );
        let listed = strays(&device.conn);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].season.id, books.bis);
        assert_eq!(listed[0].farm_name, "Los Llanos");
        assert_eq!(listed[0].records.len(), 1);
        assert_eq!(listed[0].records[0].table, "sowing_record");
        assert_eq!(listed[0].records[0].id, written);
        assert_eq!(
            listed[0].records[0].caption.as_deref(),
            Some("2026-04-10"),
            "named as the register is named everywhere"
        );
        assert_eq!(
            listed[0].suggested.as_ref().map(|book| book.id.as_str()),
            Some(books.dated.as_str()),
            "the live book whose campaign overlaps it"
        );
    }
    assert_eq!(repo::count_stray_records(&laptop.conn).unwrap(), 1);

    let moved = repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None).unwrap();
    assert_eq!(moved, 1);
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert_eq!(
            book_of(&device.conn, "sowing_record", &written),
            (books.dated.clone(), false)
        );
        assert!(strays(&device.conn).is_empty());
    }
}

// ---------------------------------------------------------------------------
// What a moved record names goes with it (corrected 2026-10-02, slice 11b)
// ---------------------------------------------------------------------------

/// A sowing in `season` that names `crop`.
fn sow_on(device: &mut Device, books: &Books, season: &str, crop: &str) -> String {
    repo::insert_sowing_record(
        &mut device.conn,
        NewSowingRecord {
            season_id: season.into(),
            farm_id: books.farm.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: Some("on the crop".into()),
            plots: vec![NewSowingPlot {
                plot_id: books.plot.clone(),
                crop_id: Some(crop.into()),
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

/// The day after a removal made today stops being offered back.
fn a_month_on() -> String {
    terrazgo_core::date::add_days(&today_utc(), repo::REMOVED_BOOK_DAYS + 1).unwrap()
}

#[test]
fn a_merge_carries_the_removed_crop_a_moved_sowing_names() {
    // The crop was removed while a sowing still named it. Left in the book that
    // went, the sowing would name a row of a removed book for good, and that
    // book could never be erased (docs/sync.md → The merge).
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    let planted = crop(&mut laptop, &books, &books.bis);
    let sown = sow_on(&mut laptop, &books, &books.bis, &planted);
    let unnamed = sow(&mut laptop, &books, &books.bis, "removed, named by nothing");
    repo::soft_delete_crop(&mut laptop.conn, &planted, None).unwrap();
    repo::soft_delete_sowing_record(&mut laptop.conn, &unnamed, None).unwrap();

    let merged = merge(&mut laptop, &books.dated, &books.bis).unwrap();
    assert_eq!(merged.moved, 1, "the live sowing is the one record moved");
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &sown),
        (books.dated.clone(), false)
    );
    assert_eq!(
        book_of(&laptop.conn, "crop", &planted),
        (books.dated.clone(), false),
        "the crop went with the sowing that names it"
    );
    assert!(removed(&laptop.conn, "crop", &planted), "still removed");
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &unnamed),
        (books.bis.clone(), true),
        "a removed record nothing moved names stays where it was"
    );
    let erased = repo::purge_due(&mut laptop.conn, &a_month_on(), None).unwrap();
    assert_eq!(
        (erased.books, erased.records),
        (1, 1),
        "the book that went, with the sowing removed in it"
    );
    assert!(
        repo::list_removed_books(&laptop.conn, &a_month_on())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn moving_records_out_of_a_removed_book_carries_the_removed_crop_they_name() {
    // The phone sowed on the crop while the laptop deleted the book, the crop
    // with it. Moving the sowing to a live book takes the crop along.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let planted = crop(&mut laptop, &books, &books.bis);
    sync_both(&mut phone, &mut laptop);
    let sown = sow_on(&mut phone, &books, &books.bis, &planted);
    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(stray_ids(&laptop.conn), vec![sown.clone()]);

    repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None).unwrap();
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert_eq!(
            book_of(&device.conn, "sowing_record", &sown),
            (books.dated.clone(), false)
        );
        assert_eq!(
            book_of(&device.conn, "crop", &planted),
            (books.dated.clone(), false)
        );
        assert!(removed(&device.conn, "crop", &planted));
    }
    // Nothing that can go is left in the book that went, so a month on it has
    // gone for the farmer: the fold no longer lists it.
    assert!(
        repo::list_removed_books(&laptop.conn, &a_month_on())
            .unwrap()
            .iter()
            .all(|entry| entry.season.id != books.bis),
        "the removed book goes, nothing left naming it"
    );
}

#[test]
fn one_of_two_opposite_removals_stays_where_it_is() {
    // Each device kept the copy the other removed, and the phone sowed on its
    // own. Moving that crop would write on top of its removal, and bringing
    // one of the pair back needs the removal to be its newest version.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let first = crop(&mut laptop, &books, &books.bis);
    let second = crop(&mut laptop, &books, &books.bis);
    sync_both(&mut phone, &mut laptop);
    repo::keep_duplicate(
        &mut phone.conn,
        &terrazgo_core::duplicates::CROP_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_crop_tx,
    )
    .unwrap();
    let sown = sow_on(&mut phone, &books, &books.bis, &first);
    repo::keep_duplicate(
        &mut laptop.conn,
        &terrazgo_core::duplicates::CROP_DUPLICATES,
        &second,
        &first,
        None,
        repo::soft_delete_crop_tx,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(stray_ids(&laptop.conn), vec![sown.clone()]);
    let before = terrazgo_core::merge::heads(&laptop.conn, "crop", &first).unwrap();

    repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None).unwrap();
    assert_eq!(book_of(&laptop.conn, "sowing_record", &sown).0, books.dated);
    assert_eq!(
        book_of(&laptop.conn, "crop", &first),
        (books.bis.clone(), true),
        "left where it is"
    );
    assert_eq!(
        terrazgo_core::merge::heads(&laptop.conn, "crop", &first).unwrap(),
        before,
        "nothing written on top of its removal"
    );
}

#[test]
fn a_removed_book_can_be_brought_back_with_what_it_holds() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    let written = sow(&mut phone, &books, &books.bis, "on the phone");
    sync_both(&mut phone, &mut laptop);

    let back = repo::restore_book(&mut laptop.conn, &books.bis, &today_utc(), None)
        .unwrap()
        .season;
    assert!(back.deleted_at.is_none());
    assert_eq!(back.label, "2025/2026 bis", "as it was");
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &written),
        (books.bis.clone(), false)
    );
    assert!(strays(&laptop.conn).is_empty());
    // Bringing back a live book with nothing removed with it changes nothing.
    let again = repo::restore_book(&mut laptop.conn, &books.bis, &today_utc(), None).unwrap();
    assert_eq!(again.records, 0);
    assert_eq!(again.season.updated_at, back.updated_at);
}

#[test]
fn a_book_is_not_brought_back_under_a_name_another_live_book_has_taken() {
    let mut laptop = Device::new(A);
    let books = two_books(&mut laptop);
    // The dated book goes, and the other takes its name.
    repo::delete_book(&mut laptop.conn, &books.dated, &[], None).unwrap();
    let bis = repo::get_season(&laptop.conn, &books.bis).unwrap();
    repo::update_season(
        &mut laptop.conn,
        &books.bis,
        UpdateSeason {
            starts_on: bis.starts_on,
            ends_on: bis.ends_on,
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert!(matches!(
        repo::restore_book(&mut laptop.conn, &books.dated, &today_utc(), None),
        Err(CoreError::Invalid("season_restore_name_taken"))
    ));
    // A removed farm's book stays removed with it.
    let gone = repo::insert_farm(&mut laptop.conn, new_farm("El Soto"), None).unwrap();
    let its_book = repo::insert_season(
        &mut laptop.conn,
        new_season(&gone.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    repo::delete_book(&mut laptop.conn, &its_book.id, &[], None).unwrap();
    repo::soft_delete_farm(&mut laptop.conn, &gone.id, None).unwrap();
    assert!(matches!(
        repo::restore_book(&mut laptop.conn, &its_book.id, &today_utc(), None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn a_record_written_before_the_merge_arrived_is_listed_and_goes_where_the_book_went() {
    // P1.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let moved = sow(&mut laptop, &books, &books.bis, "on the laptop");
    sync_both(&mut phone, &mut laptop);
    merge(&mut laptop, &books.dated, &books.bis).unwrap();
    let late = sow(&mut phone, &books, &books.bis, "merge not seen yet");
    sync_both(&mut phone, &mut laptop);

    for device in [&phone, &laptop] {
        assert_eq!(
            book_of(&device.conn, "sowing_record", &moved).0,
            books.dated
        );
        assert_eq!(stray_ids(&device.conn), vec![late.clone()]);
        let listed = strays(&device.conn);
        assert_eq!(
            listed[0].suggested.as_ref().map(|book| book.id.as_str()),
            Some(books.dated.as_str()),
            "after a merge, the book it went into"
        );
    }
    repo::move_stray_records(&mut phone.conn, &books.bis, &books.dated, None).unwrap();
    assert_eq!(
        book_of(&phone.conn, "sowing_record", &late),
        (books.dated.clone(), false)
    );
}

#[test]
fn an_edit_made_while_the_record_moved_is_reviewed_with_the_book_it_names() {
    // P2: the edit, later on the clock, goes live in the removed book.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let record = sow(&mut laptop, &books, &books.bis, "original");
    sync_both(&mut phone, &mut laptop);
    merge(&mut phone, &books.dated, &books.bis).unwrap();
    // Written second, by B: later on the clock, or B's id breaks the tie.
    let plot = books.plot.clone();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("edited"), &[&plot]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    assert_eq!(queue(&laptop.conn), 1);
    assert_eq!(stray_ids(&laptop.conn), vec![record.clone()]);
    // The review says the versions disagree about the book, by name.
    let shown = review(&laptop.conn, "sowing_record", &record, CORE_ROW_CAPTIONS).unwrap();
    let book = shown
        .lines
        .iter()
        .find(|line| line.column == "season_id")
        .expect("the book is compared");
    let named: Vec<Option<String>> = book
        .values
        .iter()
        .map(|value| value.as_ref().and_then(|value| value.display.clone()))
        .collect();
    assert_eq!(
        named,
        vec![Some("2025/2026 bis".into()), Some("2025/2026".into())],
        "the live version (the edit) first"
    );
    // Moving it now would decide the conflict unseen.
    assert!(matches!(
        repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None),
        Err(CoreError::Invalid("stray_conflicts_waiting"))
    ));
    // Keep the edit, then move it: the edit ends up in the book that stayed.
    let edit = laptop
        .heads_of("sowing_record", &record)
        .into_iter()
        .find(|head| head.device == B)
        .unwrap();
    resolve(
        &mut laptop.conn,
        "sowing_record",
        &record,
        &edit.device,
        edit.seq,
        None,
    )
    .unwrap();
    repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None).unwrap();
    let notes: Option<String> = laptop
        .conn
        .query_row(
            "SELECT notes FROM sowing_record WHERE id = ?1",
            [&record],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(notes.as_deref(), Some("edited"));
    assert_eq!(
        book_of(&laptop.conn, "sowing_record", &record),
        (books.dated.clone(), false)
    );
}

#[test]
fn the_book_that_goes_edited_elsewhere_comes_back_empty_under_its_own_name() {
    // The race that sank a name choice: here no name is reused, so the edit —
    // later on the clock — brings the book back empty, with one conflict to
    // close, and nobody's files are refused.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let record = sow(&mut laptop, &books, &books.bis, "in bis");
    sync_both(&mut phone, &mut laptop);
    merge(&mut phone, &books.dated, &books.bis).unwrap();
    // Written second, by B: later on the clock, or B's id breaks the tie.
    repo::update_season(
        &mut laptop.conn,
        &books.bis,
        UpdateSeason {
            starts_on: "2025-09-16".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    for device in [&phone, &laptop] {
        assert_eq!(
            book_of(&device.conn, "sowing_record", &record).0,
            books.dated
        );
        assert!(device.conflicted("season", &books.bis));
        assert_eq!(queue(&device.conn), 1);
        let mut books_now = live_books(&device.conn);
        books_now.sort();
        let mut expected = vec![books.dated.clone(), books.bis.clone()];
        expected.sort();
        assert_eq!(books_now, expected, "back, and empty");
    }
}

#[test]
fn two_devices_merging_one_pair_the_same_way_leave_nothing_to_review() {
    // P4, and the rule that answers it: versions that say the same thing are
    // not listed.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let records: Vec<String> = (0..5)
        .map(|n| sow(&mut laptop, &books, &books.bis, &format!("r{n}")))
        .collect();
    sync_both(&mut phone, &mut laptop);
    merge(&mut laptop, &books.dated, &books.bis).unwrap();
    merge(&mut phone, &books.dated, &books.bis).unwrap();
    sync_both(&mut phone, &mut laptop);

    for device in [&phone, &laptop] {
        assert_eq!(
            queue(&device.conn),
            0,
            "every version agrees with the other"
        );
        assert!(
            device.conflicted("sowing_record", &records[0]),
            "two heads in the log, all the same"
        );
        assert_eq!(live_books(&device.conn), vec![books.dated.clone()]);
        for record in &records {
            assert_eq!(
                book_of(&device.conn, "sowing_record", record),
                (books.dated.clone(), false)
            );
        }
    }
}

#[test]
fn two_devices_merging_one_pair_opposite_ways_lose_nothing() {
    // P5, the cost of letting the person choose: both books go, every record is
    // listed, and bringing one back and moving the other's records in mends it.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let in_dated = sow(&mut laptop, &books, &books.dated, "dated");
    let in_bis = sow(&mut laptop, &books, &books.bis, "bis");
    sync_both(&mut phone, &mut laptop);
    merge(&mut laptop, &books.dated, &books.bis).unwrap();
    merge(&mut phone, &books.bis, &books.dated).unwrap();
    sync_both(&mut phone, &mut laptop);

    assert!(live_books(&laptop.conn).is_empty());
    let mut listed = stray_ids(&laptop.conn);
    listed.sort();
    let mut expected = vec![in_dated.clone(), in_bis.clone()];
    expected.sort();
    assert_eq!(listed, expected, "every record, listed");
    assert!(
        strays(&laptop.conn)
            .iter()
            .all(|book| book.suggested.is_none()),
        "no live book to suggest"
    );

    repo::restore_book(&mut laptop.conn, &books.dated, &today_utc(), None).unwrap();
    repo::move_stray_records(&mut laptop.conn, &books.bis, &books.dated, None).unwrap();
    for record in [&in_dated, &in_bis] {
        assert_eq!(
            book_of(&laptop.conn, "sowing_record", record),
            (books.dated.clone(), false)
        );
    }
    assert!(strays(&laptop.conn).is_empty());
}

#[test]
fn a_book_of_a_removed_farm_is_not_listed() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
    sow(&mut phone, &books, &books.bis, "on the phone");
    sync_both(&mut phone, &mut laptop);
    assert_eq!(strays(&laptop.conn).len(), 1);
    repo::soft_delete_farm(&mut laptop.conn, &books.farm, None).unwrap();
    assert!(
        strays(&laptop.conn).is_empty(),
        "the farm went, and everything with it"
    );
}

#[test]
fn the_list_costs_the_same_however_many_books_and_records_are_in_use() {
    // Worked out on every visit to the Status view, so what it reads must not
    // grow with the books still in use or the records in them — only with what
    // is in removed books, which is one record here in both sizes.
    let cost = |older_books: i64, records: usize, left_behind: usize| {
        let mut phone = Device::new(A);
        let mut laptop = Device::new(B);
        let books = shared_books(&mut phone, &mut laptop);
        for year in 0..older_books {
            let year = 1950 + year;
            repo::insert_season(
                &mut laptop.conn,
                new_season(&books.farm, year, &format!("{}/{year}", year - 1)),
                None,
            )
            .unwrap();
        }
        for n in 0..records {
            sow(&mut laptop, &books, &books.dated, &format!("r{n}"));
        }
        repo::delete_book(&mut laptop.conn, &books.bis, &[], None).unwrap();
        for n in 0..left_behind {
            sow(&mut phone, &books, &books.bis, &format!("left behind {n}"));
        }
        sync(&phone, &mut laptop);
        let (listed, cost) = query_cost(&mut laptop.conn, |conn| {
            repo::list_stray_records(conn, CORE_ROW_CAPTIONS).unwrap()
        });
        assert_eq!(listed.len(), 1);
        cost
    };
    let small = cost(0, 1, 1);
    let large = cost(40, 200, 1);
    // The control: the instrument does count what is in removed books.
    let more_left_behind = cost(0, 1, 2);
    assert!(
        more_left_behind.rows > small.rows,
        "the counter must be able to grow"
    );
    assert_eq!(
        small.statements, large.statements,
        "one statement per table, and per removed book"
    );
    assert_eq!(
        small.rows, large.rows,
        "no row read to be thrown away: the older books do not overlap the removed one"
    );
}

// ---------------------------------------------------------------------------
// Versions that say the same thing are not listed
// ---------------------------------------------------------------------------

#[test]
fn two_people_removing_the_same_copy_leave_nothing_to_review() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let record = sow(&mut phone, &books, &books.dated, "a copy");
    sync_both(&mut phone, &mut laptop);
    repo::soft_delete_sowing_record(&mut phone.conn, &record, None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    repo::soft_delete_sowing_record(&mut laptop.conn, &record, None).unwrap();
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert!(device.conflicted("sowing_record", &record));
        assert_eq!(
            queue(&device.conn),
            0,
            "removed a second apart is removed alike"
        );
    }
}

#[test]
fn a_removal_against_a_correction_is_still_reviewed() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let books = shared_books(&mut phone, &mut laptop);
    let record = sow(&mut phone, &books, &books.dated, "a copy");
    sync_both(&mut phone, &mut laptop);
    repo::soft_delete_sowing_record(&mut phone.conn, &record, None).unwrap();
    let plot = books.plot.clone();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("corrected"), &[&plot]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert_eq!(queue(&device.conn), 1);
    }
    let lines = review(&laptop.conn, "sowing_record", &record, CORE_ROW_CAPTIONS)
        .unwrap()
        .lines;
    assert!(lines.iter().any(|line| line.column == "deleted_at"));
}

#[test]
fn of_three_versions_only_the_one_that_differs_waits() {
    // The tablet writes last (or wins the tie on its id), so its "El Soto" is
    // live; the phone's agrees with it, the laptop's does not.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let tablet = &mut Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, tablet]);
    rename(&mut phone, &farm_id, "El Soto");
    rename(&mut laptop, &farm_id, "La Vega");
    rename(tablet, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut laptop, tablet);
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop, &*tablet] {
        assert_eq!(device.heads_of("farm", &farm_id).len(), 3);
        assert_eq!(device.farm_name(&farm_id).as_deref(), Some("El Soto"));
        let waiting: Vec<String> = device
            .conflict_rows("farm", &farm_id)
            .into_iter()
            .map(|(other, _)| other)
            .collect();
        assert_eq!(
            waiting,
            vec![B.to_string()],
            "the laptop's version alone waits"
        );
    }
}
