// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `LaboresCulturales` — `cultural_operation`, which is one register serving
//! model 9.2, the book's own "9.6" for anexo IV, model 9.3's two unprinted
//! dates and model 9.4's mechanical maintenance. All four are labores
//! culturales, so all four travel in this block.
//!
//! The two `Depositado*` booleans are the only computation here, and the twin is
//! what settled where the evidence chain lives: it hangs them off
//! `LaboresCulturales` rather than off `DatosCubierta`, so an inert cover exists
//! BECAUSE an operation said its residue stayed on the ground.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex as cue_siex;
use module_ecoscheme::models::CulturalOperationDetail;
use module_ecoscheme::repository::list_cultural_operations_for_export;
use module_ecoscheme::siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<LaborCultural>> {
    let mut out = Vec::new();
    for detail in list_cultural_operations_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &CulturalOperationDetail,
    actor: Option<&str>,
) -> Result<Option<LaborCultural>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "cultural_operation", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(
            conn,
            SIEX_TARGET,
            "cultural_operation",
            &record.id,
            "",
            actor,
        )?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let fecha_inicio = cue_siex::date_to_siex(&record.performed_on).ok_or_else(unmappable)?;
    // `None` means one day's work, never "unknown", so the start is the honest
    // end — the same reading `Riego` and `SiembraPlantacion` make.
    let fecha_fin = match record.performed_end_date.as_deref() {
        Some(end) => cue_siex::date_to_siex(end).ok_or_else(unmappable)?,
        None => fecha_inicio.clone(),
    };
    // `operation_kind_code` has a foreign key to the owned lookup and every one
    // of its rows maps, so this cannot fail short of a corrupted database.
    let tipo_labor = siex::cultural_operation_kind_to_siex(&record.operation_kind_code)
        .ok_or_else(unmappable)?;

    let left_on_plot = record
        .residue_destination_code
        .as_deref()
        .is_some_and(|code| siex::RESIDUE_LEFT_ON_PLOT.contains(&code.trim()));
    let cover_type = crate::blocks::cover_type_of(conn, record.soil_cover_id.as_deref())?;
    let dgcs = detail
        .plots
        .iter()
        .map(|plot| {
            crate::blocks::dgc_actividad(
                conn,
                &plot.plot_id,
                &record.season_id,
                cover_type,
                deleted,
                actor,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(LaborCultural {
        id_ajena_labor: alias,
        borrar: deleted.then_some(true),
        fecha_inicio,
        fecha_fin,
        tipo_labor,
        // Each boolean answers for its OWN kind of residue, so the operation
        // decides which of the two a "left on the plot" destination fills. A
        // `pruning_removal` is never either: TIPO_LABOR 11 is literally
        // "Eliminación de restos de poda", so its residue left the plot by
        // definition.
        depositado_suelo_desb: left_on_plot
            && siex::RESIDUE_KINDS_BRUSH.contains(&record.operation_kind_code.as_str()),
        depositado_suelo_poda: left_on_plot
            && siex::RESIDUE_KINDS_PRUNING.contains(&record.operation_kind_code.as_str()),
        dgcs,
    }))
}
