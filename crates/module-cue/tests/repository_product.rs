// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The product registry and its authorisations: CRUD behind the entry UI, plus
//! the authorisation kind and exceptional-substance code that decide what
//! `TipoProducto` the export sends.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::last_change;
use common::treatment::*;
use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
// Not part of module-cue's deliberately-minimal re-export surface; the
// snapshot-freeze test needs to edit machinery after the fact.

// --- product registry CRUD (entry UI, 2026-07-03) ----------------------------

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
        None,
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
        None,
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
        repo::insert_product(&mut conn, sample_new_product("  "), None),
        Err(module_cue::CueError::Invalid("empty_name"))
    ));
    assert!(matches!(
        repo::insert_active_substance(&mut conn, " ", None, None),
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
        None,
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
            None,
        ),
        Err(module_cue::CueError::Invalid("empty_name"))
    ));
}

#[test]
fn soft_delete_product_hides_it_from_registry_and_treatment_dropdown() {
    let mut conn = open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);

    repo::soft_delete_product(&mut conn, &fx.product_id, None).unwrap();

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
        repo::soft_delete_product(&mut conn, &fx.product_id, None),
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
    repo::remove_product_active_substance(&mut conn, &link_id, None).unwrap();

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
        repo::remove_product_active_substance(&mut conn, &link_id, None),
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
    repo::remove_product_authorisation(&mut conn, &auth_id, None).unwrap();

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
fn list_active_substances_is_stable_in_insertion_order() {
    let mut conn = open_in_memory().unwrap();
    repo::insert_active_substance(&mut conn, "glifosato", None, None).unwrap();
    repo::insert_active_substance(&mut conn, "azoxistrobin", None, None).unwrap();

    // Insertion order, not alphabetical: names are collated by whoever displays
    // them, because SQLite would sort them with BINARY collation and file every
    // accented name last. UUIDv7 ids make `ORDER BY id` insertion-ordered, so
    // the result is deterministic without implying an alphabet.
    let names: Vec<String> = repo::list_active_substances(&conn)
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["glifosato", "azoxistrobin"]);
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
    )
    .unwrap();
    assert_eq!(auth.kind_code, "parallel_import");
    assert!(auth.exceptional_substance_code.is_none());
}
