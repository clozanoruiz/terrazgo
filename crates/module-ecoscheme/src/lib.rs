// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo eco-scheme module: grazing, cultural operations and soil covers.
//!
//! The record book has **three** decrees. RD 1311/2012 governs the
//! phytosanitary registers (module-cue) and RD 1051/2022 the fertilisation,
//! plan de abonado and irrigation ones (module-fertilisation). RD 1054/2022
//! anexo II closes its list of the cuaderno's contents with *"otros aspectos
//! que se recojan en la respectiva normativa sectorial"*, and **RD 1048/2022**
//! is that norm for anyone claiming an ecorrégimen: ten clauses ordering an
//! annotation in the cuaderno, most within one month of the activity. The
//! printed model renders them as section 9.
//!
//! # Derived from the decree, not from the form
//!
//! The methodological rule this module exists under, established by the
//! 2026-08-11 completeness audit: **a register is derived from the decree,
//! never from the form that renders it.** Reading the printed model would have
//! lost anexo IV's duty entirely (it has no page), collapsed art. 42's three
//! annotations and their three deadlines into one row, and printed three of the
//! five dates art. 45.2 names. So the crate ships three registers shaped like
//! the decree's groupings — which are also the exchange format's own blocks —
//! rather than five shaped like the model's sub-tables.
//!
//! # Naming
//!
//! Identifiers spell it `ecoscheme` throughout, per the English-identifiers
//! rule; Spanish *ecorrégimen* belongs to values only — dictionary entries,
//! printed labels, the model's own section title. When sweeping this domain,
//! **grep both spellings**: the hyphenated English and the accented Spanish do
//! not match each other.
//!
//! Layout:
//!   * [`db`]         — embedded migrations + the backup shape probe's share.
//!   * [`models`]     — Rust structs mirroring the schema, plus `New*`/`Update*`.
//!   * [`repository`] — CRUD with audit logging, one submodule per register.
//!   * [`catalogue`]  — the coded fields' picker lists, read from core's store.
//!   * [`siex`]       — neutral-code ↔ SIEX-code mapping for the Spanish export.
//!   * [`error`]      — `EcoschemeError` / `Result`.
//!
//! This crate depends on `terrazgo-core` and on nothing else in the workspace:
//! modules never depend on each other, and `tests/migrations.rs` asserts it by
//! checking that the other modules' tables are absent from its own schema.

pub mod catalogue;
pub mod db;
pub mod error;
pub mod models;
pub mod repository;
pub mod siex;

pub use db::{BACKUP_SHAPE, migration_set, migrations, open_in_memory};
pub use error::{EcoschemeError, Result};
