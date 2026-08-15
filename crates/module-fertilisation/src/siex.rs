// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mapping between this module's neutral codes and the Spanish SIEX coding
//! (FEGA catalogues; design in docs/siex-export.md).
//!
//! Same two-tier rule as module-cue's `siex`: small closed lists are owned as
//! English lookups and mapped here, while provider lists too large to own are
//! stored verbatim as catalogue codes and only named. Both lists in this module
//! are small and closed, so both are mapped.
//!
//! A contract test (`tests/siex_mapping.rs`) checks each mapping against the
//! vendored catalogue snapshot in BOTH directions, so a snapshot refresh that
//! adds or retires a code fails the suite instead of silently under-exporting.

/// SIEX `Riego.SistemaRiego` code (catalogue `SIST_RIEGO`).
///
/// The printed model's own section 8 footnote lists these eight in this order,
/// which is why the seeded lookup follows it too.
pub fn irrigation_method_to_siex(code: &str) -> Option<i64> {
    match code {
        "surface_gravity" => Some(1),
        "sprinkler_fixed" => Some(2),
        "sprinkler_mobile" => Some(3),
        "micro_sprinkler" => Some(4),
        "misting" => Some(5),
        "drip" => Some(6),
        "hydroponic_open" => Some(7),
        "hydroponic_recirculating" => Some(8),
        _ => None,
    }
}

/// SIEX `Riego.OrigenAgua[].IdOrigenAgua` code (catalogue `ORIGEN_AGUA_RIEGO`).
pub fn water_origin_to_siex(code: &str) -> Option<i64> {
    match code {
        "surface" => Some(1),
        "groundwater" => Some(2),
        "rainwater" => Some(3),
        "reclaimed" => Some(4),
        "desalinated" => Some(5),
        "alternative" => Some(6),
        _ => None,
    }
}

/// SIEX `Riego.UnidadMedida` code (catalogue `UNIDADES_MEDIDA`), for the two
/// units a water volume can be recorded in.
pub fn volume_unit_to_siex(code: &str) -> Option<i64> {
    match code {
        "m3" => Some(3),
        "m3_ha" => Some(19),
        _ => None,
    }
}

/// Catalogue that `irrigation_record.energy_type_code` resolves against — the
/// energy driving the irrigation, stored verbatim because it is a provider
/// list this module does not own.
pub fn energy_type_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("TIPENERGIA")
}

/// SIEX `AplicacionMaterialFertilizante.TipoFertilizacion` (catalogue
/// `TIPO_FERITILIZACION` — the provider's own spelling of the id).
pub fn fertilisation_type_to_siex(code: &str) -> Option<i64> {
    match code {
        "base_dressing" => Some(1),
        "top_dressing" => Some(2),
        "amendment" => Some(3),
        _ => None,
    }
}

/// SIEX `AplicacionMaterialFertilizante.MetodoFertilizacion` (catalogue
/// `METODO_APLICACION_FERTILIZANTE`). Codes 5 and 6 are the two fertigation
/// entries Anexo III C.f asks to be distinguished.
pub fn application_method_to_siex(code: &str) -> Option<i64> {
    match code {
        "broadcast" => Some(1),
        "broadcast_buried" => Some(2),
        "banded" => Some(3),
        "banded_buried" => Some(4),
        "fertigation_sprinkler" => Some(5),
        "fertigation_localised" => Some(6),
        "foliar" => Some(7),
        _ => None,
    }
}

/// SIEX `MaterialFertilizante.TratamientoEstiercoles` (catalogue
/// `TRAT_ESTIERCOLES`).
pub fn manure_treatment_to_siex(code: &str) -> Option<i64> {
    match code {
        "none" => Some(1),
        "solid_fraction" => Some(2),
        "liquid_fraction" => Some(3),
        "ndn_effluent" => Some(4),
        "composting" => Some(5),
        "anaerobic_digestion" => Some(6),
        "solar_drying" => Some(7),
        "stripping" => Some(8),
        "membrane_separation" => Some(9),
        _ => None,
    }
}

/// SIEX `AplicacionMaterialFertilizante.Unidad` (catalogue `UNIDADES_MEDIDA`),
/// for the four rates a fertiliser dose can be stated in.
pub fn dose_unit_to_siex(code: &str) -> Option<i64> {
    match code {
        "kg_ha" => Some(17),
        "l_ha" => Some(18),
        "m3_ha" => Some(19),
        "t_ha" => Some(29),
        _ => None,
    }
}

/// The unit `fertiliser_material.density_kg_l` is fixed at, for the twin's
/// `MaterialFertilizante.UnidadesMedida` beside its `Densidad`. The column
/// carries no unit of its own because a fertiliser density is kg/L on every
/// label; this is how a serializer still says so.
pub const DENSITY_UNIT_SIEX: i64 = 12;

/// Catalogue `fertiliser_material.material_code` resolves against — Anexo III
/// C.d's first level.
pub fn fertiliser_material_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("MAT_FERTI")
}

/// Catalogue `fertiliser_material.material_detail_code` resolves against —
/// C.d's second level, the named commercial products.
pub fn fertiliser_detail_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("DETALLE_MATERIAL_FERT")
}

/// Which nutrient catalogue a composition line's `nutrient_code` indexes. The
/// three are separate lists sharing a number space, which is exactly why
/// `fertiliser_material_nutrient` stores the kind beside the code.
pub fn nutrient_catalogue(country_code: &str, kind_code: &str) -> Option<&'static str> {
    if country_code != "es" {
        return None;
    }
    match kind_code {
        "macro" => Some("MACRONUTRIENTES"),
        "micro" => Some("MICRONUTRIENTES"),
        "heavy_metal" => Some("METALES_PESADOS"),
        _ => None,
    }
}

/// Catalogue `fertilisation_practice.practice_code` resolves against.
pub fn good_practice_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("BUENAS_PRACTICAS_AMBITOS")
}

/// The `BUENAS_PRACTICAS_AMBITOS` column that says which vocabulary a row
/// belongs to, and the value that selects fertilisation's. Verbatim provider
/// strings — a mirror of an external contract, accent included.
pub const GOOD_PRACTICE_SCOPE_KEY: &str = "Ámbito";
pub const FERTILISATION_SCOPE: &str = "Fertilización";

/// The `DETALLE_MATERIAL_FERT` column carrying the parent `MAT_FERTI` code.
/// The file leads with it, which is why the importer takes its code from a
/// later column and this one rides in `attrs`.
pub const MATERIAL_PARENT_KEY: &str = "Código SIEX";
