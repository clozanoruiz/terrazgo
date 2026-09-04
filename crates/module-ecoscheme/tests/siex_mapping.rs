// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Contract test binding this module's neutral-code → SIEX-code map
//! (`module_ecoscheme::siex`) to the vendored FEGA catalogue snapshot.
//!
//! Two directions, and here the second one is the reason the test exists.
//! Owning `cultural_operation_kind` instead of storing `TIPO_LABOR` verbatim
//! means the day FEGA publishes a code for an operation we fold into another —
//! or splits its "Desbroce y siega" into the two words model 9.4 already prints
//! as two columns — nothing in the app would notice. So:
//!
//!   1. every one of our codes maps to a `TIPO_LABOR` row that still exists and
//!      is active, and
//!   2. **every active `TIPO_LABOR` row is claimed by at least one of ours** —
//!      a new upstream code fails this and makes somebody decide whether it
//!      deserves an owned kind.
//!
//! The watchdog direction fires at CI and at release-ritual time, against the
//! vendored file, which is the only cadence available: the in-app catalogue
//! refresh runs on a user's machine where no test runs. That is acceptable
//! because an unmapped upstream code is a **missed opportunity, not a defect**
//! — our lookup is the stored vocabulary, so nothing breaks and no picker
//! changes; we simply keep recording `mowing` where FEGA has a finer word. The
//! release ritual's catalogue-refresh step (docs/maintenance.md §6) is where it
//! surfaces.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::db_with_catalogues;
use module_ecoscheme::repository as repo;
use module_ecoscheme::siex;
use std::collections::{BTreeMap, HashSet};
use terrazgo_core::catalogue::active_codes;

#[test]
fn every_owned_kind_maps_to_an_active_catalogue_code() {
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "TIPO_LABOR")
        .unwrap()
        .into_iter()
        .map(|row| row.code)
        .collect();

    for lookup in repo::list_cultural_operation_kinds(&conn).unwrap() {
        let siex_code = siex::cultural_operation_kind_to_siex(&lookup.code)
            .unwrap_or_else(|| panic!("lookup '{}' has no SIEX mapping", lookup.code));
        assert!(
            active.contains(&siex_code.to_string()),
            "'{}' maps to TIPO_LABOR {siex_code}, absent or retired in the snapshot",
            lookup.code
        );
    }
}

#[test]
fn every_active_catalogue_code_is_claimed_by_an_owned_kind() {
    // The watchdog. FEGA last touched this file on 17/07/2026, adding codes 11,
    // 12 and 13 (eliminación de restos de poda, poda en verde, rulado) — which
    // is exactly the event this asserts we would notice next time.
    let conn = db_with_catalogues();
    let active: HashSet<String> = active_codes(&conn, "TIPO_LABOR")
        .unwrap()
        .into_iter()
        .map(|row| row.code)
        .collect();

    let claimed: HashSet<String> = repo::list_cultural_operation_kinds(&conn)
        .unwrap()
        .into_iter()
        .filter_map(|lookup| siex::cultural_operation_kind_to_siex(&lookup.code))
        .map(|code| code.to_string())
        .collect();

    let unclaimed: Vec<&String> = active.difference(&claimed).collect();
    assert!(
        unclaimed.is_empty(),
        "TIPO_LABOR has active codes no owned kind claims: {unclaimed:?}. \
         FEGA published a finer vocabulary — decide whether each deserves a \
         `cultural_operation_kind` row (code, i18n keys in all three locales, \
         a `Labels` entry and a mapping), or say why it does not."
    );
}

#[test]
fn the_map_collides_only_where_the_decision_is_pinned() {
    // Deliberately non-injective, unlike every other siex map in the workspace:
    // `TIPO_LABOR` 5 is "Desbroce y siega" while model 9.4 prints Siega and
    // Desbrozado as two columns, and art. 42.1.c asks which maintenance was
    // performed. So the collision is pinned rather than asserted away, and any
    // OTHER collision — two kinds quietly given the same code by a later edit —
    // still fails.
    let conn = db_with_catalogues();

    let mut by_image: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for lookup in repo::list_cultural_operation_kinds(&conn).unwrap() {
        let siex_code = siex::cultural_operation_kind_to_siex(&lookup.code).unwrap();
        by_image.entry(siex_code).or_default().push(lookup.code);
    }

    let pinned: Vec<HashSet<&str>> = siex::SHARED_SIEX_CODES
        .iter()
        .map(|group| group.iter().copied().collect())
        .collect();

    for (siex_code, codes) in &by_image {
        if codes.len() > 1 {
            let sharing: HashSet<&str> = codes.iter().map(String::as_str).collect();
            assert!(
                pinned.contains(&sharing),
                "TIPO_LABOR {siex_code} is claimed by {codes:?}, which SHARED_SIEX_CODES \
                 does not pin — either it is a mistake, or record the decision there"
            );
        }
    }

    // And the reverse, so a pin cannot outlive the collision it describes.
    for group in siex::SHARED_SIEX_CODES {
        let images: HashSet<Option<i64>> = group
            .iter()
            .map(|code| siex::cultural_operation_kind_to_siex(code))
            .collect();
        assert_eq!(
            images.len(),
            1,
            "{group:?} is pinned as sharing one SIEX code but no longer does"
        );
        assert!(
            !images.contains(&None),
            "{group:?} is pinned as sharing a SIEX code but one of them maps to nothing"
        );
    }
}

#[test]
fn the_owned_list_is_wider_than_the_catalogue_by_exactly_the_pinned_split() {
    // Pin the numbers so a refresh that moves either makes somebody look — the
    // EPPO-count philosophy (docs/cuaderno-print.md → "What the EU annex adds").
    let conn = db_with_catalogues();
    let kinds = repo::list_cultural_operation_kinds(&conn).unwrap().len();
    let active = active_codes(&conn, "TIPO_LABOR").unwrap().len();
    let extra: usize = siex::SHARED_SIEX_CODES
        .iter()
        .map(|group| group.len() - 1)
        .sum();

    assert_eq!(active, 14, "TIPO_LABOR publishes codes 0-13");
    assert_eq!(kinds, 15);
    assert_eq!(kinds - extra, active);
}

#[test]
fn a_country_with_no_coded_vocabulary_gets_none_rather_than_spains() {
    assert_eq!(siex::cultural_operation_catalogue("es"), Some("TIPO_LABOR"));
    assert_eq!(siex::cultural_operation_catalogue("fr"), None);
}

#[test]
fn an_unknown_kind_maps_to_nothing_rather_than_guessing() {
    assert_eq!(siex::cultural_operation_kind_to_siex("harvesting"), None);
}

// --- TIPO_COBERTURA_SUELO, the same watchdog for a catalogue we only read ----

#[test]
fn every_cover_type_belongs_to_a_practice_or_is_pinned_as_belonging_to_neither() {
    // The second watchdog, and the reason it is needed even though nothing maps
    // this catalogue: the FORM narrows its picker per practice
    // (`PLANT_COVER_TYPES` / `INERT_COVER_TYPES`), so a code FEGA adds would be
    // offered by neither cover form and would silently become unreachable from
    // the UI. Accounting for every active row makes that a decision instead.
    //
    // The catalogue does grow: value 6, "Regeneración Pastos permanentes",
    // was added on 27/02/2024.
    let conn = db_with_catalogues();

    let active: HashSet<String> = active_codes(&conn, "TIPO_COBERTURA_SUELO")
        .unwrap()
        .into_iter()
        .map(|row| row.code)
        .collect();

    let accounted: HashSet<String> = siex::PLANT_COVER_TYPES
        .iter()
        .chain(siex::INERT_COVER_TYPES)
        .chain(siex::NON_COVER_TYPES)
        .map(|code| (*code).to_string())
        .collect();

    let unaccounted: Vec<&String> = active.difference(&accounted).collect();
    assert!(
        unaccounted.is_empty(),
        "TIPO_COBERTURA_SUELO has cover types no practice claims and nothing pins \
         as belonging to neither: {unaccounted:?}. Decide which of RD 1048/2022 \
         arts. 42 and 43 each one establishes — or add it to NON_COVER_TYPES, \
         with the reason — and widen the form's picker accordingly."
    );

    let missing: Vec<&String> = accounted.difference(&active).collect();
    assert!(
        missing.is_empty(),
        "these cover types are pinned but no longer active upstream: {missing:?}"
    );
}

#[test]
fn the_two_cover_practices_claim_different_types() {
    // Art. 42.1.a establishes "la cubierta vegetal espontánea o sembrada";
    // art. 43.1.a "la cubierta inerte de restos de poda". Nothing is both, and
    // 5 ("otros materiales": cáscaras, piedras) is deliberately NOT art. 43's,
    // which is specifically triturated pruning residue.
    let plant: HashSet<&str> = siex::PLANT_COVER_TYPES.iter().copied().collect();
    let inert: HashSet<&str> = siex::INERT_COVER_TYPES.iter().copied().collect();
    let neither: HashSet<&str> = siex::NON_COVER_TYPES.iter().copied().collect();

    assert!(plant.is_disjoint(&inert));
    assert!(plant.is_disjoint(&neither));
    assert!(inert.is_disjoint(&neither));
    assert!(!plant.is_empty() && !inert.is_empty());
    assert!(
        neither.contains("5"),
        "an inert cover of nutshells or stones is not art. 43's cubierta de restos de poda"
    );
}
