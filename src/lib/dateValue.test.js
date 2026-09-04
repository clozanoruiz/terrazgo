// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The seam the owned date and time fields sit on. Every case here is a claim
// the module's own comments make about stored data — a blank optional date, a
// corrupt row, an hour that arrived with seconds — so the tests are the places
// those claims stop being prose.
import { describe, expect, it } from "vitest";
import { toCalendarDate, fromCalendarDate, toTime, fromTime } from "./dateValue.js";

describe("toCalendarDate", () => {
  it("reads an ISO day", () => {
    expect(fromCalendarDate(toCalendarDate("2026-08-03"))).toBe("2026-08-03");
  });

  it.each([
    // "" is the app's own empty date — eleven optional fields hold it, and a
    // bare parseDate() would THROW on it.
    ["", "the empty date"],
    [null, "a null"],
    [undefined, "an undefined"],
    ["2026-8-3", "an unpadded day"],
    ["03/08/2026", "a rendered day"],
    ["not a date", "prose"],
    // Right shape, impossible day: a corrupt row must render as a blank field
    // rather than take the whole view down on mount.
    ["2026-13-01", "month 13"],
    ["2026-02-30", "30 February"],
  ])("reads %j (%s) as no value", (input) => {
    expect(toCalendarDate(input)).toBeUndefined();
  });

  it("round-trips every day it accepts", () => {
    for (const iso of ["2026-01-01", "2026-12-31", "2024-02-29", "2026-08-03"]) {
      expect(fromCalendarDate(toCalendarDate(iso))).toBe(iso);
    }
  });
});

describe("fromCalendarDate", () => {
  it("renders no value as the empty string, never a placeholder day", () => {
    expect(fromCalendarDate(undefined)).toBe("");
    expect(fromCalendarDate(null)).toBe("");
  });
});

describe("toTime", () => {
  it("reads a wall-clock hour", () => {
    expect(fromTime(toTime("14:30"))).toBe("14:30");
    expect(fromTime(toTime("00:00"))).toBe("00:00");
    expect(fromTime(toTime("23:59"))).toBe("23:59");
  });

  it("tolerates seconds on the way in and never writes them back", () => {
    // A stored hour may have been written HH:MM:SS by an exporter.
    expect(fromTime(toTime("14:30:00"))).toBe("14:30");
    expect(fromTime(toTime("14:30:59"))).toBe("14:30");
  });

  it.each([
    ["", "the empty hour"],
    ["24:00", "hour 24"],
    ["12:60", "minute 60"],
    ["9:05", "an unpadded hour"],
    ["abc", "prose"],
  ])("reads %j (%s) as no value", (input) => {
    expect(toTime(input)).toBeUndefined();
  });
});

describe("fromTime", () => {
  it("pads both halves", () => {
    expect(fromTime(toTime("09:05"))).toBe("09:05");
  });

  it("renders no value as the empty string", () => {
    // TreatmentForm submits `draft.applicationTime || null`, so an unset hour
    // must stay exactly "" — not "00:00", which is an hour the farmer never
    // entered and which the backend would store.
    expect(fromTime(undefined)).toBe("");
    expect(fromTime(null)).toBe("");
  });
});
