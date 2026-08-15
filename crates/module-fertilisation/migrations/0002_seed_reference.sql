-- Terrazgo fertilisation module — migration 0002: seed reference / lookup data.
-- Only stable codes + i18n keys; display labels live in the app's translation
-- files and in the record book's per-language `Labels`.

-- FEGA `SIST_RIEGO`, and the model's own section 8 footnote, which lists the
-- same eight in the same order. Rowid order is that order, so a selector reads
-- the way the printed form does. The English codes are ours; `siex.rs` maps
-- them to the provider integers and a contract test pins the map both ways.
INSERT INTO irrigation_method (code, i18n_key) VALUES
    ('surface_gravity',          'irrigation_method.surface_gravity'),
    ('sprinkler_fixed',          'irrigation_method.sprinkler_fixed'),
    ('sprinkler_mobile',         'irrigation_method.sprinkler_mobile'),
    ('micro_sprinkler',          'irrigation_method.micro_sprinkler'),
    ('misting',                  'irrigation_method.misting'),
    ('drip',                     'irrigation_method.drip'),
    ('hydroponic_open',          'irrigation_method.hydroponic_open'),
    ('hydroponic_recirculating', 'irrigation_method.hydroponic_recirculating');

-- FEGA `ORIGEN_AGUA_RIEGO`. 'alternative' is the catalogue's own residual
-- category ("recursos alternativos distintos de la regeneración y
-- desalinización"), not an "other" we invented.
INSERT INTO water_origin (code, i18n_key) VALUES
    ('surface',       'water_origin.surface'),
    ('groundwater',   'water_origin.groundwater'),
    ('rainwater',     'water_origin.rainwater'),
    ('reclaimed',     'water_origin.reclaimed'),
    ('desalinated',   'water_origin.desalinated'),
    ('alternative',   'water_origin.alternative');

-- FEGA `TIPO_FERITILIZACION`, in the catalogue's own order (1 fondo,
-- 2 cobertera, 3 enmienda) — which is also the order the model's footnote
-- lists AF and AC in.
INSERT INTO fertilisation_type (code, i18n_key) VALUES
    ('base_dressing', 'fertilisation_type.base_dressing'),
    ('top_dressing',  'fertilisation_type.top_dressing'),
    ('amendment',     'fertilisation_type.amendment');

-- FEGA `METODO_APLICACION_FERTILIZANTE`. The two fertigation entries are the
-- catalogue's own 5 ("Riego por aspersión (fertirrigación)") and 6 ("Riego
-- localizado (fertirrigación)") — C.f asks for exactly that distinction.
INSERT INTO application_method (code, i18n_key, is_fertigation) VALUES
    ('broadcast',             'application_method.broadcast',             0),
    ('broadcast_buried',      'application_method.broadcast_buried',      0),
    ('banded',                'application_method.banded',                0),
    ('banded_buried',         'application_method.banded_buried',         0),
    ('fertigation_sprinkler', 'application_method.fertigation_sprinkler', 1),
    ('fertigation_localised', 'application_method.fertigation_localised', 1),
    ('foliar',                'application_method.foliar',                0);

-- FEGA `TRAT_ESTIERCOLES`. 'none' is the catalogue's own first entry
-- ("Ninguno"), not an absence we invented — a farmer stating that the manure
-- was applied untreated is making a claim, and C.d's fourth level asks for it.
INSERT INTO manure_treatment (code, i18n_key) VALUES
    ('none',                'manure_treatment.none'),
    ('solid_fraction',      'manure_treatment.solid_fraction'),
    ('liquid_fraction',     'manure_treatment.liquid_fraction'),
    ('ndn_effluent',        'manure_treatment.ndn_effluent'),
    ('composting',          'manure_treatment.composting'),
    ('anaerobic_digestion', 'manure_treatment.anaerobic_digestion'),
    ('solar_drying',        'manure_treatment.solar_drying'),
    ('stripping',           'manure_treatment.stripping'),
    ('membrane_separation', 'manure_treatment.membrane_separation');

-- Which nutrient catalogue a composition figure indexes. Ours, not FEGA's:
-- these three name the three arrays of `MaterialFertilizante`, so there is no
-- provider code to map and `siex.rs` gives each one its catalogue id instead.
INSERT INTO nutrient_kind (code, i18n_key) VALUES
    ('macro',       'nutrient_kind.macro'),
    ('micro',       'nutrient_kind.micro'),
    ('heavy_metal', 'nutrient_kind.heavy_metal');
