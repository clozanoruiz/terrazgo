-- Terrazgo core — migration 0001: core-owned schema (DDL only; seed data lives in 0002).
--
-- These tables moved here from module-phytosanitary's 0001 on 2026-06-12 (a free pre-release
-- squash edit). Ownership line: the core owns the FARM REGISTRY — land (farm, plot),
-- calendar (season), people (operator), machines (machinery), crops on the land,
-- their regional extensions and the lookups they reference — plus record_change,
-- the cross-cutting audit/sync infrastructure every module writes to. Modules own
-- their domain (phytosanitary: products, treatments). Core steps run FIRST in the
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

-- Countries the app knows how to keep a record book for. Seeded in 0002 and
-- referenced by `farm.country_code`, which is where every treatment record
-- derives the authorisation context it must be judged against.
--
-- This is the FIRST lookup table in the file, so it carries the shape all of
-- them share; the rest are not repeated. A lookup is app-versioned reference
-- data shipped with the binary: short stable TEXT code as the key, an i18n key
-- beside it, no timestamps, no soft delete, never in record_change and never
-- synced. Two rules follow from that and matter more than they look:
--   * the CODE is the durable thing. It is what user rows store and what the
--     exports speak, so a code is never renamed once shipped.
--   * NO user-facing text lives here. `i18n_key` names an entry in the
--     frontend dictionaries (src/i18n/), so the same row prints in Spanish,
--     English or Catalan without the database knowing any of them.
CREATE TABLE country (
    code     TEXT PRIMARY KEY,   -- ISO 3166-1 alpha-2, lowercase: 'es', 'fr', 'it'
    -- Translation key, not a label — resolved by the frontend at display time.
    i18n_key TEXT NOT NULL
);

CREATE TABLE production_system (
    code     TEXT PRIMARY KEY,   -- 'conventional', 'organic', 'integrated'
    i18n_key TEXT NOT NULL
);

-- Operator licence levels. NOT a universal vocabulary: these four are Spain's
-- carné (RD 1311/2012 niveles de capacitación) and France's Certiphyto has its
-- own. The table holds every country's levels; which ones a country OFFERS is
-- `repository::country::licence_level_scheme`, and `lookup_scope.rs` checks the
-- two against each other.
CREATE TABLE licence_level (
    code     TEXT PRIMARY KEY,   -- 'basic', 'qualified', 'fumigator', 'pilot' (Spain's; see licence_level_scheme)
    i18n_key TEXT NOT NULL
);

-- Units of measure, shared by every module that records an amount.
--
-- Moved here from module-phytosanitary on 2026-08-07: module-fertilisation records
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
--
-- NOT a universal vocabulary: 'atria' is a Spanish institution, and another
-- member state meets the same EU integrated-pest-management duty through its
-- own frameworks. Which codes a country OFFERS is
-- `repository::country::gip_system_scheme`, checked by `lookup_scope.rs`.
CREATE TABLE gip_system (
    code     TEXT PRIMARY KEY,   -- Spain's; see gip_system_scheme
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
    id                  TEXT PRIMARY KEY,  -- provider table id (SIEX: the idTabla, e.g. 'EFICACIA_TRATAMIENTO')
    source              TEXT NOT NULL,     -- 'siex' | future providers
    source_updated_at   TEXT,              -- newest lifecycle date across rows at import; NULL when the provider ships none
    source_digest       TEXT,              -- content hash of the bytes that produced the stored rows, whatever their origin: the vendored file, or a copy fetched from the provider. Lets a refresh recognise bytes it already holds and skip parsing them
    -- The app version whose VENDORED snapshot was last imported here; NULL when
    -- only a fetched copy has ever been adopted. This is what startup compares,
    -- and it is a version rather than a hash on purpose: the vendored files are
    -- curated as a SET for a release, so a device must not end up running one
    -- refreshed file mixed with the rest of an older set.
    imported_by_version TEXT,
    imported_at         TEXT NOT NULL      -- when THIS device last adopted the file (ISO 8601 UTC)
);

CREATE TABLE catalogue_code (
    id           INTEGER PRIMARY KEY,
    -- Which published list this code belongs to. The one place a foreign key
    -- to a catalogue is right: it links reference data to reference data, and
    -- both sides are replaced together by an import.
    catalogue_id TEXT NOT NULL REFERENCES catalogue(id),
    code         TEXT NOT NULL,          -- provider code; NOT unique per catalogue — some catalogues repeat a code per qualifying attr (e.g. one row per SIGPAC uso)
    label        TEXT NOT NULL,          -- current provider label; deliberately never snapshotted onto records — the code is what's legal, a renamed label should show its new text
    attrs        TEXT,                   -- JSON object of the provider's remaining columns, keys verbatim; NULL when the catalogue is plain code+label
    -- The provider's own lifecycle dates, as ISO 'YYYY-MM-DD': alta,
    -- modificación, baja. Theirs, not ours — they say what the authority did
    -- to the code, never what this device did with the file.
    added_on     TEXT,
    modified_on  TEXT,
    retired_on   TEXT,                   -- retired codes stay resolvable for old records; pickers filter retired_on IS NULL
    -- OURS, not the provider's, and kept apart from the three dates above for
    -- that reason: the date a fetched file was first seen NOT to carry this row
    -- any more. Providers retire codes by baja date, so a row that simply
    -- vanishes is unexplained — we keep it, because an old record still cites
    -- it, but stop offering it. Only a FETCHED file may set this (it is the
    -- provider's current list); a vendored one proves nothing, since a code can
    -- be missing from it merely by being newer than the release.
    absent_since TEXT
);

CREATE INDEX idx_catalogue_code_lookup ON catalogue_code(catalogue_id, code);

-- ============================================================================
-- Core user-data tables (UUIDv7 TEXT PKs)
-- ============================================================================

-- One holding's campaign — and so one record book: Finca Los Llanos's
-- 2025/2026. The campaign is the universal EU/PAC notion, but each farm keeps
-- its own rows for it, with its own bounds, because the book is kept per
-- explotación, an olive holding's campaign does not run like a cereal one's, and
-- a holding may keep more than one campaign in a year. Nearly every
-- user table below carries a `season_id`, and every record-book view reads
-- through it — which is why deleting a season that owns records is refused
-- rather than cascaded.
--
-- A register row names its farm AND its season, so the two must agree: a
-- record filed under another holding's season would print in neither book.
-- Every register carrying both columns enforces that with a composite foreign
-- key onto (id, farm_id) below. `crop` is the one season-scoped table that
-- cannot — it reaches its farm through its plot — so its insert checks it.
--
-- This is the FIRST user-data table in the file, so it carries the shape they
-- all share; the rest are not repeated:
--   * `id` — UUIDv7 as 36-char hyphenated TEXT, generated in RUST at insert
--     (`Uuid::now_v7()`), never by SQL and never AUTOINCREMENT. v7 keeps
--     insertion order, and a UUID means two devices can both create rows and
--     still merge when sync arrives.
--   * `created_at` / `updated_at` — ISO 8601 UTC instants
--     ('YYYY-MM-DDTHH:MM:SSZ'). Date-only columns use 'YYYY-MM-DD' instead.
--   * `deleted_at` — SOFT DELETE, and on a regulatory record it is the only
--     kind there is: RD 1311/2012 art. 16.3 requires three years' retention,
--     so rows are hidden, never removed, and every read filters
--     `deleted_at IS NULL`. A treatment written years ago must still resolve
--     the plot and operator it names.
-- Every write to a table shaped like this also appends to `record_change`
-- inside the same transaction — see that table at the foot of the file.
CREATE TABLE season (
    id            TEXT PRIMARY KEY,
    -- The holding this campaign's book is kept for. Fixed at creation, like
    -- `plot.farm_id`: moving a season would carry every record in it to another
    -- farm's book.
    farm_id       TEXT NOT NULL REFERENCES farm(id),
    -- The campaign's bounds, 'YYYY-MM-DD', both required: they are what names
    -- the book. Nothing is validated against them and no register refuses a
    -- date outside: they describe the campaign, they do not police it, and a
    -- real farm books an operation early or late.
    starts_on     TEXT NOT NULL,
    ends_on       TEXT NOT NULL,
    -- The name the farmer gave the book, if any. NULL is the everyday case: the
    -- dates name it.
    custom_label  TEXT,
    -- The book's name, what the printed book, the export files and every list
    -- show. DERIVED, and written by Rust on every insert and update from the
    -- columns above: `custom_label` when there is one, otherwise the dates'
    -- years — '2025/2026' when the campaign spans the new year, '2026' when it
    -- does not. Stored because the rule below indexes it and every reader
    -- already reads it; its inputs are stored beside it, so it cannot drift.
    label         TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'archived'
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    -- Soft delete, orthogonal to `status`: archiving retires a season that still
    -- holds records, deleting removes one created by mistake. Only an EMPTY
    -- season may be deleted (no crops, no treatment records) — hiding a season
    -- that owns regulatory records would hide the records with it, since every
    -- record-book view is season-scoped.
    deleted_at    TEXT,
    -- Backstop for the insert's own check (`invalid_date_interval`).
    CHECK (ends_on >= starts_on)
);

-- The parent key of every register's (season_id, farm_id) foreign key.
-- Logically redundant — `id` alone is unique — but SQLite only accepts a
-- composite foreign key onto a UNIQUE index over exactly those columns, and a
-- PARTIAL one does not count: the mistake surfaces as "foreign key mismatch" at
-- the first insert, not when the schema is created. It also serves reading one
-- farm's campaigns.
CREATE UNIQUE INDEX idx_season_farm ON season(farm_id, id);

-- One live book per farm per NAME — not per year: a farm may keep several
-- campaigns in one year, and a second one ending in the same year is simply
-- given a name of its own. What must not exist is two books nobody can tell
-- apart in a list, on a printed cover or in an export file name, which is also
-- what two devices creating the same book offline would produce. A deleted book
-- frees its name. Backstop for the insert's own check (`season_name_taken`).
CREATE UNIQUE INDEX idx_season_farm_label_active
    ON season(farm_id, label) WHERE deleted_at IS NULL;

-- The books current today — whose campaign ended less than a year ago, or has
-- not ended — which is what the Status view's duplicate list starts from
-- (docs/sync.md → Duplicate suspects). A seek, so the list does not grow with
-- the books behind them, however many campaigns a farm keeps.
CREATE INDEX idx_season_ends_active ON season(ends_on) WHERE deleted_at IS NULL;

-- The removed books, which is where the Status view looks for records left
-- live in a book that no longer exists — a book deleted on one device while
-- another recorded into it, or merged into another before a record written
-- elsewhere arrived (docs/sync.md → Records in a removed book). Empty on almost
-- every device, so that list costs a seek into nothing rather than a pass over
-- every book the holdings have ever kept.
CREATE INDEX idx_season_removed ON season(farm_id, id) WHERE deleted_at IS NOT NULL;

-- The holding (explotación): the unit the record book is kept FOR, and the
-- root every other user table hangs off directly or through a plot.
--
-- Model 1.1 ("DATOS DE LA EXPLOTACIÓN") prints most of these columns, and
-- RD 1311/2012 Anexo III Parte I A.1.a asks for the holding's name and address.
-- One book per farm: multi-farm is supported from day one so a smallholder who
-- later works two holdings needs no migration, and so a cooperative remains
-- possible without building for one now.
CREATE TABLE farm (
    id            TEXT PRIMARY KEY,
    -- What the farmer calls the holding; free text, and the only required
    -- field. Not a registry name — the official codes live in
    -- farm_es_extension.
    name          TEXT NOT NULL,
    -- The legal holder (titular). Kept beside `owner_tax_id` rather than
    -- derived from a user profile: the holder is a party named in a legal
    -- document, which is not the same thing as whoever operates the app.
    owner_name    TEXT,
    -- Tax/identity number of the legal holder (titular): NIF in Spain, CUAA in
    -- Italy, SIREN in France… The *concept* is universal — every country's
    -- regulatory export names the holder — so it lives in core; format
    -- validation is per-country config. User-entered from the farm's registry
    -- papers, never derivable (2026-07-15; SIEX export needs it as IdTitular).
    owner_tax_id  TEXT,
    -- Free-text "where it is" for the printed book — a paraje, a village, a
    -- road reference. Deliberately unstructured and unrelated to `address`
    -- below: the postal address of a holding is often not where the land is.
    location_text TEXT,
    -- Postal contact details of the holding (Anexo III A.1.a "nombre, dirección
    -- de la explotación"). Universal — every country's record book asks for
    -- them — so core, not the regional extension.
    address       TEXT,
    postal_code   TEXT,
    -- Two phone columns because the model prints two ("Teléfono fijo" and
    -- "móvil"), not because one is a fallback for the other.
    phone_fixed   TEXT,
    phone_mobile  TEXT,
    email         TEXT,
    -- "Fecha de apertura del cuaderno" (official model 1.1). The record book is
    -- a continuing document for the holding, so the date belongs to the farm
    -- and not to a campaign — the printed page states the campaign beside it.
    -- NULL prints the model's blank rule, which is what a farmer who never
    -- filled it in should get rather than an invented date.
    opened_on     TEXT,                        -- 'YYYY-MM-DD'
    -- Optional decimal degrees (WGS 84) for the holding's centre, used to open
    -- the map somewhere useful. The authoritative geometry of the LAND is not
    -- here — it lives in geo_feature, per plot.
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
    -- Both the key and the link: PRIMARY KEY on the FK is what enforces "at
    -- most one representative per farm" in the schema rather than in Rust.
    -- CASCADE because a representative has no meaning without the holding —
    -- and it is one of the few hard deletes here, since this is a detail of
    -- the farm rather than a regulatory record in its own right.
    farm_id             TEXT PRIMARY KEY REFERENCES farm(id) ON DELETE CASCADE,
    full_name           TEXT NOT NULL,
    -- NIF/NIE of the representative. Same universal concept as
    -- `farm.owner_tax_id`, and equally unvalidated here: format rules are
    -- per-country config.
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
    -- One extension row per farm, hard-deleted with it (CASCADE) and
    -- reconciled from the submitted form: no Spanish block on the form means
    -- no row, which is how a French holding carries none of this.
    farm_id       TEXT PRIMARY KEY REFERENCES farm(id) ON DELETE CASCADE,
    -- REGA — the national LIVESTOCK holding registry (Registro General de
    -- Explotaciones Ganaderas). Only holdings with animals have one; user-
    -- entered from the registry's papers, never derived.
    rega_code     TEXT,
    -- The holding's number in its autonomous community's farm registry
    -- (REACYL in Castilla y León, SIDEAC in Andalucía, …). One column
    -- whatever the community calls it, because the CONCEPT is national
    -- (RD 1054/2022) and only the platform differs.
    rea_code      TEXT,
    -- The NATIONAL registry number (model 1.1 "Nº Registro de Explotaciones
    -- Nacional"), next to rea_code which is the autonómico one. Both are
    -- printed side by side, so they are separate columns, never one field.
    siex_code     TEXT,
    -- INE province code, two digits ('47' Valladolid). **INE, not catastro** —
    -- FEGA keys its COMUNIDAD_AUTONOMA catalogue on the catastro code while
    -- SIEX wants INE, and the two disagree for 10 of the 17 communities, so
    -- the wrong one is silently wrong rather than an error. Feeds the report's
    -- language choice and the export.
    province_code TEXT
);

-- A parcel of land on the holding: the unit treatments, irrigation,
-- fertilisation and harvest are all recorded against, and the subject of the
-- geometry in geo_feature.
--
-- `farm_id` is deliberately IMMUTABLE — there is no API to move a plot between
-- farms. Re-homing one would silently take its whole treatment history with
-- it, and a record book that changes which holding an application belongs to is
-- a falsified book. The fix for a plot on the wrong farm is to delete it and
-- create it on the right one.
CREATE TABLE plot (
    id         TEXT PRIMARY KEY,
    -- The owning holding. Never updated: see the note above.
    farm_id    TEXT NOT NULL REFERENCES farm(id),
    -- The farmer's own name for the parcel ("La Vega", "Detrás de la casa").
    -- The OFFICIAL identity is the SIGPAC reference in plot_es_extension; this
    -- is what makes the book readable by the person who works the land.
    name       TEXT NOT NULL,
    -- The farmer's own figure for the surface, in hectares. Kept separate from
    -- what SIGPAC says the recinto measures (geo_feature.official_area_ha),
    -- which never overwrites this: a discrepancy between the two is worth
    -- showing, not resolving silently.
    area_ha    REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

-- Spanish regional extension for plot: SIGPAC reference, kept out of the core table.
-- Spanish regional extension for plot: the SIGPAC reference, kept out of the
-- core table.
--
-- SIGPAC is Spain's LPIS (the EU-mandated parcel identification system), and
-- its reference is the OFFICIAL identity of a piece of land — what the PAC
-- declaration, the record book and any inspection all name it by. It is seven
-- numeric parts in a fixed hierarchy, narrowing from the province down to the
-- individual enclosure, and all seven are needed to address one recinto.
--
-- Stored as seven TEXT columns rather than one joined string so each part can
-- be validated and rendered on its own, and because the Nube de SIGPAC
-- endpoints take them as separate path segments. They are numeric in SIGPAC
-- itself; TEXT here preserves any leading zeros the farmer's papers show.
CREATE TABLE plot_es_extension (
    plot_id             TEXT PRIMARY KEY REFERENCES plot(id) ON DELETE CASCADE,
    sigpac_province     TEXT,   -- provincia — INE province code
    sigpac_municipality TEXT,   -- municipio
    sigpac_aggregate    TEXT,   -- agregado; usually 0
    sigpac_zone         TEXT,   -- zona; usually 0
    sigpac_polygon      TEXT,   -- polígono
    sigpac_parcel       TEXT,   -- parcela
    -- recinto: the smallest unit, one homogeneous land use inside the parcela,
    -- and the one the zone checks and the official surface are reported for.
    sigpac_enclosure    TEXT
);

-- The crop present on a plot in a given season ("crop at time of treatment" links here).
CREATE TABLE crop (
    id                     TEXT PRIMARY KEY,
    -- (plot, season) is the crop's identity: the same land carries a different
    -- crop each campaign, and both halves are what a treatment resolves
    -- against when it asks "what was growing here when this was applied".
    plot_id                TEXT NOT NULL REFERENCES plot(id),
    -- Must be a season of the PLOT's farm. No key can say so — the crop reaches
    -- its farm only through `plot_id`, unlike the registers, which carry
    -- `farm_id` and a composite key — so `insert_crop` refuses the mismatch
    -- (`season_not_on_farm`). It is the one place a crop is created, and
    -- neither half ever changes afterwards. A mismatched crop would be listed
    -- in no book: each book reads its own season AND its own farm's plots.
    season_id              TEXT NOT NULL REFERENCES season(id),
    -- Free text, and NOT NULL: the species is what the book prints
    -- (model 2.1 "Cultivo"). Free rather than coded because a farmer must be
    -- able to record a crop the catalogue has no row for; `crop_code` below
    -- carries the coded form when there is a match.
    species_name           TEXT NOT NULL,
    variety                TEXT,   -- model 2.1's "Variedad"; free text, optional
    -- Conventional, organic or integrated production. Feeds the printed book
    -- and, when `gip_system_code` is unset, implies the GIP framework.
    production_system_code TEXT REFERENCES production_system(code),
    -- Surface this crop occupies on the plot (model 2.1 "Superficie cultivada").
    -- Per crop, not per plot: a plot carrying two crops splits between them, and
    -- printing the whole plot area on each row double-counts it. NULL means "not
    -- stated" and prints blank — never assume the crop fills the plot.
    area_ha                REAL,
    -- Rainfed or irrigated and by what system (Anexo III A.2.e). Describes
    -- THIS crop on THIS plot, which is not the same question as how a single
    -- watering was delivered — that is the fertilisation module's
    -- `irrigation_method`, a deliberately separate vocabulary.
    irrigation_code        TEXT REFERENCES irrigation_system(code),
    -- Open air or under cover, and under what. Also what makes the RD 34/2025
    -- greenhouse threshold (>0.1 ha) knowable.
    growing_environment_code TEXT REFERENCES growing_environment(code),
    -- GIP framework for THIS crop (model 2.1's per-row GIP column, Anexo III
    -- A.2.f): a holding can run integrated production on its vineyard and
    -- nothing on its cereal. NULL is not "none" — the report then falls back
    -- to what production_system_code already implies (organic → AE,
    -- integrated → PI), so the column keeps printing for books entered
    -- before this field existed.
    gip_system_code        TEXT REFERENCES gip_system(code),
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

-- The person who applies a treatment (aplicador), and whose licence the book
-- must be able to show. Anexo III Parte I A.1.c and B.d: every treatment
-- identifies its applicator, and the model's 1.2 table prints their carné.
--
-- Separate from `user_profile` on purpose: this is a person named in a legal
-- record, not somebody who uses the app. A farm's applicators include people
-- who never touch a phone, and a profile may belong to someone who applies
-- nothing.
CREATE TABLE operator (
    id                  TEXT PRIMARY KEY,
    full_name           TEXT NOT NULL,
    -- Anexo III A.1.c: the model's 1.2 table prints a NIF beside every name.
    -- Universal concept (the person applying is identified by their tax id in
    -- every member state), so core rather than a regional extension.
    tax_id              TEXT,
    -- The applicator's carné number (ROPO inscription in Spain). Named
    -- generically because a core table carries no regional identifier — the
    -- same column holds whatever the member state issues.
    licence_number      TEXT,
    -- Which carné: basic, qualified, fumigator, pilot. It governs what the
    -- holder may legally apply, so it prints beside the number.
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
    -- What this person is called in the app, and what `record_change.actor`
    -- resolves to when an audit trail is read years later.
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
    -- The framework this particular relationship operates under, which is why
    -- it sits on the junction and not on `farm`: a holding can be advised
    -- under integrated production by one entity and belong to an ATRIA
    -- through another.
    gip_system_code TEXT REFERENCES gip_system(code),
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- One ACTIVE link per (farm, advisor); re-linking a previously removed advisor
-- reuses the row instead of stacking duplicates in table 1.4.
CREATE UNIQUE INDEX idx_farm_advisor_active
    ON farm_advisor(farm_id, advisor_id) WHERE deleted_at IS NULL;

-- Application equipment: sprayers, atomisers, dusters, and the fixed
-- installations that treat a store. Anexo III Parte I A.1.h asks the book to
-- identify the equipment and date its inspection.
CREATE TABLE machinery (
    id                       TEXT PRIMARY KEY,
    farm_id                  TEXT NOT NULL REFERENCES farm(id),
    name                     TEXT NOT NULL,   -- what the farmer calls it
    -- Free text ('sprayer', 'atomiser', …), not a lookup: the ITV regime
    -- classifies equipment for its own purposes and no register in the book
    -- reads this, so a code list would be machinery nobody consumes.
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
    -- ROMA — Registro Oficial de Maquinaria Agrícola. The MOBILE sprayer's
    -- registry, which is the typical case and what the official model prints.
    roma_number    TEXT,
    -- REGANIP — the registry for aircraft and fixed or semi-mobile
    -- installations (greenhouse equipment, post-harvest lines). Complementary
    -- to ROMA rather than an alternative: which one applies depends on the
    -- equipment type, so normally exactly one is filled. No CHECK enforces
    -- that — it would buy nothing and could refuse an odd real case.
    reganip_number TEXT
);

-- Places and vehicles on the holding that a phytosanitary treatment can be
-- applied to: model 3.4's "local tratado" and model 3.5's "vehículo tratado".
--
-- WHY A REGISTRY AND NOT FREE TEXT (2026-08-20). RD 1311/2012 Anexo III Parte I
-- B.b requires identifying "la parcela, o en su caso, local o medio de
-- transporte tratado" — an IDENTIFICATION duty. A description retyped on every
-- record identifies nothing: two treatments of the same warehouse can spell it
-- differently and nothing ties them together, so neither the farmer nor an
-- inspector can ask "what was done in this store this year". A registry row is
-- the identity the decree asks for. (No norm requires a premises REGISTRY: RD
-- 1311/2012 art. 42-43's establishment registry is ROPO's, which covers
-- commercial treatment services and not a farmer's own store. An earlier
-- version of this comment also claimed the table would give the SIEX twin's
-- `Edificaciones[].IdEdificacion` a stable integer to alias — that was WRONG
-- and is corrected in premises_es_extension below: the field is REA's own key,
-- not ours to mint.)
--
-- ONE TABLE FOR BOTH KINDS. `premises` carries 3.5's vehicles too, which the
-- name fits imperfectly and the sources fit exactly: B.b names them in one
-- breath, and the exchange format folds both into one `Edificaciones` block.
-- Two tables would differ in three columns and double the repository, the form
-- and the tests.
--
-- IN CORE, not in module-phytosanitary: this is holding infrastructure like `machinery`
-- ("core = the farm registry — land, calendar, people, machines"), and a store
-- is a plausible second consumer for module-fertilisation, which may never
-- depend on module-phytosanitary. The register that USES it stays in module-phytosanitary.
CREATE TABLE premises_kind (
    code     TEXT PRIMARY KEY,   -- 'building' | 'vehicle'
    i18n_key TEXT NOT NULL
);

CREATE TABLE premises (
    id            TEXT PRIMARY KEY,
    farm_id       TEXT NOT NULL REFERENCES farm(id),
    -- 'building' | 'vehicle'. Core-native words on purpose: the register's own
    -- vocabulary ('storage_premises' / 'transport') is module-phytosanitary's, and core
    -- may not reference a module's lookup — the `sowing_record` precedent. The
    -- module pairs the two and refuses a mismatch.
    kind_code     TEXT NOT NULL REFERENCES premises_kind(code),
    -- What the farmer calls it, and what prints as the model's "tipo": a
    -- smallholder writing "Almacén de la finca" has answered that column, and
    -- a second free-text "type" field beside the name would be asking twice.
    name          TEXT NOT NULL,
    address       TEXT,              -- model 3.4's "dirección"; buildings only
    vehicle_model TEXT,              -- model 3.5's "modelo"; vehicles only
    plate         TEXT,              -- model 3.5's "matrícula"; vehicles only
    -- BUILDINGS ONLY, the way address is: real estate has a class in FEGA's
    -- catalogue, while a lorry has a matrícula and appears nowhere in it. Not
    -- enforced here — `plate` is not either, and describe_premises simply reads
    -- what the kind prints.
    --
    -- FEGA's own class for the building (catalogue EDIFICACIONES_INSTALACIONES),
    -- stored VERBATIM with no foreign key: 109 published rows that the user's
    -- own catalogue refresh can grow, which is the two-tier rule's second tier.
    -- Narrowed by the picker, never by the repository (the TIPO_COBERTURA_SUELO
    -- rule) — refusing an unknown code would make a lawful premises
    -- unrecordable after a refresh.
    --
    -- It is NEVER composed into a treatment's printed subject cell. The label
    -- lives in the catalogue and a refresh may reword it, so folding it into
    -- that composition would silently restate stored records; `name` is what
    -- answers the model's "tipo", and premises_link::describe_premises pins it.
    class_code    TEXT,
    -- Capacity, not the volume treated: B.f's "volumen tratado" is per
    -- treatment and stays on the record, because a partial treatment of a
    -- store is the ordinary case.
    volume_m3     REAL,
    notes         TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);

CREATE INDEX idx_premises_farm ON premises (farm_id);

-- Spanish regional extension for premises, kept out of the core table like
-- machinery's. Both columns are what the SPANISH registries say about this
-- building, they are read off the same REA page, and neither travels: a REA
-- code means nothing outside that registry, and a cadastral reference is not
-- the single string `farm.owner_tax_id` is — France's is 14 characters with
-- another structure and Italy's is three fields (foglio, particella,
-- subalterno), so a core column would be one no second country could fill.
--
-- `rea_installation_code` is what `Edificaciones[].IdEdificacion` wants, and it
-- is NOT ours to mint. The REA structure types the same field
-- (`instalacionesEdificaciones.identificador`) as "Código del edificio/
-- instalación en el REA", and Anexo V's CUE block 1.3 sits under a subloque
-- named "Instalación identificada en el REA" — the building must already be
-- registered there, so a client-assigned number would name a different one.
-- User-entered from the farmer's own REA papers, exactly like
-- `farm_es_extension.rea_code` and `farm.owner_tax_id`.
--
-- `cadastral_reference` is Anexo V CUE block 1.3's field 1, its ONLY
-- identifying field, marked Obligatorio: "Referencia catastral de la
-- edificación/instalación o de la parcela en que se ubica". No decree asks for
-- it; it is captured under the standing line that a field FEGA marks
-- Obligatorio inside a block we send is a real requirement (the
-- `PlanAbonado.Herramienta` precedent).
--
-- Both are NULLABLE and neither is pattern-checked — the roma_number /
-- rea_code / licence_number precedent. Refusing a treatment record for want of
-- a registry number would be the registry blocking the duty it serves; the
-- EXPORT precheck is where the format's requirement belongs. A future Catastro
-- lookup fills the reference through the reviewed-proposal path SIGPAC's crop
-- prefill uses, writing through the same repository function — hence no source
-- tagging: what is stored is what the user confirmed.
CREATE TABLE premises_es_extension (
    premises_id           TEXT PRIMARY KEY REFERENCES premises(id) ON DELETE CASCADE,
    cadastral_reference   TEXT,
    rea_installation_code TEXT
);

-- Geometry attached to a core entity (plot boundary today; farm boundary,
-- irrigation features later). USER DATA: synced, audit-logged, soft-deleted —
-- fetched geometry cannot be re-derived offline, so it must roam.
--
-- Subject linkage is an EXCLUSIVE ARC: one nullable FK column per subject type,
-- with a CHECK that exactly one is set. Deliberately NOT the polymorphic
-- (entity_table, entity_id) pattern of record_change/alert_acknowledgement —
-- those rows must outlive their subjects, while a geometry must die with its
-- subject, and the arc keeps real FK enforcement (orphans impossible). A new
-- subject type later = one nullable ADD COLUMN (cheap even post-release).
--
-- Rows from different sources COEXIST (a SIGPAC-fetched boundary next to a
-- manually drawn one → discrepancy display); display precedence is a UI concern.
-- Replacement soft-deletes the previous active row, so history is kept.
CREATE TABLE geo_feature (
    id               TEXT PRIMARY KEY,
    -- EXCLUSIVE ARC: exactly one of plot_id / farm_id is set, enforced by the
    -- CHECK at the foot of the table. One nullable FK per subject type rather
    -- than a polymorphic (table, id) pair, so the database still enforces
    -- referential integrity — and so a new subject type is one ADD COLUMN.
    -- The polymorphic pattern is reserved for record_change and
    -- alert_acknowledgement, which must OUTLIVE the rows they point at; a
    -- geometry must die with its subject, which is what CASCADE here says.
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
    code     TEXT PRIMARY KEY,   -- 'nitrate_vulnerable', 'phytosanitary_restriction', 'natura_2000'
    i18n_key TEXT NOT NULL
);

-- Provider-checked zone intersections per plot and campaign (added
-- 2026-07-08; design history in docs/sigpac-integration.md). Unlike the alerts
-- worked out from them, flags CANNOT be re-derived offline (they come from a provider query), so
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
    -- 'inside' | 'outside'. Both are ANSWERS: 'outside' is proof the check ran
    -- in that campaign and came back clear, which is what an inspection needs.
    -- A missing row means "never checked", and the two must not be confused.
    status         TEXT NOT NULL CHECK (status IN ('inside', 'outside')),
    coverage_pct   REAL,               -- provider's intersection percentage; NULL when outside
    detail         TEXT,               -- provider detail (e.g. 'Zona periférica'); user-visible verbatim
    source         TEXT NOT NULL,      -- 'sigpac' | future providers
    -- When the provider was asked (ISO 8601 UTC). Distinct from `created_at`,
    -- which is when this row was written: a re-check that confirms the
    -- previous answer still produces a new row with a new checked_at.
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
    -- What the abstraction point is called: a well, a spring, a stream, a
    -- channel. Free text — the model prints the farmer's own words, and no
    -- national code list names the water points of a private holding.
    denomination TEXT NOT NULL,
    -- SQLite has no boolean type: 0 or 1, constrained so nothing else lands.
    -- Which of the two it is decides whether `distance_m` is required or must
    -- be absent, so this is the column that gives the next one its meaning.
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
-- Same philosophy as plot_zone_flag's status='outside' and as the phytosanitary module's
-- register_declaration: an empty register looks exactly like an unfilled one,
-- and only the first is evidence the farmer asked the question. Section 2.2 is
-- binding, so a blank water cell beside a stated "Sin afección" would read as
-- unfinished work rather than a checked fact.
--
-- Its own table rather than a register_declaration row: that one is
-- module-phytosanitary's and farm+season scoped, while this is core, per plot and
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

-- Sowing and planting: how a crop began.
--
-- In core for the same reason as harvest below, and it is harvest's mirror
-- image — the two bracket a crop, so core ends up holding all three of `crop`
-- (what is grown), `sowing_record` (how it began) and `harvest_record` (what
-- left). Crop planning, costs and analytics will want it, and modules never
-- depend on each other.
--
-- It carries **no eco-scheme practice code**, unlike every table in
-- `module-ecoscheme`: core may not reference a module's lookup, and a sowing is
-- a farm event under no decree in particular. What makes one evidence of RD
-- 1048/2022 art. 45.2 is `flooded_on` — a core-native fact meaning the crop is
-- grown under water, which is the only marker this table needs.
--
-- SIEX twin: `SiembraPlantacion`. **Most of that block is deliberately not
-- captured** (recorded in docs/siex-export.md rather than built): its
-- `SiembraDirecta` is already recordable as a `cultural_operation` of kind
-- `no_tillage`, and its seed-provenance members restate what model 3.2's
-- `seed_treatment` already holds — a second, unlinked statement of the same
-- fact is the one failure nothing would catch. Since 2026-08-21 the link is
-- stated instead of restated: `seed_treatment.sowing_record_id` points here,
-- the only direction the dependency rule allows and the one the descriptor
-- itself points (`UsoSemillaTratada.IdAjenaSiembraPlant`). `Cantidad` is
-- captured because the twin requires it, the standing line for a field no
-- printed page shows.

-- Whether a crop was sown or planted. A two-value closed list gets a lookup of
-- its own, the `premises_kind` shape.
--
-- The column exists because the FORM already promised it: this register is
-- titled "Siembra y plantación" and asks the farmer to "anote cómo empezó cada
-- cultivo", so an orchard planting is its documented use, not a stray. No
-- decree asks for a planting date — the only clause naming this kind of act is
-- RD 1048/2022 art. 45.2's "fechas de … siembra …" for cultivos bajo agua,
-- which is rice — so the register is not derived from `SiembraPlantacion`'s
-- required 1/0 member. But a constant "siembra" at export would state something
-- false about every planting the form invites, which is the reverse of the
-- usual capture question: the value was already being collected implicitly, and
-- this makes it answerable.
CREATE TABLE sowing_kind (
    code     TEXT PRIMARY KEY,   -- 'sowing' | 'planting'
    i18n_key TEXT NOT NULL
);

CREATE TABLE sowing_record (
    id              TEXT PRIMARY KEY,
    season_id       TEXT NOT NULL,
    farm_id         TEXT NOT NULL REFERENCES farm(id),

    -- 'sowing' | 'planting'; SIEX `SiembraPlantacion` 1 and 0. NOT NULL with no
    -- default: the form defaults the picker, the schema demands an answer.
    kind_code       TEXT NOT NULL REFERENCES sowing_kind(code),

    -- 'YYYY-MM-DD'. `SiembraPlantacion.FechaInicio`; model 9.3's "Fecha de
    -- siembra en seco" and the date model 9.2's "Siembra" column prints.
    sown_on         TEXT NOT NULL,
    -- `FechaFin`. NULL = one day's work, never "unknown" — the
    -- `cultural_operation` rule, and the twin distinguishes the two.
    sowing_end_date TEXT,

    -- `FechaInundacion`; model 9.3's "Fecha de inundación". Anexo V restricts
    -- it to rice ("Sólo para el cultivo del arroz"), and it is what marks this
    -- sowing as a cultivo bajo agua — so section 9.3 keys on it rather than on
    -- a practice code this table cannot hold. Filled by CORRECTION weeks after
    -- the dry sowing, which is why the register is fully correctable and why
    -- NULL here is "not flooded (yet)", never "unknown".
    flooded_on      TEXT,

    -- `Cantidad`, kilograms of seed. Required by the twin and printed by NO
    -- page of section 9 — captured for that reason alone. Nullable because the
    -- decree asks for dates, not amounts.
    seed_quantity_kg REAL,

    notes           TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    -- The season must be this farm's own (see `season`): a record filed under
    -- another holding's season would print in neither book.
    FOREIGN KEY (season_id, farm_id) REFERENCES season(id, farm_id)
);

-- Where the sowing went, and which crop it started. Mirrors `harvest_plot`
-- field for field, including the absence of a surface column: model 9.3 asks
-- which parcels, not how much of each, exactly as model 5 does.
CREATE TABLE sowing_plot (
    id                 TEXT PRIMARY KEY,
    -- The parent register. CASCADE, and this is the shape EVERY "_plot"
    -- junction in the app shares: a child row describes one plot's share of
    -- its parent and has no meaning without it, so it is hard-deleted with the
    -- parent while the parent itself is only ever soft-deleted.
    sowing_record_id   TEXT NOT NULL REFERENCES sowing_record(id) ON DELETE CASCADE,
    -- The land. NO cascade: a plot is soft-deleted, never removed, so this
    -- reference keeps resolving for as long as the record must be readable.
    plot_id            TEXT NOT NULL REFERENCES plot(id),
    -- Which crop this sowing started, when it is known. Nullable because a
    -- sowing can be recorded before the crop row exists.
    crop_id            TEXT REFERENCES crop(id),
    -- Frozen copies of what the crop was called AT THE TIME, kept beside the
    -- live `crop_id` rather than instead of it. The test a snapshot has to
    -- pass here: if the referenced row changed, would the PAST record become
    -- WRONG? Yes — correcting a crop's species years later must not restate
    -- what was sown then, because the record says what went into the ground
    -- that day. Where the world merely changed, the value is read live
    -- instead. See docs/data-model.md -> "Nothing is ever frozen".
    crop_name_snapshot TEXT,
    variety_snapshot   TEXT,
    -- One row per plot per record: recording the same plot twice on one
    -- sowing is a data-entry slip, not two events.
    UNIQUE (sowing_record_id, plot_id)
);

CREATE INDEX idx_sowing_record_book ON sowing_record(season_id, farm_id);

-- The duplicate rule's window: the farm's sowings within a day of another
-- (docs/sync.md → Duplicate suspects). Farm-wide rather than per book, because
-- two devices that each created a book for one campaign hold exactly the pairs
-- worth finding.
CREATE INDEX idx_sowing_record_farm_day ON sowing_record(farm_id, sown_on);

-- What the purge looks through: the removed sowings, and nothing else — a live
-- row is not in it at all, so finding what is due never grows with what is
-- kept (docs/sync.md → The purge, as settled). Every register of a book has
-- one; `index_contract.rs` holds a new one to it.
CREATE INDEX idx_sowing_record_removed ON sowing_record(deleted_at)
    WHERE deleted_at IS NOT NULL;

-- What still points at a crop. The purge erases a crop only together with
-- everything that names it, and SQLite answers "does anything still name it"
-- through this column — without an index, by reading the whole table for every
-- crop it erases. Partial: a sowing that names no crop has nothing to find.
CREATE INDEX idx_sowing_plot_crop ON sowing_plot(crop_id) WHERE crop_id IS NOT NULL;

-- Commercialised harvest: model section 5.
--
-- In core, not in the phytosanitary module: what leaves the holding and to whom is
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
    season_id             TEXT NOT NULL,
    farm_id               TEXT NOT NULL REFERENCES farm(id),
    -- One date. The twin requires a FechaInicio and a FechaFin, which a
    -- serializer satisfies by sending this value as both ends; the model prints
    -- a single "Fecha" column and section 5 is not Anexo III Parte I content.
    harvested_on          TEXT NOT NULL,       -- 'YYYY-MM-DD'
    -- What was harvested, in the farmer's words, and what the book prints.
    -- Free text and NOT NULL for the `crop.species_name` reason: the coded
    -- form beside it may have no matching row.
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
    -- module-phytosanitary lookup that core could not reference, so the pairing lived in
    -- the repository alone. The narrower {kg, t} rule stays there — the key
    -- says "a unit", the repository says "a unit that can weigh a harvest".
    -- Both columns are nullable together, because the printed form leaves the
    -- cell to be filled by hand.
    quantity_value        REAL,
    quantity_unit_code    TEXT REFERENCES unit(code),   -- 'kg' | 't'
    -- Both voluntary in the model.
    delivery_note_ref     TEXT,                -- nº de albarán o factura
    -- The batch this consignment was sold under. Voluntary here, and the
    -- thread a food-safety traceback pulls on: it is what links a complaint
    -- about produce in a shop back to the plots and treatments in this book.
    lot_number            TEXT,
    buyer_name            TEXT NOT NULL,       -- nombre o razón social
    buyer_tax_id          TEXT,                -- NIF/CIF of the buyer
    buyer_address         TEXT,
    -- The model's "Nº de RGSEAA" (voluntary). Named generically for the same
    -- reason as advisor.registration_number: core tables carry no regional
    -- identifiers, so the Spanish label lives in the report labels and the UI
    -- dictionaries.
    buyer_registry_number TEXT,
    notes                 TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    deleted_at            TEXT,
    -- The season must be this farm's own (see `season`): a record filed under
    -- another holding's season would print in neither book.
    FOREIGN KEY (season_id, farm_id) REFERENCES season(id, farm_id)
);

-- Where the harvest came from, as the model's "Nº de orden parcela/s de
-- origen". No surface column: the model asks which parcels, not how much of
-- them. Reconciled from the submitted form state on update.
CREATE TABLE harvest_plot (
    -- Field for field the same shape as `sowing_plot` above, including the
    -- frozen crop name and variety and the reason they are frozen; see there.
    id                 TEXT PRIMARY KEY,
    harvest_record_id  TEXT NOT NULL REFERENCES harvest_record(id) ON DELETE CASCADE,
    plot_id            TEXT NOT NULL REFERENCES plot(id),
    crop_id            TEXT REFERENCES crop(id),
    crop_name_snapshot TEXT,                   -- frozen crop at harvest time
    variety_snapshot   TEXT,
    UNIQUE (harvest_record_id, plot_id)
);

CREATE INDEX idx_harvest_record_book ON harvest_record(season_id, farm_id);

-- The duplicate rule's window: the farm's harvests on one day (docs/sync.md →
-- Duplicate suspects).
CREATE INDEX idx_harvest_record_farm_day ON harvest_record(farm_id, harvested_on);

-- The removed harvests, as `idx_sowing_record_removed`, and what names a crop,
-- as `idx_sowing_plot_crop` — each for the purge.
CREATE INDEX idx_harvest_record_removed ON harvest_record(deleted_at)
    WHERE deleted_at IS NOT NULL;
CREATE INDEX idx_harvest_plot_crop ON harvest_plot(crop_id) WHERE crop_id IS NOT NULL;

-- Append-only audit log AND the sync delta source (docs/sync.md → Part 2).
-- Deliberately has NO foreign keys: it references many tables polymorphically
-- and must outlive the rows it records.
CREATE TABLE record_change (
    id            TEXT PRIMARY KEY,
    -- WHICH ROW CHANGED, as a polymorphic (table name, id) pair rather than a
    -- foreign key. Deliberate, and the opposite of geo_feature's exclusive
    -- arc: this log must OUTLIVE the rows it describes and must never cascade,
    -- so a real FK would be exactly wrong. `entity_table` is the table's own
    -- name as written in this file; `entity_id` is that row's UUID.
    entity_table  TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    -- The campaign the changed row belonged to, denormalised so the log can be
    -- read per season without joining back to a row that may since have been
    -- soft-deleted. NULL for entities that are not season-scoped (a farm, an
    -- operator, a product).
    season_id     TEXT,
    operation     TEXT NOT NULL,                  -- 'insert' | 'update' | 'delete'
    -- When the change was made (ISO 8601 UTC). There is deliberately NO index
    -- on this column — see the note under the table.
    changed_at    TEXT NOT NULL,
    actor         TEXT,                           -- user_profile.id of the author (the device's
                                                  -- active profile at write time); NULL = recorded
                                                  -- with no active profile
    payload       TEXT NOT NULL,                  -- JSON {"before": ..., "after": ...}
    -- THE REGISTER this change belongs to, which is the unit of merge: a
    -- register and its children merge as one statement, so a treated plot's
    -- row names its treatment record here. Usually a row's own table and id.
    -- A register keyed on a SLOT rather than a row (one plot's zone flags for
    -- one campaign and source, one season's declaration of one register) names
    -- its slot instead, as a canonical JSON array of the slot's values: two
    -- devices filling one slot are then two edits of one register, which the
    -- merge can see, instead of two unrelated rows colliding on a UNIQUE index
    -- (docs/sync.md → The unit of merge is the whole register).
    root_table    TEXT NOT NULL,
    root_id       TEXT NOT NULL,
    -- WHERE the change was made, and in which of that device's change sets.
    -- The device is `settings.json`'s device id, not the actor: one says which
    -- replica wrote the row, the other which person. Every row one transaction
    -- logs shares both values, so the pair IS the change-set key.
    origin_device TEXT NOT NULL,
    origin_seq    INTEGER NOT NULL CHECK (typeof(origin_seq) = 'integer' AND origin_seq > 0),
    -- The register's version vector AFTER this write, JSON {device: seq} with
    -- its keys sorted. It is what decides whether two changes conflict.
    version_vector TEXT NOT NULL,
    -- Hybrid logical clock: milliseconds since the Unix epoch in the high 48
    -- bits, a counter in the low 16. It orders changes ALREADY KNOWN to
    -- conflict and nothing else. Never shown as a time: it may legitimately run
    -- ahead of the wall clock, and `changed_at` is the instant an inspector
    -- reads. The type check is not ceremony — in a table without STRICT, a
    -- stray TEXT value would satisfy `>= 0` and sort after every integer.
    hlc           INTEGER NOT NULL CHECK (typeof(hlc) = 'integer' AND hlc >= 0),
    -- One row per entity per change set. Its leading pair also serves the two
    -- per-device questions: "this device's next change set" (MAX over one
    -- device) and "what has device D written since seq N" (a range scan).
    UNIQUE (origin_device, origin_seq, entity_table, entity_id)
);

-- `record_change` is the fastest-growing table in the schema, so every index
-- on it answers a question something actually asks. There is still no index on
-- `changed_at`: "changes since X" turned out to be a SEQUENCE question, per
-- device, which the UNIQUE constraint above already serves.
--
-- A row's own history — the correction trail an inspection reads.
CREATE INDEX idx_record_change_entity ON record_change(entity_table, entity_id);
-- A register's history in change-set order: what computes its current version
-- vector before every write, and what a merge reads back to the common ancestor.
CREATE INDEX idx_record_change_root ON record_change(root_table, root_id, origin_seq);
-- The clock. Each change set's stamp is one past the highest this log holds,
-- received changes included, which is how the hybrid clock absorbs what other
-- devices have seen; this index makes that a single seek instead of a scan.
CREATE INDEX idx_record_change_hlc ON record_change(hlc);

-- The holding this device syncs within (docs/sync.md → Device identity), which
-- is what makes a bundle from a neighbour's holding refusable.
--
-- DEVICE-LOCAL: never logged, never merged, not a register. It is in the
-- DATABASE rather than beside `device_id` in settings.json because a restored
-- backup is the same holding — where a restored device is a different REPLICA,
-- which is exactly why the device id is re-minted on import and this is not.
--
-- Minted LAZILY, on the first export, and never at first launch: two devices
-- set up separately would each hold a group before anything had decided what
-- joining means, and would then refuse each other for ever.
--
-- One row or none, and the CHECK is what says so rather than a rule in Rust.
-- A membership list alone could not enforce it — a contractor's phone could
-- legitimately appear in two farms' `sync_peer` lists, and would then carry one
-- holding's records into the other — so the group is an IDENTITY, and a device
-- has at most one. Per-farm scoping, which is what a contractor would actually
-- need, is multi-tenant separation and is recorded rather than scheduled.
-- Relaxing the CHECK later means rebuilding the table, since SQLite cannot drop
-- one in place: free while pre-release, one ordinary migration after.
CREATE TABLE sync_group (
    row_id     INTEGER PRIMARY KEY CHECK (row_id = 1),
    -- UUIDv7, minted on whichever device exported first and adopted by every
    -- device that joins it.
    group_id   TEXT NOT NULL,
    joined_at  TEXT NOT NULL
);

-- Every device that has written to this book, one row each (docs/sync.md →
-- Device identity). Synced and logged like any register, so any device can
-- name a conflict's other side "María's phone" instead of a UUID.
--
-- The id IS the device id — a UUIDv7 minted in Rust on the device's first
-- launch — rather than one minted for the row. Deliberate, in the manner of
-- `farm_es_extension` being keyed by its farm: two devices naming one phone
-- must write one row, not two rows claiming the same device.
CREATE TABLE sync_peer (
    id          TEXT PRIMARY KEY,
    -- What people call the device. NULL until somebody names it.
    label       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    -- A retired device (a lost phone). Soft, like every row the log refers to:
    -- the changes it made stay attributed to it.
    deleted_at  TEXT
);

-- One row per register version waiting for a person to choose it: two devices
-- wrote without seeing each other, so the register has more than one head
-- (docs/sync.md → Conflicts as the person sees them).
--
-- DERIVED, and device-local — never logged, never synced. The log
-- already answers this: every device holds the same rows and computes the same
-- heads from them, so there is nothing to send, and a synced row would be a
-- second source of truth to disagree with the log the moment a resolution
-- arrives. What this table buys is only cost: finding conflicts without it
-- means asking the question of every register ever written, which grows
-- forever. `merge::settle` writes and clears these as it goes; deleting the
-- table's contents loses nothing that cannot be recomputed.
--
-- Polymorphic and FK-free, like `record_change`: a conflict names
-- registers in a dozen tables, and the row must survive its register being
-- deleted on one side — which is itself one of the states worth reviewing.
CREATE TABLE sync_conflict (
    root_table   TEXT NOT NULL,
    root_id      TEXT NOT NULL,
    -- The version the book is showing, chosen by clock then device id.
    live_device  TEXT NOT NULL,
    live_seq     INTEGER NOT NULL,
    -- The version waiting. Three heads make two rows, not one.
    other_device TEXT NOT NULL,
    other_seq    INTEGER NOT NULL,
    -- The campaign to list it under, off the log row. NULL for a register that
    -- belongs to the holding rather than to one book (a plot, an operator).
    season_id    TEXT,
    detected_at  TEXT NOT NULL,
    -- One row per losing branch, which is what makes `settle` idempotent:
    -- re-settling a register rewrites exactly the rows it should.
    PRIMARY KEY (root_table, root_id, other_device, other_seq)
);

-- The review queue is read one book at a time, like every other list in the app.
CREATE INDEX idx_sync_conflict_season ON sync_conflict(season_id);

-- Season-first, and it REPLACED a (plot_id, season_id) index on 2026-08-24
-- rather than joining it. Season-first is how the app reads — one campaign's
-- crops, the season-delete guard — and the old column order could not serve
-- either, so both scanned the table whole. Nothing is lost by the swap: no
-- query filters crops by plot alone, and the one that reads a single plot's
-- crops binds the season too, so equality on both columns is served either way.
CREATE INDEX idx_crop_season_plot     ON crop(season_id, plot_id);

-- The removed crops, for the purge, as `idx_sowing_record_removed`.
CREATE INDEX idx_crop_removed ON crop(deleted_at) WHERE deleted_at IS NOT NULL;

-- Invisible at a smallholder's dozen plots and the first thing a
-- cooperative-sized holding would feel: every per-farm listing resolves its
-- plots through this column, and four subqueries (zone flags, water points,
-- water declarations, geometry) go through it on the way to something else.
CREATE INDEX idx_plot_farm            ON plot(farm_id);

-- Machinery is read per farm by the registry and the treatment form, and the
-- 2026-08-17 audit missed this one while catching `plot`'s: both listings were
-- scanning the table. Bounded by a holding's machines rather than by its
-- history, so it is small today and free to fix.
CREATE INDEX idx_machinery_farm       ON machinery(farm_id);

-- Integer aliases regulatory exports assign to activity records (2026-07-15;
-- moved module-phytosanitary → core 2026-08-20, design in docs/siex-export.md → gap 1).
-- In CORE because it is a generic mechanism, not a treatment one: it already
-- aliased `crop` (a core row) on the day it shipped, and the SIEX export mints
-- aliases for registers owned by core, module-phytosanitary, module-fertilisation and
-- module-ecoscheme. A module's table storing keys on behalf of two other
-- modules' rows is the coupling the layering exists to prevent — "shared DATA
-- → core", the same call that moved `unit` on 2026-08-07. SIEX's IdAjena* edit/delete keys are
-- integers ≤ 10 digits, so UUIDs cannot travel; an alias is minted at FIRST
-- export (MAX+1 per target, race-free behind the connection mutex) and then
-- NEVER updated or deleted — stability across exports is the point, and a
-- row's existence doubles as the "previously exported" marker that drives the
-- export's deletion flag for soft-deleted records. split_key discriminates
-- when one record maps to several export entries (a multi-crop treatment
-- splits into one TratamFito per crop); its value is serializer-defined,
-- opaque here. Polymorphic like record_change, so no FK. Synced user data
-- (aliases must roam and survive backups — they cannot be re-derived):
-- insert-logged in record_change. Two devices exporting independently before
-- syncing could mint colliding aliases; the design that prevents it is one
-- submitting device per farm (docs/sync.md → `export_alias` collisions),
-- built with the exporter, which nothing can reach today.
CREATE TABLE export_alias (
    id           TEXT PRIMARY KEY,
    target       TEXT NOT NULL,              -- 'siex' | future export regimes
    entity_table TEXT NOT NULL,              -- any synced register, in any crate
    -- The row this alias stands for. Polymorphic and FK-free for the
    -- record_change reason: the alias must survive the record's soft delete,
    -- because a WITHDRAWAL still has to name what is being withdrawn.
    entity_id    TEXT NOT NULL,
    split_key    TEXT NOT NULL DEFAULT '',   -- '' when the record maps 1:1
    -- The integer the receiving system knows this record by, minted at FIRST
    -- export and NEVER changed afterwards: SIEX keys its edits and deletes on
    -- it, so re-minting would orphan everything already sent and make a
    -- correction read as a second, unrelated application.
    alias        INTEGER NOT NULL,
    created_at   TEXT NOT NULL,
    UNIQUE (target, entity_table, entity_id, split_key),
    UNIQUE (target, alias)
);

-- "Does any target hold an alias for this record?" — what the purge asks before
-- erasing one: a record the authority may hold has to be withdrawn there
-- first, and only the exporter's submission log can say it was (docs/sync.md →
-- When a register goes). The UNIQUE leads with the target, which the purge
-- does not know.
CREATE INDEX idx_export_alias_entity ON export_alias(entity_table, entity_id);

-- One person's act on an alert: seen ('acknowledged') or hidden ('dismissed').
-- USER DATA, synced and logged like any register, because it is the one thing
-- about an alert no device can re-derive (docs/sync.md → Alert acknowledgements
-- roam). The alerts themselves are stored nowhere: each crate that raises them
-- works out the ones holding now whenever the list is read, and core assembles
-- it (docs/data-model.md → "Alerts: the settled design"). In CORE since
-- 2026-09-24 because any module may raise an alert, and a module may never
-- depend on another.
--
-- INSERT-ONLY: a second act is a second row, never an update of the first. Two
-- devices acting on one alert therefore write two registers rather than two
-- versions of one, so they never conflict and nobody is asked — the strongest
-- act wins at read time (`alerts::alert_status`: dismissed over acknowledged).
-- No UNIQUE for the same reason; a duplicate act changes nothing it could
-- disagree about, and a device skips writing one it can see is redundant.
--
-- WHAT the act is about is polymorphic and FK-free, like record_change: an act
-- must outlive the condition it was made on, which lapses. It also names the
-- deadline it was about, so a renewed licence's NEXT expiry is a new alert and
-- not one somebody already dismissed. Acts about a deadline that has passed are
-- never read again and are not pruned: a pruned row costs a logged delete,
-- which measured larger than the row it frees.
--
-- `alert_type_code` is FK-free too, and there is no kinds table: a kind is a
-- constant declared by the crate that raises it, so adding one needs no
-- migration — and so never moves `user_version`, which delta exchange requires
-- to be equal. An act about a kind this build does not know (synced from a
-- newer one, or about a retired kind) is kept and matches nothing.
CREATE TABLE alert_acknowledgement (
    id              TEXT PRIMARY KEY,
    alert_type_code TEXT NOT NULL,
    subject_table   TEXT NOT NULL,
    subject_id      TEXT NOT NULL,
    -- The due date the person acted on; NULL for a standing condition (a zone
    -- flag), which has no date to end on.
    due_date        TEXT,
    status          TEXT NOT NULL CHECK (status IN ('acknowledged', 'dismissed')),
    created_at      TEXT NOT NULL
);

-- The acts on the subjects of the alerts holding NOW, reached one subject at a
-- time. Acts accumulate for as long as the farmer uses the app and almost all
-- of them are about deadlines long gone, so the list must seek into this table
-- from today's subjects and never scan it. Subject first: the list asks by
-- subject id alone, and recording an act asks by all three.
CREATE INDEX idx_alert_acknowledgement_subject
    ON alert_acknowledgement(subject_id, alert_type_code, subject_table);

-- What a person said about two records that looked like one operation recorded
-- twice: that both are real ('distinct'), or that they are one and which was
-- kept ('duplicate'). USER DATA, synced and logged, because no device can
-- re-derive a judgement. The suspicions themselves are stored nowhere: each
-- register's rule is run whenever the list is read (docs/sync.md → Duplicate
-- suspects).
--
-- INSERT-ONLY, alert_acknowledgement's shape and for its reason: two devices
-- judging one pair write two registers rather than two versions of one, so
-- they never conflict. A 'duplicate' act is written in the same change set as
-- the soft delete of the record it removed, so the audit trail reads the
-- removal and its reason side by side — no register carries a reason column.
--
-- Polymorphic and FK-free like record_change: the pair may be in any of a
-- dozen tables, and an act must outlive a record's deletion, which is exactly
-- what a 'duplicate' act is about. `subject_table` is copied from the rule
-- that raised the suspicion, a constant in the owning crate, never from a
-- screen. The pair is stored smaller id first so every device names it alike.
CREATE TABLE duplicate_verdict (
    id            TEXT PRIMARY KEY,
    subject_table TEXT NOT NULL,
    first_id      TEXT NOT NULL,
    second_id     TEXT NOT NULL,
    verdict       TEXT NOT NULL CHECK (verdict IN ('distinct', 'duplicate')),
    -- For 'duplicate', the record kept, which is one of the pair; the other is
    -- the one this act removed. NULL for 'distinct', which keeps both.
    kept_id       TEXT,
    created_at    TEXT NOT NULL,
    CHECK (first_id < second_id),
    CHECK (
        (verdict = 'distinct'  AND kept_id IS NULL)
     OR (verdict = 'duplicate' AND kept_id IN (first_id, second_id))
    )
);

-- "Has this pair been judged?" — asked once per suspected pair whenever the
-- list is read, so a seek on the pair and never a scan of the acts.
CREATE INDEX idx_duplicate_verdict_pair ON duplicate_verdict(first_id, second_id);

-- "Was this removed record kept by somebody else?" — how two opposite removals
-- are found: each device kept the record the other removed.
CREATE INDEX idx_duplicate_verdict_kept ON duplicate_verdict(kept_id);

-- One register erased for good by the purge, and the version that removed it
-- (docs/sync.md → The purge, as settled). USER DATA, synced and logged: it is
-- how every other device learns what to erase, and a device joining later
-- receives it with the log.
--
-- INSERT-ONLY, alert_acknowledgement's shape and for its reason: two devices
-- erasing one register write two rows — two registers — rather than two
-- versions of one, so they never conflict.
--
-- **The row stays, and it is a marker.** It holds no content — a table, an id,
-- a version — and every write stamps its register on top of every row naming
-- it (`audit::WriteTx::register`). That is what lets a book go at all: its id
-- comes from its farm and dates, so opening the same campaign again writes the
-- same id, which must be newer than the deletion everywhere and not a rival to
-- it. An import reads it too: an arriving change the removal had seen is
-- dropped, one written on top of it applies, and one that never saw it is
-- discarded and reported.
--
-- Polymorphic and FK-free like record_change: what it names is gone.
CREATE TABLE purged_register (
    id          TEXT PRIMARY KEY,
    root_table  TEXT NOT NULL,
    root_id     TEXT NOT NULL,
    -- The register's version vector at its removal, JSON {device: seq} — the
    -- version every device compares what it holds of the register against.
    removal     TEXT NOT NULL,
    purged_at   TEXT NOT NULL
);

-- "Is this register erased, and by which removal?" — asked by every stamp and
-- by an import, once per register, so a seek and never a scan.
CREATE INDEX idx_purged_register_root ON purged_register(root_table, root_id);

-- The highest change-set number this database has held from each device
-- (docs/sync.md → What a database has held, and what a file starts from).
--
-- DEVICE-LOCAL: never logged, never synced. It is what a manifest says this
-- device holds, and where its own next number starts — the log's own highest
-- number would do, except that the purge takes change sets out of the log, and
-- a number must never be handed to a second change set. So it is raised when a
-- file applies and when the purge runs, and never lowered. In the database
-- file rather than settings.json because it describes what the file holds: a
-- backup restored elsewhere holds exactly what it held.
CREATE TABLE sync_held (
    device   TEXT PRIMARY KEY,
    through  INTEGER NOT NULL CHECK (typeof(through) = 'integer' AND through >= 0)
);

-- What this device knows each device of the group holds: the highest `seen`
-- heard from it, in a file it sent or passed on in another device's file
-- (docs/sync.md → What each device knows of the others).
--
-- DEVICE-LOCAL, and raised only after a file has applied, when holding
-- everything its sender held makes the knowledge true here. It only ever says
-- less than is true, so the purge it decides can come later than it could,
-- never early. Carried in every manifest, so the phones learn of each other
-- through the laptop.
CREATE TABLE sync_known (
    device  TEXT PRIMARY KEY,
    -- JSON {device: seq}, keys sorted, as record_change.version_vector.
    seen    TEXT NOT NULL
);
