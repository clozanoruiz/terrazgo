<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned date field: typeable DD/MM/YYYY segments plus a calendar popover.
  //
  // It replaces <input type="date">, whose picker follows the OPERATING SYSTEM
  // locale — overriding the language the holding chose, on a field that appears
  // in every register of the record book — and whose WebKitGTK popover takes an
  // input grab that only a focus change releases.
  //
  // The contract is deliberately the same primitive the native input had: a
  // "YYYY-MM-DD" string in, a "YYYY-MM-DD" string out (or "" for empty), so a
  // call site is a markup swap and no view logic moves. CalendarDate objects
  // live behind dateValue.js and never escape this component.
  import { Calendar, ChevronLeft, ChevronRight } from "@lucide/svelte";
  import { DatePicker } from "bits-ui";
  import { formatTag, formatDate, t } from "../i18n.js";
  import { refusalStore } from "./formRefusal.js";
  import { toCalendarDate, fromCalendarDate } from "./dateValue.js";

  let {
    value = $bindable(""),
    label = "",
    /// Rendered as the <small> under the field, like the native call sites did.
    hint = "",
    required = false,
    /// Range bounds as "YYYY-MM-DD" strings, matching the native min/max.
    min = "",
    max = "",
    disabled = false,
    class: klass = "",
    /// Names this field inside its form, so TzForm's `anchors` can reach it as
    /// form.elements[name] to hang a backend refusal on the right control.
    name = "",
    /// For the uncontrolled idiom: called with the new ISO string after a change.
    onchange = null,
  } = $props();

  const uid = $props.id();

  // Controlled, never bound: the string is the source of truth and writes come
  // back only through onValueChange. A $derived over `value` keeps the object
  // identity stable, so there is no binding loop and no half-typed date leaks —
  // the field only emits a value once every segment is filled.
  const dateValue = $derived(toCalendarDate(value));
  const minValue = $derived(toCalendarDate(min));
  const maxValue = $derived(toCalendarDate(max));

  // The browser still runs constraint validation before submit, so forms keep
  // blocking exactly as they did with a native input. What carries that is a
  // real (not type="hidden") input parked off-screen. Bits UI's own hidden
  // input was not enough — it carries no min, so the one range guard in the app
  // would have been lost.
  let proxy = $state(null);
  let showError = $state(false);

  // A backend refusal the form chose to hang on this field. Display only — it
  // never becomes a validity, so it cannot wedge the next submit.
  const refusals = refusalStore();
  const refusal = $derived(name && refusals ? (refusals.byName[name] ?? "") : "");

  const rangeError = $derived.by(() => {
    if (!dateValue) return "";
    if (minValue && dateValue.compare(minValue) < 0) {
      return t("form.date_min", { date: formatDate(min) });
    }
    if (maxValue && dateValue.compare(maxValue) > 0) {
      return t("form.date_max", { date: formatDate(max) });
    }
    return "";
  });

  // One string drives both surfaces, the NumberInput shape: the inline <small>
  // under this field and — because it reaches the proxy through
  // setCustomValidity rather than a `required` attribute — the entry TzForm
  // reads off validationMessage for the summary. Measured 2026-09-01: a bare
  // `required` leaves validationMessage as the BROWSER's own string ("Please
  // fill in this field."), which is the OS language, not the holding's. That is
  // the same defect that retired the native date picker, one layer down.
  const error = $derived(rangeError || (required && !value ? t("form.required") : ""));

  $effect(() => proxy?.setCustomValidity(error));

  function commit(next) {
    value = fromCalendarDate(next);
    showError = false;
    onchange?.(value);
  }
</script>

<div class="tz-field {klass}">
  <DatePicker.Root
    value={dateValue}
    onValueChange={commit}
    {minValue}
    {maxValue}
    {disabled}
    locale={formatTag()}
    granularity="day"
    weekdayFormat="short"
    fixedWeeks={true}
  >
    {#if label}
      <DatePicker.Label class="tz-label">{label}</DatePicker.Label>
    {/if}

    <DatePicker.Input id={uid} class="tz-control tz-datefield">
      {#snippet children({ segments })}
        <!-- Keyed by INDEX, not by part: a DD/MM/YYYY field carries TWO
             `literal` segments for its separators, and keying on `part` would
             throw on the duplicate. -->
        {#each segments as segment, i (i)}
          <DatePicker.Segment part={segment.part} class="tz-segment">
            {segment.value}
          </DatePicker.Segment>
        {/each}
        <DatePicker.Trigger class="tz-field-trigger" aria-label={t("form.open_calendar")}>
          <Calendar />
        </DatePicker.Trigger>
      {/snippet}
    </DatePicker.Input>

    <!-- Portalled because <main> is the app's only scroller and carries
         overflow-y: auto, which would clip a popover rendered in place.
         preventScroll={false} is passed EXPLICITLY: bits-ui's scroll lock
         releases with document.body.setAttribute("style", …), which the
         production CSP blocks (style-src-attr), leaving pointer-events: none
         on <body> for good. This content defaults to false already — saying so
         means correctness never rests on a default we did not choose. -->
    <DatePicker.Portal>
      <DatePicker.Content preventScroll={false} sideOffset={6} class="tz-popover">
        <DatePicker.Calendar class="tz-calendar">
          {#snippet children({ months, weekdays })}
            <div class="tz-calendar-head">
              <DatePicker.PrevButton class="tz-calendar-nav" aria-label={t("form.prev_month")}>
                <ChevronLeft />
              </DatePicker.PrevButton>
              <DatePicker.Heading class="tz-calendar-title" />
              <DatePicker.NextButton class="tz-calendar-nav" aria-label={t("form.next_month")}>
                <ChevronRight />
              </DatePicker.NextButton>
            </div>
            {#each months as month (month.value)}
              <DatePicker.Grid class="tz-calendar-grid">
                <DatePicker.GridHead>
                  <DatePicker.GridRow>
                    {#each weekdays as weekday (weekday)}
                      <DatePicker.HeadCell class="tz-calendar-weekday"
                        >{weekday}</DatePicker.HeadCell
                      >
                    {/each}
                  </DatePicker.GridRow>
                </DatePicker.GridHead>
                <DatePicker.GridBody>
                  {#each month.weeks as week, w (w)}
                    <DatePicker.GridRow>
                      {#each week as date (date.toString())}
                        <DatePicker.Cell {date} month={month.value} class="tz-calendar-cell">
                          <DatePicker.Day class="tz-calendar-day" />
                        </DatePicker.Cell>
                      {/each}
                    </DatePicker.GridRow>
                  {/each}
                </DatePicker.GridBody>
              </DatePicker.Grid>
            {/each}
          {/snippet}
        </DatePicker.Calendar>
      </DatePicker.Content>
    </DatePicker.Portal>
  </DatePicker.Root>

  <!-- Focusable but invisible, so the browser can report it as the invalid
       control; focus bounces to the segments, which is where a correction is
       actually made. -->
  <input
    class="tz-validity"
    bind:this={proxy}
    {value}
    {name}
    {disabled}
    data-tz-label={label}
    tabindex="-1"
    aria-hidden="true"
    onfocus={() => document.getElementById(uid)?.focus()}
    oninvalid={() => (showError = true)}
  />

  {#if hint}<small>{hint}</small>{/if}
  {#if showError && error}
    <small class="tz-field-error">{error}</small>
  {/if}
  {#if refusal}
    <small class="tz-field-error">{refusal}</small>
  {/if}
</div>
