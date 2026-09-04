// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! RD 1048/2022's section 9 of the book: 9.1 (pastoreo extensivo), 9.2 and the
//! book's own "9.6" (cultural operations, including anexo IV's annotation the
//! model has no page for), 9.3 (espacios de biodiversidad) and 9.4/9.5 (las
//! cubiertas).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use terrazgo_recordbook::{ReportLanguage, render_cuaderno};

// ---------------------------------------------------------------------------
// Section 9.1 — pastoreo extensivo (RD 1048/2022 art. 30.2 ter)
//
// The book's third decree. Its registers are conditional on claiming an
// ecorrégimen, which the app cannot know — so an empty section 9 is the normal
// state of most holdings and must print as a blank form rather than as an
// omission.
// ---------------------------------------------------------------------------

/// A grazing on the wheat plot, with the sheep that grazed it.
fn insert_grazing(
    conn: &mut Connection,
    fx: &Fixture,
    started: &str,
    ended: Option<&str>,
    animals: Vec<(&str, &str, i64)>,
) -> String {
    use module_ecoscheme::models::{GrazingAnimal, NewGrazingRecord};
    module_ecoscheme::repository::insert_grazing_record(
        conn,
        NewGrazingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "extensive_grazing".into(),
            plot_group_ref: None,
            soil_cover_id: None,
            started_on: started.into(),
            ended_on: ended.map(str::to_string),
            notes: None,
            plot_ids: vec![fx.wheat_plot_id.clone()],
            animals: animals
                .into_iter()
                .map(|(species, rega, count)| GrazingAnimal {
                    id: String::new(),
                    grazing_record_id: String::new(),
                    species_code: species.into(),
                    rega_code: rega.into(),
                    animal_count: count,
                })
                .collect(),
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn a_grazing_prints_the_sigpac_reference_the_model_asks_for() {
    // Model 9.1 column 2 asks for the reference itself, not the table-2.1
    // cross-reference every other register uses. The fixture plot carries all
    // seven parts, so they print joined the way the visor shows them.
    // The species prints its catalogue LABEL, so this test needs the real
    // vendored snapshot rather than the bare schema most of this file uses.
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
    insert_grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        vec![("03", "ES471860000001", 120)],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["grazing"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["plot_reference"], "47:186:0:0:5:23:1");
    assert_eq!(rows[0]["started_on"], "01/04/2026");
    assert_eq!(rows[0]["ended_on"], "15/06/2026");
    // The species is a catalogue label, resolved from ESPECIE_ANIMAL 03.
    assert_eq!(rows[0]["species"], "Ovinos");
    assert_eq!(rows[0]["rega"], "ES471860000001");
    assert_eq!(rows[0]["animal_count"], "120");
}

#[test]
fn an_unfinished_grazing_prints_a_blank_end_date() {
    // The annotation deadline runs from the END of grazing (art. 30.2 ter, and
    // the model's own footnote). So a blank end cell states "still grazing" —
    // not "the farmer forgot" — and the register must never invent one.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        None,
        vec![("03", "ES471860000001", 120)],
    );

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["grazing"][0]["ended_on"], "");
    assert_eq!(doc["grazing"][0]["started_on"], "01/04/2026");
}

#[test]
fn two_animal_groups_print_two_lines_repeating_the_grazing() {
    // The model's last three columns describe ONE group of animals while the
    // dates describe the grazing, so sheep and goats on the same pasture are
    // two lines with the same dates — the shape section 2.2's water points use.
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);
    insert_grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        vec![("03", "ES471860000001", 120), ("04", "ES471860000001", 18)],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["grazing"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row["started_on"], "01/04/2026");
        assert_eq!(row["plot_reference"], "47:186:0:0:5:23:1");
    }
    let species: Vec<&str> = rows
        .iter()
        .map(|r| r["species"].as_str().unwrap())
        .collect();
    assert_eq!(species, vec!["Ovinos", "Caprinos"]);
}

#[test]
fn a_plot_without_a_sigpac_reference_prints_its_name_instead() {
    // A reference is only meaningful whole. A plot missing parts would print
    // "47::0:0:5::", which looks like a reference and is not one — so the cell
    // falls back to the plot's name, which is a true statement.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bare_plot = insert_plot(&mut conn, &fx.farm_id, "Sin referencia", 3.0, None);
    use module_ecoscheme::models::{GrazingAnimal, NewGrazingRecord};
    module_ecoscheme::repository::insert_grazing_record(
        &mut conn,
        NewGrazingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "extensive_grazing".into(),
            plot_group_ref: Some("Grupo sur".into()),
            soil_cover_id: None,
            started_on: "2026-04-01".into(),
            ended_on: None,
            notes: None,
            plot_ids: vec![bare_plot],
            animals: vec![GrazingAnimal {
                id: String::new(),
                grazing_record_id: String::new(),
                species_code: "03".into(),
                rega_code: "ES471860000001".into(),
                animal_count: 40,
            }],
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["grazing"][0]["plot_reference"], "Sin referencia");
    assert_eq!(doc["grazing"][0]["group_ref"], "Grupo sur");
}

#[test]
fn a_book_with_no_grazing_prints_the_section_and_an_empty_register() {
    // Section 9 has no "APLICA TRATAMIENTO: SÍ/NO" box anywhere, so there is
    // nothing that could close this table: an untouched register offers its
    // ruled lines, exactly like the conditional registers of section 3 before
    // anyone declares them empty.
    let mut conn = db();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["grazing"].as_array().unwrap().len(), 0);

    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
}

// ---------------------------------------------------------------------------
// Section 9.2 and the book's "9.6" — cultural operations (RD 1048/2022
// arts. 31, 31.4.d and anexo IV)
//
// One register behind two printed pages, and the clearest case in the book for
// deriving a table from the decree: anexo IV orders an annotation the printed
// model gives no page to at all.
// ---------------------------------------------------------------------------

fn insert_operation(
    conn: &mut Connection,
    fx: &Fixture,
    practice: &str,
    kind: &str,
    performed_on: &str,
    plot_ids: Vec<String>,
) -> String {
    use module_ecoscheme::models::NewCulturalOperation;
    module_ecoscheme::repository::insert_cultural_operation(
        conn,
        NewCulturalOperation {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: practice.into(),
            operation_kind_code: kind.into(),
            performed_on: performed_on.into(),
            performed_end_date: None,
            activity_description: None,
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids,
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn section_92_prints_one_row_per_plot_with_the_parcel_columns_the_model_asks_for() {
    // The model's 9.2 row IS a plot: it carries the SIGPAC parts and the
    // surface in columns of their own, the way table 2.1 does, and then
    // accumulates dates by activity. So two operations on one plot are ONE
    // printed row, which is what makes this page a pivot of the register.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "mowing",
        "2026-05-12",
        vec![fx.wheat_plot_id.clone()],
    );
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "tillage",
        "2026-03-02",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["mowing"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        1,
        "two operations on one plot are one printed row"
    );
    // The parcel register's own parts, not the plot's name: the fixture plot
    // carries the full reference 47:186:0:0:5:23:1.
    assert_eq!(rows[0]["province"], "47");
    assert_eq!(rows[0]["municipality"], "186");
    assert_eq!(rows[0]["polygon"], "5");
    assert_eq!(rows[0]["parcel"], "23");
    assert_eq!(rows[0]["enclosure"], "1");
    assert_eq!(rows[0]["mowing"], "12/05/2026");
    assert_eq!(rows[0]["tillage"], "02/03/2026");
}

#[test]
fn the_mowing_column_carries_both_of_the_years_cuts() {
    // Model 9.2 footnote (1): up to two cuts a year. The column is a LIST by
    // design, so a second cut joins the cell rather than replacing the first or
    // opening a second row — losing either would be losing an annotation the
    // decree required within a month of the work.
    let mut conn = db();
    let fx = fixture(&mut conn);
    for date in ["2026-05-12", "2026-08-20"] {
        insert_operation(
            &mut conn,
            &fx,
            "sustainable_mowing",
            "mowing",
            date,
            vec![fx.wheat_plot_id.clone()],
        );
    }

    let doc = inputs(&conn, &fx);
    let rows = doc["mowing"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["mowing"], "12/05/2026 · 20/08/2026");
}

#[test]
fn an_operation_on_two_plots_prints_on_both_of_their_rows() {
    // One act, two parcels: the model identifies its row by parcel, so the date
    // belongs on each. Their order follows table 2.1's numbering, which is what
    // the "Id. de parcelas" column cross-references.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "mowing",
        "2026-05-12",
        vec![fx.wheat_plot_id.clone(), fx.barley_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["mowing"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let orders: Vec<&str> = rows.iter().map(|r| r["order"].as_str().unwrap()).collect();
    let mut sorted = orders.clone();
    sorted.sort_unstable();
    assert_eq!(orders, sorted, "rows follow table 2.1's order");
    for row in rows {
        assert_eq!(row["mowing"], "12/05/2026");
    }
}

#[test]
fn other_maintenance_prints_the_date_and_the_activity_while_the_named_columns_do_not() {
    // Model 9.2 footnote (4) asks the "otras actividades" column for a date AND
    // the activity; footnote (2) asks the Laboreo and Siembra columns for a
    // date alone. So the kind's name rides in one cell and not in the others —
    // printing "Siega" under a column headed "Siega" would be noise.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "brush_cutting",
        "2026-07-04",
        vec![fx.wheat_plot_id.clone()],
    );
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "mowing",
        "2026-05-12",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    let row = &doc["mowing"][0];
    assert_eq!(row["maintenance"], "04/07/2026 Desbroce");
    assert_eq!(row["mowing"], "12/05/2026");
}

#[test]
fn a_no_tillage_record_does_not_print_under_laboreo() {
    // A date under "Laboreo" states that the ground WAS worked. `no_tillage` is
    // the opposite statement, so it joins the maintenance column where its name
    // prints beside the date and the cell reads true.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "no_tillage",
        "2026-03-02",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    let row = &doc["mowing"][0];
    assert_eq!(row["tillage"], "");
    assert_eq!(row["maintenance"], "02/03/2026 Sin laboreo");
}

#[test]
fn the_free_description_joins_the_coded_kind_rather_than_replacing_it() {
    // Art. 31 says "cualquier otra actividad de mantenimiento", so anexo III.B's
    // list is open-ended and free text carries what no code can. It APPENDS, so
    // a reader always sees which kind was recorded as well.
    let mut conn = db();
    let fx = fixture(&mut conn);
    use module_ecoscheme::models::NewCulturalOperation;
    module_ecoscheme::repository::insert_cultural_operation(
        &mut conn,
        NewCulturalOperation {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "sustainable_mowing".into(),
            operation_kind_code: "weeding".into(),
            performed_on: "2026-06-01".into(),
            performed_end_date: None,
            activity_description: Some("Escarda manual del margen".into()),
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids: vec![fx.wheat_plot_id.clone()],
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(
        doc["mowing"][0]["maintenance"],
        "01/06/2026 Escarda — Escarda manual del margen"
    );
}

#[test]
fn an_operation_over_several_days_prints_the_interval() {
    // `LaboresCulturales` carries both ends and the register distinguishes a
    // one-day operation from an interval, so the printed cell must too.
    let mut conn = db();
    let fx = fixture(&mut conn);
    use module_ecoscheme::models::NewCulturalOperation;
    module_ecoscheme::repository::insert_cultural_operation(
        &mut conn,
        NewCulturalOperation {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "sustainable_mowing".into(),
            operation_kind_code: "mowing".into(),
            performed_on: "2026-05-12".into(),
            performed_end_date: Some("2026-05-14".into()),
            activity_description: None,
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids: vec![fx.wheat_plot_id.clone()],
        },
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["mowing"][0]["mowing"], "12/05/2026 – 14/05/2026");
}

#[test]
fn a_communal_pasture_operation_prints_on_the_page_the_model_does_not_have() {
    // RD 1048/2022 anexo IV: the maintenance dates of each pasto comunal plot,
    // within a month. The printed model has NO page for this — the book gives
    // it one, numbered "9.6" — so the row must land there and NOT in 9.2, whose
    // footnotes are about P2's two cuts and 300 m threshold.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "communal_pasture",
        "brush_cutting",
        "2026-09-10",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    assert!(
        doc["mowing"].as_array().unwrap().is_empty(),
        "anexo IV's duty is not P2's, and its footnotes are different"
    );
    let communal = doc["communal"].as_array().unwrap();
    assert_eq!(communal.len(), 1);
    assert_eq!(communal[0]["performed_on"], "10/09/2026");
    assert_eq!(communal[0]["performed_end_date"], "");
    assert_eq!(communal[0]["activity"], "Desbroce");
    assert_eq!(communal[0]["plots"], "El Prado");
}

#[test]
fn the_practices_later_seams_print_are_captured_and_reach_the_spreadsheet() {
    // A cover or flooded-crop operation is recordable now and prints on the
    // pages seams 3 and 4 add. Until then it must not be silently invisible:
    // the operations tab carries every row with the duty it evidences, so
    // nothing captured is unreadable in the meantime.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_operation(
        &mut conn,
        &fx,
        "plant_cover",
        "mowing",
        "2026-04-20",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    assert!(doc["mowing"].as_array().unwrap().is_empty());
    assert!(doc["communal"].as_array().unwrap().is_empty());

    let book = workbook(&conn, &fx);
    let rows = &sheet(&book, "9.2 Labores").rows;
    assert_eq!(rows.len(), 1);
    assert!(
        format!("{rows:?}").contains("Cubiertas vegetales en cultivos leñosos (P6)"),
        "the duty must be readable in the tab that carries both pages' rows"
    );
}

#[test]
fn the_operations_tab_unpivots_what_the_page_pivots() {
    // The PDF folds two cuts into one cell because the model's row is a plot;
    // a spreadsheet cell holding two dates can be read but not sorted, which is
    // the whole point of the second renderer. So the tab gets one row per
    // operation, each with a real date cell.
    let mut conn = db();
    let fx = fixture(&mut conn);
    for date in ["2026-05-12", "2026-08-20"] {
        insert_operation(
            &mut conn,
            &fx,
            "sustainable_mowing",
            "mowing",
            date,
            vec![fx.wheat_plot_id.clone()],
        );
    }

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["mowing"].as_array().unwrap().len(), 1);

    let book = workbook(&conn, &fx);
    let rows = &sheet(&book, "9.2 Labores").rows;
    assert_eq!(rows.len(), 2, "the sheet unfolds the pivot");
}

#[test]
fn a_book_with_no_operations_prints_both_registers_empty() {
    // Section 9 has no "APLICA TRATAMIENTO: SÍ/NO" box anywhere, so nothing can
    // close these tables: an untouched register offers its ruled lines.
    let mut conn = db();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["mowing"].as_array().unwrap().len(), 0);
    assert_eq!(doc["communal"].as_array().unwrap().len(), 0);

    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
}

// ---------------------------------------------------------------------------
// Section 9.3 — espacios de biodiversidad en cultivos bajo agua
// (RD 1048/2022 art. 45.2)
//
// The one page in the book that prints MORE columns than the model, and the
// only one assembled from three tables in three crates.
// ---------------------------------------------------------------------------

fn insert_sowing(
    conn: &mut Connection,
    fx: &Fixture,
    sown_on: &str,
    flooded_on: Option<&str>,
    plot_id: &str,
) -> String {
    use terrazgo_core::models::{NewSowingPlot, NewSowingRecord};
    terrazgo_core::repository::insert_sowing_record(
        conn,
        NewSowingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            kind_code: "sowing".into(),
            sown_on: sown_on.into(),
            sowing_end_date: None,
            flooded_on: flooded_on.map(str::to_string),
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: plot_id.to_string(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn section_93_prints_all_five_dates_article_452_names() {
    // The model has columns for three: siembra en seco, inundación and seca.
    // Art. 45.2 names five — "las fechas de nivelación, siembra, inundación y
    // secas, y construcción de caballones" — so the book adds the two the form
    // lacks. The layout is orientativo and the content binds (the PHI-column
    // precedent), and each of the five comes from a different place.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_sowing(
        &mut conn,
        &fx,
        "2026-04-10",
        Some("2026-05-05"),
        &fx.wheat_plot_id,
    );
    // Nivelación and caballones are cultural operations under P5.
    insert_operation(
        &mut conn,
        &fx,
        "flooded_biodiversity",
        "levelling",
        "2026-03-01",
        vec![fx.wheat_plot_id.clone()],
    );
    insert_operation(
        &mut conn,
        &fx,
        "flooded_biodiversity",
        "ridging",
        "2026-03-20",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["flooded"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "one row per plot");
    assert_eq!(rows[0]["levelling"], "01/03/2026");
    assert_eq!(rows[0]["sowing"], "10/04/2026");
    assert_eq!(rows[0]["flooding"], "05/05/2026");
    assert_eq!(rows[0]["ridging"], "20/03/2026");
}

#[test]
fn the_drying_date_reaches_93_from_the_treatment_it_served() {
    // Model 9.3's fourth column is "fecha de seca para tratamiento herbicida o
    // fitosanitario", and the twin puts `FechaSeca` on `TratamFito`. The field
    // is dried in order to spray, so the fact belongs to the treatment — and
    // 9.3 reads it across the crate boundary, which only the record book can do.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_sowing(
        &mut conn,
        &fx,
        "2026-04-10",
        Some("2026-05-05"),
        &fx.wheat_plot_id,
    );
    let mut sprayed = treatment(&fx, "2026-06-20");
    sprayed.drying_date = Some("2026-06-18".into());
    repo::insert_treatment_record(
        &mut conn,
        sprayed,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = inputs(&conn, &fx);
    let rows = doc["flooded"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["drying"], "18/06/2026");
}

#[test]
fn a_sowing_that_was_never_flooded_stays_off_the_flooded_crops_page() {
    // Otherwise every wheat sowing on the holding would land on a page about
    // rice. A sowing is evidence of a cultivo bajo agua only once it carries a
    // flooding date — the core-native marker, since core may not hold an
    // eco-scheme practice code.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_sowing(&mut conn, &fx, "2026-10-15", None, &fx.wheat_plot_id);

    let doc = inputs(&conn, &fx);
    assert!(doc["flooded"].as_array().unwrap().is_empty());
    // It is still a recorded sowing, and the register's own tab shows it.
    let book = workbook(&conn, &fx);
    assert_eq!(sheet(&book, "Siembra").rows.len(), 1);
}

#[test]
fn a_dry_sowing_prints_once_the_plot_is_known_to_grow_under_water() {
    // The gap this closes: a rice grower dry-sows in April and floods in May,
    // and art. 45.2 wants the sowing annotated within a month of the sowing.
    // For that month `flooded_on` is still NULL — so the plot enters the page
    // by OTHER evidence (here the nivelación), and once it is in, every sowing
    // on it prints its date.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_operation(
        &mut conn,
        &fx,
        "flooded_biodiversity",
        "levelling",
        "2026-03-01",
        vec![fx.wheat_plot_id.clone()],
    );
    insert_sowing(&mut conn, &fx, "2026-04-10", None, &fx.wheat_plot_id);

    let doc = inputs(&conn, &fx);
    let rows = doc["flooded"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["sowing"], "10/04/2026",
        "the dry sowing is not lost"
    );
    assert_eq!(rows[0]["flooding"], "", "and the water has not come yet");
}

#[test]
fn several_secas_in_one_campaign_are_a_list_like_the_cuts_in_92() {
    // Art. 45.2 says "las secas" — plural. A flooded crop is dried more than
    // once in a season, and each drying belongs to the treatment it served, so
    // the cell accumulates them in date order however the tables came back.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_sowing(
        &mut conn,
        &fx,
        "2026-04-10",
        Some("2026-05-05"),
        &fx.wheat_plot_id,
    );
    for date in ["2026-07-02", "2026-06-18"] {
        let mut t = treatment(&fx, "2026-08-01");
        t.drying_date = Some(date.into());
        repo::insert_treatment_record(
            &mut conn,
            t,
            vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
            None,
        )
        .unwrap();
    }

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["flooded"][0]["drying"], "18/06/2026 · 02/07/2026");
}

#[test]
fn the_siembra_column_of_92_is_fed_by_the_sowing_register() {
    // `TIPO_LABOR` publishes no siembra code, so this module's owned vocabulary
    // has none either — a sowing is its own register, in core. Model 9.2's
    // "Siembra" column therefore reads from there. Only plots already on that
    // page gain a date: a sowing on a plot with no P2 activity is not evidence
    // of sustainable mowing.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_operation(
        &mut conn,
        &fx,
        "sustainable_mowing",
        "mowing",
        "2026-05-12",
        vec![fx.wheat_plot_id.clone()],
    );
    insert_sowing(&mut conn, &fx, "2026-10-15", None, &fx.wheat_plot_id);
    // A sowing on a plot that recorded no P2 activity must not create a row.
    insert_sowing(&mut conn, &fx, "2026-10-16", None, &fx.barley_plot_id);

    let doc = inputs(&conn, &fx);
    let rows = doc["mowing"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["sowing"], "15/10/2026");
    assert_eq!(rows[0]["mowing"], "12/05/2026");
}

#[test]
fn the_sowing_tab_carries_what_no_printed_page_shows() {
    // `seed_quantity_kg` is captured only because the SIEX twin requires
    // `Cantidad`; no page of section 9 prints it. If it were not in this tab it
    // would be unreadable anywhere in the book.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_sowing(
        &mut conn,
        &fx,
        "2026-04-10",
        Some("2026-05-05"),
        &fx.wheat_plot_id,
    );

    let book = workbook(&conn, &fx);
    let rows = &sheet(&book, "Siembra").rows;
    assert_eq!(rows.len(), 1);
    assert!(
        format!("{rows:?}").contains("180"),
        "the seed quantity must be readable somewhere: {rows:?}"
    );
}

#[test]
fn the_sowing_tab_says_whether_each_row_was_sown_or_planted() {
    // The register's form is titled "Siembra y plantación" and asks the farmer
    // to note how each crop began, so a planting is its documented use — and no
    // printed page shows this register at all. Printing a planting as a sowing
    // would be a wrong statement, not a missing one.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_sowing(&mut conn, &fx, "2026-04-10", None, &fx.wheat_plot_id);
    terrazgo_core::repository::insert_sowing_record(
        &mut conn,
        terrazgo_core::models::NewSowingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            kind_code: "planting".into(),
            sown_on: "2026-02-20".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: None,
            notes: None,
            plots: vec![terrazgo_core::models::NewSowingPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    let rows = format!("{:?}", sheet(&workbook(&conn, &fx), "Siembra").rows);
    assert!(rows.contains("Siembra"), "{rows}");
    assert!(rows.contains("Plantación"), "{rows}");
}

#[test]
fn a_book_with_no_flooded_crop_prints_93_empty() {
    let mut conn = db();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert_eq!(doc["flooded"].as_array().unwrap().len(), 0);

    let pdf = render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
}

// ---------------------------------------------------------------------------
// Sections 9.4 and 9.5 — cubiertas (RD 1048/2022 arts. 42 and 43)
// ---------------------------------------------------------------------------

/// A cover over the fixture's wheat plot. Widths optional, because art. 42.1.e
/// is a SEPARATE annotation on a later deadline and a cover between the two is
/// the ordinary state of things.
fn insert_cover(
    conn: &mut Connection,
    fx: &Fixture,
    practice: &str,
    established_on: &str,
    widths: Option<(f64, f64, &str)>,
    maintenance: Vec<module_ecoscheme::models::CoverMaintenanceLine>,
) -> String {
    use module_ecoscheme::models::NewSoilCover;
    module_ecoscheme::repository::insert_soil_cover(
        conn,
        NewSoilCover {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: practice.into(),
            cover_type_code: if practice == "inert_cover" { "4" } else { "2" }.into(),
            established_on: established_on.into(),
            width_m: widths.map(|(w, _, _)| w),
            free_canopy_width_m: widths.map(|(_, c, _)| c),
            widths_stated_on: widths.map(|(_, _, on)| on.to_string()),
            notes: None,
            plot_ids: vec![fx.wheat_plot_id.clone()],
            maintenance,
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn cover_maintenance(
    kind: &str,
    performed_on: &str,
) -> module_ecoscheme::models::CoverMaintenanceLine {
    module_ecoscheme::models::CoverMaintenanceLine {
        id: String::new(),
        kind_code: kind.into(),
        performed_on: performed_on.into(),
        performed_end_date: None,
        animals: if kind == module_ecoscheme::models::GRAZING_MAINTENANCE {
            vec![module_ecoscheme::models::GrazingAnimal {
                id: String::new(),
                grazing_record_id: String::new(),
                species_code: "03".into(),
                rega_code: "ES471234560001".into(),
                animal_count: 40,
            }]
        } else {
            Vec::new()
        },
    }
}

#[test]
fn section_94_prints_one_row_per_cover_with_its_three_maintenance_columns() {
    // Unlike 9.2 and 9.3, the model's row here is the COVER, not the plot: one
    // establishment date and one pair of widths however many plots it covers,
    // so there is nothing to pivot.
    //
    // The three maintenance columns come from TWO other registers — siega and
    // desbroce are cultural operations, pastoreo is a grazing — because that is
    // what each of those activities is (RD 1048/2022 art. 42.1.c).
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_cover(
        &mut conn,
        &fx,
        "plant_cover",
        "2026-03-15",
        Some((2.0, 1.5, "2026-06-01")),
        vec![
            cover_maintenance("mowing", "2026-05-12"),
            cover_maintenance("brush_cutting", "2026-07-28"),
            cover_maintenance(module_ecoscheme::models::GRAZING_MAINTENANCE, "2026-06-03"),
        ],
    );

    let doc = inputs(&conn, &fx);
    let rows = doc["plant_covers"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "one cover is one row, whatever it carries");
    let row = &rows[0];
    assert_eq!(row["established_on"], "15/03/2026");
    assert_eq!(row["width"], "2");
    assert_eq!(row["free_canopy_width"], "1,5");
    assert_eq!(row["mowing"], "12/05/2026");
    assert_eq!(row["brush_cutting"], "28/07/2026");
    assert_eq!(row["grazing"], "03/06/2026");
    // 9.5 is the other practice's page and stays empty.
    assert!(doc["inert_covers"].as_array().unwrap().is_empty());
}

#[test]
fn a_cover_whose_widths_are_not_stated_yet_prints_blank_cells_not_zeros() {
    // Art. 42.1.e is an annotation of its own, due within the month before the
    // four-month live-cover period ends — later than 42.1.a's. So a cover
    // recorded in March and not yet measured is a COMPLETE record whose second
    // annotation is not due, and "0,00 m" would be a statement the farmer never
    // made.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_cover(
        &mut conn,
        &fx,
        "plant_cover",
        "2026-03-15",
        None,
        Vec::new(),
    );

    let doc = inputs(&conn, &fx);
    let row = &doc["plant_covers"].as_array().unwrap()[0];
    assert_eq!(row["established_on"], "15/03/2026");
    assert_eq!(row["width"], "");
    assert_eq!(row["free_canopy_width"], "");
    assert_eq!(row["mowing"], "");
}

#[test]
fn an_inert_cover_prints_on_95_and_never_on_94() {
    // One register, two pages, told apart by the practice — exactly as 9.2 and
    // the book's "9.6" are. Art. 43 asks for no maintenance, which is why that
    // page has no such columns.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_cover(
        &mut conn,
        &fx,
        "inert_cover",
        "2026-04-10",
        Some((3.0, 2.0, "2026-06-01")),
        Vec::new(),
    );

    let doc = inputs(&conn, &fx);
    assert!(doc["plant_covers"].as_array().unwrap().is_empty());
    let rows = doc["inert_covers"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["established_on"], "10/04/2026");
    assert_eq!(rows[0]["width"], "3");
}

#[test]
fn a_cover_grazing_prints_in_94s_column_and_not_on_the_91_page() {
    // The partition that keeps the book honest. A grazing over a cover is
    // art. 42.1.c's maintenance, printed as model 9.4's Pastoreo column; a
    // grazing without one is art. 30.2 ter's own duty, printed on 9.1.
    // Printing it on both pages would show a P6 cover grazing as if it were
    // extensive grazing, on a document an inspector reads.
    let mut conn = db();
    let fx = fixture(&mut conn);

    // One of each.
    insert_grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-04-20"),
        vec![("03", "ES471234560001", 120)],
    );
    insert_cover(
        &mut conn,
        &fx,
        "plant_cover",
        "2026-03-15",
        None,
        vec![cover_maintenance(
            module_ecoscheme::models::GRAZING_MAINTENANCE,
            "2026-06-03",
        )],
    );

    let doc = inputs(&conn, &fx);

    let grazing = doc["grazing"].as_array().unwrap();
    assert_eq!(
        grazing.len(),
        1,
        "only the grazing with no cover reaches 9.1"
    );
    assert_eq!(grazing[0]["started_on"], "01/04/2026");

    let cover = &doc["plant_covers"].as_array().unwrap()[0];
    assert_eq!(cover["grazing"], "03/06/2026");
}

#[test]
fn a_cover_operation_that_names_no_cover_still_reaches_the_operations_tab() {
    // The poda whose residue was triturated onto the land is what BRINGS a P7
    // cover into being (art. 43.1.a, `DEST_RES_VEG` 9) — it is filed against
    // the cover practice but maintains nothing, so it has no cell on 9.4 and
    // must not silently vanish. The workbook is where it can be read.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_operation(
        &mut conn,
        &fx,
        "inert_cover",
        "pruning",
        "2026-02-20",
        vec![fx.wheat_plot_id.clone()],
    );

    let doc = inputs(&conn, &fx);
    assert!(doc["plant_covers"].as_array().unwrap().is_empty());
    assert!(doc["inert_covers"].as_array().unwrap().is_empty());

    let book = workbook(&conn, &fx);
    assert_eq!(sheet(&book, "9.2 Labores").rows.len(), 1);
}

#[test]
fn the_covers_tab_carries_what_neither_printed_page_has_a_column_for() {
    // Three things: which practice the row evidences (the PDF answers that by
    // which page the row is on), what the cover is made of
    // (`TIPO_COBERTURA_SUELO`, which the twin sends and art. 42.1.a does not
    // ask to be annotated), and the date the widths were stated — which is what
    // separates a cover measured in June from one never measured.
    let mut conn = db();
    let fx = fixture(&mut conn);

    insert_cover(
        &mut conn,
        &fx,
        "plant_cover",
        "2026-03-15",
        Some((2.0, 1.5, "2026-06-01")),
        vec![cover_maintenance("mowing", "2026-05-12")],
    );
    insert_cover(
        &mut conn,
        &fx,
        "inert_cover",
        "2026-04-10",
        None,
        Vec::new(),
    );

    let book = workbook(&conn, &fx);
    let covers = sheet(&book, "9.4-9.5 Cubiertas");
    assert_eq!(
        covers.rows.len(),
        2,
        "one register, both practices, one tab"
    );

    let headers: Vec<&str> = covers.columns.iter().map(|c| c.header.as_str()).collect();
    assert!(headers.contains(&"Ecorrégimen"));
    assert!(headers.contains(&"Tipo de cobertura"));
    assert!(headers.contains(&"Fecha de medición de anchuras"));

    // Numbers stay numbers so the column can be summed; an unmeasured width is
    // blank, never a zero that would drag an average down.
    let measured = &covers.rows[0];
    assert!(matches!(measured[5], terrazgo_report::Cell::Number(w) if w == 2.0));
    let unmeasured = &covers.rows[1];
    assert!(matches!(unmeasured[5], terrazgo_report::Cell::Empty));
}

#[test]
fn a_book_with_no_cover_prints_94_and_95_empty() {
    // No "APLICA TRATAMIENTO: SÍ/NO" box anywhere in section 9: a farmer
    // claiming no ecorrégimen is not declaring the register empty, they are
    // outside the regime. So both tables print their six ruled lines.
    let mut conn = db();
    let fx = fixture(&mut conn);

    let doc = inputs(&conn, &fx);
    assert!(doc["plant_covers"].as_array().unwrap().is_empty());
    assert!(doc["inert_covers"].as_array().unwrap().is_empty());

    let pdf = terrazgo_recordbook::render_cuaderno(
        &conn,
        &fx.season_id,
        &fx.farm_id,
        GENERATED_ON,
        ReportLanguage::Es,
    )
    .unwrap();
    assert_eq!(pdf.warnings, Vec::<String>::new());
}
