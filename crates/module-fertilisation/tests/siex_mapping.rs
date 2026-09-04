// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Contract tests binding this module's neutral-code → SIEX-code maps
//! (`module_fertilisation::siex`) to the vendored FEGA catalogue snapshot.
//!
//! Two directions, for the reason module-cue's equivalent states: every mapped
//! code must exist and be active in its catalogue, AND every active catalogue
//! code must be the image of some lookup row — so a snapshot refresh that adds
//! a code fails the suite instead of silently under-offering choices in the
//! form. Both lists here are small and closed, so both get the full treatment.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::db_with_catalogues;
use module_fertilisation::models::Lookup;
use module_fertilisation::repository as repo;
use module_fertilisation::siex;
use rusqlite::Connection;
use std::collections::HashSet;
use terrazgo_core::catalogue::active_codes;

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
fn irrigation_method_map_matches_the_vendored_catalogue() {
    // The eight values of model section 8's own footnote.
    let conn = db_with_catalogues();
    let lookups = repo::list_irrigation_methods(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "SIST_RIEGO",
        siex::irrigation_method_to_siex,
    );
}

#[test]
fn water_origin_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_water_origins(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "ORIGEN_AGUA_RIEGO",
        siex::water_origin_to_siex,
    );
}

#[test]
fn every_irrigation_volume_unit_maps_to_an_active_siex_unit() {
    // One direction only: UNIDADES_MEDIDA carries 80+ units (€/ha, trampas…)
    // this module will never offer — only our own rows must map cleanly.
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "UNIDADES_MEDIDA")
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    for unit in repo::list_irrigation_volume_units(&conn).unwrap() {
        let siex_code = siex::volume_unit_to_siex(&unit.code)
            .unwrap_or_else(|| panic!("unit '{}' has no SIEX mapping", unit.code));
        assert!(
            active.contains(&siex_code.to_string()),
            "unit '{}' maps to SIEX {siex_code}, absent/retired in UNIDADES_MEDIDA",
            unit.code
        );
    }
}

#[test]
fn fertilisation_type_map_matches_the_vendored_catalogue() {
    // Anexo III C.c's three, and only three: fertirrigación is NOT among them
    // (the printed model's footnote merges two legal fields into one letter).
    let conn = db_with_catalogues();
    let lookups = repo::list_fertilisation_types(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "TIPO_FERITILIZACION",
        siex::fertilisation_type_to_siex,
    );
}

#[test]
fn application_method_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    // The lookup carries a third column, so it is not a plain `Lookup` — the
    // bijection check cares only about the codes.
    let lookups: Vec<Lookup> = repo::list_application_methods(&conn)
        .unwrap()
        .into_iter()
        .map(|row| Lookup {
            code: row.code,
            i18n_key: row.i18n_key,
        })
        .collect();
    assert_bijective(
        &conn,
        &lookups,
        "METODO_APLICACION_FERTILIZANTE",
        siex::application_method_to_siex,
    );
}

#[test]
fn manure_treatment_map_matches_the_vendored_catalogue() {
    let conn = db_with_catalogues();
    let lookups = repo::list_manure_treatments(&conn).unwrap();
    assert_bijective(
        &conn,
        &lookups,
        "TRAT_ESTIERCOLES",
        siex::manure_treatment_to_siex,
    );
}

#[test]
fn fertirrigation_is_a_method_not_a_type() {
    // The model prints "(F) fertirrigación" beside "(AF) abonado de fondo" and
    // "(AC) abonado de cobertera" as if the three were one list. They are not:
    // C.c asks WHAT kind of fertilisation, C.f asks HOW it was applied, and a
    // farmer can fertigate a cobertera. The two lookups keep them apart, and
    // the book derives the model's single letter at print time.
    let conn = db_with_catalogues();
    let types: HashSet<String> = repo::list_fertilisation_types(&conn)
        .unwrap()
        .into_iter()
        .map(|l| l.code)
        .collect();
    assert_eq!(types.len(), 3);
    assert!(
        !types.iter().any(|code| code.contains("fertigation")),
        "fertirrigación is a forma de aplicación (C.f), never a tipo (C.c)"
    );

    let fertigation: Vec<String> = repo::list_application_methods(&conn)
        .unwrap()
        .into_iter()
        .map(|l| l.code)
        .filter(|code| code.starts_with("fertigation_"))
        .collect();
    assert_eq!(
        fertigation,
        ["fertigation_sprinkler", "fertigation_localised"],
        "C.f asks fertigation to be specified as sprinkler or localised"
    );
}

#[test]
fn every_fertiliser_dose_unit_maps_to_an_active_siex_unit() {
    // One direction only, for the reason the irrigation volume test states.
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "UNIDADES_MEDIDA")
        .unwrap()
        .into_iter()
        .map(|c| c.code)
        .collect();
    for unit in repo::list_fertiliser_dose_units(&conn).unwrap() {
        let siex_code = siex::dose_unit_to_siex(&unit.code)
            .unwrap_or_else(|| panic!("unit '{}' has no SIEX mapping", unit.code));
        assert!(
            active.contains(&siex_code.to_string()),
            "unit '{}' maps to SIEX {siex_code}, absent/retired in UNIDADES_MEDIDA",
            unit.code
        );
    }
    // The density column carries no unit of its own, so the constant is the
    // only thing that can state one: it must still name a real unit.
    assert!(active.contains(&siex::DENSITY_UNIT_SIEX.to_string()));
}

#[test]
fn the_nutrient_catalogues_are_three_separate_lists() {
    // The whole reason `fertiliser_material_nutrient` stores a kind beside the
    // code: the three arrays of the SIEX material block share a number space.
    assert_eq!(
        siex::nutrient_catalogue("es", "macro"),
        Some("MACRONUTRIENTES")
    );
    assert_eq!(
        siex::nutrient_catalogue("es", "micro"),
        Some("MICRONUTRIENTES")
    );
    assert_eq!(
        siex::nutrient_catalogue("es", "heavy_metal"),
        Some("METALES_PESADOS")
    );
    assert_eq!(siex::nutrient_catalogue("es", "nonsense"), None);
    assert_eq!(siex::nutrient_catalogue("fr", "macro"), None);
}

#[test]
fn the_richness_codes_the_book_prints_are_the_ones_the_catalogue_publishes() {
    // The record snapshots three of C.h's eight values, because those three are
    // what section 6's "Riqueza N/P/K" cell prints. If FEGA ever renumbered
    // MACRONUTRIENTES, a book would silently print the wrong figures — so the
    // three codes are pinned to their published labels.
    let conn = db_with_catalogues();
    let label = |code: &str| {
        active_codes(&conn, "MACRONUTRIENTES")
            .unwrap()
            .into_iter()
            .find(|row| row.code == code)
            .map(|row| row.label)
            .unwrap_or_else(|| panic!("MACRONUTRIENTES has no code {code}"))
    };
    assert_eq!(label("1"), "N total");
    assert_eq!(label("6"), "P2O5 total");
    assert_eq!(label("9"), "K2O");
}

#[test]
fn the_energy_type_catalogue_is_named_only_for_spain() {
    // A code stored verbatim needs its catalogue named, not owned — the
    // provider list is not ours and a non-Spanish record has no coding here.
    assert_eq!(siex::energy_type_catalogue("es"), Some("TIPENERGIA"));
    assert_eq!(siex::energy_type_catalogue("fr"), None);
}

#[test]
fn the_irrigation_method_list_is_not_the_plot_level_one() {
    // Two irrigation vocabularies exist on purpose, and confusing them would
    // put "rainfed" on an irrigation record. Core's four values characterise
    // the PLOT (Anexo III A.2.e); these eight describe how one watering was
    // actually done (model section 8 / SIST_RIEGO).
    let conn = db_with_catalogues();
    let per_event: HashSet<String> = repo::list_irrigation_methods(&conn)
        .unwrap()
        .into_iter()
        .map(|l| l.code)
        .collect();
    let per_plot: HashSet<String> = terrazgo_core::repository::list_irrigation_systems(&conn)
        .unwrap()
        .into_iter()
        .map(|l| l.code)
        .collect();

    assert_eq!(per_event.len(), 8);
    assert_eq!(per_plot.len(), 4);

    // The plot list contains a value that is not an irrigation system at all,
    // and the event list must never offer it.
    assert!(per_plot.contains("rainfed"));
    assert!(
        !per_event.contains("rainfed"),
        "an irrigation record can never be 'rainfed'"
    );

    // And the event list draws a distinction the plot list cannot express:
    // one 'sprinkler' crop can be watered by a fixed installation one week and
    // a mobile one the next. That is precisely why `crop.irrigation_code`
    // could never be mapped onto SIST_RIEGO — the question belongs to the
    // event, not to the plot (docs/siex-export.md → Recorded gaps).
    assert!(per_plot.contains("sprinkler"));
    assert!(!per_event.contains("sprinkler"));
    assert!(per_event.contains("sprinkler_fixed"));
    assert!(per_event.contains("sprinkler_mobile"));
}
