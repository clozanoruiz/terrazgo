-- Terrazgo core — migration 0001: core-owned schema (DDL only; seed data lives in 0002).
--
-- These tables moved here from module-cue's 0001 on 2026-06-12 (a free pre-release
-- squash edit). Ownership line: the core owns the FARM REGISTRY — land (farm, plot),
-- calendar (season), people (operator), machines (machinery), crops on the land,
-- their regional extensions and the lookups they reference — plus record_change,
-- the cross-cutting audit/sync infrastructure every module writes to. Modules own
-- their domain (CUE: products, treatments, alerts). Core steps run FIRST in the
-- composed global sequence, so module tables may reference these.
--
-- Pre-release this file is squashed freely (dev databases are recreated, not migrated);
-- it becomes append-only the moment any database contains real data. See docs/architecture.md →
-- Migrations: one global sequence.
--
-- Conventions (see docs/data-model.md):
--   * snake_case, singular table names, lowercase English enum values.
--   * User-data PKs are UUIDv7 stored as 36-char TEXT, generated in Rust at insert.
--   * Reference/lookup tables use short stable TEXT codes (or INTEGER) and ship seeded.
--   * Dates: ISO 8601 TEXT in UTC ('YYYY-MM-DDTHH:MM:SSZ'); date-only as 'YYYY-MM-DD'.
--   * No user-facing strings here — reference tables carry an i18n_key only.
--   * foreign_keys = ON and journal_mode = WAL are set at connection time, not here.

-- ============================================================================
-- Reference / lookup tables (app-versioned, seeded in 0002, not synced)
-- ============================================================================

CREATE TABLE country (
    code     TEXT PRIMARY KEY,   -- ISO 3166-1 alpha-2, lowercase: 'es', 'fr', 'it'
    i18n_key TEXT NOT NULL
);

CREATE TABLE production_system (
    code     TEXT PRIMARY KEY,   -- 'conventional', 'organic', 'integrated'
    i18n_key TEXT NOT NULL
);

CREATE TABLE licence_level (
    code     TEXT PRIMARY KEY,   -- 'basic', 'qualified', 'fumigator' (Spanish carné today; regional mapping is config)
    i18n_key TEXT NOT NULL
);

-- ============================================================================
-- Core user-data tables (UUIDv7 TEXT PKs)
-- ============================================================================

CREATE TABLE season (
    id            TEXT PRIMARY KEY,
    campaign_year INTEGER NOT NULL,           -- Spanish PAC campaign year, e.g. 2026
    label         TEXT NOT NULL,
    starts_on     TEXT,                       -- 'YYYY-MM-DD'
    ends_on       TEXT,
    status        TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'archived'
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE farm (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    owner_name    TEXT,
    location_text TEXT,
    latitude      REAL,
    longitude     REAL,
    -- Country is a universal core concept (not a regional extension); treatment records
    -- derive their country from here.
    country_code  TEXT NOT NULL REFERENCES country(code),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);

-- Spanish regional extension for farm: REGA code never lives in the core table.
CREATE TABLE farm_es_extension (
    farm_id       TEXT PRIMARY KEY REFERENCES farm(id) ON DELETE CASCADE,
    rega_code     TEXT,
    province_code TEXT
);

CREATE TABLE plot (
    id         TEXT PRIMARY KEY,
    farm_id    TEXT NOT NULL REFERENCES farm(id),
    name       TEXT NOT NULL,
    area_ha    REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

-- Spanish regional extension for plot: SIGPAC reference, kept out of the core table.
CREATE TABLE plot_es_extension (
    plot_id             TEXT PRIMARY KEY REFERENCES plot(id) ON DELETE CASCADE,
    sigpac_province     TEXT,
    sigpac_municipality TEXT,
    sigpac_aggregate    TEXT,
    sigpac_zone         TEXT,
    sigpac_polygon      TEXT,
    sigpac_parcel       TEXT,
    sigpac_enclosure    TEXT
);

-- The crop present on a plot in a given season ("crop at time of treatment" links here).
CREATE TABLE crop (
    id                     TEXT PRIMARY KEY,
    plot_id                TEXT NOT NULL REFERENCES plot(id),
    season_id              TEXT NOT NULL REFERENCES season(id),
    species_name           TEXT NOT NULL,
    variety                TEXT,
    production_system_code TEXT REFERENCES production_system(code),
    sown_on                TEXT,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL,
    deleted_at             TEXT
);

CREATE TABLE operator (
    id                  TEXT PRIMARY KEY,
    full_name           TEXT NOT NULL,
    licence_number      TEXT,
    licence_level_code  TEXT REFERENCES licence_level(code),
    licence_expiry_date TEXT,                   -- 'YYYY-MM-DD'; drives licence_expiry alerts
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

CREATE TABLE machinery (
    id                       TEXT PRIMARY KEY,
    farm_id                  TEXT NOT NULL REFERENCES farm(id),
    name                     TEXT NOT NULL,
    type                     TEXT,
    last_inspection_date     TEXT,
    next_inspection_due_date TEXT,              -- ITV due date; drives itv_expiry alerts
    created_at               TEXT NOT NULL,
    updated_at               TEXT NOT NULL,
    deleted_at               TEXT
);

-- Spanish regional extension for machinery, kept out of the core table. Two
-- complementary registries: ROMA for mobile machinery (the typical sprayer),
-- REGANIP for aircraft and fixed/semi-mobile installations (greenhouses,
-- post-harvest). Normally exclusive per equipment, but not enforced.
CREATE TABLE machinery_es_extension (
    machinery_id   TEXT PRIMARY KEY REFERENCES machinery(id) ON DELETE CASCADE,
    roma_number    TEXT,
    reganip_number TEXT
);

-- Geometry attached to a core entity (plot boundary today; farm boundary,
-- irrigation features later). USER DATA: synced, audit-logged, soft-deleted —
-- fetched geometry cannot be re-derived offline, so it must roam, unlike alerts.
--
-- Subject linkage is an EXCLUSIVE ARC: one nullable FK column per subject type,
-- with a CHECK that exactly one is set. Deliberately NOT the polymorphic
-- (entity_table, entity_id) pattern of record_change/alert — those rows must
-- outlive or re-derive their subjects, while a geometry must die with its
-- subject, and the arc keeps real FK enforcement (orphans impossible). A new
-- subject type later = one nullable ADD COLUMN (cheap even post-release).
--
-- Rows from different sources COEXIST (a SIGPAC-fetched boundary next to a
-- manually drawn one → discrepancy display); display precedence is a UI concern.
-- Replacement soft-deletes the previous active row, so history is kept.
CREATE TABLE geo_feature (
    id               TEXT PRIMARY KEY,
    plot_id          TEXT REFERENCES plot(id) ON DELETE CASCADE,
    farm_id          TEXT REFERENCES farm(id) ON DELETE CASCADE,
    role             TEXT NOT NULL,       -- 'boundary' today; open set, lowercase English
    geometry         TEXT NOT NULL,       -- GeoJSON geometry object, EPSG:4326 (lon/lat)
    source           TEXT NOT NULL,       -- 'manual' | 'import' | future 'sigpac' | …
    campaign         INTEGER,             -- provider campaign year; NULL for manual/import
    official_area_ha REAL,                -- provider-declared surface; never copied to plot.area_ha
    properties       TEXT,                -- provider-specific attributes as JSON, keyed per source
    fetched_at       TEXT,                -- when a provider fetched it; NULL for manual/import
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT,
    CHECK ((plot_id IS NOT NULL) + (farm_id IS NOT NULL) = 1)
);

CREATE INDEX idx_geo_feature_plot ON geo_feature(plot_id);
CREATE INDEX idx_geo_feature_farm ON geo_feature(farm_id);
-- At most ONE active row per (subject, role, source): replacement is
-- soft-delete + insert in one transaction, enforced by construction.
CREATE UNIQUE INDEX idx_geo_feature_active_plot
    ON geo_feature(plot_id, role, source) WHERE deleted_at IS NULL AND plot_id IS NOT NULL;
CREATE UNIQUE INDEX idx_geo_feature_active_farm
    ON geo_feature(farm_id, role, source) WHERE deleted_at IS NULL AND farm_id IS NOT NULL;

-- Regulatory zone kinds a plot can intersect (nitrate-vulnerable, phyto
-- restriction, Natura 2000 today). Universal LPIS concept — a new type or a
-- new country's zones are new ROWS + i18n keys, never a migration.
CREATE TABLE zone_type (
    code     TEXT PRIMARY KEY,
    i18n_key TEXT NOT NULL
);

-- Provider-checked zone intersections per plot and campaign (added
-- 2026-07-08; design history in docs/sigpac-integration.md). Unlike alerts,
-- flags CANNOT be re-derived offline (they come from a provider query), so
-- they are user data: record_change-logged, synced, in backups.
--
-- Negatives are stored: status='outside' is inspection-grade proof the check
-- ran in that campaign and was clear — absence stays "never checked".
-- Re-checking replaces (soft-delete + insert) within (plot, type, campaign,
-- source); a new campaign appends, so past duties remain provable.
CREATE TABLE plot_zone_flag (
    id             TEXT PRIMARY KEY,
    plot_id        TEXT NOT NULL REFERENCES plot(id) ON DELETE CASCADE,
    zone_type_code TEXT NOT NULL REFERENCES zone_type(code),
    campaign       INTEGER NOT NULL,   -- provider campaign year checked against
    status         TEXT NOT NULL CHECK (status IN ('inside', 'outside')),
    coverage_pct   REAL,               -- provider's intersection percentage; NULL when outside
    detail         TEXT,               -- provider detail (e.g. 'Zona periférica'); user-visible verbatim
    source         TEXT NOT NULL,      -- 'sigpac' | future providers
    checked_at     TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    deleted_at     TEXT
);

CREATE INDEX idx_plot_zone_flag_plot ON plot_zone_flag(plot_id);
CREATE UNIQUE INDEX idx_plot_zone_flag_active
    ON plot_zone_flag(plot_id, zone_type_code, campaign, source)
    WHERE deleted_at IS NULL;

-- Append-only audit log AND future sync delta source. Deliberately has NO foreign keys:
-- it references many tables polymorphically and must outlive the rows it records.
CREATE TABLE record_change (
    id            TEXT PRIMARY KEY,
    entity_table  TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    season_id     TEXT,
    operation     TEXT NOT NULL,                  -- 'insert' | 'update' | 'delete'
    changed_at    TEXT NOT NULL,
    actor         TEXT,                           -- device/user id, for future sync
    payload       TEXT NOT NULL                   -- JSON {"before": ..., "after": ...}
);

CREATE INDEX idx_record_change_entity ON record_change(entity_table, entity_id);
CREATE INDEX idx_crop_plot_season     ON crop(plot_id, season_id);
