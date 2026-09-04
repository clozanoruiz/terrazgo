// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model 9.1 — the extensive-grazing register.
//!
//! Every rule pinned here comes from RD 1048/2022 art. 30.2 ter, the printed
//! model's own 9.1 footnotes, or the SIEX `Pastoreo` block; each test names
//! which.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{CoreFixture, FarmWithPlots, PlotSpec, farm_with_plots};
use module_ecoscheme::models::*;
use module_ecoscheme::open_in_memory;
use module_ecoscheme::repository as repo;
use rusqlite::Connection;

fn fixture(conn: &mut Connection) -> CoreFixture {
    farm_with_plots(
        conn,
        FarmWithPlots {
            farm_name: "Dehesa de Arriba".into(),
            plot_a: PlotSpec::new("Pasto Alto", 22.0),
            plot_b: PlotSpec::new("Pasto Bajo", 18.0),
            other_farm_plot: PlotSpec::new("Ajena", 5.0),
            ..Default::default()
        },
    )
}

fn animal(species: &str, rega: &str, count: i64) -> GrazingAnimal {
    GrazingAnimal {
        id: String::new(),
        grazing_record_id: String::new(),
        species_code: species.into(),
        rega_code: rega.into(),
        animal_count: count,
    }
}

fn sample(fx: &CoreFixture) -> NewGrazingRecord {
    NewGrazingRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "extensive_grazing".into(),
        plot_group_ref: None,
        soil_cover_id: None,
        started_on: "2026-04-01".into(),
        ended_on: Some("2026-06-15".into()),
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
        // ESPECIE_ANIMAL 03 = Ovinos.
        animals: vec![animal("03", "ES071234560001", 120)],
    }
}

#[test]
fn a_grazing_is_stored_with_its_plots_and_animals() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.plot_a.clone(), fx.plot_b.clone()];
    new.animals = vec![
        animal("03", "ES071234560001", 120),
        animal("04", "ES071234560001", 15), // Caprinos, same holding
    ];
    let detail = repo::insert_grazing_record(&mut conn, new, Some("tester")).unwrap();

    assert_eq!(detail.plots.len(), 2);
    assert_eq!(detail.animals.len(), 2);
    assert_eq!(detail.record.started_on, "2026-04-01");
    assert_eq!(detail.record.ended_on.as_deref(), Some("2026-06-15"));

    let read_back = repo::get_grazing_record(&conn, &detail.record.id).unwrap();
    assert_eq!(read_back.animals.len(), 2);
    assert_eq!(read_back.animals[0].animal_count, 120);
}

#[test]
fn an_open_grazing_leaves_the_end_null_rather_than_repeating_the_start() {
    // Art. 30.2 ter's one-month deadline runs from the date being annotated,
    // and the model's 9.1 footnote counts it from the END of grazing. So "still
    // grazing" must be storable and must not read as "grazed for one day":
    // those are different statements, and only one of them starts a clock.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.ended_on = None;
    let detail = repo::insert_grazing_record(&mut conn, new, None).unwrap();
    assert_eq!(detail.record.ended_on, None);
}

#[test]
fn a_grazing_cannot_end_before_it_starts() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.started_on = "2026-06-15".into();
    new.ended_on = Some("2026-04-01".into());
    let err = repo::insert_grazing_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_ecoscheme::EcoschemeError::Invalid("invalid_date_interval")
    ));
}

#[test]
fn only_the_practices_a_grazing_can_evidence_are_accepted() {
    // A grazing evidences P1 (art. 30.2 ter), P2's maintenance activities
    // (art. 31) or a comunal pasture's (anexo IV). Recorded against a cover
    // practice it would be a different register — and, concretely, it would
    // print on the wrong page of section 9.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for code in [
        "extensive_grazing",
        "sustainable_mowing",
        "communal_pasture",
        // RD 1048/2022 art. 42.1.c counts pastoreo as one of the three ways a
        // live cover is maintained, and model 9.4 prints it as a column — so a
        // grazing over a cover is a P6 grazing, not a P1 one.
        "plant_cover",
    ] {
        let mut new = sample(&fx);
        new.practice_code = code.into();
        assert!(
            repo::insert_grazing_record(&mut conn, new, None).is_ok(),
            "{code} must be recordable as a grazing"
        );
    }

    // Art. 43 asks for no maintenance of an inert cover, and a flooded crop is
    // not grazed.
    for code in ["inert_cover", "flooded_biodiversity"] {
        let mut new = sample(&fx);
        new.practice_code = code.into();
        let err = repo::insert_grazing_record(&mut conn, new, None).unwrap_err();
        assert!(
            matches!(
                err,
                module_ecoscheme::EcoschemeError::Invalid("practice_not_grazing")
            ),
            "{code} must not be recordable as a grazing"
        );
    }
}

#[test]
fn a_grazing_needs_a_plot_and_an_animal() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut no_plots = sample(&fx);
    no_plots.plot_ids = vec![];
    assert!(matches!(
        repo::insert_grazing_record(&mut conn, no_plots, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("no_plots")
    ));

    let mut no_animals = sample(&fx);
    no_animals.animals = vec![];
    assert!(matches!(
        repo::insert_grazing_record(&mut conn, no_animals, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("no_animals")
    ));
}

#[test]
fn a_plot_must_be_on_the_farm_that_grazed_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.other_farm_plot.clone()];
    assert!(matches!(
        repo::insert_grazing_record(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn an_animal_line_needs_a_species_a_rega_and_a_positive_count() {
    // `Pastoreo.Animales[]` is {REGA, Numero, Especie}: a line missing any of
    // the three states nothing. The count is where a typo does real harm — the
    // page reports how many animals were moved onto the pasture.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let cases = [
        (animal("", "ES071234560001", 10), "incomplete_animal_line"),
        (animal("03", "  ", 10), "incomplete_animal_line"),
        (
            animal("03", "ES071234560001", 0),
            "nonpositive_animal_count",
        ),
        (
            animal("03", "ES071234560001", -4),
            "nonpositive_animal_count",
        ),
    ];
    for (line, expected) in cases {
        let mut new = sample(&fx);
        new.animals = vec![line];
        let err = repo::insert_grazing_record(&mut conn, new, None).unwrap_err();
        match err {
            module_ecoscheme::EcoschemeError::Invalid(code) => assert_eq!(code, expected),
            other => panic!("expected Invalid({expected}), got {other:?}"),
        }
    }
}

#[test]
fn two_species_from_one_holding_are_two_lines_but_the_same_pair_folds() {
    // The UNIQUE key is (record, REGA, species) because the printed page gives
    // each combination its own line — 40 sheep and 12 goats from one holding
    // are two rows. Repeating a pair is a form filled twice, not an error, and
    // the later count is what the farmer meant.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.animals = vec![
        animal("03", "ES071234560001", 40),
        animal("04", "ES071234560001", 12),
        animal("03", "ES071234560001", 45),
    ];
    let detail = repo::insert_grazing_record(&mut conn, new, None).unwrap();
    assert_eq!(detail.animals.len(), 2);
    let sheep = detail
        .animals
        .iter()
        .find(|a| a.species_code == "03")
        .unwrap();
    assert_eq!(sheep.animal_count, 45);
}

#[test]
fn third_party_animals_keep_their_owners_rega() {
    // The REGA is per line, not per record, precisely because animals from
    // another holding carry their owner's code. Recording them under this
    // farm's REGA would misstate whose animals grazed.
    //
    // It is also what makes the twin's `AnimalesPropios`/`AnimalesTerceros`
    // derivable — Anexo V asks "Pastoreo con animales de la explotación (S/N)",
    // and a line's REGA answers it — which is why the record itself stores no
    // ownership split.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.animals = vec![
        animal("03", "ES071234560001", 40),
        animal("03", "ES289876540002", 60),
    ];
    let detail = repo::insert_grazing_record(&mut conn, new, None).unwrap();
    assert_eq!(detail.animals.len(), 2);
    let regas: Vec<&str> = detail
        .animals
        .iter()
        .map(|a| a.rega_code.as_str())
        .collect();
    assert!(regas.contains(&"ES071234560001") && regas.contains(&"ES289876540002"));
}

#[test]
fn a_correction_updates_an_animal_line_in_place_and_keeps_its_id() {
    // The audit trail must read as a correction rather than a replacement, so
    // a line that survives keeps its row id. `animal_count` is in the equality
    // test that decides "survived unchanged" — a field left out of one is
    // silently discarded while the command reports success (the 2026-08-12
    // reconcile_plots trap).
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), Some("tester")).unwrap();
    let original_id = detail.animals[0].id.clone();

    let corrected = repo::update_grazing_record(
        &mut conn,
        &detail.record.id,
        UpdateGrazingRecord {
            practice_code: "extensive_grazing".into(),
            plot_group_ref: Some("Grupo norte".into()),
            soil_cover_id: None,
            started_on: "2026-04-01".into(),
            ended_on: Some("2026-06-20".into()),
            notes: None,
            plot_ids: vec![fx.plot_a.clone()],
            animals: vec![animal("03", "ES071234560001", 118)],
        },
        Some("tester"),
    )
    .unwrap();

    assert_eq!(corrected.animals.len(), 1);
    assert_eq!(corrected.animals[0].id, original_id, "same line, corrected");
    assert_eq!(corrected.animals[0].animal_count, 118);
    assert_eq!(
        corrected.record.plot_group_ref.as_deref(),
        Some("Grupo norte")
    );
    assert_eq!(corrected.record.ended_on.as_deref(), Some("2026-06-20"));
}

#[test]
fn a_correction_that_changes_the_species_replaces_the_line() {
    // A different species is a different animal grazing, not a mistyped
    // number — so the old line goes and a new one arrives, and the log says so.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), None).unwrap();
    let original_id = detail.animals[0].id.clone();

    let corrected = repo::update_grazing_record(
        &mut conn,
        &detail.record.id,
        UpdateGrazingRecord {
            practice_code: "extensive_grazing".into(),
            plot_group_ref: None,
            soil_cover_id: None,
            started_on: "2026-04-01".into(),
            ended_on: Some("2026-06-15".into()),
            notes: None,
            plot_ids: vec![fx.plot_a.clone()],
            animals: vec![animal("01", "ES071234560001", 120)], // Bovinos
        },
        None,
    )
    .unwrap();

    assert_eq!(corrected.animals.len(), 1);
    assert_ne!(corrected.animals[0].id, original_id);
    assert_eq!(corrected.animals[0].species_code, "01");
}

#[test]
fn a_correction_reconciles_the_plot_set() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), None).unwrap();
    let kept_id = detail.plots[0].id.clone();

    let corrected = repo::update_grazing_record(
        &mut conn,
        &detail.record.id,
        UpdateGrazingRecord {
            practice_code: "extensive_grazing".into(),
            plot_group_ref: None,
            soil_cover_id: None,
            started_on: "2026-04-01".into(),
            ended_on: Some("2026-06-15".into()),
            notes: None,
            plot_ids: vec![fx.plot_a.clone(), fx.plot_b.clone()],
            animals: vec![animal("03", "ES071234560001", 120)],
        },
        None,
    )
    .unwrap();

    assert_eq!(corrected.plots.len(), 2);
    assert!(
        corrected.plots.iter().any(|p| p.id == kept_id),
        "a plot that stayed keeps its row"
    );
}

#[test]
fn every_write_is_audited_with_a_complete_row_image() {
    // The `record_change` contract: a receiving device must be able to rebuild
    // the row from `after` alone, so the payload is the whole struct and the
    // junctions are logged as entities of their own.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), Some("carlos")).unwrap();

    let (entity, operation, actor, payload): (String, String, Option<String>, String) = conn
        .query_row(
            "SELECT entity_table, operation, actor, payload FROM record_change
             WHERE entity_table = 'grazing_record' AND entity_id = ?1",
            [&detail.record.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(entity, "grazing_record");
    assert_eq!(operation, "insert");
    assert_eq!(actor.as_deref(), Some("carlos"));
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let after = &payload["after"];
    assert_eq!(after["started_on"], "2026-04-01");
    assert_eq!(after["practice_code"], "extensive_grazing");
    assert!(after.get("created_at").is_some(), "complete row image");

    // Children are logged individually, so a delta can rebuild them too.
    for table in ["grazing_plot", "grazing_animal"] {
        let logged: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM record_change WHERE entity_table = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(logged, 1, "{table} must be logged as its own entity");
    }
}

#[test]
fn a_deleted_grazing_leaves_the_register_but_keeps_its_history() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_grazing_record(&mut conn, &detail.record.id, Some("carlos")).unwrap();

    assert!(
        repo::list_grazing_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        repo::get_grazing_record(&conn, &detail.record.id).unwrap_err(),
        module_ecoscheme::EcoschemeError::NotFound
    ));

    let deletes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table = 'grazing_record' AND operation = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deletes, 1);
}

#[test]
fn records_list_oldest_first_within_their_own_season_and_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for date in ["2026-06-01", "2026-04-01", "2026-05-01"] {
        let mut new = sample(&fx);
        new.started_on = date.into();
        new.ended_on = None;
        repo::insert_grazing_record(&mut conn, new, None).unwrap();
    }

    let listed = repo::list_grazing_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let dates: Vec<&str> = listed
        .iter()
        .map(|d| d.record.started_on.as_str())
        .collect();
    assert_eq!(dates, ["2026-04-01", "2026-05-01", "2026-06-01"]);

    // Another farm's book must not see them.
    assert!(
        repo::list_grazing_records(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_season_holding_a_grazing_reports_itself_in_use() {
    // The shell chains this before soft-deleting a season: hiding the season
    // would hide its register from a book that is read season by season.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());
    let detail = repo::insert_grazing_record(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());

    // Soft-deleted records still count: their audit history is only reachable
    // through the season they belong to.
    repo::soft_delete_grazing_record(&mut conn, &detail.record.id, None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
}

#[test]
fn the_export_lister_keeps_the_withdrawn_records_the_book_hides() {
    // The SIEX export turns a withdrawn record into a `Borrar` entry under the
    // alias it was first exported with, so it is the one reader that must see
    // soft-deleted rows. Its name is the guard.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let kept = repo::insert_grazing_record(&mut conn, sample(&fx), None).unwrap();
    let mut withdrawn = sample(&fx);
    withdrawn.started_on = "2026-05-01".into();
    let withdrawn = repo::insert_grazing_record(&mut conn, withdrawn, None).unwrap();
    repo::soft_delete_grazing_record(&mut conn, &withdrawn.record.id, None).unwrap();

    assert_eq!(
        repo::list_grazing_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .len(),
        1
    );

    let listed = repo::list_grazing_records_for_export(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed.len(), 2);
    // Oldest first, like the book's own lister, and the children come along.
    assert_eq!(listed[0].record.id, kept.record.id);
    assert_eq!(listed[1].record.id, withdrawn.record.id);
    assert!(listed[1].record.deleted_at.is_some());
    assert_eq!(listed[1].plots.len(), 1);
    assert_eq!(listed[1].animals.len(), 1);

    // Another farm's export must not see them either.
    assert!(
        repo::list_grazing_records_for_export(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}
