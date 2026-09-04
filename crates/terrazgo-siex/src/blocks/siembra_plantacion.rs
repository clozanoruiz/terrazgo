// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `SiembraPlantacion` — core's sowing register, how a crop began.
//!
//! **This block is built from two registers, and that is the format's shape,
//! not ours.** `sowing_record` answers the dates, the amount and the plots;
//! model 3.2's `seed_treatment` answers the three seed-provenance members,
//! reached through the link the farmer states (`seed_treatment.sowing_record_id`
//! — the direction the dependency rule forces and the one the descriptor's own
//! `UsoSemillaTratada.IdAjenaSiembraPlant` points).
//!
//! The two registers stay separate tables, as the printed model keeps them:
//! merging them would either weaken 3.2's `surface_sown_ha NOT NULL` or invent a
//! surface model 9.3 never asks for. So the merge happens here, at export, which
//! is where the format's shape belongs.
//!
//! Nothing is inferred. A sowing with no link sends `MaterialTratado: false`
//! because the farmer did not say otherwise, `MaterialAdquirido` is read from
//! FEGA's own TIPO_TRATAMIENTO coding (4 and 5 are "adquisición de semilla
//! tratada"), and `FechaAdquisicion` is a stored column or a null.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::models::SeedTreatment;
use module_cue::repository::list_seed_treatments_for_sowing;
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::models::{SowingPlot, SowingRecordDetail};
use terrazgo_core::repository::{
    ensure_export_alias, find_export_alias, list_sowing_records_for_export,
};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<SiembraPlantacion>> {
    let mut out = Vec::new();
    for detail in list_sowing_records_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &SowingRecordDetail,
    actor: Option<&str>,
) -> Result<Option<SiembraPlantacion>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "sowing_record", &record.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "sowing_record", &record.id, "", actor)?
    };

    let siembra_plantacion =
        kind_to_siex(&record.kind_code).ok_or(SiexError::Invalid("export_code_unmappable"))?;
    let fecha_inicio =
        siex::date_to_siex(&record.sown_on).ok_or(SiexError::Invalid("export_code_unmappable"))?;
    // `None` here means one day's work, never "unknown", so the start date is
    // the honest end date rather than a fallback.
    let fecha_fin = match record.sowing_end_date.as_deref() {
        Some(end) => siex::date_to_siex(end).ok_or(SiexError::Invalid("export_code_unmappable"))?,
        None => fecha_inicio.clone(),
    };
    let fecha_inundacion = match record.flooded_on.as_deref() {
        Some(date) => {
            Some(siex::date_to_siex(date).ok_or(SiexError::Invalid("export_code_unmappable"))?)
        }
        None => None,
    };
    // Required by the schema and nullable in the register, so the precheck
    // demands it; a deletion entry falls back rather than refusing.
    let cantidad = match record.seed_quantity_kg {
        Some(value) => value,
        None if deleted => 0.0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };

    let seed = seed_provenance(conn, &record.id)?;
    let dgcs = detail
        .plots
        .iter()
        .map(|plot| dgc(conn, plot, deleted, actor))
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(SiembraPlantacion {
        id_ajena_siembra_plant: alias,
        borrar: deleted.then_some(true),
        siembra_plantacion,
        fecha_inicio,
        fecha_fin,
        fecha_inundacion,
        material_tratado: seed.treated,
        material_adquirido: seed.acquired,
        fecha_adquisicion: seed.acquired_on,
        num_lote: seed.lot,
        dgcs,
        cantidad,
    }))
}

/// SIEX codes the pair as a single digit: "1 Siembra 0 Plantación".
fn kind_to_siex(code: &str) -> Option<i64> {
    match code {
        "sowing" => Some(1),
        "planting" => Some(0),
        _ => None,
    }
}

/// What model 3.2's records say about the material this sowing used.
struct SeedProvenance {
    treated: bool,
    acquired: bool,
    acquired_on: Option<String>,
    lot: Option<String>,
}

/// Collapse every treated-seed record naming this sowing into the four members
/// the block has room for. One sowing can use several lots — the register is one
/// row per product — and each of these rules exists because of that.
fn seed_provenance(conn: &Connection, sowing_record_id: &str) -> Result<SeedProvenance> {
    let linked: Vec<SeedTreatment> = list_seed_treatments_for_sowing(conn, sowing_record_id)?;
    if linked.is_empty() {
        return Ok(SeedProvenance {
            treated: false,
            acquired: false,
            acquired_on: None,
            lot: None,
        });
    }

    // Acquired if ANY of the lots was bought: the question is whether material
    // was purchased, and one purchased sack makes the answer yes.
    let acquired_kinds = linked
        .iter()
        .filter(|record| is_acquisition(record.treatment_kind_code.as_deref()))
        .collect::<Vec<_>>();

    // The EARLIEST purchase, and it is well defined because the precheck demands
    // a date on every acquired record. Dates are ISO 8601, so they compare
    // lexicographically.
    let acquired_on = acquired_kinds
        .iter()
        .filter_map(|record| record.acquired_on.as_deref())
        .min()
        .and_then(siex::date_to_siex);

    // A lot number identifies ONE sack. With several linked records stating
    // different lots, naming one would be a false statement about the others —
    // and the member is optional, so silence is available and honest. Each lot
    // still travels on its own `UsoSemillaTratada` entry.
    let mut lots = linked
        .iter()
        .filter_map(|record| record.seed_lot.as_deref())
        .collect::<Vec<_>>();
    lots.sort_unstable();
    lots.dedup();
    let lot = match lots.as_slice() {
        [only] => Some((*only).to_string()),
        _ => None,
    };

    Ok(SeedProvenance {
        treated: true,
        acquired: !acquired_kinds.is_empty(),
        acquired_on,
        lot,
    })
}

/// TIPO_TRATAMIENTO 4 and 5 are literally "adquisición de semilla tratada …",
/// against 2 and 3 for seed treated on the holding or at a conditioning centre.
/// So `MaterialAdquirido` needs no column of its own — this catalogue IS the
/// distinction the member asks about.
pub(crate) fn is_acquisition(treatment_kind_code: Option<&str>) -> bool {
    matches!(
        treatment_kind_code,
        Some("purchased_es") | Some("purchased_abroad")
    )
}

/// One sown plot+crop. Both members are optional in the schema, but a DGC with
/// neither states nothing at all, so the precheck demands the crop and this only
/// falls back for a deletion entry whose plot never had one.
fn dgc(
    conn: &mut Connection,
    plot: &SowingPlot,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcSiembra> {
    let Some(crop_id) = &plot.crop_id else {
        return Ok(DgcSiembra {
            codigo_dgc_ajena: None,
            codigo_cultivo: None,
        });
    };
    let codigo_dgc_ajena = if deleted {
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
    Ok(DgcSiembra {
        codigo_dgc_ajena,
        codigo_cultivo: crate::blocks::crop_code_of(conn, crop_id)?,
    })
}
