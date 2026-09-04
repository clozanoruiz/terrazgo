<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The embeddable map canvas — cross-module infrastructure, no business or
  // routing knowledge. Renders base maps served by the Rust geo:// protocol
  // (never a direct network request: production CSP stays 'self' + geo:) and
  // the overlay layers registered in mapLayers.js. Drawing (terra-draw) is
  // pure frontend; the backend is only hit when the parent saves the result.
  // MapLibre (and terra-draw, only when drawing actually starts) are loaded
  // with dynamic import() so the heavy map chunk never weighs down the
  // form views — the webview fetches it the first time a map mounts.
  import { t } from "../i18n.js";
  import { errorText, geoBase, invoke } from "./backend.js";
  import { MAP_LAYERS, SPAIN_CENTER, SPAIN_ZOOM, mapPalette } from "./mapLayers.js";
  import { notify } from "./notifications.svelte.js";

  let {
    // Context handed to layer loaders ({ farmId }); center falls back to the
    // farm's coordinates when there is no geometry to fit.
    farmId = null,
    centerHint = null, // { latitude, longitude } | null
    // Visible layer ids (subset of MAP_LAYERS); parent's layer panel drives it.
    visibleLayers = ["plots"],
    // Selection is owned by the parent; clicks on selectable features update it.
    selectedPlotId = $bindable(null),
    // Drawing mode: parent flips it on; it flips back off when a polygon is
    // finished and onDrawn(geometry) has been delivered.
    drawing = $bindable(false),
    onDrawn = null,
    // Point-picking mode (e.g. the SIGPAC "what parcel is here" lookup): the
    // next map click reports its coordinates and the mode flips back off.
    picking = $bindable(false),
    onPick = null,
    // Point inspect: every plain click reports what the visible overlays
    // render there — [{ layerId, labelKey, items: [rows…] }] (see the
    // inspect() contract in mapLayers.js); [] when the point is empty.
    onInspect = null,
    // Reports the map zoom after every camera move (the layer panel warns
    // when a visible tile overlay is below its minZoom).
    onZoom = null,
    // Parent observes loaded layer data (e.g. the plot panel lists boundary
    // rows without a second backend call).
    onData = null,
    // Bump to force a data reload (after a save/delete).
    refreshToken = 0,
    // Externally supplied point to ease the camera to (the GPS locate flow):
    // { longitude, latitude, zoom? }. Each new object triggers the move.
    focusPoint = null,
    // Live GPS fix to render as a position marker (dot + accuracy circle):
    // { longitude, latitude, accuracy } | null. Null removes the marker.
    gpsPosition = null,
  } = $props();

  const palette = mapPalette();

  let container;
  let map = null;
  let draw = null;
  let styleReady = $state(false);
  let baseStyle = $state("openfreemap"); // 'openfreemap' | 'pnoa'
  // The last loaded FeatureCollection per layer id, re-applied after every
  // style switch (setStyle wipes sources) and observed by the parent.
  const layerData = {};
  let lastFitKey = null;

  // A style the map can always fall back to with a cold cache and no network:
  // plain background, stored geometry still renders — the app keeps working.
  const OFFLINE_STYLE = {
    version: 8,
    name: "offline",
    sources: {},
    layers: [{ id: "bg", type: "background", paint: { "background-color": "#dfe8dc" } }],
  };

  async function fetchStyle(styleId) {
    try {
      return JSON.parse(await invoke("get_map_style", { styleId, base: geoBase() }));
    } catch (err) {
      // Offline with nothing cached (or upstream broke): tell the user, keep
      // the map usable over the plain background. The underlying error rides
      // along — "offline" is only one of the reasons this can fail, and a
      // swallowed cause is undiagnosable from a user report.
      notify(`${t("map.basemap_unavailable")} [${errorText(err)}]`, true);
      return OFFLINE_STYLE;
    }
  }

  $effect(() => {
    let cancelled = false;
    (async () => {
      const [{ default: maplibregl }, style] = await Promise.all([
        import("maplibre-gl"),
        fetchStyle("openfreemap"),
        import("maplibre-gl/dist/maplibre-gl.css"),
      ]);
      if (cancelled) return;
      map = new maplibregl.Map({
        container,
        style,
        center: SPAIN_CENTER,
        zoom: SPAIN_ZOOM,
      });
      map.on("style.load", () => {
        styleReady = true;
        applyOverlays();
        // Layer data may have loaded before the map existed (both are async);
        // fitToContext is lastFitKey-guarded, so this only fits once per farm.
        fitToContext(farmId);
        onZoom?.(map.getZoom());
      });
      map.on("moveend", () => onZoom?.(map.getZoom()));
      wireInteractions();
    })().catch((err) => {
      if (cancelled) return;
      // MapLibre arrives as a dynamic chunk, so this can reject before there is
      // any map to report through. fetchStyle already handles its own failure
      // and falls back to OFFLINE_STYLE, so what reaches here is the chunk
      // itself — and with no map the canvas just stays empty. Saying so beats
      // a blank rectangle the user cannot tell from a slow one.
      notify(`${t("map.engine_unavailable")} [${errorText(err)}]`, true);
    });
    return () => {
      cancelled = true;
      stopDrawing();
      map?.remove();
      map = null;
    };
  });

  async function switchBase(styleId) {
    if (!map || styleId === baseStyle) return;
    baseStyle = styleId;
    styleReady = false;
    map.setStyle(await fetchStyle(styleId)); // style.load re-applies overlays
  }

  // --- overlay layers ---------------------------------------------------------

  function sourceId(layer, key = "") {
    return `layer-${layer.id}${key ? `-${key}` : ""}`;
  }

  // A vector entry declares one source (vector()) or several keyed ones
  // (vectors(), e.g. the landscape elements' area/line/point services under
  // one toggle); style specs pick theirs with sourceKey.
  function vectorSpecs(layer) {
    if (layer.vectors) return layer.vectors(geoBase());
    if (layer.vector) return { "": layer.vector(geoBase()) };
    return null;
  }

  // sourceKey is our extension, not MapLibre's — strip it before addLayer.
  function addStyleSpec(layer, spec) {
    const { sourceKey = "", ...rest } = spec;
    map.addLayer({ ...rest, source: sourceId(layer, sourceKey) });
  }

  function applyOverlays() {
    if (!map) return;
    for (const layer of MAP_LAYERS) {
      if (!visibleLayers.includes(layer.id)) continue;
      const vectors = vectorSpecs(layer);
      if (vectors) {
        // Vector-tile overlay: the source specs are static (tiles stream from
        // the geo:// protocol on demand), so there is no data to (re)set.
        // setStyle wipes sources and layers together, so the first source's
        // presence implies the whole entry is intact.
        const keys = Object.keys(vectors);
        if (!map.getSource(sourceId(layer, keys[0]))) {
          for (const key of keys) map.addSource(sourceId(layer, key), vectors[key]);
          for (const spec of layer.styles(palette)) addStyleSpec(layer, spec);
        }
        continue;
      }
      const data = layerData[layer.id] ?? { type: "FeatureCollection", features: [] };
      if (!map.getSource(sourceId(layer))) {
        map.addSource(sourceId(layer), { type: "geojson", data });
        for (const spec of layer.styles(palette)) addStyleSpec(layer, spec);
      } else {
        map.getSource(sourceId(layer)).setData(data);
      }
    }
    applySelection();
  }

  // Reload layer data when the context changes or a save/delete bumps the token.
  $effect(() => {
    void refreshToken;
    const currentFarm = farmId;
    let cancelled = false;
    (async () => {
      for (const layer of MAP_LAYERS) {
        if (!layer.load || !visibleLayers.includes(layer.id)) continue;
        try {
          const data = await layer.load(invoke, { farmId: currentFarm });
          if (cancelled) return;
          layerData[layer.id] = data;
          onData?.(layer.id, data);
        } catch (err) {
          // Layer data failures must not take the whole map down — but they
          // must not vanish silently either.
          notify(errorText(err), true);
          layerData[layer.id] = { type: "FeatureCollection", features: [] };
        }
      }
      if (styleReady) applyOverlays();
      fitToContext(currentFarm);
    })();
    return () => {
      cancelled = true;
    };
  });

  // Re-apply overlays when the visible set changes (remove = re-add the rest).
  $effect(() => {
    void visibleLayers;
    if (!map || !styleReady) return;
    for (const layer of MAP_LAYERS) {
      const visible = visibleLayers.includes(layer.id);
      for (const spec of layer.styles(palette)) {
        if (map.getLayer(spec.id)) {
          map.setLayoutProperty(spec.id, "visibility", visible ? "visible" : "none");
        }
      }
    }
    applyOverlays();
  });

  /// Fit the camera once per farm: bounds of its geometry, else its
  /// coordinates, else the Spain fallback the map opened with.
  function fitToContext(currentFarm) {
    if (!map || lastFitKey === currentFarm) return;
    const coords = [];
    for (const data of Object.values(layerData)) {
      for (const feature of data.features ?? []) {
        collectPositions(feature.geometry, coords);
      }
    }
    if (coords.length > 0) {
      const lons = coords.map((c) => c[0]);
      const lats = coords.map((c) => c[1]);
      map.fitBounds(
        [
          [Math.min(...lons), Math.min(...lats)],
          [Math.max(...lons), Math.max(...lats)],
        ],
        { padding: 60, maxZoom: 16, duration: 300 },
      );
    } else if (centerHint?.latitude != null && centerHint?.longitude != null) {
      map.jumpTo({ center: [centerHint.longitude, centerHint.latitude], zoom: 13 });
    }
    lastFitKey = currentFarm;
  }

  function collectPositions(geometry, into) {
    if (!geometry) return;
    const polygons =
      geometry.type === "Polygon"
        ? [geometry.coordinates]
        : geometry.type === "MultiPolygon"
          ? geometry.coordinates
          : [];
    for (const polygon of polygons) {
      for (const ring of polygon) {
        for (const position of ring) into.push(position);
      }
    }
  }

  // --- selection ----------------------------------------------------------------

  function applySelection() {
    if (!map?.getLayer("plots-selected")) return;
    map.setFilter("plots-selected", ["==", ["get", "plotId"], selectedPlotId ?? ""]);
  }

  $effect(() => {
    void selectedPlotId;
    if (styleReady) applySelection();
  });

  function wireInteractions() {
    map.on("click", (event) => {
      if (drawing) return;
      if (picking) {
        picking = false;
        onPick?.({ lon: event.lngLat.lng, lat: event.lngLat.lat });
        return;
      }
      reportInspect(event.point);
    });
    for (const layer of MAP_LAYERS) {
      if (!layer.selectable) continue;
      const fillId = `${layer.id}-fill`;
      map.on("click", fillId, (event) => {
        if (drawing || picking) return; // those clicks belong to their mode
        const plotId = event.features?.[0]?.properties?.plotId;
        if (plotId) selectedPlotId = plotId;
      });
      map.on("mouseenter", fillId, () => {
        if (!drawing && !picking) map.getCanvas().style.cursor = "pointer";
      });
      map.on("mouseleave", fillId, () => {
        if (!picking) map.getCanvas().style.cursor = "";
      });
    }
  }

  // Crosshair while a pick is pending, whatever the pointer is over.
  $effect(() => {
    if (map && styleReady) map.getCanvas().style.cursor = picking ? "crosshair" : "";
  });

  // Fly to a parent-supplied point; never zoom OUT from where the user is.
  // Off-screen targets TELEPORT: an ease sweeps the camera across everything
  // in between at high zoom, and each intermediate frame requests tiles the
  // geo:// pipeline cannot cancel once dispatched — on a slow connection the
  // destination tiles then wait behind dozens of junk fetches (found on the
  // first outdoor GPS test: farm → user's position left the map empty for a
  // long time). Easing stays for on-screen nudges, where it costs nothing.
  $effect(() => {
    if (!focusPoint || !map) return;
    const center = [focusPoint.longitude, focusPoint.latitude];
    const zoom = Math.max(map.getZoom(), focusPoint.zoom ?? 16);
    const zoomDelta = zoom - map.getZoom();
    if (map.getBounds().contains(center) && zoomDelta < 1.5) {
      map.easeTo({ center, zoom, duration: 600 });
    } else {
      map.jumpTo({ center, zoom });
    }
  });

  // --- GPS position marker ------------------------------------------------------

  /// The accuracy radius as a ground-true polygon (a MapLibre circle layer's
  /// radius is in screen pixels, not meters — it would not scale with zoom).
  function accuracyCircle({ longitude, latitude, accuracy }) {
    const dLat = accuracy / 111320;
    const dLon = accuracy / (111320 * Math.cos((latitude * Math.PI) / 180));
    const ring = [];
    for (let i = 0; i <= 64; i++) {
      const a = (2 * Math.PI * i) / 64;
      ring.push([longitude + dLon * Math.sin(a), latitude + dLat * Math.cos(a)]);
    }
    return { type: "Polygon", coordinates: [ring] };
  }

  // Upsert the marker on every fix; depends on styleReady so a base-style
  // switch (setStyle wipes sources) re-adds it. Added after the overlays, so
  // the dot always draws on top.
  $effect(() => {
    const pos = gpsPosition;
    if (!map || !styleReady) return;
    const source = map.getSource("gps-position");
    if (!pos) {
      if (source) {
        map.removeLayer("gps-accuracy");
        map.removeLayer("gps-dot");
        map.removeSource("gps-position");
      }
      return;
    }
    const data = {
      type: "FeatureCollection",
      features: [
        { type: "Feature", geometry: accuracyCircle(pos), properties: { kind: "accuracy" } },
        {
          type: "Feature",
          geometry: { type: "Point", coordinates: [pos.longitude, pos.latitude] },
          properties: { kind: "dot" },
        },
      ],
    };
    if (source) {
      source.setData(data);
    } else {
      map.addSource("gps-position", { type: "geojson", data });
      map.addLayer({
        id: "gps-accuracy",
        type: "fill",
        source: "gps-position",
        filter: ["==", ["get", "kind"], "accuracy"],
        paint: { "fill-color": palette.accent, "fill-opacity": 0.15 },
      });
      map.addLayer({
        id: "gps-dot",
        type: "circle",
        source: "gps-position",
        filter: ["==", ["get", "kind"], "dot"],
        paint: {
          "circle-radius": 7,
          "circle-color": palette.accent,
          "circle-stroke-color": "#ffffff",
          "circle-stroke-width": 2,
        },
      });
    }
  });

  /// What do the visible overlays render at this pixel? One group per layer
  /// that answers, one row set per distinct feature (an MVT polygon comes
  /// back once per style spec — fill and line — and tile borders can split
  /// a feature, so dedupe on the properties).
  function reportInspect(point) {
    if (!onInspect || !styleReady) return;
    const groups = [];
    for (const layer of MAP_LAYERS) {
      if (!layer.inspect || !visibleLayers.includes(layer.id)) continue;
      const ids = layer
        .styles(palette)
        .map((spec) => spec.id)
        .filter((id) => map.getLayer(id));
      if (ids.length === 0) continue;
      // eslint-disable-next-line svelte/prefer-svelte-reactivity -- function-local dedupe, never reactive state
      const seen = new Set();
      const items = [];
      for (const feature of map.queryRenderedFeatures(point, { layers: ids })) {
        const key = JSON.stringify(feature.properties);
        if (seen.has(key)) continue;
        seen.add(key);
        const rows = layer
          .inspect(feature.properties)
          .filter((row) => row.valueKey || (row.value !== null && row.value !== undefined));
        if (rows.length > 0) items.push(rows);
      }
      if (items.length > 0) groups.push({ layerId: layer.id, labelKey: layer.labelKey, items });
    }
    onInspect(groups);
  }

  // --- drawing (terra-draw) ------------------------------------------------------

  $effect(() => {
    if (drawing)
      startDrawing().catch((err) => {
        // terra-draw is a dynamic chunk too. Flipping the mode back off is the
        // load-bearing half: the parent's draw button binds to `drawing`, so
        // leaving it true lights the button over a map that will never accept
        // a vertex.
        drawing = false;
        notify(`${t("map.drawing_unavailable")} [${errorText(err)}]`, true);
      });
    else stopDrawing();
  });

  async function startDrawing() {
    if (!map || draw) return;
    const [{ TerraDraw, TerraDrawPolygonMode }, { TerraDrawMapLibreGLAdapter }] = await Promise.all(
      [import("terra-draw"), import("terra-draw-maplibre-gl-adapter")],
    );
    if (!drawing || draw) return; // cancelled while loading
    draw = new TerraDraw({
      adapter: new TerraDrawMapLibreGLAdapter({ map }),
      modes: [new TerraDrawPolygonMode()],
    });
    draw.start();
    draw.setMode("polygon");
    draw.on("finish", (id) => {
      const feature = draw.getSnapshot().find((f) => f.id === id);
      drawing = false;
      if (feature) onDrawn?.(feature.geometry);
    });
  }

  function stopDrawing() {
    if (!draw) return;
    draw.stop();
    draw = null;
  }
</script>

<div class="map-canvas">
  <div class="map-target" bind:this={container}></div>
  <div class="map-base-switch" role="group" aria-label={t("map.base_label")}>
    <button
      type="button"
      class:active={baseStyle === "openfreemap"}
      onclick={() => switchBase("openfreemap")}>{t("map.base_streets")}</button
    >
    <button type="button" class:active={baseStyle === "pnoa"} onclick={() => switchBase("pnoa")}
      >{t("map.base_ortho")}</button
    >
  </div>
</div>

<style>
  .map-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 16rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: #dfe8dc;
  }
  .map-target {
    position: absolute;
    inset: 0;
  }
  .map-base-switch {
    position: absolute;
    top: 0.6rem;
    left: 0.6rem;
    display: flex;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg);
  }
  .map-base-switch button {
    border: none;
    border-radius: 0;
    padding: 0.3rem 0.7rem;
    font-size: 0.85rem;
    background: var(--bg);
  }
  .map-base-switch button.active {
    background: var(--primary);
    color: var(--on-primary);
  }
</style>
