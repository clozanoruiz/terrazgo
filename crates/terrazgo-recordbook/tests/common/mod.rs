// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared plumbing for this crate's integration tests.
//!
//! `terrazgo-testkit` is the workspace-wide half and is re-exported here so a
//! test file has one `mod common;` and one `use` line.
//!
//! The "export-ready Spanish farm" fixture is NOT here, and that is a decision
//! rather than an omission: `terrazgo-siex/tests/export.rs` builds a
//! field-for-field twin of it, and closing that duplication would mean the
//! testkit reaching into module-cue — which is precisely the back door the
//! core-only rule exists to keep shut. Roughly sixty duplicated lines is the
//! cheaper of the two costs, and it is the same trade the crates themselves
//! make: the book and the descriptor read the same registers and share no code.

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
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::models::{FarmEsFields, NewWaterPoint, PlotEsFields};
use terrazgo_recordbook::{ReportLanguage, cuaderno_inputs, cuaderno_workbook};

pub use terrazgo_testkit::last_change;

/// A migrated in-memory database at the composed schema the book reads: core
/// plus every module it projects.
pub fn db() -> Connection {
    terrazgo_recordbook::open_in_memory().unwrap()
}

/// The same, with the vendored FEGA catalogue snapshot imported — the state a
/// running app is always in.
///
/// Deliberately not what every test opens. Importing the snapshot parses 1.6 MB
/// of vendored CSV per call, but the cost is the lesser reason: opening through
/// here is the statement *this test resolves a code to a label*, and a file
/// where some tests have catalogues and some do not leaves the next reader
/// guessing which kind they are copying. The book prints an unresolvable code
/// as itself, so the two states print different pages.
pub fn db_with_catalogues() -> Connection {
    let mut conn = db();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}

// ---------------------------------------------------------------------------
// The book's fixture: a complete Spanish farm
//
// A field-for-field twin of `terrazgo-siex/tests/export.rs`'s, for the reason
// stated at the top of this file. It lives here rather than in `report.rs`
// because the register tests were split into one file per section of the book
// and every one of them builds on it.
// ---------------------------------------------------------------------------

/// The day the book is generated on. Fixed so a printed date is an assertion
/// rather than a moving target.
pub const GENERATED_ON: &str = "2026-07-16";

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
            label: "2025/2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let farm = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
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
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    // Alphabetical by name: "El Prado" (order 1) before "La Loma" (order 2).
    let wheat_plot_id = insert_plot(
        conn,
        &farm.id,
        "El Prado",
        4.0,
        Some(PlotEsFields {
            sigpac_province: Some("47".into()),
            sigpac_municipality: Some("186".into()),
            sigpac_aggregate: Some("0".into()),
            sigpac_zone: Some("0".into()),
            sigpac_polygon: Some("5".into()),
            sigpac_parcel: Some("23".into()),
            sigpac_enclosure: Some("1".into()),
        }),
    );
    let wheat_crop_id = insert_crop(
        conn,
        &wheat_plot_id,
        &season.id,
        "wheat",
        Some("Craklin"),
        Some("organic"),
    );
    let barley_plot_id = insert_plot(conn, &farm.id, "La Loma", 3.0, None);
    let barley_crop_id = insert_crop(conn, &barley_plot_id, &season.id, "barley", None, None);

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

pub fn insert_plot(
    conn: &mut Connection,
    farm_id: &str,
    name: &str,
    area_ha: f64,
    es: Option<PlotEsFields>,
) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(area_ha),
            es,
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
    production_system: Option<&str>,
) -> String {
    repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_id.into(),
            season_id: season_id.into(),
            species_name: species.into(),
            variety: variety.map(Into::into),
            production_system_code: production_system.map(Into::into),
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
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: Some("good".into()),
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        measure_code: None,
        measure_intensity_value: None,
        measure_intensity_unit_code: None,
        measure_registration_number: None,
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

/// The same treated plot, at a stated growth stage (`EST_FENOLOGICO` code).
pub fn on_plot_at_stage(
    plot_id: &str,
    crop_id: Option<&str>,
    surface: f64,
    stage: &str,
) -> NewTreatmentPlot {
    NewTreatmentPlot {
        growth_stage_code: Some(stage.into()),
        ..on_plot(plot_id, crop_id, surface)
    }
}

pub fn inputs(conn: &Connection, fx: &Fixture) -> Value {
    cuaderno_inputs(
        conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Helpers shared by more than one section file
// ---------------------------------------------------------------------------

/// The workbook description for the fixture farm.
pub fn workbook(conn: &Connection, fx: &Fixture) -> terrazgo_report::Workbook {
    terrazgo_recordbook::cuaderno_workbook(
        conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap()
}

pub fn sheet<'a>(book: &'a terrazgo_report::Workbook, name: &str) -> &'a terrazgo_report::Sheet {
    book.sheets
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("workbook should contain sheet '{name}'"))
}

pub fn water_point(
    conn: &mut Connection,
    plot_id: &str,
    denomination: &str,
    inside_plot: bool,
    distance_m: Option<f64>,
) {
    terrazgo_core::repository::insert_water_point(
        conn,
        terrazgo_core::models::NewWaterPoint {
            plot_id: plot_id.into(),
            denomination: denomination.into(),
            inside_plot,
            distance_m,
            latitude: None,
            longitude: None,
        },
        None,
    )
    .unwrap();
}

pub fn seed_treatment(fx: &Fixture) -> NewSeedTreatment {
    NewSeedTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sown_on: "2025-11-10".into(),
        species_name: "trigo blando".into(),
        variety: Some("Nogal".into()),
        crop_code: Some("1".into()),
        seed_quantity_kg: Some(680.0),
        seed_lot: Some("L-2025-4471".into()),
        treatment_kind_code: Some("purchased_es".into()),
        acquired_on: None,
        sowing_record_id: None,
        product_name: "Celest Trio".into(),
        product_registration_number: Some("ES-24.876".into()),
        product_active_substance: Some("fludioxonil + difenoconazol".into()),
        product_id: None,
        efficacy_code: Some("good".into()),
        notes: None,
        plots: vec![NewSeedTreatmentPlot {
            plot_id: fx.wheat_plot_id.clone(),
            surface_sown_ha: 3.2,
        }],
    }
}
