// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

fn main() {
    // The build stamp the About panel prints beside the version. UTC and
    // minute-resolution: it identifies a build, it does not time an event, and
    // two builds a minute apart are the same answer to "which one is this?".
    //
    // WHAT REFRESHES IT. A build script re-runs when one of the paths it
    // declared has changed, and `tauri_build::build()` declares several —
    // tauri.conf.json, `capabilities/`, and the frontend `dist/` it embeds. So
    // every PACKAGED build re-stamps: `cargo tauri build` (desktop and
    // android alike) runs `npm run build` first, which rewrites dist/.
    //
    // A plain `cargo build` after editing only Rust does NOT, and carries the
    // previous stamp. That is the deliberate trade: the alternative is a script
    // that always re-runs, which changes this variable on every invocation and
    // therefore recompiles the shell — the workspace's slowest crate — on every
    // `cargo check`.
    println!(
        "cargo:rustc-env=TERRAZGO_BUILD_TIME={}",
        jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%MZ")
    );
    tauri_build::build()
}
