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
//   load(invoke, ctx) — returns a GeoJSON FeatureCollection for the current
//              context ({ farmId }). Feature properties carry what styles
//              and interactions need (featureId, plotId, source).
//   styles(palette) — MapLibre layer specs for the layer's source; `palette`
//              is the resolved CSS palette (MapLibre cannot read CSS vars).
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
