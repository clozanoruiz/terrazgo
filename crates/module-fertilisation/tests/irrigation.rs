// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 8 — the irrigation register.
//!
//! The rules pinned here come from three sources, and each test names its own:
//! RD 1051/2022 art. 5.e and art. 17.2, and RD 1311/2012 Anexo III Parte I
//! sección C (letters a, b and l), which art. 5.e redirects to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_fertilisation::models::*;
use module_fertilisation::open_in_memory;
use module_fertilisation::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::{NewFarm, NewPlot, NewSeason};
use terrazgo_core::repository as core_repo;

struct Fixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
    other_farm_plot: String,
}

fn fixture(conn: &mut Connection) -> Fixture {
    let season = core_repo::insert_season(
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
    let farm = |conn: &mut Connection, name: &str| {
        core_repo::insert_farm(
            conn,
            NewFarm {
                name: name.into(),
                owner_name: None,
                owner_tax_id: None,
                country_code: "es".into(),
                es: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let farm_id = farm(conn, "Finca La Vega");
    let other_farm = farm(conn, "Finca del Vecino");

    let plot = |conn: &mut Connection, farm_id: &str, name: &str, area: f64| {
        core_repo::insert_plot(
            conn,
            NewPlot {
                farm_id: farm_id.to_string(),
                name: name.into(),
                area_ha: Some(area),
                es: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let plot_a = plot(conn, &farm_id, "El Prado", 4.0);
    let plot_b = plot(conn, &farm_id, "La Loma", 3.0);
    let other_farm_plot = plot(conn, &other_farm, "Ajena", 2.0);

    Fixture {
        season_id: season.id,
        farm_id,
        plot_a,
        plot_b,
        other_farm_plot,
    }
}

fn sample(fx: &Fixture) -> NewIrrigationRecord {
    NewIrrigationRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        irrigated_on: "2026-06-14".into(),
        irrigation_end_date: None,
        irrigation_method_code: "drip".into(),
        volume_value: 320.0,
        volume_unit_code: "m3_ha".into(),
        water_nitric_n_mg_l: None,
        water_soluble_p2o5_mg_l: None,
        energy_type_code: None,
        meter_number: None,
        notes: None,
        plots: vec![NewIrrigationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            irrigated_area_ha: Some(3.5),
        }],
        water_origins: vec![],
    }
}

fn last_change(conn: &Connection, table: &str, id: &str) -> (String, serde_json::Value) {
    conn.query_row(
        "SELECT operation, payload FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map(|(op, payload)| (op, serde_json::from_str(&payload).unwrap()))
    .unwrap()
}

#[test]
fn records_an_irrigation_with_its_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let detail = repo::insert_irrigation_record(&mut conn, sample(&fx), Some("user-1")).unwrap();

    assert_eq!(detail.record.irrigation_method_code, "drip");
    assert_eq!(detail.record.volume_value, 320.0);
    assert_eq!(detail.record.volume_unit_code, "m3_ha");
    assert_eq!(detail.plots.len(), 1);
    assert_eq!(detail.plots[0].irrigated_area_ha, Some(3.5));
    // A single-day irrigation leaves the interval end NULL rather than
    // repeating the start: a serializer can then tell "one day" from "a period
    // that happened to last one day".
    assert!(detail.record.irrigation_end_date.is_none());
}

#[test]
fn logs_a_complete_row_image_for_the_record_and_each_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let detail = repo::insert_irrigation_record(&mut conn, sample(&fx), Some("user-1")).unwrap();

    let (op, payload) = last_change(&conn, "irrigation_record", &detail.record.id);
    assert_eq!(op, "insert");
    // Complete row image: Stage-2/3 sync must rebuild the row from `after`
    // alone, so a hand-picked subset would be a bug.
    assert_eq!(payload["after"]["volume_unit_code"], "m3_ha");
    assert_eq!(payload["after"]["irrigation_method_code"], "drip");
    assert_eq!(payload["after"]["season_id"], detail.record.season_id);

    let (op, payload) = last_change(&conn, "irrigation_plot", &detail.plots[0].id);
    assert_eq!(op, "insert");
    assert_eq!(payload["after"]["plot_id"], fx.plot_a);

    let actor: Option<String> = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1 LIMIT 1",
            [&detail.record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor.as_deref(), Some("user-1"));
}

#[test]
fn accepts_a_date_interval() {
    // RD 1051/2022 art. 5.f: intensive and fertigated crops may accumulate the
    // record over fortnightly periods, and the SIEX twin requires both ends.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.irrigated_on = "2026-06-01".into();
    new.irrigation_end_date = Some("2026-06-15".into());

    let detail = repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    assert_eq!(
        detail.record.irrigation_end_date.as_deref(),
        Some("2026-06-15")
    );
}

#[test]
fn rejects_an_interval_that_ends_before_it_starts() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.irrigated_on = "2026-06-15".into();
    new.irrigation_end_date = Some("2026-06-01".into());

    let err = repo::insert_irrigation_record(&mut conn, new, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_date_interval")
    ));
}

#[test]
fn records_the_conditional_water_quality_figures() {
    // Anexo III C.l asks for the nitric nitrogen and water-soluble phosphorus
    // already in the irrigation water. RD 1051/2022 art. 17.2 makes them
    // conditional on the basin authority or irrigators' community supplying
    // them, so they are stored when known and left blank when not.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_nitric_n_mg_l = Some(12.4);
    new.water_soluble_p2o5_mg_l = Some(0.8);

    let detail = repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    assert_eq!(detail.record.water_nitric_n_mg_l, Some(12.4));
    assert_eq!(detail.record.water_soluble_p2o5_mg_l, Some(0.8));
}

#[test]
fn water_quality_is_optional_but_never_negative() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    // Silence is the normal case, not an error: art. 17.2 only asks for these
    // when someone supplies them.
    let quiet = repo::insert_irrigation_record(&mut conn, sample(&fx), None).unwrap();
    assert!(quiet.record.water_nitric_n_mg_l.is_none());

    // A negative concentration is a typo, not a measurement.
    let mut bad = sample(&fx);
    bad.water_nitric_n_mg_l = Some(-1.0);
    let err = repo::insert_irrigation_record(&mut conn, bad, None).unwrap_err();
    assert!(matches!(
        err,
        module_fertilisation::FertilisationError::Invalid("invalid_water_quality")
    ));
}

#[test]
fn volume_must_be_positive_and_measured_in_a_water_unit() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut zero = sample(&fx);
    zero.volume_value = 0.0;
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, zero, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("invalid_irrigation_volume")
    ));

    // kg/ha is a perfectly good unit and a nonsense answer here. The foreign
    // key cannot catch this — it only says the code is a unit at all.
    let mut wrong_unit = sample(&fx);
    wrong_unit.volume_unit_code = "kg_ha".into();
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, wrong_unit, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("invalid_volume_unit")
    ));
}

#[test]
fn rejects_an_unknown_irrigation_method() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.irrigation_method_code = "hosepipe".into();
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, new, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("unknown_irrigation_method")
    ));
}

#[test]
fn refuses_a_plot_on_another_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.plots = vec![NewIrrigationPlot {
        plot_id: fx.other_farm_plot.clone(),
        crop_id: None,
        irrigated_area_ha: None,
    }];
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, new, None).unwrap_err(),
        module_fertilisation::FertilisationError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn requires_at_least_one_plot_and_folds_duplicates() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut none = sample(&fx);
    none.plots = vec![];
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, none, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("no_plots")
    ));

    // A form that lists a plot twice means one irrigation, not an error.
    let mut twice = sample(&fx);
    twice.plots = vec![
        NewIrrigationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            irrigated_area_ha: Some(3.5),
        },
        NewIrrigationPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: None,
            irrigated_area_ha: Some(3.5),
        },
    ];
    let detail = repo::insert_irrigation_record(&mut conn, twice, None).unwrap();
    assert_eq!(detail.plots.len(), 1);
}

#[test]
fn the_irrigated_surface_may_be_left_blank() {
    // The model prints the column, but naming the plot already says what was
    // watered — an invented hectare figure would be worse than a blank cell.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.plots[0].irrigated_area_ha = None;

    let detail = repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    assert!(detail.plots[0].irrigated_area_ha.is_none());
}

#[test]
fn records_several_water_origins_in_catalogue_order() {
    // SIEX `Riego.OrigenAgua` is an ARRAY: one irrigation can mix a river and
    // a borehole, which is why this is a junction and not a column.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_origins = vec!["groundwater".into(), "surface".into()];

    let detail = repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    // Listed the way the provider lists them, not by insertion accident.
    assert_eq!(detail.water_origins, vec!["surface", "groundwater"]);
}

#[test]
fn rejects_an_unknown_water_origin() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_origins = vec!["snowmelt".into()];
    assert!(matches!(
        repo::insert_irrigation_record(&mut conn, new, None).unwrap_err(),
        module_fertilisation::FertilisationError::Invalid("unknown_water_origin")
    ));
}

#[test]
fn corrects_a_record_in_place_and_reconciles_its_plots() {
    // Fully correctable from the start, the `seed_treatment` condition: this
    // record holds no snapshot of another row's identity, so there is nothing
    // a later edit elsewhere could rewrite underneath it.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_origins = vec!["surface".into()];
    let created = repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    let kept_plot_row_id = created.plots[0].id.clone();

    let updated = repo::update_irrigation_record(
        &mut conn,
        &created.record.id,
        UpdateIrrigationRecord {
            id: created.record.id.clone(),
            irrigated_on: "2026-06-14".into(),
            irrigation_end_date: None,
            irrigation_method_code: "sprinkler_fixed".into(),
            volume_value: 410.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: Some("1".into()),
            meter_number: Some("CT-77".into()),
            notes: None,
            plots: vec![
                NewIrrigationPlot {
                    plot_id: fx.plot_a.clone(),
                    crop_id: None,
                    irrigated_area_ha: Some(4.0),
                },
                NewIrrigationPlot {
                    plot_id: fx.plot_b.clone(),
                    crop_id: None,
                    irrigated_area_ha: Some(2.0),
                },
            ],
            water_origins: vec!["groundwater".into()],
        },
        Some("user-1"),
    )
    .unwrap();

    assert_eq!(updated.record.irrigation_method_code, "sprinkler_fixed");
    assert_eq!(updated.record.volume_value, 410.0);
    assert_eq!(updated.record.meter_number.as_deref(), Some("CT-77"));
    assert_eq!(updated.plots.len(), 2);
    assert_eq!(updated.water_origins, vec!["groundwater"]);

    // The plot that stayed keeps its identity, so the audit trail reads as a
    // correction rather than a delete plus an insert.
    let still_there = updated
        .plots
        .iter()
        .find(|p| p.plot_id == fx.plot_a)
        .unwrap();
    assert_eq!(still_there.id, kept_plot_row_id);
    assert_eq!(still_there.irrigated_area_ha, Some(4.0));

    let (op, payload) = last_change(&conn, "irrigation_record", &created.record.id);
    assert_eq!(op, "update");
    assert_eq!(payload["before"]["irrigation_method_code"], "drip");
    assert_eq!(
        payload["after"]["irrigation_method_code"],
        "sprinkler_fixed"
    );
}

#[test]
fn a_removed_water_origin_leaves_a_logged_deletion() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_origins = vec!["surface".into()];
    let created = repo::insert_irrigation_record(&mut conn, new, None).unwrap();

    let origin_row_id: String = conn
        .query_row(
            "SELECT id FROM irrigation_water_origin WHERE irrigation_record_id = ?1",
            [&created.record.id],
            |r| r.get(0),
        )
        .unwrap();

    repo::update_irrigation_record(
        &mut conn,
        &created.record.id,
        UpdateIrrigationRecord {
            id: created.record.id.clone(),
            irrigated_on: created.record.irrigated_on.clone(),
            irrigation_end_date: None,
            irrigation_method_code: created.record.irrigation_method_code.clone(),
            volume_value: created.record.volume_value,
            volume_unit_code: created.record.volume_unit_code.clone(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![NewIrrigationPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: None,
                irrigated_area_ha: Some(3.5),
            }],
            water_origins: vec![],
        },
        Some("user-1"),
    )
    .unwrap();

    // The trail still says what the farmer once stated the water came from.
    let (op, payload) = last_change(&conn, "irrigation_water_origin", &origin_row_id);
    assert_eq!(op, "delete");
    assert_eq!(payload["before"]["origin_code"], "surface");
}

#[test]
fn soft_delete_hides_the_record_but_keeps_its_history() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let created = repo::insert_irrigation_record(&mut conn, sample(&fx), None).unwrap();

    repo::soft_delete_irrigation_record(&mut conn, &created.record.id, Some("user-1")).unwrap();

    let listed = repo::list_irrigation_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(listed.is_empty());

    let (op, payload) = last_change(&conn, "irrigation_record", &created.record.id);
    assert_eq!(op, "delete");
    assert!(payload["before"]["deleted_at"].is_null());
    assert!(payload["after"]["deleted_at"].is_string());
}

#[test]
fn lists_records_oldest_first() {
    // A record book reads chronologically.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for date in ["2026-07-02", "2026-05-20", "2026-06-14"] {
        let mut new = sample(&fx);
        new.irrigated_on = date.into();
        repo::insert_irrigation_record(&mut conn, new, None).unwrap();
    }

    let listed = repo::list_irrigation_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let dates: Vec<_> = listed
        .iter()
        .map(|d| d.record.irrigated_on.as_str())
        .collect();
    assert_eq!(dates, vec!["2026-05-20", "2026-06-14", "2026-07-02"]);
}

#[test]
fn a_season_holding_an_irrigation_reports_itself_in_use() {
    // The shell chains this before deleting a season: every register is
    // season-scoped, and hiding the season would hide its records.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    assert!(!repo::season_has_records(&conn, &fx.season_id).unwrap());

    let created = repo::insert_irrigation_record(&mut conn, sample(&fx), None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());

    // Soft-deleted records still count: their audit history is only reachable
    // through the season they belong to.
    repo::soft_delete_irrigation_record(&mut conn, &created.record.id, None).unwrap();
    assert!(repo::season_has_records(&conn, &fx.season_id).unwrap());
}

#[test]
fn deleting_a_record_takes_its_children_with_it() {
    // The junctions are pure children — ON DELETE CASCADE, so a hard delete in
    // some future maintenance path cannot leave orphans behind.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.water_origins = vec!["surface".into()];
    let created = repo::insert_irrigation_record(&mut conn, new, None).unwrap();

    conn.execute(
        "DELETE FROM irrigation_record WHERE id = ?1",
        [&created.record.id],
    )
    .unwrap();

    let plots: i64 = conn
        .query_row("SELECT COUNT(*) FROM irrigation_plot", [], |r| r.get(0))
        .unwrap();
    let origins: i64 = conn
        .query_row("SELECT COUNT(*) FROM irrigation_water_origin", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!((plots, origins), (0, 0));
}
