<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, eco-schemes tab: model 9.1, pastoreo extensivo.
  //
  // This is the record book's THIRD decree. RD 1048/2022 art. 30.2 ter obliges
  // the annotation when the grazing dates differ from those declared in the
  // solicitud única, within one month — and the model counts that month from
  // the END of grazing, which is why leaving the end date empty is a first-class
  // state here and not an unfinished form.
  //
  // The register is conditional on claiming an ecorrégimen, which the app cannot
  // know by any route (the solicitud única is unreachable). So nothing here nags
  // a holding that records nothing, and the practice is the farmer's to state.
  import { formatDate, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
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

  // Session-wide reference data, read from the module rather than drilled
  // through every parent (lib/lookups.svelte.js).
  //
  // The practices are narrowed to the three this FORM offers. The repository
  // accepts a fourth, `plant_cover`: art. 42.1.c counts pastoreo as one of the
  // three ways a live cover is maintained, and model 9.4 prints it as a column.
  // Such a grazing is entered from the covers form, which knows the cover it
  // maintains — there is nothing to point at from here.
  const GRAZING_PRACTICES = ["extensive_grazing", "sustainable_mowing", "communal_pasture"];
  const practices = $derived(
    lookups.ecoPractices.filter((practice) => GRAZING_PRACTICES.includes(practice.code)),
  );

  // The 198-row species catalogue takes a country, so it is per-holding
  // reference data and loads here rather than in the session-wide store.
  let species = $state([]);
  let records = $state([]);
  // The holding's own REGA, which prefills a new animal line. Read here rather
  // than drilled through the book shell: it lives on the Spanish extension, and
  // this is the only register that asks for it.
  let farmRega = $state("");
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      const [loaded, catalogue, farm] = await Promise.all([
        invoke("list_grazing_records", { seasonId, farmId }),
        invoke("list_animal_species", { countryCode }),
        invoke("get_farm", { farmId }),
      ]);
      records = loaded;
      species = catalogue;
      farmRega = farm.es?.rega_code ?? "";
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  function speciesName(code) {
    return species.find((entry) => entry.code === code)?.name ?? code;
  }

  /// A new animal line starts on the farm's own REGA — the everyday case is a
  /// holding grazing its own animals. Third-party animals carry their owner's
  /// code, so the field stays editable.
  function emptyAnimalRow() {
    return { speciesCode: "", regaCode: farmRega, animalCount: "" };
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  let practiceCode = $state("extensive_grazing");
  let plotGroupRef = $state("");
  let startedOn = $state("");
  let endedOn = $state("");
  let notes = $state("");
  let chosenPlots = $state([]);
  let animalRows = $state([emptyAnimalRow()]);

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    practiceCode = detail?.record.practice_code ?? "extensive_grazing";
    plotGroupRef = detail?.record.plot_group_ref ?? "";
    startedOn = detail?.record.started_on ?? "";
    endedOn = detail?.record.ended_on ?? "";
    notes = detail?.record.notes ?? "";
    chosenPlots = detail?.plots.map((p) => p.plot_id) ?? [];
    animalRows = detail?.animals.length
      ? detail.animals.map((a) => ({
          speciesCode: a.species_code,
          regaCode: a.rega_code,
          animalCount: a.animal_count,
        }))
      : [emptyAnimalRow()];
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// The grazing window. An empty end date is "still grazing", never a blank:
  /// the month the annotation is due runs from the END, so the reader has to
  /// see which records have not closed.
  function grazedRange(record) {
    const start = formatDate(record.started_on);
    return `${start} – ${record.ended_on ? formatDate(record.ended_on) : t("grazing.ongoing")}`;
  }

  /// Every animal line of one record in a cell: count, species and the REGA
  /// they belong to.
  function animalsCell(detail) {
    return detail.animals
      .map((a) =>
        t("grazing.animal_detail", {
          count: a.animal_count,
          species: speciesName(a.species_code),
          rega: a.rega_code,
        }),
      )
      .join(" · ");
  }

  function togglePlot(plotId, checked) {
    chosenPlots = checked
      ? [...chosenPlots, plotId]
      : chosenPlots.filter((existing) => existing !== plotId);
  }

  async function submit() {
    const payload = {
      practice_code: practiceCode,
      plot_group_ref: plotGroupRef.trim() || null,
      started_on: startedOn,
      // An empty end date is "still grazing", which is exactly what the
      // register wants to say — never today's date as a stand-in.
      ended_on: endedOn || null,
      notes: notes.trim() || null,
      plot_ids: chosenPlots,
      animals: animalRows
        .filter((row) => row.speciesCode && row.regaCode)
        .map((row) => ({
          species_code: row.speciesCode,
          rega_code: row.regaCode.trim(),
          animal_count: Number(row.animalCount),
        })),
    };

    if (editingId) {
      await invoke("update_grazing_record", {
        grazingRecordId: editingId,
        update: payload,
      });
    } else {
      await invoke("create_grazing_record", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("grazing.delete_confirm")))) return;
      await invoke("delete_grazing_record", { grazingRecordId: record.id });
      hideForm();
      load();
    });
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <div class="view-head">
    <h3>{t("grazing.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showForm()}>
        + {t("grazing.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("grazing.intro")}</p>

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(startedOn) : t("grazing.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"grazings"}>
            <thead>
              <tr>
                <th>{t("column.date")}</th>
                <th>{t("column.practice")}</th>
                <th>{t("column.animals")}</th>
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
                      {grazedRange(detail.record)}
                    </button>
                  </td>
                  <td class="col-muted">
                    {tCode("eco_practice", detail.record.practice_code)}
                  </td>
                  <td class="col-muted">{animalsCell(detail)}</td>
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
            label={t("grazing.practice")}
            items={codeItems(practices, "eco_practice")}
            required
            bind:value={practiceCode}
          />
          <DateInput label={t("grazing.started_on")} required bind:value={startedOn} />
          <DateInput
            label={t("grazing.ended_on")}
            hint={t("grazing.ended_on_hint")}
            bind:value={endedOn}
          />
          <TextInput label={t("grazing.plot_group_ref")} bind:value={plotGroupRef}>
            <small class="detail">{t("grazing.plot_group_ref_hint")}</small>
          </TextInput>
          <TextInput label={t("treatment.notes")} bind:value={notes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("grazing.plots_section")}</legend>
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

        <fieldset class="subsection">
          <legend>{t("grazing.animals_section")}</legend>
          <p class="detail">{t("grazing.animals_hint")}</p>
          {#each animalRows as row, index (row)}
            <div class="form-grid plot-row">
              <TzCombobox
                label={t("grazing.species")}
                items={nameItems(
                  species,
                  (entry) => entry.name,
                  (entry) => entry.code,
                )}
                required
                bind:value={row.speciesCode}
              />
              <TextInput label={t("grazing.rega")} required bind:value={row.regaCode} />
              <NumberInput
                label={t("grazing.animal_count")}
                min={1}
                integer
                required
                bind:value={row.animalCount}
              />
              {#if animalRows.length > 1}
                <button
                  type="button"
                  class="btn-danger"
                  onclick={() => animalRows.splice(index, 1)}
                >
                  {t("treatment.remove")}
                </button>
              {/if}
            </div>
          {/each}
          <button type="button" onclick={() => animalRows.push(emptyAnimalRow())}>
            {t("grazing.add_animals")}
          </button>
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
