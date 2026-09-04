// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Does listing a campaign of sections 6, 7.1 and 8 cost more as it fills up?
//!
//! Each register is read as a whole — by the register view, by the book, by the
//! exporter — so the statements a listing runs must be bounded by the number of
//! CHILD TABLES it hydrates, never by the number of records. A per-record child
//! query returns exactly the right rows while doing it, which is why the
//! correctness tests in the sibling files cannot fail on this.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{CoreFixture, FarmWithPlots, PlotSpec, farm_with_plots};
use module_fertilisation::models::*;
use module_fertilisation::open_in_memory;
use module_fertilisation::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::NewCrop;
use terrazgo_core::repository as core_repo;
use terrazgo_testkit::query_cost;

/// Few records against four times as many. Both are small: the defect is a
/// count that MOVES, and four times nothing is still four times.
const FEW: usize = 3;
const MANY: usize = 12;

fn fixture(conn: &mut Connection) -> CoreFixture {
    farm_with_plots(
        conn,
        FarmWithPlots {
            other_farm_plot: PlotSpec::new("Ajena", 4.0),
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
fn listing_irrigations_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        repo::insert_irrigation_record(
            conn,
            NewIrrigationRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                irrigated_on: day(n),
                irrigation_end_date: None,
                irrigation_method_code: "drip".into(),
                volume_value: 320.0,
                volume_unit_code: "m3_ha".into(),
                water_nitric_n_mg_l: None,
                water_soluble_p2o5_mg_l: None,
                energy_type_code: None,
                meter_number: None,
                notes: None,
                plots: vec![
                    NewIrrigationPlot {
                        plot_id: fx.plot_a.clone(),
                        crop_id: None,
                        irrigated_area_ha: Some(3.5),
                    },
                    NewIrrigationPlot {
                        plot_id: fx.plot_b.clone(),
                        crop_id: None,
                        irrigated_area_ha: Some(1.5),
                    },
                ],
                // One origin, so the joined ordering is exercised too.
                water_origins: vec!["surface".into()],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_irrigation_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(
        few_statements, many_statements,
        "irrigation: {few_statements} statements for {FEW} records, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_fertilisations_costs_the_same_at_four_times_the_records() {
    let material = |conn: &mut Connection| {
        repo::insert_fertiliser_material(
            conn,
            NewFertiliserMaterial {
                name: "NAC 27".into(),
                material_code: "14".into(), // MAT_FERTI: abonos inorgánicos
                material_detail_code: None,
                supplier_name: None,
                supplier_rega: None,
                supplier_tax_id: None,
                supplier_nima: None,
                manure_treatment_code: None,
                density_kg_l: None,
                notes: None,
                nutrients: vec![MaterialNutrient {
                    id: String::new(),
                    kind_code: "macro".into(),
                    nutrient_code: "1".into(), // N total
                    percentage: 27.0,
                }],
            },
            None,
        )
        .unwrap()
        .material
        .id
    };

    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        // One material, reused: the registry is not what is being counted.
        let material_id = match repo::list_fertiliser_materials(conn).unwrap().first() {
            Some(existing) => existing.material.id.clone(),
            None => material(conn),
        };
        repo::insert_fertilisation_record(
            conn,
            NewFertilisationRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                applied_on: day(n),
                application_end_date: None,
                fertilisation_type_code: "top_dressing".into(),
                application_method_code: "broadcast".into(),
                dose_value: 250.0,
                dose_unit_code: "kg_ha".into(),
                fertiliser_material_id: material_id,
                sludge_application: false,
                sustainable_input_management: false,
                irrigation_record_id: None,
                machinery_id: None,
                service_company: None,
                service_regfer_number: None,
                delivery_note_ref: None,
                yield_estimated_kg_ha: None,
                yield_final_kg_ha: None,
                notes: None,
                plots: vec![
                    NewFertilisationPlot {
                        plot_id: fx.plot_a.clone(),
                        crop_id: None,
                        fertilised_area_ha: Some(3.5),
                    },
                    NewFertilisationPlot {
                        plot_id: fx.plot_b.clone(),
                        crop_id: None,
                        fertilised_area_ha: Some(1.5),
                    },
                ],
                practices: vec![],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_fertilisation_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

#[test]
fn listing_plans_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &CoreFixture, n: usize| {
        let crop_id = core_repo::insert_crop(
            conn,
            NewCrop {
                plot_id: fx.plot_a.clone(),
                season_id: fx.season_id.clone(),
                species_name: format!("Trigo {n}"),
                variety: None,
                production_system_code: None,
                area_ha: Some(3.0),
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
        repo::insert_fertilisation_plan(
            conn,
            NewFertilisationPlan {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                needs_n_kg_ha: 140.0,
                needs_p2o5_kg_ha: 60.0,
                needs_k2o_kg_ha: 0.0,
                expected_yield_kg_ha: 6500.0,
                preceding_crop_code: Some("60".into()),
                drawn_up_on: day(n),
                tool_generated: false,
                notes: None,
                crop_ids: vec![crop_id],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &CoreFixture| {
        repo::list_fertilisation_plans(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}
