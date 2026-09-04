// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared plumbing for this crate's integration tests.
//!
//! `terrazgo-testkit` is the workspace-wide half and is re-exported here so a
//! test file has one `mod common;` and one `use` line. What is specific to
//! core's own tests lives here.

// Each test binary compiles this whole module and uses a subset of it, so what
// one binary does not touch is not dead code — it is the other binaries' half
// of the shared helper. `unused_imports` covers the re-exports below for the
// same reason.
#![allow(dead_code, unused_imports)]
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo_core::models::{NewFarm, NewOperator, NewPlot, NewSeason};

pub use terrazgo_testkit::{
    CoreFixture, FarmWithPlots, PlotSpec, TempFile, farm_with_plots, last_change,
};

/// A migrated in-memory database at core's schema.
pub fn db() -> Connection {
    terrazgo_core::open_in_memory().unwrap()
}

/// The same, with the vendored FEGA catalogue snapshot imported — the state a
/// running app is always in.
///
/// Deliberately not what every test opens. Importing the snapshot parses 1.6 MB
/// of vendored CSV per call, but the cost is the lesser reason: opening through
/// here is the statement *this test resolves a code to a label*, and a file
/// where some tests have catalogues and some do not leaves the next reader
/// guessing which kind they are copying.
///
/// The tests of `ensure_catalogues` *itself* — idempotency, upsert-never-delete
/// — still call it by hand, because there the import is the subject rather than
/// the setup.
pub fn db_with_catalogues() -> Connection {
    let mut conn = db();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}

pub fn new_farm(name: &str) -> NewFarm {
    NewFarm {
        name: name.into(),
        owner_name: None,
        owner_tax_id: None,
        country_code: "es".into(),
        es: None,
    }
}

pub fn new_plot(farm_id: &str, name: &str) -> NewPlot {
    NewPlot {
        farm_id: farm_id.into(),
        name: name.into(),
        area_ha: Some(2.0),
        es: None,
    }
}

// ---------------------------------------------------------------------------
// Shared by more than one of the repository_*.rs files
// ---------------------------------------------------------------------------

pub fn new_season(campaign_year: i64, label: &str) -> NewSeason {
    NewSeason {
        campaign_year,
        label: label.into(),
        starts_on: None,
        ends_on: None,
    }
}

pub fn plain_operator(name: &str) -> NewOperator {
    NewOperator {
        full_name: name.into(),
        tax_id: None,
        licence_number: None,
        licence_level_code: None,
        licence_expiry_date: None,
    }
}
