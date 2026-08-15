<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, other-treatments tab: model sections 3.3, 3.4 and 3.5 —
  // postcosecha, locales de almacenamiento and medios de transporte. One form
  // and one list serve all three; the subject picker decides which section a
  // record belongs to, and what it is measured in.
  //
  // Each register is headed by the model's "APLICA TRATAMIENTO: SÍ/NO". SÍ
  // follows from having rows; NO is an explicit statement the farmer makes, so
  // it gets its own control — an empty register and one with nothing to declare
  // are different claims.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import PlantProductPicker from "./PlantProductPicker.svelte";
  import SpeciesPicker from "./SpeciesPicker.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TzCombobox from "./TzCombobox.svelte";

  let { farmId, seasonId, countryCode, plots, products, operators, machinery, advisors } = $props();

  // Session-wide reference data, read from the module instead of drilled
  // through every parent (lib/lookups.svelte.js).
  const quantityUnits = $derived(lookups.quantityUnits);
  const justifications = $derived(lookups.justifications);
  const efficacies = $derived(lookups.efficacies);
  const reasons = $derived(lookups.reasons);
  const subjectKinds = $derived(lookups.subjectKinds);
  const seedTreatmentKinds = $derived(lookups.seedTreatmentKinds);

  let records = $state([]);
  let sowings = $state([]);
  let declarations = $state([]);
  let loading = $state(true);
  let formOpen = $state(false);
  // The record being corrected, or null when the form is entering a new one.
  let editingId = $state(null);

  // What each subject is measured in — the model's own footnotes. Mirrored from
  // the backend rule so the form can only offer the right unit; the repository
  // rejects a mismatch regardless.
  const SUBJECT_UNIT = {
    postharvest: "t",
    storage_premises: "m3",
    transport: "m3",
  };

  let subjectKind = $state("postharvest");
  let treatedOn = $state("");
  let subjectDescription = $state("");
  // 3.3 only: the produce treated, as a PROD_VEGETAL code. The free description
  // stays what the register prints — "silo 2" is not in any catalogue.
  let subjectProductName = $state("");
  let subjectProductCode = $state(null);
  let quantity = $state("");
  let productId = $state("");
  let productQuantity = $state("");
  let productQuantityUnit = $state("kg");
  let operatorId = $state("");
  let machineryId = $state("");
  // Anexo III Parte I B.d — "identificación del aplicador y, en su caso, del
  // asesor" — and B.b/B.f put premises and vehicles inside B's own list, so
  // these registers carry the advisor exactly as 3.1 does.
  let advisorId = $state("");
  let efficacyCode = $state("");
  let notes = $state("");
  let problemRows = $state([emptyProblemRow()]);
  let checkedJustifications = $state([]);
  let problemCatalogues = $state({});

  const subjectUnit = $derived(SUBJECT_UNIT[subjectKind]);

  function emptyProblemRow() {
    return { category: "", code: "", filter: "" };
  }

  load();

  function load() {
    run(async () => {
      [records, sowings, declarations] = await Promise.all([
        invoke("list_non_field_treatments", { seasonId, farmId }),
        invoke("list_seed_treatments", { seasonId, farmId }),
        invoke("list_register_declarations", { farmId, seasonId }),
      ]);
    }).finally(() => (loading = false));
  }

  function rowsOf(kind) {
    return records.filter((detail) => detail.record.subject_kind_code === kind);
  }

  function declaredEmpty(kind) {
    return declarations.some((declaration) => declaration.register_code === kind);
  }

  /// SÍ / NO / neither — the three states the model's boxes can show. The seed
  /// register (3.2) is one of them, just backed by its own table.
  function answerOf(kind) {
    const rows = kind === "seed_treatment" ? sowings : rowsOf(kind);
    if (rows.length > 0) return "yes";
    return declaredEmpty(kind) ? "no" : "";
  }

  function declareEmpty(kind) {
    run(async () => {
      await invoke("set_register_declaration", { farmId, seasonId, registerCode: kind });
      notify(t("message.register_declared"));
      load();
    });
  }

  function withdrawDeclaration(kind) {
    run(async () => {
      await invoke("clear_register_declaration", { farmId, seasonId, registerCode: kind });
      notify(t("message.register_declaration_cleared"));
      load();
    });
  }

  function showForm(kind) {
    editingId = null;
    subjectKind = kind;
    treatedOn = "";
    subjectDescription = "";
    subjectProductName = "";
    subjectProductCode = null;
    quantity = "";
    productId = "";
    productQuantity = "";
    productQuantityUnit = "kg";
    operatorId = "";
    machineryId = "";
    advisorId = "";
    efficacyCode = "";
    notes = "";
    problemRows = [emptyProblemRow()];
    checkedJustifications = [];
    formOpen = true;
  }

  /// Open the form on a stored record so the farmer corrects the one thing that
  /// was wrong. The subject KIND is not offered: moving a record between the
  /// three registers would empty one and fill another, so that is a delete and
  /// a re-entry.
  function editRecord(detail) {
    const stored = detail.record;
    editingId = stored.id;
    subjectKind = stored.subject_kind_code;
    treatedOn = stored.treated_on;
    subjectDescription = stored.subject_description;
    subjectProductName = "";
    subjectProductCode = stored.subject_product_code;
    quantity = stored.treated_quantity_value ?? "";
    productId = stored.product_id;
    productQuantity = stored.product_quantity_value ?? "";
    productQuantityUnit = stored.product_quantity_unit_code ?? "kg";
    operatorId = stored.operator_id;
    machineryId = stored.machinery_id ?? "";
    advisorId = stored.advisor_id ?? "";
    efficacyCode = stored.efficacy_code ?? "";
    notes = stored.notes ?? "";
    problemRows = detail.problems.map((problem) => ({
      category: problem.reason_category_code,
      code: problem.problem_code,
      filter: "",
    }));
    for (const row of problemRows) loadProblemCatalogue(row.category);
    checkedJustifications = detail.justifications.map((j) => j.justification_code);
    formOpen = true;
  }

  function onCategoryChosen(row) {
    row.code = "";
    loadProblemCatalogue(row.category);
  }

  /// Fetch a category's catalogue once. Also called when a correction opens, so
  /// a stored problem's select shows its label rather than an empty box.
  function loadProblemCatalogue(category) {
    if (!category || problemCatalogues[category]) return;
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

  function submit(event) {
    event.preventDefault();
    const fields = {
      treated_on: treatedOn,
      subject_description: subjectDescription.trim(),
      subject_product_code: subjectKind === "postharvest" ? subjectProductCode : null,
      // Value and unit travel together; the unit follows the subject.
      treated_quantity_value: quantity === "" ? null : Number(quantity),
      treated_quantity_unit_code: quantity === "" ? null : subjectUnit,
      product_id: productId,
      product_quantity_value: productQuantity === "" ? null : Number(productQuantity),
      product_quantity_unit_code: productQuantity === "" ? null : productQuantityUnit,
      operator_id: operatorId,
      machinery_id: machineryId || null,
      advisor_id: advisorId || null,
      problems: problemRows
        .filter((row) => row.category && row.code)
        .map((row) => ({ reason_category_code: row.category, problem_code: row.code })),
      justifications: [...checkedJustifications],
      notes: notes.trim() || null,
    };
    run(async () => {
      if (editingId) {
        // A correction carries neither the campaign, the holding, the subject
        // kind nor the efficacy — that one is observed later and keeps its own
        // control in the list.
        await invoke("update_non_field_treatment", { treatmentId: editingId, update: fields });
        notify(t("message.non_field_updated"));
      } else {
        await invoke("create_non_field_treatment", {
          record: {
            ...fields,
            season_id: seasonId,
            farm_id: farmId,
            country_code: null, // derived from the farm in Rust
            subject_kind_code: subjectKind,
            efficacy_code: efficacyCode || null,
          },
        });
        notify(t("message.non_field_saved"));
      }
      formOpen = false;
      editingId = null;
      load();
    });
  }

  function setEfficacy(record, code) {
    run(async () => {
      await invoke("set_non_field_efficacy", {
        treatmentId: record.id,
        efficacyCode: code || null,
      });
      load();
    });
  }

  function deleteRecord(record) {
    run(async () => {
      if (!(await confirmDialog(t("non_field.delete_confirm")))) return;
      await invoke("delete_non_field_treatment", { treatmentId: record.id });
      notify(t("message.non_field_deleted"));
      load();
    });
  }

  // --- 3.2 uso de semilla tratada ------------------------------------------
  // A sowing, not an application: no applicator or equipment, and the product
  // is the text printed on the sack.
  let seedFormOpen = $state(false);
  let editingSowingId = $state(null);
  let sownOn = $state("");
  let seedSpecies = $state("");
  let seedCropCode = $state(null);
  let seedVariety = $state("");
  let seedQuantity = $state("");
  let seedLot = $state("");
  let seedProduct = $state("");
  let seedRegistration = $state("");
  let seedSubstance = $state("");
  let seedTreatmentKind = $state("");
  let seedNotes = $state("");
  let seedPlots = $state([emptySeedPlot()]);

  function emptySeedPlot() {
    return { plotId: "", surface: "" };
  }

  function showSeedForm(detail = null) {
    editingSowingId = detail?.record.id ?? null;
    sownOn = detail?.record.sown_on ?? "";
    seedSpecies = detail?.record.species_name ?? "";
    seedCropCode = detail?.record.crop_code ?? null;
    seedVariety = detail?.record.variety ?? "";
    seedQuantity = detail?.record.seed_quantity_kg ?? "";
    seedLot = detail?.record.seed_lot ?? "";
    seedProduct = detail?.record.product_name ?? "";
    seedRegistration = detail?.record.product_registration_number ?? "";
    seedSubstance = detail?.record.product_active_substance ?? "";
    seedTreatmentKind = detail?.record.treatment_kind_code ?? "";
    seedNotes = detail?.record.notes ?? "";
    seedPlots = detail
      ? detail.plots.map((p) => ({ plotId: p.plot_id, surface: p.surface_sown_ha }))
      : [emptySeedPlot()];
    seedFormOpen = true;
  }

  function onSeedPlotChosen(row) {
    // The plot's own area is the usual sown surface; a partial sowing lowers it.
    const detail = plots.find((p) => p.plot.id === row.plotId);
    if (detail?.plot.area_ha != null) row.surface = detail.plot.area_ha;
  }

  function submitSeed(event) {
    event.preventDefault();
    const payload = {
      sown_on: sownOn,
      species_name: seedSpecies.trim(),
      variety: seedVariety.trim() || null,
      crop_code: seedCropCode,
      seed_quantity_kg: seedQuantity === "" ? null : Number(seedQuantity),
      seed_lot: seedLot.trim() || null,
      product_name: seedProduct.trim(),
      product_registration_number: seedRegistration.trim() || null,
      product_active_substance: seedSubstance.trim() || null,
      treatment_kind_code: seedTreatmentKind || null,
      product_id: null,
      notes: seedNotes.trim() || null,
      plots: seedPlots
        .filter((row) => row.plotId)
        .map((row) => ({ plot_id: row.plotId, surface_sown_ha: Number(row.surface) })),
    };
    run(async () => {
      if (editingSowingId) {
        await invoke("update_seed_treatment", {
          seedTreatmentId: editingSowingId,
          update: payload,
        });
      } else {
        await invoke("create_seed_treatment", {
          record: { ...payload, season_id: seasonId, farm_id: farmId, efficacy_code: null },
        });
      }
      notify(t("message.seed_saved"));
      seedFormOpen = false;
      load();
    });
  }

  function setSeedEfficacy(record, code) {
    run(async () => {
      await invoke("set_seed_treatment_efficacy", {
        seedTreatmentId: record.id,
        efficacyCode: code || null,
      });
      load();
    });
  }

  function deleteSowing(record) {
    run(async () => {
      if (!(await confirmDialog(t("seed.delete_confirm")))) return;
      await invoke("delete_seed_treatment", { seedTreatmentId: record.id });
      notify(t("message.seed_deleted"));
      load();
    });
  }

  function plotName(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? plotId;
  }

  function quantityLabel(kind) {
    return kind === "postharvest" ? t("non_field.quantity_t") : t("non_field.quantity_m3");
  }

  // Only the quantity units an amount of product can be sold in.
  const productUnits = $derived(quantityUnits.filter((u) => u.code === "kg" || u.code === "l"));
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <!-- 3.2 comes first, as the model orders the registers. -->
  {@const seedAnswer = answerOf("seed_treatment")}
  <div class="view-head">
    <h3>{t("seed.title")}</h3>
    <div class="selector-buttons">
      <button type="button" onclick={() => showSeedForm()} disabled={plots.length === 0}>
        {t("seed.new")}
      </button>
      {#if seedAnswer === "no"}
        <button
          type="button"
          class="btn-cancel"
          onclick={() => withdrawDeclaration("seed_treatment")}
        >
          {t("non_field.undeclare")}
        </button>
      {:else if seedAnswer === ""}
        <button type="button" onclick={() => declareEmpty("seed_treatment")}>
          {t("non_field.declare_empty")}
        </button>
      {/if}
    </div>
  </div>
  <p class="detail">
    {t("non_field.applies")}:
    <strong>
      {#if seedAnswer === "yes"}{t("non_field.applies_yes")}
      {:else if seedAnswer === "no"}{t("non_field.applies_no")}
      {:else}—{/if}
    </strong>
    {#if seedAnswer === ""}
      · {t("non_field.applies_hint")}
    {/if}
  </p>

  <ul class="card-list">
    {#each sowings as { record, plots: sownPlots } (record.id)}
      <li class="card">
        <div class="stack">
          <strong>
            {formatDate(record.sown_on)} — {record.species_name}{record.variety
              ? ` — ${record.variety}`
              : ""}
          </strong>
          <span class="detail">
            {record.product_name}
            {#if record.product_registration_number}
              · {record.product_registration_number}
            {/if}
            {#if record.seed_lot}
              · {t("seed.lot_detail", { lot: record.seed_lot })}
            {/if}
          </span>
          <span class="detail">
            {sownPlots.map((p) => `${plotName(p.plot_id)} (${p.surface_sown_ha} ha)`).join(", ")}
            {#if record.seed_quantity_kg !== null}
              · {t("seed.quantity_detail", { kg: record.seed_quantity_kg })}
            {/if}
          </span>
          <TzSelect
            class="inline-field"
            label={t("treatment.efficacy")}
            items={codeItems(efficacies, "efficacy")}
            nullable
            nullLabel={t("treatment.efficacy_pending")}
            value={record.efficacy_code ?? ""}
            onchange={(code) => setSeedEfficacy(record, code)}
          />
        </div>
        <button type="button" onclick={() => showSeedForm({ record, plots: sownPlots })}>
          {t("form.edit")}
        </button>
        <button type="button" class="btn-danger" onclick={() => deleteSowing(record)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>

  {#if seedFormOpen}
    <form onsubmit={submitSeed}>
      <div class="form-grid">
        <DateInput label={t("seed.sown_on")} required bind:value={sownOn} />
        <label>
          <span>{t("crop.species")}</span>
          <SpeciesPicker bind:name={seedSpecies} bind:code={seedCropCode} required />
        </label>
        <label><span>{t("crop.variety")}</span><input bind:value={seedVariety} /></label>
        <label>
          <span>{t("seed.quantity")}</span>
          <input type="number" step="any" min="0.001" bind:value={seedQuantity} />
        </label>
        <label>
          <span>{t("seed.lot")}</span>
          <input bind:value={seedLot} />
          <small>{t("seed.lot_hint")}</small>
        </label>
        <label>
          <span>{t("seed.product")}</span>
          <input required bind:value={seedProduct} />
          <small>{t("seed.product_hint")}</small>
        </label>
        <label>
          <span>{t("seed.registration")}</span>
          <input bind:value={seedRegistration} />
        </label>
        <label>
          <span>{t("seed.active_substance")}</span>
          <input bind:value={seedSubstance} />
        </label>
        <TzSelect
          label={t("seed.treatment_kind")}
          hint={t("seed.treatment_kind_hint")}
          items={codeItems(seedTreatmentKinds, "seed_treatment_kind")}
          nullable
          nullLabel=""
          bind:value={seedTreatmentKind}
        />
        <label>
          <span>{t("treatment.notes")}</span>
          <input bind:value={seedNotes} />
        </label>
      </div>

      <fieldset class="subsection">
        <legend>{t("seed.plots_section")}</legend>
        {#each seedPlots as row, index (row)}
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
              onchange={() => onSeedPlotChosen(row)}
            />
            <label>
              <span>{t("seed.surface")}</span>
              <input type="number" step="any" min="0.01" required bind:value={row.surface} />
            </label>
            {#if seedPlots.length > 1}
              <button type="button" class="btn-danger" onclick={() => seedPlots.splice(index, 1)}>
                {t("treatment.remove")}
              </button>
            {/if}
          </div>
        {/each}
        <button type="button" onclick={() => seedPlots.push(emptySeedPlot())}>
          {t("treatment.add_plot")}
        </button>
      </fieldset>

      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (seedFormOpen = false)}>
          {t("form.cancel")}
        </button>
      </div>
    </form>
  {/if}

  {#each subjectKinds as kind (kind.code)}
    {@const answer = answerOf(kind.code)}
    <div class="view-head">
      <h3>{tCode("non_field_subject_kind", kind.code)}</h3>
      <div class="selector-buttons">
        <button type="button" onclick={() => showForm(kind.code)} disabled={products.length === 0}>
          {t("non_field.new")}
        </button>
        {#if answer === "no"}
          <button type="button" class="btn-cancel" onclick={() => withdrawDeclaration(kind.code)}>
            {t("non_field.undeclare")}
          </button>
        {:else if answer === ""}
          <button type="button" onclick={() => declareEmpty(kind.code)}>
            {t("non_field.declare_empty")}
          </button>
        {/if}
      </div>
    </div>

    <!-- The model's own heading, in the same three states the printed book
         shows: rows say SÍ, a declaration says NO, silence says neither. -->
    <p class="detail">
      {t("non_field.applies")}:
      <strong>
        {#if answer === "yes"}{t("non_field.applies_yes")}
        {:else if answer === "no"}{t("non_field.applies_no")}
        {:else}—{/if}
      </strong>
      {#if answer === ""}
        · {t("non_field.applies_hint")}
      {/if}
    </p>

    <ul class="card-list">
      {#each rowsOf(kind.code) as { record, problems, justifications: recordJustifications } (record.id)}
        <li class="card">
          <div class="stack">
            <strong>{formatDate(record.treated_on)} — {record.subject_description}</strong>
            <span class="detail">
              {record.product_name_snapshot}
              {#if record.product_quantity_value !== null}
                · {record.product_quantity_value}
                {tCode("unit", record.product_quantity_unit_code)}
              {/if}
              · {record.operator_name_snapshot}
              {#if record.advisor_name_snapshot !== null}
                · {record.advisor_name_snapshot}
                {#if record.advisor_registration_snapshot !== null}
                  ({record.advisor_registration_snapshot})
                {/if}
              {/if}
            </span>
            {#if record.treated_quantity_value !== null}
              <span class="detail">
                {quantityLabel(kind.code)}: {record.treated_quantity_value}
                {tCode("unit", record.treated_quantity_unit_code)}
              </span>
            {/if}
            <span class="detail">
              {problems
                .map((p) => `${tCode("reason_category", p.reason_category_code)} ${p.problem_code}`)
                .join(", ")}
            </span>
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
            onclick={() => editRecord({ record, problems, justifications: recordJustifications })}
          >
            {t("form.edit")}
          </button>
          <button type="button" class="btn-danger" onclick={() => deleteRecord(record)}>
            {t("form.delete")}
          </button>
        </li>
      {/each}
    </ul>
  {/each}

  {#if products.length === 0}
    <p>{t("treatments.missing_refs")} <a href="#/registry">{t("nav.registry")}</a></p>
  {/if}

  {#if formOpen}
    <form onsubmit={submit}>
      <div class="form-grid">
        <!-- Frozen while correcting: moving a record between the three
             registers would empty one and fill another. -->
        <TzSelect
          label={t("non_field.subject_kind")}
          items={codeItems(subjectKinds, "non_field_subject_kind")}
          required
          disabled={editingId !== null}
          bind:value={subjectKind}
        />
        <DateInput label={t("non_field.date")} required bind:value={treatedOn} />
        <label>
          <span>{t(`non_field.subject_${subjectKind}`)}</span>
          <input required bind:value={subjectDescription} />
        </label>
        {#if subjectKind === "postharvest"}
          <label>
            <span>{t("harvest.product")}</span>
            <PlantProductPicker
              bind:name={subjectProductName}
              bind:code={subjectProductCode}
              {countryCode}
            />
            <small>{t("harvest.product_hint")}</small>
          </label>
        {/if}
        <label>
          <span>{quantityLabel(subjectKind)}</span>
          <input type="number" step="any" min="0.0001" bind:value={quantity} />
          <small>{t("non_field.quantity_hint")}</small>
        </label>
        <TzSelect
          label={t("treatment.product")}
          items={nameItems(products, (p) => p.commercial_name)}
          required
          bind:value={productId}
        />
        <label>
          <span>{t("non_field.product_quantity")}</span>
          <input type="number" step="any" min="0.0001" bind:value={productQuantity} />
        </label>
        <TzSelect
          label={t("non_field.product_quantity_unit")}
          items={codeItems(productUnits, "unit")}
          disabled={productQuantity === ""}
          bind:value={productQuantityUnit}
        />
        <TzSelect
          label={t("treatment.operator")}
          items={nameItems(operators, (o) => o.full_name)}
          required
          bind:value={operatorId}
        />
        <TzSelect
          label={t("treatment.machinery")}
          items={nameItems(machinery)}
          nullable
          nullLabel={t("treatment.machinery_none")}
          bind:value={machineryId}
        />
        <TzSelect
          label={t("treatment.advisor")}
          hint={t("non_field.advisor_hint")}
          items={nameItems(advisors)}
          nullable
          nullLabel=""
          bind:value={advisorId}
        />
        <!-- Efficacy is observed after the fact, so a correction does not carry
             it: the register list keeps its own control for that. -->
        {#if editingId === null}
          <TzSelect
            label={t("treatment.efficacy")}
            hint={t("treatment.efficacy_hint")}
            items={codeItems(efficacies, "efficacy")}
            nullable
            nullLabel={t("treatment.efficacy_pending")}
            bind:value={efficacyCode}
          />
        {/if}
        <label>
          <span>{t("treatment.notes")}</span>
          <input bind:value={notes} />
        </label>
      </div>

      <fieldset class="subsection">
        <legend>{t("treatment.problems_section")}</legend>
        {#each problemRows as row, index (row)}
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
            {#if problemRows.length > 1}
              <button type="button" class="btn-danger" onclick={() => problemRows.splice(index, 1)}>
                {t("treatment.remove")}
              </button>
            {/if}
          </div>
        {/each}
        <button type="button" onclick={() => problemRows.push(emptyProblemRow())}>
          {t("treatment.add_problem")}
        </button>
      </fieldset>

      <fieldset class="subsection">
        <legend>{t("treatment.justifications_section")}</legend>
        <div class="checkbox-grid">
          {#each justifications as justification (justification.code)}
            <label class="checkbox">
              <input
                type="checkbox"
                value={justification.code}
                bind:group={checkedJustifications}
              />
              <span>{tCode("justification", justification.code)}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (formOpen = false)}>
          {t("form.cancel")}
        </button>
      </div>
    </form>
  {/if}
{/if}
