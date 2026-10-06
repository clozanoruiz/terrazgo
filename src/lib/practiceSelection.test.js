// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The good-practices list is a plain "tick what applies" except for code "0",
// which claims the opposite of every other row. The source of truth is the
// catalogue's own wording: FEGA's BUENAS_PRACTICAS_AMBITOS row
// ("0";"No realiza buenas prácticas";"SI";"SI";"SI").
import { describe, expect, it } from "vitest";

import { NO_PRACTICES_CODE, shownPractices, togglePractice } from "./practiceSelection.js";

describe("togglePractice", () => {
  it("adds and removes an ordinary code", () => {
    expect(togglePractice([], "49", true)).toEqual(["49"]);
    expect(togglePractice(["49", "20"], "49", false)).toEqual(["20"]);
  });

  it("does not duplicate a code already chosen", () => {
    expect(togglePractice(["49"], "49", true)).toEqual(["49"]);
  });

  it("drops every other practice when 'none' is claimed", () => {
    expect(togglePractice(["49", "20", "6"], NO_PRACTICES_CODE, true)).toEqual([NO_PRACTICES_CODE]);
  });

  it("drops 'none' when a practice is claimed", () => {
    expect(togglePractice([NO_PRACTICES_CODE], "49", true)).toEqual(["49"]);
  });

  it("leaves the set empty when 'none' is unticked", () => {
    // Not the same as claiming a practice: the farmer has answered nothing yet,
    // and the section is optional, so an empty set is a legal state.
    expect(togglePractice([NO_PRACTICES_CODE], NO_PRACTICES_CODE, false)).toEqual([]);
  });

  it("returns a new array rather than mutating the one passed in", () => {
    const chosen = ["49"];
    expect(togglePractice(chosen, "20", true)).not.toBe(chosen);
    expect(chosen).toEqual(["49"]);
  });
});

describe("shownPractices", () => {
  const options = [
    { code: "0", name: "No realiza buenas prácticas", offered: true },
    { code: "4", name: "Retirada", offered: false },
    { code: "5", name: "Fraccionar el abonado", offered: true },
  ];

  it("lists only what may be offered while nothing is claimed", () => {
    expect(shownPractices(options, [])).toEqual({
      shown: [options[0], options[2]],
      unknown: false,
    });
  });

  it("keeps a practice the record claims though it is no longer offered", () => {
    // Retired by the authority since the record was written: still named.
    expect(shownPractices(options, ["4"])).toEqual({ shown: options, unknown: false });
  });

  it("shows a claimed code this device's catalogue lacks as itself, and says so", () => {
    // Written on a device whose catalogue is newer (docs/sync.md → What stays
    // device-local): invisible today, so it would be saved without being seen.
    expect(shownPractices(options, ["41"])).toEqual({
      shown: [{ code: "41", name: "41" }, options[0], options[2]],
      unknown: true,
    });
  });
});
