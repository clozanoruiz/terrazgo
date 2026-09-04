<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Products section of the catalogue. Creating a product captures its first
  // per-country authorisation in the same call (an unauthorised product is
  // never offered to the treatment form); substances and further
  // authorisations are managed on the product's card. Past treatment records
  // are immune to edits here — they snapshot name, number and substances.
  import TzTooltip from "./TzTooltip.svelte";
  import { formatNumber, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import { run } from "./notifications.svelte.js";
  import NumberInput from "./NumberInput.svelte";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzTabs from "./TzTabs.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";

  let loading = $state(true);
  let products = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri". (The substance picker below is
  // ordered by the select helper instead.)
  const sortedProducts = $derived(sortedBy(products, (d) => d.product.commercial_name));

  // The repository returns a product's substances in SQL's BINARY order, which
  // files accented names last. Collated here so the card's joined line and the
  // management panel's list agree with every other name list on screen.
  const substancesOf = (detail) => sortedBy(detail.substances, substanceLabel);
  // Session-wide reference data (lib/lookups.svelte.js).
  const countries = $derived(lookups.countries);
  const formulationTypes = $derived(lookups.formulationTypes);
  const authorisationKinds = $derived(lookups.authorisationKinds);
  const units = $derived(lookups.units);
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

  // The inspector's own tabs: which child collection is showing, and which of
  // its rows (or "new") the nested panel below is about.
  let childTab = $state("substances");
  let childOpen = $state(null);
  const childTabs = $derived([
    { value: "substances", label: t("product.substances") },
    { value: "authorisations", label: t("product.authorisations") },
  ]);
  const selectedSubstance = $derived(
    editing?.substances.find((link) => link.id === childOpen) ?? null,
  );
  const selectedAuthorisation = $derived(
    editing?.authorisations.find((auth) => auth.id === childOpen) ?? null,
  );
  const childTitle = $derived(
    childOpen === "new"
      ? childTab === "substances"
        ? t("product.add_substance")
        : t("product.add_authorisation")
      : (selectedSubstance?.name ?? selectedAuthorisation?.authorisation_number ?? ""),
  );

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
    [products, substances] = await Promise.all([
      invoke("list_product_details"),
      invoke("list_active_substances"),
      loadLookups(),
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

  async function submitCreate() {
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
    await invoke("create_product", { product, authorisation });
    createOpen = false;
    await reload();
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
    childOpen = null;
    childTab = "substances";
  }

  /// The concentration as the substance table's own cell shows it, reused by
  /// the nested panel so the two never disagree.
  function substanceAmount(link) {
    if (link.concentration_value == null) return "—";
    const unit = link.concentration_unit_code
      ? ` ${tCode("unit", link.concentration_unit_code)}`
      : "";
    return `${formatNumber(link.concentration_value)}${unit}`;
  }

  async function submitEdit() {
    const update = {
      commercial_name: editName.trim(),
      holder: editHolder.trim() || null,
      formulation_type_code: editFormulationCode || null,
      default_phi_days: numberOrNull(editPhiDays),
    };
    await invoke("update_product", { productId: openId, update });
    await reload();
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(products.find((d) => d.product.id === openId) ?? null);

  function deleteProduct(detail) {
    run(async () => {
      const message = t("product.delete_confirm", { name: detail.product.commercial_name });
      if (!(await confirmDialog(message))) return;
      await invoke("delete_product", { productId: detail.product.id });
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
      subSubstanceId = "";
      subNewName = "";
      subNewCas = "";
      subConcentration = "";
      subUnitCode = "";
      childOpen = null;
      await reload();
    });
  }

  function removeSubstance(link) {
    run(async () => {
      await invoke("remove_product_substance", { linkId: link.id });
      childOpen = null;
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
      addAuthNumber = "";
      childOpen = null;
      await reload();
    });
  }

  function removeAuthorisation(auth) {
    run(async () => {
      await invoke("remove_product_authorisation", { authorisationId: auth.id });
      childOpen = null;
      await reload();
    });
  }

  // --- display helpers ---------------------------------------------------------

  // The product's own values are columns now, so the "·"-joined line they used
  // to share is gone. These two remain because they summarise a COLLECTION into
  // one cell — several authorisations, several substances — which a column can
  // hold but cannot split.
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
        ? ` — ${formatNumber(link.concentration_value)} ${link.concentration_unit_code ? tCode("unit", link.concentration_unit_code) : ""}`.trimEnd()
        : "";
    const cas = link.cas_number ? ` (${link.cas_number})` : "";
    return `${link.name}${cas}${concentration}`;
  }
</script>

<div class="view-head">
  <button type="button" onclick={startCreate}>{t("products.new")}</button>
</div>

<TzWorkspace
  open={createOpen || openId !== null}
  title={openId ? editName : t("products.new")}
  onclose={() => {
    createOpen = false;
    openId = null;
  }}
  ondelete={openId ? () => deleteProduct(editing) : null}
>
  {#snippet list()}
    {#if loading}
      <Skeleton />
    {:else if products.length === 0}
      <p class="table-empty">{t("products.empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"products"}>
          <thead>
            <tr>
              <th>{t("column.name")}</th>
              <th>{t("column.holder")}</th>
              <th>{t("column.formulation")}</th>
              <th class="col-num">{t("column.phi_days")}</th>
              <th>{t("column.authorisations")}</th>
              <th>{t("column.substances")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedProducts as detail (detail.product.id)}
              <tr
                class:selected={openId === detail.product.id}
                onclick={(e) => opensRow(e) && togglePanel(detail)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => togglePanel(detail)}>
                    {detail.product.commercial_name}
                  </button>
                </td>
                <td class="col-muted">{detail.product.holder ?? ""}</td>
                <td class="col-muted">
                  {detail.product.formulation_type_code
                    ? tCode("formulation_type", detail.product.formulation_type_code)
                    : ""}
                </td>
                <td class="col-muted col-num">{detail.product.default_phi_days ?? ""}</td>
                <td class="col-muted">
                  {detail.authorisations.length > 0
                    ? authSummary(detail)
                    : t("product.no_authorisations")}
                </td>
                <td class="col-muted">
                  {substancesOf(detail).map(substanceLabel).join(" · ")}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/snippet}

  {#snippet inspector()}
    {#if createOpen}
      <TzForm onsubmit={submitCreate}>
        <div class="form-grid">
          <TextInput label={t("product.name")} required bind:value={name} />
          <TextInput label={t("product.holder")} bind:value={holder} />
          <TzSelect
            label={t("product.formulation")}
            items={codeItems(formulationTypes, "formulation_type")}
            nullable
            bind:value={formulationCode}
          />
          <NumberInput label={t("product.phi_days")} min={0} integer bind:value={phiDays} />
        </div>
        <fieldset class="es-only">
          <legend>{t("product.auth_section")}</legend>
          <div class="form-grid">
            <TzSelect
              label={t("product.auth_country")}
              items={codeItems(countries, "country")}
              bind:value={authCountry}
            />
            <TextInput label={t("product.auth_number")} required bind:value={authNumber}>
              <!-- Follows the AUTHORISATION's country, not the farm's: the number
                     is issued by whichever country authorised the product. -->
              <RegistryHint country={authCountry} field="product.auth_number" />
            </TextInput>
            <TzSelect
              label={t("product.auth_kind")}
              items={codeItems(authorisationKinds, "authorisation_kind")}
              bind:value={authKind}
              onchange={() => authKind === "exceptional" && ensureExcSubstances(authCountry)}
            />
            {#if authKind === "exceptional"}
              <TzSelect
                label={t("product.exceptional_substance")}
                items={(excSubstances[authCountry] ?? []).map((code) => ({
                  value: code.code,
                  label: code.label,
                }))}
                required
                bind:value={authExcSubstance}
              />
            {/if}
          </div>
        </fieldset>
        <div class="form-actions">
          <button type="submit">{t("form.save")}</button>
          <button type="button" class="btn-cancel" onclick={() => (createOpen = false)}>
            {t("form.cancel")}
          </button>
        </div>
      </TzForm>
    {:else if editing}
      <!-- A product carries two child collections. They live in the inspector
           with the product they belong to, which is what the pane is for. -->
      <TzForm onsubmit={submitEdit}>
        <div class="form-grid">
          <TextInput label={t("product.name")} required bind:value={editName} />
          <TextInput label={t("product.holder")} bind:value={editHolder} />
          <TzSelect
            label={t("product.formulation")}
            items={codeItems(formulationTypes, "formulation_type")}
            nullable
            bind:value={editFormulationCode}
          />
          <NumberInput label={t("product.phi_days")} min={0} integer bind:value={editPhiDays} />
        </div>
        <div class="form-actions">
          <button type="submit">{t("form.save")}</button>
        </div>
      </TzForm>

      <!-- The product's two child collections, as tabs inside the pane. Same
           system as the screen outside it — a strip picks the collection, a
           table lists it, a row opens a panel — except the nested panel opens
           BELOW rather than beside: the inspector is already the narrow column,
           and splitting it again would leave two columns too thin to read. -->
      <TzTabs items={childTabs} bind:value={childTab} onchange={() => (childOpen = null)}>
        {#snippet panel(item)}
          <div class="view-head">
            <button type="button" onclick={() => (childOpen = "new")}>
              {item.value === "substances"
                ? t("product.add_substance")
                : t("product.add_authorisation")}
            </button>
          </div>

          {#if item.value === "substances"}
            <table class="data-table">
              <thead>
                <tr>
                  <th>{t("column.name")}</th>
                  <th>{t("substance.cas")}</th>
                  <th class="col-num">{t("substance.concentration")}</th>
                </tr>
              </thead>
              <tbody>
                {#each substancesOf(editing) as link (link.id)}
                  <tr
                    class:selected={childOpen === link.id}
                    onclick={(e) => opensRow(e) && (childOpen = link.id)}
                  >
                    <td class="col-name">
                      <button type="button" class="row-open" onclick={() => (childOpen = link.id)}>
                        {link.name}
                      </button>
                    </td>
                    <td class="col-muted">{link.cas_number ?? ""}</td>
                    <td class="col-muted col-num">
                      {link.concentration_value == null
                        ? ""
                        : formatNumber(link.concentration_value)}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if editing.substances.length === 0}
              <p class="table-empty">{t("product.no_substances")}</p>
            {/if}
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th>{t("column.country")}</th>
                  <th>{t("product.auth_number")}</th>
                  <th>{t("product.auth_kind")}</th>
                </tr>
              </thead>
              <tbody>
                {#each editing.authorisations as auth (auth.id)}
                  <tr
                    class:selected={childOpen === auth.id}
                    onclick={(e) => opensRow(e) && (childOpen = auth.id)}
                  >
                    <td class="col-name">
                      <button type="button" class="row-open" onclick={() => (childOpen = auth.id)}>
                        {tCode("country", auth.country_code)}
                      </button>
                    </td>
                    <td class="col-muted">{auth.authorisation_number}</td>
                    <td class="col-muted">{tCode("authorisation_kind", auth.kind_code)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if editing.authorisations.length === 0}
              <p class="table-empty">{t("product.no_authorisations")}</p>
            {/if}
          {/if}

          <!-- The nested panel. Adding takes the form; selecting an existing
               row shows what it holds and offers to remove it, because neither
               collection has an update command — a link is added or taken away,
               never edited. -->
          {#if childOpen}
            <div class="subpanel">
              <div class="inspector-head">
                <span>{childTitle}</span>
                <TzTooltip label={t("form.close")}>
                  {#snippet trigger(props)}
                    <button
                      {...props}
                      type="button"
                      class="inspector-close"
                      onclick={(event) => {
                        props.onclick?.(event);
                        childOpen = null;
                      }}
                      aria-label={t("form.close")}>×</button
                    >
                  {/snippet}
                </TzTooltip>
              </div>

              {#if childOpen === "new" && item.value === "substances"}
                <div class="form-grid">
                  <TzSelect
                    label={t("substance.existing")}
                    items={nameItems(substances)}
                    nullable
                    bind:value={subSubstanceId}
                  />
                  {#if !subSubstanceId}
                    <TextInput label={t("substance.new_name")} bind:value={subNewName} />
                    <TextInput label={t("substance.cas")} bind:value={subNewCas} />
                  {/if}
                  <NumberInput
                    label={t("substance.concentration")}
                    min={0}
                    bind:value={subConcentration}
                  />
                  <TzSelect
                    label={t("substance.unit")}
                    items={codeItems(units, "unit")}
                    nullable
                    bind:value={subUnitCode}
                  />
                </div>
                <div class="form-actions">
                  <button
                    type="button"
                    onclick={addSubstance}
                    disabled={!subSubstanceId && !subNewName.trim()}
                  >
                    {t("product.add_substance")}
                  </button>
                </div>
              {:else if childOpen === "new"}
                <div class="form-grid">
                  <TzSelect
                    label={t("product.auth_country")}
                    items={codeItems(countries, "country")}
                    bind:value={addAuthCountry}
                  />
                  <TextInput label={t("product.auth_number")} bind:value={addAuthNumber}>
                    <RegistryHint country={addAuthCountry} field="product.auth_number" />
                  </TextInput>
                  <TzSelect
                    label={t("product.auth_kind")}
                    items={codeItems(authorisationKinds, "authorisation_kind")}
                    bind:value={addAuthKind}
                    onchange={() =>
                      addAuthKind === "exceptional" && ensureExcSubstances(addAuthCountry)}
                  />
                  {#if addAuthKind === "exceptional"}
                    <TzSelect
                      label={t("product.exceptional_substance")}
                      items={(excSubstances[addAuthCountry] ?? []).map((code) => ({
                        value: code.code,
                        label: code.label,
                      }))}
                      bind:value={addAuthExcSubstance}
                    />
                  {/if}
                </div>
                <div class="form-actions">
                  <button type="button" onclick={addAuthorisation} disabled={!addAuthNumber.trim()}>
                    {t("product.add_authorisation")}
                  </button>
                </div>
              {:else if selectedSubstance}
                <dl class="detail-list">
                  <dt>{t("substance.cas")}</dt>
                  <dd>{selectedSubstance.cas_number ?? "—"}</dd>
                  <dt>{t("substance.concentration")}</dt>
                  <dd>{substanceAmount(selectedSubstance)}</dd>
                </dl>
                <div class="inspector-actions">
                  <button
                    type="button"
                    class="btn-danger"
                    onclick={() => removeSubstance(selectedSubstance)}
                  >
                    {t("form.remove")}
                  </button>
                </div>
              {:else if selectedAuthorisation}
                <dl class="detail-list">
                  <dt>{t("product.auth_number")}</dt>
                  <dd>{selectedAuthorisation.authorisation_number}</dd>
                  <dt>{t("product.auth_kind")}</dt>
                  <dd>{tCode("authorisation_kind", selectedAuthorisation.kind_code)}</dd>
                </dl>
                <div class="inspector-actions">
                  <button
                    type="button"
                    class="btn-danger"
                    onclick={() => removeAuthorisation(selectedAuthorisation)}
                  >
                    {t("form.remove")}
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        {/snippet}
      </TzTabs>
    {/if}
  {/snippet}
</TzWorkspace>
