// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `module-fertilisation`: RD 1051/2022's half of
//! the book — fertiliser materials, fertilisation records, the plan de abonado
//! and irrigation.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, active_actor};
use crate::state;
use crate::state::AppState;
use tauri::State;
use terrazgo_core::models::Lookup;

// --- irrigation (model 8) ---------------------------------------------------
//
// RD 1051/2022 art. 5.e: the doses and dates of irrigation belong to the same
// cuaderno duty as fertilisation, on the same one-month deadline. These wrap
// module-fertilisation, whose repository owns the rules.

#[tauri::command]
pub fn list_irrigation_methods(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_irrigation_methods(
        conn,
    )?)
}

#[tauri::command]
pub fn list_water_origins(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_water_origins(conn)?)
}

#[tauri::command]
pub fn list_irrigation_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_fertilisation::models::IrrigationRecordDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_irrigation_records(
        conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_irrigation_record(
    state: State<'_, AppState>,
    record: module_fertilisation::models::NewIrrigationRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::IrrigationRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_fertilisation::repository::insert_irrigation_record(
        conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_irrigation_record(
    state: State<'_, AppState>,
    irrigation_record_id: String,
    update: module_fertilisation::models::UpdateIrrigationRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::IrrigationRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_fertilisation::repository::update_irrigation_record(
        conn,
        &irrigation_record_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_irrigation_record(
    state: State<'_, AppState>,
    irrigation_record_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_fertilisation::repository::soft_delete_irrigation_record(
        conn,
        &irrigation_record_id,
        actor.as_deref(),
    )?;
    Ok(())
}

// --- fertilisation (model 6) ------------------------------------------------
//
// RD 1051/2022 art. 5.d, binding since 1 January 2026 and recorded within one
// month of each operation. The binding field list is RD 1311/2012 Anexo III
// Parte I sección C, which is wider than the printed model; module-fertilisation
// owns the rules, these are the transport.

/// Anexo III C.c — fondo, cobertera or enmienda. Not the model's
/// "(F)/(AF)/(AC)" list: fertigation is a method, not a type.
#[tauri::command]
pub fn list_fertilisation_types(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_fertilisation_types(
        conn,
    )?)
}

/// Anexo III C.f — how it was applied, fertigation included.
#[tauri::command]
pub fn list_application_methods(
    state: State<'_, AppState>,
) -> CmdResult<Vec<module_fertilisation::models::ApplicationMethod>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_application_methods(
        conn,
    )?)
}

/// What the manure received before it was spread — a property of the material,
/// so it fills a field on the registry form.
#[tauri::command]
pub fn list_manure_treatments(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_manure_treatments(
        conn,
    )?)
}

/// Which of the three FEGA nutrient catalogues a composition line indexes.
#[tauri::command]
pub fn list_nutrient_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_nutrient_kinds(conn)?)
}

/// Anexo III C.d's first level: the kind of material (FEGA `MAT_FERTI`).
#[tauri::command]
pub fn list_fertiliser_material_kinds(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_fertilisation::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::catalogue::fertiliser_materials(
        conn,
        &country_code,
    )?)
}

/// C.d's second level: the named commercial product, narrowed by the chosen
/// kind (1042 of the catalogue's 1243 rows sit under "abonos inorgánicos"
/// alone, so an unnarrowed picker is unusable).
#[tauri::command]
pub fn list_fertiliser_material_details(
    state: State<'_, AppState>,
    country_code: String,
    material_code: Option<String>,
) -> CmdResult<Vec<module_fertilisation::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(
        module_fertilisation::catalogue::fertiliser_material_details(
            conn,
            &country_code,
            material_code.as_deref(),
        )?,
    )
}

/// What the catalogue publishes about one named product's composition, offered
/// to the material form so Anexo III C.h's eight values need not be copied off
/// the sack by hand.
///
/// A proposal, never a record: the form applies it explicitly and the farmer
/// may edit or drop any line. Heavy metals are never proposed — the provider's
/// columns mix percentages and mg/kg with nothing to tell them apart (see
/// `module_fertilisation::catalogue`).
#[tauri::command]
pub fn fertiliser_material_composition(
    state: State<'_, AppState>,
    country_code: String,
    detail_code: String,
) -> CmdResult<Vec<module_fertilisation::catalogue::CompositionLine>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::catalogue::material_composition(
        conn,
        &country_code,
        &detail_code,
    )?)
}

/// The nutrients, micronutrients or heavy metals a composition line can name.
/// Three separate catalogues sharing a number space, which is why the kind
/// travels with the code.
#[tauri::command]
pub fn list_nutrient_codes(
    state: State<'_, AppState>,
    country_code: String,
    kind_code: String,
) -> CmdResult<Vec<module_fertilisation::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::catalogue::nutrients(
        conn,
        &country_code,
        &kind_code,
    )?)
}

/// The good practices a fertilisation record can claim, filtered to the
/// "Fertilización" ámbito — the catalogue holds three vocabularies in one file
/// and the same integer means a different practice in each.
#[tauri::command]
pub fn list_fertilisation_practices(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_fertilisation::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::catalogue::fertilisation_practices(
        conn,
        &country_code,
    )?)
}

#[tauri::command]
pub fn list_fertiliser_materials(
    state: State<'_, AppState>,
) -> CmdResult<Vec<module_fertilisation::models::FertiliserMaterialDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_fertiliser_materials(
        conn,
    )?)
}

#[tauri::command]
pub fn create_fertiliser_material(
    state: State<'_, AppState>,
    material: module_fertilisation::models::NewFertiliserMaterial,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertiliserMaterialDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(
        module_fertilisation::repository::insert_fertiliser_material(
            conn,
            material,
            actor.as_deref(),
        )?,
    )
}

#[tauri::command]
pub fn update_fertiliser_material(
    state: State<'_, AppState>,
    material_id: String,
    update: module_fertilisation::models::UpdateFertiliserMaterial,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertiliserMaterialDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(
        module_fertilisation::repository::update_fertiliser_material(
            conn,
            &material_id,
            update,
            actor.as_deref(),
        )?,
    )
}

#[tauri::command]
pub fn delete_fertiliser_material(
    state: State<'_, AppState>,
    material_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_fertilisation::repository::soft_delete_fertiliser_material(
        conn,
        &material_id,
        actor.as_deref(),
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_fertilisation_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_fertilisation::models::FertilisationRecordDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_fertilisation_records(conn, &season_id, &farm_id)?)
}

#[tauri::command]
pub fn create_fertilisation_record(
    state: State<'_, AppState>,
    record: module_fertilisation::models::NewFertilisationRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertilisationRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(
        module_fertilisation::repository::insert_fertilisation_record(
            conn,
            record,
            actor.as_deref(),
        )?,
    )
}

#[tauri::command]
pub fn update_fertilisation_record(
    state: State<'_, AppState>,
    fertilisation_record_id: String,
    update: module_fertilisation::models::UpdateFertilisationRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertilisationRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(
        module_fertilisation::repository::update_fertilisation_record(
            conn,
            &fertilisation_record_id,
            update,
            actor.as_deref(),
        )?,
    )
}

#[tauri::command]
pub fn delete_fertilisation_record(
    state: State<'_, AppState>,
    fertilisation_record_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_fertilisation::repository::soft_delete_fertilisation_record(
        conn,
        &fertilisation_record_id,
        actor.as_deref(),
    )?;
    Ok(())
}

// --- the plan de abonado (model 7.1) ----------------------------------------
//
// RD 1051/2022 art. 4.2 requires a plan per production unit from 1 September
// 2026; art. 6 says what the plan document must contain and art. 5.a what goes
// in the book. These carry art. 5.a.

#[tauri::command]
pub fn list_fertilisation_plans(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_fertilisation::models::FertilisationPlanDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_fertilisation::repository::list_fertilisation_plans(
        conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_fertilisation_plan(
    state: State<'_, AppState>,
    plan: module_fertilisation::models::NewFertilisationPlan,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertilisationPlanDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_fertilisation::repository::insert_fertilisation_plan(
        conn,
        plan,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_fertilisation_plan(
    state: State<'_, AppState>,
    plan_id: String,
    update: module_fertilisation::models::UpdateFertilisationPlan,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_fertilisation::models::FertilisationPlanDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_fertilisation::repository::update_fertilisation_plan(
        conn,
        &plan_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_fertilisation_plan(
    state: State<'_, AppState>,
    plan_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_fertilisation::repository::soft_delete_fertilisation_plan(
        conn,
        &plan_id,
        actor.as_deref(),
    )?;
    Ok(())
}
