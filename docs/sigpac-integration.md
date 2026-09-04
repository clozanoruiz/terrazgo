# SIGPAC integration & shared mapping — design notes

> Status: research + design exercise (2026-07-05), done while awaiting CUECYL's reply
> on the SIEX export. Three things are designed here because they are separable and
> only one is Spanish: the **shared map UI**, the **country-neutral parcel-provider
> layer**, and the **SIGPAC module** itself.
>
> **Update 2026-07-07 — build-order steps 1–2 are IMPLEMENTED** (schema + map
> workspace with drawing and GeoJSON/GPKG import; the shipped shape is described
> in `docs/architecture.md` → "The map tier"). Two decisions
> taken at implementation superseded drafts below:
>
> - **Geometry storage**: the plot-only `plot_geometry` draft was replaced by a
>   generic **`geo_feature`** core table using the **exclusive-arc pattern** (one
>   nullable FK per subject type — `plot_id`, `farm_id` — + CHECK exactly one set):
>   generic across future subjects (irrigation features, farm boundaries) while
>   keeping real FK enforcement, unlike a polymorphic (subject_table, subject_id)
>   pair. Provider attributes (land use, slope, irrigation coefficient…) go in a
>   source-tagged `properties` JSON column, not typed columns.
> - **Import formats**: GeoJSON **and GeoPackage** shipped in the first pass (GPKG
>   is SQLite — rusqlite + geozero, no sqlx). UTM-projected GPKGs are rejected with
>   a stable error until the proj4rs contingency is proven needed by a real file.
>   KML is conditional on the visor lacking GPKG export (unchecked).
>
> Steps 3+ (SIGPAC lookup/prefill, zone flags) remain future work; `module-sigpac`
> does not exist yet. The `geo://` protocol, cache and source registry it will plug
> into are live.

## What SIGPAC is, and what the module buys us

SIGPAC (Sistema de Información Geográfica de Parcelas Agrícolas) is Spain's LPIS —
the EU-mandated Land Parcel Identification System every member state runs as the
geographic backbone of CAP aid. Terrazgo already stores the full 7-part SIGPAC
reference on `plot_es_extension` (`sigpac_province … sigpac_enclosure`), typed by
hand today. The module turns that dead text into:

1. **Reference validation + prefill** — on plot entry, one lookup returns the
   recinto's official surface (compare/prefill `area_ha`), land use, slope,
   irrigation coefficient, and geometry. Typos in a 7-part code become visible
   immediately instead of at inspection time.
2. **Plot map** — the farm's recintos drawn over orthophoto. First visible "whole
   farm app, not a form app" feature.
3. **GPS point → recinto** (mobile) — "which recinto am I standing on" while
   recording a treatment in the field.
4. **Zone intersections → compliance alerts** — SIGPAC's query service reports
   whether a recinto intersects nitrate-vulnerable zones (feeds the fertilisation
   module's mandatory-record rule), phytosanitary restriction zones (feeds CUE),
   and Natura 2000. This is the quiet high-value item: it connects geography to
   the compliance engine we already have (`refresh_alerts`).
5. Later: cadastral cross-walk (SIGPAC code → referencia catastral) and offline
   municipality packs.

## Service inventory (what FEGA actually offers)

FEGA runs two families of services. The distinction matters legally:

- **Visor services** (`sigpac.mapa.gob.es/fega/serviciosvisorsigpac/…`) — power the
  official viewer. **Third-party application access is explicitly not permitted.**
  Never call these, even though blog posts document them.
- **"Nube de SIGPAC"** (`sigpac-hubcloud.es`) — launched precisely "to serve as a
  basis for the development of computer applications". This is our surface.
  Licence: **CC BY 4.0** (recintos + landscape elements are official "high-value
  datasets"). No authentication; no published rate limits (be polite: cache).

| Service | Protocol / format | What it gives | Terrazgo use |
| --- | --- | --- | --- |
| **Consultas SIGPAC** | REST, JSON/GeoJSON | 11 endpoints: recinto info by code or by coordinates, centroid, cadastral ref by code, all recintos of a parcel, intersections (Natura 2000, nitrate, phytosanitary, montanera, permanent pasture) | **The workhorse.** Validation, prefill, GPS lookup, zone checks |
| **Teselas vectoriales (MVT)** | XYZ tiles, PBF z12–15 (GeoJSON z15), EPSG:3857 | Recintos, declared crops, landscape elements; current + previous campaign | Recinto boundaries on the map without bulk downloads |
| **WMS** | ISO 19128 / INSPIRE, GeoServer | Rendered recintos + the SIGPAC orthophoto | Fallback imagery; MVT preferred for vectors |
| **OGC API Features** | OGC API, GeoJSON, WGS 84 | Recintos / declared crops / landscape elements as features | Alternative to Consultas for bbox queries; same data, more standard |
| **Listas de códigos** | REST, JSON | Provinces, municipalities, SIGPAC land-use codes… | Dropdowns + validation of the reference parts |
| **Descargas** | GeoPackage per province | Bulk current-campaign data | Offline packs (GPKG **is** SQLite — rusqlite reads it directly) |
| **ATOM (INSPIRE)** | Atom feeds → Shapefile/GPKG per municipality | Recintos, landscape elements, declaration lines; ~39.5 GB national total | Municipality-sized offline packs (the right granularity for a farm) |
| Salidas gráficas | — | Printable map sheets | Not needed (paperless lean) |

Example of the key endpoint (the URL path *is* our stored reference):

```
https://sigpac-hubcloud.es/servicioconsultassigpac/query/recinfo/
        {province}/{municipality}/{aggregate}/{zone}/{polygon}/{parcel}/{enclosure}.geojson
```

Returns geometry (WKT/GeoJSON + EPSG), surface (ha), land use, slope, irrigation
coefficient, admissibility, incidences.

**Base imagery for the map:** IGN's PNOA orthophoto WMTS
(`https://www.ign.es/wmts/pnoa-ma`, INSPIRE WMTS, CC BY 4.0, attribution
"PNOA cedido por © Instituto Geográfico Nacional"). SIGPAC's own WMS ortho is the
alternative. Data cadence: SIGPAC is campaign-based (annual, refreshed ~February);
cached data must record its campaign year and offer refresh at rollover.

Castilla y León side note: ITACYL republishes SIGPAC for CyL (bulk FTP) — a useful
mirror for dev/testing, but the national services are the product path.

## The EU landscape — why the provider layer is country-neutral

Every member state has an LPIS, most publish it openly, **but the capabilities
differ**, so the abstraction must be capability-based, not SIGPAC-shaped:

| Country | System | Access | Licence | Lookup by farmer-known reference? |
| --- | --- | --- | --- | --- |
| Spain | SIGPAC (FEGA) | REST query, MVT, WMS, OGC API, GPKG/ATOM | CC BY 4.0 | **Yes** — the 7-part code is public |
| France | RPG (ASP → IGN) | WFS/WMS/WMTS + annual bulk downloads | Licence Ouverte 2.0 | **No** — anonymised îlots/parcelles; farmers know their parcels only via their own PAC dossier |
| Netherlands | BRP Gewaspercelen (RVO → PDOK) | OGC API Features, WFS, downloads | open (PDOK) | No stable public per-farmer ref; bbox/point queries work well |
| Others | Luxembourg, Denmark, Austria… publish LPIS via INSPIRE portals | mixed (WFS/Atom typical) | mixed open | varies |

Consequences for the design:

- **Point/bbox query and map display generalise** (every LPIS can do "what parcel
  is here" one way or another). **Reference validation does not** — it is a Spanish
  luxury. The trait exposes capabilities; the UI shows only what the active
  provider supports.
- The **plumbing** (HTTP fetch, response cache, tile cache, attribution handling,
  campaign/version tagging) is 100% shareable. A new country = one provider
  implementation, mostly URL templates + field mapping.

## Proposed architecture — three pieces

Respecting the existing rule: **no network calls in core or CUE, ever.** All HTTP
lives in the new integration tier; everything it fetches is cached; the app remains
fully functional offline (features degrade to "cached or manual data only").

### 1. Shared map UI (shell tier, not a module)

- `MapView.svelte` in `src/lib/` wrapping **MapLibre GL JS** (the stack-table choice,
  reconfirmed: native vector-tile rendering fits SIGPAC's MVT service directly,
  WebGL performance in the WebView, no licence fees; OpenLayers would only win if
  we needed exotic projections — SIGPAC MVT is standard EPSG:3857).
  First npm runtime dependency — **decision needed**.
  *Re-examined 2026-08-12 against a raster/GeoTIFF case; verdict unchanged, and
  the reasoning (including the one thing MapLibre genuinely cannot do) is in*
  [stack-choices.md](stack-choices.md) *§1.*
- Any module embeds it: CUE (treated plots), irrigation, crop planning, sensors.
  Layers as data, same philosophy as `nav.js`.
- The webview never talks to the internet. MapLibre requests tiles from a custom
  Tauri protocol (e.g. `geo://tiles/…`) served by Rust; production CSP stays
  `default-src 'self'` + the custom scheme. This one seam gives: transparent
  offline cache, attribution enforcement, and no per-service CSP holes.
- **Geometry entry is not SIGPAC-only** (decision 2026-07-05): the map
  milestone includes **manual drawing** (candidate plugin: `terra-draw`, the one
  MapLibre's own docs use — npm dep, decision needed) and **boundary-file import**
  (GeoJSON first — zero new deps; KML/GPX from GPS tools later, each a small
  parser decision). Serves refs SIGPAC can't resolve and non-ES users alike.
- **Free base-map fallback for countries without an open ortho**: OpenFreeMap
  (OSM-based vector tiles; no API key, no usage limits, attribution auto-added by
  MapLibre) cached through the same Rust tile cache as everything else. Each
  provider can add its country's official imagery on top (ES: PNOA; FR: IGN).

### 2. `crates/terrazgo-geo` — provider layer + plumbing (shared crate)

- **`ParcelProvider` trait** (capability-based; sketch, not final):

  ```rust
  trait ParcelProvider {
      fn country(&self) -> &str;                       // "ES"
      fn capabilities(&self) -> Capabilities;          // ref_lookup | point_query | tiles | zones | bulk
      fn parcel_by_reference(&self, r: &ParcelRef) -> Result<ParcelInfo, GeoError>;
      fn parcel_by_point(&self, lon: f64, lat: f64) -> Result<ParcelInfo, GeoError>;
      fn zone_checks(&self, r: &ParcelRef) -> Result<Vec<ZoneFlag>, GeoError>;
  }
  ```

- HTTP client (**crate decision needed** — `reqwest` if we want async/concurrent tile
  prefetch, `ureq` if we keep it minimal and synchronous; recommendation: decide at
  implementation time, start with `ureq` for the query endpoints, revisit for tiles).
- **Tile + response cache in a separate SQLite file** (`geo-cache.db`, essentially
  the MBTiles schema). Deliberately *not* the user database: tiles are bulky,
  re-fetchable, derived — they must not bloat `VACUUM INTO` backups or the
  `record_change` log. Cache entries carry campaign year + fetch date.
- Geometry conversion WKT/WKB → GeoJSON: `geozero` (+ `geo-types`) — **decision
  needed**; also what we'd use to read GeoPackage geometry blobs via rusqlite.

### 3. `crates/module-sigpac` — the Spanish provider (a normal module)

- Implements `ParcelProvider` against sigpac-hubcloud; owns the SIGPAC-specific
  commands (validate reference, fetch recinto, zone checks, download municipality
  pack) and any SIGPAC lookup tables (land-use codes).
- Registered like module-cue; migrations (if any) join the composed sequence.
- CUE and fertilisation *consume its outputs through core data* (stored zone flags,
  stored geometries) — modules still don't depend on each other.

### Geometry storage (high-stakes — schema draft, not yet decided)

Fetched geometry attached to a user's plot is **user data** (syncs, backs up,
audit-logged) — unlike tiles. Proposal: a new core table rather than columns on
`plot`:

```
plot_geometry (
    id            TEXT PK,          -- UUIDv7
    plot_id       TEXT NOT NULL REFERENCES plot(id),
    geometry      TEXT NOT NULL,    -- GeoJSON, EPSG:4326
    source        TEXT NOT NULL,    -- 'sigpac' | 'manual' | 'import' | future providers
    campaign      INTEGER,          -- SIGPAC campaign year, NULL for manual
    official_area_ha REAL,          -- as returned; never overwrites plot.area_ha
    fetched_at    TEXT,
    deleted_at    TEXT
)
```

Rationale: keeps `plot` country-neutral and lean; allows manual drawing (farmers
with no usable reference, or non-ES countries) as a first-class source; official
surface stored alongside so the UI can show "declared 2.10 ha / SIGPAC 2.14 ha"
without silently mutating user input (same snapshot philosophy as treatments).
Alternative (nullable columns on `plot`) is simpler but bakes single-source-single-
version in. Table lives in core (a boundary is universal), *filled* by providers.

## Zone flags — what they are, and storage options (decided 2026-07-05: Option B)

SIGPAC's query service can report, for a recinto, whether it intersects official
regulatory zone layers. The ones that matter to Terrazgo:

- **Nitrate-vulnerable zones** (Directive 91/676/CEE, declared per CCAA): a plot
  inside one makes **fertilisation records mandatory** and caps nitrogen — this is
  the legal trigger the fertilisation module needs per plot.
- **Phytosanitary restriction zones**: plant-health demarcated areas (quarantine
  pest buffers, treatment obligations/bans) — constrains CUE treatments.
- **Natura 2000**: PAC conditionality restrictions on operations.
- Montanera / permanent pasture: aid-eligibility categories, lower priority.

Their nature drives the design: they are facts about geography that change rarely
(zone declarations are revised every few years; SIGPAC republishes per campaign),
they change what records are *legally required*, and — critically — they **cannot
be re-derived offline**: they come from a network query. "Was this plot flagged in
campaign 2027?" is an inspection-grade question.

**Option A — boolean columns on the plot** (`in_nitrate_zone`,
`in_phyto_zone`, … + `zones_checked_at`, on `plot_es_extension`).

- Pros: simplest possible; no joins; trivially readable in the plot form.
- Cons: fixed set — every new zone type, or every new country's zones, is a
  migration; refresh overwrites, so no history ("was it flagged then?" becomes
  unanswerable); no room for detail (zone identifier, partial overlap); the
  columns are inherently country-flavoured.

**Option B — a `plot_zone_flag` table**: one row per (plot, zone-type code,
campaign) with status, zone identifier, source, `checked_at`.

- Pros: open set of zone types — a new type or country is new *rows* and a new
  code, never a migration (France's equivalents would just be codes); history for
  free — refresh at campaign rollover *appends*, never overwrites, so past duties
  stay provable; room for detail; matches the existing lookup-code conventions.
- Cons: a join wherever flags are read; more repository code; needs an explicit
  refresh/dedup rule (one row per plot+type+campaign).

**Option C — don't store, query on demand** (recompute like alerts).

- Pros: never stale.
- Cons: dead on arrival — a compliance trigger that requires connectivity fails
  exactly where farms are. Alerts can be excluded from storage-as-truth because
  they re-derive from *local* tables; zone flags re-derive from a *remote*
  service. Listed only to show why storage is required.

Sync/audit consequence, whichever option wins: since another device cannot
re-derive flags offline, they must sync — i.e. be `record_change`-logged like
fetched plot geometry (fetched-user-data), **unlike** alerts (excluded because
each device re-derives them locally).

**Decision (2026-07-05): Option B** — `plot_zone_flag` table,
`record_change`-logged, append-per-campaign. The alert engine then reads
`plot_zone_flag` like any other source table. (Exact columns still to be
finalised when the module is scheduled.)

## Build order (1–4 + the MVT overlay shipped; remaining: 5–6)

1. ~~**Schema design**~~ **DONE** — shipped as `geo_feature` (exclusive arc,
   GeoJSON, `properties` JSON), superseding the `plot_geometry` draft; see the
   update note at the top.
2. ~~**Map view + manual geometry**~~ **DONE** — MapCanvas/MapView + layer
   registry, OpenFreeMap + PNOA through the `geo://` cache, drawing
   (terra-draw) and GeoJSON **+ GPKG** import (pulled forward from step 6's
   format).
3. ~~**SIGPAC lookup v1**~~ **SHIPPED 2026-07-08**: `module-sigpac`
   (reference/models/client/storage/service; all HTTP through terrazgo-geo's
   `cached_resource`, so every lookup caches and works offline afterwards),
   three shell commands, and the scope extended the same day —
   SIGPAC as a plot **entry** path, three doors into the one plot-creation
   flow: (A) type-first verify/prefill in the plot form (auto-stores the
   official boundary after save from the cached lookup), (B) map-first
   `recinfobypoint` on click → create plot or attach to the matching plot,
   (C) import-first "create plot from recinto" in the GPKG/GeoJSON picker;
   dedup by SIGPAC ref offers "attach to existing plot" instead of
   duplicating. Official area lives in `geo_feature.official_area_ha`
   (`campaign` NULL until the code-lists service is wired), full attributes
   in `properties`, discrepancy shown on plot cards. Filing alegaciones
   stays out of scope. Verified end-to-end 2026-07-08: 23/23 scripted frontend checks
   and 7/7 app-level checks with the real backend and live network (real
   recinto fetched, stored, deduped, cache-served, rendered in the real
   window); the frontend test fixtures regenerated from that harvest.

   **Pre-flight checks — resolved 2026-07-08:**
   - (a) The visor exports **GeoJSON, GML, Shapefile** — no GPKG, but also no
     KML, and GeoJSON we already import. **KML is dropped from the roadmap**
     (only a GPS-tool need could revive it).
   - (b) Real Nube GPKGs are **geographic, EPSG:4258** (verified in the 2026
     León elementos-del-paisaje file and the header of the Guipúzcoa recintos
     file: only GEOGCS ETRS89/WGS84/REGCAN95 definitions, zero UTM/PROJCS).
     **The `proj4rs` contingency stays dormant for SIGPAC files.** Canary
     files declare **REGCAN95 (EPSG:4081)** — accepted in the importer's
     identity allowlist since 2026-07-08 (the EPSG-registered REGCAN95→WGS84
     transformation is 0,0,0, so identity is correct, not approximate).
     **Reprojection decision (2026-07-08): hold off.** Other
     Spanish public data does use projected CRS (ETRS89 UTM 25828–31,
     REGCAN95 UTM 4083, INSPIRE 3034/3035, WGS84 UTM 32628–31), and the
     agreed future path is a proj4rs-backed EPSG→proj-string registry in
     terrazgo-geo covering exactly that list (proj4rs vetted: pure Rust,
     lcc/laea/etmerc + towgs84, MIT/Apache-2.0; georust `proj` rejected —
     C libproj + proj.db resource, painful for mobile cross-compilation;
     ED50 23028–31 explicitly excluded). Until then projected files keep
     failing with the stable `gpkg_unsupported_srs` error.
   - (c) sigpac-hubcloud is live on campaign **2026** (files dated 2025-12,
     two campaigns kept), CC BY 4.0, no auth, responses gzip. Live-tested:
     `query/recinfo/{7-part}.json|.geojson`,
     `query/recinfobypoint/4326/{lon}/{lat}.json`, and
     `intersection/{nitratos|fitosanitarios|red_natura}/{7-part}.json`.
     Attributes returned: superficie (ha), pendiente_media, coef_regadio,
     uso_sigpac, admisibilidad, incidencias, region, altitud, wkt + srid.
     Intersections return `surface_intersection` (m²) + `surface_tpc` (%)
     (+ `descripcion`, e.g. "Zona periférica" for fitosanitarios).
     **Unknown ref → HTTP 200 with `[]`**, not 404 — "not found" must be
     detected from the empty array. Bulk GPKG downloads are a plain directory
     listing under `https://sigpac-hubcloud.es/geopackages/{campaign}/recintos/`
     (province files 112–545 MB — step 6 material).
4. ~~**Zone intersections**~~ **SHIPPED 2026-07-08**: `zone_type` +
   `plot_zone_flag` in core (columns settled same day; negatives stored as
   proof-of-check; replace-within-campaign, append-across-campaigns,
   `record_change`-logged). Campaign year comes from the provider's
   `/geopackages/` directory listing (max year — no campaign endpoint exists
   in the consultas or code-lists services). Zone checks are folded into
   plot verification (one tap = boundary + zones; a zone failure never
   undoes the stored boundary). Alerts: `nitrate_zone` / `phyto_zone` /
   `natura_zone`, candidate = the latest campaign's 'inside' flag, subject =
   the PLOT (dismissals survive re-checks and rollovers), due date =
   campaign year end. The fertilisation module's trigger reads
   `plot_zone_flag` from core — no dependency on module-sigpac, as designed.
   ~~**MVT recinto overlay**~~ **SHIPPED 2026-07-11**: the recinto fabric as
   a toggleable vector-tile layer over both base maps — `sigpac-recintos` in
   terrazgo-geo's source allowlist + a vector entry in `mapLayers.js`, tiles
   through the `geo://` cache, attribution `SIGPAC © FEGA (CC BY 4.0)` while
   active. Service facts established against the live service and a real
   tile (2026-07-11): pbf at z12–15 only; single source-layer **`recinto`**
   (attribute keys = the recinfo set); **the URL carries no campaign year**
   (current campaign at the fixed path, previous under `/mvt/anterior/`), so
   cache rows are campaign-keyed (`sigpac-recintos@{campaign}`, resolved from
   the shared campaigns listing; old campaign evicted on first new-campaign
   store); **empty tiles answer HTTP 404**, cached and served as empty
   payloads so they cost nothing on repeat visits and stay empty offline.
   `current_campaign` moved from module-sigpac's client into
   `terrazgo_geo::fetch` (re-exported) because tile caching below the module
   tier needs it.

   ~~**Declared-crops prefill**~~ **SHIPPED 2026-08-03**: the PAC graphical
   declaration (`cultivo_declarado`) as a reviewed crop prefill — see the
   section below.

5. ~~**GPS point query**~~ **SHIPPED 2026-07-23**: recinto under the device's
   position — see "Standing on a plot" below.
6. **Offline municipality packs** (ATOM/GPKG) — only if field usage shows the
   online cache isn't enough; the GPKG reader from step 2 already does the
   hard part.

Steps 1–3 are useful with zero EU generality; the trait can even be extracted
*after* the SIGPAC implementation works (rule of three: abstract when France is
real, not before — but keep the seams from day one: no `sigpac_` names in the
shared crate or the map component).

## Declared crops — "load my crops" (shipped 2026-08-03)

The PAC graphical declaration says what each recinto was declared as growing.
That is the crop list a farmer would otherwise retype, and it is public: FEGA
publishes it as `cultivo_declarado` on the Nube de SIGPAC **OGC API Features**
endpoint (`https://sigpac-hubcloud.es/ogcapi`, CC BY 4.0, no auth). With the
plot fabric already coming from the verify flow, this closes the "the farmer's
own data without a regional registry account" story.

### Why OGC API and not the MVT twin

The map rule is "MVT > WMTS > WMS", but that rule is about *display*. This is a
**record-grade attribute lookup keyed by an identifier**, the same split the
repo already makes for recintos: displayed through the MVT overlay, looked up
through the consultas service by reference. The decisive argument is cache
durability, not bandwidth — `cached_resource` writes to the `resource` table,
which is **never evicted**, while tiles live under a 512 MiB LRU. Crop data
read out of tiles could silently stop resolving offline after enough panning.
Regulatory-adjacent data is cached by identity, not by viewport. (The consultas
REST service has no declared-crops operation among its eleven, so the OGC API
is also the only per-reference channel.)

### Service facts (live-probed 2026-08-02, re-verified 2026-08-03)

- Collection `cultivo_declarado`; the seven reference parts **and `exp_ano`**
  are queryables accepted as plain query parameters, so one recinto costs one
  ~3,7 kB request. No bbox mode, no paging concerns.
- **`exp_ano` is filterable but omitted from item responses.** The campaign a
  line belongs to is therefore the campaign that was asked for — it can never
  be read back from the feature.
- The service runs **one campaign behind**: with the campaigns listing already
  naming 2026, only `exp_ano=2025` answers.
- Nothing declared = **HTTP 200 with `numberMatched: 0`**, never a 404.
- Attributes read: `parc_producto` (PRODUCTOS code), `cultsecun_producto`
  (secondary crop), `parc_sistexp` (`"S"` secano / `"R"` regadío, both
  observed live), `parc_supcult` (**square metres** — the same m² trap as the
  MVT layer). Also present and unused: aid lines, expediente identity,
  `parc_indcultapro`, `tipo_aprovecha`.
- A recinto commonly carries **several declaration lines** (20 of 228 in a
  live Valladolid bbox sample) — typically the same crop split by irrigation
  system. Fixtures cover this case.

### The campaign fallback, and why the two campaigns are trusted differently

The current campaign is asked first and the previous one is the fallback, and
the campaign that answered travels with the lines. Cache keys are
`sigpac/cultivos/{campaign}/{ref}`, so a rollover writes new rows instead of
overwriting old ones.

For the **current** campaign a cached empty answer is trusted only for the rest
of the UTC day it was fetched on. It may predate the day FEGA loaded that
campaign, and a permanently cached "nothing declared" would hide the
declaration for the rest of the year — but the staleness that matters is
measured in *months*, so asking more often than daily buys nothing against it.
Day granularity matches the tile cache's once-per-UTC-day `last_used_at` touch,
and the unit is the right one: a campaign is loaded on some *day*. For the
**previous** campaign the cache is authoritative, empties included — that
dataset is closed. "SIGPAC has no declaration for this plot" is reported only
when both campaigns actually answered; if neither could be reached, the network
failure surfaces instead, because silence from an unreachable service is not
evidence of an empty declaration.

Three consequences of that asymmetry, all deliberate:

- **The two buttons mean different things.** "Cargar cultivos declarados" is
  cache-first and normally silent; "Actualizar desde SIGPAC" bypasses the day
  and re-asks both campaigns. A UI offering both should not have them do the
  same work, which is what an unconditional re-ask on every load made them do.
- **The daily re-ask costs one request per plot on the first load of the day**,
  for as long as the current campaign answers empty — that is the price of not
  hiding a declaration that appears mid-campaign, bounded to once a day rather
  than once a click.
- **A plot that could not be asked about is reported, never fatal.** It lands
  in `plots_unreachable` with the reason beside it, so the rest of the panel
  still works — offline, a plot with no declaration in either campaign refuses
  to answer by design, and a farm with one pasture outside the PAC declaration
  is the ordinary case, not an edge one. Not knowing and knowing there is
  nothing stay separate lines in the UI, the same distinction
  `plot_zone_flag`'s stored negatives make.

### What the import may and may not do

Nothing is written without review. Proposals are built read-only, every row is
opt-in and editable, and the writing happens through core's crop repository in
a separate confirmed step. Rows come in five kinds:

| Kind | When |
| --- | --- |
| `insert` | the plot records no crops (pre-selected — the everyday case) |
| `insert_secondary` | the line declares a `cultsecun_producto` (never an update: it is a second crop, not a correction) |
| `update` | the plot's **single** crop differs, has **no treatments this season**, and the declaration has **one** main line |
| `already_recorded` | a recorded crop matches, by catalogue code or by name |
| `blocked` | `multi_crop`, `has_treatments` or `multi_line` — shown so the discrepancy is visible, never applied |

The treatment guard is not about protecting past records: `treatment_plot`
froze the species and variety at write time, so those are safe from any crop
edit. It is about section 2.1 and section 3.1 agreeing — rewriting a crop a
treatment points at would make the book state one crop and print another beside
the treatment. The guard input comes from module-cue via the shell; the two
modules never call each other.

Two mapping rules worth keeping:

- `parc_sistexp` `"S"` prefills `rainfed`; `"R"` prefills **nothing**. SIGPAC
  says the crop is irrigated but not by which system, and Anexo III A.2.e wants
  the system (SEC/ASP/LOC/GRA) — naming one would be inventing a fact.
- An unresolvable `parc_producto` keeps the code and blanks the name. The code
  is the payload, the catalogue label is display metadata.

Every proposal row and the confirm button state the **answering campaign**
("declaración PAC campaña 2025 → temporada 2025/2026"). Recording last year's
declaration as this year's crop without saying so is the one way this feature
could do harm.

### Provenance, and the species picker

An imported crop carries `source = 'sigpac'`, `source_campaign` and
`declared_area_ha` (beside `area_ha`, never instead of it). `UpdateCrop` treats
those three as set-if-present, so an unrelated manual correction cannot erase
where a row came from.

`crop.crop_code` also lets manual entry speak the catalogue's language: the
crop form's species field is a type-ahead over PRODUCTOS, narrowed by the
plot's verified `uso_sigpac` through the vendored `CULTIVO_USO_SIGPAC`
catalogue. The narrowing degrades to the full list whenever it cannot be
trusted — no plot, no verified boundary, or a land use nothing matches — since
a filter that hides everything is worse than no filter. Free text stays valid;
it simply carries no code.

## Standing on a plot — GPS lookup and live tracking (shipped 2026-07-23)

Build-order step 5 asks "which recinto am I standing on". The backend answer
has existed since step 3 — `module_sigpac::client::recinto_by_point` — so the
feature is a GPS fix substituted for a map click, plus the frontend wiring.

`tauri-plugin-geolocation` (floor `2.3`, the minor whose `get_current_position`
and `request_permissions` shapes the code targets) is **mobile-only**: it has no
desktop implementation, so the dependency is target-gated, the builder
registration is `#[cfg(mobile)]`, and the ACL lives in a platforms-gated
`capabilities/mobile.json`. It injects its own Android manifest permissions
through the manifest merger, so the generated project needs no hand-edit.

Two design rules are worth keeping.

**Gate the UI on the platform, never on probing a command.** The first build
asked the plugin whether it was there by calling `check_permissions` and
treating a rejection as absence. That lasted one field test: the plugin rejects
with "Location services are disabled." when the device's location toggle is off,
so a rejection does not mean absence, and the whole feature silently vanished on
a phone with GPS switched off. The controls now gate on the `is_mobile` command
(`cfg!(mobile)` — compile-time truth). Generally: never gate a feature on
probing a command that can fail for reasons other than the one being tested.

**Geolocation failure is not a backend error.** A denied permission or a dead
GPS is reported locally (`map.locate_denied` / `map.locate_error` plus the raw
reason), never through the command error boundary — a phone indoors is not a
malfunction.

Live tracking rides the same plumbing: a follow toggle streams `watch_position`
fixes over a Tauri channel into a dot-and-accuracy marker. The accuracy circle
is drawn as a ground-true 64-point polygon rather than a MapLibre circle layer,
whose radius is in screen pixels and would not scale with zoom. The camera
follows only the *first* fix — re-centring on every fix would fight the user's
own panning. The watch stops on toggle-off and on view unmount, so GPS never
outlives the map.

MapLibre's built-in `GeolocateControl` was rejected: it uses
`navigator.geolocation`, which is a second permission path outside the plugin
and outside the ACL.

The button that runs the lookup sits in the map's SIGPAC panel, so it appears
only for Spanish farms — it answers a Spanish question. The follow toggle is in
the map toolbar instead, because a position on a map is provider-agnostic.

## Attribution & terms checklist

- Map corner attribution, always visible: `SIGPAC © FEGA (CC BY 4.0)` and/or
  `PNOA © Instituto Geográfico Nacional` depending on active layers.
- Never call `serviciosvisorsigpac` endpoints from the app.
- Cache aggressively; no published rate limits ≠ no limits. Bulk needs → ATOM/GPKG
  downloads, not endpoint hammering.
- CC BY 4.0 / Licence Ouverte data inside an AGPL app: no conflict (data ≠ code);
  attribution is the only obligation.

## Open questions — all resolved 2026-07-05

1. ~~Manual geometry drawing in scope?~~ **Yes** — drawing + boundary-file import
   + free base-map fallback are part of the map milestone (folded into the
   sections above).
2. ~~Zone flags storage?~~ **Option B** — `plot_zone_flag` table (see its section
   above for the trade-offs that decided it).
3. ~~SIGPAC alegaciones?~~ **Split decided.** An alegación is the formal request a
   farmer files with their autonomous community to *correct SIGPAC itself* when
   it misrepresents reality (wrong land use, boundary, irrigation coefficient…),
   normally within the PAC application window and increasingly with
   georeferenced photo evidence. The moment Terrazgo shows "declared 2.10 ha /
   SIGPAC 2.14 ha", users will ask to "fix it from here" — but the filing is
   per-CCAA administrative paperwork (gestoría/advisor territory). Decision:
   **displaying** discrepancies is in scope (nearly free once the lookup
   exists); **filing** alegaciones is out of scope.

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
