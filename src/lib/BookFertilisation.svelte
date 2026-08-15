<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, fertilisation tab: model section 6.
  //
  // The record book's SECOND decree, and the half that names it: RD 1051/2022
  // art. 5.d makes this binding since 1 January 2026, recorded within one month
  // of each operation. The binding field list is not the printed model — art.
  // 5.d redirects to RD 1311/2012 Anexo III Parte I sección C, which is wider.
  //
  // Three things this form asks that the printed model does not:
  //
  //   * the forma de aplicación (C.f) as its own field. The model's
  //     "(F)/(AF)/(AC)" footnote merges it with the tipo de fertilización
  //     (C.c), but fertirrigación is a way of applying, not a kind of
  //     fertilisation — a farmer can perfectly well fertigate a cobertera. The
  //     book derives the model's single letter at print time.
  //   * the service company and its REGFER number (C.k), a third machinery
  //     registry beside ROMA and REGANIP.
  //   * the good practices the SIEX twin requires. Optional here, because the
  //     printed model has no column for them and the decree puts them in its
  //     anexo V rather than in the register's field list.
  //
  // The material's full composition is NOT asked here: it belongs to the
  // material, which is registered once in the catalogue and reused. Each record
  // freezes only what section 6 prints — the name, the coded kind and the
  // N/P₂O₅/K₂O richness.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import BookPlan from "./BookPlan.svelte";
  import DateInput from "./DateInput.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";

  let { farmId, seasonId, countryCode, plots, crops, machinery } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const fertilisationTypes = $derived(lookups.fertilisationTypes);
  const applicationMethods = $derived(lookups.applicationMethods);
  const doseUnits = $derived(lookups.doseUnits);

  let records = $state([]);
  let materials = $state([]);
  let practiceOptions = $state([]);
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      [records, materials, practiceOptions] = await Promise.all([
        invoke("list_fertilisation_records", { seasonId, farmId }),
        invoke("list_fertiliser_materials"),
        invoke("list_fertilisation_practices", { countryCode }),
      ]);
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
  let appliedOn = $state("");
  let endDate = $state("");
  let materialId = $state("");
  let typeCode = $state("");
  let methodCode = $state("");
  let doseValue = $state("");
  let doseUnit = $state("kg_ha");
  let sludge = $state(false);
  let machineryId = $state("");
  let serviceCompany = $state("");
  let serviceRegfer = $state("");
  let deliveryNote = $state("");
  let yieldEstimated = $state("");
  let yieldFinal = $state("");
  let notes = $state("");
  let plotRows = $state([emptyPlotRow()]);
  let chosenPractices = $state([]);

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    appliedOn = detail?.record.applied_on ?? "";
    endDate = detail?.record.application_end_date ?? "";
    materialId = detail?.record.fertiliser_material_id ?? materials[0]?.material.id ?? "";
    typeCode = detail?.record.fertilisation_type_code ?? fertilisationTypes[0]?.code ?? "";
    methodCode = detail?.record.application_method_code ?? applicationMethods[0]?.code ?? "";
    doseValue = detail?.record.dose_value ?? "";
    doseUnit = detail?.record.dose_unit_code ?? "kg_ha";
    sludge = detail?.record.sludge_application ?? false;
    machineryId = detail?.record.machinery_id ?? "";
    serviceCompany = detail?.record.service_company ?? "";
    serviceRegfer = detail?.record.service_regfer_number ?? "";
    deliveryNote = detail?.record.delivery_note_ref ?? "";
    yieldEstimated = detail?.record.yield_estimated_kg_ha ?? "";
    yieldFinal = detail?.record.yield_final_kg_ha ?? "";
    notes = detail?.record.notes ?? "";
    plotRows = detail?.plots.length
      ? detail.plots.map((p) => ({
          plotId: p.plot_id,
          cropId: p.crop_id ?? "",
          areaHa: p.fertilised_area_ha ?? "",
        }))
      : [emptyPlotRow()];
    chosenPractices = [...(detail?.practices ?? [])];
    formOpen = true;
  }

  function togglePractice(code, checked) {
    chosenPractices = checked
      ? [...chosenPractices, code]
      : chosenPractices.filter((existing) => existing !== code);
  }

  /// Empty inputs become `null`, never 0: an unstated figure is unknown, and a
  /// zero would be a measurement the farmer never made.
  function optionalNumber(value) {
    return value === "" || value === null ? null : Number(value);
  }

  function submit(event) {
    event.preventDefault();
    const payload = {
      applied_on: appliedOn,
      application_end_date: endDate || null,
      fertilisation_type_code: typeCode,
      application_method_code: methodCode,
      dose_value: Number(doseValue),
      dose_unit_code: doseUnit,
      fertiliser_material_id: materialId,
      sludge_application: sludge,
      machinery_id: machineryId || null,
      service_company: serviceCompany.trim() || null,
      service_regfer_number: serviceRegfer.trim() || null,
      delivery_note_ref: deliveryNote.trim() || null,
      yield_estimated_kg_ha: optionalNumber(yieldEstimated),
      yield_final_kg_ha: optionalNumber(yieldFinal),
      notes: notes.trim() || null,
      plots: plotRows
        .filter((row) => row.plotId)
        .map((row) => ({
          plot_id: row.plotId,
          crop_id: row.cropId || null,
          fertilised_area_ha: optionalNumber(row.areaHa),
        })),
      practices: chosenPractices,
    };

    run(async () => {
      if (editingId) {
        await invoke("update_fertilisation_record", {
          fertilisationRecordId: editingId,
          update: { ...payload, id: editingId },
        });
      } else {
        await invoke("create_fertilisation_record", {
          record: { ...payload, season_id: seasonId, farm_id: farmId },
        });
      }
      notify(t("message.fertilisation_saved"));
      formOpen = false;
      load();
    });
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("fertilisation.delete_confirm")))) return;
      await invoke("delete_fertilisation_record", { fertilisationRecordId: record.id });
      notify(t("message.fertilisation_deleted"));
      load();
    });
  }

  /// The three figures the record froze, as the model's "Riqueza N/P/K" cell.
  /// An unstated one contributes nothing: a printed 0 would claim the material
  /// contains none of it.
  function richness(record) {
    return [
      ["N", record.richness_n_snapshot],
      ["P₂O₅", record.richness_p2o5_snapshot],
      ["K₂O", record.richness_k2o_snapshot],
    ]
      .filter(([, value]) => value !== null && value !== undefined)
      .map(([symbol, value]) => `${symbol} ${value}`)
      .join(" · ");
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <div class="view-head">
    <h3>{t("fertilisation.title")}</h3>
    <div class="selector-buttons">
      <button
        type="button"
        disabled={plots.length === 0 || materials.length === 0}
        onclick={() => showForm()}
      >
        + {t("fertilisation.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("fertilisation.intro")}</p>
  {#if materials.length === 0}
    <p class="detail">{t("fertilisation.no_materials")}</p>
  {/if}

  <ul class="card-list">
    {#each records as detail (detail.record.id)}
      <li class="card">
        <div class="stack">
          <strong>
            {formatDate(detail.record.applied_on)}{detail.record.application_end_date
              ? ` – ${formatDate(detail.record.application_end_date)}`
              : ""}
            — {detail.record.material_name_snapshot}
          </strong>
          <span class="detail">
            {t("fertilisation.dose_detail", {
              dose: detail.record.dose_value,
              unit: tCode("unit", detail.record.dose_unit_code),
            })}
            · {tCode("fertilisation_type", detail.record.fertilisation_type_code)}
            · {tCode("application_method", detail.record.application_method_code)}
            {#if richness(detail.record)}
              · {richness(detail.record)}
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
        <DateInput label={t("fertilisation.applied_on")} required bind:value={appliedOn} />
        <DateInput
          label={t("fertilisation.end_date")}
          hint={t("fertilisation.end_date_hint")}
          bind:value={endDate}
        />
        <TzSelect
          label={t("fertilisation.material")}
          items={nameItems(
            materials,
            (d) => d.material.name,
            (d) => d.material.id,
          )}
          required
          bind:value={materialId}
        />
        <TzSelect
          label={t("fertilisation.type")}
          items={codeItems(fertilisationTypes, "fertilisation_type")}
          required
          bind:value={typeCode}
        />
        <TzSelect
          label={t("fertilisation.method")}
          items={codeItems(applicationMethods, "application_method")}
          required
          bind:value={methodCode}
        />
        <label>
          <span>{t("fertilisation.dose")}</span>
          <input type="number" step="any" min="0.001" required bind:value={doseValue} />
        </label>
        <TzSelect
          label={t("fertilisation.dose_unit")}
          items={codeItems(doseUnits, "unit")}
          bind:value={doseUnit}
        />
        <label>
          <span>{t("fertilisation.delivery_note")}</span>
          <input bind:value={deliveryNote} />
        </label>
        <TzSelect
          label={t("fertilisation.machinery")}
          hint={t("fertilisation.machinery_hint")}
          items={nameItems(machinery)}
          nullable
          bind:value={machineryId}
        />
        <label>
          <span>{t("fertilisation.yield_estimated")}</span>
          <input type="number" step="any" min="0" bind:value={yieldEstimated} />
        </label>
        <label>
          <span>{t("fertilisation.yield_final")}</span>
          <input type="number" step="any" min="0" bind:value={yieldFinal} />
        </label>
        <label>
          <span>{t("treatment.notes")}</span>
          <input bind:value={notes} />
        </label>
        <label class="inline">
          <input type="checkbox" bind:checked={sludge} />
          <span>{t("fertilisation.sludge")}</span>
        </label>
      </div>

      <fieldset class="subsection">
        <legend>{t("fertilisation.service_section")}</legend>
        <p class="detail">{t("fertilisation.service_hint")}</p>
        <div class="form-grid">
          <label>
            <span>{t("fertilisation.service_company")}</span>
            <input bind:value={serviceCompany} />
          </label>
          <label>
            <span>{t("fertilisation.service_regfer")}</span>
            <input bind:value={serviceRegfer} />
          </label>
        </div>
      </fieldset>

      <fieldset class="subsection">
        <legend>{t("fertilisation.plots_section")}</legend>
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
              <span>{t("fertilisation.area")}</span>
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

      {#if practiceOptions.length > 0}
        <fieldset class="subsection">
          <legend>{t("fertilisation.practices_section")}</legend>
          <p class="detail">{t("fertilisation.practices_hint")}</p>
          <div class="checkbox-list">
            {#each practiceOptions as practice (practice.code)}
              <label class="inline">
                <input
                  type="checkbox"
                  checked={chosenPractices.includes(practice.code)}
                  onchange={(event) => togglePractice(practice.code, event.currentTarget.checked)}
                />
                <span>{practice.name}</span>
              </label>
            {/each}
          </div>
        </fieldset>
      {/if}

      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" onclick={() => (formOpen = false)}>{t("form.cancel")}</button>
      </div>
    </form>
  {/if}

  <!-- Model section 7.1 lives under 6 in the same tab: the plan is what these
       applications are measured against, and putting the recommendation a
       click away from the register it judges would help nobody. -->
  <BookPlan {farmId} {seasonId} {crops} />
{/if}
