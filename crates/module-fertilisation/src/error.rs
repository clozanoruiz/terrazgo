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

    /// Mirrors `CoreError::Stamp` (a write could not be stamped for the sync
    /// log — a defect of the connection's opener or of the clock, never input).
    #[error("change stamp error: {0}")]
    Stamp(&'static str),

    /// Mirrors `CoreError::ShapeViolation` (a logged row the aggregate map
    /// refuses — always a code defect).
    #[error("sync shape violation: {0}")]
    ShapeViolation(String),

    /// Mirrors `CoreError::PeerClockAhead` (an incoming bundle stamped further
    /// ahead than the merge may adopt). Nothing in this crate can raise it —
    /// only the sync transport applies bundles — but the conversion below is
    /// variant-preserving and total, which is what keeps a new core variant a
    /// compile error here rather than a silent reshaping.
    #[error("{peer}'s clock is {ahead_ms} ms ahead of this device")]
    PeerClockAhead { peer: String, ahead_ms: i64 },

    /// Mirrors `CoreError::SeasonLabelCollision` (an incoming book would take a
    /// name already in use on this device). Raised only by the sync transport,
    /// and here for the same reason as `PeerClockAhead`.
    #[error("{farm}'s book {label} would take a name already in use on this device")]
    SeasonLabelCollision { farm: String, label: String },

    #[error("plot {plot_id} is not on farm {farm_id}")]
    PlotNotOnFarm { plot_id: String, farm_id: String },

    /// Mirrors `CoreError::Invalid` (input rejected before touching the
    /// database). The payload is a stable machine code, not display text.
    #[error("invalid input: {0}")]
    Invalid(&'static str),
}

/// Variant-preserving conversion from the core crate's error, so `?` works on
/// `terrazgo-core` calls (date maths, audit helpers) without changing what
/// callers and tests match on — the `From<CoreError> for PhytosanitaryError` precedent.
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
            CoreError::Stamp(msg) => FertilisationError::Stamp(msg),
            CoreError::ShapeViolation(msg) => FertilisationError::ShapeViolation(msg),
            CoreError::PeerClockAhead { peer, ahead_ms } => {
                FertilisationError::PeerClockAhead { peer, ahead_ms }
            }
            CoreError::SeasonLabelCollision { farm, label } => {
                FertilisationError::SeasonLabelCollision { farm, label }
            }
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
            | FertilisationError::Catalogue(_)
            | FertilisationError::Stamp(_)
            | FertilisationError::ShapeViolation(_) => ("internal".into(), json!({})),
            FertilisationError::SeasonLabelCollision { farm, label } => (
                "invalid.season_label_collision".into(),
                json!({ "farm": farm, "label": label }),
            ),
            FertilisationError::PeerClockAhead { peer, ahead_ms } => (
                "invalid.peer_clock_ahead".into(),
                json!({
                    "peer": peer,
                    "ahead_ms": ahead_ms,
                    "hours": ahead_ms / (60 * 60 * 1000),
                }),
            ),
        }
    }
}
