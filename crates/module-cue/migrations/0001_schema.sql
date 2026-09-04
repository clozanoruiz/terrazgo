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

-- `unit` moved to the core's 0001 on 2026-08-07 (a free pre-release squash
-- edit), because a second module needs it: module-fertilisation records doses
-- and irrigation volumes, and a module may never depend on another module.
-- Core steps run EARLIER in the composed sequence, so the references below
-- remain valid. A measurement vocabulary is universal, not a treatment
-- concept — the same argument that moved the farm registry there.

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
-- The chemical that actually does the work, as distinct from the branded
-- product that carries it. Anexo III Parte I B names both, and only the
-- substance is comparable across brands and across countries.
--
-- USER DATA, not a lookup, and that is a decision rather than an oversight: it
-- is user-insertable, so integer rowids would collide the moment two devices
-- both added one. A read-only MAPA lookup was rejected because offline-first
-- forbids blocking a treatment record on a substance the app has never heard
-- of — a farmer in the field records what is on the label.
CREATE TABLE active_substance (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,      -- as the product label spells it
    cas_number TEXT               -- CAS registry number, the natural cross-device key
);

-- A phytosanitary product as sold: the brand on the can. What makes it legal
-- to use is not here but in `product_authorisation` — the same formulation is
-- authorised in one member state and not another, so authorisation is a
-- per-country fact about the product, never a column on it.
CREATE TABLE product (
    id                    TEXT PRIMARY KEY,
    commercial_name       TEXT NOT NULL,     -- the brand name printed on the label
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
-- What a product contains, and how much of it. A junction because a product
-- may carry several substances and a substance appears in many products.
--
-- It has its OWN UUID rather than a composite (product, substance) key, which
-- looks redundant and is not: record_change addresses every row as
-- (entity_table, entity_id), so a row with no single id could not be audited
-- or synced. The natural key survives as the UNIQUE below.
CREATE TABLE product_active_substance (
    id                      TEXT PRIMARY KEY,
    product_id              TEXT NOT NULL REFERENCES product(id) ON DELETE CASCADE,
    active_substance_id     TEXT NOT NULL REFERENCES active_substance(id),
    -- Strength as it appears on the label: value plus unit, never free text.
    -- The unit is a 'concentration' one (g/l, %), which is a different
    -- question from the dose applied per hectare — see `unit.dimension`.
    concentration_value     REAL,
    concentration_unit_code TEXT REFERENCES unit(code),
    -- One row per substance per product; the natural key the surrogate id
    -- above replaced as the primary key.
    UNIQUE (product_id, active_substance_id)
);

-- A product carries a different authorisation number per country (MAPA nº for ES).
-- kind_code (2026-07-15) classifies the authorisation's nature; the default
-- covers the typical case, so existing forms stay valid. For 'exceptional'
-- authorisations the export must name the substance by its catalogue code
-- (SIEX AUTORIZACION_EXCP → the TratamFito MateriaActiva field, mandatory only
-- for that kind) — stored verbatim, no FK, per the catalogue-code rule.
-- Permission to use a product in ONE country: the registration number the
-- book must print beside every application, and the gate the insert enforces.
-- A treatment naming a product with no authorisation in the farm's country is
-- refused, because recording it would be recording an illegal application.
CREATE TABLE product_authorisation (
    id                         TEXT PRIMARY KEY,
    product_id                 TEXT NOT NULL REFERENCES product(id) ON DELETE CASCADE,
    -- Which country's regime this authorisation belongs to. Treatments derive
    -- their country from the FARM, never from the caller, and match here.
    country_code               TEXT NOT NULL REFERENCES country(code),
    -- The official registration number (in Spain, the MAPA number on the
    -- label). This is the value the printed book cites, which is why a record
    -- freezes a copy of it at write time.
    authorisation_number       TEXT NOT NULL,
    kind_code                  TEXT NOT NULL DEFAULT 'registered' REFERENCES authorisation_kind(code),
    exceptional_substance_code TEXT,   -- catalogue code, only meaningful when kind_code = 'exceptional'
    -- The registry's own words for the authorisation's state, kept verbatim.
    status                     TEXT,
    -- The authorisation's window ('YYYY-MM-DD'). Recorded for reference and
    -- deliberately NOT enforced against an application date: a product
    -- withdrawn today was lawful when it was sprayed, and a book that refused
    -- to record last year's treatment would be falsifying history.
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
    -- Last day of the actuation when it spanned several. RD 1311/2012 Anexo III
    -- Parte I B lets the date be an INTERVAL, and the SIEX exchange format wants
    -- FechaInicio + FechaFin; NULL means the treatment was a single day, which is
    -- the ordinary case. The plazo de seguridad runs from the LAST application,
    -- so phi_end_date is derived from this column when it is set.
    application_end_date          TEXT,                          -- 'YYYY-MM-DD'
    -- Start hour of the application. Reglamento (UE) 2023/564's annex asks for
    -- "date and where relevant start time (hour)" in the treatment-of-surfaces
    -- row only, footnote 4 defining relevance as a product whose use is
    -- restricted to particular times of day, or a use where the hour matters.
    -- No Spanish form has a column for it and RD 1311/2012 Anexo III Parte I B
    -- does not ask for it, so the duty arrives from the EU regulation alone.
    -- Stored as LOCAL WALL-CLOCK 'HH:MM', deliberately not UTC: this is a time
    -- of day, not an instant. What makes an hour relevant is the hour on the
    -- ground (label restrictions, wind, bees, heat), no timezone is stored
    -- anywhere, and a UTC round-trip would print a different hour than the one
    -- the farmer recorded. When the date is an interval this is the start hour
    -- of the first application. NULL means not stated and prints blank.
    application_time              TEXT,                          -- 'HH:MM', local
    -- Model 9.3's fourth column, "Fecha de seca para tratamiento herbicida o
    -- fitosanitario", and the SIEX twin's `TratamFito.FechaSeca`. RD 1048/2022
    -- art. 45.2 counts the *secas* among the five dates a flooded crop must
    -- annotate, and it belongs on the TREATMENT rather than on the sowing
    -- because both the model's wording and the twin's placement tie it there:
    -- the field is dried in order to spray it, so the drying is an attribute of
    -- that spraying and not of the crop's water calendar. Anexo V agrees —
    -- "fecha en la que se realiza el secado para la realización del
    -- tratamiento".
    --
    -- Nullable, unvalidated beyond its shape, and NOT re-snapshotted by
    -- anything: it is a captured fact, never a frozen copy of another row.
    drying_date                   TEXT,                          -- 'YYYY-MM-DD'
    -- The chemical half of an actuation, nullable as a BLOCK (see the CHECK at
    -- the foot of the table). RD 1311/2012 art. 10.1 requires priority for
    -- non-chemical methods, and the SIEX twin follows it: TratamFito requires
    -- an applicator, a problem, justifications and an efficacy, but NOT
    -- ProductosFito — so hanging pheromone diffusers against a pest is a
    -- treatment in its own right, with no product, no dose and no plazo de
    -- seguridad to run. Recording it as a product application with invented
    -- zeros would be a false statement in a legal document.
    product_id                    TEXT REFERENCES product(id),
    country_code                  TEXT NOT NULL REFERENCES country(code),  -- which authorisation context applies
    dose_value                    REAL,
    dose_unit_code                TEXT REFERENCES unit(code),
    -- Total product actually used across the whole actuation, in kg or l
    -- (Anexo III Parte I B.i). Kept as its own pair rather than derived: a dose
    -- expressed as a concentration (g/l, ml/l, %) carries no information about
    -- how much spray was mixed, so the total is NOT recoverable from dose ×
    -- surface. NULL means the farmer did not state it and prints blank.
    total_quantity_value          REAL,
    total_quantity_unit_code      TEXT REFERENCES unit(code),    -- dimension 'quantity'
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
    -- Anexo III Parte I B.d asks for "identificación del aplicador y, EN SU
    -- CASO, del asesor" — the advisor is one more identification on the same
    -- record, exactly like the applicator, and the SIEX twin agrees by hanging
    -- AsesorValidacion off TratamFito rather than off a register of its own.
    -- Nullable because most treatments are not advised; the snapshots freeze
    -- the printed values at write time (the legal-value-capture rule), so
    -- correcting an advisor's registration number later never rewrites what a
    -- past record said.
    advisor_id                    TEXT REFERENCES advisor(id),
    advisor_name_snapshot         TEXT,
    advisor_registration_snapshot TEXT,          -- the ROPO number in Spain
    -- The NON-CHEMICAL half (SIEX OtrasActuacionesFito), which the official
    -- model prints as 3.1 bis's "Alternativas no químicas de intervención".
    -- measure_code is a TIPO_MEDIDA_FITOSANITARIA code stored verbatim with no
    -- FK — the catalogue rule: the code is the regulatory payload, the
    -- catalogue row is display metadata, and a reimport must never cascade
    -- into records.
    measure_code                  TEXT,
    -- "Intensidad de la medida (Nº de trampas, nº de difusores, etc.)" as a
    -- value + unit pair like every other amount in the book, never free text.
    measure_intensity_value       REAL,
    measure_intensity_unit_code   TEXT REFERENCES unit(code),   -- dimension 'intensity'
    measure_registration_number   TEXT,          -- twin's NumRegistroMDF
    -- Derived from the product, so both go when the product does. The plazo
    -- runs from the LAST application (application_end_date when set).
    phi_days_used                 INTEGER,                       -- input
    phi_end_date                  TEXT,                          -- derived = application date + phi_days_used
    -- LEGAL SNAPSHOTS, frozen at write time and kept BESIDE the foreign keys
    -- above rather than instead of them.
    --
    -- The test each one passes (docs/data-model.md -> "Nothing is ever
    -- frozen"): when the referenced row changes, was the PAST RECORD WRONG, or
    -- did the world merely change? Here it is the former. A product renamed or
    -- re-registered does not change what was sprayed that day — the record
    -- named the old product, and printing today's name against a five-year-old
    -- application would misstate it. Where the world merely changed, the value
    -- is read live and no snapshot exists.
    --
    -- They are RE-TAKEN when their own foreign key is corrected, and only
    -- then: fixing a typo in the date must not shift anything the correction
    -- did not name.
    product_name_snapshot         TEXT,
    authorisation_number_snapshot TEXT,
    -- Flattened to text on purpose: what the label declared at the time, not a
    -- join through today's `product_active_substance` rows.
    active_substances_snapshot    TEXT,
    -- NOT NULL, unlike the product ones: every treatment has an applicator
    -- (Anexo III Parte I B.d), including the non-chemical ones that have no
    -- product at all.
    operator_name_snapshot        TEXT NOT NULL,
    operator_licence_snapshot     TEXT,
    machinery_roma_snapshot       TEXT,          -- mobile machinery registry (the typical case)
    machinery_reganip_snapshot    TEXT,          -- aircraft / fixed installations registry
    notes                         TEXT,
    created_at                    TEXT NOT NULL,
    updated_at                    TEXT NOT NULL,
    deleted_at                    TEXT,
    -- The chemical block is all-or-nothing. Making six columns nullable so a
    -- purely non-chemical actuation can be recorded would otherwise also let a
    -- product be stored with no dose, or with no plazo de seguridad — and a
    -- product application whose phi_end_date is NULL raises no PHI alert,
    -- which is a silent wrong answer rather than a visible gap. This restores
    -- at block level what the NOT NULLs used to guarantee per column.
    CHECK (
        (product_id IS     NULL AND dose_value IS     NULL AND dose_unit_code IS     NULL
                                AND phi_days_used IS  NULL AND phi_end_date IS       NULL
                                AND product_name_snapshot IS NULL)
     OR (product_id IS NOT NULL AND dose_value IS NOT NULL AND dose_unit_code IS NOT NULL
                                AND phi_days_used IS NOT NULL AND phi_end_date IS NOT NULL
                                AND product_name_snapshot IS NOT NULL)
    ),
    -- An actuation has to BE something: a product application, a non-chemical
    -- measure, or both on the same day.
    CHECK (product_id IS NOT NULL OR measure_code IS NOT NULL),
    -- An intensity is a number and its unit or neither — a bare "12" against a
    -- measure states nothing, and a unit with no figure states less.
    CHECK (
        (measure_intensity_value IS NULL     AND measure_intensity_unit_code IS NULL)
     OR (measure_intensity_value IS NOT NULL AND measure_intensity_unit_code IS NOT NULL)
    )
);

-- Junction: one treatment entry applies to many plots, with surface treated per plot.
-- Which plots one treatment covered, and what was growing on each.
--
-- THE CROP LIVES HERE, not on the record: one spraying can cross plots
-- carrying different crops, and the decree asks what was treated on each. A
-- crop column on the record would have to pick one and be wrong about the
-- rest.
CREATE TABLE treatment_plot (
    id                  TEXT PRIMARY KEY,
    treatment_record_id TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    plot_id             TEXT NOT NULL REFERENCES plot(id),
    crop_id             TEXT REFERENCES crop(id),
    -- Hectares actually treated on this plot, which is often LESS than the
    -- plot: spot-treating one corner is ordinary. Required, because "how much
    -- surface received this dose" is what makes the dose meaningful.
    surface_treated_ha  REAL NOT NULL,
    -- Frozen at write time: correcting a crop's species later must not restate
    -- what a past application was made on. See docs/data-model.md ->
    -- "Nothing is ever frozen" for when a snapshot is justified and when the
    -- value is read live instead.
    crop_name_snapshot  TEXT,
    variety_snapshot    TEXT,
    -- The crop's growth stage, as an EST_FENOLOGICO code stored verbatim with no
    -- FK (the catalogue rule). Reglamento (UE) 2023/564's annex asks for the
    -- "growth stage in line with the BBCH monograph" and places it inside the
    -- "Crop or situation/land use" column, so it belongs to the treated crop and
    -- not to the record — which is where the exchange format puts it too
    -- (TratamFito.DGCs[].EstadoFenologico). Footnote 7 makes it conditional, on
    -- a product whose use is restricted to particular stages, hence nullable.
    -- NOTE the catalogue's code (1-10) is NOT the BBCH stage (0-9): the
    -- monograph's principal stage is a column of its own, so every reader
    -- resolves the label through module_cue::catalogue::growth_stage_label
    -- rather than printing this value.
    growth_stage_code   TEXT,
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
-- WHY the treatment was made: the pest, disease or weed it targeted.
--
-- A table rather than a column because one application legitimately targets
-- several problems at once, and each carries its own category — a tank mix
-- against a fungus AND an insect is one treatment with two reasons.
CREATE TABLE treatment_problem (
    id                   TEXT PRIMARY KEY,
    treatment_record_id  TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    -- Pest, disease, weed…: it selects WHICH provider catalogue `problem_code`
    -- is resolved against, so the pair only means something together.
    reason_category_code TEXT NOT NULL REFERENCES reason_category(code),
    -- The provider's code, stored VERBATIM with no foreign key. That is the
    -- standing rule for the big provider lists: the code is the regulatory
    -- payload while the catalogue row is display metadata, so a catalogue
    -- refresh must never cascade into a farmer's records.
    problem_code         TEXT NOT NULL,
    UNIQUE (treatment_record_id, reason_category_code, problem_code)
);

-- The IPM justifications behind a treatment (≥1 per record, enforced in the
-- repository; 2026-07-15). Known at treatment time, unlike efficacy.
-- On what BASIS the decision to treat was taken: a monitored threshold, an
-- official warning, an advisor's recommendation. RD 1311/2012 art. 10-11 make
-- integrated pest management a duty, and this is where the book shows the
-- decision was reasoned rather than routine. Several may apply at once.
CREATE TABLE treatment_justification (
    id                  TEXT PRIMARY KEY,
    treatment_record_id TEXT NOT NULL REFERENCES treatment_record(id) ON DELETE CASCADE,
    -- A small CLOSED list, so unlike `problem_code` it gets a real foreign key
    -- and is mapped to the provider's numbering at export time.
    justification_code  TEXT NOT NULL REFERENCES justification(code),
    UNIQUE (treatment_record_id, justification_code)
);

-- ---------------------------------------------------------------------------
-- Non-field treatments: model sections 3.3, 3.4 and 3.5
--
-- Three printed sections, one table. Postcosecha, locales de almacenamiento and
-- medios de transporte are structurally the same record — date, what was
-- treated, how much of it, the phytosanitary problem, the product and how much
-- was used, the applicator — differing only in WHAT the subject is. Three
-- tables would triplicate the junctions and every query over them.
--
-- Deliberately shaped like treatment_record, because the SIEX twins
-- (TratamientosPostCosecha, TratamientosEdifInstalaciones) demand the same
-- discipline the printed form does not show: coded problems, coded
-- justifications, a named applicator and an observed efficacy. Capturing less
-- would make a future un-parking of the export impossible without a migration.
CREATE TABLE non_field_subject_kind (
    code     TEXT PRIMARY KEY,   -- 'postharvest' | 'storage_premises' | 'transport'
    i18n_key TEXT NOT NULL
);

-- Treatments of things that are not a field: a store, a silo, a lorry, empty
-- premises (the model's registers 3.3, 3.4 and 3.5).
--
-- ONE TABLE for all three, because they are one register with a different
-- subject rather than three registers — the subject kind decides what prints
-- and what is measured, and treating a warehouse in tonnes rather than
-- hectares is a different CLAIM, not a unit slip.
--
-- The columns it shares with `treatment_record` mean the same things there,
-- including the legal snapshots and why they are frozen; see that table above
-- rather than repeating it here.
CREATE TABLE non_field_treatment (
    id                          TEXT PRIMARY KEY,
    season_id                   TEXT NOT NULL REFERENCES season(id),
    farm_id                     TEXT NOT NULL REFERENCES farm(id),
    country_code                TEXT NOT NULL REFERENCES country(code),
    subject_kind_code           TEXT NOT NULL REFERENCES non_field_subject_kind(code),
    treated_on                  TEXT NOT NULL,             -- 'YYYY-MM-DD'
    -- What was treated, in the wording each section asks for: the plant product
    -- (3.3), the premises' type and address (3.4), or the vehicle's type, model
    -- and plate (3.5). Free text because only 3.3 has a coded counterpart.
    subject_description         TEXT NOT NULL,
    -- 3.3 only: the PROD_VEGETAL catalogue code for the plant product treated
    -- (the SIEX twin codes it as ProductoVegetal). That is the HARVESTED
    -- PRODUCE catalogue, not the crop catalogue PRODUCTOS — "Aceitunas", not
    -- "OLIVO". Stored verbatim with NO FK, the treatment_problem.problem_code
    -- rationale. NULL for the other kinds, and for a product no picker matched.
    subject_product_code        TEXT,
    -- 3.4 / 3.5: the registry row identifying the local or vehicle treated
    -- (core's `premises`). NULLABLE, and the export precheck is what demands
    -- it — the `efficacy_code` precedent: refusing a lawful record because the
    -- farmer has not yet created a registry row would be the register blocking
    -- the duty it exists to serve. NULL on every `postharvest` record, which
    -- treats produce and not a place, and on records written before the
    -- registry existed. `subject_description` stays the printed truth and is
    -- composed from this row at write time, re-taken only when this FK changes
    -- (docs/data-model.md → "Nothing is ever frozen").
    premises_id                 TEXT REFERENCES premises(id),
    -- How much of the subject: tonnes for 3.3, cubic metres for 3.4/3.5 — the
    -- repository enforces the pairing. Nullable as a pair: the printed form
    -- leaves the cell hand-fillable and the export precheck is where a format
    -- requirement belongs (the efficacy precedent), not the insert.
    treated_quantity_value      REAL,
    treated_quantity_unit_code  TEXT REFERENCES unit(code),   -- 't' | 'm3'
    product_id                  TEXT NOT NULL REFERENCES product(id),
    -- Product actually used ("Cantidad utilizada, kg o l"), same nullable pair.
    product_quantity_value      REAL,
    product_quantity_unit_code  TEXT REFERENCES unit(code),   -- 'kg' | 'l'
    operator_id                 TEXT NOT NULL REFERENCES operator(id),
    -- Optional, like treatment_record: the SIEX twin's EquipoAplicador object
    -- has no required members, and the printed sections carry no equipment
    -- column at all.
    machinery_id                TEXT REFERENCES machinery(id),
    -- Anexo III Parte I B.d — "identificación del aplicador y, en su caso, del
    -- asesor" — reaches these three registers by B's own words: B.b identifies
    -- what was treated as "la parcela, o en su caso, local o medio de
    -- transporte tratado", and B.f asks for the volume in cubic metres "como
    -- tratamiento de locales". They are B, not a register that resembles it,
    -- so the advisor is captured here exactly as on treatment_record. The
    -- printed model shows no such column, which is why the book folds it into
    -- the applicator cell — the model is orientativo and B is what binds.
    advisor_id                  TEXT REFERENCES advisor(id),
    advisor_name_snapshot       TEXT,
    advisor_registration_snapshot TEXT,        -- the ROPO number in Spain
    -- Observed after the fact, exactly as on treatment_record.
    efficacy_code               TEXT REFERENCES efficacy(code),
    -- legal snapshots, frozen at write time:
    product_name_snapshot       TEXT NOT NULL,
    authorisation_number_snapshot TEXT,
    operator_name_snapshot      TEXT NOT NULL,
    operator_licence_snapshot   TEXT,
    machinery_roma_snapshot     TEXT,
    machinery_reganip_snapshot  TEXT,
    notes                       TEXT,
    created_at                  TEXT NOT NULL,
    updated_at                  TEXT NOT NULL,
    deleted_at                  TEXT
);

-- The coded problems and IPM justifications, ≥1 of each enforced in the
-- repository — the treatment_problem / treatment_justification contract.
-- NOTE for a future export: the twins' problem buckets are narrower than
-- treatment_record's. Postcosecha maps enfermedades / artrópodos-gasterópodos /
-- reguladores-otros, and edificaciones only the first two — neither carries
-- weeds. Capture stays permissive (a record the farmer made is real whatever a
-- parked format accepts); the export precheck is where that gets reported.
CREATE TABLE non_field_treatment_problem (
    id                      TEXT PRIMARY KEY,
    non_field_treatment_id  TEXT NOT NULL REFERENCES non_field_treatment(id) ON DELETE CASCADE,
    reason_category_code    TEXT NOT NULL REFERENCES reason_category(code),
    problem_code            TEXT NOT NULL,
    UNIQUE (non_field_treatment_id, reason_category_code, problem_code)
);

CREATE TABLE non_field_treatment_justification (
    id                      TEXT PRIMARY KEY,
    non_field_treatment_id  TEXT NOT NULL REFERENCES non_field_treatment(id) ON DELETE CASCADE,
    justification_code      TEXT NOT NULL REFERENCES justification(code),
    UNIQUE (non_field_treatment_id, justification_code)
);

-- ---------------------------------------------------------------------------
-- Treated seed: model section 3.2
--
-- What is registered is a SOWING with seed the supplier already treated — not
-- a treatment the farmer applied. Hence the product block is free capture
-- (name, registration number, active substance) with an optional link to the
-- product registry: treated seed arrives in a sack whose label names a product
-- the farmer never bought as such, and forcing a registry row first would stop
-- a lawful record being written.
--
-- The plot linkage below comes from the PRINTED model ("Id. parcelas",
-- "Superficie sembrada"); the SIEX twin `UsoSemillaTratada` carries no plots at
-- all. The model is the compliance artifact, so it wins.

-- Where the seed was treated, and by whom — the twin's required `Tratamiento`.
-- Our own codes, mapped to the FEGA TIPO_TRATAMIENTO integers at export (the
-- efficacy / justification tier-1 pattern). NOTE for a future export precheck,
-- from the field descriptor: the seed lot is required for the two purchased
-- kinds, and the product registration number is only accepted for the two
-- treated-here kinds.
CREATE TABLE seed_treatment_kind (
    code     TEXT PRIMARY KEY,   -- 'on_farm' | 'processing_centre' | 'purchased_es' | 'purchased_abroad'
    i18n_key TEXT NOT NULL
);

CREATE TABLE seed_treatment (
    id                  TEXT PRIMARY KEY,
    season_id           TEXT NOT NULL REFERENCES season(id),
    farm_id             TEXT NOT NULL REFERENCES farm(id),
    sown_on             TEXT NOT NULL,             -- 'YYYY-MM-DD'
    species_name        TEXT NOT NULL,
    variety             TEXT,
    -- Species code in the FEGA PRODUCTOS catalogue, verbatim and without a
    -- foreign key (the treatment_problem.problem_code rationale). The SIEX twin
    -- codes the same thing as `Producto`.
    crop_code           TEXT,
    -- Kilograms of seed sown. Nullable: the printed form leaves the cell to be
    -- filled by hand, and a format that requires it says so at export.
    seed_quantity_kg    REAL,
    -- The seed lot, as printed on the sack (SIEX `NumeroLote`). The one field
    -- that makes a treated-seed record traceable back to its supplier.
    seed_lot            TEXT,
    -- Nullable: the model's own table has no such column, so a book kept only
    -- on paper terms cannot be made to answer it. Records that do state it are
    -- exportable; the rest print the register exactly as the model does.
    treatment_kind_code TEXT REFERENCES seed_treatment_kind(code),
    -- When the treated seed was bought (SIEX `SiembraPlantacion.FechaAdquisicion`,
    -- 'YYYY-MM-DD'). No decree asks for it and no page prints it; Anexo V marks
    -- it Obligatorio inside a block we do send, which is the standing line that
    -- already put `Fertilizacion.BuenasPracticas` and `PlanAbonado.Herramienta`
    -- in the schema.
    --
    -- It belongs HERE rather than on `sowing_record` because what was acquired
    -- is the seed, and this is the seed's own register — the same reason
    -- `seed_lot` sits beside it. Meaningful only for the two purchased kinds;
    -- `MaterialAdquirido` itself needs no column, since TIPO_TRATAMIENTO 4 and 5
    -- are literally "adquisición de semilla tratada" and `treatment_kind_code`
    -- is that catalogue.
    acquired_on         TEXT,
    -- Which sowing used this seed (SIEX `SiembraPlantacion.MaterialTratado`,
    -- and the descriptor's own `UsoSemillaTratada.IdAjenaSiembraPlant`).
    --
    -- The direction is forced twice over. A module may reference a core table
    -- and never the reverse, so the column can only live on this side; and one
    -- sowing may use several seed lots, each naming it, which a single column on
    -- `sowing_record` would cap at one. Nullable throughout: the two registers
    -- are filled independently on the printed model and neither decree links
    -- them, so an unlinked record stays lawful and simply exports as material
    -- whose sowing was not named.
    sowing_record_id    TEXT REFERENCES sowing_record(id),
    -- What the seed was dressed with, as free text read off the seed bag's
    -- label. Free rather than a product reference because treated seed is
    -- BOUGHT already treated: the farmer never handled the product, is not its
    -- applicator, and may have nothing but the bag to go on.
    product_name        TEXT NOT NULL,
    product_registration_number TEXT,
    product_active_substance    TEXT,
    -- Set only when the treated seed's product is also in the farmer's own
    -- registry; the free-text fields above stay the printed truth either way.
    product_id          TEXT REFERENCES product(id),
    -- Observed after emergence, so it cannot be demanded at insert — the
    -- treatment_record rule. The SIEX twin lists Eficacia as required, which
    -- an export precheck is the place to enforce.
    efficacy_code       TEXT REFERENCES efficacy(code),
    notes               TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

-- Where the treated seed went, and how much ground it covered. Reconciled from
-- the submitted form state on update, like the extension tables.
-- Where the treated seed went. Unlike the other plot junctions this one
-- carries no crop snapshot: the crop is named on the parent record, because
-- treated seed IS the crop being started rather than something applied to an
-- existing one.
CREATE TABLE seed_treatment_plot (
    id                 TEXT PRIMARY KEY,
    seed_treatment_id  TEXT NOT NULL REFERENCES seed_treatment(id) ON DELETE CASCADE,
    plot_id            TEXT NOT NULL REFERENCES plot(id),
    surface_sown_ha    REAL NOT NULL,      -- hectares sown on this plot
    UNIQUE (seed_treatment_id, plot_id)
);

-- The official model heads each conditional register with "APLICA TRATAMIENTO:
-- ☐SÍ ☐NO". SÍ derives from rows existing; NO cannot — an empty register is
-- indistinguishable from an unfilled one, and only one of those is evidence
-- that the farmer checked. So the negative is stored, exactly as
-- plot_zone_flag stores an 'outside' result.
CREATE TABLE register_kind (
    code     TEXT PRIMARY KEY,   -- 'seed_treatment' | 'postharvest' | 'storage_premises' | 'transport'
    i18n_key TEXT NOT NULL
);

CREATE TABLE register_declaration (
    id            TEXT PRIMARY KEY,
    -- Scoped to one holding and one campaign: "this farm, this year, did none
    -- of that", which is a statement about a campaign and not a standing one.
    farm_id       TEXT NOT NULL REFERENCES farm(id),
    season_id     TEXT NOT NULL REFERENCES season(id),
    -- WHICH register is being declared empty. A row here is the model's
    -- "APLICA TRATAMIENTO: NO" — proof the farmer considered the register and
    -- had nothing to record, which is why a register prints in THREE states:
    -- filled, declared empty, or simply untouched. The third is not the same
    -- claim as the second, and the book must not conflate them.
    register_code TEXT NOT NULL REFERENCES register_kind(code),
    declared_on   TEXT NOT NULL,             -- 'YYYY-MM-DD', when the farmer said so
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);

-- One live declaration per register and campaign; a withdrawn one keeps its
-- history (soft delete), so re-declaring does not resurrect the old row.
CREATE UNIQUE INDEX idx_register_declaration_active
    ON register_declaration(farm_id, season_id, register_code)
    WHERE deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Analyses: model section 4
--
-- Metadata only. The register records that an analysis was made and where its
-- bulletin can be found; the bulletin itself stays in the farmer's folder,
-- which art. 16.3 obliges keeping for three years and the annex page states.
-- The app has no attachment capability, and giving it one has backup, sync and
-- mobile-storage consequences that belong to their own decision.
--
-- Section 4 carries no "APLICA TRATAMIENTO: SÍ/NO" line — it is a
-- model-recommended register (art. 16.3's conservation duty), not one of the
-- conditional ones — so no register_declaration code backs it.
-- The model's "Material analizado". Our own codes, mapped to the FEGA
-- MATERIAL_ANALIZADO integers at export — and there are FOUR of them, because
-- the authority distinguishes the standing crop from the produce taken off it.
-- The model's parenthetical hint (vegetal / tierra / agua) cannot express that
-- split, so the book prints FEGA's wording instead.
CREATE TABLE analysis_material (
    code     TEXT PRIMARY KEY,   -- 'crop' | 'harvested_produce' | 'soil' | 'water'
    i18n_key TEXT NOT NULL
);

-- What the laboratory looked for — the twin's `TiposAnalisis[]`, an array, so a
-- junction. Our own codes over the FEGA TIPO_ANALISIS six.
CREATE TABLE analysis_type (
    code     TEXT PRIMARY KEY,   -- what was determined: 'actives', 'heavy_metals', 'nutrients', 'soil_parameters'
    i18n_key TEXT NOT NULL
);


CREATE TABLE analysis_record (
    id                  TEXT PRIMARY KEY,
    season_id           TEXT NOT NULL REFERENCES season(id),
    farm_id             TEXT NOT NULL REFERENCES farm(id),
    sampled_on          TEXT NOT NULL,             -- 'YYYY-MM-DD'
    material_kind_code  TEXT NOT NULL REFERENCES analysis_material(code),
    -- The laboratory's own reference for the bulletin — what an inspector asks
    -- for to obtain the original document from the lab.
    bulletin_number     TEXT,
    lab_name            TEXT,
    -- The printed model asks for "Laboratorio (nombre y dirección)"; the twin
    -- carries only a name and a NIF. The model is the compliance artifact.
    lab_address         TEXT,
    lab_tax_id          TEXT,       -- NIF; what the SIEX twin identifies the lab by
    -- Free text, KEPT alongside the coded analysis_substance junction rather
    -- than replaced by it: SUST_ACTIVAS only codes phytosanitary actives
    -- (TipoAnalisis 1), so a heavy-metals, nutrients or soil-parameters
    -- bulletin has nothing to code and would otherwise be unrecordable.
    substances_detected TEXT,

    -- Anexo III Parte I A.3 — the soil block (added 2026-08-08).
    --
    -- It lives HERE rather than in a soil table, and here rather than in
    -- module-fertilisation, for two separate reasons. The SIEX twin settles
    -- the first: `Analitica.ParametrosSuelo` is a sub-object OF an analysis,
    -- because soil data reaches a holding as a laboratory bulletin like any
    -- other. The module boundary settles the second: `analysis_record` is
    -- module-cue's table, and a module may never add columns to another
    -- module's schema — so although the *consumer* of soil data is the
    -- fertilisation domain (RD 1051/2022 art. 5.b and art. 6 make it an input
    -- to the plan de abonado), the columns belong to the crate that owns the
    -- register, and the record book reads across both.
    --
    -- All nullable, and deliberately: A.3's minimums bind only one year after
    -- MAPA publishes its sampling and analysis guides, which it has not, and a
    -- bulletin reports whatever the farmer asked to be measured.
    --
    -- Units are fixed by the column name, the `water_nitric_n_mg_l` precedent
    -- — the twin states none. Safe here where importing a provider's number
    -- would not be: the farmer reads a figure off a bulletin into a field
    -- whose label states the unit, converting if their lab used another.
    soil_ph                  REAL,   -- dimensionless
    soil_organic_matter_pct  REAL,   -- % of dry matter
    soil_available_p_mg_kg   REAL,   -- P asimilable (Olsen/Bray), mg/kg
    soil_available_k_mg_kg   REAL,   -- K asimilable, mg/kg
    soil_total_n_pct         REAL,   -- N total, %
    soil_conductivity_ds_m   REAL,   -- CE at 25 °C, dS/m
    -- Texture is THREE figures in the twin (`Arena`/`Limo`/`Arcilla`), not one
    -- class name: the repository checks they sum to 100 when all three are
    -- given, since they are fractions of one whole.
    soil_sand_pct            REAL,   -- arena, % of the mineral fraction
    soil_silt_pct            REAL,   -- limo, %
    soil_clay_pct            REAL,   -- arcilla, %

    notes               TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

-- What was sampled, as the model's "Cultivo o cosecha muestreados" (order
-- numbers from table 2.1). No surface column: the model asks which parcels, not
-- how much of them. Reconciled from the submitted form state on update.
CREATE TABLE analysis_plot (
    id                 TEXT PRIMARY KEY,
    analysis_record_id TEXT NOT NULL REFERENCES analysis_record(id) ON DELETE CASCADE,
    plot_id            TEXT NOT NULL REFERENCES plot(id),
    crop_id            TEXT REFERENCES crop(id),
    crop_name_snapshot TEXT,                     -- frozen crop at sampling time
    variety_snapshot   TEXT,
    UNIQUE (analysis_record_id, plot_id)
);

-- WHAT the bulletin analysed, as a junction rather than a column: one sample
-- routinely goes to the lab for several determinations at once (actives,
-- heavy metals, nutrients), and the record must be able to say so.
CREATE TABLE analysis_record_type (
    id                  TEXT PRIMARY KEY,
    analysis_record_id  TEXT NOT NULL REFERENCES analysis_record(id) ON DELETE CASCADE,
    analysis_type_code  TEXT NOT NULL REFERENCES analysis_type(code),
    UNIQUE (analysis_record_id, analysis_type_code)
);

-- The active substances a residue analysis found, as FEGA SUST_ACTIVAS codes
-- stored verbatim with NO FK — the treatment_problem.problem_code rule. The
-- catalogue carries each substance's CAS number, which is the key a future
-- French or Italian export would match on; that, not its size, is why the code
-- is the payload here.
--
-- A code absent from the vendored snapshot is ACCEPTED, never rejected: the
-- snapshot travels with app releases and a laboratory does not wait for one.
-- Which phytosanitary actives the bulletin DETECTED, coded. Kept beside
-- `analysis_record.substances_detected` rather than replacing it: the
-- catalogue codes only phytosanitary actives, so a heavy-metals or nutrients
-- bulletin has nothing codeable and needs the free-text column to say anything
-- at all.
CREATE TABLE analysis_substance (
    id                  TEXT PRIMARY KEY,
    analysis_record_id  TEXT NOT NULL REFERENCES analysis_record(id) ON DELETE CASCADE,
    -- Provider catalogue code, verbatim and FK-free — the standing rule for
    -- the big provider lists.
    substance_code      TEXT NOT NULL,
    UNIQUE (analysis_record_id, substance_code)
);

-- Derived trigger + user acknowledgement state (PHI / licence / ITV). Rows are owned by
-- the reconciling refresh: derived from source tables, deleted when the condition lapses.
-- Derived state → excluded from record_change and from sync (each device re-derives).
-- Standing conditions the farmer should act on: an open plazo de seguridad, a
-- licence about to expire, an overdue ITV, a plot inside a nitrate zone.
--
-- DERIVED STATE, and the only table here that is: every row is a pure function
-- of (the source tables, today), re-created by the reconciling
-- `refresh_alerts`. That is why it is deliberately EXCLUDED from
-- record_change and from sync — each device re-derives its own, and logging
-- them would pollute the delta source with rows that carry no information.
--
-- The cost is accepted and worth knowing: an acknowledgement does not travel
-- between devices. Revisit when sync is designed.
CREATE TABLE alert (
    id              TEXT PRIMARY KEY,
    alert_type_code TEXT NOT NULL REFERENCES alert_type(code),
    -- The campaign the source record belongs to, for PHI alerts. NULL for the
    -- ones that are not season-scoped: a licence expires whatever the campaign.
    season_id       TEXT REFERENCES season(id),
    -- WHAT the alert is about, polymorphic and FK-free like record_change:
    -- alerts point at rows in three different tables, and a deleted subject
    -- must be able to take its alert with it through reconciliation rather
    -- than through a cascade.
    subject_table   TEXT NOT NULL,                -- e.g. 'treatment_record', 'operator', 'machinery'
    subject_id      TEXT NOT NULL,
    -- The date the condition turns on: the plazo's end, the licence's expiry,
    -- the ITV's due date. NULL for conditions with no date (a zone flag).
    due_date        TEXT,
    lead_days_used  INTEGER,                      -- input behind expiry alerts; NULL for phi_window
    -- 'active' | 'acknowledged' | 'dismissed'. The ONE column the refresh never
    -- touches: it re-creates rows, corrects their dates and deletes what has
    -- lapsed, but a dismissal is a decision the farmer made and must never be
    -- resurrected by a background pass.
    status          TEXT NOT NULL DEFAULT 'active',
    acknowledged_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    -- One alert per condition: makes the reconciling refresh idempotent by construction.
    UNIQUE (alert_type_code, subject_table, subject_id)
);

-- NOTE on what is NOT here. Every junction in this file declares
-- `UNIQUE (<parent>_id, …)`, and SQLite builds an index for that constraint
-- whose leading column is the parent. A second `CREATE INDEX` on the parent
-- alone is a duplicate: the planner picks the unique index for exactly the same
-- seeks, and the copy costs a write on every insert forever. Twelve such
-- indexes were removed on 2026-08-24, verified against EXPLAIN QUERY PLAN
-- before and after. Add one only for a lookup NO unique constraint already
-- leads with.
-- The house pattern every other register already carried: a book is read one
-- campaign of one holding at a time, so the composite serves that directly and
-- its season prefix still answers the season-delete guard. This table had two
-- single-column indexes instead, which made listing one season search a farm's
-- whole history and discard the rest in the WHERE.
CREATE INDEX idx_treatment_record_book    ON treatment_record(season_id, farm_id);

-- The open PHI windows, and nothing else. Both readers of phi_end_date ask a
-- question about TODAY — the alert refresh across every holding, the map's tint
-- within one — so the date leads and the farm rides behind it: a range scan
-- bounded by the windows still open rather than by the treatments ever
-- recorded. Partial, because a record with no product opens no window at all
-- and a withdrawn one carries no restriction, so neither belongs in the index.
CREATE INDEX idx_treatment_record_phi     ON treatment_record(phi_end_date, farm_id)
    WHERE deleted_at IS NULL AND phi_end_date IS NOT NULL;
CREATE INDEX idx_non_field_treatment_book ON non_field_treatment(season_id, farm_id);

-- The two link columns added in August 2026, each with a reader that filters on
-- it and neither indexed until 2026-08-24 — which is the shape of the whole
-- arc: a new column arrives with a query, and the index is the half nobody
-- notices is missing, because the query is right either way.
--   * `premises_id` — which registers a named store already appears in, read
--     before the registry allows it to be retired.
--   * `sowing_record_id` — the treated-seed rows that name one sowing, read by
--     the export to state `MaterialTratado` about it.
-- Partial, because both readers ask only about live rows.
CREATE INDEX idx_non_field_premises   ON non_field_treatment(premises_id)
    WHERE deleted_at IS NULL AND premises_id IS NOT NULL;
CREATE INDEX idx_seed_treatment_sowing ON seed_treatment(sowing_record_id)
    WHERE deleted_at IS NULL AND sowing_record_id IS NOT NULL;
CREATE INDEX idx_register_declaration_book ON register_declaration(farm_id, season_id);
CREATE INDEX idx_seed_treatment_book      ON seed_treatment(season_id, farm_id);
CREATE INDEX idx_analysis_record_book     ON analysis_record(season_id, farm_id);
CREATE INDEX idx_alert_status_due         ON alert(status, due_date);
