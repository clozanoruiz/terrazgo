# Map layers — roadmap and caching decisions

> Status: decided 2026-07-11 (which layers are wanted and how WMS data is
> cached); **nothing here is scheduled**. The service inventory and the
> per-service details live in [agro-data-services.md](agro-data-services.md)
> and [sigpac-integration.md](sigpac-integration.md); this document records
> what will be built and in what order, so each layer arrives as a small,
> known-shape task instead of reopening the design.

## The two decisions

### 1. WMS data is cached as grid-snapped tiles ("self-proxied WMTS")

The geo cache and the `geo://` protocol are XYZ-shaped
(`tiles/{source}/{z}/{x}/{y}`), and most of the wanted sources are WMS-only
(verified 2026-07-11: Catastro WMS speaks EPSG:3857 but offers no WMTS;
IDEE hydrography likewise; ITACYL is WMS/WCS-family). Options considered:

- **WMTS-only policy** — rejected: it would exclude Catastro, hydrography,
  the MITECO zone layers and ITACYL, i.e. most of the wanted list.
- **MapLibre `{bbox-epsg-3857}` pass-through** (the webview computes the
  bbox, responses cached by URL in the `resource` table) — rejected:
  float-formatted bbox strings make fragile cache keys, and the rows would
  bypass the tile table's LRU size cap.
- **Rust-side grid snapping** — **chosen.** The protocol path stays
  `tiles/{id}/{z}/{x}/{y}`; the fetch layer computes the tile's EPSG:3857
  bounding box from z/x/y (pure Web-Mercator arithmetic, no new crate) and
  substitutes it into a `GetMap` URL template (`width=height=256`,
  `crs=EPSG:3857`, `format=image/png`, `transparent=true` for overlays).
  Responses are stored and served as ordinary XYZ tiles.

This is exactly what dedicated tile proxies (MapProxy, GeoWebCache) do,
reduced to a few lines of arithmetic and with no proxy to deploy. Every WMS
source becomes one more `TileSource` entry; the cache, the LRU cap, the
offline behaviour and the frontend raster handling all apply unchanged.

Consequences:

- The service-selection rule extends to: **MVT > WMTS > WMS-gridded** —
  native tiles when the provider has them, grid-snapped `GetMap` only when
  WMS is all there is.
- A WMS source must support EPSG:3857 to qualify (checked at pre-flight;
  Catastro and IDEE hydrography confirmed). Reprojection of map imagery is
  out of scope, as it is for boundary imports.
- The tile→bbox conversion is domain logic with a public source of truth
  (the slippy-map / EPSG:3857 tiling scheme) — test-first, values cited.
- Dated raster products (NDVI composites) carry their date in the cache key,
  the same mechanism as SIGPAC's campaign-keyed tiles.

### 2. CDSE credentials are farmer-supplied

For Copernicus Data Space Ecosystem APIs (NDVI overlays beyond CyL, and the
per-plot Statistical API series), each user registers their own free CDSE
account and enters it in settings (decided 2026-07-11; resolves
agro-data-services.md open question 1 for CDSE). Grounds: the Sentinel data
licence is free/full/open including commercial use with attribution — fully
compatible with an AGPL app — but API access authenticates per account with
per-account quotas, an AGPL binary cannot embed a shared secret, and the
CDSE terms treat quota-bypass via multiple accounts as a breach. A
farmer-supplied account makes each user a legitimate quota-holder; the free
tier is ample for one farm. A server-side proxy remains a possible later
addition for zero-friction onboarding, never a replacement.

Prerequisite: the core **settings module** (which now has four customers:
tile-cache cap, language roaming, CDSE credentials, and future API keys such
as SIAR). How credentials are stored on-device (settings table vs platform
keychain) is decided when that module is designed.

## The wanted layers, in build order

Order is by infrastructure readiness — each phase reuses everything the
previous one built. Within a phase, order is free.

### Phase 1 — own data as overlays (no network, no new deps) — SHIPPED 2026-07-12

| Layer | Source | Notes |
| --- | --- | --- |
| Treatment / PHI status | `list_phi_status` → module-cue `phi_status_for_farm` | Plots tinted by "in PHI window / harvest allowed" (red/green); derived on read, same window rule as the alerts |
| Zone-flag tint | `list_zone_flags` (core `plot_zone_flag`) | Latest campaign's 'inside' per (plot, zone kind) — the chip rule — one translucent fill per zone kind, overlaps blend |

One `mapLayers.js` GeoJSON entry each, as predicted. What the panel grew was
not grouping but two smaller contracts: `defaultVisible: false` (status tints
start toggled off) and a per-layer `legend` shown while visible; grouping
waits for a phase that actually overflows a flat list.

### Phase 2 — the rest of the Nube de SIGPAC MVT service — SHIPPED 2026-07-12

| Layer | Service layer | Notes |
| --- | --- | --- |
| Declared crops | `cultivo_declarado` | Dashed gold fill+line; campaign-keyed like recintos. **The fixed path serves the PREVIOUS campaign** (the running one's declarations are still open, per the service doc) — the layer label says so. The same dataset also ships as provincial GPKG downloads (current + previous campaign, CC BY 4.0) — the data path for the crop-prefill idea in [siex-export.md](siex-export.md) → "Farmer-side data paths"; the overlay here is only its display twin |
| Landscape elements | `e_paisaje_area`, `_linea`, `_punto` | PAC conditionality (protected features); three tile services behind ONE toggle — the `mapLayers.js` contract grew `vectors()` (multi-source entry, style specs pick theirs via `sourceKey`). Sparse data: most tiles are 404-empty |

Pre-flight ran 2026-07-12 against live tiles: source-layer names equal the
path names; attribute keys match the download-service models (declared crops:
`parc_producto`, `parc_sistexp`, `parc_supcult`, `exp_ano`…; landscape:
`tipo_elemento`); pbf z12–15; empty tiles answer 404 (cached as empty, the
recinto rule). Live-verified end-to-end in the real app: all four sources
stream through `geo://` with campaign-keyed cache rows.

### Phase 3 — public WMS overlays through grid snapping (needs decision 1 implemented once)

| Layer | Provider / service | Pre-flight checks |
| --- | --- | --- |
| Cadastral parcels | Catastro WMS (3857 confirmed) | Layer names, scale limits, attribution wording; pairs with the future SIGPAC↔catastro crosswalk |
| Hydrography | IDEE `wms-inspire/hidrografia` (3857 confirmed) | Which sublayers matter (watercourses, water points); regulatory hook: phyto buffer strips near water |
| Nitrate-vulnerable zones | MITECO WMS | **Endpoint not yet pinned** (2026-07-11 probe missed); licence + 3857 check |
| Natura 2000 | MITECO WMS | Same; display-only — the stored truth remains `plot_zone_flag`, this draws the boundary pixels |
| Soil cartography (CyL) | ITACYL Atlas / IDECyL WMS | Layer selection, licence per layer, 3857 check; regional-first per the inventory's open direction |
| NDVI mosaic (CyL) | ITACYL Sentinel-2 series | Endpoint to pin down; date-keyed caching |

### Phase 4 — CDSE (needs decision 2 + settings module)

| Capability | API | Notes |
| --- | --- | --- |
| NDVI overlay (national) | Sentinel Hub OGC (WMS/WMTS, evalscript) | Rides the same gridding; per-user OAuth token through the terrazgo-geo seam |
| NDVI per-plot time series | Statistical API / openEO | Not a map layer: synced user data by the zone-flag precedent; needs its own schema design (first per-plot time series) |

Attribution "Contains modified Copernicus Sentinel data [year]" while
active.

## Standing rules that apply to every layer

- All fetching through terrazgo-geo's cache-through seam; the webview only
  ever sees `geo://`.
- Attribution visible while the layer is active (OpenFreeMap/PNOA/SIGPAC
  precedent).
- A new overlay = one source-registry entry + one `mapLayers.js` entry +
  one row in [map-data-sources.md](map-data-sources.md) (the inventory);
  anything that needs more than that is a design smell to stop on.
- Dated/campaign products record their version in the cache key and the UI
  says how fresh the data is.
