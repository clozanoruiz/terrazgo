// SPDX-License-Identifier: AGPL-3.0-or-later

//! The `geo://` custom URI scheme: the single seam through which the webview
//! reaches map data. MapLibre requests `geo://…/tiles/{source}/{z}/{x}/{y}`
//! and `geo://…/res/{prefix}/{rest}`; Rust serves them cache-first via
//! `terrazgo_geo::fetch`. The webview itself never talks to the network, so
//! the production CSP stays `default-src 'self'` plus this scheme.
//!
//! Handlers run detached from the webview thread (the *asynchronous* protocol
//! registration + `spawn_blocking` here), so a burst of tile requests fetches
//! in parallel and never blocks the UI — part of the "maps must not feel
//! sluggish" requirement (2026-07-07).

use crate::state::GeoState;
use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder};

/// Entry point wired into the builder. Clones what the worker needs and
/// returns immediately.
pub fn handle<R: tauri::Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let path = request.uri().path().to_string();
    // ureq is synchronous by design; blocking work belongs on the blocking pool.
    tauri::async_runtime::spawn_blocking(move || {
        responder.respond(respond(&app, &path));
    });
}

fn respond<R: tauri::Runtime>(app: &tauri::AppHandle<R>, path: &str) -> Response<Vec<u8>> {
    let Some(geo) = app.try_state::<GeoState>() else {
        // Setup has not managed the cache yet (or failed to) — temporary.
        return status_only(StatusCode::SERVICE_UNAVAILABLE);
    };
    match serve(&geo, path) {
        Ok(fetched) => {
            let builder = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", fetched.content_type)
                // Tiles and glyph ranges are immutable in practice; let the
                // webview's memory cache absorb repeat requests (performance
                // contract). The SQLite cache behind us survives restarts.
                .header("cache-control", "public, max-age=604800, immutable")
                // The page origin (tauri://localhost in production,
                // http://localhost:5173 in dev) is cross-origin to
                // geo://localhost, and MapLibre loads tiles with fetch() —
                // without this header the webview discards the response
                // after Rust has already served it (blank tiles, no CSP
                // violation, nothing in the console; found the hard way).
                .header("access-control-allow-origin", "*");
            builder
                .body(fetched.data)
                .unwrap_or_else(|_| status_only(StatusCode::INTERNAL_SERVER_ERROR))
        }
        Err(err) => status_only(match err {
            terrazgo_geo::GeoError::NotFound => StatusCode::NOT_FOUND,
            terrazgo_geo::GeoError::Http { status: 404 } => StatusCode::NOT_FOUND,
            // Offline with a cold cache: an empty 503 — the map shows what it
            // has and the app keeps working (offline-first principle).
            terrazgo_geo::GeoError::Offline(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }),
    }
}

/// Route a protocol path. Percent-encoding is passed through untouched: glyph
/// font-stack names arrive encoded and the upstream expects them encoded.
fn serve(geo: &GeoState, path: &str) -> terrazgo_geo::Result<terrazgo_geo::fetch::Fetched> {
    let segments: Vec<&str> = path.trim_start_matches('/').splitn(3, '/').collect();
    match segments.as_slice() {
        ["tiles", source, zxy] => {
            let mut parts = zxy.split('/');
            let z = parse_next(&mut parts)?;
            let x = parse_next(&mut parts)?;
            let y = parse_next(&mut parts)?;
            if parts.next().is_some() {
                return Err(terrazgo_geo::GeoError::NotFound);
            }
            terrazgo_geo::fetch::tile(&geo.conn, source, z, x, y)
        }
        ["res", prefix, rest] => terrazgo_geo::fetch::resource(&geo.conn, prefix, rest),
        ["res", prefix] => terrazgo_geo::fetch::resource(&geo.conn, prefix, ""),
        _ => Err(terrazgo_geo::GeoError::NotFound),
    }
}

fn parse_next<T: std::str::FromStr>(
    parts: &mut std::str::Split<'_, char>,
) -> terrazgo_geo::Result<T> {
    parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(terrazgo_geo::GeoError::NotFound)
}

fn status_only(status: StatusCode) -> Response<Vec<u8>> {
    // `Response::builder().body(...)` only errs on invalid parts; a bare
    // status with an empty body cannot fail, but stay panic-free regardless.
    Response::builder()
        .status(status)
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}
