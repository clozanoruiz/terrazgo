// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo CUE / PAC module: phytosanitary treatment records.
//!
//! Layout:
//!   * [`db`]         — connection setup + embedded migrations (the core registers these).
//!   * [`models`]     — Rust structs mirroring the schema, plus `New*` insert inputs.
//!   * [`repository`] — CRUD for `TreatmentRecord` and its dependencies, with audit
//!     logging; one submodule per entity group, re-exported from `repository`.
//!   * [`alerts`]     — pure alert rules (PHI window, licence/ITV expiry) + `AlertConfig`.
//!   * [`error`]      — `CueError` / `Result`.
//!   * [`demo`]       — demo-campaign seeding (only with the `demo` feature).
//!
//! Date maths, the `record_change` audit helpers and the farm/plot entities live
//! in `terrazgo-core` (moved 2026-06-12); `date` is re-exported here because the
//! PHI/alert rules are built on it.

pub mod alerts;
pub mod db;
#[cfg(feature = "demo")]
pub mod demo;
pub mod error;
pub mod models;
pub mod repository;

pub use db::{migration_set, migrations, open, open_in_memory};
pub use error::{CueError, Result};
pub use terrazgo_core::date;
