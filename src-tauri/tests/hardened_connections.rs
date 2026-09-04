// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every shipping file that opens a SQLite connection must also harden it.
//!
//! `terrazgo_core::db::harden`'s settings live on the CONNECTION, not in the
//! database file — a fact pinned by a test beside `harden` itself. So they do
//! not travel: each open site has to apply them, and nothing in the type system
//! says so. `Database::new` covers the long-lived connections by construction,
//! but the short-lived read-only opens (a GeoPackage, a backup being validated,
//! the corruption check) never become a `Database`, and a future crate could
//! add another.
//!
//! This is a **rule, not an allowlist**, which is the property that matters:
//! adding a crate, a module or a feature never requires editing this file. A
//! new file that opens a connection either hardens it or fails here with an
//! explanation.
//!
//! # What it deliberately does not do
//!
//! It matches per FILE, not per call site. `terrazgo-geo`'s `try_open` opens a
//! connection and hardens it one call away, inside `apply_pragmas_and_migrate`
//! — which is correct factoring, and an adjacency rule would flag it. The cost
//! is a blind spot: a file that opens two connections and hardens one passes.
//! That is a tripwire against forgetting wholesale, not a proof of coverage,
//! and it is worth being clear about which one this is.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

/// Shipping source only. Test trees are excluded: they legitimately open raw
/// connections to build fixtures, and `terrazgo-testkit` is dev-only.
const SOURCE_ROOTS: &[&str] = &["../crates", "../src-tauri/src"];

/// The crate that defines `harden` itself, and the dev-only fixture crate.
const EXEMPT_DIRS: &[&str] = &["terrazgo-testkit"];

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            // Skip build output, generated code, and each crate's test tree.
            if name == "target" || name == "gen" || name == "tests" {
                continue;
            }
            if EXEMPT_DIRS.contains(&name.as_str()) {
                continue;
            }
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The file with doc comments and `#[cfg(test)]` modules removed.
///
/// Both would otherwise produce false positives: `sql.rs` opens a connection in
/// a doc example, and most `db.rs` files open several in their tests.
fn shipping_code(source: &str) -> String {
    let without_tests = match source.find("#[cfg(test)]") {
        Some(at) => &source[..at],
        None => source,
    };
    without_tests
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with("///") && !line.trim_start().starts_with("//!")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_file_that_opens_a_connection_also_hardens_it() {
    let mut files = Vec::new();
    for root in SOURCE_ROOTS {
        rust_sources(Path::new(root), &mut files);
    }
    assert!(
        files.len() > 50,
        "the source scan found only {} files — the roots are wrong, and a \
         contract test that scans nothing passes silently",
        files.len()
    );

    let mut offenders = Vec::new();
    let mut checked = 0usize;
    for file in &files {
        let source = std::fs::read_to_string(file).unwrap();
        let code = shipping_code(&source);
        if !code.contains("Connection::open") {
            continue;
        }
        checked += 1;
        // Either it hardens directly, or it hands the connection to
        // `Database::new`, which hardens on the way in.
        if !code.contains("harden(") && !code.contains("Database::new") {
            offenders.push(file.display().to_string());
        }
    }

    assert!(
        checked > 0,
        "no shipping file appeared to open a connection — the scan is broken"
    );
    assert!(
        offenders.is_empty(),
        "these files open a SQLite connection without hardening it: {offenders:#?}\n\n\
         Every connection must go through terrazgo_core::db::harden, because its \
         settings live on the connection and not in the database file — they do \
         not travel, so each open site applies them or nobody does. Either call \
         harden() after opening, or wrap the connection in Database::new, which \
         hardens on the way in. See docs/architecture.md → \"Hardening\"."
    );
}

/// The scan is only worth anything if it would actually catch a regression.
#[test]
fn the_scan_would_catch_an_unhardened_open() {
    let offending = "fn open_it() -> Connection { Connection::open(path).unwrap() }";
    let code = shipping_code(offending);
    assert!(code.contains("Connection::open"));
    assert!(!code.contains("harden(") && !code.contains("Database::new"));

    // ...and that it does not fire on the two legitimate shapes.
    let direct = "fn open_it() { let c = Connection::open(p)?; harden(&c)?; }";
    assert!(shipping_code(direct).contains("harden("));
    let wrapped = "fn open_it() { Database::new(Connection::open(p)?) }";
    assert!(shipping_code(wrapped).contains("Database::new"));
}

/// Doc examples and test modules must not count as shipping opens — `sql.rs`
/// has one in a doc comment, and every `db.rs` has several in its tests.
#[test]
fn doc_examples_and_test_modules_are_not_shipping_code() {
    let doc = "/// let conn = Connection::open_in_memory().unwrap();\nfn real() {}";
    assert!(!shipping_code(doc).contains("Connection::open"));

    let tests = "fn real() {}\n#[cfg(test)]\nmod tests {\n  Connection::open(x);\n}";
    assert!(!shipping_code(tests).contains("Connection::open"));
}
