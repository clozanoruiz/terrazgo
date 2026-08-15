<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, treatments tab: model section 3.1, the phytosanitary
  // actuations register. The shell owns the farm/season selectors and the
  // catalogue data; this component owns the register's own list and form.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TreatmentForm from "./TreatmentForm.svelte";
  import { draftFrom, emptyDraft } from "./treatmentDraft.js";
  import TzSelect from "./TzSelect.svelte";
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
      notify(t("message.treatment_deleted"));
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
      .map((tp) => `${plotName(tp.plot_id)} (${tp.surface_treated_ha} ha)`)
      .join(", ");
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
</script>

<div class="view-head">
  <h3>{t("treatments.records_title")}</h3>
  <button
    type="button"
    onclick={() => (treatmentFormOpen ? closeForm() : showForm())}
    disabled={missingRefs || plots.length === 0}
  >
    {t("treatments.new")}
  </button>
</div>
{#if missingRefs}
  <p>{t("treatments.missing_refs")} <a href="#/registry">{t("nav.registry")}</a></p>
{/if}

{#if treatmentFormOpen}
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
    onCancel={closeForm}
  />
{/if}

<ul class="card-list">
  {#each treatments as { record, plots: treatedPlots, problems, justifications: recordJustifications } (record.id)}
    <li class="card">
      <div class="stack">
        <!-- A purely non-chemical actuation has no product to name, so the
             heading falls back to the measure it took. -->
        <strong>
          {appliedOn(record)} — {record.product_name_snapshot ?? t("treatment.non_chemical")}
        </strong>
        <span class="detail">
          {#if record.product_id !== null}
            {record.dose_value}
            {tCode("unit", record.dose_unit_code)}
          {/if}
          {#if record.total_quantity_value !== null}
            · {t("treatment.total_quantity_detail", {
              value: record.total_quantity_value,
              unit: tCode("unit", record.total_quantity_unit_code),
            })}
          {/if}
          ·
          {record.operator_name_snapshot}
        </span>
        <span class="detail">{problemSummary(problems)}</span>
        <span class="detail">
          {recordJustifications.map((j) => tCode("justification", j.justification_code)).join(", ")}
        </span>
        <span class="detail">{treatedPlotsSummary(treatedPlots)}</span>
        {#if record.phi_end_date !== null}
          <span class="detail">
            {t("treatment.phi_until", { date: formatDate(record.phi_end_date) })}
          </span>
        {/if}
        {#if record.advisor_name_snapshot !== null || record.measure_code !== null}
          <!-- What model 3.1 bis prints: who advised, and what was tried
               instead of a spray. -->
          <span class="detail">
            {[
              record.advisor_name_snapshot,
              record.measure_intensity_value !== null
                ? t("treatment.measure_intensity_detail", {
                    value: record.measure_intensity_value,
                    unit: tCode("unit", record.measure_intensity_unit_code),
                  })
                : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </span>
        {/if}
        <TzSelect
          class="inline-field"
          label={t("treatment.efficacy")}
          items={codeItems(efficacies, "efficacy")}
          nullable
          nullLabel={t("treatment.efficacy_pending")}
          value={record.efficacy_code ?? ""}
          onchange={(code) => setEfficacy(record, code)}
        />
      </div>
      <button
        type="button"
        onclick={() =>
          showForm({
            record,
            plots: treatedPlots,
            problems,
            justifications: recordJustifications,
          })}
      >
        {t("form.edit")}
      </button>
      <button type="button" class="btn-danger" onclick={() => deleteTreatment(record)}>
        {t("treatment.delete")}
      </button>
    </li>
  {/each}
</ul>
{#if treatments.length === 0 && !missingRefs}
  <p>{t("treatments.empty")}</p>
{/if}
