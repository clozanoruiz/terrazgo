<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned dropdown that NARROWS AS YOU TYPE, for the lists a plain listbox
  // cannot carry.
  //
  // It replaces the pattern this app used four times: a filter field sitting
  // beside a listbox holding hundreds of codes. Here the input IS the trigger,
  // so the second field disappears — which is also why the row cap is safe:
  // an owned dropdown renders its rows in the webview instead of handing them
  // to the OS, so long lists must narrow before they render.
  //
  // Same contract as TzSelect: the code string goes in and comes out.
  import { Check, ChevronDown } from "@lucide/svelte";
  import { Combobox } from "bits-ui";
  import { t } from "../i18n.js";
  import { refusalStore } from "./formRefusal.js";
  import { fold, searchItems } from "./collate.js";

  let {
    /// [{ value, label }] — the FULL list; narrowing happens here.
    items = [],
    value = $bindable(""),
    label = "",
    hint = "",
    placeholder = "",
    required = false,
    disabled = false,
    class: klass = "",
    /// Names this field inside its form, for TzForm's `anchors`.
    name = "",
    onchange = null,
  } = $props();

  // The same cap CataloguePicker has always used: enough to scroll, few enough
  // that a two-letter query does not paint a thousand rows.
  const MAX_VISIBLE = 40;

  const uid = $props.id();
  const labelId = `${uid}-label`;

  let query = $state("");
  let open = $state(false);

  // Fold once per list, not once per keystroke: normalize() is not cheap and
  // the biggest catalogue behind these pickers runs to thousands of rows.
  const folded = $derived(items.map((item) => ({ ...item, folded: fold(item.label) })));
  const result = $derived(searchItems(folded, query, MAX_VISIBLE));
  const chosen = $derived(items.find((item) => item.value === value));

  let proxy = $state(null);
  let showError = $state(false);

  // A backend refusal the form chose to hang on this field. Display only — it
  // never becomes a validity, so it cannot wedge the next submit.
  const refusals = refusalStore();
  const refusal = $derived(name && refusals ? (refusals.byName[name] ?? "") : "");

  // Through setCustomValidity rather than a `required` attribute, so
  // validationMessage is OUR string and not the browser's OS-language one —
  // see DateInput for the measurement.
  const error = $derived(required && !value ? t("form.required") : "");

  $effect(() => proxy?.setCustomValidity(error));

  function commit(next) {
    value = next ?? "";
    showError = false;
    onchange?.(value);
  }
</script>

<div class="tz-field {klass}">
  {#if label}
    <span class="tz-label" id={labelId}>{label}</span>
  {/if}

  <Combobox.Root
    type="single"
    items={folded}
    {value}
    onValueChange={commit}
    bind:open
    {disabled}
    onOpenChangeComplete={(isOpen) => {
      // Clear the typing on close so the next open starts from the whole list
      // rather than from whatever was last typed.
      if (!isOpen) query = "";
    }}
  >
    <div class="tz-control tz-combobox">
      <Combobox.Input
        id={uid}
        aria-labelledby={label ? labelId : undefined}
        placeholder={chosen?.label || placeholder}
        oninput={(event) => (query = event.currentTarget.value)}
        onfocus={() => (open = true)}
      />
      <Combobox.Trigger class="tz-field-trigger" aria-label={t("form.open_list")}>
        <ChevronDown />
      </Combobox.Trigger>
    </div>

    <!-- preventScroll passed EXPLICITLY — see TzSelect. Combobox.Content IS
         select-content, so it already defaults to false; saying so keeps
         correctness off a default we did not choose. -->
    <Combobox.Portal>
      <Combobox.Content preventScroll={false} sideOffset={4} class="tz-popover tz-listbox">
        <Combobox.Viewport>
          {#each result.visible as item (item.value)}
            <Combobox.Item value={item.value} label={item.label} class="tz-option">
              {#snippet children({ selected })}
                <span>{item.label}</span>
                {#if selected}
                  <Check class="tz-option-check" />
                {/if}
              {/snippet}
            </Combobox.Item>
          {/each}

          {#if result.total === 0}
            <p class="tz-option-note">{t("form.list_empty")}</p>
          {:else if result.total > result.visible.length}
            <!-- Say so rather than slicing silently: in a register whose codes
                 carry legal weight, a farmer must know the list is not all of
                 it. -->
            <p class="tz-option-note">
              {t("form.list_truncated", { shown: result.visible.length, total: result.total })}
            </p>
          {/if}
        </Combobox.Viewport>
      </Combobox.Content>
    </Combobox.Portal>
  </Combobox.Root>

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
