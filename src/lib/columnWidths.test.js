// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, it } from "vitest";
import {
  MIN_COLUMN_W,
  clampWidth,
  forgetWidths,
  readAll,
  readWidths,
  saveWidths,
} from "./columnWidths.js";

/// A localStorage stand-in. `node` is the test environment on purpose, so the
/// store is passed in rather than reached for — which is also why the real one
/// being blocked is easy to represent here.
function fakeStorage(initial = null) {
  let value = initial;
  return {
    getItem: () => value,
    setItem: (_key, next) => (value = next),
    get raw() {
      return value;
    },
  };
}

const blockedStorage = {
  getItem() {
    throw new Error("site data blocked");
  },
  setItem() {
    throw new Error("site data blocked");
  },
};

describe("clampWidth", () => {
  it("rounds to whole pixels", () => {
    expect(clampWidth(120.4)).toBe(120);
    expect(clampWidth(120.6)).toBe(121);
  });

  it("holds the floor", () => {
    expect(clampWidth(0)).toBe(MIN_COLUMN_W);
    expect(clampWidth(-500)).toBe(MIN_COLUMN_W);
  });

  // A NaN written into a style property drops the declaration silently, and the
  // column would return to its share with nothing to say why.
  it("refuses what is not a number", () => {
    expect(clampWidth(NaN)).toBeNull();
    expect(clampWidth("wide")).toBeNull();
    expect(clampWidth(undefined)).toBeNull();
    expect(clampWidth(Infinity)).toBeNull();
  });

  // Number(null), Number("") and Number([]) are all 0, so coercing first would
  // turn every one of these into a 48px column instead of a refusal. This test
  // caught exactly that.
  it("refuses the values JavaScript would coerce to zero", () => {
    expect(clampWidth(null)).toBeNull();
    expect(clampWidth("")).toBeNull();
    expect(clampWidth("   ")).toBeNull();
    expect(clampWidth([])).toBeNull();
    expect(clampWidth({})).toBeNull();
    expect(clampWidth(false)).toBeNull();
  });

  it("accepts a numeric string, which is what a style property reads back as", () => {
    expect(clampWidth("120")).toBe(120);
  });
});

describe("readAll", () => {
  it("is empty when nothing is stored", () => {
    expect(readAll(fakeStorage())).toEqual({});
  });

  it("is empty rather than throwing on malformed JSON", () => {
    expect(readAll(fakeStorage("{not json"))).toEqual({});
  });

  // An older release could have stored another shape entirely.
  it("is empty when the stored value is not an object", () => {
    expect(readAll(fakeStorage('"a string"'))).toEqual({});
    expect(readAll(fakeStorage("[1,2,3]"))).toEqual({});
  });

  it("survives a storage that throws", () => {
    expect(readAll(blockedStorage)).toEqual({});
    expect(readAll(undefined)).toEqual({});
  });
});

describe("readWidths", () => {
  it("returns the stored widths for a matching column count", () => {
    const store = fakeStorage(JSON.stringify({ operators: [100, 200, 300] }));
    expect(readWidths(store, "operators", 3)).toEqual([100, 200, 300]);
  });

  // The register gained or lost a column since these were stored; applying them
  // would misalign every one.
  it("refuses widths whose count no longer matches the table", () => {
    const store = fakeStorage(JSON.stringify({ operators: [100, 200, 300] }));
    expect(readWidths(store, "operators", 4)).toBeNull();
    expect(readWidths(store, "operators", 2)).toBeNull();
  });

  it("returns null for a table with nothing stored", () => {
    const store = fakeStorage(JSON.stringify({ operators: [100] }));
    expect(readWidths(store, "premises", 1)).toBeNull();
  });

  it("clamps what it reads back, so a stored value below the floor cannot win", () => {
    const store = fakeStorage(JSON.stringify({ operators: [10, 200] }));
    expect(readWidths(store, "operators", 2)).toEqual([MIN_COLUMN_W, 200]);
  });

  it("refuses a row holding anything unreadable", () => {
    const store = fakeStorage(JSON.stringify({ operators: [100, null] }));
    expect(readWidths(store, "operators", 2)).toBeNull();
  });
});

describe("saveWidths and forgetWidths", () => {
  it("keeps one table's widths without disturbing another's", () => {
    const store = fakeStorage(JSON.stringify({ premises: [50, 60] }));
    saveWidths(store, "operators", [100, 200]);
    expect(readAll(store)).toEqual({ premises: [50, 60], operators: [100, 200] });
  });

  it("replaces a table's previous widths", () => {
    const store = fakeStorage(JSON.stringify({ operators: [100, 200] }));
    saveWidths(store, "operators", [300, 400]);
    expect(readAll(store).operators).toEqual([300, 400]);
  });

  it("forgets one table and leaves the rest", () => {
    const store = fakeStorage(JSON.stringify({ operators: [1], premises: [2] }));
    forgetWidths(store, "operators");
    expect(readAll(store)).toEqual({ premises: [2] });
  });

  // Not being able to remember is not a reason to refuse the resize.
  it("does not throw when the store is blocked", () => {
    expect(() => saveWidths(blockedStorage, "operators", [100])).not.toThrow();
    expect(() => forgetWidths(blockedStorage, "operators")).not.toThrow();
  });
});
