// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `PlanAbonado` — model section 7.1, what the book records about the plan.
//!
//! The register stores RD 1051/2022 **art. 5.a's** four figures and nothing
//! more, because art. 6 defines a separate DOCUMENT that is drawn up and kept.
//! The twin's required set is that same list plus the tool flag, which is the
//! corroboration the design already leant on: the exchange format asks for the
//! summary, not the plan.
//!
//! Its DGCs come from `fertilisation_plan_crop`, which holds crop ids directly
//! rather than plots — a plan covers a production unit, and the repository keeps
//! a crop in at most one live plan, so no crop is ever named twice here.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::siex as cue_siex;
use module_fertilisation::models::FertilisationPlanDetail;
use module_fertilisation::repository::list_fertilisation_plans_for_export;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<PlanAbonado>> {
    let mut out = Vec::new();
    for detail in list_fertilisation_plans_for_export(conn, season_id, farm_id)? {
        if let Some(entry) = entry(conn, &detail, actor)? {
            out.push(entry);
        }
    }
    Ok(out)
}

fn entry(
    conn: &mut Connection,
    detail: &FertilisationPlanDetail,
    actor: Option<&str>,
) -> Result<Option<PlanAbonado>> {
    let plan = &detail.plan;
    let deleted = plan.deleted_at.is_some();
    let alias = if deleted {
        match find_export_alias(conn, SIEX_TARGET, "fertilisation_plan", &plan.id, "")? {
            Some(alias) => alias,
            None => return Ok(None), // never exported — nothing to withdraw
        }
    } else {
        ensure_export_alias(conn, SIEX_TARGET, "fertilisation_plan", &plan.id, "", actor)?
    };

    let unmappable = || SiexError::Invalid("export_code_unmappable");
    // Required by the schema and nullable in the register, so the precheck
    // demands it; a deletion entry falls back rather than refusing, since it
    // exists only to identify the activity it withdraws.
    let cultivo_precedente = match plan.preceding_crop_code.as_deref() {
        Some(code) => code.trim().parse::<i64>().map_err(|_| unmappable())?,
        None if deleted => 0,
        None => return Err(unmappable()),
    };

    let dgcs = detail
        .crop_ids
        .iter()
        .map(|crop_id| dgc(conn, crop_id, deleted, actor))
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(PlanAbonado {
        id_ajena_plan: alias,
        borrar: deleted.then_some(true),
        necesidad_uf_n: plan.needs_n_kg_ha,
        necesidad_uf_p2o5: plan.needs_p2o5_kg_ha,
        necesidad_uf_k2o: plan.needs_k2o_kg_ha,
        objetivo_produccion: plan.expected_yield_kg_ha,
        cultivo_precedente,
        herramienta: plan.tool_generated,
        fecha_generacion: cue_siex::date_to_siex(&plan.drawn_up_on).ok_or_else(unmappable)?,
        dgcs,
    }))
}

/// One planned crop. Unlike the other blocks' DGCs the crop is never absent —
/// `fertilisation_plan_crop.crop_id` is `NOT NULL`, because a plan with no crop
/// would recommend nitrogen for nothing.
fn dgc(
    conn: &mut Connection,
    crop_id: &str,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcPlan> {
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
    Ok(DgcPlan {
        codigo_dgc_ajena,
        codigo_cultivo: crate::blocks::crop_code_of(conn, crop_id)?,
    })
}
