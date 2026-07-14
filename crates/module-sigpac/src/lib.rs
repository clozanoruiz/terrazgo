// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo SIGPAC module: the Spanish parcel provider, querying FEGA's
//! "Nube de SIGPAC" (sigpac-hubcloud.es, CC BY 4.0 — the surface FEGA
//! publishes for third-party applications; the visor's own services are
//! prohibited).
//!
//! Design rules (docs/architecture.md + docs/sigpac-integration.md):
//!   * This crate performs NO network I/O of its own. Every request goes
//!     through [`terrazgo_geo::fetch::cached_resource`] — the single
//!     sanctioned seam — so responses land in `geo-cache.db` and every
//!     lookup seen once keeps working offline.
//!   * Service gotcha (live-tested 2026-07-08): an unknown reference answers
//!     HTTP 200 with an EMPTY FeatureCollection, never 404 — "not found" is
//!     detected from the payload and surfaces as `Ok(None)`.
//!   * Consultas serve the CURRENT campaign only; `geo_feature.fetched_at`
//!     records when. Campaign tagging (`geo_feature.campaign`) waits for the
//!     code-lists service — the query responses do not name their campaign.
//!   * All persistence goes through core's `save_geo_feature` (validation,
//!     replace-within-source, audit) — this crate only shapes the data.
//!
//! Layout:
//!   * [`reference`] — the 7-part SIGPAC reference (parse, validate, path form).
//!   * [`models`]    — [`RecintoInfo`] + response parsing.
//!   * [`client`]    — recinfo lookups by reference and by point.
//!   * [`storage`]   — recinto → `geo_feature`, plot-ref read, dedup query.
//!   * [`service`]   — the composed operations the shell's commands wrap.

pub mod client;
pub mod models;
pub mod reference;
pub mod service;
pub mod storage;

pub use models::RecintoInfo;
pub use reference::SigpacRef;

use rusqlite_migration::M;

/// This module's steps in the composed global migration sequence. SIGPAC v1
/// stores nothing of its own (lookups land in core's `geo_feature`), so the
/// set is empty — the registration seam exists so later steps (land-use code
/// lookup tables, zone flags' campaign bookkeeping) join without shell edits.
pub fn migration_set() -> Vec<M<'static>> {
    Vec::new()
}
