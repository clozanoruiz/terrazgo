// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// What a reader made the columns of a table, remembered per device.
//
// The pure half of column resizing: clamping and storage, no DOM. The wiring
// that hangs handles off a <thead> lives in columnResize.js, which is a view
// concern and is verified by the scripted checks — this half decides what a
// width may be and what survives a restart, so it is unit-tested
// (docs/frontend-conventions.md → "The two-tier rule").
//
// Widths are a display preference like the collapsed sidebar and the inspector
// width: localStorage, never the database. Nothing about them belongs to the
// holding's records, and a backup restoring a record book onto a new device
// must not impose the old device's column layout.

/// Narrow enough to hide a column's content, wide enough to still be grabbable.
export const MIN_COLUMN_W = 48;

const STORE_KEY = "terrazgo.columns";

/// A width the table can actually use: whole pixels, never below the floor.
/// Anything that is not a finite number is refused rather than coerced — a NaN
/// written into a style property silently drops the declaration, and the column
/// would go back to its share with nothing to say why.
///
/// The type is checked before the conversion, and that is not belt-and-braces:
/// `Number(null)`, `Number("")` and `Number([])` are all 0, so a stored null
/// would come back as a 48px column rather than as "this row is unreadable".
/// A numeric STRING is still accepted, because that is what a style property
/// reads back as.
export function clampWidth(px) {
  const usable = typeof px === "number" || (typeof px === "string" && px.trim() !== "");
  if (!usable) return null;
  const value = Number(px);
  if (!Number.isFinite(value)) return null;
  return Math.max(MIN_COLUMN_W, Math.round(value));
}

/// Every table's stored widths, or {} when there is nothing readable.
///
/// Storage can be absent (a private window), blocked (site data off) or hold
/// something this version does not understand — a stale shape from an older
/// release, say. All three mean the same thing here: fall back to the layout
/// the stylesheet gives, which is always correct if never remembered.
export function readAll(storage) {
  try {
    const parsed = JSON.parse(storage?.getItem(STORE_KEY) ?? "null");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

/// The widths for one table, or null when it has none — or when the count no
/// longer matches. A stored row of six widths means nothing to a table that now
/// has seven columns, and applying it would misalign every one of them.
export function readWidths(storage, tableId, columnCount) {
  const stored = readAll(storage)[tableId];
  if (!Array.isArray(stored) || stored.length !== columnCount) return null;
  const widths = stored.map(clampWidth);
  return widths.every((w) => w !== null) ? widths : null;
}

/// Remember one table's widths. Failing to remember is not a reason to refuse
/// the resize, so a blocked store is swallowed.
export function saveWidths(storage, tableId, widths) {
  try {
    const all = readAll(storage);
    all[tableId] = widths;
    storage.setItem(STORE_KEY, JSON.stringify(all));
  } catch {
    // The columns still resized; they just will not be that way next time.
  }
}

/// Forget one table's widths, so it goes back to the stylesheet's shares.
export function forgetWidths(storage, tableId) {
  try {
    const all = readAll(storage);
    delete all[tableId];
    storage.setItem(STORE_KEY, JSON.stringify(all));
  } catch {
    // Nothing to forget if it was never stored.
  }
}
