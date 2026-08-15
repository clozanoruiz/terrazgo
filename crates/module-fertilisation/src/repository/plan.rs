// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 7.1 — what the book records about the plan de abonado.
//!
//! RD 1051/2022 art. 4.2 requires a plan per production unit from 1 September
//! 2026 (1 January 2026 for irrigated units sown between 1 March and 30 June),
//! art. 6 says what the plan document must contain, and **art. 5.a says what
//! goes in the book**: expected yield, preceding crop, the N / P₂O₅ / K₂O
//! needs, and the date the plan was drawn up. This table is art. 5.a.
//!
//! Fully correctable, and not as a concession: art. 6 explicitly allows a plan
//! to be adjusted during the campaign to follow the crop and the weather.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::error::{FertilisationError, Result};
use crate::models::{
    FertilisationPlan, FertilisationPlanDetail, NewFertilisationPlan, UpdateFertilisationPlan,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use uuid::Uuid;

pub fn insert_fertilisation_plan(
    conn: &mut Connection,
    new: NewFertilisationPlan,
    actor: Option<&str>,
) -> Result<FertilisationPlanDetail> {
    validate_needs(
        &new.needs_n_kg_ha,
        &new.needs_p2o5_kg_ha,
        &new.needs_k2o_kg_ha,
    )?;
    validate_yield(new.expected_yield_kg_ha)?;

    let tx = conn.transaction()?;
    let crop_ids = validated_crops(&tx, &new.farm_id, &new.season_id, &new.crop_ids, None)?;

    let now = now_utc_iso();
    let plan = FertilisationPlan {
        id: Uuid::now_v7().to_string(),
        season_id: new.season_id,
        farm_id: new.farm_id,
        needs_n_kg_ha: new.needs_n_kg_ha,
        needs_p2o5_kg_ha: new.needs_p2o5_kg_ha,
        needs_k2o_kg_ha: new.needs_k2o_kg_ha,
        expected_yield_kg_ha: new.expected_yield_kg_ha,
        preceding_crop_code: blank_to_none(new.preceding_crop_code),
        drawn_up_on: new.drawn_up_on,
        tool_generated: new.tool_generated,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO fertilisation_plan (
            id, season_id, farm_id, needs_n_kg_ha, needs_p2o5_kg_ha, needs_k2o_kg_ha,
            expected_yield_kg_ha, preceding_crop_code, drawn_up_on, tool_generated,
            notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            plan.id,
            plan.season_id,
            plan.farm_id,
            plan.needs_n_kg_ha,
            plan.needs_p2o5_kg_ha,
            plan.needs_k2o_kg_ha,
            plan.expected_yield_kg_ha,
            plan.preceding_crop_code,
            plan.drawn_up_on,
            plan.tool_generated,
            plan.notes,
            plan.created_at,
            plan.updated_at
        ],
    )?;
    for crop_id in &crop_ids {
        insert_crop_row(&tx, &plan.id, &plan.season_id, crop_id, actor)?;
    }
    log_insert(
        &tx,
        "fertilisation_plan",
        &plan.id,
        Some(&plan.season_id),
        actor,
        &plan,
    )?;
    tx.commit()?;
    Ok(FertilisationPlanDetail { plan, crop_ids })
}

/// Full-row correction, the covered crops reconciled from the submitted state.
pub fn update_fertilisation_plan(
    conn: &mut Connection,
    id: &str,
    update: UpdateFertilisationPlan,
    actor: Option<&str>,
) -> Result<FertilisationPlanDetail> {
    validate_needs(
        &update.needs_n_kg_ha,
        &update.needs_p2o5_kg_ha,
        &update.needs_k2o_kg_ha,
    )?;
    validate_yield(update.expected_yield_kg_ha)?;

    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM fertilisation_plan WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_plan,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    let crop_ids = validated_crops(
        &tx,
        &before.farm_id,
        &before.season_id,
        &update.crop_ids,
        Some(id),
    )?;

    let mut after = before.clone();
    after.needs_n_kg_ha = update.needs_n_kg_ha;
    after.needs_p2o5_kg_ha = update.needs_p2o5_kg_ha;
    after.needs_k2o_kg_ha = update.needs_k2o_kg_ha;
    after.expected_yield_kg_ha = update.expected_yield_kg_ha;
    after.preceding_crop_code = blank_to_none(update.preceding_crop_code);
    after.drawn_up_on = update.drawn_up_on;
    after.tool_generated = update.tool_generated;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE fertilisation_plan SET
            needs_n_kg_ha = ?2, needs_p2o5_kg_ha = ?3, needs_k2o_kg_ha = ?4,
            expected_yield_kg_ha = ?5, preceding_crop_code = ?6, drawn_up_on = ?7,
            tool_generated = ?8, notes = ?9, updated_at = ?10
         WHERE id = ?1",
        params![
            id,
            after.needs_n_kg_ha,
            after.needs_p2o5_kg_ha,
            after.needs_k2o_kg_ha,
            after.expected_yield_kg_ha,
            after.preceding_crop_code,
            after.drawn_up_on,
            after.tool_generated,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(
        &tx,
        "fertilisation_plan",
        id,
        Some(&after.season_id),
        actor,
        &before,
        &after,
    )?;
    reconcile_crops(&tx, &after, &crop_ids, actor)?;
    tx.commit()?;
    Ok(FertilisationPlanDetail {
        plan: after,
        crop_ids,
    })
}

pub fn soft_delete_fertilisation_plan(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM fertilisation_plan WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_plan,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE fertilisation_plan SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "fertilisation_plan",
        id,
        Some(&before.season_id),
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_fertilisation_plan(conn: &Connection, id: &str) -> Result<FertilisationPlanDetail> {
    let plan = conn
        .query_row(
            "SELECT * FROM fertilisation_plan WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_plan,
        )
        .map_err(no_rows_to_not_found)?;
    let crop_ids = crops_of(conn, &plan.id)?;
    Ok(FertilisationPlanDetail { plan, crop_ids })
}

/// The season's plans, oldest first — the order the book reads in.
pub fn list_fertilisation_plans(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<FertilisationPlanDetail>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM fertilisation_plan
         WHERE season_id = ?1 AND farm_id = ?2 AND deleted_at IS NULL
         ORDER BY drawn_up_on, id",
    )?;
    let plans = stmt
        .query_map(params![season_id, farm_id], map_plan)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    plans
        .into_iter()
        .map(|plan| {
            let crop_ids = crops_of(conn, &plan.id)?;
            Ok(FertilisationPlanDetail { plan, crop_ids })
        })
        .collect()
}

/// Whether any plan hangs off this season — this register's arm of the guard
/// the shell chains before deleting one. Soft-deleted rows count.
pub(super) fn season_has_plans(conn: &Connection, season_id: &str) -> Result<bool> {
    let held: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM fertilisation_plan WHERE season_id = ?1)",
        [season_id],
        |r| r.get(0),
    )?;
    Ok(held)
}

// --- the covered production unit -------------------------------------------

fn reconcile_crops(
    tx: &Transaction,
    plan: &FertilisationPlan,
    desired: &[String],
    actor: Option<&str>,
) -> Result<()> {
    let current = crop_rows_tx(tx, &plan.id)?;
    for (row_id, crop_id) in &current {
        if !desired.iter().any(|d| d == crop_id) {
            tx.execute(
                "DELETE FROM fertilisation_plan_crop WHERE id = ?1",
                [row_id],
            )?;
            log_delete(
                tx,
                "fertilisation_plan_crop",
                row_id,
                Some(&plan.season_id),
                actor,
                &crop_image(row_id, &plan.id, crop_id),
                None::<&serde_json::Value>,
            )?;
        }
    }
    for crop_id in desired {
        if !current.iter().any(|(_, existing)| existing == crop_id) {
            insert_crop_row(tx, &plan.id, &plan.season_id, crop_id, actor)?;
        }
    }
    Ok(())
}

fn insert_crop_row(
    tx: &Transaction,
    plan_id: &str,
    season_id: &str,
    crop_id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let id = Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO fertilisation_plan_crop (id, fertilisation_plan_id, crop_id)
         VALUES (?1, ?2, ?3)",
        params![id, plan_id, crop_id],
    )?;
    log_insert(
        tx,
        "fertilisation_plan_crop",
        &id,
        Some(season_id),
        actor,
        &crop_image(&id, plan_id, crop_id),
    )?;
    Ok(())
}

fn crop_image(id: &str, plan_id: &str, crop_id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "fertilisation_plan_id": plan_id,
        "crop_id": crop_id,
    })
}

// --- validation ------------------------------------------------------------

/// The three needs are required and cannot be negative — a plan that
/// recommends a negative quantity of nitrogen is a typo, not a recommendation.
/// Zero IS allowed and meaningful: "this unit needs no potassium" is exactly
/// the kind of thing a plan says.
fn validate_needs(n: &f64, p2o5: &f64, k2o: &f64) -> Result<()> {
    for value in [n, p2o5, k2o] {
        if !value.is_finite() || *value < 0.0 {
            return Err(FertilisationError::Invalid("invalid_nutrient_need"));
        }
    }
    Ok(())
}

fn validate_yield(expected: f64) -> Result<()> {
    if !expected.is_finite() || expected <= 0.0 {
        return Err(FertilisationError::Invalid("invalid_expected_yield"));
    }
    Ok(())
}

/// Every crop of the production unit must exist, belong to this farm and sit in
/// this season — a plan for a crop of another campaign would print under the
/// wrong book.
///
/// **A crop belongs to at most one live plan.** Two plans recommending
/// different nitrogen for the same crop would make section 7.1 print two
/// different figures on one row, and neither would be wrong on its own.
fn validated_crops(
    tx: &Transaction,
    farm_id: &str,
    season_id: &str,
    crop_ids: &[String],
    updating: Option<&str>,
) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut kept: Vec<String> = Vec::new();
    for crop_id in crop_ids {
        if !seen.insert(crop_id.clone()) {
            continue;
        }
        let owner: (String, String) = tx
            .query_row(
                "SELECT p.farm_id, c.season_id FROM crop c
                 JOIN plot p ON p.id = c.plot_id
                 WHERE c.id = ?1 AND c.deleted_at IS NULL",
                [crop_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(no_rows_to_not_found)?;
        if owner.0 != farm_id || owner.1 != season_id {
            return Err(FertilisationError::Invalid("crop_not_in_this_book"));
        }
        let taken: Option<String> = tx
            .query_row(
                "SELECT pc.fertilisation_plan_id FROM fertilisation_plan_crop pc
                 JOIN fertilisation_plan p ON p.id = pc.fertilisation_plan_id
                 WHERE pc.crop_id = ?1 AND p.deleted_at IS NULL",
                [crop_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(plan_id) = taken
            && Some(plan_id.as_str()) != updating
        {
            return Err(FertilisationError::Invalid("crop_already_planned"));
        }
        kept.push(crop_id.clone());
    }
    if kept.is_empty() {
        return Err(FertilisationError::Invalid("no_crops"));
    }
    Ok(kept)
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

fn crops_of(conn: &Connection, plan_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT crop_id FROM fertilisation_plan_crop
         WHERE fertilisation_plan_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([plan_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(rows)
}

fn crop_rows_tx(tx: &Transaction, plan_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = tx.prepare(
        "SELECT id, crop_id FROM fertilisation_plan_crop
         WHERE fertilisation_plan_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([plan_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn map_plan(row: &Row<'_>) -> rusqlite::Result<FertilisationPlan> {
    Ok(FertilisationPlan {
        id: row.get("id")?,
        season_id: row.get("season_id")?,
        farm_id: row.get("farm_id")?,
        needs_n_kg_ha: row.get("needs_n_kg_ha")?,
        needs_p2o5_kg_ha: row.get("needs_p2o5_kg_ha")?,
        needs_k2o_kg_ha: row.get("needs_k2o_kg_ha")?,
        expected_yield_kg_ha: row.get("expected_yield_kg_ha")?,
        preceding_crop_code: row.get("preceding_crop_code")?,
        drawn_up_on: row.get("drawn_up_on")?,
        tool_generated: row.get("tool_generated")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
