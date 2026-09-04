// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the record book.
//!
//! The book borrowed `module_cue::Result` when it was extracted from that crate
//! (slice A, 2026-08-07). That was fine while treatments were the only domain
//! it read; it stops being fine the moment a second module contributes
//! sections, because a document is not a treatment and must not report failures
//! as though it were.
//!
//! The variants are deliberately thin. Assembling a book is reading and
//! formatting: there is no validation to reject and no rule to violate, so
//! everything that can go wrong is either a database/render fault (internal) or
//! a missing row. `Invalid` exists only to carry codes forwarded from the
//! crates below, so a rejection raised deeper keeps its machine code all the
//! way to the frontend dictionary instead of collapsing into `internal`.

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, RecordbookError>;

#[derive(Debug, Error)]
pub enum RecordbookError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Rendering failed (template compile, font, PDF or workbook export).
    /// Always a developer error — templates and fonts ship inside the binary —
    /// so the boundary maps it to `internal`.
    #[error("report error: {0}")]
    Report(#[from] terrazgo_report::ReportError),

    /// The farm or season the book was asked for does not exist.
    #[error("record not found")]
    NotFound,

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    /// A stable machine code forwarded from a crate below, rendered by the
    /// frontend as `error.invalid.<code>`. Same contract as
    /// `CoreError::Invalid`.
    #[error("invalid input: {0}")]
    Invalid(&'static str),

    /// A fault from a crate below that has no user-explainable form and that
    /// reading a book cannot meaningfully raise (a migration failure, a file
    /// error, a corrupt vendored catalogue). Kept as one variant rather than
    /// mirrored one-for-one: the book would never emit most of them, and the
    /// boundary maps them all to `internal` anyway. The original message
    /// rides along, because `internal` is the one code the frontend shows
    /// verbatim.
    #[error("{0}")]
    Internal(String),
}

/// Variant-preserving conversions, so `?` works on the crates the book reads
/// without flattening their diagnostics. A `NotFound` raised while reading a
/// treatment is still a `NotFound` when the book reports it — the alternative,
/// an opaque wrapped variant, would turn every domain rejection into
/// `internal` at the command boundary.
impl From<terrazgo_core::CoreError> for RecordbookError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => RecordbookError::Sqlite(e),
            CoreError::Json(e) => RecordbookError::Json(e),
            CoreError::NotFound => RecordbookError::NotFound,
            CoreError::InvalidDate(d) => RecordbookError::InvalidDate(d),
            CoreError::Invalid(msg) => RecordbookError::Invalid(msg),
            other => RecordbookError::Internal(other.to_string()),
        }
    }
}

impl From<module_fertilisation::FertilisationError> for RecordbookError {
    fn from(e: module_fertilisation::FertilisationError) -> Self {
        use module_fertilisation::FertilisationError;
        match e {
            FertilisationError::Sqlite(e) => RecordbookError::Sqlite(e),
            FertilisationError::Json(e) => RecordbookError::Json(e),
            FertilisationError::NotFound => RecordbookError::NotFound,
            FertilisationError::InvalidDate(d) => RecordbookError::InvalidDate(d),
            FertilisationError::Invalid(msg) => RecordbookError::Invalid(msg),
            other => RecordbookError::Internal(other.to_string()),
        }
    }
}

impl From<module_ecoscheme::EcoschemeError> for RecordbookError {
    fn from(e: module_ecoscheme::EcoschemeError) -> Self {
        use module_ecoscheme::EcoschemeError;
        match e {
            EcoschemeError::Sqlite(e) => RecordbookError::Sqlite(e),
            EcoschemeError::Json(e) => RecordbookError::Json(e),
            EcoschemeError::NotFound => RecordbookError::NotFound,
            EcoschemeError::InvalidDate(d) => RecordbookError::InvalidDate(d),
            EcoschemeError::Invalid(msg) => RecordbookError::Invalid(msg),
            other => RecordbookError::Internal(other.to_string()),
        }
    }
}

impl From<module_cue::CueError> for RecordbookError {
    fn from(e: module_cue::CueError) -> Self {
        use module_cue::CueError;
        match e {
            CueError::Sqlite(e) => RecordbookError::Sqlite(e),
            CueError::Json(e) => RecordbookError::Json(e),
            CueError::NotFound => RecordbookError::NotFound,
            CueError::InvalidDate(d) => RecordbookError::InvalidDate(d),
            CueError::Invalid(msg) => RecordbookError::Invalid(msg),
            // The rest are write-path diagnostics (authorisation, country,
            // plot ownership, PHI inputs) that reading a book cannot raise,
            // plus the same non-user-explainable faults as above.
            other => RecordbookError::Internal(other.to_string()),
        }
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
impl terrazgo_core::Classify for RecordbookError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            RecordbookError::NotFound => ("not_found".into(), json!({})),
            RecordbookError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            RecordbookError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            RecordbookError::Sqlite(_)
            | RecordbookError::Json(_)
            | RecordbookError::Report(_)
            | RecordbookError::Internal(_) => ("internal".into(), json!({})),
        }
    }
}
