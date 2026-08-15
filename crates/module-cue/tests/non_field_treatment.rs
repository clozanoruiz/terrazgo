// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model sections 3.3, 3.4 and 3.5 — treatments applied to something other than
//! a growing crop — and the stored "APLICA TRATAMIENTO: NO" that heads each
//! conditional register.
//!
//! The rules pinned here come from two places that disagree about how much the
//! record must carry: the printed model (RD 1311/2012 art. 16.2 — its layout is
//! orientativo, its content binding) shows date, subject, problem, quantity and
//! product, while the SIEX twins `TratamientosPostCosecha` and
//! `TratamientosEdifInstalaciones` additionally require coded problems, coded
//! justifications, a named applicator and an observed efficacy. Capture follows
//! the stricter of the two, so a future export needs no migration.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;

struct Fixture {
    season_id: String,
    farm_id: String,
    operator_id: String,
    product_id: String,
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
    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("ROPO-4700123".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap()
    .id;
    let product_id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: "Fosfuro de aluminio".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: None,
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-18.765".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
        None,
    )
    .unwrap();

    Fixture {
        season_id: season.id,
        farm_id,
        operator_id,
        product_id,
    }
}

/// A postharvest treatment: grain fumigated in store, measured in tonnes.
fn sample(fx: &Fixture, kind: &str) -> NewNonFieldTreatment {
    NewNonFieldTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        country_code: None,
        subject_kind_code: kind.into(),
        treated_on: "2026-08-20".into(),
        subject_description: "Trigo blando de la cosecha 2026".into(),
        subject_product_code: None,
        treated_quantity_value: None,
        treated_quantity_unit_code: None,
        product_id: fx.product_id.clone(),
        product_quantity_value: None,
        product_quantity_unit_code: None,
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        problems: vec![NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        notes: None,
    }
}

fn last_change(
    conn: &Connection,
    table: &str,
    id: &str,
) -> (String, serde_json::Value, serde_json::Value) {
    conn.query_row(
        "SELECT operation, payload FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map(|(op, payload)| {
        let mut doc: serde_json::Value = serde_json::from_str(&payload).unwrap();
        (op, doc["before"].take(), doc["after"].take())
    })
    .unwrap()
}

// ---------------------------------------------------------------------------
// One table, three subjects
// ---------------------------------------------------------------------------

#[test]
fn a_postharvest_treatment_is_stored_with_its_snapshots_and_junctions() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx, "postharvest");
    new.treated_quantity_value = Some(120.0);
    new.treated_quantity_unit_code = Some("t".into());
    new.product_quantity_value = Some(3.0);
    new.product_quantity_unit_code = Some("kg".into());
    let saved = repo::insert_non_field_treatment(&mut conn, new, None).unwrap();

    assert_eq!(saved.record.subject_kind_code, "postharvest");
    // Country derives from the farm, exactly as a field treatment's does.
    assert_eq!(saved.record.country_code, "es");
    // Snapshots freeze what the register prints.
    assert_eq!(saved.record.product_name_snapshot, "Fosfuro de aluminio");
    assert_eq!(
        saved.record.authorisation_number_snapshot.as_deref(),
        Some("ES-18.765")
    );
    assert_eq!(saved.record.operator_name_snapshot, "Carlos Pérez");
    assert_eq!(
        saved.record.operator_licence_snapshot.as_deref(),
        Some("ROPO-4700123")
    );
    assert_eq!(saved.problems.len(), 1);
    assert_eq!(saved.justifications.len(), 1);
    // Efficacy is observed later, so it starts absent.
    assert_eq!(saved.record.efficacy_code, None);
}

/// The three sections are one table, so a farm's book can hold all of them and
/// the register list separates them by kind.
#[test]
fn the_three_registers_share_one_table_and_list_separately() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    for (kind, unit) in [
        ("postharvest", "t"),
        ("storage_premises", "m3"),
        ("transport", "m3"),
    ] {
        let mut new = sample(&fx, kind);
        new.treated_quantity_value = Some(10.0);
        new.treated_quantity_unit_code = Some(unit.into());
        repo::insert_non_field_treatment(&mut conn, new, None).unwrap();
    }

    let all = repo::list_non_field_treatments(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(all.len(), 3);
    let kinds: Vec<&str> = all
        .iter()
        .map(|d| d.record.subject_kind_code.as_str())
        .collect();
    assert!(kinds.contains(&"postharvest"));
    assert!(kinds.contains(&"storage_premises"));
    assert!(kinds.contains(&"transport"));
}

// ---------------------------------------------------------------------------
// What the subject is decides what it is measured in
// ---------------------------------------------------------------------------

/// The model's own footnotes: 3.3 measures the treated produce in tonnes, while
/// 3.4 and 3.5 measure the treated space in cubic metres. Recording a warehouse
/// as "120 t" is not a unit slip, it is a different claim.
#[test]
fn the_treated_quantity_unit_must_match_the_subject() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let attempt = |conn: &mut Connection, kind: &str, unit: &str| {
        let mut new = sample(&fx, kind);
        new.treated_quantity_value = Some(10.0);
        new.treated_quantity_unit_code = Some(unit.into());
        repo::insert_non_field_treatment(conn, new, None)
    };

    assert!(attempt(&mut conn, "postharvest", "t").is_ok());
    assert!(attempt(&mut conn, "storage_premises", "m3").is_ok());
    assert!(attempt(&mut conn, "transport", "m3").is_ok());

    for (kind, unit) in [
        ("postharvest", "m3"),
        ("storage_premises", "t"),
        ("transport", "t"),
        // A rate is not an amount at all.
        ("postharvest", "l_ha"),
    ] {
        assert!(
            matches!(
                attempt(&mut conn, kind, unit).unwrap_err(),
                module_cue::CueError::Invalid("quantity_unit_mismatch")
            ),
            "{kind} must not be measured in {unit}"
        );
    }
}

/// The product used is measured as a product is sold: kilograms or litres.
#[test]
fn the_product_quantity_must_be_kilograms_or_litres() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let attempt = |conn: &mut Connection, unit: Option<&str>, value: Option<f64>| {
        let mut new = sample(&fx, "postharvest");
        new.product_quantity_value = value;
        new.product_quantity_unit_code = unit.map(str::to_string);
        repo::insert_non_field_treatment(conn, new, None)
    };

    assert!(attempt(&mut conn, Some("kg"), Some(3.0)).is_ok());
    assert!(attempt(&mut conn, Some("l"), Some(2.5)).is_ok());
    // Tonnes and cubic metres measure the SUBJECT, never the product.
    for unit in ["t", "m3", "l_ha"] {
        assert!(matches!(
            attempt(&mut conn, Some(unit), Some(3.0)).unwrap_err(),
            module_cue::CueError::Invalid("invalid_product_quantity")
        ));
    }
    // Half a measurement is not one.
    assert!(matches!(
        attempt(&mut conn, None, Some(3.0)).unwrap_err(),
        module_cue::CueError::Invalid("invalid_product_quantity")
    ));
    assert!(matches!(
        attempt(&mut conn, Some("kg"), None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_product_quantity")
    ));
    assert!(matches!(
        attempt(&mut conn, Some("kg"), Some(0.0)).unwrap_err(),
        module_cue::CueError::Invalid("invalid_product_quantity")
    ));
}

/// Both quantities are optional: the printed form leaves the cell to be filled
/// by hand, and demanding a figure at entry is how farmers end up inventing
/// them. A format that requires one says so at export time.
#[test]
fn both_quantities_may_be_left_unstated() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved =
        repo::insert_non_field_treatment(&mut conn, sample(&fx, "transport"), None).unwrap();
    assert_eq!(saved.record.treated_quantity_value, None);
    assert_eq!(saved.record.product_quantity_value, None);
}

// ---------------------------------------------------------------------------
// The same discipline field treatments follow
// ---------------------------------------------------------------------------

#[test]
fn a_non_field_treatment_needs_a_problem_and_a_justification() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut no_problem = sample(&fx, "postharvest");
    no_problem.problems.clear();
    assert!(matches!(
        repo::insert_non_field_treatment(&mut conn, no_problem, None).unwrap_err(),
        module_cue::CueError::Invalid("no_problems")
    ));

    let mut no_justification = sample(&fx, "postharvest");
    no_justification.justifications.clear();
    assert!(matches!(
        repo::insert_non_field_treatment(&mut conn, no_justification, None).unwrap_err(),
        module_cue::CueError::Invalid("no_justifications")
    ));
}

#[test]
fn duplicate_problems_and_justifications_are_folded_not_rejected() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut new = sample(&fx, "postharvest");
    new.problems.push(NewTreatmentProblem {
        reason_category_code: "pest".into(),
        problem_code: "135".into(),
    });
    new.justifications.push("monitoring".into());
    let saved = repo::insert_non_field_treatment(&mut conn, new, None).unwrap();
    assert_eq!(saved.problems.len(), 1);
    assert_eq!(saved.justifications.len(), 1);
}

#[test]
fn the_subject_description_may_not_be_blank() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx, "storage_premises");
    new.subject_description = "   ".into();
    assert!(matches!(
        repo::insert_non_field_treatment(&mut conn, new, None).unwrap_err(),
        module_cue::CueError::Invalid("empty_subject")
    ));
}

/// Editing the product or the operator afterwards must never rewrite a record
/// already in the book — the reason the snapshots exist.
#[test]
fn snapshots_survive_later_edits_to_the_rows_they_came_from() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None)
        .unwrap()
        .record;

    repo::update_product(
        &mut conn,
        &fx.product_id,
        UpdateProduct {
            commercial_name: "Otro nombre".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: None,
        },
        None,
    )
    .unwrap();

    let reread = repo::get_non_field_treatment(&conn, &saved.id).unwrap();
    assert_eq!(reread.record.product_name_snapshot, "Fosfuro de aluminio");
}

/// Efficacy is observed after the treatment, so it is set later — by its own
/// audit-logged setter, the only edit an otherwise immutable record allows.
#[test]
fn efficacy_is_recorded_afterwards_and_logged() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None)
        .unwrap()
        .record;

    let updated =
        repo::set_non_field_efficacy(&mut conn, &saved.id, Some("good".into()), Some("carlos"))
            .unwrap();
    assert_eq!(updated.efficacy_code.as_deref(), Some("good"));

    let (op, before, after) = last_change(&conn, "non_field_treatment", &saved.id);
    assert_eq!(op, "update");
    assert_eq!(before["efficacy_code"], serde_json::Value::Null);
    assert_eq!(after["efficacy_code"], "good");

    // And it can be taken back off while it is still an observation.
    let cleared = repo::set_non_field_efficacy(&mut conn, &saved.id, None, None).unwrap();
    assert_eq!(cleared.efficacy_code, None);
}

#[test]
fn every_row_of_the_record_is_audit_logged_with_a_complete_image() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved =
        repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), Some("carlos"))
            .unwrap();

    let (op, _, after) = last_change(&conn, "non_field_treatment", &saved.record.id);
    assert_eq!(op, "insert");
    // A receiving device must be able to rebuild the row from `after` alone.
    assert_eq!(
        after["subject_description"],
        "Trigo blando de la cosecha 2026"
    );
    assert_eq!(after["subject_kind_code"], "postharvest");
    assert_eq!(after["operator_name_snapshot"], "Carlos Pérez");

    // Junction rows are logged individually, under their own ids.
    let (_, _, problem) = last_change(&conn, "non_field_treatment_problem", &saved.problems[0].id);
    assert_eq!(problem["problem_code"], "135");
    let (_, _, justification) = last_change(
        &conn,
        "non_field_treatment_justification",
        &saved.justifications[0].id,
    );
    assert_eq!(justification["justification_code"], "monitoring");

    let actor: String = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1",
            [&saved.record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "carlos");
}

#[test]
fn soft_delete_hides_the_record_and_logs_both_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_non_field_treatment(&mut conn, sample(&fx, "transport"), None)
        .unwrap()
        .record;

    repo::soft_delete_non_field_treatment(&mut conn, &saved.id, None).unwrap();
    assert!(
        repo::list_non_field_treatments(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    let (op, before, after) = last_change(&conn, "non_field_treatment", &saved.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], serde_json::Value::Null);
    assert!(after["deleted_at"].is_string());
}

// ---------------------------------------------------------------------------
// "APLICA TRATAMIENTO: NO" — the stored negative
// ---------------------------------------------------------------------------

/// The negative is evidence, not an absence: an empty register could mean
/// nothing happened OR that nobody filled it in, and only the first is a
/// statement the farmer made. Same reasoning as `plot_zone_flag`'s stored
/// 'outside' result.
#[test]
fn declaring_a_register_empty_stores_a_row_and_logs_it() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let declared = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "postharvest",
        "2026-09-01",
        Some("carlos"),
    )
    .unwrap();
    assert_eq!(declared.register_code, "postharvest");

    let listed = repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id).unwrap();
    assert_eq!(listed.len(), 1);

    let (op, _, after) = last_change(&conn, "register_declaration", &declared.id);
    assert_eq!(op, "insert");
    assert_eq!(after["register_code"], "postharvest");
}

/// Re-declaring restates the same fact; it must not print the register twice.
#[test]
fn re_declaring_the_same_register_updates_rather_than_duplicates() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let first = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "transport",
        "2026-09-01",
        None,
    )
    .unwrap();
    let second = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "transport",
        "2026-09-15",
        None,
    )
    .unwrap();

    assert_eq!(first.id, second.id, "the same statement, restated");
    assert_eq!(second.declared_on, "2026-09-15");
    assert_eq!(
        repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id)
            .unwrap()
            .len(),
        1
    );
}

/// A declaration is per campaign: last year's "nothing to declare" says nothing
/// about this one.
#[test]
fn declarations_are_scoped_to_their_campaign() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let other = repo::insert_season(
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

    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "postharvest",
        "2026-09-01",
        None,
    )
    .unwrap();

    assert!(
        repo::list_register_declarations(&conn, &fx.farm_id, &other.id)
            .unwrap()
            .is_empty()
    );
}

/// Declaring "nothing happened" while the register holds records would be a
/// false statement in a legal document.
#[test]
fn a_register_holding_records_cannot_be_declared_empty() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None).unwrap();

    assert!(matches!(
        repo::set_register_declaration(
            &mut conn,
            &fx.farm_id,
            &fx.season_id,
            "postharvest",
            "2026-09-01",
            None,
        )
        .unwrap_err(),
        module_cue::CueError::Invalid("register_has_rows")
    ));

    // A different register is unaffected — the guard is per register.
    assert!(
        repo::set_register_declaration(
            &mut conn,
            &fx.farm_id,
            &fx.season_id,
            "transport",
            "2026-09-01",
            None,
        )
        .is_ok()
    );
}

/// The other direction, and the one that could forge evidence: recording a
/// treatment into a register already declared empty must withdraw the
/// declaration in the same transaction. The row is the stronger statement, and
/// leaving a stale NO beside it would print a contradiction.
#[test]
fn recording_into_a_register_declared_empty_withdraws_the_declaration() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let declared = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "postharvest",
        "2026-09-01",
        None,
    )
    .unwrap();

    repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), Some("carlos"))
        .unwrap();

    assert!(
        repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id)
            .unwrap()
            .is_empty(),
        "the NO must not survive a record contradicting it"
    );
    // Withdrawn, not erased: the audit trail says the farmer once declared it.
    let (op, _, after) = last_change(&conn, "register_declaration", &declared.id);
    assert_eq!(op, "delete");
    assert!(after["deleted_at"].is_string());
}

/// Withdrawing by hand is allowed too — a farmer who ticked NO in error.
#[test]
fn a_declaration_can_be_withdrawn_and_then_restated() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "storage_premises",
        "2026-09-01",
        None,
    )
    .unwrap();
    repo::clear_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "storage_premises",
        None,
    )
    .unwrap();
    assert!(
        repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id)
            .unwrap()
            .is_empty()
    );

    // Restating mints a NEW row rather than resurrecting the withdrawn one,
    // so the partial unique index stays satisfied and the history is kept.
    let again = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "storage_premises",
        "2026-10-01",
        None,
    )
    .unwrap();
    assert_eq!(again.declared_on, "2026-10-01");
    assert_eq!(
        repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id)
            .unwrap()
            .len(),
        1
    );
}

// --- corrections (slice D) ---------------------------------------------------

/// The submitted state, built from a stored record so a test can change one
/// thing at a time.
fn correction_of(record: &NonFieldTreatment) -> UpdateNonFieldTreatment {
    UpdateNonFieldTreatment {
        treated_on: record.treated_on.clone(),
        subject_description: record.subject_description.clone(),
        subject_product_code: record.subject_product_code.clone(),
        treated_quantity_value: record.treated_quantity_value,
        treated_quantity_unit_code: record.treated_quantity_unit_code.clone(),
        product_id: record.product_id.clone(),
        product_quantity_value: record.product_quantity_value,
        product_quantity_unit_code: record.product_quantity_unit_code.clone(),
        operator_id: record.operator_id.clone(),
        machinery_id: record.machinery_id.clone(),
        advisor_id: record.advisor_id.clone(),
        problems: vec![NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        }],
        justifications: vec!["monitoring".into()],
        notes: record.notes.clone(),
    }
}

#[test]
fn a_correction_keeps_snapshots_whose_row_it_did_not_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let stored = repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None)
        .unwrap()
        .record;

    // The product's registry entry is corrected afterwards.
    conn.execute(
        "UPDATE product SET commercial_name = 'Fosfuro de aluminio 56%' WHERE id = ?1",
        [&fx.product_id],
    )
    .unwrap();

    let mut update = correction_of(&stored);
    update.treated_on = "2026-08-21".into();
    let fixed = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();

    assert_eq!(fixed.record.treated_on, "2026-08-21");
    assert_eq!(
        fixed.record.product_name_snapshot, "Fosfuro de aluminio",
        "correcting the date must not rewrite what the record printed"
    );
}

#[test]
fn a_correction_keeps_the_quantity_unit_the_subject_kind_demands() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let stored = repo::insert_non_field_treatment(&mut conn, sample(&fx, "storage_premises"), None)
        .unwrap()
        .record;

    // Premises are measured in cubic metres (the model's own footnote);
    // recording one in tonnes is a different claim, not a unit slip.
    let mut update = correction_of(&stored);
    update.treated_quantity_value = Some(400.0);
    update.treated_quantity_unit_code = Some("t".into());
    assert!(matches!(
        repo::update_non_field_treatment(&mut conn, &stored.id, update, None),
        Err(module_cue::CueError::Invalid("quantity_unit_mismatch"))
    ));

    let mut update = correction_of(&stored);
    update.treated_quantity_value = Some(400.0);
    update.treated_quantity_unit_code = Some("m3".into());
    let fixed = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();
    assert_eq!(fixed.record.treated_quantity_value, Some(400.0));
}

#[test]
fn a_correction_can_name_the_advisor_that_was_missing() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let stored = repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None)
        .unwrap()
        .record;
    assert!(stored.advisor_id.is_none());

    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "ATRIA Cerealista".into(),
            tax_id: None,
            registration_number: Some("ROPO-AS-47-0912".into()),
        },
        None,
    )
    .unwrap();

    // Anexo III Parte I B.d asks for the applicator "y, en su caso, del
    // asesor", and B.b/B.f put premises and vehicles inside B's own list.
    let mut update = correction_of(&stored);
    update.advisor_id = Some(advisor.id.clone());
    let fixed = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();
    assert_eq!(
        fixed.record.advisor_name_snapshot.as_deref(),
        Some("ATRIA Cerealista")
    );
    assert_eq!(
        fixed.record.advisor_registration_snapshot.as_deref(),
        Some("ROPO-AS-47-0912")
    );

    // Correcting the advisor's ROPO number in the registry afterwards leaves
    // the record printing what it printed.
    conn.execute(
        "UPDATE advisor SET registration_number = 'ROPO-AS-47-9999' WHERE id = ?1",
        [&advisor.id],
    )
    .unwrap();
    let mut update = correction_of(&fixed.record);
    update.notes = Some("nota".into());
    let again = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();
    assert_eq!(
        again.record.advisor_registration_snapshot.as_deref(),
        Some("ROPO-AS-47-0912")
    );

    // And dropping the advisor clears both halves of the pair.
    let mut update = correction_of(&again.record);
    update.advisor_id = None;
    let cleared = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();
    assert!(cleared.record.advisor_name_snapshot.is_none());
    assert!(cleared.record.advisor_registration_snapshot.is_none());
}

#[test]
fn a_correction_reconciles_the_coded_problems_and_justifications() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let stored = repo::insert_non_field_treatment(&mut conn, sample(&fx, "transport"), None)
        .unwrap()
        .record;

    let mut update = correction_of(&stored);
    update.problems = vec![
        NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        },
    ];
    update.justifications = vec!["advisor_recommendation".into()];
    let fixed = repo::update_non_field_treatment(&mut conn, &stored.id, update, None).unwrap();

    assert_eq!(fixed.problems.len(), 2);
    assert_eq!(fixed.justifications.len(), 1);
    assert_eq!(
        fixed.justifications[0].justification_code,
        "advisor_recommendation"
    );

    // A record still needs a reason and a justification after correction, on
    // the same footing as at insert.
    let mut update = correction_of(&fixed.record);
    update.problems = vec![];
    assert!(matches!(
        repo::update_non_field_treatment(&mut conn, &stored.id, update, None),
        Err(module_cue::CueError::Invalid("no_problems"))
    ));
}

#[test]
fn a_correction_logs_complete_before_and_after_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let stored = repo::insert_non_field_treatment(&mut conn, sample(&fx, "postharvest"), None)
        .unwrap()
        .record;

    let mut update = correction_of(&stored);
    update.subject_description = "Trigo blando, silo 3".into();
    repo::update_non_field_treatment(&mut conn, &stored.id, update, Some("user-1")).unwrap();

    let (operation, before, after) = last_change(&conn, "non_field_treatment", &stored.id);
    assert_eq!(operation, "update");
    assert_eq!(
        before["subject_description"],
        "Trigo blando de la cosecha 2026"
    );
    assert_eq!(after["subject_description"], "Trigo blando, silo 3");
    assert!(after["product_name_snapshot"].is_string());
}
