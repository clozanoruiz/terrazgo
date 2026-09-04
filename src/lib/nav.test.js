// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Navigation is data, and `activeRoute` is the one piece of logic in it: which
// entry gets highlighted. Longest-prefix, because a nested route must not light
// up its parent — and the fallback must never leave nothing highlighted.
import { describe, expect, it } from "vitest";
import { NAV_ITEMS, activeRoute } from "./nav.js";

describe("activeRoute", () => {
  it("highlights the entry whose route the hash starts with", () => {
    expect(activeRoute("#/farms")).toBe("#/farms");
    expect(activeRoute("#/map")).toBe("#/map");
  });

  it("keeps a child route on its parent entry", () => {
    // #/farms/<id> has no nav entry of its own; it belongs to Farms.
    expect(activeRoute("#/farms/01a03d6d-40dd-7272-868b-f239ba740541")).toBe("#/farms");
  });

  it("keeps a route carrying a query on its entry", () => {
    expect(activeRoute("#/map?farm=abc&plot=def")).toBe("#/map");
  });

  it("prefers the LONGEST matching route", () => {
    // The guard against a short route swallowing a longer one that shares its
    // prefix. Verified against the real table rather than a fixture.
    const longest = [...NAV_ITEMS].sort((a, b) => b.route.length - a.route.length)[0];
    expect(activeRoute(longest.route)).toBe(longest.route);
  });

  it("falls back to the first entry rather than highlighting nothing", () => {
    expect(activeRoute("#/nonexistent")).toBe(NAV_ITEMS[0].route);
    expect(activeRoute("")).toBe(NAV_ITEMS[0].route);
  });
});

describe("NAV_ITEMS", () => {
  it("gives every destination a route, a label key and an icon", () => {
    // Adding a view is one entry here plus one in routes.js; a half-filled
    // entry renders a blank button.
    for (const item of NAV_ITEMS) {
      expect(item.route, JSON.stringify(item)).toMatch(/^#\//);
      expect(item.labelKey, item.route).toMatch(/^nav\./);
      expect(item.icon, item.route).toBeTruthy();
    }
  });

  it("has no duplicate routes", () => {
    const routes = NAV_ITEMS.map((i) => i.route);
    expect(new Set(routes).size).toBe(routes.length);
  });
});
