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
    FarmEsFields, NewCrop, NewFarm, NewGeoFeature, NewMachinery, NewOperator, NewPlot, NewSeason,
    NewUserProfile, NewZoneFlag, PlotEsFields, UpdateFarm, UpdateMachinery, UpdateOperator,
    UpdatePlot, UpdateUserProfile,
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
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("REA-47-99999".into()),
                province_code: Some("47".into()),
            }),
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
            latitude: Some(41.65),
            longitude: Some(-4.72),
            country_code: "es".into(),
            es: None,
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
        latitude: None,
        longitude: None,
        country_code: "es".into(),
        es: None,
    };

    // none -> some: extension inserted.
    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
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
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: None,
                province_code: Some("09".into()),
            }),
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
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: None,
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
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["campaign_year"], 2026);
    assert_eq!(after["ends_on"], Value::Null);
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
            sown_on: Some("2025-11-02".into()),
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
            sown_on: None,
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
            sown_on: None,
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
        sown_on: None,
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
fn list_operators_orders_by_name() {
    let mut conn = db();
    let operator = |name: &str| NewOperator {
        full_name: name.into(),
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: None,
    };
    repo::insert_operator(&mut conn, operator("Marta Ruiz"), None).unwrap();
    repo::insert_operator(&mut conn, operator("Ana López"), None).unwrap();

    let names: Vec<String> = repo::list_operators(&conn)
        .unwrap()
        .into_iter()
        .map(|o| o.full_name)
        .collect();
    assert_eq!(names, vec!["Ana López", "Marta Ruiz"]);
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

#[test]
fn list_licence_levels_returns_seeded_reference_data() {
    let conn = db();
    let levels = repo::list_licence_levels(&conn).unwrap();
    let codes: Vec<&str> = levels.iter().map(|l| l.code.as_str()).collect();
    // Seed order (basic → qualified → fumigator), not alphabetical.
    assert_eq!(codes, vec!["basic", "qualified", "fumigator"]);
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
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: None,
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
