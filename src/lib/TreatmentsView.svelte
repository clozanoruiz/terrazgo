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
  import { notify, run } from "./notifications.svelte.js";
  import BookCrops from "./BookCrops.svelte";
  import BookExport from "./BookExport.svelte";
  import BookHarvest from "./BookHarvest.svelte";
  import BookFertilisation from "./BookFertilisation.svelte";
  import BookIrrigation from "./BookIrrigation.svelte";
  import BookOtherTreatments from "./BookOtherTreatments.svelte";
  import BookTreatments from "./BookTreatments.svelte";
  import DateInput from "./DateInput.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { nameItems } from "./selectItems.js";

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

  // Farm-scoped data (plots, machines, products authorised in its country).
  let plots = $state([]);
  let machinery = $state([]);
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
  // 4 and 5, then the second decree's two registers (6 and 8), then the export.
  const TABS = ["crops", "treatments", "other", "harvest", "fertilisation", "irrigation", "export"];
  let tab = $state("crops");

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
      [plots, machinery, products, reportLanguages] = [[], [], [], []];
      return;
    }
    [plots, machinery, products] = await Promise.all([
      invoke("list_plots", { farmId }),
      invoke("list_machinery", { farmId }),
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

  function submitSeason(event) {
    event.preventDefault();
    const payload = {
      campaign_year: Number(campaignYear),
      label: seasonLabel.trim(),
      starts_on: startsOn || null,
      ends_on: endsOn || null,
    };
    run(async () => {
      if (editingSeasonId) {
        await invoke("update_season", { seasonId: editingSeasonId, update: payload });
      } else {
        const saved = await invoke("create_season", { season: payload });
        seasonId = saved.id;
      }
      notify(t("message.season_saved", { label: payload.label }));
      hideSeasonForm();
      seasons = await invoke("list_seasons");
      await loadBook();
    });
  }

  /// Only an empty season can go; the backend answers `season_in_use` otherwise,
  /// and the notification bell renders that as a plain explanation.
  function deleteSeason() {
    const season = seasons.find((s) => s.id === seasonId);
    if (!season) return;
    run(async () => {
      if (!(await confirmDialog(t("season.delete_confirm", { label: season.label })))) return;
      await invoke("delete_season", { seasonId: season.id });
      notify(t("message.season_deleted"));
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

<section class="view">
  <h2>{t("treatments.title")}</h2>

  {#if loading}
    <Skeleton />
  {:else if farms.length === 0}
    <p>{t("treatments.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
  {:else}
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
      <label class="selector-action selector-action-wide">
        <span>&nbsp;</span>
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
      </label>
    </div>

    {#if seasonFormOpen || seasons.length === 0}
      {#if seasons.length === 0}
        <p>{t("seasons.empty")}</p>
      {/if}
      <form onsubmit={submitSeason}>
        <div class="form-grid">
          <label>
            <span>{t("season.campaign_year")}</span>
            <input type="number" min="2000" max="2100" required bind:value={campaignYear} />
          </label>
          <label><span>{t("season.label")}</span><input required bind:value={seasonLabel} /></label>
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
      </form>
    {/if}

    {#if farmId && seasonId}
      <div class="tabstrip" role="tablist">
        {#each TABS as name (name)}
          <button
            type="button"
            role="tab"
            class="tab"
            class:active={tab === name}
            aria-selected={tab === name}
            onclick={() => (tab = name)}
          >
            {t(`book.tab_${name}`)}
          </button>
        {/each}
      </div>

      <!-- Remounted when the book changes: every register's open form and
           draft state belongs to one (farm, season), never to the next one. -->
      {#key `${farmId}:${seasonId}`}
        <div class="tabpanel" role="tabpanel">
          {#if tab === "crops"}
            <BookCrops
              {farmId}
              {seasonId}
              seasonLabel={currentSeasonLabel}
              {plots}
              {crops}
              onChanged={loadBook}
            />
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
              {advisors}
            />
          {:else if tab === "harvest"}
            <BookHarvest {farmId} {seasonId} {countryCode} {plots} {crops} />
          {:else if tab === "fertilisation"}
            <BookFertilisation {farmId} {seasonId} {countryCode} {plots} {crops} {machinery} />
          {:else if tab === "irrigation"}
            <BookIrrigation {farmId} {seasonId} {plots} {crops} />
          {:else}
            <BookExport
              {farmId}
              {seasonId}
              seasonLabel={currentSeasonLabel}
              {reportLanguages}
              {defaultLanguage}
            />
          {/if}
        </div>
      {/key}
    {/if}
  {/if}
</section>
