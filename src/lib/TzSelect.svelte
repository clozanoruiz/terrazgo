<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned dropdown, replacing <select>.
  //
  // Native selects are drawn by the OS: a menu on Linux, a different menu on
  // Windows, a full-screen sheet on Android. Owning them is what makes a form
  // look like one form everywhere — and the Android sheet is the best of the
  // three, so this has to MATCH it on a phone, not merely replace it.
  //
  // The contract is the string the call sites already bound: a code or an id in
  // and out, "" for unset. Every value in the app is already a string (ids are
  // String in Rust, codes are strings), so no coercion layer exists here.
  import { Select } from "bits-ui";
  import { t } from "../i18n.js";

  let {
    /// [{ value, label, disabled? }] — build with lib/selectItems.js.
    items = [],
    value = $bindable(""),
    label = "",
    hint = "",
    /// Trigger text while nothing is chosen. The native forms mostly used an
    /// empty `<option value="" disabled hidden>`, so "" is the default.
    placeholder = "",
    /// Whether "no answer" is a legal choice, i.e. the old `<option value="">`.
    nullable = false,
    nullLabel = "—",
    required = false,
    disabled = false,
    class: klass = "",
    /// Called with the new value. Present for the uncontrolled call sites,
    /// which pass `value` unbound, and for the cascading pairs that clear a
    /// dependent field.
    onchange = null,
  } = $props();

  const uid = $props.id();
  const labelId = `${uid}-label`;

  // The empty row goes in the ITEMS list, not just the markup: Select.Value and
  // the typeahead both read Root's `items`, so a row rendered without one there
  // is invisible to them.
  const allItems = $derived(nullable ? [{ value: "", label: nullLabel }, ...items] : items);

  // Bits UI uses "" as its own sentinel for "nothing selected", so it will not
  // resolve the empty row to a label however the items are shaped — a nullable
  // select sitting at "" showed a BLANK trigger where the native one showed its
  // `<option value="">` text. Making the placeholder that same text restores
  // parity: unset and explicitly-none look identical, which is exactly what a
  // native select does.
  const emptyText = $derived(nullable && !placeholder ? nullLabel : placeholder);

  // A tripwire, not a feature: an owned dropdown renders its rows in the
  // webview instead of handing them to the OS, so a long list is the one real
  // cost. Anything over the cap belongs in a combobox that narrows first.
  $effect(() => {
    if (items.length > 40) {
      console.warn(
        `TzSelect: ${items.length} rows for "${label || placeholder}" — over the 40-row cap; use a combobox.`,
      );
    }
  });

  let proxy = $state(null);
  let showError = $state(false);

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

  <Select.Root type="single" items={allItems} {value} onValueChange={commit} {disabled}>
    <Select.Trigger
      id={uid}
      class="tz-control tz-trigger"
      aria-labelledby={label ? labelId : undefined}
      aria-label={label ? undefined : emptyText || undefined}
    >
      <Select.Value placeholder={emptyText} />
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        aria-hidden="true"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </Select.Trigger>

    <!-- Portalled so <main>'s overflow-y: auto cannot clip it, and
         preventScroll passed EXPLICITLY: bits-ui's scroll lock releases with
         document.body.setAttribute("style", …), which the production CSP
         blocks, and a stuck lock leaves pointer-events: none on <body>. This
         content already defaults to false; saying so keeps correctness off a
         default we did not choose. -->
    <Select.Portal>
      <Select.Content preventScroll={false} sideOffset={4} class="tz-popover tz-listbox">
        <Select.Viewport>
          {#each allItems as item (item.value)}
            <Select.Item
              value={item.value}
              label={item.label}
              disabled={item.disabled ?? false}
              class="tz-option"
            >
              {#snippet children({ selected })}
                <span>{item.label}</span>
                {#if selected}
                  <svg
                    class="tz-option-check"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    aria-hidden="true"
                  >
                    <path d="M20 6L9 17l-5-5" />
                  </svg>
                {/if}
              {/snippet}
            </Select.Item>
          {/each}
        </Select.Viewport>
      </Select.Content>
    </Select.Portal>
  </Select.Root>

  <!-- Same mechanism as DateInput: the browser still runs constraint
       validation before dispatching submit, so `required` keeps blocking
       exactly as it did on a native <select>. -->
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
