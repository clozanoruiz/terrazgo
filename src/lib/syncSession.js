// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// What every sync import since the app opened said its sender holds, so the
// next export answers all of them at once: the backend trims the file to what
// every one of those devices held, and one file copied to each is complete at
// each (docs/sync.md → A trimmed reply is safe only for the device it
// answers).
//
// Module-level rather than in SettingsView, which is remounted whenever the
// farmer leaves Settings and comes back. A farmer who imports one phone's file,
// goes to look at what arrived, and returns to import the other's must still
// get one file that suits both.
//
// Deliberately not persisted: remembering a peer across restarts is the
// bookkeeping retention needs, and neither is built. After a restart the first
// export carries the whole log, which is complete everywhere.

const answered = [];

// Note one import's `peer_seen` — the sender's own `seen` vector.
export function rememberImport(seen) {
  answered.push(seen);
}

// Every vector noted so far, oldest first. A copy, so what the caller does with
// it cannot change what was noted.
export function answeredThisSession() {
  return [...answered];
}
