// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! RD 1051/2022's three sections of the book: 6 (the fertilisation register,
//! art. 5.d), 7.1 (the plan de abonado as art. 5.a records it, not as art. 6
//! defines it) and 8 (the irrigation register, art. 5.e).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_recordbook::ReportLanguage;

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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let conn = db();
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
            // BOTH counts: a counted unit inflects, and a `Plural` half left
            // holding the code would only show at one of them.
            for language in ReportLanguage::ALL {
                let labels = language.labels();
                for count in [1.0, 4.0] {
                    assert_ne!(
                        labels.intensity_unit(&code, count),
                        code,
                        "intensity unit '{code}' has no word at {count} \
                         — add an arm to Labels::intensity_unit"
                    );
                }
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
            sustainable_input_management: false,
            irrigation_record_id: None,
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
    // The coded material kind only resolves to prose against the vendored
    // catalogue; without it the code prints itself, which is the rule and has
    // its own test below.
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
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
    let mut conn = db();
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
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
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
    assert_eq!(row[14], Cell::Text("Abonado de cobertera".into()));
    assert_eq!(
        row[15],
        Cell::Text("Riego localizado (fertirrigación)".into())
    );
    // Real numbers, and the dose's unit in a column of its own so the figures
    // stay summable.
    assert_eq!(row[12], Cell::Number(250.0));
    assert_eq!(row[13], Cell::Text("kg/ha".into()));
    assert_eq!(row[9], Cell::Number(27.0));
    // K₂O was never declared: blank, never zero.
    assert_eq!(row[11], Cell::Empty);
    // The two twin-only booleans no printed cell carries, each answered.
    assert_eq!(row[7], Cell::Text("NO".into())); // lodos
    assert_eq!(row[8], Cell::Text("NO".into())); // gestión sostenible de insumos
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
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
    let material_id = nac27(&mut conn);
    let created = fertilise(&mut conn, &fx, &material_id, "top_dressing", "broadcast");
    conn.execute(
        "INSERT INTO fertilisation_practice (id, fertilisation_record_id, practice_code)
         VALUES ('p1', ?1, '3')",
        [&created.record.id],
    )
    .unwrap();

    let book = workbook(&conn, &fx);
    let practices = match &sheet(&book, "6 Fertilización").rows[0][22] {
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
    let mut conn = db();
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
            sustainable_input_management: false,
            irrigation_record_id: None,
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
    let mut conn = db();
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
    let mut conn = db();
    let fx = fixture(&mut conn);
    let with_density = slurry(&mut conn, Some(1.02));
    dose(&mut conn, &fx, &with_density, 25.0, "m3_ha", "2025-10-14");

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plan_rows"][0]["supplied"], "107,1 / — / —");

    let mut conn = db();
    let fx = fixture(&mut conn);
    let without = slurry(&mut conn, None);
    dose(&mut conn, &fx, &without, 25.0, "m3_ha", "2025-10-14");

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["plan_rows"][0]["supplied"], "— / — / —");
}

#[test]
fn the_accumulated_column_is_a_running_sum_per_production_unit() {
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
    let fx = fixture(&mut conn);
    plan_for(&mut conn, &fx, &fx.wheat_crop_id, 140.0);

    // The table's rows ARE the applications: a plan on its own recommends
    // something that has not been acted on yet, and the form is blank.
    let doc = inputs(&conn, &fx);
    assert!(doc["plan_rows"].as_array().unwrap().is_empty());
}
