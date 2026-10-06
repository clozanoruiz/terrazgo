// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The words the screens put on the purge (docs/sync.md → The purge, as
// settled): what a deleted book past its date waits for before it goes for
// good, and what an import says it erased and left out. Every reason, count
// and day is the backend's; this only says it.
//
// Framework-agnostic tier: no Svelte import.
import { formatDate, formatList, t } from "../i18n.js";
import { deviceName } from "./bookRemoval.js";

/// What a book past its date waits for, as its line in the fold says it —
/// the backend's `Erasing`. The devices are named as people call them, and the
/// Status view as the page it is.
export function erasingLine(erasing, thisDevice) {
  switch (erasing.waiting) {
    case "devices":
      return t("removed_books.erasing_devices", {
        count: erasing.devices.length,
        devices: formatList(
          erasing.devices.map(({ device, label }) => deviceName(device, label, thisDevice)),
        ),
        settings: t("nav.settings"),
        peers: t("sync.peers_title"),
      });
    case "status":
      return t("removed_books.erasing_status", { status: t("nav.status") });
    default:
      return erasing.from
        ? t("removed_books.erasing_due_on", { date: formatDate(erasing.from) })
        : t("removed_books.erasing_due");
  }
}

/// What an import erased for good, every source together — the purge another
/// device made, and the one run here once the file had applied. Books and
/// records are counted apart, and a kind with none is not mentioned. Empty
/// when nothing was erased, or when a count could not be made (`null`).
export function erasedSentence(...summaries) {
  const books = summaries.reduce((total, summary) => total + (summary?.books ?? 0), 0);
  const records = summaries.reduce((total, summary) => total + (summary?.records ?? 0), 0);
  const parts = [
    books > 0 ? t("message.sync_erased_books", { count: books }) : null,
    records > 0 ? t("message.sync_erased_records", { count: records }) : null,
  ].filter(Boolean);
  if (parts.length === 0) return "";
  return t("message.sync_erased", { what: formatList(parts) });
}

/// One sentence per device whose changes an import left out because they were
/// made to records erased for good — never applied, here or anywhere.
export function discardedSentences(discarded, thisDevice) {
  return discarded.map(({ device, device_label, changes }) =>
    t("message.sync_discarded", {
      count: changes,
      device: deviceName(device, device_label, thisDevice),
    }),
  );
}
