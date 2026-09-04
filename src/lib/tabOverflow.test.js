// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The split behind the tab strip's overflow menu. Worth pinning because both of
// its failure modes are silent: reserve too little and the last tab is sliced
// through its label, reserve too much and a strip that fits hides a tab behind
// a button for no reason.
import { describe, expect, it } from "vitest";

import { visibleTabCount } from "./tabOverflow.js";

// Six 100px tabs with a 4px gap between them: 620px laid end to end.
const SIX = [100, 100, 100, 100, 100, 100];
const GAP = 4;
const MORE = 80;

const split = (available, widths = SIX) =>
  visibleTabCount({ widths, gap: GAP, available, moreWidth: MORE });

describe("visibleTabCount", () => {
  it("shows every tab when they all fit", () => {
    expect(split(700)).toBe(6);
  });

  it("counts the gaps between tabs and not after the last one", () => {
    // 620 is the exact end-to-end width; one pixel less and the row overflows.
    expect(split(620)).toBe(6);
    expect(split(619)).toBe(5);
  });

  it("charges the menu button only once something overflows", () => {
    // At 619 the row no longer fits, so the button is rendered and paid for:
    // 619 - 80 = 539 of budget, which takes 5 tabs (516) and not 6.
    expect(split(619)).toBe(5);
    // 400 - 80 = 320: three tabs need 308, four need 412.
    expect(split(400)).toBe(3);
  });

  it("hides everything rather than showing a sliced tab", () => {
    // 120 - 80 = 40 of budget against a 100px tab.
    expect(split(120)).toBe(0);
  });

  it("shows every tab while nothing has been measured", () => {
    // A strip that has not been laid out yet. Any other answer would be
    // unrecoverable: widths can only be read while every tab is in the DOM.
    expect(split(0)).toBe(6);
    expect(visibleTabCount({ widths: SIX })).toBe(6);
  });

  it("has nothing to show for an empty strip", () => {
    expect(visibleTabCount({ widths: [], available: 500 })).toBe(0);
    expect(visibleTabCount()).toBe(0);
  });

  it("tolerates the sub-pixel width a fractional measurement sums to", () => {
    // getBoundingClientRect() returns fractions; three tabs measured at
    // 100.1 sum past a 300.2 bar by a tenth of a pixel nobody can see.
    expect(visibleTabCount({ widths: [100.1, 100.1, 100.1], gap: 0, available: 300.2 })).toBe(3);
  });

  it("keeps tabs of unequal width in order rather than packing them", () => {
    // A narrow fifth tab does NOT jump the queue past the wide fourth one: the
    // strip must read in the order the caller declared.
    expect(
      visibleTabCount({ widths: [100, 100, 300, 20], gap: 0, available: 300, moreWidth: 0 }),
    ).toBe(2);
  });
});
