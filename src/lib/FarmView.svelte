<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Farm detail: edit form, plots list and the shared create/edit plot form.
  // The SIGPAC/REGA fieldsets only apply to Spanish farms.
  import { t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { sortedBy } from "./collate.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import DateInput from "./DateInput.svelte";
  import MapCanvas from "./MapCanvas.svelte";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";

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
    // Zone chips: the latest campaign's 'inside' flags per plot (rows arrive
    // campaign-descending, so the first flag per (plot, type) wins).
    const flags = await invoke("list_zone_flags", { farmId });
    const zones = {};
    const seen = {};
    for (const flag of flags) {
      const key = `${flag.plot_id}/${flag.zone_type_code}`;
      if (seen[key]) continue;
      seen[key] = true;
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

  function submitFarm(event) {
    event.preventDefault();
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
    run(async () => {
      fillFarmForm(await invoke("update_farm", { farmId, update }));
      notify(t("message.farm_saved", { name: update.name }));
    });
  }

  function deleteFarm() {
    run(async () => {
      if (!(await confirmDialog(t("farm.delete_confirm", { name: farm.name })))) return;
      await invoke("delete_farm", { farmId });
      notify(t("message.farm_deleted"));
      location.hash = "#/farms";
    });
  }

  // --- advisory (model 1.4) ----------------------------------------------------

  /// One command for both cases: linking an advisor and restating the
  /// framework of an existing link are the same statement about the holding.
  function saveFarmAdvisor(advisorId, gipSystemCode) {
    run(async () => {
      await invoke("set_farm_advisor", { farmId, advisorId, gipSystemCode });
      notify(t("message.farm_advisor_saved"));
      await reloadAdvisors();
    });
  }

  function linkAdvisor() {
    if (!newAdvisorId) return;
    const advisorId = newAdvisorId;
    const gip = newGipCode || null;
    newAdvisorId = "";
    newGipCode = "";
    saveFarmAdvisor(advisorId, gip);
  }

  function unlinkAdvisor(detail) {
    run(async () => {
      if (!(await confirmDialog(t("farm.advisor_remove_confirm", { name: detail.advisor.name }))))
        return;
      await invoke("remove_farm_advisor", { linkId: detail.link.id });
      notify(t("message.farm_advisor_removed"));
      await reloadAdvisors();
    });
  }

  function advisorDetail(advisor) {
    return [advisor.tax_id, advisor.registration_number].filter(Boolean).join(" · ");
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

  function submitWaterPoint(event) {
    event.preventDefault();
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
    run(async () => {
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
      notify(t("message.water_point_saved", { name: denomination }));
      hideWaterForm();
      await reloadWater();
    });
  }

  function deleteWaterPoint(point) {
    run(async () => {
      if (!(await confirmDialog(t("water_point.delete_confirm", { name: point.denomination }))))
        return;
      await invoke("delete_water_point", { waterPointId: point.id });
      notify(t("message.water_point_deleted"));
      await reloadWater();
    });
  }

  /// Saves on change, like the advisor's framework select: the checkbox IS the
  /// statement. Recording a point withdraws the declaration on the backend, so
  /// the two can never both stand.
  function toggleWaterDeclaration(plotId, declared) {
    run(async () => {
      await invoke("set_water_declaration", { plotId, declared });
      notify(t(declared ? "message.water_declared" : "message.water_declaration_cleared"));
      await reloadWater();
    });
  }

  function plotNameOf(plotId) {
    return plots.find((p) => p.plot.id === plotId)?.plot.name ?? "";
  }

  function waterDetail(point) {
    return [
      plotNameOf(point.plot_id),
      point.inside_plot
        ? t("water_point.inside_yes")
        : t("water_point.outside_at", { distance: point.distance_m ?? "—" }),
      point.latitude != null ? `${point.latitude}, ${point.longitude}` : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }

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

  function submitPlot(event) {
    event.preventDefault();
    const trimmed = plotName.trim();
    const payload = {
      name: trimmed,
      area_ha: numberOrNull(plotArea),
      es: collectSigpac(),
    };
    run(async () => {
      let plotId = editingPlotId;
      if (editingPlotId) {
        await invoke("update_plot", { plotId: editingPlotId, update: payload });
      } else {
        plotId = (await invoke("create_plot", { plot: { farm_id: farmId, ...payload } })).id;
      }
      notify(t("message.plot_saved", { name: trimmed }));
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
    });
  }

  function deletePlot(plot) {
    run(async () => {
      if (!(await confirmDialog(t("plot.delete_confirm", { name: plot.name })))) return;
      await invoke("delete_plot", { plotId: plot.id });
      notify(t("message.plot_deleted"));
      await reloadPlots();
    });
  }

  // Compact "47:122:0:0:5:23:1" style SIGPAC reference for the plot card.
  function sigpacSummary(es) {
    if (!es) return null;
    const parts = SIGPAC_FIELDS.map((field) => es[field]);
    return parts.some((p) => p) ? `SIGPAC ${parts.map((p) => p ?? "·").join(":")}` : null;
  }

  function plotDetail(plot, es) {
    const official = sigpacFeatures[plot.id]?.official_area_ha;
    return [
      plot.area_ha != null ? `${plot.area_ha} ha` : null,
      sigpacSummary(es),
      official != null ? t("plot.sigpac_official", { area: official }) : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<section class="view">
  <a href="#/farms">{t("farms.back")}</a>

  {#if farm}
    <div class="view-head">
      <h2>{farm.name}</h2>
      <button type="button" class="btn-danger" onclick={deleteFarm}>{t("farm.delete")}</button>
    </div>

    <form onsubmit={submitFarm}>
      <div class="form-grid">
        <label><span>{t("farm.name")}</span><input required bind:value={name} /></label>
        <label><span>{t("farm.owner")}</span><input bind:value={ownerName} /></label>
        <label><span>{t("farm.owner_tax_id")}</span><input bind:value={ownerTaxId} /></label>
        <TzSelect
          label={t("farm.country")}
          items={codeItems(countries, "country")}
          bind:value={countryCode}
        />
        <label><span>{t("farm.address")}</span><input bind:value={address} /></label>
        <label><span>{t("farm.location")}</span><input bind:value={locationText} /></label>
        <label><span>{t("farm.postal_code")}</span><input bind:value={postalCode} /></label>
        <label><span>{t("farm.phone_fixed")}</span><input bind:value={phoneFixed} /></label>
        <label><span>{t("farm.phone_mobile")}</span><input bind:value={phoneMobile} /></label>
        <label><span>{t("farm.email")}</span><input type="email" bind:value={email} /></label>
        <DateInput
          label={t("farm.opened_on")}
          hint={t("farm.opened_on_hint")}
          bind:value={openedOn}
        />
        <label
          ><span>{t("farm.latitude")}</span>
          <input type="number" step="any" min="-90" max="90" bind:value={latitude} />
        </label>
        <label
          ><span>{t("farm.longitude")}</span>
          <input type="number" step="any" min="-180" max="180" bind:value={longitude} />
        </label>
      </div>
      {#if countryCode === "es"}
        <fieldset class="es-only">
          <legend>{t("farm.es_section")}</legend>
          <div class="form-grid">
            <label><span>{t("farm.siex")}</span><input bind:value={siexCode} /></label>
            <label><span>{t("farm.rea")}</span><input bind:value={reaCode} /></label>
            <label><span>{t("farm.rega")}</span><input bind:value={regaCode} /></label>
            <label><span>{t("farm.province")}</span><input bind:value={provinceCode} /></label>
          </div>
        </fieldset>
      {/if}
      <fieldset>
        <legend>{t("farm.representative_section")}</legend>
        <p class="detail">{t("farm.representative_hint")}</p>
        <div class="form-grid">
          <label><span>{t("farm.rep_name")}</span><input bind:value={repName} /></label>
          <label><span>{t("farm.rep_tax_id")}</span><input bind:value={repTaxId} /></label>
          <label><span>{t("farm.rep_kind")}</span><input bind:value={repKind} /></label>
          <label><span>{t("farm.rep_address")}</span><input bind:value={repAddress} /></label>
          <label><span>{t("farm.rep_locality")}</span><input bind:value={repLocality} /></label>
          <label><span>{t("farm.rep_province")}</span><input bind:value={repProvince} /></label>
          <label><span>{t("farm.rep_postal_code")}</span><input bind:value={repPostalCode} /></label
          >
          <label><span>{t("farm.rep_phone")}</span><input bind:value={repPhone} /></label>
          <label
            ><span>{t("farm.rep_email")}</span><input type="email" bind:value={repEmail} /></label
          >
        </div>
      </fieldset>
      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
      </div>
    </form>

    <div class="view-head">
      <h3>{t("farm.advisors_section")}</h3>
    </div>
    <p class="detail">{t("farm.advisors_hint")}</p>
    <ul class="card-list">
      {#each sortedFarmAdvisors as detail (detail.link.id)}
        <li class="card">
          <strong>{detail.advisor.name}</strong>
          <span class="detail">{advisorDetail(detail.advisor)}</span>
          <TzSelect
            class="card-field"
            label={t("advisor.gip_system")}
            items={codeItems(gipSystems, "gip_system")}
            nullable
            value={detail.link.gip_system_code ?? ""}
            onchange={(code) => saveFarmAdvisor(detail.advisor.id, code || null)}
          />
          <button type="button" class="btn-danger" onclick={() => unlinkAdvisor(detail)}>
            {t("farm.advisor_remove")}
          </button>
        </li>
      {/each}
    </ul>
    {#if farmAdvisors.length === 0}
      <p>{t("farm.advisors_empty")}</p>
    {/if}
    {#if advisors.length === 0}
      <p class="detail">{t("farm.advisors_none_available")}</p>
    {:else if linkableAdvisors.length > 0}
      <div class="form-grid advisor-add">
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
        <button type="button" disabled={!newAdvisorId} onclick={linkAdvisor}>
          {t("farm.advisor_add")}
        </button>
      </div>
    {:else}
      <p class="detail">{t("farm.advisors_all_linked")}</p>
    {/if}

    <div class="view-head">
      <h3>{t("plots.title")}</h3>
      <button type="button" onclick={() => showPlotForm()}>{t("plots.new")}</button>
    </div>

    {#if plotFormOpen}
      <form onsubmit={submitPlot}>
        <div class="form-grid">
          <label><span>{t("plot.name")}</span><input required bind:value={plotName} /></label>
          <label
            ><span>{t("plot.area")}</span>
            <input type="number" step="any" min="0.01" bind:value={plotArea} />
          </label>
        </div>
        {#if farm.country_code === "es"}
          <fieldset class="es-only">
            <legend>{t("plot.sigpac_section")}</legend>
            <div class="form-grid sigpac-grid">
              {#each SIGPAC_FIELDS as field (field)}
                <label><span>{t(`plot.${field}`)}</span><input bind:value={sigpac[field]} /></label>
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
                    area: sigpacLookup.recinto.properties.superficie,
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
                  <p class="detail">
                    ⚠ {t("plot.sigpac_already_on", {
                      plot: match.plot_name,
                      farm: match.farm_name,
                    })}
                  </p>
                {/each}
              {/if}
            </div>
          </fieldset>
        {/if}
        <div class="form-actions">
          <button type="submit">{t("form.save")}</button>
          <button type="button" class="btn-cancel" onclick={hidePlotForm}>{t("form.cancel")}</button
          >
        </div>
      </form>
    {/if}

    {#if loading}
      <Skeleton />
    {:else}
      <ul class="card-list">
        {#each plots as { plot, es } (plot.id)}
          <li class="card">
            <strong>{plot.name}</strong>
            <span class="detail">{plotDetail(plot, es)}</span>
            {#each zoneFlags[plot.id] ?? [] as zone (zone.zone_type_code)}
              <span class="zone-chip" title={zone.detail ?? ""}>
                {tCode("zone", zone.zone_type_code)}{zone.coverage_pct != null &&
                zone.coverage_pct < 99.95
                  ? ` ${Math.round(zone.coverage_pct)}%`
                  : ""}
              </span>
            {/each}
            <a class="card-link" href={mapHref(plot.id)}>{t("plot.on_map")}</a>
            {#if refComplete(es)}
              <button type="button" onclick={() => verifyPlot(plot)}>
                {sigpacFeatures[plot.id] ? "SIGPAC ✓" : t("plot.sigpac_verify")}
              </button>
            {/if}
            <button type="button" onclick={() => showPlotForm(plot, es)}>{t("plot.edit")}</button>
            <button type="button" class="btn-danger" onclick={() => deletePlot(plot)}
              >{t("plot.delete")}</button
            >
          </li>
        {/each}
      </ul>
      {#if plots.length === 0}
        <p>{t("plots.empty")}</p>
      {/if}

      <div class="view-head">
        <h3>{t("water_points.title")}</h3>
        <button type="button" disabled={plots.length === 0} onclick={() => showWaterForm()}>
          {t("water_points.new")}
        </button>
      </div>
      <p class="detail">{t("water_points.hint")}</p>

      {#if waterFormOpen}
        <form onsubmit={submitWaterPoint}>
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
            <label
              ><span>{t("water_point.denomination")}</span>
              <input required bind:value={waterDenomination} />
            </label>
            <label class="check">
              <input type="checkbox" bind:checked={waterInside} />
              <span>{t("water_point.inside_plot")}</span>
            </label>
            <label
              ><span>{t("water_point.distance")}</span>
              <input
                type="number"
                step="any"
                min="0.01"
                required={!waterInside}
                disabled={waterInside}
                bind:value={waterDistance}
              />
            </label>
            <label
              ><span>{t("water_point.latitude")}</span>
              <input type="number" step="any" bind:value={waterLatitude} />
            </label>
            <label
              ><span>{t("water_point.longitude")}</span>
              <input type="number" step="any" bind:value={waterLongitude} />
            </label>
          </div>
          <div class="form-actions">
            <button type="submit">{t("form.save")}</button>
            <button type="button" class="btn-cancel" onclick={hideWaterForm}>
              {t("form.cancel")}
            </button>
          </div>
        </form>
      {/if}

      <ul class="card-list">
        {#each waterPoints as point (point.id)}
          <li class="card">
            <strong>{point.denomination}</strong>
            <span class="detail">{waterDetail(point)}</span>
            <button type="button" onclick={() => showWaterForm(point)}>{t("plot.edit")}</button>
            <button type="button" class="btn-danger" onclick={() => deleteWaterPoint(point)}
              >{t("plot.delete")}</button
            >
          </li>
        {/each}
      </ul>
      {#if waterPoints.length === 0 && plots.length > 0}
        <p>{t("water_points.empty")}</p>
      {/if}

      {#if plots.length > 0}
        <ul class="card-list water-declarations">
          {#each plots as { plot } (plot.id)}
            {@const hasPoints = waterPoints.some((p) => p.plot_id === plot.id)}
            <li class="card">
              <label class="check">
                <input
                  type="checkbox"
                  disabled={hasPoints}
                  checked={waterDeclared.has(plot.id)}
                  onchange={(e) => toggleWaterDeclaration(plot.id, e.currentTarget.checked)}
                />
                <span>{t("water_points.none_on", { plot: plot.name })}</span>
              </label>
            </li>
          {/each}
        </ul>
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
  .card-link {
    align-self: center;
  }
  .sigpac-lookup {
    margin-top: 0.6rem;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
  }
  .sigpac-lookup .detail {
    margin: 0;
  }
  /* A checkbox reads as one line, not as .form-grid's stacked label + field. */
  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
  }
  .check span {
    font-size: 0.875rem;
  }
  /* The per-plot "no abstraction points" statements: a quieter list than the
     points themselves, because most of it is normally unticked. */
  .water-declarations .card {
    padding-block: 0.35rem;
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
