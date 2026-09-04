-- Terrazgo eco-scheme module — migration 0001: schema (DDL only; seed data lives in 0002).
--
-- This module owns the record book's THIRD decree. RD 1311/2012 governs the
-- phytosanitary registers (module-cue) and RD 1051/2022 the fertilisation ones
-- (module-fertilisation). RD 1054/2022 anexo II ends its list of what the
-- cuaderno must contain with "otros aspectos que se recojan en la respectiva
-- normativa sectorial", and **RD 1048/2022** is that sectoral norm for anyone
-- claiming an ecorrégimen: ten clauses ordering an annotation in the cuaderno,
-- most of them within one month of the activity. Printed model section 9.
--
-- The organising principle is the DECREE, never the printed form. Reading the
-- form would have lost three things (docs/cuaderno-print.md transcribes all of
-- them): anexo IV's duty has no printed page at all, art. 42 is three
-- annotations with three different deadlines that the form collapses into one
-- row, and model 9.3 prints only three of the five dates art. 45.2 names. So
-- this module ships THREE registers shaped like the decree's groupings — and
-- like the exchange format's own blocks — rather than five shaped like the
-- model's sub-tables.
--
-- Two of the three link to the third: `grazing_record` and `cultural_operation`
-- each carry a nullable `soil_cover_id`, declared before `soil_cover` itself is
-- created further down. SQLite resolves a foreign key by NAME when a row is
-- written rather than when the table is declared, so a forward reference inside
-- one migration is legal; the registers stay in the model's own order because
-- that is the order a reader of the printed book expects.
--
-- Pre-release this file is squashed freely (dev databases are recreated, not
-- migrated); it becomes append-only the moment any database contains real data.
-- Core steps run EARLIER in the composed global sequence, so references to
-- season, farm and plot are valid. This module references module-cue's and
-- module-fertilisation's tables NOWHERE — modules never depend on each other.
--
-- Conventions (see docs/data-model.md):
--   * snake_case, singular table names, lowercase English enum values.
--   * User-data PKs are UUIDv7 stored as 36-char TEXT, generated in Rust at insert.
--   * Reference/lookup tables use short stable TEXT codes and ship seeded.
--   * Dates: ISO 8601 TEXT in UTC ('YYYY-MM-DDTHH:MM:SSZ'); date-only as 'YYYY-MM-DD'.
--   * No user-facing strings here — reference tables carry an i18n_key only.

-- ============================================================================
-- Reference / lookup tables (app-versioned, seeded in 0002, not synced)
-- ============================================================================

-- Which of RD 1048/2022's six register-level annotation duties a record
-- evidences. Every register in this module carries it, because the same
-- activity means different things under different practices: a mowing is
-- P2's mandated maintenance on one plot and P6's cover maintenance on another,
-- and the deadline that governs it differs accordingly.
--
-- Owned rather than read from a catalogue because **FEGA publishes no P1–P7
-- list at all** — verified across all 287 entries of its catalogue registry
-- (docs/maintenance.md §1 has the enumeration recipe). And it is NOT derivable
-- from `TIPO_COBERTURA_SUELO`, the nearest-looking file: that catalogue's value
-- 1 (suelo desnudo) and 6 (regeneración de pastos permanentes) belong to
-- neither cover practice, and its 5 (otros materiales) is not P7 either, which
-- is specifically restos de poda triturados.
CREATE TABLE eco_practice (
    code     TEXT PRIMARY KEY,   -- 'extensive_grazing', 'plant_cover', …
    i18n_key TEXT NOT NULL
);

-- What was done on the land. FEGA `TIPO_LABOR` (14 active values, 0–13) is the
-- only "what was done" vocabulary the registry publishes, and SIEX's
-- `LaboresCulturales` speaks it — but this is a tier-1 OWNED lookup mapped to
-- it in `siex.rs` rather than the provider code stored verbatim, for two
-- reasons that are both about what the book prints:
--
--   * `TIPO_LABOR` 5 is "Desbroce y siega", one code where model 9.4 prints
--     two columns (Siega and Desbrozado). Storing the provider code would make
--     that distinction unrecordable, so `mowing` and `brush_cutting` are two
--     of ours mapping onto the one of theirs. The map is therefore
--     deliberately NOT injective, which its contract test pins explicitly.
--   * A verbatim catalogue code carries no i18n key, so it would print its
--     Spanish label in the Catalan book — against the rule that prose
--     translates while codes do not (docs/cuaderno-print.md → "Language of the
--     book"): this is prose, the FEGA label is not the record's legal payload.
--
-- The contract test runs in BOTH directions, and the second one is a watchdog:
-- every active `TIPO_LABOR` row must be claimed by one of ours, so the day FEGA
-- publishes a finer word (a siega separate from a desbroce, say) the suite
-- fails and somebody decides whether it deserves an owned kind. An unmapped
-- upstream code is a missed opportunity rather than a defect — our lookup is
-- the stored vocabulary, so nothing breaks and no picker changes.
CREATE TABLE cultural_operation_kind (
    code     TEXT PRIMARY KEY,   -- 'mowing', 'brush_cutting', 'pruning', …
    i18n_key TEXT NOT NULL
);

-- ============================================================================
-- 9.1 — pastoreo extensivo (RD 1048/2022 art. 30.2 ter). SIEX twin: `Pastoreo`
-- ============================================================================

-- The register of a grazing: which animals grazed which plots, from when to
-- when. Art. 30.2 ter obliges the annotation when the dates differ from those
-- declared in the solicitud única, **within one month of the new date** — and
-- the model's own footnote counts that month from the END of grazing, which is
-- why `ended_on` is the deadline-bearing column and not `started_on`.
CREATE TABLE grazing_record (
    id             TEXT PRIMARY KEY,
    season_id      TEXT NOT NULL REFERENCES season(id),
    farm_id        TEXT NOT NULL REFERENCES farm(id),

    -- Which duty this evidences. Narrowed by the repository to the practices a
    -- grazing can evidence: P1 itself, P2's pastoreo as a maintenance activity,
    -- a comunal pasture's anexo IV activities, and — since seam 4 — P6's
    -- pastoreo over a live cover, which art. 42.1.c counts as maintenance and
    -- model 9.4 prints as a column of its own. Not narrowed by the schema,
    -- because a CHECK listing codes would have to be edited in a migration
    -- every time the vocabulary grew.
    practice_code  TEXT NOT NULL REFERENCES eco_practice(code),

    -- Set when the animals grazed a cover rather than a pasture: art. 42.1.c's
    -- "tipo de mantenimiento", of which model 9.4 prints pastoreo as one of
    -- three columns. A grazing is still a grazing whichever land it happens on,
    -- and `Pastoreo` is still its twin, so it stays in this register instead of
    -- becoming a fourth kind of cultural operation.
    --
    -- The link also PARTITIONS the two printed pages: model 9.1 prints the
    -- grazings with no cover and 9.4's Pastoreo column prints the ones with
    -- one, so no grazing is ever printed twice — the P1 register would
    -- otherwise show a P6 cover grazing as if it were extensive grazing.
    soil_cover_id  TEXT REFERENCES soil_cover(id),

    -- Model 9.1 column 1, "Id. del grupo de parcelas". A free label, and
    -- deliberately permissive: the model asks for it only when the plot or
    -- group lies more than 10 km from the main livestock installation, which
    -- the app cannot know — there is no installation entity and no distance to
    -- compute. So the rule lives in the printed footnote and the field is the
    -- farmer's to fill, the `efficacy_code` precedent (capture what is known,
    -- do not invent what is not).
    plot_group_ref TEXT,

    started_on     TEXT NOT NULL,   -- 'YYYY-MM-DD', Pastoreo.FechaInicio
    -- NULL = the animals are still grazing. Not "unknown": the deadline runs
    -- from the end, so an open record is not yet late, and the advisory says
    -- exactly that rather than claiming a missed annotation.
    ended_on       TEXT,            -- 'YYYY-MM-DD', Pastoreo.FechaFin

    -- No columns for Pastoreo.AnimalesPropios / AnimalesTerceros. The twin
    -- types both as booleans — Anexo V reads "Pastoreo con animales de la
    -- explotación (S/N)" — so they ask WHETHER, not how many, and the answer
    -- falls out of each grazing_animal line: a line whose rega_code is this
    -- holding's own is its own animals, any other is a third party's. Storing
    -- it would be derived state that can drift. The printed model's own
    -- "Nº animales desplazados al pasto" is the per-line animal_count.

    notes          TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    deleted_at     TEXT
);

CREATE INDEX idx_grazing_record_season ON grazing_record (season_id, farm_id);

-- Model 9.4 resolves its Pastoreo column by cover, once per book rather than
-- once per printed cover row.
CREATE INDEX idx_grazing_record_cover ON grazing_record (soil_cover_id);

-- Model 9.1's "Referencia SIGPAC de la parcela o grupo de parcelas". The
-- printed reference is resolved from the plot at print time, never stored
-- here: it lives on `plot_es_extension` and a record that froze it would
-- disagree with the parcel register after a correction.
-- Which parcels were grazed. No surface column and no crop snapshot, unlike
-- the treatment and sowing junctions: the annotation this register owes is
-- WHICH land the animals were on and for how long, not how much of each parcel
-- they covered.
CREATE TABLE grazing_plot (
    id                TEXT PRIMARY KEY,
    grazing_record_id TEXT NOT NULL REFERENCES grazing_record(id) ON DELETE CASCADE,
    plot_id           TEXT NOT NULL REFERENCES plot(id),
    UNIQUE (grazing_record_id, plot_id)
);

-- `Pastoreo.Animales[]` = {REGA, Numero, Especie}, and model 9.1's last three
-- columns. A junction because the twin is an array and because one grazing can
-- move animals of two species, or animals from two livestock holdings, onto
-- the same pasture — each combination is its own line on the printed page.
CREATE TABLE grazing_animal (
    id                TEXT PRIMARY KEY,
    grazing_record_id TEXT NOT NULL REFERENCES grazing_record(id) ON DELETE CASCADE,
    -- FEGA `ESPECIE_ANIMAL`, verbatim and with NO foreign key — the catalogue
    -- rule: the code is the regulatory payload, the catalogue row is display
    -- metadata, and a reimport must never cascade into user records.
    species_code      TEXT NOT NULL,
    -- The livestock holding the animals come from. Prefilled from the farm's
    -- own `farm_es_extension.rega_code`, but stored per row and free text,
    -- because third-party animals carry their owner's REGA, not this farm's.
    rega_code         TEXT NOT NULL,
    -- Head count of this species from this holding. The unit is animals, not
    -- livestock units (UGM): the annotation asks how many head grazed, and
    -- converting to UGM would be an interpretation this register does not make.
    animal_count      INTEGER NOT NULL,
    -- One row per (holding, species): the same flock recorded twice is a slip,
    -- while two species from the same holding are two genuine rows.
    UNIQUE (grazing_record_id, rega_code, species_code)
);

-- ============================================================================
-- 9.2 + anexo IV — what was done on the land. SIEX twin: `LaboresCulturales`
-- ============================================================================

-- One operation carried out on one or more plots. This is the decree's widest
-- register and the reason the module is not shaped like the printed form: FOUR
-- separate duties land in this one table, on three different pages.
--
--   * art. 31 / 31.4.d — "la fecha y las actividades realizadas" on a P2 plot:
--     siega for production or maintenance, and any other anexo III.B activity.
--     Model 9.2.
--   * anexo IV — the same annotation for each pasto comunal plot, with the
--     invoices kept as evidence. **The printed model has NO page for this**;
--     the book gives it one, numbered "9.6" (docs/cuaderno-print.md).
--   * art. 45.2 — the nivelación and construcción de caballones dates on a
--     flooded crop, which model 9.3 omits from its own columns. Seam 3.
--   * art. 42.1.c — the maintenance performed ON a cover, linked back with a
--     nullable `soil_cover_id`. Model 9.4 prints siega and desbroce as two of
--     its three maintenance columns (the third, pastoreo, is a grazing).
--
-- What separates them is `practice_code`, which therefore decides the page a
-- row prints on. There is no column for the aid line the farmer claimed: the
-- solicitud única is unreachable by any route this app has, so the practice is
-- the farmer's statement about what the record evidences.
CREATE TABLE cultural_operation (
    id                  TEXT PRIMARY KEY,
    season_id           TEXT NOT NULL REFERENCES season(id),
    farm_id             TEXT NOT NULL REFERENCES farm(id),

    -- Which duty this evidences; narrowed by the repository to the five a
    -- cultural operation can carry — every practice except `extensive_grazing`,
    -- whose art. 30.2 ter duty is about grazing DATES and nothing else.
    practice_code       TEXT NOT NULL REFERENCES eco_practice(code),

    -- What was done, from this module's owned vocabulary (see the lookup's
    -- comment for why it is owned rather than `TIPO_LABOR` verbatim).
    operation_kind_code TEXT NOT NULL REFERENCES cultural_operation_kind(code),

    -- 'YYYY-MM-DD'. `LaboresCulturales` carries both ends, and an operation
    -- that ran over several days is one activity rather than several — the
    -- `treatment_record.application_end_date` precedent. NULL end = a single
    -- day, never "unknown".
    performed_on        TEXT NOT NULL,
    performed_end_date  TEXT,

    -- Model 9.2 footnote (4): the "otras actividades de mantenimiento" column
    -- asks for a date **and the activity**. The kind code answers most of that,
    -- but anexo III.B's list is open-ended and art. 31 says "cualquier otra
    -- actividad de mantenimiento", so free text carries what no code can.
    activity_description TEXT,

    -- FEGA `DEST_RES_VEG`, verbatim and with no foreign key (the catalogue
    -- rule). Its value 9, "Trituración de restos de poda y depositado sobre el
    -- terreno", **IS the P7 practice**: an inert cover comes into being because
    -- a poda row said 9. The twin agrees — `DepositadoSueloDesb`/`Poda` sit on
    -- `LaboresCulturales` and not on `DatosCubierta`, so a serializer derives
    -- both booleans from this one code rather than from the cover.
    residue_destination_code TEXT,

    -- Set when this operation maintained a cover: art. 42.1.c's "tipo de
    -- mantenimiento". NULL for every other duty, which is most rows.
    soil_cover_id       TEXT REFERENCES soil_cover(id),

    notes               TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT
);

CREATE INDEX idx_cultural_operation_season ON cultural_operation (season_id, farm_id);

-- Model 9.4's Siega and Desbrozado columns, resolved by cover once per book.
CREATE INDEX idx_cultural_operation_cover ON cultural_operation (soil_cover_id);

-- Which plots the operation covered. No surface column: model 9.2 prints the
-- plot's own SIGPAC surface, read from the parcel register at print time, and
-- an operation does not partially cover a recinto the way a treatment does.
-- Which parcels the operation was carried out on. Surface-free for the same
-- reason as `grazing_plot`: the duty is to annotate where and when.
CREATE TABLE cultural_operation_plot (
    id                   TEXT PRIMARY KEY,
    cultural_operation_id TEXT NOT NULL REFERENCES cultural_operation(id) ON DELETE CASCADE,
    plot_id              TEXT NOT NULL REFERENCES plot(id),
    UNIQUE (cultural_operation_id, plot_id)
);

-- ============================================================================
-- 9.4 + 9.5 — cubiertas (RD 1048/2022 arts. 42 y 43). SIEX twin: `DatosCubierta`
-- ============================================================================

-- A cover established over one or more plots: a live one of spontaneous or sown
-- vegetation (P6, art. 42) or an inert one of triturated pruning residue (P7,
-- art. 43). ONE table for both, because the two articles ask for the same three
-- things and the exchange format gives them one block — `practice_code` is what
-- separates model 9.4 from model 9.5, exactly as it separates 9.2 from "9.6".
--
-- **Art. 42 is three annotations on three different deadlines**, which the
-- model's single row collapses and which this table therefore splits:
--
--   * 42.1.a / 43.1.a — the establishment date, due within a month. That is
--     `established_on`, and it is what brings the row into existence.
--   * 42.1.e / 43.1.b — the two widths, due within the month before a later
--     period ends. Entered afterwards, on their own, which is why they are
--     nullable and why `widths_stated_on` exists (see below).
--   * 42.1.c — the maintenance, due on a third date and recorded as
--     `cultural_operation` or `grazing_record` rows pointing back here.
--
-- A single "cover" row carrying one date would satisfy none of that. This is
-- the clearest case in the book of a register derived from the decree rather
-- than from the form that renders it.
CREATE TABLE soil_cover (
    id              TEXT PRIMARY KEY,
    season_id       TEXT NOT NULL REFERENCES season(id),
    farm_id         TEXT NOT NULL REFERENCES farm(id),

    -- `plant_cover` (P6, model 9.4) or `inert_cover` (P7, model 9.5); narrowed
    -- by the repository, not by a CHECK, for the reason given on
    -- `grazing_record.practice_code`.
    practice_code   TEXT NOT NULL REFERENCES eco_practice(code),

    -- FEGA `TIPO_COBERTURA_SUELO`, verbatim and with no foreign key (the
    -- catalogue rule). NOT validated against the practice: the catalogue is a
    -- provider registry that grows between releases — it gained value 6 in
    -- 2024 — and the in-app refresh means a farmer's own copy can carry a code
    -- this build has never seen. Refusing it would lock them out of recording a
    -- lawful cover, so the FORM narrows the picker per practice (2 sembrada /
    -- 3 espontánea for P6, 4 restos de poda for P7) and a contract test pins
    -- the leftovers, so a code FEGA adds makes somebody decide rather than
    -- passing unnoticed.
    --
    -- The model prints no column for it: art. 42.1.a annotates the DATE of
    -- establishment of a cover "espontánea o sembrada", not which of the two it
    -- was, so the distinction lives in the printed footnote. It is stored
    -- because the twin's `TipoCobertura` asks for it and the workbook can carry
    -- what the page has no column for.
    cover_type_code TEXT NOT NULL,

    -- 42.1.a / 43.1.a. The subject of the record: a cover exists from here.
    -- Art. 43.1.a additionally forbids an inert cover established later than
    -- 15 April — reported by the advisory, never refused here, because the book
    -- records what happened and does not decide whether an aid was earned.
    established_on  TEXT NOT NULL,

    -- 42.1.e / 43.1.b, the second annotation, on its own deadline. All three
    -- together or none of them (`invalid.incomplete_widths`) — the
    -- `plot_water_point.distance_m` pairing: a width beside no statement date,
    -- or one width without the other, is a wrong answer rather than a missing
    -- one.
    --
    -- `widths_stated_on` is the column neither the decree nor the twin asks
    -- for. It exists because the deadline is what the annotation is about: with
    -- it, "measured in June" and "never measured" are distinguishable at query
    -- time, which is exactly the question the advisory has to answer. Without
    -- it they are the same NULL.
    width_m             REAL,   -- metres: the cover strip's own width
    -- Metres: the width of unshaded ground between canopies, which is the
    -- separate figure art. 42.1.e asks for alongside the cover's width.
    free_canopy_width_m REAL,
    widths_stated_on    TEXT,   -- 'YYYY-MM-DD', when the two widths above were measured

    notes           TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX idx_soil_cover_season ON soil_cover (season_id, farm_id);

-- Which plots the cover was established over. `DatosCubierta.DGCs[]`, and model
-- 9.4/9.5's "Id. Parcelas" column. No surface: the cover's extent is its two
-- widths, which is what both articles ask for.
-- Which parcels carry this cover.
CREATE TABLE soil_cover_plot (
    id            TEXT PRIMARY KEY,
    soil_cover_id TEXT NOT NULL REFERENCES soil_cover(id) ON DELETE CASCADE,
    plot_id       TEXT NOT NULL REFERENCES plot(id),
    UNIQUE (soil_cover_id, plot_id)
);
