// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! User profile CRUD. Soft-delete only: profile ids are the future author
//! stamp on `record_change.actor`, so a departed worker's row must keep
//! resolving in years-old audit trails. The ACTIVE profile is a per-device
//! choice stored in settings.json, not in this table.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{NewUserProfile, UpdateUserProfile, UserProfile};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

pub fn insert_user_profile(
    conn: &mut Connection,
    new: NewUserProfile,
    actor: Option<&str>,
) -> Result<UserProfile> {
    validate_name(&new.display_name)?;
    let tx = conn.transaction()?;
    validate_operator_link(&tx, new.operator_id.as_deref())?;
    let now = now_utc_iso();
    let profile = UserProfile {
        id: Uuid::now_v7().to_string(),
        display_name: new.display_name,
        operator_id: new.operator_id,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO user_profile (id, display_name, operator_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            profile.id,
            profile.display_name,
            profile.operator_id,
            profile.created_at,
            profile.updated_at
        ],
    )?;
    log_insert(&tx, "user_profile", &profile.id, None, actor, &profile)?;
    tx.commit()?;
    Ok(profile)
}

/// Active (non-deleted) profiles, for the Settings list and the
/// active-profile picker.
pub fn list_user_profiles(conn: &Connection) -> Result<Vec<UserProfile>> {
    let mut stmt = conn
        .prepare("SELECT * FROM user_profile WHERE deleted_at IS NULL ORDER BY display_name, id")?;
    let profiles = stmt
        .query_map([], map_user_profile)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(profiles)
}

/// Full-row update; the submitted state replaces the stored one
/// (`operator_id: None` unlinks the operator).
pub fn update_user_profile(
    conn: &mut Connection,
    id: &str,
    update: UpdateUserProfile,
    actor: Option<&str>,
) -> Result<UserProfile> {
    validate_name(&update.display_name)?;
    let tx = conn.transaction()?;
    validate_operator_link(&tx, update.operator_id.as_deref())?;
    let before = tx
        .query_row(
            "SELECT * FROM user_profile WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_user_profile,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.display_name = update.display_name;
    after.operator_id = update.operator_id;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE user_profile SET display_name = ?2, operator_id = ?3, updated_at = ?4
         WHERE id = ?1",
        params![id, after.display_name, after.operator_id, after.updated_at],
    )?;
    log_update(&tx, "user_profile", id, None, actor, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete: the row stays (the id must keep resolving as an author
/// stamp), it just leaves the profile list and the active-profile picker.
/// If this profile is the device's active one, the shell clears that
/// setting — the table doesn't know about settings.json.
pub fn soft_delete_user_profile(
    conn: &mut Connection,
    id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM user_profile WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_user_profile,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE user_profile SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "user_profile", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

/// A submitted operator link must point at an operator the pickers can
/// still show. The SQL FK guarantees existence; the non-deleted check is
/// ours (a soft-deleted operator would satisfy the FK).
fn validate_operator_link(conn: &Connection, operator_id: Option<&str>) -> Result<()> {
    let Some(operator_id) = operator_id else {
        return Ok(());
    };
    let exists = conn
        .query_row(
            "SELECT 1 FROM operator WHERE id = ?1 AND deleted_at IS NULL",
            [operator_id],
            |_| Ok(()),
        )
        .optional()?;
    if exists.is_none() {
        return Err(CoreError::Invalid("operator_not_found"));
    }
    Ok(())
}

fn map_user_profile(row: &Row) -> rusqlite::Result<UserProfile> {
    Ok(UserProfile {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        operator_id: row.get("operator_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
