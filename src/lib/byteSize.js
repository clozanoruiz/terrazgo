// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// A size on disk, written the way the reader's locale writes numbers.
//
// Shared the moment it had a second caller: the Settings view's tile-cache cap
// and the About panel's two database sizes ask the same question, and a second
// copy would be a second set of thresholds to keep in step.

import { formatUnit } from "../i18n.js";

const MIB = 1024 * 1024;

// Binary thresholds with the familiar GB/MB/kB labels. Only the NUMBER is the
// reader's ("2,5 GB" in Castilian, "2.5 GB" in English) — the unit is rendered
// by Intl from a CLDR unit id, never by appending a string.
//
// The decimals change with the scale on purpose: a gigabyte figure is worth one
// ("2,5 GB" says something "3 GB" does not), while a megabyte one is not — the
// cache is 512 MB, not 512,4 MB. Below a megabyte it floors at 1 kB rather than
// rounding to zero, because "0 kB" reads as "nothing here" for a file that
// exists.
export function formatSize(bytes) {
  if (bytes >= 1024 * MIB) return formatUnit(bytes / (1024 * MIB), "gigabyte");
  if (bytes >= MIB) return formatUnit(Math.round(bytes / MIB), "megabyte", 0);
  return formatUnit(Math.max(1, Math.round(bytes / 1024)), "kilobyte", 0);
}
