// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The pages the app points AT but never talks to.
//!
//! Spain's agricultural registries publish no machine interface: ASPAFITOS is a
//! server-rendered ASP.NET app, REGMAQ-ROMA answers a form POST with HTML, and
//! ROPO's bulk download stopped updating in 2024. Scraping is not an acceptable
//! interface here, so the app cannot look a farmer's ROMA number up for them —
//! what it can do is say which registry holds it and open that registry.
//!
//! **This is not a network seam.** Nothing in this file fetches anything; it
//! hands a URL to the platform browser and forgets it. `terrazgo-net` remains
//! the one place the app itself speaks HTTP, and neither core nor any module
//! gains a dependency from this.
//!
//! **Rust owns the URLs, and that is what keeps the webview's permission
//! surface at zero.** The frontend names a link by id — never by URL — so it
//! cannot choose a destination, and `tauri-plugin-opener` is registered with no
//! `opener:allow-open-url` granted to the webview (the `user_files.rs`
//! precedent). `src/lib/registryHints.js` says which *field* earns which id and
//! holds no URLs of its own; `tests/registry_hints.rs` keeps the two lists in
//! step.
//!
//! Every URL below was checked and answered HTTP 200 — the registries and the
//! project's own pages on 2026-08-26, the SPDX licence pages on 2026-09-04.
//!
//! ## What is deliberately absent
//!
//! REGA, SIEX, MDF, RGSEAA, REGFER and NIMA have columns in the schema but no
//! entry here: none of them resolves to a stable public lookup page. Their
//! fields keep a bare label rather than a hint that leads nowhere.
//!
//! **`farm_es_extension.rea_code` can never gain one, and that is a rule.**
//! REA is a per-community registry — REACYL in Castilla y León, SIDEAC in
//! Catalunya, and so on — and `docs/siex-export.md` → "the REA-first rule"
//! forbids any user-facing string naming a single community's service. Linking
//! it would need the province → service map for seventeen communities that the
//! same doc declines to build and keep current.

/// Allowlisted outbound links, `(id, url)`.
///
/// Ids are stable and travel to the frontend, so treat one as public API: the
/// dictionaries derive `registry.<id>_hint` from it and `registryHints.js`
/// names it. Registry proper nouns (ROMA, REGANIP, ROPO, SIGPAC) are an
/// explicit exemption to the English-identifiers rule — they are names.
const LINKS: &[(&str, &str)] = &[
    // --- Spanish registries, in the order a farmer meets them in the app
    // Visor SIGPAC — the seven-part parcel reference on every plot.
    ("sigpac", "https://sigpac.mapa.gob.es/fega/visor/"),
    // REGMAQ, the Registro Oficial de Maquinaria Agrícola: mobile sprayers.
    ("roma", "https://servicio.mapa.gob.es/regmaq/buscar.wai"),
    // REGANIP: aircraft and fixed/semi-mobile installations. Complementary to
    // ROMA per equipment type, which is why machinery carries both columns.
    ("reganip", "https://servicio.mapa.gob.es/reganip/"),
    // REGITEAF: the inspection stations, i.e. where the ITV is done. This one
    // annotates the inspection DATES rather than an identifier the farmer types.
    ("regiteaf", "https://servicio.mapa.gob.es/regiteaf/"),
    // ROPO: applicator and advisor inscriptions alike — one registry, two
    // fields (`operator.licence_number`, `advisor.registration_number`).
    ("ropo", "https://servicio.mapa.gob.es/ropo/"),
    // The phytosanitary product register: the MAPA number printed on the label.
    ("regfiweb", "https://servicio.mapa.gob.es/regfiweb"),
    // Sede Electrónica del Catastro: the reference for a building or premises.
    ("catastro", "https://www.sedecatastro.gob.es/"),
    // --- the project's own pages, for the About panel
    ("homepage", "https://terrazgo.com"),
    ("privacy", "https://terrazgo.com/privacidad.html"),
    ("source", "https://github.com/clozanoruiz/terrazgo"),
    (
        "issues",
        "https://github.com/clozanoruiz/terrazgo/issues/new/choose",
    ),
    ("licence", "https://www.gnu.org/licenses/agpl-3.0.html"),
    // --- the licences of the libraries the About panel lists
    //
    // SPDX's canonical page rather than each library's own repository: the
    // panel names 34 libraries under SIX licences, so linking the licence
    // costs six entries here where linking each project would cost 34 — and
    // "a link to their licence" is answered better by the licence's own text
    // than by a repository the reader must then search.
    //
    // The id is derived from the SPDX identifier by `licenceLinkId` in
    // `src/lib/thirdParty.js`, and `tests/third_party.rs` checks that every
    // licence the panel names resolves to an entry here.
    ("spdx_mit", "https://spdx.org/licenses/MIT.html"),
    (
        "spdx_apache_2_0",
        "https://spdx.org/licenses/Apache-2.0.html",
    ),
    (
        "spdx_bsd_3_clause",
        "https://spdx.org/licenses/BSD-3-Clause.html",
    ),
    ("spdx_isc", "https://spdx.org/licenses/ISC.html"),
    (
        "spdx_unicode_3_0",
        "https://spdx.org/licenses/Unicode-3.0.html",
    ),
    ("spdx_unlicense", "https://spdx.org/licenses/Unlicense.html"),
    // Not package licences: the embedded fonts and the bundled SQLite
    // amalgamation, which ship inside the binary rather than as dependencies.
    ("spdx_ofl_1_1", "https://spdx.org/licenses/OFL-1.1.html"),
    ("spdx_blessing", "https://spdx.org/licenses/blessing.html"),
];

/// The URL an allowlisted id names, or `None` if the id is not on the list.
pub fn url_for(id: &str) -> Option<&'static str> {
    LINKS
        .iter()
        .find(|(link_id, _)| *link_id == id)
        .map(|(_, url)| *url)
}

/// Every allowlisted id. Used by the contract test that checks the frontend's
/// `registryHints.js` names nothing this file does not carry.
pub fn link_ids() -> impl Iterator<Item = &'static str> {
    LINKS.iter().map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_id_resolves_and_unknown_one_does_not() {
        assert_eq!(
            url_for("roma"),
            Some("https://servicio.mapa.gob.es/regmaq/buscar.wai")
        );
        assert_eq!(url_for("REGMAQ"), None, "lookup is exact, not fuzzy");
        assert_eq!(url_for(""), None);
    }

    #[test]
    fn ids_are_unique() {
        let mut seen: Vec<&str> = link_ids().collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "duplicate id in LINKS");
    }

    /// A relative or `javascript:` target handed to the platform opener is the
    /// one way this list could do harm, so the shape is asserted rather than
    /// trusted to review.
    #[test]
    fn every_url_is_absolute_https() {
        for (id, url) in LINKS {
            assert!(
                url.starts_with("https://"),
                "{id} is not an absolute https URL: {url}"
            );
        }
    }
}
