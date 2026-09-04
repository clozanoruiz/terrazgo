<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned numeric field, replacing `<input type="number">`.
  //
  // The native control parses what the user types with the OPERATING SYSTEM's
  // locale rather than the language the holding chose — the same defect that
  // retired `<input type="date">`, except this one corrupts instead of merely
  // looking wrong. Measured in the shipping WebKitGTK webview: with the OS in
  // en_GB, typing "1,5" yields 15, silently. See numberValue.js for the run.
  //
  // A plain text input rather than Bits UI: the library ships no number field
  // (2.18.1), and once parsing is ours the rest is a label, a proxy for
  // constraint validation and two keyboard shortcuts. The contract mirrors
  // DateInput's — the same primitive the native input carried, so a call site
  // is a markup swap: a number in, a number out, "" for empty.
  import { t } from "../i18n.js";
  import { refusalStore } from "./formRefusal.js";
  import { fromFieldText, toFieldText, decimalSeparator } from "./numberValue.js";

  let {
    value = $bindable(""),
    label = "",
    /// Rendered as the <small> under the field, like the native call sites did.
    hint = "",
    required = false,
    /// Bounds, as numbers. Both are inclusive, matching the native attributes.
    min = null,
    max = null,
    /// Whole numbers only — animal counts, days, a campaign year. Replaces the
    /// native `step="1"`; other step values were only ever spinner hints and
    /// are deliberately not reproduced, since no rule in the decrees makes a
    /// measured width a multiple of a centimetre.
    integer = false,
    disabled = false,
    placeholder = "",
    class: klass = "",
    /// Names this field inside its form, so TzForm's `anchors` can reach it as
    /// form.elements[name] to hang a backend refusal on the right control.
    name = "",
    /// For the uncontrolled idiom: called with the new value after a change.
    onchange = null,
    /// Extra content under the field, for the one case a plain `hint` string
    /// cannot serve: the treatment form offers a computed total as a button.
    children = null,
  } = $props();

  const uid = $props.id();

  // What the user sees. Kept apart from `value` because a half-typed "1," is a
  // legitimate state that no number can represent, and rewriting the box on
  // every keystroke would fight the person typing.
  let text = $state(toFieldText(value));
  let field = $state(null);
  let proxy = $state(null);
  let focused = $state(false);
  let showError = $state(false);

  // A backend refusal the form chose to hang on this field. Display only — it
  // never becomes a validity, so it cannot wedge the next submit.
  const refusals = refusalStore();
  const refusal = $derived(name && refusals ? (refusals.byName[name] ?? "") : "");

  // The value last put on the wire, so an assignment made by the PARENT can be
  // told apart from our own write echoing back. Deliberately not $state: the
  // effect below must depend on `value` alone, never on this.
  let published = value;

  // Re-render from outside only when the parent actually changed the value —
  // loading a record into the form, or clearing it. Syncing on our own echo
  // instead was a bug worth naming: an unparseable entry publishes "", so the
  // echo wrote "" straight back over the text and the field silently emptied
  // itself on blur, taking the reason with it ("this field is required" where
  // the truth was "that is not a number").
  $effect(() => {
    if (value === published) return;
    published = value;
    if (!focused) text = toFieldText(value);
  });

  const parsed = $derived(fromFieldText(text));

  const error = $derived.by(() => {
    if (parsed.empty) return required ? t("form.required") : "";
    if (parsed.invalid) return t("form.number_invalid");
    const n = parsed.number;
    if (integer && !Number.isInteger(n)) return t("form.number_integer");
    if (min !== null && n < min) return t("form.number_min", { min: toFieldText(min) });
    if (max !== null && n > max) return t("form.number_max", { max: toFieldText(max) });
    return "";
  });

  // The browser still runs constraint validation before dispatching submit, so
  // forms keep blocking exactly as they did with a native input. A real
  // (not type="hidden") input parked off-screen carries it — the same device
  // DateInput uses, and the reason `required` is not on the visible field: an
  // unparseable entry has to block too, and only setCustomValidity can say so.
  $effect(() => proxy?.setCustomValidity(error));

  /// Update the bound value. An unparseable entry publishes "" rather than a
  /// guess and is held back by the validity proxy, so a form can never submit
  /// it — the one thing this component exists to guarantee.
  function publish() {
    const next = parsed.empty || parsed.invalid ? "" : parsed.number;
    if (next !== value) {
      published = next;
      value = next;
    }
  }

  // The value when focus arrived, so `onchange` can fire on a real change only.
  let entered = value;

  /// `onchange` keeps the NATIVE event's meaning: it fires when editing
  /// settles, not on every keystroke. The uncontrolled call sites save straight
  /// to the backend from it, so firing per keystroke would write a half-typed
  /// "9" of "90" — and then "90" — as two separate saves.
  function commit() {
    if (value !== entered) {
      entered = value;
      onchange?.(value);
    }
  }

  function onInput(event) {
    text = event.currentTarget.value;
    showError = false;
    publish();
  }

  function onBlur() {
    focused = false;
    // Canonicalise what was understood, so the reader can SEE the reading — a
    // dot typed in Castilian comes back as a comma, and a misreading would be
    // visible rather than silent.
    if (!parsed.empty && !parsed.invalid) text = toFieldText(parsed.number);
    showError = !!error;
    publish();
    commit();
  }

  /// Arrow keys step the value, which the native control did and forms rely on
  /// for quick corrections. The step is 1 for integers and otherwise follows
  /// the digits already typed, so a dose of 0,0375 nudges by 0,0001 rather than
  /// jumping to 1,0375.
  function onKeydown(event) {
    // Enter settles the edit without leaving the field, which is what the
    // native control did — the settings fields save from `onchange` and are not
    // inside a form, so without this a value only lands when focus moves away.
    if (event.key === "Enter") {
      if (!parsed.empty && !parsed.invalid) text = toFieldText(parsed.number);
      publish();
      commit();
      return;
    }
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    if (parsed.invalid) return;
    event.preventDefault();
    const current = parsed.empty ? 0 : parsed.number;
    const decimals = integer ? 0 : (toFieldText(current).split(decimalSeparator())[1] ?? "").length;
    const step = integer ? 1 : Math.pow(10, -Math.min(decimals, 6));
    const raw = current + (event.key === "ArrowUp" ? step : -step);
    // Re-round at the step's own scale: 0.1 + 0.1 is not 0.2 in binary floats.
    const stepped = Math.round(raw / step) * step;
    const clamped =
      min !== null && stepped < min ? min : max !== null && stepped > max ? max : stepped;
    text = toFieldText(Number(clamped.toPrecision(15)));
    showError = false;
    publish();
    commit();
  }
</script>

<div class="tz-field {klass}">
  {#if label}
    <label class="tz-label" for={uid}>{label}</label>
  {/if}

  <!-- type="text", not "number": the whole point is that the OS locale does
       not get to parse this. inputmode still asks a phone for a numeric keypad,
       and "decimal" is what puts a separator key on it. -->
  <input
    id={uid}
    class="tz-number"
    class:tz-invalid={showError && !!error}
    type="text"
    inputmode={integer ? "numeric" : "decimal"}
    autocomplete="off"
    autocorrect="off"
    spellcheck="false"
    enterkeyhint="done"
    value={text}
    {placeholder}
    {disabled}
    aria-invalid={showError && !!error}
    aria-describedby={hint ? `${uid}-hint` : undefined}
    bind:this={field}
    oninput={onInput}
    onfocus={() => {
      focused = true;
      entered = value;
    }}
    onblur={onBlur}
    onkeydown={onKeydown}
  />

  <!-- Focusable but invisible, so the browser can report it as the invalid
       control; focus bounces to the real field, which is where a correction is
       actually made. -->
  <input
    class="tz-validity"
    bind:this={proxy}
    value={text}
    {name}
    {disabled}
    data-tz-label={label}
    tabindex="-1"
    aria-hidden="true"
    onfocus={() => field?.focus()}
    oninvalid={() => (showError = true)}
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
     has. The class carries only what is specific to a number.

     Digits line up in a column of doses, and a partly typed figure does not
     shift the ones under it. Tabular figures only — not a monospace face,
     which would look foreign beside every other field. */
  .tz-number {
    font-variant-numeric: tabular-nums;
  }

  /* Only ever set after the field is left or a submit was refused, never while
     typing: "1," is on its way to "1,5" and must not be painted as a mistake. */
  .tz-number.tz-invalid {
    border-color: var(--danger);
  }
</style>
