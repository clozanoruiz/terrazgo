// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Spreadsheet output: a declarative workbook description rendered to .xlsx.
//!
//! The counterpart of [`crate::render_pdf`], and deliberately the same shape
//! of seam: callers describe *what* the document contains, the engine owns
//! *how* it looks (bold frozen header, autofilter, column widths, date
//! format). No module ever touches `rust_xlsxwriter` directly, so the
//! spreadsheet look stays consistent across the reports modules will add.
//!
//! # Why cells are typed
//!
//! The PDF template receives pre-formatted Spanish strings because it only
//! does layout. A spreadsheet must not: a farmer sorting by date, filtering a
//! product or summing treated hectares needs real dates and real numbers.
//! [`Cell`] therefore carries values, not display text, and numbers are
//! written with no number format at all — Excel renders them with the
//! reader's own locale, which is how a Spanish user gets decimal commas
//! without us hard-coding them.

use crate::error::ReportError;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook as XlsxWorkbook};

/// Excel's own limit on a sheet-tab name.
const MAX_SHEET_NAME: usize = 31;

/// Characters Excel rejects in a sheet-tab name.
const FORBIDDEN_IN_SHEET_NAME: [char; 7] = ['[', ']', ':', '*', '?', '/', '\\'];

/// One value in a spreadsheet cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// No value. The official forms leave blanks for hand-filling, and a
    /// blank cell is also the honest answer when data is genuinely unknown —
    /// never write a zero or a dash in its place.
    Empty,
    Text(String),
    Number(f64),
    /// An ISO `YYYY-MM-DD` date, written as a real Excel date so it sorts and
    /// filters as one. An unparseable value falls back to text rather than
    /// being dropped — the same rule the PDF's `date_es` follows.
    Date(String),
}

impl Cell {
    /// Text cell from anything displayable; empty input becomes [`Cell::Empty`]
    /// so blank strings do not litter the sheet with empty text values.
    pub fn text(value: impl Into<String>) -> Self {
        let value = value.into();
        if value.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(value)
        }
    }

    /// Optional number: `None` becomes [`Cell::Empty`].
    pub fn number(value: Option<f64>) -> Self {
        value.map_or(Cell::Empty, Cell::Number)
    }

    /// Optional ISO date: `None` (or an empty string) becomes [`Cell::Empty`].
    pub fn date(value: Option<&str>) -> Self {
        match value {
            Some(iso) if !iso.is_empty() => Cell::Date(iso.to_string()),
            _ => Cell::Empty,
        }
    }
}

/// A column: its header text and how wide the tab should open it, in Excel's
/// character-width units.
#[derive(Debug, Clone)]
pub struct Column {
    pub header: String,
    pub width: f64,
}

impl Column {
    pub fn new(header: impl Into<String>, width: f64) -> Self {
        Self {
            header: header.into(),
            width,
        }
    }
}

/// One worksheet: a tab name, its columns, and its data rows.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub columns: Vec<Column>,
    /// Rows of cells. A row shorter than `columns` leaves the rest blank; a
    /// longer one is an author error and is rejected by [`render_xlsx`].
    pub rows: Vec<Vec<Cell>>,
}

impl Sheet {
    pub fn new(name: impl Into<String>, columns: Vec<Column>) -> Self {
        Self {
            name: name.into(),
            columns,
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, row: Vec<Cell>) {
        self.rows.push(row);
    }
}

/// A whole workbook, rendered in sheet order.
#[derive(Debug, Clone, Default)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
}

impl Workbook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, sheet: Sheet) {
        self.sheets.push(sheet);
    }
}

/// A successfully rendered spreadsheet.
///
/// Manual `Debug` like [`crate::RenderedPdf`]: a failing test must not dump a
/// whole workbook into the terminal.
pub struct RenderedWorkbook {
    /// The complete .xlsx file, ready to write to disk.
    pub bytes: Vec<u8>,
    /// Number of worksheets (for UI feedback).
    pub sheet_count: usize,
}

impl std::fmt::Debug for RenderedWorkbook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderedWorkbook")
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("sheet_count", &self.sheet_count)
            .finish()
    }
}

/// Render a workbook description to .xlsx bytes.
///
/// Synchronous and CPU-bound like [`crate::render_pdf`] — callers at the Tauri
/// boundary follow the long-running-command rule (`async fn`).
pub fn render_xlsx(workbook: &Workbook) -> Result<RenderedWorkbook, ReportError> {
    if workbook.sheets.is_empty() {
        return Err(ReportError::Workbook(
            "a workbook needs at least one sheet".into(),
        ));
    }

    let mut book = XlsxWorkbook::new();
    let header_format = Format::new().set_bold();
    let date_format = Format::new().set_num_format("dd/mm/yyyy");
    let mut used_names: Vec<String> = Vec::new();

    for sheet in &workbook.sheets {
        let columns = u16::try_from(sheet.columns.len()).map_err(|_| {
            ReportError::Workbook(format!("sheet '{}' has too many columns", sheet.name))
        })?;
        if let Some(row) = sheet.rows.iter().find(|r| r.len() > sheet.columns.len()) {
            return Err(ReportError::Workbook(format!(
                "sheet '{}' has a row of {} cells but only {} columns",
                sheet.name,
                row.len(),
                sheet.columns.len()
            )));
        }

        let name = unique_sheet_name(&sheet.name, &mut used_names);
        let worksheet = book.add_worksheet();
        worksheet
            .set_name(&name)
            .map_err(|e| ReportError::Workbook(format!("sheet name '{name}': {e}")))?;

        let fail =
            |e: rust_xlsxwriter::XlsxError| ReportError::Workbook(format!("sheet '{name}': {e}"));

        for (index, column) in sheet.columns.iter().enumerate() {
            let col = index as u16;
            worksheet
                .write_string_with_format(0, col, column.header.as_str(), &header_format)
                .map_err(fail)?;
            worksheet
                .set_column_width(col, column.width)
                .map_err(fail)?;
        }

        for (index, row) in sheet.rows.iter().enumerate() {
            // +1 for the header row; a u32 row index caps at ~1M, Excel's own
            // limit, so a conversion failure here means the data is already
            // beyond what the format can hold.
            let row_index = u32::try_from(index + 1).map_err(|_| {
                ReportError::Workbook(format!("sheet '{name}' exceeds the row limit"))
            })?;
            for (col_index, cell) in row.iter().enumerate() {
                let col = col_index as u16;
                match cell {
                    Cell::Empty => {}
                    Cell::Text(value) => {
                        worksheet
                            .write_string(row_index, col, value)
                            .map_err(fail)?;
                    }
                    Cell::Number(value) => {
                        worksheet
                            .write_number(row_index, col, *value)
                            .map_err(fail)?;
                    }
                    Cell::Date(iso) => match ExcelDateTime::parse_from_str(iso) {
                        Ok(date) => {
                            worksheet
                                .write_datetime_with_format(row_index, col, &date, &date_format)
                                .map_err(fail)?;
                        }
                        // Never lose a stored value to a parse failure.
                        Err(_) => {
                            worksheet.write_string(row_index, col, iso).map_err(fail)?;
                        }
                    },
                }
            }
        }

        // Freeze the header and offer filtering — on a register of hundreds of
        // treatments both are what makes the sheet usable at all.
        worksheet.set_freeze_panes(1, 0).map_err(fail)?;
        if columns > 0 && !sheet.rows.is_empty() {
            let last_row = u32::try_from(sheet.rows.len()).unwrap_or(u32::MAX);
            worksheet
                .autofilter(0, 0, last_row, columns - 1)
                .map_err(fail)?;
        }
    }

    let bytes = book
        .save_to_buffer()
        .map_err(|e| ReportError::Workbook(e.to_string()))?;
    Ok(RenderedWorkbook {
        bytes,
        sheet_count: workbook.sheets.len(),
    })
}

/// Excel rejects some characters in tab names, caps them at 31 characters and
/// forbids duplicates. Sanitising here (rather than making every caller do it)
/// keeps section titles like "3.1 Registro de actuaciones" usable as written.
fn unique_sheet_name(requested: &str, used: &mut Vec<String>) -> String {
    let cleaned: String = requested
        .chars()
        .map(|c| {
            if FORBIDDEN_IN_SHEET_NAME.contains(&c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim();
    let base: String = if cleaned.is_empty() {
        "Hoja".to_string()
    } else {
        cleaned.chars().take(MAX_SHEET_NAME).collect()
    };

    let mut candidate = base.clone();
    let mut suffix = 2;
    while used.iter().any(|n| n.eq_ignore_ascii_case(&candidate)) {
        let tag = format!(" ({suffix})");
        let keep = MAX_SHEET_NAME.saturating_sub(tag.chars().count());
        candidate = format!("{}{tag}", base.chars().take(keep).collect::<String>());
        suffix += 1;
    }
    used.push(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Excel's three tab-name rules, checked on the function that owns them:
    /// forbidden characters, the 31-character cap, and uniqueness.
    #[test]
    fn sheet_names_are_sanitised_truncated_and_deduplicated() {
        let mut used = Vec::new();

        let cleaned = unique_sheet_name("3.1/3.2 [borrador]:*?", &mut used);
        assert_eq!(cleaned, "3.1-3.2 -borrador----");

        let long = unique_sheet_name(
            "Un nombre larguísimo que supera de largo el límite de Excel",
            &mut used,
        );
        assert_eq!(long.chars().count(), MAX_SHEET_NAME);
        assert!(long.starts_with("Un nombre larguísimo"));

        assert_eq!(unique_sheet_name("Parcelas", &mut used), "Parcelas");
        assert_eq!(unique_sheet_name("Parcelas", &mut used), "Parcelas (2)");
        assert_eq!(unique_sheet_name("Parcelas", &mut used), "Parcelas (3)");
        // Case-insensitive, like Excel itself.
        assert_eq!(unique_sheet_name("PARCELAS", &mut used), "PARCELAS (4)");

        // A blank name still has to produce a usable tab.
        assert_eq!(unique_sheet_name("   ", &mut used), "Hoja");
    }

    /// Truncation must not overflow the cap once a duplicate tag is appended.
    #[test]
    fn a_truncated_name_stays_within_the_cap_after_deduplication() {
        let long = "Registro de actuaciones fitosanitarias de la parcela";
        let mut used = Vec::new();
        let first = unique_sheet_name(long, &mut used);
        let second = unique_sheet_name(long, &mut used);

        assert_ne!(first, second);
        for name in [&first, &second] {
            assert!(
                name.chars().count() <= MAX_SHEET_NAME,
                "'{name}' exceeds Excel's cap"
            );
        }
    }
}
