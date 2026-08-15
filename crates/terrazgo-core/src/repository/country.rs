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

/// Irrigation systems (secano + the three watering methods), for the crop
/// form. Rowid order keeps the seeded rainfed → sprinkler → drip → gravity
/// progression, which is the order the official model lists the siglas in.
pub fn list_irrigation_systems(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM irrigation_system ORDER BY rowid",
    )
}

/// GIP frameworks (RD 1311/2012 art. 10-11), for the crop form's per-row GIP
/// column and the farm's advisory link. Rowid order keeps the seeded sequence,
/// which is the order the official model lists the siglas in.
pub fn list_gip_systems(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(conn, "SELECT code, i18n_key FROM gip_system ORDER BY rowid")
}

/// Open air / mesh / plastic cover / greenhouse, for the crop form.
pub fn list_growing_environments(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM growing_environment ORDER BY rowid",
    )
}

fn lookup_rows(conn: &Connection, sql: &str) -> Result<Vec<Lookup>> {
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

/// The units a PHYTOSANITARY dose can be expressed in: rates (l/ha, kg/ha, …)
/// first, then the concentrations a tank mix is measured in (g/l, %), which is
/// also the more common way Spanish labels state doses.
///
/// The set is named rather than derived from `dimension`, and that changed on
/// 2026-08-07 for a reason worth keeping. It used to be "every unit that is
/// not a quantity", which worked only while every rate in the table happened
/// to be a phytosanitary one. The fertilisation module then added m³/ha and
/// t/ha — impeccable rates, and nonsense on a plant-protection product — so
/// the filter would have started offering them here. `dimension` answers what
/// KIND of number a unit measures; it cannot answer which question the number
/// is an answer to, and only the second is what a picker needs.
///
/// Quantity units stay out for the older version of the same point: "12 l/ha"
/// and "12 l" answer different questions, and offering an amount where a rate
/// belongs invites a dose that reads as a false statement.
pub fn list_units(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM unit
         WHERE code IN ('l_ha','kg_ha','ml_ha','g_ha','ml_hl','g_hl','g_l','ml_l','pct')
         ORDER BY dimension DESC, code",
    )
}

/// The units a FERTILISER dose can be expressed in (Anexo III C.j, "cantidad
/// del producto fertilizante o material aplicado por hectárea"). Solids in
/// kg/ha or t/ha, liquids and slurries in l/ha or m³/ha — the last is why this
/// list exists separately from the phytosanitary one above.
pub fn list_fertiliser_dose_units(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM unit
         WHERE code IN ('kg_ha','l_ha','t_ha','m3_ha')
         ORDER BY CASE code
             WHEN 'kg_ha' THEN 1 WHEN 'l_ha' THEN 2
             WHEN 't_ha' THEN 3 ELSE 4 END",
    )
}

/// The units a volume of irrigation water can be expressed in. Anexo III C.l
/// names m³ per hectare, which the printed model repeats in its own column
/// heading; plain m³ is offered beside it because a meter reading is an
/// absolute volume and converting it for the farmer would be inventing a
/// hectare figure they did not state.
pub fn list_irrigation_volume_units(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM unit
         WHERE code IN ('m3_ha','m3')
         ORDER BY CASE code WHEN 'm3_ha' THEN 1 ELSE 2 END",
    )
}

/// Units that measure an AMOUNT: the total product used (Anexo III Parte I B.i
/// asks for kg or l), the tonnes / cubic metres the non-field registers measure
/// their treated subject in, and what leaves the holding at harvest. Litres and
/// kilograms first — they are what a phytosanitary product is sold and recorded
/// in.
pub fn list_quantity_units(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM unit
         WHERE dimension = 'quantity'
         ORDER BY CASE code WHEN 'l' THEN 1 WHEN 'kg' THEN 2 WHEN 't' THEN 3 ELSE 4 END",
    )
}

/// The units a NON-CHEMICAL measure's intensity can be expressed in — the
/// official model's "Intensidad de la medida (Nº de trampas, nº de difusores,
/// etc.)", which the SIEX `UNIDADES_MEDIDA` catalogue publishes both absolute
/// and per hectare.
///
/// Its own list for the same reason `list_units` is one: these are counts, and
/// offering a count where a dose belongs (or a dose where a count belongs)
/// invites a figure that reads as a false statement. Absolute before per
/// hectare, traps and diffusers before the generic unit — the order the model's
/// own examples suggest.
pub fn list_intensity_units(conn: &Connection) -> Result<Vec<Lookup>> {
    lookup_rows(
        conn,
        "SELECT code, i18n_key FROM unit
         WHERE dimension = 'intensity'
         ORDER BY CASE code
             WHEN 'traps' THEN 1 WHEN 'traps_ha' THEN 2
             WHEN 'diffusers' THEN 3 WHEN 'diffusers_ha' THEN 4
             WHEN 'units' THEN 5 ELSE 6 END",
    )
}

/// Operator licence levels (RD 1311/2012 niveles de capacitación), for the
/// operator form. Rowid order keeps the seeded rising progression.
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
