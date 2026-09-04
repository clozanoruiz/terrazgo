// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `Pastoreo` — model 9.1, RD 1048/2022 art. 30.2 ter.
//!
//! Two members are DERIVED and neither should ever become a column.
//! `AnimalesPropios` and `AnimalesTerceros` are booleans — Anexo V reads
//! *"Pastoreo con animales de la explotación (S/N)"* — and each falls out of
//! comparing an animal line's REGA with the holding's own. Two columns of those
//! names once held head counts, modelled on a 15-month-stale descriptor, and
//! were dropped on 2026-08-20 for being derived state that can drift.
//!
//! The descriptor forbids both being false. Nothing here enforces that: the
//! register already refuses a grazing with no animal line, and the precheck
//! demands the holding's own REGA once a season holds one, which together make
//! the pair unfalsifiable at this end.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex as cue_siex;
use module_ecoscheme::models::GrazingRecordDetail;
use module_ecoscheme::repository::list_grazing_records_for_export;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<Pastoreo>> {
    // Read once for the whole block: every entry compares its animal lines
    // against this one value.
    let own_rega = terrazgo_core::repository::get_farm(conn, farm_id)?
        .es
        .and_then(|es| es.rega_code)
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut out = Vec::new();
    for detail in list_grazing_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, &own_rega, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &GrazingRecordDetail,
    own_rega: &str,
    actor: Option<&str>,
) -> Result<Option<Pastoreo>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "grazing_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "grazing_record", &record.id, "", actor)?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let fecha_inicio = cue_siex::date_to_siex(&record.started_on).ok_or_else(unmappable)?;
    // Required by the format and nullable in the register, because a grazing
    // still under way has not ended and its annotation is not yet late. The
    // precheck refuses an open ACTIVE record by name; a deletion entry falls
    // back to the start date, since it exists only to identify the activity it
    // withdraws.
    let fecha_fin = match record.ended_on.as_deref() {
        Some(end) => cue_siex::date_to_siex(end).ok_or_else(unmappable)?,
        None if deleted => fecha_inicio.clone(),
        None => return Err(unmappable()),
    };

    let animales = detail
        .animals
        .iter()
        .map(|line| {
            Ok(Animal {
                rega: line.rega_code.clone(),
                numero: line.animal_count,
                especie: line
                    .species_code
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| unmappable())?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // A line whose REGA is the holding's own is its own animals; any other is a
    // third party's. With no holding REGA on record every line reads as a third
    // party's, which is a claim rather than an absence — hence the precheck.
    let animales_propios = detail
        .animals
        .iter()
        .any(|line| !own_rega.is_empty() && line.rega_code.trim() == own_rega);
    let animales_terceros = detail
        .animals
        .iter()
        .any(|line| own_rega.is_empty() || line.rega_code.trim() != own_rega);

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

    Ok(Some(Pastoreo {
        id_ajena_pastoreo: alias,
        borrar: deleted.then_some(true),
        fecha_inicio,
        fecha_fin,
        animales_propios,
        animales_terceros,
        animales,
        dgcs,
    }))
}
