// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The coded fields the sources add to a treatment beyond its bare shape:
//! the problems, justifications and efficacy of Anexo III Parte I B, the
//! actuation interval and the total quantity used (B allows an interval, and
//! B.i is not derivable from a concentration dose), 3.1 bis's advisor
//! (B.d names them in the same sentence as the applicator) and non-chemical
//! measure, and the two conditional fields Reglamento (UE) 2023/564's annex
//! asks for — the start hour and the BBCH stage.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::treatment::*;
use common::{db_with_catalogues, last_change};
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.

// --- coded problems, justifications and efficacy (SIEX gap 3) ----------------
// Design in docs/siex-export.md: the coded problems ARE the reason for
// treatment (RD 1311/2012) and the SIEX export requires ≥1 problem, 1..n
// justifications and an efficacy per TratamFito (schema v3.11.4).

/// Minimal imported catalogue so the insert-time code check has something to
/// resolve against (the app imports the real vendored snapshot at startup;
/// tests seed only what they assert on).
fn seed_disease_catalogue(conn: &Connection) {
    conn.execute(
        "INSERT INTO catalogue (id, source, imported_at) VALUES ('ENFERMEDADES', 'siex', '2026-07-15T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO catalogue_code (catalogue_id, code, label) VALUES ('ENFERMEDADES', '254', 'Septoriosis')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO catalogue_code (catalogue_id, code, label, retired_on)
         VALUES ('ENFERMEDADES', '9', 'Retired disease', '2024-01-01')",
        [],
    )
    .unwrap();
}

#[test]
fn treatment_captures_problems_justifications_and_efficacy() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "P");

    let mut new = sample_treatment(&fx, None, Some(14));
    new.problems = vec![
        NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        },
    ];
    new.justifications = vec!["threshold_exceeded".into(), "monitoring".into()];
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();
    assert!(record.efficacy_code.is_none(), "not observed yet at entry");

    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(fetched.problems.len(), 2);
    assert_eq!(fetched.problems[0].reason_category_code, "disease");
    assert_eq!(fetched.problems[0].problem_code, "254");
    assert_eq!(fetched.problems[1].reason_category_code, "pest");
    assert_eq!(fetched.justifications.len(), 2);
    assert_eq!(
        fetched.justifications[0].justification_code,
        "threshold_exceeded"
    );

    // Junction rows are synced user data → their inserts are audit-logged
    // with complete row images, like treatment_plot rows.
    let logged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table IN ('treatment_problem', 'treatment_justification')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 4);

    // The list view carries the details too.
    let listed = repo::list_treatment_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed[0].problems.len(), 2);
    assert_eq!(listed[0].justifications.len(), 2);
}

#[test]
fn treatment_requires_at_least_one_problem_and_justification() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "P");
    let plots = |p: &str| {
        vec![NewTreatmentPlot {
            plot_id: p.into(),
            crop_id: None,
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }]
    };

    let mut no_problems = sample_treatment(&fx, None, Some(14));
    no_problems.problems = vec![];
    let err =
        repo::insert_treatment_record(&mut conn, no_problems, plots(&plot), None).unwrap_err();
    assert!(matches!(err, module_cue::CueError::Invalid("no_problems")));

    let mut no_justifications = sample_treatment(&fx, None, Some(14));
    no_justifications.justifications = vec![];
    let err = repo::insert_treatment_record(&mut conn, no_justifications, plots(&plot), None)
        .unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("no_justifications")
    ));
}

#[test]
fn duplicate_problems_and_justifications_are_folded() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "P");

    let mut new = sample_treatment(&fx, None, Some(14));
    new.problems = vec![
        NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        },
    ];
    new.justifications = vec!["monitoring".into(), "monitoring".into()];
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(fetched.problems.len(), 1);
    assert_eq!(fetched.justifications.len(), 1);
}

#[test]
fn problem_codes_are_validated_against_imported_catalogues() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "P");
    seed_disease_catalogue(&conn);
    let plots = |p: &str| {
        vec![NewTreatmentPlot {
            plot_id: p.into(),
            crop_id: None,
            surface_treated_ha: 2.0,
            growth_stage_code: None,
        }]
    };

    // A code the imported catalogue doesn't know is rejected…
    let mut bogus = sample_treatment(&fx, None, Some(14));
    bogus.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "999999".into(),
    }];
    let err = repo::insert_treatment_record(&mut conn, bogus, plots(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("unknown_problem_code")
    ));

    // …a known code passes…
    let mut known = sample_treatment(&fx, None, Some(14));
    known.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "254".into(),
    }];
    repo::insert_treatment_record(&mut conn, known, plots(&plot), None).unwrap();

    // …and so does a RETIRED code: providers baja-date codes rather than
    // delete them, and a late-entered record may reference one legitimately.
    let mut retired = sample_treatment(&fx, None, Some(14));
    retired.application_date = "2026-05-02".into();
    retired.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "9".into(),
    }];
    repo::insert_treatment_record(&mut conn, retired, plots(&plot), None).unwrap();

    // A category whose catalogue is NOT imported cannot be checked — the code
    // is stored as given (the export's schema-validated tests are the second
    // net). In the running app every catalogue is imported at startup.
    let mut unchecked = sample_treatment(&fx, None, Some(14));
    unchecked.application_date = "2026-05-03".into();
    unchecked.problems = vec![NewTreatmentProblem {
        reason_category_code: "weed".into(),
        problem_code: "12345".into(),
    }];
    repo::insert_treatment_record(&mut conn, unchecked, plots(&plot), None).unwrap();
}

#[test]
fn set_treatment_efficacy_updates_and_logs() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "P");

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
    assert!(record.efficacy_code.is_none());

    // Efficacy is observed after application — the one allowed edit.
    let updated =
        repo::set_treatment_efficacy(&mut conn, &record.id, Some("fair".into()), None).unwrap();
    assert_eq!(updated.efficacy_code.as_deref(), Some("fair"));
    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(fetched.record.efficacy_code.as_deref(), Some("fair"));

    // Logged as an update with complete before/after images.
    let (before, after): (String, String) = conn
        .query_row(
            "SELECT payload, operation FROM record_change
             WHERE entity_table = 'treatment_record' AND entity_id = ?1 AND operation = 'update'
             ORDER BY changed_at DESC LIMIT 1",
            [&record.id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map(|(payload, _)| {
            let doc: serde_json::Value = serde_json::from_str(&payload).unwrap();
            (
                doc["before"]["efficacy_code"].to_string(),
                doc["after"]["efficacy_code"].to_string(),
            )
        })
        .unwrap();
    assert_eq!(before, "null");
    assert_eq!(after, "\"fair\"");

    // Deleted records are not editable.
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();
    assert!(matches!(
        repo::set_treatment_efficacy(&mut conn, &record.id, Some("good".into()), None),
        Err(module_cue::CueError::NotFound)
    ));
}

// --- the actuation interval and the total used (Anexo III Parte I B) -------
//
// Compliance logic, written test-first. RD 1311/2012 Anexo III Parte I B lets
// the date of a treatment be an INTERVAL rather than a single day, and B.i
// requires the total quantity of product used. Two consequences are pinned
// here: which end of the interval the plazo de seguridad counts from, and that
// the total is captured rather than derived.

/// The plazo de seguridad is the time that must pass between the LAST
/// treatment and harvest (RD 1311/2012, art. 3 "plazo de seguridad"; the
/// register's B.i–B.k block records the interval precisely so this can be
/// computed). So an actuation running 1–3 May with a 21-day PHI clears on
/// 24 May, not 22 May.
#[test]
fn phi_end_date_counts_from_the_last_day_of_an_interval() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Interval");

    let mut new = sample_treatment(&fx, None, Some(21));
    new.application_end_date = Some("2026-05-03".into());
    let record = repo::insert_treatment_record(
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

    assert_eq!(record.application_date, "2026-05-01");
    assert_eq!(record.application_end_date.as_deref(), Some("2026-05-03"));
    assert_eq!(
        record.phi_end_date.as_deref(),
        Some("2026-05-24"),
        "the plazo runs from the last application, not the first"
    );
}

/// Without an interval nothing moves: the single-day case is the ordinary one
/// and must keep deriving from `application_date`.
#[test]
fn phi_end_date_is_unchanged_for_a_single_day_treatment() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Single day");

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(21)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.application_end_date, None);
    assert_eq!(record.phi_end_date.as_deref(), Some("2026-05-22"));
}

/// An interval of one day is the same statement as no interval at all.
#[test]
fn an_interval_ending_on_its_start_day_behaves_like_a_single_day() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "One day interval");

    let mut new = sample_treatment(&fx, None, Some(21));
    new.application_end_date = Some("2026-05-01".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.phi_end_date.as_deref(), Some("2026-05-22"));
}

/// A leap day inside the interval must not shift the count — the date maths
/// runs on `jiff`, and the interval end is just another calendar date.
#[test]
fn phi_from_an_interval_crossing_a_leap_day_is_calendar_correct() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Leap");

    let mut new = sample_treatment(&fx, None, Some(7));
    new.application_date = "2024-02-27".into();
    new.application_end_date = Some("2024-02-29".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.phi_end_date.as_deref(), Some("2024-03-07"));
}

/// An interval that ends before it starts is not a correction to guess at.
#[test]
fn an_end_date_before_the_start_is_rejected() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Backwards");

    let mut new = sample_treatment(&fx, None, Some(21));
    new.application_end_date = Some("2026-04-30".into());
    let err = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        module_cue::CueError::Invalid("end_date_before_start")
    ));
}

/// The PHI window a plot is in starts at the FIRST application (re-entry and
/// harvest restrictions apply from the moment product was put on the ground)
/// and ends at the derived `phi_end_date`. An interval therefore only ever
/// widens the window; it never moves its start.
#[test]
fn an_interval_widens_the_phi_window_without_moving_its_start() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "Window");

    let mut new = sample_treatment(&fx, None, Some(10));
    new.application_end_date = Some("2026-05-05".into());
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

    // In the window on the first day, still in it on the day the single-date
    // reading would have cleared, clear on the interval-derived end date.
    let on = |today: &str| {
        let rows =
            repo::phi_status_for_farm(&conn, &fx.farm_id, today, repo::default_phi_horizon_days())
                .unwrap();
        status_of(&rows, &plot).phi_until.clone()
    };
    assert_eq!(on("2026-05-01"), Some("2026-05-15".into()));
    assert_eq!(on("2026-05-11"), Some("2026-05-15".into()));
    assert_eq!(on("2026-05-15"), None, "clear on the end date itself");
}

/// B.i's total is stored as value + unit, both or neither. Half a quantity is
/// not a measurement.
#[test]
fn a_total_quantity_needs_both_its_value_and_its_unit() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Halves");

    let attempt = |conn: &mut Connection, value, unit: Option<&str>| {
        let mut new = sample_treatment(&fx, None, Some(21));
        new.total_quantity_value = value;
        new.total_quantity_unit_code = unit.map(str::to_string);
        repo::insert_treatment_record(
            conn,
            new,
            vec![NewTreatmentPlot {
                plot_id: plot.clone(),
                crop_id: None,
                surface_treated_ha: 1.0,
                growth_stage_code: None,
            }],
            None,
        )
    };

    assert!(matches!(
        attempt(&mut conn, Some(12.0), None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_total_quantity")
    ));
    assert!(matches!(
        attempt(&mut conn, None, Some("l")).unwrap_err(),
        module_cue::CueError::Invalid("invalid_total_quantity")
    ));
    // Zero litres of product is not a treatment.
    assert!(matches!(
        attempt(&mut conn, Some(0.0), Some("l")).unwrap_err(),
        module_cue::CueError::Invalid("invalid_total_quantity")
    ));
    assert!(matches!(
        attempt(&mut conn, Some(-1.0), Some("l")).unwrap_err(),
        module_cue::CueError::Invalid("invalid_total_quantity")
    ));
}

/// The unit must measure an amount. A dose RATE in the total column would read
/// as "12 l/ha of product used", which is a different (and false) statement.
#[test]
fn a_total_quantity_must_use_a_quantity_unit() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Rate as total");

    let mut new = sample_treatment(&fx, None, Some(21));
    new.total_quantity_value = Some(12.0);
    new.total_quantity_unit_code = Some("l_ha".into());
    let err = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        module_cue::CueError::Invalid("invalid_total_quantity")
    ));
}

/// The happy path, and the reason the column exists at all: a concentration
/// dose (g/l) says nothing about how much product left the shed, so the total
/// is captured verbatim and travels into the full audit image.
#[test]
fn a_total_quantity_is_stored_and_logged_beside_a_concentration_dose() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_plot(&mut conn, &fx.farm_id, "Concentration");

    let mut new = sample_treatment(&fx, None, Some(21));
    new.dose_value = Some(150.0);
    new.dose_unit_code = Some("g_l".into());
    new.total_quantity_value = Some(4.5);
    new.total_quantity_unit_code = Some("kg".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.total_quantity_value, Some(4.5));
    assert_eq!(record.total_quantity_unit_code.as_deref(), Some("kg"));

    let (_, _, after) = last_change(&conn, "treatment_record", &record.id);
    assert_eq!(after["total_quantity_value"], 4.5);
    assert_eq!(after["total_quantity_unit_code"], "kg");
    assert_eq!(
        after["application_end_date"],
        serde_json::Value::Null,
        "a full row image carries the absent interval too"
    );
}

// ---------------------------------------------------------------------------
// 3.1 bis: the advisor (Anexo III Parte I B.d) and the non-chemical measure
// ---------------------------------------------------------------------------

/// A plot on the fixture's farm, for tests that need one.
fn a_plot(conn: &mut Connection, fx: &Fixture) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

/// An actuation has to BE something. Anexo III Parte I B registers what was
/// done; a record naming neither a product nor a measure records nothing.
#[test]
fn an_actuation_with_neither_product_nor_measure_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.product_id = None;
    new.dose_value = None;
    new.dose_unit_code = None;
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("treatment_without_actuation")
    ));
}

/// A dose belongs to a product. Half a chemical block is a form the farmer
/// left mid-way, and silently dropping the stated half would lose data they
/// did enter.
#[test]
fn a_dose_without_a_product_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.product_id = None;
    new.measure_code = Some("15".into());
    // dose_value / dose_unit_code left as the sample's 1.0 l/ha
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("dose_without_product")
    ));
}

/// The other half of the same rule.
#[test]
fn a_product_without_a_dose_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.dose_value = None;
    new.dose_unit_code = None;
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("product_without_dose")
    ));
}

/// A purely non-chemical actuation is a complete record: the model's 3.1 bis
/// asks for the measure and its intensity, and RD 1311/2012 art. 10.1 asks for
/// the method to be preferred where possible.
#[test]
fn a_non_chemical_actuation_stores_its_measure_and_intensity() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, None);
    new.product_id = None;
    new.dose_value = None;
    new.dose_unit_code = None;
    new.measure_code = Some("15".into()); // feromonas y atrayentes
    new.measure_intensity_value = Some(4.0);
    new.measure_intensity_unit_code = Some("diffusers_ha".into());
    new.measure_registration_number = Some("MDF-118".into());

    let record = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap();
    assert_eq!(record.measure_code.as_deref(), Some("15"));
    assert_eq!(record.measure_intensity_value, Some(4.0));
    assert_eq!(
        record.measure_intensity_unit_code.as_deref(),
        Some("diffusers_ha")
    );
    assert_eq!(
        record.measure_registration_number.as_deref(),
        Some("MDF-118")
    );
    // No product means no plazo, and `phi_days_used: None` did not fall back
    // to a product default that is not there to consult.
    assert_eq!(record.phi_days_used, None);
    assert_eq!(record.phi_end_date, None);
}

/// `TIPO_MEDIDA_FITOSANITARIA` is a closed list of fourteen entries the
/// authority publishes in full, so an unresolvable code is a mistake — the
/// `MAT_FERTI` side of the two-tier rule.
#[test]
fn an_unknown_measure_code_is_refused_when_the_catalogue_is_imported() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.measure_code = Some("9999".into());
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("unknown_measure_code")
    ));
}

/// An intensity is counted in traps or diffusers, never in litres per hectare:
/// "4 l/ha of pheromone diffusers" is not a slip, it is a different claim.
#[test]
fn an_intensity_must_be_counted_in_an_intensity_unit() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.measure_code = Some("15".into());
    new.measure_intensity_value = Some(4.0);
    new.measure_intensity_unit_code = Some("l_ha".into());
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("invalid_intensity")
    ));
}

/// Value and unit travel together, like every other amount in the book.
#[test]
fn a_half_stated_intensity_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(21));
    new.measure_code = Some("15".into());
    new.measure_intensity_value = Some(4.0);
    new.measure_intensity_unit_code = None;
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("invalid_intensity")
    ));
}

/// The advisor of Anexo III Parte I B.d is snapshotted like the applicator, so
/// correcting the advisor's registry entry never rewrites what a past record
/// printed. The ROPO number is what model 3.1 bis's validation boxes carry.
#[test]
fn the_advisor_is_frozen_onto_the_record() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: Some("12345678Z".into()),
            registration_number: Some("ROPO-8891".into()),
        },
        None,
    )
    .unwrap();

    let mut new = sample_treatment(&fx, None, Some(21));
    new.advisor_id = Some(advisor.id.clone());
    let record = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap();
    assert_eq!(record.advisor_id.as_deref(), Some(advisor.id.as_str()));
    assert_eq!(record.advisor_name_snapshot.as_deref(), Some("Ana Ruiz"));
    assert_eq!(
        record.advisor_registration_snapshot.as_deref(),
        Some("ROPO-8891")
    );

    // Correcting the advisor afterwards must not touch the printed record.
    terrazgo_core::repository::update_advisor(
        &mut conn,
        &advisor.id,
        terrazgo_core::models::UpdateAdvisor {
            name: "Ana Ruiz Gómez".into(),
            tax_id: Some("12345678Z".into()),
            registration_number: Some("ROPO-9999".into()),
        },
        None,
    )
    .unwrap();
    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(
        fetched.record.advisor_registration_snapshot.as_deref(),
        Some("ROPO-8891"),
        "the record keeps the number it was written with"
    );
}

/// A deleted advisor cannot be attached to a new record: the snapshot would be
/// unresolvable and the link would dangle.
#[test]
fn a_deleted_advisor_cannot_be_named_on_a_new_record() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: None,
            registration_number: None,
        },
        None,
    )
    .unwrap();
    terrazgo_core::repository::soft_delete_advisor(&mut conn, &advisor.id, None).unwrap();

    let mut new = sample_treatment(&fx, None, Some(21));
    new.advisor_id = Some(advisor.id);
    let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
    assert!(matches!(err, module_cue::CueError::NotFound));
}

/// The audit log carries complete row images (the `record_change` contract), so
/// the new columns must be in the payload a receiving device would rebuild from.
#[test]
fn the_audit_image_carries_the_advisor_and_the_measure() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, None);
    new.product_id = None;
    new.dose_value = None;
    new.dose_unit_code = None;
    new.measure_code = Some("12".into()); // captura masiva con trampas luminosas
    new.measure_intensity_value = Some(6.0);
    new.measure_intensity_unit_code = Some("traps".into());
    let record = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap();

    let payload: String = conn
        .query_row(
            "SELECT payload FROM record_change
             WHERE entity_table = 'treatment_record' AND entity_id = ?1",
            [&record.id],
            |r| r.get(0),
        )
        .unwrap();
    let image = &serde_json::from_str::<serde_json::Value>(&payload).unwrap()["after"];
    assert_eq!(image["measure_code"], "12");
    assert_eq!(image["measure_intensity_value"], 6.0);
    assert_eq!(image["measure_intensity_unit_code"], "traps");
    // Blank stays blank in the image too — a receiving device must be able to
    // tell "no product" from "product unknown".
    assert!(image["product_id"].is_null());
    assert!(image["phi_end_date"].is_null());
}

// --- Reglamento (UE) 2023/564's two conditional annex fields ---------------
// The annex asks for the application's start hour (surface treatments, "where
// relevant") and the crop's BBCH growth stage (per treated crop). Neither is in
// RD 1311/2012 Anexo III Parte I B, so the duty comes from the EU regulation
// alone — which does not make it optional.

#[test]
fn the_annex_fields_round_trip_and_are_optional() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(14));
    new.application_time = Some("20:30".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
            // EST_FENOLOGICO 6 = BBCH 5, espigamiento.
            growth_stage_code: Some("6".into()),
        }],
        None,
    )
    .unwrap();

    let stored = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(stored.record.application_time.as_deref(), Some("20:30"));
    assert_eq!(stored.plots[0].growth_stage_code.as_deref(), Some("6"));

    // And both absent is the ordinary case, not a rejected one: the annex asks
    // for them only where the product's use is restricted.
    let plain = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        one_plot(&plot),
        None,
    )
    .unwrap();
    assert_eq!(plain.application_time, None);
}

/// An hour is either well formed or unreadable — unlike efficacy or a total
/// quantity, which are observations a farmer may legitimately not have yet.
#[test]
fn a_malformed_application_hour_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    // "+7:30" is the one a bare `parse` would have let through.
    for bad in [
        "7pm", "25:00", "20:61", "8:30", "20:30:00", "2030", "", "+7:30", "2:5",
    ] {
        let mut new = sample_treatment(&fx, None, Some(14));
        new.application_time = Some(bad.into());
        let err = repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).unwrap_err();
        assert!(
            matches!(err, module_cue::CueError::Invalid("application_time")),
            "{bad:?} should not be storable as an hour, got {err:?}"
        );
    }

    // Midnight and the last minute of the day are both real hours.
    for good in ["00:00", "23:59"] {
        let mut new = sample_treatment(&fx, None, Some(14));
        new.application_time = Some(good.into());
        assert!(
            repo::insert_treatment_record(&mut conn, new, one_plot(&plot), None).is_ok(),
            "{good:?} is a valid hour"
        );
    }
}

/// The BBCH monograph's ten principal stages are a closed list, so an
/// unresolvable code is a mistake and not a newer catalogue — the `MAT_FERTI`
/// side of the two-tier rule, unlike `analysis_substance`.
#[test]
fn an_unknown_growth_stage_is_refused_when_the_catalogue_is_imported() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let err = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            // 11 would be BBCH 10, which the monograph does not have.
            growth_stage_code: Some("11".into()),
        }],
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("growth_stage_unknown")
    ));
}

/// Reference data must never be what stands between a farmer and a lawful
/// record: a catalogue that was never imported has no opinion.
#[test]
fn a_growth_stage_is_accepted_when_no_catalogue_is_imported() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: Some("6".into()),
        }],
        None,
    )
    .unwrap();
    assert_eq!(
        repo::get_treatment_record(&conn, &record.id).unwrap().plots[0]
            .growth_stage_code
            .as_deref(),
        Some("6")
    );
}

/// `reconcile_plots` skips a survivor whose fields all match, so every
/// correctable field has to be in that comparison. A field left out of it does
/// not fail — it silently discards the correction and reports success.
#[test]
fn correcting_only_the_growth_stage_changes_the_stored_row() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);
    assert_eq!(record.application_time, None);
    let row_id_before = repo::get_treatment_record(&conn, &record.id).unwrap().plots[0]
        .id
        .clone();

    // Nothing else moves: same plot, same surface, same crop.
    let mut update = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: Some("4".into()),
        }],
    );
    update.application_time = Some("07:15".into());
    let fixed = repo::update_treatment_record(&mut conn, &record.id, update, None).unwrap();

    assert_eq!(fixed.record.application_time.as_deref(), Some("07:15"));
    assert_eq!(fixed.plots[0].growth_stage_code.as_deref(), Some("4"));
    // Re-read, because the returned value could be right while the row is not.
    let stored = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(stored.record.application_time.as_deref(), Some("07:15"));
    assert_eq!(stored.plots[0].growth_stage_code.as_deref(), Some("4"));
    // Corrected in place rather than replaced, so the junction row's audit
    // history stays one thread (the reconcile rule: survivors keep their id).
    assert_eq!(stored.plots[0].id, row_id_before);
}

/// A correction may also withdraw either field: the farmer who stated an hour
/// that turned out to be irrelevant must be able to take it back.
#[test]
fn a_correction_can_withdraw_the_annex_fields() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    let (record, plot) = correctable_record(&mut conn, &fx);

    let mut set = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: Some("6".into()),
        }],
    );
    set.application_time = Some("20:30".into());
    repo::update_treatment_record(&mut conn, &record.id, set, None).unwrap();

    let cleared = correction_of(
        &record,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    );
    let fixed = repo::update_treatment_record(&mut conn, &record.id, cleared, None).unwrap();
    assert_eq!(fixed.record.application_time, None);
    assert_eq!(fixed.plots[0].growth_stage_code, None);
}

/// The log is the Stage-2/3 sync delta source, so `after` must be a complete
/// row image — a receiving device rebuilds the row from it alone.
#[test]
fn the_annex_fields_reach_the_audit_log() {
    let mut conn = db_with_catalogues();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = a_plot(&mut conn, &fx);

    let mut new = sample_treatment(&fx, None, Some(14));
    new.application_time = Some("20:30".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: Some("6".into()),
        }],
        None,
    )
    .unwrap();

    let record_payload: String = conn
        .query_row(
            "SELECT payload FROM record_change
             WHERE entity_table = 'treatment_record' AND entity_id = ?1",
            [&record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        record_payload.contains("\"application_time\":\"20:30\""),
        "the after-image must carry the hour: {record_payload}"
    );

    let plot_payload: String = conn
        .query_row(
            "SELECT payload FROM record_change WHERE entity_table = 'treatment_plot'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        plot_payload.contains("\"growth_stage_code\":\"6\""),
        "the after-image must carry the growth stage: {plot_payload}"
    );
}

#[test]
fn find_product_authorisation_matches_on_the_number_a_record_froze() {
    // The number is what a record legally cites, so the SIEX export resolves the
    // authorisation's kind by it rather than by an id — and a number that no
    // longer matches any row is a `None` the caller resolves, not an error.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    let found = repo::find_product_authorisation(&conn, &fx.product_id, "es", "ES-25.123")
        .unwrap()
        .unwrap();
    assert_eq!(found.authorisation_number, "ES-25.123");
    // Defaulted at insert, and what every row predating the column meant.
    assert_eq!(found.kind_code, "registered");

    // Wrong country, and a number the registry replaced: both simply no match.
    assert!(
        repo::find_product_authorisation(&conn, &fx.product_id, "fr", "ES-25.123")
            .unwrap()
            .is_none()
    );
    assert!(
        repo::find_product_authorisation(&conn, &fx.product_id, "es", "ES-99.999")
            .unwrap()
            .is_none()
    );
}
