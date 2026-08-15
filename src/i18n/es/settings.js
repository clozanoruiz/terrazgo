// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  "settings.general": "General",
  "settings.map": "Mapa",
  "settings.cache_size": "Espacio máximo para mapas sin conexión",
  "settings.cache_default": "Predeterminado ({size})",
  "settings.cache_hint":
    "Los mapas consultados se guardan para poder usarlos sin conexión; al superar el límite se eliminan primero los menos usados.",
  "settings.clear_cache": "Borrar mapas guardados",
  "settings.clear_cache_confirm":
    "¿Borrar los mapas guardados? Se descargarán de nuevo cuando haya conexión. Los datos de la explotación no se tocan.",
  "settings.catalogues": "Catálogos de referencia",
  "catalogues.hint":
    "Listas oficiales de códigos (FEGA) con las que la aplicación resuelve productos, problemas, materiales y demás. Vienen dentro de la aplicación y se actualizan con cada versión; aquí puede pedir la última publicada.",
  "catalogues.state": "{count} catálogos · {codes} códigos",
  "catalogues.updated_at": "última actualización: {date}",
  "catalogues.never": "aún sin importar",
  "catalogues.refresh": "Actualizar catálogos",
  "catalogues.refreshing": "Consultando el servicio…",
  "catalogues.updated": "nuevos: {added} · corregidos: {corrected}.",
  "catalogues.extra_columns": "columnas nuevas que esta versión no usa: {columns}.",
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
  "backup.title": "Copia de seguridad",
  "backup.import_confirm":
    "Importar una copia de seguridad SUSTITUYE todos los datos actuales por el contenido de la copia. Antes se guarda una copia de la base de datos actual. ¿Continuar?",
};
