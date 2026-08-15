// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo fertilisation module: fertilisation, irrigation and soil records.
//!
//! The record book has **two** decrees. RD 1311/2012 governs the phytosanitary
//! registers, which module-cue owns. RD 1051/2022 art. 5 (amended by
//! RD 934/2025) creates the cuaderno's fertilisation section — binding since
//! 1 January 2026, recorded within one month of each operation — and art. 5.e
//! puts irrigation doses and dates in the very same duty. That is why the
//! *record* of irrigation lives here rather than in a future Irrigation
//! module, which keeps planning: schedules, water balance, ETo.
//!
//! The binding field list is not the printed model, which predates the decree:
//! art. 5.d and 5.e both redirect to RD 1311/2012 Anexo III Parte I sección C.
//! `docs/cuaderno-print.md` transcribes it letter by letter, and the schema
//! comments cite their letter.
//!
//! Layout:
//!   * [`db`]         — embedded migrations + the backup shape probe's share.
//!   * [`models`]     — Rust structs mirroring the schema, plus `New*`/`Update*`.
//!   * [`repository`] — CRUD with audit logging, one submodule per register.
//!   * [`agronomy`]   — the arithmetic section 7.1 is assembled from.
//!   * [`catalogue`]  — the coded fields' picker lists, read from core's store.
//!   * [`siex`]       — neutral-code ↔ SIEX-code mapping for the Spanish export.
//!   * [`error`]      — `FertilisationError` / `Result`.
//!
//! This crate depends on `terrazgo-core` and on nothing else in the workspace:
//! modules never depend on each other. Where it needs something module-cue
//! also needs — units of measure — that thing lives in core.

pub mod agronomy;
pub mod catalogue;
pub mod db;
pub mod error;
pub mod models;
pub mod repository;
pub mod siex;

pub use db::{BACKUP_SHAPE, migration_set, migrations, open_in_memory};
pub use error::{FertilisationError, Result};
