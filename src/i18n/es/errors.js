// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  // Errores del límite de comandos (códigos de CommandError → error.<code>).
  // "internal" no tiene entrada error.<code> a propósito: se muestra el mensaje
  // técnico, precedido por internal_intro para orientar al usuario normal.
  "error.internal_intro": "Se ha producido un error interno:",
  "error.not_found": "El registro no existe.",
  "error.invalid.empty_name": "El nombre no puede estar vacío.",
  "error.invalid.operator_not_found": "El operador seleccionado ya no existe.",
  "error.invalid.empty_authorisation_number": "El número de registro no puede estar vacío.",
  "error.invalid.no_problems": "Indica al menos una problemática (plaga, enfermedad…).",
  "error.invalid.no_justifications": "Indica al menos una justificación de la actuación.",
  "error.invalid.end_date_before_start": "La fecha de fin no puede ser anterior a la de inicio.",
  "error.invalid.application_time": "Indica la hora en formato HH:MM (por ejemplo 20:30).",
  "error.invalid.growth_stage_unknown":
    "El estado fenológico seleccionado no está en el catálogo oficial.",
  "error.invalid.invalid_total_quantity":
    "Indica la cantidad total y su unidad (kg o l), con un valor mayor que cero.",
  "error.invalid.unknown_problem_code":
    "La problemática seleccionada no está en el catálogo oficial.",
  "error.invalid.export_precheck_failed":
    "La exportación está bloqueada: faltan datos que exige el formato oficial.",
  "error.invalid.export_code_unmappable":
    "Un código guardado no se pudo convertir a los catálogos oficiales.",
  "error.invalid.missing_exceptional_substance":
    "Una autorización excepcional debe indicar su sustancia (código del catálogo oficial).",
  "error.invalid.unknown_substance_code":
    "La sustancia indicada no está en el catálogo oficial de autorizaciones excepcionales.",
  "error.invalid.nonpositive_area": "La superficie debe ser mayor que cero.",
  "error.invalid.season_in_use":
    "No se puede eliminar una campaña con cultivos o tratamientos registrados. Elimina antes su contenido.",
  "error.invalid.missing_distance":
    "Indica la distancia a la parcela: es obligatoria cuando la captación queda fuera de ella.",
  "error.invalid.water_point_distance_inside":
    "Una captación incluida en la parcela no lleva distancia. Desmarca «incluida» o borra la distancia.",
  "error.invalid.water_point_coordinates_invalid":
    "Las coordenadas deben indicarse completas (latitud y longitud) y dentro de los límites del globo.",
  "error.invalid.plot_has_water_points":
    "Esta parcela tiene captaciones registradas, así que no puede declararse sin ellas. Elimínalas antes.",
  "error.invalid.report_language_unknown": "Ese idioma no está disponible para el cuaderno.",
  "error.invalid.cache_cap_too_small":
    "El espacio para mapas sin conexión es demasiado pequeño (mínimo 64 MB).",
  "error.invalid_date": "Fecha no válida «{date}» (se espera AAAA-MM-DD).",
  "error.authorisation_missing": "El producto {product_id} no está autorizado en «{country}».",
  "error.country_mismatch": "El país «{provided}» no coincide con el de la explotación («{farm}»).",
  "error.plot_not_on_farm": "La parcela {plot_id} no pertenece a la explotación {farm_id}.",
  "error.invalid.backup_invalid":
    "El archivo seleccionado no es una copia de seguridad válida de Terrazgo.",
  "error.invalid.backup_newer_schema":
    "Esta copia se creó con una versión más reciente de Terrazgo; actualice la aplicación primero.",
  "error.missing_phi_days":
    "No hay plazo de seguridad disponible: el producto no tiene valor por defecto y no se indicó ninguno.",
  "error.geo_http": "El servicio de mapas respondió con un error (HTTP {status}).",
  "error.geo_offline": "Sin conexión — se muestran solo los datos de mapa en caché. ({reason})",
  "error.invalid.geometry_invalid":
    "La geometría no es un contorno válido (un polígono cerrado con coordenadas válidas).",
  "error.invalid.geo_subject_missing":
    "La geometría no está asociada a una parcela ni a una explotación.",
  "error.invalid.geo_subject_ambiguous":
    "La geometría no puede pertenecer a dos elementos a la vez.",
  "error.invalid.boundary_file_unsupported":
    "Archivo no compatible — usa GeoJSON o GeoPackage (.gpkg).",
  "error.invalid.boundary_file_empty": "El archivo no contiene contornos utilizables (polígonos).",
  "error.invalid.boundary_file_too_large":
    "El archivo tiene demasiados elementos — usa un extracto menor (p. ej., un municipio).",
  "error.invalid.gpkg_unsupported_srs":
    "El GeoPackage usa un sistema de coordenadas proyectado que esta versión aún no puede leer.",
  "error.invalid.tilejson_invalid":
    "El servicio de mapas devolvió un índice de teselas inservible.",
  "error.invalid.style_unsupported":
    "El estilo del mapa base cambió en el servicio de una forma que Terrazgo aún no reconoce.",
  "error.invalid.sigpac_ref_invalid":
    "La referencia SIGPAC está incompleta o no es numérica — revisa las siete partes.",
  "error.invalid.sigpac_response_invalid": "SIGPAC respondió en un formato inesperado.",
  "error.invalid.sigpac_ref_missing":
    "La parcela no tiene una referencia SIGPAC completa — rellena antes las siete partes.",
  "error.invalid.zone_status_invalid":
    "El resultado interno de la comprobación de zonas no era utilizable.",
  "error.invalid.quantity_unit_mismatch":
    "La unidad no corresponde a lo tratado: toneladas para producto vegetal, m\u00b3 para locales y veh\u00edculos.",
  "error.invalid.invalid_product_quantity":
    "Indique la cantidad de producto y su unidad (kg o l), con un valor mayor que cero.",
  "error.invalid.empty_subject": "Indique qu\u00e9 se ha tratado.",
  "error.invalid.unknown_subject_kind": "Tipo de registro desconocido.",
  "error.invalid.register_has_rows":
    "No puede declararse sin tratamientos: el registro ya tiene anotaciones.",
  "error.invalid.empty_product_name": "Indique el nombre del producto de la etiqueta.",
  "error.invalid.no_plots": "Indique al menos una parcela.",
  "error.invalid.invalid_seed_quantity": "La cantidad de semilla debe ser mayor que cero.",
  "error.invalid.unknown_seed_treatment_kind": "Elija un tratamiento de la lista.",
  "error.invalid.unknown_analysis_material": "Elija el material analizado.",
  "error.invalid.unknown_analysis_type": "Elija un tipo de an\u00e1lisis de la lista.",
  "error.invalid.empty_buyer_name": "Indique el nombre o la raz\u00f3n social del cliente.",

  // Sección 4 — parámetros del suelo (Anexo III A.3).
  "error.invalid.invalid_soil_ph": "El pH está entre 0 y 14.",
  "error.invalid.invalid_soil_percentage": "Los porcentajes están entre 0 y 100.",
  "error.invalid.invalid_soil_value": "El valor no puede ser negativo.",
  "error.invalid.invalid_soil_texture":
    "Arena, limo y arcilla son fracciones del mismo suelo: deben sumar 100 %.",
  "error.invalid.invalid_harvest_quantity":
    "Indique la cantidad y su unidad (kg o t), o deje ambas en blanco.",
  "error.invalid.plot_not_on_farm": "La parcela elegida no pertenece a esta explotaci\u00f3n.",

  // Sección 8 — el registro de riego (RD 1051/2022 art. 5.e).
  "error.invalid.invalid_date_interval": "La fecha final no puede ser anterior a la inicial.",
  "error.invalid.invalid_irrigation_volume": "Indique un volumen de riego mayor que cero.",
  "error.invalid.invalid_volume_unit": "El volumen de riego se mide en m\u00b3/ha o en m\u00b3.",
  "error.invalid.invalid_water_quality": "El contenido en el agua de riego no puede ser negativo.",
  "error.invalid.unknown_irrigation_method": "Elija un sistema de riego de la lista.",
  "error.invalid.unknown_water_origin": "Elija una procedencia del agua de la lista.",

  // Sección 6 — el registro de fertilización (RD 1051/2022 art. 5.d).
  "error.invalid.empty_material_code": "Elija el tipo de material fertilizante.",
  "error.invalid.unknown_material_code": "Elija un tipo de material de la lista.",
  "error.invalid.unknown_manure_treatment": "Elija un tratamiento del estiércol de la lista.",
  "error.invalid.unknown_nutrient_kind": "Elija macronutriente, micronutriente o metal pesado.",
  "error.invalid.empty_nutrient_code": "Elija el nutriente de la lista.",
  "error.invalid.invalid_percentage": "La riqueza debe estar entre 0 y 100 %.",
  "error.invalid.supplier_id_conflict":
    "Indique solo uno de los tres: REGA, NIF o NIMA de la empresa suministradora.",
  "error.invalid.invalid_density": "La densidad debe ser mayor que cero.",
  "error.invalid.invalid_dose": "Indique una dosis mayor que cero.",
  "error.invalid.invalid_dose_unit": "La dosis de fertilizante se mide por hectárea.",
  "error.invalid.invalid_yield": "La producción no puede ser negativa.",
  "error.invalid.unknown_fertilisation_type": "Elija un tipo de fertilización de la lista.",
  "error.invalid.unknown_application_method": "Elija una forma de aplicación de la lista.",
  "error.invalid.machinery_not_on_farm": "La maquinaria elegida no pertenece a esta explotación.",
  "error.invalid.empty_practice_code": "Elija una buena práctica de la lista.",

  // Sección 7.1 — el plan de abonado (RD 1051/2022 art. 4.2, 5.a y 6).
  "error.invalid.invalid_nutrient_need": "Las necesidades no pueden ser negativas.",
  "error.invalid.invalid_expected_yield": "Indique un rendimiento esperado mayor que cero.",
  "error.invalid.crop_not_in_this_book": "El cultivo elegido no es de esta explotación y campaña.",
  "error.invalid.crop_already_planned": "Ese cultivo ya está incluido en otro plan de abonado.",
  "error.invalid.no_crops": "Indique al menos un cultivo de la unidad de producción.",
  "error.invalid.treatment_without_actuation":
    "Indique un producto fitosanitario, una medida no química, o ambos.",
  "error.invalid.dose_without_product":
    "Ha indicado una dosis sin producto. Elija el producto o borre la dosis.",
  "error.invalid.product_without_dose": "Indique la dosis del producto elegido.",
  "error.invalid.unknown_measure_code": "La medida indicada no figura en el catálogo oficial.",
  "error.invalid.invalid_intensity":
    "La intensidad debe indicarse con su unidad (trampas, difusores…) y ser mayor que cero.",
};
