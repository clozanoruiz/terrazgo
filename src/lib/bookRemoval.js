// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The words the screens put on a book deleted with its records (docs/sync.md →
// Deleting a book with its records): the confirmation before deleting, who
// deleted a book and when, what went with it, and until when it can come back.
// Every figure — what goes, what comes back, the last day — is the backend's;
// this only says it.
//
// Framework-agnostic tier: no Svelte import.
import { formatDate, t } from "../i18n.js";
import { kindName } from "./bookMerge.js";

/// The question asked before a book goes: its name, its farm, how many records
/// go with it, and the last day it can be brought back. A book with nothing in
/// it is not said to have "0 records".
export function deleteConfirm(season, farmName, preview) {
  const params = {
    label: season.label,
    farm: farmName,
    date: formatDate(preview.restorable_until),
  };
  return preview.records === 0
    ? t("season.delete_confirm", params)
    : t("season.delete_confirm_records", { ...params, count: preview.records });
}

/// What to call a device in a sentence: the name somebody gave it, "this
/// device", or "a device with no name" — never its id.
export function deviceName(device, label, thisDevice) {
  if (label) return label;
  return device && device === thisDevice ? t("sync.device_this") : t("sync.device_unnamed");
}

/// The parameters every sentence about a deletion takes: the day it was made,
/// who made it where this device knows a name, the device, and the last day it
/// can be undone.
function said(removal, thisDevice) {
  return {
    date: formatDate(removal.removed_at.slice(0, 10)),
    person: removal.author_name ?? "",
    device: deviceName(removal.removed_on, removal.device_label, thisDevice),
    until: formatDate(removal.restorable_until),
  };
}

/// A removed book's line in the list: when it went, by whom and where — the
/// person only where the log names one this device knows.
export function removedLine(removal, thisDevice) {
  const key = removal.author_name ? "removed_books.removed_by" : "removed_books.removed_on";
  return t(key, said(removal, thisDevice));
}

/// The line on a live book's page whose records went with it and have not come
/// back.
export function removedNotice(removal, thisDevice) {
  const key = removal.author_name ? "removed_with_book.notice_by" : "removed_with_book.notice_on";
  return t(key, said(removal, thisDevice));
}

/// How long a removed book can still be brought back — or, once that day has
/// passed and it waits to go for good (`erasing`), how long it could.
export function restorableUntil(removal, erasing = null) {
  const key = erasing ? "removed_books.was_until" : "removed_books.until";
  return t(key, { date: formatDate(removal.restorable_until) });
}

/// What went with a book, per kind — "Siembra: 2 · Cultivo: 1" — in the order
/// the backend sends, or "no records" for a book that went empty.
export function recordKinds(records) {
  if (records.length === 0) return t("removed_books.no_records");
  return records
    .map(({ table, count }) => t("strays.kind_count", { kind: kindName(table), n: count }))
    .join(" · ");
}

/// How many records went with a book, every kind together.
export function recordTotal(records) {
  return records.reduce((total, { count }) => total + count, 0);
}

/// What the notice says once a removed book is back: with how many records,
/// and nothing about records when none came back with it.
export function restoredBook(label, records) {
  return records === 0
    ? t("removed_books.restored", { label })
    : t("removed_books.restored_records", { label, count: records });
}
