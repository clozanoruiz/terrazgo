// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Report-engine errors. Library-style `thiserror` enum like every other
//! crate; the shell maps it at the command boundary when a consumer arrives.

/// Everything that can go wrong turning a template + data into a PDF.
///
/// All variants carry human-readable diagnostics rather than structured
/// codes: template failures are developer errors (a template ships inside
/// the binary, so a user can never fix one), and the boundary maps them to
/// `internal` where the raw message is exactly what a bug report needs.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// The inputs passed to the template were not representable as a Typst
    /// dictionary (root not a JSON object, or an unrepresentable number).
    #[error("invalid template inputs: {0}")]
    InvalidInputs(String),

    /// The template failed to compile (syntax error, failed `#assert`,
    /// missing input key). Carries the joined Typst diagnostics.
    #[error("template compilation failed: {0}")]
    Compile(String),

    /// The compiled document could not be exported as PDF.
    #[error("PDF export failed: {0}")]
    Pdf(String),
}
