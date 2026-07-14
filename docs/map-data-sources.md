# Map data sources & overlays — the inventory

> **Upkeep rule:** update this file in the same change that adds or alters a
> tile/resource source (`crates/terrazgo-geo/src/sources.rs`), a map overlay
> (`src/lib/mapLayers.js`), or an external map-data seam. A source that is
> not listed here does not exist as far as reviews are concerned.

Everything below reaches the webview exclusively through the `geo://`
protocol served by `terrazgo-geo`'s cache-through fetch — the single
sanctioned network seam. Once seen, everything works offline;
campaign-keyed sources evict their previous campaign's tiles at rollover.

## Base maps

| Source id | Provider / service | What it shows | Purpose in the app |
| --- | --- | --- | --- |
| `openfreemap` (+ `openfreemap-ne2` backdrop, `ofm-style`/`ofm-fonts`/`ofm-sprites` resources) | [OpenFreeMap](https://openfreemap.org) vector tiles (OSM data), liberty style rewritten in Rust | General-purpose street/terrain map | Default base layer: orientation, roads, villages, names |
| `pnoa` | IGN [PNOA](https://www.ign.es) orthophoto, WMTS GoogleMapsCompatible | Aerial imagery of Spain | "Ortho" base layer: what the land actually looks like — drawing boundaries against real field edges |

## Overlays (`mapLayers.js`)

| Overlay id | Data source | What it shows | Purpose in the app |
| --- | --- | --- | --- |
| `plots` | Own DB: `geo_feature` via `list_geo_features` | The user's stored plot boundaries (drawn / imported / SIGPAC), selected plot highlighted | The farm on the map; anchor for every other overlay |
| `phi-status` | Own DB: treatment records via `list_phi_status` (derived on read) | Plots tinted red while a PHI window contains today, green when treated and clear | The record book made visible: "can I harvest / enter this plot today?" — default off |
| `zone-flags` | Own DB: `plot_zone_flag` via `list_zone_flags` (latest campaign's 'inside' per plot and zone kind) | Nitrate-vulnerable / phyto-restriction / Natura 2000 membership as plot tints | Compliance context at a glance (fertilisation duty, treatment restrictions, conditionality) — default off |
| `sigpac-recintos` | FEGA Nube de SIGPAC MVT (`recinto`), CC BY 4.0, campaign-keyed | The official parcel fabric (gold lines) | Check own boundaries against the official registry; find references |
| `sigpac-cultivo-declarado` | FEGA Nube de SIGPAC MVT (`cultivo_declarado`), CC BY 4.0, campaign-keyed — **the fixed path serves the PREVIOUS campaign** | PAC-declared crop lines (dashed gold): crop code, secano/regadío, declared surface | See what was declared around (and on) the farm; display twin of the declared-crops GPKG downloads behind the crop-prefill idea ([siex-export.md](siex-export.md)) — default off |
| `sigpac-paisaje` | FEGA Nube de SIGPAC MVT (`e_paisaje_area`/`_linea`/`_punto`), CC BY 4.0, campaign-keyed | Protected landscape elements (vegetation islands, hedges, ponds…), blue | PAC conditionality: features that must not be removed — sparse data, most farmland has none; default off |

All the MVT overlays exist only at zoom 12–15 (over/underzoom outside);
below z12 nothing draws — the layer panel shows a "zoom in to see" hint
while such a layer is on (`minZoom` on the entry). Empty tiles answer HTTP
404 upstream and are cached as empty. MVT attribute surfaces are **m²**
(the REST lookups speak hectares — verified 2026-07-12 on recinto
10:85:0:0:29:5:27). Attribution `SIGPAC © FEGA (CC BY 4.0)` shows while any
is active.

Overlays whose entry defines `inspect()` feed the map's point-inspect panel
(click anywhere → what every *visible* overlay renders there).

## Non-tile services (same seam, `resource` cache)

| Consumer | Service | Purpose |
| --- | --- | --- |
| `module-sigpac` lookups | Nube de SIGPAC REST: recinto by reference / by point | Plot verification, map-click lookup, import dedup — a response seen once keeps the plot verifiable offline |
| `module-sigpac` zone checks | Nube de SIGPAC zone-intersection queries (nitrate / phyto / Natura) | Writes `plot_zone_flag` (stored truth; the `zone-flags` overlay renders it) |
| Campaign resolution | Provider `/geopackages/` directory listing (`sigpac/campaigns` cache row) | The only machine-readable statement of the current campaign; keys every campaign-keyed cache row |

## User-supplied files (no network)

| Path | Format | Purpose |
| --- | --- | --- |
| Boundary import (`terrazgo_geo::import`) | GeoJSON, GeoPackage (EPSG 4326/4258/4081) | Boundaries the user already has — including SIGPAC municipality downloads; SIGPAC-attributed entries can create whole plots |

## Wanted next (nothing scheduled)

The build order and pre-flight checks live in
[map-layers-roadmap.md](map-layers-roadmap.md): phase 3 = public WMS through
grid-snapped `GetMap` (Catastro, IDEE hydrography, MITECO zones, ITACYL soil
+ CyL NDVI), phase 4 = CDSE NDVI with farmer-supplied credentials.
