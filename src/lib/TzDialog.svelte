<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned modal, and the ONE place bits-ui's Dialog may be imported.
  //
  // WHY THE ESLINT DISABLE BELOW IS NOT A LOOPHOLE. The 2026-08-14 CSP
  // measurement found that bits-ui's body scroll lock RELEASES by calling
  // `document.body.setAttribute("style", …)` — blocked by `default-src 'self'`
  // — so the lock engages and never lifts, stranding `pointer-events: none` on
  // <body> until the app is restarted. That measurement was right. The
  // conclusion drawn from it, banning four components outright, was wider than
  // the defect. Re-read out of bits-ui 2.18.1 on 2026-08-26:
  //
  //   * `document.body.setAttribute("style", …)` appears EXACTLY ONCE in the
  //     whole dist — internal/body-scroll-lock.svelte.js, inside
  //     `resetBodyStyle()`;
  //   * `resetBodyStyle()` is reachable ONLY from `BodyScrollLock`'s teardown;
  //   * `new BodyScrollLock` appears EXACTLY ONCE —
  //     utilities/scroll-lock/scroll-lock.svelte, wrapped in
  //     `if (preventScroll)`;
  //   * `<ScrollLock>` is rendered by exactly two components, dialog-content
  //     and alert-dialog-content, and both forward a `preventScroll` prop.
  //
  // So `preventScroll={false}` means the lock is never CONSTRUCTED and the
  // blocked line is unreachable — the same prop, guard and mechanism the other
  // four owned controls already rely on. Here it also costs nothing at all:
  // styles.css sets `body { overflow: hidden }`, so this app has no body scroll
  // to lock. What the lock would otherwise buy — blocking interaction behind
  // the panel — comes from Dialog.Overlay and the focus trap, both unaffected.
  //
  // The ban stands for AlertDialog, DropdownMenu and ContextMenu, and it stands
  // for every view: views never import bits-ui, owned controls do, and every
  // one of them passes preventScroll={false}. See
  // docs/frontend-conventions.md → "Forbidden Bits UI components".
  //
  // eslint-disable-next-line no-restricted-imports
  import { Dialog } from "bits-ui";
  import { X } from "@lucide/svelte";
  import { t } from "../i18n.js";

  let {
    open = $bindable(false),
    /// Plain string, like every other owned control's `label`. Rendered as the
    /// panel heading and wired to aria-labelledby by Dialog.Title.
    title = "",
    /// Optional callback for a caller that needs to react to dismissal; the
    /// bound `open` is the source of truth either way.
    onClose = null,
    /// Take the full height the dialog is allowed rather than the height the
    /// content happens to need, and let the body scroll inside it.
    ///
    /// For a dialog whose content SWITCHES — the About panel's tabs — where a
    /// box that resizes under the pointer as you move between tabs reads as the
    /// window jumping rather than as the content changing. A phone dialog is
    /// already full-height, so this only ever applies on wide screens.
    fill = false,
    children,
  } = $props();

  function handleOpenChange(next) {
    open = next;
    if (!next) onClose?.();
  }
</script>

<Dialog.Root bind:open onOpenChange={handleOpenChange}>
  <Dialog.Portal>
    <Dialog.Overlay class="tz-dialog-overlay" />
    <!-- preventScroll passed EXPLICITLY — see the block comment above.

         preventOverflowTextSelection={false} because bits-ui's default fights
         this app's selection policy. Its text-selection layer sets an INLINE
         `user-select: text` on the content (and `none` on <body>) between
         pointerdown and pointerup, so a drag inside the dialog selects its
         chrome — the title included — even though styles.css sets
         `body { user-select: none }` app-wide precisely to stop that. Turning
         the layer off restores the app rule; the panel then opts its own
         technical block back in, like `.notif-panel li span` does for error
         text. Inline styles beat any selector, so CSS alone could not have
         fixed this without `!important`. -->
    <Dialog.Content
      preventScroll={false}
      preventOverflowTextSelection={false}
      class="tz-dialog {fill ? 'fill' : ''}"
    >
      <div class="tz-dialog-head">
        <Dialog.Title class="tz-dialog-title">{title}</Dialog.Title>
        <Dialog.Close class="tz-dialog-close" aria-label={t("form.close")}>
          <X />
        </Dialog.Close>
      </div>
      <div class="tz-dialog-body">
        {@render children?.()}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
