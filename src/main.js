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
// raw "state not managed" error (desktop never races: its window is created
// after setup). Poll the stateless app_ready probe until setup has finished.
// A rejection is a retry, not a mount: on Android the first invokes can fail
// while the IPC bridge itself is still coming up, and mounting on that error
// reintroduces the exact race this gate exists to prevent (seen in the field
// on a fresh v0.1.5 install). Scripted checks stay instant because their
// stubbed invoke resolves app_ready as true (fixtures.js carries the entry).
// Only the deadline is fail-open: mounting and surfacing real command errors
// beats an unexplained blank screen when the backend is genuinely broken.
//
// The timeout-and-retry below is now a BELT, not the fix. The blank first
// launch it was written for is addressed at the source: the Rust setup hook
// returns immediately and does its work on a worker, so the event loop is
// running before the webview exists (src-tauri/src/lib.rs). Keeping the retry
// costs one timer and covers the case the loop could not: a reply that is
// queued before the loop starts is not delivered when the loop starts, only
// when a later message flushes it — so posting the next probe unconditionally
// is what makes the previous answer arrive.
//
// The underlying defect is in tao, not in this app and not in wry. Reproduced
// standalone with no wry and no Tauri: a user event sent through
// EventLoopProxy before event_loop.run() sat undelivered until an unrelated
// event arrived 5 s later, on tao 0.35.3 and 0.36.0 alike, while the same
// pattern delivers on desktop. It needs concurrent ndk_glue traffic to bite,
// which an activity starting up always produces.
//
// Measured on-device before the fix (Galaxy A22, Android 13, WebView 151,
// fresh data dir): Rust executed the first `app_ready` at +0.000s and returned
// false; the setup hook finished at +0.670s; the webview did not receive that
// first answer until +6.069s — 10 ms after the SECOND probe was posted.
// Widening this constant from 2s to 6s moved the first answer from +2.07s to
// +6.07s in step, which is what identified the trigger.
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
