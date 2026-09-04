// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Does listing a campaign of section 9 cost more as the campaign fills up?
//!
//! Each register is read as a whole — by the register view, by the book, by the
//! exporter — so the statements a listing runs must be bounded by the number of
//! CHILD TABLES it hydrates, never by the number of records. A per-record child
//! query returns exactly the right rows while doing it, which is why the
//! correctness tests in the sibling files cannot fail on this.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{CoreFixture, FarmWithPlots, PlotSpec, farm_with_plots};
use module_ecoscheme::models::*;
use module_ecoscheme::open_in_memory;
use module_ecoscheme::repository as repo;
use rusqlite::Connection;
use terrazgo_testkit::query_cost;

/// Few records against four times as many. Both are small: the defect is a
/// count that MOVES, and four times nothing is still four times.
const FEW: usize = 3;
const MANY: usize = 12;

fn fixture(conn: &mut Connection) -> CoreFixture {
    farm_with_plots(
        conn,
        FarmWithPlots {
            farm_name: "Dehesa de Arriba".into(),
            plot_a: PlotSpec::new("Pasto Alto", 22.0),
            plot_b: PlotSpec::new("Pasto Bajo", 18.0),
            other_farm_plot: PlotSpec::new("Ajena", 5.0),
            ..Default::default()
        },
    )
}

/// Build `count` records with `insert`, then report what listing them costs.
fn statements_to_list<T>(
    count: usize,
    insert: impl Fn(&mut Connection, &CoreFixture, usize),
    list: impl Fn(&Connection, &CoreFixture) -> Vec<T>,
) -> (usize, usize) {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    for n in 0..count {
        insert(&mut conn, &fx, n);
    }
    let (listed, cost) = query_cost(&mut conn, |conn| list(conn, &fx));
    (listed.len(), cost.statements)
}

/// The day of the month a record is dated, so consecutive rows differ.
fn day(n: usize) -> String {
    format!("2026-05-{:02}", n + 1)
}

#[test]
fn listing_grazings_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        repo::insert_grazing_record(
            conn,
            NewGrazingRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                practice_code: "extensive_grazing".into(),
                plot_group_ref: None,
                soil_cover_id: None,
                started_on: day(n),
                ended_on: None,
                notes: None,
                plot_ids: vec![fx.plot_a.clone(), fx.plot_b.clone()],
                // ESPECIE_ANIMAL 03 = Ovinos, 04 = Caprinos.
                animals: vec![
                    GrazingAnimal {
                        id: String::new(),
                        grazing_record_id: String::new(),
                        species_code: "03".into(),
                        rega_code: "ES071234560001".into(),
                        animal_count: 120,
                    },
                    GrazingAnimal {
                        id: String::new(),
                        grazing_record_id: String::new(),
                        species_code: "04".into(),
                        rega_code: "ES071234560001".into(),
                        animal_count: 15,
                    },
                ],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_grazing_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(
        few_statements, many_statements,
        "grazing: {few_statements} statements for {FEW} records, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_cultural_operations_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        repo::insert_cultural_operation(
            conn,
            NewCulturalOperation {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                practice_code: "sustainable_mowing".into(),
                operation_kind_code: "mowing".into(),
                performed_on: day(n),
                performed_end_date: None,
                activity_description: None,
                residue_destination_code: None,
                soil_cover_id: None,
                notes: None,
                plot_ids: vec![fx.plot_a.clone(), fx.plot_b.clone()],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_cultural_operations(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

#[test]
fn listing_soil_covers_costs_the_same_at_four_times_the_records() {
    // The covers' MAINTENANCE is still read per record, deliberately — it is
    // assembled from two other registers. So this one is expected to move, and
    // the assertion is on the plots hoist: the gap must stay proportional to
    // the maintenance reads alone, not to three child tables per record.
    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        repo::insert_soil_cover(
            conn,
            NewSoilCover {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                practice_code: "plant_cover".into(),
                // TIPO_COBERTURA_SUELO 2, "Cubierta vegetal sembrada".
                cover_type_code: "2".into(),
                established_on: day(n),
                width_m: None,
                free_canopy_width_m: None,
                widths_stated_on: None,
                notes: None,
                plot_ids: vec![fx.plot_a.clone(), fx.plot_b.clone()],
                maintenance: Vec::new(),
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_soil_covers(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));

    // Two maintenance statements per cover, and nothing else per cover.
    let per_record = 2;
    assert_eq!(
        many_statements - few_statements,
        (MANY - FEW) * per_record,
        "the plots are hoisted; only the maintenance is still read per cover"
    );
}
