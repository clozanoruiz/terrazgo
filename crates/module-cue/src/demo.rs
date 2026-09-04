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
    NewAnalysisPlot, NewAnalysisRecord, NewCrop, NewFarm, NewMachinery, NewNonFieldTreatment,
    NewOperator, NewPlot, NewPremises, NewProduct, NewProductAuthorisation, NewSeason,
    NewSeedTreatment, NewSeedTreatmentPlot, NewTreatmentPlot, NewTreatmentProblem,
    NewTreatmentRecord,
};
use crate::repository;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::models::{
    FarmEsFields, FarmRepresentativeFields, NewAdvisor, NewGeoFeature, NewWaterPoint, PlotEsFields,
    UpdateFarm,
};
use terrazgo_core::repository::{
    insert_advisor, insert_water_point, save_geo_feature, set_farm_advisor, set_water_declaration,
    update_farm,
};

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
                siex_code: None,
                province_code: Some("47".into()), // Valladolid
            }),
        },
        None,
    )?;

    // The create form carries identity only; 1.1's full block is the edit
    // form's, so the demo fills it the way a farmer would — including the two
    // cells the printed page had no field for until this slice: the book's
    // opening date and the representative's province.
    update_farm(
        conn,
        &farm.id,
        UpdateFarm {
            name: "Finca Los Llanos".into(),
            owner_name: Some("Carlos Lozano".into()),
            owner_tax_id: Some("12345678Z".into()),
            location_text: Some("Medina del Campo".into()),
            address: Some("Camino de los Llanos, 14".into()),
            postal_code: Some("47400".into()),
            phone_fixed: Some("983 000 000".into()),
            phone_mobile: Some("600 000 000".into()),
            email: Some("finca@ejemplo.es".into()),
            opened_on: Some("2026-01-02".into()),
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
                siex_code: None,
                province_code: Some("47".into()),
            }),
            representative: Some(FarmRepresentativeFields {
                full_name: "Ana Lozano Ruiz".into(),
                tax_id: Some("87654321X".into()),
                representation_kind: Some("Administradora única".into()),
                address: Some("Calle Mayor, 3".into()),
                locality: Some("Valladolid".into()),
                province: Some("Valladolid".into()),
                postal_code: Some("47001".into()),
                phone: Some("600 111 222".into()),
                email: Some("ana@ejemplo.es".into()),
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

    // --- water abstraction points (official model 2.2's water half) -----------
    // All three printed states, so the rendered book shows each on one page:
    // La Vega carries two points (which join positionally across the four
    // cells), El Páramo is declared free of them, and the rest stay silent —
    // and silence is not the same claim as a checked negative.
    insert_water_point(
        conn,
        NewWaterPoint {
            plot_id: la_vega.id.clone(),
            denomination: "Pozo de la casa".into(),
            inside_plot: true,
            distance_m: None,
            latitude: Some(41.65234),
            longitude: Some(-4.72891),
        },
        None,
    )?;
    insert_water_point(
        conn,
        NewWaterPoint {
            plot_id: la_vega.id.clone(),
            denomination: "Sondeo municipal de Villanubla".into(),
            inside_plot: false,
            distance_m: Some(240.0),
            latitude: None,
            longitude: None,
        },
        None,
    )?;
    set_water_declaration(conn, &el_paramo.id, "2026-04-18", None)?;

    // --- advisory relationship (official model 1.4) ---------------------------
    // The demo holding belongs to an ATRIA, so table 1.4 prints a real row and
    // the crops below can state their GIP framework.
    let advisor = insert_advisor(
        conn,
        NewAdvisor {
            name: "ATRIA Cerealista de Castilla y León".into(),
            tax_id: Some("G47654321".into()),
            registration_number: Some("ROPO-AS-47-0912".into()),
        },
        None,
    )?;
    set_farm_advisor(conn, &farm.id, &advisor.id, Some("atria".into()), None)?;

    // --- crops for the campaign ----------------------------------------------
    let wheat_la_vega = repository::insert_crop(
        conn,
        NewCrop {
            plot_id: la_vega.id.clone(),
            season_id: season.id.clone(),
            species_name: "winter wheat".into(),
            variety: Some("Nogal".into()),
            production_system_code: Some("conventional".into()),
            area_ha: Some(3.2),
            irrigation_code: Some("rainfed".into()),
            growing_environment_code: Some("open_air".into()),
            gip_system_code: Some("atria".into()),
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
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
            area_ha: Some(5.8),
            irrigation_code: Some("rainfed".into()),
            growing_environment_code: Some("open_air".into()),
            gip_system_code: Some("atria".into()),
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
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
            area_ha: Some(2.1),
            irrigation_code: Some("rainfed".into()),
            growing_environment_code: Some("open_air".into()),
            // Deliberately unstated: the 2.1 GIP column then prints blank,
            // which is what "no consta" has to look like.
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
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
            area_ha: Some(8.75),
            irrigation_code: Some("sprinkler".into()),
            growing_environment_code: Some("open_air".into()),
            gip_system_code: Some("integrated_production".into()),
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )?;

    // --- operator and machinery ----------------------------------------------
    let operator = repository::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Lozano".into(),
            tax_id: Some("12345678Z".into()),
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
            acquired_on: Some("2018-03-15".into()),
            last_inspection_date: Some("2023-07-01".into()),
            next_inspection_due_date: Some("2026-07-01".into()),
            // A mobile sprayer registers in ROMA (REGANIP is aircraft/fixed installations).
            roma_number: Some("VA-00123".into()),
            reganip_number: None,
        },
        None,
    )?;

    // --- the premises registry (models 3.4 and 3.5) --------------------------
    // Registry rows only, and deliberately no record naming them: the demo's
    // 3.3 SÍ / 3.5 NO / 3.4 neither arrangement below shows all three states of
    // the model's "APLICA TRATAMIENTO" box, and filling 3.4 would cost one.
    terrazgo_core::repository::insert_premises(
        conn,
        NewPremises {
            farm_id: farm.id.clone(),
            kind_code: "building".into(),
            name: "Almacén de grano".into(),
            address: Some("Camino de la Vega, 4, 47170 Renedo de Esgueva".into()),
            vehicle_model: None,
            plate: None,
            // A plausible urban-style reference; the app never validates the
            // shape, and Anexo V types it as twenty characters. Both Spanish
            // registry fields ride flat and land in premises_es_extension.
            cadastral_reference: Some("47170A00500123 0000WX".into()),
            // What Edificaciones[].IdEdificacion wants — REA's own code for
            // this building, which the farmer reads off their REA papers.
            rea_installation_code: Some("4700123456".into()),
            // EDIFICACIONES_INSTALACIONES 3 = "Almacén de productos y materias
            // primas", which is where the treated grain of model 3.3 sits.
            class_code: Some("3".into()),
            volume_m3: Some(1200.0),
            notes: None,
        },
        None,
    )?;
    terrazgo_core::repository::insert_premises(
        conn,
        NewPremises {
            farm_id: farm.id.clone(),
            kind_code: "vehicle".into(),
            name: "Remolque bañera".into(),
            address: None,
            vehicle_model: Some("Rigual RB-14".into()),
            plate: Some("VA-04512-R".into()),
            // Buildings only: a trailer has a matrícula, and FEGA's catalogue
            // of edificaciones e instalaciones holds no vehicles.
            class_code: None,
            cadastral_reference: None,
            rea_installation_code: None,
            volume_m3: Some(24.0),
            notes: None,
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
            // Two plots, two days — the interval Anexo III Parte I B allows.
            // The plazo de seguridad is counted from the 19th.
            application_end_date: Some("2026-04-19".into()),
            // Unstated: a triazole fungicide restricts no hour, so Reglamento
            // (UE) 2023/564's footnote 4 does not make the hour relevant here.
            application_time: None,
            // Wheat is not a flooded crop, so there is no field to dry.
            drying_date: None,
            product_id: Some(prosaro.id.clone()),
            country_code: None, // derived from the farm
            dose_value: Some(1.0),
            dose_unit_code: Some("l_ha".into()),
            // 1 l/ha over 3,2 + 5,8 ha (Anexo III B.i).
            total_quantity_value: Some(9.0),
            total_quantity_unit_code: Some("l".into()),
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
            // Not an advised actuation: the wheat is under ATRIA, but this
            // pass was the holding's own call, so 3.1 bis does not claim it.
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            phi_days_used: None, // falls back to the product default (35)
            notes: Some("Flag-leaf fungicide pass on both wheat plots.".into()),
        },
        // Both plots carry the same species and variety, so the book prints
        // them as ONE row — and the two days caught them at different growth
        // stages, which is the case the printed cell has to state honestly
        // rather than picking one. EST_FENOLOGICO 5 = BBCH 4 (embuchamiento),
        // 6 = BBCH 5 (espigamiento).
        vec![
            NewTreatmentPlot {
                plot_id: la_vega.id.clone(),
                crop_id: Some(wheat_la_vega.id.clone()),
                surface_treated_ha: 3.2,
                growth_stage_code: Some("5".into()),
            },
            NewTreatmentPlot {
                plot_id: el_paramo.id.clone(),
                crop_id: Some(wheat_el_paramo.id.clone()),
                surface_treated_ha: 5.8,
                growth_stage_code: Some("6".into()),
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
            // A single-day pass, and the total left unstated — the honest
            // everyday state, and what a blank cell looks like in the book.
            application_end_date: None,
            drying_date: None,
            // Stated, and this is the case Reglamento (UE) 2023/564's footnote
            // 4 is about: a pyrethroid's label restricts application to outside
            // bee flight hours, so the hour is part of what makes the record
            // lawful and not just informative.
            application_time: Some("20:30".into()),
            product_id: Some(karate.id.clone()),
            country_code: None,
            dose_value: Some(75.0),
            dose_unit_code: Some("ml_ha".into()),
            total_quantity_value: None,
            total_quantity_unit_code: None,
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
            // Advised: the ATRIA technician called this one, so it carries the
            // asesor of Anexo III Parte I B.d and prints in 3.1 bis.
            advisor_id: Some(advisor.id.clone()),
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            phi_days_used: None, // product default (30)
            notes: Some("Aphid threshold exceeded on ear emergence.".into()),
        },
        // EST_FENOLOGICO 6 = BBCH 5, espigamiento — which is what the note
        // beneath this record says the aphids were found at.
        vec![NewTreatmentPlot {
            plot_id: carrascal.id.clone(),
            crop_id: Some(barley_carrascal.id.clone()),
            surface_treated_ha: 2.1,
            growth_stage_code: Some("6".into()),
        }],
        None,
    )?;

    // --- treatment 3: a purely NON-CHEMICAL actuation (model 3.1 bis) ------
    // RD 1311/2012 art. 10.1 asks professionals to prefer non-chemical methods,
    // and the record book has to be able to say one was taken: this is a
    // treatment with no product, no dose and no plazo de seguridad. The SIEX
    // twin agrees — TratamFito does not require ProductosFito.
    let t3 = repository::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            application_date: "2026-05-04".into(),
            application_end_date: None,
            drying_date: None,
            // Neither field stated: hanging diffusers is restricted to no hour
            // and to no growth stage, so both of the annex's conditional cells
            // print blank — which is the ordinary case.
            application_time: None,
            product_id: None,
            country_code: None,
            dose_value: None,
            dose_unit_code: None,
            total_quantity_value: None,
            total_quantity_unit_code: None,
            target_organism: Some("Sitobion avenae — confusión sexual".into()),
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "135".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: Some("fair".into()),
            operator_id: operator.id.clone(),
            machinery_id: None,
            advisor_id: Some(advisor.id.clone()),
            // TIPO_MEDIDA_FITOSANITARIA 15: feromonas y atrayentes para
            // monitoreo, at 4 diffusers per hectare over the barley.
            measure_code: Some("15".into()),
            measure_intensity_value: Some(4.0),
            measure_intensity_unit_code: Some("diffusers_ha".into()),
            measure_registration_number: None,
            phi_days_used: None,
            notes: Some("Difusores instalados antes del umbral de tratamiento.".into()),
        },
        vec![NewTreatmentPlot {
            plot_id: carrascal.id.clone(),
            crop_id: Some(barley_carrascal.id.clone()),
            surface_treated_ha: 2.1,
            growth_stage_code: None,
        }],
        None,
    )?;

    // --- how the wheat began (core's sowing register) ----------------------
    // Sown, not planted; the same act the treated-seed record below describes,
    // which is why that record names this one.
    let wheat_sowing = terrazgo_core::repository::insert_sowing_record(
        conn,
        terrazgo_core::models::NewSowingRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            kind_code: "sowing".into(),
            sown_on: "2025-11-10".into(),
            sowing_end_date: Some("2025-11-12".into()),
            flooded_on: None,
            seed_quantity_kg: Some(680.0),
            notes: None,
            plots: vec![
                terrazgo_core::models::NewSowingPlot {
                    plot_id: la_vega.id.clone(),
                    crop_id: Some(wheat_la_vega.id.clone()),
                },
                terrazgo_core::models::NewSowingPlot {
                    plot_id: el_paramo.id.clone(),
                    crop_id: Some(wheat_el_paramo.id.clone()),
                },
            ],
        },
        None,
    )?;

    // --- a sowing with treated seed (model 3.2) ----------------------------
    // The product is captured off the sack's label: treated seed names a
    // product the farmer never bought as such, so there is no registry row.
    repository::insert_seed_treatment(
        conn,
        NewSeedTreatment {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            sown_on: "2025-11-10".into(),
            species_name: "trigo blando".into(),
            variety: Some("Nogal".into()),
            crop_code: Some("1".into()), // PRODUCTOS: 1 = TRIGO BLANDO
            seed_quantity_kg: Some(680.0),
            seed_lot: Some("L-2025-4471".into()),
            // TIPO_TRATAMIENTO 4: bought already treated, in Spain.
            treatment_kind_code: Some("purchased_es".into()),
            // Bought a fortnight before it went in the ground.
            acquired_on: Some("2025-10-27".into()),
            sowing_record_id: Some(wheat_sowing.record.id.clone()),
            product_name: "Celest Trio".into(),
            product_registration_number: Some("ES-24.876".into()),
            product_active_substance: Some("fludioxonil + difenoconazol".into()),
            product_id: None,
            efficacy_code: Some("good".into()),
            notes: Some("Semilla certificada tratada en origen.".into()),
            plots: vec![
                NewSeedTreatmentPlot {
                    plot_id: la_vega.id.clone(),
                    surface_sown_ha: 3.2,
                },
                NewSeedTreatmentPlot {
                    plot_id: el_paramo.id.clone(),
                    surface_sown_ha: 5.8,
                },
            ],
        },
        None,
    )?;

    // --- a postharvest treatment (model 3.3) + a declared-empty register ---
    // Between them the four conditional registers show all three states the
    // "APLICA TRATAMIENTO" boxes can take: SÍ (rows), NO (declared empty) and
    // neither (nobody has been near it yet).
    repository::insert_non_field_treatment(
        conn,
        NewNonFieldTreatment {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            country_code: None,
            subject_kind_code: "postharvest".into(),
            // Produce, not a place: a postharvest record names no premises.
            premises_id: None,
            treated_on: "2026-08-20".into(),
            subject_description: "Trigo blando de la cosecha 2026, silo 2".into(),
            // PROD_VEGETAL 85 = Granos de trigo (the harvested produce, not
            // the crop: PRODUCTOS 1 is TRIGO BLANDO).
            subject_product_code: Some("85".into()),
            // Produce is measured in tonnes; the product used, in kilograms.
            treated_quantity_value: Some(120.0),
            treated_quantity_unit_code: Some("t".into()),
            product_id: karate.id.clone(),
            product_quantity_value: Some(2.4),
            product_quantity_unit_code: Some("kg".into()),
            operator_id: operator.id.clone(),
            machinery_id: None,
            // Anexo III B.d reaches the postharvest register through B.b, so
            // the advised case is worth seeding here too.
            advisor_id: Some(advisor.id.clone()),
            // Real SIEX PLAGAS code: 135 Pulgón de la espiga; stored grain
            // pests share the catalogue.
            problems: vec![NewTreatmentProblem {
                reason_category_code: "pest".into(),
                problem_code: "135".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: Some("good".into()),
            notes: Some("Fumigación preventiva del grano almacenado.".into()),
        },
        None,
    )?;

    repository::set_register_declaration(
        conn,
        &farm.id,
        &season.id,
        "transport",
        "2026-09-01",
        None,
    )?;

    // --- an analysis (model 4) and a sale (model 5) -----------------------
    // Metadata only: the register says where the bulletin is, never holds it.
    repository::insert_analysis_record(
        conn,
        NewAnalysisRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            sampled_on: "2026-06-18".into(),
            material_kind_code: "harvested_produce".into(),
            bulletin_number: Some("B-2026/1187".into()),
            lab_name: Some("Laboratorio Agroalimentario de Castilla y León".into()),
            lab_address: Some("Ctra. Burgos km 118, 47071 Valladolid".into()),
            lab_tax_id: Some("Q4700123B".into()),
            substances_detected: Some("Lambda cihalotrín 0,01 mg/kg (LMR 0,05)".into()),
            soil: Default::default(),
            notes: Some("Muestreo previo a la cosecha.".into()),
            plots: vec![
                NewAnalysisPlot {
                    plot_id: la_vega.id.clone(),
                    crop_id: Some(wheat_la_vega.id.clone()),
                },
                NewAnalysisPlot {
                    plot_id: el_paramo.id.clone(),
                    crop_id: Some(wheat_el_paramo.id.clone()),
                },
            ],
            analysis_type_codes: vec!["pesticide_residues".into()],
            // SUST_ACTIVAS 170 = LAMBDA CIHALOTRINA, the substance the free
            // text above quantifies — the code and the wording answer
            // different questions, so the register keeps both.
            substance_codes: vec!["170".into()],
        },
        None,
    )?;

    // Section 5 is core-owned: what leaves the holding is whole-farm data.
    terrazgo_core::repository::insert_harvest_record(
        conn,
        terrazgo_core::models::NewHarvestRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            harvested_on: "2026-07-24".into(),
            product_name: "Trigo blando".into(),
            plant_product_code: Some("85".into()), // PROD_VEGETAL: Granos de trigo
            quantity_value: Some(42.5),
            quantity_unit_code: Some("t".into()),
            delivery_note_ref: Some("ALB-2026/318".into()),
            lot_number: Some("L-26-07".into()),
            buyer_name: "Cooperativa Cerealista del Duero S. Coop.".into(),
            buyer_tax_id: Some("F47008123".into()),
            buyer_address: Some("Ctra. Palencia km 4, 47009 Valladolid".into()),
            buyer_registry_number: Some("21.0012345/VA".into()),
            notes: None,
            plots: vec![
                terrazgo_core::models::NewHarvestPlot {
                    plot_id: la_vega.id.clone(),
                    crop_id: Some(wheat_la_vega.id.clone()),
                },
                terrazgo_core::models::NewHarvestPlot {
                    plot_id: el_paramo.id.clone(),
                    crop_id: Some(wheat_el_paramo.id.clone()),
                },
            ],
        },
        None,
    )?;

    Ok(DemoSeedSummary {
        seeded: true,
        farm_name: Some(farm.name),
        season_label: Some(season.label),
        treatment_ids: vec![t1.id, t2.id, t3.id],
    })
}
