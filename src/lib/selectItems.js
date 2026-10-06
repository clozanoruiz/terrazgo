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
///
/// A catalogue list read in name order (the animal species, the cover types)
/// passes through here too, and keeps the `offered` flag each row carries.
export function nameItems(rows, name = (row) => row.name, id = (row) => row.id) {
  return sortedBy(
    rows.map((row) =>
      "offered" in row
        ? { value: id(row), label: name(row), offered: row.offered !== false }
        : { value: id(row), label: name(row) },
    ),
    (item) => item.label,
  );
}

/// A catalogue-backed list — FEGA codes with the names the backend resolved —
/// in the provider's order, each item saying whether it may be offered. The
/// backend sends codes it no longer offers too (retired by the authority, or
/// gone from its file), because a record may still carry one and the picker
/// has to name it. `name` reads a row's label: picks carry `name`, raw
/// catalogue rows `label`.
export function catalogueItems(rows, name = (row) => row.name) {
  return rows.map((row) => ({
    value: row.code,
    label: name(row),
    offered: row.offered !== false,
  }));
}

/// What a dropdown lists for `value`: every item it may offer, and the current
/// value whatever its state, so a record's code never shows as an empty field.
///
/// For a catalogue picker, a value no item carries is listed as itself and
/// flagged `unknown`: this device's catalogues do not have the code, typically
/// because the record was written on a device whose catalogues are newer. A
/// list that is not a catalogue invents nothing — a missing id would print as
/// a UUID.
export function pickerOptions(items, value, catalogue = false) {
  const options = items.filter((item) => item.offered !== false || item.value === value);
  const unknown = Boolean(catalogue && value) && !items.some((item) => item.value === value);
  return {
    options: unknown ? [{ value, label: value }, ...options] : options,
    unknown,
  };
}
