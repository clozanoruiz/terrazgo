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
    code     TEXT PRIMARY KEY,   -- 'basic', 'qualified', 'fumigator', 'pilot' (Spanish carné today; regional mapping is config)
    i18n_key TEXT NOT NULL
);

-- Units of measure, shared by every module that records an amount.
--
-- Moved here from module-cue on 2026-08-07: module-fertilisation records
-- fertiliser doses and irrigation volumes, and modules may never depend on
-- each other, so the vocabulary had to sit below both. It also lets
-- `harvest_record` below carry a real foreign key instead of a repository-only
-- rule. `dimension` separates the three questions a number can answer:
--   * 'dose_rate'     — how much per hectare ('l_ha', 'kg_ha', 'm3_ha')
--   * 'concentration' — how much per volume of mix ('g_l', 'pct')
--   * 'quantity'      — how much in total, actually used or treated
--                       (Anexo III Parte I B.i's "kilogramos o litros", and the
--                       tonnes / cubic metres the non-field registers ask for)
-- Mixing them is a false statement, not a formatting slip, so the selectors
-- are separate lists (`list_units` excludes quantities; `list_quantity_units`
-- is its own).
CREATE TABLE unit (
    code      TEXT PRIMARY KEY,  -- 'l_ha', 'kg_ha', 'g_l', 'pct', 'kg'
    dimension TEXT NOT NULL,     -- 'dose_rate' | 'concentration' | 'quantity' | 'intensity'
    i18n_key  TEXT NOT NULL
);

-- How a crop is watered. NOT a boolean: RD 1311/2012 Anexo III A.2.e asks for
-- "secano o regadío (indicando en su caso el sistema de riego)", and the
-- official model prints four siglas (SEC/ASP/LOC/GRA). The siglas are Spanish
-- form vocabulary and live in the report template; the codes stay English.
CREATE TABLE irrigation_system (
    code     TEXT PRIMARY KEY,   -- 'rainfed', 'sprinkler', 'drip', 'gravity'
    i18n_key TEXT NOT NULL
);

-- Open air or under cover, and under what (Anexo III A.2.e "al aire libre o
-- protegido, indicando en su caso el tipo de protección"). Also what the
-- RD 34/2025 >0.1 ha greenhouse threshold needs to be knowable.
CREATE TABLE growing_environment (
    code     TEXT PRIMARY KEY,   -- 'open_air', 'mesh', 'plastic_cover', 'greenhouse'
    i18n_key TEXT NOT NULL
);

-- The integrated pest management framework a holding (or a single crop)
-- operates under: RD 1311/2012 art. 10-11, printed by the official model in
-- 1.4 ("tipo de explotación") and again per row in 2.1. The model's siglas
-- (AE/PI/CP/Atrias/AS/NO) are Spanish form vocabulary and live in the report
-- template; the codes stay English. 'not_required' is a real answer, not a
-- missing one — most conventional holdings are under no GIP advisory duty.
CREATE TABLE gip_system (
    code     TEXT PRIMARY KEY,   -- 'organic', 'integrated_production', 'private_certification', 'atria', 'advisor_assisted', 'not_required'
    i18n_key TEXT NOT NULL
);

-- Imported reference catalogues (added 2026-07-14; design in docs/siex-export.md
-- → "Storage design"). Generic on purpose: the mechanism is country-neutral and
-- the Spanish-ness is data — each catalogue carries its provider `source`
-- ('siex' today) and provider columns ride verbatim in `attrs` JSON, the
-- geo_feature precedent. Promote a catalogue to a typed table only when a real
-- query needs its attributes; promotion is an additive copy, codes never change.
-- INTEGER PKs: shipped reference data, not user data — the UUID rule doesn't
-- apply. Excluded from record_change: each device imports its own copy from the
-- snapshot vendored in the binary (crates/terrazgo-core/catalogues/).
--
-- Imports are UPSERT-ONLY, never delete: providers retire codes by baja date
-- instead of removing them, so a code on an old record keeps resolving forever.
--
-- Deliberately NO foreign keys from user data to catalogue_code: the code value
-- is the regulatory payload, the catalogue row is display metadata; a reimport
-- must never cascade into user records. Bogus codes are caught in Rust and by
-- the export's schema-validated tests instead.
CREATE TABLE catalogue (
    id                TEXT PRIMARY KEY,  -- provider table id (SIEX: the idTabla, e.g. 'EFICACIA_TRATAMIENTO')
    source            TEXT NOT NULL,     -- 'siex' | future providers
    source_updated_at TEXT,              -- newest lifecycle date across rows at import; NULL when the provider ships none
    source_digest     TEXT,              -- content hash of the vendored file at import: what the startup fast path compares, so a refreshed snapshot is detected even when it moved no lifecycle date (and so an unchanged one is skipped without being parsed)
    imported_at       TEXT NOT NULL
);

CREATE TABLE catalogue_code (
    id           INTEGER PRIMARY KEY,
    catalogue_id TEXT NOT NULL REFERENCES catalogue(id),
    code         TEXT NOT NULL,          -- provider code; NOT unique per catalogue — some catalogues repeat a code per qualifying attr (e.g. one row per ámbito)
    label        TEXT NOT NULL,          -- current provider label; deliberately never snapshotted onto records — the code is what's legal, a renamed label should show its new text
    attrs        TEXT,                   -- JSON object of the provider's remaining columns, keys verbatim; NULL when the catalogue is plain code+label
    added_on     TEXT,                   -- provider lifecycle dates as ISO 'YYYY-MM-DD' (alta / modificación / baja)
    modified_on  TEXT,
    retired_on   TEXT                    -- retired codes stay resolvable for old records; pickers filter retired_on IS NULL
);

CREATE INDEX idx_catalogue_code_lookup ON catalogue_code(catalogue_id, code);

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
    updated_at    TEXT NOT NULL,
    -- Soft delete, orthogonal to `status`: archiving retires a season that still
    -- holds records, deleting removes one created by mistake. Only an EMPTY
    -- season may be deleted (no crops, no treatment records) — hiding a season
    -- that owns regulatory records would hide the records with it, since every
    -- record-book view is season-scoped.
    deleted_at    TEXT
);

CREATE TABLE farm (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    owner_name    TEXT,
    -- Tax/identity number of the legal holder (titular): NIF in Spain, CUAA in
    -- Italy, SIREN in France… The *concept* is universal — every country's
    -- regulatory export names the holder — so it lives in core; format
    -- validation is per-country config. User-entered from the farm's registry
    -- papers, never derivable (2026-07-15; SIEX export needs it as IdTitular).
    owner_tax_id  TEXT,
    location_text TEXT,
    -- Postal contact details of the holding (Anexo III A.1.a "nombre, dirección
    -- de la explotación"). Universal — every country's record book asks for
    -- them — so core, not the regional extension.
    address       TEXT,
    postal_code   TEXT,
    phone_fixed   TEXT,
    phone_mobile  TEXT,
    email         TEXT,
    -- "Fecha de apertura del cuaderno" (official model 1.1). The record book is
    -- a continuing document for the holding, so the date belongs to the farm
    -- and not to a campaign — the printed page states the campaign beside it.
    -- NULL prints the model's blank rule, which is what a farmer who never
    -- filled it in should get rather than an invented date.
    opened_on     TEXT,                        -- 'YYYY-MM-DD'
    latitude      REAL,
    longitude     REAL,
    -- Country is a universal core concept (not a regional extension); treatment records
    -- derive their country from here.
    country_code  TEXT NOT NULL REFERENCES country(code),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);

-- The person who signs the record book when that is not the holder: an
-- administrator, an heir, a company representative (official model 1.1
-- "TITULAR O REPRESENTANTE DE LA EXPLOTACIÓN"). At most one per farm and
-- reconciled from the submitted form state exactly like farm_es_extension —
-- absent block means no representative, which is the common case.
-- Deliberately NOT a user_profile: this is a legal capacity recorded in a
-- document, not somebody who uses the app.
CREATE TABLE farm_representative (
    farm_id             TEXT PRIMARY KEY REFERENCES farm(id) ON DELETE CASCADE,
    full_name           TEXT NOT NULL,
    tax_id              TEXT,
    -- Free text: the model prints "Tipo de representación" with no code list
    -- (apoderado, administrador único, heredero…).
    representation_kind TEXT,
    address             TEXT,
    locality            TEXT,
    -- Free text, like the address lines it sits with: this is one line of a
    -- postal address, not the coded administrative geography that
    -- farm_es_extension.province_code carries for the holding itself (which
    -- feeds the report-language map and the export). Coding it would put a
    -- Spanish code list in a core table, and a representative may sit outside
    -- Spain entirely.
    province            TEXT,
    postal_code         TEXT,
    phone               TEXT,
    email               TEXT
);

-- Spanish regional extension for farm: registry codes never live in the core
-- table. rega_code is the *livestock* registry; rea_code (added 2026-07-15) is
-- the farm's registration in its autonomous community's farm registry — the
-- national concept of RD 1054/2022, which each community runs under its own
-- name (REACYL, SIDEAC, …). One column regardless: the SIEX export's
-- CodigoRea, user-entered from the registry's papers (see
-- docs/siex-export.md → REA-first, and its regional-systems table).
CREATE TABLE farm_es_extension (
    farm_id       TEXT PRIMARY KEY REFERENCES farm(id) ON DELETE CASCADE,
    rega_code     TEXT,
    rea_code      TEXT,
    -- The NATIONAL registry number (model 1.1 "Nº Registro de Explotaciones
    -- Nacional"), next to rea_code which is the autonómico one. Both are
    -- printed side by side, so they are separate columns, never one field.
    siex_code     TEXT,
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
    -- Surface this crop occupies on the plot (model 2.1 "Superficie cultivada").
    -- Per crop, not per plot: a plot carrying two crops splits between them, and
    -- printing the whole plot area on each row double-counts it. NULL means "not
    -- stated" and prints blank — never assume the crop fills the plot.
    area_ha                REAL,
    irrigation_code        TEXT REFERENCES irrigation_system(code),
    growing_environment_code TEXT REFERENCES growing_environment(code),
    -- GIP framework for THIS crop (model 2.1's per-row GIP column, Anexo III
    -- A.2.f): a holding can run integrated production on its vineyard and
    -- nothing on its cereal. NULL is not "none" — the report then falls back
    -- to what production_system_code already implies (organic → AE,
    -- integrated → PI), so the column keeps printing for books entered
    -- before this field existed.
    gip_system_code        TEXT REFERENCES gip_system(code),
    sown_on                TEXT,
    -- Species code in the FEGA PRODUCTOS catalogue, stored verbatim and
    -- deliberately WITHOUT a foreign key (the treatment_problem.problem_code
    -- rationale): the catalogue row is display metadata, so a reimport must
    -- never cascade into user records. NULL = a free-text species with no
    -- catalogue match, which stays a valid way to record a crop.
    crop_code              TEXT,
    -- Provenance of the row: 'user' when typed by hand, 'sigpac' when it came
    -- from (or was last restated by) a PAC declaration import.
    source                 TEXT NOT NULL DEFAULT 'user',
    -- Campaign of the declaration this row was imported from. Kept because the
    -- service serves the PREVIOUS campaign: the book must be able to say which
    -- year's declaration a crop came from.
    source_campaign        INTEGER,
    -- The surface the declaration stated (parc_supcult, converted m² → ha).
    -- Stored beside area_ha, never instead of it: area_ha is the farmer's own
    -- figure and the declaration is what a third party recorded.
    declared_area_ha       REAL,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL,
    deleted_at             TEXT
);

CREATE TABLE operator (
    id                  TEXT PRIMARY KEY,
    full_name           TEXT NOT NULL,
    -- Anexo III A.1.c: the model's 1.2 table prints a NIF beside every name.
    -- Universal concept (the person applying is identified by their tax id in
    -- every member state), so core rather than a regional extension.
    tax_id              TEXT,
    licence_number      TEXT,
    licence_level_code  TEXT REFERENCES licence_level(code),
    licence_expiry_date TEXT,                   -- 'YYYY-MM-DD'; drives licence_expiry alerts
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

-- App user profile: who is using the app, for accountability — the future
-- author stamp on record_change.actor and the workflow rule that the
-- applicator records their own treatment (docs/architecture.md → sync
-- conflicts). Identification, not security: no credentials here — real
-- authentication arrives with cloud sync, and a local password on a file
-- the user owns would be theatre. USER DATA: synced, audit-logged,
-- soft-deleted only (a departed worker's id must resolve in years-old
-- audit rows). The ACTIVE profile is a per-device choice and lives in
-- settings.json, not here (docs/architecture.md → Device-local settings).
CREATE TABLE user_profile (
    id           TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    -- Optional "this user is this applicator" link: lets the treatment form
    -- prefill the active user as the operator. NULL for users who never
    -- apply treatments (manager, advisor).
    operator_id  TEXT REFERENCES operator(id),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT
);

-- The advisor, advisory group or advisory entity a holding is attached to
-- (official model 1.4; Anexo III A.1.d, art. 10-11 GIP). A capacity recorded
-- in the book, like farm_representative — never a user_profile, and
-- deliberately NOT a licence_level on operator: ROPO registers applicators and
-- advisors as different conditions, and an advisory entity is frequently a
-- company (Atria, cooperative) rather than a person.
CREATE TABLE advisor (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,      -- person's name or razón social
    tax_id              TEXT,
    -- The model's "Nº de identificación": in Spain the ROPO inscription as an
    -- advisor. Named generically because core tables carry no regional
    -- identifiers — the operator.licence_number precedent.
    registration_number TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

-- Farm ↔ advisor, carrying the GIP framework the holding operates under
-- (model 1.4's "Tipo de explotación"). A junction rather than a column on
-- farm: one advisory entity serves many farms, a farm may hold more than one
-- advisory relationship, and the framework belongs to the relationship.
CREATE TABLE farm_advisor (
    id              TEXT PRIMARY KEY,
    farm_id         TEXT NOT NULL REFERENCES farm(id),
    advisor_id      TEXT NOT NULL REFERENCES advisor(id),
    gip_system_code TEXT REFERENCES gip_system(code),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- One ACTIVE link per (farm, advisor); re-linking a previously removed advisor
-- reuses the row instead of stacking duplicates in table 1.4.
CREATE UNIQUE INDEX idx_farm_advisor_active
    ON farm_advisor(farm_id, advisor_id) WHERE deleted_at IS NULL;

CREATE TABLE machinery (
    id                       TEXT PRIMARY KEY,
    farm_id                  TEXT NOT NULL REFERENCES farm(id),
    name                     TEXT NOT NULL,
    type                     TEXT,
    -- Anexo III A.1.h asks for the acquisition date OR the last inspection
    -- date; equipment too new or too small to need an ITV still has to be
    -- datable in the book, so both columns exist and both print.
    acquired_on              TEXT,              -- 'YYYY-MM-DD'
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

-- Abstraction points for human consumption near a plot: the water half of the
-- printed model's section 2.2, and Anexo III A.1.f-g.
--
-- Printed-model-only, like harvest_plot below: the SIEX 3.11.4 schema has no
-- captación entity at any level (its only water field, OrigenAgua, sits under
-- Riego and Fertirrigacion and codes the provenance of IRRIGATION water). So
-- there is no twin to mirror and no code list to carry -- the requirement here
-- is the decree's, not the interface's.
--
-- Flat and per plot on purpose. inside_plot and distance_m describe the
-- (plot, point) PAIR, not the point, so a well serving two plots would need a
-- junction carrying both columns anyway; it is entered once per plot it
-- concerns, which is exactly what the model's per-plot row states. Real
-- geometry stays out until the Irrigation module wants it, when it belongs in
-- geo_feature rather than here.
CREATE TABLE plot_water_point (
    id           TEXT PRIMARY KEY,
    plot_id      TEXT NOT NULL REFERENCES plot(id) ON DELETE CASCADE,
    denomination TEXT NOT NULL,
    inside_plot  INTEGER NOT NULL CHECK (inside_plot IN (0, 1)),
    -- Required when the point lies outside the plot (A.1.g asks for the
    -- distance in that case), and NULL when it lies inside, where a distance
    -- would contradict the answer above. Both enforced by the repository.
    distance_m   REAL,
    -- Voluntary. WGS84/ETRS89 decimal degrees -- what the whole app speaks
    -- (SIGPAC lookups, geo_feature geometry, the boundary importer's identity
    -- class). The model heads its column "Coordenadas UTM"; the book prints
    -- what is stored and says so, and a UTM rendering can be added later from
    -- these same two numbers without touching the schema.
    latitude     REAL,
    longitude    REAL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT
);

CREATE INDEX idx_plot_water_point_plot ON plot_water_point(plot_id);

-- The stored negative: "checked, and this plot has no abstraction point".
--
-- Same philosophy as plot_zone_flag's status='outside' and as the CUE module's
-- register_declaration: an empty register looks exactly like an unfilled one,
-- and only the first is evidence the farmer asked the question. Section 2.2 is
-- binding, so a blank water cell beside a stated "Sin afección" would read as
-- unfinished work rather than a checked fact.
--
-- Its own table rather than a register_declaration row: that one is
-- module-cue's and farm+season scoped, while this is core, per plot and
-- season-less. Only the shape carries over.
CREATE TABLE plot_water_declaration (
    id          TEXT PRIMARY KEY,
    plot_id     TEXT NOT NULL REFERENCES plot(id) ON DELETE CASCADE,
    declared_on TEXT NOT NULL,   -- 'YYYY-MM-DD', when the farmer said so
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);

-- One live declaration per plot; a withdrawn one keeps its history (soft
-- delete), so re-declaring mints a new row instead of resurrecting the old.
CREATE UNIQUE INDEX idx_plot_water_declaration_active
    ON plot_water_declaration(plot_id)
    WHERE deleted_at IS NULL;

-- Commercialised harvest: model section 5.
--
-- In core, not in the CUE module: what leaves the holding and to whom is
-- whole-farm data the costs and analytics modules will want, and modules never
-- depend on each other. That placement decides two column names below.
--
-- The SIEX twin is `ComercializacionVD` (the sale), not `Cosecha` (the field
-- operation, out of scope). The twin carries neither a plot array nor a buyer
-- of any kind: harvest_plot and the whole client block below exist because the
-- PRINTED model asks for them ("Nº de orden parcela/s de origen", "Cliente"),
-- and the model is the compliance artifact.
CREATE TABLE harvest_record (
    id                    TEXT PRIMARY KEY,
    season_id             TEXT NOT NULL REFERENCES season(id),
    farm_id               TEXT NOT NULL REFERENCES farm(id),
    -- One date. The twin requires a FechaInicio and a FechaFin, which a
    -- serializer satisfies by sending this value as both ends; the model prints
    -- a single "Fecha" column and section 5 is not Anexo III Parte I content.
    harvested_on          TEXT NOT NULL,       -- 'YYYY-MM-DD'
    product_name          TEXT NOT NULL,
    -- Produce code in the FEGA PROD_VEGETAL catalogue, verbatim and without a
    -- foreign key (the crop.crop_code rationale); the twin codes the same thing
    -- as `ProductoVegetal`. NULL = free-text product with no catalogue match.
    --
    -- NOT `crop_code`, and not the catalogue that name belongs to: PROD_VEGETAL
    -- codes what leaves the holding ("Aceitunas"), PRODUCTOS codes what grows
    -- on it ("OLIVO"). Two identical column names against different catalogues
    -- is exactly the confusion this rename ended.
    plant_product_code    TEXT,
    -- Quantity as value + unit code, never free text. The foreign key became
    -- possible on 2026-08-07, when `unit` moved into core: it used to be a
    -- module-cue lookup that core could not reference, so the pairing lived in
    -- the repository alone. The narrower {kg, t} rule stays there — the key
    -- says "a unit", the repository says "a unit that can weigh a harvest".
    -- Both columns are nullable together, because the printed form leaves the
    -- cell to be filled by hand.
    quantity_value        REAL,
    quantity_unit_code    TEXT REFERENCES unit(code),   -- 'kg' | 't'
    -- Both voluntary in the model.
    delivery_note_ref     TEXT,                -- nº de albarán o factura
    lot_number            TEXT,
    buyer_name            TEXT NOT NULL,       -- nombre o razón social
    buyer_tax_id          TEXT,
    buyer_address         TEXT,
    -- The model's "Nº de RGSEAA" (voluntary). Named generically for the same
    -- reason as advisor.registration_number: core tables carry no regional
    -- identifiers, so the Spanish label lives in the report labels and the UI
    -- dictionaries.
    buyer_registry_number TEXT,
    notes                 TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    deleted_at            TEXT
);

-- Where the harvest came from, as the model's "Nº de orden parcela/s de
-- origen". No surface column: the model asks which parcels, not how much of
-- them. Reconciled from the submitted form state on update.
CREATE TABLE harvest_plot (
    id                 TEXT PRIMARY KEY,
    harvest_record_id  TEXT NOT NULL REFERENCES harvest_record(id) ON DELETE CASCADE,
    plot_id            TEXT NOT NULL REFERENCES plot(id),
    crop_id            TEXT REFERENCES crop(id),
    crop_name_snapshot TEXT,                   -- frozen crop at harvest time
    variety_snapshot   TEXT,
    UNIQUE (harvest_record_id, plot_id)
);

CREATE INDEX idx_harvest_record_book ON harvest_record(season_id, farm_id);
CREATE INDEX idx_harvest_plot_rec    ON harvest_plot(harvest_record_id);

-- Append-only audit log AND future sync delta source. Deliberately has NO foreign keys:
-- it references many tables polymorphically and must outlive the rows it records.
CREATE TABLE record_change (
    id            TEXT PRIMARY KEY,
    entity_table  TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    season_id     TEXT,
    operation     TEXT NOT NULL,                  -- 'insert' | 'update' | 'delete'
    changed_at    TEXT NOT NULL,
    actor         TEXT,                           -- user_profile.id of the author (the device's
                                                  -- active profile at write time); NULL = recorded
                                                  -- with no active profile
    payload       TEXT NOT NULL                   -- JSON {"before": ..., "after": ...}
);

CREATE INDEX idx_record_change_entity ON record_change(entity_table, entity_id);
CREATE INDEX idx_crop_plot_season     ON crop(plot_id, season_id);
