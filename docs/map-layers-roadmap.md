# Map layers: roadmap and caching decisions

This says which map layers are wanted, in what order, and how WMS data is
cached. Phases 1 and 2 are built; phases 3 and 4 are not scheduled. The list of
services and their details are in [agro-data-services.md](agro-data-services.md)
and [sigpac-integration.md](sigpac-integration.md). Writing the order down here
means each new layer is a small task of a known shape rather than a new design.

## The two decisions

### 1. WMS data is cached as grid-snapped tiles ("self-proxied WMTS")

The geo cache and the `geo://` protocol work in XYZ tiles
(`tiles/{source}/{z}/{x}/{y}`), and most of the wanted sources only offer WMS:
Catastro's WMS speaks EPSG:3857 but has no WMTS, IDEE hydrography is the same,
and ITACYL is WMS/WCS. The options were:

- **Only accept WMTS.** Rejected: it would leave out Catastro, hydrography,
  the MITECO zone layers and ITACYL, which is most of the list.
- **Let MapLibre pass the bbox through** (`{bbox-epsg-3857}`, the webview
  works out the bbox and the responses are cached by URL in the `resource`
  table). Rejected: bbox strings with floats make unreliable cache keys, and
  those rows would escape the tile table's size limit.
- **Snap to the grid in Rust.** Chosen. The protocol path stays
  `tiles/{id}/{z}/{x}/{y}`. The fetch layer works out the tile's EPSG:3857
  bounding box from z/x/y (plain Web Mercator arithmetic, no new crate) and puts
  it into a `GetMap` URL template (`width=height=256`, `crs=EPSG:3857`,
  `format=image/png`, `transparent=true` for overlays). The responses are
  stored and served as ordinary XYZ tiles.

Tile proxies like MapProxy or GeoWebCache do the same thing. Here it is a few
lines of arithmetic and there is no proxy to run. Each WMS source is one more
`TileSource` entry, and the cache, its size limit, offline use and the
frontend's raster handling all work as they are.

What follows from it:

- The service rule becomes **MVT, then WMTS, then WMS on a grid**: native tiles
  when the provider has them, grid-snapped `GetMap` only when WMS is all there
  is.
- A WMS source has to support EPSG:3857 (check this before adding it; Catastro
  and IDEE hydrography do). The app doesn't reproject map images, just as it
  doesn't reproject imported boundaries.
- Converting a tile to a bbox follows a public standard (the slippy-map /
  EPSG:3857 tiling scheme), so it is written test-first with the values cited.
- Dated rasters (NDVI composites) put their date in the cache key, the same way
  SIGPAC tiles carry their campaign.

Two more, from the raster review in [stack-choices.md](stack-choices.md) §1:

- **A farmer's own raster** (a drone orthomosaic, a scanned plan) works like
  everything else here: **Rust decodes it and cuts it into tiles once, on
  import, and serves XYZ through `geo://`**. It is not read in the webview.
  That would be slower, and on Android it can't work at all: the file dialog
  returns a `content://` URI the webview can't open, and an orthomosaic can be
  100 MB to 2 GB. It needs a storage design first. A farm's ortho is derived
  data but can't be replaced, so it fits neither the tile cache (which evicts)
  nor the app database (which is backed up) as they are today.
- **Recolouring a raster has to happen in Rust.** maplibre-gl 6.9 has no
  `raster-color` or `raster-array`, so rendered tiles are fixed images; on the
  client only opacity, hue-rotate, brightness, saturation and contrast can be
  changed. An adjustable NDVI colour ramp means caching the values and drawing
  the tiles again locally, which also keeps it working offline.

### 2. Each farmer brings their own CDSE account

For the Copernicus Data Space Ecosystem APIs (NDVI overlays outside CyL, and
the per-plot series from the Statistical API), each user signs up for their own
free CDSE account and enters it in settings. This settles
agro-data-services.md's open question 1 for CDSE. The reasons: the Sentinel
data licence is free and open, commercial use included, with attribution, so it
fits an AGPL app; but the API works per account with a quota per account, an
AGPL binary can't hold a shared secret, and CDSE's terms count spreading use
over several accounts to get round the quota as a breach. With their own
account each user is a legitimate quota holder, and the free tier is plenty for
one farm. A server-side proxy could be added later to make starting easier, but
never instead of this.

Before this can be built, the credentials need a safe place on the device. The
settings file is plain text and is not that place (architecture.md →
"Device-local settings"), so this waits for a secrets store.

## The wanted layers, in order

The order follows what infrastructure is ready: each phase reuses what the
previous one built. Within a phase the order doesn't matter.

### Phase 1: the farm's own data as overlays (built)

| Layer | Source | Notes |
| --- | --- | --- |
| Treatment / PHI status | `list_phi_status` → module-phytosanitary `phi_status_for_farm` | Plots tinted red while in a PHI window, green when harvest is allowed. Worked out when read, with the same window rule as the alerts |
| Zone-flag tint | `list_zone_flags` (core `plot_zone_flag`) | Latest campaign's 'inside' per (plot, zone kind), the same rule as the chips. One translucent fill per zone kind; where they overlap they blend |

Each is one GeoJSON entry in `mapLayers.js`. The layer panel needed two small
additions: `defaultVisible: false` (status tints start off) and a `legend` per
layer, shown while it is visible. Grouping layers can wait until a flat list
gets too long.

### Phase 2: the rest of the Nube de SIGPAC MVT service (built)

| Layer | Service layer | Notes |
| --- | --- | --- |
| Declared crops | `cultivo_declarado` | Dashed gold fill and line, keyed by campaign like the recintos. **The fixed path serves the PREVIOUS campaign** (the current one's declarations are still open, per the service docs), and the layer label says so. The same data comes as provincial GPKG downloads (current and previous campaign, CC BY 4.0), which is what the crop prefill would read ([siex-export.md](siex-export.md) → "Farmer-side data paths"). The overlay only displays it |
| Landscape elements | `e_paisaje_area`, `_linea`, `_punto` | PAC conditionality (protected features). Three tile services behind ONE toggle: `mapLayers.js` entries can have `vectors()` (several sources in one entry; each style spec picks its source with `sourceKey`). Data is sparse and most tiles are empty (404) |

Facts about the live service: the source-layer names are the same as the path
names; the attribute keys match the download service's models (declared crops:
`parc_producto`, `parc_sistexp`, `parc_supcult`, `exp_ano`…; landscape:
`tipo_elemento`); tiles are pbf at z12–15; empty tiles answer 404 and are cached
as empty, as for recintos.

### Phase 3: public WMS overlays through grid snapping (needs decision 1 built once)

| Layer | Provider / service | Check before adding |
| --- | --- | --- |
| Cadastral parcels | Catastro WMS (supports 3857) | Layer names, scale limits, attribution wording. Goes with a future SIGPAC↔catastro crosswalk |
| Hydrography | IDEE `wms-inspire/hidrografia` (supports 3857) | Which sublayers matter (watercourses, water points). Rule it supports: phyto buffer strips near water |
| Nitrate-vulnerable zones | MITECO WMS | **Endpoint not found yet**; licence and 3857 to check |
| Natura 2000 | MITECO WMS | Same. Display only: what the app relies on is still `plot_zone_flag`, this just draws the boundaries |
| Soil maps (CyL) | ITACYL Atlas / IDECyL WMS | Which layers, licence per layer, 3857. Regional first, as the inventory leans |
| NDVI mosaic (CyL) | ITACYL Sentinel-2 series | Endpoint still to find; cache keyed by date |

### Phase 4: CDSE (needs decision 2 and a secrets store)

| What | API | Notes |
| --- | --- | --- |
| NDVI overlay (national) | Sentinel Hub OGC (WMS/WMTS, evalscript) | Uses the same grid snapping; a per-user OAuth token through `terrazgo-geo` |
| NDVI series per plot | Statistical API / openEO | Not a map layer. Synced user data, like zone flags, and it needs its own schema (the first time series per plot) |

Attribution "Contains modified Copernicus Sentinel data [year]" while it is on.

## Rules for every layer

- All fetching goes through `terrazgo-geo`'s cache; the webview only ever sees
  `geo://`.
- Attribution shows while the layer is on (as for OpenFreeMap, PNOA, SIGPAC).
- A new overlay is one entry in the source registry, one in `mapLayers.js` and
  one row in [map-data-sources.md](map-data-sources.md). If it needs more than
  that, stop and rethink it.
- Dated or campaign data keeps its version in the cache key, and the UI says how
  recent it is.
