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
const SUPPORTED = {
  en: "English",
  es: "Español",
};

// Used when neither a saved preference nor the OS language matches a
// supported locale. To change it (e.g. to "en"), edit this one constant.
const FALLBACK_LOCALE = "es";

// localStorage, not the database: display language is a per-device preference.
// Migrate into the core settings table when that exists, if it should roam.
const STORAGE_KEY = "terrazgo.locale";

const listeners = [];
const loaded = {};
let current = detect();

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

export function t(key, params = {}) {
  const text = loaded[current][key] ?? loaded[FALLBACK_LOCALE][key];
  if (text === undefined) {
    console.warn(`i18n: missing key "${key}"`);
    return key;
  }
  return text.replace(/\{(\w+)\}/g, (whole, name) => params[name] ?? whole);
}

// Translate a schema code (alert type, status, table name…) under a key
// prefix. Unknown codes fall back to the raw code so a new schema value
// degrades to e.g. "frost_risk" instead of "alert.type.frost_risk".
export function tCode(prefix, code) {
  const key = `${prefix}.${code}`;
  return loaded[current][key] ?? loaded[FALLBACK_LOCALE][key] ?? code;
}

// Locale-aware rendering of a date-only ISO string (YYYY-MM-DD). Parsed
// field-by-field: new Date("YYYY-MM-DD") would mean UTC midnight and could
// render the previous day in timezones west of Greenwich.
export function formatDate(isoDate) {
  const [year, month, day] = isoDate.split("-").map(Number);
  return new Date(year, month - 1, day).toLocaleDateString(current);
}

// Top-level await: the module graph (and therefore main.js, which imports
// this file) does not execute until the active and fallback dictionaries are
// ready, which is what lets t() stay synchronous everywhere.
await Promise.all([load(current), load(FALLBACK_LOCALE)]);
document.documentElement.lang = current;
