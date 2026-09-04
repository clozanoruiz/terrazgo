// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `ComercializacionVD` — model section 5, what left the holding.
//!
//! The register is core's `harvest_record`, and the twin is the **sale**, not
//! the harvesting operation: `Cosecha` is the field work, and nothing fills it
//! (docs/siex-export.md → "The two blocks nothing will fill").
//!
//! The block carries no DGC array and no buyer of any kind, so most of what the
//! printed model asks for — the origin plots, the client's name, tax id,
//! address and RGSEAA number — has nowhere to go here. That asymmetry is the
//! usual direction reversed, and it is why the register is shaped by the model:
//! the model is the compliance artifact.
//!
//! Anexo V grades all five of its fields Voluntario, which does not soften what
//! the schema demands of an entry that IS sent — the produce code, the amount
//! and its unit are all required and all nullable in the register, so the
//! precheck asks for them.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::models::HarvestRecord;
use terrazgo_core::repository::{
    ensure_export_alias, find_export_alias, list_harvest_records_for_export,
};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<ComercializacionVd>> {
    let mut out = Vec::new();
    for detail in list_harvest_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail.record, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    record: &HarvestRecord,
    actor: Option<&str>,
) -> Result<Option<ComercializacionVd>> {
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "harvest_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "harvest_record", &record.id, "", actor)?
    };

    // The three required values are nullable in the register and demanded by the
    // precheck, so an active record always has them. A deletion entry exists to
    // identify the activity it withdraws, so it falls back rather than refusing
    // — the `UsoSemillaTratada` rule.
    let producto_vegetal = match record.plant_product_code.as_deref() {
        Some(code) => code
            .trim()
            .parse::<i64>()
            .map_err(|_| SiexError::Invalid("export_code_unmappable"))?,
        None if deleted => 0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let cantidad = match record.quantity_value {
        Some(value) => value,
        None if deleted => 0.0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    // Unlike TratamientosPostCosecha, this block carries its own unit member, so
    // the stored kg or t travels as it stands and nothing is converted.
    let unidad = match record.quantity_unit_code.as_deref() {
        Some(code) => {
            siex::quantity_unit_to_siex(code).ok_or(SiexError::Invalid("export_code_unmappable"))?
        }
        None if deleted => 0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };

    // One date at both ends: the model prints a single "Fecha" column and
    // section 5 is not Anexo III Parte I content, so the register keeps one.
    let fecha = siex::date_to_siex(&record.harvested_on)
        .ok_or(SiexError::Invalid("export_code_unmappable"))?;

    Ok(Some(ComercializacionVd {
        id_ajena_venta: alias,
        borrar: deleted.then_some(true),
        fecha_inicio: fecha.clone(),
        fecha_fin: fecha,
        producto_vegetal,
        cantidad,
        unidad,
    }))
}
