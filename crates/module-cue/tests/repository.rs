// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository tests (docs/architecture.md testing strategy #2): every public repository function is
//! exercised against an in-memory database, with the multi-plot and multi-country junction
//! cases covered explicitly.
//!
//! The country-derivation tests (default-from-farm, mismatch rejected, explicit-match
//! accepted) are compliance logic, written test-first from the requirement.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.
use terrazgo_core::models::UpdateMachinery;
use terrazgo_core::repository::update_machinery;

/// Common fixture: one season, one ES farm, one operator, and a product (no authorisation
/// yet — tests add the authorisations they need). Returns the ids tests build treatments from.
struct Fixture {
    season_id: String,
    farm_id: String, // country 'es'
    operator_id: String,
    product_id: String,
}

fn base_fixture(conn: &mut Connection) -> Fixture {
    let season = repo::insert_season(
        conn,
        NewSeason {
            campaign_year: 2026,
            label: "2026".into(),
            starts_on: None,
            ends_on: None,
        },
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
    )
    .unwrap()
    .id;

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            licence_number: Some("CL-12345".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: Some("2027-03-01".into()),
        },
    )
    .unwrap()
    .id;

    let product_id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: "Fungitop".into(),
            holder: Some("AgroCorp".into()),
            formulation_type_code: Some("sc".into()),
            default_phi_days: Some(21), // PHI per product label
        },
    )
    .unwrap()
    .id;

    let substance =
        repo::insert_active_substance(conn, "azoxistrobin", Some("131860-33-8")).unwrap();
    repo::add_product_active_substance(conn, &product_id, &substance.id, Some(250.0), Some("g_l"))
        .unwrap();

    Fixture {
        season_id: season.id,
        farm_id,
        operator_id,
        product_id,
    }
}

fn add_es_authorisation(conn: &mut Connection, product_id: &str) {
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.into(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: Some("authorised".into()),
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
    )
    .unwrap();
}

/// Build a single-plot treatment input. `country_code` is the optional explicit override.
fn sample_treatment(
    fx: &Fixture,
    country_code: Option<&str>,
    phi_days_used: Option<i64>,
) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        application_date: "2026-05-01".into(),
        product_id: fx.product_id.clone(),
        country_code: country_code.map(str::to_string),
        dose_value: 1.0,
        dose_unit_code: "l_ha".into(),
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "1".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        target_organism: None,
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        phi_days_used,
        notes: None,
    }
}

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
        }],
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
        }],
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
        }],
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
        }],
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
        }],
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
            sown_on: None,
        },
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
            sown_on: None,
        },
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
            },
            NewTreatmentPlot {
                plot_id: plot_b.clone(),
                crop_id: Some(crop_b),
                surface_treated_ha: 5.0,
            }, // partial
        ],
    )
    .unwrap();

    // PHI: 2026-06-10 + 21 days = 2026-07-01 (PHI per product label).
    assert_eq!(record.phi_days_used, 21);
    assert_eq!(record.phi_end_date, "2026-07-01");
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
            }],
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
        }],
    )
    .unwrap();

    // Editing the product after the fact must not alter the past legal record.
    conn.execute(
        "UPDATE product SET commercial_name = 'Renamed' WHERE id = ?1",
        [&fx.product_id],
    )
    .unwrap();

    let fetched = repo::get_treatment_record(&conn, &record.id).unwrap();
    assert_eq!(fetched.record.product_name_snapshot, "Fungitop");
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
        repo::insert_active_substance(&mut conn, "glifosato", Some("1071-83-6")).unwrap();

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
            last_inspection_date: Some("2025-11-01".into()),
            next_inspection_due_date: Some("2028-11-01".into()),
            roma_number: Some("VA-0042".into()),
            reganip_number: Some("REGANIP-0042".into()),
        },
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
    )
    .unwrap()
    .id;
    let machine = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: fx.farm_id.clone(),
            name: "Atomizador".into(),
            kind: Some("sprayer".into()),
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: Some("VA-1111".into()),
            reganip_number: Some("REGANIP-2222".into()),
        },
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
        }],
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
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: Some("VA-9999".into()),
            reganip_number: None,
        },
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
        }],
    )
    .unwrap();

    repo::soft_delete_treatment_record(&mut conn, &record.id).unwrap();

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
fn list_products_authorised_is_per_country_and_ordered_by_name() {
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
    )
    .unwrap();

    let names: Vec<String> = repo::list_products_authorised(&conn, "es")
        .unwrap()
        .into_iter()
        .map(|p| p.commercial_name)
        .collect();
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
    assert_eq!(
        unit_codes,
        vec![
            "g_ha", "g_hl", "kg_ha", "l_ha", "ml_ha", "ml_hl", "g_l", "ml_l", "pct"
        ]
    );

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
    )
    .unwrap();

    let one_plot = |plot_id: &str| {
        vec![NewTreatmentPlot {
            plot_id: plot_id.into(),
            crop_id: None,
            surface_treated_ha: 3.0,
        }]
    };
    // Two records in the fixture season (different dates), one in another season.
    let mut early = sample_treatment(&fx, None, Some(14));
    early.application_date = "2026-04-01".into();
    let early = repo::insert_treatment_record(&mut conn, early, one_plot(&plot)).unwrap();
    let late = repo::insert_treatment_record(
        &mut conn,
        sample_treatment(&fx, None, Some(14)), // application_date 2026-05-01
        one_plot(&plot),
    )
    .unwrap();
    let mut other = sample_treatment(&fx, None, Some(14));
    other.season_id = other_season.id.clone();
    let other = repo::insert_treatment_record(&mut conn, other, one_plot(&plot)).unwrap();

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
    repo::soft_delete_treatment_record(&mut conn, &late.id).unwrap();
    let after_delete = repo::list_treatment_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(after_delete.len(), 1);
    assert_eq!(after_delete[0].record.id, early.id);

    // The other season's record was never in this list.
    assert!(!ids.contains(&other.id.as_str()));
}

// --- product registry CRUD (entry UI, 2026-07-03) ----------------------------

/// The latest record_change row for an entity: (operation, before, after).
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
        |r| {
            let operation: String = r.get(0)?;
            let payload: String = r.get(1)?;
            Ok((operation, payload))
        },
    )
    .map(|(op, payload)| {
        let mut doc: serde_json::Value = serde_json::from_str(&payload).unwrap();
        (op, doc["before"].take(), doc["after"].take())
    })
    .unwrap()
}

fn sample_new_product(name: &str) -> NewProduct {
    NewProduct {
        commercial_name: name.into(),
        holder: None,
        formulation_type_code: None,
        default_phi_days: Some(14),
    }
}

fn es_authorisation_fields(number: &str) -> ProductAuthorisationFields {
    ProductAuthorisationFields {
        country_code: "es".into(),
        authorisation_number: number.into(),
        kind_code: None,
        exceptional_substance_code: None,
        status: Some("authorised".into()),
        valid_from: None,
        valid_until: None,
    }
}

#[test]
fn insert_product_with_authorisation_creates_both_rows_atomically() {
    let mut conn = open_in_memory().unwrap();

    let detail = repo::insert_product_with_authorisation(
        &mut conn,
        sample_new_product("Herbistop"),
        es_authorisation_fields("ES-25.999"),
    )
    .unwrap();
    assert_eq!(detail.product.commercial_name, "Herbistop");
    assert_eq!(detail.authorisations.len(), 1);
    assert_eq!(detail.authorisations[0].authorisation_number, "ES-25.999");
    assert!(detail.substances.is_empty());

    // Immediately visible to the treatment form's country-scoped dropdown.
    let offered = repo::list_products_authorised(&conn, "es").unwrap();
    assert!(offered.iter().any(|p| p.id == detail.product.id));

    // Both inserts logged.
    let (op, _, _) = last_change(&conn, "product", &detail.product.id);
    assert_eq!(op, "insert");
    let (op, _, after) = last_change(&conn, "product_authorisation", &detail.authorisations[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["authorisation_number"], "ES-25.999");
}

#[test]
fn insert_product_with_blank_authorisation_number_leaves_no_product_row() {
    let mut conn = open_in_memory().unwrap();

    let result = repo::insert_product_with_authorisation(
        &mut conn,
        sample_new_product("Herbistop"),
        es_authorisation_fields("   "),
    );
    assert!(matches!(
        result,
        Err(module_cue::CueError::Invalid("empty_authorisation_number"))
    ));

    // Atomicity: the product insert was rolled back with the failed authorisation.
    let products: i64 = conn
        .query_row("SELECT COUNT(*) FROM product", [], |r| r.get(0))
        .unwrap();
    assert_eq!(products, 0);
    let changes: i64 = conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();
    assert_eq!(changes, 0, "no orphan audit entries either");
}

#[test]
fn product_validation_rejects_blank_name() {
    let mut conn = open_in_memory().unwrap();
    assert!(matches!(
        repo::insert_product(&mut conn, sample_new_product("  ")),
        Err(module_cue::CueError::Invalid("empty_name"))
    ));
    assert!(matches!(
        repo::insert_active_substance(&mut conn, " ", None),
        Err(module_cue::CueError::Invalid("empty_name"))
    ));
}

#[test]
fn update_product_replaces_fields_and_logs_complete_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);

    let updated = repo::update_product(
        &mut conn,
        &fx.product_id,
        UpdateProduct {
            commercial_name: "Fungitop Plus".into(),
            holder: Some("AgroCorp".into()),
            formulation_type_code: Some("wg".into()),
            default_phi_days: Some(28),
        },
    )
    .unwrap();
    assert_eq!(updated.commercial_name, "Fungitop Plus");
    assert_eq!(updated.default_phi_days, Some(28));

    let (op, before, after) = last_change(&conn, "product", &fx.product_id);
    assert_eq!(op, "update");
    assert_eq!(before["commercial_name"], "Fungitop");
    assert_eq!(after["commercial_name"], "Fungitop Plus");
    // Complete images: untouched columns present on both sides.
    assert!(before.get("created_at").is_some());
    assert!(after.get("created_at").is_some());

    assert!(matches!(
        repo::update_product(
            &mut conn,
            &fx.product_id,
            UpdateProduct {
                commercial_name: " ".into(),
                holder: None,
                formulation_type_code: None,
                default_phi_days: None,
            },
        ),
        Err(module_cue::CueError::Invalid("empty_name"))
    ));
}

#[test]
fn soft_delete_product_hides_it_from_registry_and_treatment_dropdown() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    repo::soft_delete_product(&mut conn, &fx.product_id).unwrap();

    assert!(repo::list_product_details(&conn).unwrap().is_empty());
    assert!(
        repo::list_products_authorised(&conn, "es")
            .unwrap()
            .is_empty()
    );

    // The row survives (treatment history must keep resolving).
    let raw: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM product WHERE id = ?1",
            [&fx.product_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, 1);

    let (op, before, after) = last_change(&conn, "product", &fx.product_id);
    assert_eq!(op, "delete");
    assert!(before["deleted_at"].is_null());
    assert!(!after["deleted_at"].is_null());

    // Double delete is NotFound, like the other soft deletes.
    assert!(matches!(
        repo::soft_delete_product(&mut conn, &fx.product_id),
        Err(module_cue::CueError::NotFound)
    ));
}

#[test]
fn list_product_details_joins_substances_and_authorisations() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn); // Fungitop + azoxistrobin 250 g_l, no authorisation
    add_es_authorisation(&mut conn, &fx.product_id);

    let details = repo::list_product_details(&conn).unwrap();
    assert_eq!(details.len(), 1);
    let detail = &details[0];
    assert_eq!(detail.product.id, fx.product_id);
    assert_eq!(detail.substances.len(), 1);
    assert_eq!(detail.substances[0].name, "azoxistrobin");
    assert_eq!(
        detail.substances[0].cas_number.as_deref(),
        Some("131860-33-8")
    );
    assert_eq!(detail.substances[0].concentration_value, Some(250.0));
    assert_eq!(
        detail.substances[0].concentration_unit_code.as_deref(),
        Some("g_l")
    );
    assert_eq!(detail.authorisations.len(), 1);
    assert_eq!(detail.authorisations[0].country_code, "es");
}

#[test]
fn remove_product_active_substance_hard_deletes_and_logs_null_after() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);

    let link_id = repo::list_product_details(&conn).unwrap()[0].substances[0]
        .id
        .clone();
    repo::remove_product_active_substance(&mut conn, &link_id).unwrap();

    assert!(
        repo::list_product_details(&conn).unwrap()[0]
            .substances
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "product_active_substance", &link_id);
    assert_eq!(op, "delete");
    assert_eq!(before["product_id"], fx.product_id.as_str());
    assert!(after.is_null(), "hard delete has a null after-image");

    assert!(matches!(
        repo::remove_product_active_substance(&mut conn, &link_id),
        Err(module_cue::CueError::NotFound)
    ));
}

#[test]
fn remove_product_authorisation_withdraws_the_product_from_that_country() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    let auth_id = repo::list_product_details(&conn).unwrap()[0].authorisations[0]
        .id
        .clone();
    repo::remove_product_authorisation(&mut conn, &auth_id).unwrap();

    assert!(
        repo::list_products_authorised(&conn, "es")
            .unwrap()
            .is_empty()
    );
    // Still in the registry — only the country offering changed.
    assert_eq!(repo::list_product_details(&conn).unwrap().len(), 1);

    let (op, before, after) = last_change(&conn, "product_authorisation", &auth_id);
    assert_eq!(op, "delete");
    assert_eq!(before["authorisation_number"], "ES-25.123");
    assert!(after.is_null());
}

#[test]
fn list_active_substances_orders_by_name() {
    let mut conn = open_in_memory().unwrap();
    repo::insert_active_substance(&mut conn, "glifosato", None).unwrap();
    repo::insert_active_substance(&mut conn, "azoxistrobin", None).unwrap();

    let names: Vec<String> = repo::list_active_substances(&conn)
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["azoxistrobin", "glifosato"]);
}

#[test]
fn list_formulation_types_returns_seeded_reference_data() {
    let conn = open_in_memory().unwrap();
    let types = repo::list_formulation_types(&conn).unwrap();
    let codes: Vec<&str> = types.iter().map(|t| t.code.as_str()).collect();
    assert_eq!(codes, vec!["ec", "sc", "sl", "wg", "wp"]);
    assert!(
        types
            .iter()
            .all(|t| t.i18n_key.starts_with("formulation_type."))
    );
}

// --- PHI status per plot (map overlay; test-first) --------------------------
//
// The window rule is `[application_date, phi_end_date)` — phi_end_date is the
// first day harvest is allowed again (RD 1311/2012 "plazo de seguridad"; same
// convention as alerts::phi_window_is_active, whose tests pin the boundary
// days). These tests pin the per-plot aggregation on top of that rule.

fn add_status_plot(conn: &mut Connection, farm_id: &str, name: &str) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(2.0),
            es: None,
        },
    )
    .unwrap()
    .id
}

/// One treatment on the given plots at an explicit date/PHI; returns the record.
fn treat_on(
    conn: &mut Connection,
    fx: &Fixture,
    plot_ids: &[&str],
    application_date: &str,
    phi_days: i64,
) -> TreatmentRecord {
    let mut new = sample_treatment(fx, None, Some(phi_days));
    new.application_date = application_date.into();
    let plots = plot_ids
        .iter()
        .map(|id| NewTreatmentPlot {
            plot_id: (*id).into(),
            crop_id: None,
            surface_treated_ha: 1.0,
        })
        .collect();
    repo::insert_treatment_record(conn, new, plots).unwrap()
}

fn status_of<'a>(rows: &'a [PlotPhiStatus], plot_id: &str) -> &'a PlotPhiStatus {
    rows.iter()
        .find(|r| r.plot_id == plot_id)
        .expect("plot missing from PHI status")
}

#[test]
fn phi_status_in_window_from_the_application_day() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    // PHI 21 applied 2026-05-01 → harvest allowed from 2026-05-22.
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    for today in ["2026-05-01", "2026-05-10", "2026-05-21"] {
        let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, today).unwrap();
        let status = status_of(&rows, &plot);
        assert!(status.in_phi, "must be in PHI on {today}");
        assert_eq!(status.phi_until.as_deref(), Some("2026-05-22"));
    }
}

#[test]
fn phi_status_clear_on_the_end_date_itself() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    // phi_end_date is the first day harvest is allowed → clear, but still
    // listed: "treated and harvest allowed" is a state the map shows.
    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-22").unwrap();
    let status = status_of(&rows, &plot);
    assert!(!status.in_phi);
    assert_eq!(status.phi_until, None);
}

#[test]
fn phi_status_takes_the_latest_end_among_live_windows() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 7); // ends 2026-05-08
    treat_on(&mut conn, &fx, &[&plot], "2026-05-03", 21); // ends 2026-05-24

    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-05").unwrap();
    let status = status_of(&rows, &plot);
    assert!(status.in_phi);
    assert_eq!(status.phi_until.as_deref(), Some("2026-05-24"));

    // After the shorter window lapses the longer one still rules.
    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert_eq!(
        status_of(&rows, &plot).phi_until.as_deref(),
        Some("2026-05-24")
    );
}

#[test]
fn phi_status_ignores_windows_not_yet_started() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    // Planned/future-dated record: the window has not opened yet.
    treat_on(&mut conn, &fx, &[&plot], "2026-06-01", 21);

    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-20").unwrap();
    let status = status_of(&rows, &plot);
    assert!(!status.in_phi);
    assert_eq!(status.phi_until, None);
}

#[test]
fn phi_status_multi_plot_treatment_marks_every_treated_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let a = add_status_plot(&mut conn, &fx.farm_id, "A");
    let b = add_status_plot(&mut conn, &fx.farm_id, "B");
    treat_on(&mut conn, &fx, &[&a, &b], "2026-05-01", 21);

    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert_eq!(rows.len(), 2);
    assert!(status_of(&rows, &a).in_phi);
    assert!(status_of(&rows, &b).in_phi);
}

#[test]
fn phi_status_excludes_deleted_records_untreated_plots_and_other_farms() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let treated = add_status_plot(&mut conn, &fx.farm_id, "A");
    let _untreated = add_status_plot(&mut conn, &fx.farm_id, "B");
    let record = treat_on(&mut conn, &fx, &[&treated], "2026-05-01", 21);

    // A second farm with its own in-window treatment must not leak in.
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
    )
    .unwrap()
    .id;
    let other_plot = add_status_plot(&mut conn, &other_farm, "C");
    let mut other = sample_treatment(&fx, None, Some(21));
    other.farm_id = other_farm.clone();
    repo::insert_treatment_record(
        &mut conn,
        other,
        vec![NewTreatmentPlot {
            plot_id: other_plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
        }],
    )
    .unwrap();

    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert_eq!(
        rows.len(),
        1,
        "only the treated plot of this farm is listed"
    );
    assert_eq!(rows[0].plot_id, treated);

    // Soft-deleting the only record removes the plot from the status list —
    // deleted records carry no PHI restriction.
    repo::soft_delete_treatment_record(&mut conn, &record.id).unwrap();
    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert!(rows.is_empty());
}

#[test]
fn phi_status_spans_seasons() {
    // PHI is a physical restriction on the plot — a window opened by a record
    // filed under another campaign still binds today.
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    let old_season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2025,
            label: "2025".into(),
            starts_on: None,
            ends_on: None,
        },
    )
    .unwrap();
    let mut new = sample_treatment(&fx, None, Some(21));
    new.season_id = old_season.id;
    new.application_date = "2026-05-01".into();
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            plot_id: plot.clone(),
            crop_id: None,
            surface_treated_ha: 1.0,
        }],
    )
    .unwrap();

    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert!(status_of(&rows, &plot).in_phi);
}

#[test]
fn phi_status_excludes_deleted_plots() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot = add_status_plot(&mut conn, &fx.farm_id, "A");
    treat_on(&mut conn, &fx, &[&plot], "2026-05-01", 21);

    terrazgo_core::repository::soft_delete_plot(&mut conn, &plot).unwrap();
    let rows = repo::phi_status_for_farm(&conn, &fx.farm_id, "2026-05-10").unwrap();
    assert!(rows.is_empty(), "a deleted plot has no map presence");
}

// --- coded problems, justifications and efficacy (SIEX gap 3) ----------------
// Design in docs/siex-export.md: the coded problems ARE the reason for
// treatment (RD 1311/2012) and the SIEX export requires ≥1 problem, 1..n
// justifications and an efficacy per TratamFito (schema v3.11.4).

/// A treated plot to hang the junction tests on.
fn add_plot(conn: &mut Connection, farm_id: &str, name: &str) -> String {
    repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(2.0),
            es: None,
        },
    )
    .unwrap()
    .id
}

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
        }],
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
        }]
    };

    let mut no_problems = sample_treatment(&fx, None, Some(14));
    no_problems.problems = vec![];
    let err = repo::insert_treatment_record(&mut conn, no_problems, plots(&plot)).unwrap_err();
    assert!(matches!(err, module_cue::CueError::Invalid("no_problems")));

    let mut no_justifications = sample_treatment(&fx, None, Some(14));
    no_justifications.justifications = vec![];
    let err =
        repo::insert_treatment_record(&mut conn, no_justifications, plots(&plot)).unwrap_err();
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
        }],
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
        }]
    };

    // A code the imported catalogue doesn't know is rejected…
    let mut bogus = sample_treatment(&fx, None, Some(14));
    bogus.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "999999".into(),
    }];
    let err = repo::insert_treatment_record(&mut conn, bogus, plots(&plot)).unwrap_err();
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
    repo::insert_treatment_record(&mut conn, known, plots(&plot)).unwrap();

    // …and so does a RETIRED code: providers baja-date codes rather than
    // delete them, and a late-entered record may reference one legitimately.
    let mut retired = sample_treatment(&fx, None, Some(14));
    retired.application_date = "2026-05-02".into();
    retired.problems = vec![NewTreatmentProblem {
        reason_category_code: "disease".into(),
        problem_code: "9".into(),
    }];
    repo::insert_treatment_record(&mut conn, retired, plots(&plot)).unwrap();

    // A category whose catalogue is NOT imported cannot be checked — the code
    // is stored as given (the export's schema-validated tests are the second
    // net). In the running app every catalogue is imported at startup.
    let mut unchecked = sample_treatment(&fx, None, Some(14));
    unchecked.application_date = "2026-05-03".into();
    unchecked.problems = vec![NewTreatmentProblem {
        reason_category_code: "weed".into(),
        problem_code: "12345".into(),
    }];
    repo::insert_treatment_record(&mut conn, unchecked, plots(&plot)).unwrap();
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
        }],
    )
    .unwrap();
    assert!(record.efficacy_code.is_none());

    // Efficacy is observed after application — the one allowed edit.
    let updated = repo::set_treatment_efficacy(&mut conn, &record.id, Some("fair".into())).unwrap();
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
    repo::soft_delete_treatment_record(&mut conn, &record.id).unwrap();
    assert!(matches!(
        repo::set_treatment_efficacy(&mut conn, &record.id, Some("good".into())),
        Err(module_cue::CueError::NotFound)
    ));
}

// --- authorisation kind + exceptional substance (SIEX gap 3, TipoProducto) ---

#[test]
fn authorisation_kind_defaults_and_gates_the_exceptional_substance() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);

    // Default: a plain registration.
    let auth = repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-1".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
    )
    .unwrap();
    assert_eq!(auth.kind_code, "registered");
    assert!(auth.exceptional_substance_code.is_none());

    // 'exceptional' without its substance code is rejected: SIEX requires
    // MateriaActiva for TipoProducto 4 and the value exists only on the
    // authorisation papers — it cannot be derived later.
    let err = repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-2".into(),
            kind_code: Some("exceptional".into()),
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("missing_exceptional_substance")
    ));

    // With an imported AUTORIZACION_EXCP catalogue the code must resolve there.
    conn.execute(
        "INSERT INTO catalogue (id, source, imported_at) VALUES ('AUTORIZACION_EXCP', 'siex', '2026-07-15T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO catalogue_code (catalogue_id, code, label) VALUES ('AUTORIZACION_EXCP', '42', 'Substance X')",
        [],
    )
    .unwrap();
    let err = repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-3".into(),
            kind_code: Some("exceptional".into()),
            exceptional_substance_code: Some("999999".into()),
            status: None,
            valid_from: None,
            valid_until: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        module_cue::CueError::Invalid("unknown_substance_code")
    ));

    let auth = repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-4".into(),
            kind_code: Some("exceptional".into()),
            exceptional_substance_code: Some("42".into()),
            status: None,
            valid_from: None,
            valid_until: None,
        },
    )
    .unwrap();
    assert_eq!(auth.kind_code, "exceptional");
    assert_eq!(auth.exceptional_substance_code.as_deref(), Some("42"));

    // A substance code on a non-exceptional kind has no SIEX field to land in:
    // dropped rather than stored as dead data.
    let auth = repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: fx.product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-5".into(),
            kind_code: Some("parallel_import".into()),
            exceptional_substance_code: Some("42".into()),
            status: None,
            valid_from: None,
            valid_until: None,
        },
    )
    .unwrap();
    assert_eq!(auth.kind_code, "parallel_import");
    assert!(auth.exceptional_substance_code.is_none());
}
