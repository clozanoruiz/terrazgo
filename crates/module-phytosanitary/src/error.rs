// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the phytosanitary module. `thiserror` keeps this a library-style error;
//! `anyhow` is reserved for the Tauri command boundary (docs/architecture.md → Life of a command).

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, PhytosanitaryError>;

#[derive(Debug, Error)]
pub enum PhytosanitaryError {
    /// `#[from]` lets `?` convert a `rusqlite::Error` into a `PhytosanitaryError` automatically.
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

    /// Mirrors `CoreError::Catalogue` (a vendored catalogue file failed to
    /// parse — a packaging defect, never user input).
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

    // A `Report` variant lived here until 2026-08-07. The printed book moved
    // to terrazgo-recordbook in slice A but kept borrowing this crate's
    // `Result`, so render failures still had to be expressible as a PhytosanitaryError.
    // The book now owns `RecordbookError`, and a treatment module has no
    // business describing a PDF failure — the variant and this crate's
    // dependency on terrazgo-report went with it.
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
/// `PhytosanitaryError::InvalidDate`, never an opaque wrapped variant.
impl From<terrazgo_core::CoreError> for PhytosanitaryError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => PhytosanitaryError::Sqlite(e),
            CoreError::Migration(e) => PhytosanitaryError::Migration(e),
            CoreError::Json(e) => PhytosanitaryError::Json(e),
            CoreError::Io(e) => PhytosanitaryError::Io(e),
            CoreError::NotFound => PhytosanitaryError::NotFound,
            CoreError::InvalidDate(d) => PhytosanitaryError::InvalidDate(d),
            CoreError::Invalid(msg) => PhytosanitaryError::Invalid(msg),
            CoreError::Catalogue(msg) => PhytosanitaryError::Catalogue(msg),
            CoreError::Stamp(msg) => PhytosanitaryError::Stamp(msg),
            CoreError::ShapeViolation(msg) => PhytosanitaryError::ShapeViolation(msg),
            CoreError::PeerClockAhead { peer, ahead_ms } => {
                PhytosanitaryError::PeerClockAhead { peer, ahead_ms }
            }
            CoreError::SeasonLabelCollision { farm, label } => {
                PhytosanitaryError::SeasonLabelCollision { farm, label }
            }
        }
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
impl terrazgo_core::Classify for PhytosanitaryError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            PhytosanitaryError::NotFound => ("not_found".into(), json!({})),
            PhytosanitaryError::InvalidDate(date) => {
                ("invalid_date".into(), json!({ "date": date }))
            }
            PhytosanitaryError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            PhytosanitaryError::AuthorisationMissing {
                product_id,
                country,
            } => (
                "authorisation_missing".into(),
                json!({ "product_id": product_id, "country": country }),
            ),
            PhytosanitaryError::CountryMismatch { provided, farm } => (
                "country_mismatch".into(),
                json!({ "provided": provided, "farm": farm }),
            ),
            PhytosanitaryError::PlotNotOnFarm { plot_id, farm_id } => (
                "plot_not_on_farm".into(),
                json!({ "plot_id": plot_id, "farm_id": farm_id }),
            ),
            PhytosanitaryError::MissingPhiDays => ("missing_phi_days".into(), json!({})),
            PhytosanitaryError::Sqlite(_)
            | PhytosanitaryError::Migration(_)
            | PhytosanitaryError::Json(_)
            | PhytosanitaryError::Io(_)
            | PhytosanitaryError::Catalogue(_)
            | PhytosanitaryError::Stamp(_)
            | PhytosanitaryError::ShapeViolation(_) => ("internal".into(), json!({})),
            PhytosanitaryError::SeasonLabelCollision { farm, label } => (
                "invalid.season_label_collision".into(),
                json!({ "farm": farm, "label": label }),
            ),
            PhytosanitaryError::PeerClockAhead { peer, ahead_ms } => (
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
