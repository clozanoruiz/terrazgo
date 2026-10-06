<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Farms list + create form. Country and code labels are translated at
  // display time (tCode); user-entered names are shown as typed.
  import { t, tCode } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import { run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TzWorkspace from "./TzWorkspace.svelte";
  import FarmForm from "./FarmForm.svelte";
  import { emptyFarmDraft, farmPayload } from "./farmDraft.js";

  let farms = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedFarms = $derived(sortedBy(farms, (f) => f.name));
  // Session-wide reference data (lib/lookups.svelte.js).
  const countries = $derived(lookups.countries);
  let creating = $state(false);
  let loading = $state(true);

  /// The same shape the farm's own page edits, and the same component draws it
  /// (lib/farmDraft.js). Creating a holding and correcting one ask the same
  /// questions now; the only difference is that the country can be chosen here
  /// and never again.
  let farmDraft = $state(emptyFarmDraft());

  run(async () => {
    await loadLookups();
    farms = await invoke("list_farms");
  }).finally(() => (loading = false));

  function startCreate() {
    // Preselected rather than left blank: one country is the everyday case, and
    // it is the one field that cannot be corrected afterwards.
    farmDraft = emptyFarmDraft(countries[0]?.code ?? "");
    creating = true;
  }

  async function submit() {
    await invoke("create_farm", { farm: farmPayload(farmDraft, { create: true }) });
    creating = false;
    farms = await invoke("list_farms");
  }

  // The "·"-joined detail string these lists used to build is gone: the table
  // has a column per value, which is what makes them scannable down the page.
</script>

<section class="view framed">
  <div class="view-head">
    <h2>{t("farms.title")}</h2>
    <button type="button" onclick={startCreate}>{t("farms.new")}</button>
  </div>

  <TzWorkspace open={creating} title={t("farms.new")} onclose={() => (creating = false)}>
    {#snippet list()}
      {#if loading}
        <Skeleton />
      {:else if farms.length === 0}
        <p class="table-empty">{t("farms.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"farms"}>
            <thead>
              <tr>
                <th>{t("column.name")}</th>
                <th>{t("column.country")}</th>
                <th>{t("column.owner")}</th>
                <th>{t("column.tax_id")}</th>
              </tr>
            </thead>
            <tbody>
              {#each sortedFarms as farm (farm.id)}
                <!-- A farm row navigates rather than opening a panel: the
                     holding has a page of its own. The <a> is still what a
                     keyboard reaches; the row click is the pointer shortcut. -->
                <tr onclick={(e) => opensRow(e) && (location.hash = "#/farms/" + farm.id)}>
                  <td class="col-name"><a href={"#/farms/" + farm.id}>{farm.name}</a></td>
                  <td class="col-muted">{tCode("country", farm.country_code)}</td>
                  <td class="col-muted">{farm.owner_name ?? ""}</td>
                  <td class="col-muted">{farm.owner_tax_id ?? ""}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      <FarmForm bind:draft={farmDraft} {countries} creating onsubmit={submit} {formId} />
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (creating = false)}
          >{t("form.cancel")}</button
        >
      </div>
    {/snippet}
  </TzWorkspace>
</section>
