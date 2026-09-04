// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The treatment registers of the printed book: model 3.1 (the actuation
//! interval and the total used), 3.2 (semilla tratada), 3.3/3.4/3.5 (the
//! non-field registers), 3.1 bis (the advised cut) and the two conditional
//! fields Reglamento (UE) 2023/564's annex adds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;

// ---------------------------------------------------------------------------
// 3.1 — the actuation interval and the total used (Anexo III Parte I B)
// ---------------------------------------------------------------------------

/// The model's 3.1 date column is an "intervalo de fechas". A treatment that
/// ran over several days prints both ends; the PHI phrase counts from the last
/// one, so the printed book and the stored derivation agree.
#[test]
fn the_register_prints_a_date_interval_and_counts_the_phi_from_its_end() {
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
        premises_id: None,
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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

/// The register prints the sowing, cross-referencing plots by the order numbers
/// of table 2.1 — the model's "Id. parcelas", the same convention 3.1 uses.
#[test]
fn the_seed_register_prints_the_sowing_and_cross_references_its_plots() {
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
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

// --- Reglamento (UE) 2023/564's two conditional annex fields ----------------
// The Spanish model has no column for either, so each folds into the cell the
// annex itself places it in, and the sheet gives both a column of their own.

#[test]
fn the_register_folds_the_hour_into_the_date_and_the_bbch_stage_into_the_species() {
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db();
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
