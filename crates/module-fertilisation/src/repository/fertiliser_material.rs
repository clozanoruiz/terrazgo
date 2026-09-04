// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The fertiliser material registry — what section 6's records point at.
//!
//! The `product` pattern, and for the same reason: a farmer applies one
//! fertiliser many times in a campaign, and Anexo III C.h hangs eight agronomic
//! values off the material. Retyping those per application is where wrong data
//! comes from, so they live on a reusable row that each application references.
//!
//! Materials are soft-deleted (a record written years ago must still resolve
//! the material it names) and their composition lines are pure children: an
//! edit that drops one is a hard delete logged with a null after-image, the
//! same contract as `product_active_substance`.

use super::audit::{log_delete, log_insert, log_update, write_change};
use super::no_rows_to_not_found;
use crate::error::{FertilisationError, Result};
use crate::models::{
    FertiliserMaterial, FertiliserMaterialDetail, MaterialNutrient, NewFertiliserMaterial,
    UpdateFertiliserMaterial,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::json;
use std::collections::HashSet;
use terrazgo_core::date::now_utc_iso;
use uuid::Uuid;

/// The three nutrient catalogues a composition line can index, and the only
/// values `kind_code` may take. Checked here rather than left to the foreign
/// key so the caller gets a machine code instead of a constraint violation.
const NUTRIENT_KINDS: [&str; 3] = ["macro", "micro", "heavy_metal"];

pub fn insert_fertiliser_material(
    conn: &mut Connection,
    new: NewFertiliserMaterial,
    actor: Option<&str>,
) -> Result<FertiliserMaterialDetail> {
    let nutrients = validated_nutrients(&new.nutrients)?;
    let tx = conn.transaction()?;
    validate_material_code(&tx, &new.material_code)?;
    validate_manure_treatment(&tx, new.manure_treatment_code.as_deref())?;

    let now = now_utc_iso();
    let material = FertiliserMaterial {
        id: Uuid::now_v7().to_string(),
        name: non_empty(new.name, "empty_name")?,
        material_code: non_empty(new.material_code, "empty_material_code")?,
        material_detail_code: blank_to_none(new.material_detail_code),
        supplier_name: blank_to_none(new.supplier_name),
        supplier_rega: blank_to_none(new.supplier_rega),
        supplier_tax_id: blank_to_none(new.supplier_tax_id),
        supplier_nima: blank_to_none(new.supplier_nima),
        manure_treatment_code: blank_to_none(new.manure_treatment_code),
        density_kg_l: new.density_kg_l,
        notes: blank_to_none(new.notes),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    validate_supplier(&material)?;
    validate_density(material.density_kg_l)?;

    tx.execute(
        "INSERT INTO fertiliser_material (
            id, name, material_code, material_detail_code, supplier_name,
            supplier_rega, supplier_tax_id, supplier_nima, manure_treatment_code,
            density_kg_l, notes, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            material.id,
            material.name,
            material.material_code,
            material.material_detail_code,
            material.supplier_name,
            material.supplier_rega,
            material.supplier_tax_id,
            material.supplier_nima,
            material.manure_treatment_code,
            material.density_kg_l,
            material.notes,
            material.created_at,
            material.updated_at
        ],
    )?;
    log_insert(
        &tx,
        "fertiliser_material",
        &material.id,
        None,
        actor,
        &material,
    )?;

    let mut rows = Vec::new();
    for nutrient in nutrients {
        rows.push(insert_nutrient_row(&tx, &material.id, nutrient, actor)?);
    }
    tx.commit()?;
    Ok(FertiliserMaterialDetail {
        material,
        nutrients: rows,
    })
}

/// Full-row correction, composition reconciled from the submitted state.
///
/// Correcting a material never rewrites history: every application froze the
/// name and the printed richness at write time, which is exactly what the
/// snapshot columns are for.
pub fn update_fertiliser_material(
    conn: &mut Connection,
    id: &str,
    update: UpdateFertiliserMaterial,
    actor: Option<&str>,
) -> Result<FertiliserMaterialDetail> {
    let nutrients = validated_nutrients(&update.nutrients)?;
    let tx = conn.transaction()?;
    validate_material_code(&tx, &update.material_code)?;
    validate_manure_treatment(&tx, update.manure_treatment_code.as_deref())?;

    let before = tx
        .query_row(
            "SELECT * FROM fertiliser_material WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_material,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;

    let mut after = before.clone();
    after.name = non_empty(update.name, "empty_name")?;
    after.material_code = non_empty(update.material_code, "empty_material_code")?;
    after.material_detail_code = blank_to_none(update.material_detail_code);
    after.supplier_name = blank_to_none(update.supplier_name);
    after.supplier_rega = blank_to_none(update.supplier_rega);
    after.supplier_tax_id = blank_to_none(update.supplier_tax_id);
    after.supplier_nima = blank_to_none(update.supplier_nima);
    after.manure_treatment_code = blank_to_none(update.manure_treatment_code);
    after.density_kg_l = update.density_kg_l;
    after.notes = blank_to_none(update.notes);
    after.updated_at = now_utc_iso();
    validate_supplier(&after)?;
    validate_density(after.density_kg_l)?;

    tx.execute(
        "UPDATE fertiliser_material SET
            name = ?2, material_code = ?3, material_detail_code = ?4, supplier_name = ?5,
            supplier_rega = ?6, supplier_tax_id = ?7, supplier_nima = ?8,
            manure_treatment_code = ?9, density_kg_l = ?10, notes = ?11, updated_at = ?12
         WHERE id = ?1",
        params![
            id,
            after.name,
            after.material_code,
            after.material_detail_code,
            after.supplier_name,
            after.supplier_rega,
            after.supplier_tax_id,
            after.supplier_nima,
            after.manure_treatment_code,
            after.density_kg_l,
            after.notes,
            after.updated_at
        ],
    )?;
    log_update(&tx, "fertiliser_material", id, None, actor, &before, &after)?;

    let rows = reconcile_nutrients(&tx, id, nutrients, actor)?;
    tx.commit()?;
    Ok(FertiliserMaterialDetail {
        material: after,
        nutrients: rows,
    })
}

/// Soft delete: the row leaves the pickers and stays resolvable for every
/// record that ever named it. Always allowed, like a product's — the records
/// carry their own snapshots.
pub fn soft_delete_fertiliser_material(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM fertiliser_material WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_material,
        )
        .optional()?
        .ok_or(FertilisationError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE fertiliser_material SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    write_change(
        &tx,
        "fertiliser_material",
        id,
        None,
        "delete",
        actor,
        json!({ "before": serde_json::to_value(&before)?, "after": serde_json::to_value(&after)? }),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn get_fertiliser_material(conn: &Connection, id: &str) -> Result<FertiliserMaterialDetail> {
    let material = conn
        .query_row(
            "SELECT * FROM fertiliser_material WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_material,
        )
        .map_err(no_rows_to_not_found)?;
    let nutrients = nutrients_of(conn, &material.id)?;
    Ok(FertiliserMaterialDetail {
        material,
        nutrients,
    })
}

/// The material a stored record names, RETIRED ONES INCLUDED — the SIEX export.
///
/// A fertilisation record freezes only what section 6 prints; the full C.h
/// composition the descriptor asks for stays on the registry row, which is
/// soft-deleted precisely so a record written years ago can still resolve it.
/// The ordinary getter filters those out, which is right for a picker and wrong
/// for an export of a past campaign.
pub fn get_fertiliser_material_for_export(
    conn: &Connection,
    id: &str,
) -> Result<FertiliserMaterialDetail> {
    let material = conn
        .query_row(
            "SELECT * FROM fertiliser_material WHERE id = ?1",
            [id],
            map_material,
        )
        .map_err(no_rows_to_not_found)?;
    let nutrients = nutrients_of(conn, &material.id)?;
    Ok(FertiliserMaterialDetail {
        material,
        nutrients,
    })
}

/// The registry, alphabetically — a picker's order, not a record book's.
pub fn list_fertiliser_materials(conn: &Connection) -> Result<Vec<FertiliserMaterialDetail>> {
    let mut stmt =
        conn.prepare("SELECT * FROM fertiliser_material WHERE deleted_at IS NULL ORDER BY id")?;
    let materials = stmt
        .query_map([], map_material)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    materials
        .into_iter()
        .map(|material| {
            let nutrients = nutrients_of(conn, &material.id)?;
            Ok(FertiliserMaterialDetail {
                material,
                nutrients,
            })
        })
        .collect()
}

// --- composition -----------------------------------------------------------

fn reconcile_nutrients(
    tx: &Transaction,
    material_id: &str,
    desired: Vec<MaterialNutrient>,
    actor: Option<&str>,
) -> Result<Vec<MaterialNutrient>> {
    let current = nutrients_of_tx(tx, material_id)?;
    let same = |a: &MaterialNutrient, b: &MaterialNutrient| {
        a.kind_code == b.kind_code && a.nutrient_code == b.nutrient_code
    };

    for existing in &current {
        if !desired.iter().any(|d| same(d, existing)) {
            tx.execute(
                "DELETE FROM fertiliser_material_nutrient WHERE id = ?1",
                [&existing.id],
            )?;
            log_delete(
                tx,
                "fertiliser_material_nutrient",
                &existing.id,
                None,
                actor,
                &nutrient_image(existing, material_id),
                None::<&serde_json::Value>,
            )?;
        }
    }

    let mut rows = Vec::new();
    for want in desired {
        match current.iter().find(|c| same(c, &want)) {
            // A corrected percentage keeps its row identity, so the audit trail
            // reads as "this figure was wrong" rather than "this nutrient was
            // withdrawn and another added".
            Some(existing) => {
                if (existing.percentage - want.percentage).abs() > f64::EPSILON {
                    let after = MaterialNutrient {
                        id: existing.id.clone(),
                        percentage: want.percentage,
                        ..want
                    };
                    tx.execute(
                        "UPDATE fertiliser_material_nutrient SET percentage = ?2 WHERE id = ?1",
                        params![after.id, after.percentage],
                    )?;
                    log_update(
                        tx,
                        "fertiliser_material_nutrient",
                        &after.id,
                        None,
                        actor,
                        &nutrient_image(existing, material_id),
                        &nutrient_image(&after, material_id),
                    )?;
                    rows.push(after);
                } else {
                    rows.push(existing.clone());
                }
            }
            None => rows.push(insert_nutrient_row(tx, material_id, want, actor)?),
        }
    }
    rows.sort_by_key(|row| kind_rank(&row.kind_code));
    Ok(rows)
}

fn insert_nutrient_row(
    tx: &Transaction,
    material_id: &str,
    nutrient: MaterialNutrient,
    actor: Option<&str>,
) -> Result<MaterialNutrient> {
    let row = MaterialNutrient {
        id: Uuid::now_v7().to_string(),
        ..nutrient
    };
    tx.execute(
        "INSERT INTO fertiliser_material_nutrient (
            id, fertiliser_material_id, kind_code, nutrient_code, percentage
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.id,
            material_id,
            row.kind_code,
            row.nutrient_code,
            row.percentage
        ],
    )?;
    log_insert(
        tx,
        "fertiliser_material_nutrient",
        &row.id,
        None,
        actor,
        &nutrient_image(&row, material_id),
    )?;
    Ok(row)
}

/// The complete row image `record_change` requires. The model struct omits the
/// parent id (a caller already knows which material it asked for), so the log
/// image puts it back — a receiving device must rebuild the row from `after`
/// alone.
fn nutrient_image(row: &MaterialNutrient, material_id: &str) -> serde_json::Value {
    json!({
        "id": row.id,
        "fertiliser_material_id": material_id,
        "kind_code": row.kind_code,
        "nutrient_code": row.nutrient_code,
        "percentage": row.percentage,
    })
}

// --- validation ------------------------------------------------------------

/// Composition lines. Duplicates of the same (kind, code) fold — a form that
/// lists N total twice means one figure, and the UNIQUE index would reject the
/// second anyway.
///
/// Percentages are bounded at 100: a material cannot be more than itself, and
/// the one that slips through unbounded is a typo that would then be multiplied
/// by the dose in every unidad-fertilizante sum section 7.1 will assemble.
fn validated_nutrients(nutrients: &[MaterialNutrient]) -> Result<Vec<MaterialNutrient>> {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for nutrient in nutrients {
        if !NUTRIENT_KINDS.contains(&nutrient.kind_code.as_str()) {
            return Err(FertilisationError::Invalid("unknown_nutrient_kind"));
        }
        if nutrient.nutrient_code.trim().is_empty() {
            return Err(FertilisationError::Invalid("empty_nutrient_code"));
        }
        if nutrient.percentage.is_nan() || nutrient.percentage < 0.0 || nutrient.percentage > 100.0
        {
            return Err(FertilisationError::Invalid("invalid_percentage"));
        }
        if !seen.insert((nutrient.kind_code.clone(), nutrient.nutrient_code.clone())) {
            continue;
        }
        kept.push(MaterialNutrient {
            id: String::new(),
            kind_code: nutrient.kind_code.clone(),
            nutrient_code: nutrient.nutrient_code.trim().to_string(),
            percentage: nutrient.percentage,
        });
    }
    kept.sort_by_key(|row| kind_rank(&row.kind_code));
    Ok(kept)
}

/// Macronutrients, then micronutrients, then heavy metals — the order the SIEX
/// material block lists its three arrays in, and the order a label reads. Only
/// a stable one matters: a composition that came back from the database in a
/// different order than it went in would make one material look like two.
fn kind_rank(kind: &str) -> u8 {
    match kind {
        "macro" => 0,
        "micro" => 1,
        _ => 2,
    }
}

/// C.e's three supplier registries are mutually exclusive — the twin says so
/// in each of their own descriptions ("Excluyente con …"). The schema CHECK
/// enforces it too; this exists so the caller gets a machine code rather than a
/// constraint violation surfacing as an internal error.
fn validate_supplier(material: &FertiliserMaterial) -> Result<()> {
    let stated = [
        &material.supplier_rega,
        &material.supplier_tax_id,
        &material.supplier_nima,
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    if stated > 1 {
        return Err(FertilisationError::Invalid("supplier_id_conflict"));
    }
    Ok(())
}

/// Optional, but a stated density is a measurement: zero or negative is a typo.
fn validate_density(density: Option<f64>) -> Result<()> {
    match density {
        Some(value) if value.is_nan() || value <= 0.0 => {
            Err(FertilisationError::Invalid("invalid_density"))
        }
        _ => Ok(()),
    }
}

/// C.d's first level, against FEGA `MAT_FERTI` — 24 values, a closed list the
/// decree itself enumerates, so a code outside it is a typo rather than a
/// snapshot that has fallen behind. Checked only when the catalogue has been
/// imported; a bare test database has nothing to check against.
///
/// `material_detail_code` deliberately gets no such check: it names one of 1243
/// commercial products in a registry that grows between our snapshot releases,
/// and a laboratory-style list must not block a lawful record (the
/// `analysis_substance` rule).
fn validate_material_code(tx: &Transaction, code: &str) -> Result<()> {
    let imported: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalogue WHERE id = 'MAT_FERTI')",
        [],
        |r| r.get(0),
    )?;
    if !imported {
        return Ok(());
    }
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalogue_code
                       WHERE catalogue_id = 'MAT_FERTI' AND code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(FertilisationError::Invalid("unknown_material_code"));
    }
    Ok(())
}

fn validate_manure_treatment(tx: &Transaction, code: Option<&str>) -> Result<()> {
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(());
    };
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM manure_treatment WHERE code = ?1)",
        [code],
        |r| r.get(0),
    )?;
    if !known {
        return Err(FertilisationError::Invalid("unknown_manure_treatment"));
    }
    Ok(())
}

fn non_empty(value: String, code: &'static str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(FertilisationError::Invalid(code));
    }
    Ok(trimmed.to_string())
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- mapping ---------------------------------------------------------------

fn nutrients_of(conn: &Connection, material_id: &str) -> Result<Vec<MaterialNutrient>> {
    let mut stmt = conn.prepare(NUTRIENT_SQL)?;
    let rows = stmt
        .query_map([material_id], map_nutrient)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn nutrients_of_tx(tx: &Transaction, material_id: &str) -> Result<Vec<MaterialNutrient>> {
    let mut stmt = tx.prepare(NUTRIENT_SQL)?;
    let rows = stmt
        .query_map([material_id], map_nutrient)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Ordered by the seeded `nutrient_kind` rowid (macro, micro, heavy metal) and
/// then by insertion, so a material's composition reads the same way whether it
/// was just written or read back later.
const NUTRIENT_SQL: &str = "SELECT n.id, n.kind_code, n.nutrient_code, n.percentage
     FROM fertiliser_material_nutrient n
     JOIN nutrient_kind k ON k.code = n.kind_code
     WHERE n.fertiliser_material_id = ?1
     ORDER BY k.rowid, n.id";

fn map_material(row: &Row<'_>) -> rusqlite::Result<FertiliserMaterial> {
    Ok(FertiliserMaterial {
        id: row.get("id")?,
        name: row.get("name")?,
        material_code: row.get("material_code")?,
        material_detail_code: row.get("material_detail_code")?,
        supplier_name: row.get("supplier_name")?,
        supplier_rega: row.get("supplier_rega")?,
        supplier_tax_id: row.get("supplier_tax_id")?,
        supplier_nima: row.get("supplier_nima")?,
        manure_treatment_code: row.get("manure_treatment_code")?,
        density_kg_l: row.get("density_kg_l")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_nutrient(row: &Row<'_>) -> rusqlite::Result<MaterialNutrient> {
    Ok(MaterialNutrient {
        id: row.get(0)?,
        kind_code: row.get(1)?,
        nutrient_code: row.get(2)?,
        percentage: row.get(3)?,
    })
}
