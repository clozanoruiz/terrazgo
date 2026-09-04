// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Voice contract: shipping files describe decisions in a neutral solo-developer
//! voice, never as a record of a proposer-and-approver conversation (2026-07-09).
//! A comment reading "(approved 2026-07-03)" or "developer decision" describes
//! how the project is worked on rather than what the code does, and it travels
//! to every reader of the public source snapshot.
//!
//! Nothing else catches this. The storefront export guard
//! (`packaging/export-storefront.sh`) greps for the dev-tooling name alone, and
//! none of these phrases contain it; the compiler sees prose. So this test walks
//! the shipping tree — source *and* `docs/`, which the licensing contract in
//! `spdx_headers.rs` deliberately omits — and fails on any hit.
//!
//! Two classes of pattern, because precision matters more than reach here. A
//! false positive would push a future writer to mangle a legitimate sentence:
//!
//! * [`ALWAYS_BANNED`] are phrases with no innocent reading.
//! * [`BANNED_NEAR_A_DATE`] are words that are ordinary English on their own.
//!   "approved" is real regulatory vocabulary — Reglamento (CE) 1107/2009
//!   approves active substances — so it is flagged only when a digit sits
//!   within [`DATE_SPAN`] bytes, which is what turns it into an attribution
//!   ("(approved 2026-08-02)", "(2026-07-04, approved)").
//!
//! Note the bare word "approval" is deliberately NOT banned for the same
//! reason, only "needs approval" and "developer approval".
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// Phrases that are always a conversation record, whatever surrounds them.
const ALWAYS_BANNED: &[(&str, &str)] = &[
    (
        "needs approval",
        "state the open question, e.g. \"decision needed\"",
    ),
    ("propose before adding", "\"settled before coding\""),
    ("developer decision", "drop the attribution, keep the date"),
    ("developer approval", "drop the attribution, keep the date"),
    ("developer request", "drop the attribution, keep the date"),
    ("developer correction", "\"corrected <date>\""),
    ("(developer", "drop the parenthetical attribution"),
    // The UI-verification skills are dev tooling and must not be named in
    // shipping files; describe what they are instead.
    ("verifier-frontend", "\"scripted frontend checks\""),
    ("verifier-app", "\"the app-level harness\""),
];

/// Words that only read as an attribution when a date sits beside them.
const BANNED_NEAR_A_DATE: &[(&str, &str)] = &[(
    "approved",
    "keep the date alone, e.g. \"(2026-08-02)\" or \"added 2026-08-02\"",
)];

/// How far from the word a digit may sit and still make it an attribution.
/// Wide enough for "(2026-07-04, approved)", narrow enough that a nearby
/// article number in ordinary prose does not trip it.
const DATE_SPAN: usize = 14;

/// Directories walked recursively, relative to the workspace root. `docs/` is
/// here and absent from the licensing contract on purpose: it ships, and it is
/// where decision prose lands.
const SHIPPING_ROOTS: &[&str] = &["crates", "src", "src-tauri", "docs", "packaging/storefront"];
const EXTENSIONS: &[&str] = &["rs", "js", "svelte", "md", "typ", "sql", "yml"];

/// Directory names never entered: build output, tauri codegen, the generated
/// Android project, and `docs/references/`, which holds a vendored FEGA schema
/// — third-party bytes are reproduced verbatim, never linted.
const SKIP_DIRS: &[&str] = &["gen", "target", "dist", "node_modules", "references"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .to_path_buf()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    // A configured root may legitimately not exist yet; nothing to walk.
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

/// True when an ASCII digit sits within [`DATE_SPAN`] bytes of the match.
///
/// Indexing the byte slice rather than slicing the `str` keeps this safe on the
/// accented Spanish and Catalan prose in `docs/` — an arbitrary byte offset is
/// rarely a UTF-8 char boundary, and slicing a `str` there would panic.
fn near_a_date(text: &str, at: usize, len: usize) -> bool {
    let bytes = text.as_bytes();
    let start = at.saturating_sub(DATE_SPAN);
    let end = (at + len + DATE_SPAN).min(bytes.len());
    bytes[start..end].iter().any(u8::is_ascii_digit)
}

/// Every `needle` occurrence in `haystack`, as byte offsets.
fn occurrences<'a>(haystack: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    haystack.match_indices(needle).map(|(offset, _)| offset)
}

#[test]
fn shipping_files_keep_a_neutral_voice() {
    let root = workspace_root();
    let mut files = Vec::new();
    for dir in SHIPPING_ROOTS {
        collect_files(&root.join(dir), &mut files);
    }

    assert!(
        files.len() > 100,
        "only {} shipping files found — did the tree move?",
        files.len()
    );

    let mut hits = Vec::new();
    for path in &files {
        // This file quotes every banned phrase as a constant, so scanning it
        // would fail the suite on its own definitions.
        if path.ends_with("tests/neutral_voice.rs") {
            continue;
        }
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let lower = text.to_lowercase();
        let rel = path.strip_prefix(&root).unwrap_or(path).display();

        for (phrase, fix) in ALWAYS_BANNED {
            for offset in occurrences(&lower, phrase) {
                hits.push(format!(
                    "{rel}:{} — \"{phrase}\" → {fix}",
                    line_of(&lower, offset)
                ));
            }
        }
        for (word, fix) in BANNED_NEAR_A_DATE {
            for offset in occurrences(&lower, word) {
                if near_a_date(&lower, offset, word.len()) {
                    hits.push(format!(
                        "{rel}:{} — \"{word}\" beside a date → {fix}",
                        line_of(&lower, offset)
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "shipping files must describe decisions in a neutral voice, not as a \
         proposer-and-approver exchange — the public source snapshot carries \
         this prose to every reader:\n{}",
        hits.join("\n")
    );
}
