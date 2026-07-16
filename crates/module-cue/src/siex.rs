// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mapping between Terrazgo's neutral codes and the Spanish SIEX coding
//! (FEGA Anexo VII catalogues; design in docs/siex-export.md).
//!
//! Terrazgo stores country-neutral English codes for the small closed lists
//! (efficacy, justification, authorisation kind, dose unit) and maps them to
//! each country's export coding at serialization — Spain's are the integer
//! codes below. Phytosanitary *problems* are the opposite case: the provider
//! lists are far too large to own, so records store the catalogue code
//! verbatim and this module only names which catalogue a category resolves
//! against.
//!
//! A contract test (`tests/siex_mapping.rs`) checks every mapping against the
//! vendored catalogue snapshot in both directions, so a snapshot refresh that
//! adds or retires a code fails the suite instead of silently under-exporting.

/// Catalogue (SIEX idTabla) that `treatment_problem.problem_code` resolves
/// against, per record country and reason category. `None` means no coded
/// list exists for that country/category — the code cannot be checked or
/// exported, only stored.
///
/// The category also picks the `ProblematicaFito` export bucket:
/// disease → `Enfermedades.TipoEnfermedad`, pest →
/// `ArtropodosGasteropodos.TipoPlaga`, weed → `MalasHierbas.TipoMalaHierba`,
/// growth_regulator/other → `ReguladoresOtros.TipoRegulador`.
pub fn problem_catalogue(country_code: &str, reason_category_code: &str) -> Option<&'static str> {
    if country_code != "es" {
        return None;
    }
    match reason_category_code {
        "disease" => Some("ENFERMEDADES"),
        "pest" => Some("PLAGAS"),
        "weed" => Some("MALAS_HIERBAS"),
        "growth_regulator" | "other" => Some("REGULADORES_CRECIMIENTO"),
        _ => None,
    }
}

/// Catalogue that `product_authorisation.exceptional_substance_code` resolves
/// against — the granted exceptional (Art. 53) authorisations, whose code is
/// what SIEX's `MateriaActiva` field carries for TipoProducto 4.
pub fn exceptional_substance_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("AUTORIZACION_EXCP")
}

/// SIEX `Eficacia` code (catalogue EFICACIA_TRATAMIENTO).
pub fn efficacy_to_siex(code: &str) -> Option<i64> {
    match code {
        "good" => Some(1),
        "fair" => Some(2),
        "poor" => Some(3),
        _ => None,
    }
}

/// SIEX `Justificaciones[].JustAct` code (catalogue JUSTIFICACION_ACTUACION).
pub fn justification_to_siex(code: &str) -> Option<i64> {
    match code {
        "threshold_exceeded" => Some(1),
        "monitoring" => Some(2),
        "decision_support_system" => Some(3),
        "authority_warning" => Some(4),
        "advisor_recommendation" => Some(5),
        "alert_device" => Some(6),
        _ => None,
    }
}

/// SIEX `ProductosFito[].TipoProducto` code (catalogue TIPO_PRODFITO).
pub fn authorisation_kind_to_siex(code: &str) -> Option<i64> {
    match code {
        "registered" => Some(1),
        "common_name" => Some(2),
        "parallel_import" => Some(3),
        "exceptional" => Some(4),
        _ => None,
    }
}

/// SIEX date rendering: ISO `YYYY-MM-DD` → `DD/MM/YYYY`. The 3.11.4 schema
/// pattern-enforces `dd/mm/yyyy` on every `Fecha*` field; our stored dates are
/// ISO (engineering convention), so the serializer converts at the boundary.
/// Returns `None` when the input is not shaped like an ISO date — stored
/// dates are validated at insert, so that is a defect, not user input.
pub fn date_to_siex(iso_date: &str) -> Option<String> {
    let bytes = iso_date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let (year, month, day) = (&iso_date[0..4], &iso_date[5..7], &iso_date[8..10]);
    if !(year.chars().chain(month.chars()).chain(day.chars())).all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{day}/{month}/{year}"))
}

/// SIEX `CAExplotacion`: the INE code of the comunidad autónoma, derived from
/// the farm's province code (both are the INE codifications; source: INE
/// "Relación de provincias por comunidades autónomas"). Accepts the province
/// with or without the leading zero; returns the two-character CCAA code the
/// schema requires. `None` for anything outside 1–52.
pub fn province_to_ccaa(province_code: &str) -> Option<&'static str> {
    let province: u8 = province_code.trim().parse().ok()?;
    let ccaa = match province {
        4 | 11 | 14 | 18 | 21 | 23 | 29 | 41 => "01", // Andalucía
        22 | 44 | 50 => "02",                         // Aragón
        33 => "03",                                   // Asturias
        7 => "04",                                    // Illes Balears
        35 | 38 => "05",                              // Canarias
        39 => "06",                                   // Cantabria
        5 | 9 | 24 | 34 | 37 | 40 | 42 | 47 | 49 => "07", // Castilla y León
        2 | 13 | 16 | 19 | 45 => "08",                // Castilla-La Mancha
        8 | 17 | 25 | 43 => "09",                     // Cataluña
        3 | 12 | 46 => "10",                          // Comunitat Valenciana
        6 | 10 => "11",                               // Extremadura
        15 | 27 | 32 | 36 => "12",                    // Galicia
        28 => "13",                                   // Madrid
        30 => "14",                                   // Murcia
        31 => "15",                                   // Navarra
        1 | 20 | 48 => "16",                          // País Vasco
        26 => "17",                                   // La Rioja
        51 => "18",                                   // Ceuta
        52 => "19",                                   // Melilla
        _ => return None,
    };
    Some(ccaa)
}

/// SIEX `ProductosFito[].Unidad` code (catalogue UNIDADES_MEDIDA) plus the
/// factor the dose value is multiplied by, because SIEX lacks some of our
/// units and the nearest catalogue unit differs by an exact power of ten
/// (ml/ha → L/ha, g/l → mg/L) or is the same quantity under another name
/// (ml/hl → cc/hL, ml/l → L/m³ — both identities). Exact conversions only;
/// the serializer applies the factor when it emits `Dosis`.
pub fn unit_to_siex(code: &str) -> Option<(i64, f64)> {
    match code {
        "l_ha" => Some((18, 1.0)),    // L/ha
        "kg_ha" => Some((17, 1.0)),   // kg/ha
        "ml_ha" => Some((18, 0.001)), // no ml/ha in SIEX → L/ha
        "g_ha" => Some((49, 1.0)),    // g/ha
        "ml_hl" => Some((64, 1.0)),   // cc/hL ≡ ml/hl
        "g_hl" => Some((65, 1.0)),    // g/hL
        "g_l" => Some((20, 1000.0)),  // no g/L in SIEX → mg/L
        "ml_l" => Some((31, 1.0)),    // L/m³ ≡ ml/L
        "pct" => Some((14, 1.0)),     // %
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_convert_iso_to_siex() {
        // dd/mm/yyyy per the 3.11.4 schema pattern on Fecha* fields.
        assert_eq!(date_to_siex("2026-05-01").as_deref(), Some("01/05/2026"));
        assert_eq!(date_to_siex("2024-12-31").as_deref(), Some("31/12/2024"));
        // Not ISO-shaped → None, never a garbled date.
        assert_eq!(date_to_siex("01/05/2026"), None);
        assert_eq!(date_to_siex("2026-5-1"), None);
        assert_eq!(date_to_siex(""), None);
    }

    #[test]
    fn provinces_map_to_their_ine_ccaa() {
        // Spot checks against the INE relation of provinces per comunidad
        // autónoma: Valladolid (47) → Castilla y León (07), Sevilla (41) →
        // Andalucía (01), Álava (01) → País Vasco (16), Las Palmas (35) →
        // Canarias (05), Melilla (52) → Melilla (19).
        assert_eq!(province_to_ccaa("47"), Some("07"));
        assert_eq!(province_to_ccaa("41"), Some("01"));
        assert_eq!(province_to_ccaa("01"), Some("16"));
        assert_eq!(province_to_ccaa("35"), Some("05"));
        assert_eq!(province_to_ccaa("52"), Some("19"));
        // Leading zero optional — the form stores free text.
        assert_eq!(province_to_ccaa("1"), Some("16"));
        assert_eq!(province_to_ccaa("5"), Some("07"));
    }

    #[test]
    fn every_ine_province_has_a_ccaa_and_nothing_else_does() {
        for p in 1..=52u8 {
            assert!(
                province_to_ccaa(&p.to_string()).is_some(),
                "province {p} must map to a comunidad autónoma"
            );
        }
        assert_eq!(province_to_ccaa("0"), None);
        assert_eq!(province_to_ccaa("53"), None);
        assert_eq!(province_to_ccaa("VA"), None);
        assert_eq!(province_to_ccaa(""), None);
    }
}
