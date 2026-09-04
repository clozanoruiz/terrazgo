// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calendar, the people and the machines: season, crop, operator and
//! machinery — their CRUD, their soft-delete rules, and the list functions the
//! treatment entry UI reads its selectors from.
//!
//! These moved into core from module-cue when the farm registry did; the
//! Spanish regulatory meaning of each is in docs/data-model.md.
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
// Season, crop, operator, machinery. These moved here from module-cue
// (2026-06-12); the CUE suite exercises them through fixtures, but their
// contracts belong to this crate's tests.
// ---------------------------------------------------------------------------

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
