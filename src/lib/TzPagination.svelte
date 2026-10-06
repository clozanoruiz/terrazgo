<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // An owned page control, over bits-ui's Pagination: previous, the pages with
  // ellipses, next, and which rows of how many are on screen.
  //
  // Owned for the reasons every control is (docs/frontend-conventions.md →
  // Owned controls): a view never imports bits-ui, and the words are ours.
  // bits-ui names each page button "Page 3" in English whatever the app's
  // language, so every button is drawn through the `child` snippet and states
  // its own label after the spread, where it wins.
  //
  // Safe under the production CSP with nothing to state: Pagination renders
  // plain buttons, with no floating layer and so no body scroll lock.
  //
  // It replaces rows rather than appending them, which is the point of using it
  // instead of a "show more" button: a list that appends grows back into the
  // thousands of rows paging exists to avoid.
  import { Pagination } from "bits-ui";
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import { formatNumber, t } from "../i18n.js";

  let {
    /// How many rows there are in all.
    count = 0,
    /// Rows per page.
    perPage = 100,
    /// The current page, 1-based.
    page = $bindable(1),
    /// Called with the new page after the reader picks one.
    onchange = null,
  } = $props();
</script>

<!-- Nothing to page through, nothing drawn: a list that fits one page reads
     as a plain table. -->
{#if count > perPage}
  <nav class="tz-pagination" aria-label={t("pagination.aria")}>
    <Pagination.Root {count} {perPage} bind:page onPageChange={onchange}>
      {#snippet child({ props, pages, range })}
        <div {...props} class="tz-pagination-root">
          <span class="tz-pagination-range">
            {t("pagination.range", {
              from: formatNumber(range.start),
              to: formatNumber(range.end),
              total: formatNumber(count),
            })}
          </span>
          <div class="tz-pagination-pages">
            <Pagination.PrevButton>
              {#snippet child({ props })}
                <button {...props} class="tz-pagination-step" aria-label={t("pagination.previous")}>
                  <ChevronLeft />
                </button>
              {/snippet}
            </Pagination.PrevButton>
            {#each pages as item (item.key)}
              {#if item.type === "ellipsis"}
                <span class="tz-pagination-gap" aria-hidden="true">…</span>
              {:else}
                <Pagination.Page page={item}>
                  {#snippet child({ props })}
                    <button
                      {...props}
                      class="tz-pagination-page"
                      aria-label={t("pagination.page", { page: formatNumber(item.value) })}
                      aria-current={item.value === page ? "page" : undefined}
                    >
                      {formatNumber(item.value)}
                    </button>
                  {/snippet}
                </Pagination.Page>
              {/if}
            {/each}
            <Pagination.NextButton>
              {#snippet child({ props })}
                <button {...props} class="tz-pagination-step" aria-label={t("pagination.next")}>
                  <ChevronRight />
                </button>
              {/snippet}
            </Pagination.NextButton>
          </div>
        </div>
      {/snippet}
    </Pagination.Root>
  </nav>
{/if}
