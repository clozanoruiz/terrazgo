// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Frontend i18n layer. Dictionaries are plain JS modules (src/i18n/<code>.js),
// lazy-loaded via dynamic import(): only the active locale and the fallback
// are parsed at startup; others load on first switch. If a Rust-side consumer
// ever needs the strings (e.g. PDF report generation), convert them to JSON
// and share the files — the keys are the contract, not the container.

// Supported locales and their native names. Native names are shown
// untranslated in the language selector, so they must be known before any
// dictionary is loaded — hence a registry here rather than a dictionary key.
// Adding a language = one entry here + one src/i18n/<code>.js file.
//
// "Castellano", not "Español": that is what the language is called in Spain,
// where the co-official languages are Spanish too (CE art. 3).
//
// One constraint on what may be added here: the numeric field's parser assumes
// LATIN digits (`lib/numberValue.js`). Every EU locale qualifies; a language
// written with Arabic-Indic, Persian or Han numerals does not, and adding one
// means revisiting that file first.
const SUPPORTED = {
  ca: "Català",
  en: "English",
  es: "Castellano",
};

// Used when neither a saved preference nor the OS language matches a
// supported locale. To change it (e.g. to "en"), edit this one constant.
const FALLBACK_LOCALE = "es";

// The regional tag each language formats and sorts under. A bare tag is not
// enough for two different reasons, and both bite: "en" resolves to US in Intl,
// so an English user would read 08/03/2026 where every other locale in the app
// reads 03/08/2026 — and English here means European English, this being an EU
// product. "es" and "ca" resolve sensibly bare, but naming them keeps the three
// in one visible list rather than leaving two implicit and one special-cased.
//
// Read by formatDate and formatNumber below, by the owned date/time controls and
// by Intl.Collator in lib/collate.js — so a value the app renders and a value it
// lets you edit can never disagree. Named for formatting rather than for dates:
// every Intl consumer in the app resolves through it.
const FORMAT_LOCALE = {
  ca: "ca-ES",
  en: "en-GB",
  es: "es-ES",
};

// localStorage, not the database: display language is a per-device preference.
// Migrate into the core settings table when that exists, if it should roam.
const STORAGE_KEY = "terrazgo.locale";

// Which convention numbers and dates follow. Two values, and the default is
// the one every operating system already sets:
//
//   "system"   — the machine's regional format (the default)
//   "language" — the language chosen above
//
// They are SEPARATE questions, which is why this is a second key rather than a
// mode of the first: a Castilian-speaking farmer on an English-configured phone
// may want commas on screen to match the book, and a bilingual one reading the
// app in English on a Spanish machine may want their own conventions. Most
// people never open it, which is why it defaults to the OS.
//
// What this DOES NOT touch: plural selection and name ordering. Those inflect
// and collate the app's own words, so they follow the LANGUAGE — see
// languageTag() below. Getting that wrong would apply Polish plural rules to
// Spanish strings on a Polish-configured machine.
const FORMAT_KEY = "terrazgo.format";
const FORMAT_MODES = ["system", "language"];
const DEFAULT_FORMAT_MODE = "system";

const listeners = [];
const loaded = {};
let current = detect();
let format = detectFormat();

function detectFormat() {
  const saved = localStorage.getItem(FORMAT_KEY);
  return FORMAT_MODES.includes(saved) ? saved : DEFAULT_FORMAT_MODE;
}

function detect() {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved !== null && SUPPORTED[saved]) return saved;
  // navigator.language reflects the OS locale in the Tauri webview
  // (e.g. "es-ES"); only the primary subtag decides the dictionary.
  const os = (navigator.language || "").split("-")[0].toLowerCase();
  return SUPPORTED[os] ? os : FALLBACK_LOCALE;
}

async function load(code) {
  if (!loaded[code]) {
    loaded[code] = (await import(`./i18n/${code}.js`)).default;
  }
}

export function locale() {
  return current;
}

export function locales() {
  return Object.keys(SUPPORTED);
}

export function nativeName(code) {
  return SUPPORTED[code];
}

// Async because switching may load a not-yet-seen dictionary; a failed load
// (a locale registered in SUPPORTED but missing its file — a packaging bug)
// rejects without changing the current locale.
export async function setLocale(code) {
  if (!SUPPORTED[code] || code === current) return;
  await load(code);
  current = code;
  localStorage.setItem(STORAGE_KEY, code);
  document.documentElement.lang = code;
  for (const listener of listeners) listener(code);
}

// Register a callback for locale switches; App.svelte uses it to remount the
// routed content so every t() call re-evaluates.
export function onLocaleChange(listener) {
  listeners.push(listener);
}

// True if the key exists in the active or fallback dictionary. Lets callers
// choose between a translated string and their own fallback without tripping
// t()'s missing-key warning (see errorText in lib/backend.js).
export function has(key) {
  return loaded[current][key] !== undefined || loaded[FALLBACK_LOCALE][key] !== undefined;
}

// Built per locale and reused, the collate.js reasoning: constructing the Intl
// object is the expensive part and a list re-renders on every reactive update.
const pluralRules = new Map();

function pluralCategory(count) {
  // languageTag, NOT formatTag: this chooses between "día" and "días", which
  // is a fact about Castilian and nothing to do with the reader's machine.
  const tag = languageTag();
  let found = pluralRules.get(tag);
  if (!found) {
    found = new Intl.PluralRules(tag);
    pluralRules.set(tag, found);
  }
  // A CLDR category name: "one"/"other" in the three languages shipped today,
  // and "zero"/"two"/"few"/"many" in the ones EU expansion would bring.
  return found.select(count);
}

// Active locale → fallback, trying the plural variant first at each step. A key
// with no variants misses and falls through to its bare string, so passing a
// count to an unpluralized key is harmless.
//
// The category is chosen with the ACTIVE locale's rules even when the string
// comes from the fallback. That can only disagree once a locale with more than
// two categories exists, and the i18n contract test refuses a locale missing
// any category its own language requires.
function resolve(key, count) {
  if (typeof count === "number") {
    const variant = `${key}.${pluralCategory(count)}`;
    const found = loaded[current][variant] ?? loaded[FALLBACK_LOCALE][variant];
    if (found !== undefined) return found;
  }
  return loaded[current][key] ?? loaded[FALLBACK_LOCALE][key];
}

// `params.count` both selects the plural form and interpolates as {count}. One
// count per key: a sentence whose nouns inflect on two different numbers is
// written in the "Label: N" form instead (docs/frontend-conventions.md).
export function t(key, params = {}) {
  const text = resolve(key, params.count);
  if (text === undefined) {
    console.warn(`i18n: missing key "${key}"`);
    return key;
  }
  return text.replace(/\{(\w+)\}/g, (whole, name) => params[name] ?? whole);
}

// Translate a schema code (alert type, status, table name…) under a key
// prefix. Unknown codes fall back to the raw code so a new schema value
// degrades to e.g. "frost_risk" instead of "alert.type.frost_risk".
//
// `count` is for the few codes that are counted NOUNS rather than symbols —
// "1 trampa" beside "2 trampas", where "1 l/ha" and "2 l/ha" are the same word.
// Only those codes carry variants; every other one misses and falls through.
//
// Called WITHOUT a count — a picker listing the code as an option — a
// pluralized label resolves to its `other` form, the citation form such a list
// should show ("trampas", not the raw "traps").
export function tCode(prefix, code, count) {
  const key = `${prefix}.${code}`;
  return resolve(key, count) ?? resolve(`${key}.other`) ?? code;
}

// The tag the app's own LANGUAGE resolves to. Read by anything that operates on
// the app's words rather than on a reader's conventions: plural selection, and
// Intl.Collator in lib/collate.js — names sort by the language being read,
// whatever the machine is set to. That matches the book whenever the app is
// being read in the language the book prints in, which is the common case; an
// English reader of a Castilian book gets English ordering on screen, and the
// book keeps its own.
export function languageTag() {
  return FORMAT_LOCALE[current] ?? current;
}

// The machine's own regional format, resolved through Intl rather than guessed
// from navigator.language: the two can differ, and this is the one the runtime
// will actually format with. Undefined locale = "the host default".
let systemTagCache = null;

function systemTag() {
  systemTagCache ??= new Intl.NumberFormat().resolvedOptions().locale;
  return systemTagCache;
}

/// Which convention numbers and dates follow: "system" or "language".
export function formatMode() {
  return format;
}

export function formatModes() {
  return [...FORMAT_MODES];
}

/// Switch it, persist it, and tell the shell to re-render — the same listeners
/// setLocale fires, because every rendered figure on screen has just changed.
export function setFormatMode(mode) {
  if (!FORMAT_MODES.includes(mode) || mode === format) return;
  format = mode;
  localStorage.setItem(FORMAT_KEY, mode);
  for (const listener of listeners) listener(current);
}

// The tag NUMBERS and DATES are formatted under, and the one the owned
// date/time/number controls parse with — so a figure the app renders and a
// figure it lets you type can never disagree.
export function formatTag() {
  return format === "language" ? languageTag() : systemTag();
}

// Built per locale and reused, the collate.js reasoning: constructing an Intl
// object is the expensive part and a list re-renders on every reactive update.
const dateFormats = new Map();
const numberFormats = new Map();

function intl(cache, key, build) {
  let found = cache.get(key);
  if (!found) {
    found = build();
    cache.set(key, found);
  }
  return found;
}

// Locale-aware rendering of a date-only ISO string (YYYY-MM-DD). Parsed
// field-by-field: new Date("YYYY-MM-DD") would mean UTC midnight and could
// render the previous day in timezones west of Greenwich.
//
// Two-digit day and month rather than the locale's default width, for the same
// reason formatNumber fixes its decimals: the printed book writes dd/mm/yyyy
// unconditionally, and the bare default renders "3/8/2026" in Castilian and
// Catalan against the book's "03/08/2026". The ORDER of the fields still comes
// from the locale — this pins the padding, not the convention.
const DATE_PARTS = { day: "2-digit", month: "2-digit", year: "numeric" };

export function formatDate(isoDate) {
  const [year, month, day] = isoDate.split("-").map(Number);
  const tag = formatTag();
  return intl(dateFormats, tag, () => new Intl.DateTimeFormat(tag, DATE_PARTS)).format(
    new Date(year, month - 1, day),
  );
}

// Locale-aware number for display. The decimal separator is a comma in
// Castilian and Catalan and a point in English, so a hardcoded one is wrong in
// whichever language it was not written for. Trailing zeros are trimmed — a
// cover is "2 m" wide, not "2,00 m".
//
// The two options are a policy, not preferences: they hold this function to the
// SAME PRECISION as the printed book's format_number
// (crates/terrazgo-recordbook/src/lib.rs), so the two never show a different
// figure — only, at most, a different separator.
//
// They are not the same artifact and do not always read alike. The book prints
// in the HOLDING's language (Castilian, or a co-official one where the province
// makes it so) whatever the app is showing, so an English reader gets "1234.5"
// on screen against "1234,5" on the printout. That is correct: the screen
// serves whoever is using the app, the book is the legal document. What must
// never differ is the DIGITS — same decimals, same grouping, same refusal to
// round a small value into "0" — because a farmer checking a figure against
// their own printout is comparing the number, not the punctuation.
//
//   - Four decimals, because two SILENTLY FALSIFIES a regulatory value: a dose
//     of 0,0375 l/ha would print as "0,04". Callers wanting fewer pass fewer.
//     Four is an APP convention, not a regulatory one — no decree states a
//     precision. It is enough because the units already scale (a dose is
//     written in g/ha rather than kg/ha), and coordinates, which genuinely need
//     more, have their own formatter.
//   - No grouping, because the book has no thousands separator. Leaving it on
//     also made the two co-official languages disagree with each other — CLDR
//     gives Castilian minimumGroupingDigits=2 and Catalan 1, so 1234,5 grouped
//     in Catalan and not in Castilian.
//
// A nullish value formats as blank, never as "0". Intl coerces null to zero,
// and a printed 0 is a statement the farmer never made — the same rule the
// book's `amount` and the spreadsheet's `Cell::Empty` follow. Callers that
// already guard lose nothing; the ones that forget degrade to a blank cell
// instead of inventing a measurement.
export function formatNumber(value, maximumFractionDigits = 4) {
  if (value === null || value === undefined || value === "") return "";
  const tag = formatTag();
  const written = intl(
    numberFormats,
    `${tag}:${maximumFractionDigits}`,
    () => new Intl.NumberFormat(tag, { maximumFractionDigits, useGrouping: false }),
  ).format(value);
  // A nonzero measurement must never render as "0" — that is a figure the
  // farmer never wrote, the same falsehood a blank cell exists to avoid. A
  // value too small for the requested decimals falls back to significant
  // digits, which is what the record book's format_number does in Rust.
  if (Number(value) !== 0 && /^-?0$/.test(written)) {
    return intl(
      numberFormats,
      `${tag}:sig`,
      () => new Intl.NumberFormat(tag, { maximumSignificantDigits: 2, useGrouping: false }),
    ).format(value);
  }
  return written;
}

// A number with a CLDR unit identifier ("gigabyte", "megabyte", …), for the
// few quantities whose unit is the app's own rather than the farmer's. Domain
// units stay on tCode("unit", …): those are regulatory symbols that print
// verbatim in every language, which is the opposite of what this does.
export function formatUnit(value, unit, maximumFractionDigits = 1) {
  const tag = formatTag();
  return intl(
    numberFormats,
    `${tag}:${unit}:${maximumFractionDigits}`,
    () =>
      new Intl.NumberFormat(tag, {
        style: "unit",
        unit,
        unitDisplay: "short",
        maximumFractionDigits,
        useGrouping: false,
      }),
  ).format(value);
}

// A percentage held as 0–100 (the shape every column in the schema uses), not
// as the 0–1 fraction Intl expects — hence the division.
//
// Worth going through Intl rather than appending "%": Castilian and Catalan put
// a space before the sign and English does not, so the hand-built form was wrong
// in two of the three languages. Grouping is left at the Intl default here
// because a percentage cannot reach four digits.
export function formatPercent(value, maximumFractionDigits = 0) {
  const tag = formatTag();
  return intl(
    numberFormats,
    `${tag}:pct:${maximumFractionDigits}`,
    () => new Intl.NumberFormat(tag, { style: "percent", maximumFractionDigits }),
  ).format(value / 100);
}

// MONEY, when the Costs module brings it. There is no formatter here yet
// because there is no money field yet — but the rule is written down now,
// because it is the one a region setting makes easy to get wrong:
//
//   **The format locale is the reader's. The CURRENCY is the record's.**
//
// A euro amount is a euro amount whoever opens the book. So a money formatter
// takes the currency code from the DATA and passes it explicitly —
// `new Intl.NumberFormat(formatTag(), { style: "currency", currency: code })` —
// and never lets it be inferred from the locale, which would print a Spanish
// holding's costs in dollars for a reader whose machine is set to the US.
// What may follow the reader is only the convention around the figure: es-ES
// writes "1.234,56 €" where en-US writes "€1,234.56", and both name euros.
//
// No separate currency SETTING is needed for the same reason: the currency is
// data, not a preference. What the holding trades in belongs on the farm or the
// record, beside the integer cents the schema already specifies.
//
// The INPUT stays a plain number, the symbol shown as a label beside it: a
// farmer types 12,34 and the form multiplies by 100 into cents. Nothing types a
// currency symbol. Money is also what will want a fixed SCALE of two decimals —
// NumberInput carries no such bound today, deliberately, because no decree
// bounds a register's precision and bounding it once refused the five-decimal
// coordinates the book itself prints.

// A WGS84 decimal-degrees pair, mirroring the book's format_coordinates.
//
// Five decimals is about a metre, which is what locating a wellhead needs and
// what formatNumber's four would not give. Joined by " / " rather than a comma
// because the numbers themselves carry a decimal comma in two of the three
// languages — "41,65234, -4,72891" reads as four numbers.
export function formatCoordinates(latitude, longitude) {
  return `${formatNumber(latitude, 5)} / ${formatNumber(longitude, 5)}`;
}

// Top-level await: the module graph (and therefore main.js, which imports
// this file) does not execute until the active and fallback dictionaries are
// ready, which is what lets t() stay synchronous everywhere.
await Promise.all([load(current), load(FALLBACK_LOCALE)]);
document.documentElement.lang = current;
