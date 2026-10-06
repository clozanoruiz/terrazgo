<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // A modal whose body is one form and whose foot is that form's action bar.
  //
  // It exists because three kinds of caller need the same wiring and none of
  // them should own it: a register's list (TzWorkspace), a screen whose subject
  // is itself a record (the farm), and a band that used to toggle a form open
  // between two others (the campaign). Before this, a form was rendered beside
  // the list it belonged to on a wide screen and UNDER it on a phone, which is
  // where the shape stopped working: a form below a table is more page, with
  // nothing to say where the list ends and the questions begin.
  //
  // What it adds to TzDialog is the pairing of a pinned Save with the form it
  // submits, and the rule about who may close it.
  import { tick } from "svelte";
  import TzDialog from "./TzDialog.svelte";
  import { t } from "../i18n.js";

  let {
    /// Whether the panel is showing. A PLAIN prop, deliberately not bindable:
    /// a caller may pass a derived expression (`createOpen || openId !== null`)
    /// and `bind:` cannot take one. The dialog is therefore driven controlled —
    /// see `onclose` for what that obliges.
    open = false,
    /// What the panel's header calls the record being edited.
    title = "",
    /// Called when the panel is dismissed — Escape, the close button, or the
    /// caller's own Cancel, which every caller points here too. That is the
    /// whole of what makes Escape mean Cancel, draft reset included, with no
    /// code of its own.
    ///
    /// A HANDLER THAT DOES NOT CLOSE KEEPS THE PANEL OPEN, deliberately: the
    /// caller owns `open`, so the caller decides. The record book's campaign
    /// panel relied on it while a book with no campaign could not be dismissed
    /// into an empty screen; the book became a page of its own on 2026-09-16
    /// and no caller declines today, but a controlled dialog that ignored its
    /// caller would be wrong the day one does. Measured 2026-09-15 before
    /// `dismissed()` below existed: a derived `open` that stays `true` pushes
    /// no new value down, so nothing re-opened the panel.
    onclose = null,
    /// Deleting a record belongs where the record is on screen and named, not on
    /// a button repeated once per row of the table. Null while the panel is
    /// creating — there is nothing to delete yet.
    ondelete = null,
    deleteLabel = "",
    /// The form. Takes the id minted below.
    body = null,
    /// The form's own Save and Cancel, rendered into the pinned bar beside
    /// Delete rather than at the foot of the form. All three act on the record
    /// the header names, so they belong in one row; and Save is the one a long
    /// correction form pushed furthest out of reach. Takes the same id.
    actions = null,
  } = $props();

  /// One id for the panel's form, minted HERE rather than by each caller. The
  /// pinned Save sits outside the form element it submits — that is what pinning
  /// means — and `form="<id>"` is the only thing that ties the two back
  /// together, so the seam that owns the bar owns both ends of the pair.
  ///
  /// (Spelled "form element" rather than as the tag on purpose: the guard in
  /// `form_feedback.rs` reads the tag as markup wherever it appears, which is
  /// what makes it able to catch a bare one at all.)
  const formId = $props.id();

  /// Rendered whenever either half exists, because a panel that is creating has
  /// no Delete and a read-only panel has no Save. A caller with neither gets no
  /// bar rather than an empty one.
  const hasFoot = $derived(!!actions || !!ondelete);

  /// What the dialog is actually SHOWING, which is not always what the caller
  /// asked for. A writable `$derived`: bits-ui writes here to dismiss itself,
  /// and the caller's own value overrides that the moment it changes.
  let shown = $derived(open);

  /// After a dismissal, give the caller its say and then believe the answer: if
  /// `open` is still true the panel comes back, because the caller declined to
  /// close it. The tick is what lets a handler that sets a flag be seen.
  async function dismissed() {
    onclose?.();
    await tick();
    if (open) shown = true;
  }
</script>

<TzDialog
  bind:open={shown}
  {title}
  wide
  dismissible={false}
  onClose={dismissed}
  footer={hasFoot ? foot : null}
>
  {@render body?.(formId)}
</TzDialog>

{#snippet foot()}
  {#if actions}{@render actions(formId)}{/if}
  {#if ondelete}
    <button type="button" class="btn-danger tz-dialog-delete" onclick={ondelete}>
      {deleteLabel || t("form.delete")}
    </button>
  {/if}
{/snippet}
