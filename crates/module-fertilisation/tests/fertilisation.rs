// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 6 — the fertilisation register and its material registry.
//!
//! Every rule pinned here names its source: RD 1051/2022 art. 5.d and 5.g, and
//! RD 1311/2012 Anexo III Parte I sección C (letters a–k), which art. 5.d
//! redirects to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{FarmWithPlots, farm_with_plots, last_change};
use module_fertilisation::models::*;
use module_fertilisation::open_in_memory;
use module_fertilisation::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::{NewMachinery, NewSeason};
use terrazgo_core::repository as core_repo;

/// The shared land plus the two spreaders this register needs — one on the
/// farm, one on the neighbour's.
struct Fixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
    other_farm_plot: String,
    other_farm_machinery: String,
    machinery_id: String,
}

fn fixture(conn: &mut Connection) -> Fixture {
    let core = farm_with_plots(conn, FarmWithPlots::default());

    let machine = |conn: &mut Connection, farm_id: &str, name: &str| {
        core_repo::insert_machinery(
            conn,
            NewMachinery {
                farm_id: farm_id.to_string(),
                name: name.into(),
                kind: None,
                acquired_on: None,
                last_inspection_date: None,
                next_inspection_due_date: None,
                roma_number: None,
                reganip_number: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let machinery_id = machine(conn, &core.farm_id, "Abonadora centrífuga");
    let other_farm_machinery = machine(conn, &core.other_farm_id, "Cuba del vecino");

    Fixture {
        season_id: core.season_id,
        farm_id: core.farm_id,
        plot_a: core.plot_a,
        plot_b: core.plot_b,
        other_farm_plot: core.other_farm_plot,
        other_farm_machinery,
        machinery_id,
    }
}

/// A registry entry with the three richness values section 6 prints, plus two
/// of C.h's other five, so tests can tell the snapshot apart from the full
/// composition.
fn sample_material() -> NewFertiliserMaterial {
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
        nutrients: vec![
            nutrient("macro", "1", 27.0), // N total
            nutrient("macro", "3", 13.5), // N nítrico
            nutrient("macro", "4", 13.5), // N amoniacal
            nutrient("macro", "6", 0.0),  // P2O5 total
            nutrient("macro", "9", 0.0),  // K2O
        ],
    }
}

fn nutrient(kind: &str, code: &str, percentage: f64) -> MaterialNutrient {
    MaterialNutrient {
        id: String::new(),
        kind_code: kind.into(),
        nutrient_code: code.into(),
        percentage,
    }
}

fn sample(fx: &Fixture, material_id: &str) -> NewFertilisationRecord {
    NewFertilisationRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        applied_on: "2026-03-12".into(),
        application_end_date: None,
        fertilisation_type_code: "top_dressing".into(),
        application_method_code: "broadcast".into(),
        dose_value: 250.0,
        dose_unit_code: "kg_ha".into(),
        fertiliser_material_id: material_id.to_string(),
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
        plots: vec![NewFertilisationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            fertilised_area_ha: Some(3.5),
        }],
        practices: vec![],
    }
}

fn material(conn: &mut Connection) -> String {
    repo::insert_fertiliser_material(conn, sample_material(), None)
        .unwrap()
        .material
        .id
}

// --- the material registry -------------------------------------------------

#[test]
fn registers_a_material_with_its_composition() {
    let mut conn = open_in_memory().unwrap();
    let detail =
        repo::insert_fertiliser_material(&mut conn, sample_material(), Some("user-1")).unwrap();

    assert_eq!(detail.material.name, "NAC 27");
    assert_eq!(detail.material.material_code, "14");
    // Anexo III C.h asks for eight agronomic values; they live here, on the
    // reusable row, not on every application that names it.
    assert_eq!(detail.nutrients.len(), 5);
    assert!(
        detail
            .nutrients
            .iter()
            .any(|n| n.nutrient_code == "3" && n.percentage == 13.5),
        "nitric nitrogen is one of C.h's eight and has no column in the model"
    );
}

#[test]
fn the_same_nutrient_code_means_different_things_per_kind() {
    // MACRONUTRIENTES 3 is nitric nitrogen; METALES_PESADOS 3 is lead. Storing
    // them under one code without the kind would merge two claims into one.
    let mut conn = open_in_memory().unwrap();
    let mut new = sample_material();
    new.nutrients = vec![
        nutrient("macro", "3", 13.5),
        nutrient("heavy_metal", "3", 0.4),
    ];

    let detail = repo::insert_fertiliser_material(&mut conn, new, None).unwrap();
    assert_eq!(detail.nutrients.len(), 2);
    // Macronutrients first, then micro, then heavy metals — the order the SIEX
    // material block lists its arrays in, and a stable one either way.
    assert_eq!(detail.nutrients[0].kind_code, "macro");
    assert_eq!(detail.nutrients[1].kind_code, "heavy_metal");
}

#[test]
fn rejects_a_percentage_outside_zero_to_one_hundred() {
    let mut conn = open_in_memory().unwrap();
    let mut new = sample_material();
    // A typo here would be multiplied by the dose in every unidad-fertilizante
    // sum section 7.1 will assemble.
    new.nutrients = vec![nutrient("macro", "1", 270.0)];
    let err = repo::insert_fertiliser_material(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_percentage")
    ));
}

#[test]
fn rejects_two_supplier_registry_numbers() {
    // Anexo III C.e's three identifiers are mutually exclusive, and the twin
    // says so in each of their own descriptions ("Excluyente con …").
    let mut conn = open_in_memory().unwrap();
    let mut new = sample_material();
    new.supplier_rega = Some("ES090300000001".into());
    new.supplier_nima = Some("4700001234".into());

    let err = repo::insert_fertiliser_material(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("supplier_id_conflict")
    ));
}

#[test]
fn rejects_a_material_code_outside_the_published_list() {
    // MAT_FERTI is a closed 24-value list the decree itself enumerates, so a
    // code outside it is a typo rather than a snapshot fallen behind. Checked
    // only once the catalogue is imported, which a running app always has.
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let mut new = sample_material();
    new.material_code = "99".into();

    let err = repo::insert_fertiliser_material(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("unknown_material_code")
    ));
}

#[test]
fn a_material_detail_code_the_snapshot_cannot_resolve_is_still_accepted() {
    // The 1243-row product registry grows between our snapshot releases, and a
    // record must not be blocked on one (the `analysis_substance` rule).
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let mut new = sample_material();
    new.material_detail_code = Some("999999".into());

    let detail = repo::insert_fertiliser_material(&mut conn, new, None).unwrap();
    assert_eq!(
        detail.material.material_detail_code.as_deref(),
        Some("999999")
    );
}

#[test]
fn correcting_a_material_reconciles_its_composition() {
    let mut conn = open_in_memory().unwrap();
    let created =
        repo::insert_fertiliser_material(&mut conn, sample_material(), Some("user-1")).unwrap();
    let kept_row = created
        .nutrients
        .iter()
        .find(|n| n.nutrient_code == "1")
        .unwrap()
        .clone();

    let detail = repo::update_fertiliser_material(
        &mut conn,
        &created.material.id,
        UpdateFertiliserMaterial {
            id: created.material.id.clone(),
            name: "NAC 27 %".into(),
            material_code: "14".into(),
            material_detail_code: None,
            supplier_name: None,
            supplier_rega: None,
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: None,
            density_kg_l: None,
            notes: None,
            // N total corrected, the rest of the nitrogen breakdown withdrawn.
            nutrients: vec![nutrient("macro", "1", 27.5), nutrient("macro", "9", 0.0)],
        },
        Some("user-1"),
    )
    .unwrap();

    assert_eq!(detail.material.name, "NAC 27 %");
    assert_eq!(detail.nutrients.len(), 2);
    let n_total = detail
        .nutrients
        .iter()
        .find(|n| n.nutrient_code == "1")
        .unwrap();
    assert_eq!(n_total.percentage, 27.5);
    // A corrected figure keeps its row identity, so the audit trail reads as
    // "this was wrong" rather than "withdrawn and re-stated".
    assert_eq!(n_total.id, kept_row.id);

    let (op, before, after) = last_change(&conn, "fertiliser_material_nutrient", &kept_row.id);
    assert_eq!(op, "update");
    assert_eq!(before["percentage"], 27.0);
    assert_eq!(after["percentage"], 27.5);
    // The junction has no parent id in its model struct; the log image must put
    // it back, because a receiving device rebuilds the row from `after` alone.
    assert_eq!(after["fertiliser_material_id"], created.material.id);
}

#[test]
fn deleting_a_material_is_soft_so_past_records_still_resolve_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let record =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();

    repo::soft_delete_fertiliser_material(&mut conn, &material_id, None).unwrap();

    assert!(repo::list_fertiliser_materials(&conn).unwrap().is_empty());
    // The record keeps naming it, and the row is still there to be read.
    let stored = repo::get_fertilisation_record(&conn, &record.record.id).unwrap();
    assert_eq!(stored.record.fertiliser_material_id, material_id);
    assert_eq!(stored.record.material_name_snapshot, "NAC 27");
}

// --- the register ----------------------------------------------------------

#[test]
fn records_an_application_with_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let detail =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), Some("user-1"))
            .unwrap();

    assert_eq!(detail.record.fertilisation_type_code, "top_dressing");
    assert_eq!(detail.record.application_method_code, "broadcast");
    assert_eq!(detail.record.dose_value, 250.0);
    assert_eq!(detail.record.dose_unit_code, "kg_ha");
    assert_eq!(detail.plots.len(), 1);
    assert_eq!(detail.plots[0].fertilised_area_ha, Some(3.5));
    assert!(detail.record.application_end_date.is_none());
}

#[test]
fn freezes_the_material_name_and_the_richness_the_model_prints() {
    // Legal value capture: correcting the registry must never rewrite a record
    // already printed in a legal document.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let created =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();

    assert_eq!(created.record.material_name_snapshot, "NAC 27");
    // C.d's coded kind freezes beside the name: the model's own "Tipo de
    // abono/producto" column prints it, so a record that named a mineral
    // fertiliser must go on saying so even if the registry entry is later
    // corrected to a manure.
    assert_eq!(created.record.material_code_snapshot, "14");
    assert_eq!(created.record.richness_n_snapshot, Some(27.0));
    assert_eq!(created.record.richness_p2o5_snapshot, Some(0.0));
    assert_eq!(created.record.richness_k2o_snapshot, Some(0.0));

    repo::update_fertiliser_material(
        &mut conn,
        &material_id,
        UpdateFertiliserMaterial {
            id: material_id.clone(),
            name: "Otro abono".into(),
            material_code: "14".into(),
            material_detail_code: None,
            supplier_name: None,
            supplier_rega: None,
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: None,
            density_kg_l: None,
            notes: None,
            nutrients: vec![nutrient("macro", "1", 46.0)],
        },
        None,
    )
    .unwrap();

    let stored = repo::get_fertilisation_record(&conn, &created.record.id).unwrap();
    assert_eq!(stored.record.material_name_snapshot, "NAC 27");
    assert_eq!(stored.record.richness_n_snapshot, Some(27.0));
}

#[test]
fn a_richness_the_label_does_not_state_stays_blank_never_zero() {
    // Blank and zero are different claims: zero says "contains none of it", and
    // a spreadsheet would go on to add it up.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new_material = sample_material();
    new_material.nutrients = vec![nutrient("macro", "1", 46.0)]; // urea: N only
    let material_id = repo::insert_fertiliser_material(&mut conn, new_material, None)
        .unwrap()
        .material
        .id;

    let detail =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();
    assert_eq!(detail.record.richness_n_snapshot, Some(46.0));
    assert!(detail.record.richness_p2o5_snapshot.is_none());
    assert!(detail.record.richness_k2o_snapshot.is_none());
}

#[test]
fn logs_a_complete_row_image_for_the_record_and_each_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let detail =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), Some("user-1"))
            .unwrap();

    let (op, _, after) = last_change(&conn, "fertilisation_record", &detail.record.id);
    assert_eq!(op, "insert");
    assert_eq!(after["dose_unit_code"], "kg_ha");
    assert_eq!(after["material_name_snapshot"], "NAC 27");
    assert_eq!(after["season_id"], detail.record.season_id);

    let (op, _, after) = last_change(&conn, "fertilisation_plot", &detail.plots[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["plot_id"], fx.plot_a);
}

#[test]
fn accepts_a_date_interval() {
    // RD 1051/2022 art. 5.f: intensive and fertigated crops may accumulate the
    // record over fortnightly periods; the twin requires both ends.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.applied_on = "2026-03-01".into();
    new.application_end_date = Some("2026-03-15".into());

    let detail = repo::insert_fertilisation_record(&mut conn, new, None).unwrap();
    assert_eq!(
        detail.record.application_end_date.as_deref(),
        Some("2026-03-15")
    );
}

#[test]
fn rejects_an_interval_that_ends_before_it_starts() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.application_end_date = Some("2026-03-01".into());

    let err = repo::insert_fertilisation_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_date_interval")
    ));
}

#[test]
fn the_dose_must_be_a_rate_not_a_bare_amount() {
    // Anexo III C.j asks for the quantity applied PER HECTARE. "250 kg" and
    // "250 kg/ha" answer different questions, and the unit table holds both.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.dose_unit_code = "kg".into();

    let err = repo::insert_fertilisation_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_dose_unit")
    ));
}

#[test]
fn rejects_a_nonpositive_dose() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.dose_value = 0.0;

    let err = repo::insert_fertilisation_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_dose")
    ));
}

#[test]
fn records_the_sludge_flag_and_the_service_company() {
    // C.i / art. 5.g (sludge) and C.k (the service company with its REGFER
    // number — a third machinery registry beside ROMA and REGANIP).
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.sludge_application = true;
    new.service_company = Some("Abonados del Duero S.L.".into());
    new.service_regfer_number = Some("REGFER-4711".into());

    let detail = repo::insert_fertilisation_record(&mut conn, new, None).unwrap();
    assert!(detail.record.sludge_application);
    assert_eq!(
        detail.record.service_regfer_number.as_deref(),
        Some("REGFER-4711")
    );
}

#[test]
fn the_machine_is_optional_but_must_belong_to_the_holding() {
    // C.g says the machine is optional in so many words; a NAMED one still has
    // to be this farm's, or the record states something that did not happen.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let without =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();
    assert!(without.record.machinery_id.is_none());

    let mut with_own = sample(&fx, &material_id);
    with_own.machinery_id = Some(fx.machinery_id.clone());
    let detail = repo::insert_fertilisation_record(&mut conn, with_own, None).unwrap();
    assert_eq!(
        detail.record.machinery_id.as_deref(),
        Some(fx.machinery_id.as_str())
    );

    let mut with_foreign = sample(&fx, &material_id);
    with_foreign.machinery_id = Some(fx.other_farm_machinery.clone());
    let err = repo::insert_fertilisation_record(&mut conn, with_foreign, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("machinery_not_on_farm")
    ));
}

#[test]
fn refuses_a_plot_from_another_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.plots = vec![NewFertilisationPlot {
        plot_id: fx.other_farm_plot.clone(),
        crop_id: None,
        fertilised_area_ha: None,
    }];

    let err = repo::insert_fertilisation_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn folds_a_plot_listed_twice_and_refuses_an_empty_list() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let mut twice = sample(&fx, &material_id);
    twice.plots = vec![
        NewFertilisationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            fertilised_area_ha: Some(3.5),
        },
        NewFertilisationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            fertilised_area_ha: Some(3.5),
        },
    ];
    let detail = repo::insert_fertilisation_record(&mut conn, twice, None).unwrap();
    assert_eq!(detail.plots.len(), 1);

    let mut none = sample(&fx, &material_id);
    none.plots = vec![];
    let err = repo::insert_fertilisation_record(&mut conn, none, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("no_plots")
    ));
}

#[test]
fn records_the_good_practices_the_twin_requires_and_the_model_never_asks() {
    // `Fertilizacion.BuenasPracticas` is required in SIEX while the printed
    // model has no column for it, so it is captured and never demanded.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let bare =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();
    assert!(bare.practices.is_empty());

    let mut new = sample(&fx, &material_id);
    // Submitted out of order and with a duplicate: both are normal in a form.
    new.practices = vec!["7".into(), "3".into(), "7".into()];
    let detail = repo::insert_fertilisation_record(&mut conn, new, None).unwrap();
    assert_eq!(detail.practices, vec!["3".to_string(), "7".to_string()]);

    // What comes back from the database must list them the same way, or the
    // same record would print differently before and after a reload.
    let stored = repo::get_fertilisation_record(&conn, &detail.record.id).unwrap();
    assert_eq!(stored.practices, detail.practices);
}

#[test]
fn refuses_no_practices_claimed_beside_a_practice() {
    // Source of truth is the catalogue's own wording: BUENAS_PRACTICAS_AMBITOS
    // row ("0";"No realiza buenas prácticas";"Fertilización"). Holding it beside
    // another code says both that nothing was done and what was done, and the
    // SIEX twin would carry the contradiction out as two BuenaPracticaFertilizante
    // entries.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let mut both = sample(&fx, &material_id);
    both.practices = vec!["0".into(), "7".into()];
    let err = repo::insert_fertilisation_record(&mut conn, both, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("practices_contradict_none")
    ));

    // Either half alone is a legal answer: "0" is a real claim, and the section
    // is optional, so an empty set stays legal too.
    let mut alone = sample(&fx, &material_id);
    alone.practices = vec!["0".into()];
    let detail = repo::insert_fertilisation_record(&mut conn, alone, None).unwrap();
    assert_eq!(detail.practices, vec!["0".to_string()]);

    // A duplicate of "0" folds before the check, so it is not a contradiction
    // with itself.
    let mut twice = sample(&fx, &material_id);
    twice.practices = vec!["0".into(), "0".into()];
    let folded = repo::insert_fertilisation_record(&mut conn, twice, None).unwrap();
    assert_eq!(folded.practices, vec!["0".to_string()]);

    // And a correction cannot introduce it either — the register is correctable,
    // so the update path needs the same guard as the insert path.
    let update = UpdateFertilisationRecord {
        id: detail.record.id.clone(),
        applied_on: detail.record.applied_on.clone(),
        application_end_date: None,
        fertilisation_type_code: detail.record.fertilisation_type_code.clone(),
        application_method_code: detail.record.application_method_code.clone(),
        dose_value: detail.record.dose_value,
        dose_unit_code: detail.record.dose_unit_code.clone(),
        fertiliser_material_id: material_id.clone(),
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
        plots: detail
            .plots
            .iter()
            .map(|p| NewFertilisationPlot {
                plot_id: p.plot_id.clone(),
                crop_id: p.crop_id.clone(),
                fertilised_area_ha: p.fertilised_area_ha,
            })
            .collect(),
        practices: vec!["0".into(), "7".into()],
    };
    let err =
        repo::update_fertilisation_record(&mut conn, &detail.record.id, update, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("practices_contradict_none")
    ));
}

#[test]
fn correcting_a_record_reconciles_plots_and_practices() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.practices = vec!["3".into()];
    let created = repo::insert_fertilisation_record(&mut conn, new, Some("user-1")).unwrap();
    let kept_plot = created.plots[0].clone();

    let detail = repo::update_fertilisation_record(
        &mut conn,
        &created.record.id,
        UpdateFertilisationRecord {
            id: created.record.id.clone(),
            applied_on: "2026-03-13".into(),
            application_end_date: None,
            fertilisation_type_code: "base_dressing".into(),
            application_method_code: "banded_buried".into(),
            dose_value: 300.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: material_id.clone(),
            sludge_application: false,
            sustainable_input_management: false,
            irrigation_record_id: None,
            machinery_id: None,
            service_company: None,
            service_regfer_number: None,
            delivery_note_ref: Some("ALB-2026-118".into()),
            yield_estimated_kg_ha: Some(6500.0),
            yield_final_kg_ha: None,
            notes: None,
            plots: vec![
                NewFertilisationPlot {
                    plot_id: fx.plot_a.clone(),
                    crop_id: None,
                    fertilised_area_ha: Some(4.0),
                },
                NewFertilisationPlot {
                    plot_id: fx.plot_b.clone(),
                    crop_id: None,
                    fertilised_area_ha: Some(3.0),
                },
            ],
            practices: vec!["9".into()],
        },
        Some("user-1"),
    )
    .unwrap();

    assert_eq!(detail.record.applied_on, "2026-03-13");
    assert_eq!(detail.record.dose_value, 300.0);
    assert_eq!(
        detail.record.delivery_note_ref.as_deref(),
        Some("ALB-2026-118")
    );
    assert_eq!(detail.plots.len(), 2);
    assert_eq!(detail.practices, vec!["9".to_string()]);

    // The plot that stayed keeps its identity and is corrected in place.
    let corrected = detail
        .plots
        .iter()
        .find(|p| p.plot_id == fx.plot_a)
        .unwrap();
    assert_eq!(corrected.id, kept_plot.id);
    assert_eq!(corrected.fertilised_area_ha, Some(4.0));

    let (op, before, after) = last_change(&conn, "fertilisation_record", &created.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["dose_value"], 250.0);
    assert_eq!(after["dose_value"], 300.0);
}

#[test]
fn changing_the_material_retakes_the_snapshot() {
    // A record naming one fertiliser while printing another's richness would be
    // worse than either version of the mistake it was meant to fix.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let first = material(&mut conn);
    let mut urea = sample_material();
    urea.name = "Urea 46".into();
    urea.nutrients = vec![nutrient("macro", "1", 46.0)];
    let second = repo::insert_fertiliser_material(&mut conn, urea, None)
        .unwrap()
        .material
        .id;

    let created = repo::insert_fertilisation_record(&mut conn, sample(&fx, &first), None).unwrap();
    let detail = repo::update_fertilisation_record(
        &mut conn,
        &created.record.id,
        UpdateFertilisationRecord {
            id: created.record.id.clone(),
            applied_on: created.record.applied_on.clone(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "broadcast".into(),
            dose_value: 250.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: second.clone(),
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
            plots: vec![NewFertilisationPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: None,
                fertilised_area_ha: Some(3.5),
            }],
            practices: vec![],
        },
        None,
    )
    .unwrap();

    assert_eq!(detail.record.material_name_snapshot, "Urea 46");
    assert_eq!(detail.record.richness_n_snapshot, Some(46.0));
    assert!(detail.record.richness_k2o_snapshot.is_none());
}

#[test]
fn a_correction_keeps_the_snapshot_when_the_material_is_unchanged() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let created =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();

    // The registry entry is corrected after the record was written: a sack
    // relabelled, a richness figure fixed.
    conn.execute(
        "UPDATE fertiliser_material SET name = 'NAC 27 (corregido)' WHERE id = ?1",
        [&material_id],
    )
    .unwrap();
    conn.execute(
        "UPDATE fertiliser_material_nutrient SET percentage = 33.0
         WHERE fertiliser_material_id = ?1 AND kind_code = 'macro' AND nutrient_code = '1'",
        [&material_id],
    )
    .unwrap();

    // Correcting the dose names the material but does not change it, so what
    // the record printed stands — the freeze rule treatment_record follows.
    let detail = repo::update_fertilisation_record(
        &mut conn,
        &created.record.id,
        UpdateFertilisationRecord {
            id: created.record.id.clone(),
            applied_on: created.record.applied_on.clone(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "broadcast".into(),
            dose_value: 275.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: material_id.clone(),
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
            plots: vec![NewFertilisationPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: None,
                fertilised_area_ha: Some(3.5),
            }],
            practices: vec![],
        },
        None,
    )
    .unwrap();

    assert_eq!(detail.record.dose_value, 275.0);
    assert_eq!(detail.record.material_name_snapshot, "NAC 27");
    assert_eq!(detail.record.richness_n_snapshot, Some(27.0));
}

#[test]
fn rejects_an_unknown_type_or_method() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let mut bad_type = sample(&fx, &material_id);
    bad_type.fertilisation_type_code = "fertigation".into();
    let err = repo::insert_fertilisation_record(&mut conn, bad_type, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("unknown_fertilisation_type")
    ));

    let mut bad_method = sample(&fx, &material_id);
    bad_method.application_method_code = "sprayed".into();
    let err = repo::insert_fertilisation_record(&mut conn, bad_method, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("unknown_application_method")
    ));
}

#[test]
fn lists_a_seasons_records_oldest_first() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let mut later = sample(&fx, &material_id);
    later.applied_on = "2026-05-02".into();
    repo::insert_fertilisation_record(&mut conn, later, None).unwrap();
    repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();

    let listed = repo::list_fertilisation_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record.applied_on, "2026-03-12");
    assert_eq!(listed[1].record.applied_on, "2026-05-02");
}

#[test]
fn a_deleted_record_leaves_the_register_and_keeps_its_history() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let created =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();

    repo::soft_delete_fertilisation_record(&mut conn, &created.record.id, Some("user-1")).unwrap();

    assert!(
        repo::list_fertilisation_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "fertilisation_record", &created.record.id);
    assert_eq!(op, "delete");
    // Both images, as the audit contract requires for a soft delete.
    assert_eq!(before["deleted_at"], serde_json::Value::Null);
    assert!(after["deleted_at"].is_string());
}

#[test]
fn a_season_holding_only_a_fertilisation_reports_itself_in_use() {
    // The shell chains this before deleting a season. A season holding nothing
    // but a fertilisation record was deletable until this register existed —
    // the same gap seam 4 of the previous slice closed in module-cue.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());

    let created =
        repo::insert_fertilisation_record(&mut conn, sample(&fx, &material_id), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());

    repo::soft_delete_fertilisation_record(&mut conn, &created.record.id, None).unwrap();
    assert!(
        repo::season_has_records(&conn, &fx.season_id).unwrap(),
        "a soft-deleted record's audit history is only reachable through its season"
    );
}

#[test]
fn deleting_a_record_takes_its_children_with_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let mut new = sample(&fx, &material_id);
    new.practices = vec!["3".into()];
    let created = repo::insert_fertilisation_record(&mut conn, new, None).unwrap();

    conn.execute(
        "DELETE FROM fertilisation_record WHERE id = ?1",
        [&created.record.id],
    )
    .unwrap();

    let orphan_plots: i64 = conn
        .query_row("SELECT COUNT(*) FROM fertilisation_plot", [], |r| r.get(0))
        .unwrap();
    let orphan_practices: i64 = conn
        .query_row("SELECT COUNT(*) FROM fertilisation_practice", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(orphan_plots, 0);
    assert_eq!(orphan_practices, 0);
}

// ---------------------------------------------------------------------------
// The fertigation link (`Fertilizacion.Fertirrigacion`)
// ---------------------------------------------------------------------------
//
// One act the decree records twice: art. 5.d puts the fertiliser in this
// register and art. 5.e puts the water in `irrigation_record`. The exchange
// format re-joins them, and that block is the only reader anywhere of Anexo III
// C.l's two water-quality figures — so the farmer states the join rather than a
// serializer guessing it.

fn irrigation(conn: &mut Connection, fx: &Fixture) -> String {
    repo::insert_irrigation_record(
        conn,
        module_fertilisation::models::NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: "2026-03-12".into(),
            irrigation_end_date: None,
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: Some(12.5),
            water_soluble_p2o5_mg_l: Some(1.8),
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![module_fertilisation::models::NewIrrigationPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: None,
                irrigated_area_ha: Some(3.5),
            }],
            water_origins: vec![],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn a_fertigation_may_name_the_watering_that_carried_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let watering = irrigation(&mut conn, &fx);

    let record = repo::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_localised".into(),
            irrigation_record_id: Some(watering.clone()),
            ..sample(&fx, &material_id)
        },
        None,
    )
    .unwrap();
    assert_eq!(record.record.irrigation_record_id, Some(watering));
}

#[test]
fn only_a_fertigation_may_name_a_watering() {
    // On any other method the link would assert a fertigation that did not
    // happen. `is_fertigation` is read from the lookup, not matched on the code.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let watering = irrigation(&mut conn, &fx);

    assert!(matches!(
        repo::insert_fertilisation_record(
            &mut conn,
            NewFertilisationRecord {
                application_method_code: "broadcast".into(),
                irrigation_record_id: Some(watering),
                ..sample(&fx, &material_id)
            },
            None,
        )
        .unwrap_err(),
        module_fertilisation::error::FertilisationError::Invalid("link_needs_fertigation")
    ));
}

#[test]
fn the_link_may_not_reach_another_holding_another_campaign_or_a_withdrawn_watering() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);

    let fertigation = |irrigation_record_id: Option<String>| NewFertilisationRecord {
        application_method_code: "fertigation_sprinkler".into(),
        irrigation_record_id,
        ..sample(&fx, &material_id)
    };

    // Another campaign.
    let next_season = core_repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2026/2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let next_year = repo::insert_irrigation_record(
        &mut conn,
        module_fertilisation::models::NewIrrigationRecord {
            season_id: next_season.id.clone(),
            ..irrigation_payload(&fx)
        },
        None,
    )
    .unwrap()
    .record
    .id;
    assert!(matches!(
        repo::insert_fertilisation_record(&mut conn, fertigation(Some(next_year)), None)
            .unwrap_err(),
        module_fertilisation::error::FertilisationError::Invalid("irrigation_not_on_farm")
    ));

    // Withdrawn: a link is a statement about a LIVE register.
    let watering = irrigation(&mut conn, &fx);
    repo::soft_delete_irrigation_record(&mut conn, &watering, None).unwrap();
    assert!(matches!(
        repo::insert_fertilisation_record(&mut conn, fertigation(Some(watering)), None)
            .unwrap_err(),
        module_fertilisation::error::FertilisationError::NotFound
    ));
}

/// The irrigation payload the tests above vary, so a fixture change lands once.
fn irrigation_payload(fx: &Fixture) -> module_fertilisation::models::NewIrrigationRecord {
    module_fertilisation::models::NewIrrigationRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        irrigated_on: "2026-03-12".into(),
        irrigation_end_date: None,
        irrigation_method_code: "drip".into(),
        volume_value: 320.0,
        volume_unit_code: "m3_ha".into(),
        water_nitric_n_mg_l: Some(12.5),
        water_soluble_p2o5_mg_l: Some(1.8),
        energy_type_code: None,
        meter_number: None,
        notes: None,
        plots: vec![module_fertilisation::models::NewIrrigationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            irrigated_area_ha: Some(3.5),
        }],
        water_origins: vec![],
    }
}

#[test]
fn a_correction_that_stops_being_a_fertigation_must_drop_its_link() {
    // The guard reads the SUBMITTED method, not the stored one, so the two can
    // never disagree in a saved row.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let watering = irrigation(&mut conn, &fx);
    let saved = repo::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_localised".into(),
            irrigation_record_id: Some(watering.clone()),
            ..sample(&fx, &material_id)
        },
        None,
    )
    .unwrap();

    let correction = |method: &str, link: Option<String>| UpdateFertilisationRecord {
        id: saved.record.id.clone(),
        applied_on: "2026-03-12".into(),
        application_end_date: None,
        fertilisation_type_code: "top_dressing".into(),
        application_method_code: method.into(),
        dose_value: 250.0,
        dose_unit_code: "kg_ha".into(),
        fertiliser_material_id: material_id.clone(),
        sludge_application: false,
        sustainable_input_management: true,
        irrigation_record_id: link,
        machinery_id: None,
        service_company: None,
        service_regfer_number: None,
        delivery_note_ref: None,
        yield_estimated_kg_ha: None,
        yield_final_kg_ha: None,
        notes: None,
        plots: vec![NewFertilisationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            fertilised_area_ha: Some(3.5),
        }],
        practices: vec![],
    };

    assert!(matches!(
        repo::update_fertilisation_record(
            &mut conn,
            &saved.record.id,
            correction("broadcast", Some(watering)),
            None,
        )
        .unwrap_err(),
        module_fertilisation::error::FertilisationError::Invalid("link_needs_fertigation")
    ));

    let corrected = repo::update_fertilisation_record(
        &mut conn,
        &saved.record.id,
        correction("broadcast", None),
        None,
    )
    .unwrap();
    assert_eq!(corrected.record.irrigation_record_id, None);
    // And the twin-only insumos flag round-trips on the same correction.
    assert!(corrected.record.sustainable_input_management);
}
