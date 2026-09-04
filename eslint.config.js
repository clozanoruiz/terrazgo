// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// ESLint flat config for the Svelte frontend (plain JS, no TypeScript).
// Style/formatting is Prettier's job; this catches real defects: undefined
// globals, unused vars, Svelte-specific mistakes (eslint-plugin-svelte).
import js from "@eslint/js";
import svelte from "eslint-plugin-svelte";
import globals from "globals";

export default [
  js.configs.recommended,
  ...svelte.configs["flat/recommended"],
  {
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
  },
  {
    // Build-time scripts run in Node, not the webview: they are the one place
    // in this repo that legitimately touches `process` and the filesystem.
    // Deliberately narrow — `src/` stays browser-only, which is what keeps a
    // Node API from being reached for in a view by habit.
    files: ["scripts/**/*.{js,mjs}"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    // Bits UI's body scroll lock APPLIES through CSSOM (allowed) —
    // `document.body.style.pointerEvents = "none"` — and RELEASES through
    // `document.body.setAttribute("style", …)`, which `default-src 'self'`
    // blocks. A lock that engages therefore never lifts, leaving the whole app
    // deaf to input until it is restarted: silent and unrecoverable rather than
    // cosmetic.
    //
    // The escape is one prop, and every owned control passes it: `Select`,
    // `Combobox`, `DatePicker` and `TimeField` all pass `preventScroll={false}`
    // explicitly, so the lock is never constructed — keep doing that in any new
    // one. Verified again in bits-ui 2.18.1 (2026-08-26): `new BodyScrollLock`
    // exists at exactly one site, guarded by `if (preventScroll)`, and the
    // blocked `setAttribute` line is reachable only from its teardown.
    //
    // So this list is a DEFAULT-value guard, not a claim that these components
    // are broken. `src/lib/TzDialog.svelte` is the one sanctioned `Dialog`
    // import and `src/lib/TzMenu.svelte` the one sanctioned `DropdownMenu`,
    // each carrying a targeted disable with its evidence beside it;
    // `AlertDialog` and `ContextMenu` stay listed because nothing needs them,
    // and an unused escape hatch is one nobody re-derives the evidence for.
    //
    // Re-read 2026-09-03, and the earlier note that DropdownMenu "no longer
    // renders ScrollLock at all" was WRONG: `popper-layer-inner.svelte` renders
    // it for every floating layer, resolving `preventScroll ?? true`. Select
    // and Combobox default it to false themselves; DropdownMenu.Content states
    // no default, so passing it there is load-bearing rather than belt and
    // braces.
    //
    // Before 2026-08-15 this rule was only a note in docs/frontend-conventions.md
    // and a comment in four components; nothing enforced it.
    files: ["src/**/*.{js,svelte}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "bits-ui",
              importNames: ["Dialog", "AlertDialog", "DropdownMenu", "ContextMenu"],
              message:
                "Their body scroll lock releases via document.body.setAttribute('style', …), which the production CSP blocks, stranding pointer-events:none on <body>. Import one only inside an owned control that passes preventScroll={false} (TzDialog.svelte and TzMenu.svelte are the two such exceptions), never in a view.",
            },
          ],
        },
      ],
    },
  },
  {
    ignores: ["dist/", "target/", "src-tauri/"],
  },
];
