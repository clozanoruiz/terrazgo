// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The one way anything leaves the app for a browser.
//!
//! The allowlist and the reasoning live in `crate::external_links`; this file
//! is the thin boundary over it, per the house rule that a command carries no
//! logic.

use super::CmdResult;
use crate::external_links;
use tauri_plugin_opener::OpenerExt;

/// Open an allowlisted page in the platform browser.
///
/// The webview passes a `target` **id**, never a URL: Rust resolves it, which
/// is what lets `tauri-plugin-opener` be registered with no
/// `opener:allow-open-url` granted to the frontend. An id the allowlist does
/// not carry is a frontend bug rather than bad user input, but it still gets a
/// real code so it is visible in the notification bell instead of failing mute.
#[tauri::command]
pub fn open_external_link(app: tauri::AppHandle, target: String) -> CmdResult<()> {
    let url = external_links::url_for(&target)
        .ok_or(terrazgo_core::CoreError::Invalid("unknown_link"))?;
    // `None` for the app: let the platform pick the default browser rather
    // than naming one, which would be wrong on at least one desktop in three.
    app.opener().open_url(url, None::<&str>)?;
    Ok(())
}
