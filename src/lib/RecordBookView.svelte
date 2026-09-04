<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The record book (cuaderno) shell: pick a farm and a season, then work
  // through the registers the official model is made of, one tab per register.
  // This component owns only what every tab shares — the selectors, the
  // campaign itself and the catalogue data the forms reference; each register
  // lives in its own Book* child (the RegistryView pattern).
  import { locale, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { loadLookups } from "./lookups.svelte.js";
  import { run } from "./notifications.svelte.js";
  import NumberInput from "./NumberInput.svelte";
  import BookCrops from "./BookCrops.svelte";
  import BookExport from "./BookExport.svelte";
  import BookEcoschemes from "./BookEcoschemes.svelte";
  import BookSowing from "./BookSowing.svelte";
  import BookHarvest from "./BookHarvest.svelte";
  import BookFertilisation from "./BookFertilisation.svelte";
  import BookIrrigation from "./BookIrrigation.svelte";
  import BookOtherTreatments from "./BookOtherTreatments.svelte";
  import BookTreatments from "./BookTreatments.svelte";
  import DateInput from "./DateInput.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzSelect from "./TzSelect.svelte";
  import TzTabs from "./TzTabs.svelte";
  import { nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";

  let loading = $state(true);

  // Farm-independent data, loaded once.
  let farms = $state([]);
  let seasons = $state([]);
  let operators = $state([]);
  // Counts, for model 3.1 bis's "intensidad de la medida" — a third list
  // because a number of traps is neither a rate nor an amount of product.
  let advisors = $state([]);
  // Section 8's own vocabularies (module-fertilisation).

  let farmId = $state("");
  let seasonId = $state("");

  // Farm-scoped data (plots, machines, premises, products authorised in its
  // country).
  let plots = $state([]);
  let machinery = $state([]);
  let premises = $state([]);
  let products = $state([]);

  // (farm, season)-scoped data: the record book itself.
  let crops = $state([]);
  let treatments = $state([]);

  let seasonFormOpen = $state(false);

  // Season form (defaults to the current campaign year).
  // null editingSeasonId = the form creates, an id = it edits.
  let editingSeasonId = $state(null);
  let campaignYear = $state(new Date().getFullYear());
  let seasonLabel = $state(String(new Date().getFullYear()));
  let startsOn = $state("");
  let endsOn = $state("");

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
    [farms, seasons, operators, advisors] = await Promise.all([
      invoke("list_farms"),
      invoke("list_seasons"),
      invoke("list_operators"),
      invoke("list_advisors"),
    ]);
    // The session-wide reference lists come from lib/lookups.svelte.js, which
    // fetches them once for the whole app; the children read them from there
    // rather than being handed twenty props.
    await loadLookups();
    // Preselect the first farm and the newest season — the everyday case is
    // one farm, current campaign.
    if (farms.length > 0) farmId = farms[0].id;
    if (seasons.length > 0) seasonId = seasons[0].id;
    await loadFarmScope();
    await loadBook();
  }).finally(() => (loading = false));

  async function loadFarmScope() {
    if (!farmId) {
      [plots, machinery, premises, products, reportLanguages] = [[], [], [], [], []];
      return;
    }
    [plots, machinery, premises, products] = await Promise.all([
      invoke("list_plots", { farmId }),
      invoke("list_machinery", { farmId }),
      invoke("list_premises", { farmId }),
      invoke("list_products", { countryCode }),
    ]);
    await loadReportLanguages();
  }

  async function loadBook() {
    if (!farmId || !seasonId) {
      [crops, treatments] = [[], []];
      return;
    }
    [crops, treatments] = await Promise.all([
      invoke("list_crops", { seasonId, farmId }),
      invoke("list_treatment_records", { seasonId, farmId }),
    ]);
  }

  function selectFarm() {
    run(async () => {
      await loadFarmScope();
      await loadBook();
    });
  }

  function selectSeason() {
    run(loadBook);
  }

  // --- seasons ---------------------------------------------------------------

  function showSeasonForm(season = null) {
    editingSeasonId = season?.id ?? null;
    campaignYear = season?.campaign_year ?? new Date().getFullYear();
    seasonLabel = season?.label ?? String(new Date().getFullYear());
    startsOn = season?.starts_on ?? "";
    endsOn = season?.ends_on ?? "";
    seasonFormOpen = true;
  }

  function hideSeasonForm() {
    seasonFormOpen = false;
    editingSeasonId = null;
  }

  async function submitSeason() {
    const payload = {
      campaign_year: Number(campaignYear),
      label: seasonLabel.trim(),
      starts_on: startsOn || null,
      ends_on: endsOn || null,
    };
    if (editingSeasonId) {
      await invoke("update_season", { seasonId: editingSeasonId, update: payload });
    } else {
      const saved = await invoke("create_season", { season: payload });
      seasonId = saved.id;
    }
    hideSeasonForm();
    seasons = await invoke("list_seasons");
    await loadBook();
  }

  /// Only an empty season can go; the backend answers `season_in_use` otherwise,
  /// and the notification bell renders that as a plain explanation.
  function deleteSeason() {
    const season = seasons.find((s) => s.id === seasonId);
    if (!season) return;
    run(async () => {
      if (!(await confirmDialog(t("season.delete_confirm", { label: season.label })))) return;
      await invoke("delete_season", { seasonId: season.id });
      hideSeasonForm();
      seasons = await invoke("list_seasons");
      seasonId = seasons[0]?.id ?? "";
      await loadBook();
    });
  }

  // --- the book's language --------------------------------------------------
  // The layout is the Spanish official model whatever happens; the language is
  // the holding's to choose among the ones official where it sits. The backend
  // decides which those are (and which to preselect, given the UI language) —
  // provinces and statutes are not frontend knowledge.
  let reportLanguages = $state([]);
  let defaultLanguage = $state("es");

  async function loadReportLanguages() {
    if (!farmId) return;
    const info = await invoke("report_languages", { farmId, uiLocale: locale() });
    reportLanguages = info.languages;
    defaultLanguage = info.default;
  }

  const countryCode = $derived(farms.find((f) => f.id === farmId)?.country_code);
  const currentSeasonLabel = $derived(seasons.find((s) => s.id === seasonId)?.label ?? "");
</script>

<section class="view framed">
  {#if loading}
    <Skeleton />
  {:else if farms.length === 0}
    <p>{t("treatments.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
  {:else}
    <!-- The book's chrome: which holding, which campaign, and what can be done
         to the campaign. A fixed band rather than a block that scrolls with the
         register, because the answer to "which book am I writing in" must not
         be something you scroll back up to check. -->
    <div class="view-head">
      <div class="form-grid">
        <TzSelect
          label={t("treatments.farm")}
          items={nameItems(farms)}
          bind:value={farmId}
          onchange={selectFarm}
        />
        <!-- Not nameItems: campaigns arrive in the backend's own order, which is
             chronological and is the order a farmer expects. -->
        <TzSelect
          label={t("treatments.season")}
          items={seasons.map((season) => ({ value: season.id, label: season.label }))}
          disabled={seasons.length === 0}
          bind:value={seasonId}
          onchange={selectSeason}
        />
      </div>
      <div class="selector-buttons">
        <button type="button" onclick={() => showSeasonForm()}>{t("seasons.new")}</button>
        {#if seasonId}
          <button
            type="button"
            onclick={() => showSeasonForm(seasons.find((s) => s.id === seasonId))}
          >
            {t("form.edit")}
          </button>
          <button type="button" class="btn-danger" onclick={deleteSeason}>
            {t("form.delete")}
          </button>
        {/if}
      </div>
    </div>

    {#if seasonFormOpen || seasons.length === 0}
      <!-- A block between two fixed bands, so it states its own height rather
           than being squeezed by the register below it. -->
      <div class="season-form">
        {#if seasons.length === 0}
          <p>{t("seasons.empty")}</p>
        {/if}
        <TzForm onsubmit={submitSeason}>
          <div class="form-grid">
            <NumberInput
              label={t("season.campaign_year")}
              min={2000}
              max={2100}
              required
              bind:value={campaignYear}
            />
            <TextInput label={t("season.label")} required bind:value={seasonLabel} />
            <DateInput label={t("season.starts")} bind:value={startsOn} />
            <DateInput label={t("season.ends")} bind:value={endsOn} />
          </div>
          <div class="form-actions">
            <button type="submit">{t("form.save")}</button>
            {#if seasons.length > 0}
              <button type="button" class="btn-cancel" onclick={hideSeasonForm}>
                {t("form.cancel")}
              </button>
            {/if}
          </div>
        </TzForm>
      </div>
    {/if}

    {#if farmId && seasonId}
      <TzTabs items={tabItems} bind:value={tab} framed>
        {#snippet panel()}
          <!-- Remounted when the book changes: every register's open form and
               draft state belongs to one (farm, season), never to the next. -->
          {#key `${farmId}:${seasonId}`}
            {#if tab === "crops"}
              <!-- Stacked rather than given a tab each: a sowing is how the
                   crop above it began, and the two read together. A stack
                   scrolls as one column inside the frame (`.register-stack`)
                   instead of halving the pane. -->
              <div class="register-stack">
                <BookCrops
                  {farmId}
                  {seasonId}
                  seasonLabel={currentSeasonLabel}
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
              <BookIrrigation {farmId} {seasonId} {plots} {crops} />
            {:else if tab === "ecoschemes"}
              <BookEcoschemes {farmId} {seasonId} {countryCode} {plots} />
            {:else}
              <!-- Not a register but a page of actions over an advisory, so it
                   scrolls as a column too rather than trying to fill a pane. -->
              <div class="register-stack">
                <BookExport
                  {farmId}
                  {seasonId}
                  seasonLabel={currentSeasonLabel}
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

<style>
  /* Between two fixed bands of the frame, so it states its own height instead
     of being squeezed by the register below it. The inset is the frame's only:
     below the breakpoint .view already supplies one and a second would double
     it. */
  .season-form {
    flex: none;
  }

  @media (min-width: 701px) {
    .season-form {
      padding-inline: var(--space-3);
    }
  }
</style>
