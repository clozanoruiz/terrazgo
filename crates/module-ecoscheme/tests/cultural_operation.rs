// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model 9.2 and the book's "9.6" — the register of what was done on the land.
//!
//! Every rule pinned here comes from RD 1048/2022 (arts. 31, 31.4.d, 43.1.a and
//! anexo IV), the printed model's own 9.2 footnotes, or the SIEX
//! `LaboresCulturales` block; each test names which.
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
            farm_name: "Olivar de la Vega".into(),
            plot_a: PlotSpec::new("Linde Norte", 12.0),
            plot_b: PlotSpec::new("Linde Sur", 9.0),
            other_farm_plot: PlotSpec::new("Ajena", 5.0),
            ..Default::default()
        },
    )
}

fn sample(fx: &CoreFixture) -> NewCulturalOperation {
    NewCulturalOperation {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "sustainable_mowing".into(),
        operation_kind_code: "mowing".into(),
        performed_on: "2026-05-12".into(),
        performed_end_date: None,
        activity_description: None,
        residue_destination_code: None,
        soil_cover_id: None,
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
    }
}

#[test]
fn an_operation_is_stored_with_the_plots_it_covered() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.plot_a.clone(), fx.plot_b.clone()];
    let detail = repo::insert_cultural_operation(&mut conn, new, Some("tester")).unwrap();

    assert_eq!(detail.plots.len(), 2);
    assert_eq!(detail.record.operation_kind_code, "mowing");
    assert_eq!(detail.record.performed_on, "2026-05-12");

    let read_back = repo::get_cultural_operation(&conn, &detail.record.id).unwrap();
    assert_eq!(read_back.plots.len(), 2);
}

#[test]
fn a_single_day_operation_leaves_the_end_null_rather_than_repeating_the_date() {
    // `LaboresCulturales` carries both ends, so a repeated date would claim an
    // interval nobody stated. Art. 31 asks for "la fecha" — one date is a
    // complete answer, not a half-filled one.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    assert_eq!(detail.record.performed_end_date, None);

    let mut over_days = sample(&fx);
    over_days.performed_end_date = Some("2026-05-14".into());
    let spanned = repo::insert_cultural_operation(&mut conn, over_days, None).unwrap();
    assert_eq!(
        spanned.record.performed_end_date.as_deref(),
        Some("2026-05-14")
    );
}

#[test]
fn an_operation_cannot_end_before_it_starts() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.performed_on = "2026-05-12".into();
    new.performed_end_date = Some("2026-05-01".into());
    let err = repo::insert_cultural_operation(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_ecoscheme::EcoschemeError::Invalid("invalid_date_interval")
    ));
}

#[test]
fn every_practice_but_the_grazing_one_can_carry_an_operation() {
    // Five of the six duties name an activity carried out on the land: art. 31
    // and 31.4.d (P2), anexo IV (pastos comunales), art. 45.2's nivelación and
    // caballones (P5), art. 42.1.c's cover maintenance (P6) and art. 43.1.a's
    // triturated poda (P7).
    //
    // `extensive_grazing` is the exception, and deliberately: art. 30.2 ter's
    // duty is the grazing DATES, which `grazing_record` holds. An operation
    // filed against P1 would print on no page of section 9.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for code in [
        "sustainable_mowing",
        "communal_pasture",
        "flooded_biodiversity",
        "plant_cover",
        "inert_cover",
    ] {
        let mut new = sample(&fx);
        new.practice_code = code.into();
        assert!(
            repo::insert_cultural_operation(&mut conn, new, None).is_ok(),
            "{code} must be recordable as a cultural operation"
        );
    }

    let mut grazing = sample(&fx);
    grazing.practice_code = "extensive_grazing".into();
    assert!(matches!(
        repo::insert_cultural_operation(&mut conn, grazing, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("practice_not_operation")
    ));
}

#[test]
fn an_unknown_practice_or_kind_is_refused_by_the_schema() {
    // The lookups are real foreign keys, so a code that is not seeded cannot be
    // stored — the practice decides which page the row prints on, and the kind
    // decides which column.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut bad_practice = sample(&fx);
    bad_practice.practice_code = "carbon_farming".into();
    assert!(repo::insert_cultural_operation(&mut conn, bad_practice, None).is_err());

    let mut bad_kind = sample(&fx);
    bad_kind.operation_kind_code = "terracing".into();
    assert!(repo::insert_cultural_operation(&mut conn, bad_kind, None).is_err());
}

#[test]
fn an_operation_needs_a_plot() {
    // Anexo IV asks for the activities "en cada parcela" and model 9.2's row IS
    // a plot, so an operation naming none has nowhere to print.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![];
    assert!(matches!(
        repo::insert_cultural_operation(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("no_plots")
    ));
}

#[test]
fn a_plot_must_be_on_the_farm_that_worked_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.other_farm_plot.clone()];
    let err = repo::insert_cultural_operation(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_ecoscheme::EcoschemeError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn the_same_plot_named_twice_is_one_plot() {
    // A form that listed a plot twice means one operation on it, not an error
    // — and the UNIQUE index would refuse the second row anyway.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.plot_a.clone(), fx.plot_a.clone()];
    let detail = repo::insert_cultural_operation(&mut conn, new, None).unwrap();
    assert_eq!(detail.plots.len(), 1);
}

#[test]
fn the_residue_destination_that_creates_an_inert_cover_is_stored_verbatim() {
    // DEST_RES_VEG 9 = "Trituración de restos de poda y depositado sobre el
    // terreno". It is the evidence art. 43.1.a rests on: the inert cover exists
    // BECAUSE this poda's residue stayed on the land. Stored verbatim with no
    // foreign key, per the catalogue rule — the code is the payload.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.practice_code = "inert_cover".into();
    new.operation_kind_code = "pruning".into();
    new.residue_destination_code = Some(module_ecoscheme::siex::RESIDUE_LEFT_ON_GROUND.into());
    let detail = repo::insert_cultural_operation(&mut conn, new, None).unwrap();

    assert_eq!(detail.record.residue_destination_code.as_deref(), Some("9"));
}

#[test]
fn blank_free_text_is_stored_as_absent() {
    // A submitted empty string is a field the farmer left alone, and model 9.2
    // footnote (4)'s "otras actividades" cell must read as empty rather than as
    // a described activity with no description.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.activity_description = Some("   ".into());
    new.notes = Some(String::new());
    new.residue_destination_code = Some(String::new());
    let detail = repo::insert_cultural_operation(&mut conn, new, None).unwrap();

    assert_eq!(detail.record.activity_description, None);
    assert_eq!(detail.record.notes, None);
    assert_eq!(detail.record.residue_destination_code, None);
}

#[test]
fn a_correction_updates_the_row_and_reconciles_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    let kept_id = detail.plots[0].id.clone();

    let corrected = repo::update_cultural_operation(
        &mut conn,
        &detail.record.id,
        UpdateCulturalOperation {
            practice_code: "communal_pasture".into(),
            operation_kind_code: "brush_cutting".into(),
            performed_on: "2026-05-13".into(),
            performed_end_date: None,
            activity_description: Some("Desbroce del perímetro".into()),
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids: vec![fx.plot_a.clone(), fx.plot_b.clone()],
        },
        None,
    )
    .unwrap();

    assert_eq!(corrected.record.practice_code, "communal_pasture");
    assert_eq!(corrected.record.operation_kind_code, "brush_cutting");
    assert_eq!(corrected.record.performed_on, "2026-05-13");
    assert_eq!(corrected.plots.len(), 2);
    assert!(
        corrected.plots.iter().any(|p| p.id == kept_id),
        "a plot that stayed keeps its row, so the audit trail reads as a correction"
    );

    // The record moved from 9.2 to 9.6, which is a correction of what it
    // evidences and not a new record.
    let read_back = repo::get_cultural_operation(&conn, &detail.record.id).unwrap();
    assert_eq!(read_back.record.id, detail.record.id);
}

#[test]
fn a_correction_that_drops_a_plot_removes_only_that_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.plot_a.clone(), fx.plot_b.clone()];
    let detail = repo::insert_cultural_operation(&mut conn, new, None).unwrap();

    let corrected = repo::update_cultural_operation(
        &mut conn,
        &detail.record.id,
        UpdateCulturalOperation {
            practice_code: "sustainable_mowing".into(),
            operation_kind_code: "mowing".into(),
            performed_on: "2026-05-12".into(),
            performed_end_date: None,
            activity_description: None,
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids: vec![fx.plot_b.clone()],
        },
        None,
    )
    .unwrap();

    assert_eq!(corrected.plots.len(), 1);
    assert_eq!(corrected.plots[0].plot_id, fx.plot_b);

    let deleted: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table = 'cultural_operation_plot' AND operation = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deleted, 1, "the dropped plot is logged as its own deletion");
}

#[test]
fn correcting_an_unknown_or_deleted_record_is_not_found() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let update = || UpdateCulturalOperation {
        practice_code: "sustainable_mowing".into(),
        operation_kind_code: "mowing".into(),
        performed_on: "2026-05-12".into(),
        performed_end_date: None,
        activity_description: None,
        residue_destination_code: None,
        soil_cover_id: None,
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
    };

    assert!(matches!(
        repo::update_cultural_operation(&mut conn, "no-such-id", update(), None).unwrap_err(),
        module_ecoscheme::EcoschemeError::NotFound
    ));

    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_cultural_operation(&mut conn, &detail.record.id, None).unwrap();
    assert!(matches!(
        repo::update_cultural_operation(&mut conn, &detail.record.id, update(), None).unwrap_err(),
        module_ecoscheme::EcoschemeError::NotFound
    ));
}

#[test]
fn every_write_is_audited_with_a_complete_row_image() {
    // The `record_change` contract: a receiving device must be able to rebuild
    // the row from `after` alone, so the payload is the whole struct and the
    // junction is logged as an entity of its own.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), Some("carlos")).unwrap();

    let (operation, actor, payload): (String, Option<String>, String) = conn
        .query_row(
            "SELECT operation, actor, payload FROM record_change
             WHERE entity_table = 'cultural_operation' AND entity_id = ?1",
            [&detail.record.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(operation, "insert");
    assert_eq!(actor.as_deref(), Some("carlos"));
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let after = &payload["after"];
    assert_eq!(after["performed_on"], "2026-05-12");
    assert_eq!(after["operation_kind_code"], "mowing");
    assert!(after.get("created_at").is_some(), "complete row image");

    let logged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change WHERE entity_table = 'cultural_operation_plot'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 1, "the plot is logged as its own entity");
}

#[test]
fn a_deleted_operation_leaves_the_register_but_keeps_its_history() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_cultural_operation(&mut conn, &detail.record.id, Some("carlos")).unwrap();

    assert!(
        repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        repo::get_cultural_operation(&conn, &detail.record.id).unwrap_err(),
        module_ecoscheme::EcoschemeError::NotFound
    ));

    let deletes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table = 'cultural_operation' AND operation = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deletes, 1);
}

#[test]
fn operations_list_oldest_first_within_their_own_season_and_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for date in ["2026-06-01", "2026-04-01", "2026-05-01"] {
        let mut new = sample(&fx);
        new.performed_on = date.into();
        repo::insert_cultural_operation(&mut conn, new, None).unwrap();
    }

    let listed = repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let dates: Vec<&str> = listed
        .iter()
        .map(|d| d.record.performed_on.as_str())
        .collect();
    assert_eq!(dates, ["2026-04-01", "2026-05-01", "2026-06-01"]);

    assert!(
        repo::list_cultural_operations(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_season_holding_an_operation_reports_itself_in_use() {
    // The shell chains this before soft-deleting a season. It must cover EVERY
    // register the module owns: a season holding nothing but a cultural
    // operation would otherwise be deletable, and its records would vanish from
    // a book that is read season by season.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());
    let detail = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());

    // Soft-deleted records still count: their audit history is only reachable
    // through the season they belong to.
    repo::soft_delete_cultural_operation(&mut conn, &detail.record.id, None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
}

#[test]
fn the_export_lister_keeps_the_withdrawn_records_the_book_hides() {
    // See the same test on the grazing register: the SIEX export is the one
    // reader that must see soft-deleted rows, because a withdrawal travels as a
    // `Borrar` entry under the alias the record was first exported with.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let kept = repo::insert_cultural_operation(&mut conn, sample(&fx), None).unwrap();
    let mut withdrawn = sample(&fx);
    withdrawn.performed_on = "2026-07-20".into();
    let withdrawn = repo::insert_cultural_operation(&mut conn, withdrawn, None).unwrap();
    repo::soft_delete_cultural_operation(&mut conn, &withdrawn.record.id, None).unwrap();

    assert_eq!(
        repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .len(),
        1
    );

    let listed =
        repo::list_cultural_operations_for_export(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record.id, kept.record.id);
    assert_eq!(listed[1].record.id, withdrawn.record.id);
    assert!(listed[1].record.deleted_at.is_some());
    assert_eq!(listed[1].plots.len(), 1);

    assert!(
        repo::list_cultural_operations_for_export(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}
