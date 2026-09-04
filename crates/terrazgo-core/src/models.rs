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
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Crop {
    pub id: String,
    pub plot_id: String,
    pub season_id: String,
    pub species_name: String,
    pub variety: Option<String>,
    pub production_system_code: Option<String>,
    /// Surface this crop occupies on the plot (model 2.1 "Superficie
    /// cultivada"). `None` prints blank — never assume it fills the plot.
    pub area_ha: Option<f64>,
    pub irrigation_code: Option<String>,
    pub growing_environment_code: Option<String>,
    /// GIP framework for this crop (model 2.1's per-row column). `None` lets
    /// the report fall back to what `production_system_code` implies.
    pub gip_system_code: Option<String>,
    /// FEGA PRODUCTOS catalogue code for the species, stored verbatim.
    /// `None` = free-text species with no catalogue match.
    pub crop_code: Option<String>,
    /// `"user"` (typed by hand) or `"sigpac"` (from a PAC declaration import).
    pub source: String,
    /// Campaign of the declaration this row came from, when imported.
    pub source_campaign: Option<i64>,
    /// Surface the declaration stated, beside — never instead of — `area_ha`.
    pub declared_area_ha: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Operator {
    pub id: String,
    pub full_name: String,
    /// Tax/identity number (model 1.2 NIF column, Anexo III A.1.c).
    pub tax_id: Option<String>,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// App user profile — who is using the app. Identification, not security
/// (no credentials); the active profile per device lives in settings.json.
#[derive(Debug, Clone, Serialize)]
pub struct UserProfile {
    pub id: String,
    pub display_name: String,
    /// Optional "this user is this applicator" link to an operator row.
    pub operator_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// The advisor, advisory group or entity a holding is attached to (official
/// model 1.4). A capacity recorded in the book — not an app user, and not a
/// carné level an applicator holds.
#[derive(Debug, Clone, Serialize)]
pub struct Advisor {
    pub id: String,
    /// Person's name or razón social.
    pub name: String,
    pub tax_id: Option<String>,
    /// The model's "Nº de identificación" (the ROPO advisor inscription in
    /// Spain); named generically because core carries no regional identifiers.
    pub registration_number: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Farm ↔ advisor, carrying the GIP framework the holding operates under
/// (model 1.4's "Tipo de explotación").
#[derive(Debug, Clone, Serialize)]
pub struct FarmAdvisor {
    pub id: String,
    pub farm_id: String,
    pub advisor_id: String,
    pub gip_system_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A farm's advisory link with the advisor it points at — what table 1.4 and
/// the farm's advisory panel need in one round trip.
#[derive(Debug, Clone, Serialize)]
pub struct FarmAdvisorDetail {
    pub link: FarmAdvisor,
    pub advisor: Advisor,
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
    /// Anexo III A.1.h accepts the acquisition date OR the last inspection —
    /// equipment needing no ITV must still be datable in the book.
    pub acquired_on: Option<String>,
    pub last_inspection_date: Option<String>,
    pub next_inspection_due_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A place or vehicle on the holding that a treatment can be applied to:
/// model 3.4's "local tratado" and model 3.5's "vehículo tratado".
///
/// A registry rather than free text because RD 1311/2012 Anexo III Parte I B.b
/// requires the local or vehicle to be IDENTIFIED, and a description retyped
/// per record identifies nothing (docs/data-model.md → the premises registry).
#[derive(Debug, Clone, Serialize)]
pub struct Premises {
    pub id: String,
    pub farm_id: String,
    /// `building` | `vehicle`.
    pub kind_code: String,
    /// The farmer's own name for it, which is also what prints as the model's
    /// "tipo".
    pub name: String,
    /// Buildings only (model 3.4's "dirección").
    pub address: Option<String>,
    /// Vehicles only (model 3.5's "modelo" and "matrícula").
    pub vehicle_model: Option<String>,
    pub plate: Option<String>,
    /// FEGA's `EDIFICACIONES_INSTALACIONES` code, verbatim and unconstrained by
    /// a foreign key. Never composed into a record's printed subject cell.
    ///
    /// A catalogue code and therefore country-neutral by construction (the
    /// `crop.crop_code` precedent — which catalogue it speaks is configuration).
    /// The Spanish REGISTRY identifiers live in `premises_es_extension`.
    pub class_code: Option<String>,
    /// Capacity. The volume actually TREATED is B.f's, and stays on the record.
    pub volume_m3: Option<f64>,
    pub notes: Option<String>,
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
    /// Postal contact of the holding (official model 1.1). Universal, so core.
    pub address: Option<String>,
    pub postal_code: Option<String>,
    pub phone_fixed: Option<String>,
    pub phone_mobile: Option<String>,
    pub email: Option<String>,
    /// "Fecha de apertura del cuaderno" (model 1.1), `YYYY-MM-DD`. The book is
    /// a continuing document for the holding; `None` prints the model's blank
    /// rule rather than a date nobody stated.
    pub opened_on: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country_code: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Who signs the book when that is not the holder (model 1.1 "TITULAR O
/// REPRESENTANTE"). Logged to `record_change` as its own entity, like
/// `FarmEsExtension`. A legal capacity in a document — not a `user_profile`.
#[derive(Debug, Clone, Serialize)]
pub struct FarmRepresentative {
    pub farm_id: String,
    pub full_name: String,
    pub tax_id: Option<String>,
    pub representation_kind: Option<String>,
    pub address: Option<String>,
    pub locality: Option<String>,
    /// One line of a postal address, free text — not the coded geography
    /// `FarmEsExtension::province_code` carries for the holding itself.
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Representative fields as form input; `None` on the farm means "no
/// representative" and removes any existing row (the `FarmEsFields` contract).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarmRepresentativeFields {
    pub full_name: String,
    pub tax_id: Option<String>,
    pub representation_kind: Option<String>,
    pub address: Option<String>,
    pub locality: Option<String>,
    #[serde(default)]
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
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
    /// National registry number (model 1.1 "Nº Registro de Explotaciones
    /// Nacional"), printed beside the autonómico `rea_code`.
    pub siex_code: Option<String>,
    pub province_code: Option<String>,
}

/// A farm with its regional extension and representative — what the edit form
/// needs in one round trip.
#[derive(Debug, Clone, Serialize)]
pub struct FarmDetail {
    pub farm: Farm,
    pub es: Option<FarmEsExtension>,
    pub representative: Option<FarmRepresentative>,
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
    pub siex_code: Option<String>,
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
    pub area_ha: Option<f64>,
    pub irrigation_code: Option<String>,
    pub growing_environment_code: Option<String>,
    pub gip_system_code: Option<String>,
    #[serde(default)]
    pub crop_code: Option<String>,
    /// Provenance; absent means `"user"`. The manual crop form never sends the
    /// provenance fields — they describe where a row came from, not what the
    /// form holds — so they default rather than being required of every caller.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_campaign: Option<i64>,
    #[serde(default)]
    pub declared_area_ha: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct NewOperator {
    pub full_name: String,
    pub tax_id: Option<String>,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewUserProfile {
    pub display_name: String,
    pub operator_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewMachinery {
    pub farm_id: String,
    pub name: String,
    pub kind: Option<String>, // maps to column `type`
    pub acquired_on: Option<String>,
    pub last_inspection_date: Option<String>,
    pub next_inspection_due_date: Option<String>,
    /// Spanish registry numbers; an extension row is written when either is present.
    pub roma_number: Option<String>,
    pub reganip_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewPremises {
    pub farm_id: String,
    pub kind_code: String,
    pub name: String,
    pub address: Option<String>,
    pub vehicle_model: Option<String>,
    pub plate: Option<String>,
    #[serde(default)]
    pub class_code: Option<String>,
    pub volume_m3: Option<f64>,
    pub notes: Option<String>,
    /// Spanish registry fields; an extension row is written when either is
    /// present (the `NewMachinery` shape, flat rather than nested).
    #[serde(default)]
    pub cadastral_reference: Option<String>,
    #[serde(default)]
    pub rea_installation_code: Option<String>,
}

/// Full-row correction. Carries no `farm_id`: re-homing a premises would take
/// every treatment that names it to another holding — the `plot.farm_id`
/// precedent. `kind_code` IS correctable, because a mistyped kind is a typo
/// like any other; the module refuses the change if a record of the wrong
/// register already names it.
#[derive(Debug, Deserialize)]
pub struct UpdatePremises {
    pub kind_code: String,
    pub name: String,
    pub address: Option<String>,
    pub vehicle_model: Option<String>,
    pub plate: Option<String>,
    #[serde(default)]
    pub class_code: Option<String>,
    pub volume_m3: Option<f64>,
    pub notes: Option<String>,
    /// Spanish registry fields; an extension row is written when either is
    /// present (the `NewMachinery` shape, flat rather than nested).
    #[serde(default)]
    pub cadastral_reference: Option<String>,
    #[serde(default)]
    pub rea_installation_code: Option<String>,
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
    pub address: Option<String>,
    pub postal_code: Option<String>,
    pub phone_fixed: Option<String>,
    pub phone_mobile: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub opened_on: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country_code: String,
    pub es: Option<FarmEsFields>,
    /// `None` removes the representative row — same reconcile-from-submitted
    /// contract as `es`.
    pub representative: Option<FarmRepresentativeFields>,
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

/// An abstraction point for human consumption near a plot — the water half of
/// the printed model's section 2.2 (Anexo III A.1.f–g).
///
/// `inside_plot` and `distance_m` describe the *(plot, point)* pair rather than
/// the point itself, which is why a point serving two plots is recorded once per
/// plot: that is the claim the model's per-plot row makes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterPoint {
    pub id: String,
    pub plot_id: String,
    pub denomination: String,
    pub inside_plot: bool,
    /// Metres to the plot. Required when outside, always `None` when inside.
    pub distance_m: Option<f64>,
    /// Voluntary, WGS84/ETRS89 decimal degrees. Both or neither.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Input for recording a water point; the repository fills id and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWaterPoint {
    pub plot_id: String,
    pub denomination: String,
    pub inside_plot: bool,
    pub distance_m: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// Full-row update. Carries no `plot_id`: moving a point to another plot would
/// restate which plot the *original* row was about, so a mis-assigned point is
/// deleted and re-created (the `UpdateCrop` precedent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWaterPoint {
    pub denomination: String,
    pub inside_plot: bool,
    pub distance_m: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// The stored negative for one plot: "checked, and there is no abstraction
/// point here". Silence means the question was never asked, which is a
/// different claim — the `plot_zone_flag` philosophy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterDeclaration {
    pub id: String,
    pub plot_id: String,
    /// 'YYYY-MM-DD', the day the farmer stated it.
    pub declared_on: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
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
    pub tax_id: Option<String>,
    pub licence_number: Option<String>,
    pub licence_level_code: Option<String>,
    pub licence_expiry_date: Option<String>,
}

/// Full-row update for a season. `status` is deliberately absent: archiving is
/// a separate lifecycle action, not part of correcting a mistyped label or year.
#[derive(Debug, Deserialize)]
pub struct UpdateSeason {
    pub campaign_year: i64,
    pub label: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
}

/// Full-row update for a crop. `plot_id` and `season_id` are deliberately absent,
/// like `UpdatePlot`'s `farm_id`: re-homing a crop would silently re-home the
/// treatment history that points at it. Correcting a crop entered on the wrong
/// plot means deleting it and creating the right one.
///
/// The provenance fields break the full-row rule on purpose, and only they:
/// `species_name` … `crop_code` are form state, so the submitted value replaces
/// the stored one and `None` clears it, but `source`, `source_campaign` and
/// `declared_area_ha` say where the row came from. A form that does not know
/// about them must not erase them, so absent means "keep what is stored".
/// The consequence is deliberate: once stamped, provenance is a historical fact
/// this API cannot un-say.
#[derive(Debug, Deserialize)]
pub struct UpdateCrop {
    pub species_name: String,
    pub variety: Option<String>,
    pub production_system_code: Option<String>,
    pub area_ha: Option<f64>,
    pub irrigation_code: Option<String>,
    pub growing_environment_code: Option<String>,
    pub gip_system_code: Option<String>,
    #[serde(default)]
    pub crop_code: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_campaign: Option<i64>,
    #[serde(default)]
    pub declared_area_ha: Option<f64>,
}

/// Advisor form input; the repository fills `id` and timestamps.
#[derive(Debug, Deserialize)]
pub struct NewAdvisor {
    pub name: String,
    pub tax_id: Option<String>,
    pub registration_number: Option<String>,
}

/// Full-row update for an advisor: the form submits the complete desired state.
#[derive(Debug, Deserialize)]
pub struct UpdateAdvisor {
    pub name: String,
    pub tax_id: Option<String>,
    pub registration_number: Option<String>,
}

/// Full-row update for a user profile; the submitted state replaces the
/// stored one (`operator_id: None` unlinks).
#[derive(Debug, Deserialize)]
pub struct UpdateUserProfile {
    pub display_name: String,
    pub operator_id: Option<String>,
}

/// Full-row update for machinery. `farm_id` is deliberately absent, like
/// `UpdatePlot`: machinery never moves between farms. Both registry numbers
/// `None` means "no Spanish extension" and removes an existing extension row.
#[derive(Debug, Deserialize)]
pub struct UpdateMachinery {
    pub name: String,
    pub kind: Option<String>, // maps to column `type`
    pub acquired_on: Option<String>,
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

/// Spanish extension row for a premises. Logged to `record_change` as its own
/// entity (`entity_id` = `premises_id`), like machinery's.
///
/// Both fields are what the SPANISH registries say about this building and are
/// read off the same REA page. `rea_installation_code` is the authority's own
/// key for an installation registered in REA — what
/// `Edificaciones[].IdEdificacion` wants, and never ours to mint.
#[derive(Debug, Clone, Serialize)]
pub struct PremisesEsExtension {
    pub premises_id: String,
    pub cadastral_reference: Option<String>,
    pub rea_installation_code: Option<String>,
}

/// A premises with its Spanish extension — the registry list and edit form in
/// one round trip.
#[derive(Debug, Clone, Serialize)]
pub struct PremisesDetail {
    pub premises: Premises,
    pub es: Option<PremisesEsExtension>,
}

// ---------------------------------------------------------------------------
// Sowing and planting (feeds model sections 9.2 and 9.3)
// ---------------------------------------------------------------------------

/// How a crop began. Harvest's mirror image, and in core for the same reason:
/// the two bracket a crop, and crop planning, costs and analytics will want it.
///
/// Carries no eco-scheme practice code — core may not reference a module's
/// lookup, and a sowing is a farm event under no decree in particular. What
/// makes one evidence of RD 1048/2022 art. 45.2 is [`Self::flooded_on`].
#[derive(Debug, Clone, Serialize)]
pub struct SowingRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// `sowing` | `planting` — how the crop began, and SIEX
    /// `SiembraPlantacion`'s required 1/0. The register's form is titled
    /// "Siembra y plantación", so both are its documented use.
    pub kind_code: String,
    /// Model 9.3's "Fecha de siembra en seco", and the date 9.2's "Siembra"
    /// column prints. `SiembraPlantacion.FechaInicio`.
    pub sown_on: String,
    /// `None` = one day's work, never "unknown".
    pub sowing_end_date: Option<String>,
    /// `FechaInundacion`; model 9.3's second column, and the marker that this
    /// sowing is a cultivo bajo agua. `None` = not flooded (yet) — it is filled
    /// by correction weeks after a dry sowing.
    pub flooded_on: Option<String>,
    /// `Cantidad`, kilograms of seed. Required by the twin, printed by no page
    /// of section 9.
    pub seed_quantity_kg: Option<f64>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Which parcel was sown, with the crop frozen as it stood — `HarvestPlot`'s
/// mirror, including the absence of a surface column.
#[derive(Debug, Clone, Serialize)]
pub struct SowingPlot {
    pub id: String,
    pub sowing_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub crop_name_snapshot: Option<String>,
    pub variety_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SowingRecordDetail {
    pub record: SowingRecord,
    pub plots: Vec<SowingPlot>,
}

#[derive(Debug, Deserialize)]
pub struct NewSowingRecord {
    pub season_id: String,
    pub farm_id: String,
    pub kind_code: String,
    pub sown_on: String,
    #[serde(default)]
    pub sowing_end_date: Option<String>,
    #[serde(default)]
    pub flooded_on: Option<String>,
    #[serde(default)]
    pub seed_quantity_kg: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewSowingPlot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewSowingPlot {
    pub plot_id: String,
    #[serde(default)]
    pub crop_id: Option<String>,
}

/// Full-row update, on the `UpdateHarvestRecord` terms: no `season_id` or
/// `farm_id`, plots reconciled from the submitted state.
#[derive(Debug, Deserialize)]
pub struct UpdateSowingRecord {
    pub kind_code: String,
    pub sown_on: String,
    #[serde(default)]
    pub sowing_end_date: Option<String>,
    #[serde(default)]
    pub flooded_on: Option<String>,
    #[serde(default)]
    pub seed_quantity_kg: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewSowingPlot>,
}

// ---------------------------------------------------------------------------
// Commercialised harvest (model section 5)
// ---------------------------------------------------------------------------

/// What left the holding, and to whom. In core rather than in the CUE module
/// because it is whole-farm data the costs and analytics modules will want, and
/// modules never depend on each other.
#[derive(Debug, Clone, Serialize)]
pub struct HarvestRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    pub harvested_on: String,
    pub product_name: String,
    /// FEGA PROD_VEGETAL catalogue code — the HARVESTED PRODUCE, a different
    /// list from the PRODUCTOS crop codes `crop.crop_code` speaks in. Stored
    /// verbatim. `None` = free-text product with no catalogue match.
    pub plant_product_code: Option<String>,
    /// Nullable together with the unit: the printed form leaves the cell to be
    /// filled by hand, so an unstated quantity is unknown, never zero.
    pub quantity_value: Option<f64>,
    /// `kg` or `t`, enforced in the repository — `unit` is a module-cue lookup
    /// and core may never reference a module's table.
    pub quantity_unit_code: Option<String>,
    pub delivery_note_ref: Option<String>,
    pub lot_number: Option<String>,
    pub buyer_name: String,
    pub buyer_tax_id: Option<String>,
    pub buyer_address: Option<String>,
    /// The model's "Nº de RGSEAA"; named generically because core carries no
    /// regional identifiers.
    pub buyer_registry_number: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// Which parcel the harvest came from, with the crop frozen as it stood.
#[derive(Debug, Clone, Serialize)]
pub struct HarvestPlot {
    pub id: String,
    pub harvest_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub crop_name_snapshot: Option<String>,
    pub variety_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarvestRecordDetail {
    pub record: HarvestRecord,
    pub plots: Vec<HarvestPlot>,
}

#[derive(Debug, Deserialize)]
pub struct NewHarvestRecord {
    pub season_id: String,
    pub farm_id: String,
    pub harvested_on: String,
    pub product_name: String,
    #[serde(default)]
    pub plant_product_code: Option<String>,
    #[serde(default)]
    pub quantity_value: Option<f64>,
    #[serde(default)]
    pub quantity_unit_code: Option<String>,
    #[serde(default)]
    pub delivery_note_ref: Option<String>,
    #[serde(default)]
    pub lot_number: Option<String>,
    pub buyer_name: String,
    #[serde(default)]
    pub buyer_tax_id: Option<String>,
    #[serde(default)]
    pub buyer_address: Option<String>,
    #[serde(default)]
    pub buyer_registry_number: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Where it came from. At least one, and each plot must be on the farm.
    pub plots: Vec<NewHarvestPlot>,
}

#[derive(Debug, Deserialize)]
pub struct NewHarvestPlot {
    pub plot_id: String,
    #[serde(default)]
    pub crop_id: Option<String>,
}

/// Full-row update. `season_id` and `farm_id` are deliberately absent: a sale
/// never moves campaign or holding — correcting that means delete and re-enter,
/// the `UpdateCrop` precedent. The plots are reconciled from the submitted
/// state, like an extension table.
#[derive(Debug, Deserialize)]
pub struct UpdateHarvestRecord {
    pub harvested_on: String,
    pub product_name: String,
    #[serde(default)]
    pub plant_product_code: Option<String>,
    #[serde(default)]
    pub quantity_value: Option<f64>,
    #[serde(default)]
    pub quantity_unit_code: Option<String>,
    #[serde(default)]
    pub delivery_note_ref: Option<String>,
    #[serde(default)]
    pub lot_number: Option<String>,
    pub buyer_name: String,
    #[serde(default)]
    pub buyer_tax_id: Option<String>,
    #[serde(default)]
    pub buyer_address: Option<String>,
    #[serde(default)]
    pub buyer_registry_number: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewHarvestPlot>,
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
