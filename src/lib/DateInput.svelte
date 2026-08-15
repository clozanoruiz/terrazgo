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
  import { DatePicker } from "bits-ui";
  import { localeTag, formatDate, t } from "../i18n.js";
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

  // The browser still runs constraint validation before dispatching submit, so
  // forms keep blocking exactly as they did with a native input. What carries
  // that is a real (not type="hidden") input parked off-screen: `required` and
  // setCustomValidity live on it. Bits UI's own hidden input was not enough —
  // it carries no min, so the one range guard in the app would have been lost.
  let proxy = $state(null);
  let showError = $state(false);

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

  $effect(() => proxy?.setCustomValidity(rangeError));

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
    locale={localeTag()}
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
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            aria-hidden="true"
          >
            <rect x="3" y="4" width="18" height="18" rx="2" />
            <path d="M16 2v4M8 2v4M3 10h18" />
          </svg>
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
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  aria-hidden="true"
                >
                  <path d="M15 18l-6-6 6-6" />
                </svg>
              </DatePicker.PrevButton>
              <DatePicker.Heading class="tz-calendar-title" />
              <DatePicker.NextButton class="tz-calendar-nav" aria-label={t("form.next_month")}>
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  aria-hidden="true"
                >
                  <path d="M9 18l6-6-6-6" />
                </svg>
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
    {required}
    {disabled}
    tabindex="-1"
    aria-hidden="true"
    onfocus={() => document.getElementById(uid)?.focus()}
    oninvalid={() => (showError = true)}
  />

  {#if hint}<small>{hint}</small>{/if}
  {#if showError}
    <small class="tz-field-error">{rangeError || t("form.required")}</small>
  {/if}
</div>
