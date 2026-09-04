<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned tooltip, replacing the native `title` attribute.
  //
  // A `title` is drawn by the platform and has NO styling hook at all — no
  // selector, no pseudo-element — so it follows the OS and not the app: a GTK
  // chip in WebKitGTK, something else again on Android. That is the same
  // argument the selects and date fields were owned on, with one honest
  // difference worth stating: those were owned for CORRECTNESS (the native
  // picker overrode the holding's chosen language), and this one is owned for
  // appearance.
  //
  // Bits UI rather than a CSS-only tooltip, because several triggers sit inside
  // clipping ancestors — `.tabstrip` clips, `.view.framed` clips, the inspector
  // scrolls — and a tip that is a child of its trigger gets cut off by every one
  // of them. This portals to <body>.
  //
  // `preventScroll` does not appear here, and for once that is not an omission:
  // `tooltip-content.svelte` hardcodes `preventScroll={false}` itself, so the
  // body scroll lock whose teardown the production CSP blocks is never
  // constructed. Verified in bits-ui 2.18.1 (2026-09-03).
  import { Tooltip } from "bits-ui";

  let {
    /// The tip's text. Empty means NO tooltip — the trigger renders bare. That
    /// is what lets a caller decide per render (the sidebar's links want one
    /// only while collapsed, and a zone chip only when it has a detail to
    /// show) without wrapping the call site in an `{#if}`.
    label = "",
    /// Which side it opens on. "top" suits a control in a row; "right" suits
    /// one in a vertical rail, where a tip above would cover its neighbour.
    side = "top",
    /// How long the pointer must rest before it opens. Long enough not to fire
    /// while the pointer crosses a toolbar on its way somewhere else.
    delay = 400,
    /// Rendered with the trigger's own props, which the caller spreads onto ITS
    /// element. A wrapping trigger is not an option: every call site here is
    /// already a <button>, an <a> or a <span>, and a <button> inside a <button>
    /// is invalid.
    ///
    /// **A handler declared after the spread REPLACES the one in `props`, and
    /// two of them are load-bearing.** `onclick` is what dismisses the tip, and
    /// `onpointerdown` is what dismisses it when the pointer is about to be
    /// captured — so a call site needing either must CALL the one it received:
    ///
    ///     onclick={(event) => { props.onclick?.(event); mine(event); }}
    ///
    /// Clobbering `onclick` is not cosmetic. It left the collapsed-rail toggle
    /// showing its tip at the position the button had BEFORE the rail resized,
    /// with the label of the state it had just left. The handlers nothing
    /// overrides — enter, leave, focus, blur — need no such care.
    trigger,
  } = $props();
</script>

{#if label}
  <!-- A Provider per tooltip rather than one around the app: a view never
       imports bits-ui, so the alternative would be a second import in
       App.svelte. What that costs is the shared "skip the delay when moving
       straight from one tip to the next" behaviour, which is worth less than
       keeping the library behind one wrapper. -->
  <Tooltip.Provider delayDuration={delay}>
    <!-- `disableHoverableContent` is what makes the tip inert to the pointer,
         and it has to be said HERE rather than in CSS: bits-ui writes
         `pointer-events` inline off this very flag, and an inline style beats
         any stylesheet rule (measured — the sheet's `none` computed to `auto`).
         Hoverable content is right for a tip you can put a link in; this one is
         a line of text, and a line of text that swallows a click on the control
         beneath it is a defect. -->
    <Tooltip.Root disableHoverableContent>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          {@render trigger(props)}
        {/snippet}
      </Tooltip.Trigger>

      <Tooltip.Portal>
        <Tooltip.Content class="tz-tooltip" {side} sideOffset={6}>{label}</Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  </Tooltip.Provider>
{:else}
  {@render trigger({})}
{/if}
