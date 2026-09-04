// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `user_profile` and the actor stamp it exists for.
//!
//! Identification, never security: no credentials are stored, the active
//! profile is a device-local id in `settings.json`, and the profile id is what
//! `record_change.actor` carries — verbatim and unvalidated, because a foreign
//! claim must survive sync.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// User profile
// ---------------------------------------------------------------------------

fn plain_profile(name: &str) -> NewUserProfile {
    NewUserProfile {
        display_name: name.into(),
        operator_id: None,
    }
}

#[test]
fn insert_user_profile_round_trips_and_logs_full_image() {
    let mut conn = db();
    let operator = repo::insert_operator(&mut conn, plain_operator("Ana López"), None).unwrap();
    let profile = repo::insert_user_profile(
        &mut conn,
        NewUserProfile {
            display_name: "Ana".into(),
            operator_id: Some(operator.id.clone()),
        },
        None,
    )
    .unwrap();

    assert_eq!(profile.id.len(), 36, "UUIDv7 TEXT id");
    assert_eq!(profile.operator_id.as_deref(), Some(operator.id.as_str()));

    let (op, before, after) = last_change(&conn, "user_profile", &profile.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    for column in [
        "id",
        "display_name",
        "operator_id",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["operator_id"], operator.id.as_str());
}

#[test]
fn user_profile_validation_rejects_blank_name_and_bad_operator_link() {
    let mut conn = db();
    assert!(matches!(
        repo::insert_user_profile(&mut conn, plain_profile("  "), None),
        Err(CoreError::Invalid("empty_name"))
    ));
    // Nonexistent operator id.
    assert!(matches!(
        repo::insert_user_profile(
            &mut conn,
            NewUserProfile {
                display_name: "Ana".into(),
                operator_id: Some("00000000-0000-0000-0000-000000000000".into()),
            },
            None,
        ),
        Err(CoreError::Invalid("operator_not_found"))
    ));
    // A soft-deleted operator satisfies the SQL FK but must still be rejected:
    // the link points at someone the pickers can no longer show.
    let operator = repo::insert_operator(&mut conn, plain_operator("Gone"), None).unwrap();
    repo::soft_delete_operator(&mut conn, &operator.id, None).unwrap();
    assert!(matches!(
        repo::insert_user_profile(
            &mut conn,
            NewUserProfile {
                display_name: "Ana".into(),
                operator_id: Some(operator.id),
            },
            None,
        ),
        Err(CoreError::Invalid("operator_not_found"))
    ));
}

#[test]
fn list_user_profiles_orders_by_name_and_hides_deleted() {
    let mut conn = db();
    let marta = repo::insert_user_profile(&mut conn, plain_profile("Marta"), None).unwrap();
    repo::insert_user_profile(&mut conn, plain_profile("Ana"), None).unwrap();
    repo::insert_user_profile(&mut conn, plain_profile("Carlos"), None).unwrap();
    repo::soft_delete_user_profile(&mut conn, &marta.id, None).unwrap();

    let names: Vec<String> = repo::list_user_profiles(&conn)
        .unwrap()
        .into_iter()
        .map(|p| p.display_name)
        .collect();
    assert_eq!(names, ["Ana", "Carlos"]);
}

#[test]
fn update_user_profile_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let operator = repo::insert_operator(&mut conn, plain_operator("Ana López"), None).unwrap();
    let profile = repo::insert_user_profile(
        &mut conn,
        NewUserProfile {
            display_name: "Ana".into(),
            operator_id: Some(operator.id.clone()),
        },
        None,
    )
    .unwrap();

    // operator_id: None unlinks — the submitted state replaces the stored one.
    let updated = repo::update_user_profile(
        &mut conn,
        &profile.id,
        UpdateUserProfile {
            display_name: "Ana María".into(),
            operator_id: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(updated.display_name, "Ana María");
    assert_eq!(updated.operator_id, None);

    let (op, before, after) = last_change(&conn, "user_profile", &profile.id);
    assert_eq!(op, "update");
    assert_eq!(before["display_name"], "Ana");
    assert_eq!(before["operator_id"], operator.id.as_str());
    assert_eq!(after["display_name"], "Ana María");
    assert!(after["operator_id"].is_null());
}

#[test]
fn update_user_profile_rejects_blank_name_bad_link_and_missing_row() {
    let mut conn = db();
    let profile = repo::insert_user_profile(&mut conn, plain_profile("Ana"), None).unwrap();
    let update = |name: &str, operator_id: Option<String>| UpdateUserProfile {
        display_name: name.into(),
        operator_id,
    };
    assert!(matches!(
        repo::update_user_profile(&mut conn, &profile.id, update("  ", None), None),
        Err(CoreError::Invalid("empty_name"))
    ));
    assert!(matches!(
        repo::update_user_profile(
            &mut conn,
            &profile.id,
            update("Ana", Some("00000000-0000-0000-0000-000000000000".into())),
            None,
        ),
        Err(CoreError::Invalid("operator_not_found"))
    ));
    repo::soft_delete_user_profile(&mut conn, &profile.id, None).unwrap();
    assert!(matches!(
        repo::update_user_profile(&mut conn, &profile.id, update("Ana", None), None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn soft_delete_user_profile_hides_from_list_and_keeps_row() {
    let mut conn = db();
    let keep = repo::insert_user_profile(&mut conn, plain_profile("Keep"), None).unwrap();
    let gone = repo::insert_user_profile(&mut conn, plain_profile("Gone"), None).unwrap();

    repo::soft_delete_user_profile(&mut conn, &gone.id, None).unwrap();

    let listed = repo::list_user_profiles(&conn).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, keep.id);

    // The row survives: author-stamp ids must resolve forever.
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_profile WHERE id = ?1",
            [gone.id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);

    let (op, before, after) = last_change(&conn, "user_profile", &gone.id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(!after["deleted_at"].is_null(), "full after-image logged");

    // Double delete: the row is already hidden.
    assert!(matches!(
        repo::soft_delete_user_profile(&mut conn, &gone.id, None),
        Err(CoreError::NotFound)
    ));
}

// ---------------------------------------------------------------------------
// Actor stamping (record_change.actor)
// ---------------------------------------------------------------------------

/// The actor column of the latest record_change row for an entity.
fn change_actor(conn: &Connection, table: &str, id: &str) -> Option<String> {
    conn.query_row(
        "SELECT actor FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| r.get(0),
    )
    .unwrap()
}

/// Every write stamps the acting profile id verbatim — including the extension
/// entity logged inside the same transaction — and a `None` actor stays NULL
/// (the honest "no active profile" state, also the state of every pre-profile
/// row).
#[test]
fn writes_stamp_the_actor_and_none_stays_null() {
    let mut conn = db();
    let profile = repo::insert_user_profile(&mut conn, plain_profile("Ana"), None).unwrap();
    // Before any active profile exists, writes are unattributed.
    assert_eq!(change_actor(&conn, "user_profile", &profile.id), None);

    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: Some("ES470000001".into()),
                rea_code: None,
                siex_code: None,
                province_code: None,
            }),
        },
        Some(&profile.id),
    )
    .unwrap();
    assert_eq!(
        change_actor(&conn, "farm", &farm.id).as_deref(),
        Some(profile.id.as_str())
    );
    assert_eq!(
        change_actor(&conn, "farm_es_extension", &farm.id).as_deref(),
        Some(profile.id.as_str()),
        "the extension row logged in the same write carries the same author"
    );

    // Update and soft delete stamp whoever acted THEN — each row of the log
    // records its own author, not the row's original creator.
    let other = repo::insert_user_profile(&mut conn, plain_profile("Marta"), None).unwrap();
    repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca 2".into(),
            owner_name: None,
            owner_tax_id: None,
            location_text: None,
            address: None,
            postal_code: None,
            phone_fixed: None,
            phone_mobile: None,
            email: None,
            opened_on: None,
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: None,
            representative: None,
        },
        Some(&other.id),
    )
    .unwrap();
    assert_eq!(
        change_actor(&conn, "farm", &farm.id).as_deref(),
        Some(other.id.as_str())
    );

    repo::soft_delete_farm(&mut conn, &farm.id, None).unwrap();
    assert_eq!(
        change_actor(&conn, "farm", &farm.id),
        None,
        "a write with no active profile stays unattributed even on a row previously edited under one"
    );
}
