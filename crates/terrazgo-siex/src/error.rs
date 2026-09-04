// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the SIEX export.
//!
//! The serializer borrowed `module_cue::Result` while it lived inside that
//! crate. That stopped being defensible the moment it read a second domain: a
//! descriptor is not a treatment, and a failure while serializing an irrigation
//! record must not surface as a CUE error. Same reasoning, and the same shape,
//! as `RecordbookError` — the two consumer crates are siblings.
//!
//! The variants are thin for the same reason the book's are: building a
//! descriptor is reading, mapping and formatting. `Invalid` carries the codes
//! this crate raises itself (`export_precheck_failed`, `export_code_unmappable`)
//! plus any forwarded from below, so a rejection keeps its machine code all the
//! way to the frontend dictionary instead of collapsing into `internal`.

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, SiexError>;

#[derive(Debug, Error)]
pub enum SiexError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// The farm or season the export was asked for does not exist.
    #[error("record not found")]
    NotFound,

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    /// A stable machine code rendered by the frontend as `error.invalid.<code>`.
    /// Same contract as `CoreError::Invalid`.
    #[error("invalid input: {0}")]
    Invalid(&'static str),

    /// A fault from a crate below with no user-explainable form, which building
    /// a descriptor cannot meaningfully raise (a migration failure, a corrupt
    /// vendored catalogue). One variant rather than a mirror of each module's
    /// write-path diagnostics: the boundary maps them all to `internal` anyway,
    /// and the original message rides along because `internal` is the one code
    /// the frontend shows verbatim.
    #[error("{0}")]
    Internal(String),
}

/// Variant-preserving conversions, so `?` works across the crates this one
/// reads without flattening their diagnostics — the `RecordbookError`
/// precedent. A `NotFound` raised while reading a cover is still a `NotFound`
/// when the export reports it.
impl From<terrazgo_core::CoreError> for SiexError {
    fn from(e: terrazgo_core::CoreError) -> Self {
        use terrazgo_core::CoreError;
        match e {
            CoreError::Sqlite(e) => SiexError::Sqlite(e),
            CoreError::Json(e) => SiexError::Json(e),
            CoreError::NotFound => SiexError::NotFound,
            CoreError::InvalidDate(d) => SiexError::InvalidDate(d),
            CoreError::Invalid(msg) => SiexError::Invalid(msg),
            other => SiexError::Internal(other.to_string()),
        }
    }
}

impl From<module_cue::CueError> for SiexError {
    fn from(e: module_cue::CueError) -> Self {
        use module_cue::CueError;
        match e {
            CueError::Sqlite(e) => SiexError::Sqlite(e),
            CueError::Json(e) => SiexError::Json(e),
            CueError::NotFound => SiexError::NotFound,
            CueError::InvalidDate(d) => SiexError::InvalidDate(d),
            CueError::Invalid(msg) => SiexError::Invalid(msg),
            other => SiexError::Internal(other.to_string()),
        }
    }
}

impl From<module_fertilisation::FertilisationError> for SiexError {
    fn from(e: module_fertilisation::FertilisationError) -> Self {
        use module_fertilisation::FertilisationError;
        match e {
            FertilisationError::Sqlite(e) => SiexError::Sqlite(e),
            FertilisationError::Json(e) => SiexError::Json(e),
            FertilisationError::NotFound => SiexError::NotFound,
            FertilisationError::InvalidDate(d) => SiexError::InvalidDate(d),
            FertilisationError::Invalid(msg) => SiexError::Invalid(msg),
            other => SiexError::Internal(other.to_string()),
        }
    }
}

impl From<module_ecoscheme::EcoschemeError> for SiexError {
    fn from(e: module_ecoscheme::EcoschemeError) -> Self {
        use module_ecoscheme::EcoschemeError;
        match e {
            EcoschemeError::Sqlite(e) => SiexError::Sqlite(e),
            EcoschemeError::Json(e) => SiexError::Json(e),
            EcoschemeError::NotFound => SiexError::NotFound,
            EcoschemeError::InvalidDate(d) => SiexError::InvalidDate(d),
            EcoschemeError::Invalid(msg) => SiexError::Invalid(msg),
            other => SiexError::Internal(other.to_string()),
        }
    }
}

/// The command boundary's view of this error (`terrazgo_core::Classify`).
/// Exhaustive on purpose: a new variant must be classified beside its
/// declaration rather than falling through a wildcard into `internal`.
impl terrazgo_core::Classify for SiexError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            SiexError::NotFound => ("not_found".into(), json!({})),
            SiexError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            SiexError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            SiexError::Sqlite(_) | SiexError::Json(_) | SiexError::Internal(_) => {
                ("internal".into(), json!({}))
            }
        }
    }
}
