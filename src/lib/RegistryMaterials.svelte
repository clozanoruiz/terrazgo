<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Fertiliser materials section of the catalogue: list + shared create/edit
  // form, the products pattern.
  //
  // Why a registry rather than free capture on each application: Anexo III
  // sección C, letter h, asks for up to EIGHT agronomic values per material,
  // letter i adds heavy metals when sludge is applied, and a farmer spreads the
  // same fertiliser many times in a campaign. Retyping that per application is
  // where wrong data comes from — so the composition lives on a reusable row,
  // and each application freezes only what the printed book shows (the name,
  // the coded kind and the N/P₂O₅/K₂O richness).
  //
  // Materials are not farm-scoped, like products and operators: the same sack
  // is the same sack whichever holding it is spread on.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import { notify, run } from "./notifications.svelte.js";
  import CataloguePicker from "./CataloguePicker.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems } from "./selectItems.js";
  import TzCombobox from "./TzCombobox.svelte";

  let { countryCode = "es" } = $props();

  let materials = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedMaterials = $derived(sortedBy(materials, (d) => d.material.name));
  // Session-wide reference data (lib/lookups.svelte.js).
  const manureTreatments = $derived(lookups.manureTreatments);
  const nutrientKinds = $derived(lookups.nutrientKinds);
  let materialKinds = $state([]);
  // Nutrient options per kind, fetched on demand: three separate catalogues
  // sharing a number space, so the kind chooses the list.
  let nutrientOptions = $state({});
  // The 1243-row product list, narrowed to the chosen kind of material.
  let detailOptions = $state([]);
  let loading = $state(true);

  let formOpen = $state(false);
  let editingId = $state(null);
  let name = $state("");
  let materialCode = $state("");
  let detailCode = $state(null);
  let detailName = $state("");
  let supplierName = $state("");
  // C.e's three identifiers are mutually exclusive, so the form asks WHICH one
  // and then for the number — a farmer cannot fill two by accident.
  let supplierRegistry = $state("rega");
  let supplierNumber = $state("");
  let manureTreatment = $state("");
  let density = $state("");
  let notes = $state("");
  let nutrientRows = $state([]);

  run(async () => {
    [materials, materialKinds] = await Promise.all([
      invoke("list_fertiliser_materials"),
      // Country-scoped, so it is not session-wide reference data.
      invoke("list_fertiliser_material_kinds", { countryCode }),
      loadLookups(),
    ]);
  }).finally(() => (loading = false));

  function ensureNutrients(kindCode) {
    if (!kindCode || nutrientOptions[kindCode]) return;
    run(async () => {
      const codes = await invoke("list_nutrient_codes", { countryCode, kindCode });
      nutrientOptions = { ...nutrientOptions, [kindCode]: codes };
    });
  }

  function loadDetails(kindCode) {
    run(async () => {
      detailOptions = await invoke("list_fertiliser_material_details", {
        countryCode,
        materialCode: kindCode || null,
      });
      // Editing an existing material: the stored code carries no name of its
      // own, so it is resolved once the narrowed list is in.
      if (detailCode && !detailName) {
        detailName = detailOptions.find((o) => o.code === detailCode)?.name ?? "";
      }
    });
  }

  function emptyNutrient() {
    return { kindCode: "macro", nutrientCode: "", percentage: "" };
  }

  /// Take the composition the catalogue publishes for the chosen product, so
  /// Anexo III C.h's eight values need not be copied off the sack by hand.
  ///
  /// Explicitly asked for, and it never overwrites: a line the farmer already
  /// entered wins, because the label in their hand is the source of truth and
  /// this snapshot rides app releases. Heavy metals are never offered — the
  /// provider's columns mix percentages with mg/kg and nothing in the file
  /// tells them apart, so C.i's metals stay hand-entered.
  function fillFromCatalogue() {
    run(async () => {
      const lines = await invoke("fertiliser_material_composition", {
        countryCode,
        detailCode: detailCode,
      });
      const already = (line) =>
        nutrientRows.some(
          (row) => row.kindCode === line.kind_code && row.nutrientCode === line.nutrient_code,
        );
      const added = lines.filter((line) => !already(line));
      for (const line of added) {
        ensureNutrients(line.kind_code);
        nutrientRows.push({
          kindCode: line.kind_code,
          nutrientCode: line.nutrient_code,
          percentage: line.percentage,
        });
      }
      // Drop the blank row the form may be showing, now that there is content.
      nutrientRows = nutrientRows.filter((row) => row.nutrientCode !== "" || row.percentage !== "");
      notify(
        added.length > 0
          ? t("material.filled", { count: added.length })
          : t("material.filled_none"),
      );
    });
  }

  function showForm(detail = null) {
    const material = detail?.material ?? null;
    editingId = material?.id ?? null;
    name = material?.name ?? "";
    materialCode = material?.material_code ?? "";
    detailCode = material?.material_detail_code ?? null;
    detailName = "";
    supplierName = material?.supplier_name ?? "";
    supplierRegistry = material?.supplier_tax_id
      ? "tax_id"
      : material?.supplier_nima
        ? "nima"
        : "rega";
    supplierNumber =
      material?.supplier_rega ?? material?.supplier_tax_id ?? material?.supplier_nima ?? "";
    manureTreatment = material?.manure_treatment_code ?? "";
    density = material?.density_kg_l ?? "";
    notes = material?.notes ?? "";
    nutrientRows = (detail?.nutrients ?? []).map((n) => ({
      kindCode: n.kind_code,
      nutrientCode: n.nutrient_code,
      percentage: n.percentage,
    }));
    for (const row of nutrientRows) ensureNutrients(row.kindCode);
    ensureNutrients("macro");
    loadDetails(materialCode);
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  function onKindChosen() {
    // A different kind of material publishes a different product list, so a
    // detail chosen under the old one no longer means anything.
    detailCode = null;
    detailName = "";
    loadDetails(materialCode);
  }

  function submit(event) {
    event.preventDefault();
    const number = supplierNumber.trim() || null;
    const payload = {
      name: name.trim(),
      material_code: materialCode,
      material_detail_code: detailCode,
      supplier_name: supplierName.trim() || null,
      supplier_rega: supplierRegistry === "rega" ? number : null,
      supplier_tax_id: supplierRegistry === "tax_id" ? number : null,
      supplier_nima: supplierRegistry === "nima" ? number : null,
      manure_treatment_code: manureTreatment || null,
      density_kg_l: density === "" ? null : Number(density),
      notes: notes.trim() || null,
      nutrients: nutrientRows
        .filter((row) => row.nutrientCode !== "" && row.percentage !== "")
        .map((row) => ({
          kind_code: row.kindCode,
          nutrient_code: row.nutrientCode,
          percentage: Number(row.percentage),
        })),
    };

    run(async () => {
      if (editingId) {
        await invoke("update_fertiliser_material", {
          materialId: editingId,
          update: { ...payload, id: editingId },
        });
      } else {
        await invoke("create_fertiliser_material", { material: payload });
      }
      notify(t("message.material_saved"));
      hideForm();
      materials = await invoke("list_fertiliser_materials");
    });
  }

  function remove(material) {
    run(async () => {
      if (!(await confirmDialog(t("material.delete_confirm")))) return;
      await invoke("delete_fertiliser_material", { materialId: material.id });
      notify(t("message.material_deleted"));
      materials = await invoke("list_fertiliser_materials");
    });
  }

  function kindName(code) {
    return materialKinds.find((k) => k.code === code)?.name ?? code;
  }

  /// The three figures the record book prints, if the label states them.
  function richness(detail) {
    const value = (code) =>
      detail.nutrients.find((n) => n.kind_code === "macro" && n.nutrient_code === code)?.percentage;
    const parts = [
      ["N", value("1")],
      ["P₂O₅", value("6")],
      ["K₂O", value("9")],
    ].filter(([, v]) => v !== undefined);
    return parts.map(([symbol, v]) => `${symbol} ${v}`).join(" · ");
  }
</script>

<div class="view-head">
  <h3>{t("material.title")}</h3>
  <button type="button" onclick={() => showForm()}>{t("material.new")}</button>
</div>
<p class="detail">{t("material.intro")}</p>

{#if formOpen}
  <form onsubmit={submit}>
    <div class="form-grid">
      <label><span>{t("material.name")}</span><input required bind:value={name} /></label>
      <!-- MAT_FERTI: a closed list the decree enumerates, in its own order. -->
      <TzSelect
        label={t("material.kind")}
        items={materialKinds.map((kind) => ({ value: kind.code, label: kind.name }))}
        required
        bind:value={materialCode}
        onchange={onKindChosen}
      />
      <label>
        <span>{t("material.detail")}</span>
        <CataloguePicker
          bind:name={detailName}
          bind:code={detailCode}
          options={detailOptions}
          placeholder={t("material.detail")}
        />
        <small>{t("material.detail_hint")}</small>
      </label>
      <TzSelect
        label={t("material.manure_treatment")}
        items={codeItems(manureTreatments, "manure_treatment")}
        nullable
        bind:value={manureTreatment}
      />
      <label>
        <span>{t("material.density")}</span>
        <input type="number" step="any" min="0.001" bind:value={density} />
      </label>
      <label><span>{t("treatment.notes")}</span><input bind:value={notes} /></label>
    </div>

    <fieldset class="subsection">
      <legend>{t("material.supplier_section")}</legend>
      <p class="detail">{t("material.supplier_hint")}</p>
      <div class="form-grid">
        <label>
          <span>{t("material.supplier_name")}</span>
          <input bind:value={supplierName} />
        </label>
        <!-- The three supplier registries are mutually exclusive by CHECK; the
             order is the decree's, not alphabetical. -->
        <TzSelect
          label={t("material.supplier_registry")}
          items={[
            { value: "rega", label: t("material.supplier_rega") },
            { value: "tax_id", label: t("material.supplier_tax_id") },
            { value: "nima", label: t("material.supplier_nima") },
          ]}
          bind:value={supplierRegistry}
        />
        <label>
          <span>{t("material.supplier_number")}</span>
          <input bind:value={supplierNumber} />
        </label>
      </div>
    </fieldset>

    <fieldset class="subsection">
      <legend>{t("material.composition_section")}</legend>
      <p class="detail">{t("material.composition_hint")}</p>
      {#if detailCode}
        <div class="selector-buttons">
          <button type="button" onclick={fillFromCatalogue}>{t("material.fill")}</button>
        </div>
        <p class="detail">{t("material.fill_hint")}</p>
      {/if}
      {#each nutrientRows as row, index (row)}
        <div class="form-grid plot-row">
          <TzSelect
            label={t("material.nutrient_kind")}
            items={codeItems(nutrientKinds, "nutrient_kind")}
            bind:value={row.kindCode}
            onchange={() => {
              row.nutrientCode = "";
              ensureNutrients(row.kindCode);
            }}
          />
          <!-- MICRONUTRIENTES alone is 99 entries, past the listbox cap. -->
          <TzCombobox
            label={t("material.nutrient")}
            items={(nutrientOptions[row.kindCode] ?? []).map((option) => ({
              value: option.code,
              label: option.name,
            }))}
            bind:value={row.nutrientCode}
          />
          <label>
            <span>{t("material.percentage")}</span>
            <input type="number" step="any" min="0" max="100" bind:value={row.percentage} />
          </label>
          <button type="button" class="btn-danger" onclick={() => nutrientRows.splice(index, 1)}>
            {t("treatment.remove")}
          </button>
        </div>
      {/each}
      <button type="button" onclick={() => nutrientRows.push(emptyNutrient())}>
        {t("material.add_nutrient")}
      </button>
    </fieldset>

    <div class="form-actions">
      <button type="submit">{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
    </div>
  </form>
{/if}

{#if loading}
  <Skeleton />
{:else}
  <ul class="card-list">
    {#each sortedMaterials as detail (detail.material.id)}
      <li class="card">
        <div class="stack">
          <strong>{detail.material.name}</strong>
          <span class="detail">{kindName(detail.material.material_code)}</span>
          {#if richness(detail)}
            <span class="detail">{richness(detail)}</span>
          {/if}
        </div>
        <button type="button" onclick={() => showForm(detail)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => remove(detail.material)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>
  {#if materials.length === 0}
    <p>{t("material.empty")}</p>
  {/if}
{/if}
