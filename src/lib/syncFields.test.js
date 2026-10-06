// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The field map's own behaviour: what it answers for a column it knows, for
// one it does not, and what order it puts a review's lines in.
//
// What it does NOT check is whether the map keeps up with the schema or with
// the dictionaries — those are questions about files this tier cannot see, and
// src-tauri/tests/contracts/sync_fields_contract.rs asks them where the schema
// and the dictionaries both are.
import { describe, expect, it } from "vitest";
import {
  COVERED,
  SYNC_FIELDS,
  fieldOf,
  orderedLines,
  readableLines,
  startsRow,
} from "./syncFields.js";

/// One review line, as the backend hands it over.
const line = (table, column, { root = false, entity_id = "r1" } = {}) => ({
  table,
  entity_id,
  column,
  root,
  values: [],
});

describe("fieldOf", () => {
  it("answers with the register's own label key", () => {
    expect(fieldOf("treatment_record", "dose_value").key).toBe("treatment.dose");
  });

  it("carries the tCode prefix a coded column's VALUE needs", () => {
    const unit = fieldOf("treatment_record", "dose_unit_code");
    expect(unit.key).toBe("treatment.unit");
    expect(unit.code).toBe("unit");
  });

  it("has no code prefix for a plain column", () => {
    expect(fieldOf("treatment_record", "dose_value").code).toBe(null);
  });

  it("answers for a column every table carries", () => {
    // `deleted_at` is declared once, not on each of the forty tables.
    expect(fieldOf("plot", "deleted_at").key).toBe("sync.field.deleted_at");
    expect(fieldOf("treatment_record", "deleted_at").key).toBe("sync.field.deleted_at");
  });

  it("names the book a version files a record in, on every register of the book", () => {
    // Merging two books moves records between them (docs/sync.md → Merging two
    // books), so two versions can disagree about the book — declared once.
    expect(fieldOf("treatment_record", "season_id").key).toBe("sync.field.season_id");
    expect(fieldOf("crop", "season_id").key).toBe("sync.field.season_id");
  });

  it("falls back to the column itself for a register nobody has described", () => {
    // Plain rather than broken: a register a module adds is reviewable the day
    // it ships, and naming its fields is what stops it being plain.
    const unknown = fieldOf("future_module_record", "some_column");
    expect(unknown.key).toBe(null);
    expect(unknown.column).toBe("some_column");
  });

  it("falls back for a column its register does not name", () => {
    expect(fieldOf("plot", "invented_column").key).toBe(null);
  });
});

describe("orderedLines", () => {
  it("puts the register's own fields before its children", () => {
    const ordered = orderedLines([
      line("treatment_plot", "plot_id"),
      line("treatment_record", "dose_value", { root: true }),
    ]);
    expect(ordered.map((l) => l.table)).toEqual(["treatment_record", "treatment_plot"]);
  });

  it("reads a register in its form's order, not the backend's alphabet", () => {
    // The backend orders by column name, which would open a treatment with
    // `advisor_id` and put the date two thirds down.
    const ordered = orderedLines([
      line("treatment_record", "notes", { root: true }),
      line("treatment_record", "dose_value", { root: true }),
      line("treatment_record", "application_date", { root: true }),
    ]);
    expect(ordered.map((l) => l.column)).toEqual(["application_date", "dose_value", "notes"]);
  });

  it("keeps a column it does not know after the ones it does", () => {
    const ordered = orderedLines([
      line("plot", "invented_column", { root: true }),
      line("plot", "area_ha", { root: true }),
      line("plot", "name", { root: true }),
    ]);
    expect(ordered.map((l) => l.column)).toEqual(["name", "area_ha", "invented_column"]);
  });

  it("keeps each child row's lines together", () => {
    const ordered = orderedLines([
      line("treatment_plot", "plot_id", { entity_id: "b" }),
      line("treatment_plot", "surface_treated_ha", { entity_id: "a" }),
      line("treatment_plot", "plot_id", { entity_id: "a" }),
    ]);
    expect(ordered.map((l) => `${l.entity_id}.${l.column}`)).toEqual([
      "a.plot_id",
      "a.surface_treated_ha",
      "b.plot_id",
    ]);
  });

  it("does not mutate what it was given", () => {
    const lines = [line("treatment_plot", "plot_id"), line("treatment_record", "dose_value")];
    const before = [...lines];
    orderedLines(lines);
    expect(lines).toEqual(before);
  });
});

describe("startsRow", () => {
  it("is true for the first line of all", () => {
    expect(startsRow(line("plot", "name"), undefined)).toBe(true);
  });

  it("is false for another column of the same row", () => {
    const first = line("treatment_plot", "plot_id", { entity_id: "a" });
    const second = line("treatment_plot", "surface_treated_ha", { entity_id: "a" });
    expect(startsRow(second, first)).toBe(false);
  });

  it("is true for a second child row of the same table", () => {
    // Two treated plots are two blocks, not one block of six fields.
    const first = line("treatment_plot", "plot_id", { entity_id: "a" });
    const second = line("treatment_plot", "plot_id", { entity_id: "b" });
    expect(startsRow(second, first)).toBe(true);
  });
});

describe("readableLines", () => {
  /// A render that prints each line's text as the test states it, per side.
  const printed = (texts) => (line) => texts[`${line.table}.${line.entity_id}.${line.column}`];

  it("drops a reference's frozen name when it prints the same on every side", () => {
    // `product_id` resolves to the product's name and the snapshot IS the name:
    // "Producto: Decis" twice would be one fact said twice.
    const lines = [
      line("treatment_record", "product_name_snapshot", { root: true }),
      line("treatment_record", "product_id", { root: true }),
      line("treatment_record", "dose_value", { root: true }),
    ];
    const render = printed({
      "treatment_record.r1.product_id": ["Decis", "Decis"],
      "treatment_record.r1.product_name_snapshot": ["Decis", "Decis"],
      "treatment_record.r1.dose_value": ["1,5", "0,8"],
    });
    const columns = readableLines(lines, render).map(({ line }) => line.column);
    expect(columns).toEqual(["product_id", "dose_value"]);
  });

  it("keeps both when they print differently — a product renamed since", () => {
    const lines = [
      line("treatment_record", "product_id", { root: true }),
      line("treatment_record", "product_name_snapshot", { root: true }),
    ];
    const render = printed({
      "treatment_record.r1.product_id": ["Decis Expert", "Decis Expert"],
      "treatment_record.r1.product_name_snapshot": ["Decis", "Decis Expert"],
    });
    expect(readableLines(lines, render)).toHaveLength(2);
  });

  it("never merges lines of two different rows", () => {
    // Two treated plots each have a crop; the second plot's is not a repeat.
    const lines = [
      line("treatment_plot", "crop_id", { entity_id: "a" }),
      line("treatment_plot", "crop_name_snapshot", { entity_id: "b" }),
    ];
    const render = printed({
      "treatment_plot.a.crop_id": ["Trigo", "Trigo"],
      "treatment_plot.b.crop_name_snapshot": ["Trigo", "Trigo"],
    });
    expect(readableLines(lines, render)).toHaveLength(2);
  });

  it("judges a difference on the printed text, not on the stored value", () => {
    // Two phones each added the product: two ids, one name. That is the case
    // the rules are written around, not a disagreement to point at.
    const lines = [
      line("treatment_record", "product_id", { root: true }),
      line("treatment_record", "dose_value", { root: true }),
    ];
    const render = printed({
      "treatment_record.r1.product_id": ["Decis", "Decis"],
      "treatment_record.r1.dose_value": ["1,5", "0,8"],
    });
    const differs = readableLines(lines, render).map(({ differs }) => differs);
    expect(differs).toEqual([false, true]);
  });

  it("keeps a column nobody has named, however it prints", () => {
    // No label to compare, so nothing to call a repeat.
    const lines = [
      line("future_module_record", "a", { root: true }),
      line("future_module_record", "b", { root: true }),
    ];
    const render = () => ["x", "x"];
    expect(readableLines(lines, render)).toHaveLength(2);
  });
});

describe("the map itself", () => {
  it("covers the registers a farmer edits", () => {
    for (const table of ["treatment_record", "crop", "plot", "farm", "fertilisation_record"]) {
      expect(COVERED).toContain(table);
    }
  });

  it("names every column with a non-empty key", () => {
    for (const [table, columns] of Object.entries(SYNC_FIELDS)) {
      for (const [column, entry] of Object.entries(columns)) {
        const key = typeof entry === "string" ? entry : entry.key;
        // An i18n key, dotted: `treatment.dose`, `sync.field.season_label`.
        expect(key, `${table}.${column}`).toMatch(/^[a-z_]+(\.[a-z_0-9]+)+$/);
      }
    }
  });
});
