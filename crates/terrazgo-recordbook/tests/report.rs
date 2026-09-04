// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The printed cuaderno as a DOCUMENT: the data contract `cuaderno_inputs`
//! hands the Typst template, the end-to-end PDF render, the spreadsheet
//! renderer reading the same assembly, and the model fields slice 5 added.
//!
//! The per-section register tests live in the sibling `report_*.rs` files;
//! `report_language.rs` covers the Catalan output and `advisory.rs` the
//! completeness findings. Unless a test says otherwise the book prints in
//! Castilian: those assertions are the regression guard that translating it
//! changed only its wording.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use terrazgo_core::models::FarmEsFields;
use terrazgo_recordbook::{ReportLanguage, cuaderno_inputs, render_cuaderno};

// ---------------------------------------------------------------------------
// The data contract
// ---------------------------------------------------------------------------

#[test]
fn inputs_carry_farm_identity_campaign_and_generation_date() {
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    //
    // Fourteen since 2026-08-18, when the THIRD decree's section 9
    // (ecorregímenes, RD 1048/2022) and section 10 (determinadas ayudas
    // asociadas) got their headings. Section 10 will never grow a register —
    // it redirects to sections 3, 7 and 8 — so its page carries one sentence
    // and is final. Section 9's five sub-registers land one per seam and this
    // number moves with them, which is the point of asserting it.
    assert_eq!(
        pdf.page_count, 16,
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
    let mut conn = db();
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
    let mut conn = db();
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
        pdf.page_count, 16,
        "an empty book must have the same page structure as a filled one"
    );
}

// ---------------------------------------------------------------------------
// The spreadsheet: one assembly, a second renderer
// ---------------------------------------------------------------------------

/// One tab per section of the official model, in model order — a reader moving
/// between the PDF and the workbook lands in the same place.
#[test]
fn the_workbook_carries_one_sheet_per_model_section() {
    let mut conn = db();
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
            // The third decree's first register (RD 1048/2022 art. 30.2 ter).
            "9.1 Pastoreo",
            // 9.2 and the book's "9.6" share one tab: one register, two
            // printed pages, told apart by a filterable practice column.
            "9.2 Labores",
            // Models 9.4 and 9.5 share a tab, as 9.2 and "9.6" do: one
            // register, two printed pages, told apart by the practice.
            "9.4-9.5 Cubiertas",
            // The sowing register: no printed page shows it, so this tab is
            // where it can be read whole.
            "Siembra",
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
    let fx = fixture(&mut conn);

    let book = workbook(&conn, &fx);
    assert_eq!(book.sheets.len(), 23);
    assert!(sheet(&book, "3.1 Tratamientos").rows.is_empty());
    assert!(sheet(&book, "8 Riego").rows.is_empty());
    assert!(sheet(&book, "9.1 Pastoreo").rows.is_empty());
    assert!(sheet(&book, "9.2 Labores").rows.is_empty());
    assert!(sheet(&book, "9.4-9.5 Cubiertas").rows.is_empty());
    assert!(sheet(&book, "Siembra").rows.is_empty());
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
    assert_eq!(rendered.sheet_count, 23);
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
    let fx = fixture(&mut conn);
    let doc = inputs(&conn, &fx);
    assert_eq!(doc["representative"]["name"], "");
    assert_eq!(doc["representative"]["nif"], "");
}

/// 1.2's NIF column and 1.3's acquisition date — the two cells that printed
/// permanently empty before this slice (Anexo III A.1.c and A.1.h).
#[test]
fn operator_nif_and_machinery_acquisition_date_reach_the_book() {
    let mut conn = db();
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
