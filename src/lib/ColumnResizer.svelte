<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // One draggable column boundary, mounted into a <th> by `columnResize.js`.
  //
  // The handle used to be a `document.createElement("button")` inside that
  // action, which was fine until it needed a tooltip: an owned tooltip is a
  // component, and a component cannot be attached to a hand-built element. So
  // the markup moved here and the action mounts this instead — it still owns
  // every measurement and every gesture, and hands them down as callbacks.
  import TzTooltip from "./TzTooltip.svelte";

  let {
    /// Names the column, so a row of six handles does not announce "resize"
    /// six times. It is both the accessible name and the tip.
    label = "",
    onpointerdown,
    onpointermove,
    onpointerup,
    onkeydown,
  } = $props();
</script>

<TzTooltip {label}>
  {#snippet trigger(props)}
    <!-- The trigger's own pointerdown is CALLED, not replaced: it is what
         dismisses the tip as the drag begins, and pointer capture means the
         leave event that would otherwise dismiss it never arrives. Everything
         else the trigger listens for — enter, leave, focus, blur — survives the
         spread untouched. -->
    <button
      {...props}
      type="button"
      class="col-resizer"
      aria-label={label}
      onpointerdown={(event) => {
        props.onpointerdown?.(event);
        onpointerdown(event);
      }}
      {onpointermove}
      {onpointerup}
      onpointercancel={onpointerup}
      {onkeydown}
    ></button>
  {/snippet}
</TzTooltip>
