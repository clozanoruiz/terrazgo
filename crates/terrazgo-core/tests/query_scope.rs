// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Does listing core's registers and registries cost more as they fill up?
//!
//! The statements a listing runs must be bounded by the number of CHILD TABLES
//! it hydrates, never by the number of rows. That holds for the two registers
//! that bracket a crop — sowing and harvest — and equally for the three
//! registries that hang a Spanish extension off each row, where the extension
//! read was one query per plot, per machine, per premises.
//!
//! A per-row child query returns exactly the right answer while doing it, which
//! is why no correctness test in this directory can fail on it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::query_cost;

/// Few rows against four times as many. Both are small: the defect is a count
/// that MOVES, and four times nothing is still four times.
const FEW: usize = 3;
const MANY: usize = 12;

struct Land {
    season_id: String,
    farm_id: String,
    plot_id: String,
}

fn land(conn: &mut Connection) -> Land {
    let season = repo::insert_season(conn, new_season(2026, "2025/2026"), None).unwrap();
    let farm = repo::insert_farm(conn, new_farm("Finca La Vega"), None).unwrap();
    let plot = repo::insert_plot(conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    Land {
        season_id: season.id,
        farm_id: farm.id,
        plot_id: plot.id,
    }
}

/// Build `count` rows with `insert`, then report what listing them costs.
fn statements_to_list<T>(
    count: usize,
    insert: impl Fn(&mut Connection, &Land, usize),
    list: impl Fn(&Connection, &Land) -> Vec<T>,
) -> (usize, usize) {
    let mut conn = db();
    let fx = land(&mut conn);
    for n in 0..count {
        insert(&mut conn, &fx, n);
    }
    let (listed, cost) = query_cost(&mut conn, |conn| list(conn, &fx));
    (listed.len(), cost.statements)
}

fn day(n: usize) -> String {
    format!("2026-05-{:02}", n + 1)
}

#[test]
fn listing_sowings_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_sowing_record(
            conn,
            NewSowingRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                kind_code: "sowing".into(),
                sown_on: day(n),
                sowing_end_date: None,
                flooded_on: None,
                seed_quantity_kg: Some(180.0),
                notes: None,
                plots: vec![NewSowingPlot {
                    plot_id: fx.plot_id.clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &Land| {
        repo::list_sowing_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(
        few_statements, many_statements,
        "sowing: {few_statements} statements for {FEW} records, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_harvests_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_harvest_record(
            conn,
            NewHarvestRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                harvested_on: day(n),
                product_name: "trigo blando".into(),
                plant_product_code: Some("1".into()),
                quantity_value: Some(42.5),
                quantity_unit_code: Some("t".into()),
                delivery_note_ref: None,
                lot_number: None,
                buyer_name: "Cooperativa Cerealista del Duero".into(),
                buyer_tax_id: None,
                buyer_address: None,
                buyer_registry_number: None,
                notes: None,
                plots: vec![NewHarvestPlot {
                    plot_id: fx.plot_id.clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &Land| {
        repo::list_harvest_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

// --- the registries, where the child is an extension row --------------------
//
// An extension is at most one row per parent, so the per-parent read looked
// harmless. It is the same defect: a farm with 400 plots ran 400 point queries
// to draw one list, and a cooperative-sized holding is exactly where the
// registry views are largest.

#[test]
fn listing_plots_costs_the_same_at_four_times_the_plots() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        let mut plot = new_plot(&fx.farm_id, &format!("Recinto {n}"));
        plot.es = Some(PlotEsFields {
            sigpac_province: Some("47".into()),
            sigpac_municipality: Some("186".into()),
            sigpac_aggregate: None,
            sigpac_zone: None,
            sigpac_polygon: Some("12".into()),
            sigpac_parcel: Some(format!("{}", 100 + n)),
            sigpac_enclosure: Some("1".into()),
        });
        repo::insert_plot(conn, plot, None).unwrap();
    };
    let list = |conn: &Connection, fx: &Land| repo::list_plots(conn, &fx.farm_id).unwrap();

    // `land` already made one plot, so both counts carry the same offset.
    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW + 1, MANY + 1));
    assert_eq!(
        few_statements, many_statements,
        "plots: {few_statements} statements for {FEW} extensions, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_machinery_details_costs_the_same_at_four_times_the_machines() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_machinery(
            conn,
            NewMachinery {
                farm_id: fx.farm_id.clone(),
                name: format!("Atomizador {n}"),
                kind: None,
                acquired_on: None,
                last_inspection_date: None,
                next_inspection_due_date: None,
                roma_number: Some(format!("470012345{n}")),
                reganip_number: None,
            },
            None,
        )
        .unwrap();
    };
    let list =
        |conn: &Connection, fx: &Land| repo::list_machinery_details(conn, &fx.farm_id).unwrap();

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

#[test]
fn listing_premises_details_costs_the_same_at_four_times_the_premises() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_premises(
            conn,
            NewPremises {
                farm_id: fx.farm_id.clone(),
                kind_code: "building".into(),
                name: format!("Almacén {n}"),
                address: Some("Camino de la Vega, 1".into()),
                vehicle_model: None,
                plate: None,
                // EDIFICACIONES_INSTALACIONES 2 = "Almacén de maquinaria".
                class_code: Some("2".into()),
                volume_m3: Some(420.0),
                notes: None,
                cadastral_reference: Some(format!("123456{n:02}AB1234C0001XY")),
                rea_installation_code: None,
            },
            None,
        )
        .unwrap();
    };
    let list =
        |conn: &Connection, fx: &Land| repo::list_premises_details(conn, &fx.farm_id).unwrap();

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}
