// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `Riego` — model section 8, the irrigation register.
//!
//! The block this module fills was SIEX-shaped from the day the register
//! shipped: `irrigation_water_origin` is a junction because `OrigenAgua` is an
//! array, `energy_type_code` and `meter_number` exist only because the twin
//! carries them, and the date is an interval because both the twin and RD
//! 1051/2022 art. 5.f allow one. So this seam is wiring, not design.
//!
//! `BuenasPracticasRiego` comes from `irrigation_practice` (2026-10-06):
//! Voluntario in Anexo V 3.11, obligatorio condicionado in the 2027 model, and
//! sent whenever the farmer claimed one. What it does NOT send is the water's
//! nitric N and soluble P₂O₅, which the register does hold but which belong to
//! `Fertirrigacion` inside the fertilisation block rather than here.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_fertilisation::models::IrrigationRecordDetail;
use module_fertilisation::repository::list_irrigation_records_for_export;
use module_fertilisation::siex;
use module_phytosanitary::siex as cue_siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<Riego>> {
    let mut out = Vec::new();
    for detail in list_irrigation_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &IrrigationRecordDetail,
    actor: Option<&str>,
) -> Result<Option<Riego>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "irrigation_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(
            conn,
            SIEX_TARGET,
            "irrigation_record",
            &record.id,
            "",
            actor,
        )?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let fecha_inicio = cue_siex::date_to_siex(&record.irrigated_on).ok_or_else(unmappable)?;
    // `None` means one day's watering, so the start is the honest end.
    let fecha_fin = match record.irrigation_end_date.as_deref() {
        Some(end) => cue_siex::date_to_siex(end).ok_or_else(unmappable)?,
        None => fecha_inicio.clone(),
    };

    // Both NOT NULL in the register and both required here, so neither needs a
    // deletion fallback — unlike the nullable fields of the other blocks.
    let sistema_riego =
        siex::irrigation_method_to_siex(&record.irrigation_method_code).ok_or_else(unmappable)?;
    let unidad_medida =
        siex::volume_unit_to_siex(&record.volume_unit_code).ok_or_else(unmappable)?;

    let origen_agua = detail
        .water_origins
        .iter()
        .map(|origin| {
            siex::water_origin_to_siex(origin)
                .map(|id_origen_agua| OrigenAgua { id_origen_agua })
                .ok_or_else(unmappable)
        })
        .collect::<Result<Vec<_>>>()?;

    // TIPENERGIA, stored verbatim with no foreign key. An unparseable value is
    // dropped rather than refused: the member is optional, so it must not fail
    // an export of everything else — the `EstadoFenologico` rule.
    let tipo_energia = record
        .energy_type_code
        .as_deref()
        .and_then(|code| code.trim().parse::<i64>().ok());

    // Codes stored verbatim; one that is not an integer cannot be sent, the
    // fertilisation block's rule for the same catalogue.
    let buenas_practicas_riego = detail
        .practices
        .iter()
        .map(|code| {
            code.trim()
                .parse::<i64>()
                .map(|tipo_bpr| BuenaPracticaRiego { tipo_bpr })
                .map_err(|_| unmappable())
        })
        .collect::<Result<Vec<_>>>()?;

    let dgcs = detail
        .plots
        .iter()
        .map(|plot| {
            crate::blocks::dgc_superficie(
                conn,
                plot.crop_id.as_deref(),
                plot.irrigated_area_ha,
                deleted,
                actor,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(Riego {
        id_ajena_riego: alias,
        borrar: deleted.then_some(true),
        fecha_inicio,
        fecha_fin,
        sistema_riego,
        cantidad: record.volume_value,
        unidad_medida,
        origen_agua,
        tipo_energia,
        num_contador: record.meter_number.clone(),
        buenas_practicas_riego,
        dgcs,
    }))
}
