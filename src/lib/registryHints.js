// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Which official registry holds each external identifier — the single source
// of truth for the hint rendered under an identifier field, in the nav.js /
// mapLayers.js shape.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
//
// WHY THIS EXISTS. None of the Spanish registries publishes a machine
// interface, and scraping is not an acceptable one here, so the app cannot
// look a farmer's ROMA number up for them. What it can do is say which
// registry holds it and open that registry.
//
// WHY THERE ARE NO URLs IN THIS FILE. The Rust allowlist
// (src-tauri/src/external_links.rs) owns them. The webview passes a registry
// id to `open_external_link` and never a URL, which is what lets the opener
// plugin be registered with no `opener:allow-open-url` granted to the frontend.
// Keeping the URL in one place also means these two lists cannot disagree
// about a destination — only about an id, which src-tauri/tests/registry_hints.rs
// catches.
//
// Entry contract:
//   REGISTRY_HINTS[countryCode][fieldSlug] = registryId
//
//   countryCode — the farm's `country_code`, lowercase ("es"). A country with
//                 no entry simply gets no hints; there is nothing to fall back
//                 to, because a registry is a fact about one country's
//                 administration.
//   fieldSlug   — `<entity>.<field>`, matching the i18n label key the field
//                 already uses, so the call site names the thing it labels.
//   registryId  — an id in the Rust allowlist. The hint sentence is
//                 `registry.<id>_hint` and the button label
//                 `registry.<id>_open` in every dictionary.
//
// Registry proper nouns (ROMA, REGANIP, ROPO, SIGPAC) are an explicit
// exemption to the English-identifiers rule: they are names, not our design.
export const REGISTRY_HINTS = {
  es: {
    // The seven-part parcel reference gets ONE hint on its fieldset rather
    // than seven identical ones — the farmer looks the whole reference up in
    // a single visit to the visor.
    "plot.sigpac": "sigpac",
    "machinery.roma": "roma",
    "machinery.reganip": "reganip",
    // Not an identifier the farmer types: it annotates the inspection DATES,
    // pointing at where the ITV is carried out.
    "machinery.inspection": "regiteaf",
    // One registry, two fields: ROPO inscribes applicators and advisors alike.
    "operator.licence_number": "ropo",
    "advisor.registration_number": "ropo",
    "product.auth_number": "regfiweb",
    "premises.cadastral_reference": "catastro",
    // DELIBERATELY ABSENT — see external_links.rs for the full reasoning:
    // REGA, SIEX, MDF, RGSEAA, REGFER and NIMA have no stable public lookup
    // page, and `farm.rea` can never gain one, because REA is a per-community
    // registry and no user-facing string may name one community's service
    // (docs/siex-export.md → "the REA-first rule").
  },
};

/// The registry id annotating a field for a country, or null if there is none.
/// Both arguments are tolerated as null/undefined so a call site can pass a
/// farm's country before it has loaded.
export function registryHint(countryCode, fieldSlug) {
  if (!countryCode || !fieldSlug) return null;
  return REGISTRY_HINTS[countryCode]?.[fieldSlug] ?? null;
}
