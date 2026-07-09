// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo core crate: the entities and infrastructure shared by the shell and
//! every module. Modules depend on this crate; this crate depends on no module
//! and never on the app shell.
//!
//! Layout:
//!   * [`db`]         — embedded core migrations (the shell composes these FIRST
//!     into the global sequence) + test helpers.
//!   * [`models`]     — core entity structs (farm, plot) + `New*` insert inputs.
//!   * [`repository`] — CRUD for the core entities, with audit logging.
//!   * [`audit`]      — append-only `record_change` helpers, used by every crate
//!     that writes synced user data.
//!   * [`backup`]     — export a consistent snapshot / validate before import.
//!   * [`date`]       — timezone-safe date maths (`jiff`), shared app-wide.
//!   * [`geojson`]    — pure-parsing GeoJSON boundary validation (no I/O), used
//!     by the `geo_feature` write path and reused by `terrazgo-geo`'s importer.
//!   * [`error`]      — `CoreError` / `Result`.

pub mod audit;
pub mod backup;
pub mod date;
pub mod db;
pub mod error;
pub mod geojson;
pub mod models;
pub mod repository;

pub use db::{migration_set, migrations, open_in_memory};
pub use error::{CoreError, Result};
