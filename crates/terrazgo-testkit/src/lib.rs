// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared test fixtures for the Terrazgo workspace: the plumbing that every
//! crate's `tests/` directory had been copying instead of sharing.
//!
//! It is a **dev-dependency only** — no shipping crate may name it — and it
//! **depends on `terrazgo-core` and nothing else**.
//!
//! That second rule is the design, not a detail of what the first extraction
//! needed. A module may never depend on another module, and each one pins that
//! with a `the_module_runs_on_core_alone` test. A testkit that grew a
//! `module-cue` dependency would let `module-fertilisation`'s tests reach
//! `module-cue`'s schema through the back door, and those tests would keep
//! passing while it was open — the guard is on the crate graph, and the testkit
//! would be inside it. So: **anything that needs a module's tables is not a
//! testkit fixture.** It belongs in that crate's own `tests/common`.
//!
//! What lives here is what only needs core:
//!   * [`fixtures`] — the land a register test runs on (season, farm, plots).
//!   * [`audit`]    — reading back the `record_change` row a write just logged.
//!   * [`files`]    — a temp path that cleans up after itself, panic or not.
//!   * [`queries`]  — what a call costs the database, so an N+1 and an
//!     unbounded result set are things a test can fail on rather than things a
//!     later audit has to measure.
//!
//! The fixtures build core rows on whatever connection they are handed, so a
//! module test opens through its own `open_in_memory()` (core + that module's
//! migrations) and passes the result straight in.

// This whole crate is test scaffolding: a fixture that cannot build its own
// preconditions is a test failure, and the panic names the line. clippy.toml's
// `allow-unwrap-in-tests` only reaches `#[test]` fns, never helpers like these,
// which is exactly why every test file in the workspace carries this same line.
#![allow(clippy::unwrap_used, clippy::expect_used)]

pub mod audit;
pub mod files;
pub mod fixtures;
pub mod queries;

pub use audit::last_change;
pub use files::TempFile;
pub use fixtures::{CoreFixture, FarmWithPlots, PlotSpec, farm_with_plots};
pub use queries::{QueryCost, query_cost};
