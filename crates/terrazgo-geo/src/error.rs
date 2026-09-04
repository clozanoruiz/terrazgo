// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the geo crate, mirroring the `CueError` conventions:
//! `thiserror` variants, stable machine codes in `Invalid`, and a
//! variant-preserving `From<CoreError>` so `?` across the core boundary keeps
//! error identity.

use terrazgo_core::CoreError;
use terrazgo_net::NetError;
use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, GeoError>;

#[derive(Debug, Error)]
pub enum GeoError {
    /// The upstream service answered with a non-success HTTP status.
    #[error("upstream returned HTTP {status}")]
    Http { status: u16 },

    /// The network itself failed (DNS, connect, timeout) — the offline case.
    /// Callers degrade: the protocol serves what the cache has, the UI keeps
    /// working with stored geometry.
    #[error("network unavailable: {0}")]
    Offline(String),

    /// Cache database errors (`geo-cache.db`).
    #[error("cache error: {0}")]
    Cache(#[from] rusqlite::Error),

    #[error("cache migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Unknown source id, unknown resource prefix, missing file entry.
    #[error("not found")]
    NotFound,

    /// Input rejected with a stable machine code (`boundary_file_unsupported`,
    /// `gpkg_unsupported_srs`, …) rendered as `error.invalid.<code>` i18n keys.
    #[error("invalid input: {0}")]
    Invalid(&'static str),

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    /// Mirrors `CoreError::Catalogue`; nothing in this crate raises it, the
    /// variant only exists so the conversion below stays variant-preserving.
    #[error("catalogue data error: {0}")]
    Catalogue(String),
}

/// The network seam's two cases map onto the two variants this crate already
/// had — the fetch layer keeps behaving exactly as it did while the agent
/// lived inside it, and `empty_on_404` still gets a real `Http { status }` to
/// match on.
impl From<NetError> for GeoError {
    fn from(err: NetError) -> Self {
        match err {
            NetError::Http { status } => GeoError::Http { status },
            NetError::Offline(reason) => GeoError::Offline(reason),
        }
    }
}

/// Variant-preserving: a core `NotFound` stays `NotFound`, a core
/// `Invalid(code)` keeps its machine code (e.g. the GeoJSON validator's
/// `geometry_invalid`), so callers and the command boundary match on the
/// same identities regardless of which crate raised them.
impl From<CoreError> for GeoError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::Sqlite(e) => GeoError::Cache(e),
            CoreError::Migration(e) => GeoError::Migration(e),
            CoreError::Json(e) => GeoError::Json(e),
            CoreError::Io(e) => GeoError::Io(e),
            CoreError::NotFound => GeoError::NotFound,
            CoreError::Invalid(code) => GeoError::Invalid(code),
            CoreError::InvalidDate(s) => GeoError::InvalidDate(s),
            CoreError::Catalogue(s) => GeoError::Catalogue(s),
        }
    }
}

/// No cache connection — poisoned by a panic mid-write, or closed at shutdown
/// — is the offline case, not a hard failure: the cache is exactly the thing
/// that is unavailable, and every caller here already degrades gracefully when
/// it cannot be read. The alternative, propagating a panic or a `Cache` error
/// out of a tile request, would take the map down over a cache miss.
impl From<terrazgo_core::Unavailable> for GeoError {
    fn from(err: terrazgo_core::Unavailable) -> Self {
        GeoError::Offline(err.to_string())
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
impl terrazgo_core::Classify for GeoError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            GeoError::NotFound => ("not_found".into(), json!({})),
            GeoError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            GeoError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            // The two user-explainable network outcomes: the service said no,
            // or there is no network. Both leave the app fully usable.
            GeoError::Http { status } => ("geo_http".into(), json!({ "status": status })),
            // The transport detail (DNS, TLS, refused, timeout…) rides along:
            // "offline" is a diagnosis the user must be able to dispute — a
            // firewalled or proxied machine is online for the browser and
            // unreachable for us, and the reason string is the only evidence.
            GeoError::Offline(reason) => ("geo_offline".into(), json!({ "reason": reason })),
            GeoError::Cache(_)
            | GeoError::Migration(_)
            | GeoError::Json(_)
            | GeoError::Io(_)
            | GeoError::Catalogue(_) => ("internal".into(), json!({})),
        }
    }
}
