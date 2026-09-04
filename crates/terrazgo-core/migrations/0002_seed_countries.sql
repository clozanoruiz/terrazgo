-- Terrazgo core — migration 0002: seed core reference data.
-- Only stable codes + i18n keys; display labels live in the app's translation files.

INSERT INTO country (code, i18n_key) VALUES
    ('es', 'country.es'),
    ('fr', 'country.fr'),
    ('it', 'country.it');

-- Units of measure (moved here with the table from module-cue, 2026-08-07).
-- Rates first, then concentrations, then amounts; `dimension` is what keeps a
-- total from being offered where a dose belongs.
INSERT INTO unit (code, dimension, i18n_key) VALUES
    ('l_ha',  'dose_rate',     'unit.l_ha'),
    ('kg_ha', 'dose_rate',     'unit.kg_ha'),
    ('ml_ha', 'dose_rate',     'unit.ml_ha'),
    ('g_ha',  'dose_rate',     'unit.g_ha'),
    ('ml_hl', 'dose_rate',     'unit.ml_hl'),
    ('g_hl',  'dose_rate',     'unit.g_hl'),
    -- Rates that only a fertiliser or an irrigation is measured in
    -- (RD 1311/2012 Anexo III C.j and C.l). Perfectly good rates, and complete
    -- nonsense on a phytosanitary product — which is why the pickers are named
    -- lists in the repository rather than a `dimension` filter.
    ('m3_ha', 'dose_rate',     'unit.m3_ha'),
    ('t_ha',  'dose_rate',     'unit.t_ha'),
    ('g_l',   'concentration', 'unit.g_l'),
    ('ml_l',  'concentration', 'unit.ml_l'),
    ('pct',   'concentration', 'unit.pct'),
    -- Amounts, not rates: the total product used (Anexo III B.i is explicit
    -- about "kg o l"), and the tonnes / cubic metres the non-field registers
    -- measure their treated subject in.
    ('kg',    'quantity',      'unit.kg'),
    ('l',     'quantity',      'unit.l'),
    ('t',     'quantity',      'unit.t'),
    ('m3',    'quantity',      'unit.m3'),
    -- How MUCH of a non-chemical measure was deployed — the official model's
    -- "Intensidad de la medida (Nº de trampas, nº de difusores, etc.)". These
    -- are counts, not masses or volumes, which is why they are a dimension of
    -- their own: a number of traps can be neither a dose nor an amount of
    -- product. Each is offered absolute and per hectare because the SIEX
    -- UNIDADES_MEDIDA catalogue publishes both forms and they answer different
    -- questions (twelve traps in a plot, versus twelve traps for every
    -- hectare of it).
    ('traps',        'intensity', 'unit.traps'),
    ('traps_ha',     'intensity', 'unit.traps_ha'),
    ('diffusers',    'intensity', 'unit.diffusers'),
    ('diffusers_ha', 'intensity', 'unit.diffusers_ha'),
    ('units',        'intensity', 'unit.units'),
    ('units_ha',     'intensity', 'unit.units_ha');

INSERT INTO production_system (code, i18n_key) VALUES
    ('conventional', 'production_system.conventional'),
    ('organic',      'production_system.organic'),
    ('integrated',   'production_system.integrated');

-- RD 1311/2012 niveles de capacitación. 'pilot' is the aerial-application
-- carné the official model prints as a fourth column; "asesor" is deliberately
-- absent — advising is a capacity recorded against the advisor entity, not a
-- carné level an applicator holds.
INSERT INTO licence_level (code, i18n_key) VALUES
    ('basic',     'licence_level.basic'),
    ('qualified', 'licence_level.qualified'),
    ('fumigator', 'licence_level.fumigator'),
    ('pilot',     'licence_level.pilot');

-- Anexo III A.2.e; the official model's siglas (SEC/ASP/LOC/GRA) are Spanish
-- form vocabulary and live in the report template, not here.
INSERT INTO irrigation_system (code, i18n_key) VALUES
    ('rainfed',   'irrigation_system.rainfed'),
    ('sprinkler', 'irrigation_system.sprinkler'),
    ('drip',      'irrigation_system.drip'),
    ('gravity',   'irrigation_system.gravity');

-- What a `premises` row is, in core-native words. The register's own vocabulary
-- (storage_premises / transport) belongs to module-cue's
-- `non_field_subject_kind`; these two say what the THING is, which is core's
-- business, and the module pairs them.
INSERT INTO premises_kind (code, i18n_key) VALUES
    ('building', 'premises_kind.building'),
    ('vehicle',  'premises_kind.vehicle');

-- How a crop began. SIEX `SiembraPlantacion` codes the pair as 1 and 0.
INSERT INTO sowing_kind (code, i18n_key) VALUES
    ('sowing',   'sowing_kind.sowing'),
    ('planting', 'sowing_kind.planting');

-- Anexo III A.2.e; model siglas AL/M/BP/INV, likewise template-side.
INSERT INTO growing_environment (code, i18n_key) VALUES
    ('open_air',      'growing_environment.open_air'),
    ('mesh',          'growing_environment.mesh'),
    ('plastic_cover', 'growing_environment.plastic_cover'),
    ('greenhouse',    'growing_environment.greenhouse');

-- RD 1311/2012 art. 10-11: the GIP frameworks a holding can operate under.
-- Model siglas AE/PI/CP/Atrias/AS/NO, likewise template-side. 'not_required'
-- is the explicit "sin obligación de disponer de asesor", a real declaration.
INSERT INTO gip_system (code, i18n_key) VALUES
    ('organic',               'gip_system.organic'),
    ('integrated_production', 'gip_system.integrated_production'),
    ('private_certification', 'gip_system.private_certification'),
    ('atria',                 'gip_system.atria'),
    ('advisor_assisted',      'gip_system.advisor_assisted'),
    ('not_required',          'gip_system.not_required');

INSERT INTO zone_type (code, i18n_key) VALUES
    ('nitrate_vulnerable',        'zone_type.nitrate_vulnerable'),
    ('phytosanitary_restriction', 'zone_type.phytosanitary_restriction'),
    ('natura_2000',               'zone_type.natura_2000');
