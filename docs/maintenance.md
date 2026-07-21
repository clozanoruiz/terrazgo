# External data maintenance — the notebook

> **Purpose.** Once the bulk of initial development is done, most breakage
> will come from outside the repo: a provider moves an endpoint, retires a
> dataset, rotates an encoding, or publishes a new schema version. This file
> is the single place that records every external artifact and service the
> app depends on — where the authoritative copy lives, where our copy lives,
> which tests pin it, and what to check when it fails.
>
> **Upkeep rule:** update this file in the same change that adds, moves or
> removes an external dependency (a vendored file, a network service, an
> official document the code implements). Map-layer *purposes* stay in
> [map-data-sources.md](map-data-sources.md); this file owns endpoints,
> refresh procedures and failure notes.

## Quick triage — something external failed

| Symptom | Look at |
| --- | --- |
| Map blank / `geo_offline` (carries a `{reason}`) / `style_unsupported` | §2 Live services — base maps |
| SIGPAC verify/lookup/zone check fails online | §2 Live services — SIGPAC REST |
| Catalogue tests fail after a snapshot refresh (row counts, encoding tripwire, `siex_mapping` contract) | §1 Vendored — SIEX catalogues (that failure is the *design working*: the provider changed something; read the failing assertion) |
| Export file rejected by the receiving platform | §1 Vendored — CUE JSON Schema (check for a new version first) |
| Website download card empty | §2 Live services — GitHub releases API |
| Boundary import rejects a file that used to work | §3 User-supplied file formats |
| PDF report renders with wrong/missing characters | §1 Vendored — Liberation Sans fonts (and the render warnings — an unknown font family falls back silently except for the warning) |

## 1. Vendored artifacts (external data copied into the repo)

### CUE JSON Schema (the export's contract)

- **Our copy:** `docs/references/cue-schema-3.11.4.json` (byte-exact, never
  reformatted — `docs/references/` is prettier-ignored).
- **Upstream:** embedded as an OLE object inside the Anexo VI docx
  ("Interfaz Único Común") on FEGA's SIEX technical-documentation page:
  <https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex>.
  The docx also embeds the field-semantics xlsx (`BdcSix-DS-DiseñoCUE`,
  sheet `EstructuraCuadernoWS`); where sheet and schema disagree, **the
  schema wins** (it is what validates).
- **Pinned by:** `crates/module-cue/tests/export.rs` validates every export
  against it (the `jsonschema` crate, dev-dependency only).
- **Known quirks:** one malformed `$id` (`"##root/…"` under
  SiembraPlantacion → Maquinaria → items) fails draft-07 meta-validation;
  the test harness normalizes it in its in-memory copy only. `CodigoRea`
  and `CodigoSIEX` are exactly 14 characters.
- **On a new version:** download the new docx, extract the schema from the
  OLE object, vendor it next to (then instead of) the old one, re-diff
  field-by-field (the 3.3.0 → 3.11.4 re-diff in
  [siex-export.md](siex-export.md) is the template), update the export
  serializer/tests, and re-check whether the `##root` typo persists.

### FEGA SIEX catalogues (Anexo VII code lists)

- **Our copy:** `crates/terrazgo-core/catalogues/` — 16 treatment-relevant
  CSVs, idTabla filenames, embedded in the binary via `include_bytes!` and
  imported at startup (`terrazgo_core::catalogue::ensure_catalogues`,
  upsert-only).
- **Upstream:** public no-auth REST API `https://www11.fega.es/bdcsixwsp/` —
  `GET /catalogos/{idTabla}` (one CSV), `GET /catalogos/zip/` (all ~122,
  ≈1.4 MB), `GET /catalogos/{idTabla}/fecha` (last-update probe).
- **Refresh:** a release-ritual step — fetch and replace the 16 files
  byte-verbatim, run the tests. Note (2026-07-21): the `/catalogos/zip/`
  bundle ships *display-name* filenames ("Eficacia del tratamiento.csv"),
  not idTabla names — for a mechanical refresh, fetch each file directly:
  `GET /catalogos/{idTabla}` for the 16 vendored idTabla names.
- **Pinned by:** `crates/terrazgo-core/tests/catalogue.rs` (snapshot facts:
  row counts, retired-code presence, ISO dates, the € labels), a
  control-character **encoding tripwire** (fails the suite if the provider
  moves off Windows-1252/UTF-8), and the bidirectional
  `crates/module-cue/tests/siex_mapping.rs` contract tests (fail when a
  small closed catalogue gains or retires a code — JUSTIFICACION_ACTUACION
  already grew 5 → 6 once).
- **Known quirks:** documented as ISO-8859-1 but really **Windows-1252**
  (€ at 0x80 in UNIDADES_MEDIDA); codes are baja-dated, never deleted;
  crop catalogue is `PRODUCTOS` (not "CULTIVO"); `TIPO_MAQUINA_UNE` is
  string-coded with no lifecycle dates; `BUENAS_PRACTICAS_AMBITOS` repeats
  codes per ámbito.

### SIGPAC service fixtures (test data)

- **Our copy:** `crates/module-sigpac/tests/fixtures/` — real 2026 responses
  (recinto by reference/point, zone intersections, the `/geopackages/`
  campaign listing HTML).
- **Upstream:** the live services in §2. If the provider reshapes a
  response, re-harvest the fixtures from the live service and let the
  parser tests tell you what changed.

### Liberation Sans fonts (embedded in the binary for PDF reports)

- **Our copy:** `crates/terrazgo-report/fonts/` — the four Liberation Sans
  TTF faces (regular/bold/italic/bold-italic, ~1.6 MB) plus the upstream
  `LICENSE` (SIL OFL 1.1), embedded via `include_bytes!`.
- **Upstream:** liberation-fonts releases at
  <https://github.com/liberationfonts/liberation-fonts/releases>
  (v2.1.5 vendored 2026-07-16).
- **Refresh:** only on a demonstrated need (a missing glyph, an upstream
  fix) — fonts change rendering metrics, so a swap can reflow every report.
  Replace the four TTFs + LICENSE together.
- **Pinned by:** `crates/terrazgo-report/tests/render.rs` — the faces must
  parse with typst's own font parser, index under the family name
  `"Liberation Sans"` exactly (what every template's `#set text` matches
  against), and cover the Spanish glyph set (diacritics, `€`, `ª/º`, `¿¡`).

### rustls-platform-verifier Kotlin component (Android TLS)

- **Our copy:** none vendored — the compiled `.aar` ships *inside* the
  `rustls-platform-verifier-android` crate as a bundled Maven repository, and
  `src-tauri/gen/android/app/build.gradle.kts` locates it at build time via
  `cargo metadata`, pinning the exact version Cargo resolved.
- **Upstream:** <https://github.com/rustls/rustls-platform-verifier> (crate
  `rustls-platform-verifier`, workspace-pinned; the Android artifact version
  follows the `-android` sub-crate).
- **Refresh:** automatic on `cargo update` — no manual step. If the crate
  ever changes its bundled-Maven layout, the Gradle finder function is the
  thing to fix.
- **Pinned by:** the Android build itself (Gradle fails if the artifact
  cannot be resolved) and the ProGuard keep rule in
  `src-tauri/gen/android/app/proguard-rules.pro` (the class is only reached
  over JNI, so release shrinking would otherwise strip it — that breakage
  would only show in release APKs, as blank maps).

## 2. Live services (runtime network)

Everything the app fetches at runtime goes through **one seam**:
`terrazgo-geo`'s cache-through fetch, serving the `geo://` protocol. The
allowlisted registry is `crates/terrazgo-geo/src/sources.rs` — a service
not listed there cannot be reached. Once seen, responses are cached, so a
dead provider degrades to "works offline on cached data", never to a broken
app; fresh installs and never-seen areas are what actually break. Service
rule when replacing a source: prefer the most modern, bandwidth-frugal
offering (MVT > WMTS > WMS).

| Service | Endpoints | Consumer | If it dies / notes |
| --- | --- | --- | --- |
| OpenFreeMap (vector base map) | `https://tiles.openfreemap.org/styles/liberty` (style, rewritten in Rust), `/planet` (TileJSON → dated tile URLs), `/fonts/…`, `/sprites/…`, `/natural_earth/ne2sr/…` (backdrop) | `sources.rs` + `style.rs` | Free OSM-tile host with no SLA. Replacement = any MapLibre-style vector provider: new registry entries + a style rewrite in `style.rs`. Tile URLs carry a dated planet snapshot resolved from the TileJSON at fetch time — a stale cached style keeps working because tiles are cached too |
| IGN PNOA (orthophoto base) | `https://www.ign.es/wmts/pnoa-ma` (WMTS, GoogleMapsCompatible) | `sources.rs` | Spanish state provider, stable. Alternative would be another national WMTS or ESA/commercial imagery |
| Nube de SIGPAC MVT (parcel fabric overlays) | `https://sigpac-hubcloud.es/mvt/{layer}@3857@pbf/{z}/{x}/{y}.pbf` — layers `recinto`, `cultivo_declarado`, `e_paisaje_area/_linea/_punto`; previous campaign under `/mvt/anterior/` | `sources.rs` (campaign-keyed cache rows) | z12–15 only; empty tiles answer 404 (cached as empty); the fixed path serves the *current* campaign except `cultivo_declarado` (previous). CC BY 4.0 — attribution must stay |
| SIGPAC REST (lookups + zones) | `https://sigpac-hubcloud.es/servicioconsultassigpac/query` (recinto by ref/point), `…/intersection` (nitrate/phyto/Natura zone checks) | `crates/module-sigpac/src/client.rs` (through the geo seam) | Writes `plot_zone_flag` (stored truth — a dead service stops *new* checks, stored flags and alerts survive). REST responses speak hectares, MVT surfaces are m² |
| SIGPAC campaign resolution | `https://sigpac-hubcloud.es/geopackages/` (directory listing; max year dir = current campaign) | `terrazgo_geo::fetch::current_campaign` | The only machine-readable statement of the campaign; keys every campaign-keyed cache row. If the listing format changes, campaign rollover detection breaks first |
| FEGA BdcSixWsp (SIEX public API) | `https://www11.fega.es/bdcsixwsp/` — `/catalogos/*` (see §1), `/fuentesInformacion/zip` (MDF non-chemical defense registry; ROPO excluded — personal data), `POST /existeNIF` (NIF → explotaciones with SIEX/REA codes) | today: release ritual only (no in-app calls); `/existeNIF` is the future REA-code prefill seam | No auth. Guide: "BdcSixWsp — Guía de Servicios públicos de Siex" (asset of the sede portal, §4) |
| GitHub releases API (website only) | anonymous `releases` endpoint of the public repo | `site/` download card | Drafts are invisible (links fill on publish); `releases/latest` is useless while every release is a pre-release. Static fallback = the releases page |

## 3. User-supplied file formats (offline seams)

| Format | Source the user gets it from | Consumer | Notes |
| --- | --- | --- | --- |
| Boundary files: GeoJSON, GeoPackage | Anywhere — including the SIGPAC download service `https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html` (provincial recinto + cultivos-declarados GPKGs, CC BY 4.0) | `terrazgo_geo::import` | Geographic SRS only (EPSG 4326/4258/4081); projected files fail with `gpkg_unsupported_srs` — the pre-agreed proj4rs reprojection path exists if real projected files appear |
| Cultivos-declarados GPKG (crop prefill, future) | Same download service, current + previous campaign | not built yet | Model: SIGPAC ref + `PARC_PRODUCTO` + `PARC_SISTEXP` + `PARC_SUPCULT` + geometry ([model page](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html)) |
| REACYL DGC Excel export (future) | The titular's own REACYL DGC module (certificate login) | not built yet | Columns/`CodigoDGC` presence unconfirmed (CUECYL question); `.xlsx` reading would need the calamine crate — decide before coding |

## 4. Official documents & regulatory sources

The implementation cites these; when behavior and document disagree, check
whether the document moved to a new version first.

| Document | Where to (re-)fetch | What implements it |
| --- | --- | --- |
| FEGA SIEX technical docs — Anexo V (fields), VI (interface + schema), VII (catalogues), IX/X (authorizations) | <https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex> | the whole export (`module_cue::export`, `module_cue::siex`) |
| "BdcSixWsp — Guía de Servicios públicos de Siex" (v4.5.0 used) | asset of the sede portal SPA at `https://www3.sede.fega.gob.es/bdcsixpor/` | catalogue importer expectations (format, encoding, lifecycle columns) |
| RD 1311/2012 (record content, Anexo III) | <https://www.boe.es/buscar/act.php?id=BOE-A-2012-11605> | treatment record fields, PHI capture |
| RD 34/2025 (electronic-record mandate, 2027) + Reglamento (UE) 2023/564 (+ 2025/2203 postponement) | boe.es / eur-lex.europa.eu | the module's reason to exist; deadline facts |
| RD 1054/2022 (SIEX, REA-first) + resolution BOE-A-2023-13035 | boe.es | REA-first flow, farm identity fields |
| CUECYL / REACYL pages + "Instrucciones declaración DGC" PDF | agriculturaganaderia.jcyl.es | regional submission target; farmer-side DGC paths ([siex-export.md](siex-export.md)) |
| INE province ↔ comunidad autónoma relation | ine.es (codification tables) | `siex::province_to_ccaa` |
| Slippy-map tile scheme (z/x/y ↔ EPSG:3857 bbox) | OSM wiki | tile cache keys; the future WMS grid-snapping |

Contact for the regional submission side: **comercialcuecyl@jcyl.es**
(commercial-notebook test-environment onboarding, Castilla y León).

## 5. Release credentials — Android signing & Google Play

External artifacts a release depends on that deliberately live *outside* the
repo. Added 2026-07-19.

### Android release keystore (the upload key)

- **What it is:** one RSA-2048 keystore signs every release APK/AAB. For
  sideloaded (GitHub-release) APKs it is the app identity itself — Android
  refuses updates signed with a different key. For Google Play it is the
  *upload key* (Play App Signing re-signs with Google's app key, which is why
  a Play install and a sideloaded APK cannot update over each other).
- **Where it lives:** on the development machine, outside the repo, plus an
  offline backup; the password in a password manager. Never committed —
  `gen/android/.gitignore` already covers `keystore.properties`, and the
  keystore file itself must stay out of the working tree.
- **How builds find it:** `src-tauri/gen/android/keystore.properties`
  (untracked: `password=` / `keyAlias=` / `storeFile=`) is read by the
  signingConfig in `app/build.gradle.kts`. Without the file, release builds
  come out unsigned (debug builds are unaffected). CI reconstructs the file
  from the GitHub Actions secrets `ANDROID_KEYSTORE_B64` (base64 of the
  keystore), `ANDROID_KEYSTORE_PASSWORD` and `ANDROID_KEY_ALIAS` in the
  `build.yml` android job.
- **If lost:** sideload users must uninstall/reinstall forever (no key, no
  updates); the Play upload key can be reset through Play Console support
  because Play App Signing holds the real app key. Back it up accordingly —
  the keystore file plus its password are unrecoverable by anyone else.
- **If leaked:** rotate immediately — request a Play upload-key reset, and
  accept the sideload-update break (announce it in the release notes).

### Google Play Console

- **App:** `org.terrazgo.app`, distributed on the **internal-testing track**
  while the project is pre-release; promotion to production is an explicit
  decision, same as the release declaration itself.
- **First upload is manual** (Play requires it): the AAB comes from the
  `build.yml` android job's workflow artifact. Later releases can be pushed
  automatically once a Google Cloud service account with Play release
  permission exists — its JSON key becomes one more Actions secret and one
  upload step in `build.yml`.
- **Recurring Play chores:** target-API deadline (Google raises the required
  `targetSdk` roughly yearly, mid-year — Gradle carries it at
  `app/build.gradle.kts`), data-safety form and privacy policy updates when
  the app starts collecting anything new (today: nothing leaves the device).

## 6. Release checklist (external-data part)

1. **Refresh the catalogue snapshot**: one GET of
   `https://www11.fega.es/bdcsixwsp/catalogos/zip/`, replace the 16 files in
   `crates/terrazgo-core/catalogues/` verbatim, run `cargo test` — the
   snapshot-fact tests, the encoding tripwire and the `siex_mapping`
   contract tests are designed to fail loudly on provider drift instead of
   shipping it silently.
2. **Check the CUE schema version** on the FEGA documentation page. If it
   moved past 3.11.4: vendor + re-diff before the next export-touching
   release (procedure in §1).
3. **Android**: the release APK/AAB must be signed (the CI job fails if the
   keystore secrets are missing — never work around it by shipping unsigned
   or debug-signed builds), and the AAB goes to the Play internal-testing
   track (§5).
4. Glance at this file: does every row still match reality?
