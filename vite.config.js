// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Vite serves and builds the Svelte frontend. The Vite root is src/ (where
// index.html lives); production output goes to dist/ at the repo root, which
// tauri.conf.json's frontendDist points at. `cargo tauri dev` starts this dev
// server itself via beforeDevCommand.
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  root: "src",
  plugins: [svelte()],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    // Tauri's devUrl is fixed to this port; failing fast beats Vite silently
    // picking another one and the window loading nothing.
    strictPort: true,
  },
  // Don't wipe the terminal — cargo tauri dev output shares it.
  clearScreen: false,
});
