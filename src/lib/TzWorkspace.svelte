<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // A list pane with an inspector beside it (wide screens) or under it (narrow).
  //
  // What it settles is not only density. The inline create/edit form was
  // rendered ABOVE the list in nine components and BELOW it in ten — split
  // along the registry/record-book line, so the same gesture moved the list in
  // opposite directions depending on which screen you were on. A pane has no
  // such choice to make.
  //
  // The list pane is its own scroller on wide screens (see .pane-list), which is
  // what lets rows move under a form that stays put. Below 700px the inspector
  // is simply the next block, which is the layout every caller already had.
  import TzTooltip from "./TzTooltip.svelte";
  import { t } from "../i18n.js";

  // How wide the reader made the inspector, in pixels. A per-device display
  // preference like the collapsed sidebar, so it lives in localStorage and not
  // in the database — nothing about it belongs to the holding's records.
  const STORE_KEY = "terrazgo.inspector";
  const MIN_W = 260;
  /// The inspector never takes more than this share of the workspace: a pane
  /// wide enough to hide the table it is describing has stopped being a pane.
  const MAX_SHARE = 0.7;

  let workspaceEl = $state(null);
  let width = $state(readStored());
  // The ceiling in pixels, remembered between drags so a keyboard nudge clamps
  // against the same limit a drag does.
  let maxWidth = $state(0);

  function readStored() {
    try {
      const stored = Number(localStorage.getItem(STORE_KEY));
      return Number.isFinite(stored) && stored >= MIN_W ? stored : null;
    } catch {
      // A private window or blocked site data: fall back to the CSS default.
      return null;
    }
  }

  /// Clamped here rather than in CSS, because the ceiling depends on the
  /// workspace's own width and `min()` cannot see it.
  function setWidth(next) {
    const total = workspaceEl?.getBoundingClientRect().width ?? 0;
    maxWidth = total > 0 ? Math.round(total * MAX_SHARE) : maxWidth;
    const max = maxWidth > 0 ? maxWidth : next;
    width = Math.round(Math.max(MIN_W, Math.min(next, max)));
    try {
      localStorage.setItem(STORE_KEY, String(width));
    } catch {
      // Not being able to remember the width is not a reason to refuse it.
    }
  }

  /// Double-clicking the splitter puts the pane back to its default width,
  /// which is the only way back once it has been dragged.
  function resetWidth() {
    width = null;
    try {
      localStorage.removeItem(STORE_KEY);
    } catch {
      // Nothing to undo if it was never stored.
    }
  }

  // The drag. Pointer capture rather than window listeners: the pointer keeps
  // reporting to the handle even when it leaves it, which is exactly what a
  // drag needs and what makes a fast drag out of the window stop cleanly.
  function startDrag(event) {
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function drag(event) {
    if (!event.currentTarget.hasPointerCapture?.(event.pointerId)) return;
    const right = workspaceEl?.getBoundingClientRect().right ?? 0;
    setWidth(right - event.clientX);
  }

  /// The keyboard half of the window-splitter pattern: a separator that can be
  /// focused has to be movable, or it is a focus stop that does nothing.
  function nudge(event) {
    const step = event.shiftKey ? 64 : 16;
    if (event.key === "ArrowLeft") setWidth((width ?? MIN_W) + step);
    else if (event.key === "ArrowRight") setWidth((width ?? MIN_W) - step);
    else return;
    event.preventDefault();
  }

  // CSSOM, never setAttribute("style", …): the production CSP blocks the
  // attribute write and honours this one (docs/frontend-conventions.md).
  $effect(() => {
    if (!workspaceEl) return;
    if (width == null) workspaceEl.style.removeProperty("--inspector-w");
    else workspaceEl.style.setProperty("--inspector-w", `${width}px`);
  });

  let {
    // Whether the inspector is showing. The caller owns the state; this
    // component only decides where the form goes.
    open = false,
    // What the inspector's header calls what is being edited.
    title = "",
    onclose = null,
    // Deleting a record belongs where the record is on screen and named, not
    // on a button repeated once per row in the table. Null while the inspector
    // is creating — there is nothing to delete yet.
    ondelete = null,
    deleteLabel = "",
    list,
    inspector = null,
    /// The panel's own actions — Save and Cancel — rendered into the pinned bar
    /// beside Delete rather than at the foot of the form. All three act on the
    /// record the header names, so they belong in one row; and Save is the one
    /// a long correction form pushed furthest out of reach.
    ///
    /// Takes the form id below, because a pinned submit is by definition
    /// outside the form it submits.
    actions = null,
  } = $props();

  /// One id for the panel's form, minted HERE rather than by each caller. The
  /// pinned Save sits outside the form element it belongs to — that is what
  /// pinning means — and `form="<id>"` is the only thing that ties the two back
  /// together, so the seam that owns the bar owns both ends of the pair.
  ///
  /// (Spelled "form element" rather than as the tag on purpose: the guard in
  /// `form_feedback.rs` reads the tag as markup wherever it appears, which is
  /// what makes it able to catch a bare one at all.)
  const formId = $props.id();
</script>

<div class="workspace" bind:this={workspaceEl}>
  <div class="pane-list">
    {@render list()}
  </div>

  {#if open && inspector}
    <!-- The splitter: a plain <button>, drag with the pointer, arrow keys to
         nudge (Shift for a bigger step), double-click to put it back.

         NOT `role="separator"`, which is what the ARIA window-splitter pattern
         asks for. Measured 2026-09-02: eslint-plugin-svelte rejects BOTH
         spellings of it — a focusable <div role="separator"> as a
         non-interactive element given interactions, and <button
         role="separator"> as an interactive element given a non-interactive
         role — because its model has no focusable-separator variant, which the
         ARIA spec does have. A labelled button announces as "resize the panel,
         button" and is operable by every route the splitter role would have
         been; the role would add valuenow semantics and an eslint exception,
         and only one of those is worth anything to a reader. Hidden below
         700px, where there is nothing to divide. -->
    <TzTooltip label={t("workspace.resize")}>
      {#snippet trigger(props)}
        <button
          {...props}
          type="button"
          class="pane-resizer"
          aria-label={t("workspace.resize")}
          onpointerdown={(event) => {
            props.onpointerdown?.(event);
            startDrag(event);
          }}
          onpointermove={drag}
          onkeydown={nudge}
          ondblclick={resetWidth}
        ></button>
      {/snippet}
    </TzTooltip>

    <aside class="inspector">
      <div class="inspector-head">
        <span>{title}</span>
        {#if onclose}
          <TzTooltip label={t("form.cancel")}>
            {#snippet trigger(props)}
              <button
                {...props}
                type="button"
                class="inspector-close"
                onclick={(event) => {
                  props.onclick?.(event);
                  onclose();
                }}
                aria-label={t("form.cancel")}>×</button
              >
            {/snippet}
          </TzTooltip>
        {/if}
      </div>
      {@render inspector(formId)}

      <!-- One pinned bar for everything you can do to this record: the form's
           own Save and Cancel, and Delete at the trailing edge, away from
           them. Rendered whenever either half exists, because a panel that is
           creating has no Delete and a read-only panel has no Save. -->
      {#if actions || ondelete}
        <div class="inspector-actions">
          {#if actions}{@render actions(formId)}{/if}
          {#if ondelete}
            <button type="button" class="btn-danger inspector-delete" onclick={ondelete}>
              {deleteLabel || t("form.delete")}
            </button>
          {/if}
        </div>
      {/if}
    </aside>
  {/if}
</div>
