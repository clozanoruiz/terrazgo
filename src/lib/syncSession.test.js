// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// One test on purpose: the module holds state for the life of the app, and a
// vitest file shares one module instance across its tests — so "starts empty"
// is only true before anything else in this file has run.
import { describe, expect, it } from "vitest";

import { answeredThisSession, rememberImport } from "./syncSession.js";

describe("the imports an export answers", () => {
  it("starts with none, keeps each in order, and hands out a copy", () => {
    // Nothing imported yet: the export answers nobody and carries the whole log.
    expect(answeredThisSession()).toEqual([]);

    const fromP = { laptop: 4, p: 3 };
    const fromQ = { laptop: 2, q: 5 };
    rememberImport(fromP);
    rememberImport(fromQ);
    expect(answeredThisSession()).toEqual([fromP, fromQ]);

    // What the export sends cannot rewrite what was noted.
    answeredThisSession().pop();
    expect(answeredThisSession()).toEqual([fromP, fromQ]);
  });
});
