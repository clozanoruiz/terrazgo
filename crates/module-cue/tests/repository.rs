// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository tests for the treatment record itself (docs/architecture.md
//! testing strategy #2): every public function against an in-memory database,
//! with the multi-plot and multi-country junction cases covered explicitly.
//!
//! The country-derivation tests (default-from-farm, mismatch rejected,
//! explicit-match accepted) are compliance logic, written test-first from the
//! requirement. Also here: the audit payload contract, soft delete, and the
//! list functions the treatment entry UI reads.
//!
//! The rest is split into the sibling `repository_*.rs` files — the product
//! registry, the coded fields the decrees add, the PHI status the map reads,
//! and corrections.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::treatment::*;
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.
use terrazgo_core::models::UpdateMachinery;
use terrazgo_core::repository::update_machinery;

// --- country derivation / validation (compliance logic, test-first) --------

#[test]
fn country_defaults_from_the_farm() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    // Caller supplies no country_code — it must be derived from the ES farm.
    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.country_code, "es");
    assert_eq!(record.farm_id, fx.farm_id);
    assert_eq!(
        record.authorisation_number_snapshot.as_deref(),
        Some("ES-25.123")
    );
}

#[test]
fn explicit_country_mismatching_the_farm_is_rejected() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    // Farm is ES; caller wrongly claims FR → typed error, no silent acceptance.
    let err = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, Some("fr"), Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap_err();

    match err {
        module_cue::CueError::CountryMismatch { provided, farm } => {
            assert_eq!(provided, "fr");
            assert_eq!(farm, "es");
        }
        other => panic!("expected CountryMismatch, got {other:?}"),
    }
}

#[test]
fn explicit_country_matching_the_farm_is_accepted() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, Some("es"), Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    assert_eq!(record.country_code, "es");
}

#[test]
fn country_with_no_authorisation_is_still_rejected() {
    // Requirement 3 stands: the derived country must have an authorisation for the product.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn); // ES farm, but NO authorisation added
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let err = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        module_cue::CueError::AuthorisationMissing { .. }
    ));
}

#[test]
fn plot_on_a_different_farm_is_rejected() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    // A second farm with its own plot.
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
    let foreign_plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: other_farm,
            name: "X".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let err = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: foreign_plot,
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap_err();

    assert!(matches!(err, module_cue::CueError::PlotNotOnFarm { .. }));
}

// --- multi-plot junction ----------------------------------------------------

#[test]
fn treatment_applies_to_multiple_plots_in_one_entry() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    // Two plots on the same farm, each with its own crop this season.
    let plot_a = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela 1".into(),
            area_ha: Some(4.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot_b = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela 2".into(),
            area_ha: Some(6.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let crop_a = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot_a.clone(),
            season_id: fx.season_id.clone(),
            species_name: "wheat".into(),
            variety: Some("Marius".into()),
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
    .unwrap()
    .id;
    let crop_b = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot_b.clone(),
            season_id: fx.season_id.clone(),
            species_name: "barley".into(),
            variety: None,
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
    .unwrap()
    .id;

    // One treatment entry, two plots, different surface treated per plot, country derived.
    let mut input = sample_treatment(&fx, None, None); // phi None → fall back to product default (21)
    input.application_date = "2026-06-10".into();
    input.target_organism = Some("septoria".into());

    let record = repo::insert_treatment_record(
        &mut conn,
        input,
        vec![
            NewTreatmentPlot {
                plot_id: plot_a.clone(),
                crop_id: Some(crop_a),
                surface_treated_ha: 4.0,
                growth_stage_code: None,
            },
            NewTreatmentPlot {
                plot_id: plot_b.clone(),
                crop_id: Some(crop_b),
                surface_treated_ha: 5.0,
                growth_stage_code: None,
            }, // partial
        ],
        None,
    )
    .unwrap();

    // PHI: 2026-06-10 + 21 days = 2026-07-01 (PHI per product label).
    assert_eq!(record.phi_days_used, Some(21));
    assert_eq!(record.phi_end_date.as_deref(), Some("2026-07-01"));
    assert_eq!(
        record.active_substances_snapshot.as_deref(),
        Some("azoxistrobin 250 g_l")
    );

    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(fetched.plots.len(), 2);

    let surfaces: Vec<f64> = fetched.plots.iter().map(|p| p.surface_treated_ha).collect();
    assert!(surfaces.contains(&4.0) && surfaces.contains(&5.0));

    let wheat = fetched.plots.iter().find(|p| p.plot_id == plot_a).unwrap();
    assert_eq!(wheat.crop_name_snapshot.as_deref(), Some("wheat"));
    assert_eq!(wheat.variety_snapshot.as_deref(), Some("Marius"));

    let logged: i64 = conn
        .query_row(
            "SELECT count(*) FROM record_change WHERE entity_table IN ('treatment_record','treatment_plot')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 3);
}

// --- multi-country authorisation (now via per-farm country) -----------------

#[test]
fn product_authorisation_number_is_per_country() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);

    // Same product, different authorisation number per country.
    repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();
    repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "fr".into(),
            authorisation_number: "FR-2000999".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    // Farms in different countries; the record's country follows the farm.
    let farm_fr = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Ferme".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "fr".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let farm_it = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Azienda".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "it".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let plot_es = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P-ES".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot_fr = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm_fr.clone(),
            name: "P-FR".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot_it = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: farm_it.clone(),
            name: "P-IT".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let make = |conn: &mut Connection, farm_id: &str, plot_id: &str| {
        let mut input = sample_treatment(&fx, None, Some(14));
        input.farm_id = farm_id.to_string();
        repo::insert_treatment_record(
            conn,
            input,
            vec![NewTreatmentPlot {
                plot_id: plot_id.to_string(),
                crop_id: None,
                surface_treated_ha: 3.0,
                growth_stage_code: None,
            }],
            None,
        )
    };

    let es = make(&mut conn, &fx.farm_id, &plot_es).unwrap();
    assert_eq!(es.country_code, "es");
    assert_eq!(
        es.authorisation_number_snapshot.as_deref(),
        Some("ES-25.123")
    );

    let fr = make(&mut conn, &farm_fr, &plot_fr).unwrap();
    assert_eq!(fr.country_code, "fr");
    assert_eq!(
        fr.authorisation_number_snapshot.as_deref(),
        Some("FR-2000999")
    );

    // IT farm: product has no IT authorisation → rejected.
    let err = make(&mut conn, &farm_it, &plot_it).unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::AuthorisationMissing { .. }
    ));
}

// --- immutability & soft delete ---------------------------------------------

#[test]
fn snapshots_are_immutable_when_referenced_rows_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    // Editing the product after the fact must not alter the past legal record.
    conn.execute(
        "UPDATE product SET commercial_name = 'Renamed' WHERE id = ?1",
        [&fx.product_id],
    )
    .unwrap();

    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(
        fetched.record.product_name_snapshot.as_deref(),
        Some("Fungitop")
    );
}

// --- audit log payload contract (sync delta source) --------------------------

#[test]
fn audit_payload_contains_the_full_row_image() {
    let mut conn = open_in_memory().unwrap();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();

    let payload: String = conn
        .query_row(
            "SELECT payload FROM record_change WHERE entity_table = 'farm' AND entity_id = ?1",
            [&farm.id],
            |r| r.get(0),
        )
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
    let after = &parsed["after"];

    // The after-image is the sync delta source: a receiving device must be able to
    // rebuild the row from it alone, so EVERY column must be present — including
    // ones NewFarm doesn't capture yet (they serialize as null).
    for column in [
        "id",
        "name",
        "owner_name",
        "owner_tax_id",
        "location_text",
        "latitude",
        "longitude",
        "country_code",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }
    assert_eq!(after["country_code"], "es");
    assert_eq!(
        after["created_at"],
        serde_json::Value::String(farm.created_at.clone())
    );
}

#[test]
fn product_substance_link_is_logged_under_its_own_uuid() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn); // fixture already links one substance to the product

    // The junction row has a composite natural key; record_change must address it by
    // the row's own UUID (migration 0004), not by product_id.
    let entity_id: String = conn
        .query_row(
            "SELECT entity_id FROM record_change WHERE entity_table = 'product_active_substance'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        entity_id.len(),
        36,
        "entity_id should be the junction row's UUID"
    );
    assert_ne!(
        entity_id, fx.product_id,
        "entity_id must not fall back to product_id"
    );

    let row_id: String = conn
        .query_row(
            "SELECT id FROM product_active_substance WHERE product_id = ?1",
            [&fx.product_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(entity_id, row_id);
}

#[test]
fn active_substance_is_synced_user_data_with_uuid_and_full_image() {
    let mut conn = open_in_memory().unwrap();
    let substance =
        repo::insert_active_substance(&mut conn, "glifosato", Some("1071-83-6"), None).unwrap();

    // UUIDv7 TEXT id generated in Rust — insertion-order integer ids collide across
    // devices once substances sync (they are user-insertable, not a shipped lookup).
    assert_eq!(
        substance.id.len(),
        36,
        "id should be a 36-char UUID, not a rowid"
    );

    // Complete after-image in record_change: the receiving device rebuilds the row
    // from `after` alone (payload contract).
    let payload: String = conn
        .query_row(
            "SELECT payload FROM record_change WHERE entity_table = 'active_substance' AND entity_id = ?1",
            [&substance.id],
            |r| r.get(0),
        )
        .unwrap();
    let after = &serde_json::from_str::<serde_json::Value>(&payload).unwrap()["after"];
    assert_eq!(after["id"], serde_json::Value::String(substance.id.clone()));
    assert_eq!(after["name"], "glifosato");
    assert_eq!(after["cas_number"], "1071-83-6");
}

#[test]
fn machinery_insert_logs_core_row_and_spanish_extension_separately() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);

    let machine = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: Some("2025-11-01".into()),
            next_inspection_due_date: Some("2028-11-01".into()),
            roma_number: Some("VA-0042".into()),
            reganip_number: Some("REGANIP-0042".into()),
        },
        None,
    )
    .unwrap();

    let (roma, reganip): (String, String) = conn
        .query_row(
            "SELECT roma_number, reganip_number FROM machinery_es_extension WHERE machinery_id = ?1",
            [&machine.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(roma, "VA-0042");
    assert_eq!(reganip, "REGANIP-0042");

    // Core row and extension row are both synced tables → one change entry each.
    let logged: i64 = conn
        .query_row(
            "SELECT count(*) FROM record_change
             WHERE entity_id = ?1 AND entity_table IN ('machinery', 'machinery_es_extension')",
            [&machine.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 2);
}

#[test]
fn treatment_snapshot_freezes_both_machinery_registry_numbers() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let machine = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: Some("VA-1111".into()),
            reganip_number: Some("REGANIP-2222".into()),
        },
        None,
    )
    .unwrap();

    let mut input = sample_treatment(&fx, None, Some(14));
    input.machinery_id = Some(machine.id.clone());
    let record = repo::insert_treatment_record(
        &mut conn,
        input,
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    // Both registry numbers freeze onto the record — the cuaderno prints the one
    // that applies to the equipment type (RD 1311/2012 Anexo III: equipment used).
    assert_eq!(record.machinery_roma_snapshot.as_deref(), Some("VA-1111"));
    assert_eq!(
        record.machinery_reganip_snapshot.as_deref(),
        Some("REGANIP-2222")
    );

    // Editing the machinery later must never alter the past official record.
    update_machinery(
        &mut conn,
        &machine.id,
        UpdateMachinery {
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: Some("VA-9999".into()),
            reganip_number: None,
        },
        None,
    )
    .unwrap();
    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(
        fetched.record.machinery_roma_snapshot.as_deref(),
        Some("VA-1111")
    );
    assert_eq!(
        fetched.record.machinery_reganip_snapshot.as_deref(),
        Some("REGANIP-2222")
    );
}

#[test]
fn soft_delete_keeps_the_row_and_logs_the_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Parcela".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let record = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)),
        vec![NewTreatmentPlot {
            plot_id: plot,
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
        None,
    )
    .unwrap();

    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();

    let deleted_at: Option<String> = conn
        .query_row(
            "SELECT deleted_at FROM treatment_record WHERE id = ?1",
            [&record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted_at.is_some());

    let deletes: i64 = conn
        .query_row(
            "SELECT count(*) FROM record_change WHERE entity_table = 'treatment_record' AND operation = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deletes, 1);
}

// --- list functions backing the treatment entry UI (2026-07-02) --------------

#[test]
fn list_products_authorised_is_per_country() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn); // fixture product has NO authorisation yet
    add_es_authorisation(&mut conn, &fx.product_id);

    // A second ES product that must sort before "Fungitop", and an FR-only one.
    let herbex_id = repo::insert_product(
        &mut conn,
        NewProduct {
            commercial_name: "Aclarex".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: Some(7),
        },
        None,
    )
    .unwrap()
    .id;
    add_es_authorisation(&mut conn, &herbex_id);
    let fr_only_id = repo::insert_product(
        &mut conn,
        NewProduct {
            commercial_name: "Désherbant".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: None,
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fr_only_id,
            country_code: "fr".into(),
            authorisation_number: "FR-9999".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
        None,
    )
    .unwrap();

    // The country filter is what this pins; the order is insertion order, since
    // names are collated by whoever displays them (see the active-substance
    // test above), so the set is what matters.
    let mut names: Vec<String> = repo::list_products_authorised(&conn, "es")
        .unwrap()
        .into_iter()
        .map(|p| p.commercial_name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["Aclarex", "Fungitop"]);
}

#[test]
fn a_product_with_two_authorisations_in_a_country_is_listed_once() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    // A renewal: same product, same country, a later authorisation row.
    repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123-R".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2026-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(
        repo::list_products_authorised(&conn, "es").unwrap().len(),
        1
    );
}

#[test]
fn lookup_lists_return_the_seeded_reference_data() {
    let conn = open_in_memory().unwrap();

    let unit_codes: Vec<String> = repo::list_units(&conn)
        .unwrap()
        .into_iter()
        .map(|u| u.code)
        .collect();
    // dose_rate units first (the common case on Spanish labels), then concentration.
    // Quantity units are NOT here: a dose is a rate, and "12 l" in that column
    // would read as a different statement from "12 l/ha".
    assert_eq!(
        unit_codes,
        vec![
            "g_ha", "g_hl", "kg_ha", "l_ha", "ml_ha", "ml_hl", "g_l", "ml_l", "pct"
        ]
    );

    // The amounts, on their own list: the total product used (Anexo III B.i)
    // and the tonnes / cubic metres the non-field registers will measure in.
    let quantity_codes: Vec<String> = repo::list_quantity_units(&conn)
        .unwrap()
        .into_iter()
        .map(|u| u.code)
        .collect();
    assert_eq!(quantity_codes, vec!["l", "kg", "t", "m3"]);

    let reason_codes: Vec<String> = repo::list_reason_categories(&conn)
        .unwrap()
        .into_iter()
        .map(|r| r.code)
        .collect();
    assert_eq!(
        reason_codes,
        vec!["disease", "growth_regulator", "other", "pest", "weed"]
    );
}

#[test]
fn list_treatment_records_is_per_season_and_farm_with_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "P1".into(),
            area_ha: Some(3.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let other_season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let one_plot = |plot_id: &str| {
        vec![NewTreatmentPlot {
            plot_id: plot_id.into(),
            crop_id: None,
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }]
    };
    // Two records in the fixture season (different dates), one in another season.
    let mut early = sample_treatment(&fx, None, Some(14));
    early.application_date = "2026-04-01".into();
    let early = repo::insert_treatment_record(&mut conn, early, one_plot(&plot), None).unwrap();
    let late = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)), // application_date 2026-05-01
        one_plot(&plot),
        None,
    )
    .unwrap();
    let mut other = sample_treatment(&fx, None, Some(14));
    other.season_id = other_season.id.clone();
    let other = repo::insert_treatment_record(&mut conn, other, one_plot(&plot), None).unwrap();

    let listed = repo::list_treatment_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let ids: Vec<&str> = listed.iter().map(|t| t.record.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![late.id.as_str(), early.id.as_str()],
        "newest first"
    );
    assert_eq!(listed[0].plots.len(), 1);
    assert_eq!(listed[0].plots[0].plot_id, plot);

    // Soft-deleted records disappear from the list (but stay via get for audit).
    repo::soft_delete_treatment_record(&mut conn, &late.id, None).unwrap();
    let after_delete = repo::list_treatment_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(after_delete.len(), 1);
    assert_eq!(after_delete[0].record.id, early.id);

    // The other season's record was never in this list.
    assert!(!ids.contains(&other.id.as_str()));
}
