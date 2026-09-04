// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PHI status per plot — the derivation the map's tint reads.
//!
//! Compliance logic, written test-first: the window is
//! `[application_date, phi_end_date)`, the end date being the first day harvest
//! is allowed.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::treatment::*;
use module_cue::CueError;
use module_cue::date;
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.

// --- PHI status per plot (map overlay; test-first) --------------------------
//
// The window rule is `[application_date, phi_end_date)` — phi_end_date is the
// first day harvest is allowed again (RD 1311/2012 "plazo de seguridad"; same
// convention as alerts::phi_window_is_active, whose tests pin the boundary
// days). These tests pin the per-plot aggregation on top of that rule.

/// One treatment on the given plots at an explicit date/PHI; returns the record.
fn treat_on(
    conn: &mut Connection,
    fx: &Fixture,
    plot_ids: &[&str],
    application_date: &str,
    phi_days: i64,
) -> TreatmentRecord {
    let mut new = sample_treatment(fx, None, Some(phi_days));
    new.application_date = application_date.into();
    let plots = plot_ids
        .iter()
        .map(|id| NewTreatmentPlot {
            plot_id: (*id).into(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        })
        .collect();
    repo::insert_treatment_record(conn, new, plots, None).unwrap()
}

#[test]
fn phi_status_in_window_from_the_application_day() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    // PHI 21 applied 2026-05-01 → harvest allowed from 2026-05-22.
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    for today in ["2026-05-01", "2026-05-10", "2026-05-21"] {
        let rows =
            repo::phi_status_for_farm(&conn, &fx.farm_id, today, repo::default_phi_horizon_days())
                .unwrap();
        let status = status_of(&rows, &plot);
        assert!(status.in_phi, "must be in PHI on {today}");
        assert_eq!(status.phi_until.as_deref(), Some("2026-05-22"));
    }
}

#[test]
fn phi_status_clear_on_the_end_date_itself() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    // phi_end_date is the first day harvest is allowed → clear, but still
    // listed: "treated and harvest allowed" is a state the map shows.
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-22",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    let status = status_of(&rows, &plot);
    assert!(!status.in_phi);
    assert_eq!(status.phi_until, None);
}

#[test]
fn phi_status_takes_the_latest_end_among_live_windows() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 7); // ends 2026-05-08
    treat_on(&mut conn, &fx, &[&plot], "2026-05-03", 21); // ends 2026-05-24

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-05",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    let status = status_of(&rows, &plot);
    assert!(status.in_phi);
    assert_eq!(status.phi_until.as_deref(), Some("2026-05-24"));

    // After the shorter window lapses the longer one still rules.
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert_eq!(
        status_of(&rows, &plot).phi_until.as_deref(),
        Some("2026-05-24")
    );
}

#[test]
fn phi_status_ignores_windows_not_yet_started() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    // Planned/future-dated record: the window has not opened yet.
    treat_on(&mut conn, &fx, &[&plot], "2026-06-01", 21);

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-20",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    let status = status_of(&rows, &plot);
    assert!(!status.in_phi);
    assert_eq!(status.phi_until, None);
}

#[test]
fn phi_status_multi_plot_treatment_marks_every_treated_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let a = add_status_plot(&mut conn, &fx.farm_id, "A");
    let b = add_status_plot(&mut conn, &fx.farm_id, "B");
    treat_on(&mut conn, &fx, &[&a, &b], "2026-05-01", 21);

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(status_of(&rows, &a).in_phi);
    assert!(status_of(&rows, &b).in_phi);
}

#[test]
fn phi_status_excludes_deleted_records_untreated_plots_and_other_farms() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let treated = add_status_plot(&mut conn, &fx.farm_id, "A");
    let _untreated = add_status_plot(&mut conn, &fx.farm_id, "B");
    let record = treat_on(&mut conn, &fx, &[&treated], "2026-05-01", 21);

    // A second farm with its own in-window treatment must not leak in.
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let other_plot = add_status_plot(&mut conn, &other_farm, "C");
    let mut other = sample_treatment(&fx, None, Some(21));
    other.farm_id = other_farm.clone();
    repo::insert_treatment_record(
        &mut conn,
        other,
        vec![NewTreatmentPlot {
            plot_id: other_plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "only the treated plot of this farm is listed"
    );
    assert_eq!(rows[0].plot_id, treated);

    // Soft-deleting the only record removes the plot from the status list —
    // deleted records carry no PHI restriction.
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn phi_status_spans_seasons() {
    // PHI is a physical restriction on the plot — a window opened by a record
    // filed under another campaign still binds today.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    let old_season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2025,
            label: "2025".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let mut new = sample_treatment(&fx, None, Some(21));
    new.season_id = old_season.id;
    new.application_date = "2026-05-01".into();
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert!(status_of(&rows, &plot).in_phi);
}

// --- the recency horizon (2026-08-24) ---------------------------------------
//
// The tint has two states and they are scoped differently. `in_phi` is a
// question about TODAY and must never be narrowed by campaign — that is
// `phi_status_spans_seasons` above. "Clear" is a question about the recent
// past, and it is bounded by the horizon, because "treated at some point
// and currently clear" is true of every plot on a holding farmed for a decade:
// a statement that costs a scan of the whole record book to make and says
// nothing by the time it is made.

#[test]
fn phi_status_lists_a_plot_cleared_inside_the_horizon() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    // Applied 1 May with a 21-day PHI: the window closed on 22 May.
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    // A month later the plot is still worth showing as treated-and-clear.
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-06-22",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    let status = status_of(&rows, &plot);
    assert!(!status.in_phi);
    assert_eq!(status.phi_until, None);
}

#[test]
fn phi_status_forgets_a_plot_whose_window_closed_beyond_the_horizon() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21); // closed 2026-05-22

    // A horizon past the window closing, the plot drops off the map — it
    // is indistinguishable from one never treated, which is the honest state.
    let past = date::add_days("2026-05-22", repo::default_phi_horizon_days() + 1).unwrap();
    let rows =
        repo::phi_status_for_farm(&conn, &fx.farm_id, &past, repo::default_phi_horizon_days())
            .unwrap();
    assert!(rows.is_empty(), "still listed on {past}");

    // The day before, it is still there — the boundary is where it says it is.
    let inside = date::add_days("2026-05-22", repo::default_phi_horizon_days()).unwrap();
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        &inside,
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert_eq!(rows.len(), 1, "dropped a day early, on {inside}");
}

#[test]
fn a_widened_horizon_reaches_further_back_and_a_narrowed_one_less_far() {
    // The horizon became a device setting on 2026-08-26, so it has to be the
    // thing that decides — a value read from a constant inside would make this
    // test pass while the farmer's choice did nothing.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21); // window closed 2026-05-22

    // 200 days on: outside the default 90-day horizon, inside a 365-day one.
    let later = date::add_days("2026-05-22", 200).unwrap();
    assert!(
        repo::phi_status_for_farm(&conn, &fx.farm_id, &later, repo::default_phi_horizon_days())
            .unwrap()
            .is_empty(),
        "the default horizon should already have forgotten it"
    );
    assert_eq!(
        repo::phi_status_for_farm(&conn, &fx.farm_id, &later, 365)
            .unwrap()
            .len(),
        1,
        "a farmer who widened the horizon must see it again"
    );

    // ...and narrowing forgets sooner than the default would.
    let soon = date::add_days("2026-05-22", 30).unwrap();
    assert!(
        repo::phi_status_for_farm(&conn, &fx.farm_id, &soon, 7)
            .unwrap()
            .is_empty(),
        "a narrowed horizon must forget sooner, not merely not-later"
    );
}

#[test]
fn an_unset_horizon_follows_the_default_and_a_set_one_wins() {
    assert_eq!(
        repo::phi_horizon_days(None),
        repo::default_phi_horizon_days(),
        "unset must track the code default, not a value captured at write time"
    );
    assert_eq!(repo::phi_horizon_days(Some(365)), 365);
}

#[test]
fn a_horizon_is_accepted_across_the_offered_range_and_refused_outside_it() {
    for days in [
        repo::MIN_PHI_HORIZON_DAYS,
        repo::default_phi_horizon_days(),
        repo::MAX_PHI_HORIZON_DAYS,
    ] {
        assert!(repo::validate_phi_horizon_days(days).is_ok(), "{days}");
    }
    // The ceiling is what keeps the tint's cost bounded now that the value is
    // the farmer's — see `query_scope.rs`, which pins the read at the maximum.
    for days in [
        repo::MIN_PHI_HORIZON_DAYS - 1,
        repo::MAX_PHI_HORIZON_DAYS + 1,
        0,
        -90,
    ] {
        assert!(
            matches!(
                repo::validate_phi_horizon_days(days),
                Err(CueError::Invalid("phi_horizon_out_of_range"))
            ),
            "{days} should have been refused"
        );
    }
}

#[test]
fn an_old_window_never_suppresses_a_live_one_on_the_same_plot() {
    // The horizon bounds the CLEAR state only. A plot treated years ago and
    // again last week is restricted, and the ancient record must not change
    // that in either direction.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2020-04-01", 21);
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert_eq!(rows.len(), 1, "one plot, one status");
    let status = status_of(&rows, &plot);
    assert!(status.in_phi);
    assert_eq!(status.phi_until.as_deref(), Some("2026-05-22"));
}

#[test]
fn phi_status_excludes_deleted_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    terrazgo_core::repository::soft_delete_plot(&mut conn, &plot, None).unwrap();
    let rows = repo::phi_status_for_farm(
        &conn,
        &fx.farm_id,
        "2026-05-10",
        repo::default_phi_horizon_days(),
    )
    .unwrap();
    assert!(rows.is_empty(), "a deleted plot has no map presence");
}
