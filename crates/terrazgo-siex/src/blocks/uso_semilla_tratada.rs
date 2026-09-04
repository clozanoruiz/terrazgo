// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `UsoSemillaTratada` — model section 3.2, treated seed.
//!
//! The register is a SOWING that used treated seed, not an application: the
//! product may have been applied by whoever sold the sack, which is why the
//! register captures it as free text and why `seed_treatment` demands no
//! registry row.
//!
//! Two consequences for this block. `Producto` is **the crop**, not the
//! phytosanitary product — Anexo V's field 1 reads "Cultivo — código del
//! cultivo del catálogo SIEX", so it takes `seed_treatment.crop_code`. And
//! `ProductosFito` is emitted by NOTHING: it is optional in the schema, the
//! register stores no amount of product (model 3.2 prints no such column), and
//! sending a `Cantidad` we do not have would mean inventing one. Anexo V grades
//! those fields Obligatorio, but they are inside an optional child — the
//! standing distinction between required, obligatorio and binding
//! (docs/siex-export.md).

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::models::SeedTreatment;
use module_cue::repository::list_seed_treatments_for_export;
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<UsoSemillaTratada>> {
    let mut out = Vec::new();
    for detail in list_seed_treatments_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail.record, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    record: &SeedTreatment,
    actor: Option<&str>,
) -> Result<Option<UsoSemillaTratada>> {
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "seed_treatment", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "seed_treatment", &record.id, "", actor)?
    };

    // Each of the four required values is nullable in the register and demanded
    // by the precheck, so an active record always has them. A deletion entry
    // exists to identify the activity it withdraws, so it falls back rather
    // than refusing — the same rule TratamFito's efficacy follows.
    let tratamiento = match record.treatment_kind_code.as_deref() {
        Some(code) => siex::seed_treatment_kind_to_siex(code)
            .ok_or(SiexError::Invalid("export_code_unmappable"))?,
        None if deleted => 0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let producto = match record.crop_code.as_deref() {
        Some(code) => code
            .trim()
            .parse::<i64>()
            .map_err(|_| SiexError::Invalid("export_code_unmappable"))?,
        None if deleted => 0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let cantidad = match record.seed_quantity_kg {
        Some(value) => value,
        None if deleted => 0.0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let eficacia = match &record.efficacy_code {
        Some(code) => {
            siex::efficacy_to_siex(code).ok_or(SiexError::Invalid("export_code_unmappable"))?
        }
        None => 0,
    };

    Ok(Some(UsoSemillaTratada {
        id_ajena_semilla_trat: alias,
        borrar: deleted.then_some(true),
        tratamiento,
        fecha: siex::date_to_siex(&record.sown_on)
            .ok_or(SiexError::Invalid("export_code_unmappable"))?,
        producto,
        numero_lote: record.seed_lot.clone(),
        cantidad,
        eficacia,
        observaciones: record.notes.clone(),
    }))
}
