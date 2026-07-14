// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the core crate. `thiserror` keeps this a library-style error;
//! `anyhow` is reserved for the Tauri command boundary (docs/architecture.md → Life of a command).
//!
//! Module crates wrap this in their own error type with a variant-preserving
//! `From` impl (see `CueError`), so a `CoreError::NotFound` stays a `NotFound`
//! to their callers and tests.

use thiserror::Error;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    /// `#[from]` lets `?` convert a `rusqlite::Error` into a `CoreError` automatically.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// File-system work outside SQLite (backup export/import).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("record not found")]
    NotFound,

    /// Input rejected before touching the database (empty name, non-positive
    /// area, …). The payload is a stable machine code (`empty_name`,
    /// `nonpositive_area`), not display text: the command boundary forwards it
    /// to the frontend, which renders the `error.invalid.<code>` i18n key.
    #[error("invalid input: {0}")]
    Invalid(&'static str),

    #[error("invalid date '{0}' (expected YYYY-MM-DD)")]
    InvalidDate(String),

    /// A vendored catalogue file failed to parse — a packaging defect, never
    /// user input, so this is a plain message (maps to `internal` at the
    /// command boundary), not an `Invalid` machine code.
    #[error("catalogue data error: {0}")]
    Catalogue(String),
}
