# Maintenance

Most breakage will come from outside the repo: a provider moves an endpoint,
retires a dataset, changes an encoding or publishes a new schema version. This
file lists every external file and service the app depends on: where the
official copy is, where ours is, which tests check it, and what to look at when
it fails. It also holds the release checklist, the Android build notes and the
recipes for re-running the measurements.

**Keep it current:** update it in the same change that adds, moves or removes
an external dependency (a vendored file, a network service, an official
document the code implements). What each map layer is *for* is in
[map-data-sources.md](map-data-sources.md); endpoints, refresh steps and
failure notes are here.

## Quick triage: something external failed

| Symptom | Look at |
| --- | --- |
| Map blank / `geo_offline` (carries a `{reason}`) / `style_unsupported` | §2 Live services: base maps. If *everything* fails at once, see "Behind a corporate proxy" |
| SIGPAC check/lookup/zone check fails online | §2 Live services: SIGPAC REST |
| Catalogue tests fail after a snapshot refresh (row counts, encoding tripwire, `siex_mapping` contract) | §1 FEGA SIEX catalogues. The tests are doing their job: the provider changed something, so read the failing assertion |
| Export file rejected by the receiving platform | §1 CUE JSON Schema (check for a new version first) |
| Website download card empty | §2 Live services: GitHub releases API |
| Boundary import refuses a file that used to work | §3 User-supplied file formats |
| PDF report shows wrong or missing characters | §1 Liberation Sans fonts (and the render warnings: an unknown font family falls back silently, apart from the warning) |

## 1. Vendored files (external data copied into the repo)

### CUE JSON Schema (the export's contract)

- **Our copy:** `docs/references/cue-schema-3.11.4.json`, byte for byte, never
  reformatted (`docs/references/` is excluded from prettier).
- **Upstream:** embedded as an OLE object inside the Anexo VI docx
  ("Interfaz Único Común") on FEGA's SIEX technical documentation page:
  <https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex>.
  The docx also embeds the field-description xlsx (`BdcSix-DS-DiseñoCUE`,
  sheet `EstructuraCuadernoWS`). Where the sheet and the schema disagree, **the
  schema wins**, because it is what validates.
- **Checked by:** `crates/terrazgo-siex/tests/common/mod.rs` compiles it once
  per test binary, and every `export*.rs` file validates its output against it
  (the `jsonschema` crate, dev-dependency only).
- **Known quirks:** one malformed `$id` (`"##root/…"` under
  SiembraPlantacion → Maquinaria → items) fails draft-07 meta-validation; the
  tests fix it in their in-memory copy only. `CodigoRea` and `CodigoSIEX` are
  exactly 14 characters.
- **On a new version:** download the new docx, extract the schema from the OLE
  object, vendor it next to the old one (then in its place), compare field by
  field (the 3.3.0 → 3.11.4 comparison in [siex-export.md](siex-export.md) is
  the model), update the export serializer and tests, and check whether the
  `##root` typo is still there.
- **Extract the description sheets again in the same pass, and never read an
  old one.** They are inside the docx and move with it, but once extracted they
  are separate files and nothing makes an old copy look old. Keep the version
  banner (the first row) when converting a sheet; it is the only thing that
  shows which version a sheet is. When the sheet and the schema disagree, go by
  the schema rather than guessing which version changed what: the disagreement
  is often current, not historical (`ActividadCubierta` is in every sheet and
  in no schema).

### FEGA SIEX catalogues (Anexo VII code lists)

- **Our copy:** `crates/terrazgo-core/catalogues/`: one CSV per catalogue,
  named by idTabla, embedded in the binary with `include_bytes!` and imported at
  startup (`terrazgo_core::catalogue::ensure_catalogues`, upsert only).
- **Upstream, the registry:** `GET
  https://www3.sede.fega.gob.es/bdcsixpor/tablas/configJson`. This is the SIEX
  portal's own list of catalogues and **the authority on what exists** (287
  catalogues). Each has an `id` (the idTabla), `nombreCatalogo`,
  `visiblePortal`, `exportable` and a `fields` map with every column's `orden`,
  `type`, `length`, `required` and display `label`. That metadata settles a
  file's `code_col`/`label_col`/`identity_attrs` without guessing: the field
  named `codigo` (or `codigoPadre`) is the code column, `descripcion` is the
  label, and `numCamposClavePrimaria > 1` means the code alone isn't unique.
- **Upstream, the data:** public REST API with no authentication,
  `https://www11.fega.es/bdcsixwsp/`: `GET /catalogos/{idTabla}` (one CSV) and
  `GET /catalogos/{idTabla}/fecha` (date of the last update). `GET
  /catalogos/zip/` exists but **can't be used by a script**: the files in it
  are named by display name ("Eficacia del tratamiento.csv"), not by idTabla.
- **Which ones we carry:** a catalogue is vendored when a named part of the app
  reads it (the record book's coded fields, the declared-crops prefill, the
  geography that the export and the report language resolve against, the
  fertilisation and irrigation vocabularies), or when it is held for a named,
  written-down future reader. Not all 287: most would be dead weight in the
  binary. Not only what the code reads today either: that leads to concluding a
  catalogue "doesn't exist" when it does. **When a field needs a code list,
  check the registry before concluding FEGA publishes none.**
- **Held for a future reader:** `MATERIAL_VEGETAL_REPRODUCCION`,
  `PROC_VEGETAL`, `REGIMEN_TENENCIA`, `DESTINO_CULTIVO` and `DEST_COSECHA`
  have no field anywhere in the cuaderno exchange schema. They are REA/DGC
  declaration vocabulary, kept for the parked DGC path because they cost
  almost nothing. (`MATERIAL_VEGETAL_REPRODUCCION` is not what
  `SiembraPlantacion` reads: that member is `number(1)`, *"1 Siembra 0
  Plantación"*, filled from `sowing_record.kind_code`.) Drop them if the DGC
  path is ever given up for good.
- **FEGA has two surfaces, and only this one can be read by a program.** The
  catalogue registry above is an API. FEGA's *"Fuentes Fitosanitarias"* page
  (`/bdcsixpor/ffii`) is a list of links to six MAPA registries (ASPAFITOS,
  MDF, REGMAQ-ROMA, ROPO, REGANIP, REGITEAF), and **none of them has an
  interface**. The page's own `FuentesInformacion.zip` is behind a reCAPTCHA;
  ASPAFITOS (`servicio.mapa.gob.es/regfiweb`) is a server-rendered ASP.NET app
  with no JSON and no bulk export; REGMAQ-ROMA
  (`servicio.mapa.gob.es/regmaq/buscar.wai`) answers a form POST with HTML;
  ROPO's bulk file is out of date. **Scraping is not acceptable as an
  interface for this project**, so these registries aren't used as data
  sources. If a product or machinery feed is ever wanted, the way is to *ask*
  (`sgmpagri@mapa.es` is published on the registry itself). They are used as
  link destinations, though: the app can say which registry holds a number and
  open it. The URLs are in `src-tauri/src/external_links.rs`; see
  [architecture.md](architecture.md) → "Pages the app points at, and never
  talks to". So "check the registry" in the rule above means the catalogue API,
  the only surface that answers.

#### Catalogues read in a particular way

- **`TIPO_COBERTURA_SUELO`** (`soil_cover.cover_type_code`) is **narrowed per
  practice** in the covers form. RD 1048/2022 art. 42.1.a sets "la cubierta
  vegetal espontánea o sembrada" (codes 2 and 3), and art. 43.1.a "la cubierta
  inerte de restos de poda" (code 4, and not 5, which is nutshells and stones).
  The narrowing is in the PICKER, never in the repository, because the file
  grows (code 6 was added upstream) and a refresh on a user's machine could
  otherwise leave a lawful cover impossible to record. `tests/siex_mapping.rs`
  accounts for every active code, so a new upstream code fails there instead of
  quietly becoming unreachable.
- **`DEST_RES_VEG`** (`cultural_operation.residue_destination_code`): code 9,
  "Trituración de restos de poda y depositado sobre el terreno", is the one code
  in the app with meaning beyond display. It *is* art. 43's P7 practice, so a
  pruning recorded with it is the evidence of an inert cover. It is named once,
  as `module_ecoscheme::siex::RESIDUE_LEFT_ON_GROUND`, not repeated as a
  literal.
- **`TIPO_LABOR`** is mapped rather than read: `module-ecoscheme`'s own
  `cultural_operation_kind` maps onto it, and `tests/siex_mapping.rs` checks
  the map both ways. The second direction fails when FEGA adds a code no kind
  claims.
- **`EST_FENOLOGICO`** (`treatment_plot.growth_stage_code`): its code is NOT
  the BBCH stage. The monograph's 0–9 is in its own `Estadio bibliografía`
  column, so readers go through `module_phytosanitary::catalogue::growth_stage`.
- **`SUSTANCIAS_BASICAS`** (`treatment_record.measure_basic_substance_code`,
  on measure 11 "Usos de sustancias básicas") grows as the EU approves basic
  substances, so the repository stores any code and only the picker narrows.
  Its `Fecha de aprobación` is already ISO and is not a lifecycle column.
- **`ESPECIE_ANIMAL`** (the grazing register: model 9.1's "Especie animal que
  pasta" and `Pastoreo.Animales[].Especie`) has **no lifecycle columns at
  all**, like `TIPO_MAQUINA_UNE`, so every row is always active. A row count
  that *drops* on refresh means a truncated download, never a retirement.
  **`RAZAS` is not vendored**: neither model 9.1 nor `Pastoreo.Animales[]` asks
  for a breed.
- **`EDIFICACIONES_INSTALACIONES`** (`premises.class_code`): Anexo V's REA
  block 8 field 1 makes the class required *"en caso de … tratamiento … en las
  edificaciones e instalaciones que conlleve su identificación para la
  cumplimentación del CUE"*, i.e. models 3.4/3.5. (`Edificaciones[].IdEdificacion`
  is REA's own key for a registered installation, not a class code;
  `siex-export.md` covers it.) All its rows are buildings, so the column is
  buildings only. A catalogue held for a future reader is only as good as the
  reading behind it, so check that reading when the reader is built.
- **`MUNICIPIO_SIGPAC`** is the largest file we carry (8 434 rows). Model
  section 2.1 asks for the término municipal as "código y nombre", and the
  SIGPAC provider returns no name, so a catalogue is the only source. It is
  **keyed together with `Código de provincia`**, which matters: municipality
  codes repeat across provinces, so a lookup without the province returns a
  real town in the wrong province. See `cuaderno-print.md` → 2.1.

#### Adding or removing a catalogue

- **Adding.** Everything at runtime works off `VENDORED`, so nothing needs
  wiring by hand: the startup import (`ensure_catalogues`), the Settings status
  list (`catalogue_status`) and the refresh button (`refresh_catalogues` over
  `vendored_ids()`) all go through it, and the UI shows whatever comes back. To
  add one: put the CSV, byte for byte, in `catalogues/`; add the `Vendored`
  entry with its `headers` in full (the failing test prints the block to
  paste); bump the array length; add the id to `VENDORED_IDS` in
  `tests/catalogue.rs`, which fails if the two lists differ either way. The
  row-count floor in that file moves too.
  `every_vendored_file_refreshes_to_unchanged_against_itself` is the test a new
  entry trips first: a `code_header`, `label_header` or `identity_attrs` that
  can't rebuild the file shows up there rather than in the feature that needed
  the catalogue.
- **Removing is different, and needs thought.** `reconcile` never deletes and
  nothing issues a `DELETE FROM catalogue`, so dropping a file from `VENDORED`
  stops it being imported and listed but leaves its rows in **every database
  that already has them**. That is on purpose (a code stored on an old record
  must still resolve at an inspection), but it splits installs: existing ones
  keep resolving the codes, a fresh install never had them and prints the raw
  code. Only remove a catalogue that no stored record can point at.
- The data host serves non-visible catalogues without authentication too
  (FEGA data is public-sector information). The non-visible entries are mostly
  internal load tables (`ARIES_*`, `RIIA_*`), `*_VISTA` views and admin rows,
  and several give 404. Treat them as less curated: `USOS_AGUA`, for example,
  calls its retirement column `Fecha Baja` instead of the `Fecha de baja` every
  visible catalogue uses, which the parser would miss.

#### Refreshing

- **Two ways.** *In the repo* (the one that counts): a release step (§6 step 1).
  List the registry, fetch each vendored idTabla, replace the files byte for
  byte, run the tests. *In the app*: a **manual** button in Settings →
  "Catálogos de referencia" fetches the same endpoint per idTabla and takes
  what passes the checks, so a user doesn't have to wait for a release to get a
  code published last month. Nothing runs on a timer or at startup: reference
  data sits under records with legal value, and rewriting it unasked over a
  rural connection is the wrong default.
- **The in-app refresh checks a file BEFORE using it**, and the order matters:
  `reconcile` never deletes, so a bad file taken once leaves wrong rows in every
  picker for good, and no later good file can remove them.
  `terrazgo_core::catalogue::refresh_catalogue` runs, in order: digest
  (identical bytes → `Unchanged`, nothing parsed) → shape → every row has a
  label → no control characters → **the row count must not shrink** (codes are
  retired with a baja date, never removed, so a shorter file is a truncated
  download) → only then the transaction. Refusals are **per file**: one retired
  idTabla or one unreachable host must not stop the user getting the other
  catalogues' updates.
- **Strict in CI, tolerant (and reported) in the field.** The vendored files
  must have the whole pinned header row, in order (`validate_shape`). A
  *fetched* file must have "every column the app reads, found by name, exactly
  once", and any column it gained beyond that is accepted and named back to the
  user. A rename is refused either way (it shows up as a *missing* pinned
  header), but refusing a harmless added column would leave users unable to
  update at all until the next release. A shape refusal means **the app needs
  updating, not the data**, and the message says so: the app reads named
  columns and can't adapt at runtime.
- **Columns are found by name, and the names are pinned.**
  `Vendored.code_header` / `label_header` / `identity_attrs` find columns by
  name, never by position, so an added, removed or reordered column can't break
  the import. That leaves *renames*, and `Vendored.headers` pins the file's
  whole header row, checked by `validate_shape` on **every** parse (vendored and
  fetched). Positions weren't good enough: one extra leading column in
  `MAT_FERTI` left every other catalogue check passing while every stored code
  became the contents of the column next to it. Renames must be pinned even
  where nothing seems to read the column: the parser finds `Fecha de alta` /
  `de modificación` / `de baja` **by name** in the files that have them (a
  rename silently loses retirement dates, so retired codes stay in every
  picker), and several crates read `attrs` keys by name (`Ámbito
  Fertilización`, `Uso SIGPAC`, `Código SIEX`, the composition columns). FEGA
  does vary: `USOS_AGUA` writes `Fecha Baja`.
- **Pre-release, the app follows the catalogue's current meaning.** This is the
  migration-squash rule applied to catalogues. When FEGA changes what a code
  means, our map, kind and label follow the new wording (`TIPO_LABOR` 7 became
  a green pruning, so the kind that sends it is `green_pruning_with_cleaning`
  and an ordinary pruning has no kind). When a file changes shape, the readers
  move to the new shape and nothing is written to keep a database imported
  under the old one working, since those are recreated. What stays are the
  import's runtime guarantees (upsert only, `absent_since`, the refresh's
  checks), which protect records, not old catalogue versions.
- **Updating a pinned entry is a review, not a fix.** The failing test prints
  the `headers: &[…]` block to paste. Read what the provider moved, and decide
  whether the app still reads what it thinks it reads, *before* pasting.
  Regenerating it just to get green defeats the whole point.

#### What each check catches

No single one covers everything:

| Check | Catches |
| --- | --- |
| finding columns by name (`catalogue.rs`) | added, removed or reordered columns, by design |
| `Vendored.headers` + `validate_shape` | any rename, including lifecycle columns and named attrs; damage to the delimiter or BOM shows up here too |
| `column_index` ambiguity check | a file with the same header twice |
| `identity_attrs` resolution | a missing qualifier column (the import is refused) |
| row count per file (`tests/catalogue.rs`) | a wrong `identity_attrs`; otherwise silent, as the upsert merges rows onto one id |
| every row has a label | the wrong label column |
| control-character tripwire | the provider moving away from Windows-1252/UTF-8 |
| two-way `siex_mapping.rs` contract tests | a small closed catalogue adding, retiring or renumbering a code |
| pinned `(code, label)` pairs | renumbering in the open lists (EFICACIA in full; "Aceitunas"; MACRONUTRIENTES 1/6/9) |
| `UNMAPPED_COLUMNS` (module-fertilisation) | a **new** column in the one file read column by column that nobody has decided about yet |
| `catalogue.source_digest` | a *fetched* file that differs from the stored copy (detection only, not validation) |
| checks before writing in `refresh_catalogue` | a fetched file that is empty, truncated, missing labels or in the wrong encoding, before anything is written, since the upsert can't be undone |

**What no check catches:** a change of meaning with the same header and the
same code. The `DETALLE_MATERIAL_FERT` heavy-metal columns are an example: one
column holds percentages for some products and mg/kg for others. That is in
the provider's data, not in how we read it. The only defences are reading the
diff of a refresh (which the pins force) and checking against the registry.

#### Codes that disappear, and why startup keys on the app version

- **A code that disappears from the provider's file leaves the pickers but is
  never removed** (`catalogue_code.absent_since`). FEGA retires a code with a
  baja date, so a row that simply vanishes is unexplained. We keep it, because
  an old record may cite it and must still resolve, and we stop offering it.
  **Only a FETCHED file may set the mark**, because it is the provider's
  current list. A *vendored* file proves nothing: a code can be missing from it
  just because it is newer than the release, and marking it would hide every
  code a refresh had added at the next app update. Both kinds of file clear the
  mark. `absent_since` is ours and kept apart from `retired_on`, which is the
  authority's: when a dropped row comes back with a baja date, our mark clears
  and theirs takes over, so the code stays out of the picker for a reason
  someone actually gave. `active_codes` filters on both; `find_code` on
  neither. The refresh reports the count per file as `withdrawn`.
- **Known side effect on the shrink check:** it compares a new file against
  *every* stored row, absent or not, and stored rows only accumulate. So once a
  device has withdrawn a code and taken a replacement, a file with only the
  older set is refused as a truncated download. Releases aren't affected (a
  re-vendor is reviewed by hand); it only costs a user an in-app refresh they
  could otherwise have taken. Counting only non-absent rows would fix it, but
  that changes what the check *means*, so it is left as it is.
- **Not done: removing codes a refresh added that nothing uses.** It would need
  a hand-kept list of which `*_code TEXT` columns store each catalogue's codes,
  because the schema has **no foreign keys from user data to `catalogue_code`**
  on purpose. Those columns are in modules core can't depend on, so the list
  would have to come through a new `Module` trait method. And it would fail
  silently and destructively: a later column that stores codes but isn't listed
  makes them look unused, so an existing record stops resolving, and nothing in
  the schema marks a column as holding catalogue codes, so nothing can catch the
  omission. It would also gain nothing: open-catalogue pickers show what
  `active_codes` returns, so an extra code there is what the refresh was for;
  closed-list pickers read the lookup tables, never the catalogue, so an extra
  row never reaches the UI. And the row a cleanup would remove is the row that
  keeps an old treatment readable.
- **Startup imports the vendored set once per app version.** `ensure_catalogues`
  imports on the **first launch of each app version**
  (`catalogue.imported_by_version` against `CARGO_PKG_VERSION`, compared for
  equality so a downgrade also re-imports) and does nothing after that. So **a
  user's refresh survives restarts, but not an app update**: the next version's
  first run puts its own copy back. That is on purpose: the vendored files are
  curated *as a set*, and the mapping tests pass against one exact snapshot, so
  a device must never run one refreshed file mixed with an older release's
  others. An update restores every label, attribute and lifecycle date; codes a
  refresh *added* stay, because the import can't delete without breaking the
  promise that a code already on a record keeps resolving. (Startup can't key
  on `source_digest`: the refresh writes it too, so a refresh would last one
  session and then be half undone.)
- **In development:** re-vendoring a CSV *without* bumping the version doesn't
  trigger a re-import at startup. The release checklist re-vendors and bumps,
  and the pinned-header tests read the vendored bytes directly, so this only
  affects a running dev app, where the rule after a `0001` edit is already
  "delete the dev database".
- **Detecting a refresh:** `catalogue.source_digest`, an FNV-1a hash of the
  bytes behind the stored rows, compared *before* parsing, so offering a copy
  already held costs nothing. It hashes the bytes rather than using the file's
  newest lifecycle date: the date needed every file parsed before it could be
  skipped, and missed a refresh that fixed a label without changing any date
  (several catalogues have no dates at all).

#### Known quirks

- Documented as ISO-8859-1 but actually **Windows-1252** (€ at 0x80 in
  UNIDADES_MEDIDA).
- Codes are retired with a baja date, never deleted.
- The crop catalogue is `PRODUCTOS` (not "CULTIVO"), and it is **not** the same
  list as `PROD_VEGETAL`. That one codes the harvested *produce* ("Aceitunas")
  and points to the crop ("OLIVO"). SIEX's `ProductoVegetal` /
  `ProductoCosechado` fields mean the produce.
- `COMUNIDAD_AUTONOMA` only has the **seventeen comunidades**: Ceuta (INE 18)
  and Melilla (19) are missing, so a farm there has no `CAExplotacion` value to
  send (docs/siex-export.md → "Recorded gaps"). It also mixes **two codings**:
  it is keyed on the catastro code, but SIEX `CAExplotacion` wants INE, and they
  differ for 10 of the 17 communities (Castilla y León is catastro 08 / INE 07,
  and INE 07 is Castilla-La Mancha in the catastro coding). We key on INE. FEGA
  also publishes a municipal INE↔catastro table per campaign as a web page
  (search "relación de municipios por CCAA con equivalencias entre los códigos
  INE y catastro"); there is no API.
- Several catalogues have **no lifecycle dates** at all (`TIPO_MAQUINA_UNE`,
  `USO_SIGPAC`, `PROVINCIA`, `COMUNIDAD_AUTONOMA`, `PAIS`, `REGIMEN_TENENCIA`,
  `DETALLE_MATERIAL_FERT`).
- Codes repeat, per qualifying attribute, in `CULTIVO_USO_SIGPAC` (uso),
  `PROD_VEGETAL` (crop), `MATERIAL_VEGETAL_REPRODUCCION` (detalle) and
  `MUNICIPIO_SIGPAC` (province).
- `BUENAS_PRACTICAS_AMBITOS` is one row per practice with an SI/NO column per
  ámbito, so it can't retire a practice in one ámbito only: a practice leaves
  an ámbito by turning to "NO", with no date.
- Some files start with their **parent** catalogue's code rather than their
  own: `EDIFICACIONES_INSTALACIONES` (tipología) and `DETALLE_MATERIAL_FERT`
  (MAT_FERTI type).
- `TIPO_TRATAMIENTO` has no code 1; `DETALLE_MATERIAL_FERT` leaves its own
  `descripcion` column empty on many rows.

### SIGPAC service fixtures (test data)

- **Our copy:** `crates/module-sigpac/tests/fixtures/`: real responses (recinto
  by reference and by point, zone intersections, the `/geopackages/` campaign
  listing HTML, and `cultivo_declarado` declarations: a plain line, an empty
  answer, one with a secondary crop, one recinto declared in two lines).
- **Upstream:** the live services in §2. If the provider changes a response,
  fetch the fixtures again from the live service and let the parser tests show
  what changed.

### Liberation Sans fonts (embedded in the binary for PDF reports)

- **Our copy:** `crates/terrazgo-report/fonts/`: the four Liberation Sans TTF
  faces (regular, bold, italic, bold italic) plus the upstream `LICENSE` (SIL
  OFL 1.1), embedded with `include_bytes!`.
- **Upstream:** liberation-fonts releases at
  <https://github.com/liberationfonts/liberation-fonts/releases> (we carry
  v2.1.5).
- **Refresh:** only when there is a real reason (a missing glyph, an upstream
  fix). Fonts change text metrics, so a swap can reflow every report. Replace
  the four TTFs and the LICENSE together.
- **Checked by:** `crates/terrazgo-report/tests/render.rs`. The faces must load
  with Typst's own font parser, be listed under the family name
  `"Liberation Sans"` exactly (what every template's `#set text` asks for), and
  cover the Spanish characters (accents, `€`, `ª/º`, `¿¡`).

### rustls-platform-verifier Kotlin component (Android TLS)

- **Our copy:** none. The compiled `.aar` ships *inside* the
  `rustls-platform-verifier-android` crate as a bundled Maven repository, and
  `src-tauri/gen/android/app/build.gradle.kts` finds it at build time through
  `cargo metadata`, using the exact version Cargo resolved.
- **Upstream:** <https://github.com/rustls/rustls-platform-verifier> (crate
  `rustls-platform-verifier`, version set in the workspace; the Android
  artifact follows the `-android` sub-crate).
- **Refresh:** happens with `cargo update`, nothing to do by hand. If the crate
  ever changes how it bundles the Maven repository, the Gradle function that
  finds it is what to fix.
- **Checked by:** the Android build itself (Gradle fails if it can't find the
  artifact) and the ProGuard keep rule in
  `src-tauri/gen/android/app/proguard-rules.pro`. The class is only reached
  through JNI, so release shrinking would otherwise remove it, and that would
  only show in release APKs, as blank maps.

## 2. Live services (runtime network)

Everything the app fetches at runtime goes through `terrazgo-net`: the shared
HTTP agent, its TLS policy (the platform verifier) and the Android setup that
policy needs. It has two users:

- **The map**, and everything that goes with it (SIGPAC lookups, zone checks,
  crop prefill), goes through `terrazgo-geo`'s cache, which serves the `geo://`
  protocol. Its allowed sources are in `crates/terrazgo-geo/src/sources.rs`; a
  service not listed there can't be reached. Once seen, responses are cached,
  so a dead provider means "works offline on cached data", not a broken app.
  What actually breaks is a fresh install, or an area never seen before.
- **The catalogue refresh** calls `terrazgo-net` directly from the shell
  (`src-tauri/src/catalogues.rs`), because what it fetches goes into the app
  database through core's importer, and a tile cache has nothing to do with it.
  It is manual and refuses per file.

When replacing a source, prefer the most modern and lightest service (MVT, then
WMTS, then WMS).

| Service | Endpoints | Used by | If it dies / notes |
| --- | --- | --- | --- |
| OpenFreeMap (vector base map) | `https://tiles.openfreemap.org/styles/liberty` (style, rewritten in Rust), `/planet` (TileJSON → dated tile URLs), `/fonts/…`, `/sprites/…`, `/natural_earth/ne2sr/…` (backdrop) | `sources.rs` + `style.rs` | Free OSM tile host with no SLA. A replacement is any MapLibre-style vector provider: new registry entries and a style rewrite in `style.rs`. Tile URLs carry a dated planet snapshot read from the TileJSON at fetch time; an old cached style keeps working because the tiles are cached too |
| IGN PNOA (orthophoto base) | `https://www.ign.es/wmts/pnoa-ma` (WMTS, GoogleMapsCompatible) | `sources.rs` | Spanish state provider, stable. The alternative would be another national WMTS or ESA/commercial imagery |
| Nube de SIGPAC MVT (parcel overlays) | `https://sigpac-hubcloud.es/mvt/{layer}@3857@pbf/{z}/{x}/{y}.pbf`, layers `recinto`, `cultivo_declarado`, `e_paisaje_area/_linea/_punto`; previous campaign under `/mvt/anterior/` | `sources.rs` (cache rows keyed by campaign) | z12–15 only. Empty tiles answer 404 (cached as empty), and so do the tiles missing from `recinto`'s z12 set where parcels are dense, which is why that overlay starts at z13. The fixed path serves the *current* campaign, except `cultivo_declarado` (previous). CC BY 4.0, attribution must stay |
| SIGPAC REST (lookups + zones) | `https://sigpac-hubcloud.es/servicioconsultassigpac/query` (recinto by ref/point), `…/intersection` (nitrate/phyto/Natura zone checks) | `crates/module-sigpac/src/client.rs` (through `terrazgo-geo`) | Writes `plot_zone_flag`. A dead service stops *new* checks; stored flags and alerts stay. REST responses use hectares, MVT surfaces m² |
| SIGPAC declared crops (crop prefill) | `https://sigpac-hubcloud.es/ogcapi/collections/cultivo_declarado/items?f=json&provincia=…&recinto=…&exp_ano=…` (OGC API Features; the 7 reference parts + `exp_ano` are query parameters) | `crates/module-sigpac/src/client.rs` (through `terrazgo-geo`) | Serves the PREVIOUS campaign: the client asks for the current one, falls back to the one before, and labels every proposal with the campaign that answered. `exp_ano` can be filtered on but isn't in the responses. Nothing declared = HTTP 200 + `numberMatched: 0`. `parc_supcult` is in m². Cache key `sigpac/cultivos/{campaign}/{ref}`, never evicted. CC BY 4.0 |
| SIGPAC current campaign | `https://sigpac-hubcloud.es/geopackages/` (directory listing; the highest year = current campaign) | `terrazgo_geo::fetch::current_campaign` | The only machine-readable statement of the campaign, and every campaign-keyed cache row depends on it. If the listing format changes, noticing a new campaign is the first thing to break |
| FEGA BdcSixWsp (SIEX public API) | `https://www11.fega.es/bdcsixwsp/`: `/catalogos/*` (see §1), `/fuentesInformacion/zip` (MDF non-chemical defence registry; ROPO left out, it is personal data), `POST /existeNIF` (NIF → farms with SIEX/REA codes) | the release checklist and the manual in-app catalogue refresh (`src-tauri/src/catalogues.rs`, one GET per vendored idTabla). `/existeNIF` would be the way to prefill REA codes | No authentication. Guide: "BdcSixWsp — Guía de Servicios públicos de Siex" (on the sede portal, §4) |
| GitHub releases API (website only) | anonymous `releases` endpoint of the public repo | `site/` download card | Drafts aren't visible (the links fill in on publish). `releases/latest` is useless while every release is a pre-release. Fallback: the releases page |

### Behind a corporate proxy

A machine that reaches the internet only through a proxy fails **every** live
service at once, and fails it as DNS: the app resolves the host directly, the
network says "no such host", and each fetch shows `geo_offline` with the OS
error (on Windows `os error 11001`). What tells you it is a proxy and not a dead
provider: the whole table above fails together, while a browser on the same
machine loads the same URLs. Browsers read the system proxy settings;
`terrazgo-net` doesn't.

What `terrazgo-net` does read is the environment: `ALL_PROXY`, `HTTPS_PROXY`,
`HTTP_PROXY` and `NO_PROXY`, read once when the agent is built. Setting them as
Windows *user* environment variables and restarting the app is the whole fix;
no in-app setting is needed. This has been confirmed on a laptop configured
through a PAC script.

What it does **not** read is the Windows proxy configuration itself: not the
static `ProxyServer` value, not the `AutoConfigURL` of a "setup script" (PAC),
and not WPAD auto-detection. So the address has to be found once by hand:
Windows Settings → Network & Internet → Proxy shows the script URL, and opening
that URL in a browser gives readable JavaScript whose `PROXY host:port` for an
external host is the value to put in the variables.

No variable helps with a proxy that requires NTLM or Kerberos. Only basic auth
can be written, as `http://user:pass@host:port`.

## 3. User-supplied file formats (no network)

| Format | Where the user gets it | Used by | Notes |
| --- | --- | --- | --- |
| Boundary files: GeoJSON, GeoPackage | Anywhere, including the SIGPAC download service `https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html` (provincial recinto and declared-crop GPKGs, CC BY 4.0) | `terrazgo_geo::import` | Geographic SRS only (EPSG 4326/4258/4081). Projected files fail with `gpkg_unsupported_srs`; the plan for reprojecting with proj4rs is in `sigpac-integration.md` if real projected files turn up |
| Declared-crops GPKG (bulk) | Same download service, current and previous campaign | not used: the per-reference OGC API in §2 serves the crop prefill (112–545 MB per province is too much over a rural connection) | Model: SIGPAC ref + `PARC_PRODUCTO` + `PARC_SISTEXP` + `PARC_SUPCULT` + geometry ([model page](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html)) |
| Offline municipality packs (not built) | INSPIRE ATOM / bulk GPKG per municipality | would be read by the SIGPAC lookups before the network, for areas never seen online | Only if use in the field shows the cache isn't enough. Packs would cover boundaries and attributes, NOT zone intersections, which are query-only |
| REACYL DGC Excel export (future) | The farm owner's own REACYL DGC module (certificate login) | not built | The columns and whether `CodigoDGC` is there are unconfirmed (a question for CUECYL). Reading `.xlsx` would need the calamine crate, which would have to be decided before coding |

## 4. Official documents & regulatory sources

The code cites these. When the code and the document disagree, first check
whether the document has a new version.

| Document | Where to get it | What implements it |
| --- | --- | --- |
| FEGA SIEX technical docs: Anexo V (fields), VI (interface + schema), VII (catalogues), IX/X (authorizations) | <https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex> | the whole export (`terrazgo-siex`, `module_phytosanitary::siex` and each module's own) |
| FEGA "Documentación Técnica Horizontal SIEX": replaces the row above with SIEX 5.9.0 (September 2027). Anexo 03 "Bloques de datos" follows Anexo V and is still a preliminary version | <https://www.fega.gob.es/es/siex/documentacion-tecnica-horizontal-siex> | read now for what is coming (`docs/siex-export.md` → "The September 2027 model"); the reference from then on |
| "BdcSixWsp — Guía de Servicios públicos de Siex" (v4.9.0), and its companion "Estructura" document listing every catalogue's columns with their types (`Lógico`, `Decimal`, `Fecha`…) | files of the sede portal app at `https://www3.sede.fega.gob.es/bdcsixpor/` | what the catalogue importer expects (format, encoding, lifecycle columns); the Estructura is what to read when a pinned header moves |
| RD 1311/2012 (record content, Anexo III) | <https://www.boe.es/buscar/act.php?id=BOE-A-2012-11605> | treatment record fields, PHI |
| RD 34/2025 (electronic record from 2027) + Reglamento (UE) 2023/564 (+ the 2025/2203 postponement) | boe.es / eur-lex.europa.eu | why the module exists; deadlines |
| RD 1054/2022 (SIEX, REA first) + resolution BOE-A-2023-13035 | boe.es | the REA-first flow, farm identity fields |
| CUECYL / REACYL pages + the "Instrucciones declaración DGC" PDF | agriculturaganaderia.jcyl.es | regional submission; the farmer's own DGC paths ([siex-export.md](siex-export.md)) |
| INE province ↔ comunidad autónoma | ine.es (code tables) | `siex::province_to_ccaa` |
| Slippy-map tile scheme (z/x/y ↔ EPSG:3857 bbox) | OSM wiki | tile cache keys; the future WMS grid snapping |

Contact for regional submission: **comercialcuecyl@jcyl.es** (test environment
for commercial record books, Castilla y León).

## 5. Release credentials: Android signing & Google Play

Things a release needs that are kept *outside* the repo on purpose.

### Android release keystore (the upload key)

- **What it is:** one RSA-2048 keystore signs every release APK/AAB. For APKs
  installed from GitHub releases it *is* the app's identity: Android refuses an
  update signed with a different key. For Google Play it is the *upload key*
  (Play App Signing re-signs with Google's app key, which is why a Play install
  and a sideloaded APK can't update over each other).
- **Where it is:** on the development machine, outside the repo, with an
  offline backup; the password in a password manager. Never committed:
  `gen/android/.gitignore` covers `keystore.properties`, and the keystore file
  must stay out of the working tree.
- **How builds find it:** `src-tauri/gen/android/keystore.properties`
  (untracked: `password=` / `keyAlias=` / `storeFile=`) is read by the
  signingConfig in `app/build.gradle.kts`. Without the file, release builds
  come out unsigned (debug builds don't care). CI rebuilds the file from the
  GitHub Actions secrets `ANDROID_KEYSTORE_B64` (the keystore in base64),
  `ANDROID_KEYSTORE_PASSWORD` and `ANDROID_KEY_ALIAS` in the `build.yml`
  android job.
- **Setting the CI secrets:** pipe them from the checked `keystore.properties`
  and keystore file; never type them. A typed password can carry invisible
  trailing whitespace, and the android job then fails with "keystore password
  was incorrect". To fix it, set all three secrets again from the checked local
  values and run `gh run rerun --failed`; no new tag is needed. Check the
  password against the keystore with `keytool` first, and never echo it.
- **If lost:** sideload users have to uninstall and reinstall (no key, no
  updates). The Play upload key can be reset through Play Console support,
  because Play App Signing holds the real app key. Back it up accordingly: no
  one else can recover the keystore or its password.
- **If leaked:** rotate it straight away. Ask Play for an upload-key reset, and
  accept that sideloaded installs can't update (say so in the release notes).

### Google Play Console

- **App:** `org.terrazgo.app`, on the **internal-testing track** while the
  project is pre-release. Moving to production is a separate decision, like
  declaring the project released.
- **The first upload is manual** (Play requires it): the AAB comes from the
  `build.yml` android job's workflow artifact. Later uploads could be automated
  with a Google Cloud service account that has release permission on Play: its
  JSON key would be one more Actions secret and one upload step in `build.yml`.
- **Recurring Play chores:** the target API deadline (Google raises the
  required `targetSdk` about once a year, mid-year; Gradle sets it in
  `app/build.gradle.kts`), and the data-safety form and privacy policy when the
  app starts collecting anything new (today nothing leaves the device).

## 6. Release checklist

0. **Bump the version.** Three manifests must match the tag: `Cargo.toml`
   (`[workspace.package] version`, which every crate inherits), `package.json`
   and `src-tauri/tauri.conf.json`. Refresh both lockfiles afterwards
   (`cargo check` and `npm install --package-lock-only`), or the release commit
   ships a lockfile that still names the old version. Nothing under
   `src-tauri/gen/android/` needs editing: `tauri.properties` and the copied
   `tauri.conf.json` are generated at build time and untracked. `release.yml`'s
   first job fails the release if any of the three doesn't match the pushed tag,
   before anything is published. One thing nothing checks: the scripted
   frontend checks' fixture file holds a recorded `get_about_info.app_version`
   (and a recorded `build_time` next to it), so it shows the previous version
   until someone updates it. Nothing asserts on it, so nothing fails, which is
   why it is listed here.
1. **Refresh the catalogue snapshot.** List the registry (`GET
   https://www3.sede.fega.gob.es/bdcsixpor/tablas/configJson`), then for each
   idTabla in `catalogue.rs`'s `VENDORED` `GET
   https://www11.fega.es/bdcsixwsp/catalogos/{idTabla}` and replace the file in
   `crates/terrazgo-core/catalogues/` byte for byte. Run `cargo test`: the row
   count and label checks, the snapshot-fact tests, the encoding tripwire and
   the `siex_mapping` contract tests are there to fail loudly when the provider
   changes something, rather than ship it silently. Don't use
   `/catalogos/zip/`: its files are named by display name, not idTabla (§1).
   While the registry is open, look for catalogues that would serve a field
   currently assumed to have none (§1). The in-app refresh doesn't replace this
   step: it updates a *user's* database, never the repo's files, so a fresh
   install still ships whatever was last committed here.
2. **Check the CUE schema version** on the FEGA documentation page. If it is
   past 3.11.4, vendor it and compare before the next release that touches the
   export (procedure in §1). The page says that with SIEX 5.9.0, planned for
   September 2027, the agricultural and livestock technical documentation will
   be merged into a "Documentación Técnica Horizontal", and the anexos are read
   there from then on.
3. **Android.** First run `bash scripts/lint-android.sh`. It lints the app as
   built for Android, with warnings as errors, which CI never does: the
   `#[cfg(mobile)]` code and the Android side of the standard library aren't
   compiled anywhere else before a device build. It needs the Rust target and
   an NDK, no device, and says what is missing. Then the release APK/AAB must be
   signed (the CI job fails if the keystore secrets are missing; never get round
   it by shipping unsigned or debug-signed builds), and the AAB goes to the Play
   internal-testing track (§5). **After a Tauri or Android Gradle update,**
   install the signed release APK on a phone and check that the base map draws
   and a SIGPAC lookup answers. The R8 keep rule for
   `org.rustls.platformverifier` must still hold (AGP 9's `optimization {}`
   replaced `isMinifyEnabled`), and if it doesn't, it only shows in release
   builds, as blank maps.
4. **Run the storefront export guard** (`packaging/export-storefront.sh` into a
   scratch directory) *before* tagging. It fails on any reference to the dev
   tooling that should have been removed, and it is quick; finding the failure
   from a red release instead costs a tag.
5. **Check the published attestations** (provenance + SBOM, one per installer
   digest). The GitHub attestations API no longer includes the bundle: it
   returns `bundle: null` and a `bundle_url` pointing at an Azure blob served as
   `application/x-snappy` (raw-snappy-compressed JSON), so `gh api … --jq
   .attestations[].bundle` doesn't work any more. Fetch the `bundle_url`,
   snappy-decompress it, then read the DSSE payload. Provenance subjects are
   split by runner (the Linux job attests the AppImage, deb and rpm, the Windows
   job the exe and portable zip); that is expected, each runner attests what it
   built.
6. **Read the package metadata back, not just the build log.** Only the payload
   of a package is checked by the compiler, so the parts a farmer actually reads
   can be wrong while everything builds green. Parse the rpm header and the deb
   control file and check the summary, the description and `Categories=` in the
   generated `.desktop` entry. Without `bundle` descriptions, Tauri falls back to
   the *crate* description, which then appears in the user's application menu.
7. Read through this file: does every row still match reality?

### Linux packages: why one rpm is enough

The Linux job builds an AppImage, a deb and an rpm; adding a format is one word
in `bundle.targets`. The rpm needs **no `rpmbuild` and no new system dependency
on the runner**: tauri-bundler writes it with the pure-Rust `rpm` crate.

Its `Requires` are **sonames** (`libwebkit2gtk-4.1.so.0()(64bit)`,
`libgtk-3.so.0()(64bit)`), not package names. That is what lets one file install
on Fedora, openSUSE and RHEL: those distributions name the packages
differently, but the libraries the same. Don't add per-distribution builds
until a real install fails.

## 7. Dependency advisories and yanked crates

The `audit` CI job runs `cargo-deny` over `Cargo.lock` (advisories only).
`deny.toml` gives every `ignore` a reason and a condition for revisiting it.

- **Check which job failed before reading red as "the same as yesterday".** A
  badge that stays red hides the next real failure.
- **A yanked crate is not a vulnerability**, and often nothing in this repo can
  fix it: a transitive dependency may require exactly the yanked version.
  Prefer leaving the job red to adding an `ignore`, so the check stays honest.
  It goes green by itself when the crate leaves the tree or is un-yanked, but
  nothing announces it: `cargo deny check advisories` locally is how you find
  out, and `cargo tree -i <crate>` says whether the crate is still reached at
  all. The ways out, cheapest first:

  | If upstream… | What we do | Cost |
  | --- | --- | --- |
  | publishes a new patch of the crate, or un-yanks it | `cargo update -p <crate>` | one lockfile line |
  | the crate that requires it drops it in a patch release | `cargo update -p <that crate>` | lockfile only |
  | it is only fixed in a new minor of a crate further up | the crates in between bump first, then ours in `Cargo.toml` | a real dependency change |

- **A real vulnerability fixed in a patch release** is usually `cargo update -p
  <crate>`: a lockfile change, still one copy of the crate. A lockfile-only move
  is safe when it goes **up**, because a fresh resolve would pick the same
  version. Holding a crate **back** is different: a lockfile-only pin
  disappears at the next resolve, so that needs a ceiling in the manifest.

### Updating dependencies: crate copies

An update can leave Cargo holding two versions of one crate: one dependency
moves to a new major while another stays on the old one. Cargo allows it
(unless the crate links a native library, `links =`, which is why `rusqlite` is
kept to one version), but each copy is compiled separately on every target, adds
its own code to the binary wherever both are used, and is one more version to
watch for advisories. So **a new copy is weighed, not just accepted**:

- **Held back** when the newer release gives nothing the app uses, as a
  ceiling in the manifest with the reason next to it. Never as a lockfile-only
  pin, which the next `cargo update` silently undoes.
- **Taken** when it brings fixes the app benefits from, with the pair recorded
  below so you notice when it can be collapsed.

The check, before and after `cargo update`: list the names with more than one
`[[package]]` in `Cargo.lock` and compare the two lists. `cargo tree -e normal
--target <triple> -p terrazgo` shows which copies actually reach the app, and
`cargo tree -i <crate>@<version>` who holds each one. npm isn't held to this:
its duplicated packages are build, lint and test tools, none of them reaches
`dist/` (grep it), and npm gives every copy its own folder.

Current pairs:

| Crate | Copies | Why both | Collapses when |
| --- | --- | --- | --- |
| `jni` | 0.21 (`tao`) + 0.22 (`rustls-platform-verifier` 0.7, via `ureq` 3.4) | Android only; `ureq` 3.4 fixes connection pooling, connect timeouts and HTTPS proxies | a Tauri release moves `tao` to jni 0.22 |
| `base64` | 0.22 (`wry`, the Typst SVG stack) + 0.23 (`ureq`, `ureq-proto`, `plist`, `reqwest`); 0.21 (`swift-rs`) is Apple only | 0.23 came in through four patch releases, `ureq-proto`'s with a parser panic fix | `wry` and the Typst stack move to 0.23 |
| `miniz_oxide` | 0.8 (`png`) + 0.9 (`flate2` 1.1.10) | `flate2` 1.1.10 hardens the gzip decoding `ureq` applies to responses | `png` moves to 0.9 |
| `jsonschema` | held at `=0.48.1` | 0.48.2 adds `strum` 0.28 next to biblatex's 0.27, for nothing a test uses | biblatex moves to strum 0.28 |
| `dirs` | 6 (`typst-as-lib` 0.16) + 7 (`tauri` 2.12) | Tauri 2.12 has a security fix and the Gradle 9 template; both share one `dirs-sys` 0.5, so the copy is a few functions | `typst-as-lib` moves to dirs 7 |
| `infer` | 0.19 (`typst-pdf` 0.15) + 0.22 (`tauri-utils` 2.10) | the same Tauri update; a table of file signatures | `typst-pdf` moves to infer 0.22 |

`tao` and `sysinfo` (in `Cargo.toml`) and the Android Gradle stack (§8) are held
back for another reason (the platform pins them), documented where they are
pinned.

### Workflow actions: pinned by commit

Every third-party action in `.github/workflows/` and
`packaging/storefront/.github/workflows/` names a full commit, with its release
in a comment (`@6323deb… # v2.9.2`): whoever controls a tag can move it to other
code, while a commit can't move. GitHub's own `actions/*` stay on tags; they are
published as immutable releases. **When an action downloads a tool, pin the tool
too**, or pinning the action pins nothing that runs: cargo-binstall takes
`version`, and sbom-action's release carries its own Syft version. **The Tauri
CLI is the exception and stays at the newest 2.x**: the Android job's
`tauri-cli@^2` and, on desktop, tauri-action's own `@tauri-apps/cli@v2`
fallback, so a release builds with whatever 2.x is newest when it is tagged.

To move one: `git ls-remote --tags https://github.com/<owner>/<repo>.git` gives
the new tag's commit (for an annotated tag, the line ending in `^{}`); replace
the commit and the comment in every workflow that uses the action.
`dtolnay/rust-toolchain` has no tags: pin the head of its `stable` branch, whose
`toolchain` input defaults to stable. There is no Dependabot: each export
overwrites the public repo, so its pull requests would have nowhere to land.
This is done as part of a dependency update.

## 8. Android: build, device notes and what is left

### Building

The Gradle project is in `src-tauri/gen/android/` and is tracked in git (only
`gen/schemas/` is ignored). The shell crate is `crate-type = ["staticlib",
"cdylib", "rlib"]` with `#[cfg_attr(mobile, tauri::mobile_entry_point)]` on
`run()`. A debug APK (debug-keystore-signed, installable):

```
export JAVA_HOME=~/APPS/android-studio/jbr ANDROID_HOME=~/Android/Sdk \
       NDK_HOME=~/Android/Sdk/ndk/28.2.13676358 CARGO_PROFILE_DEV_STRIP=debuginfo
cargo tauri android build --debug --target aarch64 --apk
# → src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
# install: ~/Android/Sdk/platform-tools/adb install <apk>   (USB debugging on)
```

Machine setup: cmdline-tools, NDK 28.2.13676358 and the `platforms;android-37.0`
platform that `compileSdk` names, all in `~/Android/Sdk`; Java is Android
Studio's bundled JBR (`~/APPS/android-studio/jbr`); the rustup Android targets
added.

- **The JBR changes with Android Studio updates, and Gradle has a maximum Java
  version.** With JBR 25, Gradle 8.14 can't run: the build stops configuring
  `:buildSrc` with nothing but "> 25.0.3". Gradle 9.6.1 (the template since
  tauri-cli 2.12) runs on it, and the CLI now warns before building when Java
  is too new for the project's Gradle.
- `CARGO_PROFILE_DEV_STRIP=debuginfo` keeps the debug `.so` around 156 MB
  instead of about 600 MB (environment only; the repo's profile is unchanged).
- Gradle's incremental packaging can leave a replaced `.so` as dead space in the
  APK. `rm -rf src-tauri/gen/android/app/build` before judging the APK's size.
- Release builds sign themselves when `gen/android/keystore.properties` points
  at the keystore (§5), and the storefront `build.yml` has an `android` job that
  needs the keystore secrets.
- The distribution path works end to end: build → sign → Play → install from
  Play on a phone. A Play install and a sideloaded APK have different
  signatures and can never update over each other, so a test phone stays on
  one channel or the other.

### Hand edits in `gen/android`

These carry changes that `cargo tauri android init` would WIPE. Never
regenerate the project blindly; if it is ever re-initialised, re-apply them from
git. Each has been checked against tauri-cli 2.12.1 and is still needed:

- `app/build.gradle.kts`: the verifier's Maven repository and dependency,
  cleartext allowed in release builds, and the signingConfigs.
- `app/proguard-rules.pro`: the keep rule for the verifier (§1).
- `app/src/main/java/org/terrazgo/app/MainActivity.kt`: padding for the system
  bar insets (keeps the app between the status bar and the gesture area; the
  strip colour must match `--panel`), plus the IME inset, without which the soft
  keyboard draws over the page (see below).

`tauri.settings.gradle` stays generated (the CLI added `:tauri-plugin-fs` and
`:tauri-plugin-geolocation` itself), and `BuildTask.kt` is the template's own.

**Release builds allow cleartext, like debug builds**, because certificate
revocation lists are fetched over plain HTTP. `http://*.localhost` serving works
in release too (protocol interception happens before the cleartext block).

### The Gradle stack follows Tauri's template

AGP, the Kotlin Gradle plugin, the Gradle wrapper and the androidx libraries in
`gen/android` stay at what `tauri-cli`'s own Android template ships (AGP 9.3.1,
Kotlin 2.2.10, Gradle 9.6.1 and SDK 37 for tauri-cli 2.12.1), not at the newest
releases. It is the same rule `tao` and `sysinfo` follow in `Cargo.toml`: held
back by the platform, not by choice. Tauri's Android library modules build from
the cargo registry against those versions. Move them when a tauri-cli release
changes its template, with a three-way merge: fetch the template at the old and
the new `tauri-cli-v<version>` tags (`templates/mobile/android`), apply what
changed between the two to `gen/android`, and keep the hand edits above. Only
replace outright the files that are still identical to the old template.

### Debugging on a device

Debug builds have WebView inspection on: `adb shell pidof org.terrazgo.app` →
`adb forward tcp:9222 localabstract:webview_devtools_remote_<pid>` → the page
list at `http://127.0.0.1:9222/json`. Drive the page over raw CDP with Node's
built-in WebSocket (`Runtime.evaluate`); puppeteer-core's `connect` stalls
against Android WebView, so don't bother with it. Rust panics appear in `adb
logcat` under `RustStdoutStderr`.

The `plugin:geolocation|*` commands are stubbed in the scripted checks' session
(like `plugin:dialog|save`) and never recorded into `fixtures.js`.

**Checking whether a swipe scrolls** (the dead-zone bug of a universal
`overscroll-behavior: none`, `docs/frontend-conventions.md`) is done by
measuring, not by eye. On a debug build over CDP, centre a target and read
`main.scrollTop`; `adb shell input swipe` from the target's screen point; read
it again. Screen point = CSS px × `devicePixelRatio`, plus the status-bar inset
above the webview. Calibrate it once with a `touchstart` listener recording
`clientY` for a known swipe. Add a test rule with CSSOM `insertRule`, which the
CSP allows where an injected `<style>` is refused, and reload to drop it. Two
things that don't work: **held `input motionevent` drags never trigger the
Android stretch**, even with `auto` set back as a control, so a missing stretch
is still checked by hand; and **headless Chrome's
`Input.synthesizeScrollGesture` scrolls nothing**, while
`Input.dispatchTouchEvent` (start, about 20 moves, end, with a viewport that has
`hasTouch`) gives the same answer as the phone on both broken and fixed builds.
Start a sideways swipe away from the edge of the viewport.

### Device facts

- **Saving to shared storage is about 1000× slower without FUSE passthrough.**
  For the same 6 MB export, `sync_all` takes about 12 ms on a Pixel 8a (kernel
  6.1, `/mnt/pass_through/0/emulated` present) and about 12 s on a Galaxy A22
  (kernel 4.14, launched with Android 11, no such mount). **Both report
  `persist.sys.fuse.passthrough.enable=true`**, so the property doesn't prove
  the kernel can do it: a device upgraded from Android 11 keeps its old kernel.
  Accepted as it is: slow on old hardware is fine. Details in `docs/sync.md`.
- **The soft keyboard is an inset the activity has to pad for, and nothing in
  the page can replace that.** `MainActivity.kt`'s
  `setOnApplyWindowInsetsListener` pads for `systemBars() | displayCutout() |
  ime()`, by `maxOf(bars.bottom, ime.bottom)` at the bottom: **the larger of the
  two, not the sum**, because the keyboard sits ON the navigation bar, and
  adding both leaves a gap the height of the navigation bar above the keys.
  Without the IME inset the webview is never told the keyboard is there:
  `innerHeight`, `visualViewport.height` and `100dvh` don't change and
  `visualViewport.offsetTop` stays 0, so a height driven by `visualViewport`
  has nothing to read. `windowSoftInputMode` doesn't help either: at targetSdk
  35+ the window is edge-to-edge and `adjustResize` is ignored. With the inset,
  the webview shrinks as the keyboard opens, `100dvh` follows, and a bar pinned
  at the foot of a sheet stays above the keys. **So `100dvh` is only reliable
  because the activity pads for the IME**, and any full-height layout depends
  on it. And **`env(safe-area-inset-bottom)` reads 0** on such a device, because
  the Android view insets the webview rather than the system. The footer's
  `env()` term and the `.has-foot` rule that cancels a doubled one are there in
  case a platform insets differently.
- **What a phone check of the panels should find:** the sheet fills the
  viewport exactly; the head is 48 px on every panel (short form, long form, the
  About panel and each of its tabs); the close button is 44×44 and every footer
  button 44 tall; the body's bottom inset is 16 px, not a doubled safe area; a
  dropdown opened inside the sheet is the topmost element at its own centre; a
  swipe inside a scrolling panel moves its body and leaves `main.scrollTop` at
  0; and a hardware Escape closes the panel.
- **Already mobile-ready by design** (check, don't rebuild): bottom tab bar
  (navigation as data), the native dialog plugin (share sheet on mobile), no
  blocking JS dialogs, `confirmDialog()` everywhere, Svelte bundle size, the
  map loaded as a separate chunk.

### What is left for mobile

- **Demo data in release builds.** `seed_demo_data` works in release builds so
  that release APKs can be tested with data. It **must** be guarded again
  (`cfg!(not(debug_assertions))`) or removed before the stable release.
- **Webview differences.** The scripted checks drive WebKitGTK and Blink, not
  WKWebView or Android WebView. Check the `geo://` protocol's CORS behaviour on
  each platform. On Windows/WebView2, `http://geo.localhost` and CORS work.
- **New crates should cross-compile cleanly for mobile.** That is why
  pure-Rust crates like proj4rs are chosen; keep that bar.
- **Offline municipality packs**, only if use in the field shows the cache
  isn't enough (§3, and `docs/sigpac-integration.md`).

---

## 9. Re-running the query-scope audit and the measurements

`data-model.md` → "Indexes and query scope" has the findings; this is how to
produce them again. **It is a recipe, not a CI check.** Whether a `SCAN` is a
defect or a six-row lookup read in full is a judgement, and an automatic check
would need an allowlist that drifts, and then fail for nothing and get ignored.
Run it before tagging a release and after adding a register.

**Two rules already have tests**: the index pattern
(`src-tauri/tests/contracts/index_contract.rs`) and the per-record child query
(the `query_scope.rs` file in each crate that owns registers, built on
`terrazgo_testkit::query_cost`). Those fail on their own. What follows is for
the third rule, query *scope*, which needs someone to read the plans.

**The plans.** Put the schema together and ask SQLite what it would do:

```
cat crates/terrazgo-core/migrations/0001_core_schema.sql \
    crates/module-phytosanitary/migrations/0001_schema.sql \
    crates/module-fertilisation/migrations/0001_schema.sql \
    crates/module-ecoscheme/migrations/0001_schema.sql > /tmp/schema.sql
rm -f /tmp/plan.db && sqlite3 /tmp/plan.db < /tmp/schema.sql
sqlite3 /tmp/plan.db "EXPLAIN QUERY PLAN <the statement>"
```

For a full sweep, pull every Rust string literal starting with `SELECT` out of
`crates/*/src` and `src-tauri/src`, bind each `?n` with a dummy value, and
`EXPLAIN` it. **Read only the `SCAN` lines, and only for tables that GROW**: a
scan of `unit` or `production_system` is fine, and `SCAN CONSTANT ROW` comes
from `EXISTS`. Then ask of each one: *is the result bounded by the question, or
by the history?* No tool makes that call.

The measurements below are all `#[ignore]`d tests: a timing can't be an
assertion. Run them in release mode (a debug build measures the wrong thing),
and **compare slopes, not milliseconds**: the constant belongs to one machine,
the slope is the finding, and the desktop is the fast case. The reference
figures were taken on the development desktop.

**Read cost as the history grows.** The scaled-data builder is
`crates/module-phytosanitary/tests/common/scale.rs`:

```
cargo test -p module-phytosanitary --release --test query_scope -- --ignored --nocapture
```

It prints the markdown table the data-model doc records.

**What a write costs:**

```
cargo test -p module-phytosanitary --release --test write_cost -- --ignored --nocapture
```

A new record, a correction and a removal, in memory (the CPU, the part the code
decides) and on a WAL file at the app's durability (what a farmer waits for), on
a fresh book and on one holding 20 000 treatment records. The history is
written through the real repositories: filling the log with synthetic keys puts
every new key at one edge of one huge index and reports a cost real data doesn't
have. Reference: about 0.4 ms / 0.36 ms / 0.2 ms in memory, 6.5–10 ms on disk,
where one flush per save dominates.

**What the duplicate list costs to read.** It is worked out on every visit to
the Status view and to a book's page and stored nowhere (docs/sync.md →
Duplicate suspects), so it is measured like a write:

```
cargo test -p module-phytosanitary --release --test duplicate_cost -- --ignored --nocapture
```

A medium farm and a cooperative's year kept as one farm, each with ten campaigns
behind it and one treatment in a hundred of the latest recorded twice, so the
list has pairs to show, plus a worst case where nearly every spray has a real
suspect. The last column is what a form pays right after saving one of those
copies (the pairs one record is in). Two counting tests in the normal suite
(`duplicates.rs`, same crate) keep the Status view's fetches and the check
after a save flat as campaigns pile up. Reference: 4.3 ms for a medium farm,
43 ms for 8 000 treatments in the current books, 98 ms in the worst case, and
0.57 ms and 2.1 ms after a save (docs/sync.md → What it costs).

**What an import costs:**

```
cargo test -p terrazgo-core --release --test merge_cost -- --ignored --nocapture
```

It times the two halves of a delivery separately, because only one belongs to
the merge: **carrying** the log rows in (plain inserts: the transport's cost,
and the control that shows the machine hasn't changed) and **settling** each
register the rows touched. Three shapes, because the merge's two costs pull in
different directions (comparing versions is per register, applying rows is per
row):

| Shape | Carry | Settle |
| --- | --- | --- |
| A campaign, 300 records (305 registers, 1205 rows) | ~4.6–4.9 ms | ~54 ms |
| One register, 10 edits | ~56 µs | ~0.33 ms |
| One register, 200 edits | ~0.66 ms | ~2.8 ms |

In memory, on the desktop. Read the **shape**, not the numbers: settling is a
few hundred microseconds per register and grows with a register's own edit
history, not with the size of the log or the database. A day's work is a
handful of registers.

**How big a sync is:**

```
cargo test -p module-phytosanitary --release --test bundle_size -- --ignored --nocapture --test-threads=1
```

One thread, or the two measurements print into each other. The first prints the
table in `docs/sync.md` → "How big is a sync", for a day's work and for a whole
log at three scales; the second, what acting on alerts adds to the database and
the bundle, and what pruning those acts would do (the tables in "Alert
acknowledgements roam"). It is in this crate rather than core because a
treatment record with its plots, problems and justifications is the widest
register in the schema, so it is the one worth sizing. Reference: about 180–270
bytes per change set at every scale, 5.2 KB for a day's work by three
operators, 872 KB for a cooperative's whole year; 1249 B per alert act in the
database and 49 B in a bundle, +604 B per act pruned.

**Comparing two revisions.** Put each revision in its own worktree (`git
worktree add --detach <dir> <rev>`) and build each with `--release --no-run`
into **its own** `CARGO_TARGET_DIR`. With one shared directory, same-named
crates from the two trees share one artifact, and a test silently links the
other revision's library. Then run the two binaries alternately, A B A B A B,
when no `rustc` is running, and take medians. Disk timings vary by about a
millisecond between runs of the same binary, so read those as a range, not a
number.

**What a corruption check costs.** It writes up to about 513 MB and takes
minutes, so it runs on demand only:

```
cargo test -p terrazgo --test contracts quick_check_cost -- --ignored --nocapture
```

It fills the real composed schema with `record_change` rows, because that is the
table that grows without limit, and prints both pragmas side by side.
Reference: `quick_check` about 1.65 ms/MB, `integrity_check` about 5.8 ms/MB
(it walks every index entry), with the ratio growing from 2.3× at 33 MB to 3.5×
at 521 MB. It answers what the weekly check costs a cooperative-sized book, and
what the Settings button's thorough check costs.
