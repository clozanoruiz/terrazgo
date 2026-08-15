// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Turning the app's two kinds of list into the `{ value, label }` items an
// owned dropdown takes.
//
// The two exist separately because they differ in ONE decision that matters:
// whether the list may be re-ordered. Coded vocabularies carry meaning in their
// order — licence levels run basic → qualified → fumigator → pilot, BBCH stages
// run 0-9, efficacy runs good → fair → poor — so alphabetising them would be a
// regression. Entity lists are the opposite: they arrive in SQL's BINARY order,
// which puts "Ángel" after "Zubiri", and alphabetical is what a farmer expects.
//
// Framework-agnostic tier: no Svelte import.
import { tCode } from "../i18n.js";
import { sortedBy } from "./collate.js";

/// A coded vocabulary (`lookups.*`), labelled through the dictionary and kept
/// in the order the backend supplied.
export function codeItems(rows, prefix) {
  return rows.map((row) => ({ value: row.code, label: tCode(prefix, row.code) }));
}

/// User-data rows, ordered by the active language (see lib/collate.js).
///
/// The accessors are functions rather than key names because the rows are not
/// all flat — a fertiliser material arrives as `{ material, nutrients }`.
export function nameItems(rows, name = (row) => row.name, id = (row) => row.id) {
  return sortedBy(
    rows.map((row) => ({ value: id(row), label: name(row) })),
    (item) => item.label,
  );
}
