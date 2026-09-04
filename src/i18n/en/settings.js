// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.

export default {
  // The settings screen's own frame: the search field and the contents list
  // beside it. Section and group names live in lib/settingsTree.js.
  "settings.search": "Search",
  "settings.toc": "Settings sections",
  "settings.clear_search": "Clear the search",
  "settings.collapse_section": "Collapse {section}",
  "settings.expand_section": "Expand {section}",
  "settings.results.one": "{count} setting",
  "settings.results.other": "{count} settings",
  "settings.no_results": "No setting matches the search.",
  "settings.general": "General",
  "settings.group.language": "Language and format",
  "settings.map": "Map",
  "settings.group.offline_maps": "Offline maps",
  "settings.group.treated_plots": "Treated plots",
  "settings.data": "Data",
  "settings.advanced": "Advanced",
  "settings.cache_size": "Maximum space for offline maps",
  "settings.cache_default": "Default ({size})",
  "settings.cache_hint":
    "Viewed maps are stored for offline use; past the limit, the least recently used are removed first.",
  "settings.clear_cache": "Clear stored maps",
  "settings.clear_cache_confirm":
    "Clear the stored maps? They will download again when online. Farm data is not affected.",
  "settings.phi_horizon": "Keep showing treated plots for (days)",
  "settings.phi_horizon_hint":
    "How long the map keeps marking a plot that was treated and whose pre-harvest interval has since ended. Left blank: {days} days. It does not affect the interval itself, nor plots still within one, which are always shown.",
  "settings.alerts": "Alerts",
  "settings.alerts_hint":
    "How far ahead you want to be warned before an operator licence expires or a machine's roadworthiness test falls due. Left blank: {licence} and {itv} days. This is not a legal deadline: choose the time you need to renew, which depends on course and test-station availability where you are.",
  "settings.licence_lead": "Licence expiry warning (days)",
  "settings.itv_lead": "Roadworthiness test warning (days)",
  "settings.catalogues": "Reference catalogues",
  "catalogues.hint":
    "The official code lists (FEGA) the app resolves products, problems, materials and the rest against. They ship inside the app; here you can ask for the latest published one, and it is kept until a new version of the app brings its own.",
  // Two counts agreeing with different nouns: the "Label: N" form, which is
  // correct at any figure (docs/frontend-conventions.md).
  "catalogues.state": "Catalogues: {count} · Codes: {codes}",
  "catalogues.updated_at": "last updated: {date}",
  "catalogues.never": "not imported yet",
  "catalogues.refresh": "Update catalogues",
  "catalogues.refreshing": "Contacting the service…",
  "catalogues.updated": "new: {added} · corrected: {corrected}.",
  "catalogues.withdrawn.one":
    "One is no longer on the authority's list: it stops being offered, though records already citing it still resolve.",
  "catalogues.withdrawn.other":
    "{count} are no longer on the authority's list: they stop being offered, though records already citing them still resolve.",
  "catalogues.extra_columns.one": "new column this version does not use: {columns}.",
  "catalogues.extra_columns.other": "new columns this version does not use: {columns}.",
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
  "settings.maintenance": "Database maintenance",
  "settings.maintenance_hint":
    "Checks the data file thoroughly for damage and, if it is sound, compacts it to reclaim space. The app already runs a quick check weekly on its own; this one is more complete and can take a few seconds.",
  "settings.check_db": "Check and compact",
  "backup.title": "Backup",
  "backup.import_confirm":
    "Importing a backup REPLACES all current data with the backup's content. A safety copy of the current database is saved first. Continue?",
};
