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
  import { BitsConfig, Dialog } from "bits-ui";
  import { X } from "@lucide/svelte";
  import { t } from "../i18n.js";

  // WHERE A DROPDOWN INSIDE THIS PANEL GOES, AND WHY IT IS NOT <body>.
  // The stacking ladder (styles.css) runs --z-popover: 32 under --z-modal: 40,
  // deliberately: a popover has to outrank the view's sticky bands and must NOT
  // outrank the shell's own top band. Portalled to <body>, an owned control's
  // `.tz-popover` is therefore a SIBLING of this panel at a lower step, and every
  // select, combobox, catalogue picker and date picker opened inside a form here
  // would paint UNDERNEATH it. The About panel never met this, holding no
  // controls; a register's form holds dozens.
  //
  // Raising the popover past the modal would cost two rungs to fix one screen —
  // --z-tooltip sits between them — and the cost would land on all 78 TzSelect
  // sites, including every one that never meets a dialog. And it could not be
  // done in a stylesheet anyway: use-floating-layer copies the CONTENT's
  // computed z-index onto the floating wrapper it positions, so there is no
  // outer element left at a neutral step for a selector to raise.
  //
  // So the layers move instead of the ladder. A nested BitsConfig retargets the
  // portal for this subtree only; createConfigResolver walks current level →
  // parent → undefined, so `defaultLocale` keeps coming from the shell's config
  // and nothing outside this panel changes. Inside .tz-dialog's own stacking
  // context a descendant at 32 paints above the panel's content, which is all
  // that was ever wanted.
  //
  // Three details are load-bearing:
  //
  //   * the target is .tz-dialog, NEVER .tz-dialog-body — the body is
  //     `overflow-y: auto` and would clip a dropdown at its edge;
  //   * `?? undefined` and not `?? null`. undefined falls through to the shell's
  //     "body"; null is a value, and portal.svelte throws
  //     `Unknown portal target type: null` on it while the panel is closed;
  //   * .tz-dialog carries no `transform`. A transform establishes the
  //     containing block for `position: fixed` descendants, and the floating
  //     wrapper is fixed by default — so the panel is centred with
  //     `inset: 0; margin: auto` and the wrapper keeps resolving against the
  //     viewport exactly as it does from <body>.
  //
  // What did NOT need solving: Escape ordering, outside-click and focus are
  // stack-managed by bits-ui through globalThis registries and a focus-scope
  // manager, none of which consults the portal target. An open select takes the
  // Escape before the panel does, unaided.
  let dialogEl = $state(null);
  let bodyEl = $state(null);

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
    /// Wide enough for a form rather than for a paragraph. See the `.tz-dialog.wide`
    /// rule for the arithmetic; orthogonal to `fill`, and a caller wants one or
    /// the other rather than both.
    wide = false,
    /// The pinned bar at the foot of the panel: a form's own Save and Cancel,
    /// and Delete at the trailing edge. A third flex item beside the head and
    /// the body, so it stays put while the body scrolls under it.
    footer = null,
    /// Whether a click on the overlay closes the panel. True for something you
    /// are reading, which you dismiss by looking away from it. False for
    /// something you are filling in: a stray click beside a correction form is
    /// not a decision to discard it, and this panel has a close button, a
    /// Cancel and an Escape that all are. Escape is unaffected either way.
    dismissible = true,
    children,
  } = $props();

  /// Focus lands on the panel itself, never on the first control inside it.
  ///
  /// bits-ui's focus scope takes the first TABBABLE element on open, which here
  /// is the close button — where Enter, the reflex after opening anything,
  /// shuts the panel. Cancel would be no better for the same reason, and the
  /// first field is worse on a phone, where it raises the soft keyboard over a
  /// form nobody has read yet. The panel's own box is what the ARIA practices
  /// allow instead, and it is what a screen reader needs: the title first, then
  /// the content, rather than the way out.
  function focusPanel(event) {
    event.preventDefault();
    bodyEl?.focus();
  }

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
      bind:ref={dialogEl}
      preventScroll={false}
      preventOverflowTextSelection={false}
      onOpenAutoFocus={focusPanel}
      onInteractOutside={dismissible ? undefined : (event) => event.preventDefault()}
      class="tz-dialog {fill ? 'fill' : ''} {wide ? 'wide' : ''} {footer ? 'has-foot' : ''}"
    >
      <div class="tz-dialog-head">
        <Dialog.Title class="tz-dialog-title">{title}</Dialog.Title>
        <Dialog.Close class="tz-dialog-close" aria-label={t("form.close")}>
          <X />
        </Dialog.Close>
      </div>
      <!-- tabindex so a caller can park focus on the panel itself rather than
           on the first control bits-ui would otherwise reach, which here is the
           close button. -->
      <div class="tz-dialog-body" tabindex="-1" bind:this={bodyEl}>
        <BitsConfig defaultPortalTo={dialogEl ?? undefined}>
          {@render children?.()}
        </BitsConfig>
      </div>
      {#if footer}
        <div class="tz-dialog-foot">{@render footer()}</div>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
