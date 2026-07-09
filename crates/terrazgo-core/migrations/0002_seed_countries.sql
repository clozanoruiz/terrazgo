-- Terrazgo core — migration 0002: seed core reference data.
-- Only stable codes + i18n keys; display labels live in the app's translation files.

INSERT INTO country (code, i18n_key) VALUES
    ('es', 'country.es'),
    ('fr', 'country.fr'),
    ('it', 'country.it');

INSERT INTO production_system (code, i18n_key) VALUES
    ('conventional', 'production_system.conventional'),
    ('organic',      'production_system.organic'),
    ('integrated',   'production_system.integrated');

INSERT INTO licence_level (code, i18n_key) VALUES
    ('basic',     'licence_level.basic'),
    ('qualified', 'licence_level.qualified'),
    ('fumigator', 'licence_level.fumigator');

INSERT INTO zone_type (code, i18n_key) VALUES
    ('nitrate_vulnerable',        'zone_type.nitrate_vulnerable'),
    ('phytosanitary_restriction', 'zone_type.phytosanitary_restriction'),
    ('natura_2000',               'zone_type.natura_2000');
