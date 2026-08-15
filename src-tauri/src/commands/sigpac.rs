// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `module-sigpac`: parcel lookups, plot verification
//! and the reviewed crop-declaration import.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, active_actor, lock_conn};
use crate::state;
use crate::state::AppState;
use module_cue::alerts::AlertConfig;
use module_cue::repository;
use serde::Deserialize;
use serde::Serialize;
use tauri::State;
use terrazgo_core::date::today_utc;
use terrazgo_core::models::NewCrop;
use terrazgo_core::models::UpdateCrop;
use terrazgo_core::repository as core_repo;

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

    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<Option<module_sigpac::service::PlotVerification>> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let verification = module_sigpac::service::verify_plot(
        &mut conn,
        &geo.conn,
        &plot_id,
        refresh,
        actor.as_deref(),
    )?;
    if verification
        .as_ref()
        .is_some_and(|v| v.zone_flags.is_some())
    {
        repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    }
    Ok(verification)
}

/// What the PAC declaration says grows on this farm's plots, diffed against
/// the crops already recorded this season. Read-only: nothing is written until
/// the farmer confirms rows through `sigpac_accept_crop_proposals`.
///
/// The shell supplies the guard input — which crops this season's treatments
/// already point at — because it comes from the CUE module and the two modules
/// never call each other (the `sigpac_verify_plot` precedent).
#[tauri::command]
pub async fn sigpac_propose_crops(
    state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
    farm_id: String,
    season_id: String,
    refresh: bool,
) -> CmdResult<module_sigpac::service::CropProposals> {
    let conn = lock_conn(&state)?;
    let treated = repository::crop_ids_with_treatments(&conn, &season_id, &farm_id)?;
    Ok(module_sigpac::service::propose_crops(
        &conn, &geo.conn, &farm_id, &season_id, &treated, refresh,
    )?)
}

/// One accepted proposal row: a new crop the farmer reviewed and edited.
#[derive(Debug, Deserialize)]
pub struct AcceptedCropInsert {
    pub crop: NewCrop,
}

/// One accepted proposal row that restates an existing crop.
#[derive(Debug, Deserialize)]
pub struct AcceptedCropUpdate {
    pub crop_id: String,
    pub update: UpdateCrop,
}

/// A row the import declined to apply after all, and why.
#[derive(Debug, Serialize)]
pub struct SkippedCropRow {
    pub species_name: String,
    pub reason: &'static str,
}

/// What an import run did.
#[derive(Debug, Serialize)]
pub struct CropImportSummary {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: Vec<SkippedCropRow>,
}

/// Apply the proposal rows the farmer accepted, after re-checking the two
/// things that may have changed since the panel was built.
///
/// The guards are re-run rather than trusted from the panel: a treatment
/// recorded in another window, or a second confirmation of the same panel,
/// would otherwise slip past them. A row that fails a guard is reported as
/// skipped, not raised as an error — the rest of the import is good work, and
/// the farmer needs to see which rows did not land.
#[tauri::command]
pub fn sigpac_accept_crop_proposals(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    farm_id: String,
    season_id: String,
    inserts: Vec<AcceptedCropInsert>,
    updates: Vec<AcceptedCropUpdate>,
) -> CmdResult<CropImportSummary> {
    let actor = active_actor(&settings_state)?;
    let mut conn = lock_conn(&state)?;
    let treated = repository::crop_ids_with_treatments(&conn, &season_id, &farm_id)?;
    let mut existing = core_repo::list_crops(&conn, &season_id, &farm_id)?;

    let mut summary = CropImportSummary {
        inserted: 0,
        updated: 0,
        skipped: Vec::new(),
    };

    for accepted in inserts {
        let duplicate = accepted.crop.crop_code.as_deref().is_some_and(|code| {
            existing.iter().any(|crop| {
                crop.plot_id == accepted.crop.plot_id && crop.crop_code.as_deref() == Some(code)
            })
        });
        if duplicate {
            summary.skipped.push(SkippedCropRow {
                species_name: accepted.crop.species_name,
                reason: "already_recorded",
            });
            continue;
        }
        let crop = core_repo::insert_crop(&mut conn, accepted.crop, actor.as_deref())?;
        existing.push(crop);
        summary.inserted += 1;
    }

    for accepted in updates {
        if treated.contains(&accepted.crop_id) {
            summary.skipped.push(SkippedCropRow {
                species_name: accepted.update.species_name,
                reason: "has_treatments",
            });
            continue;
        }
        core_repo::update_crop(
            &mut conn,
            &accepted.crop_id,
            accepted.update,
            actor.as_deref(),
        )?;
        summary.updated += 1;
    }

    Ok(summary)
}

/// The crop species the manual form's picker offers, from the vendored FEGA
/// catalogue. With a plot, the list narrows to what its verified SIGPAC land
/// use plausibly grows; without one — or when nothing matches — every species
/// is offered. Offline always: the catalogues ship in the binary.
#[tauri::command]
pub fn list_crop_species(
    state: State<'_, AppState>,
    plot_id: Option<String>,
) -> CmdResult<module_sigpac::service::SpeciesCatalogue> {
    let conn = lock_conn(&state)?;
    Ok(module_sigpac::service::crop_species(
        &conn,
        plot_id.as_deref(),
    )?)
}
