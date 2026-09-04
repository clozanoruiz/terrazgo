// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell's own surface: readiness, platform, status, settings,
//! reference-catalogue maintenance and backups.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError, alert_config, module_backup_shape};
use crate::state;
use crate::state::AppState;
use anyhow::anyhow;
use module_cue::repository;
use serde::Serialize;
use std::path::Path;
use tauri::State;
use terrazgo_core::catalogue::CatalogueStatus;
use terrazgo_core::catalogue::RefreshReport;
use terrazgo_core::date::today_utc;
use terrazgo_core::settings::AppSettings;

#[derive(Serialize)]
pub struct AppStatus {
    pub db_path: String,
    pub schema_version: usize,
    pub app_version: &'static str,
    /// What the last corruption check found, or `None` if one has not run yet
    /// (a fresh install, or a launch where the weekly check was not yet due and
    /// none had ever run). The Status view warns only when this says `ok:
    /// false` — a healthy database says nothing, the way the rest of the app
    /// reports only what needs attention.
    pub integrity: Option<terrazgo_core::settings::IntegrityCheck>,
}

/// Readiness probe for the startup race on Android (see
/// `state::SetupComplete`). Deliberately takes `AppHandle`, never `State`:
/// it must be callable before setup has managed anything.
#[tauri::command]
pub fn app_ready(app: tauri::AppHandle) -> bool {
    use tauri::Manager;
    app.try_state::<state::SetupComplete>().is_some()
}

/// Compile-time platform truth for the frontend: mobile builds carry the
/// geolocation plugin, desktop builds never do. The frontend must gate the
/// GPS controls on this, NOT on probing a plugin command — the plugin
/// rejects `check_permissions` when the device's location services are off,
/// so a rejection does not mean absence (learned on-device, 2026-07-23).
#[tauri::command]
pub fn is_mobile() -> bool {
    cfg!(mobile)
}

#[tauri::command]
pub fn get_status(
    state: State<'_, AppState>,
    settings: State<'_, state::SettingsState>,
) -> CmdResult<AppStatus> {
    Ok(AppStatus {
        db_path: state.db_path.display().to_string(),
        schema_version: state.schema_version,
        app_version: env!("CARGO_PKG_VERSION"),
        integrity: settings
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?
            .last_integrity_check
            .clone(),
    })
}

/// The versions the About panel prints, so a bug report carries them.
#[derive(Serialize)]
pub struct AboutInfo {
    pub app_version: &'static str,
    pub tauri_version: &'static str,
    /// The webview engine's real version — WebKitGTK on Linux, WebView2 on
    /// Windows, WKWebView on macOS, the system WebView on Android. `None` when
    /// the platform cannot answer, which the panel prints as a dash rather
    /// than inventing a number.
    ///
    /// **Not the same fact as the user agent**, which the panel shows beside
    /// it: WebKitGTK's UA reports a frozen Safari-compatibility version
    /// (`AppleWebKit/605.1.15 … Version/60.5`) that tracks nothing — measured
    /// 2026-08-26 against a real engine reporting 2.52.3.
    pub webview_version: Option<String>,
    /// Which engine that version belongs to — "2.52.3" alone says nothing.
    /// Compile-time, because the webview a build links is not a runtime choice.
    pub webview_engine: &'static str,
    /// The SQLite compiled into the binary (`bundled`), which is the one that
    /// wrote the record book — not whatever the system happens to ship.
    pub sqlite_version: &'static str,
    /// The running system, e.g. "Ubuntu 24.04". `std::env::consts::OS` is a
    /// compile-time constant and would only ever say "linux".
    pub os: String,
    pub arch: &'static str,
    /// The project's own page, for the About panel to PRINT beside the title.
    /// Read from the same allowlist `open_external_link` resolves against, so
    /// the address shown and the address opened cannot drift apart — and the
    /// webview still names a link by id when it wants one opened.
    pub homepage_url: &'static str,
}

/// Versions for the About panel.
///
/// **Deliberately separate from `get_status`, and the reason is Android.**
/// `tauri::webview_version()` is implemented there by spinning
/// `loop { first_activity_id(); sleep(100ms) }` and then blocking on the main
/// pipe (wry's `android/mod.rs`), so it must never sit on the startup path —
/// the hazard the startup-ordering fix exists for (docs/architecture.md → "On
/// Android the webview starts first"). Keeping it in its own command confines
/// the blocking call to a panel that can only be reached by tapping a button
/// inside a webview that is therefore provably already up.
#[tauri::command]
pub fn get_about_info() -> AboutInfo {
    let info = os_info::get();
    let version = info.version().to_string();
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        tauri_version: tauri::VERSION,
        webview_version: tauri::webview_version().ok(),
        webview_engine: if cfg!(target_os = "windows") {
            "WebView2"
        } else if cfg!(any(target_os = "macos", target_os = "ios")) {
            "WKWebView"
        } else if cfg!(target_os = "android") {
            "Android WebView"
        } else {
            "WebKitGTK"
        },
        sqlite_version: rusqlite::version(),
        homepage_url: crate::external_links::url_for("homepage").unwrap_or_default(),
        // os_info prints "Unknown" for a version it could not determine, which
        // reads as a claim; the OS name alone is the honest answer there.
        os: if version == "Unknown" {
            info.os_type().to_string()
        } else {
            format!("{} {version}", info.os_type())
        },
        arch: std::env::consts::ARCH,
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Settings plus the code-owned defaults the UI needs to render "unset"
/// meaningfully: an unset cache cap displays the default value, not a blank,
/// and the frontend must not hardcode a copy of the constant.
///
/// Every default here is read from its owning crate rather than repeated, so
/// moving one moves what the UI shows in the same commit.
#[derive(Serialize)]
pub struct SettingsInfo {
    pub settings: AppSettings,
    pub tile_cache_default_bytes: i64,
    pub licence_lead_default_days: i64,
    pub itv_lead_default_days: i64,
    pub phi_recent_default_days: i64,
}

fn settings_info(settings: AppSettings) -> SettingsInfo {
    // The one sanctioned reach for module-cue's own defaults: this REPORTS
    // them so the UI can label an unset field with its effective value. It is
    // not a resolved config — that comes only from `alert_config`, and passing
    // this to `refresh_alerts` would be the bug the missing `Default` guards.
    let alerts = module_cue::alerts::AlertConfig::defaults();
    SettingsInfo {
        settings,
        tile_cache_default_bytes: terrazgo_geo::db::TILE_CACHE_MAX_BYTES,
        licence_lead_default_days: alerts.licence_lead_days,
        itv_lead_default_days: alerts.itv_lead_days,
        phi_recent_default_days: repository::default_phi_horizon_days(),
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
/// owning crate (the cache cap is range-checked by terrazgo-geo, the alert lead
/// times by module-cue); the file is written before the in-memory copy so a
/// failed save never leaves them disagreeing.
///
/// **Both changed settings act immediately**, which is the same promise for a
/// different reason each time: shrinking the cache must visibly free space, and
/// a lead time that only took effect after the farmer's next write would read
/// as broken.
///
/// `async` because that enforcement can VACUUM a multi-hundred-MB file
/// (seconds); the body stays synchronous — no `.await`, so holding the state
/// guards is safe.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, state::SettingsState>,
    app_state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
    settings: AppSettings,
) -> CmdResult<SettingsInfo> {
    if let Some(bytes) = settings.tile_cache_max_bytes {
        terrazgo_geo::db::validate_tile_cache_cap(bytes)?;
    }
    for days in [settings.licence_lead_days, settings.itv_lead_days]
        .into_iter()
        .flatten()
    {
        module_cue::alerts::validate_lead_days(days)?;
    }
    if let Some(days) = settings.phi_recent_days {
        repository::validate_phi_horizon_days(days)?;
    }

    terrazgo_core::settings::save_settings(&state.path, &settings)?;
    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
        *guard = settings.clone();
    }

    // Scoped so the geo lock is RELEASED before the app database is locked
    // below. The rest of the app takes those two in the opposite order
    // (`sigpac_verify_plot` holds the app connection and reaches into the
    // cache), and holding both the other way round is a genuine deadlock.
    {
        let cap = settings
            .tile_cache_max_bytes
            .unwrap_or(terrazgo_geo::db::TILE_CACHE_MAX_BYTES);
        let cache = geo.cache.lock()?;
        terrazgo_geo::db::enforce_tile_cache_cap(cache.conn()?, cap)?;
    }

    // New lead times reach the alert table now rather than at the next write.
    // Built from the submitted settings rather than re-read through
    // `alert_config`, so this cannot race the in-memory copy it just wrote.
    let config = module_cue::alerts::AlertConfig::from_overrides(
        settings.licence_lead_days,
        settings.itv_lead_days,
    );
    {
        let mut db = app_state.db.lock()?;
        repository::refresh_alerts(db.conn_mut()?, &today_utc(), &config)?;
    }

    Ok(settings_info(settings))
}

/// Empty the tile cache, keeping `resource` rows (styles, glyphs, SIGPAC
/// lookup/zone responses — a verified plot stays verifiable offline). Returns
/// the number of tiles dropped, for the notification. `async` for the VACUUM,
/// same reasoning as `update_settings`.
#[tauri::command]
pub async fn clear_tile_cache(geo: State<'_, state::GeoState>) -> CmdResult<usize> {
    let cache = geo.cache.lock()?;
    Ok(terrazgo_geo::db::clear_tile_cache(cache.conn()?)?)
}

// ---------------------------------------------------------------------------
// Database maintenance
// ---------------------------------------------------------------------------

/// What one press of "check and compact" did.
#[derive(Serialize)]
pub struct MaintenanceReport {
    pub integrity: terrazgo_core::settings::IntegrityCheck,
    /// Logical size before and after. Equal when the check failed, because
    /// nothing was rewritten — which is how the UI knows not to claim it freed
    /// anything.
    pub size_before_bytes: i64,
    pub size_after_bytes: i64,
    pub compacted: bool,
}

/// Check the database thoroughly and, only if it is sound, compact it.
///
/// **One command rather than two buttons, because the check has to gate the
/// compaction.** `VACUUM` rebuilds the file by reading every page and writing a
/// fresh one; run on a damaged database that entrenches the damage into the new
/// copy instead of revealing it. So a failed check stops here with the file
/// untouched, and the farmer is told to restore a backup.
///
/// A bad verdict is an OUTCOME and not an error: it comes back as `Ok` with
/// `integrity.ok == false`, the way a refused catalogue refresh does. Failing
/// the command would leave the farmer with an error message instead of an
/// answer to the question they asked.
///
/// `async` for the reason `export_backup` is: both halves scale with file size
/// and would freeze the window on the main thread. The body stays synchronous —
/// no `.await`, so holding the guards is safe.
#[tauri::command]
pub async fn check_and_compact_database(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<MaintenanceReport> {
    // The database lock is taken and RELEASED before the settings lock below.
    // The invariant everything here obeys is that no thread ever holds both at
    // once — `active_actor` gets there by reading settings first and releasing,
    // this by finishing with the database first. Either order is safe; holding
    // both is not.
    let (integrity, size_before_bytes, size_after_bytes) = {
        let db = state.db.lock()?;
        let conn = db.conn()?;
        let before = crate::db::database_bytes(conn)?;
        let integrity = crate::db::integrity_check(conn);
        let after = if integrity.ok {
            conn.execute_batch("VACUUM")?;
            crate::db::database_bytes(conn)?
        } else {
            before
        };
        (integrity, before, after)
    };

    {
        let mut guard = settings_state
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
        guard.last_integrity_check = Some(integrity.clone());
        terrazgo_core::settings::save_settings(&settings_state.path, &guard)?;
    }

    Ok(MaintenanceReport {
        compacted: integrity.ok,
        integrity,
        size_before_bytes,
        size_after_bytes,
    })
}

// ---------------------------------------------------------------------------
// Reference catalogues
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn catalogue_status(state: State<'_, AppState>) -> CmdResult<Vec<CatalogueStatus>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(terrazgo_core::catalogue::catalogue_status(conn)?)
}

/// Fetch every vendored catalogue from the provider and adopt the ones that
/// pass validation, reporting per file.
///
/// `async` because it does network work over dozens of files (the
/// long-running-command rule); the body stays synchronous, so holding the
/// connection guard is safe.
///
/// Two ordering rules, both deliberate. **The connection lock is taken per
/// file, after that file's bytes have arrived** — never across the network,
/// so the rest of the app keeps answering while a refresh runs (the geo-cache
/// precedent). And **a failure is always a per-file refusal**, never an early
/// return: one retired idTabla or one truncated download must not deny the
/// user the other 46 catalogues' updates.
#[tauri::command]
pub async fn refresh_catalogues(state: State<'_, AppState>) -> CmdResult<Vec<RefreshReport>> {
    let mut reports = Vec::new();
    for id in terrazgo_core::catalogue::vendored_ids() {
        let bytes = match crate::catalogues::fetch_catalogue(id) {
            Ok(bytes) => bytes,
            Err(refusal) => {
                reports.push(refusal);
                continue;
            }
        };
        let mut db = state.db.lock()?;
        let conn = db.conn_mut()?;
        reports.push(terrazgo_core::catalogue::refresh_catalogue(
            conn, id, &bytes,
        )?);
    }
    Ok(reports)
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dest_path: String,
) -> CmdResult<terrazgo_core::backup::BackupSummary> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    match crate::user_files::stage_dest(&app, &dest_path)? {
        None => Ok(terrazgo_core::backup::export_backup(
            conn,
            Path::new(&dest_path),
        )?),
        // Android content URI: `VACUUM INTO` needs a real filesystem path, so
        // the verified snapshot lands in a private staging file and is then
        // streamed to the user's chosen document.
        Some(staging) => {
            let summary = terrazgo_core::backup::export_backup(conn, staging.path())?;
            crate::user_files::copy_to_user_file(&app, staging.path(), &dest_path)?;
            Ok(terrazgo_core::backup::BackupSummary {
                path: dest_path,
                ..summary
            })
        }
    }
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
/// live connection, copy the backup over the live path, reopen through the
/// composed migration runner and refresh alerts. The lock is held across all of
/// it, so nothing reaches the database mid-swap. If reopening fails midway the
/// slot stays empty — commands report a closed database until restart — but the
/// previous data is already safe in the pre-import copy.
/// `async` for the same reason as `export_backup`: validate + safety copy +
/// file swap take time proportional to database size and must not block the
/// main thread (no `.await` inside, so holding the mutex guard is safe).
#[tauri::command]
pub async fn import_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    src_path: String,
) -> CmdResult<ImportSummary> {
    // Android content URI: staged into a private copy so validation and the
    // file swap below work on a real path. Plain paths pass through as-is.
    let src = crate::user_files::stage_user_source(&app, &src_path)?;
    let config = alert_config(&settings_state)?;
    let mut db = state.db.lock()?;

    // The live db is always at the latest composed version, so it IS the
    // ceiling of what this build supports.
    let live_version: i64 = db
        .conn()?
        .pragma_query_value(None, "user_version", |r| r.get(0))?;
    // The shape probe spans core AND every registered module, the same way the
    // migration sequence does — core cannot name a module's tables itself.
    let info =
        terrazgo_core::backup::validate_backup(src.path(), live_version, &module_backup_shape())?;

    let backups_dir = state
        .db_path
        .parent()
        .ok_or_else(|| CommandError(anyhow!("database path has no parent directory")))?
        .join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    // ISO instant with the filename-hostile characters stripped: 20260702T101500Z.
    let stamp: String = today_utc_instant().replace(['-', ':'], "");
    let safety_path = backups_dir.join(format!("pre-import-{stamp}.db"));
    terrazgo_core::backup::export_backup(db.conn()?, &safety_path)?;

    // Swap. Closing takes the WAL sidecars with it, so the copy below lands on
    // a path with nothing stale beside it — and a close that FAILS aborts here,
    // before the live file is overwritten, with the safety copy already made.
    db.close()?;
    std::fs::copy(src.path(), &state.db_path)?;

    let mut conn = crate::db::open_app_db(&state.db_path)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &config)?;
    db.replace(conn);

    Ok(ImportSummary {
        schema_version_found: info.schema_version,
        safety_backup_path: safety_path.display().to_string(),
    })
}

/// Full UTC instant (not just the date) for unique backup filenames.
fn today_utc_instant() -> String {
    terrazgo_core::date::now_utc_iso()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field must carry something a reader can act on. The failure this
    /// guards is silent: `os_info` returning an empty or "Unknown" string, or a
    /// version constant that stops resolving, would render a blank row in a
    /// panel whose whole purpose is to be pasted into a bug report.
    #[test]
    fn about_info_reports_every_version_it_promises() {
        let info = get_about_info();
        assert!(!info.app_version.is_empty());
        assert!(!info.tauri_version.is_empty());
        assert!(!info.sqlite_version.is_empty());
        // Empty would mean the allowlist lost its "homepage" id, which the panel
        // would render as a link labelled with nothing.
        assert!(info.homepage_url.starts_with("https://"));
        assert!(!info.arch.is_empty());
        assert!(!info.os.is_empty());
        assert_ne!(
            info.os, "Unknown",
            "the OS name alone beats claiming Unknown"
        );
        // SQLite is the bundled one, so it is always a dotted version.
        assert!(
            info.sqlite_version.starts_with('3'),
            "unexpected SQLite version {}",
            info.sqlite_version
        );
    }
}
