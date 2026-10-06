// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo shell: Tauri builder, startup wiring and command registration.
//! Modules are public so the integration tests can exercise the registry and
//! the composed migration runner directly.

pub mod alerts;
pub mod catalogues;
pub mod commands;
pub mod db;
pub mod duplicates;
pub mod external_links;
pub mod geo_protocol;
// Desktop only: Android already runs an app as one process, so there is no
// second copy for the lock to stop.
#[cfg(desktop)]
pub mod instance_lock;
pub mod machine;
pub mod registry;
pub mod state;
pub mod user_files;

use std::sync::Mutex;
use tauri::Manager;

/// Build and run the app. The setup hook deliberately does almost nothing so
/// the event loop starts at once; the real startup work — open + migrate the
/// database, hand the connection to Tauri's managed state — happens in `initialise` on a worker, and `app_ready` stays
/// false until it finishes. A failure there no longer aborts the process (the
/// window already exists by then): it is logged, and the frontend's readiness
/// gate fails open so the problem surfaces as ordinary command errors.
///
/// On mobile this function IS the app entry point: the macro generates the
/// JNI symbols the Android wrapper loads from the cdylib (desktop keeps
/// entering through main.rs).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Rust-side only (user_files.rs): resolves the content:// URIs the
        // dialogs return on Android. No fs commands are exposed to the
        // webview — the capabilities file deliberately grants none.
        .plugin(tauri_plugin_fs::init())
        // Rust-side only (external_links.rs): hands an allowlisted registry or
        // project URL to the platform browser. The webview passes an id, never
        // a URL, so the capabilities file grants it no opener permission — the
        // fs plugin above sets the same precedent.
        .plugin(tauri_plugin_opener::init());
    // GPS for the map's "which recinto am I standing on" lookup (P5). The
    // plugin has no desktop implementation, so it exists only in mobile
    // builds — the frontend probes for it and hides the locate button when
    // the probe rejects (i.e. on desktop).
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_geolocation::init());
    let result = builder
        // The single seam between the webview and map data: MapLibre loads
        // tiles/styles/glyphs from geo:// URLs served cache-first by Rust.
        // Asynchronous registration so handlers never block the webview.
        .register_asynchronous_uri_scheme_protocol("geo", geo_protocol::handle)
        // Startup work does NOT happen here — it is moved onto a worker so
        // this hook returns in microseconds and `run()` (the event loop) is
        // reached immediately: the window appears at once, with the spinner,
        // and nothing waits on the database to show it. On Android the webview
        // comes up in parallel with setup and can invoke commands before the
        // loop is running; before tao 0.37 a reply queued then waited for some
        // later message to flush it, and this ordering was what kept that
        // window closed. tao 0.37 fixes it at the source (tao#1304).
        // See docs/architecture.md → "On Android the webview starts first".
        .setup(|app| {
            // First, before the window: a second copy leaving here has shown
            // nothing. A file open and a lock, so the hook still returns in
            // microseconds.
            #[cfg(desktop)]
            hold_data_folder(app.handle());
            // The main window, built here rather than by Tauri. tauri.conf.json
            // declares it with `create: false` because Tauri builds its
            // windows BEFORE calling this hook — measured, a second copy's
            // window was on screen for 0.19 s before the lock sent it away.
            // On Android nothing runs between the two places, so the window
            // comes up at the moment it always did. By label, not by the flag:
            // `create: false` also means "opened later from code", and a
            // window declared for that must not open here.
            let main_window = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .ok_or("tauri.conf.json declares no \"main\" window")?;
            tauri::WebviewWindowBuilder::from_config(app.handle(), main_window)?.build()?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(err) = initialise(&handle) {
                    // Nothing here can abort the process the way a failing
                    // setup hook did: the window already exists. The frontend
                    // gate fails open after its deadline and mounts, so the
                    // failure surfaces through ordinary command errors, which
                    // beats a window that never explains itself.
                    eprintln!("fatal: Terrazgo failed to initialise: {err}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Paths stay `commands::<name>` on purpose: the parent re-exports every
            // domain file, so moving a command between them is not an API change
            // (see commands.rs). That also means the path cannot say where a
            // command lives — which is what the grouping below is for. Ordered to
            // mirror each file, so the two read side by side.

            // --- app — the shell's own surface: readiness, settings, catalogues, backups, maintenance
            commands::app_ready,
            commands::is_mobile,
            commands::get_status,
            commands::get_about_info,
            commands::get_settings,
            commands::update_settings,
            commands::clear_tile_cache,
            commands::check_and_compact_database,
            commands::catalogue_status,
            commands::refresh_catalogues,
            commands::export_backup,
            commands::inspect_backup,
            commands::import_backup,
            // --- sync: the bundle two devices hand each other (docs/sync.md)
            commands::export_sync_bundle,
            commands::inspect_sync_bundle,
            commands::import_sync_bundle,
            commands::join_sync_group,
            commands::list_sync_conflicts,
            commands::review_sync_conflict,
            commands::resolve_sync_conflict,
            commands::list_duplicates,
            commands::list_saved_duplicates,
            commands::review_duplicate_pair,
            commands::mark_duplicates_distinct,
            commands::keep_duplicate,
            commands::restore_removed_duplicate,
            commands::merge_books,
            commands::list_merge_candidates,
            commands::kept_book_by_default,
            commands::list_stray_records,
            commands::move_stray_records,
            commands::restore_season,
            commands::list_removed_books,
            commands::removed_with_book,
            commands::list_sync_peers,
            commands::rename_sync_peer,
            commands::retire_sync_peer,
            // --- core — the farm registry: farms, plots, seasons, crops, people, machines, premises; and the alert list every alert crate feeds
            commands::list_alerts,
            commands::acknowledge_alert,
            commands::dismiss_alert,
            commands::list_user_profiles,
            commands::create_user_profile,
            commands::update_user_profile,
            commands::delete_user_profile,
            commands::list_countries,
            commands::list_farms,
            commands::get_farm,
            commands::create_farm,
            commands::update_farm,
            commands::delete_farm,
            commands::list_plots,
            commands::create_plot,
            commands::update_plot,
            commands::delete_plot,
            commands::list_seasons,
            commands::list_farm_seasons,
            commands::get_season,
            commands::create_season,
            commands::update_season,
            commands::delete_season,
            commands::book_deletion_preview,
            commands::list_crops,
            commands::create_crop,
            commands::update_crop,
            commands::delete_crop,
            commands::list_operators,
            commands::list_machinery,
            commands::list_production_systems,
            commands::list_units,
            commands::list_quantity_units,
            commands::list_intensity_units,
            commands::list_irrigation_systems,
            commands::list_growing_environments,
            commands::list_licence_levels,
            commands::list_gip_systems,
            commands::list_advisors,
            commands::create_advisor,
            commands::update_advisor,
            commands::delete_advisor,
            commands::list_farm_advisors,
            commands::set_farm_advisor,
            commands::remove_farm_advisor,
            commands::create_operator,
            commands::update_operator,
            commands::delete_operator,
            commands::list_machinery_details,
            commands::create_machinery,
            commands::update_machinery,
            commands::delete_machinery,
            commands::list_premises,
            commands::list_premises_details,
            commands::list_premises_kinds,
            commands::list_sowing_kinds,
            commands::list_premises_classes,
            commands::create_premises,
            commands::update_premises,
            commands::delete_premises,
            commands::list_geo_features,
            commands::save_plot_boundary,
            commands::delete_geo_feature,
            commands::list_zone_flags,
            commands::list_water_points,
            commands::create_water_point,
            commands::update_water_point,
            commands::delete_water_point,
            commands::list_water_declarations,
            commands::set_water_declaration,
            commands::list_sowing_records,
            commands::create_sowing_record,
            commands::update_sowing_record,
            commands::delete_sowing_record,
            commands::list_harvest_records,
            commands::create_harvest_record,
            commands::update_harvest_record,
            commands::delete_harvest_record,
            commands::list_irrigation_volume_units,
            commands::list_fertiliser_dose_units,
            // --- phytosanitary — the treatment domain (module-phytosanitary): products, every register RD 1311/2012 governs
            commands::get_treatment_record,
            commands::list_reason_categories,
            commands::list_efficacies,
            commands::list_justifications,
            commands::list_problem_codes,
            commands::list_measures,
            commands::list_growth_stages,
            commands::list_basic_substances,
            commands::list_products,
            commands::list_formulation_types,
            commands::list_authorisation_kinds,
            commands::list_exceptional_substances,
            commands::list_product_details,
            commands::create_product,
            commands::update_product,
            commands::delete_product,
            commands::add_product_authorisation,
            commands::remove_product_authorisation,
            commands::list_active_substances,
            commands::create_active_substance,
            commands::add_product_substance,
            commands::remove_product_substance,
            commands::create_treatment_record,
            commands::update_treatment_record,
            commands::list_treatment_records,
            commands::set_treatment_efficacy,
            commands::delete_treatment_record,
            commands::export_cuaderno_precheck,
            commands::export_cuaderno,
            commands::list_phi_status,
            commands::seed_demo_data,
            commands::list_non_field_subject_kinds,
            commands::list_register_kinds,
            commands::list_non_field_treatments,
            commands::create_non_field_treatment,
            commands::update_non_field_treatment,
            commands::set_non_field_efficacy,
            commands::delete_non_field_treatment,
            commands::list_register_declarations,
            commands::set_register_declaration,
            commands::clear_register_declaration,
            commands::list_seed_treatments,
            commands::create_seed_treatment,
            commands::update_seed_treatment,
            commands::set_seed_treatment_efficacy,
            commands::delete_seed_treatment,
            commands::list_analysis_materials,
            commands::list_analysis_types,
            commands::list_seed_treatment_kinds,
            commands::list_plant_products,
            commands::list_substance_codes,
            commands::list_analysis_records,
            commands::create_analysis_record,
            commands::update_analysis_record,
            commands::delete_analysis_record,
            // --- fertilisation — fertilisation and irrigation (module-fertilisation): RD 1051/2022's registers
            commands::list_irrigation_methods,
            commands::list_water_origins,
            commands::list_irrigation_records,
            commands::create_irrigation_record,
            commands::update_irrigation_record,
            commands::delete_irrigation_record,
            commands::list_fertilisation_types,
            commands::list_application_methods,
            commands::list_manure_treatments,
            commands::list_nutrient_kinds,
            commands::list_fertiliser_material_kinds,
            commands::list_fertiliser_material_details,
            commands::fertiliser_material_proposal,
            commands::list_nutrient_codes,
            commands::list_fertilisation_practices,
            commands::list_irrigation_practices,
            commands::list_fertiliser_materials,
            commands::create_fertiliser_material,
            commands::update_fertiliser_material,
            commands::delete_fertiliser_material,
            commands::list_fertilisation_records,
            commands::create_fertilisation_record,
            commands::update_fertilisation_record,
            commands::delete_fertilisation_record,
            commands::list_fertilisation_plans,
            commands::create_fertilisation_plan,
            commands::update_fertilisation_plan,
            commands::delete_fertilisation_plan,
            // --- ecoscheme — eco-schemes (module-ecoscheme): RD 1048/2022's grazing, operations and soil covers
            commands::list_eco_practices,
            commands::list_cultural_operation_kinds,
            commands::list_animal_species,
            commands::list_residue_destinations,
            commands::list_grazing_records,
            commands::create_grazing_record,
            commands::update_grazing_record,
            commands::delete_grazing_record,
            commands::list_cultural_operations,
            commands::create_cultural_operation,
            commands::update_cultural_operation,
            commands::delete_cultural_operation,
            commands::list_cover_types,
            commands::list_soil_covers,
            commands::create_soil_cover,
            commands::update_soil_cover,
            commands::delete_soil_cover,
            // --- sigpac — SIGPAC lookups, zone checks and declared-crop prefill (module-sigpac)
            commands::sigpac_lookup_reference,
            commands::sigpac_lookup_point,
            commands::sigpac_verify_plot,
            commands::sigpac_propose_crops,
            commands::sigpac_accept_crop_proposals,
            commands::list_crop_species,
            // --- geo — the map tier: styles and stored geometry (terrazgo-geo)
            commands::get_map_style,
            commands::list_boundary_file,
            commands::read_boundary_feature,
            // --- recordbook — the printed record book and the SIEX export (terrazgo-recordbook)
            commands::book_advisory,
            commands::report_languages,
            commands::export_cuaderno_pdf,
            commands::export_cuaderno_xlsx,
            // --- links — the allowlisted pages the app points at (external_links.rs)
            commands::open_external_link,
        ])
        .build(tauri::generate_context!());

    // The stock template ends in `.expect(...)`; spelled out instead because
    // unwrap/expect are banned outside tests (workspace clippy lint).
    let app = match result {
        Ok(app) => app,
        Err(e) => {
            eprintln!("fatal: failed to start Terrazgo: {e}");
            std::process::exit(1);
        }
    };

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            close_databases(handle);
        }
    });
}

/// Close both databases on the way out.
///
/// SQLite deletes a database's `-wal`/`-shm` sidecars when the last connection
/// to it closes cleanly, and nothing else does — a checkpoint empties the
/// write-ahead log but leaves the files. Dropping the connections would be
/// enough in an ordinary program, but Tauri never drops managed state: the
/// platform event loop ends the process with `std::process::exit`, so this hook
/// is the last chance to close, and closing has to be explicit.
///
/// `RunEvent::Exit` arrives before that exit on desktop. Android never reaches
/// it — the system kills the process — which is what WAL is for, and why this
/// is not gated to desktop: the branch simply never runs there.
///
/// `try_state` rather than `state`: `initialise` runs on a worker thread, so a
/// window closed during startup can arrive here before either database has been
/// managed. Failures are printed rather than surfaced — there is no UI left.
fn close_databases(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<state::AppState>()
        && let Err(err) = state.db.close()
    {
        eprintln!("warning: could not close the database cleanly: {err}");
    }
    if let Some(geo) = app.try_state::<state::GeoState>()
        && let Err(err) = geo.cache.close()
    {
        eprintln!("warning: could not close the geo cache cleanly: {err}");
    }
}

/// Hold the data folder for this copy of the app, or leave if another copy
/// already holds it (`instance_lock.rs` says why two must not share it). The
/// lock is managed state, which Tauri never drops, so it is held until the
/// process ends.
///
/// A folder that cannot be resolved or created is not decided here: that is
/// `initialise`'s to report, with the window up. This only answers whether this
/// copy may go on.
#[cfg(desktop)]
fn hold_data_folder(app: &tauri::AppHandle) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return;
    };
    // A failure here reaches `claim` as a file it cannot open, which it reports.
    let _ = std::fs::create_dir_all(&data_dir);
    match instance_lock::claim(&data_dir) {
        instance_lock::Claim::Held(lock) => {
            app.manage(lock);
        }
        instance_lock::Claim::HeldElsewhere => {
            // No window and no database yet, so there is nothing to close —
            // the same exit a second launch makes in
            // `tauri-plugin-single-instance`.
            eprintln!(
                "Terrazgo is already open on {}; not starting a second copy",
                data_dir.display()
            );
            app.cleanup_before_exit();
            std::process::exit(0);
        }
        instance_lock::Claim::Unavailable(err) => {
            eprintln!(
                "warning: could not lock the data folder, so a second copy would not be stopped: {err}"
            );
        }
    }
}

/// Everything the app needs before any command can run: the databases, the
/// reference catalogues and the device-local settings. Alerts are not among
/// them — the list is worked out when read, so there is nothing to prepare.
///
/// Runs on a worker rather than in the setup hook (see the comment there), and
/// manages `SetupComplete` last so `app_ready` only answers `true` once every
/// piece of state is in place.
fn initialise(app: &tauri::AppHandle) -> anyhow::Result<()> {
    // app_data_dir is fixed by the `identifier` in tauri.conf.json:
    // ~/.local/share/org.terrazgo.app on Linux (XDG).
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("terrazgo.db");

    // Device-local settings, a plain JSON file beside the databases.
    // A missing or unreadable file just means defaults (tolerant
    // read), so loading can never abort startup.
    //
    // Loaded BEFORE the database is opened, which is load-bearing: the
    // connection needs this device's id before it can log a change.
    let settings_path = data_dir.join("settings.json");
    let mut settings = terrazgo_core::settings::load_settings(&settings_path);
    let (device_id, minted) = settings.ensure_device_id();
    if minted && let Err(err) = terrazgo_core::settings::save_settings(&settings_path, &settings) {
        // Saved before anything is written under it, and a failure is told but
        // does not stop the app, as with every other settings write at startup.
        // What it costs is bounded: this launch writes under the new id and the
        // next mints another, exactly as a restored backup does — two replica
        // names, never one name for two sets of changes. Refusing to start over
        // a settings file would lock the farmer out of their records.
        eprintln!("warning: could not save the new device id: {err}");
    }

    let mut conn = db::open_app_db(&db_path, &device_id)?;
    let schema_version = db::schema_version(&conn)?;

    // Reference catalogues (vendored FEGA snapshot). Idempotent and
    // upsert-only; after first run this is a handful of date probes.
    terrazgo_core::catalogue::ensure_catalogues(&mut conn)?;
    let tile_cache_cap = settings
        .tile_cache_max_bytes
        .unwrap_or(terrazgo_geo::db::TILE_CACHE_MAX_BYTES);

    app.manage(state::AppState {
        db: terrazgo_core::db::Database::new(conn)?,
        db_path,
        schema_version,
    });
    app.manage(state::SettingsState {
        settings: Mutex::new(settings),
        path: settings_path,
    });

    // The geo cache is a separate database with its own lifecycle:
    // derived, re-fetchable, never in backups or record_change.
    let cache_path = data_dir.join("geo-cache.db");
    let geo_cache = terrazgo_geo::db::open_cache(&cache_path)?;
    app.manage(state::GeoState {
        cache: geo_cache,
        cache_path,
    });

    // Corruption check, off the readiness path for the same reason as the tile
    // cap below: measured at ~1.7 ms/MB, so ~10 ms on a smallholder's book but
    // approaching a second on a cooperative-scale one after a decade
    // (src-tauri/tests/contracts/quick_check_cost.rs re-runs the measurement).
    //
    // It opens its OWN read-only connection rather than borrowing the app's.
    // Holding the shared lock for that long would freeze every command at
    // startup, and WAL lets a second reader in without disturbing writers. The
    // cost is a narrow race: closing the window mid-check leaves the sidecars
    // behind that once, because this connection is then the last one open.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) = db::run_due_integrity_check(&handle) {
            // A failure to CHECK is not a failure of the database; say so and
            // leave the previous verdict standing.
            eprintln!("integrity check could not run: {err}");
        }
    });

    // Tile-cache size cap, off the readiness path: usually a no-op, but the
    // reclaim VACUUM on a maxed-out cache takes seconds and must not hold the
    // app closed. Failure only means the cache stays big — log it, never fail.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(geo) = handle.try_state::<state::GeoState>() else {
            return;
        };
        let Ok(cache) = geo.cache.lock() else {
            return;
        };
        let Ok(conn) = cache.conn() else {
            return;
        };
        match terrazgo_geo::db::enforce_tile_cache_cap(conn, tile_cache_cap) {
            Ok(0) => {}
            Ok(evicted) => eprintln!("geo-cache cap: evicted {evicted} tiles"),
            Err(err) => eprintln!("geo-cache cap enforcement failed: {err}"),
        }
    });

    // The purge: what was deleted from a book goes for good once it is due
    // (docs/sync.md → The purge, as settled → When it runs). Off the readiness
    // path and a few seconds after it, so the first screen is up before it
    // takes the database — it holds the one connection while it erases.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(db::PURGE_AFTER_START);
        match db::run_due_purge(&handle) {
            Ok(0) => {}
            Ok(erased) => eprintln!("purge: {erased} registers erased for good"),
            Err(err) => eprintln!("purge could not run: {err}"),
        }
    });

    // MUST stay the last statement: `app_ready` reports readiness by probing
    // this marker (see state::SetupComplete). Managing it any earlier would
    // let commands run against half-initialised state.
    app.manage(state::SetupComplete);
    Ok(())
}
