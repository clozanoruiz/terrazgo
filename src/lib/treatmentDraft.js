// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The shape of the treatment form's working state, in one place.
//
// The register view owns the draft and fills it (blank for a new entry, from a
// stored record for a correction); `TreatmentForm.svelte` is a view over it. The
// form must not capture the record it is correcting at creation: a component
// keeps its initial values for its whole life, so a form that snapshots a prop
// silently keeps showing the old record when the farmer picks a different one to
// correct. Ownership by the opener is what the other register views do too.

export function emptyRow() {
  // growthStage is the crop's BBCH stage (Reglamento (UE) 2023/564's annex),
  // which attaches to the treated crop rather than to the record — so it lives
  // on the plot row, not beside the date.
  return { plotId: "", cropId: "", surface: "", growthStage: "" };
}

export function emptyProblemRow() {
  return { category: "", code: "" };
}

/// A blank draft: a new treatment, with one plot row and one problem row to fill.
export function emptyDraft() {
  return {
    // The id of the record being CORRECTED, or null when entering a new one.
    // The sources allow correcting an entry (RD 1311/2012 is silent on it, and
    // SIEX models it as re-sending the same alias), so the same form does both.
    editingId: null,
    applicationDate: "",
    // Optional: the interval Anexo III Parte I B allows when an actuation ran
    // over several days. The plazo de seguridad is counted from it in Rust.
    applicationEndDate: "",
    // Optional: the start hour Reglamento (UE) 2023/564's annex asks for when
    // the product's use is restricted to particular times of day. Local
    // wall-clock HH:MM — never converted, because the hour on the ground is
    // what makes it relevant.
    applicationTime: "",
    dryingDate: "",
    productId: "",
    doseValue: "",
    doseUnit: "l_ha",
    // Total product used (Anexo III Parte I B.i). Not derivable from a
    // concentration dose, so it is captured — prefilled only where the
    // arithmetic is actually sound.
    totalQuantity: "",
    totalQuantityUnit: "l",
    targetOrganism: "",
    operatorId: "",
    machineryId: "",
    phiDays: "",
    notes: "",
    // Model 3.1 bis. The advisor is Anexo III Parte I B.d ("y, en su caso, del
    // asesor"); the measure is the non-chemical alternative art. 10.1 asks
    // farmers to prefer, and a record may carry it INSTEAD of a product.
    advisorId: "",
    measureCode: "",
    measureIntensity: "",
    measureIntensityUnit: "traps",
    measureRegistration: "",
    efficacyCode: "",
    rows: [emptyRow()],
    // The coded problems treated (≥1) and IPM justifications (≥1) — required by
    // the record rules; efficacy is optional because it is observed after
    // application (the register list offers it once known).
    problemRows: [emptyProblemRow()],
    checkedJustifications: [],
  };
}

/// A draft opened on a stored record, so the farmer changes the one thing that
/// was wrong and everything else is submitted back unchanged.
export function draftFrom(entry) {
  return {
    editingId: entry.record.id,
    applicationDate: entry.record.application_date,
    applicationEndDate: entry.record.application_end_date ?? "",
    applicationTime: entry.record.application_time ?? "",
    dryingDate: entry.record.drying_date ?? "",
    productId: entry.record.product_id ?? "",
    doseValue: entry.record.dose_value ?? "",
    doseUnit: entry.record.dose_unit_code ?? "l_ha",
    totalQuantity: entry.record.total_quantity_value ?? "",
    totalQuantityUnit: entry.record.total_quantity_unit_code ?? "l",
    targetOrganism: entry.record.target_organism ?? "",
    operatorId: entry.record.operator_id,
    machineryId: entry.record.machinery_id ?? "",
    phiDays: entry.record.phi_days_used ?? "",
    notes: entry.record.notes ?? "",
    advisorId: entry.record.advisor_id ?? "",
    measureCode: entry.record.measure_code ?? "",
    measureIntensity: entry.record.measure_intensity_value ?? "",
    measureIntensityUnit: entry.record.measure_intensity_unit_code ?? "traps",
    measureRegistration: entry.record.measure_registration_number ?? "",
    efficacyCode: entry.record.efficacy_code ?? "",
    rows: entry.plots.map((plot) => ({
      plotId: plot.plot_id,
      cropId: plot.crop_id ?? "",
      surface: plot.surface_treated_ha,
      growthStage: plot.growth_stage_code ?? "",
    })),
    problemRows: entry.problems.map((problem) => ({
      category: problem.reason_category_code,
      code: problem.problem_code,
    })),
    checkedJustifications: entry.justifications.map((j) => j.justification_code),
  };
}
