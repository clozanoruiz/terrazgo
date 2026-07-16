// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Contract tests binding the neutral-code → SIEX-code maps (`module_cue::siex`)
//! to the vendored FEGA catalogue snapshot (source of truth: Anexo VII via the
//! BdcSixWsp API; design in docs/siex-export.md).
//!
//! Two directions, deliberately:
//!   * every mapped code must EXIST (active) in its catalogue — a typo or a
//!     provider renumbering fails here;
//!   * for the closed lists we own end-to-end (efficacy, justification,
//!     authorisation kind), every ACTIVE catalogue code must be the image of
//!     some lookup row — when FEGA adds a code (JUSTIFICACION_ACTUACION grew
//!     from 5 to 6 rows in 2025/26), the snapshot refresh fails the suite
//!     instead of silently under-offering choices in the form.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::repository as repo;
use module_cue::siex;
use rusqlite::Connection;
use std::collections::HashSet;
use terrazgo_core::catalogue::{active_codes, ensure_catalogues};

/// In-memory app database with the real vendored catalogue snapshot imported —
/// the state a running app is always in.
fn db_with_catalogues() -> Connection {
    let mut conn = module_cue::open_in_memory().unwrap();
    ensure_catalogues(&mut conn).unwrap();
    conn
}

/// Assert lookup table ↔ catalogue equivalence through a mapping function.
fn assert_bijective(
    conn: &Connection,
    lookups: &[terrazgo_core::models::Lookup],
    catalogue_id: &str,
    map: impl Fn(&str) -> Option<i64>,
) {
    let active: HashSet<String> = active_codes(conn, catalogue_id)
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    let mut images = HashSet::new();
    for lookup in lookups {
        let siex_code = map(&lookup.code)
            .unwrap_or_else(|| panic!("lookup '{}' has no SIEX mapping", lookup.code));
        assert!(
            active.contains(&siex_code.to_string()),
            "'{}' maps to SIEX {siex_code}, absent/retired in {catalogue_id}",
            lookup.code
        );
        assert!(
            images.insert(siex_code),
            "two lookups map to SIEX {siex_code}"
        );
    }
    assert_eq!(
        images.len(),
        active.len(),
        "{catalogue_id} has active codes no lookup covers — FEGA added one? \
         Add the lookup row, its i18n keys and the mapping"
    );
}

#[test]
fn efficacy_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_efficacies(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "EFICACIA_TRATAMIENTO",
        siex::efficacy_to_siex,
    );
}

#[test]
fn justification_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_justifications(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "JUSTIFICACION_ACTUACION",
        siex::justification_to_siex,
    );
}

#[test]
fn authorisation_kind_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_authorisation_kinds(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "TIPO_PRODFITO",
        siex::authorisation_kind_to_siex,
    );
}

#[test]
fn every_dose_unit_maps_to_an_active_siex_unit() {
    // One direction only: UNIDADES_MEDIDA carries 80+ units (€/ha, trampas…)
    // we will never offer — only our own unit rows must map cleanly.
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "UNIDADES_MEDIDA")
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    for unit in repo::list_units(&conn).unwrap() {
        let (siex_code, factor) = siex::unit_to_siex(&unit.code)
            .unwrap_or_else(|| panic!("unit '{}' has no SIEX mapping", unit.code));
        assert!(
            active.contains(&siex_code.to_string()),
            "unit '{}' maps to SIEX {siex_code}, absent/retired in UNIDADES_MEDIDA",
            unit.code
        );
        assert!(factor > 0.0, "conversion factors are positive exact ratios");
    }
}

#[test]
fn every_reason_category_resolves_to_an_imported_problem_catalogue() {
    let conn = db_with_catalogues();
    for category in repo::list_reason_categories(&conn).unwrap() {
        let catalogue_id = siex::problem_catalogue("es", &category.code).unwrap_or_else(|| {
            panic!(
                "reason category '{}' has no problem catalogue",
                category.code
            )
        });
        let imported: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM catalogue WHERE id = ?1)",
                [catalogue_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(imported, "{catalogue_id} is not in the vendored snapshot");
    }
    // Other countries have no coded lists (yet): nothing to validate against.
    assert!(siex::problem_catalogue("fr", "disease").is_none());
}
