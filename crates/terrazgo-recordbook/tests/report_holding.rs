// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Who and where the book says the holding is: model sections 1.4, 2.1 and
//! 2.2 — the advisory relationship, what a SIGPAC lookup contributes to the
//! printed plot table, the water points, and the municipality's code and name.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::{NewZoneFlag, PlotEsFields};

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

fn zone_flags(conn: &mut Connection, plot_id: &str, campaign: i64, flags: Vec<NewZoneFlag>) {
    terrazgo_core::repository::replace_zone_flags(conn, plot_id, campaign, "sigpac", flags, None)
        .unwrap();
}

/// Anexo III A.2.c–d: the provider's use code and official surface print
/// beside the farmer's own figure, never merged into it. An unverified plot
/// leaves both cells blank.
#[test]
fn plot_rows_carry_the_sigpac_use_code_and_official_surface() {
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
    let mut conn = db();
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
// Model 2.1: "Término municipal (código y nombre)"
// ---------------------------------------------------------------------------

/// The column asks for both, and the provider returns only the code. With the
/// snapshot imported — as a running app always has it — the name resolves and
/// joins the code in the PDF's single cell.
#[test]
fn the_municipality_prints_its_code_and_its_name() {
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db();
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

    let mut conn = db_with_catalogues();
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
