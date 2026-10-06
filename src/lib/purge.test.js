// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The words of the purge. What is pinned here is what a screen could get
// quietly wrong: a device named by its id, "0 books" said of an import that
// erased records only, the two sources of an import's erasure counted apart,
// the Status view named as anything but the page it is, and a day said where
// the backend sent none.
import { describe, expect, it, vi } from "vitest";

// The dictionary is stubbed to echo its key and parameters, so the test reads
// what each helper ASKED for rather than a translation.
vi.mock("../i18n.js", () => ({
  t: (key, params = {}) => `${key}(${Object.values(params).join("|")})`,
  tCode: (prefix, code) => `${prefix} ${code}`,
  formatDate: (iso) => `date:${iso}`,
  formatList: (items) => items.join(" + "),
  languageTag: () => "es-ES",
}));

const { discardedSentences, erasedSentence, erasingLine } = await import("./purge.js");

describe("erasingLine", () => {
  it("names each device it waits for as people call it, and where to retire one", () => {
    const erasing = {
      waiting: "devices",
      devices: [
        { device: "phone", label: "Móvil de Juan" },
        { device: "tablet", label: null },
      ],
    };
    expect(erasingLine(erasing, "laptop")).toBe(
      "removed_books.erasing_devices(2|Móvil de Juan + sync.device_unnamed()|nav.settings()|sync.peers_title())",
    );
  });

  it("names the Status view by its own label", () => {
    expect(erasingLine({ waiting: "status" }, "laptop")).toBe(
      "removed_books.erasing_status(nav.status())",
    );
  });

  it("gives the day it goes only when the backend sends one", () => {
    expect(erasingLine({ waiting: "due", from: "2026-11-12" }, "laptop")).toBe(
      "removed_books.erasing_due_on(date:2026-11-12)",
    );
    expect(erasingLine({ waiting: "due", from: null }, "laptop")).toBe(
      "removed_books.erasing_due()",
    );
  });
});

describe("erasedSentence", () => {
  const summary = (books, records) => ({ registers: books + records + 1, books, records });

  it("adds what another device's purge erased here to what this one's did", () => {
    expect(erasedSentence(summary(1, 400), summary(0, 12))).toBe(
      "message.sync_erased(message.sync_erased_books(1) + message.sync_erased_records(412))",
    );
  });

  it("leaves out a kind with none", () => {
    expect(erasedSentence(summary(0, 3), summary(0, 0))).toBe(
      "message.sync_erased(message.sync_erased_records(3))",
    );
    expect(erasedSentence(summary(2, 0), null)).toBe(
      "message.sync_erased(message.sync_erased_books(2))",
    );
  });

  it("says nothing when nothing was erased, or the purge could not run", () => {
    expect(erasedSentence(summary(0, 0), null)).toBe("");
    // A declaration erased with nothing else is neither a book nor a record.
    expect(erasedSentence({ registers: 1, books: 0, records: 0 })).toBe("");
  });
});

describe("discardedSentences", () => {
  it("says one sentence per device, named as people call it", () => {
    expect(
      discardedSentences(
        [
          { device: "rosa", device_label: "Móvil de Rosa", changes: 1 },
          { device: "old", device_label: null, changes: 3 },
        ],
        "laptop",
      ),
    ).toEqual([
      "message.sync_discarded(1|Móvil de Rosa)",
      "message.sync_discarded(3|sync.device_unnamed())",
    ]);
  });

  it("says nothing when nothing was left out", () => {
    expect(discardedSentences([], "laptop")).toEqual([]);
  });
});
