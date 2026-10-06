// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error type for the core crate. `thiserror` keeps this a library-style error;
//! `anyhow` is reserved for the Tauri command boundary (docs/architecture.md → Life of a command).
//!
//! Module crates wrap this in their own error type with a variant-preserving
//! `From` impl (see `PhytosanitaryError`), so a `CoreError::NotFound` stays a `NotFound`
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

    /// A write could not be stamped for the sync log: the connection carries no
    /// device identity, or the clock is outside what the stamp can encode.
    /// A defect of whoever opened the connection, or of the machine — never
    /// user input, so like `Catalogue` it maps to `internal`.
    #[error("change stamp error: {0}")]
    Stamp(&'static str),

    /// A logged row disagrees with the aggregate map (`sync::install_shape`):
    /// its table is not in the map, the map says it is never synced, or the
    /// row names a register other than the one its own values place it in.
    /// Always a code defect — the message names the table so the failing test
    /// says where — and refused rather than written, because a row filed under
    /// the wrong register would be merged as part of it forever after.
    #[error("sync shape violation: {0}")]
    ShapeViolation(String),

    /// An incoming bundle carries a stamp further ahead of this device's clock
    /// than the merge may adopt (`sync::Hlc::MAX_SKEW_MS`, two hours).
    ///
    /// A variant of its own rather than an `Invalid` code because the farmer
    /// has to be told WHICH device and BY HOW MUCH: past a couple of hours a
    /// clock is usually wrong by days or years, which means the application
    /// dates that device recorded are wrong too — a defect in the book itself,
    /// larger than any sync (docs/sync.md → How far ahead a stamp may be).
    #[error("{peer}'s clock is {ahead_ms} ms ahead of this device")]
    PeerClockAhead { peer: String, ahead_ms: i64 },

    /// An incoming book would take the name another live book of its farm has
    /// on this device (docs/sync.md → Seasons created on two devices).
    ///
    /// A variant of its own rather than an `Invalid` code because the person
    /// has to be told WHICH book to rename, and nothing on screen can say it:
    /// the refusal comes before anything in the file is applied, so the other
    /// book is not on this device yet. A book's name is only unique within its
    /// farm, so the farm is named too.
    #[error("{farm}'s book {label} would take a name already in use on this device")]
    SeasonLabelCollision { farm: String, label: String },
}

/// How a boundary error names itself to the frontend: a stable code, and the
/// params its localized message interpolates.
///
/// Implemented by every error type that can reach a Tauri command, **in the
/// crate that owns the error** rather than in the shell. Two reasons: the code
/// and its params are the error's own knowledge, and the exhaustive match then
/// sits beside the enum, so adding a variant is a compile error in the crate
/// where it was added instead of a silent fall-through to `internal` in a shell
/// file the module author never opens.
///
/// `internal` is the deliberate catch-all for anything a user cannot act on. It
/// has NO dictionary entry — the frontend prefixes a localized intro line to
/// the raw message so nothing is swallowed — and the i18n contract test
/// enforces that absence.
pub trait Classify {
    /// The stable code (`not_found`, `invalid.empty_name`, `geo_offline`) and
    /// its params as a JSON object.
    fn classify(&self) -> (String, serde_json::Value);
}

impl Classify for CoreError {
    fn classify(&self) -> (String, serde_json::Value) {
        use serde_json::json;
        match self {
            CoreError::NotFound => ("not_found".into(), json!({})),
            CoreError::InvalidDate(date) => ("invalid_date".into(), json!({ "date": date })),
            CoreError::Invalid(code) => (format!("invalid.{code}"), json!({})),
            // `hours` is what the message says and `ahead_ms` is the fact it
            // was rounded from, kept for a diagnostic. Floored, so "more than
            // {hours} hours ahead" is true rather than generous.
            CoreError::PeerClockAhead { peer, ahead_ms } => (
                "invalid.peer_clock_ahead".into(),
                json!({
                    "peer": peer,
                    "ahead_ms": ahead_ms,
                    "hours": ahead_ms / (60 * 60 * 1000),
                }),
            ),
            CoreError::SeasonLabelCollision { farm, label } => (
                "invalid.season_label_collision".into(),
                json!({ "farm": farm, "label": label }),
            ),
            CoreError::Sqlite(_)
            | CoreError::Migration(_)
            | CoreError::Json(_)
            | CoreError::Io(_)
            | CoreError::Catalogue(_)
            | CoreError::Stamp(_)
            | CoreError::ShapeViolation(_) => ("internal".into(), json!({})),
        }
    }
}
