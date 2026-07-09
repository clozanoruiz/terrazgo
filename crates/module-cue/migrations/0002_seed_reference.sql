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

INSERT INTO alert_type (code, i18n_key) VALUES
    ('phi_window',     'alert_type.phi_window'),
    ('licence_expiry', 'alert_type.licence_expiry'),
    ('itv_expiry',     'alert_type.itv_expiry'),
    ('nitrate_zone',   'alert_type.nitrate_zone'),
    ('phyto_zone',     'alert_type.phyto_zone'),
    ('natura_zone',    'alert_type.natura_zone');
