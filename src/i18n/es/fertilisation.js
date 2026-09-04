// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  // Sistemas de riego por evento (catálogo SIST_RIEGO, nota (1) del modelo,
  // sección 8). NO es la lista de la sección 2.1, que caracteriza la parcela.
  "irrigation_method.surface_gravity": "Superficie o gravedad",
  "irrigation_method.sprinkler_fixed": "Aspersión fija",
  "irrigation_method.sprinkler_mobile": "Aspersión móvil",
  "irrigation_method.micro_sprinkler": "Microaspersión",
  "irrigation_method.misting": "Nebulización",
  "irrigation_method.drip": "Goteo",
  "irrigation_method.hydroponic_open": "Hidroponía a solución perdida",
  "irrigation_method.hydroponic_recirculating": "Hidroponía con recirculación",

  // Procedencia del agua de riego (catálogo ORIGEN_AGUA_RIEGO).
  "water_origin.surface": "Superficial",
  "water_origin.groundwater": "Subterránea",
  "water_origin.rainwater": "Pluvial",
  "water_origin.reclaimed": "Regeneración",
  "water_origin.desalinated": "Desalinización",
  "water_origin.alternative": "Recursos alternativos",

  // Sección 8 — el registro de riego (RD 1051/2022 art. 5.e).
  "irrigation.title": "8. Registro de riego",
  "irrigation.intro":
    "Las dosis y las fechas de los riegos se anotan en el plazo de un mes desde cada riego (RD 1051/2022, art. 5.e).",
  "irrigation.new": "Nuevo riego",
  "irrigation.irrigated_on": "Fecha de riego",
  "irrigation.end_date": "Fecha final",
  "irrigation.end_date_hint":
    "Solo si el riego se anota por periodos; déjela en blanco para un riego puntual.",
  "irrigation.method": "Sistema de riego",
  "irrigation.volume": "Volumen de riego",
  "irrigation.volume_unit": "Unidad",
  "irrigation.volume_detail": "{volume} {unit}",
  "irrigation.meter_number": "Nº de contador",
  "irrigation.area": "Superficie regada (ha)",
  "irrigation.water_section": "Agua de riego",
  "irrigation.water_hint":
    "El contenido en nitrógeno y fósforo del agua solo se anota cuando lo facilita el organismo de cuenca o la comunidad de regantes; con analíticas propias es voluntario.",
  "irrigation.nitric_n": "N nítrico (mg/l)",
  "irrigation.soluble_p2o5": "P₂O₅ soluble (mg/l)",
  "irrigation.plots_section": "Parcelas regadas",
  "irrigation.delete_confirm": "¿Eliminar este riego del registro?",

  // Tipo de fertilización (catálogo TIPO_FERITILIZACION, Anexo III C.c). La
  // fertirrigación NO está aquí: es una forma de aplicación (C.f).
  "fertilisation_type.base_dressing": "Abonado de fondo",
  "fertilisation_type.top_dressing": "Abonado de cobertera",
  "fertilisation_type.amendment": "Aplicación de enmienda",

  // Forma de aplicación (catálogo METODO_APLICACION_FERTILIZANTE, C.f).
  "application_method.broadcast": "Esparcido general",
  "application_method.broadcast_buried": "Esparcido general y enterrado",
  "application_method.banded": "Esparcido localizado",
  "application_method.banded_buried": "Esparcido localizado y enterrado",
  "application_method.fertigation_sprinkler": "Riego por aspersión (fertirrigación)",
  "application_method.fertigation_localised": "Riego localizado (fertirrigación)",
  "application_method.foliar": "Aplicación foliar",

  // Tratamiento recibido por el estiércol (catálogo TRAT_ESTIERCOLES).
  "manure_treatment.none": "Ninguno",
  "manure_treatment.solid_fraction": "Separación sólido-líquido: fracción sólida",
  "manure_treatment.liquid_fraction": "Separación sólido-líquido: fracción líquida",
  "manure_treatment.ndn_effluent": "Nitrificación-desnitrificación (NDN)",
  "manure_treatment.composting": "Compostaje",
  "manure_treatment.anaerobic_digestion": "Digestión anaerobia",
  "manure_treatment.solar_drying": "Secado solar",
  "manure_treatment.stripping": "Stripping",
  "manure_treatment.membrane_separation": "Separación por membranas",

  // Qué catálogo indexa cada línea de composición.
  "nutrient_kind.macro": "Macronutrientes",
  "nutrient_kind.micro": "Micronutrientes",
  "nutrient_kind.heavy_metal": "Metales pesados",

  // Registro de materiales fertilizantes (el catálogo reutilizable).
  "material.title": "Materiales fertilizantes",
  "material.intro":
    "Los materiales se registran una vez y se reutilizan en cada aplicación: el Anexo III C.h pide hasta ocho valores agronómicos por material.",
  "material.new": "Nuevo material",
  "material.name": "Nombre comercial o del material",
  "material.kind": "Tipo de material",
  "material.detail": "Producto concreto",
  "material.detail_hint":
    "Opcional; la lista se acota al tipo elegido (catálogo de productos fertilizantes).",
  "material.supplier_section": "Empresa suministradora",
  "material.supplier_hint":
    "Para estiércoles, purines y otros materiales: indique solo uno de los tres identificadores.",
  "material.supplier_name": "Nombre de la empresa",
  "material.supplier_rega": "REGA (explotación ganadera)",
  "material.supplier_tax_id": "NIF (centro de gestión de estiércoles)",
  "material.supplier_nima": "NIMA (gestor de residuos)",
  "material.manure_treatment": "Tratamiento recibido",
  "material.density": "Densidad (kg/l)",
  "material.composition_section": "Composición (% sobre el material)",
  "material.composition_hint":
    "N total, P₂O₅ total y K₂O son los que imprime el cuaderno; los demás valores del Anexo III C.h y los metales pesados se guardan aquí.",
  "material.nutrient_kind": "Grupo",
  "material.nutrient": "Nutriente",
  "material.percentage": "%",
  "material.add_nutrient": "Añadir nutriente",
  "material.fill": "Rellenar desde el catálogo",
  "material.fill_hint":
    "Toma la composición que el catálogo publica para el producto elegido, sin tocar las líneas que usted ya haya anotado. Compruébela con la etiqueta: los metales pesados no se rellenan nunca, porque el catálogo mezcla porcentajes y mg/kg en las mismas columnas.",
  "material.filled.one": "Se ha añadido una línea de composición.",
  "material.filled.other": "Se han añadido {count} líneas de composición.",
  "material.filled_none": "El catálogo no añade nada que no estuviera ya anotado.",
  "material.supplier_registry": "Registro",
  "material.supplier_number": "Nº de identificación",
  "material.empty": "Aún no hay materiales fertilizantes registrados.",
  "material.delete_confirm": "¿Eliminar este material del registro?",

  // Sección 6 — el registro de fertilización.
  "fertilisation.title": "6. Registro de fertilización",
  "fertilisation.intro":
    "Las aplicaciones de fertilizantes se anotan en el plazo de un mes desde cada operación (RD 1051/2022, art. 5.d).",
  "fertilisation.new": "Nueva fertilización",
  "fertilisation.no_materials":
    "Registre antes un material fertilizante en el catálogo para poder anotar una aplicación.",
  "fertilisation.applied_on": "Fecha de aplicación",
  "fertilisation.end_date": "Fecha final",
  "fertilisation.end_date_hint":
    "Solo si la aplicación se anota por periodos; déjela en blanco para una aplicación puntual.",
  "fertilisation.material": "Material fertilizante",
  "fertilisation.type": "Tipo de fertilización",
  "fertilisation.method": "Forma de aplicación",
  "fertilisation.dose": "Dosis",
  "fertilisation.dose_unit": "Unidad",
  "fertilisation.dose_detail": "{dose} {unit}",
  "fertilisation.sludge": "Aplicación de lodos de depuradora",
  "fertilisation.sustainable_inputs": "Gesti\u00f3n sostenible de insumos",
  "fertilisation.irrigation_link": "Riego con el que se fertirrig\u00f3",
  "fertilisation.irrigation_link_hint":
    "Enl\u00e1celo con el registro de riego de la pesta\u00f1a Riego: es el mismo acto, anotado en los dos registros que exige el real decreto.",
  "fertilisation.irrigation_link_none": "Sin enlazar",
  "fertilisation.machinery": "Maquinaria",
  "fertilisation.machinery_hint": "Opcional (Anexo III C.g).",
  "fertilisation.service_section": "Empresa de servicios",
  "fertilisation.service_hint":
    "Solo cuando la aplicación la realiza una empresa ajena a la explotación (Anexo III C.k).",
  "fertilisation.service_company": "Nombre de la empresa",
  "fertilisation.service_regfer": "Nº REGFER",
  "fertilisation.delivery_note": "Nº de albarán",
  "fertilisation.yield_estimated": "Producción estimada (kg/ha)",
  "fertilisation.yield_final": "Producción final (kg/ha)",
  "fertilisation.plots_section": "Parcelas fertilizadas",
  "fertilisation.area": "Superficie fertilizada (ha)",
  "fertilisation.practices_section": "Buenas prácticas",
  "fertilisation.practices_hint":
    "El modelo impreso no las recoge; se anotan porque el cuaderno digital las pide.",
  "fertilisation.practices_none": "Ninguna seleccionada",
  "fertilisation.practices_selected.one": "1 seleccionada",
  "fertilisation.practices_selected.other": "{count} seleccionadas",
  "fertilisation.delete_confirm": "¿Eliminar esta fertilización del registro?",
  "plan.title": "7.1 Plan de abonado",
  "plan.intro":
    "El cuaderno anota el rendimiento esperado, el cultivo precedente, las necesidades de N, P₂O₅ y K₂O y la fecha de elaboración del plan (RD 1051/2022, art. 5.a). El plan en sí —parcelas, suelo, agua, dosis, maquinaria y medidas del anexo V— es un documento aparte que se conserva.",
  "plan.binding":
    "Obligatorio desde el 1 de septiembre de 2026; desde el 1 de enero de 2026 en unidades de regadío sembradas o plantadas entre el 1 de marzo y el 30 de junio.",
  "plan.new": "Nuevo plan",
  "plan.no_crops":
    "Registre antes los cultivos de la campaña: el plan se hace por unidad de producción.",
  "plan.crops": "Cultivos de la unidad de producción",
  "plan.needs_section": "Necesidades (unidades fertilizantes, kg/ha)",
  "plan.needs_n": "N",
  "plan.needs_p2o5": "P₂O₅",
  "plan.needs_k2o": "K₂O",
  "plan.expected_yield": "Rendimiento esperado (kg/ha)",
  "plan.preceding_crop": "Cultivo precedente",
  "plan.drawn_up_on": "Fecha de elaboración",
  "plan.tool_generated": "Elaborado con una herramienta de cálculo",
  "plan.needs_detail": "N {n} · P₂O₅ {p} · K₂O {k} UF/ha",
  "plan.yield_detail": "Rendimiento esperado {yield} kg/ha",
  "plan.delete_confirm": "¿Eliminar este plan de abonado?",
};
