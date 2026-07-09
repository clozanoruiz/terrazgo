// SPDX-License-Identifier: AGPL-3.0-or-later

//! Licensing contract: every app source file carries the one-line SPDX
//! identifier, so a file copied out of the repository still announces that
//! AGPL applies (decision 2026-07-03). Nothing checks this at compile time —
//! a new file without the header would silently ship — so this test walks the
//! source tree and fails on any miss.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

const SPDX_LINE: &str = "SPDX-License-Identifier: AGPL-3.0-or-later";

/// Directories walked recursively, relative to the workspace root. Dot-prefixed
/// directories (dev tooling, generated fixtures) are outside these roots on
/// purpose; build output (`target/`, `dist/`, `node_modules/`) lives at the
/// root too, so it is never entered.
const SOURCE_ROOTS: &[&str] = &["crates", "src", "src-tauri"];
const EXTENSIONS: &[&str] = &["rs", "js", "svelte"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri sits inside the workspace")
        .to_path_buf()
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            // src-tauri/gen holds tauri codegen output (and would hold mobile
            // project scaffolding); never headers-enforced.
            if path
                .file_name()
                .is_some_and(|n| n == "gen" || n == "target")
            {
                continue;
            }
            collect_sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| EXTENSIONS.iter().any(|e| ext == *e))
        {
            out.push(path);
        }
    }
}

#[test]
fn every_source_file_starts_with_the_spdx_identifier() {
    let root = workspace_root();
    let mut sources = Vec::new();
    for dir in SOURCE_ROOTS {
        collect_sources(&root.join(dir), &mut sources);
    }
    // Root-level JS config files are source too, but the root itself cannot be
    // walked (target/, node_modules/ live there).
    for file in ["vite.config.js", "eslint.config.js"] {
        sources.push(root.join(file));
    }

    assert!(
        sources.len() > 50,
        "only {} source files found — did the tree move?",
        sources.len()
    );

    let missing: Vec<_> = sources
        .iter()
        .filter(|path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            !text.lines().next().is_some_and(|l| l.contains(SPDX_LINE))
        })
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string()
        })
        .collect();

    assert!(
        missing.is_empty(),
        "source files missing the `{SPDX_LINE}` header on line 1: {missing:#?}"
    );
}
