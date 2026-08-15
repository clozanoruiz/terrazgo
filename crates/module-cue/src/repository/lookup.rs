// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! CUE lookup lists (reference data, seeded by the CUE migrations): treatment
//! reason categories, formulation types and the coded capture lists, for the
//! treatment form's selectors.
//!
//! Unit lists moved to `terrazgo_core::repository` with the `unit` table
//! (2026-08-07); they are re-exported from this crate's `repository` so
//! existing callers are unaffected.

use crate::error::Result;
use crate::models::Lookup;
use rusqlite::Connection;

/// Treatment reason categories (pest/disease/weed/…), RD 1311/2012's "reason
/// for treatment".
pub fn list_reason_categories(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM reason_category ORDER BY code",
    )
}

/// Product formulation types (WP, SC, EC, …), for the product form.
pub fn list_formulation_types(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM formulation_type ORDER BY code",
    )
}

/// Observed treatment efficacies (good/fair/poor), best first — the natural
/// reading order for a rating.
pub fn list_efficacies(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM efficacy
         ORDER BY CASE code WHEN 'good' THEN 1 WHEN 'fair' THEN 2 ELSE 3 END",
    )
}

/// IPM justifications for treating (Directive 2009/128/CE), for the form's
/// multi-select.
pub fn list_justifications(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM justification ORDER BY code",
    )
}

/// The three subjects the non-field registers cover (model 3.3/3.4/3.5), in
/// the order the model prints them.
pub fn list_non_field_subject_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM non_field_subject_kind
         ORDER BY CASE code
             WHEN 'postharvest' THEN 1
             WHEN 'storage_premises' THEN 2
             ELSE 3
         END",
    )
}

/// What model section 4 calls "Material analizado", in the order FEGA's own
/// catalogue lists it: cultivo, producto cosechado, suelo, agua de riego.
pub fn list_analysis_materials(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM analysis_material
         ORDER BY CASE code
             WHEN 'crop' THEN 1
             WHEN 'harvested_produce' THEN 2
             WHEN 'soil' THEN 3
             ELSE 4
         END",
    )
}

/// What the laboratory looked for, in FEGA catalogue order.
pub fn list_analysis_types(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM analysis_type
         ORDER BY CASE code
             WHEN 'pesticide_residues' THEN 1
             WHEN 'microbiological' THEN 2
             WHEN 'heavy_metals' THEN 3
             WHEN 'nutrients' THEN 4
             WHEN 'soil_parameters' THEN 5
             ELSE 6
         END",
    )
}

/// Where the treated seed was treated (model 3.2), in FEGA catalogue order.
pub fn list_seed_treatment_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM seed_treatment_kind
         ORDER BY CASE code
             WHEN 'on_farm' THEN 1
             WHEN 'processing_centre' THEN 2
             WHEN 'purchased_es' THEN 3
             ELSE 4
         END",
    )
}

/// The conditional registers whose "APLICA TRATAMIENTO: NO" is stored rather
/// than derived, in model order (3.2 seed treatment first).
pub fn list_register_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM register_kind
         ORDER BY CASE code
             WHEN 'seed_treatment' THEN 1
             WHEN 'postharvest' THEN 2
             WHEN 'storage_premises' THEN 3
             ELSE 4
         END",
    )
}

/// Authorisation kinds (registered/parallel import/…), for the product form.
pub fn list_authorisation_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM authorisation_kind
         ORDER BY CASE code WHEN 'registered' THEN 1 ELSE 2 END, code",
    )
}

fn list(conn: &Connection, sql: &str) -> Result<Vec<Lookup>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Lookup {
                code: r.get(0)?,
                i18n_key: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
