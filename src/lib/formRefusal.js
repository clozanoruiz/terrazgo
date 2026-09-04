// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// How a backend refusal reaches the field it names.
//
// TzForm puts a `$state({ byName })` object in context; an owned control reads
// its own entry and renders it as a second `.tz-field-error` line. View tier
// rather than agnostic, for the reason routes.js is: it imports from "svelte".
//
// The refusal is DISPLAY ONLY — it deliberately never goes through
// setCustomValidity. Two reasons, and both are about not making the form worse
// than the bell was:
//
//   * a stale custom validity would refuse the next submit until something
//     cleared it, so a refusal the farmer had already answered could wedge the
//     form. A line that merely lingers until the next submit cannot.
//   * it would fight each control's own `$effect(… setCustomValidity(error))`,
//     which is the one thing keeping the inline message and the summary entry
//     from disagreeing.
//
// So the two paths stay separate by construction: constraint validation is the
// field's own and drives blocking; a refusal is the backend's and drives text.
import { getContext, setContext } from "svelte";

const REFUSALS = Symbol("tz-refusals");

/// Called by TzForm with its reactive `{ byName: {} }` store.
export function provideRefusals(store) {
  setContext(REFUSALS, store);
}

/// Called by an owned control at init. Returns the store or null — a control
/// used outside a TzForm is ordinary and must not throw.
export function refusalStore() {
  return getContext(REFUSALS) ?? null;
}
