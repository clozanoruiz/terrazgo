// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 7.1 — the plan de abonado, as the BOOK records it.
//!
//! RD 1051/2022 art. 4.2 requires the plan, art. 6 says what the plan document
//! must contain, and art. 5.a says what goes in the book. Only the last is
//! stored here, and each test names its source.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{FarmWithPlots, PlotSpec, farm_with_plots, last_change};
use module_fertilisation::models::*;
use module_fertilisation::open_in_memory;
use module_fertilisation::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::{NewCrop, NewSeason};
use terrazgo_core::repository as core_repo;

/// A plan covers crops, not plots, so this fixture is the shared land with a
/// crop on each plot — plus a SECOND campaign, which is what the "at most one
/// live plan per crop" rule is scoped by.
struct Fixture {
    season_id: String,
    other_season_id: String,
    farm_id: String,
    wheat: String,
    barley: String,
    other_farm_crop: String,
    other_season_crop: String,
}

fn fixture(conn: &mut Connection) -> Fixture {
    let core = farm_with_plots(
        conn,
        FarmWithPlots {
            other_farm_plot: PlotSpec::new("Ajena", 4.0),
            ..Default::default()
        },
    );
    let other_season_id = core_repo::insert_season(
        conn,
        NewSeason {
            campaign_year: 2027,
            label: "2026/2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap()
    .id;

    let crop = |conn: &mut Connection, plot_id: &str, season_id: &str, species: &str| {
        core_repo::insert_crop(
            conn,
            NewCrop {
                plot_id: plot_id.to_string(),
                season_id: season_id.to_string(),
                species_name: species.into(),
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
        .unwrap()
        .id
    };

    Fixture {
        wheat: crop(conn, &core.plot_a, &core.season_id, "trigo blando"),
        barley: crop(conn, &core.plot_b, &core.season_id, "cebada"),
        other_farm_crop: crop(conn, &core.other_farm_plot, &core.season_id, "maíz"),
        other_season_crop: crop(conn, &core.plot_a, &other_season_id, "girasol"),
        season_id: core.season_id,
        other_season_id,
        farm_id: core.farm_id,
    }
}

fn sample(fx: &Fixture) -> NewFertilisationPlan {
    NewFertilisationPlan {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        // Art. 5.a's four: needs, expected yield, preceding crop, date drawn up.
        needs_n_kg_ha: 140.0,
        needs_p2o5_kg_ha: 60.0,
        needs_k2o_kg_ha: 0.0,
        expected_yield_kg_ha: 6500.0,
        preceding_crop_code: Some("60".into()),
        drawn_up_on: "2025-09-20".into(),
        tool_generated: false,
        notes: None,
        crop_ids: vec![fx.wheat.clone()],
    }
}

#[test]
fn records_exactly_what_article_5a_puts_in_the_book() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_fertilisation_plan(&mut conn, sample(&fx), Some("user-1")).unwrap();

    assert_eq!(detail.plan.needs_n_kg_ha, 140.0);
    assert_eq!(detail.plan.needs_p2o5_kg_ha, 60.0);
    // Zero IS a recommendation — "this unit needs no potassium" is exactly the
    // kind of thing a plan says, unlike an unstated richness.
    assert_eq!(detail.plan.needs_k2o_kg_ha, 0.0);
    assert_eq!(detail.plan.expected_yield_kg_ha, 6500.0);
    assert_eq!(detail.plan.preceding_crop_code.as_deref(), Some("60"));
    assert_eq!(detail.plan.drawn_up_on, "2025-09-20");
    assert_eq!(detail.crop_ids, vec![fx.wheat.clone()]);
}

#[test]
fn a_production_unit_may_be_several_plots_of_the_same_crop() {
    // Art. 4.2 asks for a plan per unidad de producción, not per parcel, and
    // `PlanAbonado.DGCs` is an array — so the covered crops are a junction.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.crop_ids = vec![fx.wheat.clone(), fx.barley.clone()];

    let detail = repo::insert_fertilisation_plan(&mut conn, new, None).unwrap();
    assert_eq!(detail.crop_ids.len(), 2);

    let stored = repo::get_fertilisation_plan(&conn, &detail.plan.id).unwrap();
    assert_eq!(stored.crop_ids, detail.crop_ids);
}

#[test]
fn a_crop_belongs_to_at_most_one_live_plan() {
    // Two plans recommending different nitrogen for one crop would make section
    // 7.1 print two different figures on the same row, and neither would be
    // wrong on its own.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let first = repo::insert_fertilisation_plan(&mut conn, sample(&fx), None).unwrap();

    let mut second = sample(&fx);
    second.needs_n_kg_ha = 90.0;
    let err = repo::insert_fertilisation_plan(&mut conn, second, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("crop_already_planned")
    ));

    // Correcting the plan that already covers it is not a conflict with itself.
    repo::update_fertilisation_plan(
        &mut conn,
        &first.plan.id,
        UpdateFertilisationPlan {
            id: first.plan.id.clone(),
            needs_n_kg_ha: 120.0,
            needs_p2o5_kg_ha: 60.0,
            needs_k2o_kg_ha: 0.0,
            expected_yield_kg_ha: 6500.0,
            preceding_crop_code: Some("60".into()),
            drawn_up_on: "2025-09-20".into(),
            tool_generated: false,
            notes: None,
            crop_ids: vec![fx.wheat.clone()],
        },
        None,
    )
    .unwrap();

    // And withdrawing the first frees the crop for another plan.
    repo::soft_delete_fertilisation_plan(&mut conn, &first.plan.id, None).unwrap();
    let mut third = sample(&fx);
    third.needs_n_kg_ha = 90.0;
    let replacement = repo::insert_fertilisation_plan(&mut conn, third, None).unwrap();
    assert_eq!(replacement.plan.needs_n_kg_ha, 90.0);
}

#[test]
fn refuses_a_crop_from_another_farm_or_another_campaign() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for crop in [&fx.other_farm_crop, &fx.other_season_crop] {
        let mut new = sample(&fx);
        new.crop_ids = vec![crop.clone()];
        let err = repo::insert_fertilisation_plan(&mut conn, new, None).unwrap_err();
        assert!(matches!(
            err,
            module_fertilisation::FertilisationError::Invalid("crop_not_in_this_book")
        ));
    }
    assert!(!fx.other_season_id.is_empty());
}

#[test]
fn refuses_a_plan_that_covers_nothing() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.crop_ids = vec![];
    let err = repo::insert_fertilisation_plan(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("no_crops")
    ));
}

#[test]
fn refuses_a_negative_need_or_a_nonpositive_yield() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut negative = sample(&fx);
    negative.needs_p2o5_kg_ha = -10.0;
    assert!(matches!(
        repo::insert_fertilisation_plan(&mut conn, negative, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("invalid_nutrient_need")
    ));

    let mut no_yield = sample(&fx);
    no_yield.expected_yield_kg_ha = 0.0;
    assert!(matches!(
        repo::insert_fertilisation_plan(&mut conn, no_yield, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("invalid_expected_yield")
    ));
}

#[test]
fn a_plan_without_a_preceding_crop_is_valid() {
    // A unit coming out of fallow has none, and inventing one would be a
    // statement about a rotation that did not happen.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.preceding_crop_code = None;

    let detail = repo::insert_fertilisation_plan(&mut conn, new, None).unwrap();
    assert!(detail.plan.preceding_crop_code.is_none());
}

#[test]
fn adjusting_a_plan_mid_campaign_is_a_correction_with_a_new_date() {
    // Art. 6 explicitly lets a plan be adjusted during the campaign to follow
    // the crop and the weather, so this is the normal case, not the exception.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let created = repo::insert_fertilisation_plan(&mut conn, sample(&fx), Some("user-1")).unwrap();

    let detail = repo::update_fertilisation_plan(
        &mut conn,
        &created.plan.id,
        UpdateFertilisationPlan {
            id: created.plan.id.clone(),
            needs_n_kg_ha: 110.0,
            needs_p2o5_kg_ha: 60.0,
            needs_k2o_kg_ha: 30.0,
            expected_yield_kg_ha: 5800.0,
            preceding_crop_code: Some("60".into()),
            drawn_up_on: "2026-02-11".into(),
            tool_generated: true,
            notes: Some("Ajuste tras las lluvias de enero".into()),
            crop_ids: vec![fx.wheat.clone(), fx.barley.clone()],
        },
        Some("user-1"),
    )
    .unwrap();

    assert_eq!(detail.plan.needs_n_kg_ha, 110.0);
    assert_eq!(detail.plan.drawn_up_on, "2026-02-11");
    assert!(detail.plan.tool_generated);
    assert_eq!(detail.crop_ids.len(), 2);

    let (op, before, after) = last_change(&conn, "fertilisation_plan", &created.plan.id);
    assert_eq!(op, "update");
    assert_eq!(before["needs_n_kg_ha"], 140.0);
    assert_eq!(after["needs_n_kg_ha"], 110.0);
}

#[test]
fn logs_a_complete_row_image_for_the_plan_and_each_covered_crop() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let detail = repo::insert_fertilisation_plan(&mut conn, sample(&fx), Some("user-1")).unwrap();

    let (op, _, after) = last_change(&conn, "fertilisation_plan", &detail.plan.id);
    assert_eq!(op, "insert");
    assert_eq!(after["needs_n_kg_ha"], 140.0);
    assert_eq!(after["season_id"], detail.plan.season_id);

    let row_id: String = conn
        .query_row(
            "SELECT id FROM fertilisation_plan_crop WHERE fertilisation_plan_id = ?1",
            [&detail.plan.id],
            |r| r.get(0),
        )
        .unwrap();
    let (op, _, after) = last_change(&conn, "fertilisation_plan_crop", &row_id);
    assert_eq!(op, "insert");
    // The junction has no model struct, so the log image must carry the parent
    // id: a receiving device rebuilds the row from `after` alone.
    assert_eq!(after["fertilisation_plan_id"], detail.plan.id);
    assert_eq!(after["crop_id"], fx.wheat);
}

#[test]
fn a_season_holding_only_a_plan_reports_itself_in_use() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());

    let created = repo::insert_fertilisation_plan(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());

    repo::soft_delete_fertilisation_plan(&mut conn, &created.plan.id, None).unwrap();
    assert!(
        repo::season_has_records(&conn, &fx.season_id).unwrap(),
        "a soft-deleted plan's audit history is only reachable through its season"
    );
}

#[test]
fn deleting_a_plan_takes_its_covered_crops_with_it_but_not_the_crops() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let created = repo::insert_fertilisation_plan(&mut conn, sample(&fx), None).unwrap();

    conn.execute(
        "DELETE FROM fertilisation_plan WHERE id = ?1",
        [&created.plan.id],
    )
    .unwrap();

    let orphans: i64 = conn
        .query_row("SELECT COUNT(*) FROM fertilisation_plan_crop", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(orphans, 0);
    // The crop itself is a core row and survives — the junction is the child.
    let crops: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM crop WHERE id = ?1",
            [&fx.wheat],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(crops, 1);
}
