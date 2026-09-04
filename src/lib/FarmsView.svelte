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
  import TzSelect from "./TzSelect.svelte";
  import { codeItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";

  let farms = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedFarms = $derived(sortedBy(farms, (f) => f.name));
  // Session-wide reference data (lib/lookups.svelte.js).
  const countries = $derived(lookups.countries);
  let creating = $state(false);
  let loading = $state(true);

  let name = $state("");
  let ownerName = $state("");
  let ownerTaxId = $state("");
  let countryCode = $state("");
  let regaCode = $state("");
  let reaCode = $state("");
  let provinceCode = $state("");

  run(async () => {
    await loadLookups();
    countryCode ||= countries[0]?.code ?? "";
    farms = await invoke("list_farms");
  }).finally(() => (loading = false));

  function startCreate() {
    name = "";
    ownerName = "";
    ownerTaxId = "";
    regaCode = "";
    reaCode = "";
    provinceCode = "";
    creating = true;
  }

  function collectEs() {
    if (countryCode !== "es") return null;
    const rega = regaCode.trim() || null;
    const rea = reaCode.trim() || null;
    const province = provinceCode.trim() || null;
    return rega || rea || province
      ? { rega_code: rega, rea_code: rea, siex_code: null, province_code: province }
      : null;
  }

  async function submit() {
    const farm = {
      name: name.trim(),
      owner_name: ownerName.trim() || null,
      owner_tax_id: ownerTaxId.trim() || null,
      country_code: countryCode,
      es: collectEs(),
    };
    await invoke("create_farm", { farm });
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
                <!-- A farm row navigates rather than opening an inspector: the
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
      <TzForm id={formId} onsubmit={submit}>
        <div class="form-grid">
          <TextInput label={t("farm.name")} required bind:value={name} />
          <TextInput label={t("farm.owner")} bind:value={ownerName} />
          <TextInput label={t("farm.owner_tax_id")} bind:value={ownerTaxId} />
          <TzSelect
            label={t("farm.country")}
            items={codeItems(countries, "country")}
            bind:value={countryCode}
          />
        </div>
        {#if countryCode === "es"}
          <fieldset class="es-only">
            <legend>{t("farm.es_section")}</legend>
            <div class="form-grid">
              <TextInput label={t("farm.rea")} bind:value={reaCode} />
              <TextInput label={t("farm.rega")} bind:value={regaCode} />
              <TextInput label={t("farm.province")} bind:value={provinceCode} />
            </div>
          </fieldset>
        {/if}
      </TzForm>
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
