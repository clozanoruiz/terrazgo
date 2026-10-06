// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Number-formatting contract: the frontend renders every number through
//! `formatNumber` (and its siblings) in `src/i18n.js`, never by building the
//! digits itself.
//!
//! The decimal separator is a comma in Castilian and Catalan and a point in
//! English, so a hand-built number is wrong in whichever language it was not
//! written for — and `toFixed` always emits a point, in every locale, with no
//! way to ask it for another. That is what makes it worth banning by name
//! rather than trusting a prose rule: this began as one `toFixed(1)` in a cache
//! size, and the rule it broke was documented nowhere.
//!
//! # What this can and cannot catch
//!
//! It catches the *class* — a call that formats digits outside the i18n layer.
//! It deliberately does NOT try to detect "a raw number interpolated into
//! markup": `{record.dose_value}` and `{record.notes}` are the same syntax, and
//! nothing static separates them without type information the frontend does not
//! carry. That half stays a review job, which is why the rule is also written
//! down in `docs/frontend-conventions.md`.
//!
//! `toLocaleString` is banned alongside `toFixed` for a different reason: it is
//! not wrong, it is *unpinned*. Called bare it follows the host's locale rather
//! than the language the holding chose, which is the same defect that retired
//! the native date picker.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// Calls that format digits, and what to reach for instead.
const BANNED: &[(&str, &str)] = &[
    (
        ".toFixed(",
        "formatNumber(value, digits) — toFixed always emits a decimal POINT",
    ),
    (
        ".toLocaleString(",
        "formatNumber/formatDate — they pin the locale to the chosen language",
    ),
    (
        ".toLocaleDateString(",
        "formatDate(iso) — it pins the locale to the chosen language",
    ),
];

/// The frontend tree. Only `src/` is scanned: Rust formats numbers for the
/// printed book under its own rules (`terrazgo-recordbook`'s `format_number`),
/// and `toFixed` is not Rust syntax anyway.
const ROOT: &str = "src";

/// The one file allowed to construct `Intl` objects and format digits — it is
/// the layer every other caller goes through.
const I18N_LAYER: &str = "i18n.js";

/// The owned numeric control and its parser, which are allowed to name the
/// native input they replace: one in prose, one in a measurement note.
const NUMBER_CONTROL: [&str; 2] = ["NumberInput.svelte", "numberValue.js"];

/// A test file NAMES what it guards, so the banned literals are data there
/// rather than usage — `numberValue.test.js` asserts that "1,5kg" is refused,
/// and a suite for this very rule would have to write `type="number"` out.
/// Scanned for SPDX and voice like any other file; exempt only from this ban.
fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".test.js"))
}

const EXTENSIONS: &[&str] = &["js", "svelte"];
const SKIP_DIRS: &[&str] = &["node_modules", "dist"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .to_path_buf()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| SKIP_DIRS.iter().any(|d| n == *d))
            {
                continue;
            }
            collect_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| EXTENSIONS.iter().any(|e| ext == *e))
        {
            out.push(path);
        }
    }
}

/// 1-based line number of a byte offset, for the failure message.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

#[test]
fn the_frontend_formats_numbers_through_the_i18n_layer() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_files(&root.join(ROOT), &mut files);
    assert!(
        files.len() > 20,
        "expected to walk the frontend tree, found {} files — has {ROOT}/ moved?",
        files.len()
    );

    let mut findings = Vec::new();
    for path in &files {
        if path.file_name().is_some_and(|n| n == I18N_LAYER) || is_test_file(path) {
            continue;
        }
        let text = fs::read_to_string(path).expect("frontend file is readable");
        for (needle, instead) in BANNED {
            for (offset, _) in text.match_indices(needle) {
                findings.push(format!(
                    "{}:{}: `{needle}` — use {instead}",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                    line_of(&text, offset),
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "numbers must be formatted through src/i18n.js \
         (docs/frontend-conventions.md → \"Numbers\"):\n  {}",
        findings.join("\n  ")
    );
}

/// No view may reach for the native numeric input again.
///
/// `<input type="number">` parses what the user types with the OPERATING
/// SYSTEM's locale, and in the mismatch case it reinterprets rather than
/// refusing: measured in the shipping WebKitGTK webview with the OS in en_GB,
/// typing "1,5" yields **15**. A farmer running the app in Castilian on an
/// English-locale machine would record ten times the dose, silently, in a
/// register read at an inspection.
///
/// `lang` does not fix it — also measured, and worth stating so nobody spends
/// the afternoon again: `lang="es"` on the input, on an ancestor, and
/// `documentElement.lang` (which the app already sets) all leave the parse at
/// 15. WebKit's number localizer reads the application locale and ignores the
/// attribute entirely.
#[test]
fn views_use_the_owned_numeric_control() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_files(&root.join(ROOT), &mut files);

    let mut findings = Vec::new();
    for path in &files {
        if path
            .file_name()
            .is_some_and(|n| NUMBER_CONTROL.iter().any(|f| n == *f))
            || is_test_file(path)
        {
            continue;
        }
        let text = fs::read_to_string(path).expect("frontend file is readable");
        for (offset, _) in text.match_indices(r#"type="number""#) {
            findings.push(format!(
                "{}:{}",
                path.strip_prefix(&root).unwrap_or(path).display(),
                line_of(&text, offset),
            ));
        }
    }

    assert!(
        findings.is_empty(),
        "use NumberInput.svelte, never a native numeric input — the OS locale \
         parses it and silently reads \"1,5\" as 15:\n  {}",
        findings.join("\n  ")
    );
}

/// The policy itself, guarded where it is written rather than where it is used.
///
/// Four decimals and no grouping are not preferences: together they make
/// `formatNumber` agree character for character with the record book's
/// `format_number` (`crates/terrazgo-recordbook/src/lib.rs`), so a farmer reads
/// the same figure on screen and on the printed book. Two decimals would round
/// a dose of 0,0375 l/ha to "0,04" — a regulatory value silently restated.
#[test]
fn the_number_policy_matches_the_printed_book() {
    let root = workspace_root();
    let i18n = fs::read_to_string(root.join("src/i18n.js")).expect("src/i18n.js is readable");

    assert!(
        i18n.contains("maximumFractionDigits = 4"),
        "formatNumber must default to 4 fraction digits, matching the book's \
         format_number ({{value:.4}}); fewer silently restates a dose"
    );
    assert!(
        i18n.contains("useGrouping: false"),
        "formatNumber must disable grouping: the printed book has no thousands \
         separator, and CLDR groups Catalan at 4 digits where Castilian does not"
    );
}
