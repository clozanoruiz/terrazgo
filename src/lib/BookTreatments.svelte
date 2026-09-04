<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, treatments tab: model section 3.1, the phytosanitary
  // actuations register. The shell owns the farm/season selectors and the
  // catalogue data; this component owns the register's own list and form.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TreatmentForm from "./TreatmentForm.svelte";
  import { draftFrom, emptyDraft } from "./treatmentDraft.js";
  import TzSelect from "./TzSelect.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import { codeItems } from "./selectItems.js";

  let {
    farmId,
    countryCode,
    seasonId,
    plots,
    crops,
    operators,
    machinery,
    products,
    advisors,
    treatments,
    onChanged,
  } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const units = $derived(lookups.units);
  const quantityUnits = $derived(lookups.quantityUnits);
  const intensityUnits = $derived(lookups.intensityUnits);
  const justifications = $derived(lookups.justifications);
  const efficacies = $derived(lookups.efficacies);
  const reasons = $derived(lookups.reasons);

  let treatmentFormOpen = $state(false);
  // The form's working state lives here, not in the form: a component keeps its
  // initial values for its whole life, so a form that copied the record at
  // creation would keep showing the first one the farmer opened. Refilling this
  // is what switching to another record to correct does.
  let draft = $state(emptyDraft());

  // Stored problems carry catalogue CODES (the regulatory payload); labels are
  // display metadata resolved from the catalogue, one fetch per category used.
  let problemLabels = $state({});

  // Resolved whenever the register changes: a newly saved record can target a
  // category none of the existing ones used.
  $effect(() => {
    const categories = new Set(
      treatments.flatMap(({ problems }) => problems.map((p) => p.reason_category_code)),
    );
    if (categories.size === 0 || !countryCode) return;
    run(async () => {
      const labels = {};
      for (const category of categories) {
        const codes = await invoke("list_problem_codes", {
          countryCode,
          reasonCategoryCode: category,
        });
        for (const code of codes) labels[`${category}:${code.code}`] = code.label;
      }
      problemLabels = labels;
    });
  });

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
      await onChanged();
    });
  }

  function deleteTreatment(record) {
    run(async () => {
      if (!(await confirmDialog(t("treatment.delete_confirm")))) return;
      await invoke("delete_treatment_record", { treatmentId: record.id });
      closeForm();
      await onChanged();
    });
  }

  /// Open the form on a stored record to correct it, or blank for a new entry.
  /// Safe to call while the form is already open — that is the point.
  function showForm(detail = null) {
    draft = detail ? draftFrom(detail) : emptyDraft();
    treatmentFormOpen = true;
  }

  async function treatmentSaved() {
    closeForm();
    await onChanged();
  }

  function closeForm() {
    treatmentFormOpen = false;
    draft = emptyDraft();
  }

  function plotName(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? plotId;
  }

  function treatedPlotsSummary(treatedPlots) {
    return treatedPlots
      .map((tp) => `${plotName(tp.plot_id)} (${formatNumber(tp.surface_treated_ha)} ha)`)
      .join(", ");
  }

  /// The dose cell. A purely non-chemical actuation has no product and so no
  /// dose; what it took instead is the measure's own intensity, which is a
  /// count of traps rather than a rate and is why the two are not one column.
  function doseCell(record) {
    if (record.product_id !== null) {
      return `${formatNumber(record.dose_value)} ${tCode("unit", record.dose_unit_code)}`;
    }
    if (record.measure_intensity_value === null) return "";
    // The count stays the RAW number: Intl.PluralRules selects on a number,
    // and the formatted string would not.
    return `${formatNumber(record.measure_intensity_value)} ${tCode(
      "unit",
      record.measure_intensity_unit_code,
      record.measure_intensity_value,
    )}`;
  }

  /// The date the model's 3.1 column asks for: a single day, or the interval
  /// Anexo III Parte I B allows when the actuation ran over several.
  function appliedOn(record) {
    const start = formatDate(record.application_date);
    return record.application_end_date
      ? `${start} – ${formatDate(record.application_end_date)}`
      : start;
  }

  // Entering a treatment needs a product and an operator to reference; the
  // hint sends the user to the catalogue view to create them.
  const missingRefs = $derived(products.length === 0 || operators.length === 0);

  /// The row the inspector is editing, so the delete button beside the form —
  /// and the efficacy control above it — know which record they are about.
  /// Null while entering a new one.
  const editing = $derived(treatments.find(({ record }) => record.id === draft.editingId) ?? null);
</script>

<div class="view-head">
  <h3>{t("treatments.records_title")}</h3>
  <!-- Always opens a blank form, never toggles the pane shut: with the entry
       form in an inspector, "new" beside a record being corrected means a new
       record, and the pane has a close button of its own. -->
  <button type="button" onclick={() => showForm()} disabled={missingRefs || plots.length === 0}>
    {t("treatments.new")}
  </button>
</div>
{#if missingRefs}
  <p>{t("treatments.missing_refs")} <a href="#/registry">{t("nav.registry")}</a></p>
{/if}

<TzWorkspace
  open={treatmentFormOpen}
  title={draft.editingId
    ? (editing?.record.product_name_snapshot ?? t("treatment.non_chemical"))
    : t("treatments.new")}
  onclose={closeForm}
  ondelete={editing ? () => deleteTreatment(editing.record) : null}
  deleteLabel={t("treatment.delete")}
>
  {#snippet list()}
    {#if treatments.length === 0}
      {#if !missingRefs}
        <p class="table-empty">{t("treatments.empty")}</p>
      {/if}
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"treatments"}>
          <thead>
            <tr>
              <th>{t("column.date")}</th>
              <th>{t("column.product")}</th>
              <th class="col-num">{t("column.dose")}</th>
              <th>{t("column.operator")}</th>
              <th>{t("column.plots")}</th>
              <th>{t("column.problems")}</th>
              <th>{t("column.phi_until")}</th>
              <th>{t("column.efficacy")}</th>
            </tr>
          </thead>
          <tbody>
            {#each treatments as entry (entry.record.id)}
              {@const record = entry.record}
              <tr
                class:selected={draft.editingId === record.id}
                onclick={(e) => opensRow(e) && showForm(entry)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showForm(entry)}>
                    {appliedOn(record)}
                  </button>
                </td>
                <!-- A purely non-chemical actuation has no product to name, so
                     the cell falls back to the measure it took. -->
                <td class="col-muted">
                  {record.product_name_snapshot ?? t("treatment.non_chemical")}
                </td>
                <td class="col-muted col-num">{doseCell(record)}</td>
                <td class="col-muted">{record.operator_name_snapshot}</td>
                <td class="col-muted">{treatedPlotsSummary(entry.plots)}</td>
                <td class="col-muted">{problemSummary(entry.problems)}</td>
                <td class="col-muted">
                  {record.phi_end_date === null ? "" : formatDate(record.phi_end_date)}
                </td>
                <td class="col-muted">
                  {record.efficacy_code
                    ? tCode("efficacy", record.efficacy_code)
                    : t("treatment.efficacy_pending")}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/snippet}

  {#snippet inspector(formId)}
    <!-- Efficacy is observed AFTER the application, so a correction never
         carries it: it has its own audited setter and saves on change. It used
         to sit on every row for want of anywhere else; the inspector names the
         record it is about, which is where it belonged. -->
    {#if editing}
      <div class="form-grid">
        <TzSelect
          label={t("treatment.efficacy")}
          hint={t("treatment.efficacy_hint")}
          items={codeItems(efficacies, "efficacy")}
          nullable
          nullLabel={t("treatment.efficacy_pending")}
          value={editing.record.efficacy_code ?? ""}
          onchange={(code) => setEfficacy(editing.record, code)}
        />
      </div>
    {/if}

    <TreatmentForm
      {draft}
      {farmId}
      {countryCode}
      {seasonId}
      {plots}
      {crops}
      {operators}
      {machinery}
      {products}
      {units}
      {quantityUnits}
      {intensityUnits}
      {advisors}
      {justifications}
      {efficacies}
      {reasons}
      onSaved={treatmentSaved}
      {formId}
    />
  {/snippet}

  {#snippet actions(formId)}
    <div class="form-actions">
      <button type="submit" form={formId}>{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={closeForm}>{t("form.cancel")}</button>
    </div>
  {/snippet}
</TzWorkspace>
