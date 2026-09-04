// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `module-ecoscheme`: RD 1048/2022's annotation duties — the
//! grazing, cultural-operation and soil-cover registers the printed model
//! renders as section 9.
//!
//! Thin wrappers, as everywhere at this boundary: the rules live in the
//! module's repository and are tested there.

use super::{CmdResult, active_actor};
use crate::state;
use crate::state::AppState;
use tauri::State;
use terrazgo_core::models::Lookup;

/// RD 1048/2022's six register-level annotation duties, which discriminate what
/// a record evidences. The whole list — which of them a holding may claim is a
/// fact about its solicitud única, unreachable by any route the app has
/// (docs/cuaderno-print.md), so the register forms narrow it rather than this.
#[tauri::command]
pub fn list_eco_practices(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::repository::list_eco_practices(conn)?)
}

/// What was done on the land (FEGA `TIPO_LABOR`, with its "Desbroce y siega"
/// split into the two columns model 9.4 prints).
#[tauri::command]
pub fn list_cultural_operation_kinds(state: State<'_, AppState>) -> CmdResult<Vec<Lookup>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::repository::list_cultural_operation_kinds(
        conn,
    )?)
}

/// The animals a grazing can name (FEGA `ESPECIE_ANIMAL`, 198 species). Takes a
/// country, so it is per-holding reference data and stays with the view that
/// knows the argument rather than joining the session-wide lookup store.
#[tauri::command]
pub fn list_animal_species(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_ecoscheme::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::catalogue::animal_species(
        conn,
        &country_code,
    )?)
}

/// Where a cultural operation's plant residue went (FEGA `DEST_RES_VEG`). No
/// page of the model prints it — it is the twin's field, and the destination
/// "trituración de restos de poda" is what turns a pruning into a P7 cover.
#[tauri::command]
pub fn list_residue_destinations(
    state: State<'_, AppState>,
    country_code: String,
) -> CmdResult<Vec<module_ecoscheme::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::catalogue::residue_destinations(
        conn,
        &country_code,
    )?)
}

// --- 9.1 pastoreo extensivo -------------------------------------------------
//
// RD 1048/2022 art. 30.2 ter: the grazing dates are annotated within one month
// of the new date, and the model counts that month from the END of grazing.

#[tauri::command]
pub fn list_grazing_records(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_ecoscheme::models::GrazingRecordDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::repository::list_grazing_records(
        conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_grazing_record(
    state: State<'_, AppState>,
    record: module_ecoscheme::models::NewGrazingRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::GrazingRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::insert_grazing_record(
        conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_grazing_record(
    state: State<'_, AppState>,
    grazing_record_id: String,
    update: module_ecoscheme::models::UpdateGrazingRecord,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::GrazingRecordDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::update_grazing_record(
        conn,
        &grazing_record_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_grazing_record(
    state: State<'_, AppState>,
    grazing_record_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_ecoscheme::repository::soft_delete_grazing_record(
        conn,
        &grazing_record_id,
        actor.as_deref(),
    )?;
    Ok(())
}

// --- 9.2 siega sostenible + "9.6" pastos comunales ---------------------------
//
// One register behind two printed pages, plus two more that later seams add.
// RD 1048/2022 art. 31 asks for "la fecha y las actividades realizadas"; anexo
// IV asks the same of each pasto comunal plot and the printed model gives it no
// page at all, which is why the book numbers one "9.6".

#[tauri::command]
pub fn list_cultural_operations(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_ecoscheme::models::CulturalOperationDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::repository::list_cultural_operations(
        conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_cultural_operation(
    state: State<'_, AppState>,
    record: module_ecoscheme::models::NewCulturalOperation,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::CulturalOperationDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::insert_cultural_operation(
        conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_cultural_operation(
    state: State<'_, AppState>,
    cultural_operation_id: String,
    update: module_ecoscheme::models::UpdateCulturalOperation,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::CulturalOperationDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::update_cultural_operation(
        conn,
        &cultural_operation_id,
        update,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn delete_cultural_operation(
    state: State<'_, AppState>,
    cultural_operation_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_ecoscheme::repository::soft_delete_cultural_operation(
        conn,
        &cultural_operation_id,
        actor.as_deref(),
    )?;
    Ok(())
}

/// What a cover is made of (FEGA `TIPO_COBERTURA_SUELO`). Country-taking, so it
/// is fetched per view rather than held in the session lookup store.
///
/// Narrowed to what the practice's own article names, so the form re-asks when
/// the farmer switches between a plant cover and an inert one.
#[tauri::command]
pub fn list_cover_types(
    state: State<'_, AppState>,
    country_code: String,
    practice_code: String,
) -> CmdResult<Vec<module_ecoscheme::catalogue::CataloguePick>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::catalogue::cover_types(
        conn,
        &country_code,
        &practice_code,
    )?)
}

#[tauri::command]
pub fn list_soil_covers(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<Vec<module_ecoscheme::models::SoilCoverDetail>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(module_ecoscheme::repository::list_soil_covers(
        conn, &season_id, &farm_id,
    )?)
}

#[tauri::command]
pub fn create_soil_cover(
    state: State<'_, AppState>,
    record: module_ecoscheme::models::NewSoilCover,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::SoilCoverDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::insert_soil_cover(
        conn,
        record,
        actor.as_deref(),
    )?)
}

#[tauri::command]
pub fn update_soil_cover(
    state: State<'_, AppState>,
    soil_cover_id: String,
    update: module_ecoscheme::models::UpdateSoilCover,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<module_ecoscheme::models::SoilCoverDetail> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(module_ecoscheme::repository::update_soil_cover(
        conn,
        &soil_cover_id,
        update,
        actor.as_deref(),
    )?)
}

/// Withdrawing a cover withdraws the maintenance recorded against it — art.
/// 42.1.c's annotation of that cover — each as its own audited soft delete.
#[tauri::command]
pub fn delete_soil_cover(
    state: State<'_, AppState>,
    soil_cover_id: String,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    module_ecoscheme::repository::soft_delete_soil_cover(conn, &soil_cover_id, actor.as_deref())?;
    Ok(())
}
