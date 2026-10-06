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
  "sync.title": "Sync with another device",
  "sync.hint":
    "Export this device's changes to a file, copy it to the other device and import it there. One file each way brings both up to date. Nothing leaves your devices: the file goes where you take it.",
  "sync.import_confirm.one":
    "This file comes from {device} and was made on {created}. It brings one change. Apply it to this record book?",
  "sync.import_confirm.other":
    "This file comes from {device} and was made on {created}. It brings {count} changes. Apply them to this record book?",
  "sync.pair_confirm":
    "This device is not paired with {device} yet. Pairing them means they share a record book: each one's changes reach the other every time you sync. Pair them?",
  "sync.join_other_confirm":
    "CAREFUL: this file comes from a different group of devices. If you continue, this device leaves its current group and joins {device}'s. Only do this if both are yours and keep the same record book.",
  "sync.peers_title": "Devices on this holding",
  "sync.peers_hint":
    'Every device that writes in this record book appears here. Name them: when two of them have written the same record at once, the screen then says "María\'s phone" instead of a code.',
  "sync.peers_empty": "So far this is the only device that has written in this record book.",
  "sync.peer_label": "Device name",
  "sync.device_this": "this device",
  "sync.device_unnamed": "an unnamed device",
  "sync.peer_unnamed": "Unnamed",
  "sync.peer_this_device": "This device",
  "sync.peer_known_since": "Known since",
  "sync.peer_retired": "Retired",
  "sync.peer_retire": "Retire",
  "sync.peer_restore": "Bring back",
  "sync.peer_retire_confirm":
    'Retire "{name}"? What it wrote stays in the record book and stays in its name; this only stops expecting it to sync again. You can bring it back at any time.',
  "conflicts.title": "Records written at once",
  "conflicts.hint":
    "Two devices wrote these records without having seen each other. The record book shows one of the versions on every device; choose which one stays.",
  "conflicts.review": "Review",
  "conflicts.written_on": "Written on {devices}",
  "conflicts.in_book": "{farm}, campaign {season}",
  "conflicts.version_live": "This is the version the record book shows",
  "conflicts.version_waiting": "Version waiting",
  "conflicts.written_by": "Written by {person}",
  "conflicts.written_at": "Written on {date}",
  "conflicts.keep": "Keep this one",
  "conflicts.keep_confirm":
    "Keep {device}'s version? The record book will show it on every device. The other one is not lost: the database keeps every version of the record.",
  "conflicts.kept": "Done. The record book shows {device}'s version.",
  "conflicts.identical":
    "Both versions say exactly the same thing. Keep either one to close the difference.",
  "conflicts.edit_hint":
    "If the right answer is neither of the two, correct the record in its own screen: any change closes the difference on every device.",
  "conflicts.yes": "Yes",
  "conflicts.no": "No",
  "conflicts.absent": "—",
  "duplicates.title": "Possible duplicates",
  "duplicates.hint":
    "These records are so alike that they may be one operation recorded twice. Review each pair: keep one of the two, or say that they are two different records.",
  "duplicates.review": "Review",
  "duplicates.written_by": "Recorded by {people}",
  "duplicates.in_book": "{farm}, campaign {season}",
  "duplicates.record": "Record {n}",
  "duplicates.on_device": "On {device}",
  "duplicates.by_person": "By {person}",
  "duplicates.written_at": "Recorded on {date}",
  "duplicates.differs_hint": "What is highlighted is what the two do not agree on.",
  "duplicates.keep": "Keep record {n}",
  "duplicates.keep_confirm":
    "Keep record {n} and remove the other one? The removed one will no longer appear in the record book, and why it was removed is recorded.",
  "duplicates.kept": "Done. One record was kept and the other removed.",
  "duplicates.both_real": "They are two different records",
  "duplicates.judged_distinct":
    "Noted: they are two different records. You will not be asked about them again.",
  "duplicates.both_removed": "No longer in the record book",
  "duplicates.both_removed_hint":
    "This operation is no longer in the record book. It had been recorded twice, and each device kept a different copy and removed the other before they synced: once they met, both were removed. Restore one so that it is recorded once again.",
  "duplicates.restore": "Restore record {n}",
  "duplicates.restored": "Done. The record is back in the record book.",
  "duplicates.book_notice.one": "One possible duplicate in this campaign.",
  "duplicates.book_notice.other": "{count} possible duplicates in this campaign.",
  "duplicates.book_title": "Possible duplicates in this campaign",
  "duplicates.removed_each": "One copy was removed by {first} and the other by {second}",
  "duplicates.remover": "{person} on {device}",
  "duplicates.removed_by": "Removed by {person} on {device} on {date}",
  "duplicates.removed_on_device": "Removed on {device} on {date}",
  "duplicates.identical":
    "The two records say exactly the same thing: it does not matter which you keep.",
  "duplicates.identical_removed":
    "The two copies say exactly the same thing: it does not matter which you restore.",
  "duplicates.back": "Back to the list",
  "duplicates.just_saved": "The one you just saved",
  "duplicates.saved_hint":
    "What you just saved looks very much like a record that was already there. If it is one operation recorded twice, keep one; if they are two, say so. You can also close this and decide later: it stays on the list of possible duplicates.",
  "strays.title": "Records in a deleted record book",
  "strays.hint":
    "These records are still in a record book that no longer exists — usually because it was merged with another or deleted while another device went on recording in it — so they appear in no record book, printed or exported. Move them into a record book of the same farm, or bring the deleted one back.",
  "strays.removed_book": "Deleted record book",
  // "Label: N" form: the record's kind does not agree with the figure.
  "strays.kind_count": "{kind}: {n}",
  "strays.show_records": "Show the records",
  "strays.and_more": "…and {more} more.",
  "strays.into": "Move them into",
  "strays.move": "Move the records",
  "strays.move_confirm.one": "Move the record from “{from}” into “{into}”?",
  "strays.move_confirm.other": "Move the {count} records from “{from}” into “{into}”?",
  "strays.moved.one": "Done. The record is now in “{into}”.",
  "strays.moved.other": "Done. The records are now in “{into}”.",
  "strays.restore": "Bring the record book back",
  "strays.restore_confirm":
    "Bring {farm}'s “{label}” record book back? It returns to the list of record books with the records it still holds and those deleted with it.",
  "strays.restored": "Done. {farm}'s “{label}” record book is back in the list.",
  "strays.no_book": "This farm has no record book to move them into: bring this one back.",
  "sync.field.deleted_at": "Deleted on",
  "sync.field.season_id": "Record book",
  "sync.field.season_label": "Campaign name",
  "sync.field.season_status": "Campaign status",
  "sync.field.crop_code": "Crop code",
  "sync.field.product_code": "Product code",
  "sync.field.declared_area": "Declared area (ha)",
  "sync.field.energy_type": "Energy type",
  "sync.field.richness_n": "Nitrogen content",
  "sync.field.richness_p2o5": "P₂O₅ content",
  "sync.field.richness_k2o": "K₂O content",
  "sync.field.source": "Where the data came from",
  "sync.field.source_campaign": "Campaign the data came from",
  "sync.field.subject": "Treated subject",
  "sync.field.subject_kind": "Kind of treated subject",
  "sync.field.premises": "Premises or vehicle",
  "sync.field.sowing": "Sowing",
  "sync.field.soil_cover": "Soil cover",
  "sync.field.advisor": "Advisor",
  "sync.field.justification": "Justification",
  "backup.title": "Backup",
  "backup.import_loss.one":
    "This record book holds 1 change the backup does not contain. Importing it removes that change from the book.",
  "backup.import_loss.other":
    "This record book holds {count} changes the backup does not contain. Importing it removes them from the book.",
  "backup.import_loss_own.one":
    "That change was written on this device, so it is on no other one: the safety copy taken before the import will be the only place left that holds it.",
  "backup.import_loss_own.other":
    "{count} of those changes were written on this device, so they are on no other one: the safety copy taken before the import will be the only place left that holds them.",
  "backup.import_confirm":
    "Importing a backup REPLACES all current data with the backup's content. A safety copy of the current database is saved first. Continue?",
};
