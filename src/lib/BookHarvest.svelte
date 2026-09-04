<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, harvest-and-analyses tab: model sections 4 (registro de
  // análisis) and 5 (registro de cosecha comercializada). Two registers in one
  // tab because a farmer files them together — the sample and the sale are the
  // two things that happen to a crop once it is standing.
  //
  // Neither carries the model's "APLICA TRATAMIENTO: SÍ/NO" line: both are
  // recommended registers (art. 16.3's duty to keep the underlying documents,
  // and food-chain traceability), not conditional ones, so there is no
  // declaration to make and no answer to store.
  //
  // Section 4 is metadata only. The analysis bulletin itself stays in the
  // farmer's folder; this register says where to find it.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import DateInput from "./DateInput.svelte";
  import PlantProductPicker from "./PlantProductPicker.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TzCombobox from "./TzCombobox.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId, seasonId, countryCode, plots, crops } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const quantityUnits = $derived(lookups.quantityUnits);
  const analysisMaterials = $derived(lookups.analysisMaterials);
  const analysisTypes = $derived(lookups.analysisTypes);

  let analyses = $state([]);
  let harvests = $state([]);
  let loading = $state(true);

  // A sold harvest is weighed, never measured by volume — the model's own
  // "Cantidad (kg)", widened to tonnes because that is how a cereal lorry is
  // weighed. The repository rejects anything else regardless.
  const HARVEST_UNITS = ["kg", "t"];
  const harvestUnits = $derived(quantityUnits.filter((unit) => HARVEST_UNITS.includes(unit.code)));

  load();

  function load() {
    run(async () => {
      [analyses, harvests] = await Promise.all([
        invoke("list_analysis_records", { seasonId, farmId }),
        invoke("list_harvest_records", { seasonId, farmId }),
      ]);
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  /// The crops recorded on one plot this campaign — what the "which crop was
  /// sampled/sold" selector offers. A plot with none still takes a record: the
  /// model asks which parcel, and the crop link is what freezes the snapshot.
  function cropsOfPlot(plotId) {
    return crops.filter((crop) => crop.plot_id === plotId);
  }

  function emptyPlotRow() {
    return { plotId: "", cropId: "" };
  }

  /// A plot change invalidates whatever crop was chosen under the old one.
  function onPlotChosen(row) {
    const available = cropsOfPlot(row.plotId);
    row.cropId = available.length === 1 ? available[0].id : "";
  }

  function submittedPlots(rows) {
    return rows
      .filter((row) => row.plotId)
      .map((row) => ({ plot_id: row.plotId, crop_id: row.cropId || null }));
  }

  // --- section 4: análisis ---------------------------------------------------

  let analysisFormOpen = $state(false);
  let editingAnalysisId = $state(null);
  let sampledOn = $state("");
  let materialKind = $state("plant");
  let bulletinNumber = $state("");
  let labName = $state("");
  let labAddress = $state("");
  let labTaxId = $state("");
  let substances = $state("");
  let analysisNotes = $state("");
  let analysisPlots = $state([emptyPlotRow()]);
  let checkedAnalysisTypes = $state([]);
  // The coded findings, as rows like the treatment form's problem rows: the
  // catalogue is 283 entries, so the picker narrows as you type.
  let substanceRows = $state([]);
  let substanceCatalogue = $state([]);

  function emptySubstanceRow() {
    return { code: "", filter: "" };
  }

  /// Loaded once, on first use: the catalogue ships in the binary, so this is a
  /// database read and not a download — but 283 rows are not worth holding
  /// before the farmer opens the form.
  function loadSubstances() {
    if (substanceCatalogue.length > 0) return;
    run(async () => {
      substanceCatalogue = await invoke("list_substance_codes", { countryCode });
    });
  }

  // The combobox narrows the catalogue itself, so this only shapes it into
  // items. SUST_ACTIVAS runs to 283 entries, which is why it is a combobox and
  // not a listbox.
  function substanceItems() {
    return substanceCatalogue.map((substance) => ({
      value: substance.code,
      label: substance.name,
    }));
  }

  /// Anexo III A.3's nine soil figures. Kept as a plain object of strings, the
  /// way every other numeric input in this app is: empty means the bulletin did
  /// not report it, which is different from zero.
  function emptySoil() {
    return {
      ph: "",
      organic_matter_pct: "",
      available_p_mg_kg: "",
      available_k_mg_kg: "",
      total_n_pct: "",
      conductivity_ds_m: "",
      sand_pct: "",
      silt_pct: "",
      clay_pct: "",
    };
  }

  let soil = $state(emptySoil());

  /// The soil block only makes sense for a soil sample, and offering it on a
  /// residue bulletin would invite figures nobody measured.
  const soilApplies = $derived(materialKind === "soil");

  function showAnalysisForm(detail = null) {
    editingAnalysisId = detail?.record.id ?? null;
    sampledOn = detail?.record.sampled_on ?? "";
    materialKind = detail?.record.material_kind_code ?? "crop";
    bulletinNumber = detail?.record.bulletin_number ?? "";
    labName = detail?.record.lab_name ?? "";
    labAddress = detail?.record.lab_address ?? "";
    labTaxId = detail?.record.lab_tax_id ?? "";
    substances = detail?.record.substances_detected ?? "";
    analysisNotes = detail?.record.notes ?? "";
    analysisPlots = detail
      ? detail.plots.map((p) => ({ plotId: p.plot_id, cropId: p.crop_id ?? "" }))
      : [emptyPlotRow()];
    checkedAnalysisTypes = detail ? detail.types.map((t) => t.analysis_type_code) : [];
    substanceRows = detail
      ? detail.substances.map((s) => ({ code: s.substance_code, filter: "" }))
      : [];
    soil = { ...emptySoil() };
    for (const [key, value] of Object.entries(detail?.record.soil ?? {})) {
      if (value !== null && value !== undefined && key in soil) soil[key] = value;
    }
    loadSubstances();
    analysisFormOpen = true;
  }

  function hideAnalysisForm() {
    analysisFormOpen = false;
    editingAnalysisId = null;
  }

  /// The row the analyses inspector is editing, so its delete button knows
  /// which record it is about. Null while creating.
  const editingAnalysis = $derived(analyses.find((d) => d.record.id === editingAnalysisId) ?? null);

  async function submitAnalysis() {
    const payload = {
      sampled_on: sampledOn,
      material_kind_code: materialKind,
      bulletin_number: bulletinNumber.trim() || null,
      lab_name: labName.trim() || null,
      lab_address: labAddress.trim() || null,
      lab_tax_id: labTaxId.trim() || null,
      substances_detected: substances.trim() || null,
      // Empty stays null, never 0: an unmeasured parameter is unknown, and a
      // zero would be a figure the laboratory never reported.
      soil: Object.fromEntries(
        Object.entries(soil).map(([key, value]) => [key, value === "" ? null : Number(value)]),
      ),
      notes: analysisNotes.trim() || null,
      plots: submittedPlots(analysisPlots),
      analysis_type_codes: [...checkedAnalysisTypes],
      substance_codes: substanceRows.filter((row) => row.code).map((row) => row.code),
    };
    if (editingAnalysisId) {
      await invoke("update_analysis_record", {
        analysisRecordId: editingAnalysisId,
        update: payload,
      });
    } else {
      await invoke("create_analysis_record", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideAnalysisForm();
    load();
  }

  function deleteAnalysis(record) {
    run(async () => {
      if (!(await confirmDialog(t("analysis.delete_confirm")))) return;
      await invoke("delete_analysis_record", { analysisRecordId: record.id });
      hideAnalysisForm();
      load();
    });
  }

  // --- section 5: cosecha comercializada -------------------------------------

  let harvestFormOpen = $state(false);
  let editingHarvestId = $state(null);
  let harvestedOn = $state("");
  let productName = $state("");
  let plantProductCode = $state(null);
  let harvestQuantity = $state("");
  let harvestUnit = $state("kg");
  let deliveryNote = $state("");
  let lotNumber = $state("");
  let buyerName = $state("");
  let buyerTaxId = $state("");
  let buyerAddress = $state("");
  let buyerRegistry = $state("");
  let harvestNotes = $state("");
  let harvestPlots = $state([emptyPlotRow()]);

  function showHarvestForm(detail = null) {
    editingHarvestId = detail?.record.id ?? null;
    harvestedOn = detail?.record.harvested_on ?? "";
    productName = detail?.record.product_name ?? "";
    plantProductCode = detail?.record.plant_product_code ?? null;
    harvestQuantity = detail?.record.quantity_value ?? "";
    harvestUnit = detail?.record.quantity_unit_code ?? "kg";
    deliveryNote = detail?.record.delivery_note_ref ?? "";
    lotNumber = detail?.record.lot_number ?? "";
    buyerName = detail?.record.buyer_name ?? "";
    buyerTaxId = detail?.record.buyer_tax_id ?? "";
    buyerAddress = detail?.record.buyer_address ?? "";
    buyerRegistry = detail?.record.buyer_registry_number ?? "";
    harvestNotes = detail?.record.notes ?? "";
    harvestPlots = detail
      ? detail.plots.map((p) => ({ plotId: p.plot_id, cropId: p.crop_id ?? "" }))
      : [emptyPlotRow()];
    harvestFormOpen = true;
  }

  function hideHarvestForm() {
    harvestFormOpen = false;
    editingHarvestId = null;
  }

  /// The row the harvest inspector is editing, so its delete button knows
  /// which record it is about. Null while creating.
  const editingHarvest = $derived(harvests.find((d) => d.record.id === editingHarvestId) ?? null);

  /// The quantity cell: value and unit travel together or the cell is blank,
  /// because a figure with no unit says nothing.
  function quantityCell(record) {
    if (record.quantity_value === null) return "";
    return `${formatNumber(record.quantity_value)} ${tCode("unit", record.quantity_unit_code)}`;
  }

  /// One record's origin plots in a cell.
  function plotsCell(originPlots) {
    return originPlots.map((p) => plotName(p.plot_id)).join(", ");
  }

  async function submitHarvest() {
    // Value and unit travel together or not at all: the printed form leaves the
    // cell to be filled by hand, and a quantity with no unit says nothing.
    const stated = harvestQuantity !== "" && harvestQuantity !== null;
    const payload = {
      harvested_on: harvestedOn,
      product_name: productName.trim(),
      plant_product_code: plantProductCode || null,
      quantity_value: stated ? Number(harvestQuantity) : null,
      quantity_unit_code: stated ? harvestUnit : null,
      delivery_note_ref: deliveryNote.trim() || null,
      lot_number: lotNumber.trim() || null,
      buyer_name: buyerName.trim(),
      buyer_tax_id: buyerTaxId.trim() || null,
      buyer_address: buyerAddress.trim() || null,
      buyer_registry_number: buyerRegistry.trim() || null,
      notes: harvestNotes.trim() || null,
      plots: submittedPlots(harvestPlots),
    };
    if (editingHarvestId) {
      await invoke("update_harvest_record", {
        harvestRecordId: editingHarvestId,
        update: payload,
      });
    } else {
      await invoke("create_harvest_record", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideHarvestForm();
    load();
  }

  function deleteHarvest(record) {
    run(async () => {
      if (!(await confirmDialog(t("harvest.delete_confirm")))) return;
      await invoke("delete_harvest_record", { harvestRecordId: record.id });
      hideHarvestForm();
      load();
    });
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <!-- Section 5 first: the sale is what the farmer files most often. -->
  <div class="view-head">
    <h3>{t("harvest.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showHarvestForm()}>
        + {t("harvest.new")}
      </button>
    </div>
  </div>

  <TzWorkspace
    open={harvestFormOpen}
    title={editingHarvestId ? productName : t("harvest.new")}
    onclose={hideHarvestForm}
    ondelete={editingHarvest ? () => deleteHarvest(editingHarvest.record) : null}
  >
    {#snippet list()}
      {#if harvests.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"harvests"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.product")}</th>
                <th class="col-num">{t("column.quantity")}</th>
                <th>{t("column.buyer")}</th>
                <th>{t("column.lot")}</th>
                <th>{t("column.plots")}</th>
              </tr>
            </thead>
            <tbody>
              {#each harvests as entry (entry.record.id)}
                {@const record = entry.record}
                <tr
                  class:selected={editingHarvestId === record.id}
                  onclick={(e) => opensRow(e) && showHarvestForm(entry)}
                >
                  <td class="col-name">
                    <button type="button" class="row-open" onclick={() => showHarvestForm(entry)}>
                      {formatDate(record.harvested_on)}
                    </button>
                  </td>
                  <td class="col-muted">{record.product_name}</td>
                  <td class="col-muted col-num">{quantityCell(record)}</td>
                  <td class="col-muted">{record.buyer_name}</td>
                  <td class="col-muted">{record.lot_number ?? ""}</td>
                  <td class="col-muted">{plotsCell(entry.plots)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      <TzForm id={formId} onsubmit={submitHarvest}>
        <div class="form-grid">
          <DateInput label={t("harvest.harvested_on")} required bind:value={harvestedOn} />
          <label>
            <span>{t("harvest.product")}</span>
            <PlantProductPicker
              bind:name={productName}
              bind:code={plantProductCode}
              {countryCode}
              required
            />
            <small>{t("harvest.product_hint")}</small>
          </label>
          <NumberInput label={t("harvest.quantity")} min={0.001} bind:value={harvestQuantity} />
          <TzSelect
            label={t("harvest.unit")}
            items={codeItems(harvestUnits, "unit")}
            bind:value={harvestUnit}
          />
          <TextInput label={t("harvest.delivery_note")} bind:value={deliveryNote} />
          <TextInput label={t("harvest.lot")} bind:value={lotNumber} />
          <TextInput label={t("treatment.notes")} bind:value={harvestNotes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("harvest.buyer_section")}</legend>
          <div class="form-grid">
            <TextInput label={t("harvest.buyer_name")} required bind:value={buyerName} />
            <TextInput label={t("harvest.buyer_tax_id")} bind:value={buyerTaxId} />
            <TextInput label={t("harvest.buyer_address")} bind:value={buyerAddress} />
            <TextInput label={t("harvest.buyer_registry")} bind:value={buyerRegistry}>
              <small>{t("harvest.buyer_registry_hint")}</small>
            </TextInput>
          </div>
        </fieldset>

        <fieldset class="subsection">
          <legend>{t("harvest.plots_section")}</legend>
          {#each harvestPlots as row, index (row)}
            <div class="form-grid plot-row">
              <TzSelect
                label={t("crop.plot")}
                items={nameItems(
                  plots,
                  (p) => p.plot.name,
                  (p) => p.plot.id,
                )}
                required
                bind:value={row.plotId}
                onchange={() => onPlotChosen(row)}
              />
              <TzSelect
                label={t("treatment.crop")}
                items={nameItems(
                  cropsOfPlot(row.plotId),
                  (crop) => `${crop.species_name}${crop.variety ? ` — ${crop.variety}` : ""}`,
                )}
                nullable
                nullLabel=""
                bind:value={row.cropId}
              />
              {#if harvestPlots.length > 1}
                <button
                  type="button"
                  class="btn-danger"
                  onclick={() => harvestPlots.splice(index, 1)}
                >
                  {t("treatment.remove")}
                </button>
              {/if}
            </div>
          {/each}
          <button type="button" onclick={() => harvestPlots.push(emptyPlotRow())}>
            {t("treatment.add_plot")}
          </button>
        </fieldset>
      </TzForm>
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideHarvestForm}>
          {t("form.cancel")}
        </button>
      </div>
    {/snippet}
  </TzWorkspace>

  <div class="view-head">
    <h3>{t("analysis.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showAnalysisForm()}>
        + {t("analysis.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("analysis.keep_hint")}</p>

  <TzWorkspace
    open={analysisFormOpen}
    title={editingAnalysisId ? formatDate(sampledOn) : t("analysis.new")}
    onclose={hideAnalysisForm}
    ondelete={editingAnalysis ? () => deleteAnalysis(editingAnalysis.record) : null}
  >
    {#snippet list()}
      {#if analyses.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"analyses"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.material")}</th>
                <th>{t("column.bulletin")}</th>
                <th>{t("column.lab")}</th>
                <th>{t("column.plots")}</th>
              </tr>
            </thead>
            <tbody>
              {#each analyses as detail (detail.record.id)}
                {@const record = detail.record}
                <tr
                  class:selected={editingAnalysisId === record.id}
                  onclick={(e) => opensRow(e) && showAnalysisForm(detail)}
                >
                  <td class="col-name">
                    <button type="button" class="row-open" onclick={() => showAnalysisForm(detail)}>
                      {formatDate(record.sampled_on)}
                    </button>
                  </td>
                  <td class="col-muted">
                    {tCode("analysis_material", record.material_kind_code)}
                  </td>
                  <td class="col-muted">{record.bulletin_number ?? ""}</td>
                  <td class="col-muted">{record.lab_name ?? ""}</td>
                  <td class="col-muted">{plotsCell(detail.plots)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      <TzForm id={formId} onsubmit={submitAnalysis}>
        <div class="form-grid">
          <DateInput label={t("analysis.sampled_on")} required bind:value={sampledOn} />
          <TzSelect
            label={t("analysis.material")}
            items={codeItems(analysisMaterials, "analysis_material")}
            required
            bind:value={materialKind}
          />
          <TextInput label={t("analysis.bulletin")} bind:value={bulletinNumber} />
          <TextInput label={t("analysis.lab_name")} bind:value={labName} />
          <TextInput label={t("analysis.lab_address")} bind:value={labAddress} />
          <TextInput label={t("analysis.lab_tax_id")} bind:value={labTaxId} />
          <TextInput label={t("analysis.substances")} bind:value={substances}>
            <small>{t("analysis.substances_hint")}</small>
          </TextInput>
          <TextInput label={t("treatment.notes")} bind:value={analysisNotes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("analysis.types")}</legend>
          <p class="detail">{t("analysis.types_hint")}</p>
          <div class="checkbox-grid">
            {#each analysisTypes as kind (kind.code)}
              <TzCheckbox
                label={tCode("analysis_type", kind.code)}
                value={kind.code}
                bind:group={checkedAnalysisTypes}
              />
            {/each}
          </div>
        </fieldset>

        <fieldset class="subsection">
          <legend>{t("analysis.substances_coded")}</legend>
          <p class="detail">{t("analysis.substances_coded_hint")}</p>
          {#each substanceRows as row, index (row)}
            <div class="form-grid plot-row">
              <!-- The filter box is gone: the combobox's own input IS the
                 trigger, so one control does what two used to. -->
              <TzCombobox
                label={t("analysis.substance")}
                items={substanceItems()}
                placeholder={t("analysis.substance_filter_hint")}
                bind:value={row.code}
              />
              <button
                type="button"
                class="btn-danger"
                onclick={() => substanceRows.splice(index, 1)}
              >
                {t("treatment.remove")}
              </button>
            </div>
          {/each}
          <button type="button" onclick={() => substanceRows.push(emptySubstanceRow())}>
            {t("analysis.add_substance")}
          </button>
        </fieldset>

        {#if soilApplies}
          <fieldset class="subsection">
            <legend>{t("analysis.soil_section")}</legend>
            <p class="detail">{t("analysis.soil_hint")}</p>
            <div class="form-grid">
              <NumberInput label={t("analysis.soil_ph")} min={0} max={14} bind:value={soil.ph} />
              <NumberInput
                label={t("analysis.soil_organic_matter")}
                min={0}
                max={100}
                bind:value={soil.organic_matter_pct}
              />
              <NumberInput
                label={t("analysis.soil_p")}
                min={0}
                bind:value={soil.available_p_mg_kg}
              />
              <NumberInput
                label={t("analysis.soil_k")}
                min={0}
                bind:value={soil.available_k_mg_kg}
              />
              <NumberInput
                label={t("analysis.soil_n")}
                min={0}
                max={100}
                bind:value={soil.total_n_pct}
              />
              <NumberInput
                label={t("analysis.soil_conductivity")}
                min={0}
                bind:value={soil.conductivity_ds_m}
              />
              <NumberInput
                label={t("analysis.soil_sand")}
                min={0}
                max={100}
                bind:value={soil.sand_pct}
              />
              <NumberInput
                label={t("analysis.soil_silt")}
                min={0}
                max={100}
                bind:value={soil.silt_pct}
              />
              <NumberInput
                label={t("analysis.soil_clay")}
                min={0}
                max={100}
                bind:value={soil.clay_pct}
              />
            </div>
          </fieldset>
        {/if}

        <fieldset class="subsection">
          <legend>{t("analysis.plots_section")}</legend>
          {#each analysisPlots as row, index (row)}
            <div class="form-grid plot-row">
              <TzSelect
                label={t("crop.plot")}
                items={nameItems(
                  plots,
                  (p) => p.plot.name,
                  (p) => p.plot.id,
                )}
                required
                bind:value={row.plotId}
                onchange={() => onPlotChosen(row)}
              />
              <TzSelect
                label={t("treatment.crop")}
                items={nameItems(
                  cropsOfPlot(row.plotId),
                  (crop) => `${crop.species_name}${crop.variety ? ` — ${crop.variety}` : ""}`,
                )}
                nullable
                nullLabel=""
                bind:value={row.cropId}
              />
              {#if analysisPlots.length > 1}
                <button
                  type="button"
                  class="btn-danger"
                  onclick={() => analysisPlots.splice(index, 1)}
                >
                  {t("treatment.remove")}
                </button>
              {/if}
            </div>
          {/each}
          <button type="button" onclick={() => analysisPlots.push(emptyPlotRow())}>
            {t("treatment.add_plot")}
          </button>
        </fieldset>
      </TzForm>
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideAnalysisForm}>
          {t("form.cancel")}
        </button>
      </div>
    {/snippet}
  </TzWorkspace>
{/if}
