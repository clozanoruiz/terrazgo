// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The About panel's third-party list against the manifests it describes.
//!
//! `src/lib/thirdParty.js` is hand-written — one row per PROJECT, which no
//! machine can group for us — and an inventory with nothing checking it is the
//! failure the catalogue audit already found once. So three things are checked
//! here, and each is a way the list could quietly stop being true:
//!
//! 1. every distributed dependency the manifests name appears in the list, so
//!    adding a library fails until it is attributed;
//! 2. every package the list names still exists in a manifest, so removing one
//!    fails until the row goes too;
//! 3. every licence the list names resolves to an allowlisted link, so a new
//!    licence cannot render a link that opens nothing.
//!
//! **Distributed is the test, not "declared".** `[dev-dependencies]` are never
//! shipped and `[build-dependencies]` run at build time without their code
//! reaching the binary, so neither is ours to attribute and neither is
//! required here. npm's `dependencies` all ship; its `devDependencies` are
//! allowed in the list but never demanded, because `svelte` is one and the
//! runtime its compiler emits is in the bundle.
//!
//! Hand-rolled readers for both manifest formats, the `i18n_contract.rs`
//! arrangement and for the same reason: it keeps the test self-contained Rust
//! with no new crates and no Node invocation.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use terrazgo::external_links;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Our own crates, which are not third-party and are never listed.
fn is_ours(name: &str) -> bool {
    name.starts_with("terrazgo-") || name.starts_with("module-") || name == "terrazgo"
}

// --- the Cargo side ----------------------------------------------------------

/// Dependency-section keys of one manifest, excluding dev and build sections.
///
/// A section header is a line that is exactly `[…]`; a dependency is a key at
/// the start of a line while no inline table or array is open. Depth tracking
/// is what keeps a multi-line `features = [ … ]` from contributing its own
/// contents as dependency names.
fn cargo_dependencies(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut in_deps = false;
    let mut depth: i32 = 0;

    for line in text.lines() {
        let trimmed = line.trim();

        if depth == 0 && trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed.trim_matches(['[', ']']);
            // `[dependencies]` and `[target.'cfg(…)'.dependencies]` count;
            // `[dev-dependencies]` and `[build-dependencies]` do not.
            in_deps = section.ends_with("dependencies")
                && !section.ends_with("dev-dependencies")
                && !section.ends_with("build-dependencies")
                // `[workspace.dependencies]` only carries versions; what a
                // crate actually uses is its own section.
                && section != "workspace.dependencies";
            continue;
        }

        if in_deps
            && depth == 0
            && !trimmed.starts_with('#')
            && let Some((key, _)) = trimmed.split_once('=')
        {
            let key = key.trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                found.insert(key.to_string());
            }
        }

        depth += line.matches(['{', '[']).count() as i32;
        depth -= line.matches(['}', ']']).count() as i32;
        // A section header is a balanced `[…]`, so it nets to zero; anything
        // negative would mean a stray closer, which TOML would reject anyway.
        depth = depth.max(0);
    }
    found
}

/// Every crate manifest in the workspace: the members, plus the root (which
/// declares no dependencies of its own but is cheap to include).
fn manifests() -> Vec<PathBuf> {
    let root = repo_root();
    let mut paths = vec![root.join("Cargo.toml"), root.join("src-tauri/Cargo.toml")];
    let crates = root.join("crates");
    let mut members: Vec<PathBuf> = fs::read_dir(&crates)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path().join("Cargo.toml");
            path.is_file().then_some(path)
        })
        .collect();
    members.sort();
    paths.extend(members);
    paths
}

/// Third-party crates our own crates depend on and that ship in the binary.
fn distributed_crates() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in manifests() {
        let text = fs::read_to_string(&path).unwrap();
        for name in cargo_dependencies(&text) {
            if !is_ours(&name) {
                found.insert(name);
            }
        }
    }
    found
}

/// Every crate named anywhere in the manifests, dev and build sections
/// included — used only to check that a listed package has not vanished.
fn any_crate_named() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in manifests() {
        let text = fs::read_to_string(&path).unwrap();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with('[') {
                continue;
            }
            if let Some((key, _)) = trimmed.split_once('=') {
                let key = key.trim();
                if !key.is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    found.insert(key.to_string());
                }
            }
        }
    }
    found
}

// --- the npm side ------------------------------------------------------------

/// The keys of one top-level object in package.json, by name.
///
/// package.json is flat enough that finding the section and reading quoted
/// keys until its closing brace is the whole job.
fn npm_section(text: &str, section: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let needle = format!("\"{section}\"");
    let Some(start) = text.find(&needle) else {
        return found;
    };
    let Some(open) = text[start..].find('{') else {
        return found;
    };
    let body = &text[start + open + 1..];
    let end = body.find('}').unwrap_or(body.len());
    for line in body[..end].lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('"')
            && let Some((key, _)) = rest.split_once('"')
        {
            found.insert(key.to_string());
        }
    }
    found
}

fn package_json() -> String {
    fs::read_to_string(repo_root().join("package.json")).unwrap()
}

// --- the list under test -----------------------------------------------------

/// One row of THIRD_PARTY, as text: everything from its `name:` to the next.
///
/// Entry-based rather than a global scan for `packages:` arrays, because a
/// `kind: "bundled"` row has to be told apart from the rest — SQLite and the
/// embedded fonts are in the binary without being packages, so no manifest and
/// no lockfile will ever name them.
fn entries(js: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut starts: Vec<usize> = js.match_indices("name: \"").map(|(i, _)| i).collect();
    starts.push(js.len());
    for pair in starts.windows(2) {
        out.push(&js[pair[0]..pair[1]]);
    }
    out
}

fn is_bundled(entry: &str) -> bool {
    entry.contains("kind: \"bundled\"")
}

/// Every string inside a `packages: [ … ]` array in thirdParty.js.
fn listed_packages(js: &str) -> BTreeSet<String> {
    collect_arrays(js, "packages:")
}

/// Packages a manifest is expected to name — everything except the bundled
/// components, which are compiled in rather than depended on.
fn packaged(js: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for entry in entries(js) {
        if !is_bundled(entry) {
            out.extend(collect_arrays(entry, "packages:"));
        }
    }
    out
}

/// Every string inside a `licences: [ … ]` array in thirdParty.js.
fn listed_licences(js: &str) -> BTreeSet<String> {
    collect_arrays(js, "licences:")
}

fn collect_arrays(js: &str, key: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = js;
    while let Some(at) = rest.find(key) {
        rest = &rest[at + key.len()..];
        let Some(open) = rest.find('[') else { break };
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        for part in rest[open + 1..open + close].split(',') {
            let part = part.trim().trim_matches('"');
            if !part.is_empty() {
                found.insert(part.to_string());
            }
        }
        rest = &rest[open + close..];
    }
    found
}

fn third_party_js() -> String {
    fs::read_to_string(repo_root().join("src/lib/thirdParty.js")).unwrap()
}

/// The frontend's `licenceLinkId`, in Rust. Kept in step by
/// `derives_the_same_link_id_as_the_frontend` below rather than by review.
fn licence_link_id(spdx: &str) -> String {
    format!(
        "spdx_{}",
        spdx.to_ascii_lowercase().replace(['.', '-'], "_")
    )
}

// --- the contracts -----------------------------------------------------------

#[test]
fn every_distributed_dependency_is_attributed() {
    let listed = listed_packages(&third_party_js());
    let missing: Vec<String> = distributed_crates()
        .into_iter()
        .filter(|name| !listed.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "\nCrates that ship but are not in src/lib/thirdParty.js:\n  {}\n\
         Add each to a row (a new project row, or the packages of an existing \
         one) so the About panel attributes it.\n",
        missing.join("\n  ")
    );

    let npm = npm_section(&package_json(), "dependencies");
    let missing: Vec<String> = npm
        .into_iter()
        .filter(|name| !listed.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "\nnpm dependencies not in src/lib/thirdParty.js:\n  {}\n",
        missing.join("\n  ")
    );
}

#[test]
fn every_listed_package_still_exists() {
    let js = third_party_js();
    let text = package_json();
    let crates = any_crate_named();
    let npm: BTreeSet<String> = npm_section(&text, "dependencies")
        .union(&npm_section(&text, "devDependencies"))
        .cloned()
        .collect();

    let gone: Vec<String> = packaged(&js)
        .into_iter()
        .filter(|name| !crates.contains(name) && !npm.contains(name))
        .collect();
    assert!(
        gone.is_empty(),
        "\nPackages listed in src/lib/thirdParty.js that no manifest names:\n  {}\n\
         A dependency that is gone must lose its row too — the panel would be \
         attributing code the app no longer ships.\n",
        gone.join("\n  ")
    );
}

#[test]
fn every_licence_has_an_allowlisted_link() {
    let ids: BTreeSet<&str> = external_links::link_ids().collect();
    for spdx in listed_licences(&third_party_js()) {
        let id = licence_link_id(&spdx);
        assert!(
            ids.contains(id.as_str()),
            "licence {spdx} needs an (\"{id}\", \"https://spdx.org/licenses/{spdx}.html\") \
             entry in src-tauri/src/external_links.rs, or its link opens nothing"
        );
    }
}

/// The generated licence texts against the list they were generated from.
///
/// `thirdPartyLicences.json` is committed so a build never has to reach the
/// cargo registry or node_modules, and so the app can show its own attribution
/// offline. The cost of committing it is that it can fall behind the list — a
/// dependency added without re-running `npm run gen:licences` would appear in
/// the panel with no licence text at all, which is a silent failure of the one
/// thing that tab exists to do.
#[test]
fn the_generated_licence_texts_cover_every_listed_package() {
    let raw = fs::read_to_string(repo_root().join("src/lib/thirdPartyLicences.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut licences: BTreeSet<String> = BTreeSet::new();
    for licence in data["licences"].as_array().unwrap() {
        licences.insert(licence["spdx"].as_str().unwrap().to_string());
        for entry in licence["texts"].as_array().unwrap() {
            for pkg in entry["packages"].as_array().unwrap() {
                covered.insert(pkg.as_str().unwrap().to_string());
            }
        }
    }

    let js = third_party_js();
    let missing: Vec<String> = listed_packages(&js)
        .into_iter()
        .filter(|name| !covered.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "\nPackages with no licence text in src/lib/thirdPartyLicences.json:\n  {}\n\
         Run `npm run gen:licences` — the generated file is behind thirdParty.js.\n",
        missing.join("\n  ")
    );

    // …and the other direction: every licence the list claims must have a text,
    // or a row offers the reader an option they cannot read.
    let claimed = listed_licences(&js);
    let unevidenced: Vec<String> = claimed.difference(&licences).cloned().collect();
    assert!(
        unevidenced.is_empty(),
        "\nLicences named in thirdParty.js with no text generated:\n  {}\n\
         Run `npm run gen:licences`; if it refuses, it will say which package \
         ships no text for them.\n",
        unevidenced.join("\n  ")
    );
}

/// Versions in the lockfiles against the versions the texts were read from.
///
/// The gap the other tests leave open: UPGRADING a dependency changes nothing
/// they look at. The package is still listed, still covered, still linked — and
/// the licence text in the panel is whatever the old version said, which is
/// wrong the moment a copyright year moves or a project re-licences. A version
/// is the cheap proxy for "the file may have changed", so the generator records
/// what it read and this compares.
#[test]
fn the_generated_licence_texts_were_read_from_the_installed_versions() {
    let raw = fs::read_to_string(repo_root().join("src/lib/thirdPartyLicences.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let recorded = data["versions"].as_object().unwrap();
    // A bundled component's version is read from the crate that carries it —
    // SQLite's from libsqlite3-sys — so resolve through that before looking a
    // name up in a lockfile that has never heard of "sqlite3".
    let source = data["versionSource"].as_object().unwrap();

    let cargo_lock = fs::read_to_string(repo_root().join("Cargo.lock")).unwrap();
    let npm_lock = fs::read_to_string(repo_root().join("package-lock.json")).unwrap();
    let npm: serde_json::Value = serde_json::from_str(&npm_lock).unwrap();

    let mut stale = Vec::new();
    for (pkg, was) in recorded {
        let was = was.as_str().unwrap_or_default();
        let lock_name = source
            .get(pkg)
            .and_then(|v| v.as_str())
            .unwrap_or(pkg.as_str());
        let now = if let Some(entry) = npm["packages"].get(format!("node_modules/{lock_name}")) {
            entry["version"].as_str().unwrap_or_default().to_string()
        } else {
            cargo_lock_version(&cargo_lock, lock_name).unwrap_or_default()
        };
        if now.is_empty() {
            stale.push(format!("{pkg}: recorded {was}, but no lockfile names it"));
        } else if was.is_empty() {
            // A bundled component with no `versionFrom`: nothing tracks it but
            // our own repository, where the licence file sits beside the bytes.
        } else if now != was {
            stale.push(format!("{pkg}: {was} → {now}"));
        }
    }

    assert!(
        stale.is_empty(),
        "\nLicence texts were read from other versions than the ones installed:\n  {}\n\
         Run `npm run gen:licences`. An upgrade can change a licence file — a \
         copyright year, a re-licence — and nothing else here would notice.\n",
        stale.join("\n  ")
    );
}

/// The `version` of a `[[package]]` block in Cargo.lock, by name.
///
/// Read positionally rather than with a TOML parser: the block is always
/// `name = "…"` immediately followed by `version = "…"`, and a dependency for a
/// parser this small is not worth taking (the `i18n_contract.rs` reasoning).
fn cargo_lock_version(lock: &str, name: &str) -> Option<String> {
    let needle = format!("\nname = \"{name}\"\n");
    let at = lock.find(&needle)?;
    let rest = &lock[at + needle.len()..];
    let line = rest.lines().next()?;
    let value = line.strip_prefix("version = \"")?;
    Some(value.trim_end_matches('"').to_string())
}

#[test]
fn derives_the_same_link_id_as_the_frontend() {
    // The two implementations are three lines each and live in different
    // languages; what keeps them together is this pair of cases, taken from the
    // shapes actually in use — a dotted version, a hyphenated name, a bare word.
    assert_eq!(licence_link_id("MIT"), "spdx_mit");
    assert_eq!(licence_link_id("Apache-2.0"), "spdx_apache_2_0");
    assert_eq!(licence_link_id("BSD-3-Clause"), "spdx_bsd_3_clause");
    assert_eq!(licence_link_id("Unicode-3.0"), "spdx_unicode_3_0");
}

#[test]
fn the_reader_finds_the_dependencies_that_are_really_there() {
    // The parser is hand-rolled, so it gets its own case: a guard that silently
    // read nothing would pass every test above while checking nothing.
    let found = distributed_crates();
    assert!(found.contains("tauri"), "{found:?}");
    assert!(found.contains("rusqlite"), "{found:?}");
    // Target-gated, so it proves `[target.'cfg(…)'.dependencies]` is read.
    assert!(found.contains("tauri-plugin-geolocation"), "{found:?}");
    // Build-only and dev-only, so neither is demanded of the list.
    assert!(!found.contains("tauri-build"), "build-dependency leaked in");
    assert!(
        !found.contains("terrazgo-testkit"),
        "dev-dependency leaked in"
    );
    assert!(!found.contains("jsonschema"), "dev-dependency leaked in");
    // A `features = [ … ]` value must never be read as a dependency name.
    assert!(!found.contains("bundled"), "an inline array leaked in");
    assert!(!found.contains("version"), "an inline table leaked in");
}
