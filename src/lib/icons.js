// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The icons the navigation names, resolved to components.
//
// This file exists because of the two-tier rule
// (docs/frontend-conventions.md). `nav.js` is framework-agnostic and may not
// import Svelte components, so it can only say WHICH icon a destination wants;
// this is the view-tier half that knows how to draw one. Exactly the
// `nav.js` / `routes.js` split, for the same reason.
//
// Keyed on Lucide's own name, so the map is checkable against
// https://lucide.dev/icons by reading it — a key that renamed the icon would
// be a second vocabulary to keep in step with nothing enforcing it.
import { Activity, House, LibraryBig, Map, NotebookPen, Settings } from "@lucide/svelte";

export const NAV_ICONS = {
  activity: Activity,
  house: House,
  "library-big": LibraryBig,
  map: Map,
  "notebook-pen": NotebookPen,
  settings: Settings,
};
