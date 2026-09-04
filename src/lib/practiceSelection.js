// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

/// The one rule the good-practices list carries beyond "tick what applies".
///
/// FEGA's `BUENAS_PRACTICAS_AMBITOS` opens each ámbito with code "0", spelled
/// "No realiza buenas prácticas" — a claim that contradicts every other row in
/// the list. Nothing in the catalogue's shape says so: it is an ordinary code
/// beside the other forty, and a record could hold it alongside them.
///
/// So the rule is stated twice on purpose. Here it keeps the form from ever
/// showing the contradiction, which is the half a farmer can see; the
/// repository refuses it as well (module-fertilisation's `validated_practices`),
/// which is the half that holds for a record arriving any other way.
export const NO_PRACTICES_CODE = "0";

/// Apply a tick or an untick to the chosen set, keeping code "0" exclusive.
///
/// Returns a new array — the caller assigns it, so Svelte sees the change.
/// Order is the caller's; the repository sorts numerically on write.
export function togglePractice(chosen, code, checked) {
  if (!checked) return chosen.filter((entry) => entry !== code);
  if (code === NO_PRACTICES_CODE) return [NO_PRACTICES_CODE];
  const kept = chosen.filter((entry) => entry !== NO_PRACTICES_CODE && entry !== code);
  return [...kept, code];
}
