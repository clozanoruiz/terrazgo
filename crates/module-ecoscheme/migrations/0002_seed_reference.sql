-- Terrazgo eco-scheme module — migration 0002: seed reference / lookup data.
-- Only stable codes + i18n keys; display labels live in the app's translation
-- files and in the record book's per-language `Labels`.

-- RD 1048/2022's six register-level annotation duties, in the order the printed
-- model's section 9 lists their pages — with the sixth, which has no page at
-- all, last. Each names the article that creates the duty, because the article
-- is what the register is derived from.
--
-- The eco-scheme numbers (P1, P2, …) are the aid regime's own naming and are
-- deliberately NOT the codes: they are a claim about the solicitud única, which
-- this app cannot see, whereas these codes name the activity being recorded.
INSERT INTO eco_practice (code, i18n_key) VALUES
    -- P1, art. 30.2 ter: grazing start/end dates, when they differ from those
    -- declared in the solicitud única. Model 9.1.
    ('extensive_grazing',    'eco_practice.extensive_grazing'),
    -- P2, arts. 31 and 31.4.d: "la fecha y las actividades realizadas" —
    -- pastoreo, siega for production or maintenance, or any other anexo III.B
    -- maintenance activity. Model 9.2.
    ('sustainable_mowing',   'eco_practice.sustainable_mowing'),
    -- P5, art. 45.2: the dates of nivelación, siembra, inundación, secas and
    -- construcción de caballones on flooded crops. Model 9.3 — which prints
    -- only three of those five.
    ('flooded_biodiversity', 'eco_practice.flooded_biodiversity'),
    -- P6, art. 42: a live cover, spontaneous or sown — its establishment date,
    -- its two widths and the maintenance performed on it, on three separate
    -- deadlines. Model 9.4.
    ('plant_cover',          'eco_practice.plant_cover'),
    -- P7, art. 43: an inert cover of triturated pruning residue, established no
    -- later than 15 April, plus the same two widths. Model 9.5.
    ('inert_cover',          'eco_practice.inert_cover'),
    -- Anexo IV: the dates of maintenance activities on each pasto comunal
    -- plot, with the invoices kept as evidence. **No printed page exists** —
    -- the book gives it one (docs/cuaderno-print.md → section map).
    ('communal_pasture',     'eco_practice.communal_pasture');

-- FEGA `TIPO_LABOR`, codes 0–13, in the catalogue's own order — so a selector
-- reads the way the provider's list does. The English codes are ours;
-- `siex.rs` maps them to the provider integers and a contract test pins the
-- map in both directions.
--
-- Fifteen codes onto fourteen catalogue rows: `mowing` and `brush_cutting`
-- both answer to `TIPO_LABOR` 5 ("Desbroce y siega"), because model 9.4 prints
-- Siega and Desbrozado as two columns and a farmer recording one has not
-- recorded the other. Splitting is safe in the direction that matters — both
-- export as 5 — while merging would lose a distinction the printed form asks
-- for.
INSERT INTO cultural_operation_kind (code, i18n_key) VALUES
    ('no_tillage',      'cultural_operation_kind.no_tillage'),       -- 0 Sin laboreo
    ('tillage',         'cultural_operation_kind.tillage'),          -- 1 Laboreo
    ('levelling',       'cultural_operation_kind.levelling'),        -- 2 Nivelación (cultivos bajo agua)
    ('ridging',         'cultural_operation_kind.ridging'),          -- 3 Caballones y tablas (bajo agua)
    ('weeding',         'cultural_operation_kind.weeding'),          -- 4 Escarda
    ('mowing',          'cultural_operation_kind.mowing'),           -- 5 Desbroce y siega
    ('brush_cutting',   'cultural_operation_kind.brush_cutting'),    -- 5 Desbroce y siega
    ('drainage',        'cultural_operation_kind.drainage'),         -- 6 Mantenimiento del drenaje
    ('pruning',         'cultural_operation_kind.pruning'),          -- 7 Poda
    ('thinning',        'cultural_operation_kind.thinning'),         -- 8 Aclareo
    ('staking',         'cultural_operation_kind.staking'),          -- 9 Entutorado
    ('grafting',        'cultural_operation_kind.grafting'),         -- 10 Injerto
    ('pruning_removal', 'cultural_operation_kind.pruning_removal'),  -- 11 Eliminación de restos de poda
    ('green_pruning',   'cultural_operation_kind.green_pruning'),    -- 12 Poda en verde
    ('rolling',         'cultural_operation_kind.rolling');          -- 13 Rulado
