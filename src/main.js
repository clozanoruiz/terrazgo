// SPDX-License-Identifier: AGPL-3.0-or-later

// Frontend entry: normalise the route, then mount the Svelte app. The module
// graph waits on i18n.js's top-level await, so t() is synchronous everywhere
// by the time any component renders.

import { mount } from "svelte";
import App from "./App.svelte";

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

mount(App, { target: document.getElementById("app") });
