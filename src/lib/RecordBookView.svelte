<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // One record book (cuaderno): a holding's campaign, opened from the record
  // book list, worked through one tab per register of the official model. This
  // component owns only what every tab shares — the campaign itself and the
  // catalogue data the forms reference; each register lives in its own Book*
  // child (the RegistryView pattern).
  //
  // It used to pick its farm and campaign from two selectors at the top. A
  // season now belongs to one farm, so a book is a season and the page is
  // opened on one: the farm comes from the season, and the way to another book
  // is back to the list.
  import { locale, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { deleteConfirm } from "./bookRemoval.js";
  import { loadLookups } from "./lookups.svelte.js";
  import { notify, run } from "./notifications.svelte.js";
  import { setPageTitle } from "./pageTitle.svelte.js";
  import BookCrops from "./BookCrops.svelte";
  import BookMerge from "./BookMerge.svelte";
  import DuplicateSuspects from "./DuplicateSuspects.svelte";
  import RemovedWithBook from "./RemovedWithBook.svelte";
  import BookExport from "./BookExport.svelte";
  import BookEcoschemes from "./BookEcoschemes.svelte";
  import BookSowing from "./BookSowing.svelte";
  import BookHarvest from "./BookHarvest.svelte";
  import BookFertilisation from "./BookFertilisation.svelte";
  import BookIrrigation from "./BookIrrigation.svelte";
  import BookOtherTreatments from "./BookOtherTreatments.svelte";
  import BookTreatments from "./BookTreatments.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzTabs from "./TzTabs.svelte";
  import TzFormDialog from "./TzFormDialog.svelte";
  import SeasonForm from "./SeasonForm.svelte";
  import { emptySeasonDraft, seasonDraftFrom, seasonPayload } from "./seasonDraft.js";

  let { seasonId } = $props();

  let loading = $state(true);

  /// The book: its season, and the holding that season belongs to.
  let season = $state(null);
  let farm = $state(null);
  const farmId = $derived(season?.farm_id ?? "");
  const countryCode = $derived(farm?.country_code);

  // Farm-independent data.
  let operators = $state([]);
  let advisors = $state([]);

  // Farm-scoped data (plots, machines, premises, products authorised in its
  // country).
  let plots = $state([]);
  let machinery = $state([]);
  let premises = $state([]);
  let products = $state([]);

  // Season-scoped data: the record book itself.
  let crops = $state([]);
  let treatments = $state([]);

  // Which register is open. Component-local on purpose: nothing links into a
  // book tab, so there is nothing for the hash to carry.
  // Model order, so the tabs read like the printed book: 2.1, 3.1, 3.2-3.5,
  // 4 and 5, then the second decree's two registers (6 and 8), then the third
  // decree's section 9, then the export.
  const TABS = [
    "crops",
    "treatments",
    "other",
    "harvest",
    "fertilisation",
    "irrigation",
    "ecoschemes",
    "export",
  ];
  let tab = $state("crops");
  const tabItems = $derived(TABS.map((name) => ({ value: name, label: t(`book.tab_${name}`) })));

  run(async () => {
    const opened = await invoke("get_season", { seasonId });
    const [detail, operatorList, advisorList] = await Promise.all([
      invoke("get_farm", { farmId: opened.farm_id }),
      invoke("list_operators"),
      invoke("list_advisors"),
      // The session-wide reference lists come from lib/lookups.svelte.js, which
      // fetches them once for the whole app; the children read them from there
      // rather than being handed twenty props.
      loadLookups(),
    ]);
    [operators, advisors] = [operatorList, advisorList];
    farm = detail.farm;
    season = opened;
    nameThePage();
    await loadFarmScope();
    await loadBook();
  }).finally(() => (loading = false));

  /// The shell's band names the book by its holding and campaign; the section
  /// name stays in the sidebar.
  function nameThePage() {
    setPageTitle(`${farm.name} · ${season.label}`);
  }

  async function loadFarmScope() {
    [plots, machinery, premises, products] = await Promise.all([
      invoke("list_plots", { farmId }),
      invoke("list_machinery", { farmId }),
      invoke("list_premises", { farmId }),
      invoke("list_products", { countryCode }),
    ]);
    await loadReportLanguages();
  }

  async function loadBook() {
    [crops, treatments] = await Promise.all([
      invoke("list_crops", { seasonId, farmId }),
      invoke("list_treatment_records", { seasonId, farmId }),
    ]);
  }

  // Bumped when a record changed underneath the open register — a decision
  // about a possible duplicate removed one copy or restored it — so the tab is
  // remounted and reads its register afresh. Each register loads its own list
  // on mount, so this is the one reload that reaches all of them.
  let revision = $state(0);

  function bookChanged() {
    revision += 1;
    run(loadBook);
  }

  /// Records came back into this book: its registers list them again, and
  /// they may pair with what is there.
  function recordsBack() {
    bookChanged();
    run(() => duplicates?.reload());
  }

  // --- the campaign ------------------------------------------------------------

  let seasonFormOpen = $state(false);
  let seasonDraft = $state(emptySeasonDraft());

  function showSeasonForm() {
    seasonDraft = seasonDraftFrom(season);
    seasonFormOpen = true;
  }

  async function submitSeason() {
    season = await invoke("update_season", { seasonId, update: seasonPayload(seasonDraft) });
    seasonFormOpen = false;
    nameThePage();
  }

  /// The book goes with everything in it (docs/sync.md → Deleting a book with
  /// its records), so the confirmation says how many records go and until when
  /// the book can be brought back — and where from: the foot of the list this
  /// page returns to. Beside Edit in the band rather than inside its panel:
  /// this page IS the book, as the farm's page is the farm, and both put what
  /// can be done to the record in the band that names it.
  /// Whether the deletion is under way: it takes every record of the book in
  /// one write, seconds on a large one, and a second press must not follow.
  let deleting = $state(false);

  function deleteBook() {
    run(async () => {
      const preview = await invoke("book_deletion_preview", { seasonId });
      if (!(await confirmDialog(deleteConfirm(season, farm.name, preview)))) return;
      deleting = true;
      try {
        await invoke("delete_season", { seasonId });
      } finally {
        deleting = false;
      }
      notify(t("season.deleted", { label: season.label }));
      location.hash = "#/record-book";
    });
  }

  // The page's duplicates line sits outside the remounted tabs, so a change
  // that can make pairs is told to it directly.
  let duplicates = $state(null);

  /// This book and another are one now. When this one went, its records are
  /// in the other, and that is the page to be on; when this one stayed, it
  /// holds the other's records too — and records typed in both books are pairs
  /// of ONE book now, so the duplicates line is read again.
  function merged(kept, absorbedId) {
    if (absorbedId === seasonId) {
      location.hash = `#/record-book/${kept.id}`;
      return;
    }
    bookChanged();
    run(() => duplicates?.reload());
  }

  // --- the book's language --------------------------------------------------
  // The layout is the Spanish official model whatever happens; the language is
  // the holding's to choose among the ones official where it sits. The backend
  // decides which those are (and which to preselect, given the UI language) —
  // provinces and statutes are not frontend knowledge.
  let reportLanguages = $state([]);
  let defaultLanguage = $state("es");

  async function loadReportLanguages() {
    const info = await invoke("report_languages", { farmId, uiLocale: locale() });
    reportLanguages = info.languages;
    defaultLanguage = info.default;
  }
</script>

<section class="view framed">
  {#if loading}
    <Skeleton />
  {:else if !season}
    <p class="table-empty">
      {t("record_book.not_found")} <a href="#/record-book">{t("nav.record_book")}</a>
    </p>
  {:else}
    <!-- What can be done to the book, in a fixed band rather than a block that
         scrolls with the register. Which book this is — the holding and the
         campaign — is the shell band's title above, so this band labels the
         section like every other band does. -->
    <div class="view-head">
      <h2>{t("record_book.section")}</h2>
      <div class="head-actions">
        <button type="button" onclick={showSeasonForm}>{t("form.edit")}</button>
        <!-- Drawn only while another book of the farm shares days with this
             one: a twin, not last year's book. -->
        <BookMerge {season} onmerged={merged} />
        <button type="button" class="btn-danger" disabled={deleting} onclick={deleteBook}>
          {t("record_book.delete")}
        </button>
      </div>
    </div>

    <TzFormDialog
      open={seasonFormOpen}
      title={season.label}
      onclose={() => (seasonFormOpen = false)}
      body={seasonFields}
      actions={seasonActions}
    />

    {#snippet seasonFields(formId)}
      <SeasonForm bind:draft={seasonDraft} onsubmit={submitSeason} {formId} />
    {/snippet}

    {#snippet seasonActions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (seasonFormOpen = false)}>
          {t("form.cancel")}
        </button>
      </div>
    {/snippet}

    <!-- Records deleted with this book that it came back without — renamed
         elsewhere while it was deleted, or opened again: one line when there
         are any, nothing otherwise. -->
    <RemovedWithBook {seasonId} onchanged={recordsBack} />

    <!-- The book's own possible duplicates, whatever its age: one line when
         there are any, nothing when there are none. -->
    <DuplicateSuspects bind:this={duplicates} {seasonId} onchanged={bookChanged} />

    {#if farmId && seasonId}
      <TzTabs items={tabItems} bind:value={tab} framed>
        {#snippet panel()}
          <!-- Remounted when the book changes: every register's open form and
               draft state belongs to one book, never to the next. And when a
               decision about a pair removed or restored a record, since the
               open register is listing it. -->
          {#key `${seasonId}|${revision}`}
            {#if tab === "crops"}
              <!-- Stacked rather than given a tab each: a sowing is how the
                   crop above it began, and the two read together. A stack
                   scrolls as one column inside the frame (`.register-stack`)
                   instead of halving the pane. -->
              <div class="register-stack">
                <BookCrops
                  {farmId}
                  {seasonId}
                  seasonLabel={season.label}
                  {countryCode}
                  {plots}
                  {crops}
                  onChanged={loadBook}
                />
                <BookSowing {farmId} {seasonId} {plots} {crops} />
              </div>
            {:else if tab === "treatments"}
              <BookTreatments
                {farmId}
                {countryCode}
                {seasonId}
                {plots}
                {crops}
                {operators}
                {machinery}
                {products}
                {advisors}
                {treatments}
                onChanged={loadBook}
              />
            {:else if tab === "other"}
              <BookOtherTreatments
                {farmId}
                {seasonId}
                {countryCode}
                {plots}
                {products}
                {operators}
                {machinery}
                {premises}
                {advisors}
              />
            {:else if tab === "harvest"}
              <!-- Two registers a farmer files together, and section 6 with the
                   7.1 plan its doses are measured against: stacks, for the same
                   reason the crops tab is one. -->
              <div class="register-stack">
                <BookHarvest {farmId} {seasonId} {countryCode} {plots} {crops} />
              </div>
            {:else if tab === "fertilisation"}
              <div class="register-stack">
                <BookFertilisation {farmId} {seasonId} {countryCode} {plots} {crops} {machinery} />
              </div>
            {:else if tab === "irrigation"}
              <BookIrrigation {farmId} {seasonId} {countryCode} {plots} {crops} />
            {:else if tab === "ecoschemes"}
              <BookEcoschemes {farmId} {seasonId} {countryCode} {plots} />
            {:else}
              <!-- Not a register but a page of actions over an advisory, so it
                   scrolls as a column too rather than trying to fill a pane. -->
              <div class="register-stack">
                <BookExport
                  {farmId}
                  {seasonId}
                  seasonLabel={season.label}
                  {reportLanguages}
                  {defaultLanguage}
                />
              </div>
            {/if}
          {/key}
        {/snippet}
      </TzTabs>
    {/if}
  {/if}
</section>
