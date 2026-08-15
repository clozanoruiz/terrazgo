// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which languages a farm's record book may be printed in.
//!
//! Castilian is official across the whole state (CE art. 3.1), so it is always
//! offered; a co-official language is offered when the holding sits in a region
//! where it is co-official, per that region's statute of autonomy. The anchor
//! is the INE province code — the only administrative geography the schema
//! already holds (`farm_es_extension.province_code`, and each plot's SIGPAC
//! reference).
//!
//! [`CO_OFFICIAL`] is written COMPLETE, ahead of the dictionaries: the result
//! is intersected with the languages that actually have a `Labels` const, so
//! adding Galician later is one dictionary and zero edits here. Until then
//! Galicia resolves to Castilian alone, which is correct — the app cannot print
//! what it cannot say.

use crate::labels::ReportLanguage;
use module_cue::Result;
use rusqlite::Connection;

/// INE province codes → the languages co-official there, beyond Castilian.
///
/// Sources: the statutes of autonomy (Catalunya art. 6, Illes Balears art. 4,
/// C. Valenciana art. 6, Galicia art. 5, País Vasco art. 6) and Navarra's
/// Ley Foral 18/1986 (Basque is co-official in the vascófona zone only — the
/// province is listed because a per-municipality map is not worth carrying to
/// decide which languages to OFFER).
///
/// Aranese/Occitan (co-official in Val d'Aran, Lleida) is deliberately absent:
/// it applies to one valley, not to province 25.
const CO_OFFICIAL: &[(u8, &str)] = &[
    // Catalunya
    (8, "ca"),  // Barcelona
    (17, "ca"), // Girona
    (25, "ca"), // Lleida
    (43, "ca"), // Tarragona
    // Illes Balears
    (7, "ca"),
    // Comunitat Valenciana — the same language under its own statutory name,
    // so it waits for its own dictionary rather than being offered as "Català".
    (3, "ca-valencia"),  // Alacant
    (12, "ca-valencia"), // Castelló
    (46, "ca-valencia"), // València
    // Galicia
    (15, "gl"), // A Coruña
    (27, "gl"), // Lugo
    (32, "gl"), // Ourense
    (36, "gl"), // Pontevedra
    // País Vasco
    (1, "eu"),  // Araba/Álava
    (20, "eu"), // Gipuzkoa
    (48, "eu"), // Bizkaia
    // Navarra
    (31, "eu"),
];

/// Province codes are TEXT in the schema and reach us as the user typed them,
/// so "7", "07" and " 07 " must all be province 7.
fn parse_province(raw: &str) -> Option<u8> {
    raw.trim().parse::<u8>().ok().filter(|code| *code > 0)
}

/// The languages offered for a holding in the given provinces.
///
/// Castilian always leads. An empty province list means the app knows nothing
/// about where the holding is — that offers EVERY shipped language rather than
/// hiding the feature, because an unfilled province field is not a statement
/// about the farmer's language.
pub fn languages_for_provinces(provinces: &[u8]) -> Vec<ReportLanguage> {
    if provinces.is_empty() {
        return ReportLanguage::ALL.to_vec();
    }
    let mut languages = vec![ReportLanguage::Es];
    for (province, code) in CO_OFFICIAL {
        if !provinces.contains(province) {
            continue;
        }
        // The intersection with what we can actually print: a co-official
        // language with no dictionary yet simply does not appear.
        if let Some(language) = ReportLanguage::from_code(code)
            && !languages.contains(&language)
        {
            languages.push(language);
        }
    }
    languages
}

/// Every province the holding touches: the farm's own registry province plus
/// the SIGPAC province of each plot. The union is deliberate — a holding can
/// straddle a boundary, and offering one language too many costs nothing while
/// offering one too few hides a right.
pub fn languages_for_farm(conn: &Connection, farm_id: &str) -> Result<Vec<ReportLanguage>> {
    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;
    let mut provinces: Vec<u8> = Vec::new();
    let mut add = |raw: Option<&str>| {
        if let Some(code) = raw.and_then(parse_province)
            && !provinces.contains(&code)
        {
            provinces.push(code);
        }
    };

    add(farm.es.as_ref().and_then(|e| e.province_code.as_deref()));
    for plot in terrazgo_core::repository::list_plots(conn, farm_id)? {
        add(plot.es.as_ref().and_then(|e| e.sigpac_province.as_deref()));
    }

    Ok(languages_for_provinces(&provinces))
}

/// Which language the chooser starts on: the one the app is already speaking,
/// when the region makes it official; otherwise Castilian.
///
/// A UI locale that is no report language at all (English) simply is not in
/// `available`, so it falls through to Castilian like any other.
pub fn default_language(available: &[ReportLanguage], ui_locale: &str) -> ReportLanguage {
    ReportLanguage::from_code(ui_locale)
        .filter(|language| available.contains(language))
        .unwrap_or(ReportLanguage::Es)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalan_province_offers_both_languages() {
        // Estatut de Catalunya art. 6: Catalan is the region's own language,
        // co-official with Castilian.
        for province in [8, 17, 25, 43] {
            assert_eq!(
                languages_for_provinces(&[province]),
                vec![ReportLanguage::Es, ReportLanguage::Ca],
                "province {province}"
            );
        }
        // Estatut de les Illes Balears art. 4.
        assert_eq!(
            languages_for_provinces(&[7]),
            vec![ReportLanguage::Es, ReportLanguage::Ca]
        );
    }

    #[test]
    fn a_castilian_only_province_offers_castilian_alone() {
        // Valladolid (47) — the development region, no co-official language.
        assert_eq!(languages_for_provinces(&[47]), vec![ReportLanguage::Es]);
    }

    #[test]
    fn a_co_official_language_without_a_dictionary_is_not_offered_yet() {
        // Galicia and the Basque Country are in the map; until their Labels
        // consts exist the intersection is empty and only Castilian shows.
        assert_eq!(languages_for_provinces(&[15]), vec![ReportLanguage::Es]);
        assert_eq!(languages_for_provinces(&[48]), vec![ReportLanguage::Es]);
        // València's own statutory name likewise waits for its dictionary.
        assert_eq!(languages_for_provinces(&[46]), vec![ReportLanguage::Es]);
    }

    #[test]
    fn a_holding_straddling_a_boundary_keeps_both_regions_languages() {
        // Lleida (25) + Huesca (22): the Catalan side still grants Catalan.
        assert_eq!(
            languages_for_provinces(&[22, 25]),
            vec![ReportLanguage::Es, ReportLanguage::Ca]
        );
    }

    #[test]
    fn an_unknown_region_offers_everything_rather_than_hiding_the_choice() {
        // No province recorded anywhere: an unfilled form field is not a
        // statement about which language the farmer is entitled to print in.
        assert_eq!(languages_for_provinces(&[]), ReportLanguage::ALL.to_vec());
    }

    #[test]
    fn province_codes_parse_however_they_were_typed() {
        assert_eq!(parse_province("07"), Some(7));
        assert_eq!(parse_province("7"), Some(7));
        assert_eq!(parse_province(" 43 "), Some(43));
        // Not a province: blank, non-numeric, or the zero placeholder.
        assert_eq!(parse_province(""), None);
        assert_eq!(parse_province("BA"), None);
        assert_eq!(parse_province("00"), None);
    }

    #[test]
    fn the_chooser_starts_on_the_language_the_app_is_speaking() {
        let both = [ReportLanguage::Es, ReportLanguage::Ca];
        assert_eq!(default_language(&both, "ca"), ReportLanguage::Ca);
        assert_eq!(default_language(&both, "es"), ReportLanguage::Es);
        // English is no report language; Castilian is the fallback.
        assert_eq!(default_language(&both, "en"), ReportLanguage::Es);
        // A Catalan-speaking UI printing a Castilian-only holding's book.
        assert_eq!(
            default_language(&[ReportLanguage::Es], "ca"),
            ReportLanguage::Es
        );
    }
}
