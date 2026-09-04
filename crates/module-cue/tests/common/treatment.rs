// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The treatment fixture, shared by every `repository_*.rs` file.
//!
//! It lives in a submodule rather than in `common/mod.rs` because it is the
//! repository tests' fixture and nothing else's: the per-register files
//! (`alerts.rs`, `analysis.rs`, `seed_treatment.rs`,
//! `non_field_treatment.rs`) build their own, for the reason stated in
//! `common/mod.rs`.

// One binary uses a subset of this; that is what shared means, not dead code.
#![allow(dead_code)]
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;

/// Common fixture: one season, one ES farm, one operator, and a product (no authorisation
/// yet — tests add the authorisations they need). Returns the ids tests build treatments from.
pub struct Fixture {
    pub season_id: String,
    pub farm_id: String, // country 'es'
    pub operator_id: String,
    pub product_id: String,
}

pub fn base_fixture(conn: &mut Connection) -> Fixture {
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

    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("CL-12345".into()),
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
            holder: Some("AgroCorp".into()),
            formulation_type_code: Some("sc".into()),
            default_phi_days: Some(21), // PHI per product label
        },
        None,
    )
    .unwrap()
    .id;

    let substance =
        repo::insert_active_substance(conn, "azoxistrobin", Some("131860-33-8"), None).unwrap();
    repo::add_product_active_substance(
        conn,
        &product_id,
        &substance.id,
        Some(250.0),
        Some("g_l"),
        None,
    )
    .unwrap();

    Fixture {
        season_id: season.id,
        farm_id,
        operator_id,
        product_id,
    }
}

/// Build a single-plot treatment input. `country_code` is the optional explicit override.
pub fn sample_treatment(
    fx: &Fixture,
    country_code: Option<&str>,
    phi_days_used: Option<i64>,
) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        application_date: "2026-05-01".into(),
        application_end_date: None,
        drying_date: None,
        application_time: None,
        product_id: Some(fx.product_id.clone()),
        country_code: country_code.map(str::to_string),
        dose_value: Some(1.0),
        dose_unit_code: Some("l_ha".into()),
        total_quantity_value: None,
        total_quantity_unit_code: None,
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "1".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        target_organism: None,
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        measure_code: None,
        measure_intensity_value: None,
        measure_intensity_unit_code: None,
        measure_registration_number: None,
        phi_days_used,
        notes: None,
    }
}

pub fn add_es_authorisation(conn: &mut Connection, product_id: &str) {
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.into(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();
}

pub fn add_status_plot(conn: &mut Connection, farm_id: &str, name: &str) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

pub fn status_of<'a>(rows: &'a [PlotPhiStatus], plot_id: &str) -> &'a PlotPhiStatus {
    rows.iter()
        .find(|r| r.plot_id == plot_id)
        .expect("plot missing from PHI status")
}

/// A treated plot to hang the junction tests on.
pub fn add_plot(conn: &mut Connection, farm_id: &str, name: &str) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

pub fn one_plot(plot_id: &str) -> Vec<NewTreatmentPlot> {
    vec![NewTreatmentPlot {
        plot_id: plot_id.into(),
        crop_id: None,
        surface_treated_ha: 1.0,
        growth_stage_code: None,
    }]
}

/// One treated plot, one record, ready to correct.
pub fn correctable_record(conn: &mut Connection, fx: &Fixture) -> (TreatmentRecord, String) {
    add_es_authorisation(conn, &fx.product_id);
    let plot = repo::insert_plot(
        conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let record = repo::insert_treatment_record(
        conn,
        sample_treatment(fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();
    (record, plot)
}

/// The submitted state, built from a record so a test can change one thing.
pub fn correction_of(
    record: &TreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
) -> UpdateTreatmentRecord {
    UpdateTreatmentRecord {
        application_date: record.application_date.clone(),
        application_end_date: record.application_end_date.clone(),
        application_time: record.application_time.clone(),
        drying_date: record.drying_date.clone(),
        product_id: record.product_id.clone(),
        dose_value: record.dose_value,
        dose_unit_code: record.dose_unit_code.clone(),
        total_quantity_value: record.total_quantity_value,
        total_quantity_unit_code: record.total_quantity_unit_code.clone(),
        target_organism: record.target_organism.clone(),
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "1".into(),
        }],
        justifications: vec!["monitoring".into()],
        operator_id: record.operator_id.clone(),
        machinery_id: record.machinery_id.clone(),
        advisor_id: record.advisor_id.clone(),
        measure_code: record.measure_code.clone(),
        measure_intensity_value: record.measure_intensity_value,
        measure_intensity_unit_code: record.measure_intensity_unit_code.clone(),
        measure_registration_number: record.measure_registration_number.clone(),
        phi_days_used: record.phi_days_used,
        notes: record.notes.clone(),
        plots,
    }
}
