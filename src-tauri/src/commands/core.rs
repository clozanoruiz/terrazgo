// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `terrazgo-core`: the farm registry (farms, plots,
//! seasons, crops, operators, machinery, advisors), its lookups, water points,
//! geometry rows, harvest and user profiles.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError, active_actor, lock_conn, reconcile_alerts};
use crate::state;
use crate::state::AppState;
use anyhow::anyhow;
use tauri::State;
use terrazgo_core::date::today_utc;
use terrazgo_core::models::Advisor;
use terrazgo_core::models::Country;
use terrazgo_core::models::Crop;
use terrazgo_core::models::Farm;
use terrazgo_core::models::FarmAdvisor;
use terrazgo_core::models::FarmAdvisorDetail;
use terrazgo_core::models::FarmDetail;
use terrazgo_core::models::GeoFeature;
use terrazgo_core::models::Lookup;
use terrazgo_core::models::Machinery;
use terrazgo_core::models::MachineryDetail;
use terrazgo_core::models::NewAdvisor;
use terrazgo_core::models::NewCrop;
use terrazgo_core::models::NewFarm;
use terrazgo_core::models::NewGeoFeature;
use terrazgo_core::models::NewMachinery;
use terrazgo_core::models::NewOperator;
use terrazgo_core::models::NewPlot;
use terrazgo_core::models::NewSeason;
use terrazgo_core::models::NewUserProfile;
use terrazgo_core::models::NewWaterPoint;
use terrazgo_core::models::Operator;
use terrazgo_core::models::Plot;
use terrazgo_core::models::PlotDetail;
use terrazgo_core::models::Season;
use terrazgo_core::models::UpdateAdvisor;
use terrazgo_core::models::UpdateCrop;
use terrazgo_core::models::UpdateFarm;
use terrazgo_core::models::UpdateMachinery;
use terrazgo_core::models::UpdateOperator;
use terrazgo_core::models::UpdatePlot;
use terrazgo_core::models::UpdateSeason;
use terrazgo_core::models::UpdateUserProfile;
use terrazgo_core::models::UpdateWaterPoint;
use terrazgo_core::models::UserProfile;
use terrazgo_core::models::WaterDeclaration;
use terrazgo_core::models::WaterPoint;
use terrazgo_core::models::ZoneFlag;
use terrazgo_core::repository as core_repo;

// ---------------------------------------------------------------------------
// User profiles (managed from the Settings view)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_user_profiles(state: State<'_, AppState>) -> CmdResult<Vec<UserProfile>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_user_profiles(&conn)?)
}

#[tauri::command]
pub fn create_user_profile(
    state: State<'_, AppState>,
    profile: NewUserProfile,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<UserProfile> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_user_profile(
        &mut conn,
        profile,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_user_profile(
    state: State<'_, AppState>,
    profile_id: String,
    update: UpdateUserProfile,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<UserProfile> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_user_profile(
        &mut conn,
        &profile_id,
        update,
        actor.as_deref(),
    )?)
}

/// Soft-deletes the profile; if it was this device's active profile, the
/// setting is cleared too (the repository doesn't know about settings.json).
/// A failure to persist that clear is not fatal: the in-memory copy is
/// already cleared and a dangling id on next launch degrades to "no active
/// profile" by design.
#[tauri::command]
pub fn delete_user_profile(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    profile_id: String,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_user_profile(&mut conn, &profile_id, actor.as_deref())?;
    drop(conn);

    let mut guard = settings_state
        .settings
        .lock()
        .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
    if guard.active_user_id.as_deref() == Some(profile_id.as_str()) {
        guard.active_user_id = None;
        terrazgo_core::settings::save_settings(&settings_state.path, &guard)?;
    }
    Ok(())
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
pub fn create_farm(
    state: State<'_, AppState>,
    farm: NewFarm,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Farm> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_farm(&mut conn, farm, actor.as_deref())?)
}

#[tauri::command]
pub fn update_farm(
    state: State<'_, AppState>,
    farm_id: String,
    update: UpdateFarm,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<FarmDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_farm(
        &mut conn,
        &farm_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_farm(
    state: State<'_, AppState>,
    farm_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_farm(
        &mut conn,
        &farm_id,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn list_plots(state: State<'_, AppState>, farm_id: String) -> CmdResult<Vec<PlotDetail>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_plots(&conn, &farm_id)?)
}

#[tauri::command]
pub fn create_plot(
    state: State<'_, AppState>,
    plot: NewPlot,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Plot> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_plot(&mut conn, plot, actor.as_deref())?)
}

#[tauri::command]
pub fn update_plot(
    state: State<'_, AppState>,
    plot_id: String,
    update: UpdatePlot,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<PlotDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_plot(
        &mut conn,
        &plot_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_plot(
    state: State<'_, AppState>,
    plot_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_plot(
        &mut conn,
        &plot_id,
        actor.as_deref(),
    )?)
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
pub fn create_season(
    state: State<'_, AppState>,
    season: NewSeason,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Season> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_season(
        &mut conn,
        season,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_season(
    state: State<'_, AppState>,
    season_id: String,
    update: UpdateSeason,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Season> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_season(
        &mut conn,
        &season_id,
        update,
        actor.as_deref(),
    )?)
}

/// Delete a season created by mistake. Only an empty season may go: core checks
/// its own half (crops) and this command chains module-cue for the treatment
/// half, since core may never reference a module table. Both refusals surface as
/// the same `invalid.season_in_use`, so the frontend has one message to show.
#[tauri::command]
pub fn delete_season(
    state: State<'_, AppState>,
    season_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    // Every module that owns season-scoped records gets a say: hiding the
    // season would hide its register from a book that is read season by season.
    // Core checks its own tables inside `soft_delete_season`; it may never
    // reference a module's, which is why the chaining happens here.
    if module_cue::repository::season_has_records(&conn, &season_id)?
        || module_fertilisation::repository::season_has_records(&conn, &season_id)?
    {
        return Err(terrazgo_core::error::CoreError::Invalid("season_in_use").into());
    }
    Ok(core_repo::soft_delete_season(
        &mut conn,
        &season_id,
        actor.as_deref(),
    )?)
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
pub fn create_crop(
    state: State<'_, AppState>,
    crop: NewCrop,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Crop> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_crop(&mut conn, crop, actor.as_deref())?)
}

#[tauri::command]
pub fn update_crop(
    state: State<'_, AppState>,
    crop_id: String,
    update: UpdateCrop,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Crop> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_crop(
        &mut conn,
        &crop_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_crop(
    state: State<'_, AppState>,
    crop_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_crop(
        &mut conn,
        &crop_id,
        actor.as_deref(),
    )?)
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
    Ok(core_repo::list_units(&conn)?)
}

/// Units of amount, kept apart from the dose units above: they answer "how much
/// was used", not "at what rate".
#[tauri::command]
pub fn list_quantity_units(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_quantity_units(&conn)?)
}

/// Units a non-chemical measure's intensity is counted in (traps, diffusers).
/// A third list rather than a filter over the other two: a count is neither a
/// rate nor an amount of product.
#[tauri::command]
pub fn list_intensity_units(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_intensity_units(&conn)?)
}

/// Irrigation systems and shelter kinds for the crop form (model 2.1's
/// Secano/Regadío and Aire libre o protegido columns, Anexo III A.2.e).
#[tauri::command]
pub fn list_irrigation_systems(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_irrigation_systems(&conn)?)
}

#[tauri::command]
pub fn list_growing_environments(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_growing_environments(&conn)?)
}

#[tauri::command]
pub fn list_licence_levels(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_licence_levels(&conn)?)
}

#[tauri::command]
pub fn list_gip_systems(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_gip_systems(&conn)?)
}

// ---------------------------------------------------------------------------
// Advisors (official model 1.4) — the entity is farm-independent, the link
// carries the GIP framework one holding is advised under.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_advisors(state: State<'_, AppState>) -> CmdResult<Vec<Advisor>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_advisors(&conn)?)
}

#[tauri::command]
pub fn create_advisor(
    state: State<'_, AppState>,
    advisor: NewAdvisor,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Advisor> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_advisor(
        &mut conn,
        advisor,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_advisor(
    state: State<'_, AppState>,
    advisor_id: String,
    update: UpdateAdvisor,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Advisor> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_advisor(
        &mut conn,
        &advisor_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_advisor(
    state: State<'_, AppState>,
    advisor_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_advisor(
        &mut conn,
        &advisor_id,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn list_farm_advisors(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<FarmAdvisorDetail>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_farm_advisors(&conn, &farm_id)?)
}

/// Attach an advisor to a farm, or restate the framework of an existing link.
#[tauri::command]
pub fn set_farm_advisor(
    state: State<'_, AppState>,
    farm_id: String,
    advisor_id: String,
    gip_system_code: Option<String>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<FarmAdvisor> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::set_farm_advisor(
        &mut conn,
        &farm_id,
        &advisor_id,
        gip_system_code,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn remove_farm_advisor(
    state: State<'_, AppState>,
    link_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::remove_farm_advisor(
        &mut conn,
        &link_id,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn create_operator(
    state: State<'_, AppState>,
    operator: NewOperator,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Operator> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let operator = core_repo::insert_operator(&mut conn, operator, actor.as_deref())?;
    reconcile_alerts(&mut conn)?;
    Ok(operator)
}

#[tauri::command]
pub fn update_operator(
    state: State<'_, AppState>,
    operator_id: String,
    update: UpdateOperator,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Operator> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let operator = core_repo::update_operator(&mut conn, &operator_id, update, actor.as_deref())?;
    reconcile_alerts(&mut conn)?;
    Ok(operator)
}

#[tauri::command]
pub fn delete_operator(
    state: State<'_, AppState>,
    operator_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_operator(&mut conn, &operator_id, actor.as_deref())?;
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

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Machinery> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let machinery = core_repo::insert_machinery(&mut conn, machinery, actor.as_deref())?;
    reconcile_alerts(&mut conn)?;
    Ok(machinery)
}

#[tauri::command]
pub fn update_machinery(
    state: State<'_, AppState>,
    machinery_id: String,
    update: UpdateMachinery,

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<MachineryDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let detail = core_repo::update_machinery(&mut conn, &machinery_id, update, actor.as_deref())?;
    reconcile_alerts(&mut conn)?;
    Ok(detail)
}

#[tauri::command]
pub fn delete_machinery(
    state: State<'_, AppState>,
    machinery_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_machinery(&mut conn, &machinery_id, actor.as_deref())?;
    reconcile_alerts(&mut conn)?;
    Ok(())
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

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<GeoFeature> {
    let actor = active_actor(&settings_state)?;
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
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_geo_feature(
    state: State<'_, AppState>,
    id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_geo_feature(
        &mut conn,
        &id,
        actor.as_deref(),
    )?)
}

/// Active zone flags of a farm's plots — feeds the plot cards' zone chips.
#[tauri::command]
pub fn list_zone_flags(state: State<'_, AppState>, farm_id: String) -> CmdResult<Vec<ZoneFlag>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_zone_flags_for_farm(&conn, &farm_id)?)
}

/// Abstraction points for human consumption on a farm's plots — model 2.2's
/// water half. A farm asset rather than a season record, so this lives on the
/// farm view and takes no season.
#[tauri::command]
pub fn list_water_points(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<WaterPoint>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_water_points(&conn, &farm_id)?)
}

#[tauri::command]
pub fn create_water_point(
    state: State<'_, AppState>,
    water_point: NewWaterPoint,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<WaterPoint> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_water_point(
        &mut conn,
        water_point,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_water_point(
    state: State<'_, AppState>,
    water_point_id: String,
    update: UpdateWaterPoint,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<WaterPoint> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_water_point(
        &mut conn,
        &water_point_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_water_point(
    state: State<'_, AppState>,
    water_point_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::soft_delete_water_point(
        &mut conn,
        &water_point_id,
        actor.as_deref(),
    )?)
}

/// Standing "this plot has no abstraction point" declarations of a farm.
#[tauri::command]
pub fn list_water_declarations(
    state: State<'_, AppState>,
    farm_id: String,
) -> CmdResult<Vec<WaterDeclaration>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_water_declarations(&conn, &farm_id)?)
}

/// State or withdraw "this plot has no abstraction point" — one command, because
/// the panel offers it as a single checkbox and the two directions are the same
/// answer. Declaring is refused while the plot holds points; recording a point
/// withdraws the declaration on its own, so the UI never has to sequence them.
#[tauri::command]
pub fn set_water_declaration(
    state: State<'_, AppState>,
    plot_id: String,
    declared: bool,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Option<WaterDeclaration>> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    if declared {
        Ok(Some(core_repo::set_water_declaration(
            &mut conn,
            &plot_id,
            &today_utc(),
            actor.as_deref(),
        )?))
    } else {
        core_repo::clear_water_declaration(&mut conn, &plot_id, actor.as_deref())?;
        Ok(None)
    }
}

// --- commercialised harvest (model 5) ---------------------------------------
//
// Core-owned, unlike every other register in the book: what leaves the holding
// is whole-farm data, so these call `core_repo`, not the module.

#[tauri::command]
pub fn list_harvest_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<terrazgo_core::models::HarvestRecordDetail>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_harvest_records(
        &conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_harvest_record(
    state: State<'_, AppState>,
    record: terrazgo_core::models::NewHarvestRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<terrazgo_core::models::HarvestRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::insert_harvest_record(
        &mut conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_harvest_record(
    state: State<'_, AppState>,
    harvest_record_id: String,
    update: terrazgo_core::models::UpdateHarvestRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<terrazgo_core::models::HarvestRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    Ok(core_repo::update_harvest_record(
        &mut conn,
        &harvest_record_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_harvest_record(
    state: State<'_, AppState>,
    harvest_record_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    core_repo::soft_delete_harvest_record(&mut conn, &harvest_record_id, actor.as_deref())?;
    Ok(())
}

/// Cubic metres per hectare (Anexo III C.l) or plain cubic metres off a meter.
/// Kept apart from the dose and quantity lists: a volume of water answers its
/// own question.
#[tauri::command]
pub fn list_irrigation_volume_units(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_irrigation_volume_units(&conn)?)
}

/// The four rates Anexo III C.j's "por hectárea" can be stated in. Kept apart
/// from the dose list treatments use and from plain quantities: a fertiliser
/// dose is a rate, and "250 kg" answers a different question from "250 kg/ha".
#[tauri::command]
pub fn list_fertiliser_dose_units(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let conn = lock_conn(&state)?;
    Ok(core_repo::list_fertiliser_dose_units(&conn)?)
}
