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

const { catalogueItems, codeItems, nameItems, pickerOptions } = await import("./selectItems.js");

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

describe("nameItems over a catalogue list", () => {
  it("keeps each code's offered flag through the sort", () => {
    const picks = [
      { code: "02", name: "Ovinos", offered: true },
      { code: "01", name: "Bovinos", offered: false },
    ];
    expect(
      nameItems(
        picks,
        (p) => p.name,
        (p) => p.code,
      ),
    ).toEqual([
      { value: "01", label: "Bovinos", offered: false },
      { value: "02", label: "Ovinos", offered: true },
    ]);
  });

  it("adds no flag to an entity list", () => {
    expect(nameItems([{ id: "a", name: "Ana" }])).toEqual([{ value: "a", label: "Ana" }]);
  });
});

describe("catalogueItems", () => {
  it("keeps the provider's order and says which codes may be offered", () => {
    const picks = [
      { code: "2", name: "Sembrada", offered: true },
      { code: "1", name: "Retirada", offered: false },
    ];
    expect(catalogueItems(picks)).toEqual([
      { value: "2", label: "Sembrada", offered: true },
      { value: "1", label: "Retirada", offered: false },
    ]);
  });

  it("reads a raw catalogue row by its label", () => {
    const rows = [{ code: "254", label: "Septoriosis", offered: true }];
    expect(catalogueItems(rows, (row) => row.label)).toEqual([
      { value: "254", label: "Septoriosis", offered: true },
    ]);
  });
});

describe("pickerOptions", () => {
  const items = [
    { value: "1", label: "Uno", offered: true },
    { value: "2", label: "Dos", offered: false },
    { value: "3", label: "Tres", offered: true },
  ];

  it("offers only the live codes for a new choice", () => {
    expect(pickerOptions(items, "", true)).toEqual({
      options: [items[0], items[2]],
      unknown: false,
    });
  });

  it("keeps a code no longer offered when it is the one the record carries", () => {
    // Retired by the authority since: the device still knows its name.
    expect(pickerOptions(items, "2", true)).toEqual({ options: items, unknown: false });
  });

  it("shows a code this device's catalogues lack as itself, and says so", () => {
    // Written on a device whose catalogues are newer (docs/sync.md → What
    // stays device-local): the bare code, never an empty-looking field.
    expect(pickerOptions(items, "999", true)).toEqual({
      options: [{ value: "999", label: "999" }, items[0], items[2]],
      unknown: true,
    });
  });

  it("invents nothing for a list that is not a catalogue", () => {
    // An id missing from an entity list would print as a UUID.
    const people = [{ value: "a", label: "Ana" }];
    expect(pickerOptions(people, "gone", false)).toEqual({ options: people, unknown: false });
  });
});
