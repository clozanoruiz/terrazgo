// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The third-party libraries this app is built on, for the About panel.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
//
// WHAT IS LISTED: the libraries we CHOSE and that ship inside the app — the
// direct dependencies of our own crates, and the npm packages whose code ends
// up in the bundle. Not the resolved graph: that is 743 crates, nearly all of
// them pulled in by something else, and a panel a farmer opens is not where a
// person reads 743 rows. Not the build and test tooling either (vite, eslint,
// prettier, vitest, cargo-deny, jsonschema, zip): none of it is distributed,
// so none of it is ours to attribute.
//
// `svelte` is the one npm entry that is a devDependency: the compiler runs at
// build time, but the runtime it emits is in the bundle we ship.
//
// AND WHAT IS IN THE BINARY WITHOUT BEING A PACKAGE: SQLite's amalgamation,
// compiled in through rusqlite's `bundled` feature, and the four Liberation
// Sans faces terrazgo-report embeds with `include_bytes!`. Those carry
// `kind: "bundled"` and name the file their licence is read from, because no
// manifest and no lockfile will ever mention them.
//
// ONE ROW IS ONE PROJECT, not one package. A reader recognises "Tauri", not
// `tauri-plugin-geolocation`, so the packages of one project share a row and
// the row names them. **Nothing is dropped by that** — every shipped package
// is still represented, which is the point: MIT, BSD-3-Clause, ISC and
// Apache-2.0 all require the notice to travel with the distribution, so a list
// curated for brevity would stop being the thing the licences ask for.
//
// A row states ONE licence, so packages only share a row when they share it.
// That is why `tao` sits apart from Tauri though it is the same project's
// crate: it is Apache-2.0 alone where the rest offer MIT as well, and a row
// saying otherwise would be a false statement about somebody's licence.
//
// THIS LIST CANNOT SILENTLY ROT. `src-tauri/tests/third_party.rs` reads the
// Cargo and npm manifests and refuses a dependency that is missing here, an
// entry naming a package that is gone, and a licence with no allowlisted link.
// A hand-written inventory with nothing checking it is exactly the failure the
// catalogue audit found (docs/maintenance.md §1).
//
// A dual licence is stored as the alternatives it offers, in the order the
// package states them: "MIT OR Apache-2.0" means the reader may take either.

export const THIRD_PARTY = [
  // --- Rust -----------------------------------------------------------------
  {
    name: "Tauri",
    kind: "rust",
    licences: ["Apache-2.0", "MIT"],
    packages: [
      "tauri",
      "tauri-plugin-dialog",
      "tauri-plugin-fs",
      "tauri-plugin-geolocation",
      "tauri-plugin-opener",
    ],
  },
  // Tauri's own windowing crate, and Apache-2.0 alone — see the note above.
  { name: "tao", kind: "rust", licences: ["Apache-2.0"], packages: ["tao"] },
  {
    name: "Typst",
    kind: "rust",
    licences: ["Apache-2.0"],
    packages: ["typst", "typst-layout", "typst-pdf"],
  },
  { name: "typst-as-lib", kind: "rust", licences: ["MIT"], packages: ["typst-as-lib"] },
  {
    name: "Serde",
    kind: "rust",
    licences: ["MIT", "Apache-2.0"],
    packages: ["serde", "serde_json"],
  },
  {
    name: "ICU4X",
    kind: "rust",
    licences: ["Unicode-3.0"],
    packages: ["icu_collator", "icu_locale_core"],
  },
  { name: "rusqlite", kind: "rust", licences: ["MIT"], packages: ["rusqlite"] },
  {
    name: "rusqlite_migration",
    kind: "rust",
    licences: ["Apache-2.0"],
    packages: ["rusqlite_migration"],
  },
  { name: "anyhow", kind: "rust", licences: ["MIT", "Apache-2.0"], packages: ["anyhow"] },
  { name: "thiserror", kind: "rust", licences: ["MIT", "Apache-2.0"], packages: ["thiserror"] },
  { name: "csv", kind: "rust", licences: ["Unlicense", "MIT"], packages: ["csv"] },
  { name: "jiff", kind: "rust", licences: ["Unlicense", "MIT"], packages: ["jiff"] },
  { name: "geozero", kind: "rust", licences: ["MIT", "Apache-2.0"], packages: ["geozero"] },
  { name: "jni", kind: "rust", licences: ["MIT", "Apache-2.0"], packages: ["jni"] },
  { name: "os_info", kind: "rust", licences: ["MIT"], packages: ["os_info"] },
  {
    name: "rust_xlsxwriter",
    kind: "rust",
    licences: ["MIT", "Apache-2.0"],
    packages: ["rust_xlsxwriter"],
  },
  {
    name: "rustls-platform-verifier",
    kind: "rust",
    licences: ["MIT", "Apache-2.0"],
    packages: ["rustls-platform-verifier"],
  },
  { name: "ureq", kind: "rust", licences: ["MIT", "Apache-2.0"], packages: ["ureq"] },
  { name: "uuid", kind: "rust", licences: ["Apache-2.0", "MIT"], packages: ["uuid"] },

  // --- JavaScript -----------------------------------------------------------
  { name: "Svelte", kind: "js", licences: ["MIT"], packages: ["svelte"] },
  { name: "Bits UI", kind: "js", licences: ["MIT"], packages: ["bits-ui"] },
  { name: "MapLibre GL JS", kind: "js", licences: ["BSD-3-Clause"], packages: ["maplibre-gl"] },
  {
    name: "Terra Draw",
    kind: "js",
    licences: ["MIT"],
    packages: ["terra-draw", "terra-draw-maplibre-gl-adapter"],
  },
  { name: "Lucide", kind: "js", licences: ["ISC"], packages: ["@lucide/svelte"] },
  {
    name: "@internationalized/date",
    kind: "js",
    licences: ["Apache-2.0"],
    packages: ["@internationalized/date"],
  },

  // --- bundled: in the binary, but not as a package ---------------------------
  //
  // Neither of these is a dependency any manifest names, and both are shipped
  // code all the same. They were missing from this list until 2026-09-04, which
  // is the failure mode of deriving an inventory from the dependency graph: it
  // sees what cargo resolves and not what the binary contains.
  {
    // The C amalgamation, compiled in through rusqlite's `bundled` feature and
    // reaching us via libsqlite3-sys — whose version is therefore SQLite's.
    name: "SQLite",
    kind: "bundled",
    licences: ["blessing"],
    packages: ["sqlite3"],
    licenceFile: "third-party/sqlite.txt",
    versionFrom: "libsqlite3-sys",
  },
  {
    // Four faces embedded with include_bytes! by terrazgo-report, ~1.6 MB of
    // the binary. The licence has always travelled beside the files; what was
    // missing was saying so anywhere a reader could see it.
    name: "Liberation Sans",
    kind: "bundled",
    licences: ["OFL-1.1"],
    packages: ["liberation-fonts"],
    licenceFile: "crates/terrazgo-report/fonts/LICENSE",
  },
  {
    // The UI typeface, one variable file in the bundle. It shares OFL-1.1 with
    // Liberation Sans but NOT the notice: the licence carries its holder's
    // copyright line inside it, so the About panel shows two texts under the
    // one licence heading. That is why the grouping is by licence and then by
    // text rather than by licence alone.
    //
    // The screen and the printed book use different faces on purpose — the
    // book stays on Liberation Sans for its Arial-compatible metrics, which
    // matter for a document rendered to an official model.
    name: "IBM Plex Sans",
    kind: "bundled",
    licences: ["OFL-1.1"],
    packages: ["ibm-plex-sans"],
    licenceFile: "src/fonts/LICENSE",
  },
];

/// The allowlisted link id for an SPDX licence id.
///
/// Derived rather than tabulated, so adding a licence is one word in the list
/// above plus one URL in `src-tauri/src/external_links.rs` — never a third
/// mapping to keep in step. The Rust side derives the same id from the same
/// rule, which is what its contract test compares.
export function licenceLinkId(spdx) {
  return `spdx_${spdx.toLowerCase().replace(/[.-]/g, "_")}`;
}

/// The libraries of one kind ("rust" | "js"), in the order declared.
export function librariesOfKind(kind) {
  return THIRD_PARTY.filter((lib) => lib.kind === kind);
}
