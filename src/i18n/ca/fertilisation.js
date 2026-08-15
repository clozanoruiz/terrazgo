// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.

export default {
  // Sistemes de reg per esdeveniment (cat\u00e0leg SIST_RIEGO, nota (1) del
  // model, secci\u00f3 8). NO \u00e9s la llista de la secci\u00f3 2.1, que
  // caracteritza la parcel\u00b7la.
  "irrigation_method.surface_gravity": "Superf\u00edcie o gravetat",
  "irrigation_method.sprinkler_fixed": "Aspersi\u00f3 fixa",
  "irrigation_method.sprinkler_mobile": "Aspersi\u00f3 m\u00f2bil",
  "irrigation_method.micro_sprinkler": "Microaspersi\u00f3",
  "irrigation_method.misting": "Nebulitzaci\u00f3",
  "irrigation_method.drip": "Degoteig",
  "irrigation_method.hydroponic_open": "Hidroponia a soluci\u00f3 perduda",
  "irrigation_method.hydroponic_recirculating": "Hidroponia amb recirculaci\u00f3",

  // Proced\u00e8ncia de l'aigua de reg (cat\u00e0leg ORIGEN_AGUA_RIEGO).
  "water_origin.surface": "Superficial",
  "water_origin.groundwater": "Subterr\u00e0nia",
  "water_origin.rainwater": "Pluvial",
  "water_origin.reclaimed": "Regeneraci\u00f3",
  "water_origin.desalinated": "Dessalinitzaci\u00f3",
  "water_origin.alternative": "Recursos alternatius",

  // Secció 8 — el registre de reg (RD 1051/2022 art. 5.e).
  "irrigation.title": "8. Registre de reg",
  "irrigation.intro":
    "Les dosis i les dates dels regs s'anoten en el termini d'un mes des de cada reg (RD 1051/2022, art. 5.e).",
  "irrigation.new": "Nou reg",
  "irrigation.irrigated_on": "Data de reg",
  "irrigation.end_date": "Data final",
  "irrigation.end_date_hint":
    "Només si el reg s'anota per períodes; deixeu-la en blanc per a un reg puntual.",
  "irrigation.method": "Sistema de reg",
  "irrigation.volume": "Volum de reg",
  "irrigation.volume_unit": "Unitat",
  "irrigation.volume_detail": "{volume} {unit}",
  "irrigation.meter_number": "Núm. de comptador",
  "irrigation.area": "Superfície regada (ha)",
  "irrigation.water_section": "Aigua de reg",
  "irrigation.water_hint":
    "El contingut en nitrogen i fòsfor de l'aigua només s'anota quan el facilita l'organisme de conca o la comunitat de regants; amb analítiques pròpies és voluntari.",
  "irrigation.nitric_n": "N nítric (mg/l)",
  "irrigation.soluble_p2o5": "P₂O₅ soluble (mg/l)",
  "irrigation.plots_section": "Parcel·les regades",
  "irrigation.delete_confirm": "Voleu suprimir aquest reg del registre?",

  // Tipus de fertilització (catàleg TIPO_FERITILIZACION, annex III C.c). La
  // fertirrigació NO hi és: és una forma d'aplicació (C.f).
  "fertilisation_type.base_dressing": "Adobat de fons",
  "fertilisation_type.top_dressing": "Adobat de cobertora",
  "fertilisation_type.amendment": "Aplicació d'esmena",

  // Forma d'aplicació (catàleg METODO_APLICACION_FERTILIZANTE, C.f).
  "application_method.broadcast": "Escampat general",
  "application_method.broadcast_buried": "Escampat general i enterrat",
  "application_method.banded": "Escampat localitzat",
  "application_method.banded_buried": "Escampat localitzat i enterrat",
  "application_method.fertigation_sprinkler": "Reg per aspersió (fertirrigació)",
  "application_method.fertigation_localised": "Reg localitzat (fertirrigació)",
  "application_method.foliar": "Aplicació foliar",

  // Tractament rebut pels fems (catàleg TRAT_ESTIERCOLES).
  "manure_treatment.none": "Cap",
  "manure_treatment.solid_fraction": "Separació sòlid-líquid: fracció sòlida",
  "manure_treatment.liquid_fraction": "Separació sòlid-líquid: fracció líquida",
  "manure_treatment.ndn_effluent": "Nitrificació-desnitrificació (NDN)",
  "manure_treatment.composting": "Compostatge",
  "manure_treatment.anaerobic_digestion": "Digestió anaeròbia",
  "manure_treatment.solar_drying": "Assecatge solar",
  "manure_treatment.stripping": "Stripping",
  "manure_treatment.membrane_separation": "Separació per membranes",

  // Quin catàleg indexa cada línia de composició.
  "nutrient_kind.macro": "Macronutrients",
  "nutrient_kind.micro": "Micronutrients",
  "nutrient_kind.heavy_metal": "Metalls pesants",

  // Registre de materials fertilitzants (el catàleg reutilitzable).
  "material.title": "Materials fertilitzants",
  "material.intro":
    "Els materials es registren un cop i es reutilitzen en cada aplicació: l'annex III C.h demana fins a vuit valors agronòmics per material.",
  "material.new": "Material nou",
  "material.name": "Nom comercial o del material",
  "material.kind": "Tipus de material",
  "material.detail": "Producte concret",
  "material.detail_hint":
    "Opcional; la llista s'acota al tipus triat (catàleg de productes fertilitzants).",
  "material.supplier_section": "Empresa subministradora",
  "material.supplier_hint":
    "Per a fems, purins i altres materials: indiqueu només un dels tres identificadors.",
  "material.supplier_name": "Nom de l'empresa",
  "material.supplier_rega": "REGA (explotació ramadera)",
  "material.supplier_tax_id": "NIF (centre de gestió de fems)",
  "material.supplier_nima": "NIMA (gestor de residus)",
  "material.manure_treatment": "Tractament rebut",
  "material.density": "Densitat (kg/l)",
  "material.composition_section": "Composició (% sobre el material)",
  "material.composition_hint":
    "N total, P₂O₅ total i K₂O són els que imprimeix el quadern; la resta de valors de l'annex III C.h i els metalls pesants es desen aquí.",
  "material.nutrient_kind": "Grup",
  "material.nutrient": "Nutrient",
  "material.percentage": "%",
  "material.add_nutrient": "Afegeix un nutrient",
  "material.fill": "Emplena des del catàleg",
  "material.fill_hint":
    "Pren la composició que el catàleg publica per al producte triat, sense tocar les línies que ja hàgiu anotat. Comproveu-la amb l'etiqueta: els metalls pesants no s'emplenen mai, perquè el catàleg barreja percentatges i mg/kg a les mateixes columnes.",
  "material.filled": "S'han afegit {count} línies de composició.",
  "material.filled_none": "El catàleg no afegeix res que no estigués ja anotat.",
  "material.richness_detail": "N {n} · P₂O₅ {p} · K₂O {k}",
  "material.supplier_registry": "Registre",
  "material.supplier_number": "Núm. d'identificació",
  "material.empty": "Encara no hi ha materials fertilitzants registrats.",
  "material.delete_confirm": "Voleu suprimir aquest material del registre?",

  // Secció 6 — el registre de fertilització.
  "fertilisation.title": "6. Registre de fertilització",
  "fertilisation.intro":
    "Les aplicacions de fertilitzants s'anoten en el termini d'un mes des de cada operació (RD 1051/2022, art. 5.d).",
  "fertilisation.new": "Fertilització nova",
  "fertilisation.no_materials":
    "Registreu abans un material fertilitzant al catàleg per poder anotar una aplicació.",
  "fertilisation.applied_on": "Data d'aplicació",
  "fertilisation.end_date": "Data final",
  "fertilisation.end_date_hint":
    "Només si l'aplicació s'anota per períodes; deixeu-la en blanc per a una aplicació puntual.",
  "fertilisation.material": "Material fertilitzant",
  "fertilisation.type": "Tipus de fertilització",
  "fertilisation.method": "Forma d'aplicació",
  "fertilisation.dose": "Dosi",
  "fertilisation.dose_unit": "Unitat",
  "fertilisation.dose_detail": "{dose} {unit}",
  "fertilisation.sludge": "Aplicació de llots de depuradora",
  "fertilisation.machinery": "Maquinària",
  "fertilisation.machinery_hint": "Opcional (annex III C.g).",
  "fertilisation.service_section": "Empresa de serveis",
  "fertilisation.service_hint":
    "Només quan l'aplicació la fa una empresa aliena a l'explotació (annex III C.k).",
  "fertilisation.service_company": "Nom de l'empresa",
  "fertilisation.service_regfer": "Núm. REGFER",
  "fertilisation.delivery_note": "Núm. d'albarà",
  "fertilisation.yield_estimated": "Producció estimada (kg/ha)",
  "fertilisation.yield_final": "Producció final (kg/ha)",
  "fertilisation.plots_section": "Parcel·les fertilitzades",
  "fertilisation.area": "Superfície fertilitzada (ha)",
  "fertilisation.practices_section": "Bones pràctiques",
  "fertilisation.practices_hint":
    "El model imprès no les recull; s'anoten perquè el quadern digital les demana.",
  "fertilisation.delete_confirm": "Voleu suprimir aquesta fertilització del registre?",
  "plan.title": "7.1 Pla d'adobat",
  "plan.intro":
    "El quadern anota el rendiment esperat, el cultiu precedent, les necessitats de N, P₂O₅ i K₂O i la data d'elaboració del pla (RD 1051/2022, art. 5.a). El pla mateix —parcel·les, sòl, aigua, dosis, maquinària i mesures de l'annex V— és un document a part que es conserva.",
  "plan.binding":
    "Obligatori des de l'1 de setembre de 2026; des de l'1 de gener de 2026 en unitats de regadiu sembrades o plantades entre l'1 de març i el 30 de juny.",
  "plan.new": "Pla nou",
  "plan.no_crops":
    "Registreu abans els cultius de la campanya: el pla es fa per unitat de producció.",
  "plan.crops": "Cultius de la unitat de producció",
  "plan.needs_section": "Necessitats (unitats fertilitzants, kg/ha)",
  "plan.needs_n": "N",
  "plan.needs_p2o5": "P₂O₅",
  "plan.needs_k2o": "K₂O",
  "plan.expected_yield": "Rendiment esperat (kg/ha)",
  "plan.preceding_crop": "Cultiu precedent",
  "plan.drawn_up_on": "Data d'elaboració",
  "plan.tool_generated": "Elaborat amb una eina de càlcul",
  "plan.needs_detail": "N {n} · P₂O₅ {p} · K₂O {k} UF/ha",
  "plan.yield_detail": "Rendiment esperat {yield} kg/ha",
  "plan.delete_confirm": "Voleu suprimir aquest pla d'adobat?",
};
