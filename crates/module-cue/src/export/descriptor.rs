// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed mirror of the official CUE descriptor (JSON Schema v3.11.4, vendored
//! at docs/references/cue-schema-3.11.4.json), restricted to what this module
//! emits: the per-farm envelope and the `TratamFito` activity block. The
//! serde renames must match the schema property names byte-for-byte — the
//! export tests validate the serialized output against the schema itself.
//! Optional fields are skipped when absent (the schema knows no null).

use serde::Serialize;

/// Schema root. The array can carry several farms; this exporter emits one
/// farm per file.
#[derive(Debug, Clone, Serialize)]
pub struct CuadernoExport {
    #[serde(rename = "CUADERNO")]
    pub cuaderno: Vec<CuadernoEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CuadernoEntry {
    /// Comunidad autónoma (INE code), derived from the farm's province.
    #[serde(rename = "CAExplotacion")]
    pub ca_explotacion: String,
    /// Titular tax id (`farm.owner_tax_id`), from the REA registration.
    #[serde(rename = "IdTitular")]
    pub id_titular: String,
    /// REA registration code (`farm_es_extension.rea_code`).
    #[serde(rename = "CodigoRea")]
    pub codigo_rea: String,
    /// Managing-entity tax id; for a titular-driven notebook it defaults to
    /// the titular's own (docs/siex-export.md → open question 7).
    #[serde(rename = "UnidadGestora")]
    pub unidad_gestora: String,
    #[serde(rename = "ActividadesExplotacion")]
    pub actividades_explotacion: ActividadesExplotacion,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActividadesExplotacion {
    #[serde(rename = "TratamFito")]
    pub tratam_fito: Vec<TratamFito>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TratamFito {
    /// Frozen integer alias (`export_alias`, keyed on record + crop split).
    #[serde(rename = "IdAjenaTratamFito")]
    pub id_ajena_tratam_fito: i64,
    /// `true` only on deletion entries for previously exported records.
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// dd/mm/yyyy (schema-pattern-enforced); one application day, so both
    /// dates carry `treatment_record.application_date`.
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<Dgc>,
    #[serde(rename = "ProblematicaFito")]
    pub problematica_fito: ProblematicaFito,
    #[serde(rename = "Justificaciones")]
    pub justificaciones: Vec<Justificacion>,
    #[serde(rename = "ProductosFito")]
    pub productos_fito: Vec<ProductoFito>,
    #[serde(rename = "IdentificadorAplicador")]
    pub identificador_aplicador: Vec<IdentificadorAplicador>,
    #[serde(rename = "Eficacia")]
    pub eficacia: i64,
    #[serde(rename = "Observaciones", skip_serializing_if = "Option::is_none")]
    pub observaciones: Option<String>,
}

/// One plot+crop unit the treatment covered. The client-assigned code is the
/// REA-independent DGC path (docs/siex-export.md → gap 2); it is absent only
/// on deletion entries whose plot never had a crop assigned.
#[derive(Debug, Clone, Serialize)]
pub struct Dgc {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "Superficie")]
    pub superficie: f64,
}

/// The four coded problem buckets; a bucket is omitted when the record treats
/// no problem of that kind.
#[derive(Debug, Clone, Serialize)]
pub struct ProblematicaFito {
    #[serde(rename = "Enfermedades", skip_serializing_if = "Option::is_none")]
    pub enfermedades: Option<Enfermedades>,
    #[serde(
        rename = "ArtropodosGasteropodos",
        skip_serializing_if = "Option::is_none"
    )]
    pub artropodos_gasteropodos: Option<ArtropodosGasteropodos>,
    #[serde(rename = "MalasHierbas", skip_serializing_if = "Option::is_none")]
    pub malas_hierbas: Option<MalasHierbas>,
    #[serde(rename = "ReguladoresOtros", skip_serializing_if = "Option::is_none")]
    pub reguladores_otros: Option<ReguladoresOtros>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Enfermedades {
    #[serde(rename = "TipoEnfermedad")]
    pub tipo_enfermedad: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtropodosGasteropodos {
    #[serde(rename = "TipoPlaga")]
    pub tipo_plaga: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MalasHierbas {
    #[serde(rename = "TipoMalaHierba")]
    pub tipo_mala_hierba: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReguladoresOtros {
    #[serde(rename = "TipoRegulador")]
    pub tipo_regulador: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Justificacion {
    #[serde(rename = "JustAct")]
    pub just_act: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductoFito {
    /// TIPO_PRODFITO code (registered/common-name/parallel-import → 1..3,
    /// exceptional → 4).
    #[serde(rename = "TipoProducto")]
    pub tipo_producto: i64,
    #[serde(rename = "NumRegistro", skip_serializing_if = "Option::is_none")]
    pub num_registro: Option<String>,
    /// AUTORIZACION_EXCP code; mandatory exactly for TipoProducto 4.
    #[serde(rename = "MateriaActiva", skip_serializing_if = "Option::is_none")]
    pub materia_activa: Option<i64>,
    /// Dose per surface/volume — Dosis XOR Cantidad per the descriptor
    /// ("nunca ambas"); our units are rates, so Dosis is what this exporter
    /// emits, converted by `siex::unit_to_siex`'s exact factor.
    #[serde(rename = "Dosis")]
    pub dosis: f64,
    #[serde(rename = "Unidad")]
    pub unidad: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdentificadorAplicador {
    #[serde(rename = "AplicadorEmpresa")]
    pub aplicador_empresa: AplicadorEmpresa,
    #[serde(rename = "EquipoAplicador")]
    pub equipo_aplicador: EquipoAplicador,
}

#[derive(Debug, Clone, Serialize)]
pub struct AplicadorEmpresa {
    #[serde(rename = "NumROPO")]
    pub num_ropo: String,
}

/// The schema requires exactly one of `NumROMA`/`NumREGANIP`/
/// `IdEquipoAplicador` (a `oneOf`), plus `AplicacionManual` — the builder
/// guarantees the exactly-one invariant.
#[derive(Debug, Clone, Serialize)]
pub struct EquipoAplicador {
    #[serde(rename = "NumROMA", skip_serializing_if = "Option::is_none")]
    pub num_roma: Option<String>,
    #[serde(rename = "NumREGANIP", skip_serializing_if = "Option::is_none")]
    pub num_reganip: Option<String>,
    #[serde(rename = "IdEquipoAplicador", skip_serializing_if = "Option::is_none")]
    pub id_equipo_aplicador: Option<String>,
    #[serde(rename = "AplicacionManual")]
    pub aplicacion_manual: bool,
}
