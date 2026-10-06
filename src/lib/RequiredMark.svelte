<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The mark beside a required field's label, and nothing else — one component
  // so the glyph, its colour and its accessibility are stated once rather than
  // in each of the six controls that render a label.
  //
  // Until 2026-09-15 nothing on screen said which fields were required. The
  // controls carry `required`, but it only drives setCustomValidity, so the
  // answer arrived on submit and never before it — on a farm form where one
  // field of twenty-six is obligatory, and on a register where a farmer is
  // deciding what to fill in.
  //
  // `aria-hidden` because the SEMANTICS are carried by `aria-required` on the
  // control itself, not by this glyph: a screen reader that read both would say
  // "asterisk" after every label and then "required" again. An asterisk alone,
  // unexplained, is the usual way this is got wrong — TzForm renders the legend
  // that explains it, once per form, and only when a form has one.
  //
  // The asterisk marks the MINORITY, which here is the required fields by a
  // wide margin. Were that ever to invert on a given form, the answer is to
  // mark the optional ones on that form, not to change this.
  let { required = false } = $props();
</script>

{#if required}<span class="tz-required" aria-hidden="true">*</span>{/if}
