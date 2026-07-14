// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español. Las claves son idénticas a en.js; los huecos `{name}`
// los rellena el argumento params de t(). i18n.js lo carga bajo demanda —
// todos los diccionarios deben definir el mismo conjunto de claves.
export default {
  "lang.label": "Idioma",

  "app.subtitle": "Gestión de la explotación",

  "status.aria": "Estado de la aplicación",
  "status.database": "Base de datos",
  "status.schema_version": "Versión del esquema",
  "status.app_version": "Versión de la aplicación",

  "actions.aria": "Acciones",
  "actions.refresh": "Actualizar alertas",
  "actions.seed": "Cargar datos de demostración",
  "actions.ack": "Visto",
  "actions.dismiss": "Descartar",

  "alerts.title": "Alertas activas",
  "alerts.empty": "No hay alertas activas.",
  "alerts.due": "vence el {date}",

  // Etiquetas de los códigos del esquema (alert.alert_type_code).
  "alert.type.phi_window": "Plazo de seguridad en curso",
  "alert.type.licence_expiry": "Carné de aplicador a punto de caducar",
  "alert.type.itv_expiry": "ITV de maquinaria próxima",
  "alert.type.nitrate_zone":
    "Parcela en zona vulnerable a nitratos — registro de fertilización obligatorio",
  "alert.type.phyto_zone": "Parcela en zona de restricción fitosanitaria",
  "alert.type.natura_zone": "Parcela en Red Natura 2000 — restricciones de condicionalidad",

  "alert.status.active": "activa",
  "alert.status.acknowledged": "vista",

  // Etiquetas de los nombres de tabla del esquema (alert.subject_table).
  "entity.treatment_record": "tratamiento",
  "entity.operator": "operador",
  "entity.machinery": "maquinaria",
  "entity.plot": "parcela",
  "zone.nitrate_vulnerable": "Vulnerable a nitratos",
  "zone.phytosanitary_restriction": "Restricción fitosanitaria",
  "zone.natura_2000": "Red Natura 2000",
  "plot.zones_unchecked": "No se pudieron comprobar las zonas — verifica de nuevo con conexión.",

  "message.refreshed": "Alertas actualizadas.",
  "message.seeded": "Demostración cargada: campaña {season} ({farm}).",
  "message.already_seeded": "La base de datos ya contiene datos; no se ha cargado nada.",

  "notif.aria": "Notificaciones",
  "notif.empty": "No hay notificaciones.",
  "notif.clear": "Borrar todas",

  "nav.aria": "Navegación principal",
  "nav.collapse": "Contraer el menú",
  "nav.expand": "Expandir el menú",
  "nav.status": "Estado",
  "nav.farms": "Explotaciones",
  "nav.map": "Mapa",
  "nav.treatments": "Tratamientos",
  "nav.registry": "Catálogo",
  "nav.settings": "Ajustes",

  "farms.title": "Explotaciones",
  "farms.new": "Nueva explotación",
  "farms.empty": "Aún no hay explotaciones; cree la primera.",
  "farms.back": "← Explotaciones",

  "farm.name": "Nombre",
  "farm.owner": "Titular",
  "farm.country": "País",
  "farm.location": "Ubicación",
  "farm.latitude": "Latitud",
  "farm.longitude": "Longitud",
  "farm.es_section": "Datos registrales (España)",
  "farm.rega": "Código REGA",
  "farm.province": "Código de provincia",
  "farm.delete": "Eliminar explotación",
  "farm.delete_confirm":
    "¿Eliminar la explotación «{name}»? Se oculta de la aplicación pero sus registros se conservan.",

  "plots.title": "Parcelas",
  "plots.new": "Nueva parcela",
  "plots.empty": "Esta explotación aún no tiene parcelas.",

  "plot.name": "Nombre",
  "plot.area": "Superficie (ha)",
  "plot.sigpac_section": "Referencia SIGPAC",
  "plot.sigpac_province": "Provincia",
  "plot.sigpac_municipality": "Municipio",
  "plot.sigpac_aggregate": "Agregado",
  "plot.sigpac_zone": "Zona",
  "plot.sigpac_polygon": "Polígono",
  "plot.sigpac_parcel": "Parcela",
  "plot.sigpac_enclosure": "Recinto",
  "plot.sigpac_verify": "Verificar en SIGPAC",
  "plot.sigpac_found": "SIGPAC: {area} ha · uso {use}.",
  "plot.sigpac_not_found": "SIGPAC no conoce esta referencia — revisa las siete partes.",
  "plot.sigpac_use_area": "Usar superficie oficial",
  "plot.sigpac_already_on": "Esta referencia ya está en {plot} ({farm}).",
  "plot.sigpac_official": "SIGPAC {area} ha",
  "plot.edit": "Editar",
  "plot.delete": "Eliminar",
  "plot.delete_confirm":
    "¿Eliminar la parcela «{name}»? Se oculta de la aplicación pero sus registros se conservan.",

  "form.save": "Guardar",
  "form.cancel": "Cancelar",
  "form.edit": "Editar",
  "form.close": "Cerrar",
  "form.delete": "Eliminar",
  "form.remove": "Quitar",

  "message.farm_saved": "Explotación «{name}» guardada.",
  "message.farm_deleted": "Explotación eliminada.",
  "message.plot_saved": "Parcela «{name}» guardada.",
  "message.plot_deleted": "Parcela eliminada.",
  "message.boundary_saved": "Contorno guardado para {name}.",
  "message.sigpac_boundary_saved": "Contorno oficial SIGPAC guardado para {name}.",
  "message.boundary_deleted": "Contorno eliminado.",

  // Espacio de trabajo del mapa (MapView / MapCanvas).
  "map.farm": "Explotación",
  "map.layers": "Capas",
  "map.layer.plots": "Contornos de parcelas",
  "map.layer.phi_status": "Plazos de seguridad",
  "map.layer.zone_flags": "Zonas reguladas",
  "map.legend.phi_in": "En plazo de seguridad — no cosechar",
  "map.legend.phi_clear": "Tratada — cosecha permitida",
  "map.layer.sigpac_recintos": "Recintos SIGPAC",
  "map.layer.cultivo_declarado": "Cultivos declarados (campaña anterior)",
  "map.layer.paisaje": "Elementos del paisaje",
  "map.inspect.title": "En este punto",
  "map.inspect.phi_until": "Cosecha permitida desde",
  "map.inspect.sigpac_ref": "Referencia SIGPAC",
  "map.inspect.land_use": "Uso SIGPAC",
  "map.inspect.surface_ha": "Superficie (ha)",
  "map.inspect.crop_code": "Cultivo declarado (código)",
  "map.inspect.exploitation_system": "Sistema (secano/regadío)",
  "map.inspect.declared_surface": "Superficie declarada (ha)",
  "map.inspect.campaign": "Campaña",
  "map.inspect.element_type": "Tipo de elemento (código)",
  "map.zoom_hint": "Acerca el mapa para ver: {layers}",
  "map.no_farms": "Crea primero una explotación para verla en el mapa.",
  "map.plots": "Parcelas",
  "map.select_plot_hint": "Selecciona una parcela para dibujar o importar su contorno.",
  "map.has_boundary": "Tiene contorno",
  "map.draw": "Dibujar contorno",
  "map.drawing_hint": "Haz clic en el mapa para añadir puntos; clic en el primero para terminar.",
  "map.draw_cancel": "Cancelar dibujo",
  "map.import": "Importar de archivo…",
  "map.import_pick": "Elige un contorno ({count} encontrados)",
  "map.import_filter": "Filtrar por nombre o atributos…",
  "map.import_use": "Usar",
  "map.sigpac_pick": "Buscar recinto SIGPAC",
  "map.sigpac_pick_hint": "Toca un punto del mapa para consultar el recinto que hay debajo.",
  "map.sigpac_none": "No hay recinto SIGPAC en ese punto.",
  "map.sigpac_attach": "Guardar contorno en {plot}",
  "map.sigpac_create": "Crear parcela con este recinto",
  "map.import_more": "…y {count} más — afina el filtro.",
  "map.boundaries": "Contornos guardados",
  "map.no_boundary": "Aún no hay contorno guardado.",
  "map.delete_boundary": "Quitar",
  "map.delete_boundary_confirm": "¿Quitar el contorno {source}? Su historial se conserva.",
  "map.source.manual": "dibujado",
  "map.source.import": "importado",
  "map.source.sigpac": "SIGPAC",
  "map.base_label": "Mapa base",
  "map.base_streets": "Mapa",
  "map.base_ortho": "Orto",
  "map.basemap_unavailable":
    "Mapa base no disponible sin conexión (aún no está en caché) — los contornos se muestran igualmente.",

  // Mapa incrustado en la vista de explotación.
  "farm.map_title": "Mapa de la explotación",
  "farm.open_map": "Abrir en el mapa",
  "plot.on_map": "En el mapa",

  "treatments.title": "Tratamientos fitosanitarios",
  "treatments.records_title": "Tratamientos",
  "treatments.farm": "Explotación",
  "treatments.season": "Campaña",
  "treatments.new": "Nuevo tratamiento",
  "treatments.empty": "Aún no hay tratamientos registrados para esta explotación y campaña.",
  "treatments.no_farms": "Cree primero una explotación con al menos una parcela:",
  "treatments.no_plots": "Esta explotación aún no tiene parcelas; añádalas en Explotaciones.",
  "treatments.missing_refs":
    "Registrar un tratamiento requiere al menos un producto autorizado y un operador; créelos en el catálogo:",

  "seasons.new": "Nueva campaña",
  "seasons.empty": "Aún no hay campañas; cree la primera.",
  "season.campaign_year": "Año de campaña",
  "season.label": "Etiqueta",
  "season.starts": "Comienza el",
  "season.ends": "Termina el",

  "crops.title": "Cultivos",
  "crops.new": "Nuevo cultivo",
  "crops.empty": "Aún no hay cultivos declarados para esta explotación y campaña.",
  "crop.plot": "Parcela",
  "crop.species": "Especie",
  "crop.variety": "Variedad",
  "crop.production_system": "Sistema de producción",
  "crop.sown_on": "Fecha de siembra",
  "crop.sown_detail": "sembrado el {date}",

  "treatment.date": "Fecha de aplicación",
  "treatment.product": "Producto",
  "treatment.dose": "Dosis",
  "treatment.unit": "Unidad",
  "treatment.reason": "Motivo",
  "treatment.target": "Organismo objetivo",
  "treatment.operator": "Operador",
  "treatment.machinery": "Maquinaria",
  "treatment.machinery_none": "— ninguna —",
  "treatment.phi_days": "Plazo de seguridad (días)",
  "treatment.phi_default": "Valor del producto: {days} días",
  "treatment.notes": "Observaciones",
  "treatment.plots_section": "Parcelas tratadas",
  "treatment.crop": "Cultivo",
  "treatment.crop_none": "— sin declarar —",
  "treatment.surface": "Superficie tratada (ha)",
  "treatment.add_plot": "Añadir parcela",
  "treatment.remove": "Quitar",
  "treatment.phi_until": "Plazo de seguridad: cosecha a partir del {date}",
  "treatment.delete": "Eliminar",
  "treatment.delete_confirm":
    "¿Eliminar este tratamiento? Se oculta de la aplicación pero se conserva en el registro de auditoría.",

  "message.season_saved": "Campaña «{label}» creada.",
  "message.crop_saved": "Cultivo «{species}» guardado.",
  "message.treatment_saved": "Tratamiento registrado; cosecha permitida a partir del {date}.",
  "message.treatment_deleted": "Tratamiento eliminado.",

  "registry.title": "Catálogo",

  "products.title": "Productos fitosanitarios",
  "products.new": "Nuevo producto",
  "products.empty": "Aún no hay productos; cree el primero.",

  "product.name": "Nombre comercial",
  "product.holder": "Titular / fabricante",
  "product.formulation": "Formulación",
  "product.phi_days": "Plazo de seguridad por defecto (días)",
  "product.phi_detail": "plazo de seguridad {days} días",
  "product.auth_section": "Autorización",
  "product.auth_country": "País",
  "product.auth_number": "Nº de registro",
  "product.no_authorisations":
    "Sin autorización en ningún país; no se ofrecerá al registrar tratamientos.",
  "product.substances": "Sustancias activas",
  "product.add_substance": "Añadir sustancia",
  "product.authorisations": "Autorizaciones",
  "product.add_authorisation": "Añadir autorización",
  "product.delete_confirm":
    "¿Eliminar el producto «{name}»? Se oculta de la aplicación pero los tratamientos pasados conservan sus datos.",

  "substance.existing": "Sustancia",
  "substance.new_name": "Nueva sustancia",
  "substance.cas": "Nº CAS",
  "substance.concentration": "Concentración",
  "substance.unit": "Unidad",

  "operators.title": "Operadores",
  "operators.new": "Nuevo operador",
  "operators.empty": "Aún no hay operadores; cree el primero.",

  "operator.full_name": "Nombre completo",
  "operator.licence_number": "Nº de carné",
  "operator.licence_level": "Nivel del carné",
  "operator.licence_expiry": "Caducidad del carné",
  "operator.expiry_detail": "carné hasta el {date}",
  "operator.delete_confirm":
    "¿Eliminar el operador «{name}»? Se oculta de la aplicación pero sus registros se conservan.",

  "machinery.title": "Maquinaria",
  "machinery.new": "Nueva máquina",
  "machinery.empty": "Aún no hay maquinaria en esta explotación.",
  "machinery.no_farms": "Cree primero una explotación:",
  "machinery.farm": "Explotación",
  "machinery.name": "Nombre",
  "machinery.kind": "Tipo",
  "machinery.last_inspection": "Última inspección (ITV)",
  "machinery.next_inspection": "Próxima inspección",
  "machinery.es_section": "Datos registrales (España)",
  "machinery.roma": "Nº ROMA (equipos móviles)",
  "machinery.reganip": "Nº REGANIP (aeronaves e instalaciones fijas)",
  "machinery.itv_detail": "próxima ITV el {date}",
  "machinery.delete_confirm":
    "¿Eliminar la máquina «{name}»? Se oculta de la aplicación pero sus registros se conservan.",

  "message.product_saved": "Producto «{name}» guardado.",
  "message.product_deleted": "Producto eliminado.",
  "message.operator_saved": "Operador «{name}» guardado.",
  "message.operator_deleted": "Operador eliminado.",
  "message.machinery_saved": "Máquina «{name}» guardada.",
  "message.machinery_deleted": "Máquina eliminada.",
  "message.substance_added": "Sustancia activa añadida.",
  "message.substance_removed": "Sustancia activa quitada.",
  "message.authorisation_added": "Autorización añadida.",
  "message.authorisation_removed": "Autorización quitada.",

  // Etiquetas de los códigos del esquema (unit, reason_category, production_system).
  "unit.l_ha": "l/ha",
  "unit.kg_ha": "kg/ha",
  "unit.ml_ha": "ml/ha",
  "unit.g_ha": "g/ha",
  "unit.ml_hl": "ml/hl",
  "unit.g_hl": "g/hl",
  "unit.g_l": "g/l",
  "unit.ml_l": "ml/l",
  "unit.pct": "%",
  "reason_category.pest": "Plaga",
  "reason_category.disease": "Enfermedad",
  "reason_category.weed": "Mala hierba",
  "reason_category.growth_regulator": "Regulador de crecimiento",
  "reason_category.other": "Otro",
  "production_system.conventional": "Convencional",
  "production_system.organic": "Ecológico",
  "production_system.integrated": "Producción integrada",
  "licence_level.basic": "Básico",
  "licence_level.qualified": "Cualificado",
  "licence_level.fumigator": "Fumigador",
  "formulation_type.wp": "WP (polvo mojable)",
  "formulation_type.sc": "SC (suspensión concentrada)",
  "formulation_type.ec": "EC (concentrado emulsionable)",
  "formulation_type.wg": "WG (granulado dispersable)",
  "formulation_type.sl": "SL (concentrado soluble)",

  "settings.general": "General",
  "settings.map": "Mapa",
  "settings.cache_size": "Espacio máximo para mapas sin conexión",
  "settings.cache_default": "Predeterminado ({size})",
  "settings.cache_hint":
    "Los mapas consultados se guardan para poder usarlos sin conexión; al superar el límite se eliminan primero los menos usados.",
  "settings.clear_cache": "Borrar mapas guardados",
  "settings.clear_cache_confirm":
    "¿Borrar los mapas guardados? Se descargarán de nuevo cuando haya conexión. Los datos de la explotación no se tocan.",
  "message.settings_saved": "Ajustes guardados.",
  "message.cache_cleared": "Mapas guardados borrados ({count}).",

  "backup.title": "Copia de seguridad",
  "actions.export_backup": "Exportar copia de seguridad",
  "actions.import_backup": "Importar copia de seguridad",
  "backup.import_confirm":
    "Importar una copia de seguridad SUSTITUYE todos los datos actuales por el contenido de la copia. Antes se guarda una copia de la base de datos actual. ¿Continuar?",
  "message.backup_saved": "Copia guardada en {path} ({size}).",
  "message.backup_imported": "Copia importada. La base de datos anterior se guardó en {path}.",

  // Errores del límite de comandos (códigos de CommandError → error.<code>).
  // "internal" no tiene entrada error.<code> a propósito: se muestra el mensaje
  // técnico, precedido por internal_intro para orientar al usuario normal.
  "error.internal_intro": "Se ha producido un error interno:",
  "error.not_found": "El registro no existe.",
  "error.invalid.empty_name": "El nombre no puede estar vacío.",
  "error.invalid.empty_authorisation_number": "El número de registro no puede estar vacío.",
  "error.invalid.nonpositive_area": "La superficie debe ser mayor que cero.",
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

  // Etiquetas de los códigos de país (tabla de referencia `country`).
  "country.es": "España",
  "country.fr": "Francia",
  "country.it": "Italia",
};
