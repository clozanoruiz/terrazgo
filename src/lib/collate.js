// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Ordering names the way the active language does.
//
// Lists of user data arrive from Rust ordered by SQL `ORDER BY name`, and
// SQLite's default BINARY collation orders by CODE POINT: `Á` is U+00C1 and `Z`
// is U+005A, so an advisor called "Ángel" sat below "Zubiri", and "Parcela 10"
// came before "Parcela 2". The SQL order stays as it is — it gives tests and
// non-UI consumers something deterministic — and DISPLAY order is decided here.
//
// Deliberately mirrors crates/terrazgo-recordbook/src/collate.rs: the same CLDR
// data through Intl, numeric ordering on, and accents distinguished rather than
// folded, so a picker on screen and a cell in the printed book agree.
//
// Framework-agnostic tier: no Svelte import (docs/frontend-conventions.md).
import { localeTag } from "../i18n.js";

// Built per locale and reused: constructing a collator is the expensive part,
// comparing with it is not, and a list re-sorts on every reactive update.
const collators = new Map();

function collator() {
  const tag = localeTag();
  let found = collators.get(tag);
  if (!found) {
    found = new Intl.Collator(tag, {
      // "Parcela 2" before "Parcela 10" — digit runs compare by value.
      numeric: true,
      // NOT sensitivity:"base". Base would call "Pena" and "Peña" EQUAL and
      // leave their order to chance; the record book's collator distinguishes
      // them (ICU tertiary strength), and the two must not disagree. Base
      // sensitivity belongs to searching, not to sorting.
      sensitivity: "variant",
    });
    collators.set(tag, found);
  }
  return found;
}

/// Compare two display strings. Nullish values sort as empty rather than
/// throwing, because a name column is occasionally blank.
export function compareText(a, b) {
  return collator().compare(a ?? "", b ?? "");
}

/// A locale-ordered copy of `rows`, keyed by `name(row)`.
///
/// Returns a new array: these lists are `$state` and sorting in place would
/// mutate the very value a `$derived` is reading.
export function sortedBy(rows, name) {
  return [...rows].sort((a, b) => compareText(name(a), name(b)));
}

// --- searching ---------------------------------------------------------------
//
// Folding lives beside ordering because the two are deliberately DIFFERENT, and
// keeping them apart in separate files would hide that. Sorting distinguishes
// accents ("Pena" before "Peña"); searching folds them away, because in a
// filter box over-matching is forgiving where under-matching is frustrating.
// Intl.Collator is the right tool for the first and cannot do the second: it
// orders and compares whole strings, and has no containment operator.

// Stroke, bar and ligature letters that NFKD leaves alone because they carry no
// combining mark at all. Spanish and Catalan need none of them (ç and ñ both
// decompose); the EU expansion this project is designed for does.
const ATOMIC = { ø: "o", ł: "l", đ: "d", ð: "d", þ: "th", ħ: "h", ŧ: "t", æ: "ae", œ: "oe" };

/// Case- and accent-insensitive form of a string, for matching only.
///
/// `\p{M}` rather than the U+0300–U+036F block: that block is one of five, and
/// misses Greek, Cyrillic, Hebrew, Arabic and Vietnamese marks. Uppercase
/// rather than lowercase because it COLLAPSES more, which is what a fold wants
/// — "Straße" folds to STRASSE so ß matches ss, and Greek ς and σ both become
/// Σ. Invariant `toUpperCase`, never `toLocaleUpperCase`: Turkish maps i to İ,
/// which would stop i matching I for every other language.
export function fold(text) {
  return (text ?? "")
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .replace(/[øłđðþħŧæœ]/gi, (c) => ATOMIC[c.toLowerCase()])
    .toUpperCase();
}

/// Rank of `folded` against a folded query: lower is better, -1 is no match.
///
/// Ranking is what makes a row cap safe. With 200 rows containing "cali", the
/// cap decides which 40 a farmer sees, and unranked that is a coin toss.
function rank(folded, tokens) {
  let best = 3;
  for (const token of tokens) {
    const at = folded.indexOf(token);
    if (at < 0) return -1; // token-AND: every token must appear somewhere
    // exact > starts-with > starts a word > appears anywhere
    const here = folded === token ? 0 : at === 0 ? 1 : folded[at - 1] === " " ? 2 : 3;
    best = Math.min(best, here);
  }
  return best;
}

/// Filter `items` by `query`, best matches first, capped.
///
/// Substring anywhere rather than prefix, so "cali" finds CALI, CÁLIDO and
/// ALCALI; token-AND on whitespace, so "olivo verde" finds "VERDE OLIVO".
/// Deliberately NOT fuzzy subsequence matching: in registers whose codes carry
/// legal weight, ranking a plausible-but-wrong pest above the right one is a
/// worse failure than asking for an accurate substring.
export function searchItems(items, query, cap) {
  const tokens = fold(query).split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return { visible: items.slice(0, cap), total: items.length };
  const scored = [];
  for (const item of items) {
    // item.folded is precomputed by the caller — normalize() is not cheap and
    // the biggest catalogue behind these pickers is 2 498 rows.
    const score = rank(item.folded ?? fold(item.label), tokens);
    if (score >= 0) scored.push({ item, score });
  }
  scored.sort((a, b) => a.score - b.score || compareText(a.item.label, b.item.label));
  return { visible: scored.slice(0, cap).map((s) => s.item), total: scored.length };
}
