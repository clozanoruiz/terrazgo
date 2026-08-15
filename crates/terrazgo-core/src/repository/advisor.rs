// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Advisor CRUD and the farm ↔ advisor link (official model 1.4).
//!
//! Advisors are NOT farm-scoped: one advisory entity serves many holdings, so
//! the entity lives on its own (like `operator`) and `farm_advisor` attaches it
//! to a farm together with the GIP framework that relationship runs under.
//!
//! Soft-delete only on both: table 1.4 of a past campaign's printed book must
//! still resolve the advisor it named.

use super::validate_name;
use crate::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::{Advisor, FarmAdvisor, FarmAdvisorDetail, NewAdvisor, UpdateAdvisor};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

pub fn insert_advisor(
    conn: &mut Connection,
    new: NewAdvisor,
    actor: Option<&str>,
) -> Result<Advisor> {
    validate_name(&new.name)?;
    let tx = conn.transaction()?;
    let now = now_utc_iso();
    let advisor = Advisor {
        id: Uuid::now_v7().to_string(),
        name: new.name,
        tax_id: new.tax_id,
        registration_number: new.registration_number,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO advisor (id, name, tax_id, registration_number, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            advisor.id,
            advisor.name,
            advisor.tax_id,
            advisor.registration_number,
            advisor.created_at,
            advisor.updated_at
        ],
    )?;
    log_insert(&tx, "advisor", &advisor.id, None, actor, &advisor)?;
    tx.commit()?;
    Ok(advisor)
}

/// Every active advisor, for the registry list and the farm's link picker.
pub fn list_advisors(conn: &Connection) -> Result<Vec<Advisor>> {
    let mut stmt = conn.prepare("SELECT * FROM advisor WHERE deleted_at IS NULL ORDER BY id")?;
    let advisors = stmt
        .query_map([], map_advisor)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(advisors)
}

/// Full-row update; the submitted state replaces the stored one.
pub fn update_advisor(
    conn: &mut Connection,
    id: &str,
    update: UpdateAdvisor,
    actor: Option<&str>,
) -> Result<Advisor> {
    validate_name(&update.name)?;
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM advisor WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_advisor,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.name = update.name;
    after.tax_id = update.tax_id;
    after.registration_number = update.registration_number;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE advisor SET name = ?2, tax_id = ?3, registration_number = ?4, updated_at = ?5
         WHERE id = ?1",
        params![
            id,
            after.name,
            after.tax_id,
            after.registration_number,
            after.updated_at
        ],
    )?;
    log_update(&tx, "advisor", id, None, actor, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete the advisor AND its farm links: an advisor who is gone advises
/// nobody, and leaving dangling links would keep printing them in table 1.4.
/// Every removed link is logged on its own, so the audit trail says which
/// holdings the removal touched.
pub fn soft_delete_advisor(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM advisor WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_advisor,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();

    let links = {
        let mut stmt =
            tx.prepare("SELECT * FROM farm_advisor WHERE advisor_id = ?1 AND deleted_at IS NULL")?;
        stmt.query_map([id], map_link)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for link in links {
        let mut after = link.clone();
        after.deleted_at = Some(now.clone());
        after.updated_at = now.clone();
        tx.execute(
            "UPDATE farm_advisor SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![link.id, now],
        )?;
        log_delete(
            &tx,
            "farm_advisor",
            &link.id,
            None,
            actor,
            &link,
            Some(&after),
        )?;
    }

    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE advisor SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "advisor", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

/// A farm's advisory links with the advisor each points at, ordered by advisor
/// name — the order table 1.4 prints them in.
pub fn list_farm_advisors(conn: &Connection, farm_id: &str) -> Result<Vec<FarmAdvisorDetail>> {
    // Columns are listed and read positionally: the two tables share column
    // names (`id`, `created_at`, …) and a name lookup would silently return
    // the join's first match for both structs.
    let mut stmt = conn.prepare(
        "SELECT farm_advisor.id, farm_advisor.farm_id, farm_advisor.advisor_id,
                farm_advisor.gip_system_code, farm_advisor.created_at, farm_advisor.updated_at,
                farm_advisor.deleted_at,
                advisor.id, advisor.name, advisor.tax_id, advisor.registration_number,
                advisor.created_at, advisor.updated_at, advisor.deleted_at
         FROM farm_advisor
         JOIN advisor ON advisor.id = farm_advisor.advisor_id
         WHERE farm_advisor.farm_id = ?1
           AND farm_advisor.deleted_at IS NULL AND advisor.deleted_at IS NULL
         ORDER BY advisor.id",
    )?;
    let rows = stmt
        .query_map([farm_id], |row| {
            Ok(FarmAdvisorDetail {
                link: FarmAdvisor {
                    id: row.get(0)?,
                    farm_id: row.get(1)?,
                    advisor_id: row.get(2)?,
                    gip_system_code: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    deleted_at: row.get(6)?,
                },
                advisor: Advisor {
                    id: row.get(7)?,
                    name: row.get(8)?,
                    tax_id: row.get(9)?,
                    registration_number: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    deleted_at: row.get(13)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Attach an advisor to a farm under a GIP framework, or update the framework
/// of an existing link. Upsert rather than create+update because the form
/// states a relationship ("this farm is advised by X under Y"), and stating it
/// twice must not print the advisor twice in table 1.4 — the unique index
/// enforces the same thing at the schema level.
pub fn set_farm_advisor(
    conn: &mut Connection,
    farm_id: &str,
    advisor_id: &str,
    gip_system_code: Option<String>,
    actor: Option<&str>,
) -> Result<FarmAdvisor> {
    let tx = conn.transaction()?;
    // Both ends must exist and be active: a link to a deleted advisor would
    // print a name the registry no longer shows.
    let known: i64 = tx.query_row(
        "SELECT COUNT(*) FROM advisor WHERE id = ?1 AND deleted_at IS NULL",
        [advisor_id],
        |row| row.get(0),
    )?;
    if known == 0 {
        return Err(CoreError::NotFound);
    }

    let existing = tx
        .query_row(
            "SELECT * FROM farm_advisor
             WHERE farm_id = ?1 AND advisor_id = ?2 AND deleted_at IS NULL",
            params![farm_id, advisor_id],
            map_link,
        )
        .optional()?;
    let now = now_utc_iso();

    let link = match existing {
        Some(before) => {
            let mut after = before.clone();
            after.gip_system_code = gip_system_code;
            after.updated_at = now;
            tx.execute(
                "UPDATE farm_advisor SET gip_system_code = ?2, updated_at = ?3 WHERE id = ?1",
                params![after.id, after.gip_system_code, after.updated_at],
            )?;
            log_update(&tx, "farm_advisor", &after.id, None, actor, &before, &after)?;
            after
        }
        None => {
            let link = FarmAdvisor {
                id: Uuid::now_v7().to_string(),
                farm_id: farm_id.to_string(),
                advisor_id: advisor_id.to_string(),
                gip_system_code,
                created_at: now.clone(),
                updated_at: now,
                deleted_at: None,
            };
            tx.execute(
                "INSERT INTO farm_advisor
                   (id, farm_id, advisor_id, gip_system_code, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    link.id,
                    link.farm_id,
                    link.advisor_id,
                    link.gip_system_code,
                    link.created_at,
                    link.updated_at
                ],
            )?;
            log_insert(&tx, "farm_advisor", &link.id, None, actor, &link)?;
            link
        }
    };
    tx.commit()?;
    Ok(link)
}

/// Detach an advisor from a farm. Soft delete: the link is history of who
/// advised the holding, and re-attaching later writes a fresh row.
pub fn remove_farm_advisor(conn: &mut Connection, id: &str, actor: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM farm_advisor WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_link,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE farm_advisor SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "farm_advisor", id, None, actor, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

fn map_advisor(row: &Row) -> rusqlite::Result<Advisor> {
    Ok(Advisor {
        id: row.get("id")?,
        name: row.get("name")?,
        tax_id: row.get("tax_id")?,
        registration_number: row.get("registration_number")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_link(row: &Row) -> rusqlite::Result<FarmAdvisor> {
    Ok(FarmAdvisor {
        id: row.get("id")?,
        farm_id: row.get("farm_id")?,
        advisor_id: row.get("advisor_id")?,
        gip_system_code: row.get("gip_system_code")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
