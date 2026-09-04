// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Navigation destinations — the single source of truth rendered twice by
// App.svelte: as the collapsible sidebar on wide screens and as the bottom
// tab bar on narrow ones. Adding a view (e.g. a future module screen) means
// adding one entry here; both layouts pick it up.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
//
// `icon` names a Lucide icon (https://lucide.dev/icons). It is a NAME and not
// the drawing, because this file may not import Svelte components: `icons.js`
// is the view-tier half that resolves one, the same split as nav.js/routes.js.
//
// `foot: true` sinks an entry to the bottom of the sidebar (app-level
// destinations like Settings, visually separated from the farm workspaces).
// The tab bar ignores the flag — a horizontal bar has no "bottom", so the
// entry simply stays last.

export const NAV_ITEMS = [
  {
    route: "#/status",
    labelKey: "nav.status",
    icon: "activity",
  },
  {
    route: "#/farms",
    labelKey: "nav.farms",
    icon: "house",
  },
  {
    route: "#/map",
    labelKey: "nav.map",
    icon: "map",
  },
  {
    // The record book, not "treatments": the view holds every register of
    // the model — crops, treatments, fertilisation, irrigation, eco-schemes —
    // and the phytosanitary one is a tab inside it. A notebook being WRITTEN
    // in rather than the old droplet, which named that one tab: this is the
    // section where the season's records are kept, not one that is read.
    route: "#/record-book",
    labelKey: "nav.record_book",
    icon: "notebook-pen",
  },
  {
    // A shelf rather than a box: the catalogue is where six reference lists
    // are kept side by side — products, operators, machinery, premises,
    // advisors, fertilisers — not one thing in a crate.
    route: "#/registry",
    labelKey: "nav.registry",
    icon: "library-big",
  },
  {
    route: "#/settings",
    labelKey: "nav.settings",
    foot: true,
    icon: "settings",
  },
];

// The route whose nav entry is highlighted for a given hash. Longest matching
// prefix wins, so "#/farms/<id>" belongs to "#/farms"; a hash that matches
// nothing falls back to the first entry (status is the default view).
export function activeRoute(hash) {
  let best = null;
  for (const item of NAV_ITEMS) {
    if (hash.startsWith(item.route) && (best === null || item.route.length > best.route.length)) {
      best = item;
    }
  }
  return (best ?? NAV_ITEMS[0]).route;
}
