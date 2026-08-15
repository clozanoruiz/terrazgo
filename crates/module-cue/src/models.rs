// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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
    /// Nature of the authorisation ('registered' by default); 'exceptional'
    /// (Art. 53 emergency) additionally names its substance by catalogue code.
    pub kind_code: String,
    pub exceptional_substance_code: Option<String>,
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
    /// Last day of the actuation when it ran over several (Anexo III Parte I B
    /// allows an interval). `None` = a single-day treatment.
    pub application_end_date: Option<String>,
    /// Start hour as local wall-clock `HH:MM`, asked for by Reglamento (UE)
    /// 2023/564's annex where the hour is relevant. `None` = not stated.
    pub application_time: Option<String>,
    /// The chemical half of the actuation, present or absent as a BLOCK
    /// (`product_id`, `dose_value`, `dose_unit_code`, `phi_days_used`,
    /// `phi_end_date`, `product_name_snapshot` are all `Some` or all `None`,
    /// enforced by a table CHECK). `None` = a purely non-chemical
    /// intervention, which RD 1311/2012 art. 10.1 asks farmers to prefer and
    /// the SIEX twin models by not requiring `ProductosFito`.
    pub product_id: Option<String>,
    pub country_code: String,
    pub dose_value: Option<f64>,
    pub dose_unit_code: Option<String>,
    /// Total product used over the whole actuation (Anexo III Parte I B.i),
    /// value and unit apart. `None` = not stated; it is deliberately not
    /// derived, because a concentration dose cannot yield a total.
    pub total_quantity_value: Option<f64>,
    pub total_quantity_unit_code: Option<String>,
    /// Free-text nuance the coded problem lists cannot express; the reason for
    /// treatment itself lives in the `treatment_problem` junction rows.
    pub target_organism: Option<String>,
    /// Observed efficacy, assessed after application — `None` until the farmer
    /// records it (`set_treatment_efficacy`); the export precheck demands it.
    pub efficacy_code: Option<String>,
    pub operator_id: String,
    pub machinery_id: Option<String>,
    /// The advisor identified on this actuation (Anexo III Parte I B.d, "y, en
    /// su caso, del asesor"). `None` for the ordinary unadvised treatment; the
    /// snapshots freeze what a past record printed.
    pub advisor_id: Option<String>,
    pub advisor_name_snapshot: Option<String>,
    pub advisor_registration_snapshot: Option<String>,
    /// The non-chemical measure (`TIPO_MEDIDA_FITOSANITARIA` code verbatim),
    /// its intensity as value + unit, and the measure's own registration
    /// number — the model's "Alternativas no químicas de intervención".
    pub measure_code: Option<String>,
    pub measure_intensity_value: Option<f64>,
    pub measure_intensity_unit_code: Option<String>,
    pub measure_registration_number: Option<String>,
    pub phi_days_used: Option<i64>,
    pub phi_end_date: Option<String>,
    pub product_name_snapshot: Option<String>,
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
    /// `EST_FENOLOGICO` code for the crop's growth stage at treatment time
    /// (Reglamento (UE) 2023/564's annex, per treated crop). `None` = not
    /// stated. Resolve for display with
    /// [`crate::catalogue::growth_stage_label`] — the code is not the BBCH
    /// stage.
    pub growth_stage_code: Option<String>,
}

/// One coded phytosanitary problem a treatment targets. The category picks the
/// catalogue the code resolves against (per the record's country) and the
/// export bucket; `problem_code` is the catalogue code verbatim (no FK — the
/// code is the regulatory payload, the catalogue row is display metadata).
#[derive(Debug, Clone, Serialize)]
pub struct TreatmentProblem {
    pub id: String,
    pub treatment_record_id: String,
    pub reason_category_code: String,
    pub problem_code: String,
}

/// One IPM justification behind a treatment (Directive 2009/128/CE).
#[derive(Debug, Clone, Serialize)]
pub struct TreatmentJustification {
    pub id: String,
    pub treatment_record_id: String,
    pub justification_code: String,
}

/// A treatment record together with its detail rows: treated plots, coded
/// problems and justifications — what the record-book list and form need.
#[derive(Debug, Clone, Serialize)]
pub struct TreatmentRecordWithPlots {
    pub record: TreatmentRecord,
    pub plots: Vec<TreatmentPlot>,
    pub problems: Vec<TreatmentProblem>,
    pub justifications: Vec<TreatmentJustification>,
}

/// Per-plot PHI standing, derived on read for the map overlay: whether any
/// active treatment's PHI window contains today, and until when. Never
/// stored — recomputing from the records each time means it cannot drift.
#[derive(Debug, Clone, Serialize)]
pub struct PlotPhiStatus {
    pub plot_id: String,
    pub in_phi: bool,
    /// Latest `phi_end_date` among the windows containing today — the first
    /// day harvest is allowed again. `None` whenever `in_phi` is false.
    pub phi_until: Option<String>,
}

/// Integer alias a regulatory export assigns to an activity record the first
/// time it is exported (SIEX's `IdAjena*` keys are integers, our ids UUIDs).
/// Never updated, never deleted: the alias is the edit/delete key on the
/// authority's side, and the row's existence marks the record as previously
/// exported. `split_key` discriminates when one record maps to several export
/// entries (a multi-crop treatment splits into one `TratamFito` per crop);
/// its value is serializer-defined, opaque here ('' for a 1:1 record).
#[derive(Debug, Clone, Serialize)]
pub struct ExportAlias {
    pub id: String,
    pub target: String,
    pub entity_table: String,
    pub entity_id: String,
    pub split_key: String,
    pub alias: i64,
    pub created_at: String,
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
    /// Defaults to 'registered' — the overwhelmingly common case.
    pub kind_code: Option<String>,
    /// Required (and only meaningful) when the kind is 'exceptional'.
    pub exceptional_substance_code: Option<String>,
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
    /// Defaults to 'registered' — the overwhelmingly common case.
    pub kind_code: Option<String>,
    /// Required (and only meaningful) when the kind is 'exceptional'.
    pub exceptional_substance_code: Option<String>,
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
    /// Last day, when the actuation spanned several. Must not precede
    /// `application_date`; the plazo de seguridad is counted from it.
    #[serde(default)]
    pub application_end_date: Option<String>,
    /// Start hour as local wall-clock `HH:MM` (Reglamento (UE) 2023/564's
    /// annex). Optional — the annex asks for it only where the hour is
    /// relevant — but a stated one must be well formed
    /// (`invalid.application_time`).
    #[serde(default)]
    pub application_time: Option<String>,
    /// The chemical half. All three travel together, and all three may be
    /// absent — a purely non-chemical actuation states a `measure_code`
    /// instead. An actuation that states neither is rejected
    /// (`invalid.treatment_without_actuation`).
    #[serde(default)]
    pub product_id: Option<String>,
    /// Optional. When `None`, the country is derived from the farm. When `Some`, it must
    /// match the farm's country or the insert fails with `CountryMismatch`.
    pub country_code: Option<String>,
    #[serde(default)]
    pub dose_value: Option<f64>,
    #[serde(default)]
    pub dose_unit_code: Option<String>,
    /// Total product used (Anexo III B.i). Both parts travel together: a value
    /// without its unit, or a unit without a value, is rejected.
    #[serde(default)]
    pub total_quantity_value: Option<f64>,
    #[serde(default)]
    pub total_quantity_unit_code: Option<String>,
    pub target_organism: Option<String>,
    /// The coded problems treated (≥1 required — they ARE the reason for
    /// treatment) and the IPM justifications (≥1 required, known at treatment
    /// time). Efficacy is optional here: it is observed after application.
    pub problems: Vec<NewTreatmentProblem>,
    pub justifications: Vec<String>,
    pub efficacy_code: Option<String>,
    pub operator_id: String,
    pub machinery_id: Option<String>,
    /// The advisor who directed this actuation (Anexo III B.d). Optional: most
    /// treatments are not advised, and the model's 3.1 bis page is explicitly
    /// "solamente para cultivos y superficies objeto de asesoramiento".
    #[serde(default)]
    pub advisor_id: Option<String>,
    /// The non-chemical measure taken (a `TIPO_MEDIDA_FITOSANITARIA` code) and
    /// how much of it. The intensity parts travel together like every other
    /// value + unit pair in the book.
    #[serde(default)]
    pub measure_code: Option<String>,
    #[serde(default)]
    pub measure_intensity_value: Option<f64>,
    #[serde(default)]
    pub measure_intensity_unit_code: Option<String>,
    #[serde(default)]
    pub measure_registration_number: Option<String>,
    /// PHI days actually used; falls back to `product.default_phi_days` when `None`.
    /// Ignored when there is no product — a measure has no plazo de seguridad.
    pub phi_days_used: Option<i64>,
    pub notes: Option<String>,
}

/// Full-row correction of a treatment record.
///
/// Nothing in the sources forbids correcting an entry: RD 1311/2012 art. 16 has
/// no provision on modifying one, Reglamento (UE) 2023/564 none on integrity or
/// change logs, and the SIEX exchange models a correction as re-sending the
/// same `IdAjenaTratamFito` with new values — reserving its `Borrar` flag for
/// withdrawal. Delete-and-re-create is the less faithful model: it states a
/// typo fix as a withdrawal plus a second event.
///
/// `season_id`, `farm_id` and `country_code` are deliberately absent — a
/// treatment never moves campaign or holding (the `UpdateCrop` precedent) — and
/// so is `efficacy_code`, which keeps its own audit-logged setter because it is
/// observed after the fact rather than submitted with the form.
///
/// Plots, problems and justifications are reconciled from the submitted state,
/// which is also how the exchange format sees them: its child arrays carry no
/// ids and no delete flags, so a correction restates them whole.
#[derive(Debug, Deserialize)]
pub struct UpdateTreatmentRecord {
    pub application_date: String,
    #[serde(default)]
    pub application_end_date: Option<String>,
    #[serde(default)]
    pub application_time: Option<String>,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub dose_value: Option<f64>,
    #[serde(default)]
    pub dose_unit_code: Option<String>,
    #[serde(default)]
    pub total_quantity_value: Option<f64>,
    #[serde(default)]
    pub total_quantity_unit_code: Option<String>,
    #[serde(default)]
    pub target_organism: Option<String>,
    pub problems: Vec<NewTreatmentProblem>,
    pub justifications: Vec<String>,
    pub operator_id: String,
    #[serde(default)]
    pub machinery_id: Option<String>,
    #[serde(default)]
    pub advisor_id: Option<String>,
    #[serde(default)]
    pub measure_code: Option<String>,
    #[serde(default)]
    pub measure_intensity_value: Option<f64>,
    #[serde(default)]
    pub measure_intensity_unit_code: Option<String>,
    #[serde(default)]
    pub measure_registration_number: Option<String>,
    #[serde(default)]
    pub phi_days_used: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewTreatmentPlot>,
}

/// One coded problem as form input (the repository fills ids).
#[derive(Debug, Deserialize)]
pub struct NewTreatmentProblem {
    pub reason_category_code: String,
    pub problem_code: String,
}

#[derive(Debug, Deserialize)]
pub struct NewTreatmentPlot {
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub surface_treated_ha: f64,
    /// `EST_FENOLOGICO` code for this crop's growth stage (Reglamento (UE)
    /// 2023/564's annex). Optional, and validated against the catalogue when
    /// stated (`invalid.growth_stage_unknown`) — the monograph's ten principal
    /// stages are a closed list.
    #[serde(default)]
    pub growth_stage_code: Option<String>,
}

// ---------------------------------------------------------------------------
// Non-field treatments (model sections 3.3 / 3.4 / 3.5) and the registers'
// stored "APLICA TRATAMIENTO: NO"
// ---------------------------------------------------------------------------

/// A treatment applied to something other than a growing crop: harvested
/// produce (3.3), storage premises (3.4) or a means of transport (3.5). One
/// struct for all three — they differ only in what the subject is.
#[derive(Debug, Clone, Serialize)]
pub struct NonFieldTreatment {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    pub country_code: String,
    /// `postharvest` | `storage_premises` | `transport`.
    pub subject_kind_code: String,
    pub treated_on: String,
    /// What was treated, in each section's own terms: the plant product, the
    /// premises' type and address, or the vehicle's type, model and plate.
    pub subject_description: String,
    /// 3.3 only: the PRODUCTOS catalogue code of the plant product treated,
    /// verbatim and unconstrained by a foreign key.
    pub subject_product_code: Option<String>,
    /// How much of the subject was treated: tonnes for produce, cubic metres
    /// for premises and vehicles. `None` prints blank.
    pub treated_quantity_value: Option<f64>,
    pub treated_quantity_unit_code: Option<String>,
    pub product_id: String,
    /// Product actually used, in kilograms or litres.
    pub product_quantity_value: Option<f64>,
    pub product_quantity_unit_code: Option<String>,
    pub operator_id: String,
    pub machinery_id: Option<String>,
    /// The advisor identified on this actuation. Anexo III Parte I B names
    /// premises and vehicles in its own list (B.b, B.f), so B.d's "y, en su
    /// caso, del asesor" binds here exactly as it does on a field treatment.
    pub advisor_id: Option<String>,
    pub advisor_name_snapshot: Option<String>,
    pub advisor_registration_snapshot: Option<String>,
    /// Observed after the fact — `None` until `set_non_field_efficacy` records
    /// it, exactly like a field treatment's.
    pub efficacy_code: Option<String>,
    pub product_name_snapshot: String,
    pub authorisation_number_snapshot: Option<String>,
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
pub struct NonFieldTreatmentProblem {
    pub id: String,
    pub non_field_treatment_id: String,
    pub reason_category_code: String,
    pub problem_code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NonFieldTreatmentJustification {
    pub id: String,
    pub non_field_treatment_id: String,
    pub justification_code: String,
}

/// A non-field treatment with its junction rows — what the register list and
/// the printed sections need.
#[derive(Debug, Clone, Serialize)]
pub struct NonFieldTreatmentDetail {
    pub record: NonFieldTreatment,
    pub problems: Vec<NonFieldTreatmentProblem>,
    pub justifications: Vec<NonFieldTreatmentJustification>,
}

/// Insert input, arriving as JSON through the Tauri command.
#[derive(Debug, Deserialize)]
pub struct NewNonFieldTreatment {
    pub season_id: String,
    pub farm_id: String,
    /// Optional; when present it must match the farm's country, like a field
    /// treatment's.
    #[serde(default)]
    pub country_code: Option<String>,
    pub subject_kind_code: String,
    pub treated_on: String,
    pub subject_description: String,
    #[serde(default)]
    pub subject_product_code: Option<String>,
    #[serde(default)]
    pub treated_quantity_value: Option<f64>,
    #[serde(default)]
    pub treated_quantity_unit_code: Option<String>,
    pub product_id: String,
    #[serde(default)]
    pub product_quantity_value: Option<f64>,
    #[serde(default)]
    pub product_quantity_unit_code: Option<String>,
    pub operator_id: String,
    #[serde(default)]
    pub machinery_id: Option<String>,
    /// Optional, like a field treatment's: most treatments are not advised.
    #[serde(default)]
    pub advisor_id: Option<String>,
    /// ≥1 of each, the same rule field treatments follow: both are known when
    /// treating, unlike efficacy.
    pub problems: Vec<NewTreatmentProblem>,
    pub justifications: Vec<String>,
    #[serde(default)]
    pub efficacy_code: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Full-row correction of a non-field treatment, on the same terms as
/// [`UpdateTreatmentRecord`].
///
/// `subject_kind_code` is absent as well as the campaign and holding: the kind
/// decides which of the three registers the record prints in, and moving it
/// would silently empty one register and fill another — and interact with the
/// stored "APLICA TRATAMIENTO: NO" of both. Correcting the kind is a delete and
/// a re-entry.
#[derive(Debug, Deserialize)]
pub struct UpdateNonFieldTreatment {
    pub treated_on: String,
    pub subject_description: String,
    #[serde(default)]
    pub subject_product_code: Option<String>,
    #[serde(default)]
    pub treated_quantity_value: Option<f64>,
    #[serde(default)]
    pub treated_quantity_unit_code: Option<String>,
    pub product_id: String,
    #[serde(default)]
    pub product_quantity_value: Option<f64>,
    #[serde(default)]
    pub product_quantity_unit_code: Option<String>,
    pub operator_id: String,
    #[serde(default)]
    pub machinery_id: Option<String>,
    #[serde(default)]
    pub advisor_id: Option<String>,
    pub problems: Vec<NewTreatmentProblem>,
    pub justifications: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// The stored "APLICA TRATAMIENTO: NO" for one register of one campaign. Its
/// existence is the statement; there is nothing else to record.
#[derive(Debug, Clone, Serialize)]
pub struct RegisterDeclaration {
    pub id: String,
    pub farm_id: String,
    pub season_id: String,
    /// `seed_treatment` | `postharvest` | `storage_premises` | `transport`.
    pub register_code: String,
    pub declared_on: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Treated seed (model section 3.2)
// ---------------------------------------------------------------------------

/// A sowing made with seed the supplier had already treated. What is recorded
/// is the sowing, not an application: the product block is free capture because
/// the label on the sack names a product the farmer never bought as such.
#[derive(Debug, Clone, Serialize)]
pub struct SeedTreatment {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    pub sown_on: String,
    pub species_name: String,
    pub variety: Option<String>,
    /// PRODUCTOS catalogue code, verbatim and without a foreign key.
    pub crop_code: Option<String>,
    pub seed_quantity_kg: Option<f64>,
    /// The lot printed on the sack — what makes the record traceable.
    pub seed_lot: Option<String>,
    /// Where the seed was treated (FEGA TIPO_TRATAMIENTO). `None` because the
    /// printed model has no such column: a book kept to the model alone cannot
    /// be made to answer it.
    pub treatment_kind_code: Option<String>,
    pub product_name: String,
    pub product_registration_number: Option<String>,
    pub product_active_substance: Option<String>,
    /// Set only when the treated seed's product is also in the farmer's own
    /// registry; the free-text fields stay the printed truth regardless.
    pub product_id: Option<String>,
    /// Observed after emergence — `None` until `set_seed_treatment_efficacy`.
    pub efficacy_code: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeedTreatmentPlot {
    pub id: String,
    pub seed_treatment_id: String,
    pub plot_id: String,
    pub surface_sown_ha: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeedTreatmentDetail {
    pub record: SeedTreatment,
    pub plots: Vec<SeedTreatmentPlot>,
}

#[derive(Debug, Deserialize)]
pub struct NewSeedTreatment {
    pub season_id: String,
    pub farm_id: String,
    pub sown_on: String,
    pub species_name: String,
    #[serde(default)]
    pub variety: Option<String>,
    #[serde(default)]
    pub crop_code: Option<String>,
    #[serde(default)]
    pub seed_quantity_kg: Option<f64>,
    #[serde(default)]
    pub seed_lot: Option<String>,
    #[serde(default)]
    pub treatment_kind_code: Option<String>,
    pub product_name: String,
    #[serde(default)]
    pub product_registration_number: Option<String>,
    #[serde(default)]
    pub product_active_substance: Option<String>,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub efficacy_code: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Where the seed went. At least one, and each plot must be on the farm.
    pub plots: Vec<NewSeedTreatmentPlot>,
}

#[derive(Debug, Deserialize)]
pub struct NewSeedTreatmentPlot {
    pub plot_id: String,
    pub surface_sown_ha: f64,
}

/// Full-row update. `season_id` and `farm_id` are deliberately absent: a sowing
/// never moves campaign or holding — correcting that means delete and re-enter,
/// the `UpdateCrop` precedent. The sown plots are reconciled from the submitted
/// state, like an extension table.
#[derive(Debug, Deserialize)]
pub struct UpdateSeedTreatment {
    pub sown_on: String,
    pub species_name: String,
    #[serde(default)]
    pub variety: Option<String>,
    #[serde(default)]
    pub crop_code: Option<String>,
    #[serde(default)]
    pub seed_quantity_kg: Option<f64>,
    #[serde(default)]
    pub seed_lot: Option<String>,
    #[serde(default)]
    pub treatment_kind_code: Option<String>,
    pub product_name: String,
    #[serde(default)]
    pub product_registration_number: Option<String>,
    #[serde(default)]
    pub product_active_substance: Option<String>,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewSeedTreatmentPlot>,
}

// ---------------------------------------------------------------------------
// Analyses (model section 4)
// ---------------------------------------------------------------------------

/// Anexo III Parte I A.3 — the soil block of a laboratory bulletin.
///
/// Every figure optional: A.3's minimums bind only one year after MAPA
/// publishes its sampling and analysis guides, and a bulletin reports whatever
/// was asked for. Units are fixed by the field name (the twin states none),
/// which is safe because a farmer reads these off a bulletin into a labelled
/// field rather than the app importing them from anywhere.
///
/// SIEX twin: `Analitica.ParametrosSuelo`, whose nine members these are.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoilParameters {
    #[serde(default)]
    pub ph: Option<f64>,
    #[serde(default)]
    pub organic_matter_pct: Option<f64>,
    /// Phosphorus and potassium as the lab reports them available to the crop.
    #[serde(default)]
    pub available_p_mg_kg: Option<f64>,
    #[serde(default)]
    pub available_k_mg_kg: Option<f64>,
    #[serde(default)]
    pub total_n_pct: Option<f64>,
    /// Electrical conductivity at 25 °C.
    #[serde(default)]
    pub conductivity_ds_m: Option<f64>,
    /// Texture, as the twin carries it: three fractions of one whole, not a
    /// class name. Checked to sum to 100 when all three are given.
    #[serde(default)]
    pub sand_pct: Option<f64>,
    #[serde(default)]
    pub silt_pct: Option<f64>,
    #[serde(default)]
    pub clay_pct: Option<f64>,
}

impl SoilParameters {
    /// Whether the bulletin reported any soil figure at all — what the book
    /// asks before printing a soil cell.
    pub fn is_empty(&self) -> bool {
        [
            self.ph,
            self.organic_matter_pct,
            self.available_p_mg_kg,
            self.available_k_mg_kg,
            self.total_n_pct,
            self.conductivity_ds_m,
            self.sand_pct,
            self.silt_pct,
            self.clay_pct,
        ]
        .iter()
        .all(Option::is_none)
    }
}

/// A laboratory analysis of plant material, soil or water. Metadata only: what
/// was analysed, by whom, and under which bulletin number the result can be
/// found — the bulletin itself stays in the farmer's folder, which art. 16.3
/// obliges keeping.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    pub sampled_on: String,
    /// `crop` | `harvested_produce` | `soil` | `water` — FEGA's four, which
    /// separate the standing crop from the produce taken off it.
    pub material_kind_code: String,
    pub bulletin_number: Option<String>,
    pub lab_name: Option<String>,
    /// The printed model asks for the laboratory's address; the SIEX twin
    /// carries only a name and a NIF.
    pub lab_address: Option<String>,
    pub lab_tax_id: Option<String>,
    /// Free text, kept beside the coded substances: SUST_ACTIVAS only codes
    /// phytosanitary actives, so a metals or nutrients bulletin has nothing to
    /// code there and would otherwise be unrecordable.
    pub substances_detected: Option<String>,
    /// Anexo III A.3, when the bulletin carried soil figures.
    #[serde(default)]
    pub soil: SoilParameters,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Which parcel was sampled, with the crop frozen as it stood.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisPlot {
    pub id: String,
    pub analysis_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub crop_name_snapshot: Option<String>,
    pub variety_snapshot: Option<String>,
}

/// What the laboratory looked for — the twin's `TiposAnalisis[]`.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisRecordType {
    pub id: String,
    pub analysis_record_id: String,
    pub analysis_type_code: String,
}

/// One active substance the analysis reported, as a FEGA SUST_ACTIVAS code
/// stored verbatim. A code the vendored snapshot cannot resolve is kept, not
/// refused: the snapshot travels with app releases and a laboratory does not
/// wait for one.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisSubstance {
    pub id: String,
    pub analysis_record_id: String,
    pub substance_code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisRecordDetail {
    pub record: AnalysisRecord,
    pub plots: Vec<AnalysisPlot>,
    pub types: Vec<AnalysisRecordType>,
    pub substances: Vec<AnalysisSubstance>,
}

#[derive(Debug, Deserialize)]
pub struct NewAnalysisRecord {
    pub season_id: String,
    pub farm_id: String,
    pub sampled_on: String,
    pub material_kind_code: String,
    #[serde(default)]
    pub bulletin_number: Option<String>,
    #[serde(default)]
    pub lab_name: Option<String>,
    #[serde(default)]
    pub lab_address: Option<String>,
    #[serde(default)]
    pub lab_tax_id: Option<String>,
    #[serde(default)]
    pub substances_detected: Option<String>,
    /// Anexo III A.3's soil block; every figure optional.
    #[serde(default)]
    pub soil: SoilParameters,
    #[serde(default)]
    pub notes: Option<String>,
    /// What was sampled. At least one, and each plot must be on the farm.
    pub plots: Vec<NewAnalysisPlot>,
    /// What the laboratory looked for. May be empty — the printed model has no
    /// such column, so a book kept to the model alone cannot answer it.
    #[serde(default)]
    pub analysis_type_codes: Vec<String>,
    /// SUST_ACTIVAS codes, accepted whether or not the vendored snapshot knows
    /// them.
    #[serde(default)]
    pub substance_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewAnalysisPlot {
    pub plot_id: String,
    #[serde(default)]
    pub crop_id: Option<String>,
}

/// Full-row update. `season_id` and `farm_id` are deliberately absent: an
/// analysis never moves campaign or holding — correcting that means delete and
/// re-enter, the `UpdateCrop` precedent. The sampled plots are reconciled from
/// the submitted state, like an extension table.
#[derive(Debug, Deserialize)]
pub struct UpdateAnalysisRecord {
    pub sampled_on: String,
    pub material_kind_code: String,
    #[serde(default)]
    pub bulletin_number: Option<String>,
    #[serde(default)]
    pub lab_name: Option<String>,
    #[serde(default)]
    pub lab_address: Option<String>,
    #[serde(default)]
    pub lab_tax_id: Option<String>,
    #[serde(default)]
    pub substances_detected: Option<String>,
    /// Anexo III A.3's soil block; every figure optional.
    #[serde(default)]
    pub soil: SoilParameters,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewAnalysisPlot>,
    #[serde(default)]
    pub analysis_type_codes: Vec<String>,
    #[serde(default)]
    pub substance_codes: Vec<String>,
}
