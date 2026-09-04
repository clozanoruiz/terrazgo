// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lookup lists owned by this module (reference data, seeded by its
//! migrations), for the fertilisation and irrigation form selectors.

use crate::error::Result;
use crate::models::{ApplicationMethod, Lookup};
use rusqlite::Connection;

/// The eight irrigation systems of model section 8's footnote (FEGA
/// `SIST_RIEGO`). Rowid order is the seeded order, which is the order the
/// printed form lists them in — a selector should read like the paper.
pub fn list_irrigation_methods(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM irrigation_method ORDER BY rowid",
    )
}

/// Where the irrigation water came from (FEGA `ORIGEN_AGUA_RIEGO`). Several
/// can apply to one irrigation, so this fills a multi-select, not a dropdown.
pub fn list_water_origins(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM water_origin ORDER BY rowid",
    )
}

/// Anexo III C.c's three (FEGA `TIPO_FERITILIZACION`). Deliberately NOT the
/// model's "(F)/(AF)/(AC)" list: fertirrigación belongs to the application
/// method below, and offering it here would let a farmer state a *way of
/// applying* where the decree asks for a *kind of fertilisation*.
pub fn list_fertilisation_types(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM fertilisation_type ORDER BY rowid",
    )
}

/// Anexo III C.f's seven (FEGA `METODO_APLICACION_FERTILIZANTE`), two of them
/// fertigation.
pub fn list_application_methods(conn: &Connection) -> Result<Vec<ApplicationMethod>> {
    let mut stmt = conn
        .prepare("SELECT code, i18n_key, is_fertigation FROM application_method ORDER BY rowid")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ApplicationMethod {
                code: row.get(0)?,
                i18n_key: row.get(1)?,
                is_fertigation: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// What the manure received before it was spread (FEGA `TRAT_ESTIERCOLES`) —
/// a property of the material, so this fills a field on the registry form, not
/// on a record.
pub fn list_manure_treatments(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM manure_treatment ORDER BY rowid",
    )
}

/// Which of the three FEGA nutrient catalogues a composition line indexes.
pub fn list_nutrient_kinds(conn: &Connection) -> Result<Vec<Lookup>> {
    rows(
        conn,
        "SELECT code, i18n_key FROM nutrient_kind ORDER BY rowid",
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
