<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Type-ahead over a reference catalogue that still accepts a name typed
  // freely. Extracted from SpeciesPicker so the harvested-produce field can
  // reuse it: the two answer different questions off different catalogues, and
  // the only thing they share is this behaviour.
  //
  // Two things travel out of here: the NAME, which is what the record book
  // prints, and the CODE, which is what a declaration or an export speaks in.
  // Picking from the list sets both; typing something the catalogue does not
  // know keeps the name and drops the code, because a code that does not mean
  // what the name says would be worse than none.
  //
  // WHY THIS IS NOT `TzCombobox`. That one answers "which code?", so the typed
  // text is a transient query and the value is the code alone. Here the typed
  // text IS a stored value — the name the book prints — and the code is
  // optional metadata derived from it. Same skin and same matcher, different
  // contract, so they stay two components rather than one with a mode flag.
  import { Combobox } from "bits-ui";
  import { t } from "../i18n.js";
  import { fold, searchItems } from "./collate.js";

  let {
    name = $bindable(""),
    code = $bindable(null),
    /// [{ code, name }] — whatever list the caller loaded.
    options = [],
    required = false,
    placeholder = t("crop.species_search"),
    /// Rendered under the field when nothing narrows the list; the species
    /// picker replaces it with its land-use filter chip.
    footer = null,
  } = $props();

  let open = $state(false);

  // How many matches to render at once: enough to scroll through, few enough
  // that a two-letter query does not paint a thousand rows.
  const MAX_VISIBLE = 40;

  // Fold once per list rather than once per keystroke: normalize() is not cheap
  // and the catalogues behind these fields run to thousands of rows.
  const items = $derived(
    options.map((option) => ({
      value: option.code,
      label: option.name,
      folded: fold(option.name),
    })),
  );

  // The typed name IS the query — that is the whole point of this control, and
  // what makes it different from TzCombobox, where the query is thrown away.
  const result = $derived(searchItems(items, name, MAX_VISIBLE));

  function exactCode(text) {
    const wanted = fold(text.trim());
    return items.find((item) => item.folded === wanted)?.value ?? null;
  }

  function onInput(event) {
    // Bits UI owns the input's text and writes it through `bind:inputValue`
    // (its own oninput runs alongside this one, and mergeProps chains them), so
    // `name` is already current. What is ours is the CODE: typing away from a
    // chosen entry detaches it, unless the text still names an option exactly —
    // retyping the same one keeps its code.
    name = event.currentTarget.value;
    code = exactCode(name);
    open = true;
  }

  function onValueChange(next) {
    // Selecting a row: Bits UI sets the input text to the item's label itself,
    // and `bind:inputValue` carries that into `name`. Only the code is ours.
    code = next || null;
  }

  function onKeydown(event) {
    // Bits UI treats Home and End as LIST navigation (jump to first/last row)
    // and preventDefaults them, which is right for a combobox whose input is a
    // throwaway query — TzCombobox keeps that behaviour. Here the input holds a
    // NAME the farmer is editing and the record will store, so they have to
    // move the caret the way they do in any text field.
    //
    // Reclaiming them is ordinary composition rather than a fight with the
    // library: mergeProps runs OUR handler first and stops the chain as soon as
    // the event is defaultPrevented. Since that also cancels the browser's own
    // caret move, we perform it here.
    if (event.key !== "Home" && event.key !== "End") return;
    const input = event.currentTarget;
    const to = event.key === "Home" ? 0 : input.value.length;
    event.preventDefault();
    if (!event.shiftKey) {
      input.setSelectionRange(to, to);
    } else if (event.key === "Home") {
      input.setSelectionRange(0, input.selectionEnd, "backward");
    } else {
      input.setSelectionRange(input.selectionStart, to, "forward");
    }
  }
</script>

<!-- allowDeselect={false}: re-picking the row already chosen must KEEP it. The
     default toggles it off, which here would silently strip the code from a
     name the farmer just confirmed — the exact failure this component exists to
     prevent. -->
<Combobox.Root
  type="single"
  {items}
  value={code ?? ""}
  {onValueChange}
  bind:inputValue={name}
  bind:open
  allowDeselect={false}
>
  <div class="tz-control tz-combobox">
    <!-- Opening on focus is the hand-rolled picker's behaviour, kept: the list
         is how a farmer discovers what the catalogue calls a crop, so it must
         be reachable without typing a guess first. -->
    <Combobox.Input
      {required}
      {placeholder}
      autocomplete="off"
      oninput={onInput}
      onkeydown={onKeydown}
      onfocus={() => (open = true)}
    />
    <Combobox.Trigger class="tz-field-trigger" aria-label={t("form.open_list")}>
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        aria-hidden="true"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </Combobox.Trigger>
  </div>

  <!-- Portalled, and preventScroll passed EXPLICITLY — see TzSelect: bits-ui's
       scroll lock releases through setAttribute("style", …), which the
       production CSP blocks. This content defaults to false already; saying so
       keeps correctness off a default we did not choose. -->
  <Combobox.Portal>
    <Combobox.Content preventScroll={false} sideOffset={4} class="tz-popover tz-listbox">
      <Combobox.Viewport>
        {#each result.visible as item (item.value)}
          <Combobox.Item value={item.value} label={item.label} class="tz-option">
            <span>{item.label}</span>
            <span class="detail">{t("crop.species_code", { code: item.value })}</span>
          </Combobox.Item>
        {/each}

        {#if result.total === 0}
          <p class="tz-option-note">{t("form.list_empty")}</p>
        {:else if result.total > result.visible.length}
          <!-- Announced rather than sliced silently: a species code reaches a
               PAC declaration, so a farmer must know the list is not all of it. -->
          <p class="tz-option-note">
            {t("form.list_truncated", { shown: result.visible.length, total: result.total })}
          </p>
        {/if}
      </Combobox.Viewport>
    </Combobox.Content>
  </Combobox.Portal>
</Combobox.Root>

<span class="detail">
  <!-- Whether the typed name is linked to the catalogue is worth seeing: the
       code is what a declaration and a future export speak in. -->
  {#if code}
    {t("crop.species_code", { code })} ·
  {/if}
  {#if footer}
    {@render footer()}
  {:else}
    {t("crop.species_free_text")}
  {/if}
</span>
