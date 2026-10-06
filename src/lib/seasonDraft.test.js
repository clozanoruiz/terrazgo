// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { emptySeasonDraft, seasonDraftFrom, seasonPayload } from "./seasonDraft.js";

const dated = {
  id: "01a0aaad-c17a-726a-b3f1-411f849eb4ce",
  farm_id: "01a0aaad-c165-7424-b4b2-d7738d3fc620",
  label: "2025/2026",
  custom_label: null,
  starts_on: "2025-09-01",
  ends_on: "2026-08-31",
  status: "active",
};
const named = { ...dated, label: "2026 primavera", custom_label: "2026 primavera" };

describe("emptySeasonDraft", () => {
  it("takes the farm the book is created for", () => {
    expect(emptySeasonDraft("farm-1").farmId).toBe("farm-1");
  });

  it("decides no dates and no name for the farmer", () => {
    const draft = emptySeasonDraft("farm-1");
    expect([draft.startsOn, draft.endsOn, draft.customLabel]).toEqual(["", "", ""]);
  });
});

describe("seasonDraftFrom", () => {
  it("fills the farm and the dates of a stored season", () => {
    const draft = seasonDraftFrom(dated);
    expect(draft.farmId).toBe(dated.farm_id);
    expect(draft.startsOn).toBe("2025-09-01");
    expect(draft.endsOn).toBe("2026-08-31");
  });

  it("leaves the name blank when the dates gave it", () => {
    // Filling "2025/2026" in would make it a typed name, and correcting the
    // dates afterwards would no longer rename the book.
    expect(seasonDraftFrom(dated).customLabel).toBe("");
  });

  it("brings back a name the farmer typed", () => {
    expect(seasonDraftFrom(named).customLabel).toBe("2026 primavera");
  });
});

describe("seasonPayload", () => {
  it("sends the farm only when creating, because a season never changes farm", () => {
    const draft = seasonDraftFrom(dated);
    expect(seasonPayload(draft, { create: true }).farm_id).toBe(dated.farm_id);
    expect(seasonPayload(draft)).not.toHaveProperty("farm_id");
  });

  it("sends a blank or spaces-only name as null, so the dates name the book", () => {
    expect(seasonPayload({ ...seasonDraftFrom(dated), customLabel: "" }).custom_label).toBeNull();
    expect(
      seasonPayload({ ...seasonDraftFrom(dated), customLabel: "   " }).custom_label,
    ).toBeNull();
  });

  it("trims a typed name", () => {
    expect(
      seasonPayload({ ...seasonDraftFrom(dated), customLabel: " 2026 otoño " }).custom_label,
    ).toBe("2026 otoño");
  });

  it("sends no campaign year: the backend reads everything from the dates", () => {
    expect(seasonPayload(seasonDraftFrom(dated))).toEqual({
      starts_on: "2025-09-01",
      ends_on: "2026-08-31",
      custom_label: null,
    });
  });
});
