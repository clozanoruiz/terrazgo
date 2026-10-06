// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The thresholds, not the punctuation: `formatUnit` is i18n.js's and tested
// there, so this mocks it down to "<number> <unit>" and asserts which unit each
// size lands on and how much precision it keeps.
import { describe, expect, it, vi } from "vitest";

vi.mock("../i18n.js", () => ({
  formatUnit: (value, unit, digits = 1) => `${Number(value.toFixed(digits))} ${unit}`,
}));

const { formatSize } = await import("./byteSize.js");

const KIB = 1024;
const MIB = 1024 * KIB;
const GIB = 1024 * MIB;

describe("formatSize", () => {
  it("keeps one decimal at gigabyte scale, where it says something", () => {
    expect(formatSize(2.5 * GIB)).toBe("2.5 gigabyte");
  });

  it("keeps none at megabyte scale, where it does not", () => {
    // The default tile-cache cap. "512 MB" is the figure the settings screen
    // offers; "512.4 MB" would be noise.
    expect(formatSize(512 * MIB)).toBe("512 megabyte");
  });

  it("falls to kilobytes below a megabyte", () => {
    expect(formatSize(300 * KIB)).toBe("300 kilobyte");
  });

  /// A file that exists must never read as nothing. An empty geo cache is a few
  /// hundred bytes and rounds to zero.
  it("floors at one kilobyte rather than reporting zero", () => {
    expect(formatSize(200)).toBe("1 kilobyte");
    expect(formatSize(0)).toBe("1 kilobyte");
  });

  it("switches unit exactly at each binary threshold", () => {
    expect(formatSize(MIB - 1)).toBe("1024 kilobyte");
    expect(formatSize(MIB)).toBe("1 megabyte");
    expect(formatSize(GIB - 1)).toBe("1024 megabyte");
    expect(formatSize(GIB)).toBe("1 gigabyte");
  });
});
