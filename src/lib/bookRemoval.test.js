// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The words of a book deleted with its records. What is pinned here is what a
// screen could get quietly wrong: "0 records" said of an empty book, a device
// named by its id, a person named when the log names none, and a count that
// leaves out a kind.
import { describe, expect, it, vi } from "vitest";

// The dictionary is stubbed to echo its key and parameters, so the test reads
// what each helper ASKED for rather than a translation.
vi.mock("../i18n.js", () => ({
  t: (key, params = {}) => `${key}(${Object.values(params).join("|")})`,
  tCode: (prefix, code) => `${prefix} ${code}`,
  formatDate: (iso) => `date:${iso}`,
  languageTag: () => "es-ES",
}));

const {
  deleteConfirm,
  deviceName,
  recordKinds,
  recordTotal,
  removedLine,
  removedNotice,
  restorableUntil,
  restoredBook,
} = await import("./bookRemoval.js");

const season = { id: "s", label: "2025/2026" };

const removal = (overrides = {}) => ({
  removed_at: "2026-09-30T18:04:11Z",
  restorable_until: "2026-10-30",
  removed_on: "laptop",
  device_label: null,
  removed_by: null,
  author_name: null,
  records: [],
  ...overrides,
});

describe("deleteConfirm", () => {
  it("does not count the records of a book that holds none", () => {
    expect(
      deleteConfirm(season, "Los Llanos", { records: 0, restorable_until: "2026-10-31" }),
    ).toBe("season.delete_confirm(2025/2026|Los Llanos|date:2026-10-31)");
  });

  it("counts what goes with a book that holds records, and says until when", () => {
    expect(
      deleteConfirm(season, "Los Llanos", { records: 412, restorable_until: "2026-10-31" }),
    ).toBe("season.delete_confirm_records(2025/2026|Los Llanos|date:2026-10-31|412)");
  });
});

describe("deviceName", () => {
  it("uses the name somebody gave the device", () => {
    expect(deviceName("laptop", "Portátil", "phone")).toBe("Portátil");
  });

  it("calls this device this device, and any other unnamed one unnamed", () => {
    expect(deviceName("phone", null, "phone")).toBe("sync.device_this()");
    expect(deviceName("laptop", null, "phone")).toBe("sync.device_unnamed()");
  });
});

describe("removedLine and removedNotice", () => {
  it("name the person only where the log names one", () => {
    expect(removedLine(removal(), "phone")).toBe(
      "removed_books.removed_on(date:2026-09-30||sync.device_unnamed()|date:2026-10-30)",
    );
    expect(removedLine(removal({ author_name: "Ana", device_label: "Portátil" }), "phone")).toBe(
      "removed_books.removed_by(date:2026-09-30|Ana|Portátil|date:2026-10-30)",
    );
  });

  it("say on a live book's page until when its records can come back", () => {
    expect(removedNotice(removal({ author_name: "Ana" }), "laptop")).toBe(
      "removed_with_book.notice_by(date:2026-09-30|Ana|sync.device_this()|date:2026-10-30)",
    );
    expect(removedNotice(removal(), "laptop")).toBe(
      "removed_with_book.notice_on(date:2026-09-30||sync.device_this()|date:2026-10-30)",
    );
  });

  it("say until when a removed book can come back, as the backend gives the day", () => {
    expect(restorableUntil(removal())).toBe("removed_books.until(date:2026-10-30)");
    expect(restorableUntil(removal(), { waiting: "devices", devices: [] })).toBe(
      "removed_books.was_until(date:2026-10-30)",
    );
  });
});

describe("recordKinds and recordTotal", () => {
  const records = [
    { table: "crop", count: 1 },
    { table: "sowing_record", count: 2 },
  ];

  it("count every kind, in the backend's order", () => {
    expect(recordKinds(records)).toBe(
      "strays.kind_count(Entity crop|1) · strays.kind_count(Entity sowing_record|2)",
    );
    expect(recordTotal(records)).toBe(3);
  });

  it("say a book went empty rather than listing nothing", () => {
    expect(recordKinds([])).toBe("removed_books.no_records()");
    expect(recordTotal([])).toBe(0);
  });
});

describe("restoredBook", () => {
  it("mentions records only when some came back", () => {
    expect(restoredBook("2025/2026", 0)).toBe("removed_books.restored(2025/2026)");
    expect(restoredBook("2025/2026", 3)).toBe("removed_books.restored_records(2025/2026|3)");
  });
});
