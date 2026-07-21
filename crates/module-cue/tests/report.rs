// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Printable cuaderno (docs/siex-export.md arc, part 2): the data contract
//! `cuaderno_inputs` feeds the Typst template, pinned as JSON — order-number
//! cross-references (model tables 1.2/1.3/2.1 ↔ 3.1), Spanish formatting,
//! per-crop-group register rows (same split as the SIEX export) — plus the
//! end-to-end render (real PDF bytes, zero template warnings).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::report::{cuaderno_inputs, render_cuaderno};
use module_cue::repository as repo;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::models::{FarmEsFields, PlotEsFields};

const GENERATED_ON: &str = "2026-07-16";

// ---------------------------------------------------------------------------
// Fixture: mirrors tests/export.rs — a complete Spanish farm
// ---------------------------------------------------------------------------

struct Fixture {
    season_id: String,
    farm_id: String,
    operator_id: String,
    product_id: String,
    wheat_plot_id: String,
    wheat_crop_id: String,
    barley_plot_id: String,
    barley_crop_id: String,
}

fn fixture(conn: &mut Connection) -> Fixture {
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

fn insert_plot(
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

fn insert_crop(
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
            sown_on: None,
        },
        None,
    )
    .unwrap()
    .id
}

fn treatment(fx: &Fixture, application_date: &str) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        application_date: application_date.into(),
        product_id: fx.product_id.clone(),
        country_code: None,
        dose_value: 1.5,
        dose_unit_code: "l_ha".into(),
        target_organism: None,
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

fn on_plot(plot_id: &str, crop_id: Option<&str>, surface: f64) -> NewTreatmentPlot {
    NewTreatmentPlot {
        plot_id: plot_id.into(),
        crop_id: crop_id.map(Into::into),
        surface_treated_ha: surface,
    }
}

fn inputs(conn: &Connection, fx: &Fixture) -> Value {
    cuaderno_inputs(conn, &fx.season_id, &fx.farm_id, GENERATED_ON).unwrap()
}

// ---------------------------------------------------------------------------
// The data contract
// ---------------------------------------------------------------------------

#[test]
fn inputs_carry_farm_identity_campaign_and_generation_date() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["campaign"], "2025/2026");
    assert_eq!(doc["generated_on"], "16/07/2026");
    assert_eq!(doc["farm"]["name"], "Finca La Vega");
    assert_eq!(doc["farm"]["owner"], "María García");
    assert_eq!(doc["farm"]["nif"], "12345678Z");
    assert_eq!(doc["farm"]["rea"], "ES244700000123");
    assert_eq!(doc["farm"]["province"], "47");
}

#[test]
fn a_farm_without_regional_data_prints_blank_not_missing() {
    let mut conn = open_in_memory().unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Bare".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();

    let doc = cuaderno_inputs(&conn, &season.id, &farm.id, GENERATED_ON).unwrap();
    // Blank strings, never null/absent: the template prints them as the
    // empty cells an official form leaves for hand-filling.
    assert_eq!(doc["farm"]["nif"], "");
    assert_eq!(doc["farm"]["rea"], "");
    assert_eq!(doc["farm"]["province"], "");
    assert_eq!(doc["plot_rows"].as_array().unwrap().len(), 0);
    assert_eq!(doc["treatments"].as_array().unwrap().len(), 0);
}

#[test]
fn plot_rows_number_parcelas_and_join_the_season_crops() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);

    // Alphabetical plot order; SIGPAC reference column by column; the GIP
    // sigla from the crop's production system (organic → AE, model 2.1
    // footnote 2); Spanish decimal comma on the surface.
    assert_eq!(rows[0]["order"], "1");
    assert_eq!(rows[0]["name"], "El Prado");
    assert_eq!(rows[0]["province"], "47");
    assert_eq!(rows[0]["municipality"], "186");
    assert_eq!(rows[0]["polygon"], "5");
    assert_eq!(rows[0]["parcel"], "23");
    assert_eq!(rows[0]["enclosure"], "1");
    assert_eq!(rows[0]["area"], "4");
    assert_eq!(rows[0]["species"], "wheat");
    assert_eq!(rows[0]["variety"], "Craklin");
    assert_eq!(rows[0]["gip"], "AE");

    assert_eq!(rows[1]["order"], "2");
    assert_eq!(rows[1]["name"], "La Loma");
    assert_eq!(rows[1]["province"], ""); // no SIGPAC reference entered
    assert_eq!(rows[1]["species"], "barley");
    assert_eq!(rows[1]["gip"], "");
}

#[test]
fn register_rows_reference_operators_equipment_and_plots_by_order_number() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let machinery_id = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            last_inspection_date: Some("2026-02-10".into()),
            next_inspection_due_date: None,
            roma_number: Some("RM-47-0042".into()),
            reganip_number: None,
        },
        None,
    )
    .unwrap()
    .id;

    // Two records: a manual one, then one with the sprayer.
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();
    let mut second = treatment(&fx, "2026-05-20");
    second.machinery_id = Some(machinery_id);
    repo::insert_treatment_record(
        &mut conn,
        second,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);

    // 1.2: one operator, snapshot name + licence, level from the live row.
    let operators = doc["operators"].as_array().unwrap();
    assert_eq!(operators.len(), 1);
    assert_eq!(operators[0]["order"], "1");
    assert_eq!(operators[0]["name"], "Carlos Pérez");
    assert_eq!(operators[0]["licence"], "ROPO-4700123");
    assert_eq!(operators[0]["level"], "Cualificado");

    // 1.3: the sprayer with its ROMA snapshot and live description/inspection.
    let machinery = doc["machinery"].as_array().unwrap();
    assert_eq!(machinery.len(), 1);
    assert_eq!(machinery[0]["order"], "1");
    assert_eq!(machinery[0]["description"], "Atomizador");
    assert_eq!(machinery[0]["roma"], "RM-47-0042");
    assert_eq!(machinery[0]["last_inspection"], "10/02/2026");

    // 3.1: chronological, order-number cross-references, "Manual" sentinel.
    let treatments = doc["treatments"].as_array().unwrap();
    assert_eq!(treatments.len(), 2);
    assert_eq!(treatments[0]["date"], "01/05/2026");
    assert_eq!(treatments[0]["plots"], "1");
    assert_eq!(treatments[0]["operator"], "1");
    assert_eq!(treatments[0]["equipment"], "Manual");
    assert_eq!(treatments[1]["date"], "20/05/2026");
    assert_eq!(treatments[1]["equipment"], "1");
}

#[test]
fn register_rows_format_dose_phi_and_efficacy_in_spanish() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["treatments"][0];
    assert_eq!(row["species"], "wheat");
    assert_eq!(row["variety"], "Craklin");
    assert_eq!(row["surface"], "2,5");
    assert_eq!(row["product"], "Fungitop");
    assert_eq!(row["reg_no"], "ES-25.123");
    assert_eq!(row["dose"], "1,5 L/ha");
    // PHI: days actually used + first day harvest is allowed again
    // (application 01/05 + 21 days → 22/05), the RD 1311/2012 pair the
    // model's 3.1 lacks a column for.
    assert_eq!(row["phi"], "21 días (hasta 22/05/2026)");
    assert_eq!(row["efficacy"], "Buena");
}

#[test]
fn multi_crop_treatments_print_one_register_row_per_crop_group() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0),
            on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0),
        ],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let treatments = doc["treatments"].as_array().unwrap();
    // Same split as the SIEX export: one row per crop snapshot group,
    // surface summed within the group only.
    assert_eq!(treatments.len(), 2);
    let barley = treatments
        .iter()
        .find(|r| r["species"] == "barley")
        .unwrap();
    let wheat = treatments.iter().find(|r| r["species"] == "wheat").unwrap();
    assert_eq!(barley["plots"], "2");
    assert_eq!(barley["surface"], "3");
    assert_eq!(wheat["plots"], "1");
    assert_eq!(wheat["surface"], "4");
    // Both rows come from the same record: shared date and product.
    assert_eq!(barley["date"], wheat["date"]);
}

#[test]
fn problem_codes_resolve_to_catalogue_labels_or_print_verbatim() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    // Seed the one catalogue row the fixture's problem uses; the second
    // problem's code stays unresolvable on purpose.
    conn.execute_batch(
        "INSERT INTO catalogue (id, source, source_updated_at, imported_at)
         VALUES ('ENFERMEDADES', 'siex', NULL, '2026-07-16T00:00:00Z');
         INSERT INTO catalogue_code (catalogue_id, code, label)
         VALUES ('ENFERMEDADES', '254', 'MILDIU');",
    )
    .unwrap();

    let mut record = treatment(&fx, "2026-05-01");
    record.problems.push(NewTreatmentProblem {
        reason_category_code: "pest".into(),
        problem_code: "135".into(),
    });
    repo::insert_treatment_record(
        &mut conn,
        record,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    // The catalogued code prints its official label; the other prints its
    // code — a printout never loses data over a missing display row.
    assert_eq!(doc["treatments"][0]["problems"], "MILDIU; 135");
}

#[test]
fn deleted_records_do_not_print() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let record = repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["treatments"].as_array().unwrap().len(), 0);
    // And the people/equipment tables list only what the printed register
    // references — nothing, here.
    assert_eq!(doc["operators"].as_array().unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// End to end: a real PDF
// ---------------------------------------------------------------------------

#[test]
fn renders_a_pdf_with_zero_template_warnings() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0),
            on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0),
        ],
        None,
    )
    .unwrap();

    let pdf = render_cuaderno(&conn, &fx.season_id, &fx.farm_id, GENERATED_ON).unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF-"), "output is not a PDF");
    // Sections 1, 2 and 3 each start on their own page.
    assert!(pdf.page_count >= 3, "got {} page(s)", pdf.page_count);
    assert_eq!(pdf.warnings, Vec::<String>::new());
}

#[test]
fn renders_even_an_empty_farm() {
    let mut conn = open_in_memory().unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Bare".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();

    // No plots, no records: every table prints its blank-form row and the
    // document still renders cleanly.
    let pdf = render_cuaderno(&conn, &season.id, &farm.id, GENERATED_ON).unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    assert_eq!(pdf.warnings, Vec::<String>::new());
}
