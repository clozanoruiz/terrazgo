// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Reading and writing the text of a numeric field, beside NumberInput.svelte
// the way dateValue.js sits beside DateInput.svelte.
//
// This exists because `<input type="number">` parses what the user types with
// the OPERATING SYSTEM's locale, not the language the holding chose — and in
// the mismatch case it does not refuse, it reinterprets. Measured in the real
// WebKitGTK webview on 2026-08-27:
//
//   OS es_ES, typing "1,5"  -> 1.5   (comma read as the decimal separator)
//   OS en_GB, typing "1,5"  -> 15    (comma read as a THOUSANDS separator)
//
// So a farmer running the app in Castilian on an English-locale machine enters
// a dose of 1,5 l/ha and records 15 — ten times the amount, in a register that
// is read at an inspection, with no error and no empty field to notice. That is
// what makes this a correctness fix and not a presentation one.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
// formatTag, not the language: the separator this parses must be the one the
// field just displayed, or a farmer would retype what they see and be refused.
import { formatTag } from "../i18n.js";

// Built per locale and reused, the collate.js reasoning.
const separators = new Map();
const writers = new Map();

/// Digits with at most ONE separator, which is always a decimal point.
///
/// There is no thousands-separator case to handle because the app has no
/// thousands separator: formatNumber renders with `useGrouping: false`, so a
/// grouped figure is never shown, never copied out of the app, and has no
/// reading here worth guessing at. "1.234,5" is simply refused — visibly, with
/// a message — which is the whole difference from the native control's silent
/// 15. Both separators are accepted as the decimal point, because a farmer
/// typing "1.5" on a keypad that offers only a dot means one and a half in any
/// language.
///
/// **Why this is not `@internationalized/number`** (Adobe's parser, which
/// `@zag-js/number-input` and Ark UI are both built on). It was evaluated and
/// measured on 2026-08-27, and it is strictly locale-bound, which is the wrong
/// property here: under `es-ES` it reads "0.0375" as **375** and ".5" as 5,
/// because a dot is Castilian's GROUP separator. That moves the silent
/// misreading from the OS locale to the app locale rather than removing it — a
/// farmer on a keypad offering only a dot would record 375 where they meant
/// 0,0375. Accepting either separator and refusing grouping is what makes the
/// reading unambiguous in every language the app speaks.
///
/// **The limit that buys**: `\d` is ASCII, so this assumes Latin digits. True
/// of every language the app ships and of every EU locale; adding one whose
/// numbering system is not Latin (Arabic-Indic, Persian, Han) means revisiting
/// this file — `toFieldText` would emit digits the regex rejects, and its own
/// output would stop round-tripping. Adobe's parser is the answer on the day
/// that happens, wrapped so both separators still work.
const NUMBER = /^[+-]?(\d+([.,]\d*)?|[.,]\d+)$/;

/// The active language's decimal separator, asked of Intl rather than hardcoded
/// — "," in Castilian and Catalan, "." in English, and whatever CLDR says for a
/// language the app has not met yet.
export function decimalSeparator() {
  const tag = formatTag();
  let found = separators.get(tag);
  if (!found) {
    found = new Intl.NumberFormat(tag).formatToParts(1.1).find((p) => p.type === "decimal").value;
    separators.set(tag, found);
  }
  return found;
}

/// The number as the field should SHOW it: the reader's separator, and every
/// digit the value actually has.
///
/// Deliberately not `formatNumber`, whose four-decimal cap is right for reading
/// and destructive for editing — it would turn a stored 0,00001 into "0" the
/// moment the field lost focus. 20 is Intl's own maximum and is what keeps this
/// lossless; it also avoids `String(value)` and its exponent notation.
export function toFieldText(value) {
  if (value === null || value === undefined || value === "") return "";
  const number = Number(value);
  if (!Number.isFinite(number)) return "";
  const tag = formatTag();
  let writer = writers.get(tag);
  if (!writer) {
    writer = new Intl.NumberFormat(tag, { maximumFractionDigits: 20, useGrouping: false });
    writers.set(tag, writer);
  }
  return writer.format(number);
}

/// Parse what the user typed, or pasted.
///
/// Returns `{ empty: true }` for nothing typed, `{ number }` for a value, or
/// `{ invalid: true }` — and the caller must treat `invalid` as BLOCKING rather
/// than as empty, because dropping an unparseable entry silently is the failure
/// this whole module exists to prevent.
export function fromFieldText(text) {
  const trimmed = String(text ?? "").trim();
  if (trimmed === "") return { empty: true };
  if (!NUMBER.test(trimmed)) return { invalid: true };
  // A trailing separator is a number still being typed ("1," on the way to
  // "1,5"), and Number("1.") is 1 — so it reads as the whole part rather than
  // blocking the form mid-keystroke.
  const number = Number(trimmed.replace(",", "."));
  return Number.isFinite(number) ? { number } : { invalid: true };
}
