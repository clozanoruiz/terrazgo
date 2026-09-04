<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Treatment entry form — the CUE module's central input (RD 1311/2012
  // mandatory fields). Multi-plot rows are dynamic; the legal snapshots, the
  // country and the PHI end date are derived in Rust at insert time, not here.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import { emptyProblemRow, emptyRow } from "./treatmentDraft.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import DateInput from "./DateInput.svelte";
  import TimeInput from "./TimeInput.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TzCombobox from "./TzCombobox.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";

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
    // The working state, owned by the register view: blank for a new entry,
    // filled from the stored record for a correction (see treatmentDraft.js).
    // This form is a view over it and holds no copy of its own — so switching
    // to a different record to correct refills the fields on screen.
    draft,
    onSaved,
    /// Names this form so the register's pinned Save can claim it. The panel
    /// owns the action bar now, and this is the longest form in the app — the
    /// one whose Save was furthest out of reach.
    formId = "",
  } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const units = $derived(lookups.units);
  const quantityUnits = $derived(lookups.quantityUnits);
  const intensityUnits = $derived(lookups.intensityUnits);
  const justifications = $derived(lookups.justifications);
  const efficacies = $derived(lookups.efficacies);
  const reasons = $derived(lookups.reasons);

  let measures = $state([]);
  let growthStages = $state([]);

  // Official problem catalogues per category, fetched once per category used
  // (600-entry lists — never re-fetched while the form is open). A category
  // present with an empty list has been asked for and has not arrived yet,
  // which is what keeps two rows sharing a category from fetching it twice.
  let problemCatalogues = $state({});

  // Fetch the catalogue behind every category the draft names. At mount that is
  // a correction's stored problems; later it is rows the farmer adds or changes.
  $effect(() => {
    for (const row of draft.problemRows) loadProblemCatalogue(row.category);
  });

  // Prefill the applicator from the active profile's linked operator (the
  // user_profile.operator_id link, the convention that the applicator records
  // their own treatment). A convenience default only: silently skipped when
  // there is no active profile, no link, or the linked operator is missing
  // from the picker — and never overriding a choice already made.
  (async () => {
    try {
      const [info, profiles] = await Promise.all([
        invoke("get_settings"),
        invoke("list_user_profiles"),
      ]);
      const active = profiles.find((profile) => profile.id === info.settings.active_user_id);
      const linked = active?.operator_id;
      if (linked && !draft.operatorId && operators.some((operator) => operator.id === linked)) {
        draft.operatorId = linked;
      }
    } catch (err) {
      console.error(err); // prefill must never block treatment entry
    }
  })();

  // The fourteen non-chemical measures (TIPO_MEDIDA_FITOSANITARIA). A closed
  // list, so a plain select; failure leaves it empty rather than blocking
  // treatment entry, like the prefill above.
  (async () => {
    try {
      measures = await invoke("list_measures", { countryCode });
    } catch (err) {
      console.error(err);
    }
  })();

  // The BBCH monograph's ten principal growth stages (EST_FENOLOGICO). Also a
  // closed list, so also a plain select — and the names already carry the BBCH
  // number, which the catalogue's own code is not.
  (async () => {
    try {
      growthStages = await invoke("list_growth_stages", { countryCode });
    } catch (err) {
      console.error(err);
    }
  })();

  /// A new category invalidates the problem chosen under the old one; the
  /// effect above fetches whatever catalogue the new one needs.
  function onCategoryChosen(row) {
    row.code = "";
  }

  /// Fetch a category's catalogue once, so its picker shows labels rather than
  /// empty boxes. The claim is staked before awaiting, so the effect above can
  /// re-run freely while a fetch is in flight.
  function loadProblemCatalogue(category) {
    if (!category || category in problemCatalogues) return;
    problemCatalogues = { ...problemCatalogues, [category]: [] };
    run(async () => {
      const codes = await invoke("list_problem_codes", {
        countryCode,
        reasonCategoryCode: category,
      });
      problemCatalogues = { ...problemCatalogues, [category]: codes };
    });
  }

  // The combobox narrows the list itself (folded, ranked and capped in
  // lib/collate.js), so this only shapes the category's codes into items —
  // the hand-rolled filter it replaces lived in a second input.
  function problemItems(row) {
    return (problemCatalogues[row.category] ?? []).map((code) => ({
      value: code.code,
      label: code.label,
    }));
  }

  function addProblemRow() {
    draft.problemRows.push(emptyProblemRow());
  }

  function removeProblemRow(index) {
    draft.problemRows.splice(index, 1);
  }

  // Shown as a hint so the farmer knows what leaving PHI blank means.
  const defaultPhi = $derived(
    products.find((p) => p.id === draft.productId)?.default_phi_days ?? null,
  );

  // A per-hectare dose times the treated surface IS the total used, so offer
  // it. A concentration dose (g/l, ml/l, %) says nothing about how much spray
  // was mixed — which is exactly why the column exists — so nothing is offered
  // there and the farmer states the figure.
  const PER_HECTARE_TOTAL = { l_ha: "l", kg_ha: "kg" };

  const suggestedTotal = $derived.by(() => {
    const unit = PER_HECTARE_TOTAL[draft.doseUnit];
    const dose = Number(draft.doseValue);
    if (!unit || !(dose > 0)) return null;
    const surfaces = draft.rows.map((row) => Number(row.surface));
    if (surfaces.length === 0 || surfaces.some((s) => !(s > 0))) return null;
    const total = dose * surfaces.reduce((sum, s) => sum + s, 0);
    // Trim float noise (1.5 × 3.2 = 4.800000000000001) without pretending to
    // more precision than the inputs carry. Rounded arithmetic rather than
    // toFixed: this produces the NUMBER the field stores, and toFixed produces
    // a dot-decimal string — the confusion the two are worth keeping apart.
    return { value: Math.round(total * 1e4) / 1e4, unit };
  });

  function applySuggestedTotal() {
    if (!suggestedTotal) return;
    draft.totalQuantity = suggestedTotal.value;
    draft.totalQuantityUnit = suggestedTotal.unit;
  }

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
    draft.rows.push(emptyRow());
  }

  function removeRow(index) {
    draft.rows.splice(index, 1);
  }

  async function submit() {
    const fields = {
      application_date: draft.applicationDate,
      application_end_date: draft.applicationEndDate || null,
      // Local wall-clock HH:MM, sent as typed. Never "" — an empty string is
      // not an hour, and the backend would refuse it as malformed.
      application_time: draft.applicationTime || null,
      // Model 9.3's "fecha de seca" (RD 1048/2022 art. 45.2): the day a flooded
      // field was dried so this treatment could be applied. Empty means the
      // ordinary case — a crop that is not grown under water.
      drying_date: draft.dryingDate || null,
      // The chemical block travels whole or not at all: a purely non-chemical
      // actuation states a measure instead, and the backend refuses halves.
      product_id: draft.productId || null,
      dose_value: draft.productId ? Number(draft.doseValue) : null,
      dose_unit_code: draft.productId ? draft.doseUnit : null,
      // Both parts travel together or neither does; the backend rejects halves.
      total_quantity_value: draft.totalQuantity === "" ? null : Number(draft.totalQuantity),
      total_quantity_unit_code: draft.totalQuantity === "" ? null : draft.totalQuantityUnit,
      target_organism: draft.targetOrganism.trim() || null,
      problems: draft.problemRows
        .filter((row) => row.category && row.code)
        .map((row) => ({ reason_category_code: row.category, problem_code: row.code })),
      justifications: [...draft.checkedJustifications],
      operator_id: draft.operatorId,
      machinery_id: draft.machineryId || null,
      phi_days_used: String(draft.phiDays).trim() === "" ? null : Number(draft.phiDays),
      advisor_id: draft.advisorId || null,
      measure_code: draft.measureCode || null,
      // Value and unit together or neither, like every other amount here.
      measure_intensity_value:
        draft.measureIntensity === "" ? null : Number(draft.measureIntensity),
      measure_intensity_unit_code:
        draft.measureIntensity === "" ? null : draft.measureIntensityUnit,
      measure_registration_number: draft.measureRegistration.trim() || null,
      notes: draft.notes.trim() || null,
    };
    const treatedPlots = draft.rows.map((row) => ({
      plot_id: row.plotId,
      crop_id: row.cropId || null,
      surface_treated_ha: Number(row.surface),
      growth_stage_code: row.growthStage || null,
    }));
    if (draft.editingId) {
      // A correction carries neither campaign nor holding (a treatment never
      // moves either) and no efficacy — that keeps its own control in the
      // list, because it is observed after the fact.
      await invoke("update_treatment_record", {
        treatmentId: draft.editingId,
        update: { ...fields, plots: treatedPlots },
      });
    } else {
      const saved = await invoke("create_treatment_record", {
        record: {
          ...fields,
          season_id: seasonId,
          farm_id: farmId,
          country_code: null, // derived from the farm in Rust
          efficacy_code: draft.efficacyCode || null,
        },
        plots: treatedPlots,
      });
      notify(t("message.treatment_saved", { date: formatDate(saved.phi_end_date) }));
    }
    await onSaved();
  }
</script>

<TzForm
  id={formId}
  onsubmit={submit}
  anchors={{
    "invalid.end_date_before_start": "application_end_date",
    "invalid.dose_without_product": "dose_value",
    "invalid.product_without_dose": "dose_value",
    "invalid.invalid_dose": "dose_value",
    "invalid.invalid_total_quantity": "total_quantity_value",
    "invalid.quantity_unit_mismatch": "total_quantity_value",
    "invalid.application_time": "application_time",
  }}
>
  <div class="form-grid">
    <DateInput
      label={t("treatment.date")}
      name="application_date"
      required
      bind:value={draft.applicationDate}
    />
    <DateInput
      label={t("treatment.end_date")}
      name="application_end_date"
      hint={t("treatment.end_date_hint")}
      min={draft.applicationDate}
      bind:value={draft.applicationEndDate}
    />
    <TimeInput
      label={t("treatment.time")}
      name="application_time"
      hint={t("treatment.time_hint")}
      bind:value={draft.applicationTime}
    />
    <DateInput
      label={t("treatment.drying_date")}
      hint={t("treatment.drying_date_hint")}
      bind:value={draft.dryingDate}
    />
    <TzSelect
      label={t("treatment.product")}
      hint={t("treatment.product_hint")}
      items={nameItems(products, (p) => p.commercial_name)}
      nullable
      nullLabel=""
      bind:value={draft.productId}
    />
    <NumberInput
      label={t("treatment.dose")}
      name="dose_value"
      min={0.001}
      required={draft.productId !== ""}
      disabled={draft.productId === ""}
      bind:value={draft.doseValue}
    />
    <TzSelect
      label={t("treatment.unit")}
      items={codeItems(units, "unit")}
      required={draft.productId !== ""}
      disabled={draft.productId === ""}
      bind:value={draft.doseUnit}
    />
    <NumberInput
      label={t("treatment.total_quantity")}
      name="total_quantity_value"
      min={0.0001}
      bind:value={draft.totalQuantity}
    >
      {#if suggestedTotal && Number(draft.totalQuantity) !== suggestedTotal.value}
        <small>
          <button type="button" class="link-button" onclick={applySuggestedTotal}>
            {t("treatment.total_quantity_suggest", {
              value: formatNumber(suggestedTotal.value),
              unit: tCode("unit", suggestedTotal.unit),
            })}
          </button>
        </small>
      {:else}
        <small>{t("treatment.total_quantity_hint")}</small>
      {/if}
    </NumberInput>
    <TzSelect
      label={t("treatment.total_quantity_unit")}
      items={codeItems(quantityUnits, "unit")}
      disabled={draft.totalQuantity === ""}
      bind:value={draft.totalQuantityUnit}
    />
    <TextInput label={t("treatment.target")} bind:value={draft.targetOrganism} />
    <TzSelect
      label={t("treatment.operator")}
      items={nameItems(operators, (o) => o.full_name)}
      required
      bind:value={draft.operatorId}
    />
    <TzSelect
      label={t("treatment.machinery")}
      items={nameItems(machinery)}
      nullable
      nullLabel={t("treatment.machinery_none")}
      bind:value={draft.machineryId}
    />
    <NumberInput
      label={t("treatment.phi_days")}
      hint={defaultPhi != null ? t("treatment.phi_default", { count: defaultPhi }) : ""}
      integer
      min={0}
      placeholder={defaultPhi != null ? String(defaultPhi) : ""}
      bind:value={draft.phiDays}
    />
    <!-- Efficacy is observed after the application, so a correction does not
         carry it: the register list keeps its own control for that. -->
    {#if !draft.editingId}
      <TzSelect
        label={t("treatment.efficacy")}
        hint={t("treatment.efficacy_hint")}
        items={codeItems(efficacies, "efficacy")}
        nullable
        nullLabel={t("treatment.efficacy_pending")}
        bind:value={draft.efficacyCode}
      />
    {/if}
    <TextInput label={t("treatment.notes")} bind:value={draft.notes} />
  </div>

  <!-- Model 3.1 bis. Everything here is optional: an ordinary treatment names
       no advisor and takes no non-chemical measure, and the page prints only
       the actuations that do. -->
  <fieldset class="subsection">
    <legend>{t("treatment.advised_section")}</legend>
    <p class="hint">{t("treatment.advised_hint")}</p>
    <div class="form-grid">
      <TzSelect
        label={t("treatment.advisor")}
        items={nameItems(advisors)}
        nullable
        nullLabel=""
        bind:value={draft.advisorId}
      />
      <!-- TIPO_MEDIDA_FITOSANITARIA: the catalogue's own order, not sorted. -->
      <TzSelect
        label={t("treatment.measure")}
        hint={t("treatment.measure_hint")}
        items={measures.map((measure) => ({ value: measure.code, label: measure.name }))}
        nullable
        nullLabel=""
        bind:value={draft.measureCode}
      />
      <NumberInput
        label={t("treatment.measure_intensity")}
        hint={t("treatment.measure_intensity_hint")}
        min={0.001}
        disabled={draft.measureCode === ""}
        bind:value={draft.measureIntensity}
      />
      <TzSelect
        label={t("treatment.measure_intensity_unit")}
        items={codeItems(intensityUnits, "unit")}
        disabled={draft.measureIntensity === ""}
        bind:value={draft.measureIntensityUnit}
      />
      <TextInput
        label={t("treatment.measure_registration")}
        disabled={draft.measureCode === ""}
        bind:value={draft.measureRegistration}
      />
    </div>
  </fieldset>

  <fieldset class="subsection">
    <legend>{t("treatment.problems_section")}</legend>
    {#each draft.problemRows as row, index (row)}
      <div class="form-grid plot-row">
        <TzSelect
          label={t("treatment.reason")}
          items={codeItems(reasons, "reason_category")}
          required
          bind:value={row.category}
          onchange={() => onCategoryChosen(row)}
        />
        <!-- The filter box is gone: the combobox's own input IS the trigger, so
             one control does what two used to. -->
        <TzCombobox
          label={t("treatment.problem")}
          items={problemItems(row)}
          placeholder={t("treatment.problem_filter_hint")}
          required
          disabled={!row.category}
          bind:value={row.code}
        />
        {#if draft.problemRows.length > 1}
          <button type="button" class="btn-danger" onclick={() => removeProblemRow(index)}>
            {t("treatment.remove")}
          </button>
        {/if}
      </div>
    {/each}
    <button type="button" onclick={addProblemRow}>{t("treatment.add_problem")}</button>
  </fieldset>

  <fieldset class="subsection">
    <legend>{t("treatment.justifications_section")}</legend>
    <div class="checkbox-grid">
      {#each justifications as justification (justification.code)}
        <TzCheckbox
          label={tCode("justification", justification.code)}
          value={justification.code}
          bind:group={draft.checkedJustifications}
        />
      {/each}
    </div>
  </fieldset>

  <fieldset class="subsection">
    <legend>{t("treatment.plots_section")}</legend>
    {#each draft.rows as row, index (row)}
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
          items={nameItems(cropsForPlot(row.plotId), cropLabel)}
          nullable
          nullLabel={t("treatment.crop_none")}
          bind:value={row.cropId}
        />
        <NumberInput label={t("treatment.surface")} min={0.01} required bind:value={row.surface} />
        <!-- EST_FENOLOGICO: BBCH order, 0-9. Never alphabetical. -->
        <TzSelect
          label={t("treatment.growth_stage")}
          items={growthStages.map((stage) => ({ value: stage.code, label: stage.name }))}
          nullable
          nullLabel=""
          bind:value={row.growthStage}
        />
        {#if draft.rows.length > 1}
          <button type="button" class="btn-danger" onclick={() => removeRow(index)}>
            {t("treatment.remove")}
          </button>
        {/if}
      </div>
    {/each}
    <button type="button" onclick={addRow}>{t("treatment.add_plot")}</button>
  </fieldset>
</TzForm>
