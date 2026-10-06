// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell's own surface: readiness, platform, status, settings,
//! reference-catalogue maintenance and backups.
//!
//! Split out of `commands.rs` (2026-08-13); the boundary machinery and the
//! re-exports stay in the parent file.

use super::{CmdResult, CommandError, module_backup_shape};
use crate::db;
use crate::state;
use crate::state::AppState;
use anyhow::anyhow;
use module_phytosanitary::repository;
use serde::Serialize;
use std::path::Path;
use terrazgo_core::repository as core_repo;
// `Manager` is what puts `config()` on a Window — the About panel reads the
// bundle identifier off the running config rather than repeating it.
use tauri::Manager;
use tauri::State;
use terrazgo_core::catalogue::CatalogueStatus;
use terrazgo_core::catalogue::RefreshReport;
use terrazgo_core::settings::AppSettings;

/// What the Status view needs, which is now one question: is the database
/// sound?
///
/// It carried the database path, the schema version and the app version too
/// until 2026-09-06, printed as a strip above the alerts. They moved to the
/// About panel's technical tab, where the rest of the build's facts already
/// were — three constants on a working screen were furniture, and having them
/// in two places meant two things to keep true.
#[derive(Serialize)]
pub struct AppStatus {
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
pub fn get_status(settings: State<'_, state::SettingsState>) -> CmdResult<AppStatus> {
    Ok(AppStatus {
        integrity: settings
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?
            .last_integrity_check
            .clone(),
    })
}

/// One of the app's two SQLite files, as the About panel reports it.
///
/// Both are described the same way on purpose: they are the two things on disk
/// that grow, and "where is it and how big has it got" is the same question
/// about each. What differs is what the answer means — losing the record book
/// loses data, losing the cache loses warm tiles (`terrazgo_geo::db`).
#[derive(Serialize)]
pub struct DatabaseInfo {
    pub path: String,
    /// The `user_version` pragma. `None` if the file cannot be read right now
    /// — a closed connection during a backup import, or a poisoned lock.
    pub schema_version: Option<usize>,
    /// What the file occupies on disk, INCLUDING its `-wal` sidecar: in WAL
    /// mode a committed write lives in the log until a checkpoint folds it
    /// back, so the database file alone understates a busy session by however
    /// much has not been checkpointed yet. Bytes, because rendering a size is
    /// the frontend's job — the same figure reads differently in each locale.
    pub bytes: Option<u64>,
}

/// The monitor the window is on. Physical pixels and the scale factor beside
/// each other, because that is the pair that explains a screenshot: the same
/// 1920×1080 panel is a different amount of app at scale 1 and at scale 2.
///
/// The REFRESH RATE is deliberately absent. Tauri's `Monitor` does not carry
/// one (name, size, position, work area and scale factor are all of it), and
/// the webview can measure what it actually paints at — so the panel measures
/// it there rather than having Rust guess at a number it cannot see.
#[derive(Serialize)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    /// Top-left corner in the virtual desktop, which is what tells a
    /// multi-monitor report which screen this was.
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}

/// The versions the About panel prints, so a bug report carries them.
#[derive(Serialize)]
pub struct AboutInfo {
    pub app_version: &'static str,
    /// When this binary was built, UTC and to the minute
    /// (`YYYY-MM-DDTHH:MMZ`). Stamped by `build.rs`, which documents when it
    /// refreshes — every packaged build, and not every `cargo build`.
    ///
    /// It rides beside the version because during a pre-release the version
    /// does not move: half a dozen builds a day all call themselves 0.1.7, so
    /// this is the part that tells one of them from another.
    pub build_time: &'static str,
    /// The bundle identifier from `tauri.conf.json`, read from the running
    /// config rather than repeated here. It is what names the data directory
    /// (`~/.local/share/org.terrazgo.app`), so a report that quotes it says
    /// which install it is about.
    pub identifier: String,
    /// Which packaging the binary came out of — see `machine::Packaging`. A
    /// code the frontend translates, never a label.
    pub packaging: crate::machine::Packaging,
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
    /// The architecture this BINARY was built for, which is the one that
    /// matters for a crash: a build can be running under emulation on a
    /// machine of another kind, and it is the build a report is about.
    pub arch: &'static str,
    pub cpu: crate::machine::CpuInfo,
    pub memory: crate::machine::MemoryInfo,
    /// `None` where the platform has no monitor to report — which is every
    /// headless run, and Android, where the window is the screen.
    pub display: Option<DisplayInfo>,
    /// The record book itself: the file every register is written to.
    pub database: DatabaseInfo,
    /// The map cache — derived, disposable, and the one that gets big
    /// (512 MiB by default). Reported beside the record book precisely so the
    /// two sizes can be told apart before anyone deletes the wrong file.
    pub geo_cache: DatabaseInfo,
    /// The project's own page, for the About panel to PRINT beside the title.
    /// Read from the same allowlist `open_external_link` resolves against, so
    /// the address shown and the address opened cannot drift apart — and the
    /// webview still names a link by id when it wants one opened.
    pub homepage_url: &'static str,
}

/// One database's location, schema version and size on disk.
///
/// Every part of it degrades to `None` on its own: a closed connection (a
/// backup import is swapping the file) costs the schema version, a file not
/// yet created costs the size, and neither may stop the panel opening.
fn database_info(path: &Path, db: &terrazgo_core::db::Database) -> DatabaseInfo {
    let schema_version = db.lock().ok().and_then(|guard| {
        guard
            .conn()
            .ok()
            .and_then(|conn| db::schema_version(conn).ok())
    });
    DatabaseInfo {
        path: path.display().to_string(),
        schema_version,
        bytes: file_bytes(path),
    }
}

/// A database file plus its write-ahead log, in bytes. `None` only if the
/// database file itself cannot be stat'ed; a missing `-wal` (nothing written
/// since the last checkpoint) simply adds nothing.
fn file_bytes(path: &Path) -> Option<u64> {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push("-wal");
    let database = std::fs::metadata(path).ok()?.len();
    let wal = std::fs::metadata(Path::new(&sidecar))
        .map(|meta| meta.len())
        .unwrap_or(0);
    Some(database + wal)
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
///
/// It takes state now, and that does not weaken the rule above: `AppState` and
/// `GeoState` are managed before the frontend mounts (`state::SetupComplete`),
/// and nothing here blocks on the webview but the one call that always did.
#[tauri::command]
pub fn get_about_info(
    window: tauri::Window,
    state: State<'_, AppState>,
    geo: State<'_, state::GeoState>,
) -> AboutInfo {
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        build_time: env!("TERRAZGO_BUILD_TIME"),
        identifier: window.config().identifier.clone(),
        packaging: crate::machine::current_packaging(),
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
        os: crate::machine::os(),
        arch: std::env::consts::ARCH,
        cpu: crate::machine::cpu(),
        memory: crate::machine::memory(),
        // A monitor the platform will not name is not an error here: the panel
        // prints a dash and everything else on the tab still answers.
        display: window
            .current_monitor()
            .ok()
            .flatten()
            .map(|monitor| DisplayInfo {
                width: monitor.size().width,
                height: monitor.size().height,
                x: monitor.position().x,
                y: monitor.position().y,
                scale: monitor.scale_factor(),
            }),
        database: database_info(&state.db_path, &state.db),
        geo_cache: database_info(&geo.cache_path, &geo.cache),
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
    // The one sanctioned reach for module-phytosanitary's own defaults: this REPORTS
    // them so the UI can label an unset field with its effective value. It is
    // not a resolved config — that is built from the device's settings where
    // the alerts are gathered (`crate::alerts`), and passing this to the rules
    // instead would be the bug the missing `Default` guards.
    let alerts = module_phytosanitary::alerts::AlertConfig::defaults();
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

/// Replace the settings wholesale — the Settings form is the source of truth
/// for every preference, like the farm/plot full-row updates. The fields the
/// machine owns (the device id, the last integrity verdict) are carried over
/// from the live copy instead: the form may be holding a stale read of them
/// (`AppSettings::apply_form`). Validation belongs to each setting's
/// owning crate (the cache cap is range-checked by terrazgo-geo, the alert lead
/// times by module-phytosanitary); the file is written before the in-memory copy so a
/// failed save never leaves them disagreeing.
///
/// **Both changed settings act immediately**: shrinking the cache must visibly
/// free space, so it is enforced here. A lead time needs nothing — the alert
/// list is worked out from the settings each time it is read.
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
    for days in [settings.licence_lead_days, settings.itv_lead_days]
        .into_iter()
        .flatten()
    {
        module_phytosanitary::alerts::validate_lead_days(days)?;
    }
    if let Some(days) = settings.phi_recent_days {
        repository::validate_phi_horizon_days(days)?;
    }

    // The form's choices over the fields the machine owns (`apply_form`), under
    // the lock, so nothing can change those fields between the read and the
    // write. File first, memory second, as every settings write does.
    let settings = {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
        let next = guard.apply_form(settings);
        terrazgo_core::settings::save_settings(&state.path, &next)?;
        *guard = next.clone();
        next
    };

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

/// What a backup file says about itself, and what importing it would cost —
/// read without applying any of it.
///
/// The import screen asks this first, for the reason the sync import does: a
/// file arriving from a USB stick should be looked at before it replaces
/// anything. Here the question it answers is sharper, because **an import is a
/// mirror rather than a merge**: whatever this book holds and the file does not
/// is gone from the live database, and the number is knowable in advance.
///
/// It also puts validation before the confirmation rather than after it, so a
/// damaged or foreign file is refused without a person first being asked to
/// approve replacing everything with it.
#[derive(Serialize)]
pub struct BackupPreview {
    pub schema_version: i64,
    /// `None` when the file is too old to say — its log predates the change
    /// stamp — in which case the screen warns in the plain terms it always did.
    pub discarded: Option<terrazgo_core::backup::DiscardedByImport>,
}

/// `async` for `import_backup`'s reason: it opens and scans a file whose size
/// is the database's. The body stays synchronous.
#[tauri::command]
pub async fn inspect_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
) -> CmdResult<BackupPreview> {
    let src = crate::user_files::stage_user_source(&app, &src_path)?;
    let db = state.db.lock()?;
    let conn = db.conn()?;
    let live_version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let info =
        terrazgo_core::backup::validate_backup(src.path(), live_version, &module_backup_shape())?;
    let device = terrazgo_core::sync::installed_device(conn)?;
    Ok(BackupPreview {
        schema_version: info.schema_version,
        discarded: terrazgo_core::backup::discarded_by_import(conn, src.path(), &device)?,
    })
}

/// Replace the live database with a backup file.
///
/// Order is the safety argument: (1) validate the file (integrity + schema
/// version — newer-than-app is rejected, older migrates forward on reopen);
/// (2) export a safety copy of the CURRENT database next to it; (3) close the
/// live connection, copy the backup over the live path, and reopen through the
/// composed migration runner. The lock is held across all of
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
    // Held across the whole swap, and taken BEFORE the database — the order
    // every command uses. The imported file gets a new device identity (see
    // below), and that identity has to be on disk before anything is written
    // under it.
    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| CommandError(anyhow!("settings mutex is poisoned")))?;
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

    // The restored database writes as a NEW device, on this machine too
    // (docs/sync.md → Device identity). It is a snapshot of the past: keeping
    // the old id would restart this device's change-set numbers below ones
    // other devices may already hold, so one number would name two different
    // changes. A new id makes that impossible rather than guarded. Saved file
    // first, memory second, like every settings write — and before the swap,
    // so a failure here leaves the live database untouched.
    let device_id = terrazgo_core::sync::mint_device_id();
    // The id this machine wrote as until now. After the swap it is a replica
    // that can never write again, and this is the only moment anything knows
    // that — `succeed_sync_peer` below is what stops the devices list filling
    // with rows that are all really this machine.
    let replaced = settings.device_id.clone();
    let mut next = settings.clone();
    next.device_id = Some(device_id.clone());
    terrazgo_core::settings::save_settings(&settings_state.path, &next)?;
    *settings = next;

    // Swap. Closing takes the WAL sidecars with it, so the copy below lands on
    // a path with nothing stale beside it — and a close that FAILS aborts here,
    // before the live file is overwritten, with the safety copy already made.
    db.close()?;
    std::fs::copy(src.path(), &state.db_path)?;

    let mut conn = crate::db::open_app_db(&state.db_path, &device_id)?;
    // `open_app_db` has just registered the new identity; this hands the old
    // one's name over to it and retires it. No actor, for `register_this_device`'s
    // reason: the app taking over from the replica it replaced is not something
    // a person did.
    if let Some(replaced) = replaced {
        core_repo::succeed_sync_peer(&mut conn, &replaced, None)?;
    }
    db.replace(conn);

    // Only after the swap succeeded: an import that failed halfway must not
    // also cost the user an older safety copy. A prune that fails is not worth
    // failing a completed import over — the next import attempts it again.
    if let Err(err) = terrazgo_core::backup::prune_pre_import_copies(
        &backups_dir,
        terrazgo_core::backup::KEEP_PRE_IMPORT_COPIES,
    ) {
        eprintln!("warning: could not prune old pre-import copies: {err}");
    }

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
    /// Every constant the About panel prints must carry something a reader can
    /// act on. The failure this guards is silent: a version constant that stops
    /// resolving, or an allowlist that lost its homepage id, renders a blank row
    /// in a panel whose whole purpose is to be pasted into a bug report.
    ///
    /// It asserts on the INPUTS rather than on `get_about_info()`, which now
    /// takes a window and two managed states and so needs a running app. The
    /// parts that are not constants are tested where they are computed —
    /// `machine.rs` for the machine probes, and the panel itself for layout.
    #[test]
    fn the_build_facts_the_about_panel_prints_all_resolve() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
        assert!(!tauri::VERSION.is_empty());
        // SQLite is the bundled one, so it is always a dotted 3.x version.
        let sqlite = rusqlite::version();
        assert!(
            sqlite.starts_with('3'),
            "unexpected SQLite version {sqlite}"
        );
        assert!(
            crate::external_links::url_for("homepage")
                .unwrap_or_default()
                .starts_with("https://"),
            "the allowlist must still carry a homepage id"
        );
        assert!(!std::env::consts::ARCH.is_empty());
    }

    /// The build stamp comes from an environment variable `build.rs` writes, so
    /// nothing in the type system says it is a timestamp. A stamp that silently
    /// became empty — or stopped being a time — would print as a blank beside
    /// the version, which is the one row a tester is asked to read out.
    #[test]
    fn the_build_stamp_is_a_utc_instant_to_the_minute() {
        let stamp = env!("TERRAZGO_BUILD_TIME");
        assert_eq!(stamp.len(), 17, "expected YYYY-MM-DDTHH:MMZ, got {stamp}");
        assert!(
            stamp.ends_with('Z'),
            "the stamp must say it is UTC: {stamp}"
        );
        // The date half is parsed rather than pattern-matched, by the same
        // parser every stored date in the app goes through.
        let (date, time) = stamp.split_once('T').expect("a date and a time");
        terrazgo_core::date::parse_date(date).expect("the stamp's date half");
        assert_eq!(time.len(), 6, "expected HH:MMZ, got {time}");
    }
}

// ---------------------------------------------------------------------------
// Sync bundles
// ---------------------------------------------------------------------------

/// What an exported bundle turned out to be, for the message the farmer reads.
#[derive(Serialize)]
pub struct SyncExport {
    pub path: String,
    pub change_sets: usize,
    pub rows: usize,
    pub size_bytes: u64,
}

/// What a bundle file says about itself, read without applying any of it.
///
/// The import screen asks this first, for two reasons. A farmer choosing a file
/// out of a folder of them should see whose it is and how old before anything
/// happens; and a bundle from a holding this device has not joined has to be
/// offered as a question — [`join_sync_group`] — rather than refused with no
/// way forward.
#[derive(Serialize)]
pub struct SyncBundleInfo {
    pub device: String,
    /// What people call that device, where anybody has named it — this device's
    /// name for it, or the one the bundle carries (`bundle::sender_label`).
    /// `None` for a device nobody has named yet: the screen says so in words,
    /// never with the id.
    pub device_label: Option<String>,
    pub group: String,
    pub created_at: String,
    pub change_sets: usize,
    pub rows: usize,
    /// `same`, `unpaired` (this device has joined nothing) or `other` (it has
    /// joined a different holding) — which decides what the screen asks.
    pub pairing: &'static str,
}

/// Write a bundle of everything the devices it answers have not got, to a
/// destination the user chose in the save dialog.
///
/// `answering` is the `seen` vector of every manifest this device has imported
/// since the app opened. The bundle leaves out only what ALL of them held —
/// their common ancestor — so one file copied to each of those devices is
/// complete at each, which is the consolidation laptop's whole routine
/// (docs/sync.md → A trimmed reply is safe only for the device it answers).
/// One import makes that exactly the precise reply to it; none makes it the
/// whole log, which is what the first exchange between two devices sends.
///
/// `async` for the reason `export_backup` is: the work scales with the log and
/// must not block the main thread. The body stays synchronous.
#[tauri::command]
pub async fn export_sync_bundle(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dest_path: String,
    answering: Vec<terrazgo_core::sync::VersionVector>,
) -> CmdResult<SyncExport> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    let device = terrazgo_core::sync::installed_device(conn)?;
    let asked = terrazgo_core::sync::VersionVector::common_ancestor_of(&answering);

    // Built in memory rather than streamed to the destination: on Android the
    // destination is a content URI whose writes materialise late, and a
    // half-written bundle in a shared folder is the failure Part 1 of the
    // design measured. One buffer, one write, one `sync_all` inside
    // `write_user_file`.
    let mut bytes = Vec::new();
    let summary = terrazgo_core::bundle::write_bundle(conn, &device, &asked, &mut bytes)?;
    let size_bytes = bytes.len() as u64;
    crate::user_files::write_user_file(&app, &dest_path, &bytes)?;
    Ok(SyncExport {
        path: dest_path,
        change_sets: summary.change_sets,
        rows: summary.rows,
        size_bytes,
    })
}

/// Read a bundle's manifest and say how it stands to this device.
///
/// **Refuses a file that leaves out changes this device lacks**, with the same
/// refusal the import would give — so the farmer hears it before any question
/// rather than after confirming. It comes before the pairing question on
/// purpose: joining another group is the one step here that costs something,
/// and a file that could not apply afterwards must not lead anyone into it.
#[tauri::command]
pub async fn inspect_sync_bundle(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
) -> CmdResult<SyncBundleInfo> {
    let src = crate::user_files::stage_user_source(&app, &src_path)?;
    let parsed = terrazgo_core::bundle::read_bundle(std::fs::File::open(src.path())?)?;
    let db = state.db.lock()?;
    let conn = db.conn()?;
    terrazgo_core::bundle::refuse_if_incomplete(conn, &parsed)?;
    let device_label = terrazgo_core::bundle::sender_label(conn, &parsed)?;
    let ours = terrazgo_core::sync::sync_group(conn)?;
    let pairing = match ours {
        None => "unpaired",
        Some(ours) if ours == parsed.manifest.group => "same",
        Some(_) => "other",
    };
    Ok(SyncBundleInfo {
        device: parsed.manifest.device,
        device_label,
        group: parsed.manifest.group,
        created_at: parsed.manifest.created_at,
        change_sets: parsed.change_sets.len(),
        rows: parsed.change_sets.iter().map(|set| set.rows.len()).sum(),
        pairing,
    })
}

/// What an applied bundle did, and what it left waiting.
#[derive(Serialize)]
pub struct SyncImported {
    #[serde(flatten)]
    pub summary: terrazgo_core::bundle::ImportSummary,
    /// What people call the device the bundle came from, read once it is
    /// applied — so a name the bundle carried counts. `None` when nobody has
    /// named it yet.
    pub peer_label: Option<String>,
    /// How many pairs the Status view lists as possible duplicates now, so the
    /// message can say so while the farmer is looking (docs/sync.md → Worked
    /// out when read). `None` when they could not be worked out: the bundle is
    /// applied either way, and a count that failed must not read as an import
    /// that did.
    pub duplicates: Option<usize>,
    /// How many live records sit in a removed book now — a record written on
    /// another device into a book merged or deleted here (docs/sync.md →
    /// Records in a removed book). `None` when it could not be counted, as for
    /// `duplicates`.
    pub strays: Option<usize>,
    /// What the purge erased here once the file had applied: what the file
    /// made due — the last device's word that it holds a deletion — beside
    /// `summary.purged`, what a purge made elsewhere took with it. `None` when
    /// it could not run, which costs nothing but a later start or import.
    pub erased: Option<core_repo::PurgeSummary>,
    /// Whether a book the file wrote into prints a code no catalogue on this
    /// device names — so the message can say this device's catalogues may need
    /// updating (docs/sync.md → What stays device-local). "May": a code the
    /// file brought outside its books is not read. `None` when it could not be
    /// worked out, as for `duplicates`.
    pub catalogues_may_lag: Option<bool>,
}

/// Apply a bundle to the live database.
///
/// Every check and every refusal is the core's (`bundle::apply_bundle`), and
/// all of it happens in one transaction — so a bundle that fails anywhere
/// leaves the database exactly as it was and the farmer's retry starts from
/// where they were. Nothing here re-implements a check, and nothing here
/// decides what to do about one.
///
/// Nothing follows for the alerts or the duplicates: both lists are worked out
/// when read, so a treatment recorded on the phone opens its plazo here the
/// moment it lands. The duplicates are counted afterwards only for the message.
#[tauri::command]
pub async fn import_sync_bundle(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
) -> CmdResult<SyncImported> {
    let src = crate::user_files::stage_user_source(&app, &src_path)?;
    let parsed = terrazgo_core::bundle::read_bundle(std::fs::File::open(src.path())?)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    let now_ms = terrazgo_core::date::now_ms();
    let summary = terrazgo_core::bundle::apply_bundle(conn, &parsed, now_ms)?;
    let peer_label = terrazgo_core::bundle::sender_label(conn, &parsed)?;
    let today = terrazgo_core::date::today_utc();
    // After every import, as at start (docs/sync.md → The purge, as settled →
    // When it runs): the file may be what tells this device that the last
    // other one holds a deletion. The import has committed; a purge that
    // cannot run is said and tried again later, never a failed import.
    let erased = match core_repo::purge_due(conn, &today, None) {
        Ok(purged) => Some(purged),
        Err(err) => {
            eprintln!("warning: the purge could not run after an import: {err}");
            None
        }
    };
    let duplicates = match core_repo::list_duplicates(
        conn,
        &crate::duplicates::policies(),
        &crate::registry::composed_row_captions(),
        terrazgo_core::duplicates::Scope::Current { today: &today },
    ) {
        Ok(list) => Some(list.suspects.len() + list.both_removed.len()),
        Err(err) => {
            eprintln!("warning: the duplicates could not be counted after an import: {err}");
            None
        }
    };
    let strays = match core_repo::count_stray_records(conn) {
        Ok(count) => Some(count),
        Err(err) => {
            eprintln!("warning: the records in removed books could not be counted: {err}");
            None
        }
    };
    let catalogues_may_lag =
        match terrazgo_recordbook::prints_unnamed_codes(conn, &summary.books, &today) {
            Ok(lag) => Some(lag),
            Err(err) => {
                eprintln!("warning: the books could not be read for unnamed codes: {err}");
                None
            }
        };
    Ok(SyncImported {
        summary,
        peer_label,
        duplicates,
        strays,
        erased,
        catalogues_may_lag,
    })
}

/// Join the holding a bundle came from.
///
/// Separate from the import on purpose: joining is an act a person performs
/// once, not a side effect of opening a file. The screen asks — gently when
/// this device has joined nothing, and with the cost stated when it means
/// leaving one group for another — and then the same file applies unchanged.
#[tauri::command]
pub fn join_sync_group(state: State<'_, AppState>, group_id: String) -> CmdResult<()> {
    let db = state.db.lock()?;
    terrazgo_core::sync::join_sync_group(db.conn()?, &group_id)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The review queue, and the devices that fill it
// ---------------------------------------------------------------------------

/// Every register two devices wrote at once and nobody has decided about yet.
///
/// Database-wide, because not every register is in a book: a plot, an operator
/// and a machine belong to the holding. The list carries each conflict's season
/// so a screen can say which book a conflicted record is in.
#[tauri::command]
pub fn list_sync_conflicts(state: State<'_, AppState>) -> CmdResult<Vec<core_repo::ConflictEntry>> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(core_repo::list_sync_conflicts(
        conn,
        &crate::registry::composed_row_captions(),
    )?)
}

/// Both versions of one conflicted register, field by field.
///
/// Reads the log and writes nothing: the version the book is not showing is
/// reconstructed from its change sets rather than materialised, so looking at
/// it changes nothing.
#[tauri::command]
pub fn review_sync_conflict(
    state: State<'_, AppState>,
    root_table: String,
    root_id: String,
) -> CmdResult<terrazgo_core::merge::ConflictReview> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(terrazgo_core::merge::review(
        conn,
        &root_table,
        &root_id,
        &crate::registry::composed_row_captions(),
    )?)
}

/// Keep one version of a conflicted register.
///
/// An ordinary audited write, whose change set descends from every version the
/// register has — so the decision is the book on every device that receives it,
/// with no message of its own (docs/sync.md → Conflicts as the person sees
/// them). Correcting the record in its own screen resolves it just as well;
/// this is the same act in one click.
///
/// The version kept may carry a different application date or product, and so
/// a different plazo — which the alert list shows the next time it is read.
#[tauri::command]
pub fn resolve_sync_conflict(
    state: State<'_, AppState>,
    root_table: String,
    root_id: String,
    keep_device: String,
    keep_seq: i64,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<()> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(terrazgo_core::merge::resolve(
        conn,
        &root_table,
        &root_id,
        &keep_device,
        keep_seq,
        actor.as_deref(),
    )?)
}

// ---------------------------------------------------------------------------
// Duplicate suspects: one operation recorded twice (docs/sync.md → Duplicate
// suspects)
// ---------------------------------------------------------------------------

/// The register a screen names, among the ones compared for duplicates. What a
/// screen sends is only looked up: the table every call below works on is the
/// constant's, and a name that is not a listed register is `NotFound`.
fn duplicate_register(table: &str) -> CmdResult<&'static crate::duplicates::DuplicateRegister> {
    Ok(crate::duplicates::register_by_table(table).ok_or(terrazgo_core::CoreError::NotFound)?)
}

/// The pairs that may be one operation recorded twice, and the pairs removed
/// twice over by opposite verdicts.
///
/// With no book, what the Status view asks — pairs with a record in a current
/// book, one whose campaign ended less than a year ago or has not ended. With
/// one, what that book's page asks, however old the book is. Worked out on
/// every call and stored nowhere, like the alerts.
#[tauri::command]
pub fn list_duplicates(
    state: State<'_, AppState>,
    season_id: Option<String>,
) -> CmdResult<core_repo::DuplicateList> {
    let today = terrazgo_core::date::today_utc();
    let scope = match &season_id {
        Some(season_id) => terrazgo_core::duplicates::Scope::Book { season_id },
        None => terrazgo_core::duplicates::Scope::Current { today: &today },
    };
    let db = state.db.lock()?;
    Ok(core_repo::list_duplicates(
        db.conn()?,
        &crate::duplicates::policies(),
        &crate::registry::composed_row_captions(),
        scope,
    )?)
}

/// The pairs a form's save just put its record in — asked right after the save,
/// never before it (docs/sync.md → The same rule, right after the form saves). The save is not
/// held up or refused by anything here: a record is recorded, and then a person
/// is shown what it resembles.
#[tauri::command]
pub fn list_saved_duplicates(
    state: State<'_, AppState>,
    register: String,
    record_id: String,
) -> CmdResult<core_repo::SavedDuplicates> {
    let register = duplicate_register(&register)?;
    let db = state.db.lock()?;
    Ok(core_repo::list_saved_duplicates(
        db.conn()?,
        &crate::duplicates::policies(),
        &crate::registry::composed_row_captions(),
        register.policy.table,
        &record_id,
    )?)
}

/// Two records of one register side by side. Reads the log and writes nothing.
#[tauri::command]
pub fn review_duplicate_pair(
    state: State<'_, AppState>,
    register: String,
    first_id: String,
    second_id: String,
) -> CmdResult<core_repo::PairReview> {
    let register = duplicate_register(&register)?;
    let db = state.db.lock()?;
    Ok(core_repo::review_pair(
        db.conn()?,
        &register.policy,
        &first_id,
        &second_id,
        &crate::registry::composed_row_captions(),
    )?)
}

/// "Both are real": the pair is never listed again, on any device once they
/// have synced.
#[tauri::command]
pub fn mark_duplicates_distinct(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    register: String,
    first_id: String,
    second_id: String,
) -> CmdResult<()> {
    let actor = super::active_actor(&settings_state)?;
    let register = duplicate_register(&register)?;
    let mut db = state.db.lock()?;
    Ok(core_repo::mark_distinct(
        db.conn_mut()?,
        &register.policy,
        &first_id,
        &second_id,
        actor.as_deref(),
    )?)
}

/// "Keep this one": the other record is removed by its own register's delete,
/// in the change set that records why.
#[tauri::command]
pub fn keep_duplicate(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    register: String,
    kept_id: String,
    removed_id: String,
) -> CmdResult<()> {
    let actor = super::active_actor(&settings_state)?;
    let register = duplicate_register(&register)?;
    let mut db = state.db.lock()?;
    core_repo::keep_duplicate::<anyhow::Error>(
        db.conn_mut()?,
        &register.policy,
        &kept_id,
        &removed_id,
        actor.as_deref(),
        register.remove,
    )?;
    Ok(())
}

/// Bring back one of two records removed by opposite verdicts, made on two
/// devices before either heard of the other.
#[tauri::command]
pub fn restore_removed_duplicate(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    register: String,
    record_id: String,
) -> CmdResult<()> {
    let actor = super::active_actor(&settings_state)?;
    let register = duplicate_register(&register)?;
    let mut db = state.db.lock()?;
    Ok(core_repo::restore_removed_duplicate(
        db.conn_mut()?,
        &register.policy,
        &record_id,
        actor.as_deref(),
    )?)
}

/// Make two books of one farm one: every record of `absorbed_id` moves into
/// `kept_id`, and `absorbed_id` goes (docs/sync.md → Merging two books). Every
/// check is core's; the duplicate policies are passed so it can see a pair
/// removed twice over in the book that goes.
///
/// `async` for `export_backup`'s reason: 1.7 s at 4 000 treatments, which on
/// the main thread would freeze the window. The body stays synchronous.
#[tauri::command]
pub async fn merge_books(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    kept_id: String,
    absorbed_id: String,
) -> CmdResult<core_repo::BookMerge> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    Ok(core_repo::merge_books(
        db.conn_mut()?,
        &kept_id,
        &absorbed_id,
        &crate::duplicates::policies(),
        actor.as_deref(),
    )?)
}

/// The books a book's page offers to merge it with: the farm's other live
/// books whose campaign overlaps its own. None, and the page offers no merge.
#[tauri::command]
pub fn list_merge_candidates(
    state: State<'_, AppState>,
    season_id: String,
) -> CmdResult<Vec<terrazgo_core::models::Season>> {
    let db = state.db.lock()?;
    Ok(core_repo::merge_candidates(db.conn()?, &season_id)?)
}

/// Which of two books a merge screen pre-selects to keep — the same one on
/// every device, so two people merging one pair agree unless one changes it.
#[tauri::command]
pub fn kept_book_by_default(
    state: State<'_, AppState>,
    first_id: String,
    second_id: String,
) -> CmdResult<String> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    let first = core_repo::get_season(conn, &first_id)?;
    let second = core_repo::get_season(conn, &second_id)?;
    Ok(core_repo::kept_by_default(&first, &second).id.clone())
}

/// Every live record in a removed book, grouped by book, with the live book
/// each could go back into (docs/sync.md → Records in a removed book). Worked
/// out on every call, like the duplicates.
#[tauri::command]
pub fn list_stray_records(state: State<'_, AppState>) -> CmdResult<Vec<core_repo::StrayBook>> {
    let db = state.db.lock()?;
    Ok(core_repo::list_stray_records(
        db.conn()?,
        &crate::registry::composed_row_captions(),
    )?)
}

/// Move the live records of a removed book into a live book of its farm.
#[tauri::command]
pub fn move_stray_records(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    from_season_id: String,
    into_season_id: String,
) -> CmdResult<usize> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    Ok(core_repo::move_stray_records(
        db.conn_mut()?,
        &from_season_id,
        &into_season_id,
        actor.as_deref(),
    )?)
}

/// Bring a book back with what was removed with it — the book itself when it
/// is removed, and its records whether it is or not (docs/sync.md → Deleting a
/// book with its records).
///
/// `async` for `export_backup`'s reason: the work scales with the records the
/// book held, and must not block the main thread.
#[tauri::command]
pub async fn restore_season(
    state: State<'_, AppState>,
    settings_state: State<'_, state::SettingsState>,
    season_id: String,
) -> CmdResult<core_repo::RestoredBook> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    Ok(core_repo::restore_book(
        db.conn_mut()?,
        &season_id,
        &terrazgo_core::date::today_utc(),
        actor.as_deref(),
    )?)
}

/// The books deleted lately, which can still be brought back — the fold at the
/// foot of the record-book list.
#[tauri::command]
pub fn list_removed_books(state: State<'_, AppState>) -> CmdResult<Vec<core_repo::RemovedBook>> {
    let db = state.db.lock()?;
    Ok(core_repo::list_removed_books(
        db.conn()?,
        &terrazgo_core::date::today_utc(),
    )?)
}

/// For a live book, what was removed with it lately and can still come back —
/// the line on its page. `None` almost always.
#[tauri::command]
pub fn removed_with_book(
    state: State<'_, AppState>,
    season_id: String,
) -> CmdResult<Option<core_repo::BookRemoval>> {
    let db = state.db.lock()?;
    Ok(core_repo::removed_with_book(
        db.conn()?,
        &season_id,
        &terrazgo_core::date::today_utc(),
    )?)
}

/// The devices that have written to this book, and which of them is this one.
#[derive(Serialize)]
pub struct SyncPeers {
    pub peers: Vec<terrazgo_core::models::SyncPeer>,
    /// This installation's own device id — the row a screen marks as "this
    /// device" and the one it may not retire.
    pub this_device: String,
}

#[tauri::command]
pub fn list_sync_peers(state: State<'_, AppState>) -> CmdResult<SyncPeers> {
    let db = state.db.lock()?;
    let conn = db.conn()?;
    Ok(SyncPeers {
        peers: core_repo::list_sync_peers(conn)?,
        this_device: terrazgo_core::sync::installed_device(conn)?,
    })
}

/// Name a device, or clear the name it was given.
///
/// A register like any other, so the name reaches every device at the next
/// sync — which is the point: a conflict that says "María's phone" is a
/// question a farmer can answer.
#[tauri::command]
pub fn rename_sync_peer(
    state: State<'_, AppState>,
    device_id: String,
    label: Option<String>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<terrazgo_core::models::SyncPeer> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(core_repo::rename_sync_peer(
        conn,
        &device_id,
        label.as_deref(),
        actor.as_deref(),
    )?)
}

/// Retire a device nobody expects to sync with again, or bring one back.
///
/// Soft, like every row the log refers to: a lost phone's changes stay in the
/// book and stay attributed to it.
#[tauri::command]
pub fn retire_sync_peer(
    state: State<'_, AppState>,
    device_id: String,
    retired: bool,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<terrazgo_core::models::SyncPeer> {
    let actor = super::active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(core_repo::retire_sync_peer(
        conn,
        &device_id,
        retired,
        actor.as_deref(),
    )?)
}
