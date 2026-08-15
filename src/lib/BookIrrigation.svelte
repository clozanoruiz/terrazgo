<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, irrigation tab: model section 8.
  //
  // This is the record book's SECOND decree. RD 1051/2022 art. 5.e puts the
  // doses and dates of irrigation inside the same cuaderno duty as
  // fertilisation — binding since 1 January 2026, recorded within one month of
  // each operation — so this is a compliance register, not a convenience. The
  // future Irrigation module keeps planning (schedules, water balance, ETo);
  // what is entered here is what happened.
  //
  // Two things the printed model does not show, and why they are asked anyway:
  //
  //   * the water's own nitric nitrogen and soluble phosphorus (Anexo III C.l).
  //     Optional, because RD 1051/2022 art. 17.2 requires them only when the
  //     organismo de cuenca or comunidad de regantes supplies the figures.
  //   * the water's source and the energy driving the pump, which the SIEX twin
  //     carries. Optional there too, and captured so a future export does not
  //     have to ask the farmer again.
  //
  // The accumulated volume the model prints is NOT entered: the book computes
  // it as a running sum, because a stored copy could disagree with the rows
  // above it.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";

  let { farmId, seasonId, plots, crops } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const irrigationMethods = $derived(lookups.irrigationMethods);
  const waterOrigins = $derived(lookups.waterOrigins);
  const volumeUnits = $derived(lookups.volumeUnits);

  let records = $state([]);
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      records = await invoke("list_irrigation_records", { seasonId, farmId });
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  function cropsOfPlot(plotId) {
    return crops.filter((crop) => crop.plot_id === plotId);
  }

  function emptyPlotRow() {
    return { plotId: "", cropId: "", areaHa: "" };
  }

  /// A plot change invalidates whatever crop was chosen under the old one.
  function onPlotChosen(row) {
    const available = cropsOfPlot(row.plotId);
    row.cropId = available.length === 1 ? available[0].id : "";
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  let irrigatedOn = $state("");
  let endDate = $state("");
  let methodCode = $state("");
  let volumeValue = $state("");
  let volumeUnit = $state("m3_ha");
  let nitricN = $state("");
  let solubleP2o5 = $state("");
  // The twin's optional `TipoEnergia`. No input yet — entering it needs a
  // picker over the TIPENERGIA catalogue, and the printed model has no column
  // for it. Round-tripped rather than dropped, so editing a record that
  // carries one does not silently erase it.
  let energyTypeCode = $state("");
  let meterNumber = $state("");
  let notes = $state("");
  let plotRows = $state([emptyPlotRow()]);
  let chosenOrigins = $state([]);

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    irrigatedOn = detail?.record.irrigated_on ?? "";
    endDate = detail?.record.irrigation_end_date ?? "";
    methodCode = detail?.record.irrigation_method_code ?? irrigationMethods[0]?.code ?? "";
    volumeValue = detail?.record.volume_value ?? "";
    volumeUnit = detail?.record.volume_unit_code ?? "m3_ha";
    nitricN = detail?.record.water_nitric_n_mg_l ?? "";
    solubleP2o5 = detail?.record.water_soluble_p2o5_mg_l ?? "";
    energyTypeCode = detail?.record.energy_type_code ?? "";
    meterNumber = detail?.record.meter_number ?? "";
    notes = detail?.record.notes ?? "";
    plotRows = detail?.plots.length
      ? detail.plots.map((p) => ({
          plotId: p.plot_id,
          cropId: p.crop_id ?? "",
          areaHa: p.irrigated_area_ha ?? "",
        }))
      : [emptyPlotRow()];
    chosenOrigins = [...(detail?.water_origins ?? [])];
    formOpen = true;
  }

  function toggleOrigin(code, checked) {
    chosenOrigins = checked
      ? [...chosenOrigins, code]
      : chosenOrigins.filter((existing) => existing !== code);
  }

  /// Empty inputs become `null`, never 0: an unstated figure is unknown, and a
  /// zero would be a measurement the farmer never made.
  function optionalNumber(value) {
    return value === "" || value === null ? null : Number(value);
  }

  function submit(event) {
    event.preventDefault();
    const payload = {
      irrigated_on: irrigatedOn,
      irrigation_end_date: endDate || null,
      irrigation_method_code: methodCode,
      volume_value: Number(volumeValue),
      volume_unit_code: volumeUnit,
      water_nitric_n_mg_l: optionalNumber(nitricN),
      water_soluble_p2o5_mg_l: optionalNumber(solubleP2o5),
      energy_type_code: energyTypeCode.trim() || null,
      meter_number: meterNumber.trim() || null,
      notes: notes.trim() || null,
      plots: plotRows
        .filter((row) => row.plotId)
        .map((row) => ({
          plot_id: row.plotId,
          crop_id: row.cropId || null,
          irrigated_area_ha: optionalNumber(row.areaHa),
        })),
      water_origins: chosenOrigins,
    };

    run(async () => {
      if (editingId) {
        await invoke("update_irrigation_record", {
          irrigationRecordId: editingId,
          update: { ...payload, id: editingId },
        });
      } else {
        await invoke("create_irrigation_record", {
          record: { ...payload, season_id: seasonId, farm_id: farmId },
        });
      }
      notify(t("message.irrigation_saved"));
      formOpen = false;
      load();
    });
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("irrigation.delete_confirm")))) return;
      await invoke("delete_irrigation_record", { irrigationRecordId: record.id });
      notify(t("message.irrigation_deleted"));
      load();
    });
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <div class="view-head">
    <h3>{t("irrigation.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showForm()}>
        + {t("irrigation.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("irrigation.intro")}</p>

  <ul class="card-list">
    {#each records as detail (detail.record.id)}
      <li class="card">
        <div class="stack">
          <strong>
            {formatDate(detail.record.irrigated_on)}{detail.record.irrigation_end_date
              ? ` – ${formatDate(detail.record.irrigation_end_date)}`
              : ""}
            — {tCode("irrigation_method", detail.record.irrigation_method_code)}
          </strong>
          <span class="detail">
            {t("irrigation.volume_detail", {
              volume: detail.record.volume_value,
              unit: tCode("unit", detail.record.volume_unit_code),
            })}
            {#if detail.water_origins.length > 0}
              · {detail.water_origins.map((code) => tCode("water_origin", code)).join(", ")}
            {/if}
          </span>
          <span class="detail">
            {detail.plots.map((p) => plotName(p.plot_id)).join(", ")}
          </span>
        </div>
        <button type="button" onclick={() => showForm(detail)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => remove(detail.record)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>

  {#if formOpen}
    <form onsubmit={submit}>
      <div class="form-grid">
        <DateInput label={t("irrigation.irrigated_on")} required bind:value={irrigatedOn} />
        <DateInput
          label={t("irrigation.end_date")}
          hint={t("irrigation.end_date_hint")}
          bind:value={endDate}
        />
        <TzSelect
          label={t("irrigation.method")}
          items={codeItems(irrigationMethods, "irrigation_method")}
          required
          bind:value={methodCode}
        />
        <label>
          <span>{t("irrigation.volume")}</span>
          <input type="number" step="any" min="0.001" required bind:value={volumeValue} />
        </label>
        <TzSelect
          label={t("irrigation.volume_unit")}
          items={codeItems(volumeUnits, "unit")}
          bind:value={volumeUnit}
        />
        <label>
          <span>{t("irrigation.meter_number")}</span>
          <input bind:value={meterNumber} />
        </label>
        <label>
          <span>{t("treatment.notes")}</span>
          <input bind:value={notes} />
        </label>
      </div>

      <fieldset class="subsection">
        <legend>{t("irrigation.water_section")}</legend>
        <p class="detail">{t("irrigation.water_hint")}</p>
        <div class="form-grid">
          <label>
            <span>{t("irrigation.nitric_n")}</span>
            <input type="number" step="any" min="0" bind:value={nitricN} />
          </label>
          <label>
            <span>{t("irrigation.soluble_p2o5")}</span>
            <input type="number" step="any" min="0" bind:value={solubleP2o5} />
          </label>
        </div>
        <div class="checkbox-list">
          {#each waterOrigins as origin (origin.code)}
            <label class="inline">
              <input
                type="checkbox"
                checked={chosenOrigins.includes(origin.code)}
                onchange={(event) => toggleOrigin(origin.code, event.currentTarget.checked)}
              />
              <span>{tCode("water_origin", origin.code)}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <fieldset class="subsection">
        <legend>{t("irrigation.plots_section")}</legend>
        {#each plotRows as row, index (row)}
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
            <label>
              <span>{t("irrigation.area")}</span>
              <input type="number" step="any" min="0.0001" bind:value={row.areaHa} />
            </label>
            {#if plotRows.length > 1}
              <button type="button" class="btn-danger" onclick={() => plotRows.splice(index, 1)}>
                {t("treatment.remove")}
              </button>
            {/if}
          </div>
        {/each}
        <button type="button" onclick={() => plotRows.push(emptyPlotRow())}>
          {t("treatment.add_plot")}
        </button>
      </fieldset>

      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" onclick={() => (formOpen = false)}>{t("form.cancel")}</button>
      </div>
    </form>
  {/if}
{/if}
