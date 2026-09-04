<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, eco-schemes tab: section 9's registers (RD 1048/2022).
  //
  // A tab of its own inside the book's tab strip, because section 9 is ONE
  // section of the printed model with several sub-tables — 9.1 pastoreo, 9.2
  // siega, 9.4/9.5 cubiertas — and the book's top-level strip already carries
  // eight destinations. Nesting them here keeps that strip at eight and mirrors
  // how the printed book is numbered.
  //
  // Which registers a holding needs depends on which ecorrégimen it claimed in
  // the solicitud única, which the app cannot see by any route. So every
  // sub-tab is always offered and nothing nags a holding that records nothing:
  // an empty section 9 is the normal state of most farms.
  import { t } from "../i18n.js";
  import BookGrazing from "./BookGrazing.svelte";
  import BookCulturalOperations from "./BookCulturalOperations.svelte";
  import BookSoilCovers from "./BookSoilCovers.svelte";
  import TzTabs from "./TzTabs.svelte";

  let { farmId, seasonId, countryCode, plots } = $props();

  // Model order, so the sub-tabs read like the printed section.
  const REGISTERS = ["grazing", "operations", "covers"];
  let register = $state("grazing");
  const tabItems = $derived(
    REGISTERS.map((name) => ({ value: name, label: t(`ecoscheme.tab_${name}`) })),
  );
</script>

<!-- The shared strip, not a hand-rolled one: this was the last `role="tab"`
     written by hand in the app, and it had the same partial ARIA contract
     TzTabs exists to close — a tab list a screen reader is told about and then
     given no way to move through. -->
<TzTabs items={tabItems} bind:value={register} nested framed>
  {#snippet panel(item)}
    {#if item.value === "grazing"}
      <BookGrazing {farmId} {seasonId} {countryCode} {plots} />
    {:else if item.value === "operations"}
      <BookCulturalOperations {farmId} {seasonId} {countryCode} {plots} />
    {:else}
      <BookSoilCovers {farmId} {seasonId} {countryCode} {plots} />
    {/if}
  {/snippet}
</TzTabs>
