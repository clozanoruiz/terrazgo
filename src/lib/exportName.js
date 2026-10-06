// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Suggested file names for the things the app hands to a save dialog: the
// printable book, its spreadsheet, and the database backup.
//
// The name has to be UNIQUE per export, and that is not cosmetic. On Android
// the destination is created by the SAF picker, and when the proposed name
// already exists the picker renames the collision — badly. Measured on an
// SM-A226B on 2026-09-05, twice, with two different results:
//
//   cuaderno_2025-2026_es_20260905.pdf  ->  cuaderno_2025-2026_es_20260905_.pdf
//   terrazgo-backup-2026-09-05.db       ->  terrazgo-backup-2026-09-05.db (1)
//
// The difference is whether Android can find the extension. `.pdf` is
// registered for the MIME type it was offered under, so the suffix goes before
// it and the name stays openable. `.db` is not, so the whole name is treated as
// the base and the counter lands AFTER the extension — leaving a file that no
// longer ends in `.db`, which the import dialog's `.db` filter then refuses to
// show. An export the app cannot offer back is the worst of the two failures,
// and it is the one that hits backups.
//
// A date alone did not prevent this: both collisions above are two exports on
// the SAME day. So the stamp carries the time as well, which makes a collision
// need two exports inside one second.
//
// The stamp is UTC, like every other instant the project writes down. A file
// name is display, so local time would be defensible, but it would buy a
// timezone edge case for nothing a farmer reads closely.

/// `YYYYMMDDTHHMMSS` in UTC — compact so it cannot be misread as a year range,
/// and second-resolution so two exports on one day differ.
export function exportStamp(now = new Date()) {
  return now.toISOString().slice(0, 19).replace(/[-:]/g, "");
}

/// Join `parts` with underscores and add the extension.
///
/// Underscores separate the fields because a campaign label already contains
/// hyphens once sanitised ("2025/2026" → "2025-2026"). Empty parts are dropped,
/// so a caller can pass an optional language without a conditional.
export function exportFileName(parts, extension, now = new Date()) {
  const fields = parts.filter(Boolean).map(sanitise);
  return `${[...fields, exportStamp(now)].join("_")}.${extension}`;
}

/// Strip what a path cannot carry. Season labels arrive as "2025/2026", and a
/// farm or crop name can hold anything the farmer typed.
function sanitise(part) {
  return String(part).replace(/[^\p{L}\p{N}._-]+/gu, "-");
}
