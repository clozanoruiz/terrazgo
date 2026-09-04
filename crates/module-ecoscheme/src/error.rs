// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the eco-scheme module. `thiserror` keeps this a
//! library-style error; `anyhow` is reserved for the Tauri command boundary
//! (docs/architecture.md → Life of a command).

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, EcoschemeError>;

#[derive(Debug, Error)]
pub enum EcoschemeError {
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
impl From<terrazgo_core::CoreError> for EcoschemeError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => EcoschemeError::Sqlite(e),
            CoreError::Migration(e) => EcoschemeError::Migration(e),
            CoreError::Json(e) => EcoschemeError::Json(e),
            CoreError::Io(e) => EcoschemeError::Io(e),
            CoreError::NotFound => EcoschemeError::NotFound,
            CoreError::InvalidDate(d) => EcoschemeError::InvalidDate(d),
            CoreError::Invalid(msg) => EcoschemeError::Invalid(msg),
            CoreError::Catalogue(msg) => EcoschemeError::Catalogue(msg),
        }
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
///
/// Exhaustive on purpose — no wildcard arm — so a variant added to the enum
/// above is a compile error here rather than a silent fall-through to
/// `internal`, which the frontend renders as a raw untranslated message.
impl terrazgo_core::Classify for EcoschemeError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            EcoschemeError::NotFound => ("not_found".into(), json!({})),
            EcoschemeError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            EcoschemeError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            EcoschemeError::PlotNotOnFarm { plot_id, farm_id } => (
                "plot_not_on_farm".into(),
                json!({ "plot_id": plot_id, "farm_id": farm_id }),
            ),
            EcoschemeError::Sqlite(_)
            | EcoschemeError::Migration(_)
            | EcoschemeError::Json(_)
            | EcoschemeError::Io(_)
            | EcoschemeError::Catalogue(_) => ("internal".into(), json!({})),
        }
    }
}
