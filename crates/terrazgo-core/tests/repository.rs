// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository tests for the core entities (docs/architecture.md testing
//! strategy #2): every public function against an in-memory database, with the
//! audit-log contract (complete row images) checked explicitly — the log is the
//! future sync delta source.
//!
//! This file holds the LAND: farm, plot and the country rule every record
//! derives from. The rest of the registry is split by entity into the sibling
//! `repository_*.rs` files, matching `src/repository/`'s one-file-per-entity
//! layout.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use terrazgo_core::CoreError;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// Farm
// ---------------------------------------------------------------------------

#[test]
fn insert_farm_with_extension_writes_both_rows_and_logs_both() {
    let mut conn = db();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: Some("Carlos".into()),
            owner_tax_id: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: Some("ES470000001".into()),
                rea_code: None,
                siex_code: None,
                province_code: Some("47".into()),
            }),
        },
        None,
    )
    .unwrap();

    let detail = repo::get_farm(&conn, &farm.id).unwrap();
    assert_eq!(detail.farm.name, "Finca");
    assert_eq!(
        detail.es.as_ref().unwrap().rega_code.as_deref(),
        Some("ES470000001")
    );

    let (op, before, after) = last_change(&conn, "farm", &farm.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: every column present, not a subset.
    assert_eq!(after["country_code"], "es");
    assert!(after.get("created_at").is_some());

    let (op, _, after) = last_change(&conn, "farm_es_extension", &farm.id);
    assert_eq!(op, "insert");
    assert_eq!(after["province_code"], "47");
}

/// The export-facing farm identifiers (docs/siex-export.md → gap 4): the
/// holder's tax id lives on the core row, the REA registration code on the
/// Spanish extension. Both must round-trip and appear in the audit images.
#[test]
fn farm_identifiers_roundtrip_and_are_audited() {
    let mut conn = db();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: Some("Carlos".into()),
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("REA-47-00123".into()),
                siex_code: None,
                province_code: Some("47".into()),
            }),
        },
        None,
    )
    .unwrap();
    assert_eq!(farm.owner_tax_id.as_deref(), Some("12345678Z"));

    let detail = repo::get_farm(&conn, &farm.id).unwrap();
    assert_eq!(detail.farm.owner_tax_id.as_deref(), Some("12345678Z"));
    assert_eq!(
        detail.es.as_ref().unwrap().rea_code.as_deref(),
        Some("REA-47-00123")
    );

    let (_, _, after) = last_change(&conn, "farm", &farm.id);
    assert_eq!(after["owner_tax_id"], "12345678Z");
    let (_, _, after) = last_change(&conn, "farm_es_extension", &farm.id);
    assert_eq!(after["rea_code"], "REA-47-00123");

    // Full-row update replaces both, like every other farm field.
    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca".into(),
            owner_name: Some("Carlos".into()),
            owner_tax_id: Some("87654321X".into()),
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
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("REA-47-99999".into()),
                siex_code: None,
                province_code: Some("47".into()),
            }),
            representative: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(detail.farm.owner_tax_id.as_deref(), Some("87654321X"));
    assert_eq!(
        detail.es.as_ref().unwrap().rea_code.as_deref(),
        Some("REA-47-99999")
    );
}

#[test]
fn list_farms_excludes_soft_deleted() {
    let mut conn = db();
    let keep = repo::insert_farm(&mut conn, new_farm("Keep"), None).unwrap();
    let gone = repo::insert_farm(&mut conn, new_farm("Gone"), None).unwrap();

    repo::soft_delete_farm(&mut conn, &gone.id, None).unwrap();

    let farms = repo::list_farms(&conn).unwrap();
    assert_eq!(farms.len(), 1);
    assert_eq!(farms[0].id, keep.id);

    // The deleted farm is hidden from get_farm too…
    assert!(matches!(
        repo::get_farm(&conn, &gone.id),
        Err(CoreError::NotFound)
    ));
    // …but the row itself survives (treatment history must keep resolving).
    let raw: i64 = conn
        .query_row("SELECT COUNT(*) FROM farm WHERE id = ?1", [&gone.id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(raw, 1);

    let (op, before, after) = last_change(&conn, "farm", &gone.id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(
        !after["deleted_at"].is_null(),
        "soft delete keeps a complete after-image"
    );
}

#[test]
fn update_farm_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Old name"), None).unwrap();

    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "New name".into(),
            owner_name: Some("Owner".into()),
            owner_tax_id: None,
            location_text: Some("Valladolid".into()),
            address: None,
            postal_code: None,
            phone_fixed: None,
            phone_mobile: None,
            email: None,
            opened_on: None,
            latitude: Some(41.65),
            longitude: Some(-4.72),
            country_code: "es".into(),
            es: None,
            representative: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(detail.farm.name, "New name");
    assert_eq!(detail.farm.location_text.as_deref(), Some("Valladolid"));
    assert!(detail.farm.updated_at >= farm.updated_at);

    let (op, before, after) = last_change(&conn, "farm", &farm.id);
    assert_eq!(op, "update");
    assert_eq!(before["name"], "Old name");
    assert_eq!(after["name"], "New name");
    // Untouched columns still appear in both images (complete-row contract).
    assert_eq!(before["country_code"], "es");
    assert_eq!(after["country_code"], "es");
}

#[test]
fn update_farm_extension_transitions_are_logged() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let base = UpdateFarm {
        name: "Finca".into(),
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
    };

    // none -> some: extension inserted.
    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
                siex_code: None,
                province_code: Some("47".into()),
            }),
            ..base
        },
        None,
    )
    .unwrap();
    assert!(detail.es.is_some());
    let (op, _, _) = last_change(&conn, "farm_es_extension", &farm.id);
    assert_eq!(op, "insert");

    // some -> some: extension updated.
    repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca".into(),
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
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
                siex_code: None,
                province_code: Some("09".into()),
            }),
            representative: None,
        },
        None,
    )
    .unwrap();
    let (op, before, after) = last_change(&conn, "farm_es_extension", &farm.id);
    assert_eq!(op, "update");
    assert_eq!(before["province_code"], "47");
    assert_eq!(after["province_code"], "09");

    // some -> none: extension hard-deleted, null after-image.
    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca".into(),
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
        None,
    )
    .unwrap();
    assert!(detail.es.is_none());
    let (op, before, after) = last_change(&conn, "farm_es_extension", &farm.id);
    assert_eq!(op, "delete");
    assert_eq!(before["province_code"], "09");
    assert!(
        after.is_null(),
        "hard delete of an extension row has a null after-image"
    );
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM farm_es_extension WHERE farm_id = ?1",
            [&farm.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn farm_validation_rejects_blank_name() {
    let mut conn = db();
    assert!(matches!(
        repo::insert_farm(&mut conn, new_farm("   "), None),
        Err(CoreError::Invalid(_))
    ));
}

#[test]
fn soft_delete_farm_twice_is_not_found() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    repo::soft_delete_farm(&mut conn, &farm.id, None).unwrap();
    assert!(matches!(
        repo::soft_delete_farm(&mut conn, &farm.id, None),
        Err(CoreError::NotFound)
    ));
}

// ---------------------------------------------------------------------------
// Plot
// ---------------------------------------------------------------------------

#[test]
fn insert_plot_with_sigpac_extension_round_trips() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "La Vega".into(),
            area_ha: Some(3.2),
            es: Some(PlotEsFields {
                sigpac_province: Some("47".into()),
                sigpac_municipality: Some("122".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("5".into()),
                sigpac_parcel: Some("23".into()),
                sigpac_enclosure: Some("1".into()),
            }),
        },
        None,
    )
    .unwrap();

    let plots = repo::list_plots(&conn, &farm.id).unwrap();
    assert_eq!(plots.len(), 1);
    assert_eq!(plots[0].plot.id, plot.id);
    let es = plots[0].es.as_ref().unwrap();
    assert_eq!(es.sigpac_polygon.as_deref(), Some("5"));

    let (op, _, after) = last_change(&conn, "plot_es_extension", &plot.id);
    assert_eq!(op, "insert");
    assert_eq!(after["sigpac_parcel"], "23");
}

#[test]
fn list_plots_is_per_farm_and_excludes_soft_deleted() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("B"), None).unwrap();
    let keep = repo::insert_plot(&mut conn, new_plot(&farm_a.id, "Keep"), None).unwrap();
    let gone = repo::insert_plot(&mut conn, new_plot(&farm_a.id, "Gone"), None).unwrap();
    repo::insert_plot(&mut conn, new_plot(&farm_b.id, "Other farm"), None).unwrap();

    repo::soft_delete_plot(&mut conn, &gone.id, None).unwrap();

    let plots = repo::list_plots(&conn, &farm_a.id).unwrap();
    assert_eq!(plots.len(), 1);
    assert_eq!(plots[0].plot.id, keep.id);

    let (op, _, after) = last_change(&conn, "plot", &gone.id);
    assert_eq!(op, "delete");
    assert!(!after["deleted_at"].is_null());
}

#[test]
fn update_plot_changes_fields_and_reconciles_extension() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Old"), None).unwrap();

    let detail = repo::update_plot(
        &mut conn,
        &plot.id,
        UpdatePlot {
            name: "New".into(),
            area_ha: Some(4.5),
            es: Some(PlotEsFields {
                sigpac_province: Some("47".into()),
                sigpac_municipality: None,
                sigpac_aggregate: None,
                sigpac_zone: None,
                sigpac_polygon: None,
                sigpac_parcel: None,
                sigpac_enclosure: None,
            }),
        },
        None,
    )
    .unwrap();
    assert_eq!(detail.plot.name, "New");
    assert_eq!(detail.plot.area_ha, Some(4.5));
    assert!(detail.es.is_some());
    // farm_id is immutable: still the original farm.
    assert_eq!(detail.plot.farm_id, farm.id);

    let (op, before, after) = last_change(&conn, "plot", &plot.id);
    assert_eq!(op, "update");
    assert_eq!(before["name"], "Old");
    assert_eq!(after["name"], "New");
}

#[test]
fn plot_validation_rejects_non_positive_area() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let bad = NewPlot {
        farm_id: farm.id.clone(),
        name: "P".into(),
        area_ha: Some(0.0),
        es: None,
    };
    assert!(matches!(
        repo::insert_plot(&mut conn, bad, None),
        Err(CoreError::Invalid(_))
    ));
}

// ---------------------------------------------------------------------------
// Countries
// ---------------------------------------------------------------------------

#[test]
fn list_countries_returns_seeded_reference_data() {
    let conn = db();
    let countries = repo::list_countries(&conn).unwrap();
    assert!(countries.len() >= 3);
    let es = countries.iter().find(|c| c.code == "es").unwrap();
    assert_eq!(es.i18n_key, "country.es");
}
