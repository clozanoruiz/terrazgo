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
//
// `titleKey` is what the shell's top band calls the screen. It lives here
// rather than in each view because the band belongs to App.svelte, and because
// a view stating its own name twice — once in the band, once as an <h2> in its
// own canvas — is how the two came to disagree. They are the `nav.*` keys on
// purpose: the band must say what the entry the user just clicked said.

import FarmsView from "./FarmsView.svelte";
import FarmView from "./FarmView.svelte";
import MapView from "./MapView.svelte";
import RegistryView from "./RegistryView.svelte";
import SettingsView from "./SettingsView.svelte";
import StatusView from "./StatusView.svelte";
import RecordBookView from "./RecordBookView.svelte";

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
    titleKey: "nav.farms",
  },
  { match: exact("#/farms"), component: FarmsView, titleKey: "nav.farms" },
  // Prefix, not exact: #/map?farm=…&plot=… deep links (the query is parsed
  // inside the view).
  { match: prefix("#/map"), component: MapView, titleKey: "nav.map" },
  { match: exact("#/record-book"), component: RecordBookView, titleKey: "nav.record_book" },
  { match: exact("#/registry"), component: RegistryView, titleKey: "nav.registry" },
  { match: exact("#/settings"), component: SettingsView, titleKey: "nav.settings" },
];

/// The view for a hash, with its props and the key naming it. Anything
/// unmatched is the status view: a bad hash shows the app's home rather than a
/// blank frame.
export function resolveRoute(hash) {
  for (const route of ROUTES) {
    const props = route.match(hash);
    if (props) return { component: route.component, props, titleKey: route.titleKey };
  }
  return { component: StatusView, props: {}, titleKey: "nav.status" };
}
