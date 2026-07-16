// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tauri commands: thin wrappers over the `terrazgo_core` and `module_cue`
//! repositories, plus the error mapping for the command boundary. Logic stays
//! in the crates and is tested there (docs/architecture.md → Testing strategy #4).

use anyhow::anyhow;
use module_cue::alerts::AlertConfig;
use module_cue::demo::DemoSeedSummary;
use module_cue::models::{
    ActiveSubstance, Alert, NewProduct, NewProductAuthorisation, NewTreatmentPlot,
    NewTreatmentRecord, PlotPhiStatus, Product, ProductActiveSubstance, ProductAuthorisation,
    ProductAuthorisationFields, ProductDetail, TreatmentRecord, TreatmentRecordWithPlots,
    UpdateProduct,
};
use module_cue::repository;
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use std::sync::MutexGuard;
use tauri::State;
use terrazgo_core::date::today_utc;
use terrazgo_core::models::{
    Country, Crop, Farm, FarmDetail, GeoFeature, Lookup, Machinery, MachineryDetail, NewCrop,
    NewFarm, NewGeoFeature, NewMachinery, NewOperator, NewPlot, NewSeason, Operator, Plot,
    PlotDetail, Season, UpdateFarm, UpdateMachinery, UpdateOperator, UpdatePlot, ZoneFlag,
};
use terrazgo_core::repository as core_repo;

use terrazgo_core::settings::AppSettings;

use crate::state;
use crate::state::AppState;

/// Serializable error for the command boundary. Tauri requires command errors
/// to implement `Serialize`; `CueError`/`anyhow::Error` do not.
///
/// Serialized as `{ code, params, message }`: `code` is a stable machine
/// string the frontend maps to an `error.<code>` i18n key, `params` carries
/// the values its `{placeholders}` interpolate, and `message` is the full
/// `{:#}` Display chain (message + causes) — the untranslated fallback for
/// codes without a dictionary entry and the debugging trail for `internal`.
pub struct CommandError(anyhow::Error);

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let (code, params) = classify(&self.0);
        let mut s = serializer.serialize_struct("CommandError", 3)?;
        s.serialize_field("code", &code)?;
        s.serialize_field("params", &params)?;
        s.serialize_field("message", &format!("{:#}", self.0))?;
        s.end()
    }
}

/// Map a boundary error to its (code, interpolation params) pair.
///
/// `anyhow::Error` keeps the concrete type it was built from, so the domain
/// errors are recovered here by downcast — the commands themselves stay on the
/// blanket `?` conversion and never name error variants. Anything that is not
/// a domain error (SQLite, migration, poisoned mutex, …) is `internal`: the
/// frontend has no dictionary entry for it and shows the raw message instead.
///
/// Public for the i18n contract test (`tests/i18n_contract.rs`), which checks
/// that every emitted code has an `error.<code>` key in every locale dictionary.
pub fn classify(err: &anyhow::Error) -> (String, serde_json::Value) {
    use serde_json::json;

    if let Some(e) = err.downcast_ref::<module_cue::CueError>() {
        use module_cue::CueError;
        return match e {
            CueError::NotFound => ("not_found".into(), json!({})),
            CueError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            CueError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            CueError::AuthorisationMissing {
                product_id,
                country,
            } => (
                "authorisation_missing".into(),
                json!({ "product_id": product_id, "country": country }),
            ),
            CueError::CountryMismatch { provided, farm } => (
                "country_mismatch".into(),
                json!({ "provided": provided, "farm": farm }),
            ),
            CueError::PlotNotOnFarm { plot_id, farm_id } => (
                "plot_not_on_farm".into(),
                json!({ "plot_id": plot_id, "farm_id": farm_id }),
            ),
            CueError::MissingPhiDays => ("missing_phi_days".into(), json!({})),
            CueError::Sqlite(_)
            | CueError::Migration(_)
            | CueError::Json(_)
            | CueError::Io(_)
            | CueError::Catalogue(_)
            | CueError::Report(_) => ("internal".into(), json!({})),
        };
    }

    if let Some(e) = err.downcast_ref::<terrazgo_core::CoreError>() {
        use terrazgo_core::CoreError;
        return match e {
            CoreError::NotFound => ("not_found".into(), json!({})),
            CoreError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            CoreError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            CoreError::Sqlite(_)
            | CoreError::Migration(_)
            | CoreError::Json(_)
            | CoreError::Io(_)
            | CoreError::Catalogue(_) => ("internal".into(), json!({})),
        };
    }

    if let Some(e) = err.downcast_ref::<terrazgo_geo::GeoError>() {
        use terrazgo_geo::GeoError;
        return match e {
            GeoError::NotFound => ("not_found".into(), json!({})),
            GeoError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            GeoError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            // The two user-explainable network outcomes: the service said no,
            // or there is no network. Both leave the app fully usable.
            GeoError::Http { status } => ("geo_http".into(), json!({ "status": status })),
            // The transport detail (DNS, TLS, refused, timeout…) rides along:
            // "offline" is a diagnosis the user must be able to dispute — a
            // firewalled or proxied machine is online for the browser and
            // unreachable for us, and the reason string is the only evidence.
            GeoError::Offline(reason) => ("geo_offline".into(), json!({ "reason": reason })),
            GeoError::Cache(_)
            | GeoError::Migration(_)
            | GeoError::Json(_)
            | GeoError::Io(_)
            | GeoError::Catalogue(_) => ("internal".into(), json!({})),
        };
    }

    ("internal".into(), json!({}))
}

// Blanket conversion so `?` maps any error (`CueError`, `rusqlite::Error`,
// plain `anyhow::Error`, …) into `CommandError` at the boundary. Legal only
// because `CommandError` itself is not `Into<anyhow::Error>` — otherwise this
// would overlap with the standard library's reflexive `From<T> for T`.
impl<E: Into<anyhow::Error>> From<E> for CommandError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// Lock the shared connection. A poisoned mutex (a panic while another command
/// held the lock) is unrecoverable for that connection — surface it as an error
/// rather than `unwrap()` (no unwrap/expect outside tests).
fn lock_conn<'a>(state: &'a State<'_, AppState>) -> CmdResult<MutexGuard<'a, Connection>> {
    state
        .conn
        .lock()
        .map_err(|_| CommandError(anyhow!("database connection mutex is poisoned")))
}

/// Same poisoned-mutex reasoning as [`lock_conn`], for the geo cache.
fn lock_geo<'a>(geo: &'a State<'_, state::GeoState>) -> CmdResult<MutexGuard<'a, Connection>> {
    geo.conn
        .lock()
        .map_err(|_| CommandError(anyhow!("geo cache mutex is poisoned")))
}

#[derive(Serialize)]
pub struct AppStatus {
    pub db_path: String,
    pub schema_version: usize,
    pub app_version: &'static str,
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> CmdResult<AppStatus> {
    Ok(AppStatus {
        db_path: state.db_path.display().to_string(),
        schema_version: state.schema_version,
        app_version: env!("CARGO_PKG_VERSION"),
    })
}

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

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Settings plus the code-owned defaults the UI needs to render "unset"
/// meaningfully: an unset cache cap displays the default value, not a blank,
/// and the frontend must not hardcode a copy of the constant.
#[derive(Serialize)]
pub struct SettingsInfo {
    pub settings: AppSettings,
    pub tile_cache_default_bytes: i64,
}

fn settings_info(settings: AppSettings) -> SettingsInfo {
    SettingsInfo {
        settings,
        tile_cache_default_bytes: terrazgo_geo::db::TILE_CACHE_MAX_BYTES,
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, state::SettingsState>) -> CmdResult<SettingsInfo> {
    let guard = state
        .settings
        .lock()
        .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
    Ok(settings_info(guard.clone()))
}

/// Replace the settings wholesale — the Settings form is the source of truth,
/// like the farm/plot full-row updates. Validation belongs to each setting's
/// owning crate (the cache cap is range-checked by terrazgo-geo); the file is
/// written before the in-memory copy so a failed save never leaves them
/// disagreeing. The new cap is enforced immediately: shrinking the cache must
/// visibly act, not wait for the next launch.
///
/// `async` because that enforcement can VACUUM a multi-hundred-MB file
/// (seconds); the body stays synchronous — no `.await`, so holding the state
/// guards is safe.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, state::SettingsState>,
    geo: State<'_, state::GeoState>,
    settings: AppSettings,
) -> CmdResult<SettingsInfo> {
    if let Some(bytes) = settings.tile_cache_max_bytes {
        terrazgo_geo::db::validate_tile_cache_cap(bytes)?;
    }

    terrazgo_core::settings::save_settings(&state.path, &settings)?;
    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
        *guard = settings.clone();
    }

    let cap = settings
        .tile_cache_max_bytes
        .unwrap_or(terrazgo_geo::db::TILE_CACHE_MAX_BYTES);
    let conn = lock_geo(&geo)?;
    terrazgo_geo::db::enforce_tile_cache_cap(&conn, cap)?;

    Ok(settings_info(settings))
}

/// Empty the tile cache, keeping `resource` rows (styles, glyphs, SIGPAC
/// lookup/zone responses — a verified plot stays verifiable offline). Returns
/// the number of tiles dropped, for the notification. `async` for the VACUUM,
/// same reasoning as `update_settings`.
#[tauri::command]
pub async fn clear_tile_cache(geo: State<'_, state::GeoState>) -> CmdResult<usize> {
    let conn = lock_geo(&geo)?;
    Ok(terrazgo_geo::db::clear_tile_cache(&conn)?)
}

// ---------------------------------------------------------------------------
// Backup export / import
// ---------------------------------------------------------------------------

/// Export a verified snapshot of the live database to `dest_path` (chosen by
/// the user in the save dialog, so overwriting is already confirmed).
///
/// `async` because sync commands run on the main thread and freeze the window
/// while they work; `VACUUM INTO` + verification scale with database size, so
/// this must run on the async runtime's pool instead. The body stays fully
/// synchronous — it blocks a worker thread, never the UI.
#[tauri::command]
pub async fn export_backup(
    state: State<'_, AppState>,
    dest_path: String,
) -> CmdResult<terrazgo_core::backup::BackupSummary> {
    let conn = lock_conn(&state)?;
    Ok(terrazgo_core::backup::export_backup(
        &conn,
        Path::new(&dest_path),
    )?)
}

#[derive(Serialize)]
pub struct ImportSummary {
    /// Schema version found in the imported file (before forward migration).
    pub schema_version_found: i64,
    /// Where the pre-import safety copy of the previous database was written.
    pub safety_backup_path: String,
}

/// Replace the live database with a backup file.
///
/// Order is the safety argument: (1) validate the file (integrity + schema
/// version — newer-than-app is rejected, older migrates forward on reopen);
/// (2) export a safety copy of the CURRENT database next to it; (3) close the
/// live connection (parking an in-memory placeholder in the mutex), copy the
/// backup over the live path, reopen through the composed migration runner and
/// refresh alerts. If reopening fails midway the placeholder stays parked —
/// commands error until restart — but the previous data is already safe in the
/// pre-import copy.
/// `async` for the same reason as `export_backup`: validate + safety copy +
/// file swap take time proportional to database size and must not block the
/// main thread (no `.await` inside, so holding the mutex guard is safe).
#[tauri::command]
pub async fn import_backup(
    state: State<'_, AppState>,
    src_path: String,
) -> CmdResult<ImportSummary> {
    let mut guard = lock_conn(&state)?;

    // The live db is always at the latest composed version, so it IS the
    // ceiling of what this build supports.
    let live_version: i64 = guard.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let info = terrazgo_core::backup::validate_backup(Path::new(&src_path), live_version)?;

    let backups_dir = state
        .db_path
        .parent()
        .ok_or_else(|| CommandError(anyhow!("database path has no parent directory")))?
        .join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    // ISO instant with the filename-hostile characters stripped: 20260702T101500Z.
    let stamp: String = today_utc_instant().replace(['-', ':'], "");
    let safety_path = backups_dir.join(format!("pre-import-{stamp}.db"));
    terrazgo_core::backup::export_backup(&guard, &safety_path)?;

    // Swap: park a placeholder so the old connection drops (closing the file
    // and checkpointing its WAL) before the copy lands on the same path.
    let placeholder = Connection::open_in_memory()?;
    drop(std::mem::replace(&mut *guard, placeholder));
    for suffix in ["-wal", "-shm"] {
        let sidecar = state.db_path.display().to_string() + suffix;
        if Path::new(&sidecar).exists() {
            std::fs::remove_file(&sidecar)?;
        }
    }
    std::fs::copy(&src_path, &state.db_path)?;

    let mut conn = crate::db::open_app_db(&state.db_path)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    *guard = conn;

    Ok(ImportSummary {
        schema_version_found: info.schema_version,
        safety_backup_path: safety_path.display().to_string(),
    })
}

/// Full UTC instant (not just the date) for unique backup filenames.
fn today_utc_instant() -> String {
    terrazgo_core::date::now_utc_iso()
}

// ---------------------------------------------------------------------------
// Farm / plot management (core entities)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_countries(state: State<'_, AppState>) -> CmdResult<Vec<Country>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_countries(&conn)?)
}

#[tauri::command]
pub fn list_farms(state: State<'_, AppState>) -> CmdResult<Vec<Farm>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_farms(&conn)?)
}

#[tauri::command]
pub fn get_farm(state: State<'_, AppState>, farm_id: String) -> CmdResult<FarmDetail> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::get_farm(&conn, &farm_id)?)
}

/// `farm` arrives as a JSON object matching `NewFarm` (snake_case fields,
/// optional `es` sub-object with the Spanish extension).
#[tauri::command]
pub fn create_farm(state: State<'_, AppState>, farm: NewFarm) -> CmdResult<Farm> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_farm(&mut conn, farm)?)
}

#[tauri::command]
pub fn update_farm(
    state: State<'_, AppState>,
    farm_id: String,
    update: UpdateFarm,
) -> CmdResult<FarmDetail> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_farm(&mut conn, &farm_id, update)?)
}

#[tauri::command]
pub fn delete_farm(state: State<'_, AppState>, farm_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_farm(&mut conn, &farm_id)?)
}

#[tauri::command]
pub fn list_plots(state: State<'_, AppState>, farm_id: String) -> CmdResult<Vec<PlotDetail>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_plots(&conn, &farm_id)?)
}

#[tauri::command]
pub fn create_plot(state: State<'_, AppState>, plot: NewPlot) -> CmdResult<Plot> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_plot(&mut conn, plot)?)
}

#[tauri::command]
pub fn update_plot(
    state: State<'_, AppState>,
    plot_id: String,
    update: UpdatePlot,
) -> CmdResult<PlotDetail> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_plot(&mut conn, &plot_id, update)?)
}

#[tauri::command]
pub fn delete_plot(state: State<'_, AppState>, plot_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_plot(&mut conn, &plot_id)?)
}

// ---------------------------------------------------------------------------
// Seasons, crops and the treatment record book
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_seasons(state: State<'_, AppState>) -> CmdResult<Vec<Season>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_seasons(&conn)?)
}

#[tauri::command]
pub fn create_season(state: State<'_, AppState>, season: NewSeason) -> CmdResult<Season> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_season(&mut conn, season)?)
}

#[tauri::command]
pub fn list_crops(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<Crop>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_crops(&conn, &season_id, &farm_id)?)
}

#[tauri::command]
pub fn create_crop(state: State<'_, AppState>, crop: NewCrop) -> CmdResult<Crop> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_crop(&mut conn, crop)?)
}

#[tauri::command]
pub fn list_operators(state: State<'_, AppState>) -> CmdResult<Vec<Operator>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_operators(&conn)?)
}

#[tauri::command]
pub fn list_machinery(state: State<'_, AppState>, farm_id: String) -> CmdResult<Vec<Machinery>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_machinery(&conn, &farm_id)?)
}

#[tauri::command]
pub fn list_production_systems(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_production_systems(&conn)?)
}

#[tauri::command]
pub fn list_units(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(repository::list_units(&conn)?)
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

/// Shorthand for the alert reconciliation the registry write commands run:
/// operator and machinery rows are alert sources (licence/ITV expiry), so
/// every change must be reflected in the alert list immediately.
fn reconcile_alerts(conn: &mut Connection) -> Result<(), CommandError> {
    repository::refresh_alerts(conn, &today_utc(), &AlertConfig::default())?;
    Ok(())
}

#[tauri::command]
pub fn list_licence_levels(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_licence_levels(&conn)?)
}

#[tauri::command]
pub fn create_operator(state: State<'_, AppState>, operator: NewOperator) -> CmdResult<Operator> {
    let mut conn = lock_conn(&state)?;
    let operator = core_repo::insert_operator(&mut conn, operator)?;
    reconcile_alerts(&mut conn)?;
    Ok(operator)
}

#[tauri::command]
pub fn update_operator(
    state: State<'_, AppState>,
    operator_id: String,
    update: UpdateOperator,
) -> CmdResult<Operator> {
    let mut conn = lock_conn(&state)?;
    let operator = core_repo::update_operator(&mut conn, &operator_id, update)?;
    reconcile_alerts(&mut conn)?;
    Ok(operator)
}

#[tauri::command]
pub fn delete_operator(state: State<'_, AppState>, operator_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_operator(&mut conn, &operator_id)?;
    reconcile_alerts(&mut conn)?;
    Ok(())
}

#[tauri::command]
pub fn list_machinery_details(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<MachineryDetail>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_machinery_details(&conn, &farm_id)?)
}

#[tauri::command]
pub fn create_machinery(
    state: State<'_, AppState>,
    machinery: NewMachinery,
) -> CmdResult<Machinery> {
    let mut conn = lock_conn(&state)?;
    let machinery = core_repo::insert_machinery(&mut conn, machinery)?;
    reconcile_alerts(&mut conn)?;
    Ok(machinery)
}

#[tauri::command]
pub fn update_machinery(
    state: State<'_, AppState>,
    machinery_id: String,
    update: UpdateMachinery,
) -> CmdResult<MachineryDetail> {
    let mut conn = lock_conn(&state)?;
    let detail = core_repo::update_machinery(&mut conn, &machinery_id, update)?;
    reconcile_alerts(&mut conn)?;
    Ok(detail)
}

#[tauri::command]
pub fn delete_machinery(state: State<'_, AppState>, machinery_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_machinery(&mut conn, &machinery_id)?;
    reconcile_alerts(&mut conn)?;
    Ok(())
}

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
) -> CmdResult<ProductDetail> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_product_with_authorisation(
        &mut conn,
        product,
        authorisation,
    )?)
}

#[tauri::command]
pub fn update_product(
    state: State<'_, AppState>,
    product_id: String,
    update: UpdateProduct,
) -> CmdResult<Product> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::update_product(&mut conn, &product_id, update)?)
}

#[tauri::command]
pub fn delete_product(state: State<'_, AppState>, product_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::soft_delete_product(&mut conn, &product_id)?)
}

#[tauri::command]
pub fn add_product_authorisation(
    state: State<'_, AppState>,
    product_id: String,
    authorisation: ProductAuthorisationFields,
) -> CmdResult<ProductAuthorisation> {
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
    )?)
}

#[tauri::command]
pub fn remove_product_authorisation(
    state: State<'_, AppState>,
    authorisation_id: String,
) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::remove_product_authorisation(
        &mut conn,
        &authorisation_id,
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
) -> CmdResult<ActiveSubstance> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::insert_active_substance(
        &mut conn,
        &name,
        cas_number.as_deref(),
    )?)
}

#[tauri::command]
pub fn add_product_substance(
    state: State<'_, AppState>,
    product_id: String,
    active_substance_id: String,
    concentration_value: Option<f64>,
    concentration_unit_code: Option<String>,
) -> CmdResult<ProductActiveSubstance> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::add_product_active_substance(
        &mut conn,
        &product_id,
        &active_substance_id,
        concentration_value,
        concentration_unit_code.as_deref(),
    )?)
}

#[tauri::command]
pub fn remove_product_substance(state: State<'_, AppState>, link_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::remove_product_active_substance(
        &mut conn, &link_id,
    )?)
}

/// Insert a treatment with its treated plots, then reconcile alerts so the new
/// PHI window shows up immediately.
#[tauri::command]
pub fn create_treatment_record(
    state: State<'_, AppState>,
    record: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
) -> CmdResult<TreatmentRecord> {
    let mut conn = lock_conn(&state)?;
    let record = repository::insert_treatment_record(&mut conn, record, plots)?;
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
) -> CmdResult<TreatmentRecord> {
    let mut conn = lock_conn(&state)?;
    Ok(repository::set_treatment_efficacy(
        &mut conn,
        &treatment_id,
        efficacy_code,
    )?)
}

/// Soft delete (regulatory records are never hard-deleted), then reconcile
/// alerts so the record's PHI alert lapses with it.
#[tauri::command]
pub fn delete_treatment_record(state: State<'_, AppState>, treatment_id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    repository::soft_delete_treatment_record(&mut conn, &treatment_id)?;
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
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
    dest_path: String,
) -> CmdResult<CuadernoExportSummary> {
    let mut guard = lock_conn(&state)?;
    let cuaderno = module_cue::export::build_cuaderno(&mut guard, &season_id, &farm_id)?;
    let json = serde_json::to_string_pretty(&cuaderno)?;
    std::fs::write(Path::new(&dest_path), &json)?;
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

#[derive(Serialize)]
pub struct CuadernoPdfSummary {
    pub path: String,
    pub size_bytes: u64,
    pub pages: usize,
}

/// Render the printable cuaderno (official-model sections 1, 2.1 and 3.1)
/// for one farm+season and write the PDF to `dest_path` (chosen by the user
/// in the save dialog). No precheck: fields the model asks for but the data
/// lacks print blank — a farmer can always print the current state. `async`
/// like the other export: rendering scales with record count.
#[tauri::command]
pub async fn export_cuaderno_pdf(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
    dest_path: String,
) -> CmdResult<CuadernoPdfSummary> {
    let guard = lock_conn(&state)?;
    let today = terrazgo_core::date::now_utc_iso();
    let generated_on = today.split('T').next().unwrap_or(&today);
    let pdf = module_cue::report::render_cuaderno(&guard, &season_id, &farm_id, generated_on)?;
    std::fs::write(Path::new(&dest_path), &pdf.bytes)?;
    Ok(CuadernoPdfSummary {
        path: dest_path,
        size_bytes: pdf.bytes.len() as u64,
        pages: pdf.page_count,
    })
}

// ---------------------------------------------------------------------------
// Geo: stored geometries, map styles, boundary-file import
// ---------------------------------------------------------------------------

/// Active geometries of a farm (its own plus its plots') — one call feeds the
/// whole map for a farm.
#[tauri::command]
pub fn list_geo_features(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<GeoFeature>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_geo_features_for_farm(&conn, &farm_id)?)
}

/// Save a plot boundary (drawn or imported), replacing this source's previous
/// one. `source` is `manual` or `import` from the UI; provider modules write
/// their own sources through their own paths later.
#[tauri::command]
pub fn save_plot_boundary(
    state: State<'_, AppState>,
    plot_id: String,
    geometry: String,
    source: String,
) -> CmdResult<GeoFeature> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::save_geo_feature(
        &mut conn,
        NewGeoFeature {
            plot_id: Some(plot_id),
            farm_id: None,
            role: "boundary".into(),
            geometry,
            source,
            campaign: None,
            official_area_ha: None,
            properties: None,
            fetched_at: None,
        },
    )?)
}

#[tauri::command]
pub fn delete_geo_feature(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_geo_feature(&mut conn, &id)?)
}

/// A MapLibre style JSON with every reference rewritten onto the geo://
/// protocol. `base` is the platform form of the protocol origin — the
/// frontend computes it (`geo://localhost/` here, `http://geo.localhost/` on
/// Windows) so the Rust side stays platform-blind.
///
/// `async`: the first call may fetch the upstream style + TileJSON.
#[tauri::command]
pub async fn get_map_style(
    geo: State<'_, state::GeoState>,
    style_id: String,
    base: String,
) -> CmdResult<String> {
    match style_id.as_str() {
        "openfreemap" => Ok(terrazgo_geo::style::openfreemap_style(&geo.conn, &base)?),
        "pnoa" => Ok(terrazgo_geo::style::pnoa_style(&base)),
        _ => Err(CommandError::from(terrazgo_geo::GeoError::NotFound)),
    }
}

/// List the selectable boundary candidates of a file the user picked (path
/// from the native open dialog). `async`: work scales with file size.
#[tauri::command]
pub async fn list_boundary_file(
    path: String,
) -> CmdResult<Vec<terrazgo_geo::import::BoundaryEntry>> {
    Ok(terrazgo_geo::import::list_boundary_file(Path::new(&path))?)
}

/// Load one candidate's geometry (validated GeoJSON) for preview/save.
#[tauri::command]
pub async fn read_boundary_feature(path: String, entry_id: String) -> CmdResult<String> {
    Ok(terrazgo_geo::import::read_boundary_geometry(
        Path::new(&path),
        &entry_id,
    )?)
}

// ---------------------------------------------------------------------------
// SIGPAC: the Spanish parcel provider (module-sigpac)
// ---------------------------------------------------------------------------

/// Look a typed 7-part reference up for form prefill (Door A). Stores
/// nothing; `None` = SIGPAC does not know the reference. `matching_plots`
/// warns when another plot already carries it. `async`: may hit the network.
#[tauri::command]
pub async fn sigpac_lookup_reference(
    state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
    parts: Vec<String>,
    refresh: bool,
) -> CmdResult<Option<module_sigpac::service::RecintoLookup>> {
    let conn = lock_conn(&state)?;
    Ok(module_sigpac::service::lookup_reference(
        &conn, &geo.conn, &parts, refresh,
    )?)
}

/// The recinto under a map click (Door B), with the plots already carrying
/// its reference so the UI offers attach-over-duplicate.
#[tauri::command]
pub async fn sigpac_lookup_point(
    state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
    lon: f64,
    lat: f64,
) -> CmdResult<Option<module_sigpac::service::RecintoLookup>> {
    let conn = lock_conn(&state)?;
    Ok(module_sigpac::service::lookup_point(
        &conn, &geo.conn, lon, lat,
    )?)
}

/// Verify a plot against SIGPAC using its stored reference and persist the
/// official boundary (`source='sigpac'`, replacing this source's previous
/// row) plus the zone checks (nitrate/phyto/Natura, folded in — decision
/// 2026-07-08). `None` = reference unknown to SIGPAC; nothing stored.
/// `refresh` bypasses the response cache (re-verification at rollover).
/// Zone flags feed the alert engine, so a refresh follows the write — the
/// shell chains the two modules (they never call each other).
#[tauri::command]
pub async fn sigpac_verify_plot(
    state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
    plot_id: String,
    refresh: bool,
) -> CmdResult<Option<module_sigpac::service::PlotVerification>> {
    let mut conn = lock_conn(&state)?;
    let verification =
        module_sigpac::service::verify_plot(&mut conn, &geo.conn, &plot_id, refresh)?;
    if verification
        .as_ref()
        .is_some_and(|v| v.zone_flags.is_some())
    {
        repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    }
    Ok(verification)
}

/// Active zone flags of a farm's plots — feeds the plot cards' zone chips.
#[tauri::command]
pub fn list_zone_flags(state: State<'_, AppState>, farm_id: String) -> CmdResult<Vec<ZoneFlag>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_zone_flags_for_farm(&conn, &farm_id)?)
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
/// debug-profile-conditional), so the guard is a runtime `cfg!` — release
/// builds refuse. Acceptable pre-release; revisit before first public release.
#[tauri::command]
pub fn seed_demo_data(state: State<'_, AppState>) -> CmdResult<DemoSeedSummary> {
    if cfg!(not(debug_assertions)) {
        return Err(CommandError(anyhow!(
            "demo seeding is disabled in release builds"
        )));
    }
    let mut conn = lock_conn(&state)?;
    let summary = module_cue::demo::seed_demo(&mut conn)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialized(err: CommandError) -> serde_json::Value {
        serde_json::to_value(&err).unwrap()
    }

    #[test]
    fn domain_error_maps_to_code_and_params() {
        let err = CommandError::from(module_cue::CueError::CountryMismatch {
            provided: "fr".into(),
            farm: "es".into(),
        });
        let value = serialized(err);
        assert_eq!(value["code"], "country_mismatch");
        assert_eq!(value["params"]["provided"], "fr");
        assert_eq!(value["params"]["farm"], "es");
        assert!(value["message"].as_str().unwrap().contains("fr"));
    }

    #[test]
    fn core_invalid_code_becomes_key_suffix() {
        let err = CommandError::from(terrazgo_core::CoreError::Invalid("empty_name"));
        assert_eq!(serialized(err)["code"], "invalid.empty_name");
    }

    #[test]
    fn non_domain_error_is_internal_with_message() {
        let err = CommandError(anyhow!("mutex is poisoned"));
        let value = serialized(err);
        assert_eq!(value["code"], "internal");
        assert_eq!(value["message"], "mutex is poisoned");
    }
}
