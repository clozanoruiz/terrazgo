# External agronomic data services: soil, NDVI, irrigation

This lists the public services that could feed three future features (soil
data on the map, NDVI per plot, and irrigation guidance) and the questions each
one raises. They would serve the Irrigation, Fertilization & soil, and
Analytics modules. None of it is built.

Two of the open questions at the end are settled in
[map-layers-roadmap.md](map-layers-roadmap.md): WMS responses are cached as
grid-snapped EPSG:3857 tiles (question 2), and each farmer brings their own
CDSE account (question 1, for CDSE). That file also says which map layers are
wanted and in what order. The rest is still open: the shape of irrigation
guidance, how per-plot values are stored, and regional against national
sources.

## What any of these has to fit

- **One network path.** All fetching goes through `terrazgo-geo`'s cache, as
  SIGPAC does. Nothing else in the app talks to the network, everything fetched
  is cached, and the app keeps working with no connection (architecture.md →
  "The map tier").
- **Two ways in, both already used.** (a) *Map overlays*: a source in the
  `terrazgo-geo` registry plus one `mapLayers.js` entry. A soil or NDVI raster
  would appear this way. (b) *Per-plot values*: query once, store on the plot,
  work offline after that. SIGPAC zone flags (`plot_zone_flag`) work like this,
  and it fits soil properties at a centroid or an NDVI figure for a polygon.
- **Picking a service:** when a provider offers the same data several ways,
  take the most modern and lightest one: MVT, then WMTS, then WMS. The rasters
  below are all WMS or WMTS. Where a provider has both, prefer WMTS, because the
  tile cache is XYZ-shaped and WMS `GetMap` responses for arbitrary bounding
  boxes don't cache well (see the open questions).
- **Licence and attribution** are shown while a layer is on, as for
  OpenFreeMap, PNOA and SIGPAC.

## 1. Soil

| Service | Coverage / resolution | What it gives | Access | Licence |
| --- | --- | --- | --- | --- |
| **ITACYL — Atlas Agroclimático** ([atlas.itacyl.es/serviciosogc](https://www.atlas.itacyl.es/en/serviciosogc)) | Castilla y León | Soil and agroclimatic layers | WMS / WCS / WFS, **no auth** | JCyL open data (check per layer) |
| **ITACYL / IDECyL soil maps** ([idecyl.jcyl.es](https://idecyl.jcyl.es)) | Castilla y León | Regional soil maps (units, properties) | OGC services, **no auth** | JCyL open data |
| **SoilGrids** (ISRIC, [rest.isric.org](https://rest.isric.org/)) | **Global**, 250 m | pH, organic carbon, texture (clay/silt/sand), CEC, N, bulk density at 6 depths (0–200 cm) | REST point query + WMS, **no auth**; fair use 5 calls/min | CC-BY 4.0 |
| ESDAC (JRC) | Europe | Various soil datasets | Mostly registered downloads, few services | varies |

Notes:

- SoilGrids is a global model on a 250 m grid. For a 2 ha plot that is a few
  pixels, good for context and prefilling, not for a prescription. Its point
  query fits the fetch-once-per-plot pattern: query the centroid and store the
  properties on the plot.
- Regional maps (ITACYL for CyL) are usually finer and checked locally, but
  coverage across Spain is a patchwork and each community's schema differs.
  It is the same situation as the parcel providers (sigpac-integration.md →
  "The EU landscape").
- **Not decided:** regional first (ITACYL where it exists), with or without
  SoilGrids as the national base; or SoilGrids only, for a uniform result. The
  provider layer should allow either.

## 2. NDVI per plot

"NDVI" covers two different things:

1. **A map overlay**: the plot drawn over an NDVI mosaic. Cheap to add (a
   raster layer through the tile cache).
2. **A time series per plot**: NDVI averaged over the plot for each date, shown
   across a campaign. This is the useful one for the farmer (spotting
   anomalies, senescence) and the harder one to get.

| Service | Coverage | Product | Access | Licence |
| --- | --- | --- | --- | --- |
| **ITACYL Sentinel-2 NDVI series** ([dataset record](https://data.europa.eu/data/datasets/spasitnasentinel_2_ndvi-xml?locale=es)) | Castilla y León | NDVI mosaics (periodic composites) | OGC services / open download, **no auth** (endpoint still to find) | JCyL open data |
| **Copernicus Data Space Ecosystem** ([dataspace.copernicus.eu](https://dataspace.copernicus.eu/analyse/apis)) — Sentinel Hub OGC | EU/global | NDVI as WMS/WMTS tiles (evalscript) | **Free account required** (OAuth), monthly quota | Copernicus (free, attribution) |
| **CDSE — Statistical API / openEO** | EU/global | **NDVI statistics over a polygon per date**, i.e. the real per-plot series | Same account and quota | Copernicus |
| ITACYL **Sativum** ([sativum.es](https://www.sativum.es/en/)) | CyL | Full per-plot NDVI monitoring platform | Registered CyL users; useful as a UX reference, not something to integrate | — |

Notes:

- For CyL the overlay can be had without an account (ITACYL mosaic). For the
  rest of Spain it can't: CDSE's OGC endpoints need one.
- In practice the time series means CDSE's Statistical API or openEO. That
  makes **credentials the deciding question**, since it would be the first
  source that needs them:
  - **(a) The farmer's own account.** A settings field where the user pastes
    their own free CDSE credentials; the app gets and refreshes tokens through
    `terrazgo-geo`. Everything stays on the device and works offline (the
    series is cached per plot). The cost is signing up and storing the
    credentials on the device.
  - **(b) A server-side proxy.** A hosted service holds one credential and the
    app calls it. No sign-up for the user, but it is the first thing that
    needs to be online and a service someone has to run (the same kind as a
    future SIEX submission client).
  - **(c) Only sources without an account.** NDVI overlay where a region
    publishes it openly (CyL today), and no time series until (a) or (b).
  - [map-layers-roadmap.md](map-layers-roadmap.md) settles CDSE on (a). The
    general question for other keyed services is still open, and the design
    shouldn't rule any of the three out.
- Processing raw Sentinel-2 in the app (downloading L2A granules and computing
  NDVI) is out. The bandwidth and computing it needs are far beyond what a farm
  app should use.

## 3. Irrigation guidance

There are two approaches. They can live together, and **which to use is not
decided**.

**Read regional advice.** Some communities run an irrigation advisory service
(Servicio de Asesoramiento al Regante) that publishes actual recommendations:

| Service | Coverage | Product | Access |
| --- | --- | --- | --- |
| **Inforiego** (ITACYL, [inforiego.org](https://www.inforiego.org/opencms/opencms/api_rest/); [JCyL open data](https://datosabiertos.jcyl.es/web/jcyl/set/es/medio-rural-pesca/consultas-inforiego/1284807462534)) | Castilla y León | Weekly irrigation needs per crop and zone; per-plot estimates | REST API, **key given on request** (meant for collective users); web and app are free |
| RIA / SAR (IFAPA) | Andalucía | Station data + advice | same regional pattern |
| Oficina del Regante (SARGA) | Aragón | Advice | same regional pattern |

**Work it out in the app from public data.** The national **SIAR** network
([servicio.mapa.gob.es/siarweb](https://servicio.mapa.gob.es/siarweb/masInformacion);
[REST API](https://datos.gob.es/es/aplicaciones/sistema-de-informacion-agroclimatico-y-de-regadios-siar))
publishes what a recommendation is built from: more than 460 agro-climatic
stations in the irrigated areas of 12 communities, with Penman-Monteith
**ETo**, rainfall and temperatures. Registration is free and gives an API key.
The FAO-56 calculation (crop coefficient Kc × ETo − effective rain, over the
crop calendar the app already stores) fits the planned Rust/Polars analytics,
and it works offline: fetch the nearest station's daily values when there is a
connection, calculate locally always. It needs a Kc table (FAO-56) as
reference data, and the app has to say clearly that it is an *estimate*, not
official advice. Where regional advice exists, it can be used to check the
estimate.

## Open questions (they cut across all three)

1. **API keys and accounts.** SIAR, Inforiego and CDSE all need a key or an
   account. Options (a), (b) and (c) above apply to all three, and whatever is
   chosen first sets the pattern. An AGPL binary can't ship a secret inside it.
2. **How rasters are cached.** The geo cache is built around XYZ tiles and
   resources. WMTS fits; plain WMS `GetMap` with arbitrary bounding boxes
   doesn't, and would need either snapping the boxes to a grid or a WMTS-only
   rule.
3. **Where per-plot values live.** Soil properties and NDVI series come from a
   provider but can't be worked out again offline, so, like zone flags, they
   would be synced user data (logged in `record_change`), not cache. NDVI is a
   *time series* per plot, bigger than anything stored per plot so far, and
   needs its own schema design.
4. **Regional against national.** Regional services are better where they exist
   (ITACYL); national or global ones are the same everywhere. As with parcel
   providers: a provider layer based on what each one can do, and a UI that
   shows what the active providers support.
5. **Dates and campaigns.** NDVI composites and ETo are dated series. Cached
   data must keep its date, and the UI must say how recent it is (the same rule
   as SIGPAC's campaign tagging).
