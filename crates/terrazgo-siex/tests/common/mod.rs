// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared plumbing for this crate's integration tests.
//!
//! `terrazgo-testkit` is the workspace-wide half and is re-exported here so a
//! test file has one `mod common;` and one `use` line.
//!
//! The "export-ready Spanish farm" fixture is NOT here, and that is a decision
//! rather than an omission: `terrazgo-recordbook/tests/report.rs` builds a
//! field-for-field twin of it, and closing that duplication would mean the
//! testkit reaching into module-cue — which is precisely the back door the
//! core-only rule exists to keep shut. Roughly sixty duplicated lines is the
//! cheaper of the two costs, and it is the same trade the crates themselves
//! make: the descriptor and the book read the same registers and share no code.

// Each test binary compiles this whole module and uses a subset of it, so what
// one binary does not touch is not dead code — it is the other binaries' half
// of the shared helper. `unused_imports` covers the re-exports below for the
// same reason.
#![allow(dead_code, unused_imports)]
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::repository as repo;
use module_fertilisation::models::{
    MaterialNutrient, NewFertilisationPlot, NewFertilisationRecord, NewFertiliserMaterial,
    NewIrrigationPlot, NewIrrigationRecord,
};
use module_fertilisation::repository as fert;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::LazyLock;
use terrazgo_core::models::FarmEsFields;
use terrazgo_siex::build_cuaderno;

pub use terrazgo_testkit::last_change;

/// A migrated in-memory database at the composed schema the descriptor reads:
/// core plus every module it projects.
pub fn db() -> Connection {
    terrazgo_siex::open_in_memory().unwrap()
}

/// The same, with the vendored FEGA catalogue snapshot imported — the state a
/// running app is always in.
///
/// Deliberately not what every test opens. Importing the snapshot parses 1.6 MB
/// of vendored CSV per call, but the cost is the lesser reason: opening through
/// here is the statement *this test resolves a code to a label*, and a file
/// where some tests have catalogues and some do not leaves the next reader
/// guessing which kind they are copying.
pub fn db_with_catalogues() -> Connection {
    let mut conn = db();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}

// ---------------------------------------------------------------------------
// The schema validator, and the export the tests read back
// ---------------------------------------------------------------------------

// The official schema, compiled once for the whole test binary. FEGA's file
// carries one malformed `$id` ("##root/…" under SiembraPlantacion/Maquinaria —
// a double '#', not a valid uri-reference), which draft-07 meta-validation
// rightly rejects. The `$id`s are decorative labels (the schema has no $ref),
// so the in-memory copy normalizes that typo; the vendored artifact stays
// byte-exact, like every official reference file.
static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let raw = include_str!("../../../../docs/references/cue-schema-3.11.4.json");
    let schema: Value = serde_json::from_str(&raw.replace("\"##root", "\"#root")).unwrap();
    jsonschema::validator_for(&schema).unwrap()
});

pub fn assert_schema_valid(doc: &Value) {
    let errors: Vec<String> = VALIDATOR
        .iter_errors(doc)
        .map(|e| format!("{e} @ {}", e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "schema violations: {errors:#?}");
}

pub fn export_json(conn: &mut Connection, season_id: &str, farm_id: &str) -> Value {
    let cuaderno = build_cuaderno(conn, season_id, farm_id, None).unwrap();
    serde_json::to_value(&cuaderno).unwrap()
}

// ---------------------------------------------------------------------------
// The descriptor's fixture: a complete, export-ready Spanish farm
//
// A field-for-field twin of `terrazgo-recordbook/tests/report.rs`'s, for the
// reason stated at the top of this file. It lives here rather than in
// `export.rs` because the block tests were split into one file per group of
// SIEX blocks and every one of them builds on it.
// ---------------------------------------------------------------------------

pub struct Fixture {
    pub season_id: String,
    pub farm_id: String,
    pub operator_id: String,
    pub product_id: String,
    pub wheat_plot_id: String,
    pub wheat_crop_id: String,
    pub barley_plot_id: String,
    pub barley_crop_id: String,
}

pub fn fixture(conn: &mut Connection) -> Fixture {
    let season = repo::insert_season(
        conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    // Valladolid (47) → Castilla y León (CAExplotacion 07).
    let farm = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                // The holding's own livestock registry code. Nothing read it
                // until `Pastoreo` arrived, and it is what separates this
                // farm's animals from a third party's on every grazing line.
                rega_code: Some("ES471820000001".into()),
                rea_code: Some("ES244700000123".into()),
                siex_code: None,
                province_code: Some("47".into()),
            }),
        },
        None,
    )
    .unwrap();

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("ROPO-4700123".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2027-03-01".into()),
        },
        None,
    )
    .unwrap()
    .id;

    let product_id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: "Fungitop".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: Some(21),
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None, // defaults to 'registered'
            exceptional_substance_code: None,
            status: None,
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    let wheat_plot_id = insert_plot(conn, &farm.id, "El Prado", 4.0);
    let wheat_crop_id = insert_crop(conn, &wheat_plot_id, &season.id, "wheat", Some("Craklin"));
    let barley_plot_id = insert_plot(conn, &farm.id, "La Loma", 3.0);
    let barley_crop_id = insert_crop(conn, &barley_plot_id, &season.id, "barley", None);

    Fixture {
        season_id: season.id,
        farm_id: farm.id,
        operator_id,
        product_id,
        wheat_plot_id,
        wheat_crop_id,
        barley_plot_id,
        barley_crop_id,
    }
}

pub fn insert_plot(conn: &mut Connection, farm_id: &str, name: &str, area_ha: f64) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(area_ha),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

pub fn insert_crop(
    conn: &mut Connection,
    plot_id: &str,
    season_id: &str,
    species: &str,
    variety: Option<&str>,
) -> String {
    repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_id.into(),
            season_id: season_id.into(),
            species_name: species.into(),
            variety: variety.map(Into::into),
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

/// A ready-to-insert single-problem treatment; tests tweak what they exercise.
pub fn treatment(fx: &Fixture, application_date: &str) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        application_date: application_date.into(),
        application_end_date: None,
        drying_date: None,
        application_time: None,
        product_id: Some(fx.product_id.clone()),
        country_code: None,
        dose_value: Some(1.5),
        dose_unit_code: Some("l_ha".into()),
        total_quantity_value: None,
        total_quantity_unit_code: None,
        target_organism: None,
        advisor_id: None,
        measure_code: None,
        measure_intensity_value: None,
        measure_intensity_unit_code: None,
        measure_registration_number: None,
        // ENFERMEDADES code 254 (mildiu) — a real catalogue code, per the
        // demo-seed convention.
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: Some("good".into()),
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        phi_days_used: None,
        notes: None,
    }
}

pub fn on_plot(plot_id: &str, crop_id: Option<&str>, surface: f64) -> NewTreatmentPlot {
    NewTreatmentPlot {
        plot_id: plot_id.into(),
        crop_id: crop_id.map(Into::into),
        surface_treated_ha: surface,
        growth_stage_code: None,
    }
}

pub fn insert_machinery(
    conn: &mut Connection,
    farm_id: &str,
    roma: Option<&str>,
    reganip: Option<&str>,
) -> String {
    repo::insert_machinery(
        conn,
        NewMachinery {
            farm_id: farm_id.into(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: roma.map(Into::into),
            reganip_number: reganip.map(Into::into),
        },
        None,
    )
    .unwrap()
    .id
}

pub fn treatment_activities(doc: &Value) -> &Vec<Value> {
    doc["CUADERNO"][0]["ActividadesExplotacion"]["TratamFito"]
        .as_array()
        .unwrap()
}

// ---------------------------------------------------------------------------
// Helpers shared by more than one block file
// ---------------------------------------------------------------------------

pub fn block<'a>(doc: &'a Value, name: &str) -> &'a Vec<Value> {
    doc["CUADERNO"][0]["ActividadesExplotacion"][name]
        .as_array()
        .unwrap_or_else(|| panic!("block {name} missing from the export"))
}

pub fn seed_treatment(fx: &Fixture) -> NewSeedTreatment {
    NewSeedTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sown_on: "2025-10-15".into(),
        species_name: "Trigo blando".into(),
        variety: Some("Craklin".into()),
        // PRODUCTOS 1 = TRIGO BLANDO — the CROP, which is what the block's
        // `Producto` member actually asks for.
        crop_code: Some("1".into()),
        seed_quantity_kg: Some(1800.0),
        seed_lot: Some("L-2025-4471".into()),
        // Seed bought already treated with a product authorised in Spain.
        // TIPO_TRATAMIENTO 4/5 are acquisitions, so the precheck demands both
        // the lot and the purchase date.
        treatment_kind_code: Some("purchased_es".into()),
        acquired_on: Some("2025-09-30".into()),
        sowing_record_id: None,
        product_name: "Celest Trio".into(),
        product_registration_number: Some("ES-24.567".into()),
        product_active_substance: Some("Fludioxonil".into()),
        product_id: None,
        efficacy_code: Some("good".into()),
        notes: None,
        plots: vec![NewSeedTreatmentPlot {
            plot_id: fx.wheat_plot_id.clone(),
            surface_sown_ha: 4.0,
        }],
    }
}

/// A complete sale (model 5): the three members the block requires.
pub fn harvest(fx: &Fixture) -> terrazgo_core::models::NewHarvestRecord {
    terrazgo_core::models::NewHarvestRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        harvested_on: "2026-07-24".into(),
        product_name: "Trigo blando".into(),
        // PROD_VEGETAL 85 = Granos de trigo — the produce that LEFT, never the
        // PRODUCTOS code of the crop that grew.
        plant_product_code: Some("85".into()),
        quantity_value: Some(42.5),
        quantity_unit_code: Some("t".into()),
        delivery_note_ref: Some("ALB-2026/318".into()),
        lot_number: Some("L-26-07".into()),
        buyer_name: "Cooperativa Cerealista del Duero".into(),
        buyer_tax_id: Some("F47008123".into()),
        buyer_address: Some("Ctra. Palencia km 4".into()),
        buyer_registry_number: Some("21.0012345/VA".into()),
        notes: None,
        plots: vec![terrazgo_core::models::NewHarvestPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
        }],
    }
}

pub fn fertilisation(fx: &Fixture, material_id: &str) -> NewFertilisationRecord {
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
        sustainable_input_management: true,
        irrigation_record_id: None,
        machinery_id: None,
        service_company: Some("Servicios Agrícolas del Duero".into()),
        service_regfer_number: Some("REGFER-4471".into()),
        delivery_note_ref: None,
        yield_estimated_kg_ha: None,
        yield_final_kg_ha: None,
        notes: None,
        plots: vec![NewFertilisationPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
            fertilised_area_ha: Some(3.5),
        }],
        // BUENAS_PRACTICAS_AMBITOS, "Fertilización" ámbito.
        practices: vec!["4".into()],
    }
}

pub fn irrigation(fx: &Fixture) -> NewIrrigationRecord {
    NewIrrigationRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        irrigated_on: "2026-06-14".into(),
        irrigation_end_date: Some("2026-06-28".into()),
        irrigation_method_code: "drip".into(),
        volume_value: 320.0,
        volume_unit_code: "m3_ha".into(),
        water_nitric_n_mg_l: Some(12.5),
        water_soluble_p2o5_mg_l: Some(1.8),
        energy_type_code: Some("2".into()),
        meter_number: Some("C-4471".into()),
        notes: None,
        plots: vec![NewIrrigationPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
            irrigated_area_ha: Some(3.5),
        }],
        water_origins: vec!["groundwater".into()],
    }
}

/// A material with one entry in each of the three composition arrays, so the
/// `kind_code` split is exercised end to end.
pub fn material(conn: &mut Connection) -> String {
    fert::insert_fertiliser_material(
        conn,
        NewFertiliserMaterial {
            name: "Purín de porcino".into(),
            // MAT_FERTI 5 — a slurry, which is why the supplier block is filled.
            material_code: "5".into(),
            material_detail_code: None,
            supplier_name: Some("Ganadería del Duero S.L.".into()),
            supplier_rega: Some("ES471820000123".into()),
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: Some("composting".into()),
            density_kg_l: Some(1.03),
            notes: None,
            nutrients: vec![
                MaterialNutrient {
                    id: String::new(),
                    kind_code: "macro".into(),
                    nutrient_code: "3".into(),
                    percentage: 0.4,
                },
                MaterialNutrient {
                    id: String::new(),
                    kind_code: "micro".into(),
                    nutrient_code: "2".into(),
                    percentage: 0.01,
                },
                MaterialNutrient {
                    id: String::new(),
                    kind_code: "heavy_metal".into(),
                    nutrient_code: "3".into(),
                    percentage: 0.002,
                },
            ],
        },
        None,
    )
    .unwrap()
    .material
    .id
}
