// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Ordering and searching. Both are decisions rather than conveniences: sorting
// must agree with the record book's Rust collator, and searching must not rank
// a plausible-but-wrong pest above the right one in a 600-entry catalogue.
import { describe, expect, it, vi } from "vitest";

let tag = "es-ES";
vi.mock("../i18n.js", () => ({ languageTag: () => tag }));

const { compareText, sortedBy, fold, searchItems } = await import("./collate.js");

const order = (names) => [...names].sort(compareText);

describe("compareText", () => {
  it("files accents where the language files them, not by code point", () => {
    // SQL returns BINARY order, where "Á" is U+00C1 and lands after "Z".
    expect(order(["Zubiri", "Ángel"])).toEqual(["Ángel", "Zubiri"]);
  });

  it("orders digit runs by value", () => {
    expect(order(["Parcela 10", "Parcela 2"])).toEqual(["Parcela 2", "Parcela 10"]);
  });

  it("distinguishes accents rather than folding them", () => {
    // NOT sensitivity:"base", which would call these EQUAL and leave their
    // order to chance while the book's collator separates them.
    expect(compareText("Pena", "Peña")).not.toBe(0);
  });

  it("treats a missing name as empty rather than throwing", () => {
    expect(() => compareText(null, undefined)).not.toThrow();
  });
});

describe("sortedBy", () => {
  it("returns a new array, leaving the reactive one alone", () => {
    // These lists are $state; sorting in place would mutate the very value a
    // $derived is reading.
    const rows = [{ name: "b" }, { name: "a" }];
    const sorted = sortedBy(rows, (r) => r.name);
    expect(sorted).not.toBe(rows);
    expect(rows.map((r) => r.name)).toEqual(["b", "a"]);
    expect(sorted.map((r) => r.name)).toEqual(["a", "b"]);
  });
});

describe("fold", () => {
  it("removes accents and case, so a filter box over-matches rather than under-matches", () => {
    expect(fold("Cálido")).toBe(fold("calido"));
    expect(fold("PEÑA")).toBe(fold("peña"));
  });

  it("collapses what uppercasing collapses", () => {
    // Uppercase rather than lowercase because it folds MORE: ß -> SS.
    expect(fold("Straße")).toBe(fold("STRASSE"));
  });

  it("handles stroke and ligature letters that carry no combining mark", () => {
    // NFKD leaves these alone, so they are mapped by hand. Spanish and Catalan
    // need none of them; the EU expansion this project is designed for does.
    expect(fold("Ø")).toBe("O");
    expect(fold("æ")).toBe("AE");
  });

  it("survives a missing string", () => {
    expect(fold(null)).toBe("");
  });
});

describe("searchItems", () => {
  const items = ["CALI", "CÁLIDO", "ALCALI", "VERDE OLIVO", "OLIVO VERDE"].map((label) => ({
    label,
  }));

  it("matches a substring anywhere, accent-blind", () => {
    const { visible } = searchItems(items, "cali", 40);
    expect(visible.map((i) => i.label).sort()).toEqual(["ALCALI", "CALI", "CÁLIDO"]);
  });

  it("ranks exact before prefix before word-start before anywhere", () => {
    // Ranking is what makes the row cap safe: with many matches, the cap
    // decides which the farmer sees, and unranked that is a coin toss.
    const { visible } = searchItems(items, "cali", 40);
    expect(visible[0].label).toBe("CALI");
  });

  it("requires every token, in any order", () => {
    const { visible } = searchItems(items, "olivo verde", 40);
    expect(visible.map((i) => i.label).sort()).toEqual(["OLIVO VERDE", "VERDE OLIVO"]);
  });

  it("reports the true total alongside the capped rows", () => {
    // The picker prints "showing N of TOTAL", so the total must count matches,
    // not the slice.
    const many = Array.from({ length: 50 }, (_, i) => ({ label: `CALI ${i}` }));
    const { visible, total } = searchItems(many, "cali", 40);
    expect(visible).toHaveLength(40);
    expect(total).toBe(50);
  });

  it("returns the head of the list when the query is empty", () => {
    const { visible, total } = searchItems(items, "   ", 3);
    expect(visible).toHaveLength(3);
    expect(total).toBe(items.length);
  });

  it("is not fuzzy: a subsequence is not a match", () => {
    // In registers whose codes carry legal weight, ranking a plausible-but-
    // wrong entry above the right one is worse than asking for an accurate
    // substring.
    expect(searchItems(items, "clo", 40).visible).toEqual([]);
  });
});
