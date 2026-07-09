-- Terrazgo CUE module — migration 0001: schema (DDL only; seed data lives in 0002).
--
-- Pre-release this file is squashed freely (dev databases are recreated, not migrated);
-- it becomes append-only the moment any database contains real data. See docs/architecture.md →
-- Migrations: one global sequence. Last squash 2026-06-12: the farm-registry
-- tables (country, farm, plot, season, crop, operator, machinery, their ES extensions
-- and lookups) and record_change moved to the core's 0001_core_schema.sql, which runs
-- EARLIER in the composed sequence — references to those tables remain valid. This
-- module owns the treatment domain: products, treatment records, alerts.
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

CREATE TABLE unit (
    code      TEXT PRIMARY KEY,  -- 'l_ha', 'kg_ha', 'g_l', 'pct'
    dimension TEXT NOT NULL,     -- 'dose_rate' | 'concentration'
    i18n_key  TEXT NOT NULL
);

CREATE TABLE reason_category (
    code     TEXT PRIMARY KEY,   -- 'pest', 'disease', 'weed', 'growth_regulator', 'other'
    i18n_key TEXT NOT NULL
);

CREATE TABLE formulation_type (
    code     TEXT PRIMARY KEY,   -- 'wp', 'sc', 'ec', 'wg', 'sl'
    i18n_key TEXT NOT NULL
);

CREATE TABLE alert_type (
    code     TEXT PRIMARY KEY,   -- 'phi_window', 'licence_expiry', 'itv_expiry'
    i18n_key TEXT NOT NULL
);

-- ============================================================================
-- Core user-data tables (UUIDv7 TEXT PKs)
-- ============================================================================

-- User data, not a lookup: each installation may register substances the app
-- doesn't ship (offline-first — a treatment record must never be blocked on an
-- unknown substance), so rows sync and need collision-free ids (2026-07-02;
-- previously an INTEGER rowid PK). A future MAPA registry import dedupes by
-- cas_number.
CREATE TABLE active_substance (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    cas_number TEXT               -- CAS registry number, the natural cross-device key
);

CREATE TABLE product (
    id                    TEXT PRIMARY KEY,
    commercial_name       TEXT NOT NULL,
    holder                TEXT,                 -- authorisation holder / manufacturer
    formulation_type_code TEXT REFERENCES formulation_type(code),
    default_phi_days      INTEGER,              -- fallback PHI; the value actually used is stored on the record
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    deleted_at            TEXT
);

-- A product has one or more active substances, each at a concentration. The row has its
-- own UUID PK so record_change can address it as (entity_table, entity_id); the natural
-- key is kept as a UNIQUE constraint.
CREATE TABLE product_active_substance (
    id                      TEXT PRIMARY KEY,
    product_id              TEXT NOT NULL REFERENCES product(id) ON DELETE CASCADE,
    active_substance_id     TEXT NOT NULL REFERENCES active_substance(id),
    concentration_value     REAL,
    concentration_unit_code TEXT REFERENCES unit(code),
    UNIQUE (product_id, active_substance_id)
);

-- A product carries a different authorisation number per country (MAPA nº for ES).
CREATE TABLE product_authorisation (
    id                   TEXT PRIMARY KEY,
    product_id           TEXT NOT NULL REFERENCES product(id) ON DELETE CASCADE,
    country_code         TEXT NOT NULL REFERENCES country(code),
    authorisation_number TEXT NOT NULL,
    status               TEXT,
    valid_from           TEXT,
    valid_until          TEXT,
    UNIQUE (product_id, country_code, authorisation_number)
);

-- Central regulatory entity. FKs are kept for querying; the *_snapshot columns freeze
-- the legally-printed values at write time so a later edit to a referenced row can never
-- silently change a past official record. phi_days_used (input) is stored alongside the
-- derived phi_end_date (convention: never store a derived value without its inputs).
CREATE TABLE treatment_record (
    id                            TEXT PRIMARY KEY,
    season_id                     TEXT NOT NULL REFERENCES season(id),
    -- The record belongs to one farm (the cuaderno is per explotación); the farm is the
    -- source for country derivation and every treated plot must be on it.
    farm_id                       TEXT NOT NULL REFERENCES farm(id),
    application_date              TEXT NOT NULL,                 -- 'YYYY-MM-DD'
    product_id                    TEXT NOT NULL REFERENCES product(id),
    country_code                  TEXT NOT NULL REFERENCES country(code),  -- which authorisation context applies
    dose_value                    REAL NOT NULL,
    dose_unit_code                TEXT NOT NULL REFERENCES unit(code),
    reason_category_code          TEXT NOT NULL REFERENCES reason_category(code),
    target_organism               TEXT,
    operator_id                   TEXT NOT NULL REFERENCES operator(id),
    machinery_id                  TEXT REFERENCES machinery(id),
    phi_days_used                 INTEGER NOT NULL,              -- input
    phi_end_date                  TEXT NOT NULL,                 -- derived = application_date + phi_days_used
    -- legal snapshots, frozen at write time:
    product_name_snapshot         TEXT NOT NULL,
    authorisation_number_snapshot TEXT,
    active_substances_snapshot    TEXT,
    operator_name_snapshot        TEXT NOT NULL,
    operator_licence_snapshot     TEXT,
    machinery_roma_snapshot       TEXT,          -- mobile machinery registry (the typical case)
    machinery_reganip_snapshot    TEXT,          -- aircraft / fixed installations registry
    notes                         TEXT,
    created_at                    TEXT NOT NULL,
    updated_at                    TEXT NOT NULL,
    deleted_at                    TEXT
);

-- Junction: one treatment entry applies to many plots, with surface treated per plot.
CREATE TABLE treatment_plot (
    id                  TEXT PRIMARY KEY,
    treatment_record_id TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    plot_id             TEXT NOT NULL REFERENCES plot(id),
    crop_id             TEXT REFERENCES crop(id),
    surface_treated_ha  REAL NOT NULL,            -- may be a partial subset of plot.area_ha
    crop_name_snapshot  TEXT,                     -- frozen crop at treatment time
    variety_snapshot    TEXT,
    UNIQUE (treatment_record_id, plot_id)
);

-- Derived trigger + user acknowledgement state (PHI / licence / ITV). Rows are owned by
-- the reconciling refresh: derived from source tables, deleted when the condition lapses.
-- Derived state → excluded from record_change and from sync (each device re-derives).
CREATE TABLE alert (
    id              TEXT PRIMARY KEY,
    alert_type_code TEXT NOT NULL REFERENCES alert_type(code),
    season_id       TEXT REFERENCES season(id),
    subject_table   TEXT NOT NULL,                -- e.g. 'treatment_record', 'operator', 'machinery'
    subject_id      TEXT NOT NULL,
    due_date        TEXT,
    lead_days_used  INTEGER,                      -- input behind expiry alerts; NULL for phi_window
    status          TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'acknowledged' | 'dismissed'
    acknowledged_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    -- One alert per condition: makes the reconciling refresh idempotent by construction.
    UNIQUE (alert_type_code, subject_table, subject_id)
);

CREATE INDEX idx_treatment_plot_treatment ON treatment_plot(treatment_record_id);
CREATE INDEX idx_treatment_record_season  ON treatment_record(season_id);
CREATE INDEX idx_treatment_record_farm    ON treatment_record(farm_id);
CREATE INDEX idx_product_auth_product     ON product_authorisation(product_id, country_code);
CREATE INDEX idx_alert_status_due         ON alert(status, due_date);
