// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core lookups (reference data, seeded by the core migrations).

use crate::error::Result;
use crate::models::{Country, Lookup};
use rusqlite::Connection;

/// Every country the app knows, for selectors. Codes are stable; display names
/// come from the i18n layer via `i18n_key`.
pub fn list_countries(conn: &Connection) -> Result<Vec<Country>> {
    let mut stmt = conn.prepare("SELECT code, i18n_key FROM country ORDER BY code")?;
    let countries = stmt
        .query_map([], |r| {
            Ok(Country {
                code: r.get(0)?,
                i18n_key: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(countries)
}

/// Production systems (conventional/organic/integrated), for the crop form.
pub fn list_production_systems(conn: &Connection) -> Result<Vec<Lookup>> {
    let mut stmt = conn.prepare("SELECT code, i18n_key FROM production_system ORDER BY code")?;
    let systems = stmt
        .query_map([], |r| {
            Ok(Lookup {
                code: r.get(0)?,
                i18n_key: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(systems)
}

/// Operator licence levels (basic/qualified/fumigator), for the operator form.
/// Rowid order keeps the seeded basic → qualified → fumigator progression.
pub fn list_licence_levels(conn: &Connection) -> Result<Vec<Lookup>> {
    let mut stmt = conn.prepare("SELECT code, i18n_key FROM licence_level ORDER BY rowid")?;
    let levels = stmt
        .query_map([], |r| {
            Ok(Lookup {
                code: r.get(0)?,
                i18n_key: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(levels)
}
