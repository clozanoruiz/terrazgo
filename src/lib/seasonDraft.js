// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * A record book's campaign form, as data: what the fields hold, how a stored
 * season fills them, and what a submit sends.
 *
 * Two screens ask these questions — the record book list creates a book, the
 * book's own page corrects its campaign — and the only difference between them
 * is the farm: a season is one holding's campaign, so the farm is chosen when
 * the book is created and never again (moving a season would carry its whole
 * book to another holding). The farmDraft.js arrangement, for the same reason.
 *
 * The campaign is its two dates. Its name is not asked for: the backend derives
 * it from the dates ("2025/2026", "2026") unless the farmer gives one, which is
 * why this module never computes a name itself — one rule, in Rust.
 *
 * Framework-agnostic on purpose (docs/frontend-conventions.md → the two-tier
 * rule), so the rules about blank fields are unit-tested here rather than
 * living in the component that draws them.
 */

/// A new book's draft: the given farm, and nothing else decided for the farmer.
export function emptySeasonDraft(farmId = "") {
  return {
    farmId,
    startsOn: "",
    endsOn: "",
    customLabel: "",
  };
}

/// Fill the form from a stored `Season`. Only the farmer's own name comes back:
/// a name the dates gave is not something they typed, and showing it would turn
/// it into one the next save keeps when the dates change.
export function seasonDraftFrom(season) {
  return {
    farmId: season.farm_id,
    startsOn: season.starts_on,
    endsOn: season.ends_on,
    customLabel: season.custom_label ?? "",
  };
}

/// What a submit sends. `create` adds the farm, which only a new book states:
/// the update command has no farm field at all.
///
/// A blank name is `null`, never `""` — no name, so the dates give one.
export function seasonPayload(draft, { create = false } = {}) {
  const name = String(draft.customLabel ?? "").trim();
  const payload = {
    starts_on: draft.startsOn,
    ends_on: draft.endsOn,
    custom_label: name === "" ? null : name,
  };
  return create ? { farm_id: draft.farmId, ...payload } : payload;
}
