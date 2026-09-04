// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lookup lists owned by this module (reference data, seeded by its
//! migrations), for the eco-scheme register form selectors.

use crate::error::Result;
use crate::models::Lookup;
use rusqlite::Connection;

/// RD 1048/2022's six register-level annotation duties. Rowid order is the
/// seeded order, which follows the printed model's own section 9 pages, with
/// the duty that has no page last.
///
/// The whole list, unfiltered: which practices a holding may claim is a fact
/// about its solicitud única, which this app cannot see by any route
/// (docs/cuaderno-print.md → the eco-scheme registers). Narrowing per register
/// is the form's job, not this list's — a grazing form offers the practices a
/// grazing can evidence.
pub fn list_eco_practices(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM eco_practice ORDER BY rowid",
    )
}

/// What was done on the land (FEGA `TIPO_LABOR`, plus our split of its code 5
/// into siega and desbroce — the two columns model 9.4 prints). Rowid order is
/// the catalogue's own.
pub fn list_cultural_operation_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM cultural_operation_kind ORDER BY rowid",
    )
}

fn rows(conn: &Connection, sql: &str) -> Result<Vec<Lookup>> {
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
