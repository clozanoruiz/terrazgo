# SIGPAC integration & shared mapping

This covers three separate pieces, only one of them Spanish: the **shared map
UI**, the **country-neutral parcel-provider layer**, and the **SIGPAC module**.
How the map tier is put together in code is in `docs/architecture.md` → "The
map tier". This file explains SIGPAC itself, which of FEGA's services the app
uses and how, and the facts about those services that the code depends on.

## What SIGPAC is, and what the module does with it

SIGPAC (Sistema de Información Geográfica de Parcelas Agrícolas) is Spain's
LPIS, the Land Parcel Identification System that every EU member state runs as
the map behind CAP aid. A plot stores its 7-part SIGPAC reference on
`plot_es_extension` (`sigpac_province … sigpac_enclosure`). The module uses it
for:

1. **Checking and prefilling a reference.** One lookup returns the recinto's
   official surface (compared with `area_ha`), land use, slope, irrigation
   coefficient and boundary. A typo in the 7-part code shows up at once instead
   of at an inspection.
2. **The plot map.** The farm's recintos drawn over the orthophoto.
3. **GPS position → recinto** (mobile): "which recinto am I standing on" while
   recording a treatment in the field.
4. **Zone checks → alerts.** SIGPAC's query service says whether a recinto
   falls inside a nitrate-vulnerable zone (this drives the fertilisation
   record duty), a phytosanitary restriction zone (treatment registers) or
   Natura 2000. The results are stored as zone flags, and core's zone alerts are
   worked out from them whenever the alert list is read.
5. **Declared crops**: the crops declared to the PAC, offered as a crop list to
   review and import.

Not built: a cadastral crosswalk (SIGPAC code → referencia catastral) and
offline municipality packs (see below).

## Which FEGA services to use

FEGA runs two families of services, and the difference is legal:

- **Visor services** (`sigpac.mapa.gob.es/fega/serviciosvisorsigpac/…`) run the
  official viewer. **Third-party applications are not allowed to use them.**
  Never call them, even though blog posts describe them.
- **"Nube de SIGPAC"** (`sigpac-hubcloud.es`) was set up "to serve as a basis
  for the development of computer applications". This is what the app uses.
  Licence **CC BY 4.0** (recintos and landscape elements are official
  "high-value datasets"). No authentication and no published rate limits, so
  cache everything and don't hammer it.

| Service | Protocol / format | What it gives | Used for |
| --- | --- | --- | --- |
| **Consultas SIGPAC** | REST, JSON/GeoJSON | 11 endpoints: recinto by code or by coordinates, centroid, cadastral ref by code, all recintos of a parcel, intersections (Natura 2000, nitrate, phytosanitary, montanera, permanent pasture) | Most things: checking, prefill, GPS lookup, zone checks |
| **Teselas vectoriales (MVT)** | XYZ tiles, PBF z12–15 (GeoJSON z15), EPSG:3857 | Recintos, declared crops, landscape elements; current and previous campaign | Map overlays without bulk downloads |
| **WMS** | ISO 19128 / INSPIRE, GeoServer | Rendered recintos and the SIGPAC orthophoto | Not used; MVT is preferred for vectors |
| **OGC API Features** | OGC API, GeoJSON, WGS 84 | Recintos, declared crops, landscape elements as features | Declared crops by reference |
| **Listas de códigos** | REST, JSON | Provinces, municipalities, SIGPAC land-use codes… | Lists and checking the parts of a reference |
| **Descargas** | GeoPackage per province | Current campaign in bulk | Offline packs if ever needed (a GPKG **is** SQLite, so rusqlite reads it directly) |
| **ATOM (INSPIRE)** | Atom feeds → Shapefile/GPKG per municipality | Recintos, landscape elements, declaration lines; about 39.5 GB for all of Spain | Municipality-sized offline packs, the right size for a farm |
| Salidas gráficas | — | Printable map sheets | Not needed |

The main endpoint uses the stored reference as its path:

```
https://sigpac-hubcloud.es/servicioconsultassigpac/query/recinfo/
        {province}/{municipality}/{aggregate}/{zone}/{polygon}/{parcel}/{enclosure}.geojson
```

It returns the boundary (WKT/GeoJSON + EPSG), surface (ha), land use, slope,
irrigation coefficient, admissibility and incidences.

**Base imagery:** IGN's PNOA orthophoto WMTS (`https://www.ign.es/wmts/pnoa-ma`,
INSPIRE WMTS, CC BY 4.0, attribution "PNOA cedido por © Instituto Geográfico
Nacional"). SIGPAC data is published per campaign (yearly, refreshed around
February), so cached data records its campaign and is refreshed when the
campaign changes.

ITACYL republishes SIGPAC for Castilla y León (bulk FTP). It is handy for
testing, but the app uses the national services.

### What the services actually return

These are facts about the live services that the code relies on. If FEGA
changes one, this is the list to check.

- **Consultas:** `query/recinfo/{7-part}.json|.geojson`,
  `query/recinfobypoint/4326/{lon}/{lat}.json` and
  `intersection/{nitratos|fitosanitarios|red_natura}/{7-part}.json`. Recinto
  attributes: superficie (ha), pendiente_media, coef_regadio, uso_sigpac,
  admisibilidad, incidencias, region, altitud, wkt + srid. Intersections return
  `surface_intersection` (m²) and `surface_tpc` (%), plus `descripcion` (e.g.
  "Zona periférica" for fitosanitarios). Responses are gzipped. FEGA keeps two
  campaigns.
- **An unknown reference returns HTTP 200 with `[]`**, not 404. "Not found" has
  to be read from the empty array.
- **There is no "current campaign" endpoint** in the consultas or code-list
  services. The campaign comes from the highest year in the `/geopackages/`
  directory listing (`https://sigpac-hubcloud.es/geopackages/{campaign}/recintos/`;
  the province files there are 112–545 MB). `terrazgo_geo::fetch` owns
  `current_campaign`, because tile caching below the module needs it too.
- **MVT:** pbf at z12–15 only; the recinto source layer is **`recinto`**, with
  the same attribute keys as recinfo. **The URL has no campaign year**: the fixed
  path serves the current campaign and `/mvt/anterior/` the previous one. So the
  cache rows are keyed by campaign (`sigpac-recintos@{campaign}`), and the old
  campaign's tiles are dropped the first time a new campaign's tile is stored.
  **Empty tiles answer HTTP 404**; they are cached and served as empty, so they
  cost nothing the second time and stay empty offline. MVT surfaces are in
  **m²**.
- **The visor exports GeoJSON, GML and Shapefile**: no GPKG and no KML. The app
  imports GeoJSON and GPKG, so there is no need for KML unless a GPS tool needs
  it.
- **Nube GPKGs are geographic (EPSG:4258)**, not UTM. Canary Islands files
  declare **REGCAN95 (EPSG:4081)**; the registered REGCAN95→WGS84
  transformation is 0,0,0, so the importer accepts it as-is and that is exact,
  not an approximation.

## The EU landscape: why the provider layer is country-neutral

Every member state has an LPIS and most publish it openly, **but they can do
different things**, so the abstraction is built on capabilities, not on
SIGPAC's shape:

| Country | System | Access | Licence | Lookup by a reference the farmer knows? |
| --- | --- | --- | --- | --- |
| Spain | SIGPAC (FEGA) | REST query, MVT, WMS, OGC API, GPKG/ATOM | CC BY 4.0 | **Yes**, the 7-part code is public |
| France | RPG (ASP → IGN) | WFS/WMS/WMTS + yearly bulk downloads | Licence Ouverte 2.0 | **No**: îlots and parcelles are anonymised; farmers only know their parcels from their own PAC file |
| Netherlands | BRP Gewaspercelen (RVO → PDOK) | OGC API Features, WFS, downloads | open (PDOK) | No stable public reference per farmer; bbox and point queries work well |
| Others | Luxembourg, Denmark, Austria… publish LPIS through INSPIRE portals | mixed (WFS/Atom usually) | mixed, open | varies |

What follows:

- **Point and bbox queries and map display work everywhere** (every LPIS can
  answer "which parcel is here" somehow). **Checking a reference doesn't**: only
  Spain allows it. The provider exposes what it can do, and the UI only shows
  what the active provider supports.
- **The plumbing is fully shared**: HTTP fetch, response cache, tile cache,
  attribution, campaign tagging. A new country is one provider, mostly URL
  templates and field mapping.

## How it is built: three pieces

The rule still holds: **no network calls in core or in the modules.** All HTTP
goes through `terrazgo-net`, used by `terrazgo-geo`'s cache; everything fetched
is cached; and the app works fully offline, with features falling back to
"cached or manual data only".

### 1. Shared map UI (shell, not a module)

- `MapCanvas.svelte`/`MapView.svelte` in `src/lib/` wrap **MapLibre GL JS**: it
  draws vector tiles natively, which suits SIGPAC's MVT service; it is fast in
  the webview; and it has no licence fees. OpenLayers would only win with
  unusual projections, and SIGPAC MVT is plain EPSG:3857. The full comparison
  is in [stack-choices.md](stack-choices.md) §1.
- Any module can use the map. Layers are data (`mapLayers.js`), the same idea
  as `nav.js`.
- The webview never talks to the internet. MapLibre asks a custom Tauri
  protocol (`geo://tiles/…`) served by Rust, and the production CSP stays
  `default-src 'self'` plus that scheme. That gives an offline cache,
  attribution in one place, and no CSP exception per service.
- **Boundaries don't have to come from SIGPAC.** The map has drawing
  (`terra-draw`) and boundary file import (GeoJSON and GeoPackage). That covers
  references SIGPAC can't resolve and farms outside Spain.
- **A free base map for countries without an open orthophoto:** OpenFreeMap
  (OSM vector tiles, no API key, no usage limits), cached through the same tile
  cache. Each provider can add its country's official imagery on top (Spain:
  PNOA; France: IGN).

### 2. `crates/terrazgo-geo`: the shared plumbing

- The tile and response cache is a separate SQLite file (`geo-cache.db`,
  close to the MBTiles schema). It is not the user database on purpose: tiles
  are bulky, can be fetched again and are derived, so they must not bloat
  backups or the `record_change` log. Cache rows carry their campaign and fetch
  date.
- `geozero` (with `geo-types`) converts WKT/WKB to GeoJSON and reads
  GeoPackage geometry blobs through rusqlite.
- A `ParcelProvider` trait isn't extracted yet. The plan is to do it when a
  second country is real, not before, but the code is already shaped for it: no
  `sigpac_` names in the shared crate or in the map component. A sketch of its
  shape, built on capabilities:

  ```rust
  trait ParcelProvider {
      fn country(&self) -> &str;                       // "ES"
      fn capabilities(&self) -> Capabilities;          // ref_lookup | point_query | tiles | zones | bulk
      fn parcel_by_reference(&self, r: &ParcelRef) -> Result<ParcelInfo, GeoError>;
      fn parcel_by_point(&self, lon: f64, lat: f64) -> Result<ParcelInfo, GeoError>;
      fn zone_checks(&self, r: &ParcelRef) -> Result<Vec<ZoneFlag>, GeoError>;
  }
  ```

### 3. `crates/module-sigpac`: the Spanish provider (a normal module)

- Talks to sigpac-hubcloud and owns the SIGPAC commands (check a reference,
  fetch a recinto, zone checks, declared crops). All HTTP goes through
  `terrazgo-geo`'s `cached_resource`, so every lookup is cached and works
  offline afterwards.
- Registered like the other modules; it has no migrations of its own.
- Other modules use its results *through core data* (stored zone flags, stored
  boundaries), so modules still don't depend on each other. The fertilisation
  trigger, for example, reads `plot_zone_flag` from core.

## Plot boundaries: `geo_feature`

A boundary attached to a user's plot is **user data**: it syncs, it is backed
up and its changes are logged, unlike tiles. It lives in the core table
**`geo_feature`**, which uses an **exclusive arc**: one nullable foreign key
per kind of subject (`plot_id`, `farm_id`) and a CHECK that exactly one is set.
That works for future subjects (irrigation features, farm boundaries) while the
database still enforces the foreign keys, which a polymorphic
(subject_table, subject_id) pair couldn't. Provider attributes (land use, slope,
irrigation coefficient…) go in a `properties` JSON column tagged with their
source, not in typed columns.

The official surface is stored as `geo_feature.official_area_ha`, beside the
farmer's `plot.area_ha` and never instead of it, so the plot card can show
"declared 2.10 ha / SIGPAC 2.14 ha" without changing what the farmer entered.

There are three ways to create a plot from SIGPAC, all leading to the same plot
form:

- **Reference first:** type the reference in the plot form, check it and
  prefill. After saving, the official boundary is stored from the cached
  lookup.
- **Map first:** click the map, `recinfobypoint` finds the recinto, then create
  a plot or attach it to the matching one.
- **Import first:** "create plot from recinto" in the GPKG/GeoJSON import list.

If a plot with the same SIGPAC reference exists, the app offers to attach the
boundary to it instead of creating a duplicate.

### Projected files

GeoPackages in a projected CRS (UTM and the like) are refused with the error
`gpkg_unsupported_srs`. SIGPAC's own files don't need reprojection, but other
Spanish public data does use projected systems (ETRS89 UTM 25828–31, REGCAN95
UTM 4083, INSPIRE 3034/3035, WGS84 UTM 32628–31). If a real file needs it, the
plan is a `proj4rs`-backed EPSG → proj-string table in `terrazgo-geo` covering
that list. `proj4rs` is pure Rust (lcc/laea/etmerc + towgs84, MIT/Apache-2.0).
The georust `proj` crate is not an option: it needs C libproj and a proj.db
file, which makes mobile cross-compiling painful. ED50 (23028–31) is left out.

## Zone flags: `plot_zone_flag`

SIGPAC's query service can say, for a recinto, whether it falls inside official
regulatory zones. The ones that matter:

- **Nitrate-vulnerable zones** (Directive 91/676/CEE, declared by each
  community): a plot inside one has to keep fertilisation records, with a cap on
  nitrogen.
- **Phytosanitary restriction zones**: plant-health areas (quarantine buffers,
  treatment duties or bans), which affect the phytosanitary registers.
- **Natura 2000**: PAC conditionality limits on operations.
- Montanera and permanent pasture: aid categories, lower priority.

These are facts about geography that rarely change (zones are revised every few
years; SIGPAC republishes each campaign). They change which records the law
requires. And they **can't be worked out offline**: they come from a network
query. "Was this plot inside the zone in campaign 2027?" is a question an
inspection can ask.

So they are stored in the core table **`plot_zone_flag`**, with a `zone_type`
lookup: one row per plot, zone type, campaign and source.

- **"Outside" is stored too.** `status = 'outside'` proves the check ran and
  came back clear; a missing row means "never checked". The two must not be
  confused.
- Within a campaign a new check replaces the row; a new campaign adds rows. Past
  answers stay, so it can be shown what applied in an earlier campaign.
- The rows are logged in `record_change` and sync, because another device
  can't work them out offline. Alerts are different: each device works them out
  from local tables.
- A new zone type, or another country's zones, is new rows and a new code,
  never a migration.

Two other designs were considered. Boolean columns on the plot
(`in_nitrate_zone`, …) would need a migration for every new zone type or
country, and a refresh would overwrite the past. Not storing the flags at all
and asking each time would fail exactly where farms are, without a connection.

In the app, the zone check runs as part of checking a plot: one tap gives the
boundary and the zones, and a failed zone check never undoes the stored
boundary. The alerts are `nitrate_zone`, `phyto_zone` and `natura_zone`. One
fires for the latest campaign's 'inside' flag, its subject is the PLOT (so
dismissing it survives re-checks and new campaigns), and it is due at the end of
the campaign year.

## Declared crops: "load my crops"

The PAC graphical declaration says what each recinto was declared as growing.
That is the crop list the farmer would otherwise type again, and it is public:
FEGA publishes it as `cultivo_declarado` on the Nube de SIGPAC **OGC API
Features** endpoint (`https://sigpac-hubcloud.es/ogcapi`, CC BY 4.0, no auth).

### Why OGC API and not the MVT layer

The "MVT, then WMTS, then WMS" rule is about *display*. This is a lookup of
record data by an identifier, the same split the app makes for recintos: shown
through the MVT overlay, looked up by reference through consultas. What decides
it is how long the cache keeps the answer. `cached_resource` writes to the
`resource` table, which is **never evicted**, while tiles live in an LRU cache
with a size limit. Crop data read from tiles could stop being available offline
after enough panning around the map. Data close to the record book is cached by
identity, not by what was on screen. (Consultas has no declared-crops endpoint,
so the OGC API is also the only way to ask by reference.)

### What the service does

- Collection `cultivo_declarado`. The seven reference parts **and `exp_ano`**
  can be passed as plain query parameters, so one recinto is one request of
  about 3.7 kB. No bbox mode, no paging.
- **`exp_ano` can be filtered on but isn't returned in the items.** The
  campaign a line belongs to is the campaign that was asked for; it can't be
  read back from the feature.
- The service runs **one campaign behind** the campaigns listing: when the
  listing names a year, only the year before answers.
- Nothing declared is **HTTP 200 with `numberMatched: 0`**, never a 404.
- Attributes used: `parc_producto` (PRODUCTOS code), `cultsecun_producto`
  (secondary crop), `parc_sistexp` (`"S"` secano / `"R"` regadío),
  `parc_supcult` (**square metres**, the same trap as the MVT layer). Also
  there and unused: aid lines, the expediente, `parc_indcultapro`,
  `tipo_aprovecha`.
- A recinto often has **several declaration lines**, usually the same crop
  split by irrigation system. The test fixtures cover this.

### Which campaign, and how far to trust the cache

The current campaign is asked first and the previous one is the fallback; the
campaign that answered goes with the lines. Cache keys are
`sigpac/cultivos/{campaign}/{ref}`, so a new campaign writes new rows instead of
overwriting old ones.

For the **current** campaign, a cached empty answer is trusted only for the
rest of the UTC day it was fetched. It may come from before FEGA loaded that
campaign, and caching "nothing declared" for good would hide the declaration
for the rest of the year. Asking more than once a day gains nothing, since the
delay that matters is months, and a campaign is loaded on some *day*. (The tile
cache also touches `last_used_at` once per UTC day.) For the **previous**
campaign the cache is final, empty answers included, because that data is
closed. "SIGPAC has no declaration for this plot" is only reported when both
campaigns actually answered. If neither could be reached, the network error is
shown instead: no answer from a service is not proof of an empty declaration.

What follows from that:

- **The two buttons do different things.** "Cargar cultivos declarados" uses
  the cache first and is normally silent; "Actualizar desde SIGPAC" ignores the
  day rule and asks both campaigns again.
- **The first load of the day costs one request per plot** while the current
  campaign answers empty. That is the price of not hiding a declaration that
  appears during the campaign, and it happens at most once a day, not once per
  click.
- **A plot that couldn't be asked about is reported, never an error that stops
  the rest.** It goes in `plots_unreachable` with the reason, and the rest of
  the panel works. Offline, a plot with no declaration in either campaign can't
  get an answer, and a farm with one pasture outside the PAC declaration is
  normal. "Couldn't find out" and "nothing there" stay separate in the UI, the
  same distinction as `plot_zone_flag`'s stored 'outside'.

### What the import may and may not do

Nothing is written without review. Proposals are built read-only, every row is
opt-in and editable, and the writing is done by core's crop repository in a
separate confirmed step. Rows are of five kinds:

| Kind | When |
| --- | --- |
| `insert` | the plot has no crops (pre-selected; the usual case) |
| `insert_secondary` | the line declares a `cultsecun_producto` (never an update: it is a second crop, not a correction) |
| `update` | the plot's **only** crop differs, has **no treatments this season**, and the declaration has **one** main line |
| `already_recorded` | a recorded crop matches, by catalogue code or by name |
| `blocked` | `multi_crop`, `has_treatments` or `multi_line`: shown so the difference is visible, never applied |

The treatment check isn't there to protect past records: `treatment_plot`
keeps the species and variety as they were when it was written, so no crop
edit can change them. It is there so that section 2.1 and section 3.1 of the
book agree. Changing a crop that a treatment points at would make the book list
one crop and print another next to the treatment. The input for the check comes
from module-phytosanitary through the shell; the two modules never call each
other.

Two mapping rules:

- `parc_sistexp` `"S"` prefills `rainfed`; `"R"` prefills **nothing**. SIGPAC
  says the crop is irrigated but not how, and Anexo III A.2.e asks for the
  system (SEC/ASP/LOC/GRA). Picking one would be making up a fact.
- A `parc_producto` code the catalogue doesn't know keeps the code and leaves
  the name blank. The code is what matters; the label is only for display.

Every proposal row and the confirm button say **which campaign answered**
("declaración PAC campaña 2025 → temporada 2025/2026"). Recording last year's
declaration as this year's crop without saying so is the one way this feature
could do harm.

### Where a crop came from, and the species picker

An imported crop has `source = 'sigpac'`, `source_campaign` and
`declared_area_ha` (next to `area_ha`, never instead of it). `UpdateCrop` only
sets these three when they are given, so a later manual edit can't erase where
a row came from.

`crop.crop_code` also lets manual entry use the catalogue: the crop form's
species field searches PRODUCTOS as you type, narrowed by the plot's checked
`uso_sigpac` through the vendored `CULTIVO_USO_SIGPAC` catalogue. When the
narrowing can't be trusted (no plot, no checked boundary, or a land use that
matches nothing) it shows the full list, since a filter that hides everything
is worse than none. Free text is still allowed; it just has no code.

## Standing on a plot: GPS lookup and live tracking

"Which recinto am I standing on" uses `module_sigpac::client::recinto_by_point`,
the same call as a map click, with a GPS fix in place of the click.

`tauri-plugin-geolocation` (minimum `2.3`, the version whose
`get_current_position` and `request_permissions` the code expects) is
**mobile-only**: it has no desktop version. So the dependency is limited to
mobile targets, it is registered under `#[cfg(mobile)]`, and its permissions are
in a mobile-only `capabilities/mobile.json`. It adds its own Android manifest
permissions through the manifest merger, so the generated project needs no hand
edits.

Two rules:

- **Decide whether to show the controls from the platform, never by calling a
  command to see if it works.** The plugin rejects with "Location services are
  disabled." when the phone's location is switched off, so a rejection doesn't
  mean the plugin is missing. Gating on it made the whole feature disappear on a
  phone with GPS off. The controls use the `is_mobile` command (`cfg!(mobile)`,
  known at compile time). In general: don't gate a feature on a command that can
  fail for other reasons than the one you are testing.
- **A location failure isn't a backend error.** A denied permission or no GPS
  signal is shown where it happens (`map.locate_denied` / `map.locate_error`
  with the reason), not through the command error path. A phone indoors isn't a
  fault.

Live tracking uses the same plugin: a follow toggle streams `watch_position`
fixes over a Tauri channel to a dot with an accuracy circle. The circle is drawn
as a 64-point polygon in real ground units, because a MapLibre circle layer
sizes in screen pixels and wouldn't scale with zoom. The camera only follows the
*first* fix; re-centring on every fix would fight the user's own panning. The
watch stops when the toggle is turned off and when the view closes, so GPS
never stays on after the map.

MapLibre's own `GeolocateControl` isn't used: it goes through
`navigator.geolocation`, a second permission path outside the plugin and the
ACL.

The lookup button is in the map's SIGPAC panel, so it only appears for Spanish
farms: it answers a Spanish question. The follow toggle is in the map toolbar,
because a position on a map is the same in any country.

## Offline municipality packs

Not built. Bulk GPKG or ATOM downloads per municipality, only if use in the
field shows the online cache isn't enough. The GPKG reader used for boundary
import already does the hard part. Endpoints and sizes are in
`docs/maintenance.md`.

## SIGPAC alegaciones

An alegación is the formal request a farmer files with their autonomous
community to *correct SIGPAC itself* when it is wrong (land use, boundary,
irrigation coefficient…), usually during the PAC application window and more and
more often with georeferenced photos. Once the app shows "declared 2.10 ha /
SIGPAC 2.14 ha", users may ask to fix it from the app. But the filing is
administrative paperwork that differs by community, the kind of thing a gestoría
or advisor does. So **showing** the difference is in scope, and **filing** an
alegación is not.

## Attribution and terms

- Map attribution always visible: `SIGPAC © FEGA (CC BY 4.0)` and/or
  `PNOA © Instituto Geográfico Nacional`, depending on the layers shown.
- Never call `serviciosvisorsigpac` endpoints from the app.
- Cache as much as possible: no published rate limit doesn't mean no limit.
  Bulk needs go to the ATOM/GPKG downloads, not to the query endpoints.
- CC BY 4.0 or Licence Ouverte data inside an AGPL app is fine (data isn't
  code); attribution is the only obligation.

## Sources

- [Nube de SIGPAC — service catalogue](https://sigpac-hubcloud.es/) and
  [FEGA overview page](https://www.fega.gob.es/es/pepac-2023-2027/sistemas-gestion-y-control/sigpac/nube-de-sigpac)
- [Consultas SIGPAC — service description](https://sigpac-hubcloud.es/html/csp/descServicio.html)
  and [example URLs](https://sigpac-hubcloud.es/html/csp/consultas/codigoSigPac.html)
- [MVT service description](https://sigpac-hubcloud.es/html/mvt/descServicio.html)
- [WMS de SIGPAC](https://www.fega.gob.es/es/ayudas-directas-y-desarrollo-rural/aplicacion-sigpac/WMS-de-SIGPAC)
  (`https://wms.mapa.gob.es/sigpac/wms`)
- [FEGA — how to obtain SIGPAC data (ATOM, regional services)](https://www.fega.gob.es/es/content/%C2%BFc%C3%B3mo-puedo-obtener-informaci%C3%B3n-contenida-en-la-base-de-datos-del-sigpac)
- [SIGPAC WMS dataset record, datos.gob.es](https://datos.gob.es/en/catalogo/e0dat0002-servicio-wms-web-map-service-recintos-del-sistema-de-informacion-geografica-de-parcelas-agricolas-sigpac)
- France: [RPG at IGN géoservices](https://geoservices.ign.fr/documentation/donnees/vecteur/rpg),
  [RPG on data.gouv.fr](https://www.data.gouv.fr/datasets/rpg)
- Netherlands: [BRP Gewaspercelen OGC API at PDOK](https://www.pdok.nl/ogc-apis/-/article/basisregistratie-gewaspercelen-brp-)
- EU: [LPIS overview, European Court of Auditors report](https://www.eca.europa.eu/Lists/ECADocuments/SR16_25/SR_LPIS_EN.pdf)
- Base imagery: [PNOA WMTS](https://www.ign.es/wmts/pnoa-ma?request=GetCapabilities&service=WMTS),
  [PNOA dataset + CC BY 4.0 licence](https://datos.gob.es/en/catalogo/e00125901-spaignpnoama)
