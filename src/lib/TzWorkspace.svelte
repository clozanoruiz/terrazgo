<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // A list, and the editor for one of its rows in a modal over it.
  //
  // What it settles is not density but separation. The inline create/edit form
  // was rendered ABOVE the list in nine components and BELOW it in ten — split
  // along the registry/record-book line, so the same gesture moved the list in
  // opposite directions depending on which screen you were on. A pane beside the
  // list answered that, and left a second question open: a pane is one more
  // thing on a screen that already carries a toolbar, a table and an action bar,
  // and below 700px it was not a pane at all but simply more page under the
  // table, with nothing to tell the form from the list above it.
  //
  // A modal has neither question to get wrong. One thing is on screen, it is the
  // same thing at every width, and on a phone it is the whole screen.
  //
  // The list stays a box of its own: `.pane-list` is what scrolls inside a
  // framed view, and what the data-table's width floor is keyed on.
  import TzFormDialog from "./TzFormDialog.svelte";

  let {
    // Whether the editor is showing. The caller owns the state; this component
    // only decides where the form goes.
    open = false,
    // What the editor's header calls what is being edited.
    title = "",
    // Must actually close — see TzFormDialog's `onclose`, which explains why
    // and what it buys.
    onclose = null,
    // Deleting a record belongs where the record is on screen and named, not on
    // a button repeated once per row in the table. Null while the editor is
    // creating — there is nothing to delete yet.
    ondelete = null,
    deleteLabel = "",
    list,
    inspector = null,
    actions = null,
  } = $props();
</script>

<div class="pane-list">
  {@render list()}
</div>

<TzFormDialog {open} {title} {onclose} {ondelete} {deleteLabel} body={inspector} {actions} />
