<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, crops tab: the season's crops per plot (model section 2.1),
  // entered by hand or proposed from the farmer's own PAC declaration. The
  // farm and season come from the shell, which owns the selectors.
  import TzTooltip from "./TzTooltip.svelte";
  import { formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { sortedBy } from "./collate.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import SpeciesPicker from "./SpeciesPicker.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

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
  let cropCode = $state(null);

  // The SIGPAC declared-crops review panel: null until the farmer asks for it.
  let proposals = $state(null);
  // Per-row editable state, keyed by row index — the proposal rows themselves
  // stay as the backend described them, so nothing edited here can be mistaken
  // for what SIGPAC actually said.
  let proposalEdits = $state([]);
  // Which proposal the panel under the review table is showing, by row index.
  // Null when none is open, which is the state a fresh review starts in: the
  // everyday case is ticking the rows and confirming without editing anything.
  let proposalOpen = $state(null);

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
    cropCode = crop?.crop_code ?? null;
    cropFormOpen = true;
  }

  function hideCropForm() {
    cropFormOpen = false;
    editingCropId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which crop it is about. Null while creating.
  const editingCrop = $derived(crops.find((crop) => crop.id === editingCropId) ?? null);

  async function submitCrop() {
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
      // Sent on edits too: the field is form state, so leaving it out would
      // detach the species from the catalogue on every unrelated correction.
      crop_code: cropCode,
    };
    if (editingCropId) {
      await invoke("update_crop", { cropId: editingCropId, update: payload });
    } else {
      await invoke("create_crop", {
        crop: { ...payload, plot_id: cropPlotId, season_id: seasonId },
      });
    }
    hideCropForm();
    await onChanged();
  }

  function deleteCrop(crop) {
    run(async () => {
      if (!(await confirmDialog(t("crop.delete_confirm", { species: cropLabel(crop) })))) return;
      await invoke("delete_crop", { cropId: crop.id });
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
      proposalOpen = null;
    });
  }

  function closeProposals() {
    proposals = null;
    proposalEdits = [];
    proposalOpen = null;
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

  /// What accepting this row would do, in the farmer's words. Three phrasings
  /// because a proposal either replaces a crop already recorded, adds a second
  /// one to the plot, or is the plot's first.
  function proposalAction(row) {
    if (row.kind === "update") {
      return t("crops.proposal_update", { name: row.existing_species_name });
    }
    return row.kind === "insert_secondary"
      ? t("crops.proposal_secondary")
      : t("crops.proposal_insert");
  }

  function plotName(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? plotId;
  }

  function cropLabel(crop) {
    return crop.variety ? `${crop.species_name} — ${crop.variety}` : crop.species_name;
  }

  // The "·"-joined detail line these rows used to share is gone: every value
  // is its own column, which is what lets a reader scan the plots — or the
  // irrigation systems — down the list instead of reading six sentences.
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
    <p class="table-empty">{t("crops.proposal_empty")}</p>
  {:else}
    <!-- `rows-static`: a proposal is not a record yet, and the tick IS the
         decision. What can be edited before accepting — the species, the
         variety, the surface — opens BELOW in a panel for the row being
         looked at, rather than three inline forms unfolding down the list.
         The campaign rides on every row: the service runs a campaign behind,
         and a record book must never record last year's declaration as this
         year's crop without saying so. -->
    <div class="table-wrap">
      <table class="data-table rows-static" use:resizableColumns={"crop-proposals"}>
        <thead>
          <tr>
            <th class="col-tick">{t("crops.proposals_accept")}</th>
            <th>{t("column.plot")}</th>
            <th>{t("column.crop")}</th>
            <th>{t("column.campaign")}</th>
            <th class="col-num">{t("column.area_ha")}</th>
            <th>{t("crops.proposals_effect")}</th>
          </tr>
        </thead>
        <tbody>
          {#each proposals.rows as row, i (`${row.plot_id}-${i}`)}
            {@const selectable = SELECTABLE.includes(row.kind)}
            <tr class:selected={proposalOpen === i}>
              <td class="col-tick">
                {#if selectable}
                  <TzCheckbox
                    label={proposalAction(row)}
                    labelHidden
                    bind:checked={proposalEdits[i].accepted}
                  />
                {/if}
              </td>
              <td class="col-name">{row.plot_name}</td>
              <td class="col-muted">
                {#if selectable}
                  <button type="button" class="row-open" onclick={() => (proposalOpen = i)}>
                    {row.species_name ?? t("crops.proposal_unresolved", { code: row.crop_code })}
                  </button>
                {:else}
                  {row.species_name ?? t("crops.proposal_unresolved", { code: row.crop_code })}
                {/if}
              </td>
              <td class="col-muted">{row.campaign}</td>
              <td class="col-muted col-num">
                {row.declared_area_ha === null ? "" : formatNumber(row.declared_area_ha)}
              </td>
              <td class="col-muted">
                {#if selectable}
                  {proposalAction(row)}
                {:else if row.kind === "already_recorded"}
                  {t("crops.proposal_already", { name: row.existing_species_name })}
                {:else}
                  {t(`crops.proposal_blocked_${row.blocked_reason}`)}
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    {#if proposalOpen !== null && proposals.rows[proposalOpen]}
      {@const row = proposals.rows[proposalOpen]}
      <div class="subpanel">
        <div class="inspector-head">
          <span>{row.plot_name}</span>
          <TzTooltip label={t("form.close")}>
            {#snippet trigger(props)}
              <button
                {...props}
                type="button"
                class="inspector-close"
                onclick={(event) => {
                  props.onclick?.(event);
                  proposalOpen = null;
                }}
                aria-label={t("form.close")}>×</button
              >
            {/snippet}
          </TzTooltip>
        </div>
        <p class="detail">
          {t("crops.proposals_campaign", { campaign: row.campaign, season: seasonLabel })}
        </p>
        {#if row.kind === "update"}
          <p class="detail">{t("crops.proposal_update_hint")}</p>
        {/if}
        <div class="form-grid">
          <label>
            <span>{t("crop.species")}</span>
            <SpeciesPicker
              bind:name={proposalEdits[proposalOpen].species}
              bind:code={proposalEdits[proposalOpen].code}
              plotId={row.plot_id}
              required
            />
          </label>
          <TextInput label={t("crop.variety")} bind:value={proposalEdits[proposalOpen].variety} />
          <NumberInput
            label={t("crop.area_ha")}
            min={0}
            bind:value={proposalEdits[proposalOpen].areaHa}
          />
        </div>
      </div>
    {/if}
  {/if}

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
        count: proposals.plots_unreachable.length,
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

<TzWorkspace
  open={cropFormOpen}
  title={editingCropId ? cropLabel({ species_name: species, variety }) : t("crops.new")}
  onclose={hideCropForm}
  ondelete={editingCrop ? () => deleteCrop(editingCrop) : null}
>
  {#snippet list()}
    {#if crops.length === 0}
      <p class="table-empty">{t("crops.empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"crops"}>
          <thead>
            <tr>
              <th>{t("column.crop")}</th>
              <th>{t("column.plot")}</th>
              <th class="col-num">{t("column.area_ha")}</th>
              <th>{t("column.production_system")}</th>
              <th>{t("column.irrigation")}</th>
              <th>{t("column.environment")}</th>
              <th>{t("column.gip")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedCrops as crop (crop.id)}
              <tr
                class:selected={editingCropId === crop.id}
                onclick={(e) => opensRow(e) && showCropForm(crop)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showCropForm(crop)}>
                    {cropLabel(crop)}
                  </button>
                </td>
                <td class="col-muted">{plotName(crop.plot_id)}</td>
                <td class="col-muted col-num">
                  {crop.area_ha == null ? "" : formatNumber(crop.area_ha)}
                </td>
                <td class="col-muted">
                  {crop.production_system_code
                    ? tCode("production_system", crop.production_system_code)
                    : ""}
                </td>
                <td class="col-muted">
                  {crop.irrigation_code ? tCode("irrigation_system", crop.irrigation_code) : ""}
                </td>
                <td class="col-muted">
                  {crop.growing_environment_code
                    ? tCode("growing_environment", crop.growing_environment_code)
                    : ""}
                </td>
                <td class="col-muted">
                  {crop.gip_system_code ? tCode("gip_system", crop.gip_system_code) : ""}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/snippet}

  {#snippet inspector(formId)}
    <TzForm id={formId} onsubmit={submitCrop}>
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
        <TextInput label={t("crop.variety")} bind:value={variety} />
        <TzSelect
          label={t("crop.production_system")}
          items={codeItems(productionSystems, "production_system")}
          nullable
          bind:value={systemCode}
        />
        <NumberInput label={t("crop.area_ha")} min={0} bind:value={cropAreaHa} />
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
      </div>
    </TzForm>
  {/snippet}

  {#snippet actions(formId)}
    <div class="form-actions">
      <button type="submit" form={formId}>{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={hideCropForm}>
        {t("form.cancel")}
      </button>
    </div>
  {/snippet}
</TzWorkspace>
