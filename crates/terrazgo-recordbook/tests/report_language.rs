// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The record book's LANGUAGE, as opposed to its layout.
//!
//! The layout is per country (the Spanish official model) and never forks; the
//! language is per region, because a co-official language must be printable
//! where it is official. Two things are worth pinning here and nowhere else:
//!
//! 1. which languages a given holding is offered (`report_languages`'s answer),
//!    driven by INE province codes;
//! 2. that a translated book translates PROSE and leaves CODES alone — the
//!    model's siglas, dose-unit symbols and FEGA catalogue labels are payload.
//!
//! `report.rs` covers the Castilian output in full; those assertions are the
//! regression guard proving this arc changed wording and nothing else.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::db;

use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::models::{FarmEsFields, NewZoneFlag, PlotEsFields};
use terrazgo_recordbook::{
    ReportLanguage, cuaderno_inputs, cuaderno_workbook, languages_for_farm, render_cuaderno,
};

const GENERATED_ON: &str = "2026-07-16";

/// A minimal but complete holding in the given province: one plot, one crop,
/// one treatment — enough for every section of the book to have a row.
struct Fixture {
    season_id: String,
    farm_id: String,
    plot_id: String,
}

fn fixture(conn: &mut Connection, province: &str) -> Fixture {
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
            name: "Mas de la Vinya".into(),
            owner_name: Some("Jordi Ferrer".into()),
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
                siex_code: None,
                province_code: Some(province.into()),
            }),
        },
        None,
    )
    .unwrap();

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Jordi Ferrer".into(),
            tax_id: None,
            licence_number: Some("ROPO-0800123".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: None,
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
            valid_from: None,
            valid_until: None,
        },
        None,
    )
    .unwrap();

    let plot_id = repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "El Camp".into(),
            area_ha: Some(2.5),
            es: Some(PlotEsFields {
                sigpac_province: Some(province.into()),
                sigpac_municipality: Some("019".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("3".into()),
                sigpac_parcel: Some("11".into()),
                sigpac_enclosure: Some("2".into()),
            }),
        },
        None,
    )
    .unwrap()
    .id;
    let crop_id = repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_id.clone(),
            season_id: season.id.clone(),
            species_name: "wheat".into(),
            variety: Some("Craklin".into()),
            production_system_code: Some("organic".into()),
            area_ha: Some(2.5),
            irrigation_code: Some("drip".into()),
            growing_environment_code: Some("greenhouse".into()),
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

    repo::insert_treatment_record(
        conn,
        NewTreatmentRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            application_date: "2026-05-01".into(),
            application_end_date: None,
            drying_date: None,
            application_time: None,
            product_id: Some(product_id),
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
            operator_id,
            machinery_id: None,
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            phi_days_used: None,
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id: plot_id.clone(),
            crop_id: Some(crop_id),
            surface_treated_ha: 2.5,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    Fixture {
        season_id: season.id,
        farm_id: farm.id,
        plot_id,
    }
}

fn inputs(conn: &Connection, fx: &Fixture, language: ReportLanguage) -> Value {
    cuaderno_inputs(conn, &fx.season_id, &fx.farm_id, GENERATED_ON, language).unwrap()
}

// ---------------------------------------------------------------------------
// Which languages a holding is offered
// ---------------------------------------------------------------------------

#[test]
fn a_holding_in_catalunya_may_print_in_either_official_language() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08"); // Barcelona
    assert_eq!(
        languages_for_farm(&conn, &fx.farm_id).unwrap(),
        vec![ReportLanguage::Es, ReportLanguage::Ca]
    );
}

#[test]
fn a_holding_in_a_castilian_only_region_is_offered_castilian_alone() {
    let mut conn = db();
    let fx = fixture(&mut conn, "47"); // Valladolid
    assert_eq!(
        languages_for_farm(&conn, &fx.farm_id).unwrap(),
        vec![ReportLanguage::Es]
    );
}

/// The province field is optional, and an unfilled form field says nothing
/// about which language the farmer may print in — so the choice stays open
/// rather than silently narrowing to Castilian.
#[test]
fn a_holding_with_no_province_anywhere_keeps_every_language_on_offer() {
    let mut conn = db();
    let season = repo::insert_season(
        &mut conn,
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
        &mut conn,
        NewFarm {
            name: "Sense província".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    let _ = season;

    assert_eq!(
        languages_for_farm(&conn, &farm.id).unwrap(),
        ReportLanguage::ALL.to_vec()
    );
}

/// The farm's registry province may be blank while its plots know exactly
/// where they are — the SIGPAC reference is the second anchor.
#[test]
fn a_plots_sigpac_province_answers_when_the_farm_block_is_blank() {
    let mut conn = db();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Mas sense registre".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "El Camp".into(),
            area_ha: Some(1.0),
            es: Some(PlotEsFields {
                sigpac_province: Some("43".into()), // Tarragona
                sigpac_municipality: Some("038".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("1".into()),
                sigpac_parcel: Some("2".into()),
                sigpac_enclosure: Some("1".into()),
            }),
        },
        None,
    )
    .unwrap();

    assert_eq!(
        languages_for_farm(&conn, &farm.id).unwrap(),
        vec![ReportLanguage::Es, ReportLanguage::Ca]
    );
}

// ---------------------------------------------------------------------------
// What translation does — and does not — touch
// ---------------------------------------------------------------------------

#[test]
fn the_catalan_book_carries_catalan_headings_and_footnotes() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    let doc = inputs(&conn, &fx, ReportLanguage::Ca);
    let labels = &doc["labels"];

    assert_eq!(labels["s1"]["title"], "1. INFORMACIÓ GENERAL");
    assert_eq!(
        labels["s21"]["title"],
        "2.1 DADES IDENTIFICATIVES I AGRONÒMIQUES DE LES PARCEL·LES"
    );
    assert_eq!(labels["s31"]["phi"], "Termini de seguretat");
    assert_eq!(labels["s31"]["date"], "Interval de dates");
    assert_eq!(labels["s31"]["total_quantity"], "Quantitat total");
    assert_eq!(labels["doc"]["campaign"], "CAMPANYA");
    // The footnote that expands the siglas is prose, so it translates.
    assert_eq!(
        labels["s21"]["note_irrigation"],
        "(SEC) secà, (ASP) aspersió, (LOC) degoteig o localitzat, (GRA) per gravetat."
    );
}

/// The closing page is pure prose, so all of it translates — including the
/// sentence that carries the duty's operative number. The article reference
/// does not: "art. 16.3 del RD 1311/2012" is a citation, not a phrase.
#[test]
fn the_annex_states_the_conservation_duty_in_the_books_language() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    assert_eq!(
        es["labels"]["annex"]["section_title"],
        "DOCUMENTACIÓN A CONSERVAR"
    );
    assert_eq!(
        ca["labels"]["annex"]["section_title"],
        "DOCUMENTACIÓ A CONSERVAR"
    );
    assert_eq!(
        ca["labels"]["annex"]["item_advice"],
        "Documentació de l'assessorament rebut."
    );

    // Both languages state the three years and cite the same article.
    for doc in [&es, &ca] {
        let retention = doc["labels"]["annex"]["retention"].as_str().unwrap();
        assert!(retention.contains("16.3"), "the citation is not translated");
    }
    assert!(
        es["labels"]["annex"]["retention"]
            .as_str()
            .unwrap()
            .contains("tres años")
    );
    assert!(
        ca["labels"]["annex"]["retention"]
            .as_str()
            .unwrap()
            .contains("tres anys")
    );
}

/// The heart of the design: prose translates, codes do not. A treatment row
/// carries both kinds, so one row proves the rule in both directions.
#[test]
fn a_translated_register_row_keeps_its_codes_and_translates_its_prose() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    let (es_row, ca_row) = (&es["treatments"][0], &ca["treatments"][0]);

    // Prose: efficacy wording and the PHI phrase.
    assert_eq!(es_row["efficacy"], "Buena");
    assert_eq!(ca_row["efficacy"], "Bona");
    assert_eq!(es_row["phi"], "21 días (hasta 22/05/2026)");
    assert_eq!(ca_row["phi"], "21 dies (fins al 22/05/2026)");

    // Codes and figures: identical in both books.
    for key in ["date", "surface", "product", "reg_no", "dose", "plots"] {
        assert_eq!(es_row[key], ca_row[key], "'{key}' must not translate");
    }
    // The dose unit is a symbol, not a word.
    assert_eq!(ca_row["dose"], "1,5 L/ha");
    // Dates keep dd/mm/yyyy and numbers the decimal comma in both languages.
    assert_eq!(ca_row["date"], "01/05/2026");
    assert_eq!(ca_row["surface"], "2,5");
}

#[test]
fn plot_rows_keep_the_models_siglas_in_the_catalan_book() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    let row = &ca["plot_rows"][0];

    // 2.1 footnotes 1, 3 and 4: siglas are the form's own codes.
    assert_eq!(row["irrigation"], "LOC");
    assert_eq!(row["environment"], "INV");
    assert_eq!(row["gip"], "AE");
    // User data is never translated either.
    assert_eq!(row["species"], "wheat");
    assert_eq!(row["variety"], "Craklin");
}

/// 2.2's summary is assembled as values, so the checked negative — the proof
/// that the question was asked — reads correctly in either language.
#[test]
fn the_zone_check_summary_reads_in_the_books_language() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    terrazgo_core::repository::replace_zone_flags(
        &mut conn,
        &fx.plot_id,
        2026,
        "sigpac",
        vec![NewZoneFlag {
            zone_type_code: "nitrate_vulnerable".into(),
            status: "outside".into(),
            coverage_pct: None,
            detail: None,
        }],
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    assert_eq!(es["zone_rows"][0]["checked"], "Sin afección — campaña 2026");
    assert_eq!(
        ca["zone_rows"][0]["checked"],
        "Sense afectació — campanya 2026"
    );
    // SÍ/NO happen to coincide; assert it rather than assume it.
    assert_eq!(es["zone_rows"][0]["fully"], "NO");
    assert_eq!(ca["zone_rows"][0]["fully"], "NO");
}

/// The water half's stated negative is prose, so it translates — and the
/// farmer's own name for the wellhead is user data, so it never does.
#[test]
fn the_water_negative_translates_but_the_farmers_own_name_does_not() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    terrazgo_core::repository::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None)
        .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    assert_eq!(
        es["zone_rows"][0]["denomination"],
        "Sin captaciones — 12/05/2026"
    );
    assert_eq!(
        ca["zone_rows"][0]["denomination"],
        "Sense captacions — 12/05/2026"
    );

    // Withdraw it by recording a point, then check the name survives both books.
    terrazgo_core::repository::insert_water_point(
        &mut conn,
        terrazgo_core::models::NewWaterPoint {
            plot_id: fx.plot_id.clone(),
            denomination: "Pou de la casa".into(),
            inside_plot: true,
            distance_m: None,
            latitude: None,
            longitude: None,
        },
        None,
    )
    .unwrap();
    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    assert_eq!(es["zone_rows"][0]["denomination"], "Pou de la casa");
    assert_eq!(ca["zone_rows"][0]["denomination"], "Pou de la casa");
    assert_eq!(es["zone_rows"][0]["water_point"], "SÍ");
    assert_eq!(ca["zone_rows"][0]["water_point"], "SÍ");
}

#[test]
fn the_catalan_workbook_names_its_tabs_and_headers_in_catalan() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    let book = cuaderno_workbook(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Ca,
    )
    .unwrap();

    let names: Vec<&str> = book.sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "1.1 Explotació",
            "1.2 Persones",
            "1.3 Equips",
            "1.4 Assessorament",
            "2.1 Parcel·les",
            "2.2 Mediambiental",
            "2.2 Captacions",
            "3.1 Tractaments",
            "3.2 Llavor tractada",
            "3.3 Postcollita",
            "3.4 Locals",
            "3.5 Transport",
            "4 Anàlisis",
            "4 Sòl",
            "5 Collita",
            "6 Fertilització",
            "6 Materials",
            "7.1 Pla d'adobat",
            "8 Reg",
            "9.1 Pasturatge",
            "9.2 Feines",
            "9.4-9.5 Cobertes",
            "Sembra",
        ]
    );

    let register = book
        .sheets
        .iter()
        .find(|s| s.name == "3.1 Tractaments")
        .unwrap();
    let headers: Vec<&str> = register.columns.iter().map(|c| c.header.as_str()).collect();
    assert!(
        headers.contains(&"Termini de seguretat (dies)"),
        "{headers:?}"
    );
    assert!(headers.contains(&"Collita permesa des de"), "{headers:?}");
    // The dose splits into value and unit in both languages — a sheet must be
    // summable, and "1,5 L/ha" in one cell is not.
    assert!(headers.contains(&"Unitat de dosi"), "{headers:?}");
}

/// The whole point of one template for every language: a missing or misspelled
/// label field would surface as a Typst warning or a compile error, never as a
/// silently blank heading.
#[test]
fn every_language_renders_a_pdf_with_zero_template_warnings() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    for language in ReportLanguage::ALL {
        let pdf =
            render_cuaderno(&conn, &fx.season_id, &fx.farm_id, GENERATED_ON, language).unwrap();
        assert_eq!(
            pdf.warnings,
            Vec::<String>::new(),
            "template warnings printing in '{}'",
            language.code()
        );
        assert!(!pdf.bytes.is_empty());
        // Every language prints the SAME book, not merely a valid one. Catalan
        // prose runs longer than Castilian in places, and a heading or footnote
        // that grew enough to push a table onto another page would give the two
        // versions different page numbering — which matters, because an
        // inspector is handed one of them and the footer says "hoja n de N".
        assert_eq!(
            pdf.page_count,
            16,
            "the book reflowed in '{}' — a longer translation moved a page break",
            language.code()
        );
    }
}

/// The Anexo III B additions follow the same rule as the rest of the row: the
/// interval's separator and the unit symbol are notation, so they are identical
/// in both books, while the footnote explaining them is prose and translates.
#[test]
fn the_interval_and_the_total_are_notation_but_their_footnotes_translate() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");

    // A second record, this one spanning days and stating its total.
    let product_id: String = conn
        .query_row("SELECT id FROM product LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let operator_id: String = conn
        .query_row("SELECT id FROM operator LIMIT 1", [], |r| r.get(0))
        .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        NewTreatmentRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            application_date: "2026-06-01".into(),
            application_end_date: Some("2026-06-03".into()),
            application_time: None,
            drying_date: None,
            product_id: Some(product_id),
            country_code: None,
            dose_value: Some(1.5),
            dose_unit_code: Some("l_ha".into()),
            total_quantity_value: Some(9.0),
            total_quantity_unit_code: Some("l".into()),
            target_organism: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "disease".into(),
                problem_code: "254".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: Some("good".into()),
            operator_id,
            machinery_id: None,
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            phi_days_used: None,
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id: fx.plot_id.clone(),
            crop_id: None,
            surface_treated_ha: 2.5,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);
    // Chronological: the June record is second.
    let (es_row, ca_row) = (&es["treatments"][1], &ca["treatments"][1]);

    assert_eq!(es_row["date"], "01/06/2026 – 03/06/2026");
    assert_eq!(ca_row["date"], es_row["date"], "an interval is notation");
    assert_eq!(es_row["total_quantity"], "9 L");
    assert_eq!(
        ca_row["total_quantity"], es_row["total_quantity"],
        "a unit symbol is notation"
    );
    // The plazo counts from the interval's end in both books.
    assert_eq!(es_row["phi"], "21 días (hasta 24/06/2026)");
    assert_eq!(ca_row["phi"], "21 dies (fins al 24/06/2026)");

    // The footnotes that explain them are prose.
    assert_ne!(
        es["labels"]["s31"]["note_date"],
        ca["labels"]["s31"]["note_date"]
    );
    assert_ne!(
        es["labels"]["s31"]["note_total_quantity"],
        ca["labels"]["s31"]["note_total_quantity"]
    );
}

/// Sections 3.3/3.4/3.5 follow the same rule as the rest of the book: the
/// headings and the register's own wording translate, while a unit symbol and
/// the farmer's description of what was treated do not.
#[test]
fn the_non_field_registers_translate_their_headings_but_not_their_data() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");

    let product_id: String = conn
        .query_row("SELECT id FROM product LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let operator_id: String = conn
        .query_row("SELECT id FROM operator LIMIT 1", [], |r| r.get(0))
        .unwrap();
    repo::insert_non_field_treatment(
        &mut conn,
        NewNonFieldTreatment {
            premises_id: None,
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            country_code: None,
            subject_kind_code: "postharvest".into(),
            treated_on: "2026-08-20".into(),
            subject_description: "Blat tou de la collita 2026".into(),
            subject_product_code: None,
            treated_quantity_value: Some(120.0),
            treated_quantity_unit_code: Some("t".into()),
            product_id,
            product_quantity_value: Some(3.0),
            product_quantity_unit_code: Some("kg".into()),
            operator_id,
            machinery_id: None,
            advisor_id: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "disease".into(),
                problem_code: "254".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: Some("good".into()),
            notes: None,
        },
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);

    // Headings and the register's own wording translate.
    assert_eq!(
        ca["labels"]["s33"]["title_postharvest"],
        "3.3 REGISTRE DE TRACTAMENTS POSTCOLLITA"
    );
    assert_eq!(ca["labels"]["s33"]["applies"], "APLICA TRACTAMENT");
    assert_eq!(
        ca["labels"]["s33"]["subject_storage"],
        "Local tractat (tipus i adreça)"
    );
    assert_ne!(
        es["labels"]["s33"]["note_applies"],
        ca["labels"]["s33"]["note_applies"]
    );

    // The row itself is notation and user data: identical in both books.
    let (es_row, ca_row) = (
        &es["non_field"][0]["rows"][0],
        &ca["non_field"][0]["rows"][0],
    );
    for key in ["date", "subject", "quantity", "product_quantity"] {
        assert_eq!(es_row[key], ca_row[key], "'{key}' must not translate");
    }
    assert_eq!(ca_row["quantity"], "120 t");
    assert_eq!(ca_row["subject"], "Blat tou de la collita 2026");
    // Efficacy is prose, so it does translate.
    assert_eq!(es_row["efficacy"], "Buena");
    assert_eq!(ca_row["efficacy"], "Bona");
    // And the SÍ box is ticked in both.
    assert_eq!(ca["non_field"][0]["applies_yes"], "X");
}

/// Section 3.2 follows the book's rule too: the heading and the seed-lot
/// footnote are prose and translate, the lot itself and the product's printed
/// name are data and do not.
#[test]
fn the_seed_register_translates_its_headings_but_not_the_sack_label() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    repo::insert_seed_treatment(
        &mut conn,
        NewSeedTreatment {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            sown_on: "2025-11-10".into(),
            species_name: "blat tou".into(),
            variety: Some("Nogal".into()),
            crop_code: None,
            seed_quantity_kg: Some(680.0),
            seed_lot: Some("L-2025-4471".into()),
            treatment_kind_code: Some("processing_centre".into()),
            acquired_on: None,
            sowing_record_id: None,
            product_name: "Celest Trio".into(),
            product_registration_number: Some("ES-24.876".into()),
            product_active_substance: None,
            product_id: None,
            efficacy_code: Some("good".into()),
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_id.clone(),
                surface_sown_ha: 2.5,
            }],
        },
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);

    assert_eq!(
        ca["labels"]["s32"]["title"],
        "3.2 REGISTRE D'ÚS DE LLAVOR TRACTADA"
    );
    assert_eq!(ca["labels"]["s32"]["seed_lot"], "Núm. de lot");
    assert_ne!(
        es["labels"]["s32"]["note_seed_lot"],
        ca["labels"]["s32"]["note_seed_lot"]
    );

    let (es_row, ca_row) = (&es["seed"][0], &ca["seed"][0]);
    for key in ["date", "seed_lot", "reg_no", "seed_quantity", "surface"] {
        assert_eq!(es_row[key], ca_row[key], "'{key}' must not translate");
    }
    assert_eq!(ca_row["seed_lot"], "L-2025-4471");
    // The product cell carries the sack's own text UNTRANSLATED, plus the
    // treatment kind, which is one of our own coded lists and therefore prose.
    assert_eq!(
        es_row["product"],
        "Celest Trio · Tratada en un centro de acondicionamiento"
    );
    assert_eq!(
        ca_row["product"],
        "Celest Trio · Tractada en un centre de condicionament"
    );
    // Efficacy is prose.
    assert_eq!(es_row["efficacy"], "Buena");
    assert_eq!(ca_row["efficacy"], "Bona");
}

/// Sections 4 and 5 follow the book's rule as well: headings and footnotes are
/// prose and translate; the bulletin number, the laboratory's own name, the
/// buyer, the lot and the unit symbol are data and do not.
#[test]
fn the_analysis_and_harvest_registers_translate_headings_but_not_the_record() {
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    repo::insert_analysis_record(
        &mut conn,
        NewAnalysisRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            sampled_on: "2026-06-18".into(),
            material_kind_code: "soil".into(),
            bulletin_number: Some("B-2026/1187".into()),
            lab_name: Some("Laboratori Agroalimentari".into()),
            lab_address: None,
            lab_tax_id: None,
            substances_detected: Some("Sense residus".into()),
            soil: Default::default(),
            notes: None,
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_id.clone(),
                crop_id: None,
            }],
            analysis_type_codes: vec!["soil_parameters".into()],
            substance_codes: vec![],
        },
        None,
    )
    .unwrap();
    terrazgo_core::repository::insert_harvest_record(
        &mut conn,
        terrazgo_core::models::NewHarvestRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            harvested_on: "2026-07-24".into(),
            product_name: "Blat tou".into(),
            plant_product_code: None,
            quantity_value: Some(42.5),
            quantity_unit_code: Some("t".into()),
            delivery_note_ref: Some("ALB-2026/318".into()),
            lot_number: Some("L-26-07".into()),
            buyer_name: "Cooperativa Cerealista del Duero".into(),
            buyer_tax_id: None,
            buyer_address: None,
            buyer_registry_number: Some("21.0012345/VA".into()),
            notes: None,
            plots: vec![terrazgo_core::models::NewHarvestPlot {
                plot_id: fx.plot_id.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);

    assert_eq!(
        ca["labels"]["s4"]["section_title"],
        "4. REGISTRE D'ANÀLISIS"
    );
    assert_eq!(
        ca["labels"]["s5"]["section_title"],
        "5. REGISTRE DE COLLITA COMERCIALITZADA"
    );
    assert_eq!(ca["labels"]["s5"]["buyer_registry"], "Núm. RGSEAA");
    assert_ne!(
        es["labels"]["s4"]["note_keep"],
        ca["labels"]["s4"]["note_keep"]
    );

    // "Material analizado" and the kinds of analysis folded in behind it are
    // closed lists the book prints as prose, so they translate — unlike the
    // siglas, which are catalogue payload.
    assert_eq!(
        es["analysis"][0]["material"],
        "Suelo · Parámetros del suelo"
    );
    assert_eq!(ca["analysis"][0]["material"], "Sòl · Paràmetres del sòl");

    for key in ["date", "bulletin", "laboratory", "substances"] {
        assert_eq!(
            es["analysis"][0][key], ca["analysis"][0][key],
            "'{key}' must not translate"
        );
    }
    for key in [
        "date",
        "product",
        "quantity",
        "lot",
        "buyer",
        "buyer_registry",
    ] {
        assert_eq!(
            es["harvest"][0][key], ca["harvest"][0][key],
            "'{key}' must not translate"
        );
    }
    // The unit symbol is a symbol, not a word.
    assert_eq!(ca["harvest"][0]["quantity"], "42,5 t");
}

/// Section 6 translates its prose and keeps the model's siglas, exactly as the
/// rest of the book does — the letters (F)/(AF)/(AC) are the model's own
/// notation, so they print the same in every language while the wording behind
/// them does not.
#[test]
fn the_fertilisation_register_translates_its_prose_but_not_the_models_siglas() {
    use module_fertilisation::models::{
        MaterialNutrient, NewFertilisationPlot, NewFertilisationRecord, NewFertiliserMaterial,
    };
    let mut conn = db();
    let fx = fixture(&mut conn, "08");
    let material_id = module_fertilisation::repository::insert_fertiliser_material(
        &mut conn,
        NewFertiliserMaterial {
            name: "Purí de porcí".into(),
            material_code: "3".into(),
            material_detail_code: None,
            supplier_name: Some("Granja de Cal Met".into()),
            supplier_rega: Some("ES080900000042".into()),
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: Some("composting".into()),
            density_kg_l: Some(1.02),
            notes: None,
            nutrients: vec![MaterialNutrient {
                id: String::new(),
                kind_code: "macro".into(),
                nutrient_code: "1".into(),
                percentage: 0.4,
            }],
        },
        None,
    )
    .unwrap()
    .material
    .id;
    module_fertilisation::repository::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            applied_on: "2026-02-20".into(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "fertigation_localised".into(),
            dose_value: 25.0,
            dose_unit_code: "m3_ha".into(),
            fertiliser_material_id: material_id,
            sludge_application: false,
            sustainable_input_management: false,
            irrigation_record_id: None,
            machinery_id: None,
            service_company: None,
            service_regfer_number: None,
            delivery_note_ref: Some("ALB-2026/44".into()),
            yield_estimated_kg_ha: Some(7000.0),
            yield_final_kg_ha: None,
            notes: None,
            plots: vec![NewFertilisationPlot {
                plot_id: fx.plot_id.clone(),
                crop_id: None,
                fertilised_area_ha: Some(3.0),
            }],
            practices: vec![],
        },
        None,
    )
    .unwrap();

    let es = inputs(&conn, &fx, ReportLanguage::Es);
    let ca = inputs(&conn, &fx, ReportLanguage::Ca);

    assert_eq!(
        ca["labels"]["s6"]["section_title"],
        "6. REGISTRE DE FERTILITZACIÓ"
    );
    assert_eq!(ca["labels"]["s6"]["delivery_note"], "Núm. d'albarà");
    assert_ne!(
        es["labels"]["s6"]["note_kind"],
        ca["labels"]["s6"]["note_kind"]
    );

    // The tipo and the forma de aplicación are closed lists the book prints as
    // prose, so they translate; the model's sigla in front of them does not.
    assert_eq!(
        es["fertilisation"][0]["kind"],
        "(F/AC) · Abonado de cobertera · Riego localizado (fertirrigación)"
    );
    assert_eq!(
        ca["fertilisation"][0]["kind"],
        "(F/AC) · Adobat de cobertora · Reg localitzat (fertirrigació)"
    );

    // The record itself is the farmer's data and the unit is notation: neither
    // translates.
    for key in [
        "dates",
        "dose",
        "delivery_note",
        "richness",
        "yield_estimated",
    ] {
        assert_eq!(
            es["fertilisation"][0][key], ca["fertilisation"][0][key],
            "'{key}' must not translate"
        );
    }
    assert_eq!(ca["fertilisation"][0]["dose"], "25 m³/ha");
    // The material's own name is the farmer's, whatever language it is in.
    assert_eq!(
        ca["fertilisation"][0]["material"],
        es["fertilisation"][0]["material"]
    );
}
