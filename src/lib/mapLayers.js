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
//              one of load/vector per entry.
//   styles(palette) — MapLibre layer specs for the layer's source; `palette`
//              is the resolved CSS palette (MapLibre cannot read CSS vars).
//              Vector-tile specs must name their "source-layer".
//   selectable — whether clicking a feature selects its plot.

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
  // SIGPAC recinto boundaries — the official parcel fabric under the user's
  // own plots. Vector tiles served cache-first by the Rust geo:// protocol
  // (source id sigpac-recintos in terrazgo-geo's registry); the service
  // publishes pbf at z12–15, single source-layer "recinto".
  {
    id: "sigpac-recintos",
    labelKey: "map.layer.sigpac_recintos",
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
  },
];

// The resolved CSS palette for map styling, read once per mount.
export function mapPalette() {
  const styles = getComputedStyle(document.documentElement);
  return {
    primary: styles.getPropertyValue("--primary").trim() || "#007830",
    accent: styles.getPropertyValue("--accent").trim() || "#0078c0",
    warning: styles.getPropertyValue("--warning").trim() || "#c09030",
  };
}

// Center of peninsular Spain — the fallback view when a farm has neither
// coordinates nor stored geometry.
export const SPAIN_CENTER = [-3.7, 40.2];
export const SPAIN_ZOOM = 5.5;
