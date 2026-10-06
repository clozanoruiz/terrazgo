// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What may live in SQLite's prepared-statement cache, and how much of it.
//!
//! The cache is an LRU keyed by SQL TEXT, it belongs to the CONNECTION, and the
//! app shares one connection across every command it runs. So it is a single
//! small fixed-size resource that every code path draws on, and overrunning it
//! is invisible: nothing fails, nothing returns a wrong answer, the statements
//! that run on every write are simply prepared afresh each time. The
//! measurement that put them in the cache (`audit::begin`, 24 µs → 4-5 µs)
//! would quietly undo itself.
//!
//! rusqlite exposes no way to ask a live connection how full its cache is —
//! `len` and `capacity` exist only inside its own `#[cfg(test)]`. So the budget
//! cannot be checked at runtime, and this is the substitute: count the cached
//! statements in the source, and compare against the capacity the connection is
//! actually given.
//!
//! Two rules make that count sound, both enforced here:
//!
//!   1. **`prepare_cached` is called in exactly one place** —
//!      `terrazgo_core::sql::cached_statement`, which takes `&'static str` so that text
//!      built per table or per parameter count cannot reach the cache. Every
//!      other caller goes through it.
//!   2. **Every cached statement is passed as a named `const`.** Nothing in the
//!      type system wants this; the count does. An inline literal is legal Rust
//!      and uncountable here, so it is refused.
//!
//! # What it deliberately does not do
//!
//! It counts NAMES, not distinct SQL. One const used from two call sites is one
//! cache entry and counts once; two consts holding identical text are one cache
//! entry and count twice. The second is the conservative direction, and the
//! first is what the name is for.
//!
//! It also cannot see a bulk loop that holds its own statements — which is the
//! shape the sync applier must use and the one failure this file's budget would
//! otherwise be asked to absorb. That rule lives with `sql::cached_statement` and in
//! docs/sync.md; there is no source pattern that distinguishes it.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use terrazgo_core::db::STATEMENT_CACHE_CAPACITY;

/// Shipping source only, as in `hardened_connections`: test trees legitimately
/// prepare whatever they like against their own connections.
const SOURCE_ROOTS: &[&str] = &["../crates", "../src-tauri/src"];

/// The one file allowed to name `prepare_cached`: it is the body of
/// `sql::cached_statement`. Matched as a path, not a file name, so that a
/// `sql.rs` in some other crate does not inherit the exemption.
const CACHE_GATEWAY: &str = "terrazgo-core/src/sql.rs";

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name == "target" || name == "gen" || name == "tests" {
                continue;
            }
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The file with doc comments and `#[cfg(test)]` modules removed — a doc
/// example that calls `cached` is prose, not a statement the app prepares.
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

/// The last top-level argument of each `cached_statement(…)` call in `code`.
///
/// Walks the argument list counting brackets rather than matching a pattern,
/// because the first argument is an expression (`&self.tx`, `conn`) and the
/// call may wrap across lines with a trailing comma. An identifier character
/// before the name means this is some other function whose name ends the same
/// way, which nothing here should judge.
fn cached_call_arguments(code: &str) -> Vec<String> {
    const CALL: &str = "cached_statement(";
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices(CALL) {
        if at > 0 {
            let before = bytes[at - 1];
            if before.is_ascii_alphanumeric() || before == b'_' {
                continue;
            }
        }
        let mut depth = 1usize;
        let mut cut = 0usize;
        let mut arguments = Vec::new();
        let args_from = at + CALL.len();
        for (offset, ch) in code[args_from..].char_indices() {
            match ch {
                '(' | '[' | '<' => depth += 1,
                ')' | ']' | '>' => {
                    depth -= 1;
                    if depth == 0 {
                        arguments.push(code[args_from + cut..args_from + offset].trim());
                        break;
                    }
                }
                ',' if depth == 1 => {
                    arguments.push(code[args_from + cut..args_from + offset].trim());
                    cut = offset + 1;
                }
                _ => {}
            }
        }
        // The last argument that is there at all: a wrapped call ends `SQL,\n)`,
        // whose final segment is empty. An unbalanced call yields nothing, and
        // is reported as an empty name so the assertion names its file.
        let last = arguments.iter().rev().find(|a| !a.is_empty());
        found.push(last.map(|a| (*a).to_owned()).unwrap_or_default());
    }
    found
}

/// Whether `arg` is a path ending in a SCREAMING_SNAKE_CASE constant.
fn is_const_name(arg: &str) -> bool {
    let name = arg.rsplit("::").next().unwrap_or(arg);
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn shipping_files() -> Vec<PathBuf> {
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
    files
}

#[test]
fn only_the_gateway_reaches_the_statement_cache() {
    let mut offenders = Vec::new();
    for file in shipping_files() {
        if file.ends_with(CACHE_GATEWAY) {
            continue;
        }
        if shipping_code(&std::fs::read_to_string(&file).unwrap()).contains("prepare_cached") {
            offenders.push(file.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "these files call prepare_cached directly: {offenders:#?}\n\n\
         The prepared-statement cache is one fixed-size LRU shared by the whole \
         app, keyed by SQL text. Everything that uses it goes through \
         terrazgo_core::sql::cached_statement, whose &'static str signature keeps \
         per-table and per-parameter-count SQL out of it, and whose callers this \
         file can therefore count. A statement whose text varies uses plain \
         prepare, or is held by the loop that runs it."
    );
}

#[test]
fn every_cached_statement_is_a_named_const_within_budget() {
    let mut names = BTreeSet::new();
    let mut anonymous = Vec::new();
    for file in shipping_files() {
        let code = shipping_code(&std::fs::read_to_string(&file).unwrap());
        for arg in cached_call_arguments(&code) {
            if is_const_name(&arg) {
                names.insert(arg);
            } else {
                anonymous.push(format!("{}: cached(…, {arg})", file.display()));
            }
        }
    }

    assert!(
        anonymous.is_empty(),
        "these cached statements are not named consts: {anonymous:#?}\n\n\
         Every statement in the shared cache is declared as a SCREAMING_SNAKE \
         const beside the code that runs it. The type system does not need this; \
         the budget below does — an inline literal occupies a cache slot that \
         nothing can count. Give it a name and a line saying what it costs."
    );
    assert!(
        !names.is_empty(),
        "no cached statements were found at all — the scan is broken, since \
         audit::begin alone runs several"
    );
    assert!(
        names.len() <= STATEMENT_CACHE_CAPACITY,
        "{} cached statements against a cache of {STATEMENT_CACHE_CAPACITY}: \
         {names:#?}\n\n\
         Past capacity the LRU starts evicting, and what it evicts first is \
         whatever ran least recently — on a write, that is the audit path, \
         which is the one thing that must never be re-prepared. Either this \
         working set is too big (a statement that does not run on every write \
         does not belong in the cache), or the capacity in \
         terrazgo_core::db::STATEMENT_CACHE_CAPACITY should rise deliberately, \
         with the reason written there.",
        names.len()
    );
}

/// The scan is only worth anything if it would actually catch a regression.
#[test]
fn the_scan_reads_the_shapes_it_claims_to() {
    // A direct call, a qualified one, and one wrapped across lines with the
    // trailing comma rustfmt puts there.
    let code = "let a = cached_statement(conn, ALPHA)?;\n\
                let b = crate::sql::cached_statement(&self.tx, super::BETA)?;\n\
                let c = sql::cached_statement(\n    &tx,\n    GAMMA,\n)?;";
    assert_eq!(
        cached_call_arguments(code),
        ["ALPHA", "super::BETA", "GAMMA"]
    );

    // Some other function whose name happens to end the same way is not ours.
    assert!(cached_call_arguments("if has_cached_statement(key) { }").is_empty());

    // An inline literal is found, and refused for not being a name.
    let literal = cached_call_arguments("cached_statement(conn, \"SELECT 1\")?");
    assert_eq!(literal.len(), 1);
    assert!(!is_const_name(&literal[0]));

    assert!(is_const_name("NEXT_SEQ_SQL") && is_const_name("audit::WRITE_CHANGE_SQL"));
    assert!(!is_const_name("sql") && !is_const_name("MixedCase") && !is_const_name(""));

    // Doc examples and test modules are prose and fixtures, not shipping calls.
    assert!(
        cached_call_arguments(&shipping_code("/// cached_statement(conn, DOC_SQL)")).is_empty()
    );
    assert!(
        cached_call_arguments(&shipping_code(
            "#[cfg(test)]\nmod tests { fn f() { cached_statement(conn, TEST_SQL); } }"
        ))
        .is_empty()
    );
}
