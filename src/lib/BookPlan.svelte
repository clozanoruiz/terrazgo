<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, model section 7.1: the plan de abonado.
  //
  // What this form asks is deliberately much LESS than a plan de abonado is.
  // RD 1051/2022 art. 6 defines the plan as a document — every recinto of the
  // production unit, soil parameters, the water available, the recommended dose
  // of each nutrient with its moment, material, form of application and
  // machinery, and the ammonia and greenhouse-gas measures of anexo V — drawn
  // up with advice and KEPT beside the book. Art. 5.a defines what goes IN the
  // book: expected yield, preceding crop, the N/P₂O₅/K₂O needs, and the date
  // the plan was drawn up. That is this form, and the SIEX twin agreeing with
  // the article is the confirmation.
  //
  // The printed 7.1 table is not entered anywhere: its aportadas and acumuladas
  // are computed from section 6's own records, because a stored copy could
  // disagree with the register above it.
  import { formatDate, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import SpeciesPicker from "./SpeciesPicker.svelte";

  let { farmId, seasonId, crops } = $props();

  let plans = $state([]);
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      plans = await invoke("list_fertilisation_plans", { seasonId, farmId });
    }).finally(() => (loading = false));
  }

  function cropLabel(cropId) {
    const crop = crops.find((c) => c.id === cropId);
    if (!crop) return cropId;
    return `${crop.species_name}${crop.variety ? ` — ${crop.variety}` : ""}`;
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  let needsN = $state("");
  let needsP2o5 = $state("");
  let needsK2o = $state("");
  let expectedYield = $state("");
  let precedingName = $state("");
  let precedingCode = $state(null);
  let drawnUpOn = $state("");
  let toolGenerated = $state(false);
  let notes = $state("");
  let chosenCrops = $state([]);

  function showForm(detail = null) {
    editingId = detail?.plan.id ?? null;
    needsN = detail?.plan.needs_n_kg_ha ?? "";
    needsP2o5 = detail?.plan.needs_p2o5_kg_ha ?? "";
    needsK2o = detail?.plan.needs_k2o_kg_ha ?? "";
    expectedYield = detail?.plan.expected_yield_kg_ha ?? "";
    precedingCode = detail?.plan.preceding_crop_code ?? null;
    precedingName = "";
    drawnUpOn = detail?.plan.drawn_up_on ?? "";
    toolGenerated = detail?.plan.tool_generated ?? false;
    notes = detail?.plan.notes ?? "";
    chosenCrops = [...(detail?.crop_ids ?? [])];
    formOpen = true;
  }

  function toggleCrop(cropId, checked) {
    chosenCrops = checked
      ? [...chosenCrops, cropId]
      : chosenCrops.filter((existing) => existing !== cropId);
  }

  function submit(event) {
    event.preventDefault();
    const payload = {
      needs_n_kg_ha: Number(needsN),
      needs_p2o5_kg_ha: Number(needsP2o5),
      needs_k2o_kg_ha: Number(needsK2o),
      expected_yield_kg_ha: Number(expectedYield),
      // The picker keeps a free-typed name without a code; only the code is
      // stored, because what the plan names is a crop of the PRODUCTOS list.
      preceding_crop_code: precedingCode,
      drawn_up_on: drawnUpOn,
      tool_generated: toolGenerated,
      notes: notes.trim() || null,
      crop_ids: chosenCrops,
    };

    run(async () => {
      if (editingId) {
        await invoke("update_fertilisation_plan", {
          planId: editingId,
          update: { ...payload, id: editingId },
        });
      } else {
        await invoke("create_fertilisation_plan", {
          plan: { ...payload, season_id: seasonId, farm_id: farmId },
        });
      }
      notify(t("message.plan_saved"));
      formOpen = false;
      load();
    });
  }

  function remove(plan) {
    run(async () => {
      if (!(await confirmDialog(t("plan.delete_confirm")))) return;
      await invoke("delete_fertilisation_plan", { planId: plan.id });
      notify(t("message.plan_deleted"));
      load();
    });
  }
</script>

{#if !loading}
  <div class="view-head">
    <h3>{t("plan.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={crops.length === 0} onclick={() => showForm()}>
        + {t("plan.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("plan.intro")}</p>
  <p class="detail">{t("plan.binding")}</p>
  {#if crops.length === 0}
    <p class="detail">{t("plan.no_crops")}</p>
  {/if}

  <ul class="card-list">
    {#each plans as detail (detail.plan.id)}
      <li class="card">
        <div class="stack">
          <strong>{detail.crop_ids.map(cropLabel).join(", ")}</strong>
          <span class="detail">
            {t("plan.needs_detail", {
              n: detail.plan.needs_n_kg_ha,
              p: detail.plan.needs_p2o5_kg_ha,
              k: detail.plan.needs_k2o_kg_ha,
            })}
            · {t("plan.yield_detail", { yield: detail.plan.expected_yield_kg_ha })}
          </span>
          <span class="detail">{formatDate(detail.plan.drawn_up_on)}</span>
        </div>
        <button type="button" onclick={() => showForm(detail)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => remove(detail.plan)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>

  {#if formOpen}
    <form onsubmit={submit}>
      <div class="form-grid">
        <DateInput label={t("plan.drawn_up_on")} required bind:value={drawnUpOn} />
        <label>
          <span>{t("plan.expected_yield")}</span>
          <input type="number" step="any" min="0.001" required bind:value={expectedYield} />
        </label>
        <label>
          <span>{t("plan.preceding_crop")}</span>
          <SpeciesPicker bind:name={precedingName} bind:code={precedingCode} />
        </label>
        <label>
          <span>{t("treatment.notes")}</span>
          <input bind:value={notes} />
        </label>
        <label class="inline">
          <input type="checkbox" bind:checked={toolGenerated} />
          <span>{t("plan.tool_generated")}</span>
        </label>
      </div>

      <fieldset class="subsection">
        <legend>{t("plan.needs_section")}</legend>
        <div class="form-grid">
          <label>
            <span>{t("plan.needs_n")}</span>
            <input type="number" step="any" min="0" required bind:value={needsN} />
          </label>
          <label>
            <span>{t("plan.needs_p2o5")}</span>
            <input type="number" step="any" min="0" required bind:value={needsP2o5} />
          </label>
          <label>
            <span>{t("plan.needs_k2o")}</span>
            <input type="number" step="any" min="0" required bind:value={needsK2o} />
          </label>
        </div>
      </fieldset>

      <fieldset class="subsection">
        <legend>{t("plan.crops")}</legend>
        <div class="checkbox-list">
          {#each crops as crop (crop.id)}
            <label class="inline">
              <input
                type="checkbox"
                checked={chosenCrops.includes(crop.id)}
                onchange={(event) => toggleCrop(crop.id, event.currentTarget.checked)}
              />
              <span>{cropLabel(crop.id)}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" onclick={() => (formOpen = false)}>{t("form.cancel")}</button>
      </div>
    </form>
  {/if}
{/if}
