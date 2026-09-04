// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Module registry: the seam through which the core sees the feature modules.

use rusqlite_migration::M;
use terrazgo_core::backup::TableShape;

/// A Terrazgo module as seen by the core.
///
/// Deliberately minimal, and grown only when a second module forces it. That
/// rule has now fired once: `backup_shape` arrived in 2026-08-13 because
/// module-cue and module-fertilisation both ship a shape constant and the shell
/// was hand-joining them — exactly the "second consumer" this comment used to
/// say to wait for. Setup hooks and exporters remain speculative; keep waiting.
///
/// Tauri commands can NOT go through this trait: `tauri::generate_handler!` is
/// a macro that needs the command function paths at compile time, so commands
/// are listed manually in `lib.rs`.
pub trait Module {
    /// Stable machine name (`"cue"`), used for diagnostics and uniqueness checks.
    fn name(&self) -> &'static str;

    /// The ordered migration steps this module contributes to the global sequence.
    fn migrations(&self) -> Vec<M<'static>>;

    /// The tables this module contributes to the backup shape probe.
    ///
    /// Core owns the probe but may never name a module's tables, so each module
    /// declares its own and the shell composes them — the same division as
    /// `migrations`.
    ///
    /// **Deliberately no default.** An empty default would let a module that
    /// ships tables forget to declare them and still compile, which is the hand-
    /// joined list's hole moved rather than closed. A module with no tables of
    /// its own says so explicitly, and the compiler asks every future one.
    fn backup_shape(&self) -> &'static [TableShape];
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

    fn backup_shape(&self) -> &'static [TableShape] {
        module_cue::BACKUP_SHAPE
    }
}

/// The fertilisation module (fertilisation, irrigation and soil records —
/// RD 1051/2022's half of the record book).
pub struct FertilisationModule;

impl Module for FertilisationModule {
    fn name(&self) -> &'static str {
        "fertilisation"
    }

    fn migrations(&self) -> Vec<M<'static>> {
        module_fertilisation::migration_set()
    }

    fn backup_shape(&self) -> &'static [TableShape] {
        module_fertilisation::BACKUP_SHAPE
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

    /// None: its lookups are stored in core's `geo_feature` and `plot_zone_flag`,
    /// which the core half of the probe already covers.
    fn backup_shape(&self) -> &'static [TableShape] {
        &[]
    }
}

/// The eco-scheme module (grazing, cultural operations and soil covers —
/// RD 1048/2022's annotation duties, the printed model's section 9).
pub struct EcoschemeModule;

impl Module for EcoschemeModule {
    fn name(&self) -> &'static str {
        "ecoscheme"
    }

    fn migrations(&self) -> Vec<M<'static>> {
        module_ecoscheme::migration_set()
    }

    fn backup_shape(&self) -> &'static [TableShape] {
        module_ecoscheme::BACKUP_SHAPE
    }
}

/// Every module compiled into this build, in registration order.
///
/// `Box<dyn Module>` is a trait object: the Vec holds modules of different
/// concrete types behind one interface, dispatched dynamically at runtime.
///
/// Registration order is load-bearing: it fixes each module's position in the
/// single global migration version sequence (see `crate::db::composed_migrations`).
/// Order is load-bearing and append-only in spirit: a module's position fixes
/// where its steps land in the global version sequence, so new modules join at
/// the tail rather than between existing ones.
pub fn registered_modules() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(CueModule),
        Box::new(FertilisationModule),
        Box::new(SigpacModule),
        // At the tail, per the rule above — not beside the other two record-book
        // modules, however much it would read better there. SigpacModule
        // contributes no migrations today, so the two placements are equivalent
        // in effect; the rule exists so nobody has to verify that each time.
        Box::new(EcoschemeModule),
    ]
}
