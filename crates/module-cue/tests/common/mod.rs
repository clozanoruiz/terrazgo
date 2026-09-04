// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared plumbing for this crate's integration tests.
//!
//! What only needs core lives in `terrazgo-testkit` and is re-exported here, so
//! a test file has one `mod common;` and one `use` line. What needs this
//! module's schema lives here — and nothing that needs *another* module's
//! schema may live in either, which is what keeps the module layering honest.
//!
//! The per-register fixtures stay in their own files on purpose: unlike the
//! other modules', they are not the same fixture with different names — the
//! alerts one builds a treatment, the analysis one a crop, the non-field one a
//! product with its authorisation. Sharing them would mean each test file
//! creating rows it does not use.

// Each test binary compiles this whole module and uses a subset of it, so what
// one binary does not touch is not dead code — it is the other binaries' half
// of the shared helper. `unused_imports` covers the re-exports below for the
// same reason.
#![allow(dead_code, unused_imports)]
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;

/// The treatment fixture the five `repository_*.rs` files share.
pub mod treatment;

/// A holding with years of history, for the tests that ask whether a query's
/// cost grows with the record book.
pub mod scale;

pub use terrazgo_testkit::{
    CoreFixture, FarmWithPlots, PlotSpec, TempFile, farm_with_plots, last_change,
};

/// A migrated in-memory database — core's schema plus this module's — with the
/// vendored FEGA catalogue snapshot imported, the state a running app is always
/// in.
///
/// Deliberately not what every test opens. Importing the snapshot parses 1.6 MB
/// of vendored CSV per call, but the cost is the lesser reason: opening through
/// here is the statement *this test resolves a code to a label*, and a file
/// where some tests have catalogues and some do not leaves the next reader
/// guessing which kind they are copying.
pub fn db_with_catalogues() -> Connection {
    let mut conn = module_cue::open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}
