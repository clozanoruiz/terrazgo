// SPDX-License-Identifier: AGPL-3.0-or-later

// The Tauri invoke handle and boundary-error rendering, shared by every view.
// Deliberately a plain module (no Svelte imports): together with i18n.js and
// the dictionaries it forms the framework-agnostic layer that would survive a
// UI-framework swap untouched.
//
// The Tauri API is injected globally because tauri.conf.json sets
// `withGlobalTauri: true` — no dependency on @tauri-apps/api needed.

import { has, t } from "../i18n.js";

export const { invoke } = window.__TAURI__.core;

// The geo:// protocol origin in its platform form (Linux/macOS
// `geo://localhost/`, Windows/Android `http://geo.localhost/`). Rust builds
// map styles against this base so the webview only ever loads geo:// URLs.
export function geoBase() {
  const base = window.__TAURI__.core.convertFileSrc("", "geo");
  return base.endsWith("/") ? base : `${base}/`;
}

// Native yes/no confirmation via the dialog plugin (same invoke transport as
// the save/open dialogs in StatusView). Used instead of window.confirm():
// blocking JS dialogs are not reliably supported by the mobile webviews, and
// the native dialog matches the platform. Resolves to a boolean.
//
// tauri-plugin-dialog 2.7.0 removed the Rust-side `confirm` command; the one
// dialog command is `message`, parameterised by `buttons`. This mirrors what
// the official JS wrapper's confirm() sends: OkCancel buttons, and the
// dialog resolves to the name of the pressed button ("Ok"/"Cancel").
export async function confirmDialog(message) {
  const result = await invoke("plugin:dialog|message", { message, buttons: "OkCancel" });
  return result === "Ok";
}

// Boundary errors arrive as { code, params, message } (CommandError in
// src-tauri/src/commands.rs). A code with a dictionary entry renders
// localized; anything else falls back to the untranslated developer message
// so nothing is swallowed. "internal" (which deliberately has no error.internal
// entry — see tests/i18n_contract.rs) gets a localized intro line in front of
// that raw message, so regular users at least learn what kind of failure it is.
export function errorText(err) {
  if (err && typeof err === "object" && err.code) {
    const key = `error.${err.code}`;
    if (has(key)) return t(key, err.params ?? {});
    const raw = err.message ?? String(err);
    if (err.code === "internal") return `${t("error.internal_intro")} ${raw}`;
    return raw;
  }
  return String(err);
}
