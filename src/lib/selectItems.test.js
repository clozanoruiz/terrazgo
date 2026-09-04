// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Nothing to do with numbers or dates — this suite exists to cover the ONE
// decision the module is built around: whether a list may be re-ordered.
// Alphabetising a coded vocabulary is a regression, and leaving an entity list
// in SQL's BINARY order files "Ángel" after "Zubiri".
import { describe, expect, it, vi } from "vitest";

// tCode is stubbed to the raw key so the test reads the ORDER, not a
// translation. languageTag is here too because collate.js reaches for it.
vi.mock("../i18n.js", () => ({
  tCode: (prefix, code) => `${prefix}.${code}`,
  languageTag: () => "es-ES",
}));

const { codeItems, nameItems } = await import("./selectItems.js");

describe("codeItems", () => {
  it("keeps the order the backend supplied", () => {
    // Licence levels run basic → qualified → fumigator → pilot; BBCH stages run
    // 0-9; efficacy runs good → fair → poor. Sorting any of them would be wrong.
    const rows = [{ code: "basic" }, { code: "qualified" }, { code: "fumigator" }];
    expect(codeItems(rows, "licence").map((i) => i.value)).toEqual([
      "basic",
      "qualified",
      "fumigator",
    ]);
  });

  it("labels through the dictionary under the prefix", () => {
    expect(codeItems([{ code: "l_ha" }], "unit")[0].label).toBe("unit.l_ha");
  });
});

describe("nameItems", () => {
  it("orders by the active language, not by code point", () => {
    // SQL's BINARY order would put "Ángel" last.
    const rows = [
      { id: 1, name: "Zubiri" },
      { id: 2, name: "Ángel" },
    ];
    expect(nameItems(rows).map((i) => i.label)).toEqual(["Ángel", "Zubiri"]);
  });

  it("takes accessors, because not every row is flat", () => {
    // A fertiliser material arrives as { material, nutrients }.
    const rows = [
      { material: { id: "b", name: "Purín" } },
      { material: { id: "a", name: "Compost" } },
    ];
    const items = nameItems(
      rows,
      (row) => row.material.name,
      (row) => row.material.id,
    );
    expect(items).toEqual([
      { value: "a", label: "Compost" },
      { value: "b", label: "Purín" },
    ]);
  });

  it("does not mutate the caller's array", () => {
    const rows = [
      { id: 1, name: "b" },
      { id: 2, name: "a" },
    ];
    nameItems(rows);
    expect(rows.map((r) => r.name)).toEqual(["b", "a"]);
  });
});
