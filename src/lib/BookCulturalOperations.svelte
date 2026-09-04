<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, eco-schemes tab: model 9.2 and the book's "9.6".
  //
  // One register behind two printed pages. RD 1048/2022 art. 31 asks for "la
  // fecha y las actividades realizadas" on a P2 plot within a month, and anexo
  // IV asks the same of each pasto comunal plot — a duty the printed model
  // gives NO page to, which is why the book numbers one 9.6 itself.
  //
  // The practice is therefore not decoration: it decides which page the record
  // prints on. The selector offers the two duties that have a page today; the
  // cover and flooded-crop practices are recordable through the backend and
  // gain their forms with the seams that give them pages.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import DateInput from "./DateInput.svelte";
  import TzCombobox from "./TzCombobox.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId, seasonId, countryCode, plots } = $props();

  // The duties this form files against — the two section 9 prints as pages of
  // its own today. The backend accepts five (every practice but extensive
  // grazing, whose duty is the grazing dates and whose register is 9.1).
  const FORM_PRACTICES = ["sustainable_mowing", "communal_pasture"];
  const practices = $derived(
    lookups.ecoPractices.filter((practice) => FORM_PRACTICES.includes(practice.code)),
  );

  let records = $state([]);
  // The residue destinations take a country, so they are per-holding reference
  // data and load here rather than in the session-wide store.
  let residueDestinations = $state([]);
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      const [loaded, destinations] = await Promise.all([
        invoke("list_cultural_operations", { seasonId, farmId }),
        invoke("list_residue_destinations", { countryCode }),
      ]);
      records = loaded;
      residueDestinations = destinations;
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  let practiceCode = $state("sustainable_mowing");
  let operationKindCode = $state("mowing");
  let performedOn = $state("");
  let performedEndDate = $state("");
  let activityDescription = $state("");
  let residueDestinationCode = $state("");
  let notes = $state("");
  let chosenPlots = $state([]);

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    practiceCode = detail?.record.practice_code ?? "sustainable_mowing";
    operationKindCode = detail?.record.operation_kind_code ?? "mowing";
    performedOn = detail?.record.performed_on ?? "";
    performedEndDate = detail?.record.performed_end_date ?? "";
    activityDescription = detail?.record.activity_description ?? "";
    residueDestinationCode = detail?.record.residue_destination_code ?? "";
    notes = detail?.record.notes ?? "";
    chosenPlots = detail?.plots.map((p) => p.plot_id) ?? [];
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// A single day, or the interval a job spread over several states.
  function performedRange(record) {
    const start = formatDate(record.performed_on);
    return record.performed_end_date
      ? `${start} – ${formatDate(record.performed_end_date)}`
      : start;
  }

  function togglePlot(plotId, checked) {
    chosenPlots = checked
      ? [...chosenPlots, plotId]
      : chosenPlots.filter((existing) => existing !== plotId);
  }

  async function submit() {
    const payload = {
      practice_code: practiceCode,
      operation_kind_code: operationKindCode,
      performed_on: performedOn,
      // An empty end date is a single day's work, which is what the register
      // wants to say — never the start date repeated, which would claim an
      // interval nobody stated.
      performed_end_date: performedEndDate || null,
      activity_description: activityDescription.trim() || null,
      residue_destination_code: residueDestinationCode || null,
      notes: notes.trim() || null,
      plot_ids: chosenPlots,
    };

    if (editingId) {
      await invoke("update_cultural_operation", {
        culturalOperationId: editingId,
        update: payload,
      });
    } else {
      await invoke("create_cultural_operation", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("operation.delete_confirm")))) return;
      await invoke("delete_cultural_operation", { culturalOperationId: record.id });
      hideForm();
      load();
    });
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <div class="view-head">
    <h3>{t("operation.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showForm()}>
        + {t("operation.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("operation.intro")}</p>

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(performedOn) : t("operation.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"cultural-operations"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.operation")}</th>
                <th>{t("column.practice")}</th>
                <th>{t("column.description")}</th>
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
                      {performedRange(detail.record)}
                    </button>
                  </td>
                  <td class="col-muted">
                    {tCode("cultural_operation_kind", detail.record.operation_kind_code)}
                  </td>
                  <td class="col-muted">{tCode("eco_practice", detail.record.practice_code)}</td>
                  <td class="col-muted">{detail.record.activity_description ?? ""}</td>
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
          <TzSelect
            label={t("operation.practice")}
            hint={t("operation.practice_hint")}
            items={codeItems(practices, "eco_practice")}
            required
            bind:value={practiceCode}
          />
          <TzSelect
            label={t("operation.kind")}
            items={codeItems(lookups.culturalOperationKinds, "cultural_operation_kind")}
            required
            bind:value={operationKindCode}
          />
          <DateInput label={t("operation.performed_on")} required bind:value={performedOn} />
          <DateInput
            label={t("operation.performed_end_date")}
            hint={t("operation.performed_end_date_hint")}
            bind:value={performedEndDate}
          />
          <TextInput label={t("operation.activity_description")} bind:value={activityDescription}>
            <small class="detail">{t("operation.activity_description_hint")}</small>
          </TextInput>
          <TzCombobox
            label={t("operation.residue_destination")}
            hint={t("operation.residue_destination_hint")}
            items={nameItems(
              residueDestinations,
              (entry) => entry.name,
              (entry) => entry.code,
            )}
            bind:value={residueDestinationCode}
          />
          <TextInput label={t("treatment.notes")} bind:value={notes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("operation.plots_section")}</legend>
          <div class="checkbox-list">
            {#each plots as entry (entry.plot.id)}
              <TzCheckbox
                label={entry.plot.name}
                checked={chosenPlots.includes(entry.plot.id)}
                onchange={(next) => togglePlot(entry.plot.id, next)}
              />
            {/each}
          </div>
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
