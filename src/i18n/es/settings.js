// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  // El armazón de la pantalla de ajustes: el buscador y el índice lateral.
  // Los nombres de las secciones y de los grupos están en lib/settingsTree.js.
  "settings.search": "Buscar",
  "settings.toc": "Secciones de ajustes",
  "settings.clear_search": "Borrar la búsqueda",
  "settings.collapse_section": "Contraer {section}",
  "settings.expand_section": "Expandir {section}",
  "settings.results.one": "{count} ajuste",
  "settings.results.other": "{count} ajustes",
  "settings.no_results": "Ningún ajuste coincide con la búsqueda.",
  "settings.general": "General",
  "settings.group.language": "Idioma y formato",
  "settings.map": "Mapa",
  "settings.group.offline_maps": "Mapas sin conexión",
  "settings.group.treated_plots": "Parcelas tratadas",
  "settings.data": "Datos",
  "settings.advanced": "Avanzado",
  "settings.cache_size": "Espacio máximo para mapas sin conexión",
  "settings.cache_default": "Predeterminado ({size})",
  "settings.cache_hint":
    "Los mapas consultados se guardan para poder usarlos sin conexión; al superar el límite se eliminan primero los menos usados.",
  "settings.clear_cache": "Borrar mapas guardados",
  "settings.clear_cache_confirm":
    "¿Borrar los mapas guardados? Se descargarán de nuevo cuando haya conexión. Los datos de la explotación no se tocan.",
  "settings.phi_horizon": "Mostrar parcelas tratadas hasta (días atrás)",
  "settings.phi_horizon_hint":
    "Durante cuánto tiempo el mapa sigue señalando una parcela ya tratada cuyo plazo de seguridad ha terminado. En blanco: {days} días. No afecta al plazo de seguridad ni a las parcelas con el plazo en curso, que se muestran siempre.",
  "settings.alerts": "Avisos",
  "settings.alerts_hint":
    "Con cuánta antelación quiere que le avisemos antes de que caduque un carné de aplicador o venza la ITV de una máquina. En blanco: {licence} y {itv} días. No es un plazo legal: elija el tiempo que necesite para renovarlo, que depende de la disponibilidad de cursos y de estaciones de ITV en su zona.",
  "settings.licence_lead": "Aviso de caducidad del carné (días)",
  "settings.itv_lead": "Aviso de vencimiento de la ITV (días)",
  "settings.catalogues": "Catálogos de referencia",
  "catalogues.hint":
    "Listas oficiales de códigos (FEGA) con las que la aplicación resuelve productos, problemas, materiales y demás. Vienen dentro de la aplicación; aquí puede pedir la última publicada, y se conserva hasta que una nueva versión de la aplicación traiga la suya.",
  // Dos recuentos que concuerdan con nombres distintos: forma "Etiqueta: N",
  // que es correcta con cualquier cifra (docs/frontend-conventions.md).
  "catalogues.state": "Catálogos: {count} · Códigos: {codes}",
  "catalogues.updated_at": "última actualización: {date}",
  "catalogues.never": "aún sin importar",
  "catalogues.refresh": "Actualizar catálogos",
  "catalogues.refreshing": "Consultando el servicio…",
  "catalogues.updated": "nuevos: {added} · corregidos: {corrected}.",
  "catalogues.withdrawn.one":
    "Uno ya no figura en la lista del organismo: deja de ofrecerse, pero los registros que ya lo citan siguen resolviéndose.",
  "catalogues.withdrawn.other":
    "{count} ya no figuran en la lista del organismo: dejan de ofrecerse, pero los registros que ya los citan siguen resolviéndose.",
  "catalogues.extra_columns.one": "columna nueva que esta versión no usa: {columns}.",
  "catalogues.extra_columns.other": "columnas nuevas que esta versión no usa: {columns}.",
  "catalogues.unchanged": "Sin cambios: {count}.",
  "catalogues.refused.shape":
    "el archivo ya no tiene la forma que esta versión sabe leer; hará falta una actualización de la aplicación",
  "catalogues.refused.empty": "el archivo llegó sin datos",
  "catalogues.refused.label": "una fila llegó sin descripción",
  "catalogues.refused.control_characters": "el archivo llegó con caracteres ilegibles",
  "catalogues.refused.shrunk":
    "el archivo trae menos filas de las ya guardadas; parece una descarga incompleta",
  "catalogues.refused.network": "no se pudo conectar con el servicio",
  "catalogues.refused.http": "el servicio respondió con un error",
  "settings.profiles": "Perfiles de usuario",
  "settings.maintenance": "Mantenimiento de la base de datos",
  "settings.maintenance_hint":
    "Revisa a fondo el archivo de datos en busca de daños y, si está en buen estado, lo compacta para recuperar espacio. La aplicación ya hace una revisión rápida cada semana por su cuenta; ésta es más completa y puede tardar unos segundos.",
  "settings.check_db": "Revisar y compactar",
  "backup.title": "Copia de seguridad",
  "backup.import_confirm":
    "Importar una copia de seguridad SUSTITUYE todos los datos actuales por el contenido de la copia. Antes se guarda una copia de la base de datos actual. ¿Continuar?",
};
