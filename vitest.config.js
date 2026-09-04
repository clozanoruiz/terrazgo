// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Unit tests for the frontend's framework-agnostic tier
// (docs/frontend-conventions.md → "The two-tier rule").
//
// Deliberately its own config rather than a `test` block in vite.config.js:
// what is testable here is exactly the tier that imports no Svelte, so the
// runner needs no Svelte plugin, and the build config stays about building.
// A module that needs a component rendered is, by that same rule, a view — and
// views are verified at runtime by the scripted checks instead.
//
// `node` and not `jsdom`: nothing in this tier touches the DOM. The modules
// that read `localStorage`/`navigator` (i18n.js) are stubbed by the tests that
// need them, which keeps the suite dependency-free.
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["src/**/*.test.js"],
    environment: "node",
  },
});
