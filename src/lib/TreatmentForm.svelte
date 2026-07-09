<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Treatment entry form — the CUE module's central input (RD 1311/2012
  // mandatory fields). Multi-plot rows are dynamic; the legal snapshots, the
  // country and the PHI end date are derived in Rust at insert time, not here.
  import { formatDate, t, tCode } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";

  let {
    farmId,
    seasonId,
    plots,
    crops,
    operators,
    machinery,
    products,
    units,
    reasons,
    onSaved,
    onCancel,
  } = $props();

  let applicationDate = $state("");
  let productId = $state("");
  let doseValue = $state("");
  let doseUnit = $state("l_ha");
  let reasonCode = $state("");
  let targetOrganism = $state("");
  let operatorId = $state("");
  let machineryId = $state("");
  let phiDays = $state("");
  let notes = $state("");
  let rows = $state([emptyRow()]);

  function emptyRow() {
    return { plotId: "", cropId: "", surface: "" };
  }

  // Shown as a hint so the farmer knows what leaving PHI blank means.
  const defaultPhi = $derived(products.find((p) => p.id === productId)?.default_phi_days ?? null);

  function cropsForPlot(plotId) {
    return crops.filter((crop) => crop.plot_id === plotId);
  }

  function cropLabel(crop) {
    return crop.variety ? `${crop.species_name} — ${crop.variety}` : crop.species_name;
  }

  function onPlotChosen(row) {
    // Prefill the treated surface with the plot's full area — the common case;
    // a partial treatment just needs the number lowered.
    const detail = plots.find((p) => p.plot.id === row.plotId);
    if (detail?.plot.area_ha != null) row.surface = detail.plot.area_ha;
    // A crop belongs to one plot, so switching plots clears the selection.
    row.cropId = "";
  }

  function addRow() {
    rows.push(emptyRow());
  }

  function removeRow(index) {
    rows.splice(index, 1);
  }

  function submit(event) {
    event.preventDefault();
    const record = {
      season_id: seasonId,
      farm_id: farmId,
      application_date: applicationDate,
      product_id: productId,
      country_code: null, // derived from the farm in Rust
      dose_value: Number(doseValue),
      dose_unit_code: doseUnit,
      reason_category_code: reasonCode,
      target_organism: targetOrganism.trim() || null,
      operator_id: operatorId,
      machinery_id: machineryId || null,
      phi_days_used: String(phiDays).trim() === "" ? null : Number(phiDays),
      notes: notes.trim() || null,
    };
    const treatedPlots = rows.map((row) => ({
      plot_id: row.plotId,
      crop_id: row.cropId || null,
      surface_treated_ha: Number(row.surface),
    }));
    run(async () => {
      const saved = await invoke("create_treatment_record", { record, plots: treatedPlots });
      notify(t("message.treatment_saved", { date: formatDate(saved.phi_end_date) }));
      await onSaved();
    });
  }
</script>

<form onsubmit={submit}>
  <div class="form-grid">
    <label>
      <span>{t("treatment.date")}</span>
      <input type="date" required bind:value={applicationDate} />
    </label>
    <label>
      <span>{t("treatment.product")}</span>
      <select required bind:value={productId}>
        <option value="" disabled hidden></option>
        {#each products as product (product.id)}
          <option value={product.id}>{product.commercial_name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{t("treatment.dose")}</span>
      <input type="number" step="any" min="0.001" required bind:value={doseValue} />
    </label>
    <label>
      <span>{t("treatment.unit")}</span>
      <select required bind:value={doseUnit}>
        {#each units as unit (unit.code)}
          <option value={unit.code}>{tCode("unit", unit.code)}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{t("treatment.reason")}</span>
      <select required bind:value={reasonCode}>
        <option value="" disabled hidden></option>
        {#each reasons as reason (reason.code)}
          <option value={reason.code}>{tCode("reason_category", reason.code)}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{t("treatment.target")}</span>
      <input bind:value={targetOrganism} />
    </label>
    <label>
      <span>{t("treatment.operator")}</span>
      <select required bind:value={operatorId}>
        <option value="" disabled hidden></option>
        {#each operators as operator (operator.id)}
          <option value={operator.id}>{operator.full_name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{t("treatment.machinery")}</span>
      <select bind:value={machineryId}>
        <option value="">{t("treatment.machinery_none")}</option>
        {#each machinery as machine (machine.id)}
          <option value={machine.id}>{machine.name}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>{t("treatment.phi_days")}</span>
      <input type="number" min="0" step="1" bind:value={phiDays} placeholder={defaultPhi ?? ""} />
      {#if defaultPhi != null}
        <small>{t("treatment.phi_default", { days: defaultPhi })}</small>
      {/if}
    </label>
    <label>
      <span>{t("treatment.notes")}</span>
      <input bind:value={notes} />
    </label>
  </div>

  <fieldset class="subsection">
    <legend>{t("treatment.plots_section")}</legend>
    {#each rows as row, index (row)}
      <div class="form-grid plot-row">
        <label>
          <span>{t("crop.plot")}</span>
          <select required bind:value={row.plotId} onchange={() => onPlotChosen(row)}>
            <option value="" disabled hidden></option>
            {#each plots as { plot } (plot.id)}
              <option value={plot.id}>{plot.name}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>{t("treatment.crop")}</span>
          <select bind:value={row.cropId}>
            <option value="">{t("treatment.crop_none")}</option>
            {#each cropsForPlot(row.plotId) as crop (crop.id)}
              <option value={crop.id}>{cropLabel(crop)}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>{t("treatment.surface")}</span>
          <input type="number" step="any" min="0.01" required bind:value={row.surface} />
        </label>
        {#if rows.length > 1}
          <button type="button" class="btn-danger" onclick={() => removeRow(index)}>
            {t("treatment.remove")}
          </button>
        {/if}
      </div>
    {/each}
    <button type="button" onclick={addRow}>{t("treatment.add_plot")}</button>
  </fieldset>

  <div class="form-actions">
    <button type="submit">{t("form.save")}</button>
    <button type="button" class="btn-cancel" onclick={onCancel}>{t("form.cancel")}</button>
  </div>
</form>
