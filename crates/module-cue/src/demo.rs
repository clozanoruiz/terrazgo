// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Demo seeding: one realistic Castilla y León campaign, inserted through the
//! public repository API so the data is exactly what the app would produce
//! (UUIDv7 ids, frozen legal snapshots, derived PHI end dates, audit log rows).
//!
//! Compiled only with the `demo` feature. Used by the `demo` example and by the
//! shell's dev-only `seed_demo_data` command. Product names and registration
//! numbers are realistic but illustrative — not regulatory reference data.
//!
//! The dates are deliberately near-future so the alert logic has something to
//! fire on: one PHI window still open (ends 2026-06-24), one already elapsed,
//! the sprayer ITV due 2026-07-01 and the operator licence expiring 2026-08-15.

use crate::error::Result;
use crate::models::{
    NewCrop, NewFarm, NewMachinery, NewOperator, NewPlot, NewProduct, NewProductAuthorisation,
    NewSeason, NewTreatmentPlot, NewTreatmentProblem, NewTreatmentRecord,
};
use crate::repository;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::models::{FarmEsFields, NewGeoFeature, PlotEsFields};
use terrazgo_core::repository::save_geo_feature;

/// Real SIGPAC recinto 47:182:0:0:7:14:1 — the exact `recinfo` response
/// harvested from sigpac-hubcloud.es on 2026-07-08 (SIGPAC © FEGA, CC BY 4.0).
/// Embedded so the demo can show a genuine official boundary fully offline;
/// pressing "verify against SIGPAC" on the plot re-fetches the live version.
const RECINTO_FIXTURE: &str = include_str!("demo/recinfo_47_182_0_0_7_14_1.geojson");

/// Just enough structure to lift geometry + attributes out of the vendored
/// response. The one-element array is enforced by the type, so a malformed
/// fixture fails as a serde error, not a panic.
#[derive(Deserialize)]
struct RecintoFixture {
    features: [RecintoFixtureFeature; 1],
}

#[derive(Deserialize)]
struct RecintoFixtureFeature {
    geometry: serde_json::Value,
    properties: serde_json::Map<String, serde_json::Value>,
}

/// What `seed_demo` did, in a shape the shell can hand straight to the UI.
#[derive(Debug, Serialize)]
pub struct DemoSeedSummary {
    /// `false` means the database already had farm data and nothing was touched.
    pub seeded: bool,
    pub farm_name: Option<String>,
    pub season_label: Option<String>,
    pub treatment_ids: Vec<String>,
}

/// Seed the demo campaign into an existing, migrated database.
///
/// Refuses to double-seed: if any farm exists the function returns
/// `seeded: false` without touching the database. (Re-seeding would trip
/// UNIQUE constraints — active-substance names, authorisation numbers — so the
/// guard keeps the dev command idempotent instead of erroring.)
pub fn seed_demo(conn: &mut Connection) -> Result<DemoSeedSummary> {
    let farms: i64 = conn.query_row("SELECT COUNT(*) FROM farm", [], |r| r.get(0))?;
    if farms > 0 {
        return Ok(DemoSeedSummary {
            seeded: false,
            farm_name: None,
            season_label: None,
            treatment_ids: Vec::new(),
        });
    }

    // --- season -------------------------------------------------------------
    let season = repository::insert_season(
        conn,
        NewSeason {
            campaign_year: 2026,
            label: "2025/2026".into(),
            starts_on: Some("2025-09-01".into()),
            ends_on: Some("2026-08-31".into()),
        },
        None,
    )?;

    // --- farm and plots -----------------------------------------------------
    let farm = repository::insert_farm(
        conn,
        NewFarm {
            name: "Finca Los Llanos".into(),
            owner_name: Some("Carlos Lozano".into()),
            owner_tax_id: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None, // no livestock on the demo farm
                rea_code: None,
                province_code: Some("47".into()), // Valladolid
            }),
        },
        None,
    )?;

    let la_vega = repository::insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "La Vega".into(),
            area_ha: Some(3.2),
            // Illustrative SIGPAC reference (provincia-municipio-polígono-parcela-recinto).
            // It does NOT exist in the registry: verifying this plot against SIGPAC
            // reports "not found" by design. Los Alcores below carries a real one.
            es: Some(PlotEsFields {
                sigpac_province: Some("47".into()),
                sigpac_municipality: Some("122".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("5".into()),
                sigpac_parcel: Some("23".into()),
                sigpac_enclosure: Some("1".into()),
            }),
        },
        None,
    )?;
    let el_paramo = repository::insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "El Páramo".into(),
            area_ha: Some(5.8),
            es: None,
        },
        None,
    )?;
    let carrascal = repository::insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "Carrascal".into(),
            area_ha: Some(2.1),
            es: None,
        },
        None,
    )?;
    // A plot with REAL SIGPAC data: the reference exists (irrigated arable
    // land on the Montes Torozos, official surface 8.897 ha), so "verify
    // against SIGPAC" succeeds on it — unlike La Vega's illustrative ref.
    // The declared area is deliberately a little lower than the official one
    // to exercise the declared-vs-official discrepancy display.
    let los_alcores = repository::insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "Los Alcores".into(),
            area_ha: Some(8.75),
            es: Some(PlotEsFields {
                sigpac_province: Some("47".into()),
                sigpac_municipality: Some("182".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("7".into()),
                sigpac_parcel: Some("14".into()),
                sigpac_enclosure: Some("1".into()),
            }),
        },
        None,
    )?;

    // Store the vendored official boundary the same way a live verification
    // would (`module-sigpac` also goes through core's `save_geo_feature`):
    // geometry + full attribute set + official area, never touching the
    // declared `plot.area_ha`. Zone flags are NOT seeded — they come from
    // query-only services, so the plot honestly shows "zones unchecked"
    // until the user runs a live verification.
    let fixture: RecintoFixture = serde_json::from_str(RECINTO_FIXTURE)?;
    let [recinto] = fixture.features;
    let official_area_ha = recinto
        .properties
        .get("superficie")
        .and_then(serde_json::Value::as_f64);
    save_geo_feature(
        conn,
        NewGeoFeature {
            plot_id: Some(los_alcores.id.clone()),
            farm_id: None,
            role: "boundary".into(),
            geometry: recinto.geometry.to_string(),
            source: "sigpac".into(),
            campaign: None,
            official_area_ha,
            properties: Some(serde_json::to_string(&recinto.properties)?),
            fetched_at: Some(now_utc_iso()),
        },
        None,
    )?;

    // --- crops for the campaign ----------------------------------------------
    let wheat_la_vega = repository::insert_crop(
        conn,
        NewCrop {
            plot_id: la_vega.id.clone(),
            season_id: season.id.clone(),
            species_name: "winter wheat".into(),
            variety: Some("Nogal".into()),
            production_system_code: Some("conventional".into()),
            sown_on: Some("2025-11-10".into()),
        },
        None,
    )?;
    let wheat_el_paramo = repository::insert_crop(
        conn,
        NewCrop {
            plot_id: el_paramo.id.clone(),
            season_id: season.id.clone(),
            species_name: "winter wheat".into(),
            variety: Some("Nogal".into()),
            production_system_code: Some("conventional".into()),
            sown_on: Some("2025-11-12".into()),
        },
        None,
    )?;
    let barley_carrascal = repository::insert_crop(
        conn,
        NewCrop {
            plot_id: carrascal.id.clone(),
            season_id: season.id.clone(),
            species_name: "barley".into(),
            variety: Some("Meseta".into()),
            production_system_code: Some("conventional".into()),
            sown_on: Some("2025-11-20".into()),
        },
        None,
    )?;
    // Spring-sown irrigated maize — the recinto's real coef_regadio is 100.
    repository::insert_crop(
        conn,
        NewCrop {
            plot_id: los_alcores.id.clone(),
            season_id: season.id.clone(),
            species_name: "maize".into(),
            variety: Some("LG 31.479".into()),
            production_system_code: Some("conventional".into()),
            sown_on: Some("2026-04-20".into()),
        },
        None,
    )?;

    // --- operator and machinery ----------------------------------------------
    let operator = repository::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Lozano".into(),
            licence_number: Some("CYL-2018-04567".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2026-08-15".into()),
        },
        None,
    )?;

    let sprayer = repository::insert_machinery(
        conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Hardi NK 600 sprayer".into(),
            kind: Some("sprayer".into()),
            last_inspection_date: Some("2023-07-01".into()),
            next_inspection_due_date: Some("2026-07-01".into()),
            // A mobile sprayer registers in ROMA (REGANIP is aircraft/fixed installations).
            roma_number: Some("VA-00123".into()),
            reganip_number: None,
        },
        None,
    )?;

    // --- products: fungicide and insecticide ----------------------------------
    let prosaro = repository::insert_product(
        conn,
        NewProduct {
            commercial_name: "Prosaro".into(),
            holder: Some("Bayer CropScience".into()),
            formulation_type_code: Some("ec".into()),
            default_phi_days: Some(35),
        },
        None,
    )?;
    let prothioconazole =
        repository::insert_active_substance(conn, "prothioconazole", Some("178928-70-6"), None)?;
    let tebuconazole =
        repository::insert_active_substance(conn, "tebuconazole", Some("107534-96-3"), None)?;
    repository::add_product_active_substance(
        conn,
        &prosaro.id,
        &prothioconazole.id,
        Some(125.0),
        Some("g_l"),
        None,
    )?;
    repository::add_product_active_substance(
        conn,
        &prosaro.id,
        &tebuconazole.id,
        Some(125.0),
        Some("g_l"),
        None,
    )?;
    repository::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: prosaro.id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25182".into(),
            kind_code: None, // defaults to 'registered'
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2019-03-01".into()),
            valid_until: Some("2031-12-31".into()),
        },
        None,
    )?;

    let karate = repository::insert_product(
        conn,
        NewProduct {
            commercial_name: "Karate Zeon".into(),
            holder: Some("Syngenta".into()),
            formulation_type_code: Some("sc".into()),
            default_phi_days: Some(30),
        },
        None,
    )?;
    let lambda_cyhalothrin =
        repository::insert_active_substance(conn, "lambda-cyhalothrin", Some("91465-08-6"), None)?;
    repository::add_product_active_substance(
        conn,
        &karate.id,
        &lambda_cyhalothrin.id,
        Some(100.0),
        Some("g_l"),
        None,
    )?;
    repository::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: karate.id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-22755".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2017-06-01".into()),
            valid_until: Some("2030-06-30".into()),
        },
        None,
    )?;

    // --- treatment 1: fungicide on both wheat plots (PHI window already past) --
    let t1 = repository::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            application_date: "2026-04-18".into(),
            product_id: prosaro.id.clone(),
            country_code: None, // derived from the farm
            dose_value: 1.0,
            dose_unit_code: "l_ha".into(),
            target_organism: Some("Septoria tritici, brown rust".into()),
            // Real SIEX ENFERMEDADES codes: 254 Septoriosis (Septoria spp.),
            // 416 Roya parda del trigo (Puccinia triticina).
            problems: vec![
                NewTreatmentProblem {
                    reason_category_code: "disease".into(),
                    problem_code: "254".into(),
                },
                NewTreatmentProblem {
                    reason_category_code: "disease".into(),
                    problem_code: "416".into(),
                },
            ],
            justifications: vec!["monitoring".into(), "advisor_recommendation".into()],
            // The PHI window is already past, so the efficacy has been observed.
            efficacy_code: Some("good".into()),
            operator_id: operator.id.clone(),
            machinery_id: Some(sprayer.id.clone()),
            phi_days_used: None, // falls back to the product default (35)
            notes: Some("Flag-leaf fungicide pass on both wheat plots.".into()),
        },
        vec![
            NewTreatmentPlot {
                plot_id: la_vega.id.clone(),
                crop_id: Some(wheat_la_vega.id.clone()),
                surface_treated_ha: 3.2,
            },
            NewTreatmentPlot {
                plot_id: el_paramo.id.clone(),
                crop_id: Some(wheat_el_paramo.id.clone()),
                surface_treated_ha: 5.8,
            },
        ],
        None,
    )?;

    // --- treatment 2: insecticide on the barley plot (PHI window still open) ---
    let t2 = repository::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            application_date: "2026-05-25".into(),
            product_id: karate.id.clone(),
            country_code: None,
            dose_value: 75.0,
            dose_unit_code: "ml_ha".into(),
            target_organism: Some("aphids (Sitobion avenae)".into()),
            // Real SIEX PLAGAS code: 135 Pulgón de la espiga (Sitobion avenae).
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "135".into(),
            }],
            justifications: vec!["threshold_exceeded".into()],
            // Recent treatment: efficacy not yet assessed — the realistic state.
            efficacy_code: None,
            operator_id: operator.id.clone(),
            machinery_id: Some(sprayer.id.clone()),
            phi_days_used: None, // product default (30)
            notes: Some("Aphid threshold exceeded on ear emergence.".into()),
        },
        vec![NewTreatmentPlot {
            plot_id: carrascal.id.clone(),
            crop_id: Some(barley_carrascal.id.clone()),
            surface_treated_ha: 2.1,
        }],
        None,
    )?;

    Ok(DemoSeedSummary {
        seeded: true,
        farm_name: Some(farm.name),
        season_label: Some(season.label),
        treatment_ids: vec![t1.id, t2.id],
    })
}
