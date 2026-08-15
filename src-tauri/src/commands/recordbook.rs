// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Commands over `terrazgo-recordbook`: the printable record book
//! in both renderings, its language offer and its completeness advisory.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError, lock_conn};
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

/// What the printed record book is missing. Advisory only — it is deliberately
/// NOT wired into any export or print path, because a farmer must be able to
/// print for an inspection while some registry data is still incomplete.
#[tauri::command]
pub fn book_advisory(
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
) -> CmdResult<terrazgo_recordbook::BookAdvisory> {
    let conn = lock_conn(&state)?;
    Ok(terrazgo_recordbook::book_advisory(
        &conn, &season_id, &farm_id,
    )?)
}

#[derive(Serialize)]
pub struct CuadernoPdfSummary {
    pub path: String,
    pub size_bytes: u64,
    pub pages: usize,
}

/// The languages this holding's record book may be printed in, and which one
/// the chooser should start on given the language the app is speaking.
///
/// Castilian is offered everywhere; a co-official language appears when the
/// holding's provinces make it official (see `terrazgo_recordbook::region`).
#[derive(Serialize)]
pub struct ReportLanguagesInfo {
    pub languages: Vec<ReportLanguageOption>,
    /// Code of the language to preselect.
    pub default: String,
}

#[derive(Serialize)]
pub struct ReportLanguageOption {
    pub code: String,
    /// The language's name in itself — never translated, so the chooser reads
    /// the same whatever the UI language is.
    pub native_name: String,
}

#[tauri::command]
pub fn report_languages(
    state: State<'_, AppState>,
    farm_id: String,
    ui_locale: String,
) -> CmdResult<ReportLanguagesInfo> {
    let conn = lock_conn(&state)?;
    let available = terrazgo_recordbook::languages_for_farm(&conn, &farm_id)?;
    let default = terrazgo_recordbook::default_language(&available, &ui_locale);
    Ok(ReportLanguagesInfo {
        languages: available
            .iter()
            .map(|language| ReportLanguageOption {
                code: language.code().to_string(),
                native_name: language.native_name().to_string(),
            })
            .collect(),
        default: default.code().to_string(),
    })
}

/// A language code from the frontend, resolved to a language the book can
/// actually be printed in. An unknown code is a bug in the caller, not a
/// silent fallback: printing a legal document in the wrong language must fail
/// loudly.
fn report_language(code: &str) -> Result<terrazgo_recordbook::ReportLanguage, CommandError> {
    terrazgo_recordbook::ReportLanguage::from_code(code)
        .ok_or_else(|| module_cue::CueError::Invalid("report_language_unknown").into())
}

/// Render the printable cuaderno (official-model sections 1, 2.1 and 3.1)
/// for one farm+season and write the PDF to `dest_path` (chosen by the user
/// in the save dialog). No precheck: fields the model asks for but the data
/// lacks print blank — a farmer can always print the current state. `async`
/// like the other export: rendering scales with record count.
#[tauri::command]
pub async fn export_cuaderno_pdf(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
    dest_path: String,
    language: String,
) -> CmdResult<CuadernoPdfSummary> {
    let language = report_language(&language)?;
    let guard = lock_conn(&state)?;
    let today = terrazgo_core::date::now_utc_iso();
    let generated_on = today.split('T').next().unwrap_or(&today);
    let pdf =
        terrazgo_recordbook::render_cuaderno(&guard, &season_id, &farm_id, generated_on, language)?;
    crate::user_files::write_user_file(&app, &dest_path, &pdf.bytes)?;
    Ok(CuadernoPdfSummary {
        path: dest_path,
        size_bytes: pdf.bytes.len() as u64,
        pages: pdf.page_count,
    })
}

#[derive(Serialize)]
pub struct CuadernoXlsxSummary {
    pub path: String,
    pub size_bytes: u64,
    pub sheets: usize,
}

/// The same record book as a spreadsheet: one sheet per section of the
/// official model, with real dates and numbers so the farmer (or their
/// gestoría) can sort, filter and sum. Same no-precheck rule as the PDF, and
/// `async` for the same reason — the work scales with record count.
#[tauri::command]
pub async fn export_cuaderno_xlsx(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    season_id: String,
    farm_id: String,
    dest_path: String,
    language: String,
) -> CmdResult<CuadernoXlsxSummary> {
    let language = report_language(&language)?;
    let guard = lock_conn(&state)?;
    let today = terrazgo_core::date::now_utc_iso();
    let generated_on = today.split('T').next().unwrap_or(&today);
    let book = terrazgo_recordbook::render_cuaderno_xlsx(
        &guard,
        &season_id,
        &farm_id,
        generated_on,
        language,
    )?;
    crate::user_files::write_user_file(&app, &dest_path, &book.bytes)?;
    Ok(CuadernoXlsxSummary {
        path: dest_path,
        size_bytes: book.bytes.len() as u64,
        sheets: book.sheet_count,
    })
}
