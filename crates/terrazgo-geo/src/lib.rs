// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo geo crate: the country-neutral plumbing behind the shared map —
//! tile/resource caching, base-map source registry, style building, and
//! boundary-file import. No user data lives here (that is core's `geo_feature`
//! table); no UI lives here (that is the shell's `MapCanvas`).
//!
//! Design rules (docs/architecture.md + docs/sigpac-integration.md):
//!   * All network I/O in the app funnels through this crate's cache-through
//!     [`fetch`]; core and modules stay offline-only. With no network the map
//!     degrades to cached tiles + stored geometry — the app keeps working.
//!   * The cache is a SEPARATE SQLite file (`geo-cache.db`): bulky, derived,
//!     re-fetchable — never in backups, `record_change`, or sync.
//!   * No `sigpac_` names here: Spain-specific service code arrives later as
//!     `module-sigpac`; this crate only knows generic sources and formats.
//!
//! Layout:
//!   * [`db`]      — open/migrate the cache database.
//!   * [`sources`] — base-map source & resource registry (data, not code).
//!   * [`fetch`]   — cache-through tile/resource fetching (`ureq`).
//!   * [`style`]   — MapLibre style JSON building/rewriting so the webview
//!     only ever sees `geo://` URLs.
//!   * [`import`]  — boundary files (GeoJSON, GeoPackage) → GeoJSON geometries.
//!   * [`error`]   — `GeoError` / `Result`.

pub mod db;
pub mod error;
pub mod fetch;
pub mod import;
pub mod sources;
pub mod style;

pub use error::{GeoError, Result};
