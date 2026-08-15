// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The seam between the app's primitives and @internationalized/date's objects.
//
// Every date in this app — stored, validated in Rust, printed in the record
// book — is a "YYYY-MM-DD" string, and every hour is a "HH:MM" local wall
// clock. The owned controls need CalendarDate and Time objects instead, so
// this module is the ONLY place those types exist: the wrappers convert at
// their edges and every call site keeps binding the string it always bound.
//
// Plain JS, no Svelte import — the framework-agnostic tier
// (docs/frontend-conventions.md → the two-tier rule).
import { parseDate, Time } from "@internationalized/date";

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
// Seconds are tolerated on the way in because a stored hour may have been
// written as HH:MM:SS by an exporter; they are never written back out.
const ISO_TIME = /^(\d{2}):(\d{2})(?::\d{2})?$/;

/// "2026-08-03" -> CalendarDate; anything else -> undefined, which is what the
/// date components read as "no value".
///
/// Deliberately not a bare parseDate(): that THROWS on "", and "" is the app's
/// own empty date — eleven optional fields hold it, and every draftFrom() fills
/// an absent date with `?? ""`. The try/catch then covers a stored value that
/// matches the shape but is not a real day (month 13, 30 February): a corrupt
/// row must render as a blank field, never take a whole view down on mount.
export function toCalendarDate(iso) {
  if (!ISO_DATE.test(iso ?? "")) return undefined;
  try {
    return parseDate(iso);
  } catch {
    return undefined;
  }
}

/// CalendarDate|undefined -> "YYYY-MM-DD" or "". CalendarDate#toString() is
/// already exactly the ISO date form, so no formatter and no timezone is
/// involved — which is the point: this value is a calendar day, not an instant.
export function fromCalendarDate(value) {
  return value ? value.toString() : "";
}

/// "14:30" -> Time. Local wall clock throughout: the hour a treatment was
/// applied is a time of day on the ground, never converted to UTC.
export function toTime(hhmm) {
  const match = ISO_TIME.exec(hhmm ?? "");
  if (!match) return undefined;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (hour > 23 || minute > 59) return undefined;
  return new Time(hour, minute);
}

/// Time|undefined -> "HH:MM" or "".
///
/// Built by hand rather than Time#toString(), which renders "HH:MM:SS": the
/// backend validates the hour's shape on write, and TreatmentForm submits
/// `draft.applicationTime || null`, so an unset hour must stay exactly "" and a
/// set one must carry no seconds the farmer never entered.
export function fromTime(value) {
  if (!value) return "";
  return `${String(value.hour).padStart(2, "0")}:${String(value.minute).padStart(2, "0")}`;
}
