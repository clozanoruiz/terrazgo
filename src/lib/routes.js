// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The hash router's table: which view answers which route, as data.
//
// Deliberately NOT in nav.js, which is the framework-agnostic tier and may not
// import Svelte components (docs/frontend-conventions.md). nav.js says what the
// navigation OFFERS; this says what each route RENDERS, and the two lists are
// not the same — `#/farms/<id>` is a real route with no nav entry of its own.
//
// Adding a module screen is one entry here plus one in nav.js. The order is
// load-bearing: the first entry whose `match` answers wins, so the farm detail
// route must precede the farms list, and any prefix route must follow the exact
// routes it would otherwise swallow.

import FarmsView from "./FarmsView.svelte";
import FarmView from "./FarmView.svelte";
import MapView from "./MapView.svelte";
import RegistryView from "./RegistryView.svelte";
import SettingsView from "./SettingsView.svelte";
import StatusView from "./StatusView.svelte";
import TreatmentsView from "./TreatmentsView.svelte";

/// `match` returns the view's props when the hash is its route, or null.
const exact = (route) => (hash) => (hash === route ? {} : null);
const prefix = (route) => (hash) => (hash.startsWith(route) ? {} : null);

export const ROUTES = [
  {
    // The farm detail page: a route with a parameter, and no nav entry — the
    // farms entry stays highlighted for it (see nav.js `activeRoute`).
    match: (hash) => {
      const found = /^#\/farms\/(.+)$/.exec(hash);
      return found ? { farmId: found[1] } : null;
    },
    component: FarmView,
  },
  { match: exact("#/farms"), component: FarmsView },
  // Prefix, not exact: #/map?farm=…&plot=… deep links (the query is parsed
  // inside the view).
  { match: prefix("#/map"), component: MapView },
  { match: exact("#/treatments"), component: TreatmentsView },
  { match: exact("#/registry"), component: RegistryView },
  { match: exact("#/settings"), component: SettingsView },
];

/// The view for a hash, with its props. Anything unmatched is the status view:
/// a bad hash shows the app's home rather than a blank frame.
export function resolveRoute(hash) {
  for (const route of ROUTES) {
    const props = route.match(hash);
    if (props) return { component: route.component, props };
  }
  return { component: StatusView, props: {} };
}
