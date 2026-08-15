// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.

export default {
  "settings.general": "General",
  "settings.map": "Map",
  "settings.cache_size": "Maximum space for offline maps",
  "settings.cache_default": "Default ({size})",
  "settings.cache_hint":
    "Viewed maps are stored for offline use; past the limit, the least recently used are removed first.",
  "settings.clear_cache": "Clear stored maps",
  "settings.clear_cache_confirm":
    "Clear the stored maps? They will download again when online. Farm data is not affected.",
  "settings.catalogues": "Reference catalogues",
  "catalogues.hint":
    "The official code lists (FEGA) the app resolves products, problems, materials and the rest against. They ship inside the app and update with each release; here you can ask for the latest published one.",
  "catalogues.state": "{count} catalogues · {codes} codes",
  "catalogues.updated_at": "last updated: {date}",
  "catalogues.never": "not imported yet",
  "catalogues.refresh": "Update catalogues",
  "catalogues.refreshing": "Contacting the service…",
  "catalogues.updated": "new: {added} · corrected: {corrected}.",
  "catalogues.extra_columns": "new columns this version does not use: {columns}.",
  "catalogues.unchanged": "Unchanged: {count}.",
  "catalogues.refused.shape":
    "the file no longer has the shape this version can read; an app update will be needed",
  "catalogues.refused.empty": "the file arrived with no data",
  "catalogues.refused.label": "a row arrived with no description",
  "catalogues.refused.control_characters": "the file arrived with unreadable characters",
  "catalogues.refused.shrunk":
    "the file carries fewer rows than the ones already stored; this looks like an incomplete download",
  "catalogues.refused.network": "could not reach the service",
  "catalogues.refused.http": "the service answered with an error",
  "settings.profiles": "User profiles",
  "backup.title": "Backup",
  "backup.import_confirm":
    "Importing a backup REPLACES all current data with the backup's content. A safety copy of the current database is saved first. Continue?",
};
