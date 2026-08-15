// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository tests for the core entities (docs/architecture.md testing strategy #2):
//! every public function against an in-memory database, with the audit-log
//! contract (complete row images) checked explicitly — the log is the future
//! sync delta source.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::CoreError;
use terrazgo_core::models::{
    FarmEsFields, FarmRepresentativeFields, NewAdvisor, NewCrop, NewFarm, NewGeoFeature,
    NewHarvestPlot, NewHarvestRecord, NewMachinery, NewOperator, NewPlot, NewSeason,
    NewUserProfile, NewWaterPoint, NewZoneFlag, PlotEsFields, UpdateAdvisor, UpdateCrop,
    UpdateFarm, UpdateHarvestRecord, UpdateMachinery, UpdateOperator, UpdatePlot, UpdateSeason,
    UpdateUserProfile, UpdateWaterPoint,
};
use terrazgo_core::repository as repo;

fn db() -> Connection {
    terrazgo_core::open_in_memory().unwrap()
}

fn new_farm(name: &str) -> NewFarm {
    NewFarm {
        name: name.into(),
        owner_name: None,
        owner_tax_id: None,
        country_code: "es".into(),
        es: None,
    }
}

fn new_plot(farm_id: &str, name: &str) -> NewPlot {
    NewPlot {
        farm_id: farm_id.into(),
        name: name.into(),
        area_ha: Some(2.0),
        es: None,
    }
}

/// The latest record_change row for an entity: (operation, before, after).
fn last_change(conn: &Connection, table: &str, id: &str) -> (String, Value, Value) {
    conn.query_row(
        "SELECT operation, payload FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| {
            let operation: String = r.get(0)?;
            let payload: String = r.get(1)?;
            Ok((operation, payload))
        },
    )
    .map(|(op, payload)| {
        let mut doc: Value = serde_json::from_str(&payload).unwrap();
        (op, doc["before"].take(), doc["after"].take())
    })
    .unwrap()
}

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

// ---------------------------------------------------------------------------
// Season, crop, operator, machinery. These moved here from module-cue
// (2026-06-12); the CUE suite exercises them through fixtures, but their
// contracts belong to this crate's tests.
// ---------------------------------------------------------------------------

fn new_season(campaign_year: i64, label: &str) -> NewSeason {
    NewSeason {
        campaign_year,
        label: label.into(),
        starts_on: None,
        ends_on: None,
    }
}

/// A plain crop with everything optional left out — the base for `..` updates
/// in the tests that only care about a field or two.
fn base_crop(plot_id: &str, season_id: &str) -> NewCrop {
    NewCrop {
        plot_id: plot_id.into(),
        season_id: season_id.into(),
        species_name: "cebada".into(),
        variety: None,
        production_system_code: None,
        area_ha: None,
        irrigation_code: None,
        growing_environment_code: None,
        gip_system_code: None,
        sown_on: None,
        crop_code: None,
        source: None,
        source_campaign: None,
        declared_area_ha: None,
    }
}

#[test]
fn insert_season_starts_active_and_logs_full_image() {
    let mut conn = db();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: Some("2025-09-01".into()),
            ends_on: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(season.id.len(), 36, "UUIDv7 TEXT id");
    assert_eq!(season.status, "active", "a new season starts active");

    let (op, before, after) = last_change(&conn, "season", &season.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: every column present, absent optionals as null.
    for column in [
        "id",
        "campaign_year",
        "label",
        "starts_on",
        "ends_on",
        "status",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["campaign_year"], 2026);
    assert_eq!(after["ends_on"], Value::Null);
    assert_eq!(after["deleted_at"], Value::Null);
}

#[test]
fn update_season_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let season = repo::insert_season(&mut conn, new_season(2025, "2025 (typo)"), None).unwrap();

    let updated = repo::update_season(
        &mut conn,
        &season.id,
        UpdateSeason {
            campaign_year: 2026,
            label: "2025/2026".into(),
            starts_on: Some("2025-09-01".into()),
            ends_on: Some("2026-08-31".into()),
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.campaign_year, 2026);
    assert_eq!(updated.label, "2025/2026");
    assert_eq!(updated.starts_on.as_deref(), Some("2025-09-01"));
    // Untouched by the update: archiving is a separate lifecycle action.
    assert_eq!(updated.status, "active");

    let (op, before, after) = last_change(&conn, "season", &season.id);
    assert_eq!(op, "update");
    assert_eq!(before["label"], "2025 (typo)");
    assert_eq!(before["campaign_year"], 2025);
    assert_eq!(after["label"], "2025/2026");
    assert_eq!(after["campaign_year"], 2026);
    assert_eq!(after["ends_on"], "2026-08-31");
}

#[test]
fn update_season_rejects_a_blank_label_and_an_unknown_id() {
    let mut conn = db();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();

    let blank = repo::update_season(
        &mut conn,
        &season.id,
        UpdateSeason {
            campaign_year: 2026,
            label: "   ".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    );
    assert!(matches!(blank, Err(CoreError::Invalid("empty_name"))));

    let missing = repo::update_season(
        &mut conn,
        "no-such-season",
        UpdateSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    );
    assert!(matches!(missing, Err(CoreError::NotFound)));
}

#[test]
fn soft_delete_season_hides_an_empty_season_and_logs_both_images() {
    let mut conn = db();
    let keep = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let mistake = repo::insert_season(&mut conn, new_season(2027, "2027 (mistake)"), None).unwrap();

    repo::soft_delete_season(&mut conn, &mistake.id, None).unwrap();

    let ids: Vec<String> = repo::list_seasons(&conn)
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, vec![keep.id], "a deleted season leaves the selector");

    let (op, before, after) = last_change(&conn, "season", &mistake.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], Value::Null);
    assert!(after["deleted_at"].is_string(), "after-image is stamped");
    assert_eq!(after["label"], "2027 (mistake)", "complete after-image");

    // Deleting twice is a not-found, like every other soft delete.
    assert!(matches!(
        repo::soft_delete_season(&mut conn, &mistake.id, None),
        Err(CoreError::NotFound)
    ));
}

/// Only an EMPTY season may be deleted: hiding one that owns records would hide
/// the records with it, since every record-book view is season-scoped. Core
/// guards its own half (crops); the shell chains module-cue for treatments.
#[test]
fn soft_delete_season_is_refused_while_a_crop_references_it() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "cebada".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    assert!(matches!(
        repo::soft_delete_season(&mut conn, &season.id, None),
        Err(CoreError::Invalid("season_in_use"))
    ));
    assert_eq!(repo::list_seasons(&conn).unwrap().len(), 1, "still there");

    // A soft-deleted crop no longer holds the season down.
    repo::soft_delete_crop(&mut conn, &crop.id, None).unwrap();
    repo::soft_delete_season(&mut conn, &season.id, None).unwrap();
    assert!(repo::list_seasons(&conn).unwrap().is_empty());
}

#[test]
fn insert_crop_ties_plot_to_season_and_logs_full_image() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "trigo blando".into(),
            variety: Some("Marcopolo".into()),
            production_system_code: Some("conventional".into()),
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: Some("2025-11-02".into()),
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(crop.plot_id, plot.id);
    assert_eq!(crop.season_id, season.id);

    let (op, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    for column in [
        "id",
        "plot_id",
        "season_id",
        "species_name",
        "variety",
        "production_system_code",
        "sown_on",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["species_name"], "trigo blando");
    assert_eq!(after["deleted_at"], Value::Null);
}

#[test]
fn update_crop_replaces_fields_and_keeps_it_on_its_plot_and_season() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "trigo blanco".into(), // the typo this whole feature exists for
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    let updated = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "trigo blando".into(),
            variety: Some("Marcopolo".into()),
            production_system_code: Some("organic".into()),
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: Some("2025-11-02".into()),
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.species_name, "trigo blando");
    assert_eq!(updated.variety.as_deref(), Some("Marcopolo"));
    // `UpdateCrop` carries neither, so a crop can never be re-homed under its
    // treatment history (the `plot.farm_id` precedent).
    assert_eq!(updated.plot_id, plot.id);
    assert_eq!(updated.season_id, season.id);

    let (op, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(op, "update");
    assert_eq!(before["species_name"], "trigo blanco");
    assert_eq!(before["variety"], Value::Null);
    assert_eq!(after["species_name"], "trigo blando");
    assert_eq!(after["production_system_code"], "organic");

    let blank = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: " ".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    );
    assert!(matches!(blank, Err(CoreError::Invalid("empty_name"))));
}

/// A hand-typed crop is `source = 'user'` without anyone saying so — the manual
/// form has no provenance fields to send.
#[test]
fn insert_crop_defaults_source_to_user() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(&mut conn, base_crop(&plot.id, &season.id), None).unwrap();

    assert_eq!(crop.source, "user");
    assert_eq!(crop.source_campaign, None);
    assert_eq!(crop.declared_area_ha, None);
    assert_eq!(crop.crop_code, None);
}

/// An imported crop carries where it came from, and the audit image carries it
/// too — a receiving device must be able to rebuild the row from `after` alone.
#[test]
fn insert_crop_with_provenance_persists_and_logs_it() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            // PRODUCTOS code 5 = CEBADA (vendored FEGA catalogue), the code the
            // SIGPAC declaration import stores verbatim.
            crop_code: Some("5".into()),
            source: Some("sigpac".into()),
            source_campaign: Some(2025),
            declared_area_ha: Some(29.68),
            ..base_crop(&plot.id, &season.id)
        },
        None,
    )
    .unwrap();

    assert_eq!(crop.crop_code.as_deref(), Some("5"));
    assert_eq!(crop.source, "sigpac");
    assert_eq!(crop.source_campaign, Some(2025));
    assert_eq!(crop.declared_area_ha, Some(29.68));

    let (op, _, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(op, "insert");
    assert_eq!(after["crop_code"], "5");
    assert_eq!(after["source"], "sigpac");
    assert_eq!(after["source_campaign"], 2025);
    assert_eq!(after["declared_area_ha"], 29.68);
}

/// Provenance is set-if-present: the manual edit form does not carry it, and a
/// typo fix must not erase which declaration a row came from. `crop_code` is
/// form state instead, so it follows the full-row rule and clears.
#[test]
fn update_crop_keeps_provenance_the_form_does_not_send() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            crop_code: Some("5".into()),
            source: Some("sigpac".into()),
            source_campaign: Some(2025),
            declared_area_ha: Some(29.68),
            ..base_crop(&plot.id, &season.id)
        },
        None,
    )
    .unwrap();

    let edited = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "cebada de dos carreras".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(28.0),
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(edited.source, "sigpac");
    assert_eq!(edited.source_campaign, Some(2025));
    assert_eq!(edited.declared_area_ha, Some(29.68));
    // Form state, so an absent value really is "no code" — detaching a species
    // from the catalogue is how free-text entry stays available.
    assert_eq!(edited.crop_code, None);

    let (_, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(before["declared_area_ha"], 29.68);
    assert_eq!(after["declared_area_ha"], 29.68);
    assert_eq!(after["crop_code"], Value::Null);
    assert_eq!(after["area_ha"], 28.0);
}

#[test]
fn soft_delete_crop_hides_it_and_logs_both_images() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let new_crop = |species: &str| NewCrop {
        plot_id: plot.id.clone(),
        season_id: season.id.clone(),
        species_name: species.into(),
        variety: None,
        production_system_code: None,
        area_ha: None,
        irrigation_code: None,
        growing_environment_code: None,
        gip_system_code: None,
        sown_on: None,
        crop_code: None,
        source: None,
        source_campaign: None,
        declared_area_ha: None,
    };
    let keep = repo::insert_crop(&mut conn, new_crop("cebada"), None).unwrap();
    let drop = repo::insert_crop(&mut conn, new_crop("veza"), None).unwrap();

    repo::soft_delete_crop(&mut conn, &drop.id, None).unwrap();

    let ids: Vec<String> = repo::list_crops(&conn, &season.id, &farm.id)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, vec![keep.id]);

    let (op, before, after) = last_change(&conn, "crop", &drop.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], Value::Null);
    assert!(after["deleted_at"].is_string());
    assert_eq!(after["species_name"], "veza", "complete after-image");

    assert!(matches!(
        repo::soft_delete_crop(&mut conn, &drop.id, None),
        Err(CoreError::NotFound)
    ));
}

/// Season and crop writes carry `record_change.season_id`, the column the future
/// sync layer scopes deltas by.
#[test]
fn season_and_crop_changes_record_their_season_scope() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "cebada".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "cebada de dos carreras".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    repo::soft_delete_crop(&mut conn, &crop.id, None).unwrap();

    let scope = |table: &str, id: &str| -> Option<String> {
        conn.query_row(
            "SELECT season_id FROM record_change
             WHERE entity_table = ?1 AND entity_id = ?2
             ORDER BY changed_at DESC, id DESC LIMIT 1",
            [table, id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(scope("crop", &crop.id).as_deref(), Some(season.id.as_str()));
    assert_eq!(
        scope("season", &season.id).as_deref(),
        Some(season.id.as_str())
    );
}

#[test]
fn insert_crop_with_unknown_plot_is_rejected_by_the_schema() {
    let mut conn = db();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let result = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: "0197fabc-0000-7000-8000-000000000000".into(),
            season_id: season.id,
            species_name: "trigo".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    );
    assert!(
        matches!(result, Err(CoreError::Sqlite(_))),
        "FK violation should surface"
    );
}

#[test]
fn insert_operator_round_trips_and_logs_full_image() {
    let mut conn = db();
    let operator = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("CL-12345".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2027-03-01".into()),
        },
        None,
    )
    .unwrap();

    assert_eq!(operator.id.len(), 36, "UUIDv7 TEXT id");

    let (op, before, after) = last_change(&conn, "operator", &operator.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    for column in [
        "id",
        "full_name",
        "licence_number",
        "licence_level_code",
        "licence_expiry_date",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["licence_expiry_date"], "2027-03-01");
}

/// Complements module-cue's with-extension test (which asserts core row and
/// registry extension are logged separately): without any registry number
/// (ROMA or REGANIP) there must be no extension row and no extension log
/// entry at all.
#[test]
fn insert_machinery_without_registry_numbers_writes_no_extension() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let machine = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: Some("2026-07-01".into()),
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap();

    let (op, before, after) = last_change(&conn, "machinery", &machine.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // The Rust field is `kind` but the column (and payload key) is `type` —
    // the serde rename keeps the sync payload aligned with the schema.
    assert_eq!(after["type"], "sprayer");
    assert_eq!(after["last_inspection_date"], Value::Null);

    let extension_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM machinery_es_extension", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(extension_rows, 0);
    let extension_logs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change WHERE entity_table = 'machinery_es_extension'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(extension_logs, 0);
}

// ---------------------------------------------------------------------------
// List functions backing the treatment entry UI selectors (2026-07-02)
// ---------------------------------------------------------------------------

#[test]
fn list_seasons_orders_newest_campaign_first() {
    let mut conn = db();
    repo::insert_season(&mut conn, new_season(2025, "2025"), None).unwrap();
    repo::insert_season(&mut conn, new_season(2027, "2027"), None).unwrap();
    repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();

    let years: Vec<i64> = repo::list_seasons(&conn)
        .unwrap()
        .iter()
        .map(|s| s.campaign_year)
        .collect();
    assert_eq!(years, vec![2027, 2026, 2025]);
}

#[test]
fn season_validation_rejects_blank_label() {
    let mut conn = db();
    let result = repo::insert_season(&mut conn, new_season(2026, "   "), None);
    assert!(matches!(result, Err(CoreError::Invalid("empty_name"))));
}

#[test]
fn crop_validation_rejects_blank_species() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();

    let result = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id,
            season_id: season.id,
            species_name: "  ".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    );
    assert!(matches!(result, Err(CoreError::Invalid("empty_name"))));
}

#[test]
fn list_crops_is_per_season_and_farm() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("Finca A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("Finca B"), None).unwrap();
    let plot_a = repo::insert_plot(&mut conn, new_plot(&farm_a.id, "A1"), None).unwrap();
    let plot_b = repo::insert_plot(&mut conn, new_plot(&farm_b.id, "B1"), None).unwrap();
    let season_1 = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let season_2 = repo::insert_season(&mut conn, new_season(2027, "2027"), None).unwrap();

    let crop = |plot_id: &str, season_id: &str, species: &str| NewCrop {
        plot_id: plot_id.into(),
        season_id: season_id.into(),
        species_name: species.into(),
        variety: None,
        production_system_code: None,
        area_ha: None,
        irrigation_code: None,
        growing_environment_code: None,
        gip_system_code: None,
        sown_on: None,
        crop_code: None,
        source: None,
        source_campaign: None,
        declared_area_ha: None,
    };
    // Only this one matches (farm A, season 1):
    let wheat =
        repo::insert_crop(&mut conn, crop(&plot_a.id, &season_1.id, "trigo"), None).unwrap();
    // Same farm, other season; other farm, same season:
    repo::insert_crop(&mut conn, crop(&plot_a.id, &season_2.id, "cebada"), None).unwrap();
    repo::insert_crop(&mut conn, crop(&plot_b.id, &season_1.id, "girasol"), None).unwrap();

    let listed = repo::list_crops(&conn, &season_1.id, &farm_a.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, wheat.id);
    assert_eq!(listed[0].species_name, "trigo");
}

#[test]
fn list_operators_is_stable_in_insertion_order() {
    let mut conn = db();
    let operator = |name: &str| NewOperator {
        full_name: name.into(),
        tax_id: None,
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: None,
    };
    repo::insert_operator(&mut conn, operator("Marta Ruiz"), None).unwrap();
    repo::insert_operator(&mut conn, operator("Ana López"), None).unwrap();

    // Insertion order, not alphabetical: names are collated by whoever displays
    // them (src/lib/collate.js, terrazgo-recordbook's NameCollator), because
    // SQLite sorts with BINARY collation and would file "Ana López" after
    // "Zubiri". UUIDv7 ids make `ORDER BY id` insertion-ordered, so this is
    // deterministic without implying an alphabet.
    let names: Vec<String> = repo::list_operators(&conn)
        .unwrap()
        .into_iter()
        .map(|o| o.full_name)
        .collect();
    assert_eq!(names, vec!["Marta Ruiz", "Ana López"]);
}

#[test]
fn list_machinery_is_per_farm() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("Finca A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("Finca B"), None).unwrap();
    let machine = |farm_id: &str, name: &str| NewMachinery {
        farm_id: farm_id.into(),
        name: name.into(),
        kind: None,
        acquired_on: None,
        last_inspection_date: None,
        next_inspection_due_date: None,
        roma_number: None,
        reganip_number: None,
    };
    repo::insert_machinery(&mut conn, machine(&farm_a.id, "Atomizador"), None).unwrap();
    repo::insert_machinery(&mut conn, machine(&farm_b.id, "Pulverizador"), None).unwrap();

    let listed = repo::list_machinery(&conn, &farm_a.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Atomizador");
}

// ---------------------------------------------------------------------------
// Operator + machinery registry CRUD (entry UI, 2026-07-03)
// ---------------------------------------------------------------------------

fn plain_operator(name: &str) -> NewOperator {
    NewOperator {
        full_name: name.into(),
        tax_id: None,
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: None,
    }
}

fn plain_machinery(farm_id: &str, name: &str) -> NewMachinery {
    NewMachinery {
        farm_id: farm_id.into(),
        name: name.into(),
        kind: None,
        acquired_on: None,
        last_inspection_date: None,
        next_inspection_due_date: None,
        roma_number: None,
        reganip_number: None,
    }
}

#[test]
fn operator_validation_rejects_blank_name() {
    let mut conn = db();
    assert!(matches!(
        repo::insert_operator(&mut conn, plain_operator("  "), None),
        Err(CoreError::Invalid("empty_name"))
    ));
}

#[test]
fn update_operator_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let operator = repo::insert_operator(&mut conn, plain_operator("Ana López"), None).unwrap();

    let updated = repo::update_operator(
        &mut conn,
        &operator.id,
        UpdateOperator {
            full_name: "Ana López García".into(),
            tax_id: None,
            licence_number: Some("CL-99".into()),
            licence_level_code: Some("basic".into()),
            licence_expiry_date: Some("2028-01-01".into()),
        },
        None,
    )
    .unwrap();
    assert_eq!(updated.full_name, "Ana López García");
    assert_eq!(updated.licence_expiry_date.as_deref(), Some("2028-01-01"));

    let (op, before, after) = last_change(&conn, "operator", &operator.id);
    assert_eq!(op, "update");
    assert_eq!(before["full_name"], "Ana López");
    assert_eq!(after["licence_number"], "CL-99");
    // Complete images: untouched columns present on both sides.
    assert!(before.get("created_at").is_some());
    assert!(after.get("created_at").is_some());
}

#[test]
fn update_operator_rejects_blank_name_and_missing_row() {
    let mut conn = db();
    let operator = repo::insert_operator(&mut conn, plain_operator("Ana"), None).unwrap();
    let update = |name: &str| UpdateOperator {
        full_name: name.into(),
        tax_id: None,
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: None,
    };
    assert!(matches!(
        repo::update_operator(&mut conn, &operator.id, update("  "), None),
        Err(CoreError::Invalid("empty_name"))
    ));
    repo::soft_delete_operator(&mut conn, &operator.id, None).unwrap();
    assert!(matches!(
        repo::update_operator(&mut conn, &operator.id, update("Ana"), None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn soft_delete_operator_hides_from_list_and_keeps_row() {
    let mut conn = db();
    let keep = repo::insert_operator(&mut conn, plain_operator("Keep"), None).unwrap();
    let gone = repo::insert_operator(&mut conn, plain_operator("Gone"), None).unwrap();

    repo::soft_delete_operator(&mut conn, &gone.id, None).unwrap();

    let listed = repo::list_operators(&conn).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, keep.id);

    let raw: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM operator WHERE id = ?1",
            [&gone.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, 1, "soft delete keeps the row");

    let (op, before, after) = last_change(&conn, "operator", &gone.id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(!after["deleted_at"].is_null());
}

#[test]
fn machinery_validation_rejects_blank_name() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    assert!(matches!(
        repo::insert_machinery(&mut conn, plain_machinery(&farm.id, " "), None),
        Err(CoreError::Invalid("empty_name"))
    ));
}

#[test]
fn update_machinery_replaces_fields_and_keeps_farm() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let machine =
        repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Old"), None).unwrap();

    let detail = repo::update_machinery(
        &mut conn,
        &machine.id,
        UpdateMachinery {
            name: "New".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: Some("2025-05-01".into()),
            next_inspection_due_date: Some("2028-05-01".into()),
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(detail.machinery.name, "New");
    assert_eq!(detail.machinery.farm_id, farm.id, "farm_id is immutable");
    assert!(detail.es.is_none());

    let (op, before, after) = last_change(&conn, "machinery", &machine.id);
    assert_eq!(op, "update");
    assert_eq!(before["name"], "Old");
    // The payload key is the real column name `type` (serde rename).
    assert_eq!(after["type"], "sprayer");
}

#[test]
fn update_machinery_reconciles_registry_extension_transitions() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let machine =
        repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Atomizador"), None).unwrap();
    let update = |roma: Option<&str>, reganip: Option<&str>| UpdateMachinery {
        name: "Atomizador".into(),
        kind: None,
        acquired_on: None,
        last_inspection_date: None,
        next_inspection_due_date: None,
        roma_number: roma.map(str::to_string),
        reganip_number: reganip.map(str::to_string),
    };

    // none -> some: extension inserted.
    let detail =
        repo::update_machinery(&mut conn, &machine.id, update(None, Some("REG-1")), None).unwrap();
    assert_eq!(detail.es.unwrap().reganip_number.as_deref(), Some("REG-1"));
    let (op, _, after) = last_change(&conn, "machinery_es_extension", &machine.id);
    assert_eq!(op, "insert");
    assert_eq!(after["roma_number"], Value::Null);
    assert_eq!(after["reganip_number"], "REG-1");

    // some -> some: extension updated, both registries carried.
    repo::update_machinery(
        &mut conn,
        &machine.id,
        update(Some("VA-1"), Some("REG-2")),
        None,
    )
    .unwrap();
    let (op, before, after) = last_change(&conn, "machinery_es_extension", &machine.id);
    assert_eq!(op, "update");
    assert_eq!(before["reganip_number"], "REG-1");
    assert_eq!(after["roma_number"], "VA-1");
    assert_eq!(after["reganip_number"], "REG-2");

    // Dropping one registry keeps the row while the other remains.
    let detail =
        repo::update_machinery(&mut conn, &machine.id, update(Some("VA-1"), None), None).unwrap();
    let es = detail.es.unwrap();
    assert_eq!(es.roma_number.as_deref(), Some("VA-1"));
    assert!(es.reganip_number.is_none());

    // both none: extension hard-deleted, null after-image.
    let detail = repo::update_machinery(&mut conn, &machine.id, update(None, None), None).unwrap();
    assert!(detail.es.is_none());
    let (op, before, after) = last_change(&conn, "machinery_es_extension", &machine.id);
    assert_eq!(op, "delete");
    assert_eq!(before["roma_number"], "VA-1");
    assert!(after.is_null());
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM machinery_es_extension WHERE machinery_id = ?1",
            [&machine.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn soft_delete_machinery_hides_from_lists() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let keep = repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Keep"), None).unwrap();
    let gone = repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Gone"), None).unwrap();

    repo::soft_delete_machinery(&mut conn, &gone.id, None).unwrap();

    let listed = repo::list_machinery(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, keep.id);

    let (op, _, after) = last_change(&conn, "machinery", &gone.id);
    assert_eq!(op, "delete");
    assert!(!after["deleted_at"].is_null());
}

#[test]
fn list_machinery_details_pairs_rows_with_their_extension() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    repo::insert_machinery(
        &mut conn,
        NewMachinery {
            reganip_number: Some("REG-7".into()),
            ..plain_machinery(&farm.id, "Atomizador")
        },
        None,
    )
    .unwrap();
    repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Remolque"), None).unwrap();

    let details = repo::list_machinery_details(&conn, &farm.id).unwrap();
    assert_eq!(details.len(), 2);
    // list_machinery orders by name: Atomizador first.
    assert_eq!(
        details[0].es.as_ref().unwrap().reganip_number.as_deref(),
        Some("REG-7")
    );
    assert!(details[1].es.is_none());
}

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

// ---------------------------------------------------------------------------
// Geo features (exclusive-arc geometry storage)
// ---------------------------------------------------------------------------

const SQUARE: &str = r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.66],[-4.72,41.65]]]}"#;
const SQUARE_B: &str = r#"{"type":"Polygon","coordinates":[[[-4.62,41.55],[-4.61,41.55],[-4.61,41.56],[-4.62,41.56],[-4.62,41.55]]]}"#;

fn boundary_for_plot(plot_id: &str, source: &str, geometry: &str) -> NewGeoFeature {
    NewGeoFeature {
        plot_id: Some(plot_id.into()),
        farm_id: None,
        role: "boundary".into(),
        geometry: geometry.into(),
        source: source.into(),
        campaign: None,
        official_area_ha: None,
        properties: None,
        fetched_at: None,
    }
}

#[test]
fn save_geo_feature_inserts_and_logs_complete_image() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let feature = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    assert_eq!(feature.plot_id.as_deref(), Some(plot.id.as_str()));
    assert!(feature.farm_id.is_none());

    let (op, before, after) = last_change(&conn, "geo_feature", &feature.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: every column present, not a subset.
    assert_eq!(after["role"], "boundary");
    assert_eq!(after["source"], "manual");
    assert_eq!(after["geometry"], SQUARE);
    assert!(after.get("created_at").is_some());
    assert!(after.get("official_area_ha").is_some());

    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, feature.id);
}

#[test]
fn save_geo_feature_replaces_active_row_within_same_source() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let first = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    let second = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE_B),
        None,
    )
    .unwrap();

    // Replacement soft-deletes the first row (history kept), with full images.
    let (op, before, after) = last_change(&conn, "geo_feature", &first.id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(!after["deleted_at"].is_null());
    assert_eq!(after["geometry"], SQUARE);

    // Only the new row is active; the old row still exists physically.
    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, second.id);
    assert_eq!(listed[0].geometry, SQUARE_B);
    let raw: i64 = conn
        .query_row("SELECT COUNT(*) FROM geo_feature", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, 2);
}

#[test]
fn geo_feature_sources_coexist() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "import", SQUARE_B),
        None,
    )
    .unwrap();

    // A manual boundary and an imported one are both active (discrepancy
    // display case), because replacement is scoped to (subject, role, source).
    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn geo_feature_farm_arc_saves_and_lists() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();

    let feature = repo::save_geo_feature(
        &mut conn,
        NewGeoFeature {
            plot_id: None,
            farm_id: Some(farm.id.clone()),
            role: "boundary".into(),
            geometry: SQUARE.into(),
            source: "manual".into(),
            campaign: None,
            official_area_ha: None,
            properties: None,
            fetched_at: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(feature.farm_id.as_deref(), Some(farm.id.as_str()));

    let listed = repo::list_geo_features_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(listed.len(), 1);
}

#[test]
fn geo_feature_arc_validation_rejects_bad_shapes() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let mut no_subject = boundary_for_plot(&plot.id, "manual", SQUARE);
    no_subject.plot_id = None;
    assert!(matches!(
        repo::save_geo_feature(&mut conn, no_subject, None),
        Err(CoreError::Invalid("geo_subject_missing"))
    ));

    let mut both_subjects = boundary_for_plot(&plot.id, "manual", SQUARE);
    both_subjects.farm_id = Some(farm.id.clone());
    assert!(matches!(
        repo::save_geo_feature(&mut conn, both_subjects, None),
        Err(CoreError::Invalid("geo_subject_ambiguous"))
    ));
}

#[test]
fn geo_feature_requires_active_subject() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    // Unknown plot id.
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot("no-such-plot", "manual", SQUARE),
            None
        ),
        Err(CoreError::NotFound)
    ));

    // Soft-deleted plot: hidden subjects don't take geometry.
    repo::soft_delete_plot(&mut conn, &plot.id, None).unwrap();
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot(&plot.id, "manual", SQUARE),
            None
        ),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn geo_feature_rejects_invalid_geometry() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();

    let unclosed = r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.70,41.60]]]}"#;
    assert!(matches!(
        repo::save_geo_feature(
            &mut conn,
            boundary_for_plot(&plot.id, "manual", unclosed),
            None
        ),
        Err(CoreError::Invalid("geometry_invalid"))
    ));
}

#[test]
fn soft_delete_geo_feature_hides_row_and_logs() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Recinto 1"), None).unwrap();
    let feature = repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot.id, "manual", SQUARE),
        None,
    )
    .unwrap();

    repo::soft_delete_geo_feature(&mut conn, &feature.id, None).unwrap();

    assert!(
        repo::list_geo_features_for_farm(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
    let (op, _, after) = last_change(&conn, "geo_feature", &feature.id);
    assert_eq!(op, "delete");
    assert!(!after["deleted_at"].is_null());

    // Second delete: already hidden.
    assert!(matches!(
        repo::soft_delete_geo_feature(&mut conn, &feature.id, None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn list_geo_features_is_scoped_to_the_farm() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("B"), None).unwrap();
    let plot_a = repo::insert_plot(&mut conn, new_plot(&farm_a.id, "Recinto A"), None).unwrap();
    let plot_b = repo::insert_plot(&mut conn, new_plot(&farm_b.id, "Recinto B"), None).unwrap();

    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot_a.id, "manual", SQUARE),
        None,
    )
    .unwrap();
    repo::save_geo_feature(
        &mut conn,
        boundary_for_plot(&plot_b.id, "manual", SQUARE_B),
        None,
    )
    .unwrap();

    let listed = repo::list_geo_features_for_farm(&conn, &farm_a.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].plot_id.as_deref(), Some(plot_a.id.as_str()));
}

// ---------------------------------------------------------------------------
// Zone flags (plot_zone_flag)
// ---------------------------------------------------------------------------

fn zone_flag(zone: &str, status: &str, pct: Option<f64>) -> NewZoneFlag {
    NewZoneFlag {
        zone_type_code: zone.into(),
        status: status.into(),
        coverage_pct: pct,
        detail: None,
    }
}

#[test]
fn replace_zone_flags_stores_results_and_logs_inserts() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    let stored = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![
            zone_flag("nitrate_vulnerable", "inside", Some(100.0)),
            zone_flag("phytosanitary_restriction", "inside", Some(99.9)),
            // Negative results are stored too: proof the check ran and was clear.
            zone_flag("natura_2000", "outside", None),
        ],
        None,
    )
    .unwrap();
    assert_eq!(stored.len(), 3);
    assert!(
        stored
            .iter()
            .all(|f| f.campaign == 2026 && f.source == "sigpac")
    );
    let natura = stored
        .iter()
        .find(|f| f.zone_type_code == "natura_2000")
        .unwrap();
    assert_eq!(natura.status, "outside");
    assert_eq!(natura.coverage_pct, None);

    // Complete after-images in the audit log (sync delta contract).
    let (op, _, after) = last_change(&conn, "plot_zone_flag", &stored[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["plot_id"], plot.id);
    assert_eq!(after["campaign"], 2026);
    assert_eq!(after["status"], "inside");
}

#[test]
fn recheck_replaces_within_campaign_and_appends_across_campaigns() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    let first = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "outside", None)],
        None,
    )
    .unwrap();
    // Re-check the SAME campaign: the zone declaration changed → replace.
    let second = repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();
    // A NEW campaign appends; the 2026 history stays provable.
    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2027,
        "sigpac",
        vec![zone_flag("nitrate_vulnerable", "inside", Some(100.0))],
        None,
    )
    .unwrap();

    let active = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(active.len(), 2); // one per campaign
    assert!(active.iter().all(|f| f.status == "inside"));

    // The replaced 2026 row is soft-deleted with a delete log, not erased.
    let (op, before, after) = last_change(&conn, "plot_zone_flag", &first[0].id);
    assert_eq!(op, "delete");
    assert_eq!(before["status"], "outside");
    assert!(after["deleted_at"].is_string());
    assert_ne!(first[0].id, second[0].id);
}

#[test]
fn zone_flags_validate_status_and_plot() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Zonas"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();

    assert!(matches!(
        repo::replace_zone_flags(
            &mut conn,
            &plot.id,
            2026,
            "sigpac",
            vec![zone_flag("nitrate_vulnerable", "maybe", None)],
            None,
        ),
        Err(CoreError::Invalid("zone_status_invalid"))
    ));
    assert!(matches!(
        repo::replace_zone_flags(&mut conn, "missing-plot", 2026, "sigpac", vec![], None),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn zone_flag_listing_is_scoped_to_the_farms_active_plots() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Mine"), None).unwrap();
    let other = repo::insert_farm(&mut conn, new_farm("Other"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "P1"), None).unwrap();
    let foreign = repo::insert_plot(&mut conn, new_plot(&other.id, "P2"), None).unwrap();

    repo::replace_zone_flags(
        &mut conn,
        &plot.id,
        2026,
        "sigpac",
        vec![zone_flag("natura_2000", "inside", Some(12.5))],
        None,
    )
    .unwrap();
    repo::replace_zone_flags(
        &mut conn,
        &foreign.id,
        2026,
        "sigpac",
        vec![zone_flag("natura_2000", "inside", Some(50.0))],
        None,
    )
    .unwrap();

    let flags = repo::list_zone_flags_for_farm(&conn, &farm.id).unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].plot_id, plot.id);

    // Deleting the plot hides its flags from the listing.
    repo::soft_delete_plot(&mut conn, &plot.id, None).unwrap();
    assert!(
        repo::list_zone_flags_for_farm(&conn, &farm.id)
            .unwrap()
            .is_empty()
    );
}

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

// ---------------------------------------------------------------------------
// Slice 5: the fields the printed model asks for
// ---------------------------------------------------------------------------

/// Model 1.1 asks for postal contact details of the holding; they are universal
/// (every country's book wants them) so they live on `farm`, not the regional
/// extension. The create form does not offer them — 1.1 is set up once, in the
/// edit form — so a new farm carries them as NULL until updated.
#[test]
fn farm_contact_details_round_trip_and_are_audited() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    assert_eq!(farm.address, None, "not on the create form");

    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            location_text: Some("Medina del Campo".into()),
            address: Some("Camino de la Vega, 4".into()),
            postal_code: Some("47400".into()),
            phone_fixed: Some("983000000".into()),
            phone_mobile: Some("600000000".into()),
            email: Some("maria@example.es".into()),
            opened_on: None,
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("ES244700000123".into()),
                siex_code: Some("ES470000000123".into()),
                province_code: Some("47".into()),
            }),
            representative: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(detail.farm.address.as_deref(), Some("Camino de la Vega, 4"));
    assert_eq!(detail.farm.postal_code.as_deref(), Some("47400"));
    assert_eq!(detail.farm.email.as_deref(), Some("maria@example.es"));
    // The national and autonómico registry numbers are separate columns: the
    // model prints them side by side, so one field could never serve both.
    let es = detail.es.expect("extension");
    assert_eq!(es.siex_code.as_deref(), Some("ES470000000123"));
    assert_eq!(es.rea_code.as_deref(), Some("ES244700000123"));

    let (_, before, after) = last_change(&conn, "farm", &farm.id);
    assert_eq!(before["address"], Value::Null);
    assert_eq!(after["address"], "Camino de la Vega, 4");
    assert_eq!(after["phone_mobile"], "600000000");
}

/// The representative follows the extension contract exactly: absent block
/// means none, present means insert-or-update, and removing it hard-deletes
/// the row with a null after-image.
#[test]
fn farm_representative_is_reconciled_from_the_submitted_state() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let update = |rep: Option<FarmRepresentativeFields>| UpdateFarm {
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
        representative: rep,
    };
    let fields = |name: &str| FarmRepresentativeFields {
        full_name: name.into(),
        tax_id: Some("87654321X".into()),
        representation_kind: Some("Administrador único".into()),
        address: None,
        locality: None,
        province: None,
        postal_code: None,
        phone: None,
        email: None,
    };

    // None → nothing stored, nothing logged.
    let detail = repo::update_farm(&mut conn, &farm.id, update(None), None).unwrap();
    assert!(detail.representative.is_none());

    // Insert.
    let detail =
        repo::update_farm(&mut conn, &farm.id, update(Some(fields("Ana Ruiz"))), None).unwrap();
    let rep = detail.representative.expect("representative stored");
    assert_eq!(rep.full_name, "Ana Ruiz");
    assert_eq!(
        rep.representation_kind.as_deref(),
        Some("Administrador único")
    );
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    assert_eq!(after["full_name"], "Ana Ruiz");

    // Update in place.
    repo::update_farm(
        &mut conn,
        &farm.id,
        update(Some(fields("Ana Ruiz Pérez"))),
        None,
    )
    .unwrap();
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "update");
    assert_eq!(before["full_name"], "Ana Ruiz");
    assert_eq!(after["full_name"], "Ana Ruiz Pérez");

    // Removing the block hard-deletes the row, logged with a null after-image
    // (the farm_es_extension precedent).
    let detail = repo::update_farm(&mut conn, &farm.id, update(None), None).unwrap();
    assert!(detail.representative.is_none());
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "delete");
    assert_eq!(before["full_name"], "Ana Ruiz Pérez");
    assert!(after.is_null());

    // And a blank name is rejected like every other user-entered name.
    let blank = repo::update_farm(&mut conn, &farm.id, update(Some(fields("  "))), None);
    assert!(matches!(blank, Err(CoreError::Invalid("empty_name"))));
}

/// Anexo III A.2.e: "secano o regadío (indicando en su caso el sistema de
/// riego)" and "al aire libre o protegido". Both are coded lists, not booleans
/// — the official model prints four siglas for each.
#[test]
fn crop_carries_its_own_surface_and_agronomic_codes() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();

    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(1.25),
            irrigation_code: Some("sprinkler".into()),
            growing_environment_code: Some("open_air".into()),
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(crop.area_ha, Some(1.25));
    assert_eq!(crop.irrigation_code.as_deref(), Some("sprinkler"));

    let updated = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(1.5),
            irrigation_code: Some("drip".into()),
            growing_environment_code: Some("greenhouse".into()),
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(updated.area_ha, Some(1.5));
    assert_eq!(
        updated.growing_environment_code.as_deref(),
        Some("greenhouse")
    );

    let (_, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(before["irrigation_code"], "sprinkler");
    assert_eq!(after["irrigation_code"], "drip");
    assert_eq!(after["area_ha"], 1.5);

    // The codes are real foreign keys — a typo cannot reach the book.
    let bad = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: Some("no-such-system".into()),
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    );
    assert!(bad.is_err(), "unknown irrigation code must be refused");
}

/// Model 1.2 prints a NIF beside every applicator, 1.3 an acquisition date
/// beside every machine (Anexo III A.1.c and A.1.h).
#[test]
fn operator_tax_id_and_machinery_acquisition_date_round_trip() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();

    let operator = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: Some("11111111H".into()),
            licence_number: Some("ROPO-1".into()),
            licence_level_code: Some("pilot".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(operator.tax_id.as_deref(), Some("11111111H"));
    // 'pilot' is the aerial carné the model prints as a fourth column.
    assert_eq!(operator.licence_level_code.as_deref(), Some("pilot"));

    let machinery = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Atomizador".into(),
            kind: None,
            acquired_on: Some("2018-03-15".into()),
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(machinery.acquired_on.as_deref(), Some("2018-03-15"));

    let (_, _, after) = last_change(&conn, "machinery", &machinery.id);
    assert_eq!(after["acquired_on"], "2018-03-15");
    let (_, _, after) = last_change(&conn, "operator", &operator.id);
    assert_eq!(after["tax_id"], "11111111H");
}

// --- commercialised harvest (model section 5) -------------------------------
//
// In core rather than in the CUE module: what leaves the holding and to whom is
// whole-farm data the costs and analytics modules will want. Fully correctable,
// like the treated-seed register — the record holds no snapshot of another
// row's identity, so there is nothing a later edit elsewhere could rewrite.

struct HarvestFixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
    crop_a: String,
}

fn harvest_fixture(conn: &mut Connection) -> HarvestFixture {
    let season = repo::insert_season(conn, new_season(2026, "2025/2026"), None).unwrap();
    let farm = repo::insert_farm(conn, new_farm("Finca La Vega"), None).unwrap();
    let plot_a = repo::insert_plot(conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let plot_b = repo::insert_plot(conn, new_plot(&farm.id, "La Loma"), None).unwrap();
    let crop_a = repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_a.id.clone(),
            season_id: season.id.clone(),
            species_name: "trigo blando".into(),
            variety: Some("Nogal".into()),
            production_system_code: None,
            sown_on: None,
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
    .unwrap();

    HarvestFixture {
        season_id: season.id,
        farm_id: farm.id,
        plot_a: plot_a.id,
        plot_b: plot_b.id,
        crop_a: crop_a.id,
    }
}

fn new_harvest(fx: &HarvestFixture) -> NewHarvestRecord {
    NewHarvestRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        harvested_on: "2026-07-24".into(),
        product_name: "trigo blando".into(),
        plant_product_code: Some("1".into()),
        quantity_value: Some(42.5),
        quantity_unit_code: Some("t".into()),
        delivery_note_ref: Some("ALB-2026/318".into()),
        lot_number: Some("L-26-07".into()),
        buyer_name: "Cooperativa Cerealista del Duero".into(),
        buyer_tax_id: Some("F47008123".into()),
        buyer_address: Some("Ctra. Palencia km 4, Valladolid".into()),
        buyer_registry_number: Some("21.0012345/VA".into()),
        notes: None,
        plots: vec![NewHarvestPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: Some(fx.crop_a.clone()),
        }],
    }
}

/// The model's field list for section 5: date, product, quantity, the parcels
/// of origin, the delivery-note and lot references, and the buyer block down to
/// the "Nº de RGSEAA" — which core stores under a neutral name because core
/// tables carry no regional identifiers.
#[test]
fn a_harvest_records_what_left_the_holding_and_to_whom() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    assert_eq!(saved.record.harvested_on, "2026-07-24");
    assert_eq!(saved.record.quantity_value, Some(42.5));
    assert_eq!(saved.record.quantity_unit_code.as_deref(), Some("t"));
    assert_eq!(
        saved.record.buyer_registry_number.as_deref(),
        Some("21.0012345/VA")
    );
    assert_eq!(saved.plots.len(), 1);
    // The harvested crop is frozen, so renaming it later cannot rewrite what
    // the printed book said was sold.
    assert_eq!(
        saved.plots[0].crop_name_snapshot.as_deref(),
        Some("trigo blando")
    );
    assert_eq!(saved.plots[0].variety_snapshot.as_deref(), Some("Nogal"));
}

/// A quantity is a value AND its unit or neither: an amount with no unit is not
/// a statement, and the set is {kg, t} because that is what the model measures a
/// sold harvest in. Enforced here rather than by a foreign key — `unit` is a
/// module-cue lookup and core may never reference a module's table.
#[test]
fn a_harvest_quantity_is_a_value_and_a_unit_or_neither() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    // Both absent: the printed cell is left to be filled by hand.
    let mut blank = new_harvest(&fx);
    blank.quantity_value = None;
    blank.quantity_unit_code = None;
    let saved = repo::insert_harvest_record(&mut conn, blank, None).unwrap();
    assert!(saved.record.quantity_value.is_none());

    for (value, unit) in [(Some(1200.0), Some("kg")), (Some(1.2), Some("t"))] {
        let mut ok = new_harvest(&fx);
        ok.quantity_value = value;
        ok.quantity_unit_code = unit.map(str::to_string);
        assert!(repo::insert_harvest_record(&mut conn, ok, None).is_ok());
    }

    // A litre of wheat is a different claim, not a unit slip; and a value with
    // no unit, or a unit with no value, says nothing.
    for (value, unit) in [
        (Some(1200.0), Some("l")),
        (Some(1200.0), Some("m3")),
        (Some(1200.0), None),
        (None, Some("kg")),
        (Some(0.0), Some("kg")),
        (Some(-3.0), Some("t")),
    ] {
        let mut bad = new_harvest(&fx);
        bad.quantity_value = value;
        bad.quantity_unit_code = unit.map(str::to_string);
        assert!(
            matches!(
                repo::insert_harvest_record(&mut conn, bad, None).unwrap_err(),
                CoreError::Invalid("invalid_harvest_quantity")
            ),
            "accepted {value:?} {unit:?}"
        );
    }
}

#[test]
fn a_harvest_needs_a_product_a_buyer_and_at_least_one_plot() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut blank_product = new_harvest(&fx);
    blank_product.product_name = "  ".into();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, blank_product, None).unwrap_err(),
        CoreError::Invalid("empty_name")
    ));

    let mut blank_buyer = new_harvest(&fx);
    blank_buyer.buyer_name = String::new();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, blank_buyer, None).unwrap_err(),
        CoreError::Invalid("empty_buyer_name")
    ));

    let mut no_plots = new_harvest(&fx);
    no_plots.plots.clear();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, no_plots, None).unwrap_err(),
        CoreError::Invalid("no_plots")
    ));
}

/// A parcel on another holding would put foreign land in this farm's book.
#[test]
fn a_harvest_plot_must_be_on_the_same_farm() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let other = repo::insert_farm(&mut conn, new_farm("Finca ajena"), None).unwrap();
    let foreign = repo::insert_plot(&mut conn, new_plot(&other.id, "El Soto"), None).unwrap();

    let mut record = new_harvest(&fx);
    record.plots = vec![NewHarvestPlot {
        plot_id: foreign.id,
        crop_id: None,
    }];
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, record, None).unwrap_err(),
        CoreError::Invalid("plot_not_on_farm")
    ));
}

#[test]
fn every_harvest_row_is_logged_with_a_complete_image_and_the_actor() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), Some("carlos")).unwrap();

    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: the log is the future sync delta source.
    for column in [
        "id",
        "season_id",
        "farm_id",
        "harvested_on",
        "product_name",
        "plant_product_code",
        "quantity_value",
        "quantity_unit_code",
        "delivery_note_ref",
        "lot_number",
        "buyer_name",
        "buyer_tax_id",
        "buyer_address",
        "buyer_registry_number",
        "notes",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }

    let (_, _, plot) = last_change(&conn, "harvest_plot", &saved.plots[0].id);
    assert_eq!(plot["crop_name_snapshot"], "trigo blando");

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
fn a_harvest_can_be_corrected_in_full_and_logs_both_images() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    let updated = repo::update_harvest_record(
        &mut conn,
        &saved.record.id,
        UpdateHarvestRecord {
            harvested_on: "2026-07-26".into(),
            product_name: "trigo blando".into(),
            plant_product_code: Some("1".into()),
            quantity_value: Some(44.0),
            quantity_unit_code: Some("t".into()),
            delivery_note_ref: Some("ALB-2026/322".into()),
            lot_number: None,
            buyer_name: "Harinera del Pisuerga S.L.".into(),
            buyer_tax_id: Some("B47999000".into()),
            buyer_address: None,
            buyer_registry_number: None,
            notes: Some("Albarán corregido tras el pesaje definitivo.".into()),
            plots: vec![NewHarvestPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.record.quantity_value, Some(44.0));
    assert_eq!(updated.record.buyer_name, "Harinera del Pisuerga S.L.");
    // Clearing an optional field really clears it.
    assert!(updated.record.lot_number.is_none());
    // The campaign and the holding are not the form's to move.
    assert_eq!(updated.record.season_id, saved.record.season_id);
    assert_eq!(updated.record.farm_id, saved.record.farm_id);

    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["quantity_value"], 42.5);
    assert_eq!(after["quantity_value"], 44.0);
}

/// The origin plots are reconciled from the submitted state — added, dropped
/// and changed rows each get their own audit entry, so the log stays
/// rebuildable.
#[test]
fn correcting_the_harvest_plots_reconciles_them_and_logs_each_change() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();
    let original_plot_row = saved.plots[0].id.clone();

    let updated = repo::update_harvest_record(
        &mut conn,
        &saved.record.id,
        UpdateHarvestRecord {
            harvested_on: saved.record.harvested_on.clone(),
            product_name: saved.record.product_name.clone(),
            plant_product_code: saved.record.plant_product_code.clone(),
            quantity_value: saved.record.quantity_value,
            quantity_unit_code: saved.record.quantity_unit_code.clone(),
            delivery_note_ref: saved.record.delivery_note_ref.clone(),
            lot_number: saved.record.lot_number.clone(),
            buyer_name: saved.record.buyer_name.clone(),
            buyer_tax_id: saved.record.buyer_tax_id.clone(),
            buyer_address: saved.record.buyer_address.clone(),
            buyer_registry_number: saved.record.buyer_registry_number.clone(),
            notes: None,
            // El Prado goes, La Loma arrives.
            plots: vec![NewHarvestPlot {
                plot_id: fx.plot_b.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.plots.len(), 1);
    assert_eq!(updated.plots[0].plot_id, fx.plot_b);

    // The dropped row is hard-deleted (it is a pure child, like an extension
    // row) and logged with a null after-image.
    let (op, _, after) = last_change(&conn, "harvest_plot", &original_plot_row);
    assert_eq!(op, "delete");
    assert!(after.is_null(), "a removed child logs a null after-image");

    let (op, _, _) = last_change(&conn, "harvest_plot", &updated.plots[0].id);
    assert_eq!(op, "insert");
}

#[test]
fn a_deleted_harvest_leaves_the_book_but_keeps_both_images() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    repo::soft_delete_harvest_record(&mut conn, &saved.record.id, None).unwrap();

    assert!(
        repo::list_harvest_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "delete");
    assert_eq!(before["buyer_name"], "Cooperativa Cerealista del Duero");
    assert!(
        after["deleted_at"].is_string(),
        "a soft delete logs the deleted row as its after-image"
    );
}

#[test]
fn harvests_list_per_farm_and_campaign_oldest_first() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut later = new_harvest(&fx);
    later.harvested_on = "2026-08-02".into();
    repo::insert_harvest_record(&mut conn, later, None).unwrap();
    let mut earlier = new_harvest(&fx);
    earlier.harvested_on = "2026-07-11".into();
    repo::insert_harvest_record(&mut conn, earlier, None).unwrap();

    let rows = repo::list_harvest_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].record.harvested_on, "2026-07-11");
    assert_eq!(rows[1].record.harvested_on, "2026-08-02");
}

/// The core half of the season-deletion guard. Every record-book view is read
/// through its season, so hiding one would hide the sale it holds — including a
/// soft-deleted one, whose audit history is reachable only that way.
#[test]
fn a_season_holding_a_harvest_cannot_be_deleted() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let empty = repo::insert_season(&mut conn, new_season(2028, "2027/2028"), None).unwrap();

    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();
    assert!(matches!(
        repo::soft_delete_season(&mut conn, &fx.season_id, None).unwrap_err(),
        CoreError::Invalid("season_in_use")
    ));
    // A season with nothing in it is still deletable.
    assert!(repo::soft_delete_season(&mut conn, &empty.id, None).is_ok());

    repo::soft_delete_harvest_record(&mut conn, &saved.record.id, None).unwrap();
    assert!(
        matches!(
            repo::soft_delete_season(&mut conn, &fx.season_id, None).unwrap_err(),
            CoreError::Invalid("season_in_use")
        ),
        "a soft-deleted sale still pins its season"
    );
}

// ---------------------------------------------------------------------------
// Water points (model 2.2's water half, Anexo III A.1.f–g)
// ---------------------------------------------------------------------------

/// A farm with two plots — enough to prove the lists are farm-scoped and the
/// declaration is per plot.
struct WaterFixture {
    farm_id: String,
    plot_id: String,
    other_plot_id: String,
}

fn water_fixture(conn: &mut Connection) -> WaterFixture {
    let farm = repo::insert_farm(conn, new_farm("Finca del agua"), None).unwrap();
    let plot = repo::insert_plot(conn, new_plot(&farm.id, "La Vega"), None).unwrap();
    let other = repo::insert_plot(conn, new_plot(&farm.id, "El Soto"), None).unwrap();
    WaterFixture {
        farm_id: farm.id,
        plot_id: plot.id,
        other_plot_id: other.id,
    }
}

fn new_water_point(plot_id: &str) -> NewWaterPoint {
    NewWaterPoint {
        plot_id: plot_id.into(),
        denomination: "Pozo del norte".into(),
        inside_plot: true,
        distance_m: None,
        latitude: None,
        longitude: None,
    }
}

#[test]
fn insert_water_point_round_trips_and_logs_a_full_image() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut new = new_water_point(&fx.plot_id);
    new.inside_plot = false;
    new.distance_m = Some(120.0);
    new.latitude = Some(41.65234);
    new.longitude = Some(-4.72891);
    let saved = repo::insert_water_point(&mut conn, new, None).unwrap();

    assert_eq!(saved.denomination, "Pozo del norte");
    assert!(!saved.inside_plot);
    assert_eq!(saved.distance_m, Some(120.0));

    let points = repo::list_water_points(&conn, &fx.farm_id).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].id, saved.id);
    assert_eq!(points[0].latitude, Some(41.65234));

    let (op, before, after) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image, not a field subset: the log is the sync delta source.
    assert_eq!(after["denomination"], "Pozo del norte");
    assert_eq!(after["inside_plot"], false);
    assert_eq!(after["distance_m"], 120.0);
    assert_eq!(after["longitude"], -4.72891);
}

/// Anexo III A.1.g asks for the distance when the point lies outside the plot,
/// and it is knowledge the farmer already has — so it is required, unlike the
/// values that are only observed later (efficacy, total quantity used).
#[test]
fn a_point_outside_the_plot_must_state_its_distance() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut outside = new_water_point(&fx.plot_id);
    outside.inside_plot = false;
    assert!(matches!(
        repo::insert_water_point(&mut conn, outside, None).unwrap_err(),
        CoreError::Invalid("missing_distance")
    ));

    // Zero is not a distance to something outside the plot.
    let mut zero = new_water_point(&fx.plot_id);
    zero.inside_plot = false;
    zero.distance_m = Some(0.0);
    assert!(matches!(
        repo::insert_water_point(&mut conn, zero, None).unwrap_err(),
        CoreError::Invalid("missing_distance")
    ));
}

/// A distance beside "included in the plot: YES" contradicts the cell next to
/// it — a wrong answer, not a missing one.
#[test]
fn a_point_inside_the_plot_cannot_carry_a_distance() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut inside = new_water_point(&fx.plot_id);
    inside.distance_m = Some(15.0);
    assert!(matches!(
        repo::insert_water_point(&mut conn, inside, None).unwrap_err(),
        CoreError::Invalid("water_point_distance_inside")
    ));
}

#[test]
fn water_point_coordinates_are_both_or_neither_and_in_range() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let mut half = new_water_point(&fx.plot_id);
    half.latitude = Some(41.65);
    assert!(matches!(
        repo::insert_water_point(&mut conn, half, None).unwrap_err(),
        CoreError::Invalid("water_point_coordinates_invalid")
    ));

    let mut off_globe = new_water_point(&fx.plot_id);
    off_globe.latitude = Some(91.0);
    off_globe.longitude = Some(-4.7);
    assert!(matches!(
        repo::insert_water_point(&mut conn, off_globe, None).unwrap_err(),
        CoreError::Invalid("water_point_coordinates_invalid")
    ));

    // Stating neither is the normal case: the model marks the column voluntary.
    let bare = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();
    assert_eq!((bare.latitude, bare.longitude), (None, None));
}

#[test]
fn water_point_validation_rejects_a_blank_denomination() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let mut blank = new_water_point(&fx.plot_id);
    blank.denomination = "   ".into();
    assert!(matches!(
        repo::insert_water_point(&mut conn, blank, None).unwrap_err(),
        CoreError::Invalid("empty_name")
    ));
}

/// Fully correctable, unlike the treatment registers: the row freezes no
/// snapshot of another row, so there is nothing an edit could rewrite.
#[test]
fn update_water_point_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let saved = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    let after = repo::update_water_point(
        &mut conn,
        &saved.id,
        UpdateWaterPoint {
            denomination: "  Sondeo municipal  ".into(),
            inside_plot: false,
            distance_m: Some(240.5),
            latitude: None,
            longitude: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(after.denomination, "Sondeo municipal");
    assert!(!after.inside_plot);
    assert_eq!(after.distance_m, Some(240.5));
    // The plot it belongs to is not part of the update.
    assert_eq!(after.plot_id, saved.plot_id);

    let (op, before, logged) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "update");
    assert_eq!(before["denomination"], "Pozo del norte");
    assert_eq!(before["inside_plot"], true);
    assert_eq!(logged["denomination"], "Sondeo municipal");
    assert_eq!(logged["distance_m"], 240.5);
}

#[test]
fn soft_delete_water_point_hides_it_and_logs_both_images() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let saved = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    repo::soft_delete_water_point(&mut conn, &saved.id, None).unwrap();
    assert!(
        repo::list_water_points(&conn, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    let (op, before, after) = last_change(&conn, "plot_water_point", &saved.id);
    assert_eq!(op, "delete");
    assert_eq!(before["denomination"], "Pozo del norte");
    assert!(after["deleted_at"].is_string());

    assert!(matches!(
        repo::soft_delete_water_point(&mut conn, &saved.id, None).unwrap_err(),
        CoreError::NotFound
    ));
}

#[test]
fn water_points_are_listed_per_farm_and_skip_deleted_plots() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let elsewhere = repo::insert_farm(&mut conn, new_farm("Otra finca"), None).unwrap();
    let far_plot = repo::insert_plot(&mut conn, new_plot(&elsewhere.id, "Lejos"), None)
        .unwrap()
        .id;

    repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();
    repo::insert_water_point(&mut conn, new_water_point(&far_plot), None).unwrap();
    let on_other =
        repo::insert_water_point(&mut conn, new_water_point(&fx.other_plot_id), None).unwrap();

    assert_eq!(
        repo::list_water_points(&conn, &fx.farm_id).unwrap().len(),
        2
    );

    // A point leaves the book with its plot; its audit history stays reachable.
    repo::soft_delete_plot(&mut conn, &fx.other_plot_id, None).unwrap();
    let left = repo::list_water_points(&conn, &fx.farm_id).unwrap();
    assert_eq!(left.len(), 1);
    assert!(left.iter().all(|p| p.id != on_other.id));
}

#[test]
fn a_water_point_needs_an_active_plot() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    repo::soft_delete_plot(&mut conn, &fx.plot_id, None).unwrap();
    assert!(matches!(
        repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap_err(),
        CoreError::NotFound
    ));
}

// --- the stored negative ----------------------------------------------------

#[test]
fn declaring_a_plot_free_of_water_points_round_trips_and_is_logged() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);

    let declared = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();
    assert_eq!(declared.declared_on, "2026-05-12");

    let standing = repo::list_water_declarations(&conn, &fx.farm_id).unwrap();
    assert_eq!(standing.len(), 1);
    assert_eq!(standing[0].plot_id, fx.plot_id);

    let (op, before, after) = last_change(&conn, "plot_water_declaration", &declared.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    assert_eq!(after["declared_on"], "2026-05-12");

    // Restating updates the standing row rather than printing the plot twice.
    let again = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-06-01", None).unwrap();
    assert_eq!(again.id, declared.id);
    assert_eq!(
        repo::list_water_declarations(&conn, &fx.farm_id)
            .unwrap()
            .len(),
        1
    );
}

/// First direction of the invariant: the rows and the "nothing here" contradict
/// each other, and the rows are the stronger statement.
#[test]
fn a_plot_holding_water_points_cannot_be_declared_free_of_them() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let point = repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    assert!(matches!(
        repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap_err(),
        CoreError::Invalid("plot_has_water_points")
    ));

    // The declaration is per plot: its neighbour is unaffected.
    assert!(repo::set_water_declaration(&mut conn, &fx.other_plot_id, "2026-05-12", None).is_ok());

    // Removing the point re-opens the question.
    repo::soft_delete_water_point(&mut conn, &point.id, None).unwrap();
    assert!(repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).is_ok());
}

/// Second direction: a stale "no captaciones" printing beside a contradicting
/// row would forge proof-of-check, so the record withdraws it as it lands.
#[test]
fn recording_a_water_point_withdraws_a_standing_declaration() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let declared = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();
    repo::set_water_declaration(&mut conn, &fx.other_plot_id, "2026-05-12", None).unwrap();

    repo::insert_water_point(&mut conn, new_water_point(&fx.plot_id), None).unwrap();

    let standing = repo::list_water_declarations(&conn, &fx.farm_id).unwrap();
    assert_eq!(
        standing.len(),
        1,
        "only this plot's declaration is withdrawn"
    );
    assert_eq!(standing[0].plot_id, fx.other_plot_id);

    // Withdrawal is a soft delete: the trail keeps saying what was declared.
    let (op, before, after) = last_change(&conn, "plot_water_declaration", &declared.id);
    assert_eq!(op, "delete");
    assert_eq!(before["declared_on"], "2026-05-12");
    assert!(after["deleted_at"].is_string());
}

#[test]
fn clearing_a_declaration_is_a_soft_delete_and_restating_mints_a_new_row() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    let first = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap();

    repo::clear_water_declaration(&mut conn, &fx.plot_id, None).unwrap();
    assert!(
        repo::list_water_declarations(&conn, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    // Clearing nothing is not an error — the panel toggles freely.
    assert!(repo::clear_water_declaration(&mut conn, &fx.plot_id, None).is_ok());

    let second = repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-07-03", None).unwrap();
    assert_ne!(
        second.id, first.id,
        "a withdrawn declaration is not resurrected"
    );
}

#[test]
fn a_declaration_needs_an_active_plot() {
    let mut conn = db();
    let fx = water_fixture(&mut conn);
    repo::soft_delete_plot(&mut conn, &fx.plot_id, None).unwrap();
    assert!(matches!(
        repo::set_water_declaration(&mut conn, &fx.plot_id, "2026-05-12", None).unwrap_err(),
        CoreError::NotFound
    ));
}
