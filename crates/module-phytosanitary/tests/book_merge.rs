// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Merging two books, as it meets this module's registers (docs/sync.md →
//! Merging two books that turned out to be one campaign).
//!
//! The merge is core's and generic — it finds a book's tables in the installed
//! aggregate map — so what is pinned here is that it finds THESE: a register of
//! this crate moves with its book, and a register declared empty ("APLICA
//! TRATAMIENTO: NO"), whose slot is keyed by its book, is withdrawn from the
//! book that goes and stated in the one that stays unless that one already
//! states it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::last_change;

use module_phytosanitary::models::*;
use module_phytosanitary::open_in_memory;
use module_phytosanitary::repository as repo;
use rusqlite::Connection;

struct Books {
    farm_id: String,
    plot_id: String,
    kept: String,
    absorbed: String,
}

fn two_books(conn: &mut Connection) -> Books {
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
    let book = |conn: &mut Connection, starts_on: &str, label: Option<&str>| {
        repo::insert_season(
            conn,
            NewSeason {
                farm_id: farm_id.clone(),
                starts_on: starts_on.into(),
                ends_on: "2026-08-31".into(),
                custom_label: label.map(str::to_owned),
            },
            None,
        )
        .unwrap()
        .id
    };
    let kept = book(conn, "2025-09-01", None);
    let absorbed = book(conn, "2025-09-15", Some("2025/2026 bis"));
    let plot_id = repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.clone(),
            name: "El Prado".into(),
            area_ha: Some(4.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    Books {
        farm_id,
        plot_id,
        kept,
        absorbed,
    }
}

fn declare(conn: &mut Connection, books: &Books, season: &str, day: &str) -> RegisterDeclaration {
    repo::set_register_declaration(conn, &books.farm_id, season, "seed_treatment", day, None)
        .unwrap()
}

fn sow_treated_seed(conn: &mut Connection, books: &Books, season: &str) -> String {
    repo::insert_seed_treatment(
        conn,
        NewSeedTreatment {
            season_id: season.into(),
            farm_id: books.farm_id.clone(),
            sown_on: "2025-11-10".into(),
            species_name: "trigo blando".into(),
            variety: None,
            crop_code: None,
            seed_quantity_kg: Some(680.0),
            seed_lot: None,
            treatment_kind_code: Some("purchased_es".into()),
            acquired_on: None,
            sowing_record_id: None,
            product_name: "Celest Trio".into(),
            product_registration_number: None,
            product_active_substance: None,
            product_id: None,
            efficacy_code: None,
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: books.plot_id.clone(),
                surface_sown_ha: 3.2,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn merge(conn: &mut Connection, books: &Books) {
    terrazgo_core::repository::merge_books(conn, &books.kept, &books.absorbed, &[], None).unwrap();
}

fn declared(conn: &Connection, books: &Books, season: &str) -> Vec<RegisterDeclaration> {
    repo::list_register_declarations(conn, &books.farm_id, season).unwrap()
}

#[test]
fn a_register_of_this_module_moves_with_its_book() {
    let mut conn = open_in_memory().unwrap();
    let books = two_books(&mut conn);
    let sown = sow_treated_seed(&mut conn, &books, &books.absorbed);
    merge(&mut conn, &books);
    let season: String = conn
        .query_row(
            "SELECT season_id FROM seed_treatment WHERE id = ?1",
            [&sown],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(season, books.kept);
    let (operation, before, after) = last_change(&conn, "seed_treatment", &sown);
    assert_eq!(operation, "update");
    assert_eq!(before["season_id"], books.absorbed.as_str());
    assert_eq!(after["season_id"], books.kept.as_str());
}

#[test]
fn a_declaration_moves_into_a_book_that_declared_nothing() {
    let mut conn = open_in_memory().unwrap();
    let books = two_books(&mut conn);
    let theirs = declare(&mut conn, &books, &books.absorbed, "2026-03-01");
    merge(&mut conn, &books);

    let now = declared(&conn, &books, &books.kept);
    assert_eq!(now.len(), 1, "the book that stays says it now");
    assert_eq!(now[0].register_code, "seed_treatment");
    assert_eq!(now[0].declared_on, "2026-03-01", "on the day it was said");
    assert_ne!(
        now[0].id, theirs.id,
        "a row of its own: the book is part of the slot, so this is another register"
    );
    let (operation, _, after) = last_change(&conn, "register_declaration", &theirs.id);
    assert_eq!(operation, "delete", "withdrawn from the book that went");
    assert!(!after["deleted_at"].is_null());
}

#[test]
fn a_book_that_declared_the_same_keeps_its_own_declaration() {
    let mut conn = open_in_memory().unwrap();
    let books = two_books(&mut conn);
    let ours = declare(&mut conn, &books, &books.kept, "2026-02-01");
    declare(&mut conn, &books, &books.absorbed, "2026-03-01");
    merge(&mut conn, &books);

    let now = declared(&conn, &books, &books.kept);
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].id, ours.id);
    assert_eq!(now[0].declared_on, "2026-02-01");
}

#[test]
fn a_declaration_is_carried_beside_records_that_contradict_it() {
    // Every screen and the printed book let records win over a declaration
    // ("SÍ"), which is the state two devices already reach by declaring and
    // recording offline. The merge adds no rule of its own.
    let mut conn = open_in_memory().unwrap();
    let books = two_books(&mut conn);
    sow_treated_seed(&mut conn, &books, &books.kept);
    declare(&mut conn, &books, &books.absorbed, "2026-03-01");
    merge(&mut conn, &books);
    assert_eq!(declared(&conn, &books, &books.kept).len(), 1);
}

#[test]
fn a_carried_sowing_moves_by_its_own_row_and_keeps_its_plots() {
    // A treated-seed record names the sowing that used its seed, and the
    // sowing was removed before the merge: the record moves, and the sowing it
    // names goes with it, still removed (docs/sync.md → The merge). Only the
    // sowing's own row names a book, so only that row is written; its plot
    // follows it on every device, which holds its history.
    use terrazgo_core::models::{NewSowingPlot, NewSowingRecord};
    use terrazgo_core::repository as core_repo;
    use terrazgo_testkit::sync::send;
    let mut laptop = open_in_memory().unwrap();
    let mut phone = open_in_memory().unwrap();
    core_repo::register_this_device(&mut laptop, None).unwrap();
    core_repo::register_this_device(&mut phone, None).unwrap();
    let group = terrazgo_core::sync::ensure_sync_group(&laptop).unwrap();
    terrazgo_core::sync::join_sync_group(&phone, &group).unwrap();
    let books = two_books(&mut laptop);
    let sown = core_repo::insert_sowing_record(
        &mut laptop,
        NewSowingRecord {
            season_id: books.absorbed.clone(),
            farm_id: books.farm_id.clone(),
            kind_code: "sowing".into(),
            sown_on: "2025-11-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(680.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: books.plot_id.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id;
    repo::insert_seed_treatment(
        &mut laptop,
        NewSeedTreatment {
            season_id: books.absorbed.clone(),
            farm_id: books.farm_id.clone(),
            sown_on: "2025-11-10".into(),
            species_name: "trigo blando".into(),
            variety: None,
            crop_code: None,
            seed_quantity_kg: Some(680.0),
            seed_lot: None,
            treatment_kind_code: Some("purchased_es".into()),
            acquired_on: None,
            sowing_record_id: Some(sown.clone()),
            product_name: "Celest Trio".into(),
            product_registration_number: None,
            product_active_substance: None,
            product_id: None,
            efficacy_code: None,
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: books.plot_id.clone(),
                surface_sown_ha: 3.2,
            }],
        },
        None,
    )
    .unwrap();
    core_repo::soft_delete_sowing_record(&mut laptop, &sown, None).unwrap();
    send(&laptop, &mut phone);

    merge(&mut laptop, &books);

    let written: Vec<(String, String)> = laptop
        .prepare(
            "SELECT entity_table, entity_id FROM record_change
             WHERE root_table = 'sowing_record' AND root_id = ?1
               AND origin_seq = (SELECT MAX(origin_seq) FROM record_change)
             ORDER BY id",
        )
        .unwrap()
        .query_map([&sown], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        written,
        vec![("sowing_record".to_owned(), sown.clone())],
        "the sowing's own row — not its plot"
    );
    send(&laptop, &mut phone);
    for (device, conn) in [("laptop", &laptop), ("phone", &phone)] {
        let (season, removed, plots): (String, bool, i64) = conn
            .query_row(
                "SELECT season_id, deleted_at IS NOT NULL,
                        (SELECT COUNT(*) FROM sowing_plot WHERE sowing_record_id = sowing_record.id)
                 FROM sowing_record WHERE id = ?1",
                [&sown],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(season, books.kept, "{device}: in the book that stays");
        assert!(removed, "{device}: still removed");
        assert_eq!(plots, 1, "{device}: with its plot");
    }
}
