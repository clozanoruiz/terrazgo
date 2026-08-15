// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Startup-gate contract: the readiness probe must stay raced against a
//! timeout, because on Android the retry is what delivers the previous answer.
//!
//! Measured on-device (Galaxy A22, Android 13, WebView 151, fresh data dir):
//! Rust executes the first `app_ready` at +0.000s and returns false, the setup
//! hook completes at +0.670s, and the webview does not receive that first
//! answer until +6.069s — 10 ms after the SECOND probe was posted. Widening the
//! probe timeout from 2s to 6s moved that delivery from +2.07s to +6.07s in
//! step with it, never with setup. **A pending reply is delivered when the next
//! IPC message is posted.**
//!
//! So a lone `await invoke("app_ready")` deadlocks against itself: the answer
//! waits for a next message, and the gate only sends one after receiving the
//! answer. The loop never comes round, its fail-open deadline is never
//! evaluated, `mount()` never runs, and the first launch after install is
//! permanently blank — which is exactly the bug this guards against, seen in
//! the field on v0.1.5 and reproducible with `adb shell pm clear`.
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
         contract with it — do not delete it: on Android the retry is what delivers the \
         previous answer (see this file's header for the measurement)."
    );

    assert!(
        code.contains("Promise.race("),
        "The app_ready probe is no longer raced against a timeout.\n\n\
         On Android a pending IPC reply is delivered when the NEXT message is posted, so a \
         lone `await invoke(\"app_ready\")` waits for an answer that is waiting for it — the \
         gate deadlocks, mount() never runs, and the first launch after install is blank \
         forever. Restore the race; the retry is the pump, not a way of giving up."
    );

    assert!(
        code.contains("setTimeout") && code.contains("reject"),
        "The race has no rejecting timeout arm, so nothing ever posts the second message \
         that delivers the first answer."
    );
}

#[test]
fn the_probe_is_never_awaited_bare() {
    let code = code_only(&main_js());

    // The exact shape that caused the bug. `probeReady()` may be awaited; the
    // raw invoke may not.
    assert!(
        !code.contains(r#"await invoke("app_ready")"#),
        "src/main.js awaits `invoke(\"app_ready\")` directly. That is the deadlock: on \
         Android the reply is delivered by the next outgoing message, so a single awaited \
         probe never settles and the gate never comes round. Await the raced helper instead."
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
