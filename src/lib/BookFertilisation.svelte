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
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import NumberInput from "./NumberInput.svelte";
  import BookPlan from "./BookPlan.svelte";
  import DateInput from "./DateInput.svelte";
  import TzCheckbox from "./TzCheckbox.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import { togglePractice } from "./practiceSelection.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId, seasonId, countryCode, plots, crops, machinery } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const fertilisationTypes = $derived(lookups.fertilisationTypes);
  const applicationMethods = $derived(lookups.applicationMethods);
  const doseUnits = $derived(lookups.doseUnits);

  let records = $state([]);
  let materials = $state([]);
  let practiceOptions = $state([]);
  let irrigations = $state([]);
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      [records, materials, practiceOptions, irrigations] = await Promise.all([
        invoke("list_fertilisation_records", { seasonId, farmId }),
        invoke("list_fertiliser_materials"),
        invoke("list_fertilisation_practices", { countryCode }),
        // Section 8's register, so a FERTIGATION can name the watering that
        // carried it. One act the decree records twice (arts. 5.d and 5.e);
        // only the farmer knows which watering it was.
        invoke("list_irrigation_records", { seasonId, farmId }),
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
  let sustainableInputs = $state(false);
  let irrigationRecordId = $state("");
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
    sustainableInputs = detail?.record.sustainable_input_management ?? false;
    irrigationRecordId = detail?.record.irrigation_record_id ?? "";
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

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// A single day, or the interval an application spread over several states.
  function appliedRange(record) {
    const start = formatDate(record.applied_on);
    return record.application_end_date
      ? `${start} – ${formatDate(record.application_end_date)}`
      : start;
  }

  /// Whether the chosen method is one of the two fertigation entries. Read from
  /// the lookup's own flag rather than matched on the code — the same source the
  /// repository and the printed model's "(F)" box use.
  const isFertigation = $derived(
    applicationMethods.some((row) => row.code === methodCode && row.is_fertigation),
  );

  /// The campaign's waterings, dated and named by what they covered: a farmer
  /// picks the one that carried the fertiliser by when it happened.
  const irrigationItems = $derived(
    irrigations.map(({ record, plots: watered }) => ({
      value: record.id,
      label: [formatDate(record.irrigated_on), watered.map((p) => plotName(p.plot_id)).join(", ")]
        .filter(Boolean)
        .join(" — "),
    })),
  );

  /// Claimed practices, summarised on the collapsed disclosure so the count is
  /// readable without opening a list of forty-one sentences.
  const practicesSummary = $derived(
    chosenPractices.length === 0
      ? t("fertilisation.practices_none")
      : t("fertilisation.practices_selected", { count: chosenPractices.length }),
  );

  /// Empty inputs become `null`, never 0: an unstated figure is unknown, and a
  /// zero would be a measurement the farmer never made.
  function optionalNumber(value) {
    return value === "" || value === null ? null : Number(value);
  }

  async function submit() {
    const payload = {
      applied_on: appliedOn,
      application_end_date: endDate || null,
      fertilisation_type_code: typeCode,
      application_method_code: methodCode,
      dose_value: Number(doseValue),
      dose_unit_code: doseUnit,
      fertiliser_material_id: materialId,
      sludge_application: sludge,
      sustainable_input_management: sustainableInputs,
      // Cleared with the method, so a correction that stops being a fertigation
      // cannot keep a link the repository would refuse anyway.
      irrigation_record_id: (isFertigation && irrigationRecordId) || null,
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
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("fertilisation.delete_confirm")))) return;
      await invoke("delete_fertilisation_record", { fertilisationRecordId: record.id });
      hideForm();
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
      .map(([symbol, value]) => `${symbol} ${formatNumber(value)}`)
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

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(appliedOn) : t("fertilisation.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"fertilisations"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.fertiliser")}</th>
                <th class="col-num">{t("column.dose")}</th>
                <th>{t("column.kind")}</th>
                <th>{t("column.method")}</th>
                <th>{t("column.richness")}</th>
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
                      {appliedRange(detail.record)}
                    </button>
                  </td>
                  <td class="col-muted">{detail.record.material_name_snapshot}</td>
                  <td class="col-muted col-num">
                    {t("fertilisation.dose_detail", {
                      dose: formatNumber(detail.record.dose_value),
                      unit: tCode("unit", detail.record.dose_unit_code),
                    })}
                  </td>
                  <td class="col-muted">
                    {tCode("fertilisation_type", detail.record.fertilisation_type_code)}
                  </td>
                  <td class="col-muted">
                    {tCode("application_method", detail.record.application_method_code)}
                  </td>
                  <td class="col-muted">{richness(detail.record)}</td>
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
          <NumberInput
            label={t("fertilisation.dose")}
            min={0.001}
            required
            bind:value={doseValue}
          />
          <TzSelect
            label={t("fertilisation.dose_unit")}
            items={codeItems(doseUnits, "unit")}
            bind:value={doseUnit}
          />
          <TextInput label={t("fertilisation.delivery_note")} bind:value={deliveryNote} />
          <TzSelect
            label={t("fertilisation.machinery")}
            hint={t("fertilisation.machinery_hint")}
            items={nameItems(machinery)}
            nullable
            bind:value={machineryId}
          />
          <NumberInput
            label={t("fertilisation.yield_estimated")}
            min={0}
            bind:value={yieldEstimated}
          />
          <NumberInput label={t("fertilisation.yield_final")} min={0} bind:value={yieldFinal} />
          <TextInput label={t("treatment.notes")} bind:value={notes} />
          <!-- Only a fertigation has a watering to name, and on any other method
             the link would assert a fertigation that did not happen. -->
          {#if isFertigation}
            <TzSelect
              label={t("fertilisation.irrigation_link")}
              hint={t("fertilisation.irrigation_link_hint")}
              items={irrigationItems}
              nullable
              nullLabel={t("fertilisation.irrigation_link_none")}
              bind:value={irrigationRecordId}
            />
          {/if}
          <TzCheckbox label={t("fertilisation.sludge")} bind:checked={sludge} />
          <TzCheckbox
            label={t("fertilisation.sustainable_inputs")}
            bind:checked={sustainableInputs}
          />
        </div>

        <fieldset class="subsection">
          <legend>{t("fertilisation.service_section")}</legend>
          <p class="detail">{t("fertilisation.service_hint")}</p>
          <div class="form-grid">
            <TextInput label={t("fertilisation.service_company")} bind:value={serviceCompany} />
            <TextInput label={t("fertilisation.service_regfer")} bind:value={serviceRegfer} />
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
              <NumberInput label={t("fertilisation.area")} min={0.0001} bind:value={row.areaHa} />
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

        <!-- Forty-one rows, each a sentence out of the FEGA catalogue, on a
           section the printed model does not carry at all — so it opens closed,
           with the claim count on the summary. Native <details>: no JS, no
           library, and it survives the production CSP because it needs neither
           (the AboutPanel precedent). -->
        {#if practiceOptions.length > 0}
          <fieldset class="subsection">
            <legend>{t("fertilisation.practices_section")}</legend>
            <details class="practices">
              <summary>{practicesSummary}</summary>
              <p class="detail">{t("fertilisation.practices_hint")}</p>
              <div class="checkbox-list stacked practices-list">
                {#each practiceOptions as practice (practice.code)}
                  <TzCheckbox
                    label={practice.name}
                    checked={chosenPractices.includes(practice.code)}
                    onchange={(next) =>
                      (chosenPractices = togglePractice(chosenPractices, practice.code, next))}
                  />
                {/each}
              </div>
            </details>
          </fieldset>
        {/if}
      </TzForm>
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
      </div>
    {/snippet}
  </TzWorkspace>

  <!-- Model section 7.1 lives under 6 in the same tab: the plan is what these
       applications are measured against, and putting the recommendation a
       click away from the register it judges would help nobody. -->
  <BookPlan {farmId} {seasonId} {crops} />
{/if}

<style>
  /* Capped so an optional forty-one-row section cannot push the save button off
     the screen; it scrolls inside itself instead. */
  .practices-list {
    max-height: 18rem;
    overflow-y: auto;
    padding-right: var(--space-2);
  }

  .practices summary {
    padding: var(--space-1) 0;
    color: var(--muted);
    font-size: 0.875rem;
    cursor: pointer;
  }
</style>
