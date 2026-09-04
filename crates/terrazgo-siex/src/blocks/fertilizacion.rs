// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `Fertilizacion` — model section 6, the fertilisation register.
//!
//! Mostly wiring, because the register was designed against this block: the
//! composition is one coded junction because the twin has three arrays keyed on
//! three catalogues, `sludge_application` exists because `AplicacionLodos` is
//! required, and the service company keeps its REGFER number beside it because
//! the decree does — the twin splits them and this module puts each half where
//! the format wants it.
//!
//! **The composition comes from the REGISTRY row, not from the record.** The
//! record freezes only what section 6 prints (the name, the model's three
//! richness figures); C.h's eight values and C.i's heavy metals stay on the
//! material, which is soft-deleted rather than removed so a decade-old record
//! still resolves what it named.
//!
//! **`Fertirrigacion` is the water side of one act the decree records twice.**
//! Art. 5.d puts the fertiliser here and art. 5.e puts the water in
//! `irrigation_record`; the format re-joins them, and this block is the only
//! reader anywhere of Anexo III C.l's two water-quality figures. The farmer
//! states the join on the §6 form and the precheck demands it whenever the
//! application method is a fertigation — which asks for nothing new, since art.
//! 5.e already obliges the irrigation record for that watering.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex as cue_siex;
use module_fertilisation::models::{FertilisationRecordDetail, IrrigationRecordDetail};
use module_fertilisation::repository::{
    get_fertiliser_material_for_export, get_irrigation_record,
    list_fertilisation_records_for_export,
};
use module_fertilisation::siex;
use rusqlite::{Connection, OptionalExtension};
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<Fertilizacion>> {
    let mut out = Vec::new();
    for detail in list_fertilisation_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &FertilisationRecordDetail,
    actor: Option<&str>,
) -> Result<Option<Fertilizacion>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "fertilisation_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(
            conn,
            SIEX_TARGET,
            "fertilisation_record",
            &record.id,
            "",
            actor,
        )?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let fecha_inicio = cue_siex::date_to_siex(&record.applied_on).ok_or_else(unmappable)?;
    let fecha_fin = match record.application_end_date.as_deref() {
        Some(end) => cue_siex::date_to_siex(end).ok_or_else(unmappable)?,
        None => fecha_inicio.clone(),
    };

    // `BUENAS_PRACTICAS_AMBITOS` codes, stored verbatim in the "Fertilización"
    // ámbito. An empty list is a legal statement here, not a gap.
    let buenas_practicas = detail
        .practices
        .iter()
        .map(|code| {
            code.trim()
                .parse::<i64>()
                .map(|tipo_bpf| BuenaPracticaFertilizante { tipo_bpf })
                .map_err(|_| unmappable())
        })
        .collect::<Result<Vec<_>>>()?;

    let material = material_block(conn, &record.fertiliser_material_id)?;
    let aplicacion = AplicacionMaterialFertilizante {
        nombre_producto: Some(record.material_name_snapshot.clone()),
        aplicacion_lodos: record.sludge_application,
        tipo_fertilizacion: siex::fertilisation_type_to_siex(&record.fertilisation_type_code)
            .ok_or_else(unmappable)?,
        metodo_fertilizacion: siex::application_method_to_siex(&record.application_method_code)
            .ok_or_else(unmappable)?,
        dosis: record.dose_value,
        unidad: siex::dose_unit_to_siex(&record.dose_unit_code).ok_or_else(unmappable)?,
        empresa_servicios: record.service_regfer_number.clone(),
    };

    let fertirrigacion = match record.irrigation_record_id.as_deref() {
        Some(id) => fertigation_block(conn, id)?,
        None => None,
    };

    let dgcs = detail
        .plots
        .iter()
        .map(|plot| {
            crate::blocks::dgc_superficie(
                conn,
                plot.crop_id.as_deref(),
                plot.fertilised_area_ha,
                deleted,
                actor,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(Fertilizacion {
        id_ajena_ferti: alias,
        borrar: deleted.then_some(true),
        fecha_inicio,
        fecha_fin,
        gestion_sost_insu: record.sustainable_input_management,
        buenas_practicas,
        material_fertilizante: Some(material),
        aplicacion_material_fertilizante: Some(aplicacion),
        equipo_aplicador: equipment_block(conn, record.machinery_id.as_deref())?,
        fertirrigacion,
        dgcs,
    }))
}

/// The material as the registry holds it, composition included.
fn material_block(conn: &Connection, material_id: &str) -> Result<MaterialFertilizante> {
    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let detail = get_fertiliser_material_for_export(conn, material_id)?;
    let material = &detail.material;

    let of_kind = |kind: &str| -> Vec<Nutriente> {
        detail
            .nutrients
            .iter()
            .filter(|row| row.kind_code == kind)
            .filter_map(|row| {
                let code = row.nutrient_code.trim().parse::<i64>().ok()?;
                Some(Nutriente {
                    tipo_macro_n: (kind == "macro").then_some(code),
                    tipo_micro_n: (kind == "micro").then_some(code),
                    tipo_metal_p: (kind == "heavy_metal").then_some(code),
                    porcentaje: row.percentage,
                })
            })
            .collect()
    };

    Ok(MaterialFertilizante {
        material: material
            .material_code
            .trim()
            .parse::<i64>()
            .map_err(|_| unmappable())?,
        detalle_material: material
            .material_detail_code
            .as_deref()
            .and_then(|code| code.trim().parse::<i64>().ok()),
        empresa_suministradora: material.supplier_name.clone(),
        nif_empresa: material.supplier_tax_id.clone(),
        rega: material.supplier_rega.clone(),
        nima: material.supplier_nima.clone(),
        tratamiento_estiercoles: material
            .manure_treatment_code
            .as_deref()
            .and_then(siex::manure_treatment_to_siex),
        macronutrientes: of_kind("macro"),
        micronutrientes: of_kind("micro"),
        metales_pesados: of_kind("heavy_metal"),
        densidad: material.density_kg_l,
        // Stated only beside a density, because that is the only thing it is the
        // unit of.
        unidades_medida: material.density_kg_l.map(|_| siex::DENSITY_UNIT_SIEX),
    })
}

/// The machine, or nothing at all. C.g makes it optional ("cuando proceda") and
/// the `oneOf` makes a half-filled block invalid, so "no machine" is said by
/// omitting the block — never by sending it empty.
fn equipment_block(
    conn: &Connection,
    machinery_id: Option<&str>,
) -> Result<Option<EquipoAplicadorFertilizacion>> {
    let Some(machinery_id) = machinery_id else {
        return Ok(None);
    };
    // Read live rather than snapshotted: this record freezes no machinery
    // number, and a corrected ROMA means the past record named the right machine
    // with a wrong number (docs/data-model.md → "Nothing is ever frozen").
    let roma: Option<String> = conn
        .query_row(
            "SELECT roma_number FROM machinery_es_extension WHERE machinery_id = ?1",
            [machinery_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(Some(match roma {
        Some(roma) if !roma.trim().is_empty() => EquipoAplicadorFertilizacion {
            num_roma: Some(roma),
            id_equipo_aplicador: None,
        },
        // Not in ROMA: named by its stable row id, which never drifts between
        // exports. `IdEquipoAplicador` is a free string(50) for exactly this.
        _ => EquipoAplicadorFertilizacion {
            num_roma: None,
            id_equipo_aplicador: Some(machinery_id.to_string()),
        },
    }))
}

/// The water side, from the linked irrigation record.
///
/// A link to a withdrawn watering yields nothing: the record that carried the
/// statement is gone, and inventing the water from a deleted row would assert
/// what the farmer retracted. An ACTIVE fertigation in that state is
/// precheck-blocked, so this only goes quiet on a deletion entry.
fn fertigation_block(
    conn: &Connection,
    irrigation_record_id: &str,
) -> Result<Option<Fertirrigacion>> {
    let detail: IrrigationRecordDetail = match get_irrigation_record(conn, irrigation_record_id) {
        Ok(detail) => detail,
        Err(module_fertilisation::error::FertilisationError::NotFound) => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let record = &detail.record;
    let unmappable = || SiexError::Invalid("export_code_unmappable");

    // Both required by the schema, both nullable here under art. 17.2, and both
    // demanded by the precheck for an active fertigation.
    let (Some(dosis_n), Some(dosis_p)) =
        (record.water_nitric_n_mg_l, record.water_soluble_p2o5_mg_l)
    else {
        return Ok(None);
    };

    Ok(Some(Fertirrigacion {
        sistema_riego: siex::irrigation_method_to_siex(&record.irrigation_method_code)
            .ok_or_else(unmappable)?,
        cantidad: record.volume_value,
        unidad: siex::volume_unit_to_siex(&record.volume_unit_code).ok_or_else(unmappable)?,
        origen_agua: detail
            .water_origins
            .iter()
            .map(|origin| {
                siex::water_origin_to_siex(origin)
                    .map(|id_origen_agua| OrigenAgua { id_origen_agua })
                    .ok_or_else(unmappable)
            })
            .collect::<Result<Vec<_>>>()?,
        num_contador: record.meter_number.clone(),
        dosis_n,
        unidad_dosis_n: siex::WATER_CONCENTRATION_UNIT_SIEX,
        dosis_p,
        unidad_dosis_p: siex::WATER_CONCENTRATION_UNIT_SIEX,
        tipo_energia: record
            .energy_type_code
            .as_deref()
            .and_then(|code| code.trim().parse::<i64>().ok()),
    }))
}
