// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for the spreadsheet half of the engine: that a described
//! workbook renders, and that author errors are refused loudly instead of
//! producing a lopsided sheet.
//!
//! What the *cells* end up containing is pinned where it is meaningful — on
//! the workbook description each module builds (see module-cue's
//! `cuaderno_workbook`) — rather than by unzipping the output here, which
//! would only re-test `rust_xlsxwriter`'s own serialisation.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use terrazgo_report::{Cell, Column, ReportError, Sheet, Workbook, render_xlsx};

fn sample_sheet() -> Sheet {
    let mut sheet = Sheet::new(
        "3.1 Tratamientos",
        vec![
            Column::new("Fecha", 12.0),
            Column::new("Superficie", 14.0),
            Column::new("Producto", 20.0),
        ],
    );
    sheet.push(vec![
        Cell::date(Some("2026-05-01")),
        Cell::Number(2.5),
        Cell::text("Fungitop"),
    ]);
    sheet
}

#[test]
fn renders_one_sheet_per_description() {
    let mut book = Workbook::new();
    book.push(sample_sheet());
    book.push(Sheet::new(
        "2.1 Parcelas",
        vec![Column::new("Parcela", 10.0)],
    ));

    let out = render_xlsx(&book).expect("workbook must render");
    assert_eq!(out.sheet_count, 2);
    // .xlsx is a zip container: the magic bytes prove a real archive came out.
    assert_eq!(&out.bytes[..2], b"PK", "output must be a zip archive");
    assert!(out.bytes.len() > 1000, "a two-sheet book is not a stub");
}

/// Absent values normalise to [`Cell::Empty`] at construction, so a blank
/// never reaches a sheet as `""`, `0` or a dash. Official forms leave blanks
/// for hand-filling; writing a zero there would be a false statement.
#[test]
fn absent_values_become_empty_cells() {
    assert_eq!(Cell::text(""), Cell::Empty);
    assert_eq!(Cell::number(None), Cell::Empty);
    assert_eq!(Cell::date(None), Cell::Empty);
    assert_eq!(Cell::date(Some("")), Cell::Empty);

    assert_eq!(Cell::text("x"), Cell::Text("x".into()));
    assert_eq!(Cell::number(Some(1.5)), Cell::Number(1.5));
    assert_eq!(
        Cell::date(Some("2026-05-01")),
        Cell::Date("2026-05-01".into())
    );
}

/// A stored date that cannot be parsed still has to reach the reader — the
/// same rule the PDF's date formatting follows. Losing a value to a
/// formatting failure is worse than showing it raw.
#[test]
fn an_unparseable_date_does_not_fail_the_export() {
    let mut sheet = Sheet::new("Hoja", vec![Column::new("Fecha", 12.0)]);
    sheet.push(vec![Cell::Date("no es una fecha".into())]);
    let mut book = Workbook::new();
    book.push(sheet);

    let out = render_xlsx(&book).expect("a bad date must not fail the export");
    assert_eq!(out.sheet_count, 1);
}

/// Excel forbids `[]:*?/\` in tab names, caps them at 31 characters and
/// rejects duplicates. Callers pass section titles verbatim, so the engine
/// repairs all three rather than failing — a report must not be blocked by a
/// tab name. (The repair itself is unit-tested next to the function.)
#[test]
fn awkward_sheet_names_are_repaired_not_rejected() {
    let mut book = Workbook::new();
    for name in [
        "3.1/3.2 [borrador]",
        "Un nombre larguísimo que supera de largo el límite de Excel",
        "Repetida",
        "Repetida",
        "",
    ] {
        book.push(Sheet::new(name, vec![Column::new("A", 8.0)]));
    }
    let out = render_xlsx(&book).expect("names must be repaired, not rejected");
    assert_eq!(out.sheet_count, 5);
}

/// Author errors are refused loudly instead of producing a lopsided sheet.
#[test]
fn a_row_wider_than_its_columns_is_rejected() {
    let mut sheet = Sheet::new("Hoja", vec![Column::new("A", 8.0)]);
    sheet.push(vec![Cell::text("a"), Cell::text("b")]);
    let mut book = Workbook::new();
    book.push(sheet);

    let err = render_xlsx(&book).expect_err("a too-wide row must fail");
    assert!(matches!(err, ReportError::Workbook(_)), "{err:?}");
}

#[test]
fn an_empty_workbook_is_rejected() {
    let err = render_xlsx(&Workbook::new()).expect_err("a workbook needs a sheet");
    assert!(matches!(err, ReportError::Workbook(_)), "{err:?}");
}

/// A short row leaves the rest of its columns blank — a section that only
/// fills its first cells is normal, not an error.
#[test]
fn a_short_row_renders() {
    let mut sheet = Sheet::new(
        "Hoja",
        vec![
            Column::new("A", 8.0),
            Column::new("B", 8.0),
            Column::new("C", 8.0),
        ],
    );
    sheet.push(vec![Cell::text("solo A")]);
    let mut book = Workbook::new();
    book.push(sheet);

    assert_eq!(
        render_xlsx(&book).expect("short row renders").sheet_count,
        1
    );
}

/// A sheet with headers but no data rows is the empty-register case: it must
/// still render (the form exists, it simply has nothing in it yet).
#[test]
fn a_sheet_with_no_rows_still_renders() {
    let mut book = Workbook::new();
    book.push(Sheet::new("Vacía", vec![Column::new("A", 8.0)]));
    assert_eq!(
        render_xlsx(&book).expect("empty sheet renders").sheet_count,
        1
    );
}
