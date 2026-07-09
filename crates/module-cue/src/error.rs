// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the CUE module. `thiserror` keeps this a library-style error;
//! `anyhow` is reserved for the Tauri command boundary (docs/architecture.md → Life of a command).

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, CueError>;

#[derive(Debug, Error)]
pub enum CueError {
    /// `#[from]` lets `?` convert a `rusqlite::Error` into a `CueError` automatically.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// File-system work outside SQLite; mirrors `CoreError::Io`.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("record not found")]
    NotFound,

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    #[error("product {product_id} has no authorisation for country '{country}'")]
    AuthorisationMissing { product_id: String, country: String },

    #[error("country '{provided}' does not match the farm's country '{farm}'")]
    CountryMismatch { provided: String, farm: String },

    #[error("plot {plot_id} is not on farm {farm_id}")]
    PlotNotOnFarm { plot_id: String, farm_id: String },

    #[error("no PHI days available: product has no default and none was supplied")]
    MissingPhiDays,

    /// Mirrors `CoreError::Invalid` (input rejected before touching the
    /// database). The payload is a stable machine code, not display text —
    /// see the `CoreError::Invalid` docs for the contract.
    #[error("invalid input: {0}")]
    Invalid(&'static str),
}

/// Variant-preserving conversion from the core crate's error, so `?` works on
/// `terrazgo-core` calls (date maths, audit helpers, farm/plot inserts) without
/// changing what callers and tests match on: a core `InvalidDate` stays a
/// `CueError::InvalidDate`, never an opaque wrapped variant.
impl From<terrazgo_core::CoreError> for CueError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => CueError::Sqlite(e),
            CoreError::Migration(e) => CueError::Migration(e),
            CoreError::Json(e) => CueError::Json(e),
            CoreError::Io(e) => CueError::Io(e),
            CoreError::NotFound => CueError::NotFound,
            CoreError::InvalidDate(d) => CueError::InvalidDate(d),
            CoreError::Invalid(msg) => CueError::Invalid(msg),
        }
    }
}
