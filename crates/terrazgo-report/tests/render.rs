// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for the report engine: the font/warning contract, the
//! JSON→`sys.inputs` mapping, and the layout-engine behaviours the crate was
//! chosen for (automatic page breaking of unbounded tables).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use terrazgo_report::{ReportError, render_pdf};

/// Every template must pin this family explicitly; the tests double as the
/// example of the pattern.
const SET_FONT: &str = "#set text(font: \"Liberation Sans\")\n";

#[test]
fn renders_minimal_pdf_without_warnings() {
    let template = format!("{SET_FONT}Hola, cuaderno.");
    let out = render_pdf(&template, &json!({})).expect("minimal template must render");
    // PDF header magic per ISO 32000-1 §7.5.2: files start with %PDF-.
    assert!(out.bytes.starts_with(b"%PDF-"), "output is not a PDF");
    assert!(out.bytes.len() > 1_000, "suspiciously small PDF");
    assert_eq!(out.page_count, 1);
    // The zero-warnings contract: with the family pinned and the faces
    // embedded, a clean compile proves the font wiring end-to-end.
    assert_eq!(out.warnings, Vec::<String>::new());
}

#[test]
fn unbounded_table_breaks_across_pages() {
    // The cuaderno's treatments register (official model section 3.1) is an
    // unbounded table — automatic page breaking with a repeating header is
    // the core reason a real layout engine was chosen. Pin it.
    let template = format!(
        "{SET_FONT}#table(columns: 2, table.header([Fecha], [Producto]), \
         ..range(200).map(i => ([fila #str(i)], [x])).flatten())"
    );
    let out = render_pdf(&template, &json!({})).expect("table template must render");
    assert!(
        out.page_count >= 2,
        "200 rows must not fit on one page (got {} page(s))",
        out.page_count
    );
    assert_eq!(out.warnings, Vec::<String>::new());
}

#[test]
fn unknown_font_family_produces_a_warning() {
    // The tripwire mechanism itself: if a template names a family the
    // embedded font book lacks, Typst falls back and warns — and the warning
    // must surface to the caller, otherwise the zero-warnings tests above
    // prove nothing.
    let template = "#set text(font: \"No Such Font\")\nHola.";
    let out = render_pdf(template, &json!({})).expect("fallback still renders");
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("unknown font family")),
        "expected an unknown-font-family warning, got: {:?}",
        out.warnings
    );
}

#[test]
fn spanish_glyphs_present_in_every_embedded_face() {
    // Spanish official documents need the full diacritic set, the euro sign,
    // ordinal indicators and inverted punctuation. Check the cmap of every
    // vendored face so coverage is pinned to the font files, not to Typst's
    // (silent) tofu fallback — through typst's own Font parser, the exact
    // code path rendering uses.
    let faces: [(&str, &[u8]); 4] = [
        (
            "Regular",
            include_bytes!("../fonts/LiberationSans-Regular.ttf"),
        ),
        ("Bold", include_bytes!("../fonts/LiberationSans-Bold.ttf")),
        (
            "Italic",
            include_bytes!("../fonts/LiberationSans-Italic.ttf"),
        ),
        (
            "BoldItalic",
            include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"),
        ),
    ];
    let needed = "áéíóúüñÁÉÍÓÚÜÑçÇ€ªº¿¡«»–";
    for (name, bytes) in faces {
        let font = typst::text::Font::new(typst::foundations::Bytes::new(bytes), 0)
            .expect("vendored face must parse");
        // The family name is what `#set text(font: ...)` matches against;
        // pin it so a bad font swap can't silently strand every template.
        assert_eq!(
            font.info().family,
            "Liberation Sans",
            "LiberationSans-{name} indexes under an unexpected family name"
        );
        for ch in needed.chars() {
            assert!(
                font.info().coverage.contains(ch as u32),
                "LiberationSans-{name} lacks a glyph for {ch:?}"
            );
        }
    }
}

#[test]
fn inputs_reach_the_template_with_types_intact() {
    // The template itself asserts on `sys.inputs`: a failed assert aborts
    // compilation, so a successful render proves every JSON shape (string,
    // int, float, bool, null, array, nested object) crossed the boundary.
    let template = format!(
        "{SET_FONT}\
         #assert(sys.inputs.farm == \"Las Vegas\")\n\
         #assert(sys.inputs.count == 3)\n\
         #assert(sys.inputs.area == 1.5)\n\
         #assert(sys.inputs.active)\n\
         #assert(sys.inputs.note == none)\n\
         #assert(sys.inputs.tags.len() == 2)\n\
         #assert(sys.inputs.tags.at(1) == \"b\")\n\
         #assert(sys.inputs.nested.code == \"ES\")\n\
         Datos recibidos."
    );
    let inputs = json!({
        "farm": "Las Vegas",
        "count": 3,
        "area": 1.5,
        "active": true,
        "note": null,
        "tags": ["a", "b"],
        "nested": { "code": "ES" },
    });
    let out = render_pdf(&template, &inputs).expect("typed inputs must all arrive");
    assert_eq!(out.warnings, Vec::<String>::new());
}

#[test]
fn root_inputs_must_be_an_object() {
    let err = render_pdf("Hola.", &json!(["not", "an", "object"])).unwrap_err();
    assert!(
        matches!(err, ReportError::InvalidInputs(_)),
        "expected InvalidInputs, got: {err:?}"
    );
}

#[test]
fn template_errors_surface_the_diagnostic() {
    let err = render_pdf("#assert(false, message: \"boom\")", &json!({})).unwrap_err();
    match err {
        ReportError::Compile(msg) => {
            assert!(msg.contains("boom"), "diagnostic lost: {msg}");
        }
        other => panic!("expected Compile, got: {other:?}"),
    }
}

#[test]
fn syntax_errors_are_compile_errors_not_panics() {
    let err = render_pdf("#let = broken(", &json!({})).unwrap_err();
    assert!(
        matches!(err, ReportError::Compile(_)),
        "expected Compile, got: {err:?}"
    );
}
