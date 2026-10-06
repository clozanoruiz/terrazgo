// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every tier-1 lookup says whether its codes mean the same thing everywhere.
//!
//! `licence_level` and `gip_system` were national schemes wearing the shape of
//! universal ones — Spain's carné levels and `atria`, a Spanish institution,
//! offered to every farm while `fr` and `it` were already seeded. Nothing
//! distinguished them from `unit` or `premises_kind`, and the schema comment
//! that said "Spanish carné today; regional mapping is config" did not prevent
//! it, because prose cannot fail a build.
//!
//! So the question is asked mechanically, of all of them:
//!
//!   1. **the universe comes from the schema** — a tier-1 lookup is
//!      structurally identifiable (its primary key is `code` and it carries an
//!      `i18n_key`), so a lookup added next year is checked the day it exists
//!      rather than the day someone remembers this file;
//!   2. **every one is classified** `Universal`, `PerCountry` or
//!      `PendingNarrowing`, and an unclassified table fails with the question
//!      it needs answered;
//!   3. **a `PerCountry` scheme and its seeded rows agree in both directions**
//!      — the tier-1 rule's bidirectional contract test, applied to the country
//!      dimension. This is what catches a code no country offers (invisible in
//!      every picker: the original bug) and a typo in a scheme (a hole in one);
//!   4. **no country-scoped picker sits in the argument-free session store**,
//!      which makes `lookups.svelte.js`'s own contract comment enforceable
//!      rather than aspirational.
//!
//! It lives in the shell for `index_contract.rs`'s reason: `terrazgo-core` may
//! depend on no module, so the same test there would see ten of the twenty-nine
//! and be blind to `eco_practice`.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use terrazgo_core::repository::{gip_system_scheme, licence_level_scheme};
use terrazgo_lib::db::composed_migrations;

/// What a lookup's codes mean across borders.
enum Scope {
    /// One vocabulary everywhere: every seeded row is offered to every country.
    Universal,
    /// A national scheme. The function says which codes a country offers, and
    /// `None` means a picker with nothing to offer — never a fallback.
    PerCountry(fn(&str) -> Option<&'static [&'static str]>),
    /// National in fact, not narrowed yet — a declared debt, with what it waits
    /// on. Counted by [`PENDING_BUDGET`], which only ever goes down.
    PendingNarrowing(&'static str),
}

/// The classification. This IS a hand-written list, and that is fine because
/// the test checks it against the schema on every run: the lesson recorded in
/// the decision log is that a list nothing re-checks drifts, not that lists are
/// forbidden. `neutral_voice.rs` keeps its banned phrases the same way.
const SCOPES: &[(&str, Scope)] = &[
    // --- core ---------------------------------------------------------------
    ("country", Scope::Universal),
    ("production_system", Scope::Universal),
    ("unit", Scope::Universal),
    ("irrigation_system", Scope::Universal),
    ("growing_environment", Scope::Universal),
    ("premises_kind", Scope::Universal),
    ("sowing_kind", Scope::Universal),
    // Universal by EU law rather than by luck: the Nitrates Directive, the
    // plant-health regime and Natura 2000 exist in every member state. A new
    // country's zones are new rows, which is what its own comment says.
    ("zone_type", Scope::Universal),
    ("licence_level", Scope::PerCountry(licence_level_scheme)),
    ("gip_system", Scope::PerCountry(gip_system_scheme)),
    // --- phytosanitary ------------------------------------------------------
    ("reason_category", Scope::Universal),
    ("formulation_type", Scope::Universal),
    ("efficacy", Scope::Universal),
    ("justification", Scope::Universal),
    ("authorisation_kind", Scope::Universal),
    ("non_field_subject_kind", Scope::Universal),
    ("register_kind", Scope::Universal),
    ("analysis_material", Scope::Universal),
    ("analysis_type", Scope::Universal),
    (
        "seed_treatment_kind",
        Scope::PendingNarrowing(
            "`purchased_es` / `purchased_abroad` hardcode which country is \
             domestic. Splitting it needs a second country's answer about what \
             its own form asks, so it waits for one rather than being guessed.",
        ),
    ),
    // --- fertilisation ------------------------------------------------------
    ("irrigation_method", Scope::Universal),
    ("water_origin", Scope::Universal),
    ("fertilisation_type", Scope::Universal),
    ("application_method", Scope::Universal),
    ("manure_treatment", Scope::Universal),
    ("nutrient_kind", Scope::Universal),
    // --- eco-scheme ---------------------------------------------------------
    ("cultural_operation_kind", Scope::Universal),
    (
        "eco_practice",
        Scope::PendingNarrowing(
            "CAP eco-schemes are defined per member state, so these six are \
             RD 1048/2022's. Narrowing needs a SECOND dimension as well as \
             country — which register may claim a practice — and the codes are \
             on three NOT NULL foreign keys from regulatory records, so the \
             answer waits for the country that forces it.",
        ),
    ),
];

/// How many lookups may sit unresolved. **This number goes down.** A third
/// national lookup cannot be parked without resolving one of these or moving
/// the bound deliberately.
const PENDING_BUDGET: usize = 2;

/// A floor under the schema predicate, so a broken one cannot pass by finding
/// nothing. Twenty-nine today.
const MINIMUM_LOOKUPS: usize = 25;

fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

/// Every tier-1 lookup in the composed schema: primary key is a single `code`
/// column, and there is an `i18n_key` beside it.
///
/// Deliberately not "exactly two columns" — `unit` carries `dimension` and
/// `application_method` carries `is_fertigation`, and both are lookups.
fn lookup_tables(conn: &Connection) -> BTreeSet<String> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    names.into_iter().filter(|t| is_lookup(conn, t)).collect()
}

fn is_lookup(conn: &Connection, table: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT name, pk FROM pragma_table_info('{table}')"
        ))
        .unwrap();
    let cols: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    let key: Vec<&String> = cols
        .iter()
        .filter(|(_, pk)| *pk > 0)
        .map(|(n, _)| n)
        .collect();
    key.len() == 1 && key[0] == "code" && cols.iter().any(|(n, _)| n == "i18n_key")
}

fn seeded_codes(conn: &Connection, table: &str) -> BTreeSet<String> {
    let mut stmt = conn.prepare(&format!("SELECT code FROM {table}")).unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .unwrap()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn every_lookup_says_whether_its_codes_are_one_countrys_or_everyones() {
    let conn = composed_schema();
    let found = lookup_tables(&conn);
    assert!(
        found.len() >= MINIMUM_LOOKUPS,
        "the lookup predicate found only {} tables, which means it is broken \
         rather than that the schema shrank",
        found.len()
    );

    let classified: BTreeSet<String> = SCOPES.iter().map(|(t, _)| (*t).to_string()).collect();
    assert_eq!(
        classified.len(),
        SCOPES.len(),
        "a table is classified twice in SCOPES"
    );

    let undeclared: Vec<&String> = found.difference(&classified).collect();
    assert!(
        undeclared.is_empty(),
        "these tier-1 lookups are not classified: {undeclared:?}.\n\
         Say whether the codes mean the same thing in every country \
         (Scope::Universal) or belong to one country's scheme \
         (Scope::PerCountry, with a `*_scheme` function beside the reader in \
         terrazgo-core's repository/country.rs). If it is national but cannot \
         be narrowed yet, Scope::PendingNarrowing with the reason -- and note \
         PENDING_BUDGET only goes down."
    );

    let vanished: Vec<&String> = classified.difference(&found).collect();
    assert!(
        vanished.is_empty(),
        "SCOPES classifies tables that are not tier-1 lookups any more: {vanished:?}"
    );
}

#[test]
fn a_per_country_scheme_and_its_seeded_rows_agree_in_both_directions() {
    let conn = composed_schema();
    // Every country the app ships, so the union below is complete rather than
    // whatever the schemes happen to name.
    let countries = seeded_codes(&conn, "country");

    for (table, scope) in SCOPES {
        let Scope::PerCountry(scheme) = scope else {
            continue;
        };
        let seeded = seeded_codes(&conn, table);
        let mut offered: BTreeSet<String> = BTreeSet::new();

        for country in &countries {
            let Some(codes) = scheme(country) else {
                continue;
            };
            for code in codes {
                assert!(
                    seeded.contains(*code),
                    "{table}: {country} offers `{code}`, which no migration seeds"
                );
                offered.insert((*code).to_string());
            }
        }

        // The direction that catches the original bug: a code sitting in the
        // table that no country offers is invisible in every picker.
        let orphans: Vec<&String> = seeded.difference(&offered).collect();
        assert!(
            orphans.is_empty(),
            "{table}: {orphans:?} are seeded but no country's scheme offers \
             them, so nothing can ever choose them"
        );
    }
}

#[test]
fn a_scheme_only_names_countries_the_app_ships() {
    let conn = composed_schema();
    let countries = seeded_codes(&conn, "country");
    // A scheme written for a country the `country` table does not carry would
    // be silently unreachable -- "eS" or "esp" rather than "es".
    for (table, scope) in SCOPES {
        let Scope::PerCountry(scheme) = scope else {
            continue;
        };
        assert!(
            countries.iter().any(|c| scheme(c).is_some()),
            "{table} is PerCountry but no seeded country has a written scheme, \
             which would leave every picker empty"
        );
    }
}

#[test]
fn no_country_scoped_picker_sits_in_the_argument_free_session_store() {
    // `lookups.svelte.js` states the contract itself: "argument-free by
    // construction: a list that needs a country or a category is per-farm
    // reference data, not session-wide". This makes it enforceable. A command
    // taking a country cannot be cached session-wide, because the cache has no
    // key for it -- every holding would see whichever country loaded first.
    let cached = session_store_commands();
    assert!(
        cached.len() > 10,
        "the session-store reader found {} commands, so it is broken",
        cached.len()
    );

    let country_scoped = commands_taking_a_country();
    assert!(
        country_scoped.len() > 5,
        "the command reader found {} country-scoped commands, so it is broken",
        country_scoped.len()
    );

    let wrong: Vec<&String> = cached.intersection(&country_scoped).collect();
    assert!(
        wrong.is_empty(),
        "these commands take a country but are cached session-wide in \
         src/lib/lookups.svelte.js: {wrong:?}. Move them out, to the view that \
         knows the country."
    );
}

#[test]
fn the_parked_lookups_are_counted_and_the_count_only_goes_down() {
    let pending: Vec<(&str, &str)> = SCOPES
        .iter()
        .filter_map(|(t, s)| match s {
            Scope::PendingNarrowing(reason) => Some((*t, *reason)),
            _ => None,
        })
        .collect();

    assert!(
        pending.len() <= PENDING_BUDGET,
        "{} lookups are parked as national-but-not-narrowed, over a budget of \
         {PENDING_BUDGET}:\n{}\nA new one cannot be parked without resolving \
         one of these, or moving the bound on purpose.",
        pending.len(),
        pending
            .iter()
            .map(|(t, r)| format!("  - {t}: {r}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The rules above all pass by FINDING something, so a broken reader would
/// leave this file green and useless. `index_contract.rs` guards itself the
/// same way.
#[test]
fn the_lookup_predicate_can_tell_a_lookup_from_a_record() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE a_lookup (code TEXT PRIMARY KEY, i18n_key TEXT NOT NULL);
         CREATE TABLE a_lookup_with_extras (
             code TEXT PRIMARY KEY, i18n_key TEXT NOT NULL, dimension TEXT);
         CREATE TABLE no_i18n_key (code TEXT PRIMARY KEY, label TEXT NOT NULL);
         CREATE TABLE uuid_keyed (id TEXT PRIMARY KEY, i18n_key TEXT NOT NULL);
         CREATE TABLE composite_key (
             code TEXT NOT NULL, scope TEXT NOT NULL, i18n_key TEXT NOT NULL,
             PRIMARY KEY (code, scope));",
    )
    .unwrap();

    assert!(is_lookup(&conn, "a_lookup"));
    // `unit` and `application_method` carry a third column and are lookups.
    assert!(is_lookup(&conn, "a_lookup_with_extras"));
    // A code without a translation key is not a tier-1 lookup.
    assert!(!is_lookup(&conn, "no_i18n_key"));
    // User data keyed by UUID is not, whatever else it carries.
    assert!(!is_lookup(&conn, "uuid_keyed"));
    // Nor is a table whose key is more than the code.
    assert!(!is_lookup(&conn, "composite_key"));
}

// --- what a form OFFERS versus what the repository ACCEPTS ------------------

/// The eco-scheme forms narrow the practice list themselves, in JS, while the
/// repository enforces its own allow-list in Rust. Two lists of the same thing
/// in two languages, and nothing compared them.
///
/// They are deliberately not equal — `BookGrazing` offers three of the four
/// grazing practices because the fourth is reached through the cover form, and
/// `BookCulturalOperations` offers two of five for the same kind of reason. A
/// SUBSET is the contract: a form may offer fewer, never a code the repository
/// would refuse, which is a save that fails with a validation error the farmer
/// cannot act on.
#[test]
fn no_eco_scheme_form_offers_a_practice_the_repository_would_refuse() {
    let pairs = [
        (
            "grazing",
            "crates/module-ecoscheme/src/repository/grazing.rs",
            "GRAZING_PRACTICES",
            "src/lib/BookGrazing.svelte",
            "GRAZING_PRACTICES",
        ),
        (
            "cultural operations",
            "crates/module-ecoscheme/src/repository/cultural_operation.rs",
            "OPERATION_PRACTICES",
            "src/lib/BookCulturalOperations.svelte",
            "FORM_PRACTICES",
        ),
        (
            "soil covers",
            "crates/module-ecoscheme/src/repository/soil_cover.rs",
            "COVER_PRACTICES",
            "src/lib/BookSoilCovers.svelte",
            "COVER_PRACTICES",
        ),
    ];

    for (register, rust_file, rust_const, js_file, js_const) in pairs {
        let accepted = string_array(&repo_root().join(rust_file), rust_const);
        let offered = string_array(&repo_root().join(js_file), js_const);
        assert!(
            !accepted.is_empty() && !offered.is_empty(),
            "{register}: a reader found nothing, so it is broken \
             (accepted {accepted:?}, offered {offered:?})"
        );

        let refused: Vec<&String> = offered.difference(&accepted).collect();
        assert!(
            refused.is_empty(),
            "{register}: the form offers {refused:?}, which the repository \
             would reject -- the farmer would hit a validation error they \
             cannot act on. {js_file} must stay a subset of {rust_file}."
        );
    }
}

/// The codes in the array literal assigned to `name` in `path`.
///
/// Language-agnostic on purpose: `const X: [&str; 4] = [...]` and
/// `const X = [...]` differ only in what sits between the name and the `=`.
/// Anchored on `= [` rather than the first `[`, because Rust's type annotation
/// is itself a bracket — the mistake this comment exists to stop being made
/// again.
fn string_array(path: &Path, name: &str) -> BTreeSet<String> {
    let src = fs::read_to_string(path).unwrap();
    let after = src
        .split_once(name)
        .unwrap_or_else(|| panic!("{} has no {name}", path.display()))
        .1;
    let body = after
        .split_once("= [")
        .expect("the const is an array literal")
        .1
        .split_once(']')
        .expect("the array literal is closed")
        .0;

    body.split(',')
        .filter_map(|part| {
            let part = part.trim();
            part.strip_prefix('"')
                .and_then(|p| p.strip_suffix('"'))
                .map(str::to_string)
        })
        .collect()
}

// --- the two source readers -------------------------------------------------
//
// Rust reading JS and Rust sources, which `i18n_contract.rs` and
// `registry_hints.rs` already established as the accepted trade: an
// integration test is its own crate, so sharing a scanner would mean either a
// module against the project's layout rule or a third test binary.

/// The command names cached in `src/lib/lookups.svelte.js`'s `COMMANDS` map.
fn session_store_commands() -> BTreeSet<String> {
    let src = fs::read_to_string(repo_root().join("src/lib/lookups.svelte.js")).unwrap();
    let body = src
        .split_once("const COMMANDS = {")
        .expect("the session store still has a COMMANDS map")
        .1
        .split_once("\n};")
        .expect("COMMANDS is still closed by a `};` on its own line")
        .0;

    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("//") {
                return None;
            }
            let (_, rest) = line.split_once(':')?;
            let value = rest.trim().trim_end_matches(',').trim();
            value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .map(str::to_string)
        })
        .collect()
}

/// Every `#[tauri::command]` whose signature takes a `country_code`.
fn commands_taking_a_country() -> BTreeSet<String> {
    let dir = repo_root().join("src-tauri/src/commands");
    let mut found = BTreeSet::new();

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        for block in src.split("#[tauri::command]").skip(1) {
            // The signature is everything up to the body's opening brace.
            let Some((signature, _)) = block.split_once('{') else {
                continue;
            };
            if !signature.contains("country_code") {
                continue;
            }
            let Some(after_fn) = signature.split_once("fn ") else {
                continue;
            };
            let name: String = after_fn
                .1
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                found.insert(name);
            }
        }
    }
    found
}

/// The readers are load-bearing for the assertions above, so they get their own
/// sanity check against values that must be there.
#[test]
fn the_source_readers_find_what_they_are_pointed_at() {
    let cached = session_store_commands();
    assert!(cached.contains("list_units"), "got: {cached:?}");
    assert!(cached.contains("list_countries"));
    // The two this arc moved out.
    assert!(!cached.contains("list_gip_systems"));
    assert!(!cached.contains("list_licence_levels"));

    let scoped = commands_taking_a_country();
    assert!(scoped.contains("list_premises_classes"), "got: {scoped:?}");
    assert!(scoped.contains("list_gip_systems"));
    assert!(scoped.contains("list_licence_levels"));
    // A command with no country must not be swept up.
    assert!(!scoped.contains("list_units"));

    let map: BTreeMap<&str, ()> = SCOPES.iter().map(|(t, _)| (*t, ())).collect();
    assert!(map.contains_key("gip_system"));
}
