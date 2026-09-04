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
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import DateInput from "./DateInput.svelte";
  import NumberInput from "./NumberInput.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

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

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// A single day, or the interval a watering that ran over several states.
  function irrigatedRange(record) {
    const start = formatDate(record.irrigated_on);
    return record.irrigation_end_date
      ? `${start} – ${formatDate(record.irrigation_end_date)}`
      : start;
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

  async function submit() {
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
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("irrigation.delete_confirm")))) return;
      await invoke("delete_irrigation_record", { irrigationRecordId: record.id });
      hideForm();
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

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(irrigatedOn) : t("irrigation.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"irrigations"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.method")}</th>
                <th class="col-num">{t("column.volume_applied")}</th>
                <th>{t("column.origin")}</th>
                <th>{t("column.plots")}</th>
              </tr>
            </thead>
            <tbody>
              {#each records as detail (detail.record.id)}
                <tr
                  class:selected={editingId === detail.record.id}
                  onclick={(e) => opensRow(e) && showForm(detail)}
                >
                  <td class="col-name">
                    <button type="button" class="row-open" onclick={() => showForm(detail)}>
                      {irrigatedRange(detail.record)}
                    </button>
                  </td>
                  <td class="col-muted">
                    {tCode("irrigation_method", detail.record.irrigation_method_code)}
                  </td>
                  <td class="col-muted col-num">
                    {t("irrigation.volume_detail", {
                      volume: formatNumber(detail.record.volume_value),
                      unit: tCode("unit", detail.record.volume_unit_code),
                    })}
                  </td>
                  <td class="col-muted">
                    {detail.water_origins.map((code) => tCode("water_origin", code)).join(", ")}
                  </td>
                  <td class="col-muted"
                    >{detail.plots.map((p) => plotName(p.plot_id)).join(", ")}</td
                  >
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      <TzForm id={formId} onsubmit={submit}>
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
          <NumberInput
            label={t("irrigation.volume")}
            min={0.001}
            required
            bind:value={volumeValue}
          />
          <TzSelect
            label={t("irrigation.volume_unit")}
            items={codeItems(volumeUnits, "unit")}
            bind:value={volumeUnit}
          />
          <TextInput label={t("irrigation.meter_number")} bind:value={meterNumber} />
          <TextInput label={t("treatment.notes")} bind:value={notes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("irrigation.water_section")}</legend>
          <p class="detail">{t("irrigation.water_hint")}</p>
          <div class="form-grid">
            <NumberInput label={t("irrigation.nitric_n")} min={0} bind:value={nitricN} />
            <NumberInput label={t("irrigation.soluble_p2o5")} min={0} bind:value={solubleP2o5} />
          </div>
          <div class="checkbox-list">
            {#each waterOrigins as origin (origin.code)}
              <TzCheckbox
                label={tCode("water_origin", origin.code)}
                checked={chosenOrigins.includes(origin.code)}
                onchange={(next) => toggleOrigin(origin.code, next)}
              />
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
              <NumberInput label={t("irrigation.area")} min={0.0001} bind:value={row.areaHa} />
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
      </TzForm>
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
      </div>
    {/snippet}
  </TzWorkspace>
{/if}
