// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The whole locale matrix in one place: what a farmer types, what is stored,
// what comes back into the field, and what the screen reads — across every app
// language and both format modes, with the host locale agreeing and disagreeing.
//
// Deliberately an INTEGRATION suite: i18n.js and lib/numberValue.js are both
// imported for real so they share the one format mode, which is the coupling
// that matters. Mocking either would test the mock. The per-module suites
// (i18n.test.js, lib/numberValue.test.js) cover their own edges.
//
// Expectations are DERIVED from the host rather than hardcoded, because CI and
// a developer's laptop do not report the same locale — hardcoding "en-GB" here
// would make the suite pass or fail on where it ran rather than on the code.
import { beforeEach, describe, expect, it } from "vitest";

const store = new Map();
globalThis.localStorage = {
  getItem: (key) => store.get(key) ?? null,
  setItem: (key, value) => store.set(key, String(value)),
};
Object.defineProperty(globalThis, "navigator", {
  value: { language: "es-ES" },
  configurable: true,
});
globalThis.document = { documentElement: {} };

const { formatNumber, formatDate, setLocale, setFormatMode, formatTag, languageTag, t } =
  await import("./i18n.js");
const { fromFieldText, toFieldText } = await import("./lib/numberValue.js");

/// The decimal separator a tag actually uses, asked of Intl so the test states
/// no opinion of its own.
const sep = (tag) =>
  new Intl.NumberFormat(tag).formatToParts(1.1).find((p) => p.type === "decimal").value;

const HOST = new Intl.NumberFormat().resolvedOptions().locale;
const LANGUAGES = ["es", "ca", "en"];
const MODES = ["language", "system"];

beforeEach(async () => {
  setFormatMode("language");
  await setLocale("es");
});

describe("the format tag each mode resolves to", () => {
  it.each(LANGUAGES)("in %s", async (language) => {
    await setLocale(language);
    setFormatMode("language");
    expect(formatTag()).toBe(languageTag());
    setFormatMode("system");
    expect(formatTag()).toBe(HOST);
  });
});

describe("what the farmer types reaches storage as one number", () => {
  // Every combination, because the defect this replaced was exactly a
  // combination one: app language and OS locale disagreeing.
  for (const language of LANGUAGES) {
    for (const mode of MODES) {
      it(`${language} / ${mode}: a comma and a dot both mean one and a half`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        // The native control read "1,5" as 15 under an English OS. Neither
        // spelling may depend on the machine, or on which language is showing.
        expect(fromFieldText("1,5")).toEqual({ number: 1.5 });
        expect(fromFieldText("1.5")).toEqual({ number: 1.5 });
        expect(fromFieldText("0,0375")).toEqual({ number: 0.0375 });
        expect(fromFieldText("0.0375")).toEqual({ number: 0.0375 });
      });

      it(`${language} / ${mode}: what is stored is a NUMBER, never text`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        // Rust receives f64 over the wire; a localised string would arrive as
        // a string and be rejected or, worse, coerced.
        expect(typeof fromFieldText("1,5").number).toBe("number");
      });

      it(`${language} / ${mode}: ambiguity is refused rather than guessed`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        for (const bad of ["1.234,5", "1,234.5", "1,5kg", "abc"]) {
          expect(fromFieldText(bad).invalid, `${bad} must be refused`).toBe(true);
        }
      });
    }
  }
});

describe("what is stored comes back into the field unchanged", () => {
  for (const language of LANGUAGES) {
    for (const mode of MODES) {
      it(`${language} / ${mode}: round trip is lossless`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        for (const value of [1.5, 0.0375, 1234.5, 0.00001, 12000, -4.72891, 41.65234, 0]) {
          expect(fromFieldText(toFieldText(value)).number, `${value}`).toBe(value);
        }
      });

      it(`${language} / ${mode}: the field shows the FORMAT tag's separator`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        // What the field shows and what it accepts must be the same
        // convention, or a farmer retyping what they see would be refused.
        expect(toFieldText(1.5)).toBe(`1${sep(formatTag())}5`);
      });
    }
  }
});

describe("what the screen reads", () => {
  for (const language of LANGUAGES) {
    for (const mode of MODES) {
      it(`${language} / ${mode}: figures follow the format tag`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        expect(formatNumber(1.5)).toBe(`1${sep(formatTag())}5`);
        expect(formatNumber(0.0375)).toBe(`0${sep(formatTag())}0375`);
      });

      it(`${language} / ${mode}: no thousands separator, ever`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        expect(formatNumber(12000)).toBe("12000");
        expect(formatNumber(1234.5)).toBe(`1234${sep(formatTag())}5`);
      });

      it(`${language} / ${mode}: a nonzero measurement never reads as zero`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        expect(formatNumber(0.00003)).not.toMatch(/^-?0$/);
      });

      it(`${language} / ${mode}: the field and the list agree`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        // Same value, two renderers: the editable field and the read-only list
        // must not disagree about the separator.
        expect(toFieldText(2.5)[1]).toBe(formatNumber(2.5)[1]);
      });
    }
  }
});

describe("the words never follow the machine", () => {
  it.each(MODES)("in %s mode, Castilian prose stays Castilian", async (mode) => {
    await setLocale("es");
    setFormatMode(mode);
    // The whole point of the two-tag split. Plural selection inflects the
    // app's own words and must not consult the host.
    expect(t("product.phi_detail", { count: 1 })).toBe("plazo de seguridad 1 día");
    expect(t("product.phi_detail", { count: 4 })).toBe("plazo de seguridad 4 días");
    expect(t("form.save")).toBe("Guardar");
  });

  it.each(MODES)("in %s mode, dates keep the book's padding", async (mode) => {
    await setLocale("es");
    setFormatMode(mode);
    // The ORDER may follow the host; the zero padding is pinned either way, so
    // a day never reads "3/8/2026" against the book's "03/08/2026".
    expect(formatDate("2026-08-03")).toMatch(/^\d{2}\D\d{2}\D\d{4}$/);
  });
});

describe("the host locale agreeing and disagreeing with the app language", () => {
  it("when they DISAGREE, figures and words come from different places", async () => {
    // The case that produced the original defect: a Castilian book on an
    // English-configured machine.
    await setLocale("es");
    setFormatMode("system");
    expect(t("form.save")).toBe("Guardar");
    expect(formatNumber(1.5)).toBe(`1${sep(HOST)}5`);
    // And the farmer can still type either separator.
    expect(fromFieldText("1,5").number).toBe(1.5);
    expect(fromFieldText("1.5").number).toBe(1.5);
  });

  it("differ exactly when their tags differ, and never otherwise", async () => {
    // Stated as an equivalence rather than "pick the language that matches
    // this machine", so it asserts something on any host: CI and a laptop do
    // not report the same locale, and a test that quietly skips on one of them
    // is worse than no test. It also rules out a third behaviour — the modes
    // must not diverge for any reason other than their tags.
    for (const language of LANGUAGES) {
      await setLocale(language);
      setFormatMode("language");
      const inLanguage = formatNumber(1234.5);
      setFormatMode("system");
      const inSystem = formatNumber(1234.5);
      expect(inLanguage === inSystem, `${language}: ${inLanguage} vs ${inSystem}`).toBe(
        sep(languageTag()) === sep(HOST),
      );
    }
  });

  it("switching the mode changes the figure but not the value", async () => {
    await setLocale("es");
    setFormatMode("language");
    const spanish = toFieldText(1234.5);
    setFormatMode("system");
    const host = toFieldText(1234.5);
    // Different text, same number underneath — which is what makes the setting
    // a display preference and not a data migration.
    expect(fromFieldText(spanish).number).toBe(1234.5);
    expect(fromFieldText(host).number).toBe(1234.5);
  });
});

describe("the screen and the printed book", () => {
  // These are the SAME vectors as
  // `numbers_render_with_decimal_comma_and_no_trailing_zeros` and
  // `a_nonzero_measurement_never_prints_as_zero` in
  // crates/terrazgo-recordbook/src/lib.rs.
  //
  // The two are NOT the same artifact. The book prints in the holding's
  // language — Castilian, or a co-official one where the province makes it so —
  // whatever the app is showing, and an English UI falls through to Castilian
  // (terrazgo-recordbook/src/region.rs). So the reader of an English app sees a
  // dot where the printout has a comma, and that is correct rather than a
  // defect: the screen serves whoever is using the app, the book is the legal
  // document. What must never differ is the DIGITS.
  //
  // Mirrored by hand, the way collate.js mirrors collate.rs: there is no shared
  // fixture because the two run in different languages, so what keeps them
  // together is that each names the other and both lists are spelled out.
  const BOOK = [
    [1.5, "1,5"],
    [2, "2"],
    [0.0375, "0,0375"],
    [12.25, "12,25"],
    [1234.5, "1234,5"],
    [12000, "12000"],
    [1.23456, "1,2346"],
    // The values that would round into a figure nobody wrote.
    [0.00003, "0,00003"],
    [-0.00003, "-0,00003"],
    [0.0000001, "0,0000001"],
    [0, "0"],
  ];

  // Read in Castilian, the two coincide completely — the case a farmer checking
  // a figure against their own printout is actually in.
  it.each(BOOK)("renders %s exactly as the book prints it", async (value, printed) => {
    await setLocale("es");
    setFormatMode("language");
    expect(formatNumber(value)).toBe(printed);
  });

  // Read in any language, under either mode, the DIGITS still match: same
  // decimals, same absence of grouping, same refusal to round a small value
  // into "0". Only the separator is the reader's own.
  const digitsOf = (text) => text.replace(/[.,]/g, "");

  for (const language of LANGUAGES) {
    for (const mode of MODES) {
      it(`${language} / ${mode}: same digits as the book, whatever the separator`, async () => {
        await setLocale(language);
        setFormatMode(mode);
        for (const [value, printed] of BOOK) {
          expect(digitsOf(formatNumber(value)), `${value} in ${language}/${mode}`).toBe(
            digitsOf(printed),
          );
        }
      });
    }
  }

  it("an English reader sees a dot where the printout has a comma", async () => {
    // Stated so the divergence is a recorded decision and not a surprise
    // someone later "fixes" by forcing the screen into the book's language.
    await setLocale("en");
    setFormatMode("language");
    expect(formatNumber(1234.5)).toBe("1234.5");
    // The book, meanwhile, prints 1234,5 — pinned on the Rust side.
    expect(formatNumber(1234.5).replace(".", ",")).toBe("1234,5");
  });

  it("agrees on dates too", async () => {
    await setLocale("es");
    setFormatMode("language");
    // The book's format_date is dd/mm/yyyy unconditionally.
    expect(formatDate("2026-05-01")).toBe("01/05/2026");
  });
});
