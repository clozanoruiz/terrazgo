// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `terrazgo-geo`: map styles and boundary-file reads.
//! The tiles themselves never come through a command — they are served by the
//! `geo://` protocol handler.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError};
use crate::state;
use tauri::State;

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
/// or, on Android, content URI from the native open dialog). `async`: work
/// scales with file size.
#[tauri::command]
pub async fn list_boundary_file(
    app: tauri::AppHandle,
    path: String,
) -> CmdResult<Vec<terrazgo_geo::import::BoundaryEntry>> {
    let src = crate::user_files::stage_user_source(&app, &path)?;
    Ok(terrazgo_geo::import::list_boundary_file(src.path())?)
}

/// Load one candidate's geometry (validated GeoJSON) for preview/save.
#[tauri::command]
pub async fn read_boundary_feature(
    app: tauri::AppHandle,
    path: String,
    entry_id: String,
) -> CmdResult<String> {
    let src = crate::user_files::stage_user_source(&app, &path)?;
    Ok(terrazgo_geo::import::read_boundary_geometry(
        src.path(),
        &entry_id,
    )?)
}
