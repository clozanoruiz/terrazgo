// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Builds src/lib/thirdPartyLicences.json — the licence texts the About panel
// shows — from the licence files the packages themselves ship.
//
// Run with `npm run gen:licences` after adding, removing or upgrading a
// dependency. The OUTPUT is committed: a build must not depend on the cargo
// registry being populated or on node_modules being installed, and a farmer's
// copy of the app has to be able to show its own attribution offline.
//
// WHY A SCRIPT WE OWN rather than cargo-about: two ecosystems. cargo-about
// covers the crates and knows nothing about the seven npm packages, half of
// which (MapLibre, Svelte, Bits UI) are the most visible things in the app. It
// remains the right tool for a complete TRANSITIVE notice at release time —
// this is the direct-dependency list the panel shows, which is a different
// artifact (see docs/frontend-conventions.md).
//
// GROUPED BY LICENCE, THEN BY TEXT. Apache-2.0's body is boilerplate, so many
// packages share one text; MIT embeds its copyright line, so 16 of our
// packages have 16 different MIT texts (measured 2026-09-04). Grouping by
// licence alone would therefore attribute fifteen of them to one wrong holder,
// which is why the second level exists.
//
// ONE LICENCE PER LIBRARY. A dual-licensed package ("MIT OR Apache-2.0") is
// shown under the option Terrazgo takes, never under both — `licenceShown` in
// src/lib/thirdParty.js decides which, and says why that is enough.
//
// A package that ships no licence file falls back to third-party/, and one
// with neither is a hard error — see that directory's README.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { THIRD_PARTY, licenceShown } from "../src/lib/thirdParty.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(root, "src/lib/thirdPartyLicences.json");

/// Packages whose text is vendored because they publish none. The key is the
/// file in third-party/; the value is every package it covers.
const VENDORED = {
  "terra-draw": ["terra-draw", "terra-draw-maplibre-gl-adapter"],
  // Dual-licensed and ships neither half; only the MIT one is shown.
  geozero: ["geozero"],
  rusqlite_migration: ["rusqlite_migration"],
  typst: ["typst", "typst-layout", "typst-pdf"],
};

/// Which SPDX id a licence file states, decided by the phrase that only that
/// licence uses. Order matters: Unicode-3.0 and ISC both contain a
/// permission-grant sentence close to MIT's, so they are tested first.
function classify(text) {
  const t = text.replace(/\s+/g, " ").toLowerCase();
  if (t.includes("sil open font license") || t.includes("open font license version 1.1"))
    return "OFL-1.1";
  if (t.includes("in place of a legal notice") && t.includes("blessing")) return "blessing";
  if (t.includes("unicode license v3") || t.includes("unicode data files")) return "Unicode-3.0";
  if (t.includes("apache license") && t.includes("version 2.0")) return "Apache-2.0";
  if (t.includes("this is free and unencumbered software released into the public domain"))
    return "Unlicense";
  if (t.includes("permission to use, copy, modify, and/or distribute")) return "ISC";
  if (t.includes("redistribution and use in source and binary forms")) return "BSD-3-Clause";
  if (t.includes("permission is hereby granted, free of charge")) return "MIT";
  return null;
}

function licenceFiles(dir) {
  if (!dir || !fs.existsSync(dir)) return [];
  return (
    fs
      .readdirSync(dir)
      // UNLICENSE is not a LICENSE prefix — csv ships one and it was missed.
      .filter((f) => /^(UN)?LICEN[CS]E|^COPYING/i.test(f) && !f.endsWith(".spdx"))
      .map((f) => path.join(dir, f))
      .filter((f) => fs.statSync(f).isFile())
      .sort()
  );
}

// --- where each package's source lives ---------------------------------------

function cargoDirs() {
  const meta = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version", "1", "--all-features"], {
      cwd: root,
      maxBuffer: 64 * 1024 * 1024,
      encoding: "utf8",
    }),
  );
  const dirs = new Map();
  for (const p of meta.packages) {
    if (!dirs.has(p.name))
      dirs.set(p.name, { dir: path.dirname(p.manifest_path), version: p.version });
  }
  return dirs;
}

/// The installed version of an npm package, read from what is on disk rather
/// than from the range in package.json.
function npmVersion(pkg) {
  const manifest = path.join(root, "node_modules", pkg, "package.json");
  if (!fs.existsSync(manifest)) return null;
  return JSON.parse(fs.readFileSync(manifest, "utf8")).version ?? null;
}

// --- collect ------------------------------------------------------------------

const listed = THIRD_PARTY;

const crateDirs = cargoDirs();
const vendoredFor = new Map();
for (const [file, pkgs] of Object.entries(VENDORED)) {
  for (const pkg of pkgs) vendoredFor.set(pkg, path.join(root, "third-party", `${file}.txt`));
}

/// spdx -> sha256 -> { text, packages: Set }
const buckets = new Map();
const problems = [];
/// The version each text was read from. A licence file changes with the
/// package — a new copyright year, a re-licence — and nothing else here would
/// notice, because the package is still listed and still covered. The Rust
/// contract test compares these against the lockfiles, so a version bump fails
/// until this file is regenerated.
const versions = {};
/// Where a package's version is READ from, when that is not the package itself:
/// SQLite is compiled in through libsqlite3-sys and has no lockfile entry of
/// its own. The contract test resolves through this before checking a version.
const versionSource = {};

for (const lib of listed) {
  let shown;
  try {
    shown = licenceShown(lib);
  } catch (err) {
    problems.push(err.message);
    continue;
  }
  for (const pkg of lib.packages) {
    const crate = lib.kind === "rust" ? crateDirs.get(pkg) : null;
    // A bundled component is in the binary without being a package: no
    // directory to scan and no lockfile entry of its own, so it names the file
    // its licence is read from and, where one exists, the crate whose version
    // tracks it (SQLite moves with libsqlite3-sys; the fonts are ours and move
    // only when we replace them).
    let files;
    if (lib.kind === "bundled") {
      files = [path.join(root, lib.licenceFile)];
      const from = lib.versionFrom ? crateDirs.get(lib.versionFrom) : null;
      if (from) {
        versions[pkg] = from.version;
        versionSource[pkg] = lib.versionFrom;
      }
    } else {
      const dir = lib.kind === "rust" ? crate?.dir : path.join(root, "node_modules", pkg);
      versions[pkg] = lib.kind === "rust" ? crate?.version : npmVersion(pkg);
      files = licenceFiles(dir);
      if (files.length === 0 && vendoredFor.has(pkg)) files = [vendoredFor.get(pkg)];
    }
    if (files.length === 0) {
      problems.push(
        `${pkg}: ships no licence file and none is vendored. Take its licence from the ` +
          `project's own repository into third-party/, add it to VENDORED above, and ` +
          `record where it came from in third-party/README.md.`,
      );
      continue;
    }
    let found = false;
    for (const file of files) {
      const text = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n").trimEnd();
      const spdx = classify(text);
      // Only the licence the row is shown under. A dual-licensed crate's other
      // file (LICENSE-APACHE beside LICENSE-MIT) is the option not taken, and
      // csv and jiff also ship a COPYING that only explains the choice.
      if (spdx !== shown) continue;
      found = true;
      // Keyed on the WORDS, not the bytes: several packages ship the same
      // licence wrapped differently (301 "changed" lines between two
      // Apache-2.0 files that read identically, measured 2026-09-04). One
      // of them is then displayed verbatim; a text that really differs —
      // a filled-in copyright line, a missing appendix — still keys apart.
      const key = createHash("sha256").update(text.replace(/\s+/g, " ").trim()).digest("hex");
      if (!buckets.has(spdx)) buckets.set(spdx, new Map());
      const texts = buckets.get(spdx);
      if (!texts.has(key)) texts.set(key, { text, packages: new Set() });
      texts.get(key).packages.add(pkg);
    }
    if (!found) {
      problems.push(
        `${pkg}: shown under ${shown} but ships no ${shown} text ` +
          `(found: ${files.map((f) => path.basename(f)).join(", ") || "nothing"}).`,
      );
    }
  }
}

if (problems.length) {
  console.error(`\nRefusing to generate:\n\n  ${problems.join("\n  ")}\n`);
  process.exit(1);
}

// --- emit ---------------------------------------------------------------------
//
// Licences in the order thirdParty.js first shows them — the order they were
// first put in a bucket — and inside each, the text shared by the most packages
// first, so the boilerplate one leads and the single-package notices follow,
// which is the order a reader scans.

// Package -> the project row it belongs to, so a box can be headed with the
// names a reader recognises ("Tauri") while the guard still works on the
// package names underneath ("tauri-plugin-geolocation").
const projectOf = new Map();
for (const lib of listed) for (const pkg of lib.packages) projectOf.set(pkg, lib.name);

const out = {
  // Regenerate with `npm run gen:licences`.
  generator: "scripts/gen-third-party.mjs",
  versions,
  versionSource,
  licences: [...buckets.keys()].map((spdx) => ({
    spdx,
    texts: [...buckets.get(spdx).values()]
      .map((entry) => {
        const packages = [...entry.packages].sort();
        const projects = [];
        for (const pkg of packages) {
          const name = projectOf.get(pkg);
          if (!projects.includes(name)) projects.push(name);
        }
        return { projects, packages, text: entry.text };
      })
      .sort(
        (a, b) =>
          b.packages.length - a.packages.length || a.packages[0].localeCompare(b.packages[0]),
      ),
  })),
};

// Two-space indent because that is what prettier expects, and this file is
// committed: an indent of 1 saved 330 bytes and cost a red `format:check`.
fs.writeFileSync(OUT, `${JSON.stringify(out, null, 2)}\n`);

const texts = out.licences.reduce((n, l) => n + l.texts.length, 0);
console.log(
  `${path.relative(root, OUT)}: ${out.licences.length} licences, ${texts} distinct texts, ` +
    `${(fs.statSync(OUT).size / 1024).toFixed(1)} KB`,
);
