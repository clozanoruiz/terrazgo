// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Frontend entry: normalise the route, then mount the Svelte app. The module
// graph waits on i18n.js's top-level await, so t() is synchronous everywhere
// by the time any component renders.

import { mount } from "svelte";
import App from "./App.svelte";
import { invoke } from "./lib/backend.js";

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
// Fail-open on invoke errors and after the bound: mounting and surfacing real
// command errors beats an unexplained blank screen — and in scripted checks
// the stubbed invoke has no app_ready, which lands in the catch on try one.
async function waitForBackend() {
  const deadline = Date.now() + 15000;
  for (;;) {
    try {
      if (await invoke("app_ready")) return;
    } catch {
      return;
    }
    if (Date.now() >= deadline) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
await waitForBackend();

mount(App, { target: document.getElementById("app") });
