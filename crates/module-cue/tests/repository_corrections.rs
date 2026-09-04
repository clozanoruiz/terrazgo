// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Correcting a stored record, and who is recorded as having done it.
//!
//! Nothing forbids correction — RD 1311/2012 art. 16 has no provision on
//! modifying entries, and SIEX models records as mutable by design — so a
//! correction is an update with a re-derived `phi_end_date`, not a withdrawal
//! plus a new event. Also here: the season deletion guard's module half (core
//! may never reference a module table, so the shell chains the two checks) and
//! the actor stamp.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::treatment::*;
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.

// ---------------------------------------------------------------------------
// Actor stamping (record_change.actor)
// ---------------------------------------------------------------------------

/// A treatment insert stamps the acting profile id on every row it logs —
/// the record AND its junction rows — and the deletion write records its own
/// author independently.
#[test]
fn treatment_writes_stamp_the_actor_on_every_logged_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        Some("profile-ana"),
    )
    .unwrap();

    let actors: Vec<Option<String>> = {
        let mut stmt = conn
            .prepare(
                "SELECT actor FROM record_change
                 WHERE entity_table IN
                   ('treatment_record', 'treatment_plot', 'treatment_problem',
                    'treatment_justification')
                 ORDER BY id",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert!(actors.len() >= 4, "record + plot + problem + justification");
    assert!(
        actors.iter().all(|a| a.as_deref() == Some("profile-ana")),
        "every row logged in the insert carries the same author: {actors:?}"
    );

    // The delete write is attributed to whoever deleted, not the creator.
    repo::soft_delete_treatment_record(&mut conn, &record.id, Some("profile-marta")).unwrap();
    let delete_actor: Option<String> = conn
        .query_row(
            "SELECT actor FROM record_change
             WHERE entity_table = 'treatment_record' AND operation = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(delete_actor.as_deref(), Some("profile-marta"));
}

// --- season deletion guard (the module half) --------------------------------

/// `season_has_treatments` is what the shell's `delete_season` chains before
/// calling core's `soft_delete_season`: core owns the season row but may never
/// reference `treatment_record`, so the module answers for its own table.
/// Soft-deleted records still count — their audit history is only reachable
/// through the season they belong to.
#[test]
fn season_has_treatments_sees_records_including_soft_deleted_ones() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    let empty = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    assert!(!repo::season_has_treatments(&conn, &fx.season_id).unwrap());
    assert!(!repo::season_has_treatments(&conn, &empty.id).unwrap());

    let plot = add_plot(&mut conn, &fx.farm_id, "Parcela 1");
    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert!(repo::season_has_treatments(&conn, &fx.season_id).unwrap());
    assert!(
        !repo::season_has_treatments(&conn, &empty.id).unwrap(),
        "the guard is per season, not global"
    );

    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();
    assert!(
        repo::season_has_treatments(&conn, &fx.season_id).unwrap(),
        "a soft-deleted record still pins its season"
    );
}

/// `crop_ids_with_treatments` is the guard the shell hands to the SIGPAC
/// declared-crops import: a crop this season's treatments point at may not be
/// rewritten from a third party's declaration, because the record book would
/// then state one crop in section 2.1 and another beside the treatment.
#[test]
fn crop_ids_with_treatments_reports_only_live_records_of_this_farm_and_season() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    let plot = add_plot(&mut conn, &fx.farm_id, "Parcela 1");
    let new_crop = |species: &str, season_id: &str| NewCrop {
        plot_id: plot.clone(),
        season_id: season_id.into(),
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
    };
    let treated = repo::insert_crop(&mut conn, new_crop("cebada", &fx.season_id), None)
        .unwrap()
        .id;
    let untouched = repo::insert_crop(&mut conn, new_crop("veza", &fx.season_id), None)
        .unwrap()
        .id;

    assert!(
        repo::crop_ids_with_treatments(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: Some(treated.clone()),
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    let guarded = repo::crop_ids_with_treatments(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(guarded.contains(&treated));
    assert!(
        !guarded.contains(&untouched),
        "an untreated crop stays freely replaceable"
    );

    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra finca".into(),
            country_code: "es".into(),
            owner_name: None,
            owner_tax_id: None,
            es: None,
        },
        None,
    )
    .unwrap();
    assert!(
        repo::crop_ids_with_treatments(&conn, &fx.season_id, &other_farm.id)
            .unwrap()
            .is_empty(),
        "the guard is per farm"
    );

    // A record that left the book releases its crop: nothing printed refers to
    // it any more, so the import may propose over it again.
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();
    assert!(
        repo::crop_ids_with_treatments(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
}

// --- correcting a stored record (slice D) -----------------------------------
//
// Nothing in the sources forbids it: RD 1311/2012 art. 16 has no provision on
// modifying an entry, Reglamento (UE) 2023/564 none on integrity or change
// logs, and SIEX 3.11.4 models a correction as re-sending the same
// `IdAjenaTratamFito` with new values, reserving `Borrar` for withdrawal.

#[test]
fn correcting_the_date_re_derives_the_plazo_de_seguridad() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);
    // 2026-05-01 + 14 days (RD 1311/2012: the plazo runs from the application).
    assert_eq!(record.phi_end_date.as_deref(), Some("2026-05-15"));

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    update.application_date = "2026-05-10".into();
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();
    assert_eq!(fixed.record.phi_end_date.as_deref(), Some("2026-05-24"));
}

#[test]
fn a_corrected_interval_counts_the_plazo_from_its_end() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    // The actuation actually spanned three days. The plazo de seguridad is the
    // time between the LAST application and harvest, so it runs from the end.
    update.application_end_date = Some("2026-05-03".into());
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();
    assert_eq!(fixed.record.phi_end_date.as_deref(), Some("2026-05-17"));
}

#[test]
fn a_correction_keeps_snapshots_whose_row_it_did_not_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    // The registry entry is corrected after the record was written.
    conn.execute(
        "UPDATE product SET commercial_name = 'Fungitop Extra' WHERE id = ?1",
        [&fx.product_id],
    )
    .unwrap();
    conn.execute(
        "UPDATE operator SET full_name = 'Carlos Pérez Gómez' WHERE id = ?1",
        [&fx.operator_id],
    )
    .unwrap();

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 2.5,
            growth_stage_code: None,
        }],
    );
    update.notes = Some("superficie corregida".into());
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();

    // Correcting the surface must not rewrite what the record printed about
    // rows it never named — that is what the snapshot columns are for.
    assert_eq!(
        fixed.record.product_name_snapshot.as_deref(),
        Some("Fungitop")
    );
    assert_eq!(fixed.record.operator_name_snapshot, "Carlos Pérez");
}

#[test]
fn changing_the_product_re_takes_its_snapshot() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let other = repo::insert_product_with_authorisation(
        &mut conn,
        NewProduct {
            commercial_name: "Insectop".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: Some(7),
        },
        ProductAuthorisationFields {
            country_code: "es".into(),
            authorisation_number: "ES-26.999".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2025-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap()
    .product;

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    update.product_id = Some(other.id.clone());
    // A different product is a different application: its own printed values.
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();
    assert_eq!(
        fixed.record.product_name_snapshot.as_deref(),
        Some("Insectop")
    );
    assert_eq!(
        fixed.record.authorisation_number_snapshot.as_deref(),
        Some("ES-26.999")
    );
}

#[test]
fn a_correction_can_withdraw_the_product_for_a_non_chemical_measure() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    // It was not a spray after all: pheromone diffusers were hung instead.
    update.product_id = None;
    update.dose_value = None;
    update.dose_unit_code = None;
    update.phi_days_used = None;
    update.measure_code = Some("4".into());
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();

    // The whole chemical block goes together (the table CHECK), and with no
    // product there is no plazo de seguridad left to run.
    assert!(fixed.record.product_id.is_none());
    assert!(fixed.record.dose_value.is_none());
    assert!(fixed.record.phi_days_used.is_none());
    assert!(fixed.record.phi_end_date.is_none());
    assert!(fixed.record.product_name_snapshot.is_none());
    assert_eq!(fixed.record.measure_code.as_deref(), Some("4"));
}

#[test]
fn a_correction_reconciles_the_treated_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);
    let second = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Segunda".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let before = repo::get_treatment_record(&conn, &record.id).unwrap();
    let survivor_row_id = before.plots[0].id.clone();

    let update = correction_of(
        &record,
        vec![
            NewTreatmentPlot {
                plot_id: plot.clone(),
                crop_id: None,
                surface_treated_ha: 2.0, // corrected
                growth_stage_code: None,
            },
            NewTreatmentPlot {
                plot_id: second.clone(),
                crop_id: None,
                surface_treated_ha: 1.0, // added
                growth_stage_code: None,
            },
        ],
    );
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();
    assert_eq!(fixed.plots.len(), 2);
    let survivor = fixed.plots.iter().find(|p| p.plot_id == plot).unwrap();
    assert_eq!(survivor.surface_treated_ha, 2.0);
    // The survivor keeps its row id, so its audit history stays one thread.
    assert_eq!(survivor.id, survivor_row_id);

    // And dropping one removes it rather than leaving a stale row behind.
    let update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: second,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
    );
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();
    assert_eq!(fixed.plots.len(), 1);
}

#[test]
fn a_correction_reconciles_problems_and_justifications() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    update.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "2".into(),
    }];
    update.justifications = vec!["advisor_recommendation".into(), "monitoring".into()];
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();

    assert_eq!(fixed.problems.len(), 1);
    assert_eq!(fixed.problems[0].problem_code, "2");
    assert_eq!(fixed.justifications.len(), 2);
}

#[test]
fn a_correction_refuses_a_plot_from_another_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, _plot) = correctable_record(&mut conn, &fx);
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra finca".into(),
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
            name: "Ajena".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: foreign_plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
    );
    assert!(matches!(
        repo::update_treatment_record(&mut conn, &record.id, update, None),
        Err(module_cue::CueError::PlotNotOnFarm { .. })
    ));
}

#[test]
fn a_correction_logs_complete_before_and_after_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    update.application_date = "2026-05-02".into();
    repo::update_treatment_record(&mut conn, &record.id, update, Some("user-1")).unwrap();

    let (payload, actor): (String, Option<String>) = conn
        .query_row(
            "SELECT payload, actor FROM record_change
             WHERE entity_table = 'treatment_record' AND operation = 'update'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(actor.as_deref(), Some("user-1"));
    assert_eq!(payload["before"]["application_date"], "2026-05-01");
    assert_eq!(payload["after"]["application_date"], "2026-05-02");
    // Complete row images, both sides: a receiving device rebuilds from them.
    assert!(payload["before"]["operator_name_snapshot"].is_string());
    assert!(payload["after"]["operator_name_snapshot"].is_string());
}

#[test]
fn a_deleted_record_cannot_be_corrected() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();

    let update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    assert!(matches!(
        repo::update_treatment_record(&mut conn, &record.id, update, None),
        Err(module_cue::CueError::NotFound)
    ));
}
