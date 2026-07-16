-- Terrazgo CUE module — migration 0002: seed reference / lookup data.
-- Only stable codes + i18n keys; display labels live in the app's translation files.
-- (country seed moved to the core's 0002_seed_countries.sql, 2026-06-12.)

INSERT INTO unit (code, dimension, i18n_key) VALUES
    ('l_ha',  'dose_rate',     'unit.l_ha'),
    ('kg_ha', 'dose_rate',     'unit.kg_ha'),
    ('ml_ha', 'dose_rate',     'unit.ml_ha'),
    ('g_ha',  'dose_rate',     'unit.g_ha'),
    ('ml_hl', 'dose_rate',     'unit.ml_hl'),
    ('g_hl',  'dose_rate',     'unit.g_hl'),
    ('g_l',   'concentration', 'unit.g_l'),
    ('ml_l',  'concentration', 'unit.ml_l'),
    ('pct',   'concentration', 'unit.pct');

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
