<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned menu, and the ONE place bits-ui's DropdownMenu may be imported.
  //
  // WHY THE ESLINT DISABLE BELOW IS NOT A LOOPHOLE — and why it is doing more
  // work here than the one in TzDialog. The rule has never been "these
  // components are broken"; it is "a view never imports bits-ui, an owned
  // control may, and every one of them passes `preventScroll={false}`". This is
  // an owned control and it passes it.
  //
  // Re-read out of bits-ui 2.18.1 on 2026-09-03, because the note in
  // docs/frontend-conventions.md said DropdownMenu no longer takes the lock at
  // all and that is NOT what the dist says:
  //
  //   * `<ScrollLock>` is rendered by three components, not two —
  //     `dialog-content`, `alert-dialog-content`, and
  //     `utilities/popper-layer/popper-layer-inner.svelte`, which every
  //     floating layer goes through, this menu included;
  //   * popper-layer-inner resolves it as `preventScroll ?? true`, so an
  //     unstated prop means the lock IS constructed;
  //   * `Select.Content` and `Combobox.Content` declare `preventScroll = false`
  //     as their own default and are safe without help. `DropdownMenu.Content`
  //     declares no default at all.
  //
  // So `preventScroll={false}` below is load-bearing rather than belt and
  // braces: without it this menu constructs the lock, whose teardown calls
  // `document.body.setAttribute("style", …)` — blocked by the production CSP —
  // stranding `pointer-events: none` on <body> until the app restarts.
  //
  // eslint-disable-next-line no-restricted-imports
  import { DropdownMenu } from "bits-ui";
  import { Check, ChevronDown } from "@lucide/svelte";

  let {
    /// [{ value, label }] — the rows, in the order they are offered.
    items = [],
    /// Which row is the current one, "" for none. A menu that picks one of a
    /// set is a radio group and says so: the rows get `role="menuitemradio"`
    /// and `aria-checked`, which is what tells a screen reader that the tab it
    /// cannot see in the strip is the one it is on.
    value = "",
    /// Called with the picked value.
    onselect = null,
    /// The trigger's text, and its accessible name.
    label = "",
    /// Classes for the trigger, so it can be dressed as whatever it sits
    /// among — a tab, for the strip's overflow button.
    triggerClass = "",
    open = $bindable(false),
  } = $props();
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Trigger class={triggerClass}>
    <span>{label}</span>
    <ChevronDown />
  </DropdownMenu.Trigger>

  <!-- Portalled so <main>'s overflow-y: auto cannot clip it, and preventScroll
       passed EXPLICITLY — see the block comment above for why this one is not
       optional. `align="end"` because the trigger sits at the trailing edge of
       its bar: aligning the sheet's start there would hang it off the screen. -->
  <DropdownMenu.Portal>
    <DropdownMenu.Content
      preventScroll={false}
      sideOffset={4}
      align="end"
      class="tz-popover tz-menu"
    >
      <DropdownMenu.RadioGroup {value} onValueChange={(next) => onselect?.(next)}>
        {#each items as item (item.value)}
          <DropdownMenu.RadioItem value={item.value} class="tz-option">
            {#snippet children({ checked })}
              <span>{item.label}</span>
              {#if checked}
                <Check class="tz-option-check" />
              {/if}
            {/snippet}
          </DropdownMenu.RadioItem>
        {/each}
      </DropdownMenu.RadioGroup>
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
