<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned time field: typeable HH:MM segments, no popover (there is nothing
  // useful to put in one — a clock face is slower than typing two numbers).
  //
  // Same contract as DateInput: an "HH:MM" string in, an "HH:MM" string out,
  // or "" for empty. Time objects live behind dateValue.js.
  //
  // The value is a LOCAL WALL CLOCK, deliberately outside the app's ISO-UTC
  // convention: what makes an application hour relevant is the hour on the
  // ground (label restrictions, bees, heat), no timezone is stored anywhere,
  // and a UTC round-trip would print back an hour the farmer never recorded.
  import { TimeField } from "bits-ui";
  import { localeTag, t } from "../i18n.js";
  import { toTime, fromTime } from "./dateValue.js";

  let {
    value = $bindable(""),
    label = "",
    hint = "",
    required = false,
    disabled = false,
    class: klass = "",
    onchange = null,
  } = $props();

  const uid = $props.id();
  const timeValue = $derived(toTime(value));

  let proxy = $state(null);
  let showError = $state(false);

  function commit(next) {
    value = fromTime(next);
    showError = false;
    onchange?.(value);
  }
</script>

<div class="tz-field {klass}">
  <!-- hourCycle is FIXED at 24, not derived from the locale: `en` would
       otherwise add a dayPeriod segment and show 2:30 PM for a value that is
       stored, exported and printed as 24-hour. The locale still drives the
       separator and the placeholder wording. -->
  <TimeField.Root
    value={timeValue}
    onValueChange={commit}
    {disabled}
    locale={localeTag()}
    hourCycle="24"
    granularity="minute"
  >
    {#if label}
      <TimeField.Label class="tz-label">{label}</TimeField.Label>
    {/if}

    <TimeField.Input id={uid} class="tz-control tz-timefield">
      {#snippet children({ segments })}
        <!-- Keyed by index: an HH:MM field carries a `literal` separator, and a
             field with seconds would carry two. -->
        {#each segments as segment, i (i)}
          <TimeField.Segment part={segment.part} class="tz-segment">
            {segment.value}
          </TimeField.Segment>
        {/each}
      {/snippet}
    </TimeField.Input>
  </TimeField.Root>

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
    <small class="tz-field-error">{t("form.required")}</small>
  {/if}
</div>
