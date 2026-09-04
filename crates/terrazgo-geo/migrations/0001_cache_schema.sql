-- Terrazgo geo-cache — migration 0001: tile/resource cache schema.
--
-- This is geo-cache.db, a SEPARATE database from terrazgo.db, with its own
-- (tiny) migration sequence. Everything here is bulky, re-fetchable, derived
-- data: it is deliberately excluded from backups (`VACUUM INTO` runs on the
-- user db only), from record_change, and from any future sync. Deleting the
-- file loses nothing but warm caches.
--
-- Column comments live INSIDE each CREATE TABLE on purpose: SQLite stores the
-- statement text verbatim in `sqlite_schema`, so they travel into the database
-- file and show up in `.schema` and any inspection tool. Comments written
-- above a statement (like this block) are discarded at parse time and exist
-- only in this file.

-- Map tiles: the raster and vector map imagery the webview draws, cached
-- byte-for-byte as the upstream served it.
--
-- WHERE THE ROWS COME FROM: `terrazgo_geo::fetch`, on a cache miss, over the
-- one sanctioned network seam (terrazgo-net). Nothing else writes here, and
-- the webview never reaches upstream itself — it asks the `geo://` protocol
-- handler, which reads this table first.
--
-- Keyed by OUR source id rather than the upstream URL (see sources.rs):
-- upstream templates rotate (OpenFreeMap publishes dated snapshot paths) while
-- the logical tile is unchanged, so a URL key would throw away a warm cache on
-- every rotation.
--
-- WHAT PRUNES IT: `db.rs::enforce_tile_cache_cap`, an LRU eviction under a
-- user-settable ceiling (`tile_cache_max_bytes`, default 512 MiB). This table
-- is the ONLY one capped — see the `resource` comment for why that asymmetry
-- is deliberate.
CREATE TABLE tile (
    -- Our own source id from sources.rs ('openfreemap', 'pnoa-ortho',
    -- 'sigpac-recintos@2026'). Campaign-keyed sources carry an '@{year}'
    -- suffix because the SIGPAC endpoint always serves the CURRENT campaign
    -- with no year in the URL: without the suffix, tiles from two campaigns
    -- would silently mix after the ~February rollover.
    source       TEXT    NOT NULL,
    -- Standard XYZ tile address. z is the zoom level; x and y are the column
    -- and row within it, y counted from the top (the XYZ convention, not TMS).
    -- Each source declares the zoom range it actually serves.
    z            INTEGER NOT NULL,
    x            INTEGER NOT NULL,
    y            INTEGER NOT NULL,
    -- The response body, unmodified. MAY BE ZERO LENGTH, and that is a
    -- cached answer rather than a failure: SIGPAC answers HTTP 404 for a tile
    -- containing no recintos, and sources marked `empty_on_404` store that as
    -- an empty payload — an empty body is a valid empty vector tile, and
    -- re-asking upstream about known-empty countryside on every pan is rude.
    -- Offline, such a tile must read as "nothing here", never as an error.
    data         BLOB    NOT NULL,
    -- MIME type to serve back on the geo:// response, taken from the upstream
    -- header or from the source's declared default when it did not say.
    content_type TEXT    NOT NULL,
    -- When this copy was downloaded (ISO 8601 UTC). Informational: nothing
    -- expires on age, because a map tile does not go stale in a way a farmer
    -- would notice, and evicting by fetch date would throw away exactly the
    -- tiles of the farm being worked daily.
    fetched_at   TEXT    NOT NULL,
    -- When the tile was last SERVED (ISO 8601 UTC) — the LRU key eviction
    -- orders by. Touched at most once per UTC day, so a burst of panning does
    -- not turn a read-heavy workload into a write-heavy one.
    last_used_at TEXT    NOT NULL,
    -- The tile's full address; there is no surrogate id because the address
    -- IS the identity and every read is a point lookup on all four parts.
    PRIMARY KEY (source, z, x, y)
);

-- Non-tile HTTP resources: map style JSON, TileJSON documents, glyph (font)
-- ranges and sprite sheets, plus the SIGPAC lookup and zone-check responses.
--
-- WHERE THE ROWS COME FROM: the same cache-through fetch as `tile`, for the
-- allowlisted `geo://res/{prefix}/{rest}` paths in sources.rs. The allowlist is
-- what stops the webview using the protocol to reach arbitrary hosts.
--
-- DELIBERATELY UNCAPPED, unlike `tile`. Styles, glyphs and sprites are small
-- and bounded, and the SIGPAC lookup and zone responses stored here are what
-- keeps an already-verified plot verifiable with no network. Evicting those to
-- save kilobytes would quietly break a compliance promise, so the size cap
-- covers tiles only.
CREATE TABLE resource (
    -- The geo:// path this was fetched for, which is both the cache key and
    -- the request the protocol handler answers with it.
    key          TEXT PRIMARY KEY,
    data         BLOB NOT NULL,   -- the response body, unmodified
    content_type TEXT NOT NULL,   -- MIME type to serve back
    fetched_at   TEXT NOT NULL    -- ISO 8601 UTC; informational, nothing expires on age
);
