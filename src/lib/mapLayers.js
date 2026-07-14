// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Map overlay layers — the single source of truth rendered by MapCanvas and
// listed in MapView's layer panel. The map is cross-module infrastructure:
// a future module adds its overlay (treatments, zone flags, irrigation) by
// adding one entry here, exactly like nav.js entries add screens.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
//
// Entry contract:
//   id       — stable slug; also namespaces the MapLibre source/layer ids.
//   labelKey — i18n key for the layer panel.
//   load(invoke, ctx) — GeoJSON layers only: returns a FeatureCollection for
//              the current context ({ farmId }). Feature properties carry
//              what styles and interactions need (featureId, plotId, source).
//   vector(base) — vector-tile layers only: returns the MapLibre source spec
//              (tiles through the geo:// protocol, zoom bounds, attribution).
//              `base` is the platform protocol origin from geoBase(). Exactly
//              one of load/vector/vectors per entry.
//   vectors(base) — multi-source vector entry: returns { key: sourceSpec }
//              when one toggle covers several tile services (the landscape
//              elements' area/line/point). Style specs pick their source
//              with `sourceKey` (omitted = the single/unnamed source).
//   styles(palette) — MapLibre layer specs for the layer's source; `palette`
//              is the resolved CSS palette (MapLibre cannot read CSS vars).
//              Vector-tile specs must name their "source-layer".
//   selectable — whether clicking a feature selects its plot.
//   defaultVisible — set false for layers that start toggled off (status
//              tints would bury the base view if they all started on).
//   legend   — optional [{ colorKey, labelKey }]: the layer panel shows these
//              swatches while the layer is visible, colorKey resolved through
//              mapPalette(). Layers whose color coding isn't self-evident
//              must carry one.
//   inspect(props) — optional: rows for the map's point-inspect panel, built
//              from one rendered feature's properties. Each row may carry
//              labelKey / valueKey (i18n keys, translated by the view) and
//              value (raw text); rows with a null value and no valueKey are
//              skipped. Only VISIBLE layers are inspected — the panel
//              reflects what the map shows.
//   minZoom  — tile layers only: below this map zoom the service has no
//              tiles and the layer silently draws nothing, so the layer
//              panel shows a "zoom in" hint while the layer is on. Keep in
//              sync with the source spec's minzoom.

export const MAP_LAYERS = [
  {
    id: "plots",
    labelKey: "map.layer.plots",
    selectable: true,
    async load(invoke, { farmId }) {
      if (!farmId) return { type: "FeatureCollection", features: [] };
      const rows = await invoke("list_geo_features", { farmId });
      return {
        type: "FeatureCollection",
        features: rows.map((row) => ({
          type: "Feature",
          properties: {
            featureId: row.id,
            plotId: row.plot_id,
            farmId: row.farm_id,
            source: row.source,
            role: row.role,
          },
          geometry: JSON.parse(row.geometry),
        })),
      };
    },
    styles(palette) {
      return [
        {
          id: "plots-fill",
          type: "fill",
          paint: { "fill-color": palette.primary, "fill-opacity": 0.15 },
        },
        {
          id: "plots-line",
          type: "line",
          paint: { "line-color": palette.primary, "line-width": 2 },
        },
        // The selected plot's boundary pops in the accent color.
        {
          id: "plots-selected",
          type: "line",
          paint: { "line-color": palette.accent, "line-width": 4 },
          filter: ["==", ["get", "plotId"], ""],
        },
      ];
    },
  },
  // Treatment / PHI status — plots tinted by whether a phytosanitary
  // treatment's PHI window (plazo de seguridad) contains today. Derived on
  // read by module-cue; red = harvest restricted, green = treated and clear.
  // Untreated plots carry no tint. One feature per plot: when a plot stores
  // several boundary sources they overlap anyway, and stacked translucent
  // fills would double the tint.
  {
    id: "phi-status",
    labelKey: "map.layer.phi_status",
    selectable: false,
    defaultVisible: false,
    legend: [
      { colorKey: "danger", labelKey: "map.legend.phi_in" },
      { colorKey: "primary", labelKey: "map.legend.phi_clear" },
    ],
    async load(invoke, { farmId }) {
      if (!farmId) return { type: "FeatureCollection", features: [] };
      const [rows, statuses] = await Promise.all([
        invoke("list_geo_features", { farmId }),
        invoke("list_phi_status", { farmId }),
      ]);
      const byPlot = new Map(statuses.map((s) => [s.plot_id, s]));
      return {
        type: "FeatureCollection",
        features: onePerPlot(rows)
          .filter((row) => byPlot.has(row.plot_id))
          .map((row) => ({
            type: "Feature",
            properties: {
              plotId: row.plot_id,
              inPhi: byPlot.get(row.plot_id).in_phi,
              phiUntil: byPlot.get(row.plot_id).phi_until,
            },
            geometry: JSON.parse(row.geometry),
          })),
      };
    },
    styles(palette) {
      return [
        {
          id: "phi-status-in",
          type: "fill",
          filter: ["==", ["get", "inPhi"], true],
          paint: { "fill-color": palette.danger, "fill-opacity": 0.4 },
        },
        {
          id: "phi-status-clear",
          type: "fill",
          filter: ["==", ["get", "inPhi"], false],
          paint: { "fill-color": palette.primary, "fill-opacity": 0.35 },
        },
      ];
    },
    inspect(props) {
      return props.inPhi
        ? [
            { valueKey: "map.legend.phi_in" },
            { labelKey: "map.inspect.phi_until", value: props.phiUntil },
          ]
        : [{ valueKey: "map.legend.phi_clear" }];
    },
  },
  // Zone flags — the stored nitrate / phyto-restriction / Natura 2000 checks
  // (latest campaign's 'inside' per plot and zone kind, the plot cards' chip
  // rule) as plot tints. Overlapping memberships blend: three translucent
  // fills, one per zone kind.
  {
    id: "zone-flags",
    labelKey: "map.layer.zone_flags",
    selectable: false,
    defaultVisible: false,
    legend: [
      { colorKey: "accent", labelKey: "zone.nitrate_vulnerable" },
      { colorKey: "warning", labelKey: "zone.phytosanitary_restriction" },
      { colorKey: "primary", labelKey: "zone.natura_2000" },
    ],
    async load(invoke, { farmId }) {
      if (!farmId) return { type: "FeatureCollection", features: [] };
      const [rows, flags] = await Promise.all([
        invoke("list_geo_features", { farmId }),
        invoke("list_zone_flags", { farmId }),
      ]);
      // Latest campaign wins per (plot, zone kind): flags arrive newest
      // campaign first, so the first row seen decides.
      const seen = new Set();
      const zones = new Map();
      for (const flag of flags) {
        const key = `${flag.plot_id}/${flag.zone_type_code}`;
        if (seen.has(key)) continue;
        seen.add(key);
        if (flag.status === "inside") {
          if (!zones.has(flag.plot_id)) zones.set(flag.plot_id, {});
          zones.get(flag.plot_id)[flag.zone_type_code] = true;
        }
      }
      return {
        type: "FeatureCollection",
        features: onePerPlot(rows)
          .filter((row) => zones.has(row.plot_id))
          .map((row) => ({
            type: "Feature",
            properties: { plotId: row.plot_id, ...zones.get(row.plot_id) },
            geometry: JSON.parse(row.geometry),
          })),
      };
    },
    styles(palette) {
      return [
        {
          id: "zone-flags-nitrate",
          type: "fill",
          filter: ["==", ["get", "nitrate_vulnerable"], true],
          paint: { "fill-color": palette.accent, "fill-opacity": 0.35 },
        },
        {
          id: "zone-flags-phyto",
          type: "fill",
          filter: ["==", ["get", "phytosanitary_restriction"], true],
          paint: { "fill-color": palette.warning, "fill-opacity": 0.35 },
        },
        {
          id: "zone-flags-natura",
          type: "fill",
          filter: ["==", ["get", "natura_2000"], true],
          paint: { "fill-color": palette.primary, "fill-opacity": 0.35 },
        },
      ];
    },
    inspect(props) {
      return ["nitrate_vulnerable", "phytosanitary_restriction", "natura_2000"]
        .filter((code) => props[code])
        .map((code) => ({ valueKey: `zone.${code}` }));
    },
  },
  // Declared-crop lines (líneas de declaración gráfica) — what was declared
  // per surface in the PAC campaign, drawn dashed so it reads apart from the
  // recinto fabric. The service's fixed path serves the PREVIOUS campaign
  // (the running one's declarations are still open) — the label says so.
  {
    id: "sigpac-cultivo-declarado",
    labelKey: "map.layer.cultivo_declarado",
    minZoom: 12,
    selectable: false,
    defaultVisible: false,
    vector(base) {
      return {
        type: "vector",
        tiles: [`${base}tiles/sigpac-cultivo-declarado/{z}/{x}/{y}`],
        minzoom: 12,
        maxzoom: 15,
        attribution: "SIGPAC © FEGA (CC BY 4.0)",
      };
    },
    styles(palette) {
      return [
        {
          id: "sigpac-cultivo-declarado-fill",
          type: "fill",
          "source-layer": "cultivo_declarado",
          paint: { "fill-color": palette.warning, "fill-opacity": 0.12 },
        },
        {
          id: "sigpac-cultivo-declarado-line",
          type: "line",
          "source-layer": "cultivo_declarado",
          paint: {
            "line-color": palette.warning,
            "line-width": 1.5,
            "line-dasharray": [2, 2],
          },
        },
      ];
    },
    // Codes shown raw for now — the Anexo VII / PAC catalogues that name
    // them are the SIEX catalogue-study deliverable.
    inspect(props) {
      return [
        { labelKey: "map.inspect.crop_code", value: props.parc_producto },
        { labelKey: "map.inspect.exploitation_system", value: props.parc_sistexp },
        { labelKey: "map.inspect.declared_surface", value: m2ToHa(props.parc_supcult) },
        { labelKey: "map.inspect.campaign", value: props.exp_ano },
      ];
    },
  },
  // Landscape elements (PAC conditionality protected features: islands of
  // vegetation, hedges, ponds…). Three tile services — area, line, point —
  // behind one toggle; sparse data, most tiles are empty.
  {
    id: "sigpac-paisaje",
    labelKey: "map.layer.paisaje",
    minZoom: 12,
    selectable: false,
    defaultVisible: false,
    vectors(base) {
      const source = (kind) => ({
        type: "vector",
        tiles: [`${base}tiles/sigpac-paisaje-${kind}/{z}/{x}/{y}`],
        minzoom: 12,
        maxzoom: 15,
        attribution: "SIGPAC © FEGA (CC BY 4.0)",
      });
      return { area: source("area"), linea: source("linea"), punto: source("punto") };
    },
    styles(palette) {
      return [
        {
          id: "sigpac-paisaje-area-fill",
          type: "fill",
          sourceKey: "area",
          "source-layer": "e_paisaje_area",
          paint: { "fill-color": palette.accent, "fill-opacity": 0.35 },
        },
        {
          id: "sigpac-paisaje-area-line",
          type: "line",
          sourceKey: "area",
          "source-layer": "e_paisaje_area",
          paint: { "line-color": palette.accent, "line-width": 1.5 },
        },
        {
          id: "sigpac-paisaje-linea",
          type: "line",
          sourceKey: "linea",
          "source-layer": "e_paisaje_linea",
          paint: { "line-color": palette.accent, "line-width": 2.5 },
        },
        {
          id: "sigpac-paisaje-punto",
          type: "circle",
          sourceKey: "punto",
          "source-layer": "e_paisaje_punto",
          paint: {
            "circle-color": palette.accent,
            "circle-radius": 4,
            "circle-opacity": 0.8,
          },
        },
      ];
    },
    inspect(props) {
      return [{ labelKey: "map.inspect.element_type", value: props.tipo_elemento }];
    },
  },
  // SIGPAC recinto boundaries — the official parcel fabric under the user's
  // own plots. Vector tiles served cache-first by the Rust geo:// protocol
  // (source id sigpac-recintos in terrazgo-geo's registry); the service
  // publishes pbf at z12–15, single source-layer "recinto".
  {
    id: "sigpac-recintos",
    labelKey: "map.layer.sigpac_recintos",
    minZoom: 12,
    selectable: false,
    vector(base) {
      return {
        type: "vector",
        tiles: [`${base}tiles/sigpac-recintos/{z}/{x}/{y}`],
        minzoom: 12,
        maxzoom: 15,
        attribution: "SIGPAC © FEGA (CC BY 4.0)",
      };
    },
    styles(palette) {
      return [
        // Invisible fill: recintos draw as lines, but the point-inspect
        // query needs the polygon interior to be hit-testable.
        {
          id: "sigpac-recintos-fill",
          type: "fill",
          "source-layer": "recinto",
          paint: { "fill-color": palette.warning, "fill-opacity": 0 },
        },
        {
          id: "sigpac-recintos-line",
          type: "line",
          "source-layer": "recinto",
          paint: {
            "line-color": palette.warning,
            "line-width": 1,
            "line-opacity": 0.8,
          },
        },
      ];
    },
    inspect(props) {
      const ref = [
        props.provincia,
        props.municipio,
        props.agregado,
        props.zona,
        props.poligono,
        props.parcela,
        props.recinto,
      ].join(":");
      return [
        { labelKey: "map.inspect.sigpac_ref", value: ref },
        { labelKey: "map.inspect.land_use", value: props.uso_sigpac },
        { labelKey: "map.inspect.surface_ha", value: m2ToHa(props.superficie) },
      ];
    },
  },
];

// SIGPAC MVT tiles carry surfaces in m² while the REST services speak ha
// (verified against recinto 10:85:0:0:29:5:27 — MVT superficie 1152241,
// REST 115.2241 ha; a declared line's parc_supcult 70000 = 7 ha).
// Rounding to whole m² BEFORE dividing = rounding the result to 4 ha
// decimals (the REST services' own precision); the raw tile values carry
// float noise (1152241.0265…) that would otherwise render as-is.
function m2ToHa(value) {
  return value == null ? null : Math.round(value) / 10000;
}

// Status overlays tint each plot once — when a plot stores several boundary
// sources (drawn next to SIGPAC) the geometries overlap, and stacked
// translucent fills would read as a darker, different color.
function onePerPlot(rows) {
  const seen = new Set();
  return rows.filter((row) => {
    if (!row.plot_id || seen.has(row.plot_id)) return false;
    seen.add(row.plot_id);
    return true;
  });
}

// The resolved CSS palette for map styling, read once per mount.
export function mapPalette() {
  const styles = getComputedStyle(document.documentElement);
  return {
    primary: styles.getPropertyValue("--primary").trim() || "#007830",
    accent: styles.getPropertyValue("--accent").trim() || "#0078c0",
    warning: styles.getPropertyValue("--warning").trim() || "#c09030",
    danger: styles.getPropertyValue("--danger").trim() || "#c62828",
  };
}

// Center of peninsular Spain — the fallback view when a farm has neither
// coordinates nor stored geometry.
export const SPAIN_CENTER = [-3.7, 40.2];
export const SPAIN_ZOOM = 5.5;
