// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The name a detail page goes by, for the shell's top band. The band belongs to
// App.svelte, but only the view knows which farm or book it loaded, so the view
// states it here and the shell reads it.
//
// The label is stamped with the hash it was set on, and the shell shows it only
// while that hash is current. That is what keeps a farm's name from lingering
// over the next page, without the shell having to clear it at the right moment
// — a clear on navigation would race the view that is mounting and publishing
// its own.

export const pageTitle = $state({ label: "", hash: "" });

export function setPageTitle(label) {
  pageTitle.label = label;
  pageTitle.hash = location.hash;
}
