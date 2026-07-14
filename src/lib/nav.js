// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Navigation destinations — the single source of truth rendered twice by
// App.svelte: as the collapsible sidebar on wide screens and as the bottom
// tab bar on narrow ones. Adding a view (e.g. a future module screen) means
// adding one entry here; both layouts pick it up.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
//
// `icon` is SVG path data (24×24 viewBox, Feather icons, MIT), drawn with
// stroke: currentColor; several subpaths may share one `d` string.
//
// `foot: true` sinks an entry to the bottom of the sidebar (app-level
// destinations like Settings, visually separated from the farm workspaces).
// The tab bar ignores the flag — a horizontal bar has no "bottom", so the
// entry simply stays last.

export const NAV_ITEMS = [
  {
    route: "#/status",
    labelKey: "nav.status",
    icon: "M22 12h-4l-3 9L9 3l-3 9H2",
  },
  {
    route: "#/farms",
    labelKey: "nav.farms",
    icon: "M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M9 22V12h6v10",
  },
  {
    route: "#/map",
    labelKey: "nav.map",
    icon: "M1 6v16l7-4 8 4 7-4V2l-7 4-8-4z M8 2v16 M16 6v16",
  },
  {
    route: "#/treatments",
    labelKey: "nav.treatments",
    icon: "M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z",
  },
  {
    route: "#/registry",
    labelKey: "nav.registry",
    icon: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z M3.27 6.96L12 12.01l8.73-5.05 M12 22.08V12",
  },
  {
    route: "#/settings",
    labelKey: "nav.settings",
    foot: true,
    icon: "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z",
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
