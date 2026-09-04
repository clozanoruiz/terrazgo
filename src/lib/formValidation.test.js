// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The module takes anything that iterates like `form.elements`, so the fixtures
// here are plain objects with the four properties it reads. That is what keeps
// the suite on the `node` environment with no DOM dependency, and it is not a
// shortcut: the browser's own behaviour was measured separately (a customError
// blocks submit, `novalidate` + checkValidity still fires `invalid`), and what
// is left for a unit test is the reading — which fields are reported, in what
// order, and under what name.
import { describe, expect, it } from "vitest";

import { firstPerName, invalidFields } from "./formValidation.js";

/// A control the browser would validate. `valid: false` unless told otherwise,
/// since a fixture only exists here to be a problem or to prove it is skipped.
function field({
  valid = false,
  message = "obligatorio",
  label,
  labels,
  name,
  willValidate = true,
}) {
  return {
    willValidate,
    validity: { valid },
    validationMessage: message,
    dataset: label === undefined ? {} : { tzLabel: label },
    labels,
    name,
  };
}

describe("invalidFields", () => {
  it("reports invalid controls in the order given, which is DOM order", () => {
    const problems = invalidFields([
      field({ label: "Fecha" }),
      field({ label: "Dosis" }),
      field({ label: "Superficie" }),
    ]);
    expect(problems.map((p) => p.label)).toEqual(["Fecha", "Dosis", "Superficie"]);
  });

  it("skips valid controls", () => {
    const problems = invalidFields([
      field({ label: "Fecha" }),
      field({ label: "Cultivo", valid: true }),
      field({ label: "Dosis" }),
    ]);
    expect(problems.map((p) => p.label)).toEqual(["Fecha", "Dosis"]);
  });

  it("skips what the browser itself would skip", () => {
    // A disabled field is left empty on purpose; reporting it would name a
    // problem the farmer has no way to act on.
    const problems = invalidFields([
      field({ label: "Distancia", willValidate: false }),
      field({ label: "Fecha" }),
    ]);
    expect(problems.map((p) => p.label)).toEqual(["Fecha"]);
  });

  it("prefers the control's own label over an associated <label>", () => {
    // An owned control's proxy has no <label> of its own, so data-tz-label is
    // the only source; where both exist the control's own is the specific one.
    const problems = invalidFields([
      field({ label: "Fecha de aplicación", labels: [{ textContent: "algo distinto" }] }),
    ]);
    expect(problems[0].label).toBe("Fecha de aplicación");
  });

  it("falls back to the associated <label>, trimmed", () => {
    // The shape CataloguePicker's call sites use: <label><span>Especie</span>…
    const problems = invalidFields([field({ labels: [{ textContent: "\n  Especie\n" }] })]);
    expect(problems[0].label).toBe("Especie");
  });

  it("invents no label when there is none", () => {
    const problems = invalidFields([field({ message: "Escriba un número" })]);
    expect(problems[0]).toMatchObject({ label: "", message: "Escriba un número" });
  });

  it("carries the element through, so the summary can focus it", () => {
    const el = field({ label: "Fecha" });
    expect(invalidFields([el])[0].el).toBe(el);
  });

  it("returns nothing for a clean form, and tolerates no form at all", () => {
    expect(invalidFields([field({ valid: true })])).toEqual([]);
    expect(invalidFields([])).toEqual([]);
    expect(invalidFields(undefined)).toEqual([]);
  });
});

describe("firstPerName", () => {
  it("keeps one entry per named group", () => {
    // Boxes sharing a name are one native group: one tick satisfies all of
    // them, so they are one problem and not three.
    const problems = invalidFields([
      field({ label: "Trigo", name: "practices" }),
      field({ label: "Cebada", name: "practices" }),
      field({ label: "Avena", name: "practices" }),
    ]);
    expect(firstPerName(problems).map((p) => p.label)).toEqual(["Trigo"]);
  });

  it("leaves unnamed controls alone", () => {
    const problems = invalidFields([field({ label: "Fecha" }), field({ label: "Dosis" })]);
    expect(firstPerName(problems)).toHaveLength(2);
  });

  it("does not collapse different names", () => {
    const problems = invalidFields([
      field({ label: "Fecha", name: "date" }),
      field({ label: "Dosis", name: "dose" }),
    ]);
    expect(firstPerName(problems).map((p) => p.label)).toEqual(["Fecha", "Dosis"]);
  });
});
