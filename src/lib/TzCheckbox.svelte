<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned checkbox — and the second owned control that is NOT built on Bits
  // UI, for the reason NumberInput is not: the library ships nothing this needs.
  //
  // The other owned controls exist for CORRECTNESS. The native date picker
  // follows the OS locale over the language the holding chose; the native
  // numeric field reads "1,5" as 15 under an English OS (the literal spelling
  // is left out on purpose — number_formatting.rs scans for it by substring,
  // comments included). A checkbox has no locale parsing, no
  // platform popover and no input grab — it is the one native control with
  // nothing wrong with it, so what is owned here is the SKIN, and the element
  // underneath stays a real `<input type="checkbox">`.
  //
  // That keeps, correctly and for free, everything a `<button role="checkbox">`
  // would have to re-implement: the role and checked state, space to toggle,
  // the label as a click target, and constraint validation. Read out of
  // bits-ui 2.18.1: `Checkbox.Root` renders `<button {...mergedProps}>` and
  // renders its form input only when `name` is set
  // (`shouldRender = Boolean(this.root.trueName)`), so adopting it would have
  // traded a working input for a button plus a hidden input beside it.
  //
  // The skin lives in styles.css with the other .tz-* blocks rather than in a
  // scoped block here: the global `.form-grid label` and `.form-grid input`
  // rules have to be opted out of by name, and a rule that exists to override
  // another belongs beside it.
  import { Check } from "@lucide/svelte";
  import { t } from "../i18n.js";
  import { refusalStore } from "./formRefusal.js";

  let {
    /// Single-checkbox form: the checked state itself.
    checked = $bindable(false),
    /// Group form: an array of the chosen `value`s. Present because a component
    /// cannot forward Svelte's `bind:group`, which is an `<input>`-only
    /// directive — so the call sites that had one bind this instead. When it is
    /// an array it drives `checked`, which is then ignored.
    group = $bindable(null),
    /// This box's entry in `group`. Unused in the single-checkbox form.
    value = "",
    label = "",
    /// Keeps the label out of SIGHT but not out of the accessible tree: a
    /// checkbox in a table cell is named by the row it sits in, and a visible
    /// label there would repeat the row. The text is still the control's only
    /// accessible name, so it is hidden, never dropped. Hints and error
    /// messages go with it, which is why nothing that can refuse uses this.
    labelHidden = false,
    /// Rendered as the <small> under the label, like the native call sites did.
    hint = "",
    required = false,
    disabled = false,
    class: klass = "",
    /// Names this field inside its form, for TzForm's `anchors`. Boxes sharing
    /// a name are a native group, where one tick satisfies `required` for all —
    /// which is the meaning a grouped checkbox wants, and the reason this is
    /// worth having even though nothing passes it yet.
    name = "",
    /// Called with the new checked state after a change, for the uncontrolled
    /// idiom the other owned controls use.
    onchange = null,
  } = $props();

  const uid = $props.id();

  const grouped = $derived(Array.isArray(group));
  const isChecked = $derived(grouped ? group.includes(value) : checked);

  // The input is real, so it carries its own validity and needs no proxy. What
  // it must not carry is the bare `required` attribute, for the reason every
  // owned control now avoids it: that leaves validationMessage as the browser's
  // own string, in the OS language rather than the holding's (measured
  // 2026-09-01, see DateInput). Nothing passes `required` here today; the path
  // exists so the first field that does is not the one that discovers the gap.
  let input = $state(null);
  let showError = $state(false);

  const error = $derived(required && !isChecked ? t("form.required") : "");

  $effect(() => input?.setCustomValidity(error));

  // A backend refusal the form chose to hang on this field. Display only — it
  // never becomes a validity, so it cannot wedge the next submit.
  const refusals = refusalStore();
  const refusal = $derived(name && refusals ? (refusals.byName[name] ?? "") : "");

  function onChange(event) {
    const next = event.currentTarget.checked;
    if (grouped) {
      group = next ? [...group, value] : group.filter((entry) => entry !== value);
    } else {
      checked = next;
    }
    onchange?.(next);
  }
</script>

<!-- The input sits INSIDE the label, so the association is implicit and the
     whole row is the click target without an id/for pair to keep in step. -->
<label
  class="tz-check {klass}"
  class:tz-check-disabled={disabled}
  class:tz-check-bare={labelHidden}
>
  <input
    bind:this={input}
    class="tz-check-input"
    type="checkbox"
    checked={isChecked}
    {value}
    {name}
    {disabled}
    data-tz-label={label}
    aria-describedby={hint ? `${uid}-hint` : undefined}
    onchange={onChange}
    oninvalid={() => (showError = true)}
  />

  <!-- The box is drawn by .tz-check-input itself (appearance: none); this span
       carries only the tick, so the glyph can be centred over the box without
       the input having to host a child. Same path as TzSelect's row check. -->
  <span class="tz-check-mark" aria-hidden="true">
    <Check strokeWidth={3} />
  </span>

  <span class="tz-check-text">
    {label}
    {#if hint}<small id="{uid}-hint">{hint}</small>{/if}
    {#if showError && error}
      <small class="tz-field-error">{error}</small>
    {/if}
    {#if refusal}
      <small class="tz-field-error">{refusal}</small>
    {/if}
  </span>
</label>
