// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Module registry: the seam through which the core sees the feature modules.

use rusqlite_migration::M;
use terrazgo_core::backup::TableShape;
use terrazgo_core::merge::RowCaption;
use terrazgo_core::sync::TableSync;

/// A Terrazgo module as seen by the core.
///
/// Deliberately minimal, and grown only when a second module forces it. That
/// rule has now fired twice: `backup_shape` arrived in 2026-08-13 because
/// module-phytosanitary and module-fertilisation both ship a shape constant and the shell
/// was hand-joining them — exactly the "second consumer" this comment used to
/// say to wait for — and `sync_shape` followed on 2026-09-18 for the same
/// reason, three modules each classifying their tables for the merge. Setup
/// hooks and exporters remain speculative; keep waiting.
///
/// Tauri commands can NOT go through this trait: `tauri::generate_handler!` is
/// a macro that needs the command function paths at compile time, so commands
/// are listed manually in `lib.rs`.
pub trait Module {
    /// Stable machine name (`"phytosanitary"`), used for diagnostics and uniqueness checks.
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

    /// This module's half of the aggregate map: what the sync merge does with
    /// each of its tables — a register of its own, part of another register,
    /// a slot-keyed register, or never synced (docs/sync.md → The aggregate map).
    ///
    /// **No default, for `backup_shape`'s reason.** A module that ships tables
    /// does not compile until it has said how each one merges, and the shell's
    /// contract test refuses any table in the composed schema that nobody
    /// classified.
    fn sync_shape(&self) -> &'static [TableSync];

    /// How a row of each of this module's tables is named to a person: what a
    /// conflict over it is called, and what a reference to it says instead of a
    /// UUID (docs/sync.md → Conflicts as the person sees them).
    ///
    /// The third shape the trait carries, and it arrived with the same second
    /// consumer the others did — three modules whose registers a review screen
    /// has to name. Unlike them it has no completeness duty: a table missing
    /// from the map costs a name, never correctness, so the conflict review
    /// falls back to the id it holds.
    fn row_captions(&self) -> &'static [RowCaption];
}

/// The phytosanitary module (field and non-field treatments, seed treatments
/// and the register declarations — RD 1311/2012's half of the record book).
///
/// Named for the domain, not the document: the *cuaderno de explotación* is
/// what all three register modules write into, and `terrazgo-recordbook`
/// assembles (renamed from `cue` 2026-09-05).
pub struct PhytosanitaryModule;

impl Module for PhytosanitaryModule {
    fn name(&self) -> &'static str {
        "phytosanitary"
    }

    fn migrations(&self) -> Vec<M<'static>> {
        module_phytosanitary::migration_set()
    }

    fn backup_shape(&self) -> &'static [TableShape] {
        module_phytosanitary::BACKUP_SHAPE
    }

    fn sync_shape(&self) -> &'static [TableSync] {
        module_phytosanitary::SYNC_SHAPE
    }

    fn row_captions(&self) -> &'static [RowCaption] {
        module_phytosanitary::ROW_CAPTIONS
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

    fn sync_shape(&self) -> &'static [TableSync] {
        module_fertilisation::SYNC_SHAPE
    }

    fn row_captions(&self) -> &'static [RowCaption] {
        module_fertilisation::ROW_CAPTIONS
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

    /// None, for the same reason: what it stores lands in core's tables, which
    /// core's own half of the map classifies.
    fn sync_shape(&self) -> &'static [TableSync] {
        &[]
    }

    /// None: core names the rows it owns.
    fn row_captions(&self) -> &'static [RowCaption] {
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

    fn sync_shape(&self) -> &'static [TableSync] {
        module_ecoscheme::SYNC_SHAPE
    }

    fn row_captions(&self) -> &'static [RowCaption] {
        module_ecoscheme::ROW_CAPTIONS
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
        Box::new(PhytosanitaryModule),
        Box::new(FertilisationModule),
        Box::new(SigpacModule),
        // At the tail, per the rule above — not beside the other two record-book
        // modules, however much it would read better there. SigpacModule
        // contributes no migrations today, so the two placements are equivalent
        // in effect; the rule exists so nobody has to verify that each time.
        Box::new(EcoschemeModule),
    ]
}

/// The whole aggregate map: core's half first, then each registered module's —
/// the same composition `composed_migrations` does for the schema, and for the
/// same reason: core may never name a module's tables.
///
/// Public for the classification contract test, which checks it against the
/// composed schema, and for the merge engine that will read it.
pub fn composed_sync_shape() -> Vec<TableSync> {
    let mut shape = terrazgo_core::sync::CORE_SYNC_SHAPE.to_vec();
    for module in registered_modules() {
        shape.extend_from_slice(module.sync_shape());
    }
    shape
}

/// The whole naming map: core's half first, then each registered module's.
///
/// Composed like the aggregate map, and for the same reason — core may never
/// name a module's tables, and only the shell sees them all.
pub fn composed_row_captions() -> Vec<RowCaption> {
    let mut captions = terrazgo_core::merge::CORE_ROW_CAPTIONS.to_vec();
    for module in registered_modules() {
        captions.extend_from_slice(module.row_captions());
    }
    captions
}
