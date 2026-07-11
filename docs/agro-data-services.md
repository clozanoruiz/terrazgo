# External agronomic data services — soil, NDVI, irrigation

> Status: research (2026-07-09). This inventories the public services that
> could feed three future capabilities — soil characteristics on the map,
> NDVI at plot level, and irrigation guidance — and lays out the integration
> questions each one opens. The services here would serve the Irrigation,
> Fertilization & soil, and Analytics modules; none of those has started.
>
> **Update 2026-07-11:** two of the open questions below are now decided in
> [map-layers-roadmap.md](map-layers-roadmap.md) — WMS responses are cached
> as grid-snapped EPSG:3857 tiles (question 2), and CDSE credentials are
> farmer-supplied (question 1, for CDSE). That document also fixes which
> map layers are wanted and their build order. Everything else here
> (irrigation guidance shape, per-plot storage schema, regional-vs-national
> layering) remains undecided.

## Ground rules any of these must fit

- **The single network seam.** All fetching goes through `terrazgo-geo`'s
  cache-through layer, exactly like SIGPAC: nothing else in the app talks to
  the network, everything fetched is cached, and the app keeps working with no
  connectivity (architecture.md → "The map tier").
- **Two integration shapes already exist.** (a) *Map overlays*: an allowlisted
  source in the `terrazgo-geo` registry + one `mapLayers.js` entry — how a
  soil or NDVI raster would appear over the base maps. (b) *Per-plot values*:
  query once, store on the plot, works offline forever after — how SIGPAC zone
  flags work (`plot_zone_flag`), and the natural pattern for soil properties
  at a centroid or an NDVI statistic for a polygon.
- **Service selection rule (2026-07-07):** when a provider offers several
  services for the same data, pick the most modern and bandwidth-frugal —
  MVT > WMTS > WMS. All the raster products below are WMS/WMTS territory;
  where a provider offers both, WMTS is strongly preferred because the tile
  cache is XYZ-shaped (arbitrary-bbox WMS `GetMap` responses do not cache
  naturally — see Open questions).
- **Licence and attribution** displayed while a layer is active, as with
  OpenFreeMap/PNOA/SIGPAC today.

## 1. Soil characteristics

| Service | Coverage / resolution | What it gives | Access | Licence |
| --- | --- | --- | --- | --- |
| **ITACYL — Atlas Agroclimático** ([atlas.itacyl.es/serviciosogc](https://www.atlas.itacyl.es/en/serviciosogc)) | Castilla y León | Soil and agroclimatic layers | WMS / WCS / WFS, **no auth** | JCyL open data (verify per layer) |
| **ITACYL / IDECyL soil cartography** ([idecyl.jcyl.es](https://idecyl.jcyl.es)) | Castilla y León | Regional soil maps (units, properties) | OGC services, **no auth** | JCyL open data |
| **SoilGrids** (ISRIC, [rest.isric.org](https://rest.isric.org/)) | **Global**, 250 m | pH, organic carbon, texture (clay/silt/sand), CEC, N, bulk density at 6 depth intervals (0–200 cm) | REST point query + WMS, **no auth**; fair use 5 calls/min | CC-BY 4.0 |
| ESDAC (JRC) | Europe | Various soil datasets | Mostly registered downloads, weak service surface | varies |

Notes:

- SoilGrids' 250 m grid is a *global model*: for a 2 ha plot that is a handful
  of pixels — context and prefill, never a prescription. Its point query maps
  perfectly onto the fetch-once-per-plot pattern (centroid → store properties
  on the plot).
- Regional cartography (ITACYL for CyL) is generally finer and locally
  validated, but Spain-wide coverage is a per-community patchwork and the
  layer schemas differ — the same capability-based provider situation as LPIS
  (sigpac-integration.md → "The EU landscape").
- **Open direction, not decided:** regional-first (ITACYL where available)
  with or without SoilGrids as the country-neutral base/complement; or
  SoilGrids-only for uniformity. The provider-layer design should keep both
  arrangements possible.

## 2. NDVI at plot level

Two distinct products hide under "NDVI":

1. **A map overlay** — the plot painted over an NDVI mosaic. Cheap to
   integrate (raster layer through the tile cache).
2. **A per-plot time series** — NDVI aggregated over the plot polygon per
   date, chartable across a campaign. This is the agronomically useful one
   (anomaly detection, senescence) and the harder one to source.

| Service | Coverage | Product | Access | Licence |
| --- | --- | --- | --- | --- |
| **ITACYL Sentinel-2 NDVI series** ([dataset record](https://data.europa.eu/data/datasets/spasitnasentinel_2_ndvi-xml?locale=es)) | Castilla y León | NDVI mosaics (periodic composites) | OGC services / open download, **no auth** (endpoint to pin down at design time) | JCyL open data |
| **Copernicus Data Space Ecosystem** ([dataspace.copernicus.eu](https://dataspace.copernicus.eu/analyse/apis)) — Sentinel Hub OGC | EU/global | NDVI rendered as WMS/WMTS tiles (evalscript) | **Free account required** (OAuth), monthly quota | Copernicus (free, attribution) |
| **CDSE — Statistical API / openEO** | EU/global | **NDVI statistics over a polygon per date** — the true per-plot time series | Same account/quota | Copernicus |
| ITACYL **Sativum** ([sativum.es](https://www.sativum.es/en/)) | CyL | Per-plot NDVI monitoring, full platform | Registered CyL users; a reference UX, not an integration surface | — |

Notes:

- The map-overlay half has a no-auth path today for CyL (ITACYL mosaic);
  nationally it does not — CDSE's OGC endpoints need an account.
- The time-series half realistically means the CDSE Statistical API/openEO.
  That makes **credential handling the gating design decision** (first data
  source to require one):
  - **(a) Farmer-supplied account.** A settings field where the user pastes
    their own (free) CDSE credentials; the app exchanges/refreshes tokens
    through the `terrazgo-geo` seam. Keeps everything client-side and
    offline-first (series cached per plot); cost is sign-up friction and
    credential storage on-device. Plausible short-term path.
  - **(b) Server-side proxy.** A hosted component holds one credential and
    the app calls it — no user sign-up, but it is the first always-online
    dependency and a hosted service to run (fits the open-core service model,
    same family as the future SIEX submission client). Longer-term option.
  - **(c) No-auth sources only.** Overlay-only NDVI where a region publishes
    it openly (CyL now); no time series until (a) or (b).
  - **Undecided.** (a) looks more plausible than (b) in the short term, but no
    commitment; the design should not foreclose any of the three.
- Raw Sentinel-2 processing in-app (download L2A granules, compute NDVI) is
  out: bandwidth and compute are wildly out of proportion for a farm app.

## 3. Irrigation guidance

Two philosophies, not mutually exclusive — and **which one (or both) is an
open decision**:

**Consume regional advisories.** Some communities run a Servicio de
Asesoramiento al Regante that publishes actual recommendations:

| Service | Coverage | Product | Access |
| --- | --- | --- | --- |
| **Inforiego** (ITACYL, [inforiego.org](https://www.inforiego.org/opencms/opencms/api_rest/); [JCyL open data](https://datosabiertos.jcyl.es/web/jcyl/set/es/medio-rural-pesca/consultas-inforiego/1284807462534)) | Castilla y León | Weekly irrigation needs per crop/zone; per-plot estimates | REST API, **key granted on request** (aimed at collective users); web/app free |
| RIA / SAR (IFAPA) | Andalucía | Station data + advisory | similar regional pattern |
| Oficina del Regante (SARGA) | Aragón | Advisory | similar regional pattern |

**Compute in-app from public data.** The national
**SIAR** network ([servicio.mapa.gob.es/siarweb](https://servicio.mapa.gob.es/siarweb/masInformacion);
[REST API](https://datos.gob.es/es/aplicaciones/sistema-de-informacion-agroclimatico-y-de-regadios-siar))
publishes what a recommendation is made of: 460+ agro-climatic stations in the
irrigated zones of 12 CCAA, with Penman-Monteith **ETo**, rainfall and
temperatures. Free registration → API key. The FAO-56 calculation
(crop coefficient Kc × ETo − effective rain, over the crop calendar we already
store) is well within the in-app Rust/Polars analytics plan, and it degrades
offline gracefully: sync the nearest station's daily values when connected,
compute locally always. Needs a Kc catalogue (FAO-56 tables) as reference
data, and honesty about being an *estimate*, not an official advisory —
regional advisories double as validation where they exist.

## Open questions (cross-cutting, all undecided)

1. **API-key / credential handling** — SIAR, Inforiego and CDSE all gate on
   keys or accounts. Options (a)/(b)/(c) above apply to all three; whatever is
   chosen first sets the precedent. An AGPL binary cannot ship an embedded
   secret.
2. **Raster caching shape.** The geo cache is XYZ-tile- and resource-oriented.
   WMTS fits it; plain WMS `GetMap` (arbitrary bboxes) does not cache
   naturally and would need either a bbox-snapping scheme or WMTS-only policy.
3. **Where per-plot values live.** Soil properties and NDVI series are
   provider-derived but not re-derivable offline — by the zone-flag precedent
   they would be synced user data (`record_change`-logged), not cache. NDVI is
   a *time series* per plot: bigger than anything stored per plot so far;
   needs its own schema thinking.
4. **Regional vs national layering.** Regional services are better where they
   exist (ITACYL), national/global ones are uniform. The parcel-provider
   lesson applies: capability-based provider layer, UI shows what the active
   providers support.
5. **Update cadence and campaign tagging.** NDVI composites and ETo are dated
   series; cached data must carry its date and the UI must say how fresh it
   is (same rule as SIGPAC campaign tagging).
