// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module's duplicate rules (docs/sync.md → Duplicate suspects): what each
//! raises and what it leaves alone. The machinery the rules go through —
//! scopes, verdicts, two devices — is pinned in core.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{CoreFixture, FarmWithPlots, farm_with_plots};
use module_fertilisation::duplicates::{
    FERTILISATION_DUPLICATES, IRRIGATION_DUPLICATES, PLAN_DUPLICATES,
};
use module_fertilisation::models::*;
use module_fertilisation::open_in_memory;
use module_fertilisation::repository as repo;
use rusqlite::Connection;
use terrazgo_core::duplicates::{DuplicatePolicy, Scope};
use terrazgo_core::models::NewCrop;
use terrazgo_core::repository as core_repo;

const TODAY: &str = "2026-06-11";

const POLICIES: [DuplicatePolicy; 3] = [
    FERTILISATION_DUPLICATES,
    IRRIGATION_DUPLICATES,
    PLAN_DUPLICATES,
];

fn fixture() -> (Connection, CoreFixture) {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    (conn, fx)
}

/// The pairs the Status view lists, as `(register, first, second)`.
fn suspected(conn: &Connection) -> Vec<(&'static str, String, String)> {
    core_repo::list_duplicates(
        conn,
        &POLICIES,
        module_fertilisation::ROW_CAPTIONS,
        Scope::Current { today: TODAY },
    )
    .unwrap()
    .suspects
    .into_iter()
    .map(|pair| {
        let [first, second] = pair.records;
        (pair.register, first.id, second.id)
    })
    .collect()
}

fn pair_of(register: &'static str, a: &str, b: &str) -> Vec<(&'static str, String, String)> {
    let (first, second) = if a < b { (a, b) } else { (b, a) };
    vec![(register, first.to_owned(), second.to_owned())]
}

// ---------------------------------------------------------------------------
// Fertilisation
// ---------------------------------------------------------------------------

/// A material the farmer added, of catalogue kind `code`.
fn material(conn: &mut Connection, name: &str, code: &str) -> String {
    repo::insert_fertiliser_material(
        conn,
        NewFertiliserMaterial {
            name: name.into(),
            material_code: code.into(),
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
                nutrient_code: "1".into(),
                percentage: 27.0,
            }],
        },
        None,
    )
    .unwrap()
    .material
    .id
}

fn application(
    conn: &mut Connection,
    fx: &CoreFixture,
    material_id: &str,
    day: &str,
    on: &[&str],
) -> String {
    repo::insert_fertilisation_record(
        conn,
        NewFertilisationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            applied_on: day.into(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "broadcast".into(),
            dose_value: 250.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: material_id.into(),
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
            plots: on
                .iter()
                .map(|plot| NewFertilisationPlot {
                    plot_id: (*plot).into(),
                    crop_id: None,
                    fertilised_area_ha: None,
                })
                .collect(),
            practices: vec![],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn one_application_recorded_twice_is_a_suspect_whichever_material_row_it_names() {
    // Two phones each added "NAC 27": two material rows, one catalogue kind —
    // which is what each record snapshotted.
    let (mut conn, fx) = fixture();
    let nac = material(&mut conn, "NAC 27", "14");
    let twin = material(&mut conn, "Nitrato amónico cálcico", "14");
    let first = application(
        &mut conn,
        &fx,
        &nac,
        "2026-03-12",
        &[&fx.plot_a, &fx.plot_b],
    );
    let second = application(&mut conn, &fx, &twin, "2026-03-13", &[&fx.plot_b]);
    assert_eq!(
        suspected(&conn),
        pair_of("fertilisation_record", &first, &second)
    );
}

#[test]
fn another_material_or_other_plots_or_two_days_apart_is_not_a_suspect() {
    let (mut conn, fx) = fixture();
    let nac = material(&mut conn, "NAC 27", "14");
    let manure = material(&mut conn, "Estiércol de oveja", "1");
    application(&mut conn, &fx, &nac, "2026-03-12", &[&fx.plot_a]);
    application(&mut conn, &fx, &manure, "2026-03-12", &[&fx.plot_a]);
    application(&mut conn, &fx, &nac, "2026-03-12", &[&fx.plot_b]);
    application(&mut conn, &fx, &nac, "2026-03-14", &[&fx.plot_a]);
    assert!(suspected(&conn).is_empty(), "{:?}", suspected(&conn));
}

// ---------------------------------------------------------------------------
// Irrigation
// ---------------------------------------------------------------------------

fn watering(
    conn: &mut Connection,
    fx: &CoreFixture,
    from: &str,
    to: Option<&str>,
    on: &[&str],
) -> String {
    repo::insert_irrigation_record(
        conn,
        NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: from.into(),
            irrigation_end_date: to.map(str::to_owned),
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: on
                .iter()
                .map(|plot| NewIrrigationPlot {
                    plot_id: (*plot).into(),
                    crop_id: None,
                    irrigated_area_ha: None,
                })
                .collect(),
            water_origins: vec![],
            practices: vec![],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn one_watering_recorded_twice_is_a_suspect() {
    let (mut conn, fx) = fixture();
    let first = watering(&mut conn, &fx, "2026-06-14", None, &[&fx.plot_a]);
    let second = watering(
        &mut conn,
        &fx,
        "2026-06-14",
        None,
        &[&fx.plot_a, &fx.plot_b],
    );
    assert_eq!(
        suspected(&conn),
        pair_of("irrigation_record", &first, &second)
    );
}

#[test]
fn watering_on_consecutive_days_is_ordinary_and_not_a_suspect() {
    let (mut conn, fx) = fixture();
    for day in ["2026-06-14", "2026-06-15", "2026-06-16"] {
        watering(&mut conn, &fx, day, None, &[&fx.plot_a]);
    }
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_day_recorded_inside_an_accumulated_fortnight_is_a_suspect() {
    // Art. 5.f lets an intensive crop's waterings be accumulated a fortnight at
    // a time; one of those days recorded on its own as well is the same water
    // counted twice.
    let (mut conn, fx) = fixture();
    let fortnight = watering(
        &mut conn,
        &fx,
        "2026-06-01",
        Some("2026-06-15"),
        &[&fx.plot_a],
    );
    let day = watering(&mut conn, &fx, "2026-06-09", None, &[&fx.plot_a]);
    assert_eq!(
        suspected(&conn),
        pair_of("irrigation_record", &fortnight, &day)
    );
}

// ---------------------------------------------------------------------------
// Fertilisation plans
// ---------------------------------------------------------------------------

fn crop(conn: &mut Connection, fx: &CoreFixture, plot: &str, species: &str) -> String {
    core_repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot.into(),
            season_id: fx.season_id.clone(),
            species_name: species.into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
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

fn plan(conn: &mut Connection, fx: &CoreFixture, day: &str, crops: &[&str]) -> String {
    repo::insert_fertilisation_plan(
        conn,
        NewFertilisationPlan {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            needs_n_kg_ha: 140.0,
            needs_p2o5_kg_ha: 60.0,
            needs_k2o_kg_ha: 0.0,
            expected_yield_kg_ha: 6500.0,
            preceding_crop_code: None,
            drawn_up_on: day.into(),
            tool_generated: false,
            notes: None,
            crop_ids: crops.iter().map(|id| (*id).to_owned()).collect(),
        },
        None,
    )
    .unwrap()
    .plan
    .id
}

#[test]
fn two_plans_for_one_crop_in_one_book_are_a_suspect_whatever_their_dates() {
    // A crop is in one plan, and the form refuses a second on the same device
    // (`crop_already_planned`) — so two plans for one crop are two devices that
    // each drew it up offline. The second device's plan is written here the
    // way its bundle would materialise it: a row the local guard never saw.
    let (mut conn, fx) = fixture();
    let wheat = crop(&mut conn, &fx, &fx.plot_a, "trigo blando");
    let barley = crop(&mut conn, &fx, &fx.plot_b, "cebada");
    let first = plan(&mut conn, &fx, "2025-09-20", &[&wheat]);
    let second = plan(&mut conn, &fx, "2025-10-30", &[&barley]);
    assert!(suspected(&conn).is_empty(), "no crop in common yet");
    conn.execute(
        "INSERT INTO fertilisation_plan_crop (id, fertilisation_plan_id, crop_id)
         VALUES ('arrived-by-sync', ?1, ?2)",
        [&second, &wheat],
    )
    .unwrap();
    assert_eq!(
        suspected(&conn),
        pair_of("fertilisation_plan", &first, &second)
    );
}

#[test]
fn plans_for_different_crops_are_not_a_suspect() {
    let (mut conn, fx) = fixture();
    let wheat = crop(&mut conn, &fx, &fx.plot_a, "trigo blando");
    let barley = crop(&mut conn, &fx, &fx.plot_b, "cebada");
    plan(&mut conn, &fx, "2025-09-20", &[&wheat]);
    plan(&mut conn, &fx, "2025-09-20", &[&barley]);
    assert!(suspected(&conn).is_empty());
}

// ---------------------------------------------------------------------------
// Right after a save
// ---------------------------------------------------------------------------

#[test]
fn what_a_save_finds_is_what_the_book_page_lists() {
    // Every shape this module's rules read, in one farm, and every live record
    // checked: the check a form runs after saving and the book's page must
    // name the same pairs (docs/sync.md → The same rule, right after the form saves).
    let (mut conn, fx) = fixture();
    let nac = material(&mut conn, "NAC 27", "1");
    let nac_again = material(&mut conn, "NAC 27", "1");
    let urea = material(&mut conn, "Urea", "2");
    application(&mut conn, &fx, &nac, "2026-03-01", &[&fx.plot_a]);
    application(
        &mut conn,
        &fx,
        &nac_again,
        "2026-03-02",
        &[&fx.plot_a, &fx.plot_b],
    );
    application(&mut conn, &fx, &urea, "2026-03-01", &[&fx.plot_a]);
    application(&mut conn, &fx, &nac, "2026-03-05", &[&fx.plot_b]);
    // A fortnight accumulated, a day inside it, a day just after it, and the
    // day after that.
    watering(
        &mut conn,
        &fx,
        "2026-05-01",
        Some("2026-05-15"),
        &[&fx.plot_a],
    );
    watering(&mut conn, &fx, "2026-05-10", None, &[&fx.plot_a]);
    watering(&mut conn, &fx, "2026-05-16", None, &[&fx.plot_a]);
    watering(
        &mut conn,
        &fx,
        "2026-05-17",
        None,
        &[&fx.plot_a, &fx.plot_b],
    );
    let wheat = crop(&mut conn, &fx, &fx.plot_a, "trigo blando");
    let barley = crop(&mut conn, &fx, &fx.plot_b, "cebada");
    plan(&mut conn, &fx, "2025-09-20", &[&wheat]);
    let second = plan(&mut conn, &fx, "2025-10-30", &[&barley]);
    // The second device's plan for the same crop, as its bundle would
    // materialise it (see the test above).
    conn.execute(
        "INSERT INTO fertilisation_plan_crop (id, fertilisation_plan_id, crop_id)
         VALUES ('arrived-by-sync', ?1, ?2)",
        [&second, &wheat],
    )
    .unwrap();

    let checked = terrazgo_testkit::duplicates::assert_saved_pairs_match_the_book(
        &conn,
        &POLICIES,
        module_fertilisation::ROW_CAPTIONS,
    );
    assert_eq!(checked, 4 + 4 + 2);
    // One application, the day inside the fortnight, and the plan: the farm
    // has pairs, so the check compared something.
    assert_eq!(suspected(&conn).len(), 3);
}
