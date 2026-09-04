// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The registry-hint contract, which spans three files no compiler reads
//! together:
//!
//!  * `src/lib/registryHints.js` says which FIELD earns which registry id;
//!  * `src-tauri/src/external_links.rs` says which id has a URL;
//!  * `src/i18n/<locale>/external.js` says what the hint READS.
//!
//! A hint naming an id the allowlist has never heard of does not fail at
//! build time, at mount time, or even at render time — it fails when a farmer
//! taps the button and gets `error.invalid.unknown_link`. A hint whose text
//! key is missing renders the raw key. Both are silent until someone is
//! already looking for a number they cannot find, so they are pinned here.
//!
//! Only one direction is checked. An allowlist entry no hint names is fine and
//! expected: the About panel's `homepage`, `source`, `issues`, `licence` and
//! `privacy` are links, not registries, and carry no `_hint` keys.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use terrazgo::external_links;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/src-tauri
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Collect the `"key": "value"` pairs out of a JS object literal.
///
/// A deliberately small reader rather than a shared one: integration tests are
/// separate crates, so sharing with `i18n_contract.rs` would mean either a
/// `tests/common/mod.rs` (against this project's no-`mod.rs` module rule) or a
/// `tests/common.rs` that cargo would build as a third, empty test binary.
/// Thirty lines of scanner is the cheaper of the three, and what it parses —
/// a flat map of string to string — cannot grow more complicated without the
/// data file changing shape first.
fn js_string_pairs(source: &str) -> BTreeMap<String, String> {
    let without_comments: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut pairs = BTreeMap::new();
    let mut rest = without_comments.as_str();
    while let Some(open) = rest.find('"') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('"') else {
            break;
        };
        let key = &after_open[..close];
        let tail = after_open[close + 1..].trim_start();
        // A string, a colon, then another string is an entry; anything else is
        // ordinary syntax and we simply carry on from after the first string.
        match tail.strip_prefix(':').map(str::trim_start) {
            Some(value_part) if value_part.starts_with('"') => {
                let after_quote = &value_part[1..];
                match after_quote.find('"') {
                    Some(end) => {
                        pairs.insert(key.to_string(), after_quote[..end].to_string());
                        rest = &after_quote[end + 1..];
                    }
                    None => break,
                }
            }
            _ => rest = &after_open[close + 1..],
        }
    }
    pairs
}

/// Every registry id named by `registryHints.js`, whatever country names it.
fn hinted_ids() -> BTreeSet<String> {
    let path = repo_root().join("src/lib/registryHints.js");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
    let ids: BTreeSet<String> = js_string_pairs(&source).into_values().collect();
    assert!(
        !ids.is_empty(),
        "no registry ids parsed from {} — did the data file change shape?",
        path.display()
    );
    ids
}

/// The locale directories, so a language added later is covered without
/// anyone remembering this file.
fn locales() -> Vec<String> {
    let dir = repo_root().join("src/i18n");
    let mut found: Vec<String> = fs::read_dir(&dir)
        .expect("src/i18n exists")
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|path| path.is_dir())
        .map(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .expect("locale dir name")
                .to_string()
        })
        .collect();
    found.sort();
    assert!(found.len() >= 2, "expected at least es and en");
    found
}

#[test]
fn every_hinted_registry_has_a_url_in_the_rust_allowlist() {
    let allowlisted: BTreeSet<&str> = external_links::link_ids().collect();
    let missing: Vec<String> = hinted_ids()
        .into_iter()
        .filter(|id| !allowlisted.contains(id.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "registryHints.js names {missing:?}, which src-tauri/src/external_links.rs does not \
         carry — the hint would render and then fail with error.invalid.unknown_link when \
         tapped. Add the URL there, or drop the hint."
    );
}

#[test]
fn every_hinted_registry_has_its_text_in_every_locale() {
    let ids = hinted_ids();
    let mut missing: Vec<String> = Vec::new();

    for locale in locales() {
        let path = repo_root()
            .join("src/i18n")
            .join(&locale)
            .join("external.js");
        let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
        let dict = js_string_pairs(&source);
        for id in &ids {
            // Both halves are load-bearing: the sentence explains where the
            // number lives, the button label says which registry opens.
            for suffix in ["hint", "open"] {
                let key = format!("registry.{id}_{suffix}");
                if !dict.contains_key(&key) {
                    missing.push(format!("{locale}: {key}"));
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "missing registry hint text (a hint with no entry renders its raw key): {missing:#?}"
    );
}
