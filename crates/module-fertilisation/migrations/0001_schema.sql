-- Terrazgo fertilisation module — migration 0001: schema (DDL only; seed data lives in 0002).
--
-- This module owns the record book's SECOND decree. RD 1311/2012 governs the
-- phytosanitary registers (module-cue); RD 1051/2022 art. 5, as amended by
-- RD 934/2025, creates the cuaderno's fertilisation section and puts irrigation
-- doses and dates in the same duty — binding since 1 January 2026, recorded
-- within one month of each operation. Model sections 6, 7.1 and 8.
--
-- The binding field list is NOT the printed model: art. 5.d and 5.e both
-- redirect to RD 1311/2012 Anexo III Parte I **sección C**, which is wider
-- (docs/cuaderno-print.md transcribes it letter by letter, and the columns
-- below cite their letter).
--
-- Pre-release this file is squashed freely (dev databases are recreated, not
-- migrated); it becomes append-only the moment any database contains real data.
-- Core steps run EARLIER in the composed global sequence, so the references to
-- season, farm, plot, crop and unit below are valid. This module references
-- module-cue's tables NOWHERE — modules never depend on each other.
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

-- The irrigation system used for ONE watering, model section 8's "Sistema de
-- Riego". The model's own footnote is the FEGA `SIST_RIEGO` catalogue verbatim
-- (8 values), and SIEX `Riego` requires the code per event.
--
-- NOT the same list as core's `irrigation_system`, and the difference is the
-- reason both exist. Core's four values (rainfed/sprinkler/drip/gravity)
-- characterise the PLOT — Anexo III A.2.e's "secano o regadío (indicando en su
-- caso el sistema de riego)" — and one of them is not an irrigation system at
-- all. These eight describe how a particular irrigation was actually done. A
-- crop watered by "sprinkler" can be irrigated by a fixed installation one week
-- and a mobile one the next, which is precisely why the recorded
-- `crop.irrigation_code -> SIST_RIEGO` mapping gap was never closable on the
-- crop: the question is asked of the event, not of the plot.
--
-- Tier 1 (an owned English lookup mapped to the provider code at export)
-- rather than the catalogue code verbatim: eight values is a small closed
-- list, and "goteo is 6" is a fact about the Spanish registry, not about
-- irrigation.
CREATE TABLE irrigation_method (
    code     TEXT PRIMARY KEY,   -- 'drip', 'sprinkler_fixed', …
    i18n_key TEXT NOT NULL
);

-- Where the irrigation water came from (FEGA `ORIGEN_AGUA_RIEGO`, 6 values;
-- SIEX `Riego.OrigenAgua[]`). Optional, and an ARRAY in the twin — one
-- irrigation can mix a river and a borehole — so it hangs off a junction
-- rather than a column.
CREATE TABLE water_origin (
    code     TEXT PRIMARY KEY,   -- 'surface', 'groundwater', …
    i18n_key TEXT NOT NULL
);

-- Anexo III C.c's "tipo de tratamiento": enmienda, abonado de fondo, abonado
-- de cobertera. FEGA `TIPO_FERITILIZACION` (3 values, the provider's own
-- spelling of the id) and SIEX `AplicacionMaterialFertilizante.TipoFertilizacion`.
--
-- The printed model's footnote merges this with C.f below into one "(F)/(AF)/
-- (AC)" letter, which is why the book derives that letter at print time
-- instead of storing it: fertirrigación is NOT in this list, it is a way of
-- applying (C.f), and a farmer can perfectly well fertigate a cobertera.
CREATE TABLE fertilisation_type (
    code     TEXT PRIMARY KEY,   -- 'base_dressing', 'top_dressing', 'amendment'
    i18n_key TEXT NOT NULL
);

-- Anexo III C.f's "forma de aplicación … en particular si es por
-- fertirrigación, especificando si es por aspersión, localizada, etc."
-- FEGA `METODO_APLICACION_FERTILIZANTE` (7 values); SIEX
-- `AplicacionMaterialFertilizante.MetodoFertilizacion`.
CREATE TABLE application_method (
    code     TEXT PRIMARY KEY,   -- 'broadcast', 'fertigation_localised', …
    i18n_key TEXT NOT NULL,
    -- Stored rather than derived from the code, because it is exactly what the
    -- model's "(F)" box asks and what a future `Fertirrigacion` block would
    -- key on. Two of the seven are fertigation; a reader should not have to
    -- know which by reading the identifiers.
    is_fertigation INTEGER NOT NULL DEFAULT 0
);

-- What the manure received before it was applied (FEGA `TRAT_ESTIERCOLES`,
-- 9 values; SIEX `MaterialFertilizante.TratamientoEstiercoles`). A property of
-- the material, not of the application: the same batch of separated slurry is
-- the same batch every time it is spread.
CREATE TABLE manure_treatment (
    code     TEXT PRIMARY KEY,   -- 'none', 'composting', …
    i18n_key TEXT NOT NULL
);

-- Which of the three FEGA nutrient catalogues a composition figure belongs to.
-- Not a provider list itself — it SELECTS one (`MACRONUTRIENTES` 16 rows,
-- `MICRONUTRIENTES` 7, `METALES_PESADOS` 7), which is why it has no SIEX code
-- and why `fertiliser_material_nutrient` needs it: the integer 3 means
-- "N nítrico" in the first and "Plomo (Pb)" in the third.
CREATE TABLE nutrient_kind (
    code     TEXT PRIMARY KEY,   -- 'macro', 'micro', 'heavy_metal'
    i18n_key TEXT NOT NULL
);

-- ============================================================================
-- Section 8 — the irrigation register (RD 1051/2022 art. 5.e; Anexo III C.l)
-- ============================================================================

CREATE TABLE irrigation_record (
    id                      TEXT PRIMARY KEY,
    season_id               TEXT NOT NULL REFERENCES season(id),
    farm_id                 TEXT NOT NULL REFERENCES farm(id),

    -- C.a. An INTERVAL, like a treatment's: art. 5.f lets intensive and
    -- fertigated crops accumulate the record over fortnightly periods, and the
    -- twin requires FechaInicio + FechaFin. A single-day irrigation leaves the
    -- end NULL and a serializer sends the start as both ends.
    irrigated_on            TEXT NOT NULL,       -- 'YYYY-MM-DD'
    irrigation_end_date     TEXT,                -- 'YYYY-MM-DD', NULL = single day

    -- Model section 8's "Sistema de Riego"; required, as in the twin.
    irrigation_method_code  TEXT NOT NULL REFERENCES irrigation_method(code),

    -- C.l: "cantidad de agua aportada en cada riego (en m3 por hectárea)".
    -- Value + unit code, never free text. The unit is a real foreign key since
    -- `unit` moved into core (2026-08-07); the repository narrows it to the two
    -- codes that can answer this question, exactly as harvest_record does — the
    -- key says "a unit", the repository says "a unit a volume of water can be
    -- measured in".
    volume_value            REAL NOT NULL,
    volume_unit_code        TEXT NOT NULL REFERENCES unit(code),  -- 'm3_ha' | 'm3'

    -- The rest of C.l, and the one part of this register the printed model has
    -- no column for: the nitric nitrogen and water-soluble phosphorus already
    -- present IN the irrigation water, which count towards what the crop
    -- receives. Both nullable, and deliberately so: art. 17.2 requires them
    -- only when the organismo de cuenca, comunidad de regantes or equivalent
    -- supplies the figures, and makes them voluntary when the holder analyses
    -- the water themselves. Demanding them would turn a conditional duty into
    -- an unconditional one and invite invented numbers.
    water_nitric_n_mg_l     REAL,
    water_soluble_p2o5_mg_l REAL,

    -- Twin-only, both optional there (`Riego.TipoEnergia`, `NumContador`).
    -- Captured because they cost a column each and an un-parked export would
    -- otherwise have to ask the farmer again. `energy_type_code` is the FEGA
    -- TIPENERGIA code stored verbatim with no foreign key (the catalogue rule:
    -- the code is the payload, the catalogue row is display metadata, and a
    -- reimport must never cascade into records).
    energy_type_code        TEXT,
    meter_number            TEXT,

    notes                   TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    deleted_at              TEXT
);

CREATE INDEX idx_irrigation_record_season ON irrigation_record (season_id, farm_id);

-- C.b, and model section 8's "Id. parcelas" + "Superficie regada (ha)".
-- Recorded per unidad homogénea de cultivo, which is what a (plot, crop) pair
-- is here — the same unit `treatment_plot` and the SIEX DGC already use.
CREATE TABLE irrigation_plot (
    id                   TEXT PRIMARY KEY,
    irrigation_record_id TEXT NOT NULL REFERENCES irrigation_record(id) ON DELETE CASCADE,
    plot_id              TEXT NOT NULL REFERENCES plot(id),
    crop_id              TEXT REFERENCES crop(id),
    -- Nullable: the model prints the column, but a farmer who irrigated the
    -- whole plot has already said so by naming it, and an invented hectare
    -- figure is worse than a blank cell to fill in by hand.
    irrigated_area_ha    REAL,
    UNIQUE (irrigation_record_id, plot_id)
);

-- `Riego.OrigenAgua[]`. A junction because the twin is an array; unique per
-- (record, origin) so listing a source twice folds instead of erroring.
CREATE TABLE irrigation_water_origin (
    id                   TEXT PRIMARY KEY,
    irrigation_record_id TEXT NOT NULL REFERENCES irrigation_record(id) ON DELETE CASCADE,
    origin_code          TEXT NOT NULL REFERENCES water_origin(code),
    UNIQUE (irrigation_record_id, origin_code)
);

-- ============================================================================
-- Section 6 — the fertilisation register (RD 1051/2022 art. 5.d; Anexo III C)
-- ============================================================================

-- The reusable material registry, the `product` pattern and for the same
-- reason: a farmer applies one fertiliser many times in a campaign, and C.h
-- hangs EIGHT agronomic values off it. Retyping those per application is where
-- wrong data comes from, and the composition is a property of the sack, not of
-- the day it was spread.
--
-- Soft-deleted and audit-logged like `product`, so a record written years ago
-- still resolves the material it names even after the registry entry is
-- retired.
CREATE TABLE fertiliser_material (
    id                    TEXT PRIMARY KEY,

    -- What the label, the delivery note or the manure document calls it. Free
    -- text: SIEX carries it as `AplicacionMaterialFertilizante.NombreProducto`,
    -- a plain string beside the coded material, and a farmer naming a heap of
    -- their own manure has no registry entry to point at.
    name                  TEXT NOT NULL,

    -- C.d, first level: FEGA `MAT_FERTI` (24 values, from "Estiércol sólido de
    -- ovino" to "Lodos EDAR"). Stored verbatim without a foreign key — the
    -- catalogue rule: the code is the regulatory payload, the catalogue row is
    -- display metadata, and a snapshot refresh must never cascade into records.
    material_code         TEXT NOT NULL,
    -- C.d, second level: FEGA `DETALLE_MATERIAL_FERT` (1243 named products,
    -- each with its own published composition). Optional, because the first
    -- level alone answers C.d for manures and own-farm materials.
    material_detail_code  TEXT,

    -- C.e, required for the manure and residue cases (MAT_FERTI d.2–d.4): the
    -- supplying business, identified by exactly ONE registry number. The twin
    -- says "excluyente" three times in its own descriptions, so the CHECK is
    -- theirs, not ours — REGA for a livestock holding, NIF for a manure
    -- management centre, NIMA for a waste manager.
    supplier_name         TEXT,
    supplier_rega         TEXT,
    supplier_tax_id       TEXT,
    supplier_nima         TEXT,

    manure_treatment_code TEXT REFERENCES manure_treatment(code),

    -- SIEX `MaterialFertilizante.Densidad`, which the farmer needs to turn a
    -- litre dose into kilograms of nutrient. No unit column: the twin pairs it
    -- with `UnidadesMedida`, but a density is kg/L (= g/cm³) in every fertiliser
    -- label, so a unit column would vary over a set of one — and kg/L is code
    -- 12 of `UNIDADES_MEDIDA`, so a serializer can still state it.
    density_kg_l          REAL,

    notes                 TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    deleted_at            TEXT,

    -- A table-level constraint, so it must follow every column: SQLite ends the
    -- column list at the first table constraint.
    CHECK ((supplier_rega IS NOT NULL) + (supplier_tax_id IS NOT NULL)
           + (supplier_nima IS NOT NULL) <= 1)
);

-- C.h (eight agronomic values), C.i (heavy metals when sludge is applied) and
-- the micronutrients a label may declare, as ONE coded junction.
--
-- Not eight named columns: C.i has no home in a fixed column set, nor do the
-- seven micronutrients, and `MACRONUTRIENTES` itself carries sixteen entries —
-- the eight of C.h plus organic carbon, calcium, magnesium and sulphur. The
-- three SIEX arrays (`Macronutrientes`, `Micronutrientes`, `MetalesPesados`)
-- differ only in which catalogue their integer indexes, which is exactly what
-- `kind_code` records.
--
-- Codes verbatim, no foreign key (the catalogue rule). A pure child of the
-- material: it dies with it.
CREATE TABLE fertiliser_material_nutrient (
    id                     TEXT PRIMARY KEY,
    fertiliser_material_id TEXT NOT NULL REFERENCES fertiliser_material(id) ON DELETE CASCADE,
    kind_code              TEXT NOT NULL REFERENCES nutrient_kind(code),
    nutrient_code          TEXT NOT NULL,
    -- Percentage of the material, as the label states it (SIEX `Porcentaje`).
    percentage             REAL NOT NULL,
    UNIQUE (fertiliser_material_id, kind_code, nutrient_code)
);

-- One fertiliser application (or one accumulated period of them) over a set of
-- plots. SIEX twin: `Fertilizacion`.
CREATE TABLE fertilisation_record (
    id                      TEXT PRIMARY KEY,
    season_id               TEXT NOT NULL REFERENCES season(id),
    farm_id                 TEXT NOT NULL REFERENCES farm(id),

    -- C.a, an interval for the same reason irrigation's is: art. 5.f allows
    -- intensive and fertigated crops to accumulate the record over fortnightly
    -- periods, and the twin requires FechaInicio + FechaFin.
    applied_on              TEXT NOT NULL,       -- 'YYYY-MM-DD'
    application_end_date    TEXT,                -- 'YYYY-MM-DD', NULL = single day

    -- C.c and C.f — two separate legal fields that the printed model's
    -- "(F)/(AF)/(AC)" footnote merges into one letter. Both required, as in the
    -- twin.
    fertilisation_type_code TEXT NOT NULL REFERENCES fertilisation_type(code),
    application_method_code TEXT NOT NULL REFERENCES application_method(code),

    -- C.j: "cantidad del producto o material aplicado por hectárea". Value +
    -- unit code, never free text; the repository narrows the unit to the four
    -- rates a fertiliser dose can be stated in, exactly as irrigation narrows
    -- its volume.
    dose_value              REAL NOT NULL,
    dose_unit_code          TEXT NOT NULL REFERENCES unit(code),  -- kg_ha|l_ha|t_ha|m3_ha

    -- The material applied, by foreign key AND by snapshot — the Legal value
    -- capture rule. The snapshot holds only what section 6 PRINTS (the name and
    -- the model's "Riqueza N/P/K"); the full eight of C.h stay on the registry
    -- row, which is soft-deleted and therefore always resolvable. A second
    -- snapshot junction would duplicate the composition into every application.
    fertiliser_material_id  TEXT NOT NULL REFERENCES fertiliser_material(id),
    material_name_snapshot  TEXT NOT NULL,
    -- C.d's coded kind travels with the name because the model's own "Tipo de
    -- abono/producto" column prints it and C.d is a binding field: a record
    -- that named a manure must go on saying so even if the registry entry is
    -- later corrected to something else.
    material_code_snapshot  TEXT NOT NULL,
    richness_n_snapshot     REAL,
    richness_p2o5_snapshot  REAL,
    richness_k2o_snapshot   REAL,

    -- C.i / RD 1051/2022 art. 5.g, and required in the twin
    -- (`AplicacionMaterialFertilizante.AplicacionLodos`). Kept explicit rather
    -- than derived from `material_code = 22`: the decree asks the farmer to
    -- state it, and a derived answer would silently change if the catalogue
    -- were recoded.
    sludge_application      INTEGER NOT NULL DEFAULT 0,

    -- C.g, which says the machine is optional in so many words ("cuando
    -- proceda"). The twin agrees by omission: its `EquipoAplicador` block is
    -- not required, though when present it must carry a registration number,
    -- so an application with no machine simply omits the block.
    machinery_id            TEXT REFERENCES machinery(id),

    -- C.k: the service company, when the applicator is not the holding's own,
    -- with its REGFER registration number (RD 1051/2022 art. 18 — a THIRD
    -- machinery registry beside ROMA and REGANIP). The decree attaches the
    -- number to the company; the twin splits them, carrying `EmpresaServicios`
    -- on the application and `NumREGFER` inside the equipment block. Stored
    -- together here, because that is where the duty puts them.
    service_company         TEXT,
    service_regfer_number   TEXT,

    -- The printed model's own columns, which sección C does not ask for.
    delivery_note_ref       TEXT,                -- "Nº de albarán"
    yield_estimated_kg_ha   REAL,                -- "Producción estimada (kg/ha)"
    yield_final_kg_ha       REAL,                -- "Producción final (kg/ha)"

    notes                   TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    deleted_at              TEXT
);

CREATE INDEX idx_fertilisation_record_season ON fertilisation_record (season_id, farm_id);

-- C.b, and the model's "Referencia SIGPAC" + "Sup. (ha)" columns. Recorded per
-- unidad homogénea de cultivo — a (plot, crop) pair, the unit `treatment_plot`
-- and `irrigation_plot` already use and the one the SIEX DGC speaks.
CREATE TABLE fertilisation_plot (
    id                      TEXT PRIMARY KEY,
    fertilisation_record_id TEXT NOT NULL REFERENCES fertilisation_record(id) ON DELETE CASCADE,
    plot_id                 TEXT NOT NULL REFERENCES plot(id),
    crop_id                 TEXT REFERENCES crop(id),
    -- Nullable for irrigation's reason: naming the plot already says what was
    -- fertilised, and an invented hectare figure is worse than a blank cell.
    fertilised_area_ha      REAL,
    UNIQUE (fertilisation_record_id, plot_id)
);

-- `Fertilizacion.BuenasPracticas[]`, which the twin REQUIRES while the printed
-- model has no column for it and RD 1051/2022 puts good practices in its
-- anexo V rather than in the register's field list. So it is captured and never
-- demanded — the `seed_treatment.treatment_kind_code` rule: a book kept to the
-- printed model must not be blocked on a question the model never asks.
--
-- `BUENAS_PRACTICAS_AMBITOS` is keyed on (code, ámbito) and the same integer
-- means different things in each: 41 rows under "Fertilización", 31 under
-- "Riego", 26 under "Fitosanitario". The code is stored verbatim and THIS
-- TABLE fixes the ámbito.
CREATE TABLE fertilisation_practice (
    id                      TEXT PRIMARY KEY,
    fertilisation_record_id TEXT NOT NULL REFERENCES fertilisation_record(id) ON DELETE CASCADE,
    practice_code           TEXT NOT NULL,
    UNIQUE (fertilisation_record_id, practice_code)
);

-- ============================================================================
-- Section 7.1 — the plan de abonado (RD 1051/2022 art. 4.2, 5.a and 6)
-- ============================================================================

-- What the CUADERNO records about the plan — which is much less than the plan
-- itself, and the distinction is the whole design.
--
-- **Art. 6 defines a DOCUMENT**: it must identify every recinto of the
-- production unit, carry soil parameters, account for rainfall and available
-- irrigation, give the recommended dose of each nutrient with the moment, the
-- kind of material, the form of application and the machinery, and describe the
-- ammonia and greenhouse-gas measures of anexo V. That document is drawn up
-- (with advice, once art. 6.6's transition elapses) and KEPT.
--
-- **Art. 5.a defines the RECORD**: "rendimiento esperado, cultivo precedente,
-- necesidades de N, de P2O5 y de K2O y fecha de elaboración del plan", written
-- into the book at the start of the campaign. That is this table, and it is
-- exactly the SIEX `PlanAbonado` required set — the twin agreeing with the
-- article is the confirmation that the book carries the summary, not the plan.
--
-- Binding from **1 September 2026**, and from 1 January 2026 for irrigated
-- units sown or planted between 1 March and 30 June. Exempt: units of
-- unfertilised pasture only, and units of ≤10 ha that are rainfed or given over
-- to pasture or fodder for self-consumption.
CREATE TABLE fertilisation_plan (
    id                    TEXT PRIMARY KEY,
    season_id             TEXT NOT NULL REFERENCES season(id),
    farm_id               TEXT NOT NULL REFERENCES farm(id),

    -- Art. 5.a, in its own order. Units are fixed by the field and stated in
    -- the printed footnote: the needs are unidades fertilizantes, kg/ha of N,
    -- of P₂O₅ and of K₂O; the yield objective is kg/ha of produce.
    needs_n_kg_ha         REAL NOT NULL,
    needs_p2o5_kg_ha      REAL NOT NULL,
    needs_k2o_kg_ha       REAL NOT NULL,
    expected_yield_kg_ha  REAL NOT NULL,
    -- The crop that preceded this one, as a `PRODUCTOS` code stored verbatim
    -- and without a foreign key (the catalogue rule). Nullable because a unit
    -- coming out of fallow has no preceding crop to name, and inventing one
    -- would be a statement about a rotation that did not happen.
    preceding_crop_code   TEXT,
    -- "Fecha de elaboración del plan". Art. 6 lets a plan be adjusted during
    -- the campaign, so this moves when it is redrawn — which is why the plan
    -- is fully correctable rather than replaced.
    drawn_up_on           TEXT NOT NULL,       -- 'YYYY-MM-DD'

    -- Twin-only (`PlanAbonado.Herramienta`): whether a calculation tool
    -- produced the plan. Captured because it costs one column and the export
    -- requires it; the printed model has no box for it.
    tool_generated        INTEGER NOT NULL DEFAULT 0,

    notes                 TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    deleted_at            TEXT
);

CREATE INDEX idx_fertilisation_plan_season ON fertilisation_plan (season_id, farm_id);

-- The production unit the plan covers. A junction, not a column, because
-- `PlanAbonado.DGCs` is an ARRAY and a unidad de producción may well be several
-- plots carrying the same crop — art. 4.2 asks for a plan per unit, not per
-- parcel.
--
-- The repository keeps a crop in at most ONE live plan: two plans recommending
-- different nitrogen for the same crop would make section 7.1 print two
-- different figures for one row.
CREATE TABLE fertilisation_plan_crop (
    id                     TEXT PRIMARY KEY,
    fertilisation_plan_id  TEXT NOT NULL REFERENCES fertilisation_plan(id) ON DELETE CASCADE,
    crop_id                TEXT NOT NULL REFERENCES crop(id),
    UNIQUE (fertilisation_plan_id, crop_id)
);
