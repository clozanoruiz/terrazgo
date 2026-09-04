// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The display side of the number policy. The parser has its own suite
// (lib/numberValue.test.js); this pins what a farmer READS.
//
// i18n.js is written for a browser and reads three globals at import. Stubbing
// them is cheaper than pulling in jsdom for a module that never touches a
// node — and it keeps the whole suite on the `node` environment.
import { beforeEach, describe, expect, it } from "vitest";

// Assigned here rather than in a beforeAll hook: i18n.js reads them while it is
// being imported, and the import below is awaited at module evaluation — which
// happens before any hook runs.
const store = new Map();
globalThis.localStorage = {
  getItem: (key) => store.get(key) ?? null,
  setItem: (key, value) => store.set(key, String(value)),
};
// navigator is a getter-only property on globalThis in Node, so it is defined
// rather than assigned.
Object.defineProperty(globalThis, "navigator", {
  value: { language: "es-ES" },
  configurable: true,
});
globalThis.document = { documentElement: {} };

const {
  formatNumber,
  formatPercent,
  formatCoordinates,
  formatDate,
  setLocale,
  setFormatMode,
  formatMode,
  languageTag,
  formatTag,
} = await import("./i18n.js");

// Most of what follows is about the app's own conventions, so it runs in
// "language" mode; the system-mode contract has its own block at the bottom.
beforeEach(async () => {
  setFormatMode("language");
  await setLocale("es");
});

describe("formatNumber", () => {
  it("uses the language's decimal separator", async () => {
    expect(formatNumber(1.5)).toBe("1,5");
    await setLocale("en");
    expect(formatNumber(1.5)).toBe("1.5");
    await setLocale("es");
  });

  it("never groups thousands, in any language", async () => {
    // The printed book has no thousands separator, and CLDR would otherwise
    // group Catalan at four digits where Castilian does not — so the two
    // co-official languages would disagree with each other.
    for (const locale of ["es", "ca", "en"]) {
      await setLocale(locale);
      expect(formatNumber(1234.5)).not.toMatch(/[\s.,]\d{3}[.,]/);
      expect(formatNumber(12000)).toBe("12000");
    }
    await setLocale("es");
  });

  it("keeps four decimals, so a dose is not restated", () => {
    // Two would print 0,0375 l/ha as "0,04".
    expect(formatNumber(0.0375)).toBe("0,0375");
  });

  it("NEVER renders a nonzero measurement as zero", () => {
    // A value too small for four decimals falls back to significant digits.
    // Rounding it to "0" would put a figure in front of an inspector that the
    // farmer never wrote — the same falsehood a blank cell exists to avoid.
    for (const tiny of [0.00003, -0.00003, 1e-7, 0.000049]) {
      expect(formatNumber(tiny), `${tiny} must not read as zero`).not.toMatch(/^-?0$/);
    }
    // Zero itself still reads as zero.
    expect(formatNumber(0)).toBe("0");
  });

  it("blanks a nullish value rather than writing a zero", () => {
    for (const empty of [null, undefined, ""]) expect(formatNumber(empty)).toBe("");
  });
});

describe("formatPercent", () => {
  it("puts the space where the language wants it", async () => {
    // Castilian and Catalan separate the sign; English does not.
    expect(formatPercent(42)).toMatch(/^42\s%$/u);
    await setLocale("en");
    expect(formatPercent(42)).toBe("42%");
    await setLocale("es");
  });
});

describe("formatCoordinates", () => {
  it("keeps five decimals and joins with a slash", () => {
    // Five decimals is about a metre. The joiner is not a comma because the
    // numbers themselves carry one: "41,65234, -4,72891" reads as four numbers.
    expect(formatCoordinates(41.65234, -4.72891)).toBe("41,65234 / -4,72891");
  });

  it("does not round a coordinate to the four decimals a dose uses", () => {
    expect(formatCoordinates(41.65234, -4.72891)).toContain("41,65234");
  });
});

describe("formatDate", () => {
  it("pads day and month, matching the printed book", async () => {
    // The locale's default width renders "3/8/2026" in Castilian against the
    // book's "03/08/2026".
    expect(formatDate("2026-08-03")).toBe("03/08/2026");
    await setLocale("ca");
    expect(formatDate("2026-08-03")).toBe("03/08/2026");
    await setLocale("es");
  });

  it("takes the field ORDER from the locale, not from the padding rule", async () => {
    await setLocale("en");
    // en-GB, not en-US: this is an EU product and English here is European.
    expect(languageTag()).toBe("en-GB");
    expect(formatDate("2026-08-03")).toBe("03/08/2026");
    await setLocale("es");
  });

  it("does not shift the day in timezones west of Greenwich", () => {
    // Parsed field-by-field: new Date("YYYY-MM-DD") would mean UTC midnight.
    expect(formatDate("2026-01-01")).toBe("01/01/2026");
  });
});

// --- the region setting ------------------------------------------------------

describe("formatMode", () => {
  it("defaults to the system, which is where most people already answered", () => {
    // Read before any beforeEach could have moved it: a fresh install with no
    // saved preference follows the machine.
    expect(["system", "language"]).toContain(formatMode());
  });

  it("switches what NUMBERS and DATES are formatted under", () => {
    setFormatMode("language");
    expect(formatTag()).toBe(languageTag());
    setFormatMode("system");
    // The host default, resolved through Intl rather than guessed from
    // navigator.language — the two can differ.
    expect(formatTag()).toBe(new Intl.NumberFormat().resolvedOptions().locale);
  });

  it("does NOT move plural selection, which belongs to the language", async () => {
    // Inflecting "día"/"días" is a fact about Castilian and nothing to do with
    // the reader's machine. Applying the host's rules to Spanish words would be
    // the bug this split exists to prevent.
    await setLocale("es");
    setFormatMode("system");
    const { t } = await import("./i18n.js");
    expect(t("product.phi_detail", { count: 1 })).toBe("plazo de seguridad 1 día");
    expect(t("product.phi_detail", { count: 4 })).toBe("plazo de seguridad 4 días");
  });

  it("ignores a value it does not recognise", () => {
    setFormatMode("language");
    setFormatMode("klingon");
    expect(formatMode()).toBe("language");
  });
});
