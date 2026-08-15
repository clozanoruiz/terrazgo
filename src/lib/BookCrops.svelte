<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, crops tab: the season's crops per plot (model section 2.1),
  // entered by hand or proposed from the farmer's own PAC declaration. The
  // farm and season come from the shell, which owns the selectors.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { sortedBy } from "./collate.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import SpeciesPicker from "./SpeciesPicker.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";

  let { farmId, seasonId, seasonLabel, plots, crops, onChanged } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const productionSystems = $derived(lookups.productionSystems);
  const irrigationSystems = $derived(lookups.irrigationSystems);
  const growingEnvironments = $derived(lookups.growingEnvironments);
  const gipSystems = $derived(lookups.gipSystems);

  let cropFormOpen = $state(false);

  // Crop form; same create/edit convention as the registry sections.
  let editingCropId = $state(null);
  let cropPlotId = $state("");
  let species = $state("");
  let variety = $state("");
  let systemCode = $state("");
  let cropAreaHa = $state("");
  let irrigationCode = $state("");
  let environmentCode = $state("");
  let gipCode = $state("");
  let sownOn = $state("");
  let cropCode = $state(null);

  // The SIGPAC declared-crops review panel: null until the farmer asks for it.
  let proposals = $state(null);
  // Per-row editable state, keyed by row index — the proposal rows themselves
  // stay as the backend described them, so nothing edited here can be mistaken
  // for what SIGPAC actually said.
  let proposalEdits = $state([]);

  function showCropForm(crop = null) {
    editingCropId = crop?.id ?? null;
    cropPlotId = crop?.plot_id ?? "";
    species = crop?.species_name ?? "";
    variety = crop?.variety ?? "";
    systemCode = crop?.production_system_code ?? "";
    cropAreaHa = crop?.area_ha ?? "";
    irrigationCode = crop?.irrigation_code ?? "";
    environmentCode = crop?.growing_environment_code ?? "";
    gipCode = crop?.gip_system_code ?? "";
    sownOn = crop?.sown_on ?? "";
    cropCode = crop?.crop_code ?? null;
    cropFormOpen = true;
  }

  function hideCropForm() {
    cropFormOpen = false;
    editingCropId = null;
  }

  function submitCrop(event) {
    event.preventDefault();
    // The plot and season are absent from the edit payload on purpose: a crop
    // never moves, or it would take its treatment history with it.
    const payload = {
      species_name: species.trim(),
      variety: variety.trim() || null,
      production_system_code: systemCode || null,
      // The surface THIS crop occupies; blank means "not stated" and prints
      // blank, which is the honest answer on a plot carrying several crops.
      area_ha: cropAreaHa === "" ? null : Number(cropAreaHa),
      irrigation_code: irrigationCode || null,
      growing_environment_code: environmentCode || null,
      gip_system_code: gipCode || null,
      sown_on: sownOn || null,
      // Sent on edits too: the field is form state, so leaving it out would
      // detach the species from the catalogue on every unrelated correction.
      crop_code: cropCode,
    };
    run(async () => {
      if (editingCropId) {
        await invoke("update_crop", { cropId: editingCropId, update: payload });
      } else {
        await invoke("create_crop", {
          crop: { ...payload, plot_id: cropPlotId, season_id: seasonId },
        });
      }
      notify(t("message.crop_saved", { species: payload.species_name }));
      hideCropForm();
      await onChanged();
    });
  }

  function deleteCrop(crop) {
    run(async () => {
      if (!(await confirmDialog(t("crop.delete_confirm", { species: cropLabel(crop) })))) return;
      await invoke("delete_crop", { cropId: crop.id });
      notify(t("message.crop_deleted"));
      hideCropForm();
      await onChanged();
    });
  }

  // --- SIGPAC declared crops -------------------------------------------------

  /// Rows the farmer can actually act on; the rest are shown as information.
  const SELECTABLE = ["insert", "insert_secondary", "update"];

  function loadProposals(refresh = false) {
    run(async () => {
      const found = await invoke("sigpac_propose_crops", { farmId, seasonId, refresh });
      proposals = found;
      proposalEdits = found.rows.map((row) => ({
        // Plain new crops on an empty plot are the everyday case, so they
        // start selected; restating an existing crop, or adding a second one,
        // is a decision the farmer makes deliberately.
        accepted: row.kind === "insert",
        species: row.species_name ?? "",
        code: row.crop_code,
        variety: "",
        areaHa: row.declared_area_ha ?? "",
      }));
    });
  }

  function closeProposals() {
    proposals = null;
    proposalEdits = [];
  }

  const acceptedCount = $derived(
    proposals
      ? proposals.rows.filter(
          (row, i) => SELECTABLE.includes(row.kind) && proposalEdits[i]?.accepted,
        )
      : [],
  );

  /// The campaigns the accepted rows actually come from, for the confirm
  /// button — usually one, but a farm can straddle two if SIGPAC has loaded
  /// the new campaign for some municipalities and not others.
  const acceptedCampaigns = $derived([...new Set(acceptedCount.map((row) => row.campaign))].sort());

  function applyProposals() {
    const inserts = [];
    const updates = [];
    proposals.rows.forEach((row, i) => {
      const edit = proposalEdits[i];
      if (!edit?.accepted || !SELECTABLE.includes(row.kind)) return;
      const fields = {
        species_name: edit.species.trim(),
        variety: edit.variety.trim() || null,
        area_ha: edit.areaHa === "" ? null : Number(edit.areaHa),
        irrigation_code: row.suggested_irrigation_code,
        crop_code: edit.code,
        // What the row came from, so the book can always say which campaign's
        // declaration a crop repeats.
        source: "sigpac",
        source_campaign: row.campaign,
        declared_area_ha: row.declared_area_ha,
      };
      if (row.kind === "update") {
        const existing = crops.find((crop) => crop.id === row.existing_crop_id);
        updates.push({
          crop_id: row.existing_crop_id,
          update: {
            ...fields,
            // Untouched by the declaration: SIGPAC says nothing about these,
            // so a restatement must not quietly blank them.
            production_system_code: existing?.production_system_code ?? null,
            growing_environment_code: existing?.growing_environment_code ?? null,
            gip_system_code: existing?.gip_system_code ?? null,
            sown_on: existing?.sown_on ?? null,
            variety: edit.variety.trim() || existing?.variety || null,
            irrigation_code: fields.irrigation_code ?? existing?.irrigation_code ?? null,
          },
        });
      } else {
        inserts.push({
          crop: {
            ...fields,
            plot_id: row.plot_id,
            season_id: seasonId,
            production_system_code: null,
            growing_environment_code: null,
            gip_system_code: null,
            sown_on: null,
          },
        });
      }
    });

    run(async () => {
      const summary = await invoke("sigpac_accept_crop_proposals", {
        farmId,
        seasonId,
        inserts,
        updates,
      });
      notify(
        t("message.crops_imported", {
          inserted: summary.inserted,
          updated: summary.updated,
        }),
      );
      if (summary.skipped.length > 0) {
        notify(
          t("message.crops_import_skipped", {
            count: summary.skipped.length,
            reasons: summary.skipped
              .map((row) => `${row.species_name} (${t(`crops.proposal_blocked_${row.reason}`)})`)
              .join("; "),
          }),
          true,
        );
      }
      closeProposals();
      await onChanged();
    });
  }

  // Collated, not left in the order module-sigpac returned: that is SQL's
  // BINARY order, which files every accented name after every unaccented one.
  // Collated: the repository returns BINARY order, which files accented species
  // names last, and the printed book orders the same names with ICU.
  const sortedCrops = $derived(sortedBy(crops, cropLabel));

  function proposalPlots(list) {
    return sortedBy(list, (plot) => plot.plot_name)
      .map((plot) => plot.plot_name)
      .join(", ");
  }

  function plotName(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? plotId;
  }

  function cropLabel(crop) {
    return crop.variety ? `${crop.species_name} — ${crop.variety}` : crop.species_name;
  }

  function cropDetail(crop) {
    return [
      plotName(crop.plot_id),
      crop.area_ha ? t("crop.area_detail", { area: crop.area_ha }) : null,
      crop.production_system_code ? tCode("production_system", crop.production_system_code) : null,
      crop.irrigation_code ? tCode("irrigation_system", crop.irrigation_code) : null,
      crop.growing_environment_code
        ? tCode("growing_environment", crop.growing_environment_code)
        : null,
      crop.gip_system_code ? tCode("gip_system", crop.gip_system_code) : null,
      crop.sown_on ? t("crop.sown_detail", { date: formatDate(crop.sown_on) }) : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<div class="view-head">
  <h3>{t("crops.title")}</h3>
  <div class="selector-buttons">
    <button type="button" onclick={() => showCropForm()} disabled={plots.length === 0}>
      {t("crops.new")}
    </button>
    <button type="button" onclick={() => loadProposals(false)} disabled={plots.length === 0}>
      {t("crops.load_declared")}
    </button>
  </div>
</div>
{#if plots.length === 0}
  <p>{t("treatments.no_plots")}</p>
{/if}

{#if proposals}
  <div class="view-head">
    <h4>{t("crops.proposals_title")}</h4>
    <div class="selector-buttons">
      <button type="button" onclick={() => loadProposals(true)}>
        {t("crops.proposals_refresh")}
      </button>
      <button type="button" class="btn-cancel" onclick={closeProposals}>
        {t("form.cancel")}
      </button>
    </div>
  </div>

  {#if proposals.rows.length === 0}
    <p>{t("crops.proposal_empty")}</p>
  {/if}

  <ul class="card-list">
    {#each proposals.rows as row, i (`${row.plot_id}-${i}`)}
      <li class="card">
        <div class="stack">
          <strong>
            {row.plot_name} —
            {#if row.species_name}
              {row.species_name}
            {:else}
              {t("crops.proposal_unresolved", { code: row.crop_code })}
            {/if}
          </strong>
          <!-- The campaign rides on EVERY row: the service runs a campaign
               behind, and a record book must never record last year's
               declaration as this year's crop without saying so. -->
          <span class="detail">
            {t("crops.proposals_campaign", { campaign: row.campaign, season: seasonLabel })}
            {#if row.declared_area_ha !== null}
              · {t("crops.proposal_declared_area", { area: row.declared_area_ha })}
            {/if}
          </span>

          {#if SELECTABLE.includes(row.kind)}
            <label class="inline-field">
              <input type="checkbox" bind:checked={proposalEdits[i].accepted} />
              <span>
                {#if row.kind === "update"}
                  {t("crops.proposal_update", { name: row.existing_species_name })}
                {:else if row.kind === "insert_secondary"}
                  {t("crops.proposal_secondary")}
                {:else}
                  {t("crops.proposal_insert")}
                {/if}
              </span>
            </label>
            {#if row.kind === "update"}
              <span class="detail">{t("crops.proposal_update_hint")}</span>
            {/if}
            {#if proposalEdits[i].accepted}
              <div class="form-grid">
                <label>
                  <span>{t("crop.species")}</span>
                  <SpeciesPicker
                    bind:name={proposalEdits[i].species}
                    bind:code={proposalEdits[i].code}
                    plotId={row.plot_id}
                    required
                  />
                </label>
                <label>
                  <span>{t("crop.variety")}</span>
                  <input bind:value={proposalEdits[i].variety} />
                </label>
                <label>
                  <span>{t("crop.area_ha")}</span>
                  <input type="number" min="0" step="0.0001" bind:value={proposalEdits[i].areaHa} />
                </label>
              </div>
            {/if}
          {:else if row.kind === "already_recorded"}
            <span class="detail">
              {t("crops.proposal_already", { name: row.existing_species_name })}
            </span>
          {:else}
            <span class="detail">
              {t(`crops.proposal_blocked_${row.blocked_reason}`)}
            </span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  {#if proposals.plots_without_declaration.length > 0}
    <p class="detail">
      {t("crops.proposal_none", {
        current: proposals.current_campaign,
        previous: proposals.current_campaign - 1,
        plots: proposalPlots(proposals.plots_without_declaration),
      })}
    </p>
  {/if}
  {#if proposals.plots_without_reference.length > 0}
    <p class="detail">
      {t("crops.proposal_no_ref", { plots: proposalPlots(proposals.plots_without_reference) })}
    </p>
  {/if}
  <!-- "We could not ask" is deliberately its own line: it is not the same
       claim as "SIGPAC has nothing for this plot". -->
  {#if proposals.plots_unreachable.length > 0}
    <p class="detail">
      {t("crops.proposal_unreachable", {
        reason: proposals.unreachable_reason ?? "",
        plots: proposalPlots(proposals.plots_unreachable),
      })}
    </p>
  {/if}

  {#if acceptedCount.length > 0}
    <div class="form-actions">
      <button type="button" onclick={applyProposals}>
        {t("crops.proposal_confirm", {
          count: acceptedCount.length,
          campaigns: acceptedCampaigns.join(", "),
        })}
      </button>
    </div>
  {/if}
{/if}

{#if cropFormOpen}
  <form onsubmit={submitCrop}>
    <div class="form-grid">
      <!-- Locked while editing: a crop never changes plot (its treatment
           history points here). Correcting one means delete + create. -->
      <TzSelect
        label={t("crop.plot")}
        items={nameItems(
          plots,
          (p) => p.plot.name,
          (p) => p.plot.id,
        )}
        required
        disabled={editingCropId !== null}
        bind:value={cropPlotId}
      />
      <label>
        <span>{t("crop.species")}</span>
        <SpeciesPicker bind:name={species} bind:code={cropCode} plotId={cropPlotId} required />
      </label>
      <label><span>{t("crop.variety")}</span><input bind:value={variety} /></label>
      <TzSelect
        label={t("crop.production_system")}
        items={codeItems(productionSystems, "production_system")}
        nullable
        bind:value={systemCode}
      />
      <label>
        <span>{t("crop.area_ha")}</span>
        <input type="number" min="0" step="0.0001" bind:value={cropAreaHa} />
      </label>
      <TzSelect
        label={t("crop.irrigation")}
        items={codeItems(irrigationSystems, "irrigation_system")}
        nullable
        bind:value={irrigationCode}
      />
      <TzSelect
        label={t("crop.growing_environment")}
        items={codeItems(growingEnvironments, "growing_environment")}
        nullable
        bind:value={environmentCode}
      />
      <TzSelect
        label={t("crop.gip_system")}
        items={codeItems(gipSystems, "gip_system")}
        nullable
        bind:value={gipCode}
      />
      <DateInput label={t("crop.sown_on")} bind:value={sownOn} />
    </div>
    <div class="form-actions">
      <button type="submit">{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={hideCropForm}>
        {t("form.cancel")}
      </button>
    </div>
  </form>
{/if}

<ul class="card-list">
  {#each sortedCrops as crop (crop.id)}
    <li class="card">
      <strong>{cropLabel(crop)}</strong>
      <span class="detail">{cropDetail(crop)}</span>
      <button type="button" onclick={() => showCropForm(crop)}>{t("form.edit")}</button>
      <button type="button" class="btn-danger" onclick={() => deleteCrop(crop)}>
        {t("form.delete")}
      </button>
    </li>
  {/each}
</ul>
{#if crops.length === 0}
  <p>{t("crops.empty")}</p>
{/if}
