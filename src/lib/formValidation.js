// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Reading a form's constraint validation back out, so a view can show every
// problem at once instead of letting the browser paint one bubble.
//
// Deliberately a plain module (no Svelte imports): it takes anything that
// iterates like `form.elements` and returns plain objects, which keeps it in
// the framework-agnostic tier and unit-testable on the `node` environment with
// hand-made fakes — no jsdom (docs/frontend-conventions.md → "The two-tier
// rule").
//
// The browser already evaluates every control on submit; what it does NOT do is
// hand you the list. This is that list.

/// The text naming a field in the summary. `data-tz-label` first, because the
/// owned controls know their own label and a `.tz-validity` proxy has no
/// `<label>` of its own. Otherwise the associated label's text, which is what
/// serves a control the caller wrapped in `<label><span>…</span>` — the shape
/// CataloguePicker's call sites use.
///
/// Never invented: a control with no label at all contributes its message
/// alone, since a made-up name in a register's form is worse than none.
function fieldLabel(el) {
  const stated = el.dataset?.tzLabel;
  if (stated) return stated.trim();
  // `labels` is a NodeList on real elements and undefined on a plain object.
  return visibleText(el.labels?.[0]);
}

/// A label's text WITHOUT anything marked `aria-hidden`, which is how an
/// accessible name is computed and, here, how the required mark stays out of
/// the summary. Measured 2026-09-15 before this existed: the summary named the
/// field "Nombre completo*", asterisk and all, because the fallback read the
/// whole label.
///
/// Cloned rather than string-subtracted so nested and repeated marks behave,
/// and guarded so the plain-object fakes this module is tested with — which
/// have a `textContent` and no `cloneNode` — still work.
function visibleText(label) {
  if (!label) return "";
  if (typeof label.cloneNode !== "function") return label.textContent?.trim() ?? "";
  const copy = label.cloneNode(true);
  for (const hidden of copy.querySelectorAll?.('[aria-hidden="true"]') ?? []) hidden.remove();
  return copy.textContent?.trim() ?? "";
}

/// Every control in `elements` the browser considers invalid, in DOM order —
/// which is the order they appear on screen, so the summary reads down the form.
///
/// `willValidate` excludes what the browser itself would skip: disabled and
/// readonly controls, `type="hidden"`, buttons and fieldsets. Without it a
/// disabled field left empty would be reported as a problem the farmer cannot
/// act on.
///
/// The `el` handle travels with each entry so the summary can focus it. On an
/// owned control that is the off-screen proxy, whose own `onfocus` bounces to
/// the real field — the correction is made where the farmer can see it.
export function invalidFields(elements) {
  const out = [];
  for (const el of elements ?? []) {
    if (!el.willValidate || el.validity?.valid !== false) continue;
    out.push({ label: fieldLabel(el), message: el.validationMessage ?? "", el });
  }
  return out;
}

/// One control per name, keeping the first. A radio group and a native checkbox
/// group share a name and would otherwise report the same problem once per box.
export function firstPerName(problems) {
  const seen = new Set();
  return problems.filter(({ el }) => {
    const name = el.name;
    if (!name) return true;
    if (seen.has(name)) return false;
    seen.add(name);
    return true;
  });
}
