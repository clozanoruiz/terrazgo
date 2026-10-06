// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The words the book-merging screens put on books and records (docs/sync.md →
// Merging two books that turned out to be one campaign): a book as a picker
// offers it, a record left in a removed book as the Status view names it, and
// those records counted by kind. Every decision — which book is kept by
// default, which one a removed book's records are suggested into — is the
// backend's; this only says it.
//
// Framework-agnostic tier: no Svelte import.
import { formatDate, languageTag, t, tCode } from "../i18n.js";

/// A book as a picker offers it: its name, and its campaign's dates — which
/// is what tells "2025/2026" from "2025/2026 bis" when both are one campaign.
export function bookItem(season) {
  return { value: season.id, label: bookName(season) };
}

export function bookName(season) {
  return t("season.option", {
    label: season.label,
    span: bookSpan(season),
  });
}

/// A book's campaign, first day to last.
export function bookSpan(season) {
  return t("season.span", {
    starts: formatDate(season.starts_on),
    ends: formatDate(season.ends_on),
  });
}

/// The other book a merge starts on: the only one there is, or none, so a
/// person with several to choose from chooses rather than accepting a guess.
export function onlyOther(others) {
  return others.length === 1 ? others[0].id : "";
}

/// A removed book's records counted by kind, in the order the backend listed
/// them — its register's table, then the record — so the count line and the
/// list under it read in the same order.
export function recordsByKind(records) {
  const counts = new Map();
  for (const record of records) {
    counts.set(record.table, (counts.get(record.table) ?? 0) + 1);
  }
  return [...counts].map(([table, n]) => ({ table, n }));
}

/// A register's kind as a heading: the `entity.*` names are written for the
/// middle of a sentence, so the first letter is raised — in the reader's own
/// language, which decides the casing rules.
export function kindName(table) {
  const kind = tCode("entity", table);
  return kind ? kind[0].toLocaleUpperCase(languageTag()) + kind.slice(1) : kind;
}

/// A record as the Status view names it: its kind, and the name the book
/// knows it by where its register has one — most registers go by their day,
/// which the backend hands back as the column holds it.
export function recordName(record) {
  const kind = kindName(record.table);
  if (!record.caption) return kind;
  const caption = /^\d{4}-\d{2}-\d{2}$/.test(record.caption)
    ? formatDate(record.caption)
    : record.caption;
  return `${kind} · ${caption}`;
}
