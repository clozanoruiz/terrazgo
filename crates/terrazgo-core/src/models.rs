// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core entity structs mirroring the core-owned schema.
//!
//! Domain structs derive `Serialize` so the repository can freeze a full row into the
//! `record_change.payload` JSON (and so the shell can hand them to the UI). Input
//! structs (`New*`, `Update*`) also derive `Deserialize` because they arrive as JSON
//! through Tauri commands; the repository fills in `id` (via `Uuid::now_v7()`) and
//! timestamps.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Country {
    pub code: String,
    pub i18n_key: String,
}

/// Generic seeded lookup row (production system, dose unit, reason category, …):
/// a stable code plus the i18n key the display layer translates it with.
#[derive(Debug, Clone, Serialize)]
pub struct Lookup {
    pub code: String,
    pub i18n_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Season {
    pub id: String,
    pub campaign_year: i64,
    pub label: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Crop {
    pub id: String,
    pub plot_id: String,
    pub season_id: String,
    pub species_name: String,
    pub variety: Option<String>,
    pub production_system_code: Option<String>,
    pub sown_on: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Operator {
    pub id: String,
    pub full_name: String,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Machinery {
    pub id: String,
    pub farm_id: String,
    pub name: String,
    /// The column is `type` (a Rust keyword, so the field is `kind`); `#[serde(rename)]`
    /// makes the audit payload use the real column name.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub last_inspection_date: Option<String>,
    pub next_inspection_due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Spanish extension row for machinery. Logged to `record_change` as its own entity
/// (`entity_id` = `machinery_id`, the table's PK) because it is synced user data too.
/// Two complementary registries: ROMA for mobile machinery (the typical sprayer),
/// REGANIP for aircraft and fixed/semi-mobile installations.
#[derive(Debug, Clone, Serialize)]
pub struct MachineryEsExtension {
    pub machinery_id: String,
    pub roma_number: Option<String>,
    pub reganip_number: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Farm {
    pub id: String,
    pub name: String,
    pub owner_name: Option<String>,
    /// Tax/identity number of the legal holder (NIF/CUAA/SIREN…); regulatory
    /// exports name the holder with it. Format validation is per-country.
    pub owner_tax_id: Option<String>,
    pub location_text: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country_code: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Spanish extension row for farm. Logged to `record_change` as its own entity
/// (`entity_id` = `farm_id`, the table's PK) because it is synced user data too.
#[derive(Debug, Clone, Serialize)]
pub struct FarmEsExtension {
    pub farm_id: String,
    pub rega_code: Option<String>,
    /// REA registration code (REACYL in CyL) — the SIEX export's CodigoRea,
    /// user-entered from the farm's REA papers. REGA is the livestock registry;
    /// the two are different registrations.
    pub rea_code: Option<String>,
    pub province_code: Option<String>,
}

/// A farm with its regional extension — what the edit form needs in one round trip.
#[derive(Debug, Clone, Serialize)]
pub struct FarmDetail {
    pub farm: Farm,
    pub es: Option<FarmEsExtension>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plot {
    pub id: String,
    pub farm_id: String,
    pub name: String,
    pub area_ha: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Spanish extension row for plot: the SIGPAC reference.
#[derive(Debug, Clone, Serialize)]
pub struct PlotEsExtension {
    pub plot_id: String,
    pub sigpac_province: Option<String>,
    pub sigpac_municipality: Option<String>,
    pub sigpac_aggregate: Option<String>,
    pub sigpac_zone: Option<String>,
    pub sigpac_polygon: Option<String>,
    pub sigpac_parcel: Option<String>,
    pub sigpac_enclosure: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlotDetail {
    pub plot: Plot,
    pub es: Option<PlotEsExtension>,
}

/// Geometry attached to a core entity (plot boundary today). Subject linkage is
/// an exclusive arc — exactly one of `plot_id`/`farm_id` is set (schema CHECK +
/// repository validation). Rows from different `source`s coexist; replacement
/// within one (subject, role, source) soft-deletes the previous row.
#[derive(Debug, Clone, Serialize)]
pub struct GeoFeature {
    pub id: String,
    pub plot_id: Option<String>,
    pub farm_id: Option<String>,
    pub role: String,
    /// GeoJSON geometry object (Polygon/MultiPolygon), EPSG:4326 lon/lat.
    pub geometry: String,
    pub source: String,
    pub campaign: Option<i64>,
    /// Provider-declared surface, stored for discrepancy display; never copied
    /// onto `plot.area_ha` (user input is never silently overwritten).
    pub official_area_ha: Option<f64>,
    /// Provider-specific attributes as JSON, interpreted per `source`.
    pub properties: Option<String>,
    pub fetched_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Spanish farm extension fields as form input (no `farm_id` — the repository
/// knows which row they belong to).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarmEsFields {
    pub rega_code: Option<String>,
    pub rea_code: Option<String>,
    pub province_code: Option<String>,
}

/// Spanish plot extension fields (SIGPAC reference) as form input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotEsFields {
    pub sigpac_province: Option<String>,
    pub sigpac_municipality: Option<String>,
    pub sigpac_aggregate: Option<String>,
    pub sigpac_zone: Option<String>,
    pub sigpac_polygon: Option<String>,
    pub sigpac_parcel: Option<String>,
    pub sigpac_enclosure: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewSeason {
    pub campaign_year: i64,
    pub label: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewCrop {
    pub plot_id: String,
    pub season_id: String,
    pub species_name: String,
    pub variety: Option<String>,
    pub production_system_code: Option<String>,
    pub sown_on: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewOperator {
    pub full_name: String,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewMachinery {
    pub farm_id: String,
    pub name: String,
    pub kind: Option<String>, // maps to column `type`
    pub last_inspection_date: Option<String>,
    pub next_inspection_due_date: Option<String>,
    /// Spanish registry numbers; an extension row is written when either is present.
    pub roma_number: Option<String>,
    pub reganip_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewFarm {
    pub name: String,
    pub owner_name: Option<String>,
    pub owner_tax_id: Option<String>,
    /// The farm's country (ISO 3166-1 alpha-2 code). Required: treatment records derive
    /// their country from here.
    pub country_code: String,
    /// Spanish regional fields; written to `farm_es_extension` when present.
    pub es: Option<FarmEsFields>,
}

#[derive(Debug, Deserialize)]
pub struct NewPlot {
    pub farm_id: String,
    pub name: String,
    pub area_ha: Option<f64>,
    /// SIGPAC reference; written to `plot_es_extension` when present.
    pub es: Option<PlotEsFields>,
}

/// Full-row update for a farm: the form submits the complete desired state.
/// `es: None` means "no extension" and removes an existing extension row.
#[derive(Debug, Deserialize)]
pub struct UpdateFarm {
    pub name: String,
    pub owner_name: Option<String>,
    pub owner_tax_id: Option<String>,
    pub location_text: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country_code: String,
    pub es: Option<FarmEsFields>,
}

/// Full-row update for a plot. `farm_id` is deliberately absent: a plot never
/// moves between farms (it would silently re-home historical treatment records).
#[derive(Debug, Deserialize)]
pub struct UpdatePlot {
    pub name: String,
    pub area_ha: Option<f64>,
    pub es: Option<PlotEsFields>,
}

/// One provider-checked zone intersection for a plot in a campaign. Unlike
/// alerts, flags cannot be re-derived offline, so they are user data
/// (audit-logged, synced, backed up). `status='outside'` rows are kept as
/// proof the check ran and was clear.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneFlag {
    pub id: String,
    pub plot_id: String,
    pub zone_type_code: String,
    pub campaign: i64,
    /// 'inside' | 'outside'.
    pub status: String,
    /// Provider's intersection percentage; `None` when outside.
    pub coverage_pct: Option<f64>,
    /// Provider detail (e.g. "Zona periférica"), shown verbatim.
    pub detail: Option<String>,
    pub source: String,
    pub checked_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// One zone result from a provider check, before storage fills identity,
/// campaign context and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewZoneFlag {
    pub zone_type_code: String,
    pub status: String,
    pub coverage_pct: Option<f64>,
    pub detail: Option<String>,
}

/// Input for saving a geometry. The repository fills `id` and timestamps and
/// replaces (soft-deletes) any active row with the same (subject, role, source).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewGeoFeature {
    pub plot_id: Option<String>,
    pub farm_id: Option<String>,
    pub role: String,
    pub geometry: String,
    pub source: String,
    pub campaign: Option<i64>,
    pub official_area_ha: Option<f64>,
    pub properties: Option<String>,
    pub fetched_at: Option<String>,
}

/// Full-row update for an operator: the form submits the complete desired state.
/// Past treatment records are unaffected — they snapshot the operator's name and
/// licence at write time.
#[derive(Debug, Deserialize)]
pub struct UpdateOperator {
    pub full_name: String,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
}

/// Full-row update for machinery. `farm_id` is deliberately absent, like
/// `UpdatePlot`: machinery never moves between farms. Both registry numbers
/// `None` means "no Spanish extension" and removes an existing extension row.
#[derive(Debug, Deserialize)]
pub struct UpdateMachinery {
    pub name: String,
    pub kind: Option<String>, // maps to column `type`
    pub last_inspection_date: Option<String>,
    pub next_inspection_due_date: Option<String>,
    pub roma_number: Option<String>,
    pub reganip_number: Option<String>,
}

/// Machinery with its Spanish extension — what the registry list and edit form
/// need in one round trip (mirrors `FarmDetail`/`PlotDetail`).
#[derive(Debug, Clone, Serialize)]
pub struct MachineryDetail {
    pub machinery: Machinery,
    pub es: Option<MachineryEsExtension>,
}
