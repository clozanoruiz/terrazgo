<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The record book (cuaderno) view: pick a farm and a season, then declare
  // crops and enter phytosanitary treatments. The dropdowns read the
  // product/operator/machinery catalogue maintained in RegistryView.
  import { formatDate, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import TreatmentForm from "./TreatmentForm.svelte";

  let loading = $state(true);

  // Farm-independent data, loaded once.
  let farms = $state([]);
  let seasons = $state([]);
  let operators = $state([]);
  let units = $state([]);
  let reasons = $state([]);
  let justifications = $state([]);
  let efficacies = $state([]);
  let productionSystems = $state([]);

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
  let cropFormOpen = $state(false);
  let treatmentFormOpen = $state(false);

  // Season form (defaults to the current campaign year).
  let campaignYear = $state(new Date().getFullYear());
  let seasonLabel = $state(String(new Date().getFullYear()));
  let startsOn = $state("");
  let endsOn = $state("");

  // Crop form.
  let cropPlotId = $state("");
  let species = $state("");
  let variety = $state("");
  let systemCode = $state("");
  let sownOn = $state("");

  run(async () => {
    [farms, seasons, operators, units, reasons, justifications, efficacies, productionSystems] =
      await Promise.all([
        invoke("list_farms"),
        invoke("list_seasons"),
        invoke("list_operators"),
        invoke("list_units"),
        invoke("list_reason_categories"),
        invoke("list_justifications"),
        invoke("list_efficacies"),
        invoke("list_production_systems"),
      ]);
    // Preselect the first farm and the newest season — the everyday case is
    // one farm, current campaign.
    if (farms.length > 0) farmId = farms[0].id;
    if (seasons.length > 0) seasonId = seasons[0].id;
    await loadFarmScope();
    await loadBook();
  }).finally(() => (loading = false));

  async function loadFarmScope() {
    if (!farmId) {
      [plots, machinery, products] = [[], [], []];
      return;
    }
    const countryCode = farms.find((f) => f.id === farmId)?.country_code;
    [plots, machinery, products] = await Promise.all([
      invoke("list_plots", { farmId }),
      invoke("list_machinery", { farmId }),
      invoke("list_products", { countryCode }),
    ]);
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
    await resolveProblemLabels();
  }

  // Stored problems carry catalogue CODES (the regulatory payload); labels are
  // display metadata resolved from the catalogue, one fetch per category used.
  let problemLabels = $state({});

  async function resolveProblemLabels() {
    const countryCode = farms.find((f) => f.id === farmId)?.country_code;
    const categories = new Set(
      treatments.flatMap(({ problems }) => problems.map((p) => p.reason_category_code)),
    );
    const labels = {};
    for (const category of categories) {
      const codes = await invoke("list_problem_codes", {
        countryCode,
        reasonCategoryCode: category,
      });
      for (const code of codes) labels[`${category}:${code.code}`] = code.label;
    }
    problemLabels = labels;
  }

  function problemSummary(problems) {
    return problems
      .map(
        (p) =>
          problemLabels[`${p.reason_category_code}:${p.problem_code}`] ??
          `${tCode("reason_category", p.reason_category_code)} ${p.problem_code}`,
      )
      .join(", ");
  }

  function setEfficacy(record, efficacyCode) {
    run(async () => {
      await invoke("set_treatment_efficacy", {
        treatmentId: record.id,
        efficacyCode: efficacyCode || null,
      });
      await loadBook();
    });
  }

  function selectFarm() {
    cropFormOpen = false;
    treatmentFormOpen = false;
    precheck = null;
    run(async () => {
      await loadFarmScope();
      await loadBook();
    });
  }

  function selectSeason() {
    cropFormOpen = false;
    treatmentFormOpen = false;
    precheck = null;
    run(loadBook);
  }

  function submitSeason(event) {
    event.preventDefault();
    const season = {
      campaign_year: Number(campaignYear),
      label: seasonLabel.trim(),
      starts_on: startsOn || null,
      ends_on: endsOn || null,
    };
    run(async () => {
      const saved = await invoke("create_season", { season });
      notify(t("message.season_saved", { label: saved.label }));
      seasonFormOpen = false;
      seasons = await invoke("list_seasons");
      seasonId = saved.id;
      await loadBook();
    });
  }

  function submitCrop(event) {
    event.preventDefault();
    const crop = {
      plot_id: cropPlotId,
      season_id: seasonId,
      species_name: species.trim(),
      variety: variety.trim() || null,
      production_system_code: systemCode || null,
      sown_on: sownOn || null,
    };
    run(async () => {
      await invoke("create_crop", { crop });
      notify(t("message.crop_saved", { species: crop.species_name }));
      cropFormOpen = false;
      species = "";
      variety = "";
      sownOn = "";
      await loadBook();
    });
  }

  function deleteTreatment(record) {
    run(async () => {
      if (!(await confirmDialog(t("treatment.delete_confirm")))) return;
      await invoke("delete_treatment_record", { treatmentId: record.id });
      notify(t("message.treatment_deleted"));
      await loadBook();
    });
  }

  async function treatmentSaved() {
    treatmentFormOpen = false;
    await loadBook();
  }

  // --- official export (SIEX descriptor JSON) -----------------------------
  // Precheck first: the backend refuses to build while data required by the
  // official format is missing, so the UI shows what to fix instead of a
  // one-shot error. The save dialog only opens when the precheck is clean.
  let precheck = $state(null);

  function precheckIsClean(check) {
    return (
      check.farm_missing_fields.length === 0 &&
      check.records_missing_efficacy.length === 0 &&
      check.records_missing_operator_licence.length === 0 &&
      check.plots_missing_crop.length === 0
    );
  }

  function exportCuaderno() {
    run(async () => {
      const check = await invoke("export_cuaderno_precheck", { seasonId, farmId });
      precheck = check;
      if (!precheckIsClean(check)) return; // the fix-it list below is the feedback
      // Season labels can carry path-hostile characters ("2025/2026").
      const label = (seasons.find((s) => s.id === seasonId)?.label ?? seasonId).replace(
        /[^\p{L}\p{N}._-]+/gu,
        "-",
      );
      const path = await invoke("plugin:dialog|save", {
        options: {
          defaultPath: `cuaderno-siex-${label}.json`,
          filters: [{ name: "JSON", extensions: ["json"] }],
        },
      });
      if (!path) return;
      const summary = await invoke("export_cuaderno", { seasonId, farmId, destPath: path });
      precheck = null;
      notify(t("message.cuaderno_exported", { path: summary.path, entries: summary.entries }));
    });
  }

  // --- printable cuaderno (PDF) --------------------------------------------
  // No precheck: the printed record book shows the current state, and fields
  // the official model asks for but the data lacks print blank — so the only
  // step is choosing where to save.
  function exportCuadernoPdf() {
    run(async () => {
      const label = (seasons.find((s) => s.id === seasonId)?.label ?? seasonId).replace(
        /[^\p{L}\p{N}._-]+/gu,
        "-",
      );
      const path = await invoke("plugin:dialog|save", {
        options: {
          defaultPath: `cuaderno-${label}.pdf`,
          filters: [{ name: "PDF", extensions: ["pdf"] }],
        },
      });
      if (!path) return;
      const summary = await invoke("export_cuaderno_pdf", { seasonId, farmId, destPath: path });
      notify(t("message.cuaderno_pdf_exported", { path: summary.path, pages: summary.pages }));
    });
  }

  function recordLabel(ref) {
    return `${formatDate(ref.application_date)} — ${ref.product_name}`;
  }

  function plotName(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? plotId;
  }

  function treatedPlotsSummary(treatedPlots) {
    return treatedPlots
      .map((tp) => `${plotName(tp.plot_id)} (${tp.surface_treated_ha} ha)`)
      .join(", ");
  }

  function cropLabel(crop) {
    return crop.variety ? `${crop.species_name} — ${crop.variety}` : crop.species_name;
  }

  function cropDetail(crop) {
    return [
      plotName(crop.plot_id),
      crop.production_system_code ? tCode("production_system", crop.production_system_code) : null,
      crop.sown_on ? t("crop.sown_detail", { date: formatDate(crop.sown_on) }) : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }

  // Entering a treatment needs a product and an operator to reference; the
  // hint sends the user to the catalogue view to create them.
  const missingRefs = $derived(products.length === 0 || operators.length === 0);
</script>

<section class="view">
  <h2>{t("treatments.title")}</h2>

  {#if loading}
    <Skeleton />
  {:else if farms.length === 0}
    <p>{t("treatments.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
  {:else}
    <div class="form-grid">
      <label>
        <span>{t("treatments.farm")}</span>
        <select bind:value={farmId} onchange={selectFarm}>
          {#each farms as farm (farm.id)}
            <option value={farm.id}>{farm.name}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>{t("treatments.season")}</span>
        <select bind:value={seasonId} onchange={selectSeason} disabled={seasons.length === 0}>
          {#each seasons as season (season.id)}
            <option value={season.id}>{season.label}</option>
          {/each}
        </select>
      </label>
      <label class="selector-action">
        <span>&nbsp;</span>
        <button type="button" onclick={() => (seasonFormOpen = !seasonFormOpen)}>
          {t("seasons.new")}
        </button>
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
          <label>
            <span>{t("season.starts")}</span>
            <input type="date" bind:value={startsOn} />
          </label>
          <label>
            <span>{t("season.ends")}</span>
            <input type="date" bind:value={endsOn} />
          </label>
        </div>
        <div class="form-actions">
          <button type="submit">{t("form.save")}</button>
          {#if seasons.length > 0}
            <button type="button" class="btn-cancel" onclick={() => (seasonFormOpen = false)}>
              {t("form.cancel")}
            </button>
          {/if}
        </div>
      </form>
    {/if}

    {#if farmId && seasonId}
      <div class="view-head">
        <h3>{t("crops.title")}</h3>
        <button
          type="button"
          onclick={() => (cropFormOpen = !cropFormOpen)}
          disabled={plots.length === 0}
        >
          {t("crops.new")}
        </button>
      </div>
      {#if plots.length === 0}
        <p>{t("treatments.no_plots")}</p>
      {/if}

      {#if cropFormOpen}
        <form onsubmit={submitCrop}>
          <div class="form-grid">
            <label>
              <span>{t("crop.plot")}</span>
              <select required bind:value={cropPlotId}>
                <option value="" disabled hidden></option>
                {#each plots as { plot } (plot.id)}
                  <option value={plot.id}>{plot.name}</option>
                {/each}
              </select>
            </label>
            <label><span>{t("crop.species")}</span><input required bind:value={species} /></label>
            <label><span>{t("crop.variety")}</span><input bind:value={variety} /></label>
            <label>
              <span>{t("crop.production_system")}</span>
              <select bind:value={systemCode}>
                <option value="">—</option>
                {#each productionSystems as system (system.code)}
                  <option value={system.code}>{tCode("production_system", system.code)}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>{t("crop.sown_on")}</span>
              <input type="date" bind:value={sownOn} />
            </label>
          </div>
          <div class="form-actions">
            <button type="submit">{t("form.save")}</button>
            <button type="button" class="btn-cancel" onclick={() => (cropFormOpen = false)}>
              {t("form.cancel")}
            </button>
          </div>
        </form>
      {/if}

      <ul class="card-list">
        {#each crops as crop (crop.id)}
          <li class="card">
            <strong>{cropLabel(crop)}</strong>
            <span class="detail">{cropDetail(crop)}</span>
          </li>
        {/each}
      </ul>
      {#if crops.length === 0}
        <p>{t("crops.empty")}</p>
      {/if}

      <div class="view-head">
        <h3>{t("treatments.records_title")}</h3>
        <button
          type="button"
          onclick={() => (treatmentFormOpen = !treatmentFormOpen)}
          disabled={missingRefs || plots.length === 0}
        >
          {t("treatments.new")}
        </button>
      </div>
      {#if missingRefs}
        <p>{t("treatments.missing_refs")} <a href="#/registry">{t("nav.registry")}</a></p>
      {/if}

      {#if treatmentFormOpen}
        <TreatmentForm
          {farmId}
          countryCode={farms.find((f) => f.id === farmId)?.country_code}
          {seasonId}
          {plots}
          {crops}
          {operators}
          {machinery}
          {products}
          {units}
          {justifications}
          {efficacies}
          {reasons}
          onSaved={treatmentSaved}
          onCancel={() => (treatmentFormOpen = false)}
        />
      {/if}

      <ul class="card-list">
        {#each treatments as { record, plots: treatedPlots, problems, justifications: recordJustifications } (record.id)}
          <li class="card">
            <div class="stack">
              <strong>{formatDate(record.application_date)} — {record.product_name_snapshot}</strong
              >
              <span class="detail">
                {record.dose_value}
                {tCode("unit", record.dose_unit_code)} ·
                {record.operator_name_snapshot}
              </span>
              <span class="detail">{problemSummary(problems)}</span>
              <span class="detail">
                {recordJustifications
                  .map((j) => tCode("justification", j.justification_code))
                  .join(", ")}
              </span>
              <span class="detail">{treatedPlotsSummary(treatedPlots)}</span>
              <span class="detail">
                {t("treatment.phi_until", { date: formatDate(record.phi_end_date) })}
              </span>
              <label class="inline-field">
                <span>{t("treatment.efficacy")}</span>
                <select
                  value={record.efficacy_code ?? ""}
                  onchange={(event) => setEfficacy(record, event.currentTarget.value)}
                >
                  <option value="">{t("treatment.efficacy_pending")}</option>
                  {#each efficacies as efficacy (efficacy.code)}
                    <option value={efficacy.code}>{tCode("efficacy", efficacy.code)}</option>
                  {/each}
                </select>
              </label>
            </div>
            <button type="button" class="btn-danger" onclick={() => deleteTreatment(record)}>
              {t("treatment.delete")}
            </button>
          </li>
        {/each}
      </ul>
      {#if treatments.length === 0 && !missingRefs}
        <p>{t("treatments.empty")}</p>
      {/if}

      <div class="view-head">
        <h3>{t("export.title")}</h3>
        <button type="button" onclick={exportCuaderno} disabled={treatments.length === 0}>
          {t("export.run")}
        </button>
      </div>
      <p class="detail">{t("export.hint")}</p>

      {#if precheck && !precheckIsClean(precheck)}
        <p>{t("export.blocked_intro")}</p>
        <ul class="card-list">
          {#if precheck.farm_missing_fields.length > 0}
            <li class="card">
              <div class="stack">
                <strong>{t("export.farm_fields")}</strong>
                <span class="detail">
                  {precheck.farm_missing_fields.map((f) => t(`export.field_${f}`)).join(", ")}
                </span>
                <a href="#/farms">{t("nav.farms")}</a>
              </div>
            </li>
          {/if}
          {#if precheck.records_missing_efficacy.length > 0}
            <li class="card">
              <div class="stack">
                <strong>{t("export.missing_efficacy")}</strong>
                <span class="detail">
                  {precheck.records_missing_efficacy.map(recordLabel).join("; ")}
                </span>
              </div>
            </li>
          {/if}
          {#if precheck.records_missing_operator_licence.length > 0}
            <li class="card">
              <div class="stack">
                <strong>{t("export.missing_licence")}</strong>
                <span class="detail">
                  {precheck.records_missing_operator_licence.map(recordLabel).join("; ")}
                </span>
              </div>
            </li>
          {/if}
          {#if precheck.plots_missing_crop.length > 0}
            <li class="card">
              <div class="stack">
                <strong>{t("export.missing_crop")}</strong>
                <span class="detail">
                  {precheck.plots_missing_crop
                    .map((ref) => `${ref.plot_name} (${formatDate(ref.application_date)})`)
                    .join("; ")}
                </span>
              </div>
            </li>
          {/if}
        </ul>
      {/if}

      <div class="view-head">
        <h3>{t("export.pdf_title")}</h3>
        <button type="button" onclick={exportCuadernoPdf}>
          {t("export.pdf_run")}
        </button>
      </div>
      <p class="detail">{t("export.pdf_hint")}</p>
    {/if}
  {/if}
</section>
