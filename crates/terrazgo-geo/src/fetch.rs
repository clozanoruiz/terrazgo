// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cache-through HTTP fetching for tiles and map resources.
//!
//! Performance contract (2026-07-07): the cache mutex is NEVER
//! held across network I/O — tile requests arrive in bursts from the map and
//! must be able to fetch in parallel; the lock is taken briefly for the cache
//! lookup, dropped for the network round trip, and re-taken for the store.
//!
//! Offline contract: a cache hit never touches the network; a cache miss with
//! no network yields [`GeoError::Offline`], which the protocol layer turns
//! into an empty response — the map keeps working on cached tiles + stored
//! geometry.

use crate::error::{GeoError, Result};
use crate::sources::{self, TileSource};
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;
use terrazgo_core::date::now_utc_iso;

/// A fetched (or cached) payload plus the content type to serve it with.
pub struct Fetched {
    pub data: Vec<u8>,
    pub content_type: String,
}

/// Serve one tile: cache first, upstream on miss. For TileJSON-resolved
/// sources (OpenFreeMap publishes rotating dated snapshot paths) a 404 from a
/// stale template triggers one re-resolve + retry.
pub fn tile(cache: &Mutex<Connection>, source_id: &str, z: u8, x: u32, y: u32) -> Result<Fetched> {
    let source = sources::tile_source(source_id).ok_or(GeoError::NotFound)?;
    if z > source.max_zoom {
        return Err(GeoError::NotFound);
    }

    // Scoped so the guard is dropped before any network I/O.
    let cached = {
        let conn = lock(cache)?;
        cached_tile(&conn, source_id, z, x, y)?
    };
    if let Some(hit) = cached {
        return Ok(hit);
    }

    let url = upstream_tile_url(cache, source, z, x, y, false)?;
    let fetched = match http_get(&url, source.content_type) {
        Err(GeoError::Http { status: 404 }) if source.tilejson_url.is_some() => {
            let url = upstream_tile_url(cache, source, z, x, y, true)?;
            http_get(&url, source.content_type)
        }
        other => other,
    }?;

    lock(cache)?.execute(
        "INSERT OR REPLACE INTO tile (source, z, x, y, data, content_type, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            source_id,
            z,
            x,
            y,
            fetched.data,
            fetched.content_type,
            now_utc_iso()
        ],
    )?;
    Ok(fetched)
}

/// Serve one allowlisted non-tile resource (style JSON, glyph range, sprite
/// sheet): cache first, upstream on miss. `rest` is the percent-encoded path
/// remainder from the webview and is appended to the base URL — the allowlist
/// means the webview can never steer the app to an arbitrary host.
pub fn resource(cache: &Mutex<Connection>, prefix: &str, rest: &str) -> Result<Fetched> {
    let base = sources::resource_base(prefix).ok_or(GeoError::NotFound)?;
    if rest.contains("..") {
        return Err(GeoError::NotFound);
    }
    let key = format!("res/{prefix}/{rest}");
    let url = format!("{}{rest}", base.base_url);
    cached_resource(
        cache,
        &key,
        &url,
        content_type_for(rest, base.content_type),
        false,
    )
}

/// Fetch-with-cache for an internal resource addressed by an explicit key and
/// URL (also used by `style` for TileJSON documents). `refresh` bypasses the
/// cache read (but still stores the new payload).
pub fn cached_resource(
    cache: &Mutex<Connection>,
    key: &str,
    url: &str,
    content_type: &str,
    refresh: bool,
) -> Result<Fetched> {
    if !refresh
        && let Some(hit) = lock(cache)?
            .query_row(
                "SELECT data, content_type FROM resource WHERE key = ?1",
                [key],
                |r| {
                    Ok(Fetched {
                        data: r.get(0)?,
                        content_type: r.get(1)?,
                    })
                },
            )
            .optional()?
    {
        return Ok(hit);
    }

    let fetched = http_get(url, content_type)?;
    lock(cache)?.execute(
        "INSERT OR REPLACE INTO resource (key, data, content_type, fetched_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![key, fetched.data, fetched.content_type, now_utc_iso()],
    )?;
    Ok(fetched)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn lock(cache: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>> {
    // A poisoned mutex means another thread panicked mid-write; treat the
    // cache as unavailable rather than propagating the panic.
    cache
        .lock()
        .map_err(|_| GeoError::Offline("geo cache lock poisoned".into()))
}

fn cached_tile(
    conn: &Connection,
    source_id: &str,
    z: u8,
    x: u32,
    y: u32,
) -> Result<Option<Fetched>> {
    Ok(conn
        .query_row(
            "SELECT data, content_type FROM tile
             WHERE source = ?1 AND z = ?2 AND x = ?3 AND y = ?4",
            params![source_id, z, x, y],
            |r| {
                Ok(Fetched {
                    data: r.get(0)?,
                    content_type: r.get(1)?,
                })
            },
        )
        .optional()?)
}

/// The upstream URL for one tile: a static template, or one resolved from the
/// source's TileJSON document (cached; `refresh` forces a re-resolve).
fn upstream_tile_url(
    cache: &Mutex<Connection>,
    source: &TileSource,
    z: u8,
    x: u32,
    y: u32,
    refresh: bool,
) -> Result<String> {
    let template = match (source.url_template, source.tilejson_url) {
        (Some(template), _) => template.to_string(),
        (None, Some(tilejson_url)) => {
            let key = format!("tilejson/{}", source.id);
            let doc = cached_resource(cache, &key, tilejson_url, "application/json", refresh)?;
            let tilejson: serde_json::Value = serde_json::from_slice(&doc.data)?;
            tilejson["tiles"]
                .get(0)
                .and_then(serde_json::Value::as_str)
                .ok_or(GeoError::Invalid("tilejson_invalid"))?
                .to_string()
        }
        (None, None) => return Err(GeoError::NotFound),
    };
    Ok(template
        .replace("{z}", &z.to_string())
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string()))
}

/// Sprite sheets mix JSON and PNG under one base; pick by extension.
fn content_type_for(rest: &str, fallback: &'static str) -> &'static str {
    if rest.ends_with(".json") {
        "application/json"
    } else if rest.ends_with(".png") {
        "image/png"
    } else if rest.ends_with(".pbf") {
        "application/x-protobuf"
    } else {
        fallback
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            // Identify politely to the public services we cache from.
            .user_agent("Terrazgo/0.1 (offline-first farm app)")
            .build()
            .into()
    })
}

fn http_get(url: &str, fallback_content_type: &str) -> Result<Fetched> {
    let mut response = agent().get(url).call().map_err(|err| match err {
        ureq::Error::StatusCode(status) => GeoError::Http { status },
        other => GeoError::Offline(other.to_string()),
    })?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(fallback_content_type)
        .to_string();
    let data = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| GeoError::Offline(e.to_string()))?;
    Ok(Fetched { data, content_type })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_cache_in_memory;

    fn cache() -> Mutex<Connection> {
        Mutex::new(open_cache_in_memory().expect("in-memory cache"))
    }

    #[test]
    fn cached_tile_roundtrip_and_miss() {
        let cache = cache();
        {
            let conn = cache.lock().unwrap();
            assert!(
                cached_tile(&conn, "pnoa", 13, 3990, 3105)
                    .unwrap()
                    .is_none()
            );
            conn.execute(
                "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at)
                 VALUES ('pnoa', 13, 3990, 3105, x'FFD8', 'image/jpeg', '2026-07-07T00:00:00Z')",
                [],
            )
            .unwrap();
        }
        // A cache hit is served without any network (this test has none).
        let hit = tile(&cache, "pnoa", 13, 3990, 3105).unwrap();
        assert_eq!(hit.data, vec![0xFF, 0xD8]);
        assert_eq!(hit.content_type, "image/jpeg");
    }

    #[test]
    fn unknown_source_and_excess_zoom_are_not_found() {
        let cache = cache();
        assert!(matches!(
            tile(&cache, "no-such-source", 1, 0, 0),
            Err(GeoError::NotFound)
        ));
        // pnoa max_zoom is 19.
        assert!(matches!(
            tile(&cache, "pnoa", 20, 0, 0),
            Err(GeoError::NotFound)
        ));
    }

    #[test]
    fn resource_rejects_unknown_prefix_and_traversal() {
        let cache = cache();
        assert!(matches!(
            resource(&cache, "not-allowlisted", "x"),
            Err(GeoError::NotFound)
        ));
        assert!(matches!(
            resource(&cache, "ofm-fonts", "../../etc/passwd"),
            Err(GeoError::NotFound)
        ));
    }

    #[test]
    fn cached_resource_is_served_without_network() {
        let cache = cache();
        cache
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO resource (key, data, content_type, fetched_at)
                 VALUES ('res/ofm-style/', x'7B7D', 'application/json', '2026-07-07T00:00:00Z')",
                [],
            )
            .unwrap();
        let hit = resource(&cache, "ofm-style", "").unwrap();
        assert_eq!(hit.data, b"{}");
    }

    #[test]
    fn tile_urls_substitute_placeholders() {
        let cache = cache();
        let pnoa = sources::tile_source("pnoa").unwrap();
        let url = upstream_tile_url(&cache, pnoa, 13, 3990, 3105, false).unwrap();
        assert!(url.contains("tilematrix=13"));
        assert!(url.contains("tilerow=3105"));
        assert!(url.contains("tilecol=3990"));
    }
}
