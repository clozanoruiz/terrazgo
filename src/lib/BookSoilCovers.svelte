<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, eco-schemes tab: models 9.4 and 9.5, las cubiertas.
  //
  // One form behind two printed pages, as the operations form is: RD 1048/2022
  // art. 42 governs a live cover (P6, model 9.4) and art. 43 an inert one of
  // triturated pruning residue (P7, model 9.5), and the practice decides which
  // page a record lands on.
  //
  // The shape of the form is the shape of the article. **Art. 42 is THREE
  // annotations with three different deadlines** — the establishment date, the
  // two widths, and the maintenance — which the printed model collapses into
  // one row. So the widths are a block that stays empty until they are
  // measured, and the maintenance is a repeated line at the foot.
  //
  // Those maintenance lines are not this register's rows. A siega and a
  // desbroce are cultural operations and a pastoreo is a grazing, whichever
  // land they happen on, so the backend writes each into the register that owns
  // it — in the same transaction as the cover, inheriting its plots and its
  // practice. That inheritance is what makes a sub-form safe here: a line
  // cannot name a plot the cover was never established over.
  import { formatDate, formatNumber, t, tCode } from "../i18n.js";
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

  // The maintenance kind that is a grazing rather than a cultural operation.
  // Mirrors `module_ecoscheme::models::GRAZING_MAINTENANCE`; the backend
  // refuses anything outside the three model 9.4 prints.
  const GRAZING = "grazing";

  // Arts. 42 and 43 are the only clauses that establish a cover — the same two
  // the repository accepts.
  const COVER_PRACTICES = ["plant_cover", "inert_cover"];
  const practices = $derived(
    lookups.ecoPractices.filter((practice) => COVER_PRACTICES.includes(practice.code)),
  );

  let records = $state([]);
  // Narrowed per practice by the backend (art. 42.1.a's "espontánea o
  // sembrada" against art. 43.1.a's "restos de poda"), so it reloads when the
  // practice changes rather than being filtered here — the codes stay in one
  // place, with the contract test that watches them.
  let coverTypes = $state([]);
  let species = $state([]);
  let farmRega = $state("");
  let loading = $state(true);

  load();

  function load() {
    run(async () => {
      const [loaded, animalSpecies, farm] = await Promise.all([
        invoke("list_soil_covers", { seasonId, farmId }),
        invoke("list_animal_species", { countryCode }),
        invoke("get_farm", { farmId }),
      ]);
      records = loaded;
      species = animalSpecies;
      farmRega = farm.es?.rega_code ?? "";
    }).finally(() => (loading = false));
  }

  function plotName(plotId) {
    return plots.find(({ plot }) => plot.id === plotId)?.plot.name ?? plotId;
  }

  function coverTypeName(code) {
    return coverTypes.find((entry) => entry.code === code)?.name ?? code;
  }

  function maintenanceLabel(kindCode) {
    return kindCode === GRAZING
      ? t("cover.maintenance_grazing")
      : tCode("cultural_operation_kind", kindCode);
  }

  const maintenanceItems = $derived([
    ...lookups.culturalOperationKinds
      .filter((kind) => kind.code === "mowing" || kind.code === "brush_cutting")
      .map((kind) => ({
        value: kind.code,
        label: tCode("cultural_operation_kind", kind.code),
      })),
    { value: GRAZING, label: t("cover.maintenance_grazing") },
  ]);

  function emptyAnimalRow() {
    return { speciesCode: "", regaCode: farmRega, animalCount: "" };
  }

  function emptyMaintenanceRow() {
    return { id: "", kindCode: "mowing", performedOn: "", animals: [emptyAnimalRow()] };
  }

  let formOpen = $state(false);
  let editingId = $state(null);
  let practiceCode = $state("plant_cover");
  let coverTypeCode = $state("");
  let establishedOn = $state("");
  let widthM = $state("");
  let freeCanopyWidthM = $state("");
  let widthsStatedOn = $state("");
  let notes = $state("");
  let chosenPlots = $state([]);
  let maintenanceRows = $state([]);

  // Art. 43 asks for no maintenance of an inert cover and model 9.5 prints no
  // such columns, so the sub-form disappears rather than offering something the
  // backend would refuse.
  const takesMaintenance = $derived(practiceCode === "plant_cover");

  // Reloading the cover-type list is a real fetch, so it hangs off the chosen
  // practice rather than being run once at mount.
  $effect(() => {
    const practice = practiceCode;
    invoke("list_cover_types", { countryCode, practiceCode: practice })
      .then((loaded) => {
        if (practice !== practiceCode) return;
        coverTypes = loaded;
        // A kind the new practice cannot be is cleared rather than carried
        // over: an inert cover described as "sembrada" is a wrong record.
        if (coverTypeCode && !loaded.some((entry) => entry.code === coverTypeCode)) {
          coverTypeCode = "";
        }
      })
      // A catalogue that cannot be read leaves the picker empty; it is a
      // typing aid, not a gate on recording the cover.
      .catch(() => (coverTypes = []));
  });

  function showForm(detail = null) {
    editingId = detail?.record.id ?? null;
    practiceCode = detail?.record.practice_code ?? "plant_cover";
    coverTypeCode = detail?.record.cover_type_code ?? "";
    establishedOn = detail?.record.established_on ?? "";
    widthM = detail?.record.width_m ?? "";
    freeCanopyWidthM = detail?.record.free_canopy_width_m ?? "";
    widthsStatedOn = detail?.record.widths_stated_on ?? "";
    notes = detail?.record.notes ?? "";
    chosenPlots = detail?.plots.map((p) => p.plot_id) ?? [];
    maintenanceRows =
      detail?.maintenance.map((line) => ({
        id: line.id,
        kindCode: line.kind_code,
        performedOn: line.performed_on,
        animals: line.animals.length
          ? line.animals.map((a) => ({
              speciesCode: a.species_code,
              regaCode: a.rega_code,
              animalCount: a.animal_count,
            }))
          : [emptyAnimalRow()],
      })) ?? [];
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(records.find((d) => d.record.id === editingId) ?? null);

  /// The widths cell. Blank until they are measured, because art. 42 makes them
  /// their own annotation on their own deadline — "not yet" is a state, not a
  /// gap, and `widths_stated_on` is what separates the two.
  function widthsCell(record) {
    if (!record.widths_stated_on) return t("cover.widths_pending");
    return `${formatNumber(record.width_m)} · ${formatNumber(record.free_canopy_width_m)}`;
  }

  /// The maintenance lines of one record in a cell: what was done, and when.
  function maintenanceCell(detail) {
    return detail.maintenance
      .map((line) => `${maintenanceLabel(line.kind_code)} ${formatDate(line.performed_on)}`)
      .join(" · ");
  }

  function togglePlot(plotId, checked) {
    chosenPlots = checked
      ? [...chosenPlots, plotId]
      : chosenPlots.filter((existing) => existing !== plotId);
  }

  /// Empty inputs become `null`, never 0: an unmeasured width is unknown, and
  /// a zero would say the cover has no width at all.
  function optionalNumber(value) {
    return value === "" || value === null ? null : Number(value);
  }

  async function submit() {
    const payload = {
      practice_code: practiceCode,
      cover_type_code: coverTypeCode,
      established_on: establishedOn,
      // The three move together — one annotation, one deadline (art. 42.1.e /
      // 43.1.b) — and the backend refuses a partial triple.
      width_m: optionalNumber(widthM),
      free_canopy_width_m: optionalNumber(freeCanopyWidthM),
      widths_stated_on: widthsStatedOn || null,
      notes: notes.trim() || null,
      plot_ids: chosenPlots,
      maintenance: takesMaintenance
        ? maintenanceRows
            .filter((row) => row.performedOn)
            .map((row) => ({
              id: row.id,
              kind_code: row.kindCode,
              performed_on: row.performedOn,
              performed_end_date: null,
              // Animals belong to a grazing and to nothing else; sending them
              // on a siega is refused rather than silently dropped.
              animals:
                row.kindCode === GRAZING
                  ? row.animals
                      .filter((a) => a.speciesCode && a.regaCode)
                      .map((a) => ({
                        species_code: a.speciesCode,
                        rega_code: a.regaCode.trim(),
                        animal_count: Number(a.animalCount),
                      }))
                  : [],
            }))
        : [],
    };

    if (editingId) {
      await invoke("update_soil_cover", { soilCoverId: editingId, update: payload });
    } else {
      await invoke("create_soil_cover", {
        record: { ...payload, season_id: seasonId, farm_id: farmId },
      });
    }
    hideForm();
    load();
  }

  function remove(record) {
    run(async () => {
      if (!(await confirmDialog(t("cover.delete_confirm")))) return;
      await invoke("delete_soil_cover", { soilCoverId: record.id });
      hideForm();
      load();
    });
  }
</script>

{#if loading}
  <p>{t("non_field.loading")}</p>
{:else}
  <div class="view-head">
    <h3>{t("cover.title")}</h3>
    <div class="selector-buttons">
      <button type="button" disabled={plots.length === 0} onclick={() => showForm()}>
        + {t("cover.new")}
      </button>
    </div>
  </div>
  <p class="detail">{t("cover.intro")}</p>

  <TzWorkspace
    open={formOpen}
    title={editingId ? formatDate(establishedOn) : t("cover.new")}
    onclose={hideForm}
    ondelete={editing ? () => remove(editing.record) : null}
  >
    {#snippet list()}
      {#if records.length === 0}
        <p class="table-empty">{t("table.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"soil-covers"}>
            <thead>
              <tr>
                <th>{t("column.established_on")}</th>
                <th>{t("column.practice")}</th>
                <th>{t("column.cover_type")}</th>
                <th>{t("column.widths")}</th>
                <th>{t("column.maintenance")}</th>
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
                      {formatDate(detail.record.established_on)}
                    </button>
                  </td>
                  <td class="col-muted">{tCode("eco_practice", detail.record.practice_code)}</td>
                  <td class="col-muted">{coverTypeName(detail.record.cover_type_code)}</td>
                  <td class="col-muted">{widthsCell(detail.record)}</td>
                  <td class="col-muted">{maintenanceCell(detail)}</td>
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
            label={t("cover.practice")}
            hint={t("cover.practice_hint")}
            items={codeItems(practices, "eco_practice")}
            required
            bind:value={practiceCode}
          />
          <TzCombobox
            label={t("cover.type")}
            hint={t("cover.type_hint")}
            items={nameItems(
              coverTypes,
              (entry) => entry.name,
              (entry) => entry.code,
            )}
            bind:value={coverTypeCode}
          />
          <DateInput
            label={t("cover.established_on")}
            hint={t("cover.established_on_hint")}
            required
            bind:value={establishedOn}
          />
          <TextInput label={t("treatment.notes")} bind:value={notes} />
        </div>

        <fieldset class="subsection">
          <legend>{t("cover.widths_section")}</legend>
          <p class="detail">{t("cover.widths_hint")}</p>
          <div class="form-grid">
            <NumberInput label={t("cover.width_m")} min={0} bind:value={widthM} />
            <NumberInput
              label={t("cover.free_canopy_width_m")}
              min={0}
              bind:value={freeCanopyWidthM}
            />
            <DateInput label={t("cover.widths_stated_on")} bind:value={widthsStatedOn} />
          </div>
        </fieldset>

        <fieldset class="subsection">
          <legend>{t("cover.plots_section")}</legend>
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
          <legend>{t("cover.maintenance_section")}</legend>
          {#if takesMaintenance}
            <p class="detail">{t("cover.maintenance_hint")}</p>
            {#each maintenanceRows as row, index (row)}
              <div class="form-grid plot-row">
                <TzSelect
                  label={t("cover.maintenance_kind")}
                  items={maintenanceItems}
                  required
                  bind:value={row.kindCode}
                />
                <DateInput
                  label={t("cover.maintenance_date")}
                  required
                  bind:value={row.performedOn}
                />
                <button
                  type="button"
                  class="btn-danger"
                  onclick={() => maintenanceRows.splice(index, 1)}
                >
                  {t("treatment.remove")}
                </button>
              </div>
              {#if row.kindCode === GRAZING}
                <p class="detail">{t("cover.maintenance_animals_hint")}</p>
                {#each row.animals as animal, animalIndex (animal)}
                  <div class="form-grid plot-row">
                    <TzCombobox
                      label={t("grazing.species")}
                      items={nameItems(
                        species,
                        (entry) => entry.name,
                        (entry) => entry.code,
                      )}
                      required
                      bind:value={animal.speciesCode}
                    />
                    <TextInput label={t("grazing.rega")} required bind:value={animal.regaCode} />
                    <NumberInput
                      label={t("grazing.animal_count")}
                      min={1}
                      integer
                      required
                      bind:value={animal.animalCount}
                    />
                    {#if row.animals.length > 1}
                      <button
                        type="button"
                        class="btn-danger"
                        onclick={() => row.animals.splice(animalIndex, 1)}
                      >
                        {t("treatment.remove")}
                      </button>
                    {/if}
                  </div>
                {/each}
                <button type="button" onclick={() => row.animals.push(emptyAnimalRow())}>
                  {t("grazing.add_animals")}
                </button>
              {/if}
            {/each}
            <button type="button" onclick={() => maintenanceRows.push(emptyMaintenanceRow())}>
              {t("cover.add_maintenance")}
            </button>
          {:else}
            <p class="detail">{t("cover.no_maintenance")}</p>
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
