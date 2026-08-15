// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cross-boundary contracts between the Rust command layer and the JS i18n
//! dictionaries — the compiler checks neither side, so these tests do:
//!
//! 1. every locale dictionary defines exactly the same key set;
//! 2. for each key, the `{placeholder}` set matches across locales;
//! 3. every error code the command boundary can emit has an `error.<code>`
//!    entry in every locale, and that entry's placeholders match the params
//!    the boundary sends — except `internal`, whose ABSENCE is the contract
//!    (the frontend shows the raw developer message, preceded by the
//!    localized `error.internal_intro` line);
//! 4. every `Invalid("<reason>")` machine code used anywhere in the crates
//!    has an `error.invalid.<reason>` entry (found by scanning the sources,
//!    so a new validation rule fails here until both dictionaries know it).
//!
//! The dictionaries are plain JS modules parsed with a tiny hand-rolled
//! reader (escape-aware `"key": "value"` pairs in either quote style, `//`
//! comment lines stripped, Prettier line-wrapping tolerated), so the test
//! stays self-contained Rust — no Node invocation, no new crates.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use module_cue::CueError;
use terrazgo::commands::classify;
use terrazgo_core::CoreError;

type Dictionary = BTreeMap<String, String>;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/src-tauri
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Parse one string literal (double- OR single-quoted — Prettier picks the
/// style that needs fewer escapes) at the start of `s`; returns (content, rest).
fn parse_string(s: &str) -> Option<(String, &str)> {
    let mut chars = s.char_indices();
    let quote = match chars.next() {
        Some((_, c @ ('"' | '\''))) => c,
        _ => return None,
    };
    let mut out = String::new();
    let mut escaped = false;
    for (i, c) in chars {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == quote {
            return Some((out, &s[i + 1..]));
        } else {
            out.push(c);
        }
    }
    None // unterminated string
}

/// Parse every `"key": "value"` pair out of a dictionary module's source.
///
/// Stream-based, not line-based: Prettier wraps long entries so the key and
/// its value may sit on different lines. `//` comment lines are stripped first
/// (they may contain stray quotes/apostrophes that would confuse the scanner);
/// JS string literals cannot contain raw newlines, so stripping whole comment
/// lines never cuts a value in half.
fn parse_dictionary(source: &str) -> Dictionary {
    let without_comments: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut dict = Dictionary::new();
    let mut rest = without_comments.trim_start();
    while !rest.is_empty() {
        match parse_string(rest) {
            Some((key, after_key)) => {
                // A string followed by `:` and another string is an entry;
                // anything else (shouldn't happen in a dictionary) is skipped.
                let after_colon = after_key.trim_start().strip_prefix(':');
                match after_colon.and_then(|s| parse_string(s.trim_start())) {
                    Some((value, after_value)) => {
                        dict.insert(key, value);
                        rest = after_value.trim_start();
                    }
                    None => rest = after_key.trim_start(),
                }
            }
            None => {
                // Not at a string: advance one char (past `export default {`,
                // commas, braces, …).
                let mut indices = rest.char_indices();
                indices.next();
                rest = match indices.next() {
                    Some((i, _)) => rest[i..].trim_start(),
                    None => "",
                };
            }
        }
    }
    dict
}

/// Every area file of one locale: `src/i18n/<locale>/*.js`, in name order.
///
/// The locale's own `src/i18n/<locale>.js` is the entry point i18n.js imports
/// and holds no entries — since 2026-08-13 the keys live in per-area files, so
/// a new module adds one file per locale rather than editing three long ones.
fn locale_parts(locale: &str) -> Vec<(String, Dictionary)> {
    let dir = repo_root().join("src/i18n").join(locale);
    let mut parts: Vec<(String, Dictionary)> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("js"))
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("area file name")
                .to_string();
            let dict = parse_dictionary(&fs::read_to_string(&path).expect("readable dictionary"));
            assert!(
                !dict.is_empty(),
                "no entries parsed from {} — did the dictionary format change?",
                path.display()
            );
            (name, dict)
        })
        .collect();
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!parts.is_empty(), "{} holds no area files", dir.display());
    parts
}

/// Every locale, merged from its area files and keyed by locale code.
fn dictionaries() -> BTreeMap<String, Dictionary> {
    let dir = repo_root().join("src/i18n");
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("src/i18n exists") {
        let path = entry.expect("readable dir entry").path();
        if !path.is_dir() {
            continue;
        }
        let locale = path
            .file_name()
            .and_then(|s| s.to_str())
            .expect("locale dir name")
            .to_string();
        let mut merged = Dictionary::new();
        for (_, part) in locale_parts(&locale) {
            merged.extend(part);
        }
        result.insert(locale, merged);
    }
    assert!(
        result.len() >= 2,
        "expected at least the es and en dictionaries in {}",
        dir.display()
    );
    result
}

/// No key may be defined by two area files of the same locale.
///
/// The merge in `<locale>.js` spreads the areas in order, so a duplicate would
/// be silently overwritten — one area's wording would win and the other's would
/// vanish with nothing to show for it. This is the hazard the split introduced,
/// so it is the guard the split owes.
#[test]
fn no_key_is_defined_by_two_area_files() {
    for locale in dictionaries().keys() {
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for (area, dict) in locale_parts(locale) {
            for key in dict.keys() {
                if let Some(first) = seen.get(key) {
                    panic!("'{locale}': '{key}' is defined in both {first}.js and {area}.js");
                }
                seen.insert(key.clone(), area.clone());
            }
        }
    }
}

/// The `{placeholder}` names used in a translated string.
fn placeholders(value: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let name = &after[..close];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            names.insert(name.to_string());
        }
        rest = &after[close + 1..];
    }
    names
}

#[test]
fn locale_dictionaries_define_identical_key_sets() {
    let dicts = dictionaries();
    let mut locales = dicts.iter();
    let (reference_locale, reference) = locales.next().expect("at least one dictionary");
    let reference_keys: BTreeSet<_> = reference.keys().collect();
    for (locale, dict) in locales {
        let keys: BTreeSet<_> = dict.keys().collect();
        let missing: Vec<_> = reference_keys.difference(&keys).collect();
        let extra: Vec<_> = keys.difference(&reference_keys).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "dictionary key sets differ: '{locale}' is missing {missing:?} and has extra \
             {extra:?} relative to '{reference_locale}' (every locale file must define the \
             same keys — the i18n layer contract)"
        );
    }
}

#[test]
fn placeholders_match_across_locales() {
    let dicts = dictionaries();
    let mut locales = dicts.iter();
    let (reference_locale, reference) = locales.next().expect("at least one dictionary");
    for (locale, dict) in locales {
        for (key, value) in dict {
            let Some(reference_value) = reference.get(key) else {
                continue; // key-set parity is the previous test's job
            };
            assert_eq!(
                placeholders(value),
                placeholders(reference_value),
                "placeholders for key '{key}' differ between '{locale}' and '{reference_locale}'"
            );
        }
    }
}

/// One sample per domain-error variant the boundary maps to a code.
///
/// When a new variant is added, `classify`'s exhaustive match forces a mapping
/// — extend this list at the same time so the dictionary check covers it.
/// The `invalid.*` family is covered separately by scanning the sources.
fn emitted_codes() -> Vec<(String, serde_json::Value)> {
    let samples: Vec<anyhow::Error> = vec![
        CueError::NotFound.into(),
        CueError::InvalidDate("2026-13-01".into()).into(),
        CueError::AuthorisationMissing {
            product_id: "p".into(),
            country: "es".into(),
        }
        .into(),
        CueError::CountryMismatch {
            provided: "fr".into(),
            farm: "es".into(),
        }
        .into(),
        CueError::PlotNotOnFarm {
            plot_id: "pl".into(),
            farm_id: "f".into(),
        }
        .into(),
        CueError::MissingPhiDays.into(),
        CoreError::NotFound.into(),
        CoreError::InvalidDate("2026-13-01".into()).into(),
        terrazgo_geo::GeoError::Http { status: 503 }.into(),
        terrazgo_geo::GeoError::Offline("dns failure".into()).into(),
    ];
    samples.iter().map(classify).collect()
}

#[test]
fn every_emitted_error_code_has_a_key_with_matching_params_in_every_locale() {
    let dicts = dictionaries();
    for (code, params) in emitted_codes() {
        let key = format!("error.{code}");
        let param_names: BTreeSet<String> = params
            .as_object()
            .expect("classify params are a JSON object")
            .keys()
            .cloned()
            .collect();
        for (locale, dict) in &dicts {
            let value = dict.get(&key).unwrap_or_else(|| {
                panic!("dictionary '{locale}' is missing '{key}' for boundary code '{code}'")
            });
            assert_eq!(
                placeholders(value),
                param_names,
                "placeholders of '{key}' in '{locale}' must match the params classify sends"
            );
        }
    }
}

#[test]
fn internal_deliberately_has_no_dictionary_entry() {
    // The frontend shows the raw developer message for `internal`, prefixed by
    // the `error.internal_intro` line (see errorText in src/lib/backend.js);
    // an `error.internal` entry proper would silently hide the raw message.
    for (locale, dict) in dictionaries() {
        assert!(
            !dict.contains_key("error.internal"),
            "'{locale}' defines error.internal — the untranslated fallback is the contract"
        );
    }
}

/// Collect every `Invalid("<reason>")` string literal under a crate's src/.
fn invalid_reason_codes(dir: &Path, found: &mut BTreeSet<String>) {
    for entry in fs::read_dir(dir).expect("readable source dir") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            invalid_reason_codes(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let source = fs::read_to_string(&path).expect("readable source file");
            let mut rest = source.as_str();
            while let Some(at) = rest.find("Invalid(\"") {
                let literal = &rest[at + "Invalid(".len()..];
                if let Some((reason, _)) = parse_string(literal) {
                    found.insert(reason);
                }
                rest = &rest[at + "Invalid(".len()..];
            }
        }
    }
}

/// Every workspace member's `src/`, read from the root manifest's own member
/// list.
///
/// Derived rather than listed, because a hand-maintained list stops covering
/// whatever someone forgets to add and says nothing about it: until 2026-08-13
/// this test named five crates, while `module-sigpac` and `src-tauri` were also
/// emitting `Invalid("…")` codes and going unchecked. Their keys happened to be
/// present; nothing was making sure.
fn workspace_src_dirs() -> Vec<PathBuf> {
    let root = repo_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest");
    let members = manifest
        .split_once("members = [")
        .expect("the workspace manifest lists its members")
        .1
        .split_once(']')
        .expect("the members list is closed")
        .0;
    let dirs: Vec<PathBuf> = members
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            let start = entry.find('"')? + 1;
            let end = entry[start..].find('"')? + start;
            Some(root.join(&entry[start..end]).join("src"))
        })
        .filter(|dir| dir.is_dir())
        .collect();

    // A crate on disk but absent from the manifest would be skipped just as
    // silently as the hand-written list used to skip one, so the derivation is
    // checked against the directory it derives from.
    for entry in fs::read_dir(root.join("crates")).expect("readable crates dir") {
        let src = entry.expect("readable dir entry").path().join("src");
        if src.is_dir() {
            assert!(
                dirs.contains(&src),
                "{} exists but is not a workspace member — it would go unscanned",
                src.display()
            );
        }
    }
    dirs
}

#[test]
fn every_invalid_reason_code_has_a_key_in_every_locale() {
    let mut reasons = BTreeSet::new();
    for dir in workspace_src_dirs() {
        invalid_reason_codes(&dir, &mut reasons);
    }
    assert!(
        !reasons.is_empty(),
        "no Invalid(\"…\") reason codes found — did the validation error convention change?"
    );

    let dicts = dictionaries();
    for reason in &reasons {
        let key = format!("error.invalid.{reason}");
        for (locale, dict) in &dicts {
            assert!(
                dict.contains_key(&key),
                "dictionary '{locale}' is missing '{key}' — a repository uses \
                 Invalid(\"{reason}\") so the frontend needs a translation for it"
            );
        }
    }
}
