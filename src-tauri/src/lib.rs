// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo shell: Tauri builder, startup wiring and command registration.
//! Modules are public so the integration tests can exercise the registry and
//! the composed migration runner directly.

pub mod catalogues;
pub mod commands;
pub mod db;
pub mod geo_protocol;
pub mod registry;
pub mod state;
pub mod user_files;

use module_cue::alerts::AlertConfig;
use std::sync::Mutex;
use tauri::Manager;
use terrazgo_core::date::today_utc;

/// Build and run the app. The setup hook deliberately does almost nothing so
/// the event loop starts at once; the real startup work — open + migrate the
/// database, refresh alerts against today, hand the connection to Tauri's
/// managed state — happens in `initialise` on a worker, and `app_ready` stays
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
        .plugin(tauri_plugin_fs::init());
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
        // reached immediately. That ordering is load-bearing on Android:
        // there, the webview comes up in parallel with setup and can invoke
        // commands before the loop is running, and a reply queued before
        // `event_loop.run()` is not delivered when the loop starts — it waits
        // for some later message to flush it. Measured in a standalone tao
        // reproduction (no wry, no Tauri): an event sent before `run()` sat
        // undelivered until an unrelated event arrived 5 s later, on tao
        // 0.35.3 and 0.36.0 alike. Returning from setup at once puts the loop
        // up before the webview exists, so nothing can be queued into that
        // window and the defect is unreachable rather than worked around.
        // See docs/architecture.md → "On Android the webview starts first".
        .setup(|app| {
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
            commands::app_ready,
            commands::is_mobile,
            commands::get_status,
            commands::get_settings,
            commands::update_settings,
            commands::clear_tile_cache,
            commands::catalogue_status,
            commands::refresh_catalogues,
            commands::list_user_profiles,
            commands::create_user_profile,
            commands::update_user_profile,
            commands::delete_user_profile,
            commands::export_backup,
            commands::import_backup,
            commands::list_alerts,
            commands::refresh_alerts,
            commands::acknowledge_alert,
            commands::dismiss_alert,
            commands::get_treatment_record,
            commands::seed_demo_data,
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
            commands::create_season,
            commands::update_season,
            commands::delete_season,
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
            commands::list_non_field_subject_kinds,
            commands::list_register_kinds,
            commands::list_analysis_materials,
            commands::list_analysis_types,
            commands::list_seed_treatment_kinds,
            commands::list_plant_products,
            commands::list_substance_codes,
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
            commands::list_analysis_records,
            commands::create_analysis_record,
            commands::update_analysis_record,
            commands::delete_analysis_record,
            commands::list_harvest_records,
            commands::create_harvest_record,
            commands::update_harvest_record,
            commands::delete_harvest_record,
            commands::list_irrigation_methods,
            commands::list_water_origins,
            commands::list_irrigation_volume_units,
            commands::list_irrigation_records,
            commands::create_irrigation_record,
            commands::update_irrigation_record,
            commands::delete_irrigation_record,
            commands::list_fertilisation_types,
            commands::list_application_methods,
            commands::list_manure_treatments,
            commands::list_nutrient_kinds,
            commands::list_fertiliser_dose_units,
            commands::list_fertiliser_material_kinds,
            commands::list_fertiliser_material_details,
            commands::list_nutrient_codes,
            commands::fertiliser_material_composition,
            commands::list_fertilisation_practices,
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
            commands::list_reason_categories,
            commands::list_efficacies,
            commands::list_justifications,
            commands::list_problem_codes,
            commands::list_measures,
            commands::list_growth_stages,
            commands::list_products,
            commands::list_licence_levels,
            commands::list_irrigation_systems,
            commands::list_growing_environments,
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
            commands::book_advisory,
            commands::export_cuaderno,
            commands::report_languages,
            commands::export_cuaderno_pdf,
            commands::export_cuaderno_xlsx,
            commands::list_geo_features,
            commands::save_plot_boundary,
            commands::delete_geo_feature,
            commands::get_map_style,
            commands::list_boundary_file,
            commands::read_boundary_feature,
            commands::sigpac_lookup_reference,
            commands::sigpac_lookup_point,
            commands::sigpac_verify_plot,
            commands::sigpac_propose_crops,
            commands::sigpac_accept_crop_proposals,
            commands::list_crop_species,
            commands::list_zone_flags,
            commands::list_water_points,
            commands::create_water_point,
            commands::update_water_point,
            commands::delete_water_point,
            commands::list_water_declarations,
            commands::set_water_declaration,
            commands::list_phi_status,
        ])
        .run(tauri::generate_context!());

    // The stock template ends in `.expect(...)`; spelled out instead because
    // unwrap/expect are banned outside tests (workspace clippy lint).
    if let Err(e) = result {
        eprintln!("fatal: failed to start Terrazgo: {e}");
        std::process::exit(1);
    }
}

/// Everything the app needs before any command can run: the databases, the
/// reference catalogues, the derived alerts and the device-local settings.
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

    let mut conn = db::open_app_db(&db_path)?;
    let schema_version = db::schema_version(&conn)?;

    // Reference catalogues (vendored FEGA snapshot). Idempotent and
    // upsert-only; after first run this is a handful of date probes.
    terrazgo_core::catalogue::ensure_catalogues(&mut conn)?;

    // Idempotent reconciliation — over-calling is sanctioned by the
    // repository docs; a dismissal is never resurrected.
    module_cue::repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;

    app.manage(state::AppState {
        conn: Mutex::new(conn),
        db_path,
        schema_version,
    });

    // Device-local settings, a plain JSON file beside the databases.
    // A missing or unreadable file just means defaults (tolerant
    // read), so loading can never abort startup.
    let settings_path = data_dir.join("settings.json");
    let settings = terrazgo_core::settings::load_settings(&settings_path);
    let tile_cache_cap = settings
        .tile_cache_max_bytes
        .unwrap_or(terrazgo_geo::db::TILE_CACHE_MAX_BYTES);
    app.manage(state::SettingsState {
        settings: Mutex::new(settings),
        path: settings_path,
    });

    // The geo cache is a separate database with its own lifecycle:
    // derived, re-fetchable, never in backups or record_change.
    let geo_conn = terrazgo_geo::db::open_cache(&data_dir.join("geo-cache.db"))?;
    app.manage(state::GeoState {
        conn: Mutex::new(geo_conn),
    });

    // Tile-cache size cap, off the readiness path: usually a no-op, but the
    // reclaim VACUUM on a maxed-out cache takes seconds and must not hold the
    // app closed. Failure only means the cache stays big — log it, never fail.
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(geo) = handle.try_state::<state::GeoState>() else {
            return;
        };
        let Ok(conn) = geo.conn.lock() else {
            return;
        };
        match terrazgo_geo::db::enforce_tile_cache_cap(&conn, tile_cache_cap) {
            Ok(0) => {}
            Ok(evicted) => eprintln!("geo-cache cap: evicted {evicted} tiles"),
            Err(err) => eprintln!("geo-cache cap enforcement failed: {err}"),
        }
    });

    // MUST stay the last statement: `app_ready` reports readiness by probing
    // this marker (see state::SetupComplete). Managing it any earlier would
    // let commands run against half-initialised state.
    app.manage(state::SetupComplete);
    Ok(())
}
