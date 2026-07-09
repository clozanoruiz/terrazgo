// SPDX-License-Identifier: AGPL-3.0-or-later

// The global notification center as reactive state (`.svelte.js` enables
// runes), rendered by NotificationBell.svelte. run() wraps every command
// call: a boundary error becomes a red notification and opens the panel so
// the failure is seen immediately; views push success notifications with
// notify(). Items stay until dismissed — the bell badge counts what waits.

import { errorText } from "./backend.js";

let nextId = 1;

export const notifications = $state({ items: [], open: false });

export function notify(text, isError = false) {
  notifications.items.unshift({ id: nextId++, text, isError });
  if (isError) notifications.open = true;
}

export function dismiss(id) {
  const at = notifications.items.findIndex((n) => n.id === id);
  if (at !== -1) notifications.items.splice(at, 1);
}

export function clearAll() {
  notifications.items.length = 0;
  notifications.open = false;
}

export async function run(action) {
  try {
    await action();
  } catch (err) {
    notify(errorText(err), true);
  }
}
