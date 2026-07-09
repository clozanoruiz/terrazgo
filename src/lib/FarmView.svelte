<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Farm detail: edit form, plots list and the shared create/edit plot form.
  // The SIGPAC/REGA fieldsets only apply to Spanish farms.
  import { t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import MapCanvas from "./MapCanvas.svelte";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

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

  let countries = $state([]);
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
  let countryCode = $state("");
  let locationText = $state("");
  let latitude = $state("");
  let longitude = $state("");
  let regaCode = $state("");
  let provinceCode = $state("");

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
    countryCode = detail.farm.country_code;
    locationText = detail.farm.location_text ?? "";
    latitude = detail.farm.latitude ?? "";
    longitude = detail.farm.longitude ?? "";
    regaCode = detail.es?.rega_code ?? "";
    provinceCode = detail.es?.province_code ?? "";
  }

  run(async () => {
    countries = await invoke("list_countries");
    fillFarmForm(await invoke("get_farm", { farmId }));
    await reloadPlots();
  }).finally(() => (loading = false));

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
    const province = provinceCode.trim() || null;
    return rega || province ? { rega_code: rega, province_code: province } : null;
  }

  function submitFarm(event) {
    event.preventDefault();
    const update = {
      name: name.trim(),
      owner_name: ownerName.trim() || null,
      location_text: locationText.trim() || null,
      latitude: numberOrNull(latitude),
      longitude: numberOrNull(longitude),
      country_code: countryCode,
      es: collectFarmEs(),
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
        <label
          ><span>{t("farm.country")}</span>
          <select bind:value={countryCode}>
            {#each countries as country (country.code)}
              <option value={country.code}>{tCode("country", country.code)}</option>
            {/each}
          </select>
        </label>
        <label><span>{t("farm.location")}</span><input bind:value={locationText} /></label>
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
            <label><span>{t("farm.rega")}</span><input bind:value={regaCode} /></label>
            <label><span>{t("farm.province")}</span><input bind:value={provinceCode} /></label>
          </div>
        </fieldset>
      {/if}
      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
      </div>
    </form>

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
  .zone-chip {
    align-self: center;
    font-size: 0.75rem;
    padding: 0.1rem 0.5rem;
    border: 1px solid var(--warning);
    border-radius: 999px;
    color: var(--warning);
    white-space: nowrap;
  }
</style>
