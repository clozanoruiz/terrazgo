# Map data sources & overlays

> **Keep this file current.** Update it in the same change that adds or alters
> a tile or resource source (`crates/terrazgo-geo/src/sources.rs`), a map
> overlay (`src/lib/mapLayers.js`), or any other way map data reaches the app.
> Endpoints, refresh steps and what to do if a provider disappears are in
> [maintenance.md](maintenance.md). This file says what each source shows and
> why the app uses it.

All map data reaches the webview through the `geo://` protocol, which
`terrazgo-geo` serves from its cache, fetching on a miss. That is the only
network path for map data. Anything seen once keeps working offline. Sources
keyed by campaign drop the previous campaign's tiles when the campaign changes.

## Base maps

| Source id | Provider / service | What it shows | What the app uses it for |
| --- | --- | --- | --- |
| `openfreemap` (+ `openfreemap-ne2` backdrop, `ofm-style`/`ofm-fonts`/`ofm-sprites` resources) | [OpenFreeMap](https://openfreemap.org) vector tiles (OSM data), liberty style rewritten in Rust | General street and terrain map | Default base layer: roads, villages, names |
| `pnoa` | IGN [PNOA](https://www.ign.es) orthophoto, WMTS GoogleMapsCompatible | Aerial imagery of Spain | "Ortho" base layer: drawing boundaries against the real field edges |

## Overlays (`mapLayers.js`)

| Overlay id | Data source | What it shows | What the app uses it for |
| --- | --- | --- | --- |
| `plots` | Own DB: `geo_feature` via `list_geo_features` | The user's plot boundaries (drawn, imported or from SIGPAC), with the selected plot highlighted | The farm on the map; every other overlay is read against it |
| `phi-status` | Own DB: treatment records via `list_phi_status` (worked out when read) | Plots in red while a PHI window covers today, green when treated and clear | "Can I harvest or enter this plot today?" Off by default |
| `zone-flags` | Own DB: `plot_zone_flag` via `list_zone_flags` (latest campaign's 'inside' per plot and zone kind) | Nitrate-vulnerable, phyto-restriction and Natura 2000 zones as plot tints | Which rules apply where (fertilisation duty, treatment restrictions, conditionality). Off by default |
| `sigpac-recintos` | FEGA Nube de SIGPAC MVT (`recinto`), CC BY 4.0, keyed by campaign | The official parcel layout (gold lines) | Compare the user's boundaries with the official registry; find references |
| `sigpac-cultivo-declarado` | FEGA Nube de SIGPAC MVT (`cultivo_declarado`), CC BY 4.0, keyed by campaign. **The fixed path serves the PREVIOUS campaign** | Crops declared to the PAC (dashed gold): crop code, secano/regadío, declared surface | What was declared on and around the farm. The same data also comes as GPKG downloads, which the crop prefill would use ([siex-export.md](siex-export.md)). Off by default |
| `sigpac-paisaje` | FEGA Nube de SIGPAC MVT (`e_paisaje_area`/`_linea`/`_punto`), CC BY 4.0, keyed by campaign | Protected landscape elements (vegetation islands, hedges, ponds…), in blue | PAC conditionality: features that must not be removed. Most farmland has none. Off by default |

The MVT overlays only exist at zoom 12–15; outside that range MapLibre over- or
underzooms, and below z12 nothing is drawn. While such a layer is on, the layer
panel shows a "zoom in to see" hint (`minZoom` on the entry).
`sigpac-recintos` starts at z13 because the service's z12 recinto tiles are
missing where parcels are dense, and a missing tile would draw as empty (see
[architecture.md](architecture.md) → "The map tier"). Empty tiles answer HTTP
404 upstream and are cached as empty. Surfaces in the MVT attributes are in
**m²**; the REST lookups use hectares. The attribution `SIGPAC © FEGA (CC BY
4.0)` shows while any of them is on.

Overlays whose entry defines `inspect()` feed the map's point-inspect panel:
click anywhere to see what every *visible* overlay shows at that point.

## Other services (same path, `resource` cache)

| Used by | Service | What for |
| --- | --- | --- |
| `module-sigpac` lookups | Nube de SIGPAC REST: recinto by reference or by point | Checking plots, map-click lookup, avoiding duplicate imports. A response seen once keeps the plot checkable offline |
| `module-sigpac` zone checks | Nube de SIGPAC zone-intersection queries (nitrate / phyto / Natura) | Writes `plot_zone_flag`, which the `zone-flags` overlay draws |
| Campaign lookup | Provider `/geopackages/` directory listing (`sigpac/campaigns` cache row) | The only machine-readable place that says which campaign is current. Every campaign-keyed cache row uses it |

## Files the user brings (no network)

| Path | Format | What for |
| --- | --- | --- |
| Boundary import (`terrazgo_geo::import`) | GeoJSON, GeoPackage (EPSG 4326/4258/4081) | Boundaries the user already has, including SIGPAC municipality downloads. Entries with SIGPAC attributes can create whole plots |

## Wanted next

The order and the checks to run first are in
[map-layers-roadmap.md](map-layers-roadmap.md): phase 3 is public WMS through
grid-snapped `GetMap` (Catastro, IDEE hydrography, MITECO zones, ITACYL soil
and CyL NDVI), phase 4 is CDSE NDVI with the farmer's own credentials.
