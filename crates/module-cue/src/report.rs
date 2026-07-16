// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The printable cuaderno (PDF): data assembly for `templates/cuaderno.typ`,
//! rendered in-process by the shared report engine (`terrazgo-report`).
//!
//! Unlike the SIEX export there is NO precheck gate: the printed record book
//! shows what exists, and fields the official model asks for but the data
//! lacks print blank — a farmer must be able to print for an inspection
//! even while some registry data is incomplete. Soft-deleted records are
//! audit history and never print.
//!
//! Everything the template receives is a pre-formatted STRING (dd/mm/yyyy
//! dates, decimal-comma numbers, Spanish display words for the closed
//! lookups): the assembly is where all knowledge lives, the template only
//! does layout. Cross-references follow the official model: section 3.1
//! names operators/equipment/plots by the order numbers of tables 1.2, 1.3
//! and 2.1, and all four lists are built here from the same records, so a
//! reference can never dangle.

use crate::error::Result;
use crate::export::crop_groups;
use crate::models::TreatmentRecordWithPlots;
use crate::repository::list_treatment_records;
use crate::siex;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::HashMap;
use terrazgo_report::RenderedPdf;

const TEMPLATE: &str = include_str!("../templates/cuaderno.typ");

/// Assemble and render the cuaderno for one farm+season.
pub fn render_cuaderno(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
) -> Result<RenderedPdf> {
    let inputs = cuaderno_inputs(conn, season_id, farm_id, generated_on_iso)?;
    Ok(terrazgo_report::render_pdf(TEMPLATE, &inputs)?)
}

/// The template's `sys.inputs`, public so tests can pin the data contract
/// without parsing a PDF. `generated_on_iso` is passed in (not read from the
/// clock) so output is reproducible.
pub fn cuaderno_inputs(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
) -> Result<Value> {
    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;
    let campaign: String =
        conn.query_row("SELECT label FROM season WHERE id = ?1", [season_id], |r| {
            r.get(0)
        })?;

    // Register order: oldest first — a record book reads chronologically.
    let mut records = list_treatment_records(conn, season_id, farm_id)?;
    records.reverse();

    let plot_orders = plot_rows(conn, season_id, farm_id)?;
    let operators = operator_rows(conn, &records);
    let machinery = machinery_rows(conn, &records)?;
    let treatments = treatment_rows(conn, &records, &plot_orders.orders, &operators, &machinery)?;

    let es = farm.farm; // shadow the detail struct; extension handled below
    let ext = farm.es;
    Ok(json!({
        "campaign": campaign,
        "generated_on": date_es(generated_on_iso),
        "farm": {
            "name": es.name,
            "owner": es.owner_name.unwrap_or_default(),
            "nif": es.owner_tax_id.unwrap_or_default(),
            "rea": ext.as_ref().and_then(|e| e.rea_code.clone()).unwrap_or_default(),
            "location": es.location_text.unwrap_or_default(),
            "province": ext.as_ref().and_then(|e| e.province_code.clone()).unwrap_or_default(),
        },
        "operators": operators.iter().map(|o| json!({
            "order": o.order.to_string(),
            "name": o.name,
            "nif": "",
            "licence": o.licence,
            "level": o.level_label(),
        })).collect::<Vec<_>>(),
        "machinery": machinery.iter().map(|m| json!({
            "order": m.order.to_string(),
            "description": m.description,
            "roma": m.roma,
            "reganip": m.reganip,
            "last_inspection": m.last_inspection,
        })).collect::<Vec<_>>(),
        "plot_rows": plot_orders.rows,
        "treatments": treatments,
    }))
}

// ---------------------------------------------------------------------------
// Section 2.1 — parcelas (+ the plot_id → order map 3.1 references)
// ---------------------------------------------------------------------------

struct PlotRows {
    rows: Vec<Value>,
    orders: HashMap<String, usize>,
}

/// All active plots of the farm, ordered by name; one row per (plot, season
/// crop), repeating the plot's order number — the model groups rows by
/// parcela. A plot without a crop this season still prints (blank species).
fn plot_rows(conn: &Connection, season_id: &str, farm_id: &str) -> Result<PlotRows> {
    let plots = terrazgo_core::repository::list_plots(conn, farm_id)?;
    let crops = terrazgo_core::repository::list_crops(conn, season_id, farm_id)?;

    let mut rows = Vec::new();
    let mut orders = HashMap::new();
    for (idx, detail) in plots.iter().enumerate() {
        let order = idx + 1;
        orders.insert(detail.plot.id.clone(), order);
        let es = detail.es.as_ref();
        let sigpac = |field: Option<&String>| field.cloned().unwrap_or_default();
        let base = json!({
            "order": order.to_string(),
            "name": detail.plot.name,
            "province": sigpac(es.and_then(|e| e.sigpac_province.as_ref())),
            "municipality": sigpac(es.and_then(|e| e.sigpac_municipality.as_ref())),
            "aggregate": sigpac(es.and_then(|e| e.sigpac_aggregate.as_ref())),
            "zone": sigpac(es.and_then(|e| e.sigpac_zone.as_ref())),
            "polygon": sigpac(es.and_then(|e| e.sigpac_polygon.as_ref())),
            "parcel": sigpac(es.and_then(|e| e.sigpac_parcel.as_ref())),
            "enclosure": sigpac(es.and_then(|e| e.sigpac_enclosure.as_ref())),
            "area": detail.plot.area_ha.map(number_es).unwrap_or_default(),
            "species": "",
            "variety": "",
            "gip": "",
        });

        let plot_crops: Vec<_> = crops
            .iter()
            .filter(|c| c.plot_id == detail.plot.id && c.deleted_at.is_none())
            .collect();
        if plot_crops.is_empty() {
            rows.push(base);
        } else {
            for crop in plot_crops {
                let mut row = base.clone();
                row["species"] = json!(crop.species_name);
                row["variety"] = json!(crop.variety.clone().unwrap_or_default());
                row["gip"] = json!(gip_code(crop.production_system_code.as_deref()));
                rows.push(row);
            }
        }
    }
    Ok(PlotRows { rows, orders })
}

/// The model's GIP column speaks its own sigla; only two of our production
/// systems have an official equivalent — (AE) Agricultura Ecológica, (PI)
/// Producción Integrada. Conventional farming is not an advisory system, so
/// it prints blank.
fn gip_code(production_system: Option<&str>) -> &'static str {
    match production_system {
        Some("organic") => "AE",
        Some("integrated") => "PI",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Sections 1.2 / 1.3 — who and what applied the treatments
// ---------------------------------------------------------------------------

struct OperatorRow {
    operator_id: String,
    order: usize,
    name: String,
    licence: String,
    level: Option<String>,
}

impl OperatorRow {
    /// Spanish display word for the licence level (carné de aplicador,
    /// RD 1311/2012 niveles de capacitación).
    fn level_label(&self) -> &'static str {
        match self.level.as_deref() {
            Some("basic") => "Básico",
            Some("qualified") => "Cualificado",
            Some("fumigator") => "Fumigador",
            _ => "",
        }
    }
}

/// Operators as the records name them — identity is the FK, display values
/// are the record snapshots (the legal values), latest record wins when an
/// operator was edited between treatments. Order = first appearance in the
/// chronological register. The carné level is not snapshotted (only the
/// number is), so it reads from the operator row, blank if gone.
fn operator_rows(conn: &Connection, records: &[TreatmentRecordWithPlots]) -> Vec<OperatorRow> {
    let mut rows: Vec<OperatorRow> = Vec::new();
    for rec in records {
        let record = &rec.record;
        match rows
            .iter_mut()
            .find(|o| o.operator_id == record.operator_id)
        {
            Some(row) => {
                row.name = record.operator_name_snapshot.clone();
                row.licence = record.operator_licence_snapshot.clone().unwrap_or_default();
            }
            None => {
                let level: Option<String> = conn
                    .query_row(
                        "SELECT licence_level_code FROM operator WHERE id = ?1",
                        [&record.operator_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                rows.push(OperatorRow {
                    operator_id: record.operator_id.clone(),
                    order: rows.len() + 1,
                    name: record.operator_name_snapshot.clone(),
                    licence: record.operator_licence_snapshot.clone().unwrap_or_default(),
                    level,
                });
            }
        }
    }
    rows
}

struct MachineryRow {
    machinery_id: String,
    order: usize,
    description: String,
    roma: String,
    reganip: String,
    last_inspection: String,
}

/// Equipment as the records name it: registry numbers from the snapshots,
/// description and inspection date from the current row when it still
/// exists (a deleted machine keeps printing through its snapshots).
fn machinery_rows(
    conn: &Connection,
    records: &[TreatmentRecordWithPlots],
) -> Result<Vec<MachineryRow>> {
    let mut rows: Vec<MachineryRow> = Vec::new();
    for rec in records {
        let record = &rec.record;
        let Some(machinery_id) = &record.machinery_id else {
            continue; // manual application — no 1.3 entry
        };
        if rows.iter().any(|m| &m.machinery_id == machinery_id) {
            continue;
        }
        let current: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT name, last_inspection_date FROM machinery WHERE id = ?1",
                [machinery_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let (description, last_inspection) = match current {
            Some((name, inspection)) => (name, inspection.map(|d| date_es(&d)).unwrap_or_default()),
            None => (String::new(), String::new()),
        };
        rows.push(MachineryRow {
            machinery_id: machinery_id.clone(),
            order: rows.len() + 1,
            description,
            roma: record.machinery_roma_snapshot.clone().unwrap_or_default(),
            reganip: record
                .machinery_reganip_snapshot
                .clone()
                .unwrap_or_default(),
            last_inspection,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Section 3.1 — the register
// ---------------------------------------------------------------------------

/// One printed row per (record, crop-snapshot group) — the same split the
/// SIEX export applies, so the paper register and the electronic one always
/// carry the same line items.
fn treatment_rows(
    conn: &Connection,
    records: &[TreatmentRecordWithPlots],
    plot_orders: &HashMap<String, usize>,
    operators: &[OperatorRow],
    machinery: &[MachineryRow],
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for rec in records {
        let record = &rec.record;
        let date = date_es(&record.application_date);
        let problems = problem_labels(conn, rec)?;
        let operator = operators
            .iter()
            .find(|o| o.operator_id == record.operator_id)
            .map(|o| o.order.to_string())
            .unwrap_or_default();
        let equipment = match &record.machinery_id {
            None => "Manual".to_string(),
            Some(id) => machinery
                .iter()
                .find(|m| &m.machinery_id == id)
                .map(|m| m.order.to_string())
                .unwrap_or_default(),
        };
        let dose = format!(
            "{} {}",
            number_es(record.dose_value),
            unit_es(&record.dose_unit_code)
        );
        let phi = format!(
            "{} días (hasta {})",
            record.phi_days_used,
            date_es(&record.phi_end_date)
        );

        for (_, plots) in crop_groups(&rec.plots) {
            let mut order_refs: Vec<usize> = plots
                .iter()
                .filter_map(|p| plot_orders.get(&p.plot_id).copied())
                .collect();
            order_refs.sort_unstable();
            let surface: f64 = plots.iter().map(|p| p.surface_treated_ha).sum();
            let first = plots.first();
            rows.push(json!({
                "plots": order_refs.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                "species": first.and_then(|p| p.crop_name_snapshot.clone()).unwrap_or_default(),
                "variety": first.and_then(|p| p.variety_snapshot.clone()).unwrap_or_default(),
                "date": date,
                "surface": number_es(surface),
                "problems": problems,
                "operator": operator,
                "equipment": equipment,
                "product": record.product_name_snapshot,
                "reg_no": record.authorisation_number_snapshot.clone().unwrap_or_default(),
                "dose": dose,
                "phi": phi,
                "efficacy": efficacy_es(record.efficacy_code.as_deref()),
                "notes": record.notes.clone().unwrap_or_default(),
            }));
        }
    }
    Ok(rows)
}

/// Problem codes resolved to their official Spanish catalogue labels,
/// joined "; ". A code the imported catalogues cannot resolve (or a test
/// database without catalogues) prints the code itself — the record's legal
/// payload is the code, the label is display sugar.
fn problem_labels(conn: &Connection, rec: &TreatmentRecordWithPlots) -> Result<String> {
    let mut labels = Vec::new();
    for problem in &rec.problems {
        let label =
            siex::problem_catalogue(&rec.record.country_code, &problem.reason_category_code)
                .and_then(|catalogue| {
                    terrazgo_core::catalogue::find_code(conn, catalogue, &problem.problem_code)
                        .ok()
                        .and_then(|rows| rows.into_iter().next())
                        .map(|row| row.label)
                })
                .unwrap_or_else(|| problem.problem_code.clone());
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(labels.join("; "))
}

// ---------------------------------------------------------------------------
// Spanish display formatting
// ---------------------------------------------------------------------------

/// ISO date → dd/mm/yyyy; anything unparseable passes through verbatim (a
/// printout must never lose data over a malformed historical value).
fn date_es(iso: &str) -> String {
    siex::date_to_siex(iso).unwrap_or_else(|| iso.to_string())
}

/// Decimal-comma number, up to 4 decimals, trailing zeros trimmed
/// ("1,5", "2", "0,0375").
fn number_es(value: f64) -> String {
    let s = format!("{value:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// Dose-unit display symbol (the same closed list `unit.*` i18n keys cover
/// in the UI; the template is Spanish content, so the mapping lives here).
fn unit_es(code: &str) -> &'static str {
    match code {
        "l_ha" => "L/ha",
        "kg_ha" => "kg/ha",
        "ml_ha" => "ml/ha",
        "g_ha" => "g/ha",
        "ml_hl" => "ml/hl",
        "g_hl" => "g/hl",
        "g_l" => "g/L",
        "ml_l" => "ml/L",
        "pct" => "%",
        _ => "",
    }
}

/// The model's footnote wording: buena, regular o mala ("indicar buena,
/// regular o mala" — Andalucía model, section 3.1 footnote 5).
fn efficacy_es(code: Option<&str>) -> &'static str {
    match code {
        Some("good") => "Buena",
        Some("fair") => "Regular",
        Some("poor") => "Mala",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_render_with_decimal_comma_and_no_trailing_zeros() {
        assert_eq!(number_es(1.5), "1,5");
        assert_eq!(number_es(2.0), "2");
        assert_eq!(number_es(0.0375), "0,0375");
        assert_eq!(number_es(12.25), "12,25");
    }

    #[test]
    fn closed_lookups_map_to_the_official_spanish_words() {
        // Efficacy wording per the model's footnote: buena, regular o mala.
        assert_eq!(efficacy_es(Some("good")), "Buena");
        assert_eq!(efficacy_es(Some("fair")), "Regular");
        assert_eq!(efficacy_es(Some("poor")), "Mala");
        assert_eq!(efficacy_es(None), "");
        // GIP sigla per the model's section 2.1 footnote 2.
        assert_eq!(gip_code(Some("organic")), "AE");
        assert_eq!(gip_code(Some("integrated")), "PI");
        assert_eq!(gip_code(Some("conventional")), "");
        assert_eq!(gip_code(None), "");
    }

    #[test]
    fn every_seeded_dose_unit_has_a_display_symbol() {
        for code in [
            "l_ha", "kg_ha", "ml_ha", "g_ha", "ml_hl", "g_hl", "g_l", "ml_l", "pct",
        ] {
            assert!(!unit_es(code).is_empty(), "unit '{code}' prints blank");
        }
    }
}
