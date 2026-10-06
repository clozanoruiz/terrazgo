// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The book-merging screens' words. What is pinned here is the part a screen
// could get quietly wrong: a book told apart from its twin by its dates, a
// date caption read as a date, a count per kind that follows the backend's
// order, and no guess at which other book is meant when there are several.
import { describe, expect, it, vi } from "vitest";

// The dictionary is stubbed to echo its key and parameters, so the test reads
// what each helper ASKED for rather than a translation.
vi.mock("../i18n.js", () => ({
  t: (key, params = {}) => `${key}(${Object.values(params).join("|")})`,
  tCode: (prefix, code) => `${prefix} ${code}`,
  formatDate: (iso) => `date:${iso}`,
  languageTag: () => "es-ES",
}));

const { bookItem, bookSpan, kindName, onlyOther, recordName, recordsByKind } =
  await import("./bookMerge.js");

const book = (id, label, starts, ends) => ({
  id,
  label,
  starts_on: starts,
  ends_on: ends,
});

describe("bookItem", () => {
  it("names a book by its label and its campaign's dates", () => {
    const item = bookItem(book("b", "2025/2026 bis", "2025-09-15", "2026-08-31"));
    expect(item.value).toBe("b");
    expect(item.label).toBe(
      "season.option(2025/2026 bis|season.span(date:2025-09-15|date:2026-08-31))",
    );
  });

  it("formats both ends of the campaign as dates", () => {
    expect(bookSpan(book("a", "2026", "2026-01-01", "2026-12-31"))).toBe(
      "season.span(date:2026-01-01|date:2026-12-31)",
    );
  });
});

describe("onlyOther", () => {
  it("starts on the other book when there is exactly one", () => {
    expect(onlyOther([book("x", "2026", "2026-01-01", "2026-12-31")])).toBe("x");
  });

  it("guesses nothing among several, or none", () => {
    const two = [
      book("x", "2026", "2026-01-01", "2026-12-31"),
      book("y", "2025", "2025-01-01", "2025-12-31"),
    ];
    expect(onlyOther(two)).toBe("");
    expect(onlyOther([])).toBe("");
  });
});

describe("recordsByKind", () => {
  it("counts each kind once, in the order the backend listed them", () => {
    const records = [
      { table: "harvest_record", id: "1" },
      { table: "sowing_record", id: "2" },
      { table: "sowing_record", id: "3" },
      { table: "treatment_record", id: "4" },
    ];
    expect(recordsByKind(records)).toEqual([
      { table: "harvest_record", n: 1 },
      { table: "sowing_record", n: 2 },
      { table: "treatment_record", n: 1 },
    ]);
  });

  it("is empty for no records", () => {
    expect(recordsByKind([])).toEqual([]);
  });
});

describe("recordName", () => {
  it("reads a caption holding a day as a date", () => {
    expect(recordName({ table: "sowing_record", caption: "2026-04-10" })).toBe(
      "Entity sowing_record · date:2026-04-10",
    );
  });

  it("prints any other caption as it is", () => {
    expect(recordName({ table: "crop", caption: "Trigo blando" })).toBe(
      "Entity crop · Trigo blando",
    );
  });

  it("is the kind alone when the register names nothing", () => {
    expect(recordName({ table: "register_declaration", caption: null })).toBe(
      "Entity register_declaration",
    );
  });
});

describe("kindName", () => {
  it("raises the first letter of a name written for mid-sentence", () => {
    expect(kindName("crop")).toBe("Entity crop");
  });
});
