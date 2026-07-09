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
    ignores: ["dist/", "target/", "src-tauri/"],
  },
];
