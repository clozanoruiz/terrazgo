// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Models 9.4 and 9.5 — the register of covers.
//!
//! Every rule pinned here comes from RD 1048/2022 arts. 42 and 43, the printed
//! model's own 9.4/9.5 columns, or the SIEX `DatosCubierta` block; each test
//! names which. The article worth keeping in view throughout is **42.1, which
//! is three annotations on three different deadlines** — the establishment
//! date, the two widths, and the maintenance — collapsed by the printed form
//! into one row and split here into a row, a nullable triple and two other
//! registers.
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
            farm_name: "Olivar de la Umbría".into(),
            plot_a: PlotSpec::new("Olivar Alto", 8.0),
            plot_b: PlotSpec::new("Olivar Bajo", 6.0),
            other_farm_plot: PlotSpec::new("Ajena", 5.0),
            ..Default::default()
        },
    )
}

/// A live cover on one plot, with no widths stated yet — which is the normal
/// state of a cover between art. 42.1.a's deadline and art. 42.1.e's.
fn sample(fx: &CoreFixture) -> NewSoilCover {
    NewSoilCover {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "plant_cover".into(),
        // TIPO_COBERTURA_SUELO 2, "Cubierta vegetal sembrada".
        cover_type_code: "2".into(),
        established_on: "2026-03-15".into(),
        width_m: None,
        free_canopy_width_m: None,
        widths_stated_on: None,
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
        maintenance: Vec::new(),
    }
}

fn maintenance(kind_code: &str, performed_on: &str) -> CoverMaintenanceLine {
    CoverMaintenanceLine {
        id: String::new(),
        kind_code: kind_code.into(),
        performed_on: performed_on.into(),
        performed_end_date: None,
        animals: Vec::new(),
    }
}

fn grazing_line(performed_on: &str, count: i64) -> CoverMaintenanceLine {
    CoverMaintenanceLine {
        id: String::new(),
        kind_code: GRAZING_MAINTENANCE.into(),
        performed_on: performed_on.into(),
        performed_end_date: None,
        animals: vec![GrazingAnimal {
            id: String::new(),
            grazing_record_id: String::new(),
            species_code: "03".into(),
            rega_code: "ES071234560001".into(),
            animal_count: count,
        }],
    }
}

// --- the record itself -----------------------------------------------------

#[test]
fn a_cover_is_stored_with_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.plot_ids = vec![fx.plot_a.clone(), fx.plot_b.clone()];
    let detail = repo::insert_soil_cover(&mut conn, new, Some("carlos")).unwrap();

    assert_eq!(detail.record.practice_code, "plant_cover");
    assert_eq!(detail.record.cover_type_code, "2");
    assert_eq!(detail.record.established_on, "2026-03-15");
    assert_eq!(detail.plots.len(), 2);
    assert!(detail.maintenance.is_empty());
}

#[test]
fn a_cover_with_no_widths_yet_is_a_complete_record() {
    // Art. 42.1.a and 42.1.e are two annotations on two deadlines: the
    // establishment date is due within a month, the widths within the month
    // before the four-month live-cover period ends. So a cover whose widths are
    // not stated yet is not an incomplete record — it is a record whose second
    // annotation is not due yet, and the advisory says so rather than the
    // repository refusing it.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    assert!(detail.record.width_m.is_none());
    assert!(detail.record.free_canopy_width_m.is_none());
    assert!(detail.record.widths_stated_on.is_none());
}

#[test]
fn the_widths_are_one_annotation_so_they_arrive_together_or_not_at_all() {
    // Art. 42.1.e asks for "la anchura de la cubierta **y** la anchura libre de
    // la proyección de copa" as one annotation on one deadline, so a half-filled
    // triple is a wrong answer rather than a missing one — the
    // `plot_water_point.distance_m` pairing.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let partial = [
        (Some(2.0), None, None),
        (None, Some(1.5), None),
        (None, None, Some("2026-06-01")),
        (Some(2.0), Some(1.5), None),
        (Some(2.0), None, Some("2026-06-01")),
    ];
    for (width, canopy, stated) in partial {
        let mut new = sample(&fx);
        new.width_m = width;
        new.free_canopy_width_m = canopy;
        new.widths_stated_on = stated.map(str::to_string);
        let err = repo::insert_soil_cover(&mut conn, new, None).unwrap_err();
        assert!(
            matches!(
                err,
                module_ecoscheme::EcoschemeError::Invalid("incomplete_widths")
            ),
            "a partial width triple must be refused: {width:?} {canopy:?} {stated:?}"
        );
    }

    let mut complete = sample(&fx);
    complete.width_m = Some(2.0);
    complete.free_canopy_width_m = Some(1.5);
    complete.widths_stated_on = Some("2026-06-01".into());
    let detail = repo::insert_soil_cover(&mut conn, complete, None).unwrap();
    assert_eq!(detail.record.width_m, Some(2.0));
    assert_eq!(
        detail.record.widths_stated_on.as_deref(),
        Some("2026-06-01")
    );
}

#[test]
fn a_stated_width_has_to_be_a_width() {
    // A cover 0 m wide is not a cover, and a negative one is a typo. Unlike the
    // widths' presence, this is checkable at write time.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for (width, canopy) in [(0.0, 1.5), (2.0, -0.5)] {
        let mut new = sample(&fx);
        new.width_m = Some(width);
        new.free_canopy_width_m = Some(canopy);
        new.widths_stated_on = Some("2026-06-01".into());
        let err = repo::insert_soil_cover(&mut conn, new, None).unwrap_err();
        assert!(matches!(
            err,
            module_ecoscheme::EcoschemeError::Invalid("nonpositive_width")
        ));
    }
}

#[test]
fn only_the_two_practices_a_cover_can_evidence_are_accepted() {
    // Arts. 42 and 43 are the only clauses that establish a cover. P1 is a
    // grazing, P2 a mown plot, P5 a flooded crop and anexo IV a comunal
    // pasture — a cover filed against any of them would print on no page.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for code in ["plant_cover", "inert_cover"] {
        let mut new = sample(&fx);
        new.practice_code = code.into();
        // TIPO_COBERTURA_SUELO 4, "Cubierta inerte de restos de poda".
        new.cover_type_code = if code == "inert_cover" { "4" } else { "2" }.into();
        assert!(repo::insert_soil_cover(&mut conn, new, None).is_ok());
    }

    for code in [
        "extensive_grazing",
        "sustainable_mowing",
        "communal_pasture",
        "flooded_biodiversity",
    ] {
        let mut new = sample(&fx);
        new.practice_code = code.into();
        let err = repo::insert_soil_cover(&mut conn, new, None).unwrap_err();
        assert!(
            matches!(
                err,
                module_ecoscheme::EcoschemeError::Invalid("practice_not_cover")
            ),
            "{code} must not establish a cover"
        );
    }
}

#[test]
fn a_cover_needs_a_plot_and_it_must_be_on_the_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut none = sample(&fx);
    none.plot_ids.clear();
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, none, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("no_plots")
    ));

    let mut foreign = sample(&fx);
    foreign.plot_ids = vec![fx.other_farm_plot.clone()];
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, foreign, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn a_cover_type_the_catalogue_grows_later_is_stored_rather_than_refused() {
    // The two-tier rule. `TIPO_COBERTURA_SUELO` is a provider registry that
    // grows between releases — it gained code 6 in 2024 — and the in-app
    // refresh means a farmer's own copy can carry a code this build has never
    // seen. Refusing it would lock them out of recording a lawful cover, so the
    // FORM narrows the picker and the record accepts what it is given.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.cover_type_code = "99".into();
    let detail = repo::insert_soil_cover(&mut conn, new, None).unwrap();
    assert_eq!(detail.record.cover_type_code, "99");
}

// --- maintenance (art. 42.1.c) ---------------------------------------------

#[test]
fn maintenance_is_written_into_the_registers_that_own_it() {
    // Art. 42.1.c's "tipo de mantenimiento" is model 9.4's last three columns.
    // A siega and a desbroce are cultural operations; a pastoreo is a grazing.
    // This register owns no maintenance table — it writes through the other
    // two, so one form cannot validate what another does not.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.maintenance = vec![
        maintenance("mowing", "2026-05-12"),
        maintenance("brush_cutting", "2026-07-28"),
        grazing_line("2026-06-03", 40),
    ];
    let detail = repo::insert_soil_cover(&mut conn, new, Some("carlos")).unwrap();

    assert_eq!(detail.maintenance.len(), 3);
    // Ordered by date whichever table each line came from.
    let dates: Vec<&str> = detail
        .maintenance
        .iter()
        .map(|line| line.performed_on.as_str())
        .collect();
    assert_eq!(dates, ["2026-05-12", "2026-06-03", "2026-07-28"]);

    let operations = repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(operations.len(), 2, "the siega and the desbroce");
    for operation in &operations {
        assert_eq!(operation.record.practice_code, "plant_cover");
        assert_eq!(
            operation.record.soil_cover_id.as_deref(),
            Some(detail.record.id.as_str())
        );
        assert_eq!(
            operation.plots.len(),
            1,
            "a maintenance line inherits the cover's plots"
        );
    }

    let grazings = repo::list_grazing_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(grazings.len(), 1);
    assert_eq!(
        grazings[0].record.soil_cover_id.as_deref(),
        Some(detail.record.id.as_str())
    );
    assert_eq!(grazings[0].animals[0].animal_count, 40);
}

#[test]
fn a_grazing_line_states_its_animals_like_any_other_grazing() {
    // Entering a pastoreo from the cover form does not excuse it from what the
    // register demands of every grazing.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    let mut line = grazing_line("2026-06-03", 40);
    line.animals.clear();
    new.maintenance = vec![line];
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("no_animals")
    ));
}

#[test]
fn only_the_three_kinds_model_94_prints_are_maintenance() {
    // A poda or a rulado is a cultural operation in its own right, recorded on
    // the 9.2 register where it prints — not a cover's maintenance.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.maintenance = vec![maintenance("pruning", "2026-05-12")];
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("not_a_maintenance_kind")
    ));
}

#[test]
fn an_inert_cover_takes_no_maintenance() {
    // Art. 43 asks for an establishment date and two widths and nothing else;
    // model 9.5 has no maintenance columns. A line against one would print
    // nowhere, so it is refused rather than silently stored.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.practice_code = "inert_cover".into();
    new.cover_type_code = "4".into();
    new.maintenance = vec![maintenance("mowing", "2026-05-12")];
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("maintenance_on_an_inert_cover")
    ));
}

#[test]
fn animals_on_a_line_that_is_not_a_grazing_are_refused_rather_than_dropped() {
    // Silently discarding them would let a form lose a head count while the
    // command reports success — the `reconcile_plots` trap of 2026-08-12.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    let mut line = maintenance("mowing", "2026-05-12");
    line.animals = grazing_line("2026-05-12", 10).animals;
    new.maintenance = vec![line];
    assert!(matches!(
        repo::insert_soil_cover(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("animals_on_a_non_grazing_line")
    ));
}

#[test]
fn a_refused_maintenance_line_leaves_no_cover_behind() {
    // The whole point of writing the cover and its maintenance in ONE
    // transaction: a book must never hold a cover whose third annotation
    // half-saved.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    let mut line = grazing_line("2026-06-03", 40);
    line.animals.clear();
    new.maintenance = vec![maintenance("mowing", "2026-05-12"), line];
    assert!(repo::insert_soil_cover(&mut conn, new, None).is_err());

    assert!(
        repo::list_soil_covers(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty(),
        "the siega that had already been written must roll back with the cover"
    );
}

// --- corrections -----------------------------------------------------------

#[test]
fn stating_the_widths_later_is_a_correction_not_a_new_record() {
    // The ordinary lifecycle of a P6 cover: established in March, measured in
    // June. Art. 42.1.e's annotation lands on the row art. 42.1.a created.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    let updated = repo::update_soil_cover(
        &mut conn,
        &detail.record.id,
        UpdateSoilCover {
            practice_code: "plant_cover".into(),
            cover_type_code: "2".into(),
            established_on: "2026-03-15".into(),
            width_m: Some(2.0),
            free_canopy_width_m: Some(1.5),
            widths_stated_on: Some("2026-06-01".into()),
            notes: None,
            plot_ids: vec![fx.plot_a.clone()],
            maintenance: Vec::new(),
        },
        Some("carlos"),
    )
    .unwrap();

    assert_eq!(updated.record.id, detail.record.id);
    assert_eq!(updated.record.width_m, Some(2.0));
    assert_eq!(updated.record.established_on, "2026-03-15");
}

#[test]
fn a_correction_keeps_a_maintenance_line_that_stayed_and_withdraws_one_that_went() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.maintenance = vec![
        maintenance("mowing", "2026-05-12"),
        maintenance("brush_cutting", "2026-07-28"),
    ];
    let detail = repo::insert_soil_cover(&mut conn, new, None).unwrap();
    let kept = detail.maintenance[0].clone();

    // The siega's date was wrong and the desbroce never happened.
    let mut corrected = kept.clone();
    corrected.performed_on = "2026-05-14".into();
    let updated = repo::update_soil_cover(
        &mut conn,
        &detail.record.id,
        UpdateSoilCover {
            practice_code: "plant_cover".into(),
            cover_type_code: "2".into(),
            established_on: "2026-03-15".into(),
            width_m: None,
            free_canopy_width_m: None,
            widths_stated_on: None,
            notes: None,
            plot_ids: vec![fx.plot_a.clone()],
            maintenance: vec![corrected],
        },
        Some("carlos"),
    )
    .unwrap();

    assert_eq!(updated.maintenance.len(), 1);
    assert_eq!(
        updated.maintenance[0].id, kept.id,
        "a correction keeps the record's id, so the audit trail reads as a correction"
    );
    assert_eq!(updated.maintenance[0].performed_on, "2026-05-14");

    // The withdrawn one is soft-deleted, not erased.
    let withdrawn: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cultural_operation WHERE deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(withdrawn, 1);
}

#[test]
fn a_line_corrected_across_the_two_registers_is_a_withdrawal_and_a_new_record() {
    // A siega corrected to a pastoreo cannot be an in-place edit: the two live
    // in different tables and no row moves between them. The honest audit trail
    // is that the annotation said one activity and now says another.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.maintenance = vec![maintenance("mowing", "2026-05-12")];
    let detail = repo::insert_soil_cover(&mut conn, new, None).unwrap();

    let mut swapped = grazing_line("2026-05-12", 25);
    swapped.id = detail.maintenance[0].id.clone();
    let updated = repo::update_soil_cover(
        &mut conn,
        &detail.record.id,
        UpdateSoilCover {
            practice_code: "plant_cover".into(),
            cover_type_code: "2".into(),
            established_on: "2026-03-15".into(),
            width_m: None,
            free_canopy_width_m: None,
            widths_stated_on: None,
            notes: None,
            plot_ids: vec![fx.plot_a.clone()],
            maintenance: vec![swapped],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.maintenance.len(), 1);
    assert_eq!(updated.maintenance[0].kind_code, GRAZING_MAINTENANCE);
    assert_ne!(updated.maintenance[0].id, detail.maintenance[0].id);
    assert!(
        repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty(),
        "the siega is withdrawn"
    );
}

#[test]
fn withdrawing_a_cover_withdraws_the_maintenance_recorded_against_it() {
    // Those rows are art. 42.1.c's annotation OF THIS COVER — they print in its
    // columns and exist as its third deadline — so a cover withdrawn as a
    // mistake leaves no siega behind pointing at nothing.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx);
    new.maintenance = vec![
        maintenance("mowing", "2026-05-12"),
        grazing_line("2026-06-03", 40),
    ];
    let detail = repo::insert_soil_cover(&mut conn, new, Some("carlos")).unwrap();

    repo::soft_delete_soil_cover(&mut conn, &detail.record.id, Some("carlos")).unwrap();

    assert!(
        repo::list_soil_covers(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        repo::list_cultural_operations(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        repo::list_grazing_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    // Withdrawn, never erased: each keeps its own audited history.
    for table in ["soil_cover", "cultural_operation", "grazing_record"] {
        let deleted: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE deleted_at IS NOT NULL"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted, 1, "{table} keeps its withdrawn row");
    }
}

// --- the link, from the other side -----------------------------------------

#[test]
fn a_maintenance_record_must_name_a_cover_of_its_own_practice() {
    // Model 9.4 is the P6 page and 9.5 the P7 one, so a siega filed under P2
    // but pointed at a plant cover would claim art. 31's duty while printing as
    // art. 42.1.c's maintenance.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let cover = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();

    let operation = |practice: &str, cover_id: Option<String>| NewCulturalOperation {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: practice.into(),
        operation_kind_code: "mowing".into(),
        performed_on: "2026-05-12".into(),
        performed_end_date: None,
        activity_description: None,
        residue_destination_code: None,
        soil_cover_id: cover_id,
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
    };

    let err = repo::insert_cultural_operation(
        &mut conn,
        operation("sustainable_mowing", Some(cover.record.id.clone())),
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        module_ecoscheme::EcoschemeError::Invalid("cover_practice_mismatch")
    ));

    assert!(
        repo::insert_cultural_operation(
            &mut conn,
            operation("plant_cover", Some(cover.record.id.clone())),
            None
        )
        .is_ok()
    );
}

#[test]
fn a_maintenance_record_cannot_name_a_cover_that_is_not_there() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = NewGrazingRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "plant_cover".into(),
        plot_group_ref: None,
        soil_cover_id: Some("no-such-cover".into()),
        started_on: "2026-06-03".into(),
        ended_on: None,
        notes: None,
        plot_ids: vec![fx.plot_a.clone()],
        animals: grazing_line("2026-06-03", 40).animals,
    };
    assert!(matches!(
        repo::insert_grazing_record(&mut conn, new.clone(), None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("cover_not_found")
    ));

    // A withdrawn cover is not there either.
    let cover = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_soil_cover(&mut conn, &cover.record.id, None).unwrap();
    new.soil_cover_id = Some(cover.record.id.clone());
    assert!(matches!(
        repo::insert_grazing_record(&mut conn, new, None).unwrap_err(),
        module_ecoscheme::EcoschemeError::Invalid("cover_not_found")
    ));
}

#[test]
fn a_cover_on_another_farm_cannot_be_maintained_from_this_one() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let cover = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();

    // The other farm's own season-scoped record, pointed at this farm's cover.
    let other_farm_id: String = conn
        .query_row(
            "SELECT farm_id FROM plot WHERE id = ?1",
            [&fx.other_farm_plot],
            |r| r.get(0),
        )
        .unwrap();
    let err = repo::insert_cultural_operation(
        &mut conn,
        NewCulturalOperation {
            season_id: fx.season_id.clone(),
            farm_id: other_farm_id,
            practice_code: "plant_cover".into(),
            operation_kind_code: "mowing".into(),
            performed_on: "2026-05-12".into(),
            performed_end_date: None,
            activity_description: None,
            residue_destination_code: None,
            soil_cover_id: Some(cover.record.id.clone()),
            notes: None,
            plot_ids: vec![fx.other_farm_plot.clone()],
        },
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        module_ecoscheme::EcoschemeError::Invalid("cover_on_another_farm")
    ));
}

// --- audit and scoping -----------------------------------------------------

#[test]
fn every_write_is_audited_with_a_complete_row_image() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_soil_cover(&mut conn, sample(&fx), Some("carlos")).unwrap();

    let (operation, actor, payload): (String, Option<String>, String) = conn
        .query_row(
            "SELECT operation, actor, payload FROM record_change
             WHERE entity_table = 'soil_cover' AND entity_id = ?1",
            [&detail.record.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(operation, "insert");
    assert_eq!(actor.as_deref(), Some("carlos"));
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let after = &payload["after"];
    assert_eq!(after["established_on"], "2026-03-15");
    assert_eq!(after["cover_type_code"], "2");
    assert!(after.get("created_at").is_some(), "complete row image");

    let logged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change WHERE entity_table = 'soil_cover_plot'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 1, "the plot is logged as its own entity");
}

#[test]
fn covers_list_oldest_first_within_their_own_season_and_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for date in ["2026-04-02", "2026-01-20", "2026-03-15"] {
        let mut new = sample(&fx);
        new.established_on = date.into();
        repo::insert_soil_cover(&mut conn, new, None).unwrap();
    }

    let listed = repo::list_soil_covers(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let dates: Vec<&str> = listed
        .iter()
        .map(|d| d.record.established_on.as_str())
        .collect();
    assert_eq!(dates, ["2026-01-20", "2026-03-15", "2026-04-02"]);
}

#[test]
fn a_season_holding_a_cover_reports_itself_in_use() {
    // The shell chains this before deleting a season; a season holding nothing
    // but a cover would otherwise be deletable.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());
    repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
}

#[test]
fn the_export_lister_keeps_the_withdrawn_records_the_book_hides() {
    // See the same test on the other two registers. A withdrawn cover matters
    // more than most: deleting one withdraws its maintenance lines too, and the
    // export has to state all of it under the aliases it already sent.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let kept = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    let mut withdrawn = sample(&fx);
    withdrawn.established_on = "2026-11-02".into();
    let withdrawn = repo::insert_soil_cover(&mut conn, withdrawn, None).unwrap();
    repo::soft_delete_soil_cover(&mut conn, &withdrawn.record.id, None).unwrap();

    assert_eq!(
        repo::list_soil_covers(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .len(),
        1
    );

    let listed = repo::list_soil_covers_for_export(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record.id, kept.record.id);
    assert_eq!(listed[1].record.id, withdrawn.record.id);
    assert!(listed[1].record.deleted_at.is_some());
    assert_eq!(listed[1].plots.len(), 1);

    assert!(
        repo::list_soil_covers_for_export(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_export_getter_resolves_a_withdrawn_cover_the_ordinary_one_hides() {
    // A maintenance record names the cover it maintained, and the export
    // restates that cover's type on every DGC of the entry — including on a
    // deletion entry, whose cover is withdrawn by then.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let record = repo::insert_soil_cover(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_soil_cover(&mut conn, &record.record.id, None).unwrap();

    assert!(repo::get_soil_cover(&conn, &record.record.id).is_err());

    let found = repo::get_soil_cover_for_export(&conn, &record.record.id).unwrap();
    assert_eq!(found.id, record.record.id);
    assert_eq!(found.cover_type_code, record.record.cover_type_code);
    assert!(found.deleted_at.is_some());
}
