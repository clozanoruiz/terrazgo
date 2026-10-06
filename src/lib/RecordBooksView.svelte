<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The record books: one row per holding's campaign, and a way to start a new
  // one. A row opens the book on its own page — the RecordBookView that used to
  // be this route, and picked its farm and campaign from two selectors instead.
  //
  // A book is a season row: a season belongs to one farm (docs/data-model.md),
  // so the list is the seasons, named by their farm.
  import { t, formatDate } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import { compareText } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TzWorkspace from "./TzWorkspace.svelte";
  import SeasonForm from "./SeasonForm.svelte";
  import TzPagination from "./TzPagination.svelte";
  import RemovedBooks from "./RemovedBooks.svelte";
  import { emptySeasonDraft, seasonPayload } from "./seasonDraft.js";

  /// Books per page. The list is the one that grows with farms × years, and
  /// the rows rendered are its cost: measured 2026-09-16 under a 6× CPU
  /// throttle, 100 books painted in ~55 ms and 4 000 in ~3 s. A holding with a
  /// few farms never reaches a second page, so it still sees every book.
  const PAGE_SIZE = 100;

  let farms = $state([]);
  let seasons = $state([]);
  let total = $state(0);
  let page = $state(1);
  let loading = $state(true);
  let creating = $state(false);
  let seasonDraft = $state(emptySeasonDraft());

  run(async () => {
    [farms] = await Promise.all([invoke("list_farms"), loadPage()]);
  }).finally(() => (loading = false));

  async function loadPage() {
    const result = await invoke("list_seasons", {
      limit: PAGE_SIZE,
      offset: (page - 1) * PAGE_SIZE,
    });
    seasons = result.seasons;
    total = result.total;
  }

  const farmNames = $derived(new Map(farms.map((farm) => [farm.id, farm.name])));

  // The latest ending first, as the backend sends them, and then by farm name —
  // re-collated within the page, because SQL's BINARY order files
  // an accented name last (src/lib/collate.js). A page boundary still falls
  // where SQL put it; one page can be collated, a set of pages cannot.
  const books = $derived(
    seasons
      .map((season) => ({ season, farmName: farmNames.get(season.farm_id) ?? "" }))
      .sort(
        (a, b) =>
          b.season.ends_on.localeCompare(a.season.ends_on) || compareText(a.farmName, b.farmName),
      ),
  );

  function startCreate() {
    // The first farm preselected: one holding is the everyday case.
    seasonDraft = emptySeasonDraft(farms[0]?.id ?? "");
    creating = true;
  }

  /// A new book opens straight away: creating one is how a farmer starts
  /// writing in it.
  async function submit() {
    const saved = await invoke("create_season", {
      season: seasonPayload(seasonDraft, { create: true }),
    });
    creating = false;
    location.hash = `#/record-book/${saved.id}`;
  }

  function open(season) {
    location.hash = `#/record-book/${season.id}`;
  }
</script>

<section class="view framed">
  <div class="view-head">
    <h2>{t("record_books.title")}</h2>
    <button type="button" disabled={loading || farms.length === 0} onclick={startCreate}>
      {t("record_books.new")}
    </button>
  </div>

  <TzWorkspace open={creating} title={t("record_books.new")} onclose={() => (creating = false)}>
    {#snippet list()}
      {#if loading}
        <Skeleton />
      {:else if farms.length === 0}
        <p class="table-empty">
          {t("treatments.no_farms")} <a href="#/farms">{t("nav.farms")}</a>
        </p>
      {:else if total === 0}
        <p class="table-empty">{t("record_books.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"record-books"}>
            <thead>
              <tr>
                <th>{t("column.campaign")}</th>
                <th>{t("column.farm")}</th>
                <th>{t("column.starts")}</th>
                <th>{t("column.ends")}</th>
              </tr>
            </thead>
            <tbody>
              {#each books as { season, farmName } (season.id)}
                <tr onclick={(e) => opensRow(e) && open(season)}>
                  <td class="col-name">
                    <button type="button" class="row-open" onclick={() => open(season)}>
                      {season.label}
                    </button>
                  </td>
                  <td>{farmName}</td>
                  <td class="col-muted">{season.starts_on ? formatDate(season.starts_on) : ""}</td>
                  <td class="col-muted">{season.ends_on ? formatDate(season.ends_on) : ""}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <TzPagination count={total} perPage={PAGE_SIZE} bind:page onchange={() => run(loadPage)} />
      {/if}
      {#if !loading}
        <!-- Books deleted lately, which can still come back: a closed fold,
             and nothing at all when none was. -->
        <RemovedBooks onrestored={() => run(loadPage)} />
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      <SeasonForm bind:draft={seasonDraft} {farms} creating onsubmit={submit} {formId} />
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (creating = false)}>
          {t("form.cancel")}
        </button>
      </div>
    {/snippet}
  </TzWorkspace>
</section>
