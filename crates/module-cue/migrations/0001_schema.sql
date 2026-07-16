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

-- Treatment efficacy as observed after application (2026-07-15). Small closed
-- list with universal meaning → English-coded lookup mapped to each country's
-- export coding at serialization (the unit/reason_category pattern); Spain:
-- SIEX EFICACIA_TRATAMIENTO. A contract test keeps the export mapping in sync
-- with the vendored catalogue snapshot.
CREATE TABLE efficacy (
    code     TEXT PRIMARY KEY,   -- 'good', 'fair', 'poor'
    i18n_key TEXT NOT NULL
);

-- Why the treatment was applied — the IPM justifications of Directive
-- 2009/128/CE (thresholds, monitoring, DSS, official warning, advisor…).
-- Same pattern as efficacy; Spain: SIEX JUSTIFICACION_ACTUACION.
CREATE TABLE justification (
    code     TEXT PRIMARY KEY,   -- 'threshold_exceeded', 'monitoring', …
    i18n_key TEXT NOT NULL
);

-- Nature of a product's per-country authorisation (2026-07-15). EU-universal
-- concepts (Reg. 1107/2009: standard registration, parallel trade permit,
-- Art. 53 emergency authorisation); Spain: SIEX TIPO_PRODFITO.
CREATE TABLE authorisation_kind (
    code     TEXT PRIMARY KEY,   -- 'registered', 'common_name', 'parallel_import', 'exceptional'
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
-- kind_code (2026-07-15) classifies the authorisation's nature; the default
-- covers the typical case, so existing forms stay valid. For 'exceptional'
-- authorisations the export must name the substance by its catalogue code
-- (SIEX AUTORIZACION_EXCP → the TratamFito MateriaActiva field, mandatory only
-- for that kind) — stored verbatim, no FK, per the catalogue-code rule.
CREATE TABLE product_authorisation (
    id                         TEXT PRIMARY KEY,
    product_id                 TEXT NOT NULL REFERENCES product(id) ON DELETE CASCADE,
    country_code               TEXT NOT NULL REFERENCES country(code),
    authorisation_number       TEXT NOT NULL,
    kind_code                  TEXT NOT NULL DEFAULT 'registered' REFERENCES authorisation_kind(code),
    exceptional_substance_code TEXT,   -- catalogue code, only meaningful when kind_code = 'exceptional'
    status                     TEXT,
    valid_from                 TEXT,
    valid_until                TEXT,
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
    -- The reason for treatment lives in treatment_problem since 2026-07-15
    -- (each coded problem carries its own category — one record can target a
    -- disease AND a pest); target_organism stays as optional free-text nuance
    -- the coded lists cannot express.
    target_organism               TEXT,
    -- Observed efficacy, assessed AFTER application — nullable by design: on
    -- application day it is unknowable, so the export precheck (not the
    -- insert) demands it. Never force farmers to invent a value at entry.
    efficacy_code                 TEXT REFERENCES efficacy(code),
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

-- The coded phytosanitary problems a treatment targets (≥1 per record,
-- enforced in the repository like the other insert validations; 2026-07-15,
-- design in docs/siex-export.md → gap 3). problem_code is a reference-catalogue
-- code stored verbatim — deliberately NO FK to catalogue_code (the code is the
-- regulatory payload, the catalogue row is display metadata; reimports must
-- never cascade into records). The per-row category picks which catalogue the
-- code resolves against for the record's country (Spain: disease →
-- ENFERMEDADES, pest → PLAGAS, weed → MALAS_HIERBAS, growth_regulator/other →
-- REGULADORES_CRECIMIENTO) and the export bucket it lands in; codes repeat
-- across catalogues, hence the category in the natural key.
CREATE TABLE treatment_problem (
    id                   TEXT PRIMARY KEY,
    treatment_record_id  TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    reason_category_code TEXT NOT NULL REFERENCES reason_category(code),
    problem_code         TEXT NOT NULL,
    UNIQUE (treatment_record_id, reason_category_code, problem_code)
);

-- The IPM justifications behind a treatment (≥1 per record, enforced in the
-- repository; 2026-07-15). Known at treatment time, unlike efficacy.
CREATE TABLE treatment_justification (
    id                  TEXT PRIMARY KEY,
    treatment_record_id TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    justification_code  TEXT NOT NULL REFERENCES justification(code),
    UNIQUE (treatment_record_id, justification_code)
);

-- Integer aliases regulatory exports assign to activity records (2026-07-15,
-- design in docs/siex-export.md → gap 1). SIEX's IdAjena* edit/delete keys are
-- integers ≤ 10 digits, so UUIDs cannot travel; an alias is minted at FIRST
-- export (MAX+1 per target, race-free behind the connection mutex) and then
-- NEVER updated or deleted — stability across exports is the point, and a
-- row's existence doubles as the "previously exported" marker that drives the
-- export's deletion flag for soft-deleted records. split_key discriminates
-- when one record maps to several export entries (a multi-crop treatment
-- splits into one TratamFito per crop); its value is serializer-defined,
-- opaque here. Polymorphic like record_change, so no FK. Synced user data
-- (aliases must roam and survive backups — they cannot be re-derived):
-- insert-logged in record_change. Known limit, recorded in the design doc:
-- two devices exporting independently before syncing could mint colliding
-- aliases — a sync-stage-2 design item, acceptable while one device exports.
CREATE TABLE export_alias (
    id           TEXT PRIMARY KEY,
    target       TEXT NOT NULL,              -- 'siex' | future export regimes
    entity_table TEXT NOT NULL,              -- 'treatment_record' first
    entity_id    TEXT NOT NULL,
    split_key    TEXT NOT NULL DEFAULT '',   -- '' when the record maps 1:1
    alias        INTEGER NOT NULL,
    created_at   TEXT NOT NULL,
    UNIQUE (target, entity_table, entity_id, split_key),
    UNIQUE (target, alias)
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
CREATE INDEX idx_treatment_problem_treatment ON treatment_problem(treatment_record_id);
CREATE INDEX idx_treatment_justification_treatment ON treatment_justification(treatment_record_id);
CREATE INDEX idx_treatment_record_season  ON treatment_record(season_id);
CREATE INDEX idx_treatment_record_farm    ON treatment_record(farm_id);
CREATE INDEX idx_product_auth_product     ON product_authorisation(product_id, country_code);
CREATE INDEX idx_alert_status_due         ON alert(status, due_date);
