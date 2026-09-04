// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 4 — the register of analyses.
//!
//! Two things this register asserts about itself, both pinned here. It is
//! METADATA ONLY: it records that an analysis exists and where its bulletin can
//! be found, never the bulletin. And it is CORRECTABLE in full, for the
//! treated-seed reason — none of it is a snapshot of another row, so there is
//! nothing a later edit elsewhere could silently rewrite.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::last_change;

use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;

struct Fixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
    crop_a: String,
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
    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot = |conn: &mut Connection, name: &str, area: f64| {
        repo::insert_plot(
            conn,
            NewPlot {
                farm_id: farm_id.clone(),
                name: name.into(),
                area_ha: Some(area),
                es: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let plot_a = plot(conn, "El Prado", 4.0);
    let plot_b = plot(conn, "La Loma", 3.0);

    let crop_a = repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_a.clone(),
            season_id: season.id.clone(),
            species_name: "trigo blando".into(),
            variety: Some("Nogal".into()),
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
    .unwrap()
    .id;

    Fixture {
        season_id: season.id,
        farm_id,
        plot_a,
        plot_b,
        crop_a,
    }
}

fn sample(fx: &Fixture) -> NewAnalysisRecord {
    NewAnalysisRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sampled_on: "2026-06-18".into(),
        material_kind_code: "crop".into(),
        bulletin_number: Some("B-2026/1187".into()),
        lab_name: Some("Laboratorio Agroalimentario de Castilla y León".into()),
        lab_address: Some("Ctra. Burgos km 118, Valladolid".into()),
        lab_tax_id: Some("Q4700123B".into()),
        substances_detected: Some("tebuconazol 0,02 mg/kg".into()),
        soil: Default::default(),
        notes: None,
        plots: vec![NewAnalysisPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: Some(fx.crop_a.clone()),
        }],
        // 252 is TEBUCONAZOL in the FEGA SUST_ACTIVAS catalogue.
        analysis_type_codes: vec!["pesticide_residues".into()],
        substance_codes: vec!["252".into()],
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// The model's field list for section 4: date, material analysed, the parcels
/// sampled, the bulletin number, the laboratory (name and address) and the
/// substances detected. The lab's NIF joins them from the SIEX twin
/// (`Analitica.Nif`), which the printed form has no column for.
#[test]
fn an_analysis_records_the_bulletin_the_lab_and_what_was_sampled() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();

    assert_eq!(saved.record.sampled_on, "2026-06-18");
    assert_eq!(saved.record.material_kind_code, "crop");
    assert_eq!(saved.record.bulletin_number.as_deref(), Some("B-2026/1187"));
    assert_eq!(saved.record.lab_tax_id.as_deref(), Some("Q4700123B"));
    assert_eq!(
        saved.record.substances_detected.as_deref(),
        Some("tebuconazol 0,02 mg/kg")
    );
    assert_eq!(saved.plots.len(), 1);
    assert_eq!(saved.plots[0].plot_id, fx.plot_a);

    // The sampled crop is frozen, so renaming it later cannot rewrite what the
    // printed book said was analysed.
    assert_eq!(
        saved.plots[0].crop_name_snapshot.as_deref(),
        Some("trigo blando")
    );
    assert_eq!(saved.plots[0].variety_snapshot.as_deref(), Some("Nogal"));
}

/// The material analysed is the one field beyond the date that the SIEX twin
/// makes mandatory. FOUR values, not the model's three-word hint: FEGA
/// separates the standing crop from the produce harvested off it.
#[test]
fn the_material_analysed_must_be_one_the_model_prints() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for code in ["crop", "harvested_produce", "soil", "water"] {
        let mut record = sample(&fx);
        record.material_kind_code = code.into();
        let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();
        assert_eq!(saved.record.material_kind_code, code);
    }

    let mut invented = sample(&fx);
    invented.material_kind_code = "air".into();
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, invented, None).unwrap_err(),
        module_cue::CueError::Invalid("unknown_analysis_material")
    ));
}

/// An analysis that names no parcel says nothing the book can print in the
/// "Cultivo o cosecha muestreados" column.
#[test]
fn an_analysis_needs_at_least_one_sampled_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut none = sample(&fx);
    none.plots.clear();
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, none, None).unwrap_err(),
        module_cue::CueError::Invalid("no_plots")
    ));
}

/// Sampling a parcel that belongs to another holding would put a foreign plot
/// in this farm's book.
#[test]
fn a_sampled_plot_must_be_on_the_same_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca ajena".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let foreign_plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: other_farm,
            name: "El Soto".into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let mut record = sample(&fx);
    record.plots = vec![NewAnalysisPlot {
        plot_id: foreign_plot,
        crop_id: None,
    }];
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, record, None).unwrap_err(),
        module_cue::CueError::PlotNotOnFarm { .. }
    ));
}

/// The same plot listed twice is one sample, not an error — the UNIQUE index
/// would reject the second row anyway.
#[test]
fn a_plot_listed_twice_is_folded_into_one_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut record = sample(&fx);
    record.plots = vec![
        NewAnalysisPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: Some(fx.crop_a.clone()),
        },
        NewAnalysisPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
        },
    ];
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();
    assert_eq!(saved.plots.len(), 1);
    // The first mention wins, crop link included.
    assert_eq!(saved.plots[0].crop_id.as_deref(), Some(fx.crop_a.as_str()));
}

/// A blank optional field is stored as nothing rather than as an empty string:
/// the printed cell must be blank, and blank is not the same as "".
#[test]
fn blank_optional_fields_are_stored_as_nothing() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut record = sample(&fx);
    record.bulletin_number = Some("   ".into());
    record.lab_name = Some(String::new());
    record.substances_detected = None;
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();

    assert!(saved.record.bulletin_number.is_none());
    assert!(saved.record.lab_name.is_none());
    assert!(saved.record.substances_detected.is_none());
}

#[test]
fn every_inserted_row_is_logged_with_the_actor() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), Some("carlos")).unwrap();

    let (op, before, after) = last_change(&conn, "analysis_record", &saved.record.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: the log is the future sync delta source.
    for column in [
        "id",
        "season_id",
        "farm_id",
        "sampled_on",
        "material_kind_code",
        "bulletin_number",
        "lab_name",
        "lab_address",
        "lab_tax_id",
        "substances_detected",
        "notes",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }

    let (_, _, plot) = last_change(&conn, "analysis_plot", &saved.plots[0].id);
    assert_eq!(plot["crop_name_snapshot"], "trigo blando");

    let actor: String = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1",
            [&saved.record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "carlos");
}

// ---------------------------------------------------------------------------
// Correction
//
// Fully correctable, unlike treatment_record: this register holds no snapshot
// of another row's identity, so there is nothing a later edit could rewrite.
// ---------------------------------------------------------------------------

#[test]
fn an_analysis_can_be_corrected_in_full_and_logs_both_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();

    let updated = repo::update_analysis_record(
        &mut conn,
        &saved.record.id,
        UpdateAnalysisRecord {
            sampled_on: "2026-06-20".into(),
            material_kind_code: "soil".into(),
            analysis_type_codes: vec!["soil_parameters".into()],
            substance_codes: vec![],
            bulletin_number: Some("B-2026/1188".into()),
            lab_name: Some("Laboratorio Regional".into()),
            lab_address: Some("Polígono San Cristóbal, Valladolid".into()),
            lab_tax_id: Some("Q4700999C".into()),
            substances_detected: Some("sin residuos detectados".into()),
            soil: Default::default(),
            notes: Some("Repetición del muestreo.".into()),
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.record.sampled_on, "2026-06-20");
    assert_eq!(updated.record.material_kind_code, "soil");
    assert_eq!(updated.record.lab_tax_id.as_deref(), Some("Q4700999C"));
    // The campaign and the holding are not the form's to move.
    assert_eq!(updated.record.season_id, saved.record.season_id);
    assert_eq!(updated.record.farm_id, saved.record.farm_id);

    let (op, before, after) = last_change(&conn, "analysis_record", &saved.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["bulletin_number"], "B-2026/1187");
    assert_eq!(after["bulletin_number"], "B-2026/1188");
}

/// The sampled plots are reconciled from the submitted state — added, dropped
/// and changed rows each get their own audit entry, so the log stays
/// rebuildable.
#[test]
fn correcting_the_sampled_plots_reconciles_them_and_logs_each_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();
    let original_plot_row = saved.plots[0].id.clone();

    let updated = repo::update_analysis_record(
        &mut conn,
        &saved.record.id,
        UpdateAnalysisRecord {
            sampled_on: saved.record.sampled_on.clone(),
            material_kind_code: saved.record.material_kind_code.clone(),
            analysis_type_codes: vec!["pesticide_residues".into()],
            substance_codes: vec!["252".into()],
            bulletin_number: saved.record.bulletin_number.clone(),
            lab_name: saved.record.lab_name.clone(),
            lab_address: saved.record.lab_address.clone(),
            lab_tax_id: saved.record.lab_tax_id.clone(),
            substances_detected: saved.record.substances_detected.clone(),
            soil: Default::default(),
            notes: None,
            // El Prado goes, La Loma arrives.
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_b.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.plots.len(), 1);
    assert_eq!(updated.plots[0].plot_id, fx.plot_b);

    // The dropped row is hard-deleted (it is a pure child, like an extension
    // row) and logged with a null after-image.
    let (op, _, after) = last_change(&conn, "analysis_plot", &original_plot_row);
    assert_eq!(op, "delete");
    assert!(after.is_null(), "a removed child logs a null after-image");

    let (op, _, after) = last_change(&conn, "analysis_plot", &updated.plots[0].id);
    assert_eq!(op, "insert");
    assert!(after["crop_name_snapshot"].is_null());
}

/// Re-pointing a sampled plot at a different crop re-resolves the snapshot:
/// restating the plot is the same act as re-entering it.
#[test]
fn repointing_a_sampled_plot_at_another_crop_refreshes_its_snapshot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut record = sample(&fx);
    record.plots = vec![NewAnalysisPlot {
        plot_id: fx.plot_a.clone(),
        crop_id: None,
    }];
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();
    assert!(saved.plots[0].crop_name_snapshot.is_none());
    let plot_row = saved.plots[0].id.clone();

    let updated = repo::update_analysis_record(
        &mut conn,
        &saved.record.id,
        UpdateAnalysisRecord {
            sampled_on: saved.record.sampled_on.clone(),
            material_kind_code: saved.record.material_kind_code.clone(),
            analysis_type_codes: vec!["pesticide_residues".into()],
            substance_codes: vec!["252".into()],
            bulletin_number: None,
            lab_name: None,
            lab_address: None,
            lab_tax_id: None,
            substances_detected: None,
            soil: Default::default(),
            notes: None,
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    // Same row, corrected in place — the audit trail reads as a correction.
    assert_eq!(updated.plots[0].id, plot_row);
    assert_eq!(
        updated.plots[0].crop_name_snapshot.as_deref(),
        Some("trigo blando")
    );
    let (op, _, _) = last_change(&conn, "analysis_plot", &plot_row);
    assert_eq!(op, "update");
}

#[test]
fn a_deleted_analysis_leaves_the_book_but_keeps_both_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();

    repo::soft_delete_analysis_record(&mut conn, &saved.record.id, None).unwrap();

    assert!(
        repo::list_analysis_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "analysis_record", &saved.record.id);
    assert_eq!(op, "delete");
    assert_eq!(before["bulletin_number"], "B-2026/1187");
    assert!(
        after["deleted_at"].is_string(),
        "a soft delete logs the deleted row as its after-image"
    );
}

#[test]
fn analyses_list_per_farm_and_campaign_oldest_first() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut later = sample(&fx);
    later.sampled_on = "2026-07-02".into();
    repo::insert_analysis_record(&mut conn, later, None).unwrap();
    let mut earlier = sample(&fx);
    earlier.sampled_on = "2026-05-04".into();
    repo::insert_analysis_record(&mut conn, earlier, None).unwrap();

    let rows = repo::list_analysis_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].record.sampled_on, "2026-05-04");
    assert_eq!(rows[1].record.sampled_on, "2026-07-02");

    let other_season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2026/2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    assert!(
        repo::list_analysis_records(&conn, &other_season.id, &fx.farm_id)
            .unwrap()
            .is_empty(),
        "the register is per campaign"
    );
}

// ---------------------------------------------------------------------------
// The season-deletion guard
//
// Every register is season-scoped and every record-book view is read through
// its season, so hiding the season would hide the records — including
// soft-deleted ones, whose audit history is reachable only that way.
// ---------------------------------------------------------------------------

#[test]
fn an_analysis_pins_its_season_even_after_being_deleted() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let other = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2028,
            label: "2027/2028".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());

    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
    assert!(
        !repo::season_has_records(&conn, &other.id).unwrap(),
        "the guard is per season, not global"
    );

    repo::soft_delete_analysis_record(&mut conn, &saved.record.id, None).unwrap();
    assert!(
        repo::season_has_records(&conn, &fx.season_id).unwrap(),
        "a soft-deleted record still pins its season"
    );
}

/// The gap seams 2 and 3 left: a season holding only a sowing was deletable,
/// and the sowing would have vanished from a book that is read season by
/// season. `season_has_records` answers for every register this module owns.
#[test]
fn a_sowing_alone_also_pins_its_season() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());

    repo::insert_seed_treatment(
        &mut conn,
        NewSeedTreatment {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            sown_on: "2025-11-10".into(),
            species_name: "trigo blando".into(),
            variety: None,
            crop_code: None,
            seed_quantity_kg: None,
            seed_lot: None,
            treatment_kind_code: None,
            acquired_on: None,
            sowing_record_id: None,
            product_name: "Celest Trio".into(),
            product_registration_number: None,
            product_active_substance: None,
            product_id: None,
            efficacy_code: None,
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_a.clone(),
                surface_sown_ha: 3.2,
            }],
        },
        None,
    )
    .unwrap();

    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
}

// ---------------------------------------------------------------------------
// What was looked for, and what was found (slice 8.5 commit 2)
// ---------------------------------------------------------------------------

/// The kinds of analysis are a closed list we own and map to FEGA's
/// `TIPO_ANALISIS` at export, so an invented one is refused — it could not be
/// exported and would print as nothing.
#[test]
fn the_kinds_of_analysis_must_be_ones_the_catalogue_publishes() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut record = sample(&fx);
    record.analysis_type_codes = vec!["heavy_metals".into(), "nutrients".into()];
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();
    let codes: Vec<&str> = saved
        .types
        .iter()
        .map(|t| t.analysis_type_code.as_str())
        .collect();
    assert_eq!(codes, ["heavy_metals", "nutrients"]);

    let mut invented = sample(&fx);
    invented.analysis_type_codes = vec!["astrology".into()];
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, invented, None).unwrap_err(),
        module_cue::CueError::Invalid("unknown_analysis_type")
    ));
}

/// A substance code, unlike a kind of analysis, is NOT checked against the
/// vendored snapshot: the catalogue travels with app releases and a laboratory's
/// bulletin does not wait for one. The code is stored and prints itself — the
/// `treatment_problem.problem_code` rule.
#[test]
fn a_substance_code_the_snapshot_does_not_know_is_still_recorded() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut record = sample(&fx);
    record.substance_codes = vec!["999999".into()];
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();

    assert_eq!(saved.substances.len(), 1);
    assert_eq!(saved.substances[0].substance_code, "999999");
}

/// Both junctions fold duplicates and drop blanks: a form that lists a finding
/// twice means one finding, not an error the UNIQUE index should report.
#[test]
fn repeated_and_blank_codes_fold_into_one_row_each() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut record = sample(&fx);
    record.analysis_type_codes = vec!["nutrients".into(), "nutrients".into(), "  ".into()];
    record.substance_codes = vec!["252".into(), " 252 ".into(), String::new()];
    let saved = repo::insert_analysis_record(&mut conn, record, None).unwrap();

    assert_eq!(saved.types.len(), 1);
    assert_eq!(saved.substances.len(), 1);
    assert_eq!(saved.substances[0].substance_code, "252");
}

/// Correcting the record reconciles both junctions from the submitted state,
/// each row logged on its own — the `analysis_plot` shape, so the audit trail
/// stays rebuildable.
#[test]
fn correcting_the_findings_reconciles_both_junctions_and_logs_each_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();
    let dropped_type = saved.types[0].id.clone();
    let dropped_substance = saved.substances[0].id.clone();

    let updated = repo::update_analysis_record(
        &mut conn,
        &saved.record.id,
        UpdateAnalysisRecord {
            sampled_on: saved.record.sampled_on.clone(),
            material_kind_code: saved.record.material_kind_code.clone(),
            // The residues screen turned out to be a metals bulletin.
            analysis_type_codes: vec!["heavy_metals".into()],
            substance_codes: vec![],
            bulletin_number: saved.record.bulletin_number.clone(),
            lab_name: saved.record.lab_name.clone(),
            lab_address: saved.record.lab_address.clone(),
            lab_tax_id: saved.record.lab_tax_id.clone(),
            // Which is why the free text stays: SUST_ACTIVAS has no code for
            // cadmium, so the coded list cannot carry this finding.
            substances_detected: Some("cadmio 0,05 mg/kg".into()),
            soil: Default::default(),
            notes: None,
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.types.len(), 1);
    assert_eq!(updated.types[0].analysis_type_code, "heavy_metals");
    assert!(updated.substances.is_empty());

    for (table, id) in [
        ("analysis_record_type", &dropped_type),
        ("analysis_substance", &dropped_substance),
    ] {
        let (op, _, after) = last_change(&conn, table, id);
        assert_eq!(op, "delete", "{table} row must log its removal");
        assert!(after.is_null(), "a removed child logs a null after-image");
    }
    let (op, _, after) = last_change(&conn, "analysis_record_type", &updated.types[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["analysis_type_code"], "heavy_metals");
}

/// Restating a finding that is already there keeps its row — the correction
/// reads as a correction, not as a delete and a re-insert.
#[test]
fn restating_an_unchanged_finding_keeps_its_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();

    let updated = repo::update_analysis_record(
        &mut conn,
        &saved.record.id,
        UpdateAnalysisRecord {
            sampled_on: "2026-06-19".into(),
            material_kind_code: saved.record.material_kind_code.clone(),
            analysis_type_codes: vec!["pesticide_residues".into()],
            substance_codes: vec!["252".into()],
            bulletin_number: saved.record.bulletin_number.clone(),
            lab_name: saved.record.lab_name.clone(),
            lab_address: saved.record.lab_address.clone(),
            lab_tax_id: saved.record.lab_tax_id.clone(),
            substances_detected: saved.record.substances_detected.clone(),
            soil: Default::default(),
            notes: None,
            plots: vec![NewAnalysisPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.types[0].id, saved.types[0].id);
    assert_eq!(updated.substances[0].id, saved.substances[0].id);
}

// ---------------------------------------------------------------------------
// Anexo III Parte I A.3 — the soil block (RD 1051/2022 art. 5.b makes the same
// data an input to the plan de abonado)
// ---------------------------------------------------------------------------

fn soil(sand: Option<f64>, silt: Option<f64>, clay: Option<f64>) -> SoilParameters {
    SoilParameters {
        ph: Some(6.8),
        organic_matter_pct: Some(2.1),
        available_p_mg_kg: Some(18.0),
        available_k_mg_kg: Some(240.0),
        total_n_pct: Some(0.12),
        conductivity_ds_m: Some(0.35),
        sand_pct: sand,
        silt_pct: silt,
        clay_pct: clay,
    }
}

#[test]
fn records_the_nine_soil_parameters_the_twin_carries() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.material_kind_code = "soil".into();
    new.soil = soil(Some(40.0), Some(35.0), Some(25.0));

    let detail = repo::insert_analysis_record(&mut conn, new, Some("user-1")).unwrap();
    assert_eq!(detail.record.soil.ph, Some(6.8));
    assert_eq!(detail.record.soil.organic_matter_pct, Some(2.1));
    assert_eq!(detail.record.soil.available_p_mg_kg, Some(18.0));
    assert_eq!(detail.record.soil.conductivity_ds_m, Some(0.35));
    // Texture is three fractions in the twin, not a class name.
    assert_eq!(detail.record.soil.sand_pct, Some(40.0));

    let stored = repo::get_analysis_record(&conn, &detail.record.id).unwrap();
    assert_eq!(stored.record.soil.clay_pct, Some(25.0));
}

#[test]
fn an_analysis_with_no_soil_figures_is_normal() {
    // A.3's minimums bind only one year after MAPA publishes its sampling
    // guides, and a residue bulletin has no soil figures at all.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let detail = repo::insert_analysis_record(&mut conn, sample(&fx), None).unwrap();
    assert!(detail.record.soil.is_empty());
}

#[test]
fn a_partial_bulletin_records_what_it_reported() {
    // Labs report what was asked for; a pH-and-organic-matter bulletin must
    // not be blocked on the other seven.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.soil = SoilParameters {
        ph: Some(7.4),
        organic_matter_pct: Some(1.8),
        ..Default::default()
    };

    let detail = repo::insert_analysis_record(&mut conn, new, None).unwrap();
    assert!(!detail.record.soil.is_empty());
    assert_eq!(detail.record.soil.ph, Some(7.4));
    assert!(detail.record.soil.available_p_mg_kg.is_none());
}

#[test]
fn the_three_texture_fractions_must_sum_to_one_whole() {
    // Sand, silt and clay are fractions of the same soil, so 30/30/30 is a
    // bulletin transcribed wrong rather than a soil that exists.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.soil = soil(Some(30.0), Some(30.0), Some(30.0));
    let err = repo::insert_analysis_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("invalid_soil_texture")
    ));

    // A lab's rounding must not be refused: one point of tolerance.
    let mut rounded = sample(&fx);
    rounded.soil = soil(Some(40.3), Some(34.9), Some(25.2));
    repo::insert_analysis_record(&mut conn, rounded, None).unwrap();

    // And a partial texture is not checked at all — two fractions say nothing
    // about the third.
    let mut partial = sample(&fx);
    partial.soil = soil(Some(40.0), Some(35.0), None);
    repo::insert_analysis_record(&mut conn, partial, None).unwrap();
}

#[test]
fn a_stated_figure_has_to_be_possible() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut bad_ph = sample(&fx);
    bad_ph.soil = SoilParameters {
        ph: Some(15.0),
        ..Default::default()
    };
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, bad_ph, None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_soil_ph")
    ));

    let mut bad_pct = sample(&fx);
    bad_pct.soil = SoilParameters {
        organic_matter_pct: Some(120.0),
        ..Default::default()
    };
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, bad_pct, None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_soil_percentage")
    ));

    let mut negative = sample(&fx);
    negative.soil = SoilParameters {
        available_p_mg_kg: Some(-3.0),
        ..Default::default()
    };
    assert!(matches!(
        repo::insert_analysis_record(&mut conn, negative, None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_soil_value")
    ));
}

#[test]
fn correcting_a_bulletin_corrects_its_soil_figures() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.soil = soil(None, None, None);
    let created = repo::insert_analysis_record(&mut conn, new, Some("user-1")).unwrap();

    let update = UpdateAnalysisRecord {
        sampled_on: created.record.sampled_on.clone(),
        material_kind_code: "soil".into(),
        bulletin_number: created.record.bulletin_number.clone(),
        lab_name: created.record.lab_name.clone(),
        lab_address: None,
        lab_tax_id: None,
        substances_detected: None,
        soil: SoilParameters {
            ph: Some(7.1),
            ..soil(None, None, None)
        },
        notes: None,
        plots: vec![NewAnalysisPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
        }],
        analysis_type_codes: vec![],
        substance_codes: vec![],
    };
    let detail =
        repo::update_analysis_record(&mut conn, &created.record.id, update, Some("user-1"))
            .unwrap();
    assert_eq!(detail.record.soil.ph, Some(7.1));

    let (op, before, after) = last_change(&conn, "analysis_record", &created.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["soil"]["ph"], 6.8);
    assert_eq!(after["soil"]["ph"], 7.1);
}
