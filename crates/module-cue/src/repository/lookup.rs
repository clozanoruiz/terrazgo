// SPDX-License-Identifier: AGPL-3.0-or-later

//! CUE lookup lists (reference data, seeded by the CUE migrations): dose units
//! and treatment reason categories, for the treatment form's selectors.

use crate::error::Result;
use crate::models::Lookup;
use rusqlite::Connection;

/// Every dose unit. Dose-rate units (l/ha, kg/ha, …) come before concentration
/// units (g/l, %): sorting on `dimension` DESC puts 'dose_rate' first, and that
/// is also the more common way Spanish labels state doses.
pub fn list_units(conn: &Connection) -> Result<Vec<Lookup>> {
    list(
        conn,
        "SELECT code, i18n_key FROM unit ORDER BY dimension DESC, code",
    )
}

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
