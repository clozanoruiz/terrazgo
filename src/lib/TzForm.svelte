<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Every form in the app. It owns the one thing a <form> could not do for
  // itself: say what is wrong, in one place, in the holding's language.
  //
  // The browser already evaluates every control on submit — what it does with
  // the result is paint a single bubble, in the OS language, beside whichever
  // control it reached first. On a register that runs several screens tall
  // (BookOtherTreatments, FarmView) that is close to no answer at all. So the
  // form carries `novalidate` and runs checkValidity() itself: same evaluation,
  // no bubbles, and the whole list in hand.
  //
  // Measured 2026-09-01 in headless Chrome, because the whole inline tier rests
  // on it: `novalidate` suppresses only the AUTOMATIC validation, so an explicit
  // checkValidity() still fires `invalid` at every failing control — which is
  // what flips each owned control's own `showError`. It also means a submit
  // event now arrives even when controls are invalid, so the gate below is not
  // optional.
  //
  // The handler is plain async and reports a refusal by THROWING; `run()` from
  // notifications.svelte.js is deliberately not used here, because the bell is
  // the wrong place for a failure that belongs to a form the farmer is looking
  // at. What still belongs to the bell is everything that belongs to no form:
  // successes worth reporting, and failures from loading, the map, exports and
  // the catalogue refresh.
  import { tick } from "svelte";
  import { t } from "../i18n.js";
  import { errorText } from "./backend.js";
  import { firstPerName, invalidFields } from "./formValidation.js";
  import { provideRefusals } from "./formRefusal.js";

  let {
    /// async () => void. Throws a boundary error to report a refusal.
    onsubmit,
    /// Names the form so a submit button OUTSIDE it can claim it with
    /// `form="<id>"`. That is what lets a panel pin Save to its foot: a pinned
    /// button is by definition not inside the scrolling form. Minted by
    /// TzFormDialog, which owns both ends of the pair; a form nobody pins leaves
    /// it unset and renders no attribute.
    id = "",
    /// { "<error code>": "<field name>" } — which control a refusal belongs to,
    /// declared by the FORM because the same code names a different field in
    /// each of them (`empty_name` is emitted by seven registers).
    anchors = {},
    class: klass = "",
    children,
  } = $props();

  let form = $state(null);
  let summary = $state(null);

  /// Whether this form has a required field, and so whether the asterisk beside
  /// one needs explaining. Read off the rendered DOM rather than declared by
  /// each caller: `required` is stated on the control, twenty-six forms would
  /// otherwise have to repeat the answer, and a form that gained or lost its
  /// last required field would be relying on someone noticing.
  ///
  /// An unexplained asterisk is the usual way this is got wrong, and the legend
  /// is the half that stops it being decoration — so it is rendered once per
  /// form, and never on a form with nothing to explain.
  ///
  /// It resolves on the pass that fills `form` through `bind:this`, by which
  /// point the children are in the DOM. It does NOT re-run when the form's own
  /// markup changes afterwards, which would matter only to a form whose LAST
  /// required field is conditional — none is, every one of them having a
  /// required name or date that is always rendered.
  const hasRequired = $derived(!!form?.querySelector('[aria-required="true"]'));

  /// Client-side problems, read back off the form after checkValidity().
  let problems = $state([]);
  /// The backend's refusal when no field on screen owns it: one message, because
  /// Rust stops at the first rule it cannot satisfy (see docs/architecture.md →
  /// the command boundary).
  let refusal = $state("");
  /// The same message when a field DOES own it, read out rather than shown: the
  /// field already shows it, and the summary that would otherwise have said it
  /// twice was also what announced it.
  let announced = $state("");
  /// Which field that refusal names, if the form declared one. Read by the
  /// owned controls through context; display only, never a validity.
  const refusals = $state({ byName: {} });

  provideRefusals(refusals);

  const count = $derived(problems.length);
  const shown = $derived(count > 0 || refusal !== "");

  /// The message half of a summary line. The separator is built here rather
  /// than written between the tags because Svelte trims whitespace at a tag
  /// boundary, and a wrapped line glued the dash to the message.
  function entryText(problem) {
    return problem.label ? ` — ${problem.message}` : problem.message;
  }

  function clear() {
    problems = [];
    refusal = "";
    announced = "";
    refusals.byName = {};
  }

  /// A refusal is said ONCE on screen: under the field it names when the form
  /// anchored its code and that field is rendered, which also takes focus, so a
  /// long panel scrolls to it; otherwise in the summary at the top, the only
  /// place left. It used to be both, and a one-line message in a big box above
  /// the same line under the field read as two errors.
  function reject(err) {
    const message = errorText(err);
    const name = err?.code ? anchors[err.code] : null;
    // `form.elements[name]` returns a RadioNodeList when several controls share
    // the name; focus/label want one element, and the first is the one on top.
    const found = name ? form?.elements?.[name] : null;
    const el = found?.length === undefined ? found : found[0];
    if (el) {
      refusals.byName[name] = message;
      announced = message;
      el.focus();
    } else {
      refusal = message;
    }
  }

  async function handle(event) {
    event.preventDefault();
    clear();

    // Fires `invalid` at each failing control, which is what draws every inline
    // message — so the summary and the fields are populated by one pass.
    if (!form.checkValidity()) {
      // One entry per named group: boxes sharing a name are one problem.
      problems = firstPerName(invalidFields(form.elements));
      // AFTER the DOM has caught up. `summary` is filled by `bind:this` on an
      // element that does not exist until `problems` is non-empty, and
      // `bind:this` lands in an effect rather than in this synchronous block —
      // so focusing here without the tick focuses null, and the reader is left
      // on the Save button with the answer somewhere above them. Measured
      // 2026-09-15: the FIRST failed submit moved nothing and the second
      // worked, which is exactly the shape a missing tick makes. It matters
      // more now than it did inline — Save is pinned to the foot of a panel
      // that scrolls, so the summary can be entirely off-screen.
      await tick();
      summary?.focus();
      return;
    }

    try {
      await onsubmit();
    } catch (err) {
      reject(err);
    }
  }
</script>

<form bind:this={form} id={id || undefined} class={klass} novalidate onsubmit={handle}>
  {#if shown}
    <!-- tabindex=-1 so the form can move focus here: a farmer who pressed Save
         at the foot of a long register would otherwise be told nothing they can
         see. role=alert announces it to a screen reader for the same reason. -->
    <div class="validation-summary" role="alert" tabindex="-1" bind:this={summary}>
      <p class="validation-summary-title">
        {count > 0 ? t("form.check_fields", { count }) : t("form.save_refused")}
      </p>

      {#if refusal}
        <p class="validation-summary-refusal">{refusal}</p>
      {/if}

      {#if count > 0}
        <ul>
          {#each problems as problem, i (i)}
            <li>
              <button
                type="button"
                class="link-button"
                aria-label={t("form.goto_field")}
                onclick={() => problem.el.focus()}
              >
                {#if problem.label}<strong>{problem.label}</strong>{/if}{entryText(problem)}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}

  <!-- Always in the DOM, because a live region added together with its text is
       not reliably announced. Empty until a field-owned refusal fills it. -->
  <p class="form-announce" role="alert">{announced}</p>

  {#if hasRequired}
    <p class="form-required-legend">
      <span class="tz-required" aria-hidden="true">*</span>
      {t("form.required_legend")}
    </p>
  {/if}

  {@render children?.()}
</form>
