<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The Map workspace — the routed hub around MapCanvas: farm selector,
  // layer panel (data-driven from mapLayers.js, ready for future module
  // layers), plot panel with the draw / import / delete boundary workflows.
  // Deep-linkable: #/map?farm=<id>&plot=<id> (FarmView's "open in map").
  import { t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import MapCanvas from "./MapCanvas.svelte";
  import { MAP_LAYERS, mapPalette } from "./mapLayers.js";
  import { notify, run } from "./notifications.svelte.js";

  const params = new URLSearchParams((location.hash.split("?")[1] ?? "").replace(/\?/g, "&"));

  let farms = $state([]);
  let plots = $state([]);
  let farmId = $state(params.get("farm"));
  let selectedPlotId = $state(params.get("plot"));
  let visibleLayers = $state(MAP_LAYERS.filter((l) => l.defaultVisible !== false).map((l) => l.id));
  let mapZoom = $state(null);
  const palette = mapPalette();
  const visibleLegends = $derived(
    MAP_LAYERS.filter((l) => l.legend && visibleLayers.includes(l.id)),
  );
  // Tile overlays draw nothing below their service's minimum zoom — warn
  // instead of leaving an on-toggle that silently shows nothing.
  const belowZoomLayers = $derived(
    mapZoom === null
      ? []
      : MAP_LAYERS.filter(
          (l) => l.minZoom != null && visibleLayers.includes(l.id) && mapZoom < l.minZoom,
        ).map((l) => t(l.labelKey)),
  );
  let drawing = $state(false);
  // SIGPAC point lookup (Door B): pick a point, then create or attach.
  let picking = $state(false);
  let sigpacResult = $state(null);
  let refreshToken = $state(0);
  // Boundary rows of the selected plot, derived from the map's loaded data.
  let plotFeatures = $state({});
  // Point-inspect result: what the visible overlays render where the user
  // clicked (null = nothing there / no click yet).
  let inspectGroups = $state(null);

  // Import picker state (a file has been read, the user picks a feature).
  let importPath = $state(null);
  let importEntries = $state([]);
  let importFilter = $state("");

  const selectedFarm = $derived(farms.find((f) => f.id === farmId) ?? null);
  const selectedPlot = $derived(plots.find(({ plot }) => plot.id === selectedPlotId)?.plot ?? null);
  const selectedBoundaries = $derived(
    (plotFeatures.plots?.features ?? [])
      .filter((f) => f.properties.plotId === selectedPlotId)
      .map((f) => f.properties),
  );
  const filteredImportEntries = $derived(
    importEntries.filter((entry) => {
      if (!importFilter.trim()) return true;
      const needle = importFilter.trim().toLowerCase();
      const haystack = `${entry.name ?? ""} ${JSON.stringify(entry.properties ?? {})}`;
      return haystack.toLowerCase().includes(needle);
    }),
  );

  run(async () => {
    farms = await invoke("list_farms");
    if (!farmId && farms.length > 0) farmId = farms[0].id;
    await reloadPlots();
  });

  async function reloadPlots() {
    plots = farmId ? await invoke("list_plots", { farmId }) : [];
    if (selectedPlotId && !plots.some(({ plot }) => plot.id === selectedPlotId)) {
      selectedPlotId = null;
    }
  }

  function switchFarm(event) {
    farmId = event.target.value;
    selectedPlotId = null;
    drawing = false;
    picking = false;
    sigpacResult = null;
    cancelImport();
    run(reloadPlots);
    syncHash();
  }

  function selectPlot(plotId) {
    selectedPlotId = selectedPlotId === plotId ? null : plotId;
    syncHash();
  }

  // Keep the hash shareable without triggering the router (replaceState does
  // not fire hashchange, and the #/map prefix keeps this view active anyway).
  function syncHash() {
    const parts = [];
    if (farmId) parts.push(`farm=${encodeURIComponent(farmId)}`);
    if (selectedPlotId) parts.push(`plot=${encodeURIComponent(selectedPlotId)}`);
    history.replaceState(null, "", `#/map${parts.length ? `?${parts.join("&")}` : ""}`);
  }

  function onLayerData(layerId, data) {
    plotFeatures = { ...plotFeatures, [layerId]: data };
  }

  function onInspect(groups) {
    inspectGroups = groups.length > 0 ? groups : null;
  }

  function toggleLayer(layerId) {
    visibleLayers = visibleLayers.includes(layerId)
      ? visibleLayers.filter((id) => id !== layerId)
      : [...visibleLayers, layerId];
  }

  // --- boundary workflows -------------------------------------------------------

  function saveBoundary(geometry, source) {
    run(async () => {
      await invoke("save_plot_boundary", {
        plotId: selectedPlotId,
        geometry: JSON.stringify(geometry),
        source,
      });
      notify(t("message.boundary_saved", { name: selectedPlot?.name ?? "" }));
      refreshToken += 1;
    });
  }

  function onDrawn(geometry) {
    if (!selectedPlotId) return;
    saveBoundary(geometry, "manual");
  }

  function deleteBoundary(featureProps) {
    run(async () => {
      const label = tCode("map.source", featureProps.source);
      if (!(await confirmDialog(t("map.delete_boundary_confirm", { source: label })))) return;
      await invoke("delete_geo_feature", { id: featureProps.featureId });
      notify(t("message.boundary_deleted"));
      refreshToken += 1;
    });
  }

  function pickImportFile() {
    run(async () => {
      const selection = await invoke("plugin:dialog|open", {
        options: {
          multiple: false,
          directory: false,
          filters: [{ name: "GeoJSON / GeoPackage", extensions: ["geojson", "json", "gpkg"] }],
        },
      });
      const path = Array.isArray(selection) ? selection[0] : selection;
      if (!path) return;
      importEntries = await invoke("list_boundary_file", { path });
      importPath = path;
      importFilter = "";
    });
  }

  function importEntry(entry) {
    run(async () => {
      const geometry = await invoke("read_boundary_feature", {
        path: importPath,
        entryId: entry.id,
      });
      await invoke("save_plot_boundary", {
        plotId: selectedPlotId,
        geometry,
        source: "import",
      });
      notify(t("message.boundary_saved", { name: selectedPlot?.name ?? "" }));
      cancelImport();
      refreshToken += 1;
    });
  }

  function cancelImport() {
    importPath = null;
    importEntries = [];
    importFilter = "";
  }

  // --- SIGPAC (module-sigpac; ES farms only) ------------------------------------

  function onPick({ lon, lat }) {
    run(async () => {
      sigpacResult = (await invoke("sigpac_lookup_point", { lon, lat })) ?? { notFound: true };
    });
  }

  function sigpacRefPath(reference) {
    return [
      reference.province,
      reference.municipality,
      reference.aggregate,
      reference.zone,
      reference.polygon,
      reference.parcel,
      reference.enclosure,
    ].join(":");
  }

  function esFieldsFromParts(parts) {
    const [province, municipality, aggregate, zone, polygon, parcel, enclosure] = parts.map(String);
    return {
      sigpac_province: province,
      sigpac_municipality: municipality,
      sigpac_aggregate: aggregate,
      sigpac_zone: zone,
      sigpac_polygon: polygon,
      sigpac_parcel: parcel,
      sigpac_enclosure: enclosure,
    };
  }

  async function afterPlotCreated(plotId, name) {
    notify(t("message.plot_saved", { name }));
    await reloadPlots();
    refreshToken += 1;
    selectedPlotId = plotId;
    syncHash();
  }

  /// Door B create: official area lands as the suggested declared area — the
  /// user accepted it by choosing "create from this recinto"; editable later.
  function createFromRecinto() {
    const { reference, properties } = sigpacResult.recinto;
    run(async () => {
      const name = `SIGPAC ${reference.polygon}-${reference.parcel}-${reference.enclosure}`;
      const plot = await invoke("create_plot", {
        plot: {
          farm_id: farmId,
          name,
          area_ha: properties.superficie ?? null,
          es: esFieldsFromParts([
            reference.province,
            reference.municipality,
            reference.aggregate,
            reference.zone,
            reference.polygon,
            reference.parcel,
            reference.enclosure,
          ]),
        },
      });
      const verified = await invoke("sigpac_verify_plot", { plotId: plot.id, refresh: false });
      if (verified?.zone_check_error) notify(t("plot.zones_unchecked"), true);
      sigpacResult = null;
      await afterPlotCreated(plot.id, name);
    });
  }

  function attachRecinto(match) {
    run(async () => {
      const verified = await invoke("sigpac_verify_plot", {
        plotId: match.plot_id,
        refresh: false,
      });
      notify(t("message.sigpac_boundary_saved", { name: match.plot_name }));
      if (verified?.zone_check_error) notify(t("plot.zones_unchecked"), true);
      sigpacResult = null;
      refreshToken += 1;
      selectedPlotId = match.plot_id;
      syncHash();
    });
  }

  // Door C: an imported SIGPAC file (GPKG/GeoJSON) carries the reference as
  // attribute columns — enough to create the plot, not just a boundary.
  const SIGPAC_PROP_KEYS = [
    "provincia",
    "municipio",
    "agregado",
    "zona",
    "poligono",
    "parcela",
    "recinto",
  ];

  function sigpacPartsFromProps(props) {
    if (!props) return null;
    const values = SIGPAC_PROP_KEYS.map((key) => props[key]);
    return values.every((v) => v !== undefined && v !== null && v !== "")
      ? values.map(String)
      : null;
  }

  function importEntryAsPlot(entry) {
    const parts = sigpacPartsFromProps(entry.properties);
    run(async () => {
      const geometry = await invoke("read_boundary_feature", {
        path: importPath,
        entryId: entry.id,
      });
      // SIGPAC GPKGs carry dn_surface in m²; suggest it as hectares.
      const surface = Number(entry.properties?.dn_surface);
      const areaHa = Number.isFinite(surface) ? Math.round(surface / 100) / 100 : null;
      const name = `SIGPAC ${parts[4]}-${parts[5]}-${parts[6]}`;
      const plot = await invoke("create_plot", {
        plot: { farm_id: farmId, name, area_ha: areaHa, es: esFieldsFromParts(parts) },
      });
      await invoke("save_plot_boundary", { plotId: plot.id, geometry, source: "import" });
      cancelImport();
      await afterPlotCreated(plot.id, name);
    });
  }

  // Everything except the name-ish keys, compacted for the picker row.
  function entrySummary(entry) {
    const props = entry.properties ?? {};
    const parts = Object.entries(props)
      .filter(([, v]) => v !== null && v !== "")
      .slice(0, 6)
      .map(([k, v]) => `${k}: ${v}`);
    return parts.join(" · ");
  }
</script>

<section class="view map-view">
  <div class="map-toolbar">
    <label class="map-farm-pick">
      <span>{t("map.farm")}</span>
      <select value={farmId} onchange={switchFarm}>
        {#each farms as farm (farm.id)}
          <option value={farm.id}>{farm.name}</option>
        {/each}
      </select>
    </label>
    <div class="map-layer-toggles" role="group" aria-label={t("map.layers")}>
      {#each MAP_LAYERS as layer (layer.id)}
        <label>
          <input
            type="checkbox"
            checked={visibleLayers.includes(layer.id)}
            onchange={() => toggleLayer(layer.id)}
          />
          {t(layer.labelKey)}
        </label>
      {/each}
    </div>
  </div>

  {#if visibleLegends.length > 0}
    <div class="map-legend">
      {#each visibleLegends as layer (layer.id)}
        {#each layer.legend as item (item.labelKey)}
          <span class="legend-item">
            <span class="legend-swatch" style="background: {palette[item.colorKey]}"></span>
            {t(item.labelKey)}
          </span>
        {/each}
      {/each}
    </div>
  {/if}

  {#if belowZoomLayers.length > 0}
    <p class="map-zoom-hint detail">
      {t("map.zoom_hint", { layers: belowZoomLayers.join(" · ") })}
    </p>
  {/if}

  {#if farms.length === 0}
    <p>{t("map.no_farms")}</p>
  {:else}
    <div class="map-workspace">
      <div class="map-area">
        <MapCanvas
          {farmId}
          centerHint={selectedFarm}
          {visibleLayers}
          {refreshToken}
          bind:selectedPlotId
          bind:drawing
          bind:picking
          {onDrawn}
          {onPick}
          {onInspect}
          onZoom={(z) => (mapZoom = z)}
          onData={onLayerData}
        />
      </div>

      <aside class="map-side">
        {#if inspectGroups}
          <div class="map-inspect">
            <div class="map-inspect-head">
              <h4>{t("map.inspect.title")}</h4>
              <button type="button" class="btn-cancel" onclick={() => (inspectGroups = null)}>
                ✕
              </button>
            </div>
            {#each inspectGroups as group (group.layerId)}
              <h5>{t(group.labelKey)}</h5>
              {#each group.items as rows, i (i)}
                <ul class="map-inspect-rows">
                  {#each rows as row, j (j)}
                    <li>
                      {#if row.labelKey}<span class="detail">{t(row.labelKey)}:</span>{/if}
                      {row.valueKey ? t(row.valueKey) : row.value}
                    </li>
                  {/each}
                </ul>
              {/each}
            {/each}
          </div>
        {/if}

        <h3>{t("map.plots")}</h3>
        {#if plots.length === 0}
          <p class="detail">{t("plots.empty")}</p>
        {/if}
        <ul class="map-plot-list">
          {#each plots as { plot } (plot.id)}
            <li>
              <button
                type="button"
                class:active={plot.id === selectedPlotId}
                onclick={() => selectPlot(plot.id)}
              >
                {plot.name}
                {#if (plotFeatures.plots?.features ?? []).some((f) => f.properties.plotId === plot.id)}
                  <span class="has-boundary" title={t("map.has_boundary")}>▰</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>

        {#if selectedFarm?.country_code === "es"}
          <div class="map-sigpac">
            <h4>SIGPAC</h4>
            {#if picking}
              <p class="detail">{t("map.sigpac_pick_hint")}</p>
              <button type="button" class="btn-cancel" onclick={() => (picking = false)}>
                {t("form.cancel")}
              </button>
            {:else}
              <button
                type="button"
                onclick={() => {
                  picking = true;
                  sigpacResult = null;
                }}
              >
                {t("map.sigpac_pick")}
              </button>
            {/if}
            {#if sigpacResult?.notFound}
              <p class="detail">{t("map.sigpac_none")}</p>
            {:else if sigpacResult}
              <p class="detail">
                {sigpacRefPath(sigpacResult.recinto.reference)}
                · {sigpacResult.recinto.properties.superficie} ha · {sigpacResult.recinto.properties
                  .uso_sigpac}
              </p>
              {#each sigpacResult.matching_plots as match (match.plot_id)}
                <button type="button" onclick={() => attachRecinto(match)}>
                  {t("map.sigpac_attach", { plot: match.plot_name })}
                </button>
              {/each}
              {#if sigpacResult.matching_plots.length === 0}
                <button type="button" onclick={createFromRecinto}>
                  {t("map.sigpac_create")}
                </button>
              {/if}
            {/if}
          </div>
        {/if}

        {#if selectedPlot}
          <div class="map-plot-actions">
            <h4>{selectedPlot.name}</h4>
            {#if drawing}
              <p class="detail">{t("map.drawing_hint")}</p>
              <button type="button" class="btn-cancel" onclick={() => (drawing = false)}>
                {t("map.draw_cancel")}
              </button>
            {:else}
              <button type="button" onclick={() => (drawing = true)}>{t("map.draw")}</button>
              <button type="button" onclick={pickImportFile}>{t("map.import")}</button>
            {/if}

            {#if selectedBoundaries.length > 0}
              <h5>{t("map.boundaries")}</h5>
              <ul class="map-boundary-list">
                {#each selectedBoundaries as props (props.featureId)}
                  <li>
                    <span>{tCode("map.source", props.source)}</span>
                    <button type="button" class="btn-danger" onclick={() => deleteBoundary(props)}>
                      {t("map.delete_boundary")}
                    </button>
                  </li>
                {/each}
              </ul>
            {:else if !drawing}
              <p class="detail">{t("map.no_boundary")}</p>
            {/if}
          </div>
        {:else}
          <p class="detail">{t("map.select_plot_hint")}</p>
          <!-- Import without a selected plot: SIGPAC files can CREATE plots. -->
          <button type="button" onclick={pickImportFile}>{t("map.import")}</button>
        {/if}
      </aside>
    </div>

    {#if importPath}
      <div class="map-import-picker">
        <div class="view-head">
          <h3>{t("map.import_pick", { count: importEntries.length })}</h3>
          <button type="button" class="btn-cancel" onclick={cancelImport}>
            {t("form.cancel")}
          </button>
        </div>
        {#if importEntries.length > 8}
          <input
            class="map-import-filter"
            placeholder={t("map.import_filter")}
            bind:value={importFilter}
          />
        {/if}
        <ul class="card-list">
          {#each filteredImportEntries.slice(0, 200) as entry (entry.id)}
            <li class="card">
              <strong>{entry.name ?? entry.id}</strong>
              <span class="detail">{entrySummary(entry)}</span>
              {#if selectedPlot}
                <button type="button" onclick={() => importEntry(entry)}>
                  {t("map.import_use")}
                </button>
              {/if}
              {#if selectedFarm?.country_code === "es" && sigpacPartsFromProps(entry.properties)}
                <button type="button" onclick={() => importEntryAsPlot(entry)}>
                  {t("map.sigpac_create")}
                </button>
              {/if}
            </li>
          {/each}
        </ul>
        {#if filteredImportEntries.length > 200}
          <p class="detail">
            {t("map.import_more", { count: filteredImportEntries.length - 200 })}
          </p>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .map-view {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }
  .map-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
  }
  .map-farm-pick {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .map-layer-toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem 0.9rem;
  }
  .map-layer-toggles label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .map-legend {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem 1rem;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .legend-swatch {
    width: 0.85rem;
    height: 0.85rem;
    border-radius: 3px;
    opacity: 0.7;
    border: 1px solid var(--border);
  }
  .map-workspace {
    display: flex;
    gap: 0.9rem;
    /* Fill the viewport under the toolbar/head; the tabbar media query
       below reserves space on narrow screens. */
    height: calc(100dvh - 12rem);
    min-height: 22rem;
  }
  .map-area {
    flex: 1;
    min-width: 0;
  }
  .map-side {
    width: 15rem;
    flex: none;
    overflow-y: auto;
    padding-right: 0.2rem;
  }
  .map-side h3 {
    margin-top: 0;
  }
  .map-plot-list {
    list-style: none;
    margin: 0 0 1rem;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .map-plot-list button {
    width: 100%;
    text-align: left;
    background: var(--panel);
    color: inherit;
  }
  .map-plot-list button.active {
    outline: 2px solid var(--primary);
  }
  .has-boundary {
    color: var(--primary);
    float: right;
  }
  .map-zoom-hint {
    margin: 0;
    font-size: 0.85rem;
  }
  .map-inspect {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    padding: 0.6rem 0.7rem;
    margin-bottom: 0.9rem;
    font-size: 0.9rem;
  }
  .map-inspect-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .map-inspect-head h4,
  .map-inspect h5 {
    margin: 0.3rem 0;
  }
  .map-inspect-head button {
    padding: 0 0.4rem;
  }
  .map-inspect-rows {
    list-style: none;
    margin: 0 0 0.4rem;
    padding: 0;
  }
  .map-inspect-rows + .map-inspect-rows {
    border-top: 1px dashed var(--border);
    padding-top: 0.4rem;
  }
  .map-plot-actions,
  .map-sigpac {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    border-top: 1px solid var(--border);
    padding-top: 0.7rem;
    margin-bottom: 0.7rem;
  }
  .map-sigpac h4 {
    margin: 0;
  }
  .map-boundary-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .map-boundary-list li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .map-import-filter {
    width: 100%;
    margin-bottom: 0.6rem;
  }
  @media (max-width: 700px) {
    .map-workspace {
      flex-direction: column;
      height: auto;
    }
    .map-area {
      height: 55dvh;
      min-height: 18rem;
    }
    .map-side {
      width: auto;
    }
  }
</style>
