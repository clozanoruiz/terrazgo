<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, other-treatments tab: model sections 3.2, 3.3, 3.4 and 3.5 —
  // semilla tratada, postcosecha, locales de almacenamiento and medios de
  // transporte. FOUR registers behind a sub-tab strip, one at a time.
  //
  // One form still serves the last three, and the sub-tab is what decides which
  // register a record belongs to — which is why the form no longer asks for the
  // subject kind, and why it also decides what the amount is measured in.
  //
  // Each register is headed by the model's "APLICA TRATAMIENTO: SÍ/NO". SÍ
  // follows from having rows; NO is an explicit statement the farmer makes, so
  // it gets its own control — an empty register and one with nothing to declare
  // are different claims. Only the open register's answer is on screen now; the
  // export advisory is what reports every register left undeclared at once.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import DateInput from "./DateInput.svelte";
  import PlantProductPicker from "./PlantProductPicker.svelte";
  import SpeciesPicker from "./SpeciesPicker.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TzCombobox from "./TzCombobox.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzTabs from "./TzTabs.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId, seasonId, countryCode, plots, products, operators, machinery, premises, advisors } =
    $props();

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
  /// Core's sowing register (the crops tab's "Siembra y plantación"), so a
  /// treated-seed record can name the sowing it fed.
  let sowingRecords = $state([]);
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

  // Which registry kind each register may name, mirroring
  // module_cue::premises_link the way SUBJECT_UNIT mirrors its own rule. A
  // postharvest record treats produce and names no place at all, which is why
  // it is absent here rather than mapped to something.
  const SUBJECT_PREMISES_KIND = {
    storage_premises: "building",
    transport: "vehicle",
  };

  let subjectKind = $state("postharvest");
  let treatedOn = $state("");
  let subjectDescription = $state("");
  // 3.4 / 3.5: the registry row identifying the local or vehicle treated
  // (Anexo III Parte I B.b). Optional, because refusing a record for want of a
  // registry row would be the register blocking the duty it serves — with none
  // named, the free description below is what the register states.
  let premisesId = $state("");
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

  // The premises this register may name, and the one currently named. An empty
  // list means the catalogue holds none of that kind yet, not that the field is
  // unavailable.
  const premisesKind = $derived(SUBJECT_PREMISES_KIND[subjectKind]);
  const premisesOptions = $derived(
    premisesKind ? (premises ?? []).filter((row) => row.kind_code === premisesKind) : [],
  );
  const chosenPremises = $derived(premisesOptions.find((row) => row.id === premisesId));

  /// What the chosen row holds, so the farmer can see they picked the right
  /// store — the field is labelled "tipo y dirección", so it shows both.
  /// Separators, not commas: this is the row's data, NOT a preview of the
  /// printed cell, which is composed in Rust. Mirroring that composition here
  /// would put a second, unchecked copy of the rule in the frontend.
  function premisesDetail(row) {
    return [row.name, row.kind_code === "vehicle" ? row.vehicle_model : row.address, row.plate]
      .filter(Boolean)
      .join(" · ");
  }

  function emptyProblemRow() {
    return { category: "", code: "", filter: "" };
  }

  load();

  function load() {
    run(async () => {
      [records, sowings, declarations, sowingRecords] = await Promise.all([
        invoke("list_non_field_treatments", { seasonId, farmId }),
        invoke("list_seed_treatments", { seasonId, farmId }),
        invoke("list_register_declarations", { farmId, seasonId }),
        // Core's sowing register, so a 3.2 record can name the sowing that used
        // this seed. The exchange format hangs "material tratado" off the
        // SOWING, and only the farmer knows which one it was.
        invoke("list_sowing_records", { seasonId, farmId }),
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
      load();
    });
  }

  function withdrawDeclaration(kind) {
    run(async () => {
      await invoke("clear_register_declaration", { farmId, seasonId, registerCode: kind });
      load();
    });
  }

  function showForm(kind) {
    editingId = null;
    subjectKind = kind;
    treatedOn = "";
    subjectDescription = "";
    premisesId = "";
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
    // Clearing the link leaves this text standing as the record's own
    // statement, which is exactly what the repository does with it.
    premisesId = stored.premises_id ?? "";
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

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form —
  /// and the efficacy control above it — know which record they are about.
  /// Null while entering a new one.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

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

  async function submit() {
    const fields = {
      treated_on: treatedOn,
      // Ignored by the backend when a premises is named: the description is
      // composed from the registry row instead, so the two cannot disagree.
      subject_description: subjectDescription.trim(),
      premises_id: premisesId || null,
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
    if (editingId) {
      // A correction carries neither the campaign, the holding, the subject
      // kind nor the efficacy — that one is observed later and keeps its own
      // control in the list.
      await invoke("update_non_field_treatment", { treatmentId: editingId, update: fields });
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
    }
    hideForm();
    load();
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
      hideForm();
      load();
    });
  }

  /// The amount of product a record used, as its own cell. Value and unit
  /// travel together, so an unstated amount is blank rather than a bare zero.
  function productAmount(record) {
    if (record.product_quantity_value === null) return "";
    return `${formatNumber(record.product_quantity_value)} ${tCode(
      "unit",
      record.product_quantity_unit_code,
    )}`;
  }

  /// The amount of produce, store or vehicle treated — tonnes for postcosecha,
  /// cubic metres for the other two, which is what the subject decides.
  function treatedAmount(record) {
    if (record.treated_quantity_value === null) return "";
    return `${formatNumber(record.treated_quantity_value)} ${tCode(
      "unit",
      record.treated_quantity_unit_code,
    )}`;
  }

  function problemsCell(problems) {
    return problems
      .map((p) => `${tCode("reason_category", p.reason_category_code)} ${p.problem_code}`)
      .join(", ");
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
  let seedAcquiredOn = $state("");
  let seedSowingId = $state("");
  let seedNotes = $state("");
  let seedPlots = $state([emptySeedPlot()]);

  function emptySeedPlot() {
    return { plotId: "", surface: "" };
  }

  /// TIPO_TRATAMIENTO 4 and 5 are "adquisición de semilla tratada"; 2 and 3 are
  /// seed treated on the holding or at a conditioning centre. The purchase date
  /// is only a question for the first pair.
  const seedIsAcquired = $derived(
    seedTreatmentKind === "purchased_es" || seedTreatmentKind === "purchased_abroad",
  );

  /// The campaign's sowings, dated and named by what they started, because a
  /// farmer picks the sowing by when it happened rather than by an id.
  const sowingItems = $derived(
    sowingRecords.map(({ record, plots: sownPlots }) => ({
      value: record.id,
      label: [
        formatDate(record.sown_on),
        sownPlots
          .map((p) => p.crop_name_snapshot ?? plotName(p.plot_id))
          .filter(Boolean)
          .join(", "),
      ]
        .filter(Boolean)
        .join(" — "),
    })),
  );

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
    seedAcquiredOn = detail?.record.acquired_on ?? "";
    seedSowingId = detail?.record.sowing_record_id ?? "";
    seedNotes = detail?.record.notes ?? "";
    seedPlots = detail
      ? detail.plots.map((p) => ({ plotId: p.plot_id, surface: p.surface_sown_ha }))
      : [emptySeedPlot()];
    seedFormOpen = true;
  }

  function hideSeedForm() {
    seedFormOpen = false;
    editingSowingId = null;
  }

  /// The row the seed inspector is editing, so its delete button and its
  /// efficacy control know which record they are about. Null while creating.
  const editingSeed = $derived(sowings.find((d) => d.record.id === editingSowingId) ?? null);

  /// One seed record's plots and the surface sown on each.
  function seedPlotsCell(sownPlots) {
    return sownPlots
      .map((p) => `${plotName(p.plot_id)} (${formatNumber(p.surface_sown_ha)} ha)`)
      .join(", ");
  }

  function onSeedPlotChosen(row) {
    // The plot's own area is the usual sown surface; a partial sowing lowers it.
    const detail = plots.find((p) => p.plot.id === row.plotId);
    if (detail?.plot.area_ha != null) row.surface = detail.plot.area_ha;
  }

  async function submitSeed() {
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
      // Only meaningful for the two purchased kinds, and cleared with them so a
      // corrected record cannot keep a purchase date for seed treated at home.
      acquired_on: (seedIsAcquired && seedAcquiredOn) || null,
      sowing_record_id: seedSowingId || null,
      product_id: null,
      notes: seedNotes.trim() || null,
      plots: seedPlots
        .filter((row) => row.plotId)
        .map((row) => ({ plot_id: row.plotId, surface_sown_ha: Number(row.surface) })),
    };
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
    hideSeedForm();
    load();
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
      hideSeedForm();
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

  // --- the four registers, one at a time ------------------------------------
  //
  // Stacked, these read as one long page of four tables that a reader has to
  // count headings to tell apart. They are not a set that reads together: the
  // model numbers them 3.2 to 3.5 as separate sections, a record is postcosecha
  // OR a store OR a vehicle, and even the unit differs — tonnes for produce,
  // cubic metres for the other two. Nothing here cross-references anything
  // else, which is what a stack is for.
  //
  // The strip is the same shape section 9 already uses for its sub-tables
  // (BookEcoschemes), so this is the app's vocabulary rather than a new idea.
  const registers = $derived([
    { value: "seed_treatment", label: t("seed.tab") },
    ...subjectKinds.map((kind) => ({
      value: kind.code,
      label: t(`non_field.tab_${kind.code}`),
    })),
  ]);
  let register = $state("seed_treatment");

  /// The subject kind the open sub-tab stands for, or null on the seed tab —
  /// which is a register of its own with its own table and its own form.
  const openKind = $derived(subjectKinds.find((kind) => kind.code === register) ?? null);
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <!-- Model order: 3.2 first, then the three subject registers. Switching
       register closes whatever form was open — a record being corrected in one
       register means nothing in the next. -->
  <TzTabs
    items={registers}
    bind:value={register}
    nested
    framed
    onchange={() => {
      hideForm();
      hideSeedForm();
    }}
  >
    {#snippet panel(item)}
      {#if item.value === "seed_treatment"}
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

        <TzWorkspace
          open={seedFormOpen}
          title={editingSowingId ? seedSpecies : t("seed.new")}
          onclose={hideSeedForm}
          ondelete={editingSeed ? () => deleteSowing(editingSeed.record) : null}
        >
          {#snippet list()}
            {#if sowings.length === 0}
              <p class="table-empty">{t("table.empty")}</p>
            {:else}
              <div class="table-wrap">
                <table class="data-table" use:resizableColumns={"seed-treatments"}>
                  <thead>
                    <tr>
                      <th>{t("column.date")}</th>
                      <th>{t("column.species")}</th>
                      <th>{t("column.product")}</th>
                      <th>{t("column.lot")}</th>
                      <th>{t("column.plots")}</th>
                      <th class="col-num">{t("column.seed_kg")}</th>
                      <th>{t("column.efficacy")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each sowings as entry (entry.record.id)}
                      {@const record = entry.record}
                      <tr
                        class:selected={editingSowingId === record.id}
                        onclick={(e) => opensRow(e) && showSeedForm(entry)}
                      >
                        <td class="col-name">
                          <button
                            type="button"
                            class="row-open"
                            onclick={() => showSeedForm(entry)}
                          >
                            {formatDate(record.sown_on)}
                          </button>
                        </td>
                        <td class="col-muted">
                          {record.species_name}{record.variety ? ` — ${record.variety}` : ""}
                        </td>
                        <td class="col-muted">{record.product_name}</td>
                        <td class="col-muted">{record.seed_lot ?? ""}</td>
                        <td class="col-muted">{seedPlotsCell(entry.plots)}</td>
                        <td class="col-muted col-num">
                          {record.seed_quantity_kg === null
                            ? ""
                            : formatNumber(record.seed_quantity_kg)}
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
            <!-- Efficacy is observed after the fact, so a correction never carries
           it: it keeps its own audited setter, now beside the record the
           inspector names rather than repeated on every row. -->
            {#if editingSeed}
              <div class="form-grid">
                <TzSelect
                  label={t("treatment.efficacy")}
                  items={codeItems(efficacies, "efficacy")}
                  nullable
                  nullLabel={t("treatment.efficacy_pending")}
                  value={editingSeed.record.efficacy_code ?? ""}
                  onchange={(code) => setSeedEfficacy(editingSeed.record, code)}
                />
              </div>
            {/if}

            <TzForm id={formId} onsubmit={submitSeed}>
              <div class="form-grid">
                <DateInput label={t("seed.sown_on")} required bind:value={sownOn} />
                <label>
                  <span>{t("crop.species")}</span>
                  <SpeciesPicker bind:name={seedSpecies} bind:code={seedCropCode} required />
                </label>
                <TextInput label={t("crop.variety")} bind:value={seedVariety} />
                <NumberInput label={t("seed.quantity")} min={0.001} bind:value={seedQuantity} />
                <TextInput label={t("seed.lot")} bind:value={seedLot}>
                  <small>{t("seed.lot_hint")}</small>
                </TextInput>
                <TextInput label={t("seed.product")} required bind:value={seedProduct}>
                  <small>{t("seed.product_hint")}</small>
                </TextInput>
                <TextInput label={t("seed.registration")} bind:value={seedRegistration} />
                <TextInput label={t("seed.active_substance")} bind:value={seedSubstance} />
                <TzSelect
                  label={t("seed.treatment_kind")}
                  hint={t("seed.treatment_kind_hint")}
                  items={codeItems(seedTreatmentKinds, "seed_treatment_kind")}
                  nullable
                  nullLabel=""
                  bind:value={seedTreatmentKind}
                />
                <!-- Only the two purchased kinds have a purchase to date. -->
                {#if seedIsAcquired}
                  <DateInput
                    label={t("seed.acquired_on")}
                    hint={t("seed.acquired_on_hint")}
                    bind:value={seedAcquiredOn}
                  />
                {/if}
                <TzSelect
                  label={t("seed.sowing_link")}
                  hint={t("seed.sowing_link_hint")}
                  items={sowingItems}
                  nullable
                  nullLabel={t("seed.sowing_link_none")}
                  bind:value={seedSowingId}
                />
                <TextInput label={t("treatment.notes")} bind:value={seedNotes} />
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
                    <NumberInput
                      label={t("seed.surface")}
                      min={0.01}
                      required
                      bind:value={row.surface}
                    />
                    {#if seedPlots.length > 1}
                      <button
                        type="button"
                        class="btn-danger"
                        onclick={() => seedPlots.splice(index, 1)}
                      >
                        {t("treatment.remove")}
                      </button>
                    {/if}
                  </div>
                {/each}
                <button type="button" onclick={() => seedPlots.push(emptySeedPlot())}>
                  {t("treatment.add_plot")}
                </button>
              </fieldset>
            </TzForm>
          {/snippet}

          {#snippet actions(formId)}
            <div class="form-actions">
              <button type="submit" form={formId}>{t("form.save")}</button>
              <button type="button" class="btn-cancel" onclick={hideSeedForm}>
                {t("form.cancel")}
              </button>
            </div>
          {/snippet}
        </TzWorkspace>
      {:else if openKind}
        {@const kind = openKind}
        {@const answer = answerOf(kind.code)}
        <div class="view-head">
          <h3>{tCode("non_field_subject_kind", kind.code)}</h3>
          <div class="selector-buttons">
            <button
              type="button"
              onclick={() => showForm(kind.code)}
              disabled={products.length === 0}
            >
              {t("non_field.new")}
            </button>
            {#if answer === "no"}
              <button
                type="button"
                class="btn-cancel"
                onclick={() => withdrawDeclaration(kind.code)}
              >
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

        {#if products.length === 0}
          <p class="detail">
            {t("treatments.missing_refs")} <a href="#/registry">{t("nav.registry")}</a>
          </p>
        {/if}

        <!-- Three registers, ONE form: the sub-tab is what decides which of
             them a record belongs to, which is why the form no longer asks. -->
        <TzWorkspace
          open={formOpen}
          title={editingId ? subjectDescription : t("non_field.new")}
          onclose={hideForm}
          ondelete={editing ? () => deleteRecord(editing.record) : null}
        >
          {#snippet list()}
            {#if rowsOf(kind.code).length === 0}
              <p class="table-empty">{t("table.empty")}</p>
            {:else}
              <div class="table-wrap">
                <table class="data-table" use:resizableColumns={`non-field-${kind.code}`}>
                  <thead>
                    <tr>
                      <th>{t("column.date")}</th>
                      <th>{t("column.subject")}</th>
                      <th>{t("column.product")}</th>
                      <th class="col-num">{t("column.dose")}</th>
                      <th class="col-num">{quantityLabel(kind.code)}</th>
                      <th>{t("column.operator")}</th>
                      <th>{t("column.problems")}</th>
                      <th>{t("column.efficacy")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each rowsOf(kind.code) as entry (entry.record.id)}
                      {@const record = entry.record}
                      <tr
                        class:selected={editingId === record.id}
                        onclick={(e) => opensRow(e) && editRecord(entry)}
                      >
                        <td class="col-name">
                          <button type="button" class="row-open" onclick={() => editRecord(entry)}>
                            {formatDate(record.treated_on)}
                          </button>
                        </td>
                        <td class="col-muted">{record.subject_description}</td>
                        <td class="col-muted">{record.product_name_snapshot}</td>
                        <td class="col-muted col-num">{productAmount(record)}</td>
                        <td class="col-muted col-num">{treatedAmount(record)}</td>
                        <td class="col-muted">{record.operator_name_snapshot}</td>
                        <td class="col-muted">{problemsCell(entry.problems)}</td>
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
            <!-- Efficacy is observed after the fact, so a correction never carries
             it: it keeps its own audited setter, now beside the record the
             inspector names rather than repeated on every row. -->
            {#if editing}
              <div class="form-grid">
                <TzSelect
                  label={t("treatment.efficacy")}
                  items={codeItems(efficacies, "efficacy")}
                  nullable
                  nullLabel={t("treatment.efficacy_pending")}
                  value={editing.record.efficacy_code ?? ""}
                  onchange={(code) => setEfficacy(editing.record, code)}
                />
              </div>
            {/if}

            <TzForm id={formId} onsubmit={submit}>
              <div class="form-grid">
                <!-- The subject picker is gone: the sub-tab a record is entered
                 from IS its register, so the field had one lawful value and no
                 decision to offer. It was already frozen while correcting,
                 because moving a record between the three registers would
                 empty one and fill another. -->
                <DateInput label={t("non_field.date")} required bind:value={treatedOn} />
                <!-- Models 3.4 and 3.5: name the store or vehicle from the catalogue,
             because Anexo III Parte I B.b asks for it to be IDENTIFIED and a
             description retyped per record identifies nothing. Naming none
             stays lawful — the free field below takes over. -->
                {#if premisesKind}
                  <TzSelect
                    label={t("non_field.premises")}
                    hint={t("non_field.premises_hint")}
                    items={nameItems(premisesOptions)}
                    nullable
                    nullLabel={t("non_field.premises_none")}
                    bind:value={premisesId}
                  />
                {/if}
                {#if chosenPremises}
                  <!-- Not a <label>: the registry row IS the answer, the printed cell
               is composed from it in Rust, so there is nothing to type here
               and nothing for a label to name. -->
                  <div class="static-field">
                    <span>{t(`non_field.subject_${subjectKind}`)}</span>
                    <p class="detail">{premisesDetail(chosenPremises)}</p>
                    <small>{t("non_field.premises_composed")}</small>
                  </div>
                {:else}
                  <TextInput
                    label={t(`non_field.subject_${subjectKind}`)}
                    required
                    bind:value={subjectDescription}
                  >
                    {#if premisesKind && premisesOptions.length === 0}
                      <small>
                        {t("non_field.premises_missing")}
                        <a href="#/registry">{t("nav.registry")}</a>
                      </small>
                    {/if}
                  </TextInput>
                {/if}
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
                <NumberInput
                  label={quantityLabel(subjectKind)}
                  hint={t("non_field.quantity_hint")}
                  min={0.0001}
                  bind:value={quantity}
                />
                <TzSelect
                  label={t("treatment.product")}
                  items={nameItems(products, (p) => p.commercial_name)}
                  required
                  bind:value={productId}
                />
                <NumberInput
                  label={t("non_field.product_quantity")}
                  min={0.0001}
                  bind:value={productQuantity}
                />
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
                <TextInput label={t("treatment.notes")} bind:value={notes} />
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
                      <button
                        type="button"
                        class="btn-danger"
                        onclick={() => problemRows.splice(index, 1)}
                      >
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
                    <TzCheckbox
                      label={tCode("justification", justification.code)}
                      value={justification.code}
                      bind:group={checkedJustifications}
                    />
                  {/each}
                </div>
              </fieldset>
            </TzForm>
          {/snippet}

          {#snippet actions(formId)}
            <div class="form-actions">
              <button type="submit" form={formId}>{t("form.save")}</button>
              <button type="button" class="btn-cancel" onclick={hideForm}>
                {t("form.cancel")}
              </button>
            </div>
          {/snippet}
        </TzWorkspace>
      {/if}
    {/snippet}
  </TzTabs>
{/if}
