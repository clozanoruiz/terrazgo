-- Terrazgo geo-cache — migration 0001: tile/resource cache schema.
--
-- This is geo-cache.db, a SEPARATE database from terrazgo.db, with its own
-- (tiny) migration sequence. Everything here is bulky, re-fetchable, derived
-- data: it is deliberately excluded from backups (`VACUUM INTO` runs on the
-- user db only), from record_change, and from any future sync. Deleting the
-- file loses nothing but warm caches.

-- Map tiles, keyed by our source id (see terrazgo-geo sources.rs), not by
-- upstream URL — upstream templates may rotate (dated snapshot paths) while
-- the logical tile stays the same. Campaign-keyed sources suffix the id
-- (`sigpac-recintos@2026`). last_used_at powers the LRU size cap (see
-- db.rs enforce_tile_cache_cap): serving a tile touches it at most once
-- per UTC day, eviction removes the stalest first.
CREATE TABLE tile (
    source       TEXT    NOT NULL,
    z            INTEGER NOT NULL,
    x            INTEGER NOT NULL,
    y            INTEGER NOT NULL,
    data         BLOB    NOT NULL,
    content_type TEXT    NOT NULL,
    fetched_at   TEXT    NOT NULL,
    last_used_at TEXT    NOT NULL,
    PRIMARY KEY (source, z, x, y)
);

-- Non-tile HTTP resources (style JSON, TileJSON, glyph ranges, sprites),
-- keyed by their geo:// path.
CREATE TABLE resource (
    key          TEXT PRIMARY KEY,
    data         BLOB NOT NULL,
    content_type TEXT NOT NULL,
    fetched_at   TEXT NOT NULL
);
