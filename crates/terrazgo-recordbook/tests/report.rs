// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Printable cuaderno (docs/siex-export.md arc, part 2): the data contract
//! `cuaderno_inputs` feeds the Typst template, pinned as JSON — order-number
//! cross-references (model tables 1.2/1.3/2.1 ↔ 3.1), Castilian formatting,
//! per-crop-group register rows (same split as the SIEX export) — plus the
//! end-to-end render (real PDF bytes, zero template warnings).
//!
//! Unless a test says otherwise it prints in Castilian: these assertions are
//! the regression guard that translating the book changed only its wording.
//! `language.rs` covers the Catalan output and the region rules.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::models::{FarmEsFields, NewZoneFlag, PlotEsFields};
use terrazgo_recordbook::open_in_memory;
use terrazgo_recordbook::{ReportLanguage, cuaderno_inputs, render_cuaderno};

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
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
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

fn treatment(fx: &Fixture, application_date: &str) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        application_date: application_date.into(),
        application_end_date: None,
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

fn on_plot(plot_id: &str, crop_id: Option<&str>, surface: f64) -> NewTreatmentPlot {
    NewTreatmentPlot {
        plot_id: plot_id.into(),
        crop_id: crop_id.map(Into::into),
        surface_treated_ha: surface,
        growth_stage_code: None,
    }
}

/// The same treated plot, at a stated growth stage (`EST_FENOLOGICO` code).
fn on_plot_at_stage(
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

fn inputs(conn: &Connection, fx: &Fixture) -> Value {
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

    let doc = cuaderno_inputs(
        &conn,
        &season.id,
        &farm.id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    // Blank strings, never null/absent: the template prints them as the
    // empty cells an official form leaves for hand-filling.
    assert_eq!(doc["farm"]["nif"], "");
    assert_eq!(doc["farm"]["rea"], "");
    assert_eq!(doc["farm"]["province"], "");
    assert_eq!(doc["plot_rows"].as_array().unwrap().len(), 0);
    assert_eq!(doc["treatments"].as_array().unwrap().len(), 0);
}

#[test]
fn plot_rows_number_the_plots_and_join_the_season_crops() {
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
            acquired_on: None,
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

    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF-"), "output is not a PDF");
    assert_eq!(pdf.warnings, Vec::<String>::new());
    // Exact, not a lower bound: every section opens with a `#pagebreak()`, so
    // the count is a structural fingerprint of the book. A table that silently
    // spilled onto another page, or a section that stopped starting its own,
    // shows up here — which nothing else in this suite can see, because the
    // layout lives inside the Typst template. Update the tally deliberately
    // when the book gains or loses a section.
    //
    //   1 información general · 2 parcelas (2.1 + 2.2) · 3 tratamientos (3.1)
    //   · 3.2–3.5 los registros condicionales · 4 análisis · 5 cosecha
    //   · 6 fertilización · 7.1 plan de abonado · 8 riego
    //   · documentación a conservar
    //
    // Eleven since 2026-08-08, the second decree having arrived in three
    // sections — 8 (art. 5.e), 6 (art. 5.d) and 7.1 (art. 4.2/5.a). Twelve
    // since 2026-08-09, when 3.1 bis (cultivos objeto de asesoramiento) got
    // its page. Moved deliberately: this number is the book's structural
    // fingerprint, so it must only change when a section does.
    assert_eq!(
        pdf.page_count, 12,
        "the book's page structure changed — check the render before updating this"
    );
}

/// Both branches of the template's `blank-rows-for` must survive rendering: a
/// register left untouched offers a hand-fillable form, and one declared
/// "APLICA TRATAMIENTO: NO" closes its table, because there the tick IS the
/// content and inviting empty lines would solicit a contradiction.
///
/// The row COUNT lives inside the Typst template, so what Rust pins here is
/// that the rule's input is right and that neither branch breaks the render;
/// the printed result is read in the rendered demo book.
#[test]
fn a_declared_register_and_an_untouched_one_both_render_cleanly() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Untouched: neither box ticked, so the table offers blank lines.
    let untouched = inputs(&conn, &fx);
    let seed = &untouched["seed_applies_no"];
    assert_eq!(seed, "", "nothing declared yet");
    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
    let untouched_pages = pdf.page_count;

    // Declared empty: the NO box is ticked, so the table closes.
    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "seed_treatment",
        "2026-05-01",
        None,
    )
    .unwrap();
    let declared = inputs(&conn, &fx);
    assert_eq!(declared["seed_applies_no"], "X", "the NO box is ticked");

    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
    assert_eq!(
        pdf.page_count, untouched_pages,
        "closing one register's blank lines must not reflow the book"
    );
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

    // No plots, no records: every table prints its blank form and the document
    // still renders cleanly.
    let pdf = render_cuaderno(
        &conn,
        &season.id,
        &farm.id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    assert_eq!(pdf.warnings, Vec::<String>::new());
    // A book with nothing in it is the same shape as a full one: the form
    // exists either way, and its blank rows must not push it onto extra pages.
    assert_eq!(
        pdf.page_count, 12,
        "an empty book must have the same page structure as a filled one"
    );
}

// ---------------------------------------------------------------------------
// Sections 2.1 / 2.2 — what a provider lookup contributes to the printed book
// ---------------------------------------------------------------------------

/// Store a SIGPAC-fetched boundary for a plot, the way `module-sigpac`'s
/// verify flow does: official surface in its own column, provider attributes
/// as source-tagged JSON.
fn save_sigpac_boundary(conn: &mut Connection, plot_id: &str, land_use: &str, area_ha: f64) {
    terrazgo_core::repository::save_geo_feature(
        conn,
        terrazgo_core::models::NewGeoFeature {
            plot_id: Some(plot_id.into()),
            farm_id: None,
            role: "boundary".into(),
            geometry: r#"{"type":"Polygon","coordinates":[[[-4.7,41.6],[-4.7,41.7],[-4.6,41.7],[-4.6,41.6],[-4.7,41.6]]]}"#.into(),
            source: "sigpac".into(),
            campaign: Some(2026),
            official_area_ha: Some(area_ha),
            properties: Some(format!(r#"{{"uso_sigpac":"{land_use}","superficie":{area_ha}}}"#)),
            fetched_at: Some("2026-07-20T10:00:00Z".into()),
        },
        None,
    )
    .unwrap();
}

fn water_point(
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

fn zone_flags(conn: &mut Connection, plot_id: &str, campaign: i64, flags: Vec<NewZoneFlag>) {
    terrazgo_core::repository::replace_zone_flags(conn, plot_id, campaign, "sigpac", flags, None)
        .unwrap();
}

/// Anexo III A.2.c–d: the provider's use code and official surface print
/// beside the farmer's own figure, never merged into it. An unverified plot
/// leaves both cells blank.
#[test]
fn plot_rows_carry_the_sigpac_use_code_and_official_surface() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    save_sigpac_boundary(&mut conn, &fx.wheat_plot_id, "TA", 4.1234);

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();

    assert_eq!(rows[0]["land_use"], "TA");
    assert_eq!(rows[0]["sigpac_area"], "4,1234", "Spanish decimal comma");
    // The user's own area is untouched by the provider figure.
    assert_eq!(rows[0]["area"], "4");

    // La Loma was never verified.
    assert_eq!(rows[1]["land_use"], "");
    assert_eq!(rows[1]["sigpac_area"], "");
}

/// "Superficie cultivada" is per crop row. Repeating the whole plot on every
/// row double-counts it, so the split prints blank until `crop.area_ha` exists
/// (docs/cuaderno-print.md → Capture design).
#[test]
fn a_plot_with_several_crops_leaves_the_cultivated_area_blank() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: fx.wheat_plot_id.clone(),
            season_id: fx.season_id.clone(),
            species_name: "veza".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    let wheat_plot: Vec<_> = rows.iter().filter(|r| r["order"] == "1").collect();
    assert_eq!(wheat_plot.len(), 2, "one row per crop on the plot");
    for row in &wheat_plot {
        assert_eq!(
            row["area"], "",
            "unknown share prints blank, not the plot area"
        );
    }

    // The single-crop plot still prints its area — that share IS known.
    let single: Vec<_> = rows.iter().filter(|r| r["order"] == "2").collect();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0]["area"], "3");
}

/// Zone flags store negatives, so an unaffected plot can print proof that the
/// check happened. A plot never checked prints blank — silence is a different
/// claim from "checked, nothing found".
#[test]
fn zone_rows_print_negatives_as_proof_of_check() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    zone_flags(
        &mut conn,
        &fx.wheat_plot_id,
        2026,
        vec![
            NewZoneFlag {
                zone_type_code: "nitrate_vulnerable".into(),
                status: "outside".into(),
                coverage_pct: None,
                detail: None,
            },
            NewZoneFlag {
                zone_type_code: "natura_2000".into(),
                status: "outside".into(),
                coverage_pct: None,
                detail: None,
            },
        ],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["zone_rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "one row per plot, checked or not");

    assert_eq!(rows[0]["order"], "1");
    assert_eq!(rows[0]["species"], "wheat");
    assert_eq!(rows[0]["fully"], "NO");
    assert_eq!(rows[0]["partly"], "NO");
    assert_eq!(rows[0]["checked"], "Sin afección — campaña 2026");

    assert_eq!(rows[1]["checked"], "", "never verified: blank, not a claim");
    assert_eq!(rows[1]["fully"], "");
    assert_eq!(rows[1]["partly"], "");
}

/// Total affection only when every affecting zone covers the whole plot; a
/// partial intersection — or an unknown percentage — is "parcialmente".
#[test]
fn zone_rows_separate_total_from_partial_affection() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    zone_flags(
        &mut conn,
        &fx.wheat_plot_id,
        2026,
        vec![NewZoneFlag {
            zone_type_code: "natura_2000".into(),
            status: "inside".into(),
            coverage_pct: Some(100.0),
            detail: None,
        }],
    );
    zone_flags(
        &mut conn,
        &fx.barley_plot_id,
        2026,
        vec![NewZoneFlag {
            zone_type_code: "phytosanitary_restriction".into(),
            status: "inside".into(),
            coverage_pct: Some(12.5),
            detail: None,
        }],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["zone_rows"].as_array().unwrap();

    assert_eq!(rows[0]["fully"], "SÍ");
    assert_eq!(rows[0]["partly"], "NO");
    assert_eq!(rows[0]["checked"], "Red Natura 2000 (100 %) — campaña 2026");

    assert_eq!(rows[1]["fully"], "NO");
    assert_eq!(rows[1]["partly"], "SÍ");
    assert_eq!(
        rows[1]["checked"],
        "Restricción fitosanitaria (12,5 %) — campaña 2026"
    );
}

/// Flags append across campaigns; the book shows the latest answer per zone,
/// the same candidate rule the alert engine uses.
#[test]
fn zone_rows_report_the_latest_campaign_checked() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    zone_flags(
        &mut conn,
        &fx.wheat_plot_id,
        2025,
        vec![NewZoneFlag {
            zone_type_code: "natura_2000".into(),
            status: "inside".into(),
            coverage_pct: Some(80.0),
            detail: None,
        }],
    );
    zone_flags(
        &mut conn,
        &fx.wheat_plot_id,
        2026,
        vec![NewZoneFlag {
            zone_type_code: "natura_2000".into(),
            status: "outside".into(),
            coverage_pct: None,
            detail: None,
        }],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["zone_rows"].as_array().unwrap();
    assert_eq!(rows[0]["checked"], "Sin afección — campaña 2026");
    assert_eq!(rows[0]["partly"], "NO", "the 2025 'inside' no longer rules");
}

/// Section 2.2's water half prints in the same three states as its zones half,
/// and for the same reason: a stated negative and silence are different claims.
#[test]
fn water_rows_distinguish_points_a_declared_negative_and_silence() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    water_point(&mut conn, &fx.wheat_plot_id, "Pozo del norte", true, None);
    terrazgo_core::repository::set_water_declaration(
        &mut conn,
        &fx.barley_plot_id,
        "2026-05-12",
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["zone_rows"].as_array().unwrap();

    // A recorded point answers all four columns it can.
    assert_eq!(rows[0]["water_point"], "SÍ");
    assert_eq!(rows[0]["denomination"], "Pozo del norte");
    assert_eq!(rows[0]["distance"], "", "a point inside states no distance");

    // The declared negative states itself in the denomination cell alone:
    // "NO" in the first column would assert a point exists outside the plot.
    assert_eq!(rows[1]["denomination"], "Sin captaciones — 12/05/2026");
    assert_eq!(rows[1]["water_point"], "");
    assert_eq!(rows[1]["distance"], "");
}

/// A plot nobody has looked at leaves all four cells blank — hand-fillable,
/// and never mistakable for "checked, and there are none".
#[test]
fn an_unasked_plot_leaves_the_water_cells_blank() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    let rows = doc["zone_rows"].as_array().unwrap();
    for row in rows {
        for key in ["water_point", "distance", "coordinates", "denomination"] {
            assert_eq!(row[key], "", "{key} should be blank on an unasked plot");
        }
    }
}

/// Several points on one plot join positionally, so the four columns can be
/// read across as one point per position — blanks are KEPT for that reason,
/// unlike the species join, which drops them.
#[test]
fn several_points_on_one_plot_join_positionally() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    water_point(&mut conn, &fx.wheat_plot_id, "Pozo de la casa", true, None);
    water_point(
        &mut conn,
        &fx.wheat_plot_id,
        "Sondeo municipal",
        false,
        Some(240.0),
    );

    let doc = inputs(&conn, &fx);
    let row = &doc["zone_rows"].as_array().unwrap()[0];
    assert_eq!(row["water_point"], "SÍ; NO");
    assert_eq!(row["denomination"], "Pozo de la casa; Sondeo municipal");
    // The inside point holds its position so the distance lines up under the
    // "NO" it belongs to — printed as a dash rather than as nothing, because
    // a cell reading "; 240" looks like a stray separator instead of a
    // statement that the first point has no distance (2026-08-10).
    assert_eq!(row["distance"], "—; 240");
}

/// Voluntary, and printed as what we store: WGS84 lat/lon, five decimals
/// (about a metre) and " / " between them, because both numbers already carry
/// a decimal comma in every printed language.
#[test]
fn water_coordinates_print_as_the_stored_lat_lon_pair() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_water_point(
        &mut conn,
        terrazgo_core::models::NewWaterPoint {
            plot_id: fx.wheat_plot_id.clone(),
            denomination: "Pozo del norte".into(),
            inside_plot: true,
            distance_m: None,
            latitude: Some(41.652_34),
            longitude: Some(-4.728_91),
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(
        doc["zone_rows"].as_array().unwrap()[0]["coordinates"],
        "41,65234 / -4,72891"
    );
}

// ---------------------------------------------------------------------------
// The spreadsheet: one assembly, a second renderer
// ---------------------------------------------------------------------------

/// The workbook description for the fixture farm.
fn workbook(conn: &Connection, fx: &Fixture) -> terrazgo_report::Workbook {
    terrazgo_recordbook::cuaderno_workbook(
        conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap()
}

fn sheet<'a>(book: &'a terrazgo_report::Workbook, name: &str) -> &'a terrazgo_report::Sheet {
    book.sheets
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("workbook should contain sheet '{name}'"))
}

/// One tab per section of the official model, in model order — a reader moving
/// between the PDF and the workbook lands in the same place.
#[test]
fn the_workbook_carries_one_sheet_per_model_section() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let book = workbook(&conn, &fx);
    let names: Vec<&str> = book.sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "1.1 Explotación",
            "1.2 Personas",
            "1.3 Equipos",
            "1.4 Asesoramiento",
            "2.1 Parcelas",
            "2.2 Medioambiental",
            "2.2 Captaciones",
            "3.1 Tratamientos",
            "3.2 Semilla tratada",
            "3.3 Postcosecha",
            "3.4 Locales",
            "3.5 Transporte",
            "4 Análisis",
            "4 Suelo",
            "5 Cosecha",
            "6 Fertilización",
            // Not a section of the printed model: the material registry the
            // section-6 records point at, where Anexo III C.h's eight
            // agronomic values, the micronutrients and the sludge heavy metals
            // actually live. A register row has no room for them.
            "6 Materiales",
            "7.1 Plan de abonado",
            "8 Riego",
        ]
    );
    // Excel caps tab names at 31 characters; these are authored, not repaired.
    for name in names {
        assert!(name.chars().count() <= 31, "'{name}' is too long for a tab");
    }
}

/// The whole point of the spreadsheet: dates and numbers are VALUES, so the
/// farmer can sort by date and sum hectares. The PDF's Spanish display
/// strings ("01/05/2026", "2,5") must not leak into the cells.
#[test]
fn the_register_sheet_writes_dates_and_numbers_as_values() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    assert_eq!(register.rows.len(), 1);
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        row[index].clone()
    };

    // ISO dates, not dd/mm/yyyy strings. The interval's two ends are separate
    // columns so each one sorts and filters; a single-day treatment leaves the
    // end blank rather than repeating its start.
    assert_eq!(column("Fecha inicio"), Cell::Date("2026-05-01".into()));
    assert_eq!(column("Fecha fin"), Cell::Empty);
    // 21 PHI days from the product label → harvest allowed 22/05.
    assert_eq!(
        column("Cosecha permitida desde"),
        Cell::Date("2026-05-22".into())
    );
    // Real numbers, not "2,5".
    assert_eq!(column("Superficie tratada (ha)"), Cell::Number(2.5));
    assert_eq!(column("Dosis"), Cell::Number(1.5));
    assert_eq!(column("Plazo de seguridad (días)"), Cell::Number(21.0));
    // The unit travels in its own column, so the dose stays summable.
    assert_eq!(column("Unidad de dosis"), Cell::Text("L/ha".into()));
}

/// The joined plot-name cell is ordered by the book's LANGUAGE, not by bytes.
///
/// SQLite's default BINARY collation orders by code point, so `Á` (U+00C1)
/// sorted after `Z` (U+005A) and a plot called "Ángel" printed last; a byte
/// sort also reads "Parcela 10" before "Parcela 2". This pins the CALL SITE,
/// not just the comparator — collate.rs proves the ordering, this proves the
/// register actually uses it.
#[test]
fn joined_plot_names_are_ordered_by_language_not_by_bytes() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Four names whose byte order and collated order disagree in both ways.
    let angel = insert_plot(&mut conn, &fx.farm_id, "Ángel", 1.0, None);
    let ten = insert_plot(&mut conn, &fx.farm_id, "Parcela 10", 1.0, None);
    let two = insert_plot(&mut conn, &fx.farm_id, "Parcela 2", 1.0, None);
    let zubiri = insert_plot(&mut conn, &fx.farm_id, "Zubiri", 1.0, None);

    // No crop on any of them, so all four land in ONE register row and their
    // names are joined into a single cell.
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot(&zubiri, None, 1.0),
            on_plot(&ten, None, 1.0),
            on_plot(&angel, None, 1.0),
            on_plot(&two, None, 1.0),
        ],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = register
        .rows
        .iter()
        .find(|r| {
            let index = register
                .columns
                .iter()
                .position(|c| c.header == "Parcelas")
                .unwrap();
            matches!(&r[index], Cell::Text(text) if text.contains("Ángel"))
        })
        .expect("the four-plot row should be in the register");
    let index = register
        .columns
        .iter()
        .position(|c| c.header == "Parcelas")
        .unwrap();

    assert_eq!(
        row[index],
        Cell::Text("Ángel, Parcela 2, Parcela 10, Zubiri".into())
    );

    // And the byte ordering this replaced really would have been wrong, so the
    // assertion above is pinning a fix rather than restating a default.
    let mut bytes = ["Zubiri", "Parcela 10", "Ángel", "Parcela 2"];
    bytes.sort_unstable();
    assert_eq!(bytes, ["Parcela 10", "Parcela 2", "Zubiri", "Ángel"]);
}

/// The PDF cross-references plots, operators and equipment by order number.
/// The sheet keeps those numbers AND resolves the names, so it reconciles
/// with the printed book while staying filterable on its own.
#[test]
fn the_register_sheet_carries_both_order_numbers_and_names() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let machinery_id = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: None,
            acquired_on: None,
            last_inspection_date: Some("2026-02-10".into()),
            next_inspection_due_date: None,
            roma_number: Some("RM-47-0042".into()),
            reganip_number: None,
        },
        None,
    )
    .unwrap()
    .id;
    let mut with_machinery = treatment(&fx, "2026-05-01");
    with_machinery.machinery_id = Some(machinery_id);
    repo::insert_treatment_record(
        &mut conn,
        with_machinery,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.0)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap();
        row[index].clone()
    };

    assert_eq!(column("Id. parcelas"), Cell::Text("1".into()));
    assert_eq!(column("Parcelas"), Cell::Text("El Prado".into()));
    assert_eq!(column("Nº aplicador"), Cell::Number(1.0));
    assert_eq!(column("Aplicador"), Cell::Text("Carlos Pérez".into()));
    assert_eq!(column("Nº equipo"), Cell::Number(1.0));
    assert_eq!(column("Equipo"), Cell::Text("Atomizador".into()));
}

/// Manual application is a value the model defines (footnote 3), not missing
/// data: the equipment number is empty but the name says "Manual".
#[test]
fn manual_application_is_labelled_not_left_blank() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0)],
        None,
    )
    .unwrap();

    let register_book = workbook(&conn, &fx);
    let register = sheet(&register_book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap();
        row[index].clone()
    };
    assert_eq!(column("Nº equipo"), Cell::Empty);
    assert_eq!(column("Equipo"), Cell::Text("Manual".into()));
}

/// Unknown surfaces stay empty cells rather than becoming a zero — the same
/// rule the printed form follows, and a zero would be a false statement a
/// spreadsheet would happily add up.
#[test]
fn unknown_surfaces_are_empty_cells_never_zero() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: fx.wheat_plot_id.clone(),
            season_id: fx.season_id.clone(),
            species_name: "veza".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let plots = sheet(&book, "2.1 Parcelas");
    let cultivated = plots
        .columns
        .iter()
        .position(|c| c.header == "Superficie cultivada (ha)")
        .unwrap();
    let sigpac = plots
        .columns
        .iter()
        .position(|c| c.header == "Superficie SIGPAC (ha)")
        .unwrap();

    // Two crops on El Prado: the split is unknown, so both rows are blank.
    let shared: Vec<_> = plots
        .rows
        .iter()
        .filter(|r| r[0] == Cell::Number(1.0))
        .collect();
    assert_eq!(shared.len(), 2);
    for row in shared {
        assert_eq!(row[cultivated], Cell::Empty);
    }
    // The single-crop plot keeps its real number; SIGPAC was never fetched.
    let single: Vec<_> = plots
        .rows
        .iter()
        .filter(|r| r[0] == Cell::Number(2.0))
        .collect();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0][cultivated], Cell::Number(3.0));
    assert_eq!(single[0][sigpac], Cell::Empty);
}

/// A farm with nothing recorded still produces the full set of tabs: the form
/// exists, it simply has no rows yet.
#[test]
fn an_empty_farm_still_renders_every_sheet() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let book = workbook(&conn, &fx);
    assert_eq!(book.sheets.len(), 19);
    assert!(sheet(&book, "3.1 Tratamientos").rows.is_empty());
    assert!(sheet(&book, "8 Riego").rows.is_empty());
    assert!(sheet(&book, "2.2 Captaciones").rows.is_empty());
    assert!(sheet(&book, "1.2 Personas").rows.is_empty());
    assert!(sheet(&book, "1.4 Asesoramiento").rows.is_empty());
    // 1.1 is a label/value block, always populated.
    assert!(!sheet(&book, "1.1 Explotación").rows.is_empty());

    let rendered = terrazgo_recordbook::render_cuaderno_xlsx(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .expect("an empty farm must still export");
    assert_eq!(rendered.sheet_count, 19);
    assert_eq!(&rendered.bytes[..2], b"PK");
}

// ---------------------------------------------------------------------------
// Slice 5: the fields the model asked for and the book could not yet print
// ---------------------------------------------------------------------------

/// Model 2.1 footnotes 4 and 5: the siglas are Spanish form vocabulary, so the
/// assembly resolves the stored English codes once and both outputs print the
/// same letters. Neither is a boolean — Anexo III A.2.e asks for the irrigation
/// *system*, not just "is it irrigated".
#[test]
fn plot_rows_print_the_irrigation_and_shelter_abbreviations() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::update_crop(
        &mut conn,
        &fx.wheat_crop_id,
        terrazgo_core::models::UpdateCrop {
            species_name: "wheat".into(),
            variety: Some("Craklin".into()),
            production_system_code: Some("organic".into()),
            area_ha: Some(2.5),
            irrigation_code: Some("drip".into()),
            growing_environment_code: Some("greenhouse".into()),
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    assert_eq!(rows[0]["irrigation"], "LOC", "goteo o localizado");
    assert_eq!(rows[0]["environment"], "INV", "invernadero");
    // The crop's own surface now wins over the plot's.
    assert_eq!(rows[0]["area"], "2,5");

    // Untouched crop: no codes stored, both cells blank.
    assert_eq!(rows[1]["irrigation"], "");
    assert_eq!(rows[1]["environment"], "");
}

/// A crop that states its own surface is believed even when it shares the plot
/// — the blank of slice 3 was "unknown", not "unknowable".
#[test]
fn a_stated_crop_surface_prints_even_on_a_shared_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: fx.wheat_plot_id.clone(),
            season_id: fx.season_id.clone(),
            species_name: "veza".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(1.5),
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    let shared: Vec<_> = rows.iter().filter(|r| r["order"] == "1").collect();
    assert_eq!(shared.len(), 2);
    let veza = shared.iter().find(|r| r["species"] == "veza").unwrap();
    let wheat = shared.iter().find(|r| r["species"] == "wheat").unwrap();
    assert_eq!(veza["area"], "1,5", "stated share prints");
    assert_eq!(
        wheat["area"], "",
        "the crop that states nothing stays blank"
    );
}

/// Model 1.1 in full, plus the "titular o representante" block. An absent
/// representative still renders its rows — the form stays hand-fillable.
#[test]
fn farm_block_carries_contact_details_and_both_registry_numbers() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::update_farm(
        &mut conn,
        &fx.farm_id,
        terrazgo_core::models::UpdateFarm {
            name: "Finca La Vega".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            location_text: Some("Medina del Campo".into()),
            address: Some("Camino de la Vega, 4".into()),
            postal_code: Some("47400".into()),
            phone_fixed: Some("983000000".into()),
            phone_mobile: Some("600000000".into()),
            email: Some("maria@example.es".into()),
            opened_on: None,
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("ES244700000123".into()),
                siex_code: Some("ES470000000999".into()),
                province_code: Some("47".into()),
            }),
            representative: Some(terrazgo_core::models::FarmRepresentativeFields {
                full_name: "Ana Ruiz".into(),
                tax_id: Some("87654321X".into()),
                representation_kind: Some("Administradora única".into()),
                address: None,
                locality: None,
                province: None,
                postal_code: None,
                phone: None,
                email: None,
            }),
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["farm"]["address"], "Camino de la Vega, 4");
    assert_eq!(doc["farm"]["postal_code"], "47400");
    assert_eq!(doc["farm"]["phone_mobile"], "600000000");
    assert_eq!(doc["farm"]["email"], "maria@example.es");
    // National and autonómico registry numbers print side by side.
    assert_eq!(doc["farm"]["siex"], "ES470000000999");
    assert_eq!(doc["farm"]["rea"], "ES244700000123");
    assert_eq!(doc["representative"]["name"], "Ana Ruiz");
    assert_eq!(doc["representative"]["kind"], "Administradora única");
}

#[test]
fn a_farm_without_a_representative_still_renders_the_block() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let doc = inputs(&conn, &fx);
    assert_eq!(doc["representative"]["name"], "");
    assert_eq!(doc["representative"]["nif"], "");
}

/// 1.2's NIF column and 1.3's acquisition date — the two cells that printed
/// permanently empty before this slice (Anexo III A.1.c and A.1.h).
#[test]
fn operator_nif_and_machinery_acquisition_date_reach_the_book() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::update_operator(
        &mut conn,
        &fx.operator_id,
        terrazgo_core::models::UpdateOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: Some("11111111H".into()),
            licence_number: Some("ROPO-4700123".into()),
            licence_level_code: Some("pilot".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();
    let machinery_id = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: None,
            acquired_on: Some("2018-03-15".into()),
            last_inspection_date: Some("2026-02-10".into()),
            next_inspection_due_date: None,
            roma_number: Some("RM-47-0042".into()),
            reganip_number: None,
        },
        None,
    )
    .unwrap()
    .id;
    let mut record = treatment(&fx, "2026-05-01");
    record.machinery_id = Some(machinery_id);
    repo::insert_treatment_record(
        &mut conn,
        record,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let operator = &doc["operators"][0];
    assert_eq!(operator["nif"], "11111111H");
    assert_eq!(
        operator["level"], "Piloto",
        "the aerial carné the model prints"
    );

    let machine = &doc["machinery"][0];
    assert_eq!(machine["acquired_on"], "15/03/2018");
    assert_eq!(machine["last_inspection"], "10/02/2026");
}

// ---------------------------------------------------------------------------
// Section 1.4 — the advisory relationship, and what it makes 1.2 and 2.1 print
// ---------------------------------------------------------------------------

fn advisor(conn: &mut Connection, name: &str, tax_id: Option<&str>) -> String {
    terrazgo_core::repository::insert_advisor(
        conn,
        terrazgo_core::models::NewAdvisor {
            name: name.into(),
            tax_id: tax_id.map(Into::into),
            registration_number: Some("ROPO-AS-47-0912".into()),
        },
        None,
    )
    .unwrap()
    .id
}

/// Table 1.4 prints the holding's advisory relationships with the framework
/// each runs under (Anexo III A.1.d).
#[test]
fn advisory_links_print_in_table_1_4() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Before any link the table is empty — the template still prints it as a
    // blank fillable form, which is what "no advisor" looks like on paper.
    assert!(
        inputs(&conn, &fx)["advisors"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let advisor_id = advisor(&mut conn, "ATRIA Cerealista", Some("G47654321"));
    terrazgo_core::repository::set_farm_advisor(
        &mut conn,
        &fx.farm_id,
        &advisor_id,
        Some("atria".into()),
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["advisors"][0];
    assert_eq!(row["name"], "ATRIA Cerealista");
    assert_eq!(row["nif"], "G47654321");
    assert_eq!(row["registration_number"], "ROPO-AS-47-0912");
    assert_eq!(row["gip"], "Atrias", "the model's sigla, not the code");
}

/// A link that belongs to another farm never reaches this book.
#[test]
fn table_1_4_is_scoped_to_the_farm_being_printed() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca vecina".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    let advisor_id = advisor(&mut conn, "Asesoría vecina", Some("G47000111"));
    terrazgo_core::repository::set_farm_advisor(
        &mut conn,
        &other_farm.id,
        &advisor_id,
        Some("advisor_assisted".into()),
        None,
    )
    .unwrap();

    assert!(
        inputs(&conn, &fx)["advisors"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// 1.2's "Asesor" column is a separate cross, not a carné level: it marks the
/// people who are also registered as advisors, matched by NIF.
#[test]
fn the_advisor_cross_marks_operators_registered_as_advisors() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    // Same person, written with the separators a second form might carry.
    advisor(&mut conn, "Carlos Pérez", Some("11.111.111-h"));
    terrazgo_core::repository::update_operator(
        &mut conn,
        &fx.operator_id,
        terrazgo_core::models::UpdateOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: Some("11111111H".into()),
            licence_number: Some("ROPO-4700123".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();
    let second = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Lucía Ruiz".into(),
            tax_id: Some("22222222J".into()),
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();

    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.0)],
        None,
    )
    .unwrap();
    let mut by_second = treatment(&fx, "2026-05-08");
    by_second.operator_id = second.id.clone();
    repo::insert_treatment_record(
        &mut conn,
        by_second,
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["operators"][0]["advisor"], "X");
    assert_eq!(doc["operators"][1]["advisor"], "");
}

/// An operator with no NIF must not be crossed just because some advisor row
/// has none either — two blanks are not a match.
#[test]
fn the_advisor_cross_never_matches_two_missing_tax_ids() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    advisor(&mut conn, "Asesoría sin NIF", None);
    terrazgo_core::repository::update_operator(
        &mut conn,
        &fx.operator_id,
        terrazgo_core::models::UpdateOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["operators"][0]["advisor"], "");
}

/// 2.1's GIP column comes from the crop when it states a framework, and falls
/// back to what the production system implies when it does not.
#[test]
fn plot_rows_print_the_crops_gip_framework() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::update_crop(
        &mut conn,
        &fx.wheat_crop_id,
        terrazgo_core::models::UpdateCrop {
            species_name: "wheat".into(),
            variety: None,
            // Organic would imply AE; the stated framework wins.
            production_system_code: Some("organic".into()),
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: Some("private_certification".into()),
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    terrazgo_core::repository::update_crop(
        &mut conn,
        &fx.barley_crop_id,
        terrazgo_core::models::UpdateCrop {
            species_name: "barley".into(),
            variety: None,
            production_system_code: Some("integrated".into()),
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    assert_eq!(rows[0]["gip"], "CP", "stated framework wins over AE");
    assert_eq!(rows[1]["gip"], "PI", "derived from producción integrada");
}

/// The workbook's advisory tab, and 1.2's cross as a filterable SÍ/NO.
#[test]
fn the_advisory_sheet_carries_the_framework_code_beside_its_abbreviation() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let advisor_id = advisor(&mut conn, "ATRIA Cerealista", Some("G47654321"));
    terrazgo_core::repository::set_farm_advisor(
        &mut conn,
        &fx.farm_id,
        &advisor_id,
        Some("atria".into()),
        None,
    )
    .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.0)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let advisors = sheet(&book, "1.4 Asesoramiento");
    assert_eq!(
        advisors.rows[0],
        vec![
            Cell::Text("ATRIA Cerealista".into()),
            Cell::Text("G47654321".into()),
            Cell::Text("ROPO-AS-47-0912".into()),
            Cell::Text("Atrias".into()),
            Cell::Text("atria".into()),
        ]
    );

    // The cross is a paper convention; a filterable column needs both answers.
    let people = sheet(&book, "1.2 Personas");
    let index = people
        .columns
        .iter()
        .position(|c| c.header == "Asesor")
        .unwrap();
    assert_eq!(people.rows[0][index], Cell::Text("NO".into()));
}

// ---------------------------------------------------------------------------
// 3.1 — the actuation interval and the total used (Anexo III Parte I B)
// ---------------------------------------------------------------------------

/// The model's 3.1 date column is an "intervalo de fechas". A treatment that
/// ran over several days prints both ends; the PHI phrase counts from the last
/// one, so the printed book and the stored derivation agree.
#[test]
fn the_register_prints_a_date_interval_and_counts_the_phi_from_its_end() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_end_date = Some("2026-05-03".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["treatments"][0];
    assert_eq!(row["date"], "01/05/2026 – 03/05/2026");
    // 21 days from the LAST application (03/05), not the first.
    assert_eq!(row["phi"], "21 días (hasta 24/05/2026)");
}

/// A single day stays a single date — the register must not invent a range.
#[test]
fn a_single_day_treatment_prints_one_date() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    assert_eq!(inputs(&conn, &fx)["treatments"][0]["date"], "01/05/2026");
}

/// An interval whose end is its start is the same statement as a single day,
/// so the book prints one date rather than "01/05/2026 – 01/05/2026".
#[test]
fn an_interval_ending_on_its_start_day_prints_as_one_date() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_end_date = Some("2026-05-01".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    assert_eq!(inputs(&conn, &fx)["treatments"][0]["date"], "01/05/2026");
}

/// Anexo III B.i: the total product used. It prints with its unit symbol, and
/// an unstated total prints BLANK — the official form leaves the cell for hand
/// filling, and a zero would be a statement the farmer never made.
#[test]
fn the_register_prints_the_total_quantity_used_and_blanks_it_when_unstated() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut stated = treatment(&fx, "2026-05-01");
    stated.total_quantity_value = Some(9.0);
    stated.total_quantity_unit_code = Some("l".into());
    repo::insert_treatment_record(
        &mut conn,
        stated,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-04-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["treatments"].as_array().unwrap();
    // The register reads chronologically: April first, then May.
    assert_eq!(rows[0]["date"], "01/04/2026");
    assert_eq!(rows[0]["total_quantity"], "");
    assert_eq!(rows[1]["date"], "01/05/2026");
    assert_eq!(rows[1]["total_quantity"], "9 L");
}

/// The sheet's job is arithmetic: the interval's ends are real dates in their
/// own columns, and the total is a number apart from its unit so a season's
/// product use can be summed.
#[test]
fn the_register_sheet_splits_the_interval_and_the_total_into_typed_columns() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_end_date = Some("2026-05-03".into());
    new.total_quantity_value = Some(9.0);
    new.total_quantity_unit_code = Some("l".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        row[index].clone()
    };

    assert_eq!(column("Fecha inicio"), Cell::Date("2026-05-01".into()));
    assert_eq!(column("Fecha fin"), Cell::Date("2026-05-03".into()));
    assert_eq!(column("Cantidad total"), Cell::Number(9.0));
    assert_eq!(column("Unidad de cantidad"), Cell::Text("L".into()));
    // Derived from the interval's end, like the PDF.
    assert_eq!(
        column("Cosecha permitida desde"),
        Cell::Date("2026-05-24".into())
    );
}

/// An unstated total leaves both cells empty. A spreadsheet adds zeros up, so
/// writing one would turn "not measured" into "none used".
#[test]
fn an_unstated_total_is_empty_in_the_sheet_never_zero() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap();
        row[index].clone()
    };

    assert_eq!(column("Cantidad total"), Cell::Empty);
    assert_eq!(column("Unidad de cantidad"), Cell::Empty);
}

// ---------------------------------------------------------------------------
// Sections 3.3 / 3.4 / 3.5 — the non-field registers
// ---------------------------------------------------------------------------

fn non_field(fx: &Fixture, kind: &str, subject: &str) -> NewNonFieldTreatment {
    NewNonFieldTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        country_code: None,
        subject_kind_code: kind.into(),
        treated_on: "2026-08-20".into(),
        subject_description: subject.into(),
        subject_product_code: None,
        treated_quantity_value: None,
        treated_quantity_unit_code: None,
        product_id: fx.product_id.clone(),
        product_quantity_value: None,
        product_quantity_unit_code: None,
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        notes: None,
    }
}

/// The three registers arrive in model order and each carries only its own
/// rows, so one assembly feeds three printed tables.
#[test]
fn the_three_non_field_registers_print_in_model_order_with_their_own_rows() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut grain = non_field(&fx, "postharvest", "Trigo blando cosecha 2026");
    grain.treated_quantity_value = Some(120.0);
    grain.treated_quantity_unit_code = Some("t".into());
    grain.product_quantity_value = Some(3.0);
    grain.product_quantity_unit_code = Some("kg".into());
    repo::insert_non_field_treatment(&mut conn, grain, None).unwrap();
    repo::insert_non_field_treatment(
        &mut conn,
        non_field(&fx, "transport", "Camión rígido, Iveco, 1234 ABC"),
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let registers = doc["non_field"].as_array().unwrap();
    assert_eq!(registers.len(), 3);
    assert_eq!(registers[0]["kind"], "postharvest");
    assert_eq!(registers[1]["kind"], "storage_premises");
    assert_eq!(registers[2]["kind"], "transport");

    let grain_rows = registers[0]["rows"].as_array().unwrap();
    assert_eq!(grain_rows.len(), 1);
    assert_eq!(grain_rows[0]["subject"], "Trigo blando cosecha 2026");
    assert_eq!(grain_rows[0]["date"], "20/08/2026");
    // Produce is measured in tonnes, the product used in kilograms.
    assert_eq!(grain_rows[0]["quantity"], "120 t");
    assert_eq!(grain_rows[0]["product_quantity"], "3 kg");

    // The register nobody used stays empty rather than borrowing another's rows.
    assert!(registers[1]["rows"].as_array().unwrap().is_empty());
    assert_eq!(registers[2]["rows"].as_array().unwrap().len(), 1);
}

/// The model heads each conditional register with two boxes. Rows tick SÍ, a
/// stored declaration ticks NO, and an untouched register ticks neither —
/// three states, because "nobody filled this in" is not the same claim as
/// "nothing happened".
#[test]
fn the_applies_boxes_distinguish_rows_from_a_declaration_from_silence() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(
        &mut conn,
        non_field(&fx, "postharvest", "Trigo blando"),
        None,
    )
    .unwrap();
    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "storage_premises",
        "2026-09-01",
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let registers = doc["non_field"].as_array().unwrap();

    // Rows exist → SÍ.
    assert_eq!(registers[0]["applies_yes"], "X");
    assert_eq!(registers[0]["applies_no"], "");
    // Declared empty → NO.
    assert_eq!(registers[1]["applies_yes"], "");
    assert_eq!(registers[1]["applies_no"], "X");
    // Never touched → neither box.
    assert_eq!(registers[2]["applies_yes"], "");
    assert_eq!(registers[2]["applies_no"], "");
}

/// An unstated quantity prints blank in the PDF and stays empty in the sheet.
/// The official form leaves the cell for hand-filling, and a spreadsheet would
/// add a zero up.
#[test]
fn unstated_non_field_quantities_print_blank_and_never_zero() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(
        &mut conn,
        non_field(&fx, "storage_premises", "Almacén, Ctra. de Soria km 4"),
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["non_field"][1]["rows"][0];
    assert_eq!(row["quantity"], "");
    assert_eq!(row["product_quantity"], "");

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "3.4 Locales");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };
    assert_eq!(column("Volumen (m³)"), Cell::Empty);
    assert_eq!(column("Cantidad utilizada"), Cell::Empty);
    // The subject and the answer are always there.
    assert_eq!(
        column("Local tratado (tipo y dirección)"),
        Cell::Text("Almacén, Ctra. de Soria km 4".into())
    );
    assert_eq!(column("Aplica tratamiento"), Cell::Text("SÍ".into()));
}

/// A register declared empty has no rows to hang its answer on, so the sheet
/// writes the declaration as a row of its own — that statement IS the content,
/// and a reader filtering the tab must be able to see it.
#[test]
fn a_register_declared_empty_still_says_so_in_the_sheet() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "transport",
        "2026-09-01",
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let declared = sheet(&book, "3.5 Transporte");
    assert_eq!(declared.rows.len(), 1);
    assert_eq!(declared.rows[0][0], Cell::Text("NO".into()));
    assert!(declared.rows[0][1..].iter().all(|c| *c == Cell::Empty));

    // An untouched register writes nothing at all — silence, not a "no".
    assert!(sheet(&book, "3.3 Postcosecha").rows.is_empty());
}

/// Typed cells in the register tabs, the same rule section 3.1 follows: real
/// dates, real numbers, the unit in its own column so amounts stay summable.
#[test]
fn the_non_field_sheet_writes_dates_and_numbers_as_values() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut grain = non_field(&fx, "postharvest", "Trigo blando");
    grain.treated_quantity_value = Some(120.0);
    grain.treated_quantity_unit_code = Some("t".into());
    grain.product_quantity_value = Some(3.0);
    grain.product_quantity_unit_code = Some("kg".into());
    repo::insert_non_field_treatment(&mut conn, grain, None).unwrap();

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "3.3 Postcosecha");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };
    assert_eq!(column("Fecha"), Cell::Date("2026-08-20".into()));
    assert_eq!(column("Cantidad (t)"), Cell::Number(120.0));
    assert_eq!(column("Unidad"), Cell::Text("t".into()));
    assert_eq!(column("Cantidad utilizada"), Cell::Number(3.0));
    assert_eq!(column("Unidad utilizada"), Cell::Text("kg".into()));
    assert_eq!(column("Aplicador"), Cell::Text("Carlos Pérez".into()));
}

// ---------------------------------------------------------------------------
// Section 3.2 — uso de semilla tratada
// ---------------------------------------------------------------------------

fn seed_treatment(fx: &Fixture) -> NewSeedTreatment {
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

/// The register prints the sowing, cross-referencing plots by the order numbers
/// of table 2.1 — the model's "Id. parcelas", the same convention 3.1 uses.
#[test]
fn the_seed_register_prints_the_sowing_and_cross_references_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["seed"][0];
    // "El Prado" is order 1 in table 2.1.
    assert_eq!(row["plots"], "1");
    assert_eq!(row["date"], "10/11/2025");
    assert_eq!(row["species"], "trigo blando");
    assert_eq!(row["variety"], "Nogal");
    assert_eq!(row["surface"], "3,2");
    assert_eq!(row["seed_quantity"], "680");
    assert_eq!(row["seed_lot"], "L-2025-4471");
    // The model prints no column for where the seed was treated, so the kind
    // rides in the product cell rather than being dropped.
    assert_eq!(row["product"], "Celest Trio · Adquirida tratada en España");
    assert_eq!(row["reg_no"], "ES-24.876");
    assert_eq!(row["active_substance"], "fludioxonil + difenoconazol");
    assert_eq!(row["efficacy"], "Buena");
    // A sowing ticks the register's SÍ box.
    assert_eq!(doc["seed_applies_yes"], "X");
    assert_eq!(doc["seed_applies_no"], "");
}

/// The seed register answers the same three ways every conditional register
/// does; it just happens to be backed by a different table.
#[test]
fn the_seed_register_answers_no_only_when_declared_and_blank_otherwise() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Untouched: neither box.
    let doc = inputs(&conn, &fx);
    assert_eq!(doc["seed_applies_yes"], "");
    assert_eq!(doc["seed_applies_no"], "");

    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "seed_treatment",
        "2026-09-01",
        None,
    )
    .unwrap();
    let doc = inputs(&conn, &fx);
    assert_eq!(doc["seed_applies_yes"], "");
    assert_eq!(doc["seed_applies_no"], "X");
}

/// An unstated seed quantity prints blank and stays empty in the sheet — never
/// a zero, which a spreadsheet would happily total.
#[test]
fn an_unstated_seed_quantity_prints_blank_and_is_empty_in_the_sheet() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = seed_treatment(&fx);
    new.seed_quantity_kg = None;
    new.seed_lot = None;
    repo::insert_seed_treatment(&mut conn, new, None).unwrap();

    assert_eq!(inputs(&conn, &fx)["seed"][0]["seed_quantity"], "");

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "3.2 Semilla tratada");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };
    assert_eq!(column("Cantidad de semilla (kg)"), Cell::Empty);
    assert_eq!(column("Nº de lote"), Cell::Empty);
}

/// Typed cells, the rule every register follows: real dates and real numbers,
/// plus the resolved plot names beside the order numbers the PDF prints.
#[test]
fn the_seed_sheet_writes_dates_and_numbers_as_values() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "3.2 Semilla tratada");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };
    assert_eq!(column("Aplica tratamiento"), Cell::Text("SÍ".into()));
    assert_eq!(column("Fecha de siembra"), Cell::Date("2025-11-10".into()));
    assert_eq!(column("Superficie sembrada (ha)"), Cell::Number(3.2));
    assert_eq!(column("Cantidad de semilla (kg)"), Cell::Number(680.0));
    assert_eq!(column("Id. parcelas"), Cell::Text("1".into()));
    assert_eq!(column("Parcelas"), Cell::Text("El Prado".into()));
    assert_eq!(column("Nº de lote"), Cell::Text("L-2025-4471".into()));
}

/// A sowing spanning two plots sums their surfaces into the one printed figure
/// and lists both order numbers, ascending.
#[test]
fn a_sowing_over_two_plots_sums_its_surface_and_lists_both_orders() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = seed_treatment(&fx);
    new.plots.push(NewSeedTreatmentPlot {
        plot_id: fx.barley_plot_id.clone(),
        surface_sown_ha: 2.5,
    });
    repo::insert_seed_treatment(&mut conn, new, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["seed"][0]["plots"], "1, 2");
    assert_eq!(doc["seed"][0]["surface"], "5,7");
}

// ---------------------------------------------------------------------------
// Sections 4 and 5 — análisis and cosecha comercializada
//
// Two model-recommended registers (art. 16.3's conservation duty and food-chain
// traceability), so neither carries an "APLICA TRATAMIENTO" line: they print
// their rows or an empty hand-fillable table, never a SÍ/NO answer.
// ---------------------------------------------------------------------------

fn analysis(fx: &Fixture) -> NewAnalysisRecord {
    NewAnalysisRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sampled_on: "2026-06-18".into(),
        material_kind_code: "harvested_produce".into(),
        bulletin_number: Some("B-2026/1187".into()),
        lab_name: Some("Laboratorio Agroalimentario".into()),
        lab_address: Some("Ctra. Burgos km 118, Valladolid".into()),
        lab_tax_id: Some("Q4700123B".into()),
        substances_detected: Some("Lambda cihalotrín 0,01 mg/kg".into()),
        soil: Default::default(),
        notes: None,
        plots: vec![NewAnalysisPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
        }],
        analysis_type_codes: vec!["pesticide_residues".into()],
        // 170 is LAMBDA CIHALOTRINA in the FEGA SUST_ACTIVAS catalogue.
        substance_codes: vec!["170".into()],
    }
}

fn harvest(fx: &Fixture) -> terrazgo_core::models::NewHarvestRecord {
    terrazgo_core::models::NewHarvestRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        harvested_on: "2026-07-24".into(),
        product_name: "Trigo blando".into(),
        plant_product_code: Some("85".into()), // PROD_VEGETAL "Granos de trigo"
        quantity_value: Some(42.5),
        quantity_unit_code: Some("t".into()),
        delivery_note_ref: Some("ALB-2026/318".into()),
        lot_number: Some("L-26-07".into()),
        buyer_name: "Cooperativa Cerealista del Duero".into(),
        buyer_tax_id: Some("F47008123".into()),
        buyer_address: Some("Ctra. Palencia km 4, Valladolid".into()),
        buyer_registry_number: Some("21.0012345/VA".into()),
        notes: None,
        plots: vec![terrazgo_core::models::NewHarvestPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
        }],
    }
}

/// Section 4 prints the bulletin's whereabouts, not the bulletin: the model's
/// "Laboratorio (nombre y dirección)" is one cell, so the three stored fields
/// are joined for the PDF — and only the ones the farmer filled in.
#[test]
fn the_analysis_register_prints_where_the_bulletin_can_be_found() {
    let mut conn = open_in_memory().unwrap();
    // With the catalogue snapshot imported, as a running app always has it:
    // the coded substance is stored as a number and printed as a name.
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    repo::insert_analysis_record(&mut conn, analysis(&fx), None).unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["analysis"].as_array().expect("an analysis array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["date"], "18/06/2026");
    // FEGA's own wording, and the kinds of analysis folded in behind it — the
    // model has a "Material analizado" column but none for TiposAnalisis.
    assert_eq!(
        rows[0]["material"],
        "Producto cosechado · Residuos de sustancias activas fitosanitarias"
    );
    // The coded finding resolves against the vendored catalogue and joins the
    // farmer's own wording in the model's single "sustancias" cell.
    assert_eq!(
        rows[0]["substances"],
        "LAMBDA CIHALOTRINA · Lambda cihalotrín 0,01 mg/kg"
    );
    assert_eq!(rows[0]["bulletin"], "B-2026/1187");
    assert_eq!(
        rows[0]["laboratory"],
        "Laboratorio Agroalimentario — Ctra. Burgos km 118, Valladolid — Q4700123B"
    );
    // The plots come back as table 2.1's order numbers, the model's own
    // cross-reference.
    assert_eq!(rows[0]["plots"], "1");
}

/// A substance code the vendored snapshot cannot resolve prints ITSELF — the
/// `problem_code` rule. The laboratory does not wait for our next release, and a
/// finding that vanished from the book would be worse than an unresolved code.
#[test]
fn a_substance_code_the_snapshot_cannot_resolve_prints_itself() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    let mut record = analysis(&fx);
    record.substance_codes = vec!["99999".into()];
    record.substances_detected = None;
    repo::insert_analysis_record(&mut conn, record, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["analysis"][0]["substances"], "99999");
}

/// A laboratory the farmer only half-identified must not print stray dashes.
#[test]
fn an_analysis_with_only_a_lab_name_prints_no_separators() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut record = analysis(&fx);
    record.lab_address = None;
    record.lab_tax_id = None;
    repo::insert_analysis_record(&mut conn, record, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(
        doc["analysis"][0]["laboratory"],
        "Laboratorio Agroalimentario"
    );
}

#[test]
fn the_analysis_sheet_writes_dates_as_values_and_splits_the_laboratory() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_analysis_record(&mut conn, analysis(&fx), None).unwrap();

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "4 Análisis");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };

    assert_eq!(column("Fecha"), Cell::Date("2026-06-18".into()));
    // The sheet keeps the fields apart so each one filters on its own, where
    // the PDF joins them into the model's single cell.
    assert_eq!(
        column("Laboratorio"),
        Cell::Text("Laboratorio Agroalimentario".into())
    );
    assert_eq!(
        column("NIF del laboratorio"),
        Cell::Text("Q4700123B".into())
    );
    // The order numbers stay, and the names are resolved beside them, so the
    // two documents reconcile row for row.
    assert_eq!(column("Id. parcelas"), Cell::Text("1".into()));
    assert_eq!(column("Parcelas"), Cell::Text("El Prado".into()));
    // What the model has no column for gets its own here rather than riding in
    // a neighbour's cell: a spreadsheet is filtered per field.
    assert_eq!(
        column("Tipos de análisis"),
        Cell::Text("Residuos de sustancias activas fitosanitarias".into())
    );
    assert_eq!(
        column("Material analizado"),
        Cell::Text("Producto cosechado".into())
    );
}

/// Section 3.2's spreadsheet column for the same reason: the treatment kind
/// rides in the PDF's product cell, and stands alone where it can be filtered.
#[test]
fn the_seed_sheet_gives_the_treatment_kind_its_own_column() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "3.2 Semilla tratada");
    let index = sheet
        .columns
        .iter()
        .position(|c| c.header == "Tratamiento de la semilla")
        .expect("a treatment-kind column");
    assert_eq!(
        sheet.rows[0][index],
        Cell::Text("Adquirida tratada en España".into())
    );
    let product = sheet
        .columns
        .iter()
        .position(|c| c.header == "Nombre comercial")
        .expect("a product column");
    // And the product cell stays the sack's own text, unfolded.
    assert_eq!(sheet.rows[0][product], Cell::Text("Celest Trio".into()));
}

/// Section 5's quantity is one number and one unit — printed together in the
/// PDF's single "Cantidad" cell, kept apart in the sheet so it is summable.
#[test]
fn the_harvest_register_prints_the_sale_and_its_buyer() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_harvest_record(&mut conn, harvest(&fx), None).unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["harvest"].as_array().expect("a harvest array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["date"], "24/07/2026");
    assert_eq!(rows[0]["quantity"], "42,5 t");
    assert_eq!(rows[0]["buyer"], "Cooperativa Cerealista del Duero");
    assert_eq!(rows[0]["buyer_registry"], "21.0012345/VA");
    assert_eq!(rows[0]["plots"], "1");
}

/// Blank stays blank: the model leaves the cell to be filled by hand, and a
/// zero is a claim the farmer did not make.
#[test]
fn an_unstated_harvest_quantity_prints_blank_and_is_empty_in_the_sheet() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut record = harvest(&fx);
    record.quantity_value = None;
    record.quantity_unit_code = None;
    terrazgo_core::repository::insert_harvest_record(&mut conn, record, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["harvest"][0]["quantity"], "");

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "5 Cosecha");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };
    assert_eq!(column("Cantidad"), Cell::Empty);
    // The unit collapses to a blank cell too: without a value it says nothing.
    assert_eq!(column("Unidad"), Cell::Empty);
}

#[test]
fn the_harvest_sheet_writes_the_quantity_as_a_number_beside_its_unit() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_harvest_record(&mut conn, harvest(&fx), None).unwrap();

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "5 Cosecha");
    let column = |header: &str| {
        let index = sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        sheet.rows[0][index].clone()
    };

    assert_eq!(column("Fecha"), Cell::Date("2026-07-24".into()));
    // "42,5 t" in one cell would not be summable.
    assert_eq!(column("Cantidad"), Cell::Number(42.5));
    assert_eq!(column("Unidad"), Cell::Text("t".into()));
    assert_eq!(column("Nº RGSEAA"), Cell::Text("21.0012345/VA".into()));
}

/// A sale spanning two parcels lists both order numbers and both names.
#[test]
fn a_sale_from_two_plots_lists_both_orders_and_both_names() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut record = harvest(&fx);
    record.plots.push(terrazgo_core::models::NewHarvestPlot {
        plot_id: fx.barley_plot_id.clone(),
        crop_id: Some(fx.barley_crop_id.clone()),
    });
    terrazgo_core::repository::insert_harvest_record(&mut conn, record, None).unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["harvest"][0]["plots"], "1, 2");

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "5 Cosecha");
    let names = sheet
        .columns
        .iter()
        .position(|c| c.header == "Parcelas")
        .map(|i| sheet.rows[0][i].clone())
        .expect("a plots column");
    assert_eq!(names, Cell::Text("El Prado, La Loma".into()));
}

/// The 2.2 water half exists twice on purpose: joined into the section's own
/// per-plot row, and one row per point here with the numbers as NUMBERS. A
/// joined string cannot be sorted, filtered or summed, which is the whole
/// reason the book also exports as a spreadsheet.
#[test]
fn the_water_sheet_writes_one_typed_row_per_abstraction_point() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_water_point(
        &mut conn,
        terrazgo_core::models::NewWaterPoint {
            plot_id: fx.wheat_plot_id.clone(),
            denomination: "Sondeo municipal".into(),
            inside_plot: false,
            distance_m: Some(240.0),
            latitude: Some(41.652_34),
            longitude: Some(-4.728_91),
        },
        None,
    )
    .unwrap();
    water_point(&mut conn, &fx.wheat_plot_id, "Pozo de la casa", true, None);

    let book = workbook(&conn, &fx);
    let tab = sheet(&book, "2.2 Captaciones");
    assert_eq!(tab.rows.len(), 2, "one row per point, not per plot");

    let column = |header: &str| {
        tab.columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"))
    };
    let outside = &tab.rows[0];
    assert!(matches!(outside[column("Distancia (m)")], Cell::Number(n) if n == 240.0));
    assert!(matches!(outside[column("Latitud")], Cell::Number(n) if (n - 41.652_34).abs() < 1e-9));
    assert!(matches!(outside[column("Longitud")], Cell::Number(n) if (n + 4.728_91).abs() < 1e-9));
    assert!(matches!(
        &outside[column("Captación incluida en la parcela")],
        Cell::Text(t) if t == "NO"
    ));

    // Blank, never zero: a point inside the plot has no distance to state, and
    // a zero is a measurement the farmer never made.
    let inside = &tab.rows[1];
    assert!(matches!(inside[column("Distancia (m)")], Cell::Empty));
    assert!(matches!(inside[column("Latitud")], Cell::Empty));
}

/// With no points to hang the answer on, the declaration IS the content — the
/// same call the declared-empty registers make in 3.3–3.5.
#[test]
fn the_water_sheet_writes_a_declared_plot_as_a_single_row() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::set_water_declaration(
        &mut conn,
        &fx.barley_plot_id,
        "2026-05-12",
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let tab = sheet(&book, "2.2 Captaciones");
    assert_eq!(tab.rows.len(), 1, "the wheat plot said nothing at all");
    let denomination = tab
        .columns
        .iter()
        .position(|c| c.header == "Denominación")
        .unwrap();
    assert!(matches!(
        &tab.rows[0][denomination],
        Cell::Text(t) if t == "Sin captaciones — 12/05/2026"
    ));
}

// ---------------------------------------------------------------------------
// Section 8 — the irrigation register (RD 1051/2022 art. 5.e)
// ---------------------------------------------------------------------------

fn irrigate(
    conn: &mut Connection,
    fx: &Fixture,
    date: &str,
    method: &str,
    volume: f64,
    unit: &str,
) -> module_fertilisation::models::IrrigationRecordDetail {
    use module_fertilisation::models::{NewIrrigationPlot, NewIrrigationRecord};
    module_fertilisation::repository::insert_irrigation_record(
        conn,
        NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: date.into(),
            irrigation_end_date: None,
            irrigation_method_code: method.into(),
            volume_value: volume,
            volume_unit_code: unit.into(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![NewIrrigationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: Some(fx.wheat_crop_id.clone()),
                irrigated_area_ha: Some(4.0),
            }],
            water_origins: vec!["groundwater".into()],
        },
        None,
    )
    .unwrap()
}

#[test]
fn irrigation_rows_cross_reference_plots_and_resolve_their_codes() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    irrigate(&mut conn, &fx, "2026-06-14", "drip", 320.0, "m3_ha");

    let doc = inputs(&conn, &fx);
    let row = &doc["irrigation"][0];
    // The plot is named by its order number in table 2.1, like every other
    // register in the book.
    assert_eq!(row["plots"], "1");
    assert_eq!(row["area"], "4");
    assert_eq!(row["dates"], "14/06/2026");
    // Codes resolve to prose through the labels, never stored as prose.
    assert_eq!(row["method"], "Goteo");
    assert_eq!(row["source"], "Subterránea");
    assert_eq!(row["volume"], "320 m³/ha");
}

#[test]
fn the_cumulative_volume_is_a_running_sum_of_the_table() {
    // The model prints "Volumen acumulado" and we never store it: it is a sum
    // over the rows above, and a stored copy could disagree with them.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    irrigate(&mut conn, &fx, "2026-05-02", "drip", 100.0, "m3_ha");
    irrigate(&mut conn, &fx, "2026-06-14", "drip", 250.0, "m3_ha");
    irrigate(
        &mut conn,
        &fx,
        "2026-07-01",
        "sprinkler_fixed",
        50.0,
        "m3_ha",
    );

    let doc = inputs(&conn, &fx);
    let cumulative: Vec<_> = doc["irrigation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["cumulative"].as_str().unwrap())
        .collect();
    assert_eq!(cumulative, vec!["100", "350", "400"]);
}

#[test]
fn an_absolute_volume_does_not_join_the_per_hectare_running_total() {
    // A meter reading in m³ measures a different thing from m³/ha. Adding it
    // to a per-hectare series would produce a total true of no field, so that
    // row prints a blank cumulative cell and leaves the total untouched.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    irrigate(&mut conn, &fx, "2026-05-02", "drip", 100.0, "m3_ha");
    irrigate(&mut conn, &fx, "2026-06-14", "drip", 900.0, "m3");
    irrigate(&mut conn, &fx, "2026-07-01", "drip", 50.0, "m3_ha");

    let doc = inputs(&conn, &fx);
    let rows = doc["irrigation"].as_array().unwrap();
    assert_eq!(rows[0]["cumulative"], "100");
    assert_eq!(
        rows[1]["cumulative"], "",
        "an absolute volume has no per-ha total"
    );
    assert_eq!(rows[1]["volume"], "900 m³");
    assert_eq!(
        rows[2]["cumulative"], "150",
        "and it did not disturb the series"
    );
}

#[test]
fn the_water_quality_cell_folds_two_values_and_stays_blank_when_unstated() {
    // Anexo III C.l asks for both; RD 1051/2022 art. 17.2 makes them
    // conditional, and the printed model has no column for either — so they
    // ride in one cell and the sheet splits them into two.
    use module_fertilisation::models::{NewIrrigationPlot, NewIrrigationRecord};
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    module_fertilisation::repository::insert_irrigation_record(
        &mut conn,
        NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: "2026-06-14".into(),
            irrigation_end_date: None,
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: Some(12.4),
            water_soluble_p2o5_mg_l: Some(0.8),
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![NewIrrigationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
                irrigated_area_ha: None,
            }],
            water_origins: vec![],
        },
        None,
    )
    .unwrap();
    irrigate(&mut conn, &fx, "2026-07-01", "drip", 50.0, "m3_ha");

    let doc = inputs(&conn, &fx);
    let rows = doc["irrigation"].as_array().unwrap();
    assert_eq!(rows[0]["water_quality"], "12,4 · 0,8");
    // No plot stated a surface, so the cell is blank rather than 0.
    assert_eq!(rows[0]["area"], "");
    assert_eq!(rows[1]["water_quality"], "", "unstated stays blank");
}

#[test]
fn an_irrigation_interval_prints_both_dates() {
    // RD 1051/2022 art. 5.f allows accumulating over fortnightly periods.
    use module_fertilisation::models::{NewIrrigationPlot, NewIrrigationRecord};
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    module_fertilisation::repository::insert_irrigation_record(
        &mut conn,
        NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: "2026-06-01".into(),
            irrigation_end_date: Some("2026-06-15".into()),
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![NewIrrigationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
                irrigated_area_ha: None,
            }],
            water_origins: vec![],
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["irrigation"][0]["dates"], "01/06/2026 – 15/06/2026");
}

#[test]
fn the_irrigation_sheet_carries_typed_numbers_and_its_own_columns() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    irrigate(&mut conn, &fx, "2026-06-14", "drip", 320.0, "m3_ha");

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "8 Riego");
    let row = &sheet.rows[0];
    // A date is a real date and a volume a real number — the sheet exists to
    // be sorted and summed, which text cells cannot be.
    assert!(matches!(row[0], terrazgo_report::Cell::Date(_)));
    assert!(matches!(row[5], terrazgo_report::Cell::Number(v) if v == 320.0));
    assert!(matches!(row[7], terrazgo_report::Cell::Number(v) if v == 320.0));
    // The unit is its own column, so the numbers stay summable.
    assert!(matches!(&row[6], terrazgo_report::Cell::Text(t) if t == "m³/ha"));
    // Blank, never zero, where nothing was stated.
    assert!(matches!(row[8], terrazgo_report::Cell::Empty));
    assert!(matches!(row[9], terrazgo_report::Cell::Empty));
}

#[test]
fn a_deleted_irrigation_does_not_print() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let record = irrigate(&mut conn, &fx, "2026-06-14", "drip", 320.0, "m3_ha");
    module_fertilisation::repository::soft_delete_irrigation_record(
        &mut conn,
        &record.record.id,
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert!(doc["irrigation"].as_array().unwrap().is_empty());
}

#[test]
fn every_seeded_unit_has_a_printable_rendering() {
    // `unit_symbol` falls back to "" for an unknown code, so a unit seeded
    // without an arm there prints a bare number in a legal document — which is
    // exactly what happened when m³/ha arrived with section 8. This is the
    // mechanical guard: seeding a unit and forgetting how it prints fails
    // here, not in a rendered PDF nobody re-read.
    //
    // There are two renderings, because there are two kinds of unit. A dose
    // unit is a SYMBOL (`L/ha` reads the same in every language) and lives in
    // `unit_symbol`; an intensity is a COUNT, which is prose, so "trampas" /
    // "trampes" comes from `Labels`. The guard covers both so it stays total:
    // every seeded unit must print SOMETHING, whichever kind it is.
    let conn = open_in_memory().unwrap();
    let mut stmt = conn.prepare("SELECT code, dimension FROM unit").unwrap();
    let units: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(units.len() >= 21, "the seed shrank unexpectedly");
    for (code, dimension) in units {
        if dimension == "intensity" {
            // `intensity_unit` falls back to the code itself, so the real
            // assertion is that it was TRANSLATED — a code leaking onto the
            // page would read as "4 diffusers_ha".
            // ALL rather than a hand-listed pair: a third language must be
            // caught by this guard the day it is added.
            for language in ReportLanguage::ALL {
                let labels = language.labels();
                assert_ne!(
                    labels.intensity_unit(&code),
                    code,
                    "intensity unit '{code}' has no word — add an arm to Labels::intensity_unit"
                );
            }
        } else {
            assert!(
                !terrazgo_recordbook::unit_display_symbol(&code).is_empty(),
                "unit '{code}' has no printable symbol — add an arm to unit_symbol"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Section 6 — the fertilisation register (RD 1051/2022 art. 5.d)
//
// The binding field list is RD 1311/2012 Anexo III Parte I sección C, which is
// wider than the printed model — so several of these tests pin what the book
// does with a legal field the model has no column for.
// ---------------------------------------------------------------------------

fn nac27(conn: &mut Connection) -> String {
    module_fertilisation::repository::insert_fertiliser_material(conn, nac27_input(), None)
        .unwrap()
        .material
        .id
}

fn nac27_input() -> module_fertilisation::models::NewFertiliserMaterial {
    use module_fertilisation::models::{MaterialNutrient, NewFertiliserMaterial};
    let nutrient = |kind: &str, code: &str, percentage: f64| MaterialNutrient {
        id: String::new(),
        kind_code: kind.into(),
        nutrient_code: code.into(),
        percentage,
    };
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
            nutrient("macro", "1", 27.0),      // N total — the model prints this
            nutrient("macro", "3", 13.5),      // N nítrico — C.h, no model column
            nutrient("macro", "6", 0.0),       // P₂O₅ total
            nutrient("heavy_metal", "1", 0.4), // cadmio — C.i
        ],
    }
}

fn fertilise(
    conn: &mut Connection,
    fx: &Fixture,
    material_id: &str,
    type_code: &str,
    method_code: &str,
) -> module_fertilisation::models::FertilisationRecordDetail {
    use module_fertilisation::models::{NewFertilisationPlot, NewFertilisationRecord};
    module_fertilisation::repository::insert_fertilisation_record(
        conn,
        NewFertilisationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            applied_on: "2026-03-12".into(),
            application_end_date: None,
            fertilisation_type_code: type_code.into(),
            application_method_code: method_code.into(),
            dose_value: 250.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: material_id.to_string(),
            sludge_application: false,
            machinery_id: None,
            service_company: None,
            service_regfer_number: None,
            delivery_note_ref: Some("ALB-2026-118".into()),
            yield_estimated_kg_ha: Some(6500.0),
            yield_final_kg_ha: None,
            notes: None,
            plots: vec![NewFertilisationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: Some(fx.wheat_crop_id.clone()),
                fertilised_area_ha: Some(4.0),
            }],
            practices: vec![],
        },
        None,
    )
    .unwrap()
}

#[test]
fn fertilisation_rows_cross_reference_plots_and_resolve_their_codes() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    // The coded material kind only resolves to prose against the vendored
    // catalogue; without it the code prints itself, which is the rule and has
    // its own test below.
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let material_id = nac27(&mut conn);
    fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");

    let doc = inputs(&conn, &fx);
    let row = &doc["fertilisation"][0];
    assert_eq!(row["plots"], "1");
    assert_eq!(row["area"], "4");
    assert_eq!(row["dates"], "12/03/2026");
    assert_eq!(row["dose"], "250 kg/ha");
    assert_eq!(row["delivery_note"], "ALB-2026-118");
    assert_eq!(row["yield_estimated"], "6500");
    // An unstated final yield is blank, never 0 — the harvest has not happened.
    assert_eq!(row["yield_final"], "");
    // The crop on the fertilised plot, as the model's "Cultivo" column —
    // species and variety, exactly as the crop row states them.
    assert_eq!(row["crops"], "wheat — Craklin");
    // C.d's coded kind rides in the material cell, resolved against MAT_FERTI.
    assert_eq!(
        row["material"],
        "NAC 27 · Productos fertilizantes: abonos inorgánicos"
    );
}

#[test]
fn the_model_sigla_carries_both_legal_fields_it_merges() {
    // The model's footnote lists (F) fertirrigación beside (AF) abonado de
    // fondo and (AC) abonado de cobertera as if the three were one list. They
    // are not: (F) answers Anexo III C.f (forma de aplicación) and AF/AC answer
    // C.c (tipo de fertilización), so a fertigated cobertera is honestly
    // "F/AC" and the cell also spells out the method the letter drops.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);

    fertilise(
        &mut conn,
        &fx,
        &material_id,
        "base_dressing",
        "banded_buried",
    );
    fertilise(
        &mut conn,
        &fx,
        &material_id,
        "top_dressing",
        "fertigation_localised",
    );
    // An enmienda has no sigla in the model at all; the cell must still name
    // both legal fields rather than printing a stray separator.
    fertilise(&mut conn, &fx, &material_id, "amendment", "broadcast");

    let doc = inputs(&conn, &fx);
    let kinds: Vec<&str> = doc["fertilisation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "(AF) · Abonado de fondo · Esparcido localizado y enterrado",
            "(F/AC) · Abonado de cobertera · Riego localizado (fertirrigación)",
            "Aplicación de enmienda · Esparcido general",
        ]
    );
}

#[test]
fn the_richness_cell_names_each_figure_and_omits_the_unstated_ones() {
    // "Riqueza N/P/K" with a bare "27 / 0" would leave a reader guessing which
    // figure is missing. Each is stated with its symbol, and one the label
    // never declared contributes nothing — a printed 0 would claim the material
    // contains none of it.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");

    let doc = inputs(&conn, &fx);
    // NAC 27 declares N total and P₂O₅ total, and says nothing about K₂O.
    assert_eq!(doc["fertilisation"][0]["richness"], "N 27 / P₂O₅ 0");
}

#[test]
fn a_sludge_application_is_marked_beside_the_material() {
    use terrazgo_report::Cell;
    // C.i / art. 5.g. The printed model has no box for it, so it rides in the
    // material cell and takes a column of its own in the sheet.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    let created = fertilise(&mut conn, &fx, &material_id, "amendment", "broadcast");
    conn.execute(
        "UPDATE fertilisation_record SET sludge_application = 1 WHERE id = ?1",
        [&created.record.id],
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert!(
        doc["fertilisation"][0]["material"]
            .as_str()
            .unwrap()
            .ends_with("· aplicación de lodos"),
        "got {}",
        doc["fertilisation"][0]["material"]
    );
    assert_eq!(
        sheet(&workbook(&conn, &fx), "6 Fertilización").rows[0][7],
        Cell::Text("SÍ".into()),
        "the sheet answers the flag in a column of its own"
    );
}

#[test]
fn the_material_tab_carries_the_composition_the_register_row_cannot() {
    use terrazgo_report::Cell;
    // Anexo III C.h asks for eight agronomic values per material, C.i adds the
    // sludge heavy metals and a label may declare micronutrients on top. The
    // printed register prints three of them; the rest are real numbers worth
    // filtering, so they get a tab with one row per figure.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    nac27(&mut conn);

    let book = workbook(&conn, &fx);
    let materials = sheet(&book, "6 Materiales");
    assert_eq!(materials.rows.len(), 4, "one row per composition figure");
    // Macronutrients first, then heavy metals — the order the SIEX material
    // block lists its arrays in.
    assert_eq!(materials.rows[0][7], Cell::Text("Macronutrientes".into()));
    assert_eq!(materials.rows[0][8], Cell::Text("N total".into()));
    assert_eq!(materials.rows[0][9], Cell::Number(27.0));
    assert_eq!(materials.rows[3][7], Cell::Text("Metales pesados".into()));
    // Code 1 is "N total" in MACRONUTRIENTES and "Cadmio (Cd)" in
    // METALES_PESADOS: the kind is what tells them apart.
    assert_eq!(materials.rows[3][8], Cell::Text("Cadmio (Cd)".into()));
    // Every row repeats the material so each stands alone under a filter.
    assert_eq!(materials.rows[3][0], Cell::Text("NAC 27".into()));
}

#[test]
fn the_manure_treatment_prints_as_prose_not_as_its_code() {
    // A lookup this app owns must never leak its identifier into a document —
    // the `material_kind` class of defect, caught here by reading a rendered
    // sheet rather than by a compiler.
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut slurry = {
        let mut material = nac27_input();
        material.name = "Purín de porcino".into();
        material.material_code = "3".into();
        material
    };
    slurry.manure_treatment_code = Some("solid_fraction".into());
    module_fertilisation::repository::insert_fertiliser_material(&mut conn, slurry, None).unwrap();

    let book = workbook(&conn, &fx);
    assert_eq!(
        sheet(&book, "6 Materiales").rows[0][5],
        Cell::Text("Separación sólido-líquido: fracción sólida".into())
    );
}

#[test]
fn the_fertilisation_sheet_splits_what_the_pdf_folds_into_one_cell() {
    use terrazgo_report::Cell;
    // The analysis-kinds precedent: a value with no column in the model rides
    // in a neighbouring printed cell AND gets a column of its own here, where
    // it can be filtered rather than read.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let material_id = nac27(&mut conn);
    fertilise(
        &mut conn,
        &fx,
        &material_id,
        "top_dressing",
        "fertigation_localised",
    );

    let book = workbook(&conn, &fx);
    let row = &sheet(&book, "6 Fertilización").rows[0];
    assert_eq!(row[5], Cell::Text("NAC 27".into()));
    assert_eq!(
        row[6],
        Cell::Text("Productos fertilizantes: abonos inorgánicos".into())
    );
    // The two legal fields the model's single letter merges, each on its own.
    assert_eq!(row[13], Cell::Text("Abonado de cobertera".into()));
    assert_eq!(
        row[14],
        Cell::Text("Riego localizado (fertirrigación)".into())
    );
    // Real numbers, and the dose's unit in a column of its own so the figures
    // stay summable.
    assert_eq!(row[11], Cell::Number(250.0));
    assert_eq!(row[12], Cell::Text("kg/ha".into()));
    assert_eq!(row[8], Cell::Number(27.0));
    // K₂O was never declared: blank, never zero.
    assert_eq!(row[10], Cell::Empty);
}

#[test]
fn good_practices_reach_the_sheet_resolved_in_their_own_scope() {
    use terrazgo_report::Cell;
    // The SIEX twin requires `BuenasPracticas` while the printed model has no
    // column for it, so the practices are captured and appear only here — as
    // whole sentences no register cell could carry. The catalogue holds three
    // vocabularies keyed by ámbito and the same integer means a different
    // practice in each, so resolving without the scope would print an
    // irrigation practice on a fertilisation record.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let material_id = nac27(&mut conn);
    let created = fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");
    conn.execute(
        "INSERT INTO fertilisation_practice (id, fertilisation_record_id, practice_code)
         VALUES ('p1', ?1, '3')",
        [&created.record.id],
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let practices = match &sheet(&book, "6 Fertilización").rows[0][21] {
        Cell::Text(text) => text.clone(),
        other => panic!("expected text, got {other:?}"),
    };
    assert_eq!(
        practices, "Aplicación de purines mediante inyección",
        "code 3 in the Fertilización ámbito — not what 3 means under Riego"
    );
}

#[test]
fn a_farm_with_no_fertilisation_prints_an_empty_register() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let doc = inputs(&conn, &fx);
    assert!(doc["fertilisation"].as_array().unwrap().is_empty());
    assert!(
        sheet(&workbook(&conn, &fx), "6 Fertilización")
            .rows
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Section 7.1 — the plan de abonado (RD 1051/2022 art. 4.2, 5.a and 6)
//
// Only the recommendation is stored. The rest of the table is section 6's own
// records seen again, so these tests are mostly about arithmetic that must
// never quietly produce a number it cannot stand behind.
// ---------------------------------------------------------------------------

fn plan_for(
    conn: &mut Connection,
    fx: &Fixture,
    crop_id: &str,
    n: f64,
) -> module_fertilisation::models::FertilisationPlanDetail {
    module_fertilisation::repository::insert_fertilisation_plan(
        conn,
        module_fertilisation::models::NewFertilisationPlan {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            needs_n_kg_ha: n,
            needs_p2o5_kg_ha: 60.0,
            needs_k2o_kg_ha: 0.0,
            expected_yield_kg_ha: 6500.0,
            preceding_crop_code: Some("60".into()),
            drawn_up_on: "2025-09-20".into(),
            tool_generated: false,
            notes: None,
            crop_ids: vec![crop_id.to_string()],
        },
        None,
    )
    .unwrap()
}

/// A material dosed by volume, which only becomes kilograms with a density.
fn slurry(conn: &mut Connection, density: Option<f64>) -> String {
    use module_fertilisation::models::{MaterialNutrient, NewFertiliserMaterial};
    module_fertilisation::repository::insert_fertiliser_material(
        conn,
        NewFertiliserMaterial {
            name: "Purín de porcino".into(),
            material_code: "3".into(),
            material_detail_code: None,
            supplier_name: None,
            supplier_rega: None,
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: None,
            density_kg_l: density,
            notes: None,
            nutrients: vec![MaterialNutrient {
                id: String::new(),
                kind_code: "macro".into(),
                nutrient_code: "1".into(),
                percentage: 0.42,
            }],
        },
        None,
    )
    .unwrap()
    .material
    .id
}

fn dose(
    conn: &mut Connection,
    fx: &Fixture,
    material_id: &str,
    value: f64,
    unit: &str,
    date: &str,
) {
    use module_fertilisation::models::{NewFertilisationPlot, NewFertilisationRecord};
    module_fertilisation::repository::insert_fertilisation_record(
        conn,
        NewFertilisationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            applied_on: date.into(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "broadcast".into(),
            dose_value: value,
            dose_unit_code: unit.into(),
            fertiliser_material_id: material_id.to_string(),
            sludge_application: false,
            machinery_id: None,
            service_company: None,
            service_regfer_number: None,
            delivery_note_ref: None,
            yield_estimated_kg_ha: None,
            yield_final_kg_ha: None,
            notes: None,
            plots: vec![NewFertilisationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: Some(fx.wheat_crop_id.clone()),
                fertilised_area_ha: Some(4.0),
            }],
            practices: vec![],
        },
        None,
    )
    .unwrap();
}

#[test]
fn supplied_units_are_the_dose_times_the_richness() {
    // A unidad fertilizante is kg/ha of the nutrient (the model's footnote 2),
    // so 250 kg/ha of a 27 % nitrogen fertiliser supplies 67,5 UF N/ha.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");

    let doc = inputs(&conn, &fx);
    let row = &doc["plan_rows"][0];
    // N 27 % and a STATED P₂O₅ of 0 % both compute; the material declares no
    // potassium at all, so that figure is a dash rather than a zero — the two
    // are different claims, and only one of them is the label's.
    assert_eq!(row["supplied"], "67,5 / 0 / —");
    assert_eq!(row["dose"], "250 kg/ha");
}

#[test]
fn a_volume_dose_supplies_a_known_amount_only_with_a_density() {
    // 25 m³/ha of slurry at 1,02 kg/L is 25 500 kg/ha, and at 0,42 % N that is
    // 107,1 UF N/ha. Without the density the app cannot say, and a dash is the
    // honest answer — an assumed 1,0 kg/L would understate the nitrogen.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let with_density = slurry(&mut conn, Some(1.02));
    dose(&mut conn, &fx, &with_density, 25.0, "m3_ha", "2025-10-14");

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plan_rows"][0]["supplied"], "107,1 / — / —");

    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let without = slurry(&mut conn, None);
    dose(&mut conn, &fx, &without, 25.0, "m3_ha", "2025-10-14");

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plan_rows"][0]["supplied"], "— / — / —");
}

#[test]
fn the_accumulated_column_is_a_running_sum_per_production_unit() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    dose(&mut conn, &fx, &material_id, 100.0, "kg_ha", "2025-11-02");
    dose(&mut conn, &fx, &material_id, 250.0, "kg_ha", "2026-03-12");

    let doc = inputs(&conn, &fx);
    let accumulated: Vec<&str> = doc["plan_rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["accumulated"].as_str().unwrap())
        .collect();
    // 27 UF N, then 27 + 67,5 = 94,5. The potassium column never starts,
    // because the material never declared any.
    assert_eq!(accumulated, vec!["27 / 0 / —", "94,5 / 0 / —"]);
}

#[test]
fn one_unknown_contribution_stops_the_running_total_for_good() {
    // A slurry application with no density behind it contributes an unknown
    // amount of nitrogen. Skipping it would print a total that reads as "this
    // much has been applied" while being short by that application, and a
    // farmer comparing it against the recommendation would over-fertilise.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let nac = nac27(&mut conn);
    let unknown = slurry(&mut conn, None);
    dose(&mut conn, &fx, &nac, 100.0, "kg_ha", "2025-11-02");
    dose(&mut conn, &fx, &unknown, 25.0, "m3_ha", "2026-01-20");
    dose(&mut conn, &fx, &nac, 250.0, "kg_ha", "2026-03-12");

    let doc = inputs(&conn, &fx);
    let accumulated: Vec<&str> = doc["plan_rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["accumulated"].as_str().unwrap())
        .collect();
    assert_eq!(
        accumulated,
        vec!["27 / 0 / —", "— / — / —", "— / — / —"],
        "the nitrogen total is no longer knowable, and says so"
    );
}

#[test]
fn the_recommendation_is_the_only_stored_block() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");

    // With no plan yet, the row still prints — the applications happened.
    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plan_rows"][0]["recommended"], "— / — / —");

    plan_for(&mut conn, &fx, &fx.wheat_crop_id, 140.0);
    let doc = inputs(&conn, &fx);
    let row = &doc["plan_rows"][0];
    assert_eq!(row["recommended"], "140 / 60 / 0");
    // And the applied figures are unchanged by the plan: they come from
    // section 6, which is the point of assembling rather than storing them.
    assert_eq!(row["supplied"], "67,5 / 0 / —");
}

#[test]
fn the_plan_sheet_splits_every_block_into_real_numbers() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");
    plan_for(&mut conn, &fx, &fx.wheat_crop_id, 140.0);

    let book = workbook(&conn, &fx);
    let row = &sheet(&book, "7.1 Plan de abonado").rows[0];
    assert_eq!(row[6], Cell::Number(250.0));
    assert_eq!(row[7], Cell::Text("kg/ha".into()));
    // Nine columns where the PDF prints three cells, so applied and
    // recommended can actually be compared.
    assert_eq!(row[8], Cell::Number(67.5));
    assert_eq!(row[11], Cell::Number(67.5));
    assert_eq!(row[14], Cell::Number(140.0));
    assert_eq!(row[16], Cell::Number(0.0));
}

#[test]
fn an_unknown_figure_is_empty_in_the_sheet_never_zero() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let unknown = slurry(&mut conn, None);
    dose(&mut conn, &fx, &unknown, 25.0, "m3_ha", "2025-10-14");

    let book = workbook(&conn, &fx);
    let row = &sheet(&book, "7.1 Plan de abonado").rows[0];
    // A spreadsheet adds zeros up; it leaves blanks alone.
    assert_eq!(row[8], Cell::Empty);
    assert_eq!(row[11], Cell::Empty);
}

#[test]
fn a_farm_with_no_fertilisation_has_an_empty_plan_table() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    plan_for(&mut conn, &fx, &fx.wheat_crop_id, 140.0);

    // The table's rows ARE the applications: a plan on its own recommends
    // something that has not been acted on yet, and the form is blank.
    let doc = inputs(&conn, &fx);
    assert!(doc["plan_rows"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Anexo III A.3 — the soil block, and the second decree's annex additions
// ---------------------------------------------------------------------------

fn soil_analysis(conn: &mut Connection, fx: &Fixture, soil: module_cue::models::SoilParameters) {
    module_cue::repository::insert_analysis_record(
        conn,
        module_cue::models::NewAnalysisRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            sampled_on: "2025-09-08".into(),
            material_kind_code: "soil".into(),
            bulletin_number: Some("S-2025/912".into()),
            lab_name: Some("Laboratorio Agroalimentario".into()),
            lab_address: None,
            lab_tax_id: None,
            substances_detected: None,
            soil,
            notes: None,
            plots: vec![module_cue::models::NewAnalysisPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
            }],
            analysis_type_codes: vec!["soil_parameters".into()],
            substance_codes: vec![],
        },
        None,
    )
    .unwrap();
}

#[test]
fn soil_figures_ride_in_the_findings_cell_the_model_has_no_page_for() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    soil_analysis(
        &mut conn,
        &fx,
        module_cue::models::SoilParameters {
            ph: Some(6.8),
            organic_matter_pct: Some(2.1),
            available_p_mg_kg: Some(18.0),
            available_k_mg_kg: Some(240.0),
            total_n_pct: Some(0.12),
            conductivity_ds_m: Some(0.35),
            sand_pct: Some(40.0),
            silt_pct: Some(35.0),
            clay_pct: Some(25.0),
        },
    );

    let doc = inputs(&conn, &fx);
    let cell = doc["analysis"][0]["substances"].as_str().unwrap();
    // Each figure with the unit its field is named for, and texture as the
    // three fractions the twin carries rather than a class name.
    assert_eq!(
        cell,
        "pH 6,8 · M.O. 2,1 % · P 18 mg/kg · K 240 mg/kg · N 0,12 % · CE 0,35 dS/m \
         · Text. 40 / 35 / 25 %"
    );
}

#[test]
fn a_partial_bulletin_prints_only_what_it_reported() {
    // A missing figure is not a zero: the farmer did not ask for it.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    soil_analysis(
        &mut conn,
        &fx,
        module_cue::models::SoilParameters {
            ph: Some(7.4),
            organic_matter_pct: Some(1.8),
            ..Default::default()
        },
    );

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["analysis"][0]["substances"], "pH 7,4 · M.O. 1,8 %");
}

#[test]
fn a_residue_bulletin_carries_no_soil_text_and_no_soil_row() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    soil_analysis(&mut conn, &fx, Default::default());

    let doc = inputs(&conn, &fx);
    // Nothing folded in, and no stray separator.
    assert_eq!(doc["analysis"][0]["substances"], "");
    // And the soil tab skips it rather than writing a row of nine blanks,
    // which would say the soil was measured and found to be nothing.
    let book = workbook(&conn, &fx);
    assert!(sheet(&book, "4 Suelo").rows.is_empty());
    let _ = Cell::Empty;
}

#[test]
fn the_soil_tab_gives_every_parameter_a_column_of_real_numbers() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    soil_analysis(
        &mut conn,
        &fx,
        module_cue::models::SoilParameters {
            ph: Some(6.8),
            organic_matter_pct: Some(2.1),
            available_p_mg_kg: Some(18.0),
            ..Default::default()
        },
    );

    let book = workbook(&conn, &fx);
    let row = &sheet(&book, "4 Suelo").rows[0];
    assert_eq!(row[0], Cell::Date("2025-09-08".into()));
    assert_eq!(row[3], Cell::Text("S-2025/912".into()));
    assert_eq!(row[4], Cell::Number(6.8));
    assert_eq!(row[5], Cell::Number(2.1));
    assert_eq!(row[6], Cell::Number(18.0));
    // Blank, never zero — a spreadsheet averages what it is given.
    assert_eq!(row[7], Cell::Empty);
    assert_eq!(row[12], Cell::Empty);
}

#[test]
fn the_annex_lists_the_second_decrees_documents_too() {
    // The printed model's seven items predate RD 1051/2022. A book that tells
    // a holding what to keep and omits the plan de abonado, the sludge
    // application document and the manure quality document would mislead it.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let doc = inputs(&conn, &fx);
    let annex = &doc["labels"]["annex"];

    assert!(annex["item_plan"].as_str().unwrap().contains("art. 6"));
    assert!(annex["item_sludge"].as_str().unwrap().contains("art. 5.g"));
    assert!(annex["item_manure"].as_str().unwrap().contains("art. 13.2"));
    // The manure document is not needed when the holder supplies their own —
    // art. 13.2 says so explicitly, and a list that omitted the exception
    // would ask for paperwork the decree does not.
    assert!(
        annex["item_manure"]
            .as_str()
            .unwrap()
            .contains("propio titular")
    );
}

// ---------------------------------------------------------------------------
// Model 3.1 bis — the advised cut
//
// Not a second register: RD 1311/2012 Anexo III Parte I B is ONE list (a-k)
// covering every treatment, and B.d puts the advisor on it ("identificación
// del aplicador y, EN SU CASO, del asesor"). The page is a second VIEW of the
// same records, showing the two things 3.1 has no column for.
// ---------------------------------------------------------------------------

/// A treatment carrying a non-chemical measure and its intensity, with no
/// product at all — what art. 10.1 asks farmers to prefer.
fn non_chemical_treatment(fx: &Fixture, application_date: &str) -> NewTreatmentRecord {
    let mut new = treatment(fx, application_date);
    new.product_id = None;
    new.dose_value = None;
    new.dose_unit_code = None;
    new.measure_code = Some("15".into()); // feromonas y atrayentes para monitoreo
    new.measure_intensity_value = Some(4.0);
    new.measure_intensity_unit_code = Some("diffusers_ha".into());
    new
}

fn an_advisor(conn: &mut Connection) -> String {
    terrazgo_core::repository::insert_advisor(
        conn,
        terrazgo_core::models::NewAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: None,
            registration_number: Some("ROPO-8891".into()),
        },
        None,
    )
    .unwrap()
    .id
}

#[test]
fn an_unadvised_treatment_stays_off_the_advised_page() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    // It IS in the binding register...
    assert_eq!(doc["treatments"].as_array().unwrap().len(), 1);
    // ...and not on a page headed "solamente para cultivos objeto de
    // asesoramiento", which it was not.
    assert!(doc["advised"].as_array().unwrap().is_empty());
}

#[test]
fn a_treatment_naming_an_advisor_appears_on_both_pages() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let advisor_id = an_advisor(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.advisor_id = Some(advisor_id);
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["treatments"].as_array().unwrap().len(), 1);
    let advised = &doc["advised"].as_array().unwrap()[0];
    // The chemical half fills 3.1 bis's "alternativas químicas" columns.
    assert_eq!(advised["product"], "Fungitop");
    assert_eq!(advised["dose"], "1,5 L/ha");
    assert_eq!(advised["product_date"], "01/05/2026");
    // The non-chemical half is empty — nothing was tried instead, and a blank
    // is the honest statement of that.
    assert_eq!(advised["measure"], "");
    assert_eq!(advised["intensity"], "");
    assert_eq!(advised["measure_date"], "");
}

#[test]
fn a_non_chemical_actuation_prints_its_measure_and_intensity() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    repo::insert_treatment_record(
        &mut conn,
        non_chemical_treatment(&fx, "2026-05-04"),
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 2.1)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let advised = &doc["advised"].as_array().unwrap()[0];
    // The measure resolves against TIPO_MEDIDA_FITOSANITARIA, not against the
    // holding-level MEDIDA_PREVENTIVA_CULTURAL list (a different catalogue for
    // a different question).
    assert_eq!(advised["measure"], "Feromonas y atrayentes para monitoreo");
    // A count, worded rather than symbolised — "4 difusores/ha", not "4".
    assert_eq!(advised["intensity"], "4 difusores/ha");
    assert_eq!(advised["measure_date"], "04/05/2026");
    // No product, so the chemical columns and the plazo stay blank rather than
    // printing zeros.
    assert_eq!(advised["product"], "");
    assert_eq!(advised["dose"], "");
    assert_eq!(advised["product_date"], "");
    let row = &doc["treatments"].as_array().unwrap()[0];
    assert_eq!(row["phi"], "", "a measure imposes no plazo de seguridad");
    assert_eq!(row["dose"], "");
}

#[test]
fn the_validation_boxes_prefill_only_when_one_advisor_signed_the_page() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let ana = an_advisor(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.advisor_id = Some(ana);
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["advised_advisor"], "Ana Ruiz");
    assert_eq!(doc["advised_ropo"], "ROPO-8891");

    // A second advisor on the same page and the lines go blank: the boxes ask
    // for a signature, and naming one of two advisors against a signature
    // nobody gave would be the book asserting something it cannot know.
    let luis = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "Luis Marín".into(),
            tax_id: None,
            registration_number: Some("ROPO-7742".into()),
        },
        None,
    )
    .unwrap()
    .id;
    let mut second = treatment(&fx, "2026-05-08");
    second.advisor_id = Some(luis);
    repo::insert_treatment_record(
        &mut conn,
        second,
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 2.1)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["advised"].as_array().unwrap().len(), 2);
    assert_eq!(doc["advised_advisor"], "");
    assert_eq!(doc["advised_ropo"], "");
}

/// The reachable version of the two-advisor case: ONE advisor whose registry
/// entry is corrected mid-season. Snapshots freeze at write time, so the page
/// prints their name twice against two different ROPO numbers — and prefilling
/// either would put a number on a signature line that half the page below it
/// contradicts.
#[test]
fn a_corrected_registration_number_blanks_the_validation_boxes() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let advisor_id = an_advisor(&mut conn);

    let mut first = treatment(&fx, "2026-05-01");
    first.advisor_id = Some(advisor_id.clone());
    repo::insert_treatment_record(
        &mut conn,
        first,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    // The same person, same name, corrected ROPO number.
    terrazgo_core::repository::update_advisor(
        &mut conn,
        &advisor_id,
        terrazgo_core::models::UpdateAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: None,
            registration_number: Some("ROPO-9002".into()),
        },
        None,
    )
    .unwrap();

    let mut second = treatment(&fx, "2026-05-08");
    second.advisor_id = Some(advisor_id);
    repo::insert_treatment_record(
        &mut conn,
        second,
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 2.1)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["advised"].as_array().unwrap().len(), 2);
    // The boxes state neither. Comparing advisor ids would have prefilled one
    // of the two numbers against a signature nobody gave.
    assert_eq!(doc["advised_advisor"], "");
    assert_eq!(doc["advised_ropo"], "");

    // ...and the two rows really do disagree, each carrying the number that
    // was true when it was written. That is the snapshot rule working, not a
    // bug the blank boxes are papering over.
    let book = workbook(&conn, &fx);
    let treatments = sheet(&book, "3.1 Tratamientos");
    let ropo = treatments
        .columns
        .iter()
        .position(|c| c.header == "Nº Inscripción ROPO")
        .unwrap();
    let printed: Vec<&terrazgo_report::Cell> = treatments.rows.iter().map(|r| &r[ropo]).collect();
    assert!(
        matches!(printed[0], terrazgo_report::Cell::Text(v) if v == "ROPO-8891"),
        "got {:?}",
        printed[0]
    );
    assert!(
        matches!(printed[1], terrazgo_report::Cell::Text(v) if v == "ROPO-9002"),
        "got {:?}",
        printed[1]
    );
}

/// A row carrying only a measure nominates nobody, so it neither confirms nor
/// contradicts: the page still names exactly one advisor and the boxes fill.
#[test]
fn a_measure_only_row_does_not_blank_the_validation_boxes() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let advisor_id = an_advisor(&mut conn);

    let mut advised = treatment(&fx, "2026-05-01");
    advised.advisor_id = Some(advisor_id);
    repo::insert_treatment_record(
        &mut conn,
        advised,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        non_chemical_treatment(&fx, "2026-05-08"),
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 2.1)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["advised"].as_array().unwrap().len(), 2);
    assert_eq!(doc["advised_advisor"], "Ana Ruiz");
    assert_eq!(doc["advised_ropo"], "ROPO-8891");
}

/// A crop stating no area of its own leaves the cell blank. `unwrap_or_default`
/// would print "0", and a cultivated surface of zero hectares is a statement
/// the farmer never made — the same rule that stops the cell being filled from
/// the plot's area, which is a different figure.
#[test]
fn a_crop_stating_no_area_prints_a_blank_surface_never_a_zero() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut advised = treatment(&fx, "2026-05-01");
    advised.advisor_id = Some(an_advisor(&mut conn));
    repo::insert_treatment_record(
        &mut conn,
        advised,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["advised"][0]["crop_surface"], "");
    // The TREATED surface is a different figure and was stated, so it prints.
    assert_eq!(doc["advised"][0]["treated_surface"], "3,2");
}

/// Deleting a crop is allowed precisely because `treatment_plot` freezes what
/// the record book prints, so the book must go on printing this row in full —
/// including the cultivated surface, which is read live from the crop rather
/// than frozen onto the record.
#[test]
fn a_soft_deleted_crop_still_states_its_cultivated_surface() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::update_crop(
        &mut conn,
        &fx.wheat_crop_id,
        terrazgo_core::models::UpdateCrop {
            species_name: "wheat".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(7.5),
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    let mut advised = treatment(&fx, "2026-05-01");
    advised.advisor_id = Some(an_advisor(&mut conn));
    repo::insert_treatment_record(
        &mut conn,
        advised,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 3.2)],
        None,
    )
    .unwrap();

    let before = inputs(&conn, &fx);
    assert_eq!(before["advised"][0]["crop_surface"], "7,5");

    terrazgo_core::repository::soft_delete_crop(&mut conn, &fx.wheat_crop_id, None).unwrap();

    let after = inputs(&conn, &fx);
    assert_eq!(
        after["advised"][0]["crop_surface"], "7,5",
        "a deleted crop must not silently blank a surface the book already printed"
    );
}

#[test]
fn the_sheet_carries_the_advised_columns_as_filterable_values() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let advisor_id = an_advisor(&mut conn);
    let mut new = non_chemical_treatment(&fx, "2026-05-04");
    new.advisor_id = Some(advisor_id);
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 2.1)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    // One tab, not two: 3.1 bis is a view of these rows, so duplicating them
    // onto a second sheet would make one event two rows.
    let treatments = sheet(&book, "3.1 Tratamientos");
    assert_eq!(treatments.rows.len(), 1);
    let row = &treatments.rows[0];
    use terrazgo_report::Cell;
    // Address the columns by their HEADER rather than by counting from the
    // end: the arithmetic would keep passing against the wrong cell the next
    // time a column lands in the middle of the sheet.
    let at = |header: &str| {
        let index = treatments
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        &row[index]
    };
    assert!(matches!(at("Asesor"), Cell::Text(v) if v == "Ana Ruiz"));
    assert!(matches!(at("Nº Inscripción ROPO"), Cell::Text(v) if v == "ROPO-8891"));
    assert!(
        matches!(at("Tipo de medida"), Cell::Text(v) if v == "Feromonas y atrayentes para monitoreo")
    );
    // A real number beside its own unit column, so intensities can be sorted.
    assert!(
        matches!(at("Intensidad de la medida"), Cell::Number(v) if (v - 4.0).abs() < f64::EPSILON)
    );
    assert!(matches!(at("Unidad de intensidad"), Cell::Text(v) if v == "difusores/ha"));
    // No product: dose and PHI are Empty, never 0 — a spreadsheet adds zeros up.
    assert!(matches!(at("Dosis"), Cell::Empty), "dose must be blank");
    assert!(
        matches!(at("Plazo de seguridad (días)"), Cell::Empty),
        "a measure imposes no plazo"
    );
}

// ---------------------------------------------------------------------------
// Model 2.1: "Término municipal (código y nombre)"
// ---------------------------------------------------------------------------

/// The column asks for both, and the provider returns only the code. With the
/// snapshot imported — as a running app always has it — the name resolves and
/// joins the code in the PDF's single cell.
#[test]
fn the_municipality_prints_its_code_and_its_name() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    assert_eq!(rows[0]["municipality"], "186 · VALVERDE DE CAMPOS");
    // The province column keeps printing the CODE: model 2.1 asks it for
    // "Código Provincia", unlike 1.1's "Provincia", which asks for the name.
    assert_eq!(rows[0]["province"], "47");
    // A plot with no SIGPAC reference prints neither, and no stray separator.
    assert_eq!(rows[1]["municipality"], "");
}

/// The province is part of the KEY, not context: municipality codes repeat
/// across provinces, so 001 must resolve differently in Álava and Valladolid,
/// and a plot that states no province gets no name rather than the first of 52
/// candidates.
#[test]
fn a_municipality_code_means_a_different_town_in_each_province() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    let es = |province: Option<&str>, municipality: Option<&str>| {
        Some(PlotEsFields {
            sigpac_province: province.map(Into::into),
            sigpac_municipality: municipality.map(Into::into),
            sigpac_aggregate: None,
            sigpac_zone: None,
            sigpac_polygon: None,
            sigpac_parcel: None,
            sigpac_enclosure: None,
        })
    };
    // Alphabetical by name, so these land as rows 3..7 after El Prado/La Loma.
    insert_plot(
        &mut conn,
        &fx.farm_id,
        "P3 alava",
        1.0,
        es(Some("01"), Some("001")),
    );
    insert_plot(
        &mut conn,
        &fx.farm_id,
        "P4 valladolid",
        1.0,
        es(Some("47"), Some("001")),
    );
    // The SIGPAC reference parses its parts as numbers, so a verified plot
    // stores "10", not "010"; the catalogue is zero-padded to three.
    insert_plot(
        &mut conn,
        &fx.farm_id,
        "P5 unpadded",
        1.0,
        es(Some("34"), Some("10")),
    );
    // No province: unkeyable, so no name — never a guess.
    insert_plot(
        &mut conn,
        &fx.farm_id,
        "P6 no province",
        1.0,
        es(None, Some("001")),
    );
    // A code the snapshot cannot resolve prints alone, the `problem_code` rule.
    insert_plot(
        &mut conn,
        &fx.farm_id,
        "P7 unknown",
        1.0,
        es(Some("47"), Some("999")),
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["plot_rows"].as_array().unwrap();
    let cell = |name: &str| {
        rows.iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("no plot row named '{name}'"))["municipality"]
            .clone()
    };
    assert_eq!(cell("P3 alava"), "001 · ALEGRÍA-DULANTZI");
    assert_eq!(cell("P4 valladolid"), "001 · ADALIA");
    assert_eq!(cell("P5 unpadded"), "10 · AMPUDIA");
    assert_eq!(cell("P6 no province"), "001");
    assert_eq!(cell("P7 unknown"), "999");
}

/// Without the snapshot the book still prints — the code alone, exactly as it
/// did before this column resolved anything. A record book must never depend on
/// reference data to render.
#[test]
fn a_book_rendered_without_the_snapshot_still_prints_the_code() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plot_rows"][0]["municipality"], "186");
}

/// The spreadsheet keeps the two apart: a joined "186 · VALVERDE DE CAMPOS"
/// can be read but not filtered, which is the whole point of the second
/// renderer (the 2.2 captaciones precedent).
#[test]
fn the_workbook_gives_the_municipality_name_its_own_column() {
    use terrazgo_report::Cell;

    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);

    let book = workbook(&conn, &fx);
    let sheet = sheet(&book, "2.1 Parcelas");
    let index = |header: &str| {
        sheet
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"))
    };
    let row = &sheet.rows[0];
    assert!(matches!(&row[index("Municipio")], Cell::Text(v) if v == "186"));
    assert!(
        matches!(&row[index("Municipio (nombre)")], Cell::Text(v) if v == "VALVERDE DE CAMPOS")
    );
}

// --- Reglamento (UE) 2023/564's two conditional annex fields ----------------
// The Spanish model has no column for either, so each folds into the cell the
// annex itself places it in, and the sheet gives both a column of their own.

#[test]
fn the_register_folds_the_hour_into_the_date_and_the_bbch_stage_into_the_species() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_time = Some("20:30".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        // EST_FENOLOGICO 6 — whose BBCH principal stage is 5, not 6.
        vec![on_plot_at_stage(
            &fx.wheat_plot_id,
            Some(&fx.wheat_crop_id),
            2.5,
            "6",
        )],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let row = &doc["treatments"][0];
    assert_eq!(row["date"], "01/05/2026 · 20:30");
    // The BBCH NUMBER, never the catalogue's own code (which is 6) and never
    // FEGA's sentence-long wording, which would wrap the register to fourteen
    // lines per row. The footnote on the column says what the number is.
    assert_eq!(row["species"], "wheat · BBCH 5");
}

/// Both fields are conditional, so the ordinary record states neither — and
/// then the two cells must read exactly as they did before this existed, with
/// no stray separator (the `join_detail` rule).
#[test]
fn a_record_stating_neither_annex_field_prints_the_plain_cells() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
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
    assert_eq!(row["date"], "01/05/2026");
    assert_eq!(row["species"], "wheat");
}

/// The stage belongs to the treated crop, and one printed row can span several
/// plots that share a species and variety — so an actuation running over two
/// days can have caught them at different stages. Printing the first plot's
/// stage would state something false about the other; the cell lists both.
#[test]
fn a_row_spanning_plots_at_different_stages_prints_every_stage() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    // A second wheat plot with the SAME species and variety, so the two share
    // one crop group and therefore one printed row.
    let second_plot = insert_plot(&mut conn, &fx.farm_id, "La Vega", 3.0, None);
    let second_crop = insert_crop(
        &mut conn,
        &second_plot,
        &fx.season_id,
        "wheat",
        Some("Craklin"),
        None,
    );

    let mut new = treatment(&fx, "2026-05-01");
    new.application_end_date = Some("2026-05-02".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![
            on_plot_at_stage(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5, "5"),
            on_plot_at_stage(&second_plot, Some(&second_crop), 3.0, "6"),
        ],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let treatments = doc["treatments"].as_array().expect("a treatment array");
    // One row, because species and variety agree — the crop-group split.
    assert_eq!(treatments.len(), 1);
    // Both stages, under one "BBCH" — the row spans two plots that were at
    // different ones, and naming only the first would misstate the other.
    assert_eq!(treatments[0]["species"], "wheat · BBCH 4 / 5");
}

/// Two plots at the SAME stage state one thing, so the cell says it once.
#[test]
fn one_stage_shared_by_every_plot_prints_once() {
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    let second_plot = insert_plot(&mut conn, &fx.farm_id, "La Vega", 3.0, None);
    let second_crop = insert_crop(
        &mut conn,
        &second_plot,
        &fx.season_id,
        "wheat",
        Some("Craklin"),
        None,
    );

    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot_at_stage(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5, "6"),
            on_plot_at_stage(&second_plot, Some(&second_crop), 3.0, "6"),
        ],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["treatments"][0]["species"], "wheat · BBCH 5");
}

/// An unresolvable code still prints — the snapshot rides app releases, and a
/// record written against a later catalogue must stay readable (`problem_code`).
#[test]
fn an_unresolvable_growth_stage_still_prints_in_the_register() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        // No catalogue imported at all, so nothing can be resolved.
        vec![on_plot_at_stage(
            &fx.wheat_plot_id,
            Some(&fx.wheat_crop_id),
            2.5,
            "6",
        )],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    // The code stands in for the stage it could not resolve, rather than the
    // cell going blank on a value the farmer did record.
    assert_eq!(doc["treatments"][0]["species"], "wheat · BBCH 6");
}

/// The sheet keeps both as columns of their own, because a value folded into a
/// neighbour cannot be filtered — and the hour stays TEXT, since a local
/// wall-clock hour has no date for Excel's time type to anchor it to.
#[test]
fn the_register_sheet_gives_both_annex_fields_their_own_columns() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_time = Some("20:30".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot_at_stage(
            &fx.wheat_plot_id,
            Some(&fx.wheat_crop_id),
            2.5,
            "6",
        )],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        row[index].clone()
    };

    assert_eq!(column("Hora inicio"), Cell::Text("20:30".into()));
    // The sheet keeps FEGA's full wording, where a sentence costs nothing and
    // a reader can filter on the name.
    assert_eq!(
        column("Estado fenológico (BBCH)"),
        Cell::Text("5 · Emergencia de la inflorescencia (tallo principal)/ espigamiento".into())
    );
    // And the species column stays the species alone: the sheet has no reason
    // to fold, so it does not.
    assert_eq!(column("Especie"), Cell::Text("wheat".into()));
}

/// Unstated leaves both cells empty rather than writing anything.
#[test]
fn unstated_annex_fields_are_empty_in_the_sheet() {
    use terrazgo_report::Cell;
    let mut conn = open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let register = sheet(&book, "3.1 Tratamientos");
    let row = &register.rows[0];
    let column = |header: &str| {
        let index = register
            .columns
            .iter()
            .position(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column '{header}'"));
        row[index].clone()
    };

    // Empty, not an empty string: the engine normalises blank to a truly empty
    // cell, so a filter on the column skips these rows instead of matching "".
    assert_eq!(column("Hora inicio"), Cell::Empty);
    assert_eq!(column("Estado fenológico (BBCH)"), Cell::Empty);
}

/// §2.1 must order several crops on one plot the way the screen does.
///
/// SQLite sorts with BINARY collation, which files every accented name after
/// every unaccented one — "Álamo" lands after "Avena" because the first byte of
/// `Á` is 0xC3. The registry screen sorts the same names with `Intl.Collator`
/// (`src/lib/collate.js`), and this crate's `NameCollator` exists so a picker
/// and a printed cell cannot disagree.
///
/// Both cells are checked because they join positionally: sorting the species
/// and the varieties as two separate lists would pair a species with another
/// crop's variety.
#[test]
fn several_crops_on_one_plot_print_in_collated_order_not_byte_order() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Inserted in an order that is neither the byte order nor the collated one,
    // so passing cannot be an accident of insertion.
    let plot_id = insert_plot(&mut conn, &fx.farm_id, "Las Tres", 4.0, None);
    insert_crop(
        &mut conn,
        &plot_id,
        &fx.season_id,
        "Avena",
        Some("v-avena"),
        None,
    );
    insert_crop(
        &mut conn,
        &plot_id,
        &fx.season_id,
        "Álamo",
        Some("v-alamo"),
        None,
    );
    insert_crop(
        &mut conn,
        &plot_id,
        &fx.season_id,
        "Boj",
        Some("v-boj"),
        None,
    );

    let value = inputs(&conn, &fx);

    // Collated: Á sorts with A, so before Avena and Boj. Byte order would give
    // Avena, Boj, Álamo.
    let species: Vec<&str> = value["plot_rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["name"] == "Las Tres")
        .map(|r| r["species"].as_str().unwrap())
        .collect();
    assert_eq!(species, vec!["Álamo", "Avena", "Boj"]);

    // The joined cells of the zone half, which pair positionally.
    let zone = value["zone_rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|z| z["species"].as_str().unwrap_or_default().contains("Álamo"))
        .expect("the three-crop plot prints a zone row");
    assert_eq!(zone["species"], "Álamo; Avena; Boj");
    assert_eq!(zone["variety"], "v-alamo; v-avena; v-boj");
}
