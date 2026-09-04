// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed mirror of the official CUE descriptor (JSON Schema v3.11.4, vendored
//! at docs/references/cue-schema-3.11.4.json): the per-farm envelope and the
//! activity blocks this crate emits. The serde renames must match the schema
//! property names byte-for-byte — the tests validate the serialized output
//! against the schema itself. Optional fields are skipped when absent (the
//! schema knows no null).
//!
//! `ActividadesExplotacion` declares no required properties and every block is
//! `0..n`, so a block is omitted entirely when the campaign holds none of its
//! records — "required" inside a block binds only an entry that is actually
//! sent. Anexo V's own OBLIGATORIEDAD column is the separate question of what a
//! holding must report; see docs/siex-export.md.

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

/// The activity blocks this exporter emits. Every one is `0..n` and none is
/// required, so a block with no records serializes as an empty array — which is
/// what `skip_serializing_if` turns into "absent" rather than "none happened".
#[derive(Debug, Clone, Serialize)]
pub struct ActividadesExplotacion {
    #[serde(rename = "TratamFito", skip_serializing_if = "Vec::is_empty")]
    pub tratam_fito: Vec<TratamFito>,
    #[serde(
        rename = "TratamientosPostCosecha",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub tratamientos_post_cosecha: Vec<TratamientoPostCosecha>,
    #[serde(
        rename = "TratamientosEdifInstalaciones",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub tratamientos_edif_instalaciones: Vec<TratamientoEdifInstalaciones>,
    #[serde(rename = "UsoSemillaTratada", skip_serializing_if = "Vec::is_empty")]
    pub uso_semilla_tratada: Vec<UsoSemillaTratada>,
    #[serde(rename = "Analitica", skip_serializing_if = "Vec::is_empty")]
    pub analitica: Vec<Analitica>,
    #[serde(rename = "ComercializacionVD", skip_serializing_if = "Vec::is_empty")]
    pub comercializacion_vd: Vec<ComercializacionVd>,
    #[serde(rename = "SiembraPlantacion", skip_serializing_if = "Vec::is_empty")]
    pub siembra_plantacion: Vec<SiembraPlantacion>,
    #[serde(rename = "Fertilizacion", skip_serializing_if = "Vec::is_empty")]
    pub fertilizacion: Vec<Fertilizacion>,
    #[serde(rename = "Riego", skip_serializing_if = "Vec::is_empty")]
    pub riego: Vec<Riego>,
    #[serde(rename = "PlanAbonado", skip_serializing_if = "Vec::is_empty")]
    pub plan_abonado: Vec<PlanAbonado>,
    #[serde(rename = "Pastoreo", skip_serializing_if = "Vec::is_empty")]
    pub pastoreo: Vec<Pastoreo>,
    #[serde(rename = "LaboresCulturales", skip_serializing_if = "Vec::is_empty")]
    pub labores_culturales: Vec<LaborCultural>,
    #[serde(rename = "DatosCubierta", skip_serializing_if = "Vec::is_empty")]
    pub datos_cubierta: Vec<DatosCubierta>,
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
    /// `HH:MM:SS` (Anexo VI: `string(8)`), from the local wall-clock `HH:MM`
    /// the record stores — a farmer records no seconds, so the serializer pads
    /// them, the same shaping the dd/mm/yyyy dates get. Omitted when the hour
    /// was not stated, which Reglamento (UE) 2023/564 makes the ordinary case.
    #[serde(rename = "HoraTratamiento", skip_serializing_if = "Option::is_none")]
    pub hora_tratamiento: Option<String>,
    /// dd/mm/yyyy, from `treatment_record.drying_date`: the day the field was
    /// dried *in order to* spray it, which is why the column sits on the
    /// treatment and not on the flooded crop's own calendar.
    ///
    /// Anexo V field 4 grades it Obligatorio *"cuando se trate de cultivos bajo
    /// agua"*, and the export does NOT gate on that. The condition is not the
    /// crop being flooded but the field having been dried for this treatment,
    /// and a rice herbicide applied on water is a lawful record with no drying
    /// date to state — so demanding one of every treatment on a flooded crop
    /// would refuse records the decree permits.
    #[serde(rename = "FechaSeca", skip_serializing_if = "Option::is_none")]
    pub fecha_seca: Option<String>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<Dgc>,
    #[serde(rename = "ProblematicaFito")]
    pub problematica_fito: ProblematicaFito,
    #[serde(rename = "Justificaciones")]
    pub justificaciones: Vec<Justificacion>,
    /// The non-chemical half of model 3.1 bis. Present exactly when
    /// `ProductosFito` is absent — see that field.
    #[serde(
        rename = "OtrasActuacionesFito",
        skip_serializing_if = "Option::is_none"
    )]
    pub otras_actuaciones_fito: Option<OtrasActuacionesFito>,
    /// **Omitted entirely, not sent empty, when the actuation carried no
    /// product** — which the schema allows, `ProductosFito` being absent from
    /// `TratamFito`'s required set. That absence is what makes a purely
    /// non-chemical actuation a first-class entry rather than a refusal.
    ///
    /// Anexo V grades all five members of `OtrasActuacionesFito` *"excluyente
    /// con el subbloque siguiente de «Productos fitosanitarios»"*, so the two
    /// are never both present; the precheck refuses the mixed record rather
    /// than choosing which half to drop.
    #[serde(rename = "ProductosFito", skip_serializing_if = "Vec::is_empty")]
    pub productos_fito: Vec<ProductoFito>,
    #[serde(rename = "IdentificadorAplicador")]
    pub identificador_aplicador: Vec<IdentificadorAplicador>,
    /// The advisor of Anexo III Parte I B.d's *"y, en su caso, del asesor"*.
    /// Sent whenever the record names one — and the precheck refuses a record
    /// that names an advisor with no ROPO number, because Anexo V grades field
    /// 50 Obligatorio here where blocks 1.2/1.3 grade it Voluntario, and this
    /// block's only carriable member is that number.
    #[serde(rename = "AsesorValidacion", skip_serializing_if = "Option::is_none")]
    pub asesor_validacion: Option<AsesorValidacion>,
    #[serde(rename = "Eficacia")]
    pub eficacia: i64,
    #[serde(rename = "Observaciones", skip_serializing_if = "Option::is_none")]
    pub observaciones: Option<String>,
}

/// Model 3.1 bis's "Alternativas no químicas de intervención" — one measure per
/// actuation, which is why the twin types it as an object rather than an array
/// and why the register hangs its four columns off the record.
///
/// **No decree creates this block.** RD 1311/2012 Anexo III Parte I B, the list
/// art. 16.1 binds the record to, is eleven lettered items describing a
/// *producto fitosanitario* and has no non-chemical member at all; the duty to
/// prefer non-chemical methods is art. 10's, and it obliges the choice rather
/// than an annotation. So this block is the format's own, and the register
/// exists because model 3.1 bis prints those columns for art. 10-11 GIP
/// compliance.
#[derive(Debug, Clone, Serialize)]
pub struct OtrasActuacionesFito {
    /// `TIPO_MEDIDA_FITOSANITARIA` code as an integer — the catalogue runs
    /// 1-12, 14, 15, with no 13.
    #[serde(rename = "TipoMedida")]
    pub tipo_medida: i64,
    /// The intensity: "Nº de trampas, nº de difusores, etc.". Nullable as a
    /// pair in the register and required as a pair here, because Anexo V grades
    /// fields 17 and 18 Obligatorio — the JSON Schema requires only
    /// `TipoMedida`, and this is the seam-4 cover-widths case again, where the
    /// grading rather than `required` decides (docs/siex-export.md → "The law
    /// outranks the format").
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    /// `UNIDADES_MEDIDA` code for that count, through
    /// `module_cue::siex::intensity_unit_to_siex`.
    #[serde(rename = "Unidad")]
    pub unidad: i64,
    /// The measure's own registration in the MDF registry (*medios de defensa
    /// fitosanitaria*, 1,235 rows and deliberately **not** vendored — the code
    /// is stored verbatim, so nothing needs it to resolve).
    #[serde(rename = "NumRegistroMDF", skip_serializing_if = "Option::is_none")]
    pub num_registro_mdf: Option<String>,
    //
    // `BuenasPracticas` is captured by nothing and has no member here: its
    // catalogue (`BUENAS_PRACTICAS_AMBITOS`, 97 rows) repeats each code once
    // per ámbito, so the code alone is not an identity and a single integer
    // cannot say which row was meant. Voluntario in Anexo V, and no printed
    // column anywhere.
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
    /// `EST_FENOLOGICO` code as an integer (the schema types it `number(2)` and
    /// validates it against the catalogue), so this is the catalogue's own code
    /// and NOT the BBCH stage that the book prints. Hangs off the DGC because
    /// the growth stage belongs to the treated crop — which is where
    /// Reglamento (UE) 2023/564's annex puts it too. Omitted when unstated.
    #[serde(rename = "EstadoFenologico", skip_serializing_if = "Option::is_none")]
    pub estado_fenologico: Option<i64>,
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
    /// ("nunca ambas"), converted by `siex::unit_to_siex`'s exact factor.
    ///
    /// Emitting Dosis is a CHOICE, not the only option: since 2026-08-04
    /// `treatment_record` also captures a total quantity used (Anexo III B.i,
    /// `total_quantity_value` + `_unit_code`), which is exactly what `Cantidad`
    /// is for. The dose is preferred because it is the value every record
    /// carries, while a total is absent whenever the dose is a concentration.
    /// If `Cantidad` is ever wanted instead, the data is already stored — see
    /// the dormant-export inventory in docs/siex-export.md.
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

// ---------------------------------------------------------------------------
// The three non-field registers (model sections 3.3, 3.4, 3.5) and the
// analytics one (section 4).
//
// They share TratamFito's vocabulary and almost none of its shapes, which is
// why they carry their own types rather than reusing its. Three differences
// cost a reader time if assumed away:
//
//   * their `EquipoAplicador` has NO required member and NO `AplicacionManual`
//     — it carries `EquipoPropio` instead, which Anexo V grades Voluntario;
//   * `ProductosFito` differs per block: post-harvest has no `Dosis` at all and
//     requires `Cantidad`, while the buildings one accepts either and requires
//     only `Unidad`;
//   * their `ProblematicaFito` is NARROWER — neither carries `MalasHierbas`,
//     and the buildings block has no `ReguladoresOtros` either. A record whose
//     problem cannot be expressed is refused by the precheck, never dropped.
// ---------------------------------------------------------------------------

/// Model 3.3 — a treatment applied to harvested produce.
#[derive(Debug, Clone, Serialize)]
pub struct TratamientoPostCosecha {
    #[serde(rename = "IdAjenaTratamPostco")]
    pub id_ajena_tratam_postco: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FechaActuacion")]
    pub fecha_actuacion: String,
    /// PROD_VEGETAL code of the produce treated (Anexo V: "Producto vegetal
    /// tratado según catálogo SIEX"), which is NOT the crop catalogue.
    #[serde(rename = "ProductoVegetal")]
    pub producto_vegetal: i64,
    /// The produce treated, in KILOGRAMS: Anexo V fixes the unit ("UNIDADES
    /// VÁLIDAS: kg") and the block carries no unit member, while the printed
    /// model asks for tonnes — so the builder converts.
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    #[serde(rename = "ProblematicaFito")]
    pub problematica_fito: ProblematicaPostCosecha,
    #[serde(rename = "Justificaciones")]
    pub justificaciones: Vec<Justificacion>,
    #[serde(rename = "ProductosFito")]
    pub productos_fito: Vec<ProductoFitoCantidad>,
    #[serde(rename = "IdentificadorAplicador")]
    pub identificador_aplicador: Vec<IdentificadorAplicadorNoField>,
    #[serde(rename = "AsesorValidacion", skip_serializing_if = "Option::is_none")]
    pub asesor_validacion: Option<AsesorValidacion>,
    #[serde(rename = "Eficacia")]
    pub eficacia: i64,
    #[serde(rename = "Observaciones", skip_serializing_if = "Option::is_none")]
    pub observaciones: Option<String>,
}

/// Models 3.4 and 3.5 — a treatment applied to a building or a vehicle.
#[derive(Debug, Clone, Serialize)]
pub struct TratamientoEdifInstalaciones {
    #[serde(rename = "IdAjenaTratamEdif")]
    pub id_ajena_tratam_edif: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// `1..n` in the schema. One record treats one place here, so exactly one.
    #[serde(rename = "Edificaciones")]
    pub edificaciones: Vec<Edificacion>,
    #[serde(rename = "FechaActuacion")]
    pub fecha_actuacion: String,
    #[serde(rename = "ProblematicaFito")]
    pub problematica_fito: ProblematicaEdificacion,
    #[serde(rename = "Justificaciones")]
    pub justificaciones: Vec<Justificacion>,
    #[serde(rename = "ProductosFito")]
    pub productos_fito: Vec<ProductoFitoCantidad>,
    #[serde(rename = "IdentificadorAplicador")]
    pub identificador_aplicador: Vec<IdentificadorAplicadorNoField>,
    #[serde(rename = "AsesorValidacion", skip_serializing_if = "Option::is_none")]
    pub asesor_validacion: Option<AsesorValidacion>,
    #[serde(rename = "Eficacia")]
    pub eficacia: i64,
    #[serde(rename = "Observaciones", skip_serializing_if = "Option::is_none")]
    pub observaciones: Option<String>,
}

/// The building treated, identified by REA's own code for it.
///
/// `IdEdificacion` is NOT a client alias: the REA structure types the same
/// field as "Código del edificio/instalación en el REA", and Anexo V puts this
/// whole block under a subloque called "Instalación identificada en el REA". It
/// comes from `premises_es_extension.rea_installation_code`, user-entered from
/// the farmer's own REA papers, and the precheck refuses a record without one.
#[derive(Debug, Clone, Serialize)]
pub struct Edificacion {
    #[serde(rename = "IdEdificacion")]
    pub id_edificacion: i64,
    /// B.f's treated volume, when the whole building was not treated.
    #[serde(rename = "Volumen", skip_serializing_if = "Option::is_none")]
    pub volumen: Option<f64>,
    /// UNIDADES_MEDIDA code for that volume ("m3 o m2" per the descriptor).
    #[serde(rename = "Unidad", skip_serializing_if = "Option::is_none")]
    pub unidad: Option<i64>,
}

/// Model 3.2 — treated seed, which the format models as an event of its own
/// rather than as part of the sowing (see docs/siex-export.md).
#[derive(Debug, Clone, Serialize)]
pub struct UsoSemillaTratada {
    #[serde(rename = "IdAjenaSemillaTrat")]
    pub id_ajena_semilla_trat: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// TIPO_TRATAMIENTO: where the seed was treated.
    #[serde(rename = "Tratamiento")]
    pub tratamiento: i64,
    #[serde(rename = "Fecha")]
    pub fecha: String,
    /// Anexo V field 1: "Cultivo — código del cultivo del catálogo SIEX", so
    /// this is the PRODUCTOS code of what was sown, despite the member's name.
    #[serde(rename = "Producto")]
    pub producto: i64,
    #[serde(rename = "NumeroLote", skip_serializing_if = "Option::is_none")]
    pub numero_lote: Option<String>,
    /// Seed weight in kilograms (Anexo V: "UNIDADES VÁLIDAS: kg").
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    #[serde(rename = "Eficacia")]
    pub eficacia: i64,
    #[serde(rename = "Observaciones", skip_serializing_if = "Option::is_none")]
    pub observaciones: Option<String>,
}

/// Model 4 — a laboratory analysis. The whole block is Voluntario in Anexo V
/// ("en caso de haberse realizado") and the schema requires only the alias, the
/// material and the date, both of which our register stores NOT NULL — so this
/// is the one block of the four that needs no precheck rule at all.
#[derive(Debug, Clone, Serialize)]
pub struct Analitica {
    #[serde(rename = "IdAjenaAna")]
    pub id_ajena_ana: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// MATERIAL_ANALIZADO: crop, harvested produce, soil or water.
    #[serde(rename = "MaterialAnalizado")]
    pub material_analizado: i64,
    #[serde(rename = "Fecha")]
    pub fecha: String,
    #[serde(rename = "Laboratorio", skip_serializing_if = "Option::is_none")]
    pub laboratorio: Option<String>,
    #[serde(rename = "Nif", skip_serializing_if = "Option::is_none")]
    pub nif: Option<String>,
    #[serde(rename = "NumBoletin", skip_serializing_if = "Option::is_none")]
    pub num_boletin: Option<String>,
    #[serde(rename = "TiposAnalisis", skip_serializing_if = "Vec::is_empty")]
    pub tipos_analisis: Vec<TipoAnalisis>,
    #[serde(rename = "TiposSustancias", skip_serializing_if = "Vec::is_empty")]
    pub tipos_sustancias: Vec<TipoSustancia>,
    /// Anexo III A.3's soil minimums, which live on the same record here
    /// because the twin puts them inside the analysis too.
    #[serde(rename = "ParametrosSuelo", skip_serializing_if = "Option::is_none")]
    pub parametros_suelo: Option<ParametrosSuelo>,
    #[serde(rename = "DGCs", skip_serializing_if = "Vec::is_empty")]
    pub dgcs: Vec<DgcAnalitica>,
}

/// Model 5 — what left the holding, from core's `harvest_record`.
///
/// The twin of the SALE, not of the harvesting operation: `Cosecha` is the
/// field work, with thirteen Obligatorio fields no decree asks a cuaderno to
/// keep, and nothing here fills it.
///
/// Two members our register holds are **not** sent. `TipoVenta` (1 = cosecha
/// comercializada, 0 = venta directa) is optional, Voluntario, and unstored —
/// the printed model draws no such distinction. `NumFactura` and `NumLote`
/// appear in the descriptor sheet and **not in the JSON Schema**, so the schema
/// wins (the 2026-07-14 re-diff rule) and `delivery_note_ref` / `lot_number`
/// stay printed-only.
#[derive(Debug, Clone, Serialize)]
pub struct ComercializacionVd {
    #[serde(rename = "IdAjenaVenta")]
    pub id_ajena_venta: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// One sale date, sent as both ends: the register keeps a single
    /// `harvested_on` because the model prints one "Fecha" column.
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// PROD_VEGETAL code — the harvested produce ("Granos de trigo"), never the
    /// PRODUCTOS crop code ("TRIGO BLANDO") that codes what grows.
    #[serde(rename = "ProductoVegetal")]
    pub producto_vegetal: i64,
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    /// UNIDADES_MEDIDA code; the block carries its own unit, unlike
    /// `TratamientosPostCosecha`, so the stored kg or t is sent as it stands.
    #[serde(rename = "Unidad")]
    pub unidad: i64,
}

/// Model 9.3's own register (core's `sowing_record`) — how a crop began.
///
/// `SiembraPlantacion` the MEMBER is not the crop: the WS descriptor types it
/// `number(1)`, "1 Siembra 0 Plantación". Anexo V's "Cultivo" field is
/// `DGCs[].CodigoCultivo`, per DGC, which is where this block puts it.
///
/// Three members come from model 3.2's register rather than from this one,
/// through `seed_treatment.sowing_record_id`: `MaterialTratado` (a linked
/// record exists), `MaterialAdquirido` (TIPO_TRATAMIENTO 4/5 are literally
/// "adquisición de semilla tratada") and `FechaAdquisicion`. The seed's facts
/// belong to the seed's register; this block is where the format asks for them.
///
/// Members captured by nothing, each for its own reason: `SiembraDirecta` is
/// already a `cultural_operation` of kind `no_tillage`; `DosisSiembra`,
/// `MarcoPlantacion`, `DensidadPlantacion` and `UnidadesRemolacha` are all
/// mutually exclusive alternatives to `Cantidad`, which is the one this
/// register stores; and `Maquinaria` is Voluntario in every field.
#[derive(Debug, Clone, Serialize)]
pub struct SiembraPlantacion {
    #[serde(rename = "IdAjenaSiembraPlant")]
    pub id_ajena_siembra_plant: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    /// 1 = siembra, 0 = plantación (`sowing_record.kind_code`).
    #[serde(rename = "SiembraPlantacion")]
    pub siembra_plantacion: i64,
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    /// `sowing_end_date`, falling back to the start: `None` there means one
    /// day's work, never "unknown".
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// Rice only, and normally filled by a correction weeks later.
    #[serde(rename = "FechaInundacion", skip_serializing_if = "Option::is_none")]
    pub fecha_inundacion: Option<String>,
    #[serde(rename = "MaterialTratado")]
    pub material_tratado: bool,
    #[serde(rename = "MaterialAdquirido")]
    pub material_adquirido: bool,
    /// Required by the schema, conditional in Anexo V ("en caso de haberse
    /// adquirido"). The property carries no `type`, so a null satisfies it —
    /// which is what an own-seed sowing sends, rather than a date it does not
    /// have. `Option<Option<String>>` would be the alternative; the member is
    /// always present and only its value varies.
    #[serde(rename = "FechaAdquisicion")]
    pub fecha_adquisicion: Option<String>,
    #[serde(rename = "NumLote", skip_serializing_if = "Option::is_none")]
    pub num_lote: Option<String>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcSiembra>,
    /// Kilograms of seed.
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
}

/// A sown plot+crop unit. Every member is optional in the schema, and the
/// surface is deliberately absent: `sowing_plot` mirrors `harvest_plot` and
/// carries none, because model 9.3 asks which parcels, not how much of each —
/// and the descriptor says `SuperficieCultivada` "es igual a la superficie DGC
/// salvo que se indique lo contrario", so omitting it states exactly that.
#[derive(Debug, Clone, Serialize)]
pub struct DgcSiembra {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
}

/// Model section 6 — one fertiliser application, or one accumulated period
/// (`fertilisation_record`).
///
/// The register was built SIEX-shaped, so most of this is a rename: the
/// composition is a coded junction because the twin has three arrays, the
/// service company and its REGFER number are stored together because the decree
/// puts them together, and `sludge_application` exists because
/// `AplicacionLodos` is required.
///
/// `BuenasPracticasRiego` is emitted by nothing: it is optional here AND on
/// `Riego`, Voluntario in both of Anexo V's blocks, and has no column in either
/// printed section — the recorded-gap rule, unchanged.
#[derive(Debug, Clone, Serialize)]
pub struct Fertilizacion {
    #[serde(rename = "IdAjenaFerti")]
    pub id_ajena_ferti: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// Anexo V field 5, Obligatorio and named by no decree.
    #[serde(rename = "GestionSostInsu")]
    pub gestion_sost_insu: bool,
    /// Required by the schema, but with no `minItems` — and Anexo V's own field
    /// 6 says the field "irá vacío" when no good practice was declared, so an
    /// empty array is the correct statement rather than a precheck failure.
    #[serde(rename = "BuenasPracticas")]
    pub buenas_practicas: Vec<BuenaPracticaFertilizante>,
    #[serde(
        rename = "MaterialFertilizante",
        skip_serializing_if = "Option::is_none"
    )]
    pub material_fertilizante: Option<MaterialFertilizante>,
    #[serde(
        rename = "AplicacionMaterialFertilizante",
        skip_serializing_if = "Option::is_none"
    )]
    pub aplicacion_material_fertilizante: Option<AplicacionMaterialFertilizante>,
    #[serde(rename = "EquipoAplicador", skip_serializing_if = "Option::is_none")]
    pub equipo_aplicador: Option<EquipoAplicadorFertilizacion>,
    /// Built from the linked `irrigation_record` when the application was a
    /// fertigation — the one act arts. 5.d and 5.e record twice.
    #[serde(rename = "Fertirrigacion", skip_serializing_if = "Option::is_none")]
    pub fertirrigacion: Option<Fertirrigacion>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcSuperficie>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuenaPracticaFertilizante {
    /// `BUENAS_PRACTICAS_AMBITOS` code in the "Fertilización" ámbito — the same
    /// integer means a different practice in each of the three ámbitos, which
    /// is why `fertilisation_practice` fixes the ámbito by existing.
    #[serde(rename = "TipoBPF")]
    pub tipo_bpf: i64,
}

/// The material as the registry holds it — the composition included, because
/// the twin puts it inside the application rather than in a registry of its own.
#[derive(Debug, Clone, Serialize)]
pub struct MaterialFertilizante {
    /// `MAT_FERTI`, the first coded level of Anexo III C.d.
    #[serde(rename = "Material")]
    pub material: i64,
    /// `DETALLE_MATERIAL_FERT`, the second level; absent for manures and
    /// own-farm materials, which the first level already answers.
    #[serde(rename = "DetalleMaterial", skip_serializing_if = "Option::is_none")]
    pub detalle_material: Option<i64>,
    #[serde(
        rename = "EmpresaSuministradora",
        skip_serializing_if = "Option::is_none"
    )]
    pub empresa_suministradora: Option<String>,
    /// Exactly one of these three identifies the supplier — the twin says
    /// "excluyente" three times and a CHECK on the registry row enforces it.
    #[serde(rename = "NifEmpresa", skip_serializing_if = "Option::is_none")]
    pub nif_empresa: Option<String>,
    #[serde(rename = "REGA", skip_serializing_if = "Option::is_none")]
    pub rega: Option<String>,
    #[serde(rename = "NIMA", skip_serializing_if = "Option::is_none")]
    pub nima: Option<String>,
    #[serde(
        rename = "TratamientoEstiercoles",
        skip_serializing_if = "Option::is_none"
    )]
    pub tratamiento_estiercoles: Option<i64>,
    #[serde(rename = "Macronutrientes", skip_serializing_if = "Vec::is_empty")]
    pub macronutrientes: Vec<Nutriente>,
    #[serde(rename = "Micronutrientes", skip_serializing_if = "Vec::is_empty")]
    pub micronutrientes: Vec<Nutriente>,
    #[serde(rename = "MetalesPesados", skip_serializing_if = "Vec::is_empty")]
    pub metales_pesados: Vec<Nutriente>,
    #[serde(rename = "Densidad", skip_serializing_if = "Option::is_none")]
    pub densidad: Option<f64>,
    /// The unit of that density. The column carries none because a fertiliser
    /// density is kg/L on every label; this is how the serializer still says so.
    #[serde(rename = "UnidadesMedida", skip_serializing_if = "Option::is_none")]
    pub unidades_medida: Option<i64>,
}

/// One entry of any of the three composition arrays. They differ only in which
/// catalogue their integer indexes, which is what `kind_code` records — so one
/// Rust type serializes into three differently-named members.
#[derive(Debug, Clone, Serialize)]
pub struct Nutriente {
    #[serde(rename = "TipoMacroN", skip_serializing_if = "Option::is_none")]
    pub tipo_macro_n: Option<i64>,
    #[serde(rename = "TipoMicroN", skip_serializing_if = "Option::is_none")]
    pub tipo_micro_n: Option<i64>,
    #[serde(rename = "TipoMetalP", skip_serializing_if = "Option::is_none")]
    pub tipo_metal_p: Option<i64>,
    #[serde(rename = "Porcentaje")]
    pub porcentaje: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AplicacionMaterialFertilizante {
    /// The material's own name. Anexo V restricts the field to inhibitors,
    /// liming and other amendments and biostimulants; it is Voluntario and free
    /// text, and the registry's name is a true answer for any material, so it
    /// is always sent rather than filtered on a catalogue reading.
    #[serde(rename = "NombreProducto", skip_serializing_if = "Option::is_none")]
    pub nombre_producto: Option<String>,
    #[serde(rename = "AplicacionLodos")]
    pub aplicacion_lodos: bool,
    /// `TIPO_FERITILIZACION` (the provider's own spelling) — C.c.
    #[serde(rename = "TipoFertilizacion")]
    pub tipo_fertilizacion: i64,
    /// `METODO_APLICACION_FERTILIZANTE` — C.f, the separate legal field the
    /// printed model merges into one letter.
    #[serde(rename = "MetodoFertilizacion")]
    pub metodo_fertilizacion: i64,
    #[serde(rename = "Dosis")]
    pub dosis: f64,
    #[serde(rename = "Unidad")]
    pub unidad: i64,
    /// The service company's REGFER number (C.k). The company's NAME has no
    /// member here, so it stays a printed-only column.
    #[serde(rename = "EmpresaServicios", skip_serializing_if = "Option::is_none")]
    pub empresa_servicios: Option<String>,
}

/// The applicator equipment. Omitted entirely when the record names no machine,
/// which C.g allows in so many words ("cuando proceda") — and which the `oneOf`
/// over the three identifiers makes the only correct way to say "no machine",
/// since a half-filled block would fail validation.
#[derive(Debug, Clone, Serialize)]
pub struct EquipoAplicadorFertilizacion {
    #[serde(rename = "NumROMA", skip_serializing_if = "Option::is_none")]
    pub num_roma: Option<String>,
    #[serde(rename = "IdEquipoAplicador", skip_serializing_if = "Option::is_none")]
    pub id_equipo_aplicador: Option<String>,
}

/// The water side of a fertigation, read from the linked `irrigation_record`.
///
/// Member for member this is `Riego` without its dates, alias and DGCs — plus
/// `DosisN`/`DosisP`, which are Anexo III **C.l**'s two water-quality figures
/// and appear in no printed column and in no other block. This is their only
/// reader anywhere in the format.
#[derive(Debug, Clone, Serialize)]
pub struct Fertirrigacion {
    #[serde(rename = "SistemaRiego")]
    pub sistema_riego: i64,
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    #[serde(rename = "Unidad")]
    pub unidad: i64,
    #[serde(rename = "OrigenAgua", skip_serializing_if = "Vec::is_empty")]
    pub origen_agua: Vec<OrigenAgua>,
    #[serde(rename = "NumContador", skip_serializing_if = "Option::is_none")]
    pub num_contador: Option<String>,
    /// mg/L of nitric nitrogen in the irrigation water, with its unit code.
    #[serde(rename = "DosisN")]
    pub dosis_n: f64,
    #[serde(rename = "UnidadDosisN")]
    pub unidad_dosis_n: i64,
    #[serde(rename = "DosisP")]
    pub dosis_p: f64,
    #[serde(rename = "UnidadDosisP")]
    pub unidad_dosis_p: i64,
    #[serde(rename = "TipoEnergia", skip_serializing_if = "Option::is_none")]
    pub tipo_energia: Option<i64>,
}

/// Model section 8 — one irrigation, or one accumulated period of them
/// (`irrigation_record`).
///
/// Two members the register holds go nowhere: `water_nitric_n_mg_l` and
/// `water_soluble_p2o5_mg_l` are Anexo V block 9's fields 41-42, which live on
/// `Fertirrigacion` inside the FERTILISATION block, not here — the twin asks
/// about the water only when it is carrying fertiliser.
#[derive(Debug, Clone, Serialize)]
pub struct Riego {
    #[serde(rename = "IdAjenaRiego")]
    pub id_ajena_riego: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    /// `irrigation_end_date`, falling back to the start: `None` there means one
    /// day's watering, never "unknown".
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// `SIST_RIEGO` code — how this watering was done, which is deliberately not
    /// core's plot-level `irrigation_system` (one of whose values is "rainfed").
    #[serde(rename = "SistemaRiego")]
    pub sistema_riego: i64,
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    /// `UNIDADES_MEDIDA`. Anexo V names m³ and L as the valid units while the
    /// register stores m³ or m³/ha; the catalogue carries m³/ha as its own code,
    /// so a per-hectare volume states itself rather than being converted with a
    /// surface the record may not carry.
    #[serde(rename = "UnidadMedida")]
    pub unidad_medida: i64,
    #[serde(rename = "OrigenAgua", skip_serializing_if = "Vec::is_empty")]
    pub origen_agua: Vec<OrigenAgua>,
    #[serde(rename = "TipoEnergia", skip_serializing_if = "Option::is_none")]
    pub tipo_energia: Option<i64>,
    #[serde(rename = "NumContador", skip_serializing_if = "Option::is_none")]
    pub num_contador: Option<String>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcSuperficie>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrigenAgua {
    #[serde(rename = "IdOrigenAgua")]
    pub id_origen_agua: i64,
}

/// Model section 7.1 — what the book records ABOUT the plan de abonado, which
/// is much less than the plan (`fertilisation_plan`).
///
/// The twin's required set is art. 5.a's four figures plus the preceding crop,
/// the tool flag and the date — the exchange format agreeing with the article
/// is what confirmed the book carries the summary and not the document art. 6
/// defines. Its optional `Asesor` (a REGFER code) and `FechaAsesoramiento` are
/// stored by nothing: art. 6.6's advice requirement is on the DOCUMENT, and the
/// register the decree describes names no advisor.
#[derive(Debug, Clone, Serialize)]
pub struct PlanAbonado {
    #[serde(rename = "IdAjenaPlan")]
    pub id_ajena_plan: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "NecesidadUFN")]
    pub necesidad_uf_n: f64,
    #[serde(rename = "NecesidadUFP2O5")]
    pub necesidad_uf_p2o5: f64,
    #[serde(rename = "NecesidadUFK2O")]
    pub necesidad_uf_k2o: f64,
    #[serde(rename = "ObjetivoProduccion")]
    pub objetivo_produccion: f64,
    /// PRODUCTOS code of the crop that preceded this one. Required by the
    /// schema and nullable in the register — a unit coming out of fallow has
    /// none — so the precheck asks for it rather than the serializer inventing
    /// a rotation that did not happen.
    #[serde(rename = "CultivoPrecedente")]
    pub cultivo_precedente: i64,
    /// Whether a digital nutrient-advice tool produced the plan. No decree asks
    /// for it; Anexo V marks it Obligatorio inside a block we do send.
    #[serde(rename = "Herramienta")]
    pub herramienta: bool,
    #[serde(rename = "FechaGeneracion")]
    pub fecha_generacion: String,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcPlan>,
}

/// The DGC shape `Fertilizacion` and `Riego` share — identical in the schema,
/// down to the optional `Superficie` that states a treated area differing from
/// the DGC's own.
#[derive(Debug, Clone, Serialize)]
pub struct DgcSuperficie {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
    /// Omitted when the register did not state one: both junctions leave the
    /// area nullable because naming the plot already says what was covered, and
    /// the descriptor reads an absent `Superficie` as "the DGC's own".
    #[serde(rename = "Superficie", skip_serializing_if = "Option::is_none")]
    pub superficie: Option<f64>,
}

/// `PlanAbonado`'s DGC, which carries no surface: a plan states doses per
/// hectare, so the area it covers adds nothing.
#[derive(Debug, Clone, Serialize)]
pub struct DgcPlan {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TipoAnalisis {
    #[serde(rename = "TipoAnalisis")]
    pub tipo_analisis: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TipoSustancia {
    #[serde(rename = "TipoSustancia")]
    pub tipo_sustancia: i64,
}

/// Every member optional, and every one of ours nullable: a bulletin states
/// what it measured, so an absent figure is absent rather than zero.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ParametrosSuelo {
    #[serde(rename = "MateriaOrganica", skip_serializing_if = "Option::is_none")]
    pub materia_organica: Option<f64>,
    #[serde(rename = "Arena", skip_serializing_if = "Option::is_none")]
    pub arena: Option<f64>,
    #[serde(rename = "Limo", skip_serializing_if = "Option::is_none")]
    pub limo: Option<f64>,
    #[serde(rename = "Arcilla", skip_serializing_if = "Option::is_none")]
    pub arcilla: Option<f64>,
    #[serde(rename = "Ph", skip_serializing_if = "Option::is_none")]
    pub ph: Option<f64>,
    #[serde(rename = "FosforoAsimilable", skip_serializing_if = "Option::is_none")]
    pub fosforo_asimilable: Option<f64>,
    #[serde(rename = "PotasioAsimilable", skip_serializing_if = "Option::is_none")]
    pub potasio_asimilable: Option<f64>,
    #[serde(rename = "NitrogenoTotal", skip_serializing_if = "Option::is_none")]
    pub nitrogeno_total: Option<f64>,
    #[serde(rename = "Conductividad", skip_serializing_if = "Option::is_none")]
    pub conductividad: Option<f64>,
}

impl ParametrosSuelo {
    /// Whether the bulletin stated any soil figure at all — an all-absent block
    /// is omitted rather than sent empty.
    pub fn is_empty(&self) -> bool {
        self.materia_organica.is_none()
            && self.arena.is_none()
            && self.limo.is_none()
            && self.arcilla.is_none()
            && self.ph.is_none()
            && self.fosforo_asimilable.is_none()
            && self.potasio_asimilable.is_none()
            && self.nitrogeno_total.is_none()
            && self.conductividad.is_none()
    }
}

/// An analysis' DGC reference. Unlike `TratamFito`'s, every member is optional
/// here, so a sample taken off a plot with no crop still names the plot.
#[derive(Debug, Clone, Serialize)]
pub struct DgcAnalitica {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    /// PRODUCTOS code of the crop sampled, when the crop row carries one.
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
}

/// Post-harvest's problem buckets: no `MalasHierbas` (the schema has none).
#[derive(Debug, Clone, Serialize)]
pub struct ProblematicaPostCosecha {
    #[serde(rename = "Enfermedades", skip_serializing_if = "Option::is_none")]
    pub enfermedades: Option<Enfermedades>,
    #[serde(
        rename = "ArtropodosGasteropodos",
        skip_serializing_if = "Option::is_none"
    )]
    pub artropodos_gasteropodos: Option<ArtropodosGasteropodos>,
    #[serde(rename = "ReguladoresOtros", skip_serializing_if = "Option::is_none")]
    pub reguladores_otros: Option<ReguladoresOtros>,
}

/// A building's problem buckets: diseases and pests only.
#[derive(Debug, Clone, Serialize)]
pub struct ProblematicaEdificacion {
    #[serde(rename = "Enfermedades", skip_serializing_if = "Option::is_none")]
    pub enfermedades: Option<Enfermedades>,
    #[serde(
        rename = "ArtropodosGasteropodos",
        skip_serializing_if = "Option::is_none"
    )]
    pub artropodos_gasteropodos: Option<ArtropodosGasteropodos>,
}

/// `ProductosFito` for the non-field blocks, which state the AMOUNT used
/// rather than a dose per surface — model 3.3/3.4/3.5's own "Cantidad
/// utilizada (kg o l)", which is exactly what our register captures.
#[derive(Debug, Clone, Serialize)]
pub struct ProductoFitoCantidad {
    #[serde(rename = "TipoProducto")]
    pub tipo_producto: i64,
    #[serde(rename = "NumRegistro", skip_serializing_if = "Option::is_none")]
    pub num_registro: Option<String>,
    #[serde(rename = "MateriaActiva", skip_serializing_if = "Option::is_none")]
    pub materia_activa: Option<i64>,
    #[serde(rename = "Cantidad")]
    pub cantidad: f64,
    #[serde(rename = "Unidad")]
    pub unidad: i64,
}

/// The non-field applicator block. `AplicacionManual` does not exist here — it
/// is `TratamFito`'s — but the same `oneOf` over the three equipment
/// identifiers does, so exactly one is always named, sentinel included.
#[derive(Debug, Clone, Serialize)]
pub struct IdentificadorAplicadorNoField {
    #[serde(rename = "AplicadorEmpresa")]
    pub aplicador_empresa: AplicadorEmpresa,
    #[serde(rename = "EquipoAplicador")]
    pub equipo_aplicador: EquipoAplicadorNoField,
}

#[derive(Debug, Clone, Serialize)]
pub struct EquipoAplicadorNoField {
    #[serde(rename = "NumROMA", skip_serializing_if = "Option::is_none")]
    pub num_roma: Option<String>,
    #[serde(rename = "NumREGANIP", skip_serializing_if = "Option::is_none")]
    pub num_reganip: Option<String>,
    #[serde(rename = "IdEquipoAplicador", skip_serializing_if = "Option::is_none")]
    pub id_equipo_aplicador: Option<String>,
}

/// The advisor identified on the actuation (Anexo III Parte I B.d, which
/// reaches these registers through B.b and B.f).
///
/// Only `NumROPO` is required, and only `NumROPO` is sent: `Validacion`,
/// `Confirmacion` and `Contrato` describe a sign-off the book cannot hold,
/// because model 3.1 bis asks for a handwritten signature and this app has no
/// signature capability by design. Claiming any of them would be inventing the
/// one thing the advisor block exists to attest.
#[derive(Debug, Clone, Serialize)]
pub struct AsesorValidacion {
    #[serde(rename = "NumROPO")]
    pub num_ropo: String,
}

/// `Pastoreo` — model 9.1, RD 1048/2022 art. 30.2 ter.
///
/// `FechaFin` is REQUIRED here while `grazing_record.ended_on` is nullable,
/// because the decree's month runs from *"la nueva fecha de inicio o fin que
/// haya resultado de la modificación"* — so a grazing still under way is not
/// late, it is unfinished. The format has no shape for an unfinished grazing, so
/// the precheck names those records and the export refuses; nothing is invented
/// and nothing is dropped in silence.
#[derive(Debug, Clone, Serialize)]
pub struct Pastoreo {
    #[serde(rename = "IdAjenaPastoreo")]
    pub id_ajena_pastoreo: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// Derived from each animal line's REGA against the holding's own, never
    /// stored: the two columns that once held head counts were dropped on
    /// 2026-08-20 when the refreshed descriptor showed the twin asks *whether*,
    /// not *how many*. The descriptor also forbids both being false, which the
    /// register's own "at least one animal line" rule guarantees — given the
    /// farm states its REGA, which the precheck demands of a season holding
    /// grazings.
    #[serde(rename = "AnimalesPropios")]
    pub animales_propios: bool,
    #[serde(rename = "AnimalesTerceros")]
    pub animales_terceros: bool,
    #[serde(rename = "Animales", skip_serializing_if = "Vec::is_empty")]
    pub animales: Vec<Animal>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcActividad>,
}

/// One animal group of a grazing: `grazing_animal`, which is a junction exactly
/// because the twin is an array.
#[derive(Debug, Clone, Serialize)]
pub struct Animal {
    #[serde(rename = "REGA")]
    pub rega: String,
    #[serde(rename = "Numero")]
    pub numero: i64,
    /// `ESPECIE_ANIMAL` code as an integer. Stored verbatim and deliberately
    /// unvalidated at insert (the provider-registry rule), so the precheck is
    /// what catches a value this required member cannot carry.
    #[serde(rename = "Especie")]
    pub especie: i64,
}

/// `LaboresCulturales` — model 9.2, the book's own "9.6", and 9.4's mechanical
/// maintenance, all of which are one register (`cultural_operation`).
///
/// `Maquinaria[]` is emitted by nothing: Anexo V grades all six of its fields
/// Voluntario and no printed page has a column for any of them, so the register
/// records no machinery.
#[derive(Debug, Clone, Serialize)]
pub struct LaborCultural {
    #[serde(rename = "IdAjenaLabor")]
    pub id_ajena_labor: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FechaInicio")]
    pub fecha_inicio: String,
    #[serde(rename = "FechaFin")]
    pub fecha_fin: String,
    /// `TIPO_LABOR`, through `module_ecoscheme::siex`. The map is deliberately
    /// not injective: `mowing` and `brush_cutting` both answer to 5, "Desbroce y
    /// siega", which is one code for what model 9.4 prints as two columns.
    #[serde(rename = "TipoLabor")]
    pub tipo_labor: i64,
    /// Both DERIVED from `residue_destination_code` read together with the kind,
    /// and both always sent: Anexo V grades them Obligatorio, and a boolean
    /// omitted is a boolean unanswered.
    #[serde(rename = "DepositadoSueloDesb")]
    pub depositado_suelo_desb: bool,
    #[serde(rename = "DepositadoSueloPoda")]
    pub depositado_suelo_poda: bool,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcActividad>,
}

/// `DatosCubierta` — models 9.4 and 9.5, RD 1048/2022 arts. 42 and 43.
///
/// The three optional members are nullable as a group in the register, because
/// art. 42.1.e falls due *"en el mes anterior al final del periodo mínimo de
/// cuatro meses"* while 42.1.a is due within a month of establishment — so a
/// cover between the two deadlines is a complete record with no widths. The
/// export nonetheless refuses one: Anexo V grades both widths **Obligatorio**
/// for exactly the three cover types this register can hold, and that grading is
/// the test this arc settled on (docs/siex-export.md → "The law outranks the
/// format"). The record book prints such a cover and never gates; the two
/// documents answer to different rules, which is the whole reason they are
/// separate crates.
///
/// `ActividadCubierta[]` is emitted by nothing, and that is not an oversight:
/// the descriptor sheet declares it `1..n` and the JSON Schema has no such
/// member at all — a live disagreement inside 3.11.4, settled by the standing
/// rule that the schema is what validates.
#[derive(Debug, Clone, Serialize)]
pub struct DatosCubierta {
    #[serde(rename = "IdAjenaCubierta")]
    pub id_ajena_cubierta: i64,
    #[serde(rename = "Borrar", skip_serializing_if = "Option::is_none")]
    pub borrar: Option<bool>,
    #[serde(rename = "FecEstablecimientoCub")]
    pub fec_establecimiento_cub: String,
    #[serde(rename = "AnchuraCubierta", skip_serializing_if = "Option::is_none")]
    pub anchura_cubierta: Option<f64>,
    #[serde(rename = "AnchuraLibreProy", skip_serializing_if = "Option::is_none")]
    pub anchura_libre_proy: Option<f64>,
    /// `TIPO_COBERTURA_SUELO`, stored verbatim because this member sends that
    /// very code.
    #[serde(rename = "TipoCobertura", skip_serializing_if = "Option::is_none")]
    pub tipo_cobertura: Option<i64>,
    #[serde(rename = "DGCs")]
    pub dgcs: Vec<DgcCubierta>,
}

/// The DGC shape `Pastoreo` and `LaboresCulturales` share — the two blocks whose
/// Anexo V rows carry an "Actividad en la cubierta" subloque, which is what
/// `Cubiertas` is and what distinguishes this from [`DgcSuperficie`].
///
/// `Superficie` is always omitted. Neither junction has a surface column —
/// model 9.1 asks for the parcel reference and 9.2 prints the plot's own SIGPAC
/// surface — and the descriptor reads an absent `Superficie` as the DGC's own
/// ("es igual a la superficie DGC salvo que se indique lo contrario"), so
/// omitting states what the register knows. Sending the crop's `area_ha`
/// instead would assert that every hectare of it was grazed or worked, which no
/// row here says.
#[derive(Debug, Clone, Serialize)]
pub struct DgcActividad {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
    /// Present only when the record maintained a cover (`soil_cover_id`), which
    /// is per record — so the descriptor's rule that one activity may not mix
    /// DGCs with and without a cover holds by construction.
    #[serde(rename = "Cubiertas", skip_serializing_if = "Vec::is_empty")]
    pub cubiertas: Vec<Cubierta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cubierta {
    #[serde(rename = "TipoCobertura")]
    pub tipo_cobertura: i64,
}

/// `DatosCubierta`'s own DGC, which carries neither a surface nor a `Cubiertas`
/// array: the block IS the cover, so restating one inside it would be circular.
#[derive(Debug, Clone, Serialize)]
pub struct DgcCubierta {
    #[serde(rename = "CodigoDGCAjena", skip_serializing_if = "Option::is_none")]
    pub codigo_dgc_ajena: Option<i64>,
    #[serde(rename = "CodigoCultivo", skip_serializing_if = "Option::is_none")]
    pub codigo_cultivo: Option<i64>,
}
