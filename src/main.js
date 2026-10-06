// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Frontend entry: normalise the route, then mount the Svelte app. The module
// graph waits on i18n.js's top-level await, so t() is synchronous everywhere
// by the time any component renders.

import { mount } from "svelte";
import App from "./App.svelte";
import { invoke } from "./lib/backend.js";
import { loadLookups } from "./lib/lookups.svelte.js";

// A real route lets the nav highlighting match; replaceState fires no events.
if (!location.hash) {
  history.replaceState(null, "", "#/status");
}

// Native-app context-menu policy: text-editing controls keep the webview's
// native GTK cut/copy/paste menu; everywhere else right-click does nothing —
// the default menu there exposes browser actions (Reload, Back) that have no
// place in a desktop app.
window.addEventListener("contextmenu", (event) => {
  const el = event.target instanceof Element ? event.target : null;
  if (el && el.closest("input, textarea, [contenteditable]")) return;
  event.preventDefault();
});

// On Android the webview loads in parallel with the Rust setup hook, so an
// invoke fired at mount can land before managed state exists and fail with a
// raw "state not managed" error (desktop too: its window is up while the
// database work still runs on a worker). Poll the stateless app_ready probe
// until setup has finished.
// A rejection is a retry, not a mount: on Android the first invokes can fail
// while the IPC bridge itself is still coming up, and mounting on that error
// reintroduces the exact race this gate exists to prevent (seen in the field
// on a fresh v0.1.5 install). Scripted checks stay instant because their
// stubbed invoke resolves app_ready as true (fixtures.js carries the entry).
// Only the deadline is fail-open: mounting and surfacing real command errors
// beats an unexplained blank screen when the backend is genuinely broken.
//
// Each probe has its own timeout so the deadline can fire: an answer that never
// arrives would otherwise hold the loop past it, and the screen would stay
// blank instead of mounting.
//
// It was written for a defect in tao's Android event loop: a reply queued
// before `event_loop.run()` was delivered only when a later message flushed
// it, so posting the next probe was what made the previous answer arrive
// (Galaxy A22, Android 13, fresh data dir: the first answer reached the webview
// at +6.069s, 10 ms after the SECOND probe was posted). tao 0.37 fixes it at
// the source — the loop drains user events on every poll (tao#1304) — and the
// setup hook returning at once had already put the loop up before the webview
// exists (src-tauri/src/lib.rs).
const PROBE_TIMEOUT_MS = 2000;

function probeReady() {
  return Promise.race([
    invoke("app_ready"),
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error("app_ready timed out")), PROBE_TIMEOUT_MS),
    ),
  ]);
}

async function waitForBackend() {
  const deadline = Date.now() + 30000;
  for (;;) {
    try {
      if (await probeReady()) return;
    } catch {
      // Not ready, IPC not up yet, or the answer is still parked — all retries.
    }
    if (Date.now() >= deadline) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
await waitForBackend();

// Warm the session-wide reference lists while the first view renders. Not
// awaited: they are small and every view that needs them awaits the same
// promise, so blocking the mount on twenty-odd tiny queries would delay the
// first paint to no purpose (lib/lookups.svelte.js).
loadLookups().catch(() => {
  // A failed warm-up is not a startup failure — the first view that needs the
  // lists retries and surfaces the error through its own run() wrapper.
});

// Svelte 5's mount() appends, so the boot spinner (src/index.html) has to go
// first or it would sit above the app for the rest of the session.
const target = document.getElementById("app");
target.replaceChildren();
mount(App, { target });
