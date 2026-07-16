<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Farms list + create form. Country and code labels are translated at
  // display time (tCode); user-entered names are shown as typed.
  import { t, tCode } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let farms = $state([]);
  let countries = $state([]);
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
    countries = await invoke("list_countries");
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
      ? { rega_code: rega, rea_code: rea, province_code: province }
      : null;
  }

  function submit(event) {
    event.preventDefault();
    const farm = {
      name: name.trim(),
      owner_name: ownerName.trim() || null,
      owner_tax_id: ownerTaxId.trim() || null,
      country_code: countryCode,
      es: collectEs(),
    };
    run(async () => {
      await invoke("create_farm", { farm });
      notify(t("message.farm_saved", { name: farm.name }));
      creating = false;
      farms = await invoke("list_farms");
    });
  }

  function farmDetail(farm) {
    return [tCode("country", farm.country_code), farm.owner_name].filter(Boolean).join(" · ");
  }
</script>

<section class="view">
  <div class="view-head">
    <h2>{t("farms.title")}</h2>
    <button type="button" onclick={startCreate}>{t("farms.new")}</button>
  </div>

  {#if creating}
    <form onsubmit={submit}>
      <div class="form-grid">
        <label><span>{t("farm.name")}</span><input required bind:value={name} /></label>
        <label><span>{t("farm.owner")}</span><input bind:value={ownerName} /></label>
        <label><span>{t("farm.owner_tax_id")}</span><input bind:value={ownerTaxId} /></label>
        <label
          ><span>{t("farm.country")}</span>
          <select bind:value={countryCode}>
            {#each countries as country (country.code)}
              <option value={country.code}>{tCode("country", country.code)}</option>
            {/each}
          </select>
        </label>
      </div>
      {#if countryCode === "es"}
        <fieldset class="es-only">
          <legend>{t("farm.es_section")}</legend>
          <div class="form-grid">
            <label><span>{t("farm.rea")}</span><input bind:value={reaCode} /></label>
            <label><span>{t("farm.rega")}</span><input bind:value={regaCode} /></label>
            <label><span>{t("farm.province")}</span><input bind:value={provinceCode} /></label>
          </div>
        </fieldset>
      {/if}
      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={() => (creating = false)}
          >{t("form.cancel")}</button
        >
      </div>
    </form>
  {/if}

  {#if loading}
    <Skeleton />
  {:else}
    <ul class="card-list">
      {#each farms as farm (farm.id)}
        <li class="card">
          <a href={"#/farms/" + farm.id}>
            <strong>{farm.name}</strong>
            <span class="detail">{farmDetail(farm)}</span>
          </a>
        </li>
      {/each}
    </ul>
    {#if farms.length === 0}
      <p>{t("farms.empty")}</p>
    {/if}
  {/if}
</section>
