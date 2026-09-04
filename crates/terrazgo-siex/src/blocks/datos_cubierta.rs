// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `DatosCubierta` — models 9.4 and 9.5, RD 1048/2022 arts. 42 and 43.
//!
//! One block for both pages, because both articles ask for the same three
//! things; `soil_cover.practice_code` is what separates a live cover from an
//! inert one, and the format needs no such distinction — `TipoCobertura`
//! already carries it.
//!
//! The cover's maintenance is not here. Art. 42.1.c's siega, desbroce and
//! pastoreo are `cultural_operation` and `grazing_record` rows in their own
//! right, and each travels as its own `LaboresCulturales` or `Pastoreo` entry
//! naming this cover through `Cubiertas`. The twin agrees, which is why
//! `DepositadoSueloDesb`/`Poda` sit on the operation and not here.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex as cue_siex;
use module_ecoscheme::models::SoilCoverDetail;
use module_ecoscheme::repository::list_soil_covers_for_export;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<DatosCubierta>> {
    let mut out = Vec::new();
    for detail in list_soil_covers_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &SoilCoverDetail,
    actor: Option<&str>,
) -> Result<Option<DatosCubierta>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "soil_cover", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "soil_cover", &record.id, "", actor)?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let fec_establecimiento_cub =
        cue_siex::date_to_siex(&record.established_on).ok_or_else(unmappable)?;

    let dgcs = detail
        .plots
        .iter()
        .map(|plot| {
            crate::blocks::dgc_cubierta(conn, &plot.plot_id, &record.season_id, deleted, actor)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(DatosCubierta {
        id_ajena_cubierta: alias,
        borrar: deleted.then_some(true),
        fec_establecimiento_cub,
        // The widths are nullable as a group here because art. 42.1.e falls due
        // later than 42.1.a. The precheck demands them of an ACTIVE record —
        // Anexo V grades both Obligatorio — so a null reaching this point is a
        // deletion entry, which asserts nothing and states nothing.
        anchura_cubierta: record.width_m,
        anchura_libre_proy: record.free_canopy_width_m,
        tipo_cobertura: record.cover_type_code.trim().parse::<i64>().ok(),
        dgcs,
    }))
}
