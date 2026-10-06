// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deleting a book, as it meets this module's one register keyed by its book
//! (docs/sync.md → Deleting a book with its records).
//!
//! Each register of the book is pinned in its own file — deleted with its
//! book, brought back whole. What is left for here is the declaration ("APLICA
//! TRATAMIENTO: NO"): a slot whose key names the book, which the deletion
//! withdraws and bringing the book back states again — and leaves withdrawn
//! when it was withdrawn before the book went.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::last_change;

use module_phytosanitary::models::*;
use module_phytosanitary::open_in_memory;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;

struct Book {
    farm_id: String,
    season_id: String,
}

fn book(conn: &mut Connection) -> Book {
    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            country_code: "es".into(),
            ..NewFarm::default()
        },
        None,
    )
    .unwrap()
    .id;
    let season_id = repo::insert_season(
        conn,
        NewSeason {
            farm_id: farm_id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap()
    .id;
    Book { farm_id, season_id }
}

fn declared(conn: &Connection, book: &Book) -> Vec<RegisterDeclaration> {
    repo::list_register_declarations(conn, &book.farm_id, &book.season_id).unwrap()
}

fn restore(conn: &mut Connection, book: &Book) -> terrazgo_core::repository::RestoredBook {
    terrazgo_core::repository::restore_book(
        conn,
        &book.season_id,
        &terrazgo_core::date::today_utc(),
        None,
    )
    .unwrap()
}

#[test]
fn a_declaration_goes_with_its_book_and_comes_back_with_it() {
    let mut conn = open_in_memory().unwrap();
    let book = book(&mut conn);
    let said = repo::set_register_declaration(
        &mut conn,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        "2026-03-01",
        None,
    )
    .unwrap();

    assert_eq!(
        terrazgo_core::repository::delete_book(&mut conn, &book.season_id, &[], None).unwrap(),
        0,
        "a declaration is not a record, and the confirmation does not count it"
    );
    assert!(declared(&conn, &book).is_empty());
    let (operation, _, after) = last_change(&conn, "register_declaration", &said.id);
    assert_eq!(operation, "delete");
    assert!(!after["deleted_at"].is_null());

    let back = restore(&mut conn, &book);
    assert_eq!(back.records, 0);
    let now = declared(&conn, &book);
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].id, said.id, "the same row, stated again");
    assert_eq!(now[0].declared_on, "2026-03-01", "on the day it was said");
}

#[test]
fn a_declaration_withdrawn_before_the_book_went_stays_withdrawn() {
    let mut conn = open_in_memory().unwrap();
    let book = book(&mut conn);
    repo::set_register_declaration(
        &mut conn,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        "2026-03-01",
        None,
    )
    .unwrap();
    repo::clear_register_declaration(
        &mut conn,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        None,
    )
    .unwrap();
    terrazgo_core::repository::delete_book(&mut conn, &book.season_id, &[], None).unwrap();

    restore(&mut conn, &book);
    assert!(
        declared(&conn, &book).is_empty(),
        "taken back on its own, so it stays taken back"
    );
}

#[test]
fn a_declaration_is_erased_with_its_book_and_counted_as_neither() {
    // A slot's row is not a record a person entered: the purge takes it, and
    // the import's message counts the book alone — which went, for the
    // farmer, though its row stays (docs/sync.md → What can go).
    let mut conn = open_in_memory().unwrap();
    let book = book(&mut conn);
    repo::set_register_declaration(
        &mut conn,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        "2026-03-01",
        None,
    )
    .unwrap();
    terrazgo_core::repository::delete_book(&mut conn, &book.season_id, &[], None).unwrap();
    let a_month_on = terrazgo_core::date::add_days(
        &terrazgo_core::date::today_utc(),
        terrazgo_core::repository::REMOVED_BOOK_DAYS + 1,
    )
    .unwrap();
    let erased = terrazgo_core::repository::purge_due(&mut conn, &a_month_on, None).unwrap();
    assert_eq!(
        (erased.registers, erased.books, erased.records),
        (1, 1, 0),
        "its declaration; one book, no records"
    );
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM register_declaration", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn a_declaration_made_again_after_its_erasure_is_stamped_on_top_of_it() {
    // A slot is keyed by what it is — its book and register — so declaring the
    // same register in the same book again, once the campaign is opened
    // again, writes the register the purge erased. The phone made the first
    // declaration and the laptop erased it, so the removal names the phone;
    // the laptop holds nothing of that history any more, and its new
    // declaration is newer than the deletion only because every stamp reads
    // the marker (docs/sync.md → It travels as a row per register — and the
    // row is a marker).
    use terrazgo_testkit::sync::send;
    let mut laptop = open_in_memory().unwrap();
    let mut phone = open_in_memory().unwrap();
    terrazgo_core::repository::register_this_device(&mut laptop, None).unwrap();
    terrazgo_core::repository::register_this_device(&mut phone, None).unwrap();
    let group = terrazgo_core::sync::ensure_sync_group(&laptop).unwrap();
    terrazgo_core::sync::join_sync_group(&phone, &group).unwrap();
    let book = book(&mut laptop);
    let season = terrazgo_core::repository::get_season(&laptop, &book.season_id).unwrap();
    send(&laptop, &mut phone);
    repo::set_register_declaration(
        &mut phone,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        "2026-03-01",
        None,
    )
    .unwrap();
    send(&phone, &mut laptop);
    terrazgo_core::repository::delete_book(&mut laptop, &book.season_id, &[], None).unwrap();
    send(&laptop, &mut phone);
    send(&phone, &mut laptop);
    let a_month_on = terrazgo_core::date::add_days(
        &terrazgo_core::date::today_utc(),
        terrazgo_core::repository::REMOVED_BOOK_DAYS + 1,
    )
    .unwrap();
    let erased = terrazgo_core::repository::purge_due(&mut laptop, &a_month_on, None).unwrap();
    assert_eq!(erased.registers, 1, "the declaration");
    let removal: String = laptop
        .query_row(
            "SELECT removal FROM purged_register WHERE root_table = 'register_declaration'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let removal = terrazgo_core::sync::VersionVector::from_json(&removal).unwrap();
    let phone_id = terrazgo_core::sync::installed_device(&phone).unwrap();
    assert!(
        removal.get(&phone_id) > 0,
        "the control: the removal names the phone"
    );

    let again = terrazgo_core::repository::insert_season(
        &mut laptop,
        terrazgo_core::models::NewSeason {
            farm_id: book.farm_id.clone(),
            starts_on: season.starts_on,
            ends_on: season.ends_on,
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(again.id, book.season_id, "the campaign's id is derived");
    let said = repo::set_register_declaration(
        &mut laptop,
        &book.farm_id,
        &book.season_id,
        "seed_treatment",
        "2026-04-01",
        None,
    )
    .unwrap();
    let slot: String = laptop
        .query_row(
            "SELECT root_id FROM record_change WHERE entity_id = ?1",
            [&said.id],
            |r| r.get(0),
        )
        .unwrap();
    let current = terrazgo_core::merge::heads(&laptop, "register_declaration", &slot).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(
        current[0].vector.compare(&removal),
        terrazgo_core::sync::Causality::Newer,
        "newer than the deletion everywhere, not a rival to it"
    );
}
