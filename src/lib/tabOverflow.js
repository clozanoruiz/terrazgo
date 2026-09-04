// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// How many tabs fit in the strip — and therefore how many go into the overflow
// menu beside it.
//
// Split out of TzTabs.svelte because it is arithmetic over measurements rather
// than anything Svelte does: the frontend's framework-agnostic tier
// (docs/frontend-conventions.md → "The two-tier rule"), which is the tier that
// gets unit tests. The component measures and renders; this decides.

/// Sub-pixel slack. Widths come from `getBoundingClientRect()` and are
/// fractional, so a row that fits exactly can still sum to a hair more than the
/// box holding it. Without this, half a pixel nobody can see banishes the last
/// tab into the menu.
const EPSILON = 0.5;

/// `widths` are the tabs' natural widths in order, `gap` the strip's column
/// gap, `available` the content width of the bar they share, and `moreWidth`
/// the width of the overflow group (its divider included) once it has been
/// rendered at least once.
export function visibleTabCount({ widths = [], gap = 0, available = 0, moreWidth = 0 } = {}) {
  const count = widths.length;
  if (count === 0) return 0;

  // Nothing measured yet: a strip that has not been laid out, or one inside a
  // panel that was still hidden when it mounted. Answering "all of them" is
  // what MAKES the next measurement possible — a tab's width can only be read
  // while it is in the DOM, so any other answer here would leave the split
  // unable to correct itself.
  if (!(available > 0)) return count;

  const total = widths.reduce((sum, width) => sum + width, 0) + gap * (count - 1);
  if (total <= available + EPSILON) return count;

  // The menu button is rendered only when something overflows, so its width is
  // charged on this branch and nowhere else — otherwise a strip that fits
  // exactly would reserve room for a button it never shows.
  const budget = available - moreWidth;
  let used = 0;
  let fits = 0;
  for (let i = 0; i < count; i += 1) {
    const needed = widths[i] + (i > 0 ? gap : 0);
    if (used + needed > budget + EPSILON) break;
    used += needed;
    fits += 1;
  }

  // Zero is a legitimate answer on a very narrow screen. A bar showing only the
  // menu button states the truth — everything is in there — where a tab sliced
  // through the middle of its label does not.
  return fits;
}
