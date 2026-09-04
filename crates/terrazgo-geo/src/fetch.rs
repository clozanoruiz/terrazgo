// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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
use terrazgo_core::date::now_utc_iso;
use terrazgo_core::db::Database;

/// A fetched (or cached) payload plus the content type to serve it with.
///
/// Defined by [`terrazgo_net`] and re-exported here: a cache hit and a network
/// response must be the same shape, since this module's whole job is that
/// callers cannot tell which one they got.
pub use terrazgo_net::Fetched;

/// Serve one tile: cache first, upstream on miss. For TileJSON-resolved
/// sources (OpenFreeMap publishes rotating dated snapshot paths) a 404 from a
/// stale template triggers one re-resolve + retry.
pub fn tile(cache: &Database, source_id: &str, z: u8, x: u32, y: u32) -> Result<Fetched> {
    let source = sources::tile_source(source_id).ok_or(GeoError::NotFound)?;
    if z < source.min_zoom || z > source.max_zoom {
        return Err(GeoError::NotFound);
    }

    let cache_key = tile_cache_key(cache, source)?;

    // Scoped so the guard is dropped before any network I/O.
    let cached = {
        let guard = cache.lock()?;
        cached_tile(guard.conn()?, &cache_key, z, x, y)?
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
        // Upstream says "no features here" — a cacheable empty tile, not an
        // error (an empty body is a valid empty vector tile).
        Err(GeoError::Http { status: 404 }) if source.empty_on_404 => Ok(Fetched {
            data: Vec::new(),
            content_type: source.content_type.to_string(),
        }),
        other => other,
    }?;

    // Re-taken only now that the network round trip is over.
    let guard = cache.lock()?;
    store_tile(guard.conn()?, source, &cache_key, z, x, y, &fetched)?;
    Ok(fetched)
}

/// The cache row key for one source's tiles. Campaign-keyed sources cache
/// under `{id}@{campaign}` — their upstream URL has no campaign in it, so
/// the key is what keeps tiles from different campaigns apart at rollover.
/// Resolution is cache-first (one fetch ever until something refreshes it,
/// e.g. a plot verification), so it adds no per-tile network cost and works
/// offline once seen.
fn tile_cache_key(cache: &Database, source: &TileSource) -> Result<String> {
    if source.campaign_keyed {
        Ok(format!("{}@{}", source.id, current_campaign(cache, false)?))
    } else {
        Ok(source.id.to_string())
    }
}

/// Store one fetched tile. For campaign-keyed sources this is also the lazy
/// rollover cleanup: storing a tile for the current campaign evicts any
/// earlier campaign's rows for the same source.
fn store_tile(
    conn: &Connection,
    source: &TileSource,
    cache_key: &str,
    z: u8,
    x: u32,
    y: u32,
    fetched: &Fetched,
) -> Result<()> {
    if source.campaign_keyed {
        conn.execute(
            "DELETE FROM tile WHERE source LIKE ?1 AND source <> ?2",
            params![format!("{}@%", source.id), cache_key],
        )?;
    }
    let now = now_utc_iso();
    conn.execute(
        "INSERT OR REPLACE INTO tile
             (source, z, x, y, data, content_type, fetched_at, last_used_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![cache_key, z, x, y, fetched.data, fetched.content_type, now],
    )?;
    Ok(())
}

/// The URL whose directory listing names the available SIGPAC campaigns
/// (`2025/`, `2026/`) — the only machine-readable place the provider states
/// the current campaign (neither the consultas responses nor the MVT URLs
/// carry it).
const CAMPAIGNS_URL: &str = "https://sigpac-hubcloud.es/geopackages/";

/// The current SIGPAC campaign year, read from the provider's download
/// directory listing (max year directory). Cached like everything else, so
/// once seen it resolves offline; `refresh` re-reads at campaign rollover
/// (plot verification does — campaign-keyed tile caching picks the new year
/// up from the shared cache row). Lives here rather than in module-sigpac
/// because campaign-keyed tile caching needs it and modules sit above this
/// crate; module-sigpac re-exports it.
pub fn current_campaign(cache: &Database, refresh: bool) -> Result<i64> {
    let fetched = cached_resource(
        cache,
        "sigpac/campaigns",
        CAMPAIGNS_URL,
        "text/html",
        refresh,
    )?;
    let listing = String::from_utf8_lossy(&fetched.data);
    // Directory anchors look like /geopackages/2026/ — scan for 4-digit years.
    let campaign = listing
        .match_indices("/geopackages/")
        .filter_map(|(at, _)| {
            let year = listing.get(at + "/geopackages/".len()..)?.get(..5)?;
            let (digits, slash) = year.split_at(4);
            (slash == "/").then(|| digits.parse::<i64>().ok()).flatten()
        })
        .max()
        .ok_or(GeoError::Invalid("sigpac_response_invalid"))?;
    Ok(campaign)
}

/// Serve one allowlisted non-tile resource (style JSON, glyph range, sprite
/// sheet): cache first, upstream on miss. `rest` is the percent-encoded path
/// remainder from the webview and is appended to the base URL — the allowlist
/// means the webview can never steer the app to an arbitrary host.
pub fn resource(cache: &Database, prefix: &str, rest: &str) -> Result<Fetched> {
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
    cache: &Database,
    key: &str,
    url: &str,
    content_type: &str,
    refresh: bool,
) -> Result<Fetched> {
    // Scoped so the guard is dropped before the network round trip below.
    let cached = if refresh {
        None
    } else {
        let guard = cache.lock()?;
        guard
            .conn()?
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
    };
    if let Some(hit) = cached {
        return Ok(hit);
    }

    let fetched = http_get(url, content_type)?;
    let guard = cache.lock()?;
    guard.conn()?.execute(
        "INSERT OR REPLACE INTO resource (key, data, content_type, fetched_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![key, fetched.data, fetched.content_type, now_utc_iso()],
    )?;
    Ok(fetched)
}

/// A stored resource together with when it was stored.
///
/// Separate from [`Fetched`] because the timestamp only means something for
/// something already in the cache — a response fresh off the network has not
/// been stored yet.
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub data: Vec<u8>,
    pub content_type: String,
    /// ISO 8601 UTC, as written by [`cached_resource`].
    pub fetched_at: String,
}

/// What the cache already holds for `key`, without ever reaching the network.
///
/// `cached_resource` cannot answer "is this a stored empty answer or has it
/// simply never been asked?" — both look like a cache miss to a caller that
/// only sees the payload. The declared-crops campaign probe needs that
/// difference, and it needs the age too: an empty answer for a campaign still
/// being loaded is re-asked once a day, while the same emptiness for a closed
/// campaign is final.
pub fn cached(cache: &Database, key: &str) -> Result<Option<CachedEntry>> {
    let guard = cache.lock()?;
    let hit = guard
        .conn()?
        .query_row(
            "SELECT data, content_type, fetched_at FROM resource WHERE key = ?1",
            [key],
            |r| {
                Ok(CachedEntry {
                    data: r.get(0)?,
                    content_type: r.get(1)?,
                    fetched_at: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(hit)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn cached_tile(
    conn: &Connection,
    source_id: &str,
    z: u8,
    x: u32,
    y: u32,
) -> Result<Option<Fetched>> {
    let hit = conn
        .query_row(
            "SELECT data, content_type, last_used_at FROM tile
             WHERE source = ?1 AND z = ?2 AND x = ?3 AND y = ?4",
            params![source_id, z, x, y],
            |r| {
                Ok((
                    Fetched {
                        data: r.get(0)?,
                        content_type: r.get(1)?,
                    },
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((fetched, last_used_at)) = hit else {
        return Ok(None);
    };
    // LRU bookkeeping for the size cap (db::enforce_tile_cache_cap), day
    // granularity: a serve touches last_used_at at most once per UTC day, so
    // tile bursts don't turn every cache read into a write.
    let now = now_utc_iso();
    if last_used_at.get(..10) != now.get(..10) {
        conn.execute(
            "UPDATE tile SET last_used_at = ?1
             WHERE source = ?2 AND z = ?3 AND x = ?4 AND y = ?5",
            params![now, source_id, z, x, y],
        )?;
    }
    Ok(Some(fetched))
}

/// The upstream URL for one tile: a static template, or one resolved from the
/// source's TileJSON document (cached; `refresh` forces a re-resolve).
fn upstream_tile_url(
    cache: &Database,
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

/// The one network call this crate makes, through the shared seam. The agent,
/// its TLS trust policy and the Android bootstrap live in [`terrazgo_net`];
/// what stays here is everything around them — caching, keys, eviction.
fn http_get(url: &str, fallback_content_type: &str) -> Result<Fetched> {
    Ok(terrazgo_net::http_get(url, fallback_content_type)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_cache_in_memory;

    fn cache() -> Database {
        open_cache_in_memory().expect("in-memory cache")
    }

    /// Seed one row through a lock that is released before this returns.
    ///
    /// The scoping is the whole point. Every fetch function here takes the
    /// cache lock itself and `std::sync::Mutex` is not reentrant, so a seeding
    /// guard still alive across a later `tile()` or `resource()` call does not
    /// fail the test — it hangs it.
    fn seed(cache: &Database, sql: &str, params: impl rusqlite::Params) {
        let guard = cache.lock().unwrap();
        guard.conn().unwrap().execute(sql, params).unwrap();
    }

    #[test]
    fn cached_tile_roundtrip_and_miss() {
        let cache = cache();
        {
            let guard = cache.lock().unwrap();
            let conn = guard.conn().unwrap();
            assert!(cached_tile(conn, "pnoa", 13, 3990, 3105).unwrap().is_none());
            conn.execute(
                "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at)
                 VALUES ('pnoa', 13, 3990, 3105, x'FFD8', 'image/jpeg',
                         '2026-07-07T00:00:00Z', '2026-07-07T00:00:00Z')",
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
        seed(
            &cache,
            "INSERT INTO resource (key, data, content_type, fetched_at)
             VALUES ('res/ofm-style/', x'7B7D', 'application/json', '2026-07-07T00:00:00Z')",
            [],
        );
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

    // --- SIGPAC recinto MVT overlay (campaign-keyed caching) ----------------

    /// Shape of the real /geopackages/ directory listing (anchors per year).
    const CAMPAIGN_LISTING: &[u8] =
        br#"<a href="/geopackages/2025/">2025/</a> <a href="/geopackages/2026/">2026/</a>"#;

    fn seed_campaigns(cache: &Database, listing: &[u8]) {
        seed(
            cache,
            "INSERT INTO resource (key, data, content_type, fetched_at)
             VALUES ('sigpac/campaigns', ?1, 'text/html', '2026-07-11T00:00:00Z')",
            params![listing],
        );
    }

    #[test]
    fn current_campaign_reads_the_max_year_from_the_listing() {
        let with_years = cache();
        seed_campaigns(&with_years, CAMPAIGN_LISTING);
        assert_eq!(current_campaign(&with_years, false).unwrap(), 2026);

        let without_years = cache();
        seed_campaigns(&without_years, b"<html>no years here</html>");
        assert!(matches!(
            current_campaign(&without_years, false),
            Err(GeoError::Invalid("sigpac_response_invalid"))
        ));
    }

    #[test]
    fn sigpac_tiles_are_served_from_the_campaign_keyed_cache_without_network() {
        let cache = cache();
        seed_campaigns(&cache, CAMPAIGN_LISTING);
        seed(
            &cache,
            "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at)
             VALUES ('sigpac-recintos@2026', 15, 15887, 12108, x'1A00',
                     'application/x-protobuf', '2026-07-11T00:00:00Z', '2026-07-11T00:00:00Z')",
            [],
        );
        // This test has no network; the campaign-suffixed row must be enough.
        let hit = tile(&cache, "sigpac-recintos", 15, 15887, 12108).unwrap();
        assert_eq!(hit.data, vec![0x1A, 0x00]);

        // The key follows the resolved campaign — a row cached under a
        // previous campaign can no longer satisfy lookups after rollover.
        let source = sources::tile_source("sigpac-recintos").unwrap();
        assert_eq!(
            tile_cache_key(&cache, source).unwrap(),
            "sigpac-recintos@2026"
        );
        assert_eq!(
            tile_cache_key(&cache, sources::tile_source("pnoa").unwrap()).unwrap(),
            "pnoa"
        );
    }

    #[test]
    fn sigpac_zoom_bounds_are_enforced() {
        // The MVT service publishes pbf tiles at z12–15 only (service
        // description + live probe, 2026-07-11).
        let cache = cache();
        for z in [11, 16] {
            assert!(matches!(
                tile(&cache, "sigpac-recintos", z, 0, 0),
                Err(GeoError::NotFound)
            ));
        }
    }

    #[test]
    fn serving_a_cached_tile_touches_last_used_at_across_days() {
        let cache = cache();
        seed(
            &cache,
            "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at)
             VALUES ('pnoa', 13, 3990, 3105, x'FFD8', 'image/jpeg',
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        );
        tile(&cache, "pnoa", 13, 3990, 3105).unwrap();
        let guard = cache.lock().unwrap();
        let last_used: String = guard
            .conn()
            .unwrap()
            .query_row("SELECT last_used_at FROM tile", [], |r| r.get(0))
            .unwrap();
        // Touched to today (LRU input for the size cap), not left at January.
        assert_ne!(last_used, "2026-01-01T00:00:00Z");
        assert_eq!(last_used.get(..10), now_utc_iso().get(..10));
    }

    #[test]
    fn storing_a_tile_evicts_earlier_campaigns_of_the_same_source() {
        let cache = cache();
        let guard = cache.lock().unwrap();
        let conn = guard.conn().unwrap();
        conn.execute(
            "INSERT INTO tile (source, z, x, y, data, content_type, fetched_at, last_used_at) VALUES
             ('sigpac-recintos@2025', 15, 1, 1, x'00', 'application/x-protobuf',
              '2025-07-11T00:00:00Z', '2025-07-11T00:00:00Z'),
             ('pnoa', 13, 1, 1, x'00', 'image/jpeg', '2025-07-11T00:00:00Z', '2025-07-11T00:00:00Z')",
            [],
        )
        .unwrap();

        let source = sources::tile_source("sigpac-recintos").unwrap();
        let fetched = Fetched {
            data: vec![0x1A],
            content_type: "application/x-protobuf".into(),
        };
        store_tile(conn, source, "sigpac-recintos@2026", 15, 2, 2, &fetched).unwrap();

        let count = |src: &str| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM tile WHERE source = ?1", [src], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(count("sigpac-recintos@2025"), 0, "old campaign evicted");
        assert_eq!(count("sigpac-recintos@2026"), 1);
        assert_eq!(count("pnoa"), 1, "other sources untouched");
    }

    #[test]
    fn a_closed_cache_reads_as_offline_rather_than_failing_hard() {
        let cache = cache();
        cache.close().unwrap();

        // The cache is exactly the thing that is unavailable, and every caller
        // here already degrades when it cannot be read — the protocol layer
        // answers empty and the map keeps working on what it has. Propagating
        // a hard error instead would take the map down at shutdown.
        assert!(matches!(
            cached(&cache, "res/glyphs/anything"),
            Err(GeoError::Offline(_))
        ));
        assert!(matches!(
            tile(&cache, "pnoa", 13, 3990, 3105),
            Err(GeoError::Offline(_))
        ));
    }
}
