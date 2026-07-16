<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Products section of the catalogue. Creating a product captures its first
  // per-country authorisation in the same call (an unauthorised product is
  // never offered to the treatment form); substances and further
  // authorisations are managed on the product's card. Past treatment records
  // are immune to edits here — they snapshot name, number and substances.
  import { t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let loading = $state(true);
  let products = $state([]);
  let countries = $state([]);
  let formulationTypes = $state([]);
  let authorisationKinds = $state([]);
  let units = $state([]);
  let substances = $state([]);

  // Exceptional-authorisation substance catalogue per country, fetched when a
  // form first selects the 'exceptional' kind.
  let excSubstances = $state({});

  function ensureExcSubstances(countryCode) {
    if (!countryCode || excSubstances[countryCode]) return;
    run(async () => {
      const codes = await invoke("list_exceptional_substances", { countryCode });
      excSubstances = { ...excSubstances, [countryCode]: codes };
    });
  }

  // Create form.
  let createOpen = $state(false);
  let name = $state("");
  let holder = $state("");
  let formulationCode = $state("");
  let phiDays = $state("");
  let authCountry = $state("");
  let authNumber = $state("");
  let authKind = $state("registered");
  let authExcSubstance = $state("");

  // Per-card management panel (edit fields + substances + authorisations).
  let openId = $state(null);
  let editName = $state("");
  let editHolder = $state("");
  let editFormulationCode = $state("");
  let editPhiDays = $state("");

  // Add-substance controls: pick an existing substance OR name a new one.
  let subSubstanceId = $state("");
  let subNewName = $state("");
  let subNewCas = $state("");
  let subConcentration = $state("");
  let subUnitCode = $state("");

  // Add-authorisation controls.
  let addAuthCountry = $state("");
  let addAuthNumber = $state("");
  let addAuthKind = $state("registered");
  let addAuthExcSubstance = $state("");

  run(async () => {
    [products, countries, formulationTypes, authorisationKinds, units, substances] =
      await Promise.all([
        invoke("list_product_details"),
        invoke("list_countries"),
        invoke("list_formulation_types"),
        invoke("list_authorisation_kinds"),
        invoke("list_units"),
        invoke("list_active_substances"),
      ]);
    authCountry ||= countries[0]?.code ?? "";
    addAuthCountry ||= countries[0]?.code ?? "";
  }).finally(() => (loading = false));

  async function reload() {
    products = await invoke("list_product_details");
  }

  function numberOrNull(value) {
    const trimmed = String(value ?? "").trim();
    if (trimmed === "") return null;
    const parsed = Number(trimmed);
    return Number.isNaN(parsed) ? null : parsed;
  }

  // --- create ----------------------------------------------------------------

  function startCreate() {
    name = "";
    holder = "";
    formulationCode = "";
    phiDays = "";
    authNumber = "";
    authKind = "registered";
    authExcSubstance = "";
    createOpen = true;
    openId = null;
  }

  function submitCreate(event) {
    event.preventDefault();
    const product = {
      commercial_name: name.trim(),
      holder: holder.trim() || null,
      formulation_type_code: formulationCode || null,
      default_phi_days: numberOrNull(phiDays),
    };
    const authorisation = {
      country_code: authCountry,
      authorisation_number: authNumber.trim(),
      kind_code: authKind,
      exceptional_substance_code: authKind === "exceptional" ? authExcSubstance || null : null,
      status: null,
      valid_from: null,
      valid_until: null,
    };
    run(async () => {
      await invoke("create_product", { product, authorisation });
      notify(t("message.product_saved", { name: product.commercial_name }));
      createOpen = false;
      await reload();
    });
  }

  // --- manage one product ------------------------------------------------------

  function togglePanel(detail) {
    if (openId === detail.product.id) {
      openId = null;
      return;
    }
    openId = detail.product.id;
    createOpen = false;
    editName = detail.product.commercial_name;
    editHolder = detail.product.holder ?? "";
    editFormulationCode = detail.product.formulation_type_code ?? "";
    editPhiDays = detail.product.default_phi_days ?? "";
    subSubstanceId = "";
    subNewName = "";
    subNewCas = "";
    subConcentration = "";
    subUnitCode = "";
    addAuthNumber = "";
    addAuthKind = "registered";
    addAuthExcSubstance = "";
  }

  function submitEdit(event) {
    event.preventDefault();
    const update = {
      commercial_name: editName.trim(),
      holder: editHolder.trim() || null,
      formulation_type_code: editFormulationCode || null,
      default_phi_days: numberOrNull(editPhiDays),
    };
    run(async () => {
      await invoke("update_product", { productId: openId, update });
      notify(t("message.product_saved", { name: update.commercial_name }));
      await reload();
    });
  }

  function deleteProduct(detail) {
    run(async () => {
      const message = t("product.delete_confirm", { name: detail.product.commercial_name });
      if (!(await confirmDialog(message))) return;
      await invoke("delete_product", { productId: detail.product.id });
      notify(t("message.product_deleted"));
      openId = null;
      await reload();
    });
  }

  function addSubstance() {
    run(async () => {
      let substanceId = subSubstanceId;
      if (!substanceId) {
        const created = await invoke("create_active_substance", {
          name: subNewName.trim(),
          casNumber: subNewCas.trim() || null,
        });
        substances = await invoke("list_active_substances");
        substanceId = created.id;
      }
      await invoke("add_product_substance", {
        productId: openId,
        activeSubstanceId: substanceId,
        concentrationValue: numberOrNull(subConcentration),
        concentrationUnitCode: subUnitCode || null,
      });
      notify(t("message.substance_added"));
      subSubstanceId = "";
      subNewName = "";
      subNewCas = "";
      subConcentration = "";
      subUnitCode = "";
      await reload();
    });
  }

  function removeSubstance(link) {
    run(async () => {
      await invoke("remove_product_substance", { linkId: link.id });
      notify(t("message.substance_removed"));
      await reload();
    });
  }

  function addAuthorisation() {
    run(async () => {
      await invoke("add_product_authorisation", {
        productId: openId,
        authorisation: {
          country_code: addAuthCountry,
          authorisation_number: addAuthNumber.trim(),
          kind_code: addAuthKind,
          exceptional_substance_code:
            addAuthKind === "exceptional" ? addAuthExcSubstance || null : null,
          status: null,
          valid_from: null,
          valid_until: null,
        },
      });
      notify(t("message.authorisation_added"));
      addAuthNumber = "";
      await reload();
    });
  }

  function removeAuthorisation(auth) {
    run(async () => {
      await invoke("remove_product_authorisation", { authorisationId: auth.id });
      notify(t("message.authorisation_removed"));
      await reload();
    });
  }

  // --- display helpers ---------------------------------------------------------

  function productDetailLine(detail) {
    return [
      detail.product.holder,
      detail.product.formulation_type_code
        ? tCode("formulation_type", detail.product.formulation_type_code)
        : null,
      detail.product.default_phi_days != null
        ? t("product.phi_detail", { days: detail.product.default_phi_days })
        : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }

  function authSummary(detail) {
    return detail.authorisations
      .map((a) => {
        const kind =
          a.kind_code !== "registered" ? ` (${tCode("authorisation_kind", a.kind_code)})` : "";
        return `${tCode("country", a.country_code)} ${a.authorisation_number}${kind}`;
      })
      .join(" · ");
  }

  function substanceLabel(link) {
    const concentration =
      link.concentration_value != null
        ? ` — ${link.concentration_value} ${link.concentration_unit_code ? tCode("unit", link.concentration_unit_code) : ""}`.trimEnd()
        : "";
    const cas = link.cas_number ? ` (${link.cas_number})` : "";
    return `${link.name}${cas}${concentration}`;
  }
</script>

<div class="view-head">
  <h3>{t("products.title")}</h3>
  <button type="button" onclick={startCreate}>{t("products.new")}</button>
</div>

{#if createOpen}
  <form onsubmit={submitCreate}>
    <div class="form-grid">
      <label><span>{t("product.name")}</span><input required bind:value={name} /></label>
      <label><span>{t("product.holder")}</span><input bind:value={holder} /></label>
      <label>
        <span>{t("product.formulation")}</span>
        <select bind:value={formulationCode}>
          <option value="">—</option>
          {#each formulationTypes as type (type.code)}
            <option value={type.code}>{tCode("formulation_type", type.code)}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>{t("product.phi_days")}</span>
        <input type="number" min="0" step="1" bind:value={phiDays} />
      </label>
    </div>
    <fieldset class="es-only">
      <legend>{t("product.auth_section")}</legend>
      <div class="form-grid">
        <label>
          <span>{t("product.auth_country")}</span>
          <select bind:value={authCountry}>
            {#each countries as country (country.code)}
              <option value={country.code}>{tCode("country", country.code)}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>{t("product.auth_number")}</span>
          <input required bind:value={authNumber} />
        </label>
        <label>
          <span>{t("product.auth_kind")}</span>
          <select
            bind:value={authKind}
            onchange={() => authKind === "exceptional" && ensureExcSubstances(authCountry)}
          >
            {#each authorisationKinds as kind (kind.code)}
              <option value={kind.code}>{tCode("authorisation_kind", kind.code)}</option>
            {/each}
          </select>
        </label>
        {#if authKind === "exceptional"}
          <label>
            <span>{t("product.exceptional_substance")}</span>
            <select required bind:value={authExcSubstance}>
              <option value="" disabled hidden></option>
              {#each excSubstances[authCountry] ?? [] as code (code.id)}
                <option value={code.code}>{code.label}</option>
              {/each}
            </select>
          </label>
        {/if}
      </div>
    </fieldset>
    <div class="form-actions">
      <button type="submit">{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={() => (createOpen = false)}>
        {t("form.cancel")}
      </button>
    </div>
  </form>
{/if}

{#if loading}
  <Skeleton />
{:else}
  <ul class="card-list">
    {#each products as detail (detail.product.id)}
      <li class="card">
        <div class="stack">
          <strong>{detail.product.commercial_name}</strong>
          <span class="detail">{productDetailLine(detail)}</span>
          {#if detail.authorisations.length > 0}
            <span class="detail">{authSummary(detail)}</span>
          {:else}
            <span class="detail">{t("product.no_authorisations")}</span>
          {/if}
          {#if detail.substances.length > 0}
            <span class="detail">{detail.substances.map(substanceLabel).join(" · ")}</span>
          {/if}
        </div>
        <button type="button" onclick={() => togglePanel(detail)}>
          {openId === detail.product.id ? t("form.close") : t("form.edit")}
        </button>
        <button type="button" class="btn-danger" onclick={() => deleteProduct(detail)}>
          {t("form.delete")}
        </button>

        {#if openId === detail.product.id}
          <form onsubmit={submitEdit}>
            <div class="form-grid">
              <label><span>{t("product.name")}</span><input required bind:value={editName} /></label
              >
              <label><span>{t("product.holder")}</span><input bind:value={editHolder} /></label>
              <label>
                <span>{t("product.formulation")}</span>
                <select bind:value={editFormulationCode}>
                  <option value="">—</option>
                  {#each formulationTypes as type (type.code)}
                    <option value={type.code}>{tCode("formulation_type", type.code)}</option>
                  {/each}
                </select>
              </label>
              <label>
                <span>{t("product.phi_days")}</span>
                <input type="number" min="0" step="1" bind:value={editPhiDays} />
              </label>
            </div>
            <div class="form-actions">
              <button type="submit">{t("form.save")}</button>
            </div>
          </form>

          <h4>{t("product.substances")}</h4>
          <ul class="card-list">
            {#each detail.substances as link (link.id)}
              <li class="card">
                <span class="detail">{substanceLabel(link)}</span>
                <button type="button" class="btn-danger" onclick={() => removeSubstance(link)}>
                  {t("form.remove")}
                </button>
              </li>
            {/each}
          </ul>
          <div class="form-grid">
            <label>
              <span>{t("substance.existing")}</span>
              <select bind:value={subSubstanceId}>
                <option value="">—</option>
                {#each substances as substance (substance.id)}
                  <option value={substance.id}>{substance.name}</option>
                {/each}
              </select>
            </label>
            {#if !subSubstanceId}
              <label><span>{t("substance.new_name")}</span><input bind:value={subNewName} /></label>
              <label><span>{t("substance.cas")}</span><input bind:value={subNewCas} /></label>
            {/if}
            <label>
              <span>{t("substance.concentration")}</span>
              <input type="number" step="any" min="0" bind:value={subConcentration} />
            </label>
            <label>
              <span>{t("substance.unit")}</span>
              <select bind:value={subUnitCode}>
                <option value="">—</option>
                {#each units as unit (unit.code)}
                  <option value={unit.code}>{tCode("unit", unit.code)}</option>
                {/each}
              </select>
            </label>
            <label class="selector-action">
              <span>&nbsp;</span>
              <button
                type="button"
                onclick={addSubstance}
                disabled={!subSubstanceId && !subNewName.trim()}
              >
                {t("product.add_substance")}
              </button>
            </label>
          </div>

          <h4>{t("product.authorisations")}</h4>
          <ul class="card-list">
            {#each detail.authorisations as auth (auth.id)}
              <li class="card">
                <span class="detail">
                  {tCode("country", auth.country_code)} — {auth.authorisation_number}
                </span>
                <button type="button" class="btn-danger" onclick={() => removeAuthorisation(auth)}>
                  {t("form.remove")}
                </button>
              </li>
            {/each}
          </ul>
          <div class="form-grid">
            <label>
              <span>{t("product.auth_country")}</span>
              <select bind:value={addAuthCountry}>
                {#each countries as country (country.code)}
                  <option value={country.code}>{tCode("country", country.code)}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>{t("product.auth_number")}</span>
              <input bind:value={addAuthNumber} />
            </label>
            <label>
              <span>{t("product.auth_kind")}</span>
              <select
                bind:value={addAuthKind}
                onchange={() =>
                  addAuthKind === "exceptional" && ensureExcSubstances(addAuthCountry)}
              >
                {#each authorisationKinds as kind (kind.code)}
                  <option value={kind.code}>{tCode("authorisation_kind", kind.code)}</option>
                {/each}
              </select>
            </label>
            {#if addAuthKind === "exceptional"}
              <label>
                <span>{t("product.exceptional_substance")}</span>
                <select bind:value={addAuthExcSubstance}>
                  <option value="" disabled hidden></option>
                  {#each excSubstances[addAuthCountry] ?? [] as code (code.id)}
                    <option value={code.code}>{code.label}</option>
                  {/each}
                </select>
              </label>
            {/if}
            <label class="selector-action">
              <span>&nbsp;</span>
              <button type="button" onclick={addAuthorisation} disabled={!addAuthNumber.trim()}>
                {t("product.add_authorisation")}
              </button>
            </label>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  {#if products.length === 0}
    <p>{t("products.empty")}</p>
  {/if}
{/if}
