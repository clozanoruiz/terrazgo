// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `Analitica` — model section 4, plus A.3's soil block.
//!
//! The one block of seam 1 that adds no precheck rule. Anexo V grades all eight
//! of its fields Voluntario ("en caso de haberse realizado") and the schema
//! requires only the alias, the material and the date — and the register stores
//! both of the latter NOT NULL, so every analysis a farmer records can be sent
//! as it stands. Everything else is emitted when present and omitted when not,
//! which is what an analysis bulletin is: a statement of what was measured.
//!
//! Its DGCs are looser than `TratamFito`'s too. Every member is optional here,
//! so a soil sample taken on a plot carrying no crop still names the plot,
//! where a treatment on a cropless plot is precheck-blocked.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::models::{AnalysisPlot, AnalysisRecordDetail};
use module_cue::repository::list_analysis_records_for_export;
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<Analitica>> {
    let mut out = Vec::new();
    for detail in list_analysis_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &AnalysisRecordDetail,
    actor: Option<&str>,
) -> Result<Option<Analitica>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "analysis_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "analysis_record", &record.id, "", actor)?
    };

    let material_analizado = siex::analysis_material_to_siex(&record.material_kind_code)
        .ok_or(SiexError::Invalid("export_code_unmappable"))?;
    let tipos_analisis = detail
        .types
        .iter()
        .map(|row| {
            siex::analysis_type_to_siex(&row.analysis_type_code)
                .map(|tipo_analisis| TipoAnalisis { tipo_analisis })
                .ok_or(SiexError::Invalid("export_code_unmappable"))
        })
        .collect::<Result<Vec<_>>>()?;
    // SUST_ACTIVAS codes, stored verbatim without a foreign key. An unparseable
    // one is refused rather than dropped: unlike the growth stage, this is the
    // finding itself — the substance the laboratory detected.
    let tipos_sustancias = detail
        .substances
        .iter()
        .map(|row| {
            row.substance_code
                .trim()
                .parse::<i64>()
                .map(|tipo_sustancia| TipoSustancia { tipo_sustancia })
                .map_err(|_| SiexError::Invalid("export_code_unmappable"))
        })
        .collect::<Result<Vec<_>>>()?;

    let soil = &record.soil;
    let parametros_suelo = ParametrosSuelo {
        materia_organica: soil.organic_matter_pct,
        arena: soil.sand_pct,
        limo: soil.silt_pct,
        arcilla: soil.clay_pct,
        ph: soil.ph,
        fosforo_asimilable: soil.available_p_mg_kg,
        potasio_asimilable: soil.available_k_mg_kg,
        nitrogeno_total: soil.total_n_pct,
        conductividad: soil.conductivity_ds_m,
    };

    let dgcs = detail
        .plots
        .iter()
        .map(|plot| dgc(conn, plot, deleted, actor))
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(Analitica {
        id_ajena_ana: alias,
        borrar: deleted.then_some(true),
        material_analizado,
        fecha: siex::date_to_siex(&record.sampled_on)
            .ok_or(SiexError::Invalid("export_code_unmappable"))?,
        laboratorio: record.lab_name.clone(),
        nif: record.lab_tax_id.clone(),
        num_boletin: record.bulletin_number.clone(),
        tipos_analisis,
        tipos_sustancias,
        parametros_suelo: (!parametros_suelo.is_empty()).then_some(parametros_suelo),
        dgcs,
    }))
}

/// One sampled plot+crop. `CodigoCultivo` is the crop's own PRODUCTOS code,
/// read live rather than snapshotted: it identifies the species for the
/// authority, and a crop row whose code was corrected should export the
/// corrected one (the record's frozen `crop_name_snapshot` is what the BOOK
/// prints, and that is a separate question).
fn dgc(
    conn: &mut Connection,
    plot: &AnalysisPlot,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcAnalitica> {
    let (codigo_dgc_ajena, codigo_cultivo) = match &plot.crop_id {
        Some(crop_id) => {
            let alias = if deleted {
                find_export_alias(conn, SIEX_TARGET, "crop", crop_id, "")?
            } else {
                Some(ensure_export_alias(
                    conn,
                    SIEX_TARGET,
                    "crop",
                    crop_id,
                    "",
                    actor,
                )?)
            };
            (alias, crate::blocks::crop_code_of(conn, crop_id)?)
        }
        // A sample off a plot with no crop declared: the block allows it, so the
        // entry is sent with neither member rather than being refused.
        None => (None, None),
    };
    Ok(DgcAnalitica {
        codigo_dgc_ajena,
        codigo_cultivo,
    })
}
