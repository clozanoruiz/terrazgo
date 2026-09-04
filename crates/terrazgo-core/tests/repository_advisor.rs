// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Advisors and the farm ↔ advisor link (official model 1.4).
//!
//! RD 1311/2012 Anexo III Parte I B.d names the advisor in the same sentence as
//! the applicator, which is what makes this a register rather than a contact
//! list.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use serde_json::Value;
use terrazgo_core::CoreError;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// Advisors and the farm ↔ advisor link (official model 1.4)
// ---------------------------------------------------------------------------

fn plain_advisor(name: &str) -> NewAdvisor {
    NewAdvisor {
        name: name.into(),
        tax_id: Some("B47123456".into()),
        registration_number: Some("ROPO-AS-4471".into()),
    }
}

#[test]
fn insert_advisor_round_trips_and_logs_full_image() {
    let mut conn = db();
    let advisor = repo::insert_advisor(
        &mut conn,
        plain_advisor("Asesoría Agrícola del Duero S.L."),
        None,
    )
    .unwrap();

    assert_eq!(advisor.name, "Asesoría Agrícola del Duero S.L.");
    assert_eq!(advisor.registration_number.as_deref(), Some("ROPO-AS-4471"));

    let (op, before, after) = last_change(&conn, "advisor", &advisor.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: the log is the future sync delta source.
    for column in [
        "id",
        "name",
        "tax_id",
        "registration_number",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
}

#[test]
fn advisor_validation_rejects_a_blank_name() {
    let mut conn = db();
    let err = repo::insert_advisor(&mut conn, plain_advisor("   "), None).unwrap_err();
    assert!(matches!(err, CoreError::Invalid("empty_name")));
}

#[test]
fn update_advisor_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let advisor =
        repo::insert_advisor(&mut conn, plain_advisor("Asesoría del Duero"), None).unwrap();

    let updated = repo::update_advisor(
        &mut conn,
        &advisor.id,
        UpdateAdvisor {
            name: "Asesoría del Duero S. Coop.".into(),
            tax_id: Some("F47999999".into()),
            registration_number: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.name, "Asesoría del Duero S. Coop.");
    assert!(updated.registration_number.is_none());

    let (op, before, after) = last_change(&conn, "advisor", &advisor.id);
    assert_eq!(op, "update");
    assert_eq!(before["registration_number"], "ROPO-AS-4471");
    assert_eq!(after["registration_number"], Value::Null);
    assert_eq!(after["tax_id"], "F47999999");
}

#[test]
fn set_farm_advisor_links_then_updates_the_same_row() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let advisor = repo::insert_advisor(&mut conn, plain_advisor("Atria Cerealista"), None).unwrap();

    let link = repo::set_farm_advisor(&mut conn, &farm.id, &advisor.id, Some("atria".into()), None)
        .unwrap();
    let (op, _, after) = last_change(&conn, "farm_advisor", &link.id);
    assert_eq!(op, "insert");
    assert_eq!(after["gip_system_code"], "atria");

    // Stating the relationship again updates the framework in place — table
    // 1.4 must never print the same advisor twice.
    let again = repo::set_farm_advisor(
        &mut conn,
        &farm.id,
        &advisor.id,
        Some("advisor_assisted".into()),
        None,
    )
    .unwrap();
    assert_eq!(again.id, link.id);
    let (op, before, after) = last_change(&conn, "farm_advisor", &link.id);
    assert_eq!(op, "update");
    assert_eq!(before["gip_system_code"], "atria");
    assert_eq!(after["gip_system_code"], "advisor_assisted");

    let details = repo::list_farm_advisors(&conn, &farm.id).unwrap();
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].advisor.name, "Atria Cerealista");
    assert_eq!(
        details[0].link.gip_system_code.as_deref(),
        Some("advisor_assisted")
    );
}

#[test]
fn farm_advisor_link_rejects_an_unknown_advisor_and_an_unknown_gip_code() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let advisor = repo::insert_advisor(&mut conn, plain_advisor("Asesoría"), None).unwrap();

    let err =
        repo::set_farm_advisor(&mut conn, &farm.id, "no-such-advisor", None, None).unwrap_err();
    assert!(matches!(err, CoreError::NotFound));

    // The GIP framework is a seeded lookup: a bogus code is a schema error.
    assert!(
        repo::set_farm_advisor(
            &mut conn,
            &farm.id,
            &advisor.id,
            Some("biodynamic".into()),
            None,
        )
        .is_err()
    );
}

#[test]
fn remove_farm_advisor_detaches_without_touching_the_advisor() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let advisor = repo::insert_advisor(&mut conn, plain_advisor("Asesoría"), None).unwrap();
    let link = repo::set_farm_advisor(&mut conn, &farm.id, &advisor.id, Some("atria".into()), None)
        .unwrap();

    repo::remove_farm_advisor(&mut conn, &link.id, None).unwrap();

    assert!(
        repo::list_farm_advisors(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(repo::list_advisors(&conn).unwrap().len(), 1);
    let (op, before, after) = last_change(&conn, "farm_advisor", &link.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], Value::Null);
    assert!(after["deleted_at"].is_string());

    // Re-attaching after a removal starts a fresh link (the partial unique
    // index only constrains ACTIVE rows).
    let again = repo::set_farm_advisor(&mut conn, &farm.id, &advisor.id, None, None).unwrap();
    assert_ne!(again.id, link.id);
    assert_eq!(repo::list_farm_advisors(&conn, &farm.id).unwrap().len(), 1);
}

#[test]
fn soft_delete_advisor_hides_it_and_its_farm_links() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let other = repo::insert_farm(&mut conn, new_farm("Otra finca"), None).unwrap();
    let advisor = repo::insert_advisor(&mut conn, plain_advisor("Asesoría"), None).unwrap();
    let link = repo::set_farm_advisor(&mut conn, &farm.id, &advisor.id, Some("atria".into()), None)
        .unwrap();
    repo::set_farm_advisor(&mut conn, &other.id, &advisor.id, None, None).unwrap();

    repo::soft_delete_advisor(&mut conn, &advisor.id, None).unwrap();

    assert!(repo::list_advisors(&conn).unwrap().is_empty());
    assert!(
        repo::list_farm_advisors(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        repo::list_farm_advisors(&conn, &other.id)
            .unwrap()
            .is_empty()
    );
    // The row survives (a past campaign's table 1.4 must still resolve it)
    // and each detached link is audited on its own.
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM advisor", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    let (op, _, _) = last_change(&conn, "farm_advisor", &link.id);
    assert_eq!(op, "delete");
}

#[test]
fn list_advisors_excludes_deleted_and_is_stable_in_insertion_order() {
    let mut conn = db();
    repo::insert_advisor(&mut conn, plain_advisor("Zamora Asesores"), None).unwrap();
    let first = repo::insert_advisor(&mut conn, plain_advisor("Agroasesoría"), None).unwrap();
    repo::insert_advisor(&mut conn, plain_advisor("Meseta GIP"), None).unwrap();
    repo::soft_delete_advisor(&mut conn, &first.id, None).unwrap();

    // The soft-delete filter is what this pins. The order is insertion order —
    // names are collated by whoever displays them (see the operator test) — so
    // "Zamora Asesores" comes first because it was inserted first.
    let names: Vec<String> = repo::list_advisors(&conn)
        .unwrap()
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert_eq!(names, vec!["Zamora Asesores", "Meseta GIP"]);
}

#[test]
fn list_gip_systems_returns_the_official_frameworks_in_model_order() {
    let conn = db();
    let systems = repo::list_gip_systems(&conn).unwrap();
    let codes: Vec<&str> = systems.iter().map(|s| s.code.as_str()).collect();
    // RD 1311/2012 art. 10-11, in the order the official model's 1.4 footnote
    // lists the siglas: AE, PI, CP, Atrias, AS, NO.
    assert_eq!(
        codes,
        vec![
            "organic",
            "integrated_production",
            "private_certification",
            "atria",
            "advisor_assisted",
            "not_required",
        ]
    );
    assert!(
        systems
            .iter()
            .all(|s| s.i18n_key.starts_with("gip_system."))
    );
}

#[test]
fn list_licence_levels_returns_seeded_reference_data() {
    let conn = db();
    let levels = repo::list_licence_levels(&conn).unwrap();
    let codes: Vec<&str> = levels.iter().map(|l| l.code.as_str()).collect();
    // Seed order (the RD 1311/2012 niveles de capacitación, rising), not
    // alphabetical. "asesor" is deliberately absent: advising is a capacity of
    // the advisor entity, not a carné an applicator holds.
    assert_eq!(codes, vec!["basic", "qualified", "fumigator", "pilot"]);
    assert!(
        levels
            .iter()
            .all(|l| l.i18n_key.starts_with("licence_level."))
    );
}

#[test]
fn list_production_systems_returns_seeded_reference_data() {
    let conn = db();
    let systems = repo::list_production_systems(&conn).unwrap();
    let codes: Vec<&str> = systems.iter().map(|s| s.code.as_str()).collect();
    assert_eq!(codes, vec!["conventional", "integrated", "organic"]);
    assert!(
        systems
            .iter()
            .all(|s| s.i18n_key.starts_with("production_system."))
    );
}
