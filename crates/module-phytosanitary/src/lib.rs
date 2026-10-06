// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo phytosanitary module: RD 1311/2012's half of the record book —
//! field and non-field treatments, seed treatments and the register
//! declarations.
//!
//! **Named for the domain, not for the document** (renamed from `cue`
//! 2026-09-05). CUE is the *cuaderno de explotación* itself, so that name
//! claimed the whole book for one of its three authors: `module-fertilisation`
//! (RD 1051/2022) and `module-ecoscheme` (RD 1048/2022) write into the same
//! book, and `terrazgo-recordbook` assembles the printed document from all
//! three.
//!
//! Layout:
//!   * [`db`]            — connection setup + embedded migrations (the core registers these).
//!   * [`models`]        — Rust structs mirroring the schema, plus `New*` insert inputs.
//!   * [`repository`]    — CRUD for `TreatmentRecord` and its dependencies, with audit
//!     logging; one submodule per entity group, re-exported from `repository`.
//!   * [`alerts`]        — the alerts this module raises: its three kinds, their pure
//!     rules (PHI window, licence/ITV expiry) and `AlertConfig`. Core assembles the list.
//!   * [`grouping`]      — how one treatment splits into presented rows, a rule the
//!     printed book and the SIEX export must agree on.
//!   * [`premises_link`] — which of core's premises the non-field register may name.
//!   * [`siex`]          — neutral-code ↔ SIEX-code mapping for the Spanish export.
//!   * [`catalogue`]     — reference-catalogue reads the book's coded fields need.
//!   * [`error`]         — `PhytosanitaryError` / `Result`.
//!   * [`demo`]          — demo-campaign seeding (only with the `demo` feature).
//!
//! The descriptor export itself is NOT here: it moved to `terrazgo-siex`
//! (2026-08-20), because ten of the format's fifteen blocks come from modules
//! this one may never depend on.
//!
//! Date maths, the `record_change` audit helpers and the farm/plot entities live
//! in `terrazgo-core` (moved 2026-06-12); `date` is re-exported here because the
//! PHI/alert rules are built on it.

pub mod alerts;
pub mod catalogue;
pub mod db;
#[cfg(feature = "demo")]
pub mod demo;
pub mod duplicates;
pub mod error;
pub mod grouping;
pub mod models;
pub mod premises_link;
pub mod repository;
pub mod siex;

pub use db::{
    BACKUP_SHAPE, REGISTER_DECLARATION_SLOT, ROW_CAPTIONS, SYNC_SHAPE, migration_set, migrations,
    open, open_in_memory,
};
pub use error::{PhytosanitaryError, Result};
pub use grouping::crop_groups;
pub use terrazgo_core::date;
