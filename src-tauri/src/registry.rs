// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Module registry: the seam through which the core sees the feature modules.

use rusqlite_migration::M;

/// A Terrazgo module as seen by the core.
///
/// Deliberately minimal: today the only thing the core needs from a module is
/// its migration steps. The shape is expected to grow (setup hooks, exporters)
/// but is speculative with a single module registered — resist adding methods
/// until a second module actually needs them.
///
/// Tauri commands can NOT go through this trait: `tauri::generate_handler!` is
/// a macro that needs the command function paths at compile time, so commands
/// are listed manually in `lib.rs`.
pub trait Module {
    /// Stable machine name (`"cue"`), used for diagnostics and uniqueness checks.
    fn name(&self) -> &'static str;

    /// The ordered migration steps this module contributes to the global sequence.
    fn migrations(&self) -> Vec<M<'static>>;
}

/// The CUE / PAC module (phytosanitary treatment records).
pub struct CueModule;

impl Module for CueModule {
    fn name(&self) -> &'static str {
        "cue"
    }

    fn migrations(&self) -> Vec<M<'static>> {
        module_cue::migration_set()
    }
}

/// The SIGPAC module (Spanish parcel lookups). No migrations yet — its
/// lookups land in core's `geo_feature` — but registering it now fixes its
/// position in the global sequence for when its own tables arrive.
pub struct SigpacModule;

impl Module for SigpacModule {
    fn name(&self) -> &'static str {
        "sigpac"
    }

    fn migrations(&self) -> Vec<M<'static>> {
        module_sigpac::migration_set()
    }
}

/// Every module compiled into this build, in registration order.
///
/// `Box<dyn Module>` is a trait object: the Vec holds modules of different
/// concrete types behind one interface, dispatched dynamically at runtime.
///
/// Registration order is load-bearing: it fixes each module's position in the
/// single global migration version sequence (see `crate::db::composed_migrations`).
pub fn registered_modules() -> Vec<Box<dyn Module>> {
    vec![Box::new(CueModule), Box::new(SigpacModule)]
}
