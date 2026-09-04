// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the book records about samples and produce: model section 4
//! (análisis), section 5 (cosecha comercializada) and Anexo III A.3's soil
//! block, which rides on the analysis register rather than a page of its own.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;

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
    // With the catalogue snapshot imported, as a running app always has it:
    // the coded substance is stored as a number and printed as a name.
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
