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

/// Catalogue that `analysis_substance.substance_code` resolves against — the
/// active substances a residue analysis reports, whose rows carry the CAS
/// number a future non-Spanish export would match on.
pub fn substance_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("SUST_ACTIVAS")
}

/// Catalogue that `treatment_record.measure_code` resolves against — the
/// non-chemical measures of model 3.1 bis's "Alternativas no químicas de
/// intervención" (the twin's `OtrasActuacionesFito.TipoMedida`).
///
/// Deliberately NOT `MEDIDA_PREVENTIVA_CULTURAL`, which backs a different
/// question: that one hangs off `DatosExplotacion` and declares which IPM
/// practices the HOLDING follows ("rotación de cultivos", "asesoramiento por
/// un asesor en GIP") — no date, no plot, no intensity, and no column anywhere
/// in the printed model.
pub fn measure_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("TIPO_MEDIDA_FITOSANITARIA")
}

/// Catalogue that `treatment_plot.growth_stage_code` resolves against — the
/// BBCH principal growth stages (the twin's `DGCs[].EstadoFenologico`), asked
/// for by Reglamento (UE) 2023/564's annex.
///
/// Its code is NOT the BBCH stage: FEGA numbers the rows 1-10 and publishes the
/// monograph's own 0-9 in a column beside them, so a picker or a printed cell
/// resolves through [`crate::catalogue::growth_stage_label`].
pub fn growth_stage_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("EST_FENOLOGICO")
}

/// Catalogue that `harvest_record.plant_product_code` and
/// `non_field_treatment.subject_product_code` resolve against — the HARVESTED
/// PRODUCE, which is a different list from the crop catalogue `PRODUCTOS` that
/// `crop.crop_code` and `seed_treatment.crop_code` speak in.
pub fn plant_product_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("PROD_VEGETAL")
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

/// SIEX `Analitica.MaterialAnalizado` code (catalogue MATERIAL_ANALIZADO).
///
/// Four values, because FEGA separates the standing crop from the produce taken
/// off it — a distinction the printed model's parenthetical "(vegetal / tierra /
/// agua)" cannot express, and the reason the book prints FEGA's wording.
pub fn analysis_material_to_siex(code: &str) -> Option<i64> {
    match code {
        "crop" => Some(1),
        "harvested_produce" => Some(2),
        "soil" => Some(3),
        "water" => Some(4),
        _ => None,
    }
}

/// SIEX `Analitica.TiposAnalisis[]` code (catalogue TIPO_ANALISIS).
pub fn analysis_type_to_siex(code: &str) -> Option<i64> {
    match code {
        "pesticide_residues" => Some(1),
        "microbiological" => Some(2),
        "heavy_metals" => Some(3),
        "nutrients" => Some(4),
        "soil_parameters" => Some(5),
        "gmo_presence" => Some(6),
        _ => None,
    }
}

/// SIEX `UsoSemillaTratada.Tratamiento` code (catalogue TIPO_TRATAMIENTO),
/// whose codes start at 2 — there is no code 1.
pub fn seed_treatment_kind_to_siex(code: &str) -> Option<i64> {
    match code {
        "on_farm" => Some(2),
        "processing_centre" => Some(3),
        "purchased_es" => Some(4),
        "purchased_abroad" => Some(5),
        _ => None,
    }
}

/// SIEX `SistemaExplotacion` code (catalogue SIST_EXPLOTACION): is the crop
/// irrigated at all, R or S.
///
/// Total but not injective, and deliberately NOT a map onto `SIST_RIEGO`: our
/// `sprinkler` sits between the catalogue's "Aspersión fija" and "Aspersión
/// móvil" with nothing in the record to choose between them, and `rainfed` has
/// no SIST_RIEGO code at all. Guessing either would bake a statement the farmer
/// never made into a regulatory export; the finer system is the Irrigation
/// module's to capture.
pub fn irrigation_to_siex_exploitation(code: &str) -> Option<&'static str> {
    match code {
        "rainfed" => Some("S"),
        "sprinkler" | "drip" | "gravity" => Some("R"),
        _ => None,
    }
}

/// SIEX `SistemaCultivo` code (catalogue SIST_CULTIVO).
///
/// One-directional: the catalogue publishes 33 systems (bodegas de setas,
/// sustratos, entutorados…) that the model's four-value "aire libre / protegido"
/// column does not offer, so the reverse map is not ours to write.
pub fn growing_environment_to_siex(code: &str) -> Option<i64> {
    match code {
        "open_air" => Some(1),
        "mesh" => Some(2),
        "plastic_cover" => Some(3), // "Cubierta no accesible"
        "greenhouse" => Some(4),    // "Invernadero (cubierta accesible)"
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

/// SIEX `Unidad` code for a unit of AMOUNT — how much was used or how much was
/// treated — as against [`unit_to_siex`]'s rates and concentrations.
///
/// Every one of ours is in UNIDADES_MEDIDA verbatim, so unlike the dose units
/// there is no conversion factor to carry: kg is kg. A `dose_rate` code has no
/// answer here and vice versa, which is what keeps a rate out of a field that
/// asks for a quantity.
pub fn quantity_unit_to_siex(code: &str) -> Option<i64> {
    match code {
        "m2" => Some(1),
        "m3" => Some(3),
        "l" => Some(4),
        "kg" => Some(5),
        "t" => Some(6),
        _ => None,
    }
}

/// SIEX `OtrasActuacionesFito.Unidad` code for the INTENSITY of a non-chemical
/// measure — the official model's "Intensidad de la medida (Nº de trampas, nº
/// de difusores, etc.)".
///
/// A third list beside [`unit_to_siex`]'s rates and [`quantity_unit_to_siex`]'s
/// amounts, because a count is neither: every one of ours is in
/// `UNIDADES_MEDIDA` verbatim, so there is no factor to carry, and a dose code
/// has no answer here or vice versa.
///
/// **The count's own unit is sent, not the generic one.** Anexo V's field 18
/// narrows the "unidades válidas" per measure kind to Unidades, uds./m², uds./ha,
/// m², m² malla/ha, kg and kg/ha — a list that omits Trampas and Difusores even
/// though the catalogue publishes both and the JSON Schema asks only that the
/// code be in the catalogue. Answering "12 trampas" with code 11 (Unidades)
/// would drop *what was counted*, which is the one thing the model asks for by
/// name; so the exact code goes out and the narrowing is left to the receiver.
pub fn intensity_unit_to_siex(code: &str) -> Option<i64> {
    match code {
        "traps" => Some(27),        // Trampas
        "traps_ha" => Some(24),     // Trampas/ha
        "diffusers" => Some(25),    // Difusores
        "diffusers_ha" => Some(22), // Difusores/ha
        "units" => Some(11),        // Unidades
        "units_ha" => Some(16),     // Unidades/ha
        _ => None,
    }
}

/// Whether `TIPO_MEDIDA_FITOSANITARIA` code `measure` is one the authority
/// demands an MDF registration number for.
///
/// Anexo V field 19 grades `Registro MDF` **Obligatorio** and scopes it: *"Solo
/// se muestra en caso en que la alternativa no química sea: suelta de OCB,
/// trampas y otros y feromonas y atrayentes para monitoreo"* — codes 1, 14 and
/// 15. The Anexo VI descriptor sheet names a *different* set (OCB, plantas
/// banker, trampas cromotrópicas) and no decree names the field at all, RD
/// 1311/2012 Anexo III Parte I B having no non-chemical member of any kind. So
/// this reads Anexo V, which is the definition-of-variables document and the
/// one published as a corrección de errores (docs/siex-export.md → "Seam 5").
///
/// Only the *demand* is enforced. Anexo V's other half — "en caso contrario el
/// campo debe ir vacío" — is not, because the two documents disagree about which
/// kinds are in scope and honouring either would silently discard a number the
/// farmer recorded.
pub fn measure_requires_mdf_number(measure: i64) -> bool {
    matches!(measure, 1 | 14 | 15)
}

/// A stored mass converted to kilograms, for the fields SIEX fixes the unit of
/// instead of carrying one.
///
/// `TratamientosPostCosecha.Cantidad` is the case: Anexo V defines it as "peso
/// en kg del producto vegetal tratado" with "UNIDADES VÁLIDAS: kg" and the
/// block has no unit member — while the printed model asks for tonnes, which is
/// what the register stores. Emitting the stored number unconverted would
/// understate a 120 t lot as 120 kg.
pub fn mass_in_kg(value: f64, unit_code: &str) -> Option<f64> {
    match unit_code {
        "kg" => Some(value),
        "t" => Some(value * 1000.0),
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
        // The two ciudades autónomas take INE's own 18 and 19. FEGA's
        // COMUNIDAD_AUTONOMA publishes the seventeen comunidades and so cannot
        // resolve either to a label — which is a missing label, not a missing
        // code, and must never be "fixed" by sending "00 Comunidad
        // Desconocida" or a neighbour's code for a holding whose location is
        // perfectly known (docs/siex-export.md → Recorded gaps).
        assert_eq!(province_to_ccaa("51"), Some("18"));
        assert_eq!(province_to_ccaa("52"), Some("19"));
        assert!(!matches!(province_to_ccaa("51"), Some("00")));
        assert!(!matches!(province_to_ccaa("52"), Some("00")));
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
