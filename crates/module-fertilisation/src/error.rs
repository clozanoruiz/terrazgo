// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the fertilisation module. `thiserror` keeps this a
//! library-style error; `anyhow` is reserved for the Tauri command boundary
//! (docs/architecture.md → Life of a command).

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, FertilisationError>;

#[derive(Debug, Error)]
pub enum FertilisationError {
    /// `#[from]` lets `?` convert a `rusqlite::Error` automatically.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("record not found")]
    NotFound,

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    #[error("catalogue data error: {0}")]
    Catalogue(String),

    #[error("plot {plot_id} is not on farm {farm_id}")]
    PlotNotOnFarm { plot_id: String, farm_id: String },

    /// Mirrors `CoreError::Invalid` (input rejected before touching the
    /// database). The payload is a stable machine code, not display text.
    #[error("invalid input: {0}")]
    Invalid(&'static str),
}

/// Variant-preserving conversion from the core crate's error, so `?` works on
/// `terrazgo-core` calls (date maths, audit helpers) without changing what
/// callers and tests match on — the `From<CoreError> for CueError` precedent.
impl From<terrazgo_core::CoreError> for FertilisationError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => FertilisationError::Sqlite(e),
            CoreError::Migration(e) => FertilisationError::Migration(e),
            CoreError::Json(e) => FertilisationError::Json(e),
            CoreError::Io(e) => FertilisationError::Io(e),
            CoreError::NotFound => FertilisationError::NotFound,
            CoreError::InvalidDate(d) => FertilisationError::InvalidDate(d),
            CoreError::Invalid(msg) => FertilisationError::Invalid(msg),
            CoreError::Catalogue(msg) => FertilisationError::Catalogue(msg),
        }
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
impl terrazgo_core::Classify for FertilisationError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            FertilisationError::NotFound => ("not_found".into(), json!({})),
            FertilisationError::InvalidDate(date) => {
                ("invalid_date".into(), json!({ "date": date }))
            }
            FertilisationError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            FertilisationError::PlotNotOnFarm { plot_id, farm_id } => (
                "plot_not_on_farm".into(),
                json!({ "plot_id": plot_id, "farm_id": farm_id }),
            ),
            FertilisationError::Sqlite(_)
            | FertilisationError::Migration(_)
            | FertilisationError::Json(_)
            | FertilisationError::Io(_)
            | FertilisationError::Catalogue(_) => ("internal".into(), json!({})),
        }
    }
}
