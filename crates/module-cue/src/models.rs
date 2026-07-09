// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rust structs mirroring the schema.
//!
//! Domain structs derive `Serialize` so the repository can freeze a full row into the
//! `record_change.payload` JSON. `New*` structs are the insert inputs: they omit `id`,
//! timestamps and frozen snapshots, which the repository fills in (IDs via `Uuid::now_v7()`).

use serde::{Deserialize, Serialize};

// The farm-registry entities (land, calendar, people, machines) live in
// terrazgo-core since 2026-06-12; re-exported because CUE callers treat them
// as part of this module's data model.
pub use terrazgo_core::models::{
    Crop, Farm, Lookup, Machinery, MachineryEsExtension, NewCrop, NewFarm, NewMachinery,
    NewOperator, NewPlot, NewSeason, Operator, Plot, Season,
};

// ---------------------------------------------------------------------------
// Domain structs (returned by the repository)
// ---------------------------------------------------------------------------

/// Synced user-data row (UUIDv7 PK since 2026-07-02): installations may
/// register substances the app doesn't ship, so ids must be collision-free
/// across devices. `cas_number` is the natural cross-device key.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveSubstance {
    pub id: String,
    pub name: String,
    pub cas_number: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Product {
    pub id: String,
    pub commercial_name: String,
    pub holder: Option<String>,
    pub formulation_type_code: Option<String>,
    pub default_phi_days: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Junction row product ↔ active substance. Has its own UUID PK so
/// `record_change` can address it by `entity_id`.
#[derive(Debug, Clone, Serialize)]
pub struct ProductActiveSubstance {
    pub id: String,
    pub product_id: String,
    pub active_substance_id: String,
    pub concentration_value: Option<f64>,
    pub concentration_unit_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductAuthorisation {
    pub id: String,
    pub product_id: String,
    pub country_code: String,
    pub authorisation_number: String,
    pub status: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreatmentRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    pub application_date: String,
    pub product_id: String,
    pub country_code: String,
    pub dose_value: f64,
    pub dose_unit_code: String,
    pub reason_category_code: String,
    pub target_organism: Option<String>,
    pub operator_id: String,
    pub machinery_id: Option<String>,
    pub phi_days_used: i64,
    pub phi_end_date: String,
    pub product_name_snapshot: String,
    pub authorisation_number_snapshot: Option<String>,
    pub active_substances_snapshot: Option<String>,
    pub operator_name_snapshot: String,
    pub operator_licence_snapshot: Option<String>,
    pub machinery_roma_snapshot: Option<String>,
    pub machinery_reganip_snapshot: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreatmentPlot {
    pub id: String,
    pub treatment_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub surface_treated_ha: f64,
    pub crop_name_snapshot: Option<String>,
    pub variety_snapshot: Option<String>,
}

/// A treatment record together with every plot it was applied to.
#[derive(Debug, Clone, Serialize)]
pub struct TreatmentRecordWithPlots {
    pub record: TreatmentRecord,
    pub plots: Vec<TreatmentPlot>,
}

/// Derived alert row, owned by `repository::refresh_alerts` (reconciliation). Serialize
/// is for the future Tauri commands, not for `record_change` — derived state is never
/// audit-logged or synced. There is no `NewAlert`: users acknowledge or dismiss alerts,
/// they never create them.
#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub id: String,
    pub alert_type_code: String,
    pub season_id: Option<String>,
    pub subject_table: String,
    pub subject_id: String,
    pub due_date: Option<String>,
    pub lead_days_used: Option<i64>,
    pub status: String,
    pub acknowledged_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Insert inputs
// ---------------------------------------------------------------------------

/// Deserialize: arrives as JSON through the `create_product` Tauri command.
#[derive(Debug, Deserialize)]
pub struct NewProduct {
    pub commercial_name: String,
    pub holder: Option<String>,
    pub formulation_type_code: Option<String>,
    pub default_phi_days: Option<i64>,
}

pub struct NewProductAuthorisation {
    pub product_id: String,
    pub country_code: String,
    pub authorisation_number: String,
    pub status: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
}

/// Authorisation fields without `product_id` — the form input when the product
/// row is being created (or extended) in the same call.
#[derive(Debug, Deserialize)]
pub struct ProductAuthorisationFields {
    pub country_code: String,
    pub authorisation_number: String,
    pub status: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
}

/// Full-row update for a product: the form submits the complete desired state.
/// Past treatment records are unaffected — they snapshot the product's name,
/// authorisation number, substances and the PHI days actually used.
#[derive(Debug, Deserialize)]
pub struct UpdateProduct {
    pub commercial_name: String,
    pub holder: Option<String>,
    pub formulation_type_code: Option<String>,
    pub default_phi_days: Option<i64>,
}

/// One product ↔ substance link joined with the substance itself, flattened for
/// display: `id` is the junction row's (what remove takes), the rest is what
/// the product card shows.
#[derive(Debug, Clone, Serialize)]
pub struct ProductSubstance {
    pub id: String,
    pub active_substance_id: String,
    pub name: String,
    pub cas_number: Option<String>,
    pub concentration_value: Option<f64>,
    pub concentration_unit_code: Option<String>,
}

/// A product with its substances and per-country authorisations — what the
/// registry list and edit form need in one round trip.
#[derive(Debug, Clone, Serialize)]
pub struct ProductDetail {
    pub product: Product,
    pub substances: Vec<ProductSubstance>,
    pub authorisations: Vec<ProductAuthorisation>,
}

/// Deserialize: this input (and `NewTreatmentPlot`) arrives as JSON through the
/// `create_treatment_record` Tauri command, like the core `New*` structs.
#[derive(Debug, Deserialize)]
pub struct NewTreatmentRecord {
    pub season_id: String,
    /// The farm this record belongs to; its country drives `country_code`.
    pub farm_id: String,
    pub application_date: String,
    pub product_id: String,
    /// Optional. When `None`, the country is derived from the farm. When `Some`, it must
    /// match the farm's country or the insert fails with `CountryMismatch`.
    pub country_code: Option<String>,
    pub dose_value: f64,
    pub dose_unit_code: String,
    pub reason_category_code: String,
    pub target_organism: Option<String>,
    pub operator_id: String,
    pub machinery_id: Option<String>,
    /// PHI days actually used; falls back to `product.default_phi_days` when `None`.
    pub phi_days_used: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewTreatmentPlot {
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub surface_treated_ha: f64,
}
