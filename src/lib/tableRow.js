// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Clicking anywhere on a table row opens that record. Plain JS with no Svelte
// import, so it belongs to the framework-agnostic tier and is unit-tested
// (docs/frontend-conventions.md → "The two-tier rule").

/// Whether a click that landed somewhere in a row should open the row.
///
/// A row is opened by pointer, but the row itself is NOT the accessible
/// control: a `<tr>` carries the `row` role and cannot be focused or activated
/// from the keyboard, so each row also holds a real `<button>` on its name.
/// That leaves one thing to settle — a click on a control inside the row
/// belongs to that control, not to the row — and this is it. Without the guard
/// the name button would fire twice (its own handler, then the row's as the
/// event bubbles), and a future "Quitar" button in a cell would open the record
/// it was meant to remove.
export function opensRow(event) {
  const target = event.target;
  if (!(target instanceof Element)) return true;
  return !target.closest("button, a, input, select, textarea, label, [role='button']");
}
