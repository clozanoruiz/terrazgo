// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo core crate: the entities and infrastructure shared by the shell and
//! every module. Modules depend on this crate; this crate depends on no module
//! and never on the app shell.
//!
//! Layout:
//!   * [`db`]         — embedded core migrations (the shell composes these FIRST
//!     into the global sequence) + test helpers, and [`Database`], the handle
//!     every long-lived connection is held by so shutdown can close it.
//!   * [`models`]     — core entity structs (farm, plot) + `New*` insert inputs.
//!   * [`repository`] — CRUD for the core entities, with audit logging.
//!   * [`audit`]      — append-only `record_change` helpers, used by every crate
//!     that writes synced user data.
//!   * [`backup`]     — export a consistent snapshot / validate before import.
//!   * [`catalogue`]  — imported reference catalogues (vendored FEGA SIEX
//!     snapshot, upsert-only `ensure_catalogues` at startup).
//!   * [`date`]       — timezone-safe date maths (`jiff`), shared app-wide.
//!   * [`settings`]   — device-local app settings (`settings.json`): typed
//!     struct, atomic write, tolerant read. Not in the database, not in
//!     backups, no secrets.
//!   * [`geojson`]    — pure-parsing GeoJSON boundary validation (no I/O), used
//!     by the `geo_feature` write path and reused by `terrazgo-geo`'s importer.
//!   * [`error`]      — `CoreError` / `Result`.
//!
//! # Open, write, read back
//!
//! ```
//! use terrazgo_core::models::{NewFarm, NewPlot};
//! use terrazgo_core::repository as repo;
//!
//! // Core's schema alone, in memory, with foreign keys on. The app opens its
//! // real database through the shell's composed runner instead — core steps
//! // first, then each registered module's.
//! let mut conn = terrazgo_core::open_in_memory()?;
//!
//! // Every write takes an actor: the active user profile's id, or None. It is
//! // a parameter rather than connection state so it cannot be forgotten past
//! // the compiler, and so a backup import swapping the connection cannot
//! // silently drop it.
//! let farm = repo::insert_farm(
//!     &mut conn,
//!     NewFarm {
//!         name: "Finca La Vega".into(),
//!         owner_name: None,
//!         owner_tax_id: None,
//!         country_code: "es".into(),
//!         es: None,
//!     },
//!     Some("profile-1"),
//! )?;
//!
//! let plot = repo::insert_plot(
//!     &mut conn,
//!     NewPlot {
//!         farm_id: farm.id.clone(),
//!         name: "El Prado".into(),
//!         area_ha: Some(4.0),
//!         es: None,
//!     },
//!     Some("profile-1"),
//! )?;
//!
//! assert_eq!(repo::list_plots(&conn, &farm.id)?.len(), 1);
//!
//! // Both writes are in the append-only audit log, which is also the delta
//! // source Stage-2 sync will read. The payload is a COMPLETE row image: a
//! // receiving device must be able to rebuild the row from it alone.
//! let logged: i64 = conn.query_row(
//!     "SELECT COUNT(*) FROM record_change WHERE entity_id IN (?1, ?2)",
//!     [&farm.id, &plot.id],
//!     |r| r.get(0),
//! )?;
//! assert_eq!(logged, 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! That is the only repository example in this crate, and deliberately so: a
//! doc example per repository function would spin a database each, for
//! documentation whose real specification is the test beside it. Pure
//! functions — [`date`], [`geojson`] — carry a worked example each, because
//! there the example IS the specification and costs nothing to run.

pub mod audit;
pub mod backup;
pub mod catalogue;
pub mod date;
pub mod db;
pub mod error;
pub mod geojson;
pub mod models;
pub mod repository;
pub mod settings;
pub mod sql;

pub use db::{Database, DbGuard, Unavailable, migration_set, migrations, open_in_memory};
pub use error::{Classify, CoreError, Result};
