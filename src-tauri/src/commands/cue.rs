// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `module-cue`: the treatment domain — products and
//! substances, every register of the record book RD 1311/2012 governs, alerts,
//! and the (dormant) SIEX export.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, active_actor, lock_conn};
use crate::state;
use crate::state::AppState;
use module_cue::alerts::AlertConfig;
use module_cue::demo::DemoSeedSummary;
use module_cue::models::ActiveSubstance;
use module_cue::models::Alert;
use module_cue::models::NewProduct;
use module_cue::models::NewProductAuthorisation;
use module_cue::models::NewTreatmentPlot;
use module_cue::models::NewTreatmentRecord;
use module_cue::models::PlotPhiStatus;
use module_cue::models::Product;
use module_cue::models::ProductActiveSubstance;
use module_cue::models::ProductAuthorisation;
use module_cue::models::ProductAuthorisationFields;
use module_cue::models::ProductDetail;
use module_cue::models::TreatmentRecord;
use module_cue::models::TreatmentRecordWithPlots;
use module_cue::models::UpdateProduct;
use module_cue::models::UpdateTreatmentRecord;
use module_cue::repository;
use serde::Serialize;
use tauri::State;
use terrazgo_core::date::today_utc;
use terrazgo_core::models::Lookup;

#[tauri::command]
pub fn list_alerts(state: State<'_, AppState>) -> CmdResult<Vec<Alert>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_active_alerts(&conn)?)
}

/// Reconcile alerts against today, then return the fresh list (one round-trip
/// for the UI). Idempotent by design; never touches acknowledged/dismissed status.
#[tauri::command]
pub fn refresh_alerts(state: State<'_, AppState>) -> CmdResult<Vec<Alert>> {
    let mut conn = lock_conn(&state)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(repository::list_active_alerts(&conn)?)
}

#[tauri::command]
pub fn acknowledge_alert(state: State<'_, AppState>, alert_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::acknowledge_alert(&mut conn, &alert_id)?)
}

#[tauri::command]
pub fn dismiss_alert(state: State<'_, AppState>, alert_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::dismiss_alert(&mut conn, &alert_id)?)
}

#[tauri::command]
pub fn get_treatment_record(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<TreatmentRecordWithPlots> {
    let conn = lock_conn(&state)?;
    Ok(repository::get_treatment_record(&conn, &id)?)
}

#[tauri::command]
pub fn list_reason_categories(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_reason_categories(&conn)?)
}

#[tauri::command]
pub fn list_efficacies(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_efficacies(&conn)?)
}

#[tauri::command]
pub fn list_justifications(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_justifications(&conn)?)
}

/// Active codes of the reference catalogue that problems of one category
/// resolve against for one country — the treatment form's problem picker.
/// Empty when the country has no coded list for the category (nothing to
/// offer; the record then stores whatever the user typed unchecked).
#[tauri::command]
pub fn list_problem_codes(
    state: State<'_, AppState>,
    country_code: String,
    reason_category_code: String,
) -> CmdResult<Vec<terrazgo_core::catalogue::CatalogueCode>> {
    let conn = lock_conn(&state)?;
    match module_cue::siex::problem_catalogue(&country_code, &reason_category_code) {
        Some(catalogue_id) => Ok(terrazgo_core::catalogue::active_codes(&conn, catalogue_id)?),
        None => Ok(Vec::new()),
    }
}

/// The non-chemical measures the treatment form offers for model 3.1 bis's
/// "Tipo de medida" — a closed list of fourteen, so a plain select rather than
/// a type-ahead picker.
#[tauri::command]
pub fn list_measures(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_cue::catalogue::CataloguePick>> {
    let conn = lock_conn(&state)?;
    Ok(module_cue::catalogue::measures(&conn, &country_code)?)
}

/// Growth stages the treatment form may offer per treated crop (Reglamento (UE)
/// 2023/564's annex). Named as the book prints them: the BBCH stage, which is
/// not the catalogue's own code.
#[tauri::command]
pub fn list_growth_stages(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_cue::catalogue::CataloguePick>> {
    let conn = lock_conn(&state)?;
    Ok(module_cue::catalogue::growth_stages(&conn, &country_code)?)
}

/// Products the treatment form may offer: only those authorised in the given
/// country (the farm's), because the insert rejects any other.
#[tauri::command]
pub fn list_products(state: State<'_, AppState>, country_code: String) -> CmdResult<Vec<Product>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_products_authorised(&conn, &country_code)?)
}

// ---------------------------------------------------------------------------
// Registry: operators, machinery, products (entry UI, 2026-07-03)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_formulation_types(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_formulation_types(&conn)?)
}

#[tauri::command]
pub fn list_authorisation_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_authorisation_kinds(&conn)?)
}

/// Active exceptional-authorisation codes (substance + product per code) for
/// the product form, shown only when the authorisation kind is 'exceptional'.
#[tauri::command]
pub fn list_exceptional_substances(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<terrazgo_core::catalogue::CatalogueCode>> {
    let conn = lock_conn(&state)?;
    match module_cue::siex::exceptional_substance_catalogue(&country_code) {
        Some(catalogue_id) => Ok(terrazgo_core::catalogue::active_codes(&conn, catalogue_id)?),
        None => Ok(Vec::new()),
    }
}

/// The registry's product list: every active product with its substances and
/// authorisations (country-agnostic, unlike `list_products`).
#[tauri::command]
pub fn list_product_details(state: State<'_, AppState>) -> CmdResult<Vec<ProductDetail>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_product_details(&conn)?)
}

/// Create a product with its first authorisation in one transaction — a
/// product without one would never be offered to the treatment form.
#[tauri::command]
pub fn create_product(
    state: State<'_, AppState>,
    product: NewProduct,
    authorisation: ProductAuthorisationFields,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<ProductDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_product_with_authorisation(
        &mut conn,
        product,
        authorisation,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_product(
    state: State<'_, AppState>,
    product_id: String,
    update: UpdateProduct,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Product> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::update_product(
        &mut conn,
        &product_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_product(
    state: State<'_, AppState>,
    product_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::soft_delete_product(
        &mut conn,
        &product_id,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn add_product_authorisation(
    state: State<'_, AppState>,
    product_id: String,
    authorisation: ProductAuthorisationFields,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<ProductAuthorisation> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id,
            country_code: authorisation.country_code,
            authorisation_number: authorisation.authorisation_number,
            kind_code: authorisation.kind_code,
            exceptional_substance_code: authorisation.exceptional_substance_code,
            status: authorisation.status,
            valid_from: authorisation.valid_from,
            valid_until: authorisation.valid_until,
        },
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn remove_product_authorisation(
    state: State<'_, AppState>,
    authorisation_id: String,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::remove_product_authorisation(
        &mut conn,
        &authorisation_id,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn list_active_substances(state: State<'_, AppState>) -> CmdResult<Vec<ActiveSubstance>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_active_substances(&conn)?)
}

#[tauri::command]
pub fn create_active_substance(
    state: State<'_, AppState>,
    name: String,
    cas_number: Option<String>,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<ActiveSubstance> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_active_substance(
        &mut conn,
        &name,
        cas_number.as_deref(),
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn add_product_substance(
    state: State<'_, AppState>,
    product_id: String,
    active_substance_id: String,
    concentration_value: Option<f64>,
    concentration_unit_code: Option<String>,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<ProductActiveSubstance> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::add_product_active_substance(
        &mut conn,
        &product_id,
        &active_substance_id,
        concentration_value,
        concentration_unit_code.as_deref(),
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn remove_product_substance(
    state: State<'_, AppState>,
    link_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::remove_product_active_substance(
        &mut conn,
        &link_id,
        actor.as_deref(),
    )?)
}

/// Insert a treatment with its treated plots, then reconcile alerts so the new
/// PHI window shows up immediately.
#[tauri::command]
pub fn create_treatment_record(
    state: State<'_, AppState>,
    record: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<TreatmentRecord> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let record = repository::insert_treatment_record(&mut conn, record, plots, actor.as_deref())?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(record)
}

/// Correct a treatment. Alerts are reconciled afterwards for the same reason
/// the insert does it: a corrected date or plazo moves the PHI window, and an
/// alert still pointing at the old one would be a wrong answer about when the
/// crop may be harvested.
#[tauri::command]
pub fn update_treatment_record(
    state: State<'_, AppState>,
    treatment_id: String,
    update: UpdateTreatmentRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<TreatmentRecordWithPlots> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let record =
        repository::update_treatment_record(&mut conn, &treatment_id, update, actor.as_deref())?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(record)
}

#[tauri::command]
pub fn list_treatment_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<TreatmentRecordWithPlots>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_treatment_records(
        &conn, &season_id, &farm_id,
    )?)
}

/// Record (or correct) the observed efficacy — the one edit a stored treatment
/// allows, because efficacy is assessed after application.
#[tauri::command]
pub fn set_treatment_efficacy(
    state: State<'_, AppState>,
    treatment_id: String,
    efficacy_code: Option<String>,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<TreatmentRecord> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::set_treatment_efficacy(
        &mut conn,
        &treatment_id,
        efficacy_code,
        actor.as_deref(),
    )?)
}

/// Soft delete (regulatory records are never hard-deleted), then reconcile
/// alerts so the record's PHI alert lapses with it.
#[tauri::command]
pub fn delete_treatment_record(
    state: State<'_, AppState>,
    treatment_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    repository::soft_delete_treatment_record(&mut conn, &treatment_id, actor.as_deref())?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SIEX cuaderno export
// ---------------------------------------------------------------------------

/// What blocks a valid SIEX export of the selected farm+season — empty lists
/// mean ready. Read-only; the UI renders the result as a fix-it list.
#[tauri::command]
pub fn export_cuaderno_precheck(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<module_cue::export::ExportPrecheck> {
    let conn = lock_conn(&state)?;
    Ok(module_cue::export::export_precheck(
        &conn, &season_id, &farm_id,
    )?)
}

#[derive(Serialize)]
pub struct CuadernoExportSummary {
    pub path: String,
    pub size_bytes: u64,
    /// `TratamFito` entries written (after the per-crop splits, so this can
    /// exceed the record count).
    pub entries: usize,
}

/// Build the SIEX descriptor for one farm+season and write it to `dest_path`
/// (chosen by the user in the save dialog, so overwriting is already
/// confirmed). Fails with `invalid.export_precheck_failed` while the precheck
/// is not clean — the frontend runs the precheck first and shows the list.
/// `async` like the backup commands: the work scales with record count and
/// must not block the main thread (no `.await` inside, so holding the
/// connection guard is safe).
#[tauri::command]
pub async fn export_cuaderno(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
    dest_path: String,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<CuadernoExportSummary> {
    let actor = active_actor(&settings_state)?;
    let mut guard = lock_conn(&state)?;
    let cuaderno =
        module_cue::export::build_cuaderno(&mut guard, &season_id, &farm_id, actor.as_deref())?;
    let json = serde_json::to_string_pretty(&cuaderno)?;
    crate::user_files::write_user_file(&app, &dest_path, json.as_bytes())?;
    let entries = cuaderno
        .cuaderno
        .iter()
        .map(|entry| entry.actividades_explotacion.tratam_fito.len())
        .sum();
    Ok(CuadernoExportSummary {
        path: dest_path,
        size_bytes: json.len() as u64,
        entries,
    })
}

/// Per-plot PHI standing (in window / harvest allowed) of a farm's plots
/// against today — feeds the map's PHI overlay.
#[tauri::command]
pub fn list_phi_status(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<PlotPhiStatus>> {
    let conn = lock_conn(&state)?;
    Ok(repository::phi_status_for_farm(
        &conn,
        &farm_id,
        &today_utc(),
    )?)
}

/// Dev-only: seed the demo campaign so the UI has something to show.
///
/// The demo code is compiled in unconditionally (cargo features cannot be
/// debug-profile-conditional). Release builds seed too while the project is
/// pre-release — field testing runs on release APKs and needs data to poke
/// at; the seeder itself refuses to run twice. Before the stable release
/// this must be revisited: re-guard with `cfg!(not(debug_assertions))` or
/// drop the command outright.
#[tauri::command]
pub fn seed_demo_data(state: State<'_, AppState>) -> CmdResult<DemoSeedSummary> {
    let mut conn = lock_conn(&state)?;
    let summary = module_cue::demo::seed_demo(&mut conn)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(summary)
}

// --- non-field treatments (model 3.3 / 3.4 / 3.5) --------------------------

#[tauri::command]
pub fn list_non_field_subject_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_non_field_subject_kinds(&conn)?)
}

#[tauri::command]
pub fn list_register_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_register_kinds(&conn)?)
}

#[tauri::command]
pub fn list_non_field_treatments(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_cue::models::NonFieldTreatmentDetail>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_non_field_treatments(
        &conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_non_field_treatment(
    state: State<'_, AppState>,
    record: module_cue::models::NewNonFieldTreatment,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::NonFieldTreatmentDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_non_field_treatment(
        &mut conn,
        record,
        actor.as_deref(),
    )?)
}

/// Correct a non-field treatment. No alert refresh: these registers carry no
/// plazo de seguridad — nothing in them feeds the alert engine.
#[tauri::command]
pub fn update_non_field_treatment(
    state: State<'_, AppState>,
    treatment_id: String,
    update: module_cue::models::UpdateNonFieldTreatment,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::NonFieldTreatmentDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::update_non_field_treatment(
        &mut conn,
        &treatment_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn set_non_field_efficacy(
    state: State<'_, AppState>,
    treatment_id: String,
    efficacy_code: Option<String>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::NonFieldTreatment> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::set_non_field_efficacy(
        &mut conn,
        &treatment_id,
        efficacy_code,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_non_field_treatment(
    state: State<'_, AppState>,
    treatment_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    repository::soft_delete_non_field_treatment(&mut conn, &treatment_id, actor.as_deref())?;
    Ok(())
}

// --- the registers' stored "APLICA TRATAMIENTO: NO" ------------------------

#[tauri::command]
pub fn list_register_declarations(
    state: State<'_, AppState>,
    farm_id: String,
    season_id: String,
) -> CmdResult<Vec<module_cue::models::RegisterDeclaration>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_register_declarations(
        &conn, &farm_id, &season_id,
    )?)
}

#[tauri::command]
pub fn set_register_declaration(
    state: State<'_, AppState>,
    farm_id: String,
    season_id: String,
    register_code: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::RegisterDeclaration> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::set_register_declaration(
        &mut conn,
        &farm_id,
        &season_id,
        &register_code,
        &today_utc(),
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn clear_register_declaration(
    state: State<'_, AppState>,
    farm_id: String,
    season_id: String,
    register_code: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    repository::clear_register_declaration(
        &mut conn,
        &farm_id,
        &season_id,
        &register_code,
        actor.as_deref(),
    )?;
    Ok(())
}

// --- treated seed (model 3.2) ----------------------------------------------

#[tauri::command]
pub fn list_seed_treatments(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_cue::models::SeedTreatmentDetail>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_seed_treatments(
        &conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_seed_treatment(
    state: State<'_, AppState>,
    record: module_cue::models::NewSeedTreatment,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::SeedTreatmentDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_seed_treatment(
        &mut conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_seed_treatment(
    state: State<'_, AppState>,
    seed_treatment_id: String,
    update: module_cue::models::UpdateSeedTreatment,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::SeedTreatmentDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::update_seed_treatment(
        &mut conn,
        &seed_treatment_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn set_seed_treatment_efficacy(
    state: State<'_, AppState>,
    seed_treatment_id: String,
    efficacy_code: Option<String>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::SeedTreatment> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::set_seed_treatment_efficacy(
        &mut conn,
        &seed_treatment_id,
        efficacy_code,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_seed_treatment(
    state: State<'_, AppState>,
    seed_treatment_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    repository::soft_delete_seed_treatment(&mut conn, &seed_treatment_id, actor.as_deref())?;
    Ok(())
}

// --- analyses (model 4) -----------------------------------------------------

#[tauri::command]
pub fn list_analysis_materials(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_analysis_materials(&conn)?)
}

/// What the laboratory looked for (model section 4). Its own list because the
/// model prints no column for it — the book folds it into the material cell.
#[tauri::command]
pub fn list_analysis_types(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_analysis_types(&conn)?)
}

/// Where treated seed was treated (model section 3.2).
#[tauri::command]
pub fn list_seed_treatment_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_seed_treatment_kinds(&conn)?)
}

/// The harvested produce a sale (section 5) or a postharvest treatment (3.3)
/// can name — FEGA's `PROD_VEGETAL`, which is NOT the crop catalogue the
/// species picker offers. Empty for a country with no coded list.
#[tauri::command]
pub fn list_plant_products(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_cue::catalogue::CataloguePick>> {
    let conn = lock_conn(&state)?;
    Ok(module_cue::catalogue::plant_products(&conn, &country_code)?)
}

/// The active substances an analysis can report (FEGA `SUST_ACTIVAS`), for the
/// findings multi-select. Offline always: the catalogues ship in the binary.
#[tauri::command]
pub fn list_substance_codes(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_cue::catalogue::CataloguePick>> {
    let conn = lock_conn(&state)?;
    Ok(module_cue::catalogue::substances(&conn, &country_code)?)
}

#[tauri::command]
pub fn list_analysis_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_cue::models::AnalysisRecordDetail>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_analysis_records(
        &conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_analysis_record(
    state: State<'_, AppState>,
    record: module_cue::models::NewAnalysisRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::AnalysisRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_analysis_record(
        &mut conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_analysis_record(
    state: State<'_, AppState>,
    analysis_record_id: String,
    update: module_cue::models::UpdateAnalysisRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_cue::models::AnalysisRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(repository::update_analysis_record(
        &mut conn,
        &analysis_record_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_analysis_record(
    state: State<'_, AppState>,
    analysis_record_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    repository::soft_delete_analysis_record(&mut conn, &analysis_record_id, actor.as_deref())?;
    Ok(())
}
