// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo shell: Tauri builder, startup wiring and command registration.
//! Modules are public so the integration tests can exercise the registry and
//! the composed migration runner directly.

pub mod commands;
pub mod db;
pub mod geo_protocol;
pub mod registry;
pub mod state;

use module_cue::alerts::AlertConfig;
use std::sync::Mutex;
use tauri::Manager;
use terrazgo_core::date::today_utc;

/// Build and run the app. Startup order matters: open + migrate the database
/// first, refresh alerts against today, then hand the connection to Tauri's
/// managed state. Any failure here aborts startup — correct behaviour for
/// "the database didn't open or migrate".
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // The single seam between the webview and map data: MapLibre loads
        // tiles/styles/glyphs from geo:// URLs served cache-first by Rust.
        // Asynchronous registration so handlers never block the webview.
        .register_asynchronous_uri_scheme_protocol("geo", geo_protocol::handle)
        .setup(|app| {
            // app_data_dir is fixed by the `identifier` in tauri.conf.json:
            // ~/.local/share/org.terrazgo.desktop on Linux (XDG).
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
            module_cue::repository::refresh_alerts(
                &mut conn,
                &today_utc(),
                &AlertConfig::default(),
            )?;

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

            // Tile-cache size cap, off the startup path: usually a no-op,
            // but the reclaim VACUUM on a maxed-out cache takes seconds and
            // must not delay the window. Failure only means the cache stays
            // big — log it, never abort startup.
            let handle = app.handle().clone();
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::update_settings,
            commands::clear_tile_cache,
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
            commands::list_crops,
            commands::create_crop,
            commands::list_operators,
            commands::list_machinery,
            commands::list_production_systems,
            commands::list_units,
            commands::list_reason_categories,
            commands::list_products,
            commands::list_licence_levels,
            commands::create_operator,
            commands::update_operator,
            commands::delete_operator,
            commands::list_machinery_details,
            commands::create_machinery,
            commands::update_machinery,
            commands::delete_machinery,
            commands::list_formulation_types,
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
            commands::list_treatment_records,
            commands::delete_treatment_record,
            commands::list_geo_features,
            commands::save_plot_boundary,
            commands::delete_geo_feature,
            commands::get_map_style,
            commands::list_boundary_file,
            commands::read_boundary_feature,
            commands::sigpac_lookup_reference,
            commands::sigpac_lookup_point,
            commands::sigpac_verify_plot,
            commands::list_zone_flags,
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
