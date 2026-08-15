// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Registration contract: `tauri::generate_handler!` needs command function
//! paths at compile time, so the list in `lib.rs` is maintained by hand — a
//! command written but never registered only fails at runtime, when the UI
//! calls it. This test compares the two sources so the mistake fails in CI
//! instead.
//!
//! It scans the whole `src/commands/` directory rather than one file (since
//! 2026-08-13), which is what let the commands be split per domain: naming a
//! single file here was the structural reason they all had to live in one.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn read_source(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Every command source: the boundary file and each per-domain file beside it.
///
/// Derived from the directory, so adding `commands/weather.rs` needs no edit
/// here and cannot go unchecked.
fn command_sources() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut sources = vec![read_source("src/commands.rs")];
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            sources.push(fs::read_to_string(&path).unwrap());
        }
    }
    assert!(
        sources.len() > 1,
        "src/commands/ holds no .rs files — did the split get reverted?"
    );
    sources
}

/// Every function annotated `#[tauri::command]`, across all of them.
fn defined_commands() -> BTreeSet<String> {
    let source = command_sources().join("\n");
    let mut names = BTreeSet::new();
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        if line.trim() != "#[tauri::command]" {
            continue;
        }
        // The fn follows the attribute, possibly after further attributes.
        // Commands may be `pub fn` or `pub async fn` (long-running ones are
        // async so they run off the main thread instead of freezing the UI).
        let name = lines
            .by_ref()
            .find_map(|l| {
                let sig = l.trim();
                sig.strip_prefix("pub async fn ")
                    .or_else(|| sig.strip_prefix("pub fn "))
                    .map(str::to_string)
            })
            .and_then(|sig| sig.split('(').next().map(str::to_string))
            .expect("#[tauri::command] must be followed by a pub fn");
        names.insert(name);
    }
    assert!(
        !names.is_empty(),
        "no #[tauri::command] functions found under src/commands — did they move?"
    );
    names
}

/// Every command listed in lib.rs's generate_handler! block.
fn registered_commands() -> BTreeSet<String> {
    let source = read_source("src/lib.rs");
    let start = source
        .find("generate_handler![")
        .expect("lib.rs contains the generate_handler! block");
    let block = &source[start + "generate_handler![".len()..];
    let block = &block[..block.find(']').expect("generate_handler! block is closed")];
    block
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .rsplit("::")
                .next()
                .expect("rsplit always yields")
                .to_string()
        })
        .collect()
}

#[test]
fn every_command_is_registered_and_every_registration_exists() {
    let defined = defined_commands();
    let registered = registered_commands();

    let unregistered: Vec<_> = defined.difference(&registered).collect();
    let phantom: Vec<_> = registered.difference(&defined).collect();
    assert!(
        unregistered.is_empty() && phantom.is_empty(),
        "command registration drift: {unregistered:?} are defined under src/commands but missing \
         from generate_handler! in lib.rs; {phantom:?} are registered but have no \
         #[tauri::command] definition"
    );
}
