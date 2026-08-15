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
    // Four Bits UI components are unusable under this app's production CSP, and
    // the failure is silent and unrecoverable rather than cosmetic: they take a
    // body scroll lock that APPLIES through CSSOM (allowed) —
    // `document.body.style.pointerEvents = "none"` — and RELEASES through
    // `document.body.setAttribute("style", …)`, which `default-src 'self'`
    // blocks. The lock therefore engages and never lifts, leaving the whole app
    // deaf to input until it is restarted.
    //
    // The owned controls (Select, Combobox, DatePicker, TimeField) are fine
    // because every one of them passes `preventScroll={false}` explicitly, so
    // the lock never engages — keep doing that in any new one.
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
                "Unusable under the production CSP: their body scroll lock releases via document.body.setAttribute('style', …), which is blocked, stranding pointer-events:none on <body>. Use a Popover-based control that passes preventScroll={false}.",
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
