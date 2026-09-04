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
    code         TEXT NOT NULL,          -- provider code; NOT unique per catalogue — some catalogues repeat a code per qualifying attr (e.g. one row per ámbito)
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

-- The campaign a record belongs to: the universal EU/PAC season, and the axis
-- almost every register in the app is scoped by. Nearly every user table below
-- carries a `season_id`, and every record-book view reads through it — which is
-- why deleting a season that owns records is refused rather than cascaded.
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
    campaign_year INTEGER NOT NULL,           -- Spanish PAC campaign year, e.g. 2026
    -- What the farmer calls it, free text: '2026' and '2025/2026' are both
    -- right, because a campaign spanning the new year is normal and only the
    -- holding knows which convention its paperwork uses.
    label         TEXT NOT NULL,
    -- Optional campaign bounds. Nothing is validated against them and no
    -- register refuses a date outside: they describe the campaign, they do not
    -- police it, and a real farm books an operation early or late.
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
-- IN CORE, not in module-cue: this is holding infrastructure like `machinery`
-- ("core = the farm registry — land, calendar, people, machines"), and a store
-- is a plausible second consumer for module-fertilisation, which may never
-- depend on module-cue. The register that USES it stays in module-cue.
CREATE TABLE premises_kind (
    code     TEXT PRIMARY KEY,   -- 'building' | 'vehicle'
    i18n_key TEXT NOT NULL
);

CREATE TABLE premises (
    id            TEXT PRIMARY KEY,
    farm_id       TEXT NOT NULL REFERENCES farm(id),
    -- 'building' | 'vehicle'. Core-native words on purpose: the register's own
    -- vocabulary ('storage_premises' / 'transport') is module-cue's, and core
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
    -- EXCLUSIVE ARC: exactly one of plot_id / farm_id is set, enforced by the
    -- CHECK at the foot of the table. One nullable FK per subject type rather
    -- than a polymorphic (table, id) pair, so the database still enforces
    -- referential integrity — and so a new subject type is one ADD COLUMN.
    -- The polymorphic pattern is reserved for record_change and alert, which
    -- must OUTLIVE the rows they point at; a geometry must die with its
    -- subject, which is what CASCADE here says.
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
    season_id       TEXT NOT NULL REFERENCES season(id),
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
    deleted_at      TEXT
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
    -- module-cue lookup that core could not reference, so the pairing lived in
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
    deleted_at            TEXT
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

-- Append-only audit log AND future sync delta source. Deliberately has NO foreign keys:
-- it references many tables polymorphically and must outlive the rows it records.
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
    payload       TEXT NOT NULL                   -- JSON {"before": ..., "after": ...}
);

-- `record_change` is the fastest-growing table in the schema and is indexed only
-- by the row it describes, which is deliberate: nothing in the app reads it by
-- time, so nothing is slow. The first "changes since X" belongs to the Stage-2
-- sync design, and so does the index that query will want — building one now
-- would be guessing at a query nobody has written.
CREATE INDEX idx_record_change_entity ON record_change(entity_table, entity_id);

-- Season-first, and it REPLACED a (plot_id, season_id) index on 2026-08-24
-- rather than joining it. Season-first is how the app reads — one campaign's
-- crops, the season-delete guard — and the old column order could not serve
-- either, so both scanned the table whole. Nothing is lost by the swap: no
-- query filters crops by plot alone, and the one that reads a single plot's
-- crops binds the season too, so equality on both columns is served either way.
CREATE INDEX idx_crop_season_plot     ON crop(season_id, plot_id);

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
-- moved module-cue → core 2026-08-20, design in docs/siex-export.md → gap 1).
-- In CORE because it is a generic mechanism, not a treatment one: it already
-- aliased `crop` (a core row) on the day it shipped, and the SIEX export mints
-- aliases for registers owned by core, module-cue, module-fertilisation and
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
-- insert-logged in record_change. Known limit, recorded in the design doc:
-- two devices exporting independently before syncing could mint colliding
-- aliases — a sync-stage-2 design item, acceptable while one device exports.
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
