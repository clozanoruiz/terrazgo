// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.

export default {
  // Per-event irrigation systems (SIST_RIEGO catalogue, the model's own
  // section 8 footnote). NOT the section 2.1 list, which describes the plot.
  "irrigation_method.surface_gravity": "Surface or gravity",
  "irrigation_method.sprinkler_fixed": "Fixed sprinkler",
  "irrigation_method.sprinkler_mobile": "Mobile sprinkler",
  "irrigation_method.micro_sprinkler": "Micro-sprinkler",
  "irrigation_method.misting": "Misting",
  "irrigation_method.drip": "Drip",
  "irrigation_method.hydroponic_open": "Open hydroponics",
  "irrigation_method.hydroponic_recirculating": "Recirculating hydroponics",

  // Irrigation water source (ORIGEN_AGUA_RIEGO catalogue).
  "water_origin.surface": "Surface water",
  "water_origin.groundwater": "Groundwater",
  "water_origin.rainwater": "Rainwater",
  "water_origin.reclaimed": "Reclaimed water",
  "water_origin.desalinated": "Desalinated water",
  "water_origin.alternative": "Alternative resources",

  // Section 8 — the irrigation register (RD 1051/2022 art. 5.e).
  "irrigation.title": "8. Irrigation register",
  "irrigation.intro":
    "Irrigation doses and dates are recorded within one month of each irrigation (RD 1051/2022, art. 5.e).",
  "irrigation.new": "New irrigation",
  "irrigation.irrigated_on": "Irrigation date",
  "irrigation.end_date": "End date",
  "irrigation.end_date_hint":
    "Only if irrigation is recorded by periods; leave blank for a single irrigation.",
  "irrigation.method": "Irrigation system",
  "irrigation.volume": "Irrigation volume",
  "irrigation.volume_unit": "Unit",
  "irrigation.volume_detail": "{volume} {unit}",
  "irrigation.meter_number": "Meter number",
  "irrigation.area": "Irrigated area (ha)",
  "irrigation.water_section": "Irrigation water",
  "irrigation.water_hint":
    "The water's nitrogen and phosphorus content is recorded only when the basin authority or irrigators' community supplies it; from your own analyses it is voluntary.",
  "irrigation.nitric_n": "Nitric N (mg/l)",
  "irrigation.soluble_p2o5": "Soluble P₂O₅ (mg/l)",
  "irrigation.plots_section": "Irrigated plots",
  "irrigation.delete_confirm": "Delete this irrigation from the register?",

  // Fertilisation type (catalogue TIPO_FERITILIZACION, Anexo III C.c).
  // Fertigation is NOT here: it is a way of applying (C.f).
  "fertilisation_type.base_dressing": "Base dressing",
  "fertilisation_type.top_dressing": "Top dressing",
  "fertilisation_type.amendment": "Soil amendment",

  // Application method (catalogue METODO_APLICACION_FERTILIZANTE, C.f).
  "application_method.broadcast": "Broadcast",
  "application_method.broadcast_buried": "Broadcast and incorporated",
  "application_method.banded": "Banded",
  "application_method.banded_buried": "Banded and incorporated",
  "application_method.fertigation_sprinkler": "Sprinkler irrigation (fertigation)",
  "application_method.fertigation_localised": "Localised irrigation (fertigation)",
  "application_method.foliar": "Foliar application",

  // Treatment the manure received (catalogue TRAT_ESTIERCOLES).
  "manure_treatment.none": "None",
  "manure_treatment.solid_fraction": "Solid-liquid separation: solid fraction",
  "manure_treatment.liquid_fraction": "Solid-liquid separation: liquid fraction",
  "manure_treatment.ndn_effluent": "Nitrification-denitrification (NDN)",
  "manure_treatment.composting": "Composting",
  "manure_treatment.anaerobic_digestion": "Anaerobic digestion",
  "manure_treatment.solar_drying": "Solar drying",
  "manure_treatment.stripping": "Stripping",
  "manure_treatment.membrane_separation": "Membrane separation",

  // Which catalogue each composition line indexes.
  "nutrient_kind.macro": "Macronutrients",
  "nutrient_kind.micro": "Micronutrients",
  "nutrient_kind.heavy_metal": "Heavy metals",

  // The reusable fertiliser material registry.
  "material.title": "Fertiliser materials",
  "material.intro":
    "A material is registered once and reused by every application: Anexo III C.h asks for up to eight agronomic values per material.",
  "material.new": "New material",
  "material.name": "Commercial or material name",
  "material.kind": "Kind of material",
  "material.detail": "Specific product",
  "material.detail_hint":
    "Optional; the list narrows to the chosen kind (fertiliser product catalogue).",
  "material.supplier_section": "Supplying business",
  "material.supplier_hint":
    "For manures, slurries and other materials: state only one of the three identifiers.",
  "material.supplier_name": "Business name",
  "material.supplier_rega": "REGA (livestock holding)",
  "material.supplier_tax_id": "Tax id (manure management centre)",
  "material.supplier_nima": "NIMA (waste manager)",
  "material.manure_treatment": "Treatment received",
  "material.density": "Density (kg/l)",
  "material.composition_section": "Composition (% of the material)",
  "material.composition_hint":
    "Total N, total P₂O₅ and K₂O are what the record book prints; the rest of Anexo III C.h and the heavy metals are kept here.",
  "material.nutrient_kind": "Group",
  "material.nutrient": "Nutrient",
  "material.percentage": "%",
  "material.add_nutrient": "Add nutrient",
  "material.fill": "Fill from the catalogue",
  "material.fill_hint":
    "Takes the composition the catalogue publishes for the chosen product, leaving any line you already entered untouched. Check it against the label: heavy metals are never filled in, because the catalogue mixes percentages and mg/kg in the same columns.",
  "material.filled": "{count} composition lines added.",
  "material.filled_none": "The catalogue adds nothing that was not already recorded.",
  "material.richness_detail": "N {n} · P₂O₅ {p} · K₂O {k}",
  "material.supplier_registry": "Registry",
  "material.supplier_number": "Identification no.",
  "material.empty": "No fertiliser materials registered yet.",
  "material.delete_confirm": "Delete this material from the registry?",

  // Section 6 — the fertilisation register.
  "fertilisation.title": "6. Fertilisation register",
  "fertilisation.intro":
    "Fertiliser applications are recorded within one month of each operation (RD 1051/2022, art. 5.d).",
  "fertilisation.new": "New fertilisation",
  "fertilisation.no_materials":
    "Register a fertiliser material in the catalogue first, so an application can name it.",
  "fertilisation.applied_on": "Application date",
  "fertilisation.end_date": "End date",
  "fertilisation.end_date_hint":
    "Only when the application is recorded over a period; leave blank for a single day.",
  "fertilisation.material": "Fertiliser material",
  "fertilisation.type": "Fertilisation type",
  "fertilisation.method": "Application method",
  "fertilisation.dose": "Dose",
  "fertilisation.dose_unit": "Unit",
  "fertilisation.dose_detail": "{dose} {unit}",
  "fertilisation.sludge": "Sewage sludge applied",
  "fertilisation.machinery": "Machinery",
  "fertilisation.machinery_hint": "Optional (Anexo III C.g).",
  "fertilisation.service_section": "Service company",
  "fertilisation.service_hint":
    "Only when the application is carried out by a business other than the holding (Anexo III C.k).",
  "fertilisation.service_company": "Business name",
  "fertilisation.service_regfer": "REGFER no.",
  "fertilisation.delivery_note": "Delivery note no.",
  "fertilisation.yield_estimated": "Estimated yield (kg/ha)",
  "fertilisation.yield_final": "Final yield (kg/ha)",
  "fertilisation.plots_section": "Fertilised plots",
  "fertilisation.area": "Fertilised area (ha)",
  "fertilisation.practices_section": "Good practices",
  "fertilisation.practices_hint":
    "The printed model has no column for them; they are recorded because the digital record book asks for them.",
  "fertilisation.delete_confirm": "Delete this fertilisation from the register?",
  "plan.title": "7.1 Fertilisation plan",
  "plan.intro":
    "The record book carries the expected yield, the preceding crop, the N, P₂O₅ and K₂O requirements and the date the plan was drawn up (RD 1051/2022, art. 5.a). The plan itself — parcels, soil, water, doses, machinery and the anexo V measures — is a separate document to be kept.",
  "plan.binding":
    "Required from 1 September 2026; from 1 January 2026 for irrigated units sown or planted between 1 March and 30 June.",
  "plan.new": "New plan",
  "plan.no_crops": "Record the campaign's crops first: a plan is drawn up per production unit.",
  "plan.crops": "Crops of the production unit",
  "plan.needs_section": "Requirements (fertiliser units, kg/ha)",
  "plan.needs_n": "N",
  "plan.needs_p2o5": "P₂O₅",
  "plan.needs_k2o": "K₂O",
  "plan.expected_yield": "Expected yield (kg/ha)",
  "plan.preceding_crop": "Preceding crop",
  "plan.drawn_up_on": "Date drawn up",
  "plan.tool_generated": "Produced with a calculation tool",
  "plan.needs_detail": "N {n} · P₂O₅ {p} · K₂O {k} UF/ha",
  "plan.yield_detail": "Expected yield {yield} kg/ha",
  "plan.delete_confirm": "Delete this fertilisation plan?",
};
