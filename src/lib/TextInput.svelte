<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned single-line text field.
  //
  // The third owned control that is NOT built on Bits UI, and the one with the
  // smallest reason: a text input parses nothing, draws no popover and has no
  // locale to get wrong. What the raw `<input required>` got wrong was the
  // REPORTING — with no message of its own it fell back to the browser's, which
  // follows the OS language rather than the one the holding chose, at nineteen
  // call sites across the registry and the record book.
  //
  // So the element underneath stays a plain `<input type="text">`, exactly as
  // TzCheckbox keeps a real checkbox, and no off-screen `.tz-validity` proxy is
  // needed: a real input carries its own validity. The wrapper exists to hold
  // the label, the hint and the error line in the shape `.tz-field` gives every
  // other control.
  //
  // Contract mirrors NumberInput's: a string in, a string out, "" for unset.
  import { t } from "../i18n.js";
  import { refusalStore } from "./formRefusal.js";

  let {
    value = $bindable(""),
    label = "",
    /// Rendered as the <small> under the field, like the native call sites did.
    hint = "",
    required = false,
    /// "text" or "email". Not a general passthrough: `date`, `time`, `number`
    /// and `checkbox` each have an owned control of their own for reasons the
    /// other components state, and routing them through here would reopen them.
    type = "text",
    /// Length bounds, matching the native attributes. Present because the
    /// registry fields they would serve are identifiers with real lengths;
    /// nothing passes them yet.
    maxlength = null,
    disabled = false,
    placeholder = "",
    class: klass = "",
    /// Names this field inside its form, so TzForm's `anchors` can reach it as
    /// form.elements[name] to hang a backend refusal on the right control.
    name = "",
    /// For the uncontrolled idiom: called with the new value after a change.
    onchange = null,
    /// Extra content under the field — the registry hint the identifier fields
    /// carry (RegistryHint.svelte).
    children = null,
  } = $props();

  const uid = $props.id();

  let field = $state(null);
  let showError = $state(false);
  let error = $state("");

  /// Decide what is wrong and say it in the holding's language.
  ///
  /// Imperative rather than a `$derived`, because one of the two answers is the
  /// BROWSER's: `type="email"` is left on the element so its own check keeps
  /// blocking, and `validity.typeMismatch` is not a reactive source a derived
  /// could read. Clearing our message first is what makes that readable —
  /// a standing customError masks nothing, but it is the only state in which
  /// the element's own verdict can be trusted to be current.
  ///
  /// The emptiness test is trimmed, because a name of three spaces is not a
  /// name, and the backend trims before it validates: accepting it here would
  /// only move the refusal one round trip away.
  function syncValidity() {
    if (!field) return;
    field.setCustomValidity("");
    let next = "";
    if (required && !field.value.trim()) next = t("form.required");
    else if (field.validity.typeMismatch) next = t("form.email_invalid");
    // Through setCustomValidity rather than the `required` attribute: that is
    // what keeps validationMessage OURS, so TzForm's summary can read it
    // straight off the element (measured 2026-09-01, see DateInput).
    field.setCustomValidity(next);
    error = next;
  }

  $effect(() => {
    // Named so the effect depends on them, not just on `field`.
    void value;
    void required;
    void type;
    syncValidity();
  });

  // A backend refusal the form chose to hang on this field. Display only — it
  // never becomes a validity, so it cannot wedge the next submit.
  const refusals = refusalStore();
  const refusal = $derived(name && refusals ? (refusals.byName[name] ?? "") : "");

  function onInput(event) {
    value = event.currentTarget.value;
    showError = false;
  }
</script>

<div class="tz-field {klass}">
  {#if label}
    <label class="tz-label" for={uid}>{label}</label>
  {/if}

  <input
    id={uid}
    bind:this={field}
    {type}
    {value}
    {name}
    {maxlength}
    {placeholder}
    {disabled}
    class="tz-text"
    class:tz-invalid={(showError && !!error) || !!refusal}
    aria-invalid={(showError && !!error) || !!refusal}
    aria-describedby={hint ? `${uid}-hint` : undefined}
    oninput={onInput}
    onblur={() => (showError = true)}
    oninvalid={() => (showError = true)}
    onchange={() => onchange?.(value)}
  />

  {#if hint}<small id="{uid}-hint">{hint}</small>{/if}
  {@render children?.()}
  {#if showError && error}
    <small class="tz-field-error">{error}</small>
  {/if}
  {#if refusal}
    <small class="tz-field-error">{refusal}</small>
  {/if}
</div>

<style>
  /* Deliberately NOT .tz-control: this is a real <input>, so the global
     `.form-grid input` rule already gives it the same box every other field
     has. The class carries only the invalid state — the same shape, and the
     same reasoning, as NumberInput's.

     TWO classes, and that is not style. The global rule is
     `.form-grid input:not(.tz-validity, .tz-control *, .tz-check *)`, and
     `:not()` takes the specificity of its heaviest argument, so it weighs
     (0,2,1) — more than a single class plus Svelte's scoping hash. Measured
     2026-09-01: with one class the field kept the ordinary grey border while
     reporting itself invalid. NumberInput wins the same contest only because
     `.tz-number.tz-invalid` was already two. */
  .tz-text.tz-invalid {
    border-color: var(--danger);
  }
</style>
