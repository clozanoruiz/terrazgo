// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, it } from "vitest";

import { draftFrom, emptyDraft } from "./treatmentDraft.js";

/// A stored record with only the non-chemical half filled in — the shape
/// `get_treatment_record` returns.
function measureOnly(record) {
  return {
    record: {
      operator_id: "op",
      problems: [],
      justifications: [],
      ...record,
    },
    plots: [],
    problems: [],
    justifications: [],
  };
}

describe("the measure half of a treatment draft", () => {
  it("starts blank, counting traps", () => {
    const draft = emptyDraft();
    expect(draft.measureCode).toBe("");
    expect(draft.measureIntensityUnit).toBe("traps");
    expect(draft.measureBasicSubstance).toBe("");
  });

  it("carries a stored basic substance back into the form", () => {
    const draft = draftFrom(
      measureOnly({
        measure_code: "11",
        measure_basic_substance_code: "28",
        measure_intensity_value: 2.5,
        measure_intensity_unit_code: "kg_ha",
      }),
    );
    expect(draft.measureCode).toBe("11");
    expect(draft.measureBasicSubstance).toBe("28");
    expect(draft.measureIntensityUnit).toBe("kg_ha");
  });

  it("reads a record with no substance as blank, not as null", () => {
    const draft = draftFrom(measureOnly({ measure_code: "15" }));
    expect(draft.measureBasicSubstance).toBe("");
  });
});
