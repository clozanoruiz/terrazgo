-- Terrazgo CUE module — migration 0002: seed reference / lookup data.
-- Only stable codes + i18n keys; display labels live in the app's translation files.
-- (country seed moved to the core's 0002_seed_countries.sql, 2026-06-12;
--  the unit seed followed the table there on 2026-08-07.)

-- The three subjects model sections 3.3, 3.4 and 3.5 register.
INSERT INTO non_field_subject_kind (code, i18n_key) VALUES
    ('postharvest',      'non_field_subject_kind.postharvest'),
    ('storage_premises', 'non_field_subject_kind.storage_premises'),
    ('transport',        'non_field_subject_kind.transport');

-- The conditional registers whose "APLICA TRATAMIENTO: NO" is stored rather
-- than derived. Seed treatment (3.2) joins the three above.
INSERT INTO register_kind (code, i18n_key) VALUES
    ('seed_treatment',   'register_kind.seed_treatment'),
    ('postharvest',      'register_kind.postharvest'),
    ('storage_premises', 'register_kind.storage_premises'),
    ('transport',        'register_kind.transport');

-- Where the treated seed was treated (FEGA TIPO_TRATAMIENTO, whose codes start
-- at 2 — ours are named, and module_cue::siex holds the mapping).
INSERT INTO seed_treatment_kind (code, i18n_key) VALUES
    ('on_farm',           'seed_treatment_kind.on_farm'),
    ('processing_centre', 'seed_treatment_kind.processing_centre'),
    ('purchased_es',      'seed_treatment_kind.purchased_es'),
    ('purchased_abroad',  'seed_treatment_kind.purchased_abroad');

-- What model section 4 calls "Material analizado". Four values, not the model's
-- three-word hint: FEGA separates the standing crop from the produce harvested
-- off it, and a book that conflated them would export the wrong one.
INSERT INTO analysis_material (code, i18n_key) VALUES
    ('crop',              'analysis_material.crop'),
    ('harvested_produce', 'analysis_material.harvested_produce'),
    ('soil',              'analysis_material.soil'),
    ('water',             'analysis_material.water');

-- What the laboratory looked for (FEGA TIPO_ANALISIS).
INSERT INTO analysis_type (code, i18n_key) VALUES
    ('pesticide_residues', 'analysis_type.pesticide_residues'),
    ('microbiological',    'analysis_type.microbiological'),
    ('heavy_metals',       'analysis_type.heavy_metals'),
    ('nutrients',          'analysis_type.nutrients'),
    ('soil_parameters',    'analysis_type.soil_parameters'),
    ('gmo_presence',       'analysis_type.gmo_presence');

INSERT INTO reason_category (code, i18n_key) VALUES
    ('pest',             'reason_category.pest'),
    ('disease',          'reason_category.disease'),
    ('weed',             'reason_category.weed'),
    ('growth_regulator', 'reason_category.growth_regulator'),
    ('other',            'reason_category.other');

INSERT INTO formulation_type (code, i18n_key) VALUES
    ('wp', 'formulation_type.wp'),
    ('sc', 'formulation_type.sc'),
    ('ec', 'formulation_type.ec'),
    ('wg', 'formulation_type.wg'),
    ('sl', 'formulation_type.sl');

INSERT INTO efficacy (code, i18n_key) VALUES
    ('good', 'efficacy.good'),
    ('fair', 'efficacy.fair'),
    ('poor', 'efficacy.poor');

INSERT INTO justification (code, i18n_key) VALUES
    ('threshold_exceeded',      'justification.threshold_exceeded'),
    ('monitoring',              'justification.monitoring'),
    ('decision_support_system', 'justification.decision_support_system'),
    ('authority_warning',       'justification.authority_warning'),
    ('advisor_recommendation',  'justification.advisor_recommendation'),
    ('alert_device',            'justification.alert_device');

INSERT INTO authorisation_kind (code, i18n_key) VALUES
    ('registered',      'authorisation_kind.registered'),
    ('common_name',     'authorisation_kind.common_name'),
    ('parallel_import', 'authorisation_kind.parallel_import'),
    ('exceptional',     'authorisation_kind.exceptional');

INSERT INTO alert_type (code, i18n_key) VALUES
    ('phi_window',     'alert_type.phi_window'),
    ('licence_expiry', 'alert_type.licence_expiry'),
    ('itv_expiry',     'alert_type.itv_expiry'),
    ('nitrate_zone',   'alert_type.nitrate_zone'),
    ('phyto_zone',     'alert_type.phyto_zone'),
    ('natura_zone',    'alert_type.natura_zone');
