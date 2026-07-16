// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo report crate: in-process PDF generation via Typst, shared by all
//! modules (the CUE cuaderno first; fertilisation plans, irrigation summaries
//! and cost reports later). See `docs/architecture.md` → "Report engine".
//!
//! Fully offline by construction: the Typst templates are embedded by their
//! owning module (`include_str!`), the fonts are embedded here
//! (`include_bytes!`), and no Typst-Universe package resolution is compiled
//! in — an `@preview` import in a template fails the compile loudly instead
//! of reaching for the network.
//!
//! # Contract for template authors
//!
//! - Pin the family: `#set text(font: "Liberation Sans")`. Typst treats an
//!   unknown family as a *warning* plus silent fallback, so [`render_pdf`]
//!   returns the warnings and each template's tests must assert they are
//!   empty (see this crate's own tests for the pattern).
//! - Data arrives as `sys.inputs`, converted from a `serde_json::Value`
//!   object: strings, ints, floats, bools, `null` → `none`, arrays and
//!   nested objects all map to their Typst counterparts.
//! - Report labels are per-country template content (Spanish for the
//!   official cuaderno), never UI i18n keys.

mod error;

pub use error::ReportError;

use serde_json::Value as JsonValue;
use typst::diag::SourceDiagnostic;
use typst::foundations::{Array, Dict, IntoValue, Str, Value};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;

/// The Liberation Sans faces embedded in the binary (~1.6 MB, OFL-1.1 — the
/// licence is vendored alongside the files in `fonts/`). Liberation Sans is
/// metric-compatible with Arial, the look of the official Spanish
/// administrative forms the reports reproduce.
static FONTS: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationSans-Regular.ttf"),
    include_bytes!("../fonts/LiberationSans-Bold.ttf"),
    include_bytes!("../fonts/LiberationSans-Italic.ttf"),
    include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"),
];

/// A successfully rendered document.
///
/// The manual `Debug` prints the byte COUNT, not the bytes — a failing test
/// must not dump a whole PDF into the terminal.
pub struct RenderedPdf {
    /// The complete PDF file, ready to write to disk.
    pub bytes: Vec<u8>,
    /// Number of pages in the document (for UI feedback — "3 páginas").
    pub page_count: usize,
    /// Typst compile warnings. An unknown font family lands HERE, not in an
    /// error — callers (and every template's tests) must treat a non-empty
    /// list as a defect, because the PDF was rendered with fallbacks.
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for RenderedPdf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderedPdf")
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("page_count", &self.page_count)
            .field("warnings", &self.warnings)
            .finish()
    }
}

/// Compile a Typst template with the given inputs and export it as PDF.
///
/// `inputs` must be a JSON object; it becomes the template's `sys.inputs`
/// dictionary. The whole pipeline is synchronous and CPU-bound — callers at
/// the Tauri boundary follow the long-running-command rule (`async fn`).
pub fn render_pdf(template: &str, inputs: &JsonValue) -> Result<RenderedPdf, ReportError> {
    let dict = json_to_dict(inputs)?;

    // The engine is rebuilt per call: parsing four faces is milliseconds,
    // and a stateless function beats caching until a profile says otherwise.
    let engine = TypstEngine::builder()
        .main_file(template.to_owned())
        .fonts(FONTS)
        .build();

    let warned = engine.compile_with_input::<_, PagedDocument>(dict);
    let warnings: Vec<String> = warned.warnings.iter().map(render_diagnostic).collect();
    let document = warned
        .output
        .map_err(|e| ReportError::Compile(e.to_string()))?;

    let bytes = typst_pdf::pdf(&document, &PdfOptions::default())
        .map_err(|diags| ReportError::Pdf(join_diagnostics(&diags)))?;

    Ok(RenderedPdf {
        bytes,
        page_count: document.pages().len(),
        warnings,
    })
}

/// One diagnostic as a readable line: the message, plus Typst's hints when
/// it offers any (they often name the available font families).
fn render_diagnostic(diag: &SourceDiagnostic) -> String {
    if diag.hints.is_empty() {
        diag.message.to_string()
    } else {
        // Hints are span-carrying values; only the text matters here.
        let hints: Vec<&str> = diag.hints.iter().map(|h| h.v.as_str()).collect();
        format!("{} ({})", diag.message, hints.join("; "))
    }
}

fn join_diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(render_diagnostic)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The root of `sys.inputs` must be an object — Typst's `sys.inputs` is a
/// dictionary by definition.
fn json_to_dict(inputs: &JsonValue) -> Result<Dict, ReportError> {
    match inputs {
        JsonValue::Object(map) => map
            .iter()
            .map(|(k, v)| Ok((Str::from(k.as_str()), json_to_value(v)?)))
            .collect(),
        other => Err(ReportError::InvalidInputs(format!(
            "template inputs must be a JSON object, got: {other}"
        ))),
    }
}

fn json_to_value(json: &JsonValue) -> Result<Value, ReportError> {
    Ok(match json {
        JsonValue::Null => Value::None,
        JsonValue::Bool(b) => (*b).into_value(),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_value()
            } else if let Some(f) = n.as_f64() {
                // u64 values above i64::MAX land here; for display data the
                // float precision loss is acceptable and explicit.
                f.into_value()
            } else {
                // serde_json numbers always answer as_f64 unless the
                // arbitrary-precision feature is on; defensive, not dead.
                return Err(ReportError::InvalidInputs(format!(
                    "unrepresentable number: {n}"
                )));
            }
        }
        JsonValue::String(s) => s.as_str().into_value(),
        JsonValue::Array(items) => Value::Array(
            items
                .iter()
                .map(json_to_value)
                .collect::<Result<Array, _>>()?,
        ),
        // json_to_dict's not-an-object arm is unreachable from here (this
        // value IS an object); only nested-number errors can propagate.
        JsonValue::Object(_) => Value::Dict(json_to_dict(json)?),
    })
}
