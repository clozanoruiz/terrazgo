// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The settings tree is data, and `searchSettings` is the logic in it: what a
// query leaves on screen and what the contents list counts beside each label.
//
// The dictionary is the real Spanish one rather than a fixture. A search that
// works against invented strings proves nothing about a search over the words
// the farmer actually reads — and it is the only way to catch the tree naming
// a key that no longer exists, which would silently make a setting findable
// only by typing its key.
import { describe, expect, it, vi } from "vitest";
import es from "../i18n/es.js";

// collate.js reads the active language for its collator, and i18n.js touches
// localStorage at module load — the collate.test.js arrangement, for the same
// reason. The dictionary itself is a plain module and is imported directly.
vi.mock("../i18n.js", () => ({ languageTag: () => "es-ES" }));

const { SETTINGS_TREE, searchSettings, settingsAnchor, settingsAnchors } =
  await import("./settingsTree.js");

/// What the view passes in production, minus the placeholder interpolation:
/// `{days}` stays literal, which costs nothing because no query is a brace.
const textOf = (key) => es[key] ?? "";

const sections = () => SETTINGS_TREE;
const groups = () => SETTINGS_TREE.flatMap((s) => s.groups);
const items = () => groups().flatMap((g) => g.items);

describe("SETTINGS_TREE", () => {
  it("names only keys the dictionary defines", () => {
    // The whole search corpus. A stale key here would search its own name and
    // never match anything a reader could type.
    const named = [
      ...sections().map((s) => s.labelKey),
      ...groups().flatMap((g) => [g.labelKey, g.hintKey]),
      ...items().flatMap((i) => i.keys),
    ].filter(Boolean);

    for (const key of named) {
      expect(es[key], `missing dictionary key: ${key}`).toBeTruthy();
    }
  });

  it("gives every node an id that is unique across all three levels", () => {
    // `hits` is one flat map over sections, groups and items, so a collision
    // would make one node's visibility silently decide another's.
    const ids = [
      ...sections().map((s) => s.id),
      ...groups().map((g) => g.id),
      ...items().map((i) => i.id),
    ];
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("gives every group at least one item", () => {
    // A group with no items would count zero hits and therefore never appear,
    // which is a heading that exists in the data and nowhere else.
    for (const group of groups()) {
      expect(group.items.length, group.id).toBeGreaterThan(0);
    }
  });

  it("lists anchors in document order, sections before their own groups", () => {
    const order = settingsAnchors();
    for (const section of sections()) {
      const at = order.indexOf(section.id);
      expect(at, section.id).toBeGreaterThanOrEqual(0);
      for (const group of section.groups) {
        expect(order.indexOf(group.id), group.id).toBeGreaterThan(at);
      }
    }
  });
});

describe("settingsAnchor", () => {
  it("turns a dotted node id into one DOM id", () => {
    expect(settingsAnchor("map.offline")).toBe("set-map-offline");
    expect(settingsAnchor("general")).toBe("set-general");
  });
});

describe("searchSettings", () => {
  it("shows everything when nothing is typed", () => {
    const result = searchSettings("", textOf);
    expect(result.filtering).toBe(false);
    expect(result.total).toBe(items().length);
    for (const item of items()) expect(result.hits[item.id], item.id).toBe(1);
  });

  it("counts a section as the sum of its groups", () => {
    const { hits } = searchSettings("", textOf);
    for (const section of sections()) {
      const sum = section.groups.reduce((n, g) => n + hits[g.id], 0);
      expect(hits[section.id], section.id).toBe(sum);
    }
  });

  it("keeps only the settings whose own text matches", () => {
    // "caducidad" is the licence lead time's own label and appears nowhere
    // else — not even in the hint above it, which says "caduque".
    const { hits, total } = searchSettings("caducidad", textOf);
    expect(hits.licence_lead).toBe(1);
    expect(hits.itv_lead).toBe(0);
    expect(hits.cache_size).toBe(0);
    expect(total).toBe(1);
    // The count travels up: one hit in the group, one in the section.
    expect(hits["general.alerts"]).toBe(1);
    expect(hits.general).toBe(1);
    expect(hits.map).toBe(0);
  });

  it("keeps a whole group when only the hint ABOVE the fields matches", () => {
    // The real case this rule exists for: "carné" is written in the alerts
    // hint, not in either field's label. Returning the hint's group with its
    // fields removed would answer a good search with an empty heading.
    const { hits } = searchSettings("carné", textOf);
    expect(hits.licence_lead).toBe(1);
    expect(hits.itv_lead).toBe(1);
    expect(hits.map).toBe(0);
  });

  it("ignores accents and case, both ways round", () => {
    // A farmer types on a phone keyboard; requiring the accent to find the
    // accented word is the failure this shares with the catalogue pickers.
    expect(searchSettings("carne", textOf).hits.licence_lead).toBe(1);
    expect(searchSettings("CARNÉ", textOf).hits.licence_lead).toBe(1);
  });

  it("keeps a whole group when the group's own heading matches", () => {
    // Searching for the heading should hand back the group intact rather than
    // an empty shell whose title matched and whose fields did not.
    const { hits } = searchSettings("avisos", textOf);
    expect(hits["general.alerts"]).toBe(2);
    expect(hits.licence_lead).toBe(1);
    expect(hits.itv_lead).toBe(1);
  });

  it("keeps a whole section when the section's own heading matches", () => {
    const { hits } = searchSettings("mapa", textOf);
    const map = sections().find((s) => s.id === "map");
    expect(hits.map).toBe(map.groups.flatMap((g) => g.items).length);
  });

  it("matches a hint, not only a label", () => {
    // "vulnerable"-style prose lives in the hints, and a farmer searching for
    // what a setting DOES is typing the hint's words, not the label's.
    // "compacta" appears in the maintenance hint and in no label at all.
    const { hits } = searchSettings("compacta", textOf);
    expect(hits["advanced.maintenance"]).toBeGreaterThan(0);
  });

  it("requires every token, in any order", () => {
    // Token-AND, matching the catalogue pickers: two words narrow, they do
    // not widen.
    expect(searchSettings("mapas conexión", textOf).hits.cache_size).toBe(1);
    expect(searchSettings("conexión mapas", textOf).hits.cache_size).toBe(1);
    expect(searchSettings("mapas zzz", textOf).hits.cache_size).toBe(0);
  });

  it("reports an empty result rather than falling back to everything", () => {
    // The failure worth naming: a search that matches nothing must not read as
    // a search that was not run.
    const result = searchSettings("zzzznothing", textOf);
    expect(result.filtering).toBe(true);
    expect(result.total).toBe(0);
    for (const section of sections()) expect(result.hits[section.id], section.id).toBe(0);
  });
});
