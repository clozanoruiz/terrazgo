// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The numeric field's parser decides what number lands in a regulatory
// register, so it is business logic and gets the test-first treatment the
// project asks for — not the "UI, no unit tests" exemption, which covers views.
//
// These cases are what a farmer can actually put in the box: typed, pasted, or
// loaded back out of a record. `fromFieldText` IS the path the component takes
// (NumberInput calls it on the raw input value), so testing it directly tests
// the real thing — there is no mask layer in between whose behaviour could
// differ from the helper's.
import { describe, expect, it, vi } from "vitest";

// i18n.js reads localStorage and awaits its dictionaries at module load, which
// is neither available nor relevant here. Only the locale tag matters.
let tag = "es-ES";
vi.mock("../i18n.js", () => ({ formatTag: () => tag }));

const { fromFieldText, toFieldText, decimalSeparator } = await import("./numberValue.js");

/// Compact reading of a parse result, so a table row states one thing.
const read = (text) => {
  const r = fromFieldText(text);
  return r.empty ? "blank" : r.invalid ? "refused" : r.number;
};

describe("fromFieldText", () => {
  it.each([
    // Either separator is the decimal point. A farmer on a keypad offering
    // only a dot means one and a half whichever language the app is in.
    ["1,5", 1.5],
    ["1.5", 1.5],
    ["0,0375", 0.0375],
    ["0.0375", 0.0375],
    // Three digits after the separator is an everyday dose, not grouping.
    ["0,001", 0.001],
    ["1,234", 1.234],
    [",5", 0.5],
    [".5", 0.5],
    ["1234,5", 1234.5],
    ["12000", 12000],
    ["-4,72891", -4.72891],
    ["+3,5", 3.5],
    ["  2,5  ", 2.5],
    // Still being typed: "1," is on its way to "1,5" and must not block.
    ["1,", 1],
    ["1.", 1],
  ])("reads %j as %s", (input, expected) => {
    expect(read(input)).toBe(expected);
  });

  it.each([
    ["", "blank"],
    ["   ", "blank"],
  ])("treats %j as empty", (input, expected) => {
    expect(read(input)).toBe(expected);
  });

  it.each([
    // Grouped input is ambiguous and the app never renders it, so it is
    // refused rather than guessed at. This is the whole difference from the
    // native control, which reads "1,5" as 15 under an English OS.
    ["1.234,5"],
    ["1,234.5"],
    ["1 234,5"],
    ["1,2,3"],
    ["1..5"],
    // Never coerced by stripping: a masking library reads "1,5kg" as 1,5 and
    // "abc" as 0. In a register an inspector reads, silence is the defect.
    ["1,5kg"],
    ["abc"],
    ["1e5"],
    ["-"],
    [","],
    ["--1"],
  ])("refuses %j", (input) => {
    expect(read(input)).toBe("refused");
  });
});

describe("toFieldText", () => {
  it("renders the reader's separator", () => {
    tag = "es-ES";
    expect(toFieldText(1.5)).toBe("1,5");
    tag = "en-GB";
    expect(toFieldText(1.5)).toBe("1.5");
    tag = "es-ES";
  });

  it("blanks a nullish value rather than writing a zero", () => {
    // A printed 0 is a statement the farmer never made.
    for (const empty of [null, undefined, ""]) expect(toFieldText(empty)).toBe("");
  });

  it("keeps every digit the value has", () => {
    // NOT formatNumber's four-decimal cap: that is right for reading and
    // destructive for editing — it would turn a stored 0,00001 into "0" the
    // moment the field lost focus.
    expect(toFieldText(0.00001)).toBe("0,00001");
    expect(toFieldText(1e-7)).toBe("0,0000001");
  });

  it("never uses exponent notation", () => {
    expect(toFieldText(1e-7)).not.toContain("e");
  });
});

describe("the round trip", () => {
  it.each([1.5, 0.0375, 1234.5, 0.00001, 12000, -4.72891, 2, 0])(
    "reads back %s unchanged",
    (value) => {
      expect(fromFieldText(toFieldText(value)).number).toBe(value);
    },
  );

  // The property that matters more than any single case: whatever the
  // formatter writes, the parser must accept. Both sides are locale-driven, so
  // they can drift apart when a language is added — this is what would say so.
  it.each(["es-ES", "ca-ES", "en-GB"])("holds in %s", (locale) => {
    tag = locale;
    for (const value of [1.5, -4.72891, 0.0375, 1234.5, 12000, 0.00001]) {
      const written = toFieldText(value);
      expect(fromFieldText(written), `${locale} wrote ${written}`).toEqual({ number: value });
    }
    tag = "es-ES";
  });

  // The documented limit, pinned so it is a decision and not a surprise: the
  // parser assumes Latin digits. Every EU locale qualifies. Adding one that
  // does not means revisiting numberValue.js — Adobe's @internationalized/number
  // is the answer that day, wrapped so both separators still work.
  it("does NOT hold for a non-Latin numbering system", () => {
    tag = "ar-EG";
    expect(fromFieldText(toFieldText(1.5)).invalid).toBe(true);
    tag = "es-ES";
  });
});

describe("decimalSeparator", () => {
  it("comes from CLDR, not a hardcoded table", () => {
    tag = "es-ES";
    expect(decimalSeparator()).toBe(",");
    tag = "ca-ES";
    expect(decimalSeparator()).toBe(",");
    tag = "en-GB";
    expect(decimalSeparator()).toBe(".");
    tag = "es-ES";
  });
});
