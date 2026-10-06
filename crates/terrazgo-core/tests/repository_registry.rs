// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The calendar, the people and the machines: season, crop, operator and
//! machinery — their CRUD, their soft-delete rules, and the list functions the
//! treatment entry UI reads its selectors from.
//!
//! These moved into core from module-phytosanitary when the farm registry did; the
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
// Season, crop, operator, machinery. These moved here from module-phytosanitary
// (2026-06-12); the phytosanitary suite exercises them through fixtures, but their
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
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(season.id.len(), 36, "UUIDv7 TEXT id");
    assert_eq!(season.status, "active", "a new season starts active");
    assert_eq!(season.label, "2025/2026", "the dates name an unnamed book");

    let (op, before, after) = last_change(&conn, "season", &season.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: every column present, absent optionals as null.
    for column in [
        "id",
        "farm_id",
        "label",
        "custom_label",
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
    assert_eq!(after["farm_id"], farm.id.as_str());
    assert_eq!(after["label"], "2025/2026");
    assert_eq!(after["custom_label"], Value::Null);
    assert_eq!(after["ends_on"], "2026-08-31");
    assert_eq!(after["deleted_at"], Value::Null);
}

#[test]
fn update_season_replaces_fields_and_logs_complete_images() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season =
        repo::insert_season(&mut conn, new_season(&farm.id, 2025, "2025 (typo)"), None).unwrap();

    let updated = repo::update_season(
        &mut conn,
        &season.id,
        UpdateSeason {
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();

    // The typed name is gone, so the dates name the book again.
    assert_eq!(updated.label, "2025/2026");
    assert_eq!(updated.custom_label, None);
    assert_eq!(updated.starts_on, "2025-09-01");
    // Untouched by the update: archiving is a separate lifecycle action.
    assert_eq!(updated.status, "active");

    let (op, before, after) = last_change(&conn, "season", &season.id);
    assert_eq!(op, "update");
    assert_eq!(before["label"], "2025 (typo)");
    assert_eq!(before["custom_label"], "2025 (typo)");
    assert_eq!(after["label"], "2025/2026");
    assert_eq!(after["custom_label"], Value::Null);
    assert_eq!(after["ends_on"], "2026-08-31");
}

#[test]
fn update_season_rejects_reversed_dates_and_an_unknown_id() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();

    let reversed = repo::update_season(
        &mut conn,
        &season.id,
        UpdateSeason {
            starts_on: "2026-08-31".into(),
            ends_on: "2025-09-01".into(),
            custom_label: None,
        },
        None,
    );
    assert!(matches!(
        reversed,
        Err(CoreError::Invalid("invalid_date_interval"))
    ));

    let missing = repo::update_season(
        &mut conn,
        "no-such-season",
        UpdateSeason {
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    );
    assert!(matches!(missing, Err(CoreError::NotFound)));
}

/// The name is derived again on every correction: fixing a date renames a book
/// its dates name, and leaves alone a book the farmer named.
#[test]
fn correcting_the_dates_renames_only_a_book_the_dates_name() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let dated =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let named =
        repo::insert_season(&mut conn, new_season(&farm.id, 2027, "Olivar 2027"), None).unwrap();

    let calendar = |custom: Option<&str>| UpdateSeason {
        starts_on: "2026-01-01".into(),
        ends_on: "2026-12-31".into(),
        custom_label: custom.map(str::to_string),
    };
    let renamed = repo::update_season(&mut conn, &dated.id, calendar(None), None).unwrap();
    assert_eq!(renamed.label, "2026");

    let kept = repo::update_season(
        &mut conn,
        &named.id,
        UpdateSeason {
            starts_on: "2026-10-01".into(),
            ends_on: "2027-09-30".into(),
            custom_label: Some("Olivar 2027".into()),
        },
        None,
    )
    .unwrap();
    assert_eq!(kept.label, "Olivar 2027");
}

#[test]
fn deleting_an_empty_book_hides_it_and_logs_both_images() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let keep = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
    let mistake = repo::insert_season(
        &mut conn,
        new_season(&farm.id, 2027, "2027 (mistake)"),
        None,
    )
    .unwrap();

    repo::delete_book(&mut conn, &mistake.id, &[], None).unwrap();

    let ids: Vec<String> = repo::list_seasons(&conn, 100, 0)
        .unwrap()
        .seasons
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
        repo::delete_book(&mut conn, &mistake.id, &[], None),
        Err(CoreError::NotFound)
    ));
}

/// What a screen picking one of a farm's books offers: that farm's live books,
/// the latest ending first — never another farm's, never a removed one, and
/// none at all for a removed farm.
#[test]
fn a_farm_lists_its_own_live_books_latest_first() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Los Llanos"), None).unwrap();
    let neighbour = repo::insert_farm(&mut conn, new_farm("El Soto"), None).unwrap();
    let older =
        repo::insert_season(&mut conn, new_season(&farm.id, 2025, "2024/2025"), None).unwrap();
    let newer =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let removed =
        repo::insert_season(&mut conn, new_season(&farm.id, 2027, "2026/2027"), None).unwrap();
    repo::delete_book(&mut conn, &removed.id, &[], None).unwrap();
    let theirs = repo::insert_season(
        &mut conn,
        new_season(&neighbour.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();

    let ids = |conn: &rusqlite::Connection, farm_id: &str| -> Vec<String> {
        repo::list_farm_seasons(conn, farm_id)
            .unwrap()
            .into_iter()
            .map(|season| season.id)
            .collect()
    };
    assert_eq!(ids(&conn, &farm.id), vec![newer.id, older.id]);
    assert_eq!(ids(&conn, &neighbour.id), vec![theirs.id]);

    // The book itself is still live: the farm going is what takes it out.
    repo::soft_delete_farm(&mut conn, &neighbour.id, None).unwrap();
    assert!(ids(&conn, &neighbour.id).is_empty());
}

/// A book is deleted with what is in it: every record-book view is
/// season-scoped, so a book removed around a live crop would hide the crop
/// (docs/sync.md → Deleting a book with its records).
#[test]
fn deleting_a_book_takes_its_crops_with_it() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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

    assert_eq!(repo::count_book_records(&conn, &season.id).unwrap(), 1);

    assert_eq!(
        repo::delete_book(&mut conn, &season.id, &[], None).unwrap(),
        1,
        "the crop went with its book"
    );
    assert!(
        repo::list_seasons(&conn, 100, 0)
            .unwrap()
            .seasons
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], Value::Null);
    assert!(
        after["deleted_at"].is_string(),
        "removed, as its own delete would"
    );
    assert_eq!(after["species_name"], "cebada", "complete after-image");
}

#[test]
fn insert_crop_ties_plot_to_season_and_logs_full_image() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2026".into()),
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();
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
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2026".into()),
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

/// Complements module-phytosanitary's with-extension test (which asserts core row and
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
fn list_seasons_orders_the_latest_ending_first() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2025, "2024/2025"), None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2027, "2026/2027"), None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();

    let labels: Vec<String> = repo::list_seasons(&conn, 100, 0)
        .unwrap()
        .seasons
        .into_iter()
        .map(|s| s.label)
        .collect();
    assert_eq!(labels, vec!["2026/2027", "2025/2026", "2024/2025"]);
}

/// The record book list pages: latest ending first, then farm name, and every
/// page reports the total a page control draws its pages from.
#[test]
fn list_seasons_pages_newest_campaign_then_farm() {
    let mut conn = db();
    let vega = repo::insert_farm(&mut conn, new_farm("La Vega"), None).unwrap();
    let alamo = repo::insert_farm(&mut conn, new_farm("Alamo"), None).unwrap();
    for year in [2024, 2025, 2026] {
        for farm in [&vega, &alamo] {
            repo::insert_season(
                &mut conn,
                new_season(&farm.id, year, &year.to_string()),
                None,
            )
            .unwrap();
        }
    }
    let names = |page: &SeasonPage| -> Vec<(i64, String)> {
        page.seasons
            .iter()
            .map(|s| {
                let farm = if s.farm_id == vega.id {
                    "La Vega"
                } else {
                    "Alamo"
                };
                (s.ends_on[..4].parse().unwrap(), farm.to_string())
            })
            .collect()
    };

    let first = repo::list_seasons(&conn, 4, 0).unwrap();
    assert_eq!(first.total, 6);
    assert_eq!(
        names(&first),
        vec![
            (2026, "Alamo".to_string()),
            (2026, "La Vega".to_string()),
            (2025, "Alamo".to_string()),
            (2025, "La Vega".to_string()),
        ]
    );
    let second = repo::list_seasons(&conn, 4, 4).unwrap();
    assert_eq!(second.total, 6, "every page carries the whole total");
    assert_eq!(
        names(&second),
        vec![(2024, "Alamo".to_string()), (2024, "La Vega".to_string())]
    );
    assert!(repo::list_seasons(&conn, 4, 8).unwrap().seasons.is_empty());
}

/// The page size is the caller's to choose within bounds, never unbounded: the
/// list is paged precisely because rows rendered are the cost.
#[test]
fn list_seasons_clamps_its_page_size_and_offset() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    for year in 2020..2023 {
        repo::insert_season(
            &mut conn,
            new_season(&farm.id, year, &year.to_string()),
            None,
        )
        .unwrap();
    }
    assert_eq!(
        repo::list_seasons(&conn, 0, 0).unwrap().seasons.len(),
        1,
        "at least one"
    );
    assert_eq!(
        repo::list_seasons(&conn, i64::MAX, -5)
            .unwrap()
            .seasons
            .len(),
        3,
        "a huge limit is cut to the maximum and a negative offset reads as the start"
    );
    // The view asks for 100 a page, so the ceiling must allow it — checked when
    // the test compiles, being a fact about two constants.
    const _: () = assert!(
        repo::SEASON_PAGE_MAX >= 100,
        "a page of 100 must be possible"
    );
}

/// A name typed as spaces is no name: the dates name the book, rather than the
/// save being refused over a field the farmer may leave blank.
#[test]
fn a_blank_name_lets_the_dates_name_the_book() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "   "), None).unwrap();
    assert_eq!(season.label, "2025/2026");
    assert_eq!(season.custom_label, None);
}

#[test]
fn a_season_ending_before_it_starts_is_refused() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let result = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: farm.id.clone(),
            starts_on: "2026-08-31".into(),
            ends_on: "2025-09-01".into(),
            custom_label: None,
        },
        None,
    );
    assert!(matches!(
        result,
        Err(CoreError::Invalid("invalid_date_interval"))
    ));
}

// ---------------------------------------------------------------------------
// A season is one holding's campaign, and so one record book (2026-09-16)
// ---------------------------------------------------------------------------

/// Books are told apart by name: a holding may keep several campaigns in one
/// year, but never two books a list, a printed cover or an export file name
/// could not distinguish.
#[test]
fn a_farm_may_keep_two_campaigns_in_one_year_under_different_names() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let spring = |custom: Option<&str>| NewSeason {
        farm_id: farm.id.clone(),
        starts_on: "2026-02-01".into(),
        ends_on: "2026-06-30".into(),
        custom_label: custom.map(str::to_string),
    };
    let autumn = |custom: Option<&str>| NewSeason {
        farm_id: farm.id.clone(),
        starts_on: "2026-07-01".into(),
        ends_on: "2026-11-30".into(),
        custom_label: custom.map(str::to_string),
    };
    let first = repo::insert_season(&mut conn, spring(None), None).unwrap();
    assert_eq!(first.label, "2026");

    // The dates alone would name the second one "2026" too.
    assert!(matches!(
        repo::insert_season(&mut conn, autumn(None), None),
        Err(CoreError::Invalid("season_name_taken"))
    ));
    assert_eq!(
        repo::list_seasons(&conn, 100, 0).unwrap().total,
        1,
        "nothing written"
    );

    let second = repo::insert_season(&mut conn, autumn(Some("2026 otoño")), None).unwrap();
    assert_eq!(second.label, "2026 otoño");
    assert_eq!(repo::list_seasons(&conn, 100, 0).unwrap().total, 2);
}

#[test]
fn another_farm_may_keep_a_book_of_the_same_name() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("Finca A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("Finca B"), None).unwrap();
    let a =
        repo::insert_season(&mut conn, new_season(&farm_a.id, 2026, "2025/2026"), None).unwrap();
    let b =
        repo::insert_season(&mut conn, new_season(&farm_b.id, 2026, "2025/2026"), None).unwrap();

    assert_ne!(a.id, b.id, "one row per farm, not one shared campaign row");
    assert_eq!(
        (a.label.as_str(), b.label.as_str()),
        ("2025/2026", "2025/2026")
    );
    assert_eq!(b.farm_id, farm_b.id);
}

#[test]
fn correcting_a_book_into_a_name_the_farm_already_uses_is_refused() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let later =
        repo::insert_season(&mut conn, new_season(&farm.id, 2027, "2026/2027"), None).unwrap();

    // Moving its dates back a year would name it "2025/2026" too.
    assert!(matches!(
        repo::update_season(
            &mut conn,
            &later.id,
            UpdateSeason {
                starts_on: "2025-09-01".into(),
                ends_on: "2026-08-31".into(),
                custom_label: None,
            },
            None,
        ),
        Err(CoreError::Invalid("season_name_taken"))
    ));

    // Keeping its own name is not a collision with itself.
    let corrected = repo::update_season(
        &mut conn,
        &later.id,
        UpdateSeason {
            starts_on: "2026-10-01".into(),
            ends_on: "2027-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(corrected.label, "2026/2027");
}

#[test]
fn a_deleted_book_frees_its_name() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let mistake =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    repo::delete_book(&mut conn, &mistake.id, &[], None).unwrap();

    let again = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None);
    assert!(again.is_ok(), "the name rule counts live books only");
}

// --- one campaign, one id (docs/sync.md → Seasons created on two devices) ---

#[test]
fn two_devices_creating_one_campaign_create_one_book() {
    // The id is derived from the farm and the dates, so the two devices never
    // had to agree on anything: the same campaign IS the same row.
    let farm = "0192f3a4-0000-7000-8000-00000000000f";
    assert_eq!(
        repo::season_id(farm, "2026-09-01", "2027-08-31"),
        repo::season_id(farm, "2026-09-01", "2027-08-31")
    );
}

#[test]
fn recreating_a_deleted_book_revives_it_rather_than_making_a_second() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let mistake =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    repo::delete_book(&mut conn, &mistake.id, &[], None).unwrap();

    let again =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "Campaña buena"), None).unwrap();

    assert_eq!(
        again.id, mistake.id,
        "a campaign has one id on this farm forever — a second would be a book \
         no other device could recognise as the same one"
    );
    assert_eq!(again.label, "Campaña buena", "revived with the new name");
    assert!(again.deleted_at.is_none() && again.status == "active");
    assert_eq!(repo::get_season(&conn, &mistake.id).unwrap().id, mistake.id);

    let books: i64 = conn
        .query_row("SELECT COUNT(*) FROM season", [], |r| r.get(0))
        .unwrap();
    assert_eq!(books, 1, "revived in place, not inserted beside itself");
}

#[test]
fn a_revival_continues_the_register_rather_than_starting_one() {
    // It is logged as an update, so the register's version vector carries on
    // from where the deletion left it — which is what lets another device merge
    // the revival instead of seeing an unrelated book appear.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season =
        repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    repo::delete_book(&mut conn, &season.id, &[], None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();

    let operations: Vec<String> = conn
        .prepare(
            "SELECT operation FROM record_change
             WHERE entity_table = 'season' AND entity_id = ?1 ORDER BY id",
        )
        .unwrap()
        .query_map([&season.id], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(operations, ["insert", "delete", "update"]);
}

#[test]
fn a_second_live_book_for_the_same_dates_is_refused_legibly() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();

    // A different NAME, so the older rule would have allowed it; the dates are
    // what make it the same campaign.
    let twin = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "Otro nombre"), None);
    assert!(matches!(
        twin,
        Err(CoreError::Invalid("season_dates_taken"))
    ));
}

#[test]
fn get_season_opens_a_live_book_and_nothing_else() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();

    let opened = repo::get_season(&conn, &season.id).unwrap();
    assert_eq!(opened.id, season.id);
    assert_eq!(opened.farm_id, farm.id, "the book says whose it is");

    assert!(matches!(
        repo::get_season(&conn, "no-such-season"),
        Err(CoreError::NotFound)
    ));
    repo::delete_book(&mut conn, &season.id, &[], None).unwrap();
    assert!(matches!(
        repo::get_season(&conn, &season.id),
        Err(CoreError::NotFound)
    ));
}

/// A deleted farm's books leave the list and stop opening, as the farm leaves
/// every farm picker — the same population in both places.
#[test]
fn a_deleted_farms_books_leave_the_list() {
    let mut conn = db();
    let kept = repo::insert_farm(&mut conn, new_farm("Finca A"), None).unwrap();
    let gone = repo::insert_farm(&mut conn, new_farm("Finca B"), None).unwrap();
    let kept_book =
        repo::insert_season(&mut conn, new_season(&kept.id, 2026, "2026"), None).unwrap();
    let gone_book =
        repo::insert_season(&mut conn, new_season(&gone.id, 2026, "2026"), None).unwrap();

    repo::soft_delete_farm(&mut conn, &gone.id, None).unwrap();

    let ids: Vec<String> = repo::list_seasons(&conn, 100, 0)
        .unwrap()
        .seasons
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, vec![kept_book.id]);
    assert!(matches!(
        repo::get_season(&conn, &gone_book.id),
        Err(CoreError::NotFound)
    ));
}

/// The crop is the one season-scoped table the schema cannot hold to its farm:
/// it reaches the farm through its plot, so `insert_crop` checks. A crop on
/// farm B filed under farm A's season would be listed in neither book.
#[test]
fn insert_crop_refuses_a_season_of_another_farm() {
    let mut conn = db();
    let farm_a = repo::insert_farm(&mut conn, new_farm("Finca A"), None).unwrap();
    let farm_b = repo::insert_farm(&mut conn, new_farm("Finca B"), None).unwrap();
    let plot_b = repo::insert_plot(&mut conn, new_plot(&farm_b.id, "B1"), None).unwrap();
    let season_a =
        repo::insert_season(&mut conn, new_season(&farm_a.id, 2026, "2026"), None).unwrap();

    assert!(matches!(
        repo::insert_crop(&mut conn, base_crop(&plot_b.id, &season_a.id), None),
        Err(CoreError::Invalid("season_not_on_farm"))
    ));
    let crops: i64 = conn
        .query_row("SELECT COUNT(*) FROM crop", [], |r| r.get(0))
        .unwrap();
    assert_eq!(crops, 0, "refused before anything was written");
}

#[test]
fn crop_validation_rejects_blank_species() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(&farm.id, 2026, "2026"), None).unwrap();

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
    let season_1 =
        repo::insert_season(&mut conn, new_season(&farm_a.id, 2026, "2026"), None).unwrap();
    let season_2 =
        repo::insert_season(&mut conn, new_season(&farm_a.id, 2027, "2027"), None).unwrap();
    // Farm B's own 2026: a season belongs to one farm, so B's crops cannot be
    // filed under season 1.
    let season_b =
        repo::insert_season(&mut conn, new_season(&farm_b.id, 2026, "2026"), None).unwrap();

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
    // Same farm, other season; other farm, same campaign year:
    repo::insert_crop(&mut conn, crop(&plot_a.id, &season_2.id, "cebada"), None).unwrap();
    repo::insert_crop(&mut conn, crop(&plot_b.id, &season_b.id, "girasol"), None).unwrap();

    let listed = repo::list_crops(&conn, &season_1.id, &farm_a.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, wheat.id);
    assert_eq!(listed[0].species_name, "trigo");

    // The farm half of the filter still holds on its own: farm A asked about
    // farm B's season sees none of B's crops.
    assert!(
        repo::list_crops(&conn, &season_b.id, &farm_a.id)
            .unwrap()
            .is_empty()
    );
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

// ---------------------------------------------------------------------------
// Dates are refused on the way in: the alert rules read them later, and a date
// nobody checked at the form would only be found on the Status view, days on.
// ---------------------------------------------------------------------------

fn log_rows(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap()
}

fn refused_date(result: Result<impl std::fmt::Debug, CoreError>, date: &str) {
    match result {
        Err(CoreError::InvalidDate(got)) => assert_eq!(got, date),
        other => panic!("expected InvalidDate({date}), got {other:?}"),
    }
}

#[test]
fn an_operator_whose_licence_date_cannot_be_read_is_refused_and_nothing_is_written() {
    let mut conn = db();
    let before = log_rows(&conn);
    let mut new = plain_operator("Ana López");
    new.licence_expiry_date = Some("15/08/2026".into());
    refused_date(repo::insert_operator(&mut conn, new, None), "15/08/2026");
    assert_eq!(log_rows(&conn), before, "a refused insert logs nothing");
    assert!(repo::list_operators(&conn).unwrap().is_empty());

    let operator = repo::insert_operator(&mut conn, plain_operator("Ana López"), None).unwrap();
    let update = UpdateOperator {
        full_name: "Ana López".into(),
        tax_id: None,
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: Some("2026-8-15".into()),
    };
    refused_date(
        repo::update_operator(&mut conn, &operator.id, update, None),
        "2026-8-15",
    );
    assert_eq!(
        repo::list_operators(&conn).unwrap()[0].licence_expiry_date,
        None,
        "the stored row is untouched"
    );
}

#[test]
fn an_operator_with_no_licence_date_or_a_valid_one_is_accepted() {
    let mut conn = db();
    repo::insert_operator(&mut conn, plain_operator("Sin carné"), None).unwrap();
    let mut dated = plain_operator("Con carné");
    dated.licence_expiry_date = Some("2028-02-29".into());
    repo::insert_operator(&mut conn, dated, None).unwrap();
    assert_eq!(repo::list_operators(&conn).unwrap().len(), 2);
}

#[test]
fn a_machine_with_any_unreadable_date_is_refused_on_insert_and_update() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    // Each of the three dates on its own: a check on one would pass the others.
    let set = |machine: &mut NewMachinery, which: usize, date: &str| match which {
        0 => machine.acquired_on = Some(date.into()),
        1 => machine.last_inspection_date = Some(date.into()),
        _ => machine.next_inspection_due_date = Some(date.into()),
    };
    for which in 0..3 {
        let mut new = plain_machinery(&farm.id, "Atomizador");
        set(&mut new, which, "2026-02-30");
        refused_date(repo::insert_machinery(&mut conn, new, None), "2026-02-30");
    }
    assert!(repo::list_machinery(&conn, &farm.id).unwrap().is_empty());

    let machine =
        repo::insert_machinery(&mut conn, plain_machinery(&farm.id, "Atomizador"), None).unwrap();
    for which in 0..3 {
        let bad = |slot: usize| (which == slot).then(|| "01/07/2026".to_string());
        let update = UpdateMachinery {
            name: "Atomizador".into(),
            kind: None,
            acquired_on: bad(0),
            last_inspection_date: bad(1),
            next_inspection_due_date: bad(2),
            roma_number: None,
            reganip_number: None,
        };
        refused_date(
            repo::update_machinery(&mut conn, &machine.id, update, None),
            "01/07/2026",
        );
    }
}
