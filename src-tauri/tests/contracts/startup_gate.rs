// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Startup-gate contract: the readiness probe must stay raced against a
//! timeout, or an answer that never arrives holds the gate forever — its
//! fail-open deadline is never evaluated, `mount()` never runs, and the screen
//! stays blank.
//!
//! That is not hypothetical: it is the bug this guards against, seen in the
//! field on v0.1.5. Before tao 0.37, a reply queued while Android was starting
//! was delivered only when the NEXT IPC message was posted (Galaxy A22: the
//! first answer arrived at +6.069s, 10 ms after the second probe), so a lone
//! `await invoke("app_ready")` waited for an answer that was waiting for it.
//! tao 0.37 fixes that defect (tao#1304); the race stays because it is what
//! makes the deadline reachable whatever else keeps an answer from arriving.
//!
//! Why a source scan rather than a behavioural test: the failure only exists on
//! a device, and the frontend has no test runner by decision (testing strategy
//! #5). This is the same shape as the other contract tests here — the compiler
//! cannot check it, so CI reads the source instead.
//!
//! Making the command `async` does NOT substitute for this; that was measured
//! and rejected (the parking is in reply delivery, not command dispatch — see
//! `docs/architecture.md` → "On Android the webview starts first").
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

/// `src/main.js`, the frontend entry that owns the gate.
fn main_js() -> String {
    // ../src/main.js — the tests live in src-tauri/, the frontend is a sibling.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .join("src/main.js");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Source with `//` line comments stripped, so the prose above the gate (which
/// quotes the very pattern this test forbids) cannot satisfy or trip an
/// assertion. Block comments are not used in this file; string literals do not
/// contain these fragments.
fn code_only(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_readiness_probe_is_raced_against_a_timeout() {
    let code = code_only(&main_js());

    assert!(
        code.contains(r#"invoke("app_ready")"#),
        "src/main.js no longer probes app_ready. If the startup gate moved, move this \
         contract with it — do not delete it: the gate must reach its fail-open deadline \
         even when an answer never arrives (see this file's header)."
    );

    assert!(
        code.contains("Promise.race("),
        "The app_ready probe is no longer raced against a timeout.\n\n\
         A probe whose answer never arrives then holds the gate forever: the fail-open \
         deadline is never evaluated, mount() never runs, and the screen stays blank — the \
         first-launch bug of v0.1.5. Restore the race."
    );

    assert!(
        code.contains("setTimeout") && code.contains("reject"),
        "The race has no rejecting timeout arm, so a probe that never settles still holds \
         the gate past its deadline."
    );
}

#[test]
fn the_probe_is_never_awaited_bare() {
    let code = code_only(&main_js());

    // The exact shape that caused the bug. `probeReady()` may be awaited; the
    // raw invoke may not.
    assert!(
        !code.contains(r#"await invoke("app_ready")"#),
        "src/main.js awaits `invoke(\"app_ready\")` directly: if that answer never arrives \
         the gate never comes round, and the deadline never fires. Await the raced helper \
         instead."
    );
}

#[test]
fn the_gate_keeps_its_fail_open_deadline() {
    let code = code_only(&main_js());

    assert!(
        code.contains("deadline"),
        "The startup gate lost its deadline. Mounting and surfacing real command errors \
         beats an unexplained blank screen when the backend is genuinely broken — the gate \
         must fail open, not wait forever."
    );
}
