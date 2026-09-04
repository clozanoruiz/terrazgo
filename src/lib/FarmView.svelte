<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Farm detail: edit form, plots list and the shared create/edit plot form.
  // The SIGPAC/REGA fieldsets only apply to Spanish farms.
  import TzTooltip from "./TzTooltip.svelte";
  import { TriangleAlert } from "@lucide/svelte";
  import { formatCoordinates, formatNumber, formatPercent, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { sortedBy } from "./collate.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import TzCheckbox from "./TzCheckbox.svelte";
  import NumberInput from "./NumberInput.svelte";
  import DateInput from "./DateInput.svelte";
  import MapCanvas from "./MapCanvas.svelte";
  import { notify, run } from "./notifications.svelte.js";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let { farmId } = $props();

  // Read-only embedded map: clicking a boundary highlights it; editing
  // happens in the Map workspace (the "open in map" links).
  let mapSelectedPlotId = $state(null);

  function mapHref(plotId = null) {
    return plotId ? `#/map?farm=${farmId}&plot=${plotId}` : `#/map?farm=${farmId}`;
  }

  let loading = $state(true);

  const SIGPAC_FIELDS = [
    "sigpac_province",
    "sigpac_municipality",
    "sigpac_aggregate",
    "sigpac_zone",
    "sigpac_polygon",
    "sigpac_parcel",
    "sigpac_enclosure",
  ];

  // Session-wide reference data (lib/lookups.svelte.js).
  const countries = $derived(lookups.countries);
  let farm = $state(null);
  let plots = $state([]);
  // Active SIGPAC boundary per plot id (from geo_feature) — drives the
  // verified badge and the declared-vs-official discrepancy display.
  let sigpacFeatures = $state({});
  // Latest-campaign 'inside' zone flags per plot id — the compliance chips.
  let zoneFlags = $state({});

  // Farm edit form fields (the form is the source of truth on save).
  let name = $state("");
  let ownerName = $state("");
  let ownerTaxId = $state("");
  let countryCode = $state("");
  let locationText = $state("");
  let latitude = $state("");
  let longitude = $state("");
  let regaCode = $state("");
  let reaCode = $state("");
  let provinceCode = $state("");

  // Model 1.1's contact block and the "titular o representante" block. The
  // representative is optional: a blank name means there is none, which is the
  // common case (the holder signs their own book).
  let address = $state("");
  let postalCode = $state("");
  let phoneFixed = $state("");
  let phoneMobile = $state("");
  let email = $state("");
  // Model 1.1's "Fecha de apertura del cuaderno". Blank prints the model's
  // ruled line, so the page stays hand-fillable for a book nobody dated.
  let openedOn = $state("");
  let siexCode = $state("");
  let repName = $state("");
  let repTaxId = $state("");
  let repKind = $state("");
  let repAddress = $state("");
  let repLocality = $state("");
  let repProvince = $state("");
  let repPostalCode = $state("");
  let repPhone = $state("");
  let repEmail = $state("");

  // Advisory (official model 1.4): the advisor entities live in the catalogue,
  // this panel only states which of them advise THIS holding, and under which
  // GIP framework.
  let advisors = $state([]);
  let farmAdvisors = $state([]);
  const gipSystems = $derived(lookups.gipSystems);
  let newAdvisorId = $state("");
  let newGipCode = $state("");

  const linkableAdvisors = $derived(
    advisors.filter((a) => !farmAdvisors.some((d) => d.advisor.id === a.id)),
  );

  // Collated: the repository returns BINARY order, which files accented names
  // last (src/lib/collate.js).
  const sortedFarmAdvisors = $derived(sortedBy(farmAdvisors, (d) => d.advisor.name));

  // Water abstraction points for human consumption (official model 2.2's water
  // half). A farm asset, not a season record — hence this view and not a
  // record-book tab.
  let waterPoints = $state([]);
  // Plot ids the farmer has declared free of abstraction points. A stored
  // negative, like an 'outside' zone flag: it proves the question was asked,
  // which a blank cell cannot.
  let waterDeclared = $state(new Set());
  let waterFormOpen = $state(false);
  let editingWaterPointId = $state(null);
  let waterPlotId = $state("");
  let waterDenomination = $state("");
  let waterInside = $state(true);
  let waterDistance = $state("");
  let waterLatitude = $state("");
  let waterLongitude = $state("");

  // Plot form; null editingPlotId = the form creates, an id = it edits.
  let plotFormOpen = $state(false);
  let editingPlotId = $state(null);
  let plotName = $state("");
  let plotArea = $state("");
  let sigpac = $state({});

  function fillFarmForm(detail) {
    farm = detail.farm;
    name = detail.farm.name;
    ownerName = detail.farm.owner_name ?? "";
    ownerTaxId = detail.farm.owner_tax_id ?? "";
    countryCode = detail.farm.country_code;
    locationText = detail.farm.location_text ?? "";
    latitude = detail.farm.latitude ?? "";
    longitude = detail.farm.longitude ?? "";
    address = detail.farm.address ?? "";
    postalCode = detail.farm.postal_code ?? "";
    phoneFixed = detail.farm.phone_fixed ?? "";
    phoneMobile = detail.farm.phone_mobile ?? "";
    openedOn = detail.farm.opened_on ?? "";
    email = detail.farm.email ?? "";
    regaCode = detail.es?.rega_code ?? "";
    reaCode = detail.es?.rea_code ?? "";
    siexCode = detail.es?.siex_code ?? "";
    provinceCode = detail.es?.province_code ?? "";
    repName = detail.representative?.full_name ?? "";
    repTaxId = detail.representative?.tax_id ?? "";
    repKind = detail.representative?.representation_kind ?? "";
    repAddress = detail.representative?.address ?? "";
    repLocality = detail.representative?.locality ?? "";
    repProvince = detail.representative?.province ?? "";
    repPostalCode = detail.representative?.postal_code ?? "";
    repPhone = detail.representative?.phone ?? "";
    repEmail = detail.representative?.email ?? "";
  }

  run(async () => {
    [advisors] = await Promise.all([invoke("list_advisors"), loadLookups()]);
    fillFarmForm(await invoke("get_farm", { farmId }));
    await reloadAdvisors();
    await reloadPlots();
    await reloadWater();
  }).finally(() => (loading = false));

  async function reloadAdvisors() {
    farmAdvisors = await invoke("list_farm_advisors", { farmId });
  }

  async function reloadWater() {
    const [points, declarations] = await Promise.all([
      invoke("list_water_points", { farmId }),
      invoke("list_water_declarations", { farmId }),
    ]);
    waterPoints = points;
    waterDeclared = new Set(declarations.map((d) => d.plot_id));
  }

  async function reloadPlots() {
    plots = await invoke("list_plots", { farmId });
    const features = await invoke("list_geo_features", { farmId });
    const next = {};
    for (const feature of features) {
      if (feature.source === "sigpac" && feature.plot_id) next[feature.plot_id] = feature;
    }
    sigpacFeatures = next;
    // Zone chips. The backend already answers with each plot's CURRENT standing
    // — one row per (plot, zone type) — so there is nothing to deduplicate
    // here; a chip is drawn for each of them that says 'inside'.
    const flags = await invoke("list_zone_flags", { farmId });
    const zones = {};
    for (const flag of flags) {
      if (flag.status === "inside") (zones[flag.plot_id] ??= []).push(flag);
    }
    zoneFlags = zones;
  }

  function numberOrNull(value) {
    const trimmed = String(value ?? "").trim();
    if (trimmed === "") return null;
    const parsed = Number(trimmed);
    return Number.isNaN(parsed) ? null : parsed;
  }

  // --- farm edit -------------------------------------------------------------

  function collectFarmEs() {
    if (countryCode !== "es") return null;
    const rega = regaCode.trim() || null;
    const rea = reaCode.trim() || null;
    const siex = siexCode.trim() || null;
    const province = provinceCode.trim() || null;
    return rega || rea || siex || province
      ? { rega_code: rega, rea_code: rea, siex_code: siex, province_code: province }
      : null;
  }

  /// The name is what makes a representative exist: with it blank the whole
  /// block is submitted as null, which removes any stored row.
  function collectRepresentative() {
    const fullName = repName.trim();
    if (!fullName) return null;
    return {
      full_name: fullName,
      tax_id: repTaxId.trim() || null,
      representation_kind: repKind.trim() || null,
      address: repAddress.trim() || null,
      locality: repLocality.trim() || null,
      province: repProvince.trim() || null,
      postal_code: repPostalCode.trim() || null,
      phone: repPhone.trim() || null,
      email: repEmail.trim() || null,
    };
  }

  async function submitFarm() {
    const update = {
      name: name.trim(),
      owner_name: ownerName.trim() || null,
      owner_tax_id: ownerTaxId.trim() || null,
      location_text: locationText.trim() || null,
      address: address.trim() || null,
      postal_code: postalCode.trim() || null,
      phone_fixed: phoneFixed.trim() || null,
      phone_mobile: phoneMobile.trim() || null,
      email: email.trim() || null,
      opened_on: openedOn || null,
      latitude: numberOrNull(latitude),
      longitude: numberOrNull(longitude),
      country_code: countryCode,
      es: collectFarmEs(),
      representative: collectRepresentative(),
    };
    fillFarmForm(await invoke("update_farm", { farmId, update }));
  }

  function deleteFarm() {
    run(async () => {
      if (!(await confirmDialog(t("farm.delete_confirm", { name: farm.name })))) return;
      await invoke("delete_farm", { farmId });
      location.hash = "#/farms";
    });
  }

  // --- advisory (model 1.4) ----------------------------------------------------

  /// One command for both cases: linking an advisor and restating the
  /// framework of an existing link are the same statement about the holding.
  function saveFarmAdvisor(advisorId, gipSystemCode) {
    run(async () => {
      await invoke("set_farm_advisor", { farmId, advisorId, gipSystemCode });
      await reloadAdvisors();
    });
  }

  function linkAdvisor() {
    if (!newAdvisorId) return;
    const advisorId = newAdvisorId;
    const gip = newGipCode || null;
    newAdvisorId = "";
    newGipCode = "";
    advisorPanel = null;
    saveFarmAdvisor(advisorId, gip);
  }

  function unlinkAdvisor(detail) {
    run(async () => {
      if (!(await confirmDialog(t("farm.advisor_remove_confirm", { name: detail.advisor.name }))))
        return;
      await invoke("remove_farm_advisor", { linkId: detail.link.id });
      advisorPanel = null;
      await reloadAdvisors();
    });
  }

  // Which advisory link the pane is showing: null when closed, "new" while
  // linking one, or the link's own id. A link has exactly one editable field —
  // the GIP framework, which saves on change — so the pane states the advisor
  // and offers that one control, rather than a form with nothing to submit.
  let advisorPanel = $state(null);
  const editingAdvisor = $derived(farmAdvisors.find((d) => d.link.id === advisorPanel) ?? null);

  function showAdvisorPanel(detail = null) {
    newAdvisorId = "";
    newGipCode = "";
    advisorPanel = detail?.link.id ?? "new";
  }

  // --- water abstraction points (model 2.2) ------------------------------------

  function showWaterForm(point = null) {
    editingWaterPointId = point?.id ?? null;
    waterPlotId = point?.plot_id ?? plots[0]?.plot?.id ?? "";
    waterDenomination = point?.denomination ?? "";
    waterInside = point ? point.inside_plot : true;
    waterDistance = point?.distance_m ?? "";
    waterLatitude = point?.latitude ?? "";
    waterLongitude = point?.longitude ?? "";
    waterFormOpen = true;
  }

  function hideWaterForm() {
    waterFormOpen = false;
    editingWaterPointId = null;
  }

  async function submitWaterPoint() {
    const denomination = waterDenomination.trim();
    // A point inside the plot has no distance to state; sending one is refused
    // by the backend, because it contradicts the answer beside it.
    const payload = {
      denomination,
      inside_plot: waterInside,
      distance_m: waterInside ? null : numberOrNull(waterDistance),
      latitude: numberOrNull(waterLatitude),
      longitude: numberOrNull(waterLongitude),
    };
    if (editingWaterPointId) {
      await invoke("update_water_point", {
        waterPointId: editingWaterPointId,
        update: payload,
      });
    } else {
      await invoke("create_water_point", {
        waterPoint: { plot_id: waterPlotId, ...payload },
      });
    }
    hideWaterForm();
    await reloadWater();
  }

  function deleteWaterPoint(point) {
    run(async () => {
      if (!(await confirmDialog(t("water_point.delete_confirm", { name: point.denomination }))))
        return;
      await invoke("delete_water_point", { waterPointId: point.id });
      hideWaterForm();
      await reloadWater();
    });
  }

  /// Saves on change, like the advisor's framework select: the checkbox IS the
  /// statement. Recording a point withdraws the declaration on the backend, so
  /// the two can never both stand.
  function toggleWaterDeclaration(plotId, declared) {
    run(async () => {
      await invoke("set_water_declaration", { plotId, declared });
      await reloadWater();
    });
  }

  function plotNameOf(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? "";
  }

  /// The point the inspector is showing, so its delete button knows which one
  /// it is about. Null while creating.
  const editingWaterPoint = $derived(
    waterPoints.find((point) => point.id === editingWaterPointId) ?? null,
  );

  // --- plots -------------------------------------------------------------------

  function showPlotForm(plot = null, es = null) {
    editingPlotId = plot?.id ?? null;
    plotName = plot?.name ?? "";
    plotArea = plot?.area_ha ?? "";
    const next = {};
    for (const field of SIGPAC_FIELDS) next[field] = es?.[field] ?? "";
    sigpac = next;
    sigpacLookup = null;
    plotFormOpen = true;
  }

  function hidePlotForm() {
    plotFormOpen = false;
    editingPlotId = null;
    sigpacLookup = null;
  }

  // --- SIGPAC lookup (Door A: verify/prefill while typing) --------------------

  let sigpacLookup = $state(null);
  const sigpacComplete = $derived(
    farm?.country_code === "es" &&
      SIGPAC_FIELDS.every((field) => String(sigpac[field] ?? "").trim() !== ""),
  );

  function sigpacParts() {
    return SIGPAC_FIELDS.map((field) => String(sigpac[field] ?? "").trim());
  }

  function lookupSigpac() {
    const parts = sigpacParts();
    run(async () => {
      const result = await invoke("sigpac_lookup_reference", { parts, refresh: false });
      // Remember which parts were looked up: the post-save verification only
      // runs if the reference was not edited afterwards.
      sigpacLookup = result
        ? { ...result, parts: parts.join("/") }
        : { notFound: true, parts: parts.join("/") };
    });
  }

  const sigpacDuplicates = $derived(
    (sigpacLookup?.matching_plots ?? []).filter((m) => m.plot_id !== editingPlotId),
  );

  function verifyPlot(plot) {
    run(async () => {
      const result = await invoke("sigpac_verify_plot", {
        plotId: plot.id,
        refresh: Boolean(sigpacFeatures[plot.id]),
      });
      if (result) {
        notify(t("message.sigpac_boundary_saved", { name: plot.name }));
        if (result.zone_check_error) notify(t("plot.zones_unchecked"), true);
      } else {
        notify(t("plot.sigpac_not_found"), true);
      }
      await reloadPlots();
    });
  }

  function refComplete(es) {
    return Boolean(es) && SIGPAC_FIELDS.every((field) => String(es[field] ?? "").trim() !== "");
  }

  function collectSigpac() {
    if (farm.country_code !== "es") return null;
    const es = {};
    let any = false;
    for (const field of SIGPAC_FIELDS) {
      const value = String(sigpac[field] ?? "").trim();
      es[field] = value || null;
      if (value) any = true;
    }
    return any ? es : null;
  }

  async function submitPlot() {
    const trimmed = plotName.trim();
    const payload = {
      name: trimmed,
      area_ha: numberOrNull(plotArea),
      es: collectSigpac(),
    };
    let plotId = editingPlotId;
    if (editingPlotId) {
      await invoke("update_plot", { plotId: editingPlotId, update: payload });
    } else {
      plotId = (await invoke("create_plot", { plot: { farm_id: farmId, ...payload } })).id;
    }
    // A successful in-form lookup means the response is already cached, so
    // storing the official boundary now works offline too. Skipped if the
    // reference was edited after the lookup.
    if (sigpacLookup?.recinto && sigpacLookup.parts === sigpacParts().join("/")) {
      const verified = await invoke("sigpac_verify_plot", { plotId, refresh: false });
      notify(t("message.sigpac_boundary_saved", { name: trimmed }));
      if (verified?.zone_check_error) notify(t("plot.zones_unchecked"), true);
    }
    hidePlotForm();
    await reloadPlots();
  }

  function deletePlot(plot) {
    run(async () => {
      if (!(await confirmDialog(t("plot.delete_confirm", { name: plot.name })))) return;
      await invoke("delete_plot", { plotId: plot.id });
      hidePlotForm();
      await reloadPlots();
    });
  }

  // Compact "47:122:0:0:5:23:1" style SIGPAC reference, as the table's own
  // cell. The "SIGPAC" prefix the card line carried is gone: the column is
  // headed with it.
  function sigpacSummary(es) {
    if (!es) return null;
    const parts = SIGPAC_FIELDS.map((field) => es[field]);
    return parts.some((p) => p) ? parts.map((p) => p ?? "·").join(":") : null;
  }

  /// The plot the inspector is editing, so the delete button and the two
  /// per-plot actions beside the form know which one they are about. Null
  /// while creating.
  const editingPlot = $derived(plots.find(({ plot }) => plot.id === editingPlotId) ?? null);
</script>

<section class="view">
  <a href="#/farms">{t("farms.back")}</a>

  {#if farm}
    <div class="view-head">
      <h2>{farm.name}</h2>
      <button type="button" class="btn-danger" onclick={deleteFarm}>{t("farm.delete")}</button>
    </div>

    <TzForm onsubmit={submitFarm}>
      <div class="form-grid">
        <TextInput label={t("farm.name")} required bind:value={name} />
        <TextInput label={t("farm.owner")} bind:value={ownerName} />
        <TextInput label={t("farm.owner_tax_id")} bind:value={ownerTaxId} />
        <TzSelect
          label={t("farm.country")}
          items={codeItems(countries, "country")}
          bind:value={countryCode}
        />
        <TextInput label={t("farm.address")} bind:value={address} />
        <TextInput label={t("farm.location")} bind:value={locationText} />
        <TextInput label={t("farm.postal_code")} bind:value={postalCode} />
        <TextInput label={t("farm.phone_fixed")} bind:value={phoneFixed} />
        <TextInput label={t("farm.phone_mobile")} bind:value={phoneMobile} />
        <TextInput label={t("farm.email")} type="email" bind:value={email} />
        <DateInput
          label={t("farm.opened_on")}
          hint={t("farm.opened_on_hint")}
          bind:value={openedOn}
        />
        <NumberInput label={t("farm.latitude")} min={-90} max={90} bind:value={latitude} />
        <NumberInput label={t("farm.longitude")} min={-180} max={180} bind:value={longitude} />
      </div>
      {#if countryCode === "es"}
        <fieldset class="es-only">
          <legend>{t("farm.es_section")}</legend>
          <div class="form-grid">
            <TextInput label={t("farm.siex")} bind:value={siexCode} />
            <TextInput label={t("farm.rea")} bind:value={reaCode} />
            <TextInput label={t("farm.rega")} bind:value={regaCode} />
            <TextInput label={t("farm.province")} bind:value={provinceCode} />
          </div>
        </fieldset>
      {/if}
      <fieldset>
        <legend>{t("farm.representative_section")}</legend>
        <p class="detail">{t("farm.representative_hint")}</p>
        <div class="form-grid">
          <TextInput label={t("farm.rep_name")} bind:value={repName} />
          <TextInput label={t("farm.rep_tax_id")} bind:value={repTaxId} />
          <TextInput label={t("farm.rep_kind")} bind:value={repKind} />
          <TextInput label={t("farm.rep_address")} bind:value={repAddress} />
          <TextInput label={t("farm.rep_locality")} bind:value={repLocality} />
          <TextInput label={t("farm.rep_province")} bind:value={repProvince} />
          <TextInput label={t("farm.rep_postal_code")} bind:value={repPostalCode} />
          <TextInput label={t("farm.rep_phone")} bind:value={repPhone} />
          <TextInput label={t("farm.rep_email")} type="email" bind:value={repEmail} />
        </div>
      </fieldset>
      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
      </div>
    </TzForm>

    <div class="view-head">
      <h3>{t("farm.advisors_section")}</h3>
      <button
        type="button"
        disabled={linkableAdvisors.length === 0}
        onclick={() => showAdvisorPanel()}
      >
        {t("farm.advisor_add")}
      </button>
    </div>
    <p class="detail">{t("farm.advisors_hint")}</p>
    {#if advisors.length === 0}
      <p class="detail">{t("farm.advisors_none_available")}</p>
    {:else if linkableAdvisors.length === 0 && farmAdvisors.length > 0}
      <p class="detail">{t("farm.advisors_all_linked")}</p>
    {/if}

    <TzWorkspace
      open={advisorPanel !== null}
      title={editingAdvisor ? editingAdvisor.advisor.name : t("farm.advisor_add")}
      onclose={() => (advisorPanel = null)}
      ondelete={editingAdvisor ? () => unlinkAdvisor(editingAdvisor) : null}
      deleteLabel={t("farm.advisor_remove")}
    >
      {#snippet list()}
        {#if farmAdvisors.length === 0}
          <p class="table-empty">{t("farm.advisors_empty")}</p>
        {:else}
          <div class="table-wrap">
            <table class="data-table" use:resizableColumns={"farm-advisors"}>
              <thead>
                <tr>
                  <th>{t("column.name")}</th>
                  <th>{t("column.tax_id")}</th>
                  <th>{t("column.registration_number")}</th>
                  <th>{t("column.gip")}</th>
                </tr>
              </thead>
              <tbody>
                {#each sortedFarmAdvisors as detail (detail.link.id)}
                  <tr
                    class:selected={advisorPanel === detail.link.id}
                    onclick={(e) => opensRow(e) && showAdvisorPanel(detail)}
                  >
                    <td class="col-name">
                      <button
                        type="button"
                        class="row-open"
                        onclick={() => showAdvisorPanel(detail)}
                      >
                        {detail.advisor.name}
                      </button>
                    </td>
                    <td class="col-muted">{detail.advisor.tax_id ?? ""}</td>
                    <td class="col-muted">{detail.advisor.registration_number ?? ""}</td>
                    <td class="col-muted">
                      {detail.link.gip_system_code
                        ? tCode("gip_system", detail.link.gip_system_code)
                        : ""}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/snippet}

      {#snippet inspector()}
        {#if editingAdvisor}
          <!-- The framework is the only thing a link holds, and it saves on
               change: the select IS the statement, so there is nothing to
               submit and no form to wrap it in. -->
          <div class="form-grid">
            <TzSelect
              label={t("advisor.gip_system")}
              items={codeItems(gipSystems, "gip_system")}
              nullable
              value={editingAdvisor.link.gip_system_code ?? ""}
              onchange={(code) => saveFarmAdvisor(editingAdvisor.advisor.id, code || null)}
            />
          </div>
        {:else}
          <div class="form-grid">
            <TzSelect
              label={t("advisors.title")}
              items={nameItems(linkableAdvisors)}
              nullable
              bind:value={newAdvisorId}
            />
            <TzSelect
              label={t("advisor.gip_system")}
              items={codeItems(gipSystems, "gip_system")}
              nullable
              bind:value={newGipCode}
            />
          </div>
        {/if}
      {/snippet}

      <!-- Guarded, and the guard is the one this row already had before it moved
           into the pinned bar: an EXISTING link has nothing to add and saves on
           change, so the panel offers it nothing to press. -->
      {#snippet actions()}
        {#if !editingAdvisor}
          <div class="form-actions">
            <button type="button" disabled={!newAdvisorId} onclick={linkAdvisor}>
              {t("farm.advisor_add")}
            </button>
            <button type="button" class="btn-cancel" onclick={() => (advisorPanel = null)}>
              {t("form.cancel")}
            </button>
          </div>
        {/if}
      {/snippet}
    </TzWorkspace>

    <div class="view-head">
      <h3>{t("plots.title")}</h3>
      <button type="button" onclick={() => showPlotForm()}>{t("plots.new")}</button>
    </div>

    <TzWorkspace
      open={plotFormOpen}
      title={editingPlotId ? plotName : t("plots.new")}
      onclose={hidePlotForm}
      ondelete={editingPlot ? () => deletePlot(editingPlot.plot) : null}
      deleteLabel={t("plot.delete")}
    >
      {#snippet list()}
        {#if loading}
          <Skeleton />
        {:else if plots.length === 0}
          <p class="table-empty">{t("plots.empty")}</p>
        {:else}
          <div class="table-wrap">
            <table class="data-table" use:resizableColumns={"plots"}>
              <thead>
                <tr>
                  <th>{t("column.name")}</th>
                  <th class="col-num">{t("column.area_ha")}</th>
                  <th>{t("column.sigpac")}</th>
                  <th class="col-num">{t("column.official_area")}</th>
                  <th>{t("column.zones")}</th>
                </tr>
              </thead>
              <tbody>
                {#each plots as { plot, es } (plot.id)}
                  <tr
                    class:selected={editingPlotId === plot.id}
                    onclick={(e) => opensRow(e) && showPlotForm(plot, es)}
                  >
                    <td class="col-name">
                      <button type="button" class="row-open" onclick={() => showPlotForm(plot, es)}>
                        {plot.name}
                      </button>
                    </td>
                    <td class="col-muted col-num">
                      {plot.area_ha == null ? "" : formatNumber(plot.area_ha)}
                    </td>
                    <td class="col-muted">{sigpacSummary(es) ?? ""}</td>
                    <td class="col-muted col-num">
                      {sigpacFeatures[plot.id]?.official_area_ha == null
                        ? ""
                        : formatNumber(sigpacFeatures[plot.id].official_area_ha)}
                    </td>
                    <td>
                      {#each zoneFlags[plot.id] ?? [] as zone (zone.zone_type_code)}
                        <TzTooltip label={zone.detail ?? ""}>
                          {#snippet trigger(props)}
                            <span {...props} class="zone-chip">
                              {tCode("zone", zone.zone_type_code)}{zone.coverage_pct != null &&
                              zone.coverage_pct < 99.95
                                ? ` ${formatPercent(zone.coverage_pct)}`
                                : ""}
                            </span>
                          {/snippet}
                        </TzTooltip>
                      {/each}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/snippet}

      {#snippet inspector(formId)}
        <!-- Two things a plot can do that are neither editing nor deleting it:
             show its boundary on the map, and ask SIGPAC for it. They were a
             link and a button on every row; here they name the plot the pane
             is about. -->
        {#if editingPlot}
          <div class="plot-actions">
            <a href={mapHref(editingPlotId)}>{t("plot.on_map")}</a>
            {#if refComplete(editingPlot.es)}
              <button type="button" onclick={() => verifyPlot(editingPlot.plot)}>
                {sigpacFeatures[editingPlotId] ? "SIGPAC ✓" : t("plot.sigpac_verify")}
              </button>
            {/if}
          </div>
        {/if}

        <TzForm id={formId} onsubmit={submitPlot}>
          <div class="form-grid">
            <TextInput label={t("plot.name")} required bind:value={plotName} />
            <NumberInput label={t("plot.area")} min={0.01} bind:value={plotArea} />
          </div>
          {#if farm.country_code === "es"}
            <fieldset class="es-only">
              <legend>{t("plot.sigpac_section")}</legend>
              <!-- One hint for the whole seven-part reference: the farmer looks
                 the lot up in a single visit to the visor, so seven identical
                 notes would be seven times the noise for one answer. -->
              <RegistryHint country={farm.country_code} field="plot.sigpac" block />
              <div class="form-grid sigpac-grid">
                {#each SIGPAC_FIELDS as field (field)}
                  <TextInput label={t(`plot.${field}`)} bind:value={sigpac[field]} />
                {/each}
              </div>
              <div class="sigpac-lookup">
                <button type="button" disabled={!sigpacComplete} onclick={lookupSigpac}>
                  {t("plot.sigpac_verify")}
                </button>
                {#if sigpacLookup?.notFound}
                  <p class="detail">{t("plot.sigpac_not_found")}</p>
                {:else if sigpacLookup?.recinto}
                  <p class="detail">
                    {t("plot.sigpac_found", {
                      area: formatNumber(sigpacLookup.recinto.properties.superficie),
                      use: sigpacLookup.recinto.properties.uso_sigpac,
                    })}
                    <button
                      type="button"
                      onclick={() => (plotArea = sigpacLookup.recinto.properties.superficie)}
                    >
                      {t("plot.sigpac_use_area")}
                    </button>
                  </p>
                  {#each sigpacDuplicates as match (match.plot_id)}
                    <p class="detail warn">
                      <TriangleAlert />
                      {t("plot.sigpac_already_on", {
                        plot: match.plot_name,
                        farm: match.farm_name,
                      })}
                    </p>
                  {/each}
                {/if}
              </div>
            </fieldset>
          {/if}
        </TzForm>
      {/snippet}

      {#snippet actions(formId)}
        <div class="form-actions">
          <button type="submit" form={formId}>{t("form.save")}</button>
          <button type="button" class="btn-cancel" onclick={hidePlotForm}>
            {t("form.cancel")}
          </button>
        </div>
      {/snippet}
    </TzWorkspace>

    {#if !loading}
      <div class="view-head">
        <h3>{t("water_points.title")}</h3>
        <button type="button" disabled={plots.length === 0} onclick={() => showWaterForm()}>
          {t("water_points.new")}
        </button>
      </div>
      <p class="detail">{t("water_points.hint")}</p>

      <TzWorkspace
        open={waterFormOpen}
        title={editingWaterPointId ? waterDenomination : t("water_points.new")}
        onclose={hideWaterForm}
        ondelete={editingWaterPoint ? () => deleteWaterPoint(editingWaterPoint) : null}
      >
        {#snippet list()}
          {#if waterPoints.length === 0}
            <p class="table-empty">{t("water_points.empty")}</p>
          {:else}
            <div class="table-wrap">
              <table class="data-table" use:resizableColumns={"water-points"}>
                <thead>
                  <tr>
                    <th>{t("column.denomination")}</th>
                    <th>{t("column.plot")}</th>
                    <th>{t("column.inside")}</th>
                    <th class="col-num">{t("column.distance")}</th>
                    <th>{t("column.coordinates")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each waterPoints as point (point.id)}
                    <tr
                      class:selected={editingWaterPointId === point.id}
                      onclick={(e) => opensRow(e) && showWaterForm(point)}
                    >
                      <td class="col-name">
                        <button type="button" class="row-open" onclick={() => showWaterForm(point)}>
                          {point.denomination}
                        </button>
                      </td>
                      <td class="col-muted">{plotNameOf(point.plot_id)}</td>
                      <td class="col-muted">
                        {point.inside_plot ? t("water_point.inside_yes") : ""}
                      </td>
                      <!-- Blank for a point INSIDE the plot: there is no
                           distance to state, and a 0 would be an answer
                           nobody gave. -->
                      <td class="col-muted col-num">
                        {point.distance_m == null ? "" : formatNumber(point.distance_m)}
                      </td>
                      <td class="col-muted">
                        {point.latitude == null
                          ? ""
                          : formatCoordinates(point.latitude, point.longitude)}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {/snippet}

        {#snippet inspector(formId)}
          <TzForm
            id={formId}
            onsubmit={submitWaterPoint}
            anchors={{
              "invalid.missing_distance": "distance_m",
              "invalid.water_point_distance_inside": "distance_m",
            }}
          >
            <div class="form-grid">
              <TzSelect
                label={t("plot.name")}
                items={nameItems(
                  plots,
                  (p) => p.plot.name,
                  (p) => p.plot.id,
                )}
                required
                disabled={Boolean(editingWaterPointId)}
                bind:value={waterPlotId}
              />
              <TextInput
                label={t("water_point.denomination")}
                required
                bind:value={waterDenomination}
              />
              <TzCheckbox label={t("water_point.inside_plot")} bind:checked={waterInside} />
              <NumberInput
                label={t("water_point.distance")}
                name="distance_m"
                min={0.01}
                required={!waterInside}
                disabled={waterInside}
                bind:value={waterDistance}
              />
              <NumberInput
                label={t("water_point.latitude")}
                min={-90}
                max={90}
                bind:value={waterLatitude}
              />
              <NumberInput
                label={t("water_point.longitude")}
                min={-180}
                max={180}
                bind:value={waterLongitude}
              />
            </div>
          </TzForm>
        {/snippet}

        {#snippet actions(formId)}
          <div class="form-actions">
            <button type="submit" form={formId}>{t("form.save")}</button>
            <button type="button" class="btn-cancel" onclick={hideWaterForm}>
              {t("form.cancel")}
            </button>
          </div>
        {/snippet}
      </TzWorkspace>

      <!-- The stored negatives: one row per plot saying the farmer looked and
           found nothing. `rows-static` because the checkbox IS the row — there
           is no record here to open, and a row that offers to be clicked and
           then does nothing is a worse row than a plain one. -->
      {#if plots.length > 0}
        <div class="table-wrap water-declarations">
          <table class="data-table rows-static">
            <thead>
              <tr>
                <th>{t("column.plot")}</th>
                <th>{t("water_points.none_column")}</th>
              </tr>
            </thead>
            <tbody>
              {#each plots as { plot } (plot.id)}
                {@const hasPoints = waterPoints.some((p) => p.plot_id === plot.id)}
                <tr>
                  <td class="col-name">{plot.name}</td>
                  <td>
                    <TzCheckbox
                      label={t("water_points.none_on", { plot: plot.name })}
                      labelHidden
                      disabled={hasPoints}
                      checked={waterDeclared.has(plot.id)}
                      onchange={(next) => toggleWaterDeclaration(plot.id, next)}
                    />
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

      <div class="view-head">
        <h3>{t("farm.map_title")}</h3>
        <a href={mapHref()}>{t("farm.open_map")}</a>
      </div>
      <div class="farm-map-embed">
        <MapCanvas {farmId} centerHint={farm} bind:selectedPlotId={mapSelectedPlotId} />
      </div>
    {/if}
  {/if}
</section>

<style>
  .farm-map-embed {
    height: 24rem;
  }
  .sigpac-lookup {
    margin-top: var(--space-2);
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .sigpac-lookup .detail {
    margin: 0;
  }
  /* "this reference is already on another plot" — a caution, not a failure, so
     it keeps .detail's muted size and only the icon carries the warning. */
  .detail.warn {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1);
  }
  .detail.warn :global(svg) {
    width: 0.9rem;
    height: 0.9rem;
    flex: none;
    margin-top: 0.1em;
    color: var(--warning);
  }
  /* What a plot can do that is neither editing nor deleting it, at the head of
     its pane: show its boundary on the map, and ask SIGPAC for it. */
  .plot-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding-bottom: var(--space-2);
  }
  /* The declarations follow the points in the same section, and two tables
     whose header rows touch read as one table with a stray heading in the
     middle. This is the gap that says they are two answers to one question. */
  .water-declarations {
    margin-top: var(--space-5);
  }
  .zone-chip {
    align-self: center;
    font-size: 0.75rem;
    padding: 0.1rem 0.5rem;
    border: 1px solid var(--warning);
    border-radius: var(--radius-pill);
    color: var(--warning);
    white-space: nowrap;
  }
</style>
