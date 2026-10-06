// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// What a register's form says after it saves, and who listens: the book page's
// duplicate check (docs/sync.md → The same rule, right after the form saves).
//
// A form saves as it always did — nothing here can hold a save up or refuse
// one — and then calls `checkSaved(register, id)`. The book page's duplicates
// panel is listening, asks the backend which pairs the save put the record in,
// and shows the person any it finds. The forms live in a dozen components under
// the book page, so the call goes through this one hub rather than a callback
// threaded through every register's props.
//
// Framework-agnostic tier (docs/frontend-conventions.md): no Svelte, so it is
// unit-tested.

let listener = null;

/// Listen for saves. One listener at a time — there is one book page open —
/// and the returned function stops listening, but only if nobody has taken over
/// since, so a page unmounting after the next one mounted cannot silence it.
/// Shaped for a Svelte `$effect`, which calls what it returns on teardown.
export function onSaved(handler) {
  listener = handler;
  return () => {
    if (listener === handler) listener = null;
  };
}

/// Say that a register's record was just saved. Resolves once the listener has
/// run its check — a pair on screen, or none found — and never waits for the
/// person's answer: the form is done, and whatever they decide rereads the
/// book's registers itself. Resolves at once when nothing is listening: a form
/// outside a book page saves exactly the same.
export async function checkSaved(register, id) {
  if (!listener || !id) return;
  await listener(register, id);
}
