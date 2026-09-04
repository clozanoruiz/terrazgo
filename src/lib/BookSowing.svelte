<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, crops tab: the sowing register.
  //
  // Harvest's mirror image — the two bracket a crop — which is why it lives in
  // `terrazgo-core` beside it rather than in a module, and why it sits in the
  // crops tab: this is how the crop above it began.
  //
  // It is a register in its own right AND the source of two columns of the
  // third decree's pages: model 9.2's "Siembra" (RD 1048/2022 art. 31) and
  // model 9.3's "siembra en seco" and "inundación" (art. 45.2). Nothing here
  // mentions an eco-scheme, because a sowing belongs to no decree in
  // particular; what makes one evidence of a cultivo bajo agua is the flooding
  // date.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import DateInput from "./DateInput.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import { lookups } from "./lookups.svelte.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId, seasonId, plots, crops } = $props();

  let records = $state([]);
  let loading = $state(true);
  const kinds = $derived(lookups.sowingKinds);

  load();

  function load() {
    run(async () => {
      records = await invoke("list_sowing_records", { seasonId, farmId });
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  /// The crops declared on a plot, as the owned dropdown's items. An empty
  /// first entry is offered because the crop is optional: a sowing is a real
  /// record whether or not the crop row exists yet.
  function cropItems(plotId) {
    return [
      { value: "", label: t("sowing.crop_unset") },
      ...nameItems(
        crops.filter((crop) => crop.plot_id === plotId),
        (crop) => crop.species_name,
        (crop) => crop.id,
      ),
    ];
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  // Defaulted rather than left empty: a sowing is the common case, and the
  // column is NOT NULL because the export has to state which it was.
  let kindCode = $state("sowing");
  let sownOn = $state("");
  let sowingEndDate = $state("");
  let floodedOn = $state("");
  let seedQuantityKg = $state("");
  let notes = $state("");
  // One entry per chosen plot: { plotId, cropId }. The crop is what the sowing
  // started, and it is frozen onto the row at save time.
  let chosenPlots = $state([]);

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    kindCode = detail?.record.kind_code ?? "sowing";
    sownOn = detail?.record.sown_on ?? "";
    sowingEndDate = detail?.record.sowing_end_date ?? "";
    floodedOn = detail?.record.flooded_on ?? "";
    seedQuantityKg = detail?.record.seed_quantity_kg ?? "";
    notes = detail?.record.notes ?? "";
    chosenPlots = detail?.plots.map((p) => ({ plotId: p.plot_id, cropId: p.crop_id ?? "" })) ?? [];
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// What one record's plots say in a cell: the crop each sowing started, or
  /// the plot's own name where no crop row was named.
  function plotsCell(detail) {
    return detail.plots.map((p) => p.crop_name_snapshot ?? plotName(p.plot_id)).join(", ");
  }

  /// A single day, or the interval the register allows when the sowing ran
  /// over several.
  function sownRange(record) {
    const start = formatDate(record.sown_on);
    return record.sowing_end_date ? `${start} – ${formatDate(record.sowing_end_date)}` : start;
  }

  function togglePlot(plotId, checked) {
    chosenPlots = checked
      ? [...chosenPlots, { plotId, cropId: "" }]
      : chosenPlots.filter((entry) => entry.plotId !== plotId);
  }

  function chosen(plotId) {
    return chosenPlots.find((entry) => entry.plotId === plotId);
  }

  async function submit() {
    const payload = {
      kind_code: kindCode,
      sown_on: sownOn,
      // Empty is one day's work, never the start date repeated — that would
      // claim an interval nobody stated.
      sowing_end_date: sowingEndDate || null,
      // Empty is "not flooded", which for every crop but rice is the permanent
      // answer. A rice grower fills it weeks later, by correcting this record.
      flooded_on: floodedOn || null,
      seed_quantity_kg: seedQuantityKg === "" ? null : Number(seedQuantityKg),
      notes: notes.trim() || null,
      plots: chosenPlots.map((entry) => ({
        plot_id: entry.plotId,
        crop_id: entry.cropId || null,
      })),
    };

    if (editingId) {
      await invoke("update_sowing_record", { sowingRecordId: editingId, update: payload });
    } else {
      await invoke("create_sowing_record", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("sowing.delete_confirm")))) return;
      await invoke("delete_sowing_record", { sowingRecordId: record.id });
      hideForm();
      load();
    });
  }
</script>

{#if !loading}
  <div class="view-head">
    <h3>{t("sowing.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showForm()}>
        + {t("sowing.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("sowing.intro")}</p>

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(sownOn) : t("sowing.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"sowings"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.kind")}</th>
                <th>{t("column.plots")}</th>
                <th>{t("column.flooded")}</th>
                <th class="col-num">{t("column.seed_kg")}</th>
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
                      {sownRange(detail.record)}
                    </button>
                  </td>
                  <td class="col-muted">{tCode("sowing_kind", detail.record.kind_code)}</td>
                  <td class="col-muted">{plotsCell(detail)}</td>
                  <td class="col-muted">
                    {detail.record.flooded_on ? formatDate(detail.record.flooded_on) : ""}
                  </td>
                  <td class="col-muted col-num">
                    {detail.record.seed_quantity_kg == null
                      ? ""
                      : formatNumber(detail.record.seed_quantity_kg)}
                  </td>
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
            label={t("sowing.kind")}
            items={codeItems(kinds, "sowing_kind")}
            required
            bind:value={kindCode}
          />
          <DateInput label={t("sowing.sown_on")} required bind:value={sownOn} />
          <DateInput
            label={t("sowing.sowing_end_date")}
            hint={t("sowing.sowing_end_date_hint")}
            bind:value={sowingEndDate}
          />
          <DateInput
            label={t("sowing.flooded_on")}
            hint={t("sowing.flooded_on_hint")}
            bind:value={floodedOn}
          />
          <NumberInput
            label={t("sowing.seed_quantity")}
            hint={t("sowing.seed_quantity_hint")}
            min={0}
            bind:value={seedQuantityKg}
          />
          <TextInput label={t("treatment.notes")} bind:value={notes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("sowing.plots_section")}</legend>
          <div class="checkbox-list">
            {#each plots as entry (entry.plot.id)}
              <TzCheckbox
                label={entry.plot.name}
                checked={!!chosen(entry.plot.id)}
                onchange={(next) => togglePlot(entry.plot.id, next)}
              />
            {/each}
          </div>
          <!-- The crop pickers appear only for the plots actually chosen, so an
             unpicked plot costs one chip above instead of a whole row. -->
          {#if chosenPlots.length > 0}
            <p class="detail">{t("sowing.plots_hint")}</p>
            <div class="form-grid">
              {#each chosenPlots as picked (picked.plotId)}
                <TzSelect
                  label={plotName(picked.plotId)}
                  items={cropItems(picked.plotId)}
                  bind:value={picked.cropId}
                />
              {/each}
            </div>
          {/if}
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
