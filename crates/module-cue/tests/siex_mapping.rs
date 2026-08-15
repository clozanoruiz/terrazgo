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
fn analysis_material_map_matches_the_vendored_catalogue() {
    // The catalogue this test binds to is why the lookup has four rows and not
    // the model's three: FEGA separates the standing crop (1) from the produce
    // harvested off it (2). A three-row lookup fails here.
    let conn = db_with_catalogues();
    let lookups = repo::list_analysis_materials(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "MATERIAL_ANALIZADO",
        siex::analysis_material_to_siex,
    );
}

#[test]
fn analysis_type_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_analysis_types(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "TIPO_ANALISIS",
        siex::analysis_type_to_siex,
    );
}

#[test]
fn seed_treatment_kind_map_matches_the_vendored_catalogue() {
    // TIPO_TRATAMIENTO starts at 2 — the bijection is over what the catalogue
    // actually publishes, not over 1..=n.
    let conn = db_with_catalogues();
    let lookups = repo::list_seed_treatment_kinds(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "TIPO_TRATAMIENTO",
        siex::seed_treatment_kind_to_siex,
    );
}

#[test]
fn growing_environments_map_to_the_siex_growing_system_they_name() {
    // One direction only: SIST_CULTIVO carries 33 systems (bodegas, sustratos,
    // entutorados…) our four-value column deliberately does not offer. What the
    // test must catch is a RENUMBERING — code 3 ceasing to mean an inaccessible
    // cover — so each mapping is anchored to a word of the catalogue's own
    // label, not merely to the code existing.
    let conn = db_with_catalogues();
    let anchors = [
        ("open_air", "aire libre"),
        ("mesh", "malla"),
        ("plastic_cover", "cubierta no accesible"),
        ("greenhouse", "invernadero"),
    ];
    let catalogue = active_codes(&conn, "SIST_CULTIVO").unwrap();
    for environment in terrazgo_core::repository::list_growing_environments(&conn).unwrap() {
        let siex_code = siex::growing_environment_to_siex(&environment.code)
            .unwrap_or_else(|| panic!("'{}' has no SIEX mapping", environment.code));
        let row = catalogue
            .iter()
            .find(|c| c.code == siex_code.to_string())
            .unwrap_or_else(|| panic!("SIEX {siex_code} absent/retired in SIST_CULTIVO"));
        let (_, anchor) = anchors
            .iter()
            .find(|(code, _)| *code == environment.code)
            .expect("every growing environment is anchored");
        assert!(
            row.label.to_lowercase().contains(anchor),
            "'{}' maps to SIST_CULTIVO {siex_code} = '{}', which no longer names '{anchor}'",
            environment.code,
            row.label
        );
    }
}

#[test]
fn every_irrigation_system_maps_to_an_active_exploitation_system() {
    // SIST_EXPLOTACION answers a coarser question than our column does — is the
    // holding irrigated at all — so the map is total but not injective, and
    // SIST_RIEGO is deliberately NOT its target: 'sprinkler' sits between
    // "Aspersión fija" and "Aspersión móvil" with nothing in the record to
    // choose on, and 'rainfed' has no SIST_RIEGO code at all.
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "SIST_EXPLOTACION")
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    let mut seen = HashSet::new();
    for system in terrazgo_core::repository::list_irrigation_systems(&conn).unwrap() {
        let siex_code = siex::irrigation_to_siex_exploitation(&system.code)
            .unwrap_or_else(|| panic!("irrigation '{}' has no SIEX mapping", system.code));
        assert!(
            active.contains(siex_code),
            "'{}' maps to SIST_EXPLOTACION {siex_code}, absent/retired",
            system.code
        );
        seen.insert(siex_code);
    }
    // Both halves of the catalogue are reachable: a map that answered "R" to
    // everything would still pass every assertion above.
    assert_eq!(seen, HashSet::from(["R", "S"]));
    assert_eq!(siex::irrigation_to_siex_exploitation("rainfed"), Some("S"));
    assert_eq!(siex::irrigation_to_siex_exploitation("moon"), None);
}

#[test]
fn the_coded_catalogues_the_record_book_reads_are_all_vendored() {
    // The naming functions are the only place these ids are written down; a
    // typo in one silently turns a picker into an empty list.
    let conn = db_with_catalogues();
    for catalogue_id in [
        siex::substance_catalogue("es").unwrap(),
        siex::plant_product_catalogue("es").unwrap(),
    ] {
        let imported: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM catalogue WHERE id = ?1)",
                [catalogue_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(imported, "{catalogue_id} is not in the vendored snapshot");
    }
    // Other countries code their own way; nothing is offered rather than
    // Spain's list being offered wrongly.
    assert!(siex::substance_catalogue("fr").is_none());
    assert!(siex::plant_product_catalogue("it").is_none());
}

#[test]
fn every_province_of_the_catalogue_maps_to_a_vendored_comunidad() {
    // Now that PROVINCIA and COMUNIDAD_AUTONOMA are vendored, the hand-written
    // province → CCAA table can be checked against the authority's own lists in
    // both directions instead of by spot check.
    let conn = db_with_catalogues();
    let communities: HashSet<String> = active_codes(&conn, "COMUNIDAD_AUTONOMA")
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    let mut reached = HashSet::new();
    for province in active_codes(&conn, "PROVINCIA").unwrap() {
        // "00 Provincia sin determinar" is a placeholder, not a province — the
        // same kind of row as COMUNIDAD_AUTONOMA's "Comunidad Desconocida".
        if province.code == "00" {
            assert_eq!(siex::province_to_ccaa(&province.code), None);
            continue;
        }
        let ccaa = siex::province_to_ccaa(&province.code).unwrap_or_else(|| {
            panic!(
                "province {} ({}) has no CCAA",
                province.code, province.label
            )
        });
        // Ceuta and Melilla are ciudades autónomas, and FEGA's catalogue
        // publishes comunidades only — 17 rows, no 18/19. The INE codes our map
        // returns are still the right answer to the question it asks; what has
        // no answer is `CAExplotacion` for a holding there, which is a recorded
        // export gap (docs/siex-export.md) and not something to paper over by
        // mapping the two cities onto a neighbouring comunidad.
        if matches!(province.code.as_str(), "51" | "52") {
            assert!(!communities.contains(ccaa));
            continue;
        }
        assert!(
            communities.contains(ccaa),
            "province {} maps to CCAA {ccaa}, which COMUNIDAD_AUTONOMA does not publish",
            province.code
        );
        reached.insert(ccaa.to_string());
    }
    assert_eq!(
        reached, communities,
        "a comunidad autónoma no province reaches — the catalogue gained a row?"
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
