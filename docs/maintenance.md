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
- **Pinned by:** `crates/terrazgo-siex/tests/common/mod.rs` compiles it once
  per test binary and every `export*.rs` file validates its output against it
  (the `jsonschema` crate, dev-dependency only).
- **Known quirks:** one malformed `$id` (`"##root/…"` under
  SiembraPlantacion → Maquinaria → items) fails draft-07 meta-validation;
  the test harness normalizes it in its in-memory copy only. `CodigoRea`
  and `CodigoSIEX` are exactly 14 characters.
- **On a new version:** download the new docx, extract the schema from the
  OLE object, vendor it next to (then instead of) the old one, re-diff
  field-by-field (the 3.3.0 → 3.11.4 re-diff in
  [siex-export.md](siex-export.md) is the template), update the export
  serializer/tests, and re-check whether the `##root` typo persists.
- **Re-extract the descriptor sheets in the SAME pass, and never read an old
  one.** The docx embeds them, so they move with it — but they are separate
  files once extracted, and nothing makes a stale copy look stale. On
  2026-08-19 every local descriptor was still 3.3 / 3.3.0 (24/05/2024) while
  the schema had been 3.11.4 since July, and reading one produced two wrong
  conclusions about `DatosCubierta` before the versions were checked. Two
  rules fall out. **Keep the version banner**: two of the three extracted
  sheets had had their first row dropped during conversion, which is why only
  the third revealed its age. And when the sheet and the schema disagree,
  reach for the schema rather than reasoning about which version changed
  what — the disagreement is often live rather than historical
  (`ActividadCubierta` is in every descriptor and in no schema).
- **The refresh pays for itself.** That same 3.3.0 → 3.11.4 descriptor diff
  showed `DatosCubierta`'s widths relaxed from required to optional, `Pastoreo`'s
  `FechaFin` tightened the other way, and `AnimalesPropios`/`AnimalesTerceros`
  arriving as **booleans** where two shipped columns had assumed head counts
  ([siex-export.md](siex-export.md) records all three).

### FEGA SIEX catalogues (Anexo VII code lists)

- **Our copy:** `crates/terrazgo-core/catalogues/` — 49 CSVs (≈1.4 MB),
  idTabla filenames, embedded in the binary via `include_bytes!` and imported
  at startup (`terrazgo_core::catalogue::ensure_catalogues`, upsert-only).
- **Upstream, the registry:** `GET
  https://www3.sede.fega.gob.es/bdcsixpor/tablas/configJson` — the public SIEX
  portal's own catalogue registry, and the **authoritative list of what
  exists**: 287 catalogues, each with `id` (the idTabla), `nombreCatalogo`,
  `visiblePortal`, `exportable`, and a `fields` map giving every column's
  `orden`, `type`, `length`, `required` and display `label`. That metadata is
  what settles a file's `code_col`/`label_col`/`identity_attrs` mechanically:
  the field named `codigo` (or `codigoPadre`) is the code column,
  `descripcion` the label, and `numCamposClavePrimaria > 1` means the code
  alone is not unique.
- **Upstream, the data:** public no-auth REST API
  `https://www11.fega.es/bdcsixwsp/` — `GET /catalogos/{idTabla}` (one CSV),
  `GET /catalogos/{idTabla}/fecha` (last-update probe). `GET /catalogos/zip/`
  exists but is **not usable mechanically**: the bundle ships *display-name*
  filenames ("Eficacia del tratamiento.csv"), not idTabla names.
- **Which of the 287 we carry:** the ones a named part of the app reads —
  the record book's coded fields, the declared-crops prefill, the geography
  the export and the report-language offer resolve against, and the
  vocabularies the Fertilization and Irrigation modules will need. Carrying
  all of them would be dead weight in the binary; carrying only what compiles
  today is how slice 8 came to state four times that a catalogue "does not
  exist in the vendored FEGA set" when it existed upstream all along
  (MATERIAL_ANALIZADO, TIPO_ANALISIS, SUST_ACTIVAS, TIPO_TRATAMIENTO —
  vendored 2026-08-05). **When a seam needs a coded field, check the registry
  before concluding the authority publishes no list.**
- **The rule has a second limb (2026-08-11): a catalogue may also be carried
  against a *named, recorded* future consumer** — otherwise "a named part of
  the app reads it" would quietly be false for nine of the 48, which is worse
  than saying so. The nine, all told 7.3 kB, split in two:
  - **Pre-positioned for the eco-scheme registers** (model section 9, duties in
    `cuaderno-print.md`): **this sub-list is now empty** — the arc's four seams
    gave every one of its catalogues a real consumer.
    (**`TIPO_COBERTURA_SUELO` left it on 2026-08-19**: `soil_cover.cover_type_code`
    reads it, offered by the covers form. It is read in a way none of the others
    is — **narrowed per practice**, because RD 1048/2022 art. 42.1.a establishes
    "la cubierta vegetal espontánea o sembrada" (codes 2 and 3) and art. 43.1.a
    "la cubierta inerte de restos de poda" (code 4, and specifically not 5,
    which is nutshells and stones). The narrowing is in the PICKER and never in
    the repository, because this file grows between releases — it gained code 6
    on 27/02/2024 — and a refresh on a user's machine could otherwise leave a
    lawful cover unrecordable. `tests/siex_mapping.rs` accounts for every active
    code, so an upstream addition fails there rather than quietly becoming
    unreachable from the UI.)
    (**`DEST_RES_VEG` left this list on 2026-08-19**: `cultural_operation.residue_destination_code`
    reads it, offered by the operations form. Its value 9, "Trituración de
    restos de poda y depositado sobre el terreno", is the one code in this app
    with meaning beyond display — it *is* art. 43's P7 practice, so a pruning
    recorded against it is what evidences an inert cover. Named once, as
    `module_ecoscheme::siex::RESIDUE_LEFT_ON_GROUND`, rather than left as a
    literal at each reader.)
    (**`TIPO_LABOR` left this list on 2026-08-18**: `module-ecoscheme`'s owned
    `cultural_operation_kind` maps onto it, and `tests/siex_mapping.rs` pins
    the map in both directions — the second being a watchdog that fails when
    FEGA adds a code no owned kind claims. It is the one catalogue this app
    maps rather than reads, so its consumer is a contract test as much as a
    picker.)
    (`EST_FENOLOGICO` was on this list until 2026-08-12 and has left it: it now
    has a real consumer, `treatment_plot.growth_stage_code`, read by the
    treatment form's picker and by both renderers of section 3.1. Note its code
    is NOT the BBCH stage — the monograph's 0-9 sits in its own
    `Estadio bibliografía` column, so readers go through
    `module_cue::catalogue::growth_stage`.)
  - **Orphaned by the parked DGC path**: `MATERIAL_VEGETAL_REPRODUCCION`
    (**re-checked 2026-08-19 and again 2026-08-21 — still orphaned, and the
    second check corrected the first one's reasoning**. The 2026-08-19 note said
    the twin's required `SiembraPlantacion: integer` is Anexo V's calculated
    "Cultivo", so the crop rather than the reproductive material. It is neither:
    the WS descriptor types that member `number(1)`, *"1 Siembra 0 Plantación"*,
    and it is filled from `sowing_record.kind_code` since seam 2. Anexo V's
    "Cultivo" is `DGCs[].CodigoCultivo`, per-DGC. The conclusion is unchanged and
    now firmer — no field anywhere reads this catalogue, and the candidate
    consumer the eco-scheme arc named for it does not exist),
    `PROC_VEGETAL`, `REGIMEN_TENENCIA`, `DESTINO_CULTIVO` and `DEST_COSECHA`
    have no field anywhere in the cuaderno exchange schema — they are REA/DGC
    declaration vocabulary. Kept because the cost is negligible and the path is
    dormant rather than dead; drop them if it is ever abandoned outright.
- **FEGA has TWO surfaces and only this one is machine-readable** (checked
  2026-08-19, while planning the eco-scheme arc). The 287-catalogue registry
  above is an API; FEGA's *"Fuentes Fitosanitarias"* page (`/bdcsixpor/ffii`)
  is an index of links to six external MAPA registries — ASPAFITOS, MDF,
  REGMAQ-ROMA, ROPO, REGANIP, REGITEAF — and **none of them has an interface**.
  Its own `FuentesInformacion.zip` sits behind a reCAPTCHA; ASPAFITOS
  (`servicio.mapa.gob.es/regfiweb`) is a server-rendered ASP.NET app with no
  JSON surface and no advertised bulk export; REGMAQ-ROMA
  (`servicio.mapa.gob.es/regmaq/buscar.wai`) answers a form POST with HTML;
  ROPO offers a bulk file that has not been updated since 2024. So the barrier
  is the interface, not the data — the same shape as the 2026-08-02 CUECYL
  answer. **Scraping is not an acceptable interface for this project**, so
  these stay unused *as data sources*; if a product- or machinery-registry feed
  is ever wanted, the route is to *ask* (`sgmpagri@mapa.es` is published on the
  registry itself, and writing to `cuecyl@jcyl.es` is what settled the export
  design). **They are used as destinations, though** — this negative is what
  produced the registry hints (2026-08-26): if the app cannot fetch a registry,
  it can still say which one holds a number and open it. The URLs live in
  `src-tauri/src/external_links.rs`; see
  [architecture.md](architecture.md) → "Pages the app points at, and never
  talks to".
  Recorded so the question is not re-derived, and so the standing rule above is
  read correctly: "check the registry" means the catalogue API, which is the
  only surface that answers.
- **`ESPECIE_ANIMAL` joined the set 2026-08-18** (198 rows, 10 kB), with the
  grazing register that reads it: model 9.1's "Especie animal que pasta" and
  `Pastoreo.Animales[].Especie`. It carries **no lifecycle columns at all** —
  no alta, modificación or baja — so every row is permanently active, the
  `TIPO_MAQUINA_UNE` precedent; a row count that *drops* on refresh therefore
  means a truncated download, never an upstream retirement.
  **`RAZAS` is deliberately NOT vendored**, recorded here so it is not
  re-derived: neither model 9.1 nor `Pastoreo.Animales[]` asks for a breed, and
  the selection rule needs a named consumer.
- **`EDIFICACIONES_INSTALACIONES` got a real consumer on 2026-08-21**, and the
  one it had been held against was a guess. It was vendored for
  "`Edificaciones[].IdEdificacion` typing (3.4)" — but `IdEdificacion` turns out
  to be REA's own key for a registered installation, not a class code
  (`siex-export.md` settles it). The genuine consumer is `premises.class_code`:
  Anexo V's REA bloque 8 field 1 makes the class obligatory *"en caso de …
  tratamiento … en las edificaciones e instalaciones que conlleve su
  identificación para la cumplimentación del CUE"*, which is models 3.4/3.5.
  Worth recording as a caution about the selection rule's second limb: a
  catalogue *held against a named future consumer* is only as good as the
  reading behind the name, and this one survived four months on a wrong one.
  All 109 rows are real estate, so the column is buildings-only.
- **`MUNICIPIO_SIGPAC` joined the set 2026-08-11** (8 434 rows, ~340 kB — a
  third of the whole snapshot, and the largest single file we carry). Model
  section 2.1 asks for the término municipal as "código y nombre" while the
  SIGPAC provider returns no name in any response, so a catalogue is the only
  route. It is **composite-keyed on `Código de provincia`**, which is not a
  nicety: municipality codes repeat across provinces, so an unqualified lookup
  returns a real town in the wrong province. See `cuaderno-print.md` → 2.1.
- **Adding or removing a vendored catalogue.** Everything at *runtime* is
  driven off `VENDORED`, so nothing needs wiring by hand: startup import
  (`ensure_catalogues`), the Settings status list (`catalogue_status`) and the
  refresh button (`refresh_catalogues` over `vendored_ids()`) all iterate it,
  and the UI renders whatever comes back. To **add** one: drop the byte-verbatim
  CSV in `catalogues/`, add the `Vendored` entry (its `headers` pinned in full —
  the failing test prints the paste-ready block), bump the array length, and add
  the id to `VENDORED_IDS` in `tests/catalogue.rs`, which fails in **either**
  direction if the two drift. The row-count floor in that file moves too.
  `every_vendored_file_refreshes_to_unchanged_against_itself` is the breadth
  guard a new entry trips first: a `code_header`, `label_header` or
  `identity_attrs` that cannot re-derive the file shows up there rather than in
  whatever feature prompted the addition.
- **Removing one is not symmetrical, and this is the part to think about.**
  `reconcile` never deletes and nothing anywhere issues a `DELETE FROM
  catalogue`, so dropping a file from `VENDORED` stops it being imported and
  listed but leaves its rows in **every database that already has them**. That
  is deliberate — a code stored on a years-old record must still resolve at
  inspection time — but it means a removal splits the population: existing
  installs keep resolving the codes, a fresh install never had them and prints
  the raw code instead. Only remove a catalogue no stored record can reference. The data host
  serves non-visible catalogues without auth too (FEGA data is public-sector
  information). The 146 non-visible entries are mostly internal load tables
  (`ARIES_*`, `RIIA_*`), `*_VISTA` views and metacatalogue admin rows, and
  several 404 on the data host. Treat the flag as "less curated": e.g.
  `USOS_AGUA` heads its retirement column `Fecha Baja`, not the
  `Fecha de baja` every visible catalogue uses, which the parser would miss.
- **Refresh, two paths.** *In the repo* (the authoritative one): a
  release-ritual step (§6 step 1) — enumerate from the registry, fetch each
  vendored idTabla, replace byte-verbatim, run the tests. *In the app*
  (2026-08-09): a **manual** button in Settings → "Catálogos de referencia"
  fetches the same endpoint per idTabla and adopts what passes, so a user
  whose provider published a code last month does not have to wait for a
  release. No timer and nothing at startup — reference data underpins records
  with legal value, and rewriting it unasked on a rural connection is the
  wrong default.
- **The in-app refresh validates BEFORE it adopts**, and that ordering is the
  design, not a nicety: `reconcile` never deletes, so a bad file adopted once
  leaves bogus rows in every picker forever and no later good file can remove
  them. `terrazgo_core::catalogue::refresh_catalogue` therefore runs, in
  order: digest (identical bytes → `Unchanged`, nothing parsed) → shape →
  every row has a label → no control characters → **the row count must not
  shrink** (codes are baja-dated, never removed, so a shorter file is a
  truncated download) → only then the transaction. Refusals are **per file**:
  one retired idTabla or one unreachable host must not deny the user the other
  46 catalogues' updates.
- **Strict in CI, tolerant-with-report in the field.** The vendored files are
  held to the complete pinned header row, in order (`validate_shape`); a
  *fetched* file is held to "every column the app reads still resolves by
  name, exactly once", and anything the file has gained beyond that is adopted
  and named back to the user. A rename still refuses either way — it shows up
  as a *missing* pinned header — while refusing a harmless appended column
  would leave users unable to update at all until the next release. A shape
  refusal is an **app-update event, not a data-update event**, and the message
  says so: the app reads named columns and cannot adapt at runtime.
- **Robust by construction, then pinned.** Columns are resolved **by name**,
  never by index (2026-08-08): `Vendored.code_header` / `label_header` /
  `identity_attrs`. That removes the inserted / removed / reordered-column
  class outright — the importer finds its columns wherever they now sit — and
  leaves only *renames*, which `Vendored.headers` pins as the file's complete
  header row, checked by `validate_shape` on **every** parse (so the same rule
  will guard a fetched refresh, not just the vendored files).
  Why positions were not good enough: injecting one leading column into
  `MAT_FERTI` left all twenty other catalogue guards passing while every stored
  code became the contents of the column beside it.
  Why renames must be pinned even where nothing seems to read the column: the
  parser matches `Fecha de alta` / `de modificación` / `de baja` **by name in
  the 40 files that carry them** (a rename silently loses retirement dates, so
  retired codes stay in every picker), and four crates read `attrs` keys by
  name (`Ámbito`, `Uso SIGPAC`, `Código SIEX`, the 19 composition columns).
  FEGA's own variance is real — `USOS_AGUA` heads it `Fecha Baja`.
- **Updating a pinned entry is a REVIEW, never a fix.** The failing test prints
  the paste-ready `headers: &[…]` block; read what the provider moved and
  decide whether the app still reads what it thinks it reads *before* pasting.
  Regenerating blindly to get green defeats the entire mechanism.
- **What each guard catches ALONE** — no single one covers the field:

  | Guard | Catches |
  | --- | --- |
  | name-based resolution (`catalogue.rs`) | inserted / removed / reordered columns, by construction |
  | `Vendored.headers` + `validate_shape` | any rename, incl. lifecycle columns and named attrs; delimiter/BOM damage surfaces here too |
  | `column_index` ambiguity check | a file carrying the same header twice |
  | `identity_attrs` resolution | a vanished qualifier column (refuses the import) |
  | row-count-per-file (`tests/catalogue.rs`) | a wrong `identity_attrs` — otherwise silent, the upsert collapses rows onto one id |
  | every-row-has-a-label | a wrong label column choice |
  | control-character tripwire | the provider leaving Windows-1252/UTF-8 |
  | bidirectional `siex_mapping.rs` contract tests | a small closed catalogue gaining, retiring or renumbering a code |
  | sentinel `(code, label)` pins | renumbering in the open lists (EFICACIA pinned in full; "Aceitunas"; MACRONUTRIENTES 1/6/9) |
  | `UNMAPPED_COLUMNS` (module-fertilisation) | a **new** column in the one file read column-wise that nobody has decided about |
  | `catalogue.source_digest` | that a *fetched* file differs from the stored copy — detection only, never validation |
  | pre-write checks in `refresh_catalogue` | a fetched file that is empty, truncated, unlabelled or mis-encoded — before anything is written, since the upsert cannot be undone |

- **A code that vanishes from the provider's file leaves the pickers, but is
  never removed** (`catalogue_code.absent_since`, 2026-08-27). FEGA retires a
  code with a baja date, so a row that simply disappears is unexplained: we
  keep it, because a years-old record still cites it and must still resolve,
  and we stop offering it. **Only a FETCHED file may set the mark** — it is the
  provider's current list, so absence from it is evidence; a *vendored* file
  proves nothing, since a code can be missing from it merely by being newer
  than the release, and inferring there would hide every code a refresh had
  added at the next app update. Both origins clear it: presence is presence.
  `absent_since` is **ours** and deliberately separate from `retired_on`, which
  is the authority's — so when a dropped row comes back baja-dated, our mark
  clears and theirs takes over, and the code stays out of the picker for a
  reason someone actually stated. `active_codes` filters both; `find_code`
  filters neither. The count is reported per file as `withdrawn`.
- **Known interaction with the shrink guard:** the guard compares a new file
  against *every* stored row, marked absent or not, and stored rows only ever
  accumulate. So once a device has withdrawn a code and adopted a replacement,
  a file carrying only the older set is refused as a truncated download. The
  release path is unaffected (a re-vendor is reviewed by hand); it costs a
  user an in-app refresh they could otherwise have taken. Left as is —
  counting only non-absent rows would fix it, and is a change to what the
  guard *means*, not a tidy-up.
- **Rejected: garbage-collecting the codes a refresh added that nothing uses.**
  It would need a hand-maintained registry of catalogue → the subset of ~75
  `*_code TEXT` columns storing its codes, because the schema deliberately
  carries **no foreign keys from user data to `catalogue_code`**. Those columns
  live in four modules core may never depend on, so the registry would have to
  arrive through a new `Module` trait method — and its failure mode is silent
  and destructive: a later column that stores codes and is not registered makes
  them look unused, so an existing record stops resolving. Nothing in the
  schema marks a column as catalogue-backed, so no scan can catch the omission.
  The benefit also inverts: open-catalogue pickers are built straight from
  `active_codes`, so an extra code in a picker *is* what the refresh was for,
  while closed-list pickers read the lookup tables and never the catalogue, so
  an extra row there never reaches the UI at all. And the row a collector would
  remove is the same row that keeps a years-old treatment readable.
- **What NO guard catches, so it is not mistakenly believed covered:** in-band
  semantic drift — same header, same code, different meaning. The
  `DETALLE_MATERIAL_FERT` heavy-metal columns are exactly this (one column
  carrying percentages for some products and mg/kg for others), and it lives
  in the provider's data, not in our reading of it. The mitigations are human
  review of a refresh diff (which the pin forces) and the registry
  cross-check below.
- **What a refresh is worth over time, and why startup keys on the app
  version.** `ensure_catalogues` imports the vendored snapshot on the **first
  launch of each app version** (`catalogue.imported_by_version` vs
  `CARGO_PKG_VERSION`, compared by equality so a downgrade also re-imports)
  and does nothing on the launches after. So **a user's refresh survives every
  restart, and does not survive an app update** — the next version's first run
  restores its own copy over the top. That is deliberate: the vendored files
  are curated *as a set*, and the mapping bijections and catalogue suites are
  green against one exact snapshot, so a device must never run one refreshed
  file mixed with the rest of an older release's set. What an update restores
  is every label, attribute and lifecycle date; codes a refresh *added* stay,
  because the import cannot delete without breaking the promise that a code
  already written onto a record keeps resolving.
  Before 2026-08-27 startup compared `source_digest` instead — which the
  refresh also writes, so an adoption silently lasted exactly one session:
  corrections reverted at the next launch while added codes stayed, leaving a
  state neither file ever was.
- **Dev-time consequence:** re-vendoring a CSV *without* bumping the version no
  longer triggers a startup re-import. The release ritual re-vendors and bumps,
  and the pinned-header tests parse the vendored bytes directly rather than the
  database, so this only bites a running dev app — where the standing rule for
  a `0001` edit is already "delete the dev database".
- **Refresh detection:** `catalogue.source_digest`, an FNV-1a hash of the bytes
  that produced the stored rows, compared *before* parsing, so re-offering a
  copy already held costs nothing. Bytes rather than the file's newest
  lifecycle date, which was wrong in both directions — it parsed every file
  before it could decide to skip it, and it silently ignored a refresh that
  corrected a label without moving any date (several catalogues ship no dates
  at all).
- **Known quirks:**
  - Documented as ISO-8859-1 but really **Windows-1252** (€ at 0x80 in
    UNIDADES_MEDIDA).
  - Codes are baja-dated, never deleted.
  - The crop catalogue is `PRODUCTOS` (not "CULTIVO"), and it is **not** the
    same list as `PROD_VEGETAL` — that one codes the harvested *produce*
    ("Aceitunas") and cross-references the crop ("OLIVO"). SIEX's
    `ProductoVegetal` / `ProductoCosechado` fields mean the latter.
  - `COMUNIDAD_AUTONOMA` publishes the **seventeen comunidades only** — the
    ciudades autónomas of Ceuta (INE 18) and Melilla (19) are absent, so a
    holding there has no `CAExplotacion` value to send
    (docs/siex-export.md → "Recorded gaps"). It also carries **two
    incompatible codings**: it is keyed on
    the catastro code, but SIEX `CAExplotacion` wants INE, and they diverge
    for 10 of the 17 communities (Castilla y León is catastro 08 / INE 07 —
    and INE 07 is Castilla-La Mancha in the catastro coding). We key on INE.
    FEGA also publishes a municipal-level INE↔catastro equivalence per
    campaign as a content page (search "relación de municipios por CCAA con
    equivalencias entre los códigos INE y catastro"); no API.
  - Several catalogues carry **no lifecycle dates** at all (`TIPO_MAQUINA_UNE`,
    `USO_SIGPAC`, `PROVINCIA`, `COMUNIDAD_AUTONOMA`, `PAIS`,
    `REGIMEN_TENENCIA`, `DETALLE_MATERIAL_FERT`).
  - Codes repeat, per qualifying attribute, in `BUENAS_PRACTICAS_AMBITOS`
    (ámbito), `CULTIVO_USO_SIGPAC` (uso), `PROD_VEGETAL` (crop) and
    `MATERIAL_VEGETAL_REPRODUCCION` (detalle).
  - Some files lead with their **parent** catalogue's code rather than their
    own: `EDIFICACIONES_INSTALACIONES` (tipología) and
    `DETALLE_MATERIAL_FERT` (MAT_FERTI type).
  - `TIPO_TRATAMIENTO` has no code 1; `DETALLE_MATERIAL_FERT` leaves its own
    `descripcion` column blank on 83 rows.

### SIGPAC service fixtures (test data)

- **Our copy:** `crates/module-sigpac/tests/fixtures/` — real 2026 responses
  (recinto by reference/point, zone intersections, the `/geopackages/`
  campaign listing HTML, and the `cultivo_declarado` declarations harvested
  2026-08-03: a plain line, an empty answer, one with a secondary crop, one
  recinto declared in two lines).
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
`terrazgo-net` — the process-wide HTTP agent, its platform-verifier TLS
policy and the Android bootstrap that policy needs. It has exactly two
consumers. **The map** (and everything riding it: SIGPAC lookups, zone
checks, crop prefill) goes through `terrazgo-geo`'s cache-through fetch,
serving the `geo://` protocol, whose allowlisted registry is
`crates/terrazgo-geo/src/sources.rs` — a service not listed there cannot be
reached; once seen, responses are cached, so a dead provider degrades to
"works offline on cached data", never to a broken app, and fresh installs and
never-seen areas are what actually break. **The catalogue refresh** calls the
seam directly from the shell (`src-tauri/src/catalogues.rs`), because its
payload lands in the app database via core's upsert importer and there is
nothing for a tile cache to do with it; it is manual, and refuses per file.
Service rule when replacing a source: prefer the most modern, bandwidth-frugal
offering (MVT > WMTS > WMS).

| Service | Endpoints | Consumer | If it dies / notes |
| --- | --- | --- | --- |
| OpenFreeMap (vector base map) | `https://tiles.openfreemap.org/styles/liberty` (style, rewritten in Rust), `/planet` (TileJSON → dated tile URLs), `/fonts/…`, `/sprites/…`, `/natural_earth/ne2sr/…` (backdrop) | `sources.rs` + `style.rs` | Free OSM-tile host with no SLA. Replacement = any MapLibre-style vector provider: new registry entries + a style rewrite in `style.rs`. Tile URLs carry a dated planet snapshot resolved from the TileJSON at fetch time — a stale cached style keeps working because tiles are cached too |
| IGN PNOA (orthophoto base) | `https://www.ign.es/wmts/pnoa-ma` (WMTS, GoogleMapsCompatible) | `sources.rs` | Spanish state provider, stable. Alternative would be another national WMTS or ESA/commercial imagery |
| Nube de SIGPAC MVT (parcel fabric overlays) | `https://sigpac-hubcloud.es/mvt/{layer}@3857@pbf/{z}/{x}/{y}.pbf` — layers `recinto`, `cultivo_declarado`, `e_paisaje_area/_linea/_punto`; previous campaign under `/mvt/anterior/` | `sources.rs` (campaign-keyed cache rows) | z12–15 only; empty tiles answer 404 (cached as empty); the fixed path serves the *current* campaign except `cultivo_declarado` (previous). CC BY 4.0 — attribution must stay |
| SIGPAC REST (lookups + zones) | `https://sigpac-hubcloud.es/servicioconsultassigpac/query` (recinto by ref/point), `…/intersection` (nitrate/phyto/Natura zone checks) | `crates/module-sigpac/src/client.rs` (through the geo seam) | Writes `plot_zone_flag` (stored truth — a dead service stops *new* checks, stored flags and alerts survive). REST responses speak hectares, MVT surfaces are m² |
| SIGPAC declared crops (crop prefill) | `https://sigpac-hubcloud.es/ogcapi/collections/cultivo_declarado/items?f=json&provincia=…&recinto=…&exp_ano=…` (OGC API Features; the 7 reference parts + `exp_ano` are queryables) | `crates/module-sigpac/src/client.rs` (through the geo seam) | Serves the PREVIOUS campaign — the client asks current, falls back to current−1, and labels every proposal with the campaign that answered. `exp_ano` is filterable but absent from responses. Nothing declared = HTTP 200 + `numberMatched: 0`. `parc_supcult` is in m². Cache key `sigpac/cultivos/{campaign}/{ref}`, never evicted. CC BY 4.0 |
| SIGPAC campaign resolution | `https://sigpac-hubcloud.es/geopackages/` (directory listing; max year dir = current campaign) | `terrazgo_geo::fetch::current_campaign` | The only machine-readable statement of the campaign; keys every campaign-keyed cache row. If the listing format changes, campaign rollover detection breaks first |
| FEGA BdcSixWsp (SIEX public API) | `https://www11.fega.es/bdcsixwsp/` — `/catalogos/*` (see §1), `/fuentesInformacion/zip` (MDF non-chemical defense registry; ROPO excluded — personal data), `POST /existeNIF` (NIF → explotaciones with SIEX/REA codes) | the release ritual, and — since 2026-08-09 — the manual in-app catalogue refresh (`src-tauri/src/catalogues.rs`, one GET per vendored idTabla); `/existeNIF` is the future REA-code prefill seam | No auth. Guide: "BdcSixWsp — Guía de Servicios públicos de Siex" (asset of the sede portal, §4) |
| GitHub releases API (website only) | anonymous `releases` endpoint of the public repo | `site/` download card | Drafts are invisible (links fill on publish); `releases/latest` is useless while every release is a pre-release. Static fallback = the releases page |

## 3. User-supplied file formats (offline seams)

| Format | Source the user gets it from | Consumer | Notes |
| --- | --- | --- | --- |
| Boundary files: GeoJSON, GeoPackage | Anywhere — including the SIGPAC download service `https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html` (provincial recinto + cultivos-declarados GPKGs, CC BY 4.0) | `terrazgo_geo::import` | Geographic SRS only (EPSG 4326/4258/4081); projected files fail with `gpkg_unsupported_srs` — the pre-agreed proj4rs reprojection path exists if real projected files appear |
| Cultivos-declarados GPKG (bulk) | Same download service, current + previous campaign | not used — the per-reference OGC API in §2 serves the crop prefill instead (112–545 MB per province is not a rural-connectivity download) | Model: SIGPAC ref + `PARC_PRODUCTO` + `PARC_SISTEXP` + `PARC_SUPCULT` + geometry ([model page](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html)) |
| REACYL DGC Excel export (future) | The titular's own REACYL DGC module (certificate login) | not built yet | Columns/`CodigoDGC` presence unconfirmed (CUECYL question); `.xlsx` reading would need the calamine crate — decide before coding |

## 4. Official documents & regulatory sources

The implementation cites these; when behavior and document disagree, check
whether the document moved to a new version first.

| Document | Where to (re-)fetch | What implements it |
| --- | --- | --- |
| FEGA SIEX technical docs — Anexo V (fields), VI (interface + schema), VII (catalogues), IX/X (authorizations) | <https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex> | the whole export (`terrazgo-siex`, `module_cue::siex` and each module's own) |
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
- **Setting the CI secrets:** pipe them from the verified `keystore.properties`
  / keystore file, never type them by hand. A hand-typed password (invisible
  trailing whitespace) failed the android job with "keystore password was
  incorrect" on the v0.1.5 tag; re-setting all three secrets programmatically
  from the locally verified values fixed it with `gh run rerun --failed`, no
  re-tag needed. Verify the password against the keystore with `keytool`
  first, and never echo the value.
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

0. **Bump the version** — not external data, but this is where the checklist
   lives. Three manifests must agree with the tag: `Cargo.toml`
   (`[workspace.package] version`, which every crate inherits), `package.json`
   and `src-tauri/tauri.conf.json`. Refresh both lockfiles afterwards
   (`cargo check` and `npm install --package-lock-only`) or the release commit
   ships a lockfile still naming the old version. Nothing under
   `src-tauri/gen/android/` needs editing — `tauri.properties` and the copied
   `tauri.conf.json` are generated at build time and untracked. The check is
   enforced, not remembered: `release.yml`'s first job fails the release if any
   of the three disagrees with the pushed tag, before anything is published.
   One place *nothing* enforces it: the scripted frontend checks' fixture file
   carries a harvested `get_status.app_version`, so it names the previous
   version until someone updates it. It is a displayed value with no assertion
   on it, so it fails nothing — which is exactly why it needs a line here.
1. **Refresh the catalogue snapshot**: enumerate the registry
   (`GET https://www3.sede.fega.gob.es/bdcsixpor/tablas/configJson`), then for
   each idTabla in `catalogue.rs`'s `VENDORED`
   `GET https://www11.fega.es/bdcsixwsp/catalogos/{idTabla}` and replace the
   file in `crates/terrazgo-core/catalogues/` byte-verbatim; run `cargo test`
   — the row-count and label guards, the snapshot-fact tests, the encoding
   tripwire and the `siex_mapping` contract tests are designed to fail loudly
   on provider drift instead of shipping it silently. Do **not** use
   `/catalogos/zip/`: it ships display-name filenames, not idTabla names (§1).
   While the registry is open, skim it for catalogues a seam since the last
   release assumed did not exist (§1). The in-app refresh does not replace
   this step: it updates a *user's* database, never the repository's vendored
   files, so a fresh install still ships whatever was last committed here.
2. **Check the CUE schema version** on the FEGA documentation page. If it
   moved past 3.11.4: vendor + re-diff before the next export-touching
   release (procedure in §1).
3. **Android**: the release APK/AAB must be signed (the CI job fails if the
   keystore secrets are missing — never work around it by shipping unsigned
   or debug-signed builds), and the AAB goes to the Play internal-testing
   track (§5).
4. **Run the storefront export guard** (`packaging/export-storefront.sh` into a
   scratch directory) *before* tagging. It fails on any surviving reference to
   the scrubbed dev tooling, and it is cheap; discovering the failure from a red
   release instead costs a tag. It caught a stale doc line on `main` before
   v0.1.6 that would have aborted the release after the tag was pushed.
5. **Verifying the published attestations** (provenance + SBOM, one per
   installer digest): the GitHub attestations API no longer inlines the
   bundle — it returns `bundle: null` plus a `bundle_url` pointing at an Azure
   blob served as `application/x-snappy` (raw-snappy-compressed JSON), so the
   old `gh api … --jq .attestations[].bundle` decode path is dead. Fetch the
   `bundle_url`, snappy-decompress it, then read the DSSE payload. Provenance
   subjects are split per runner (the Linux job attests AppImage + deb + rpm,
   the Windows job the exe + portable zip) — expected, each runner attests what
   it built.
6. **Read the package metadata back, not just the build log.** A package is
   metadata plus a payload and only the payload is compiler-checked, so the
   parts a farmer actually reads can be wrong while everything builds green.
   Parse the rpm header and the deb control file and check the summary,
   description and `Categories=` in the generated `.desktop` entry. This is not
   hypothetical: `bundle` declared no descriptions until 2026-08-15, so tauri
   fell back to the *crate* description and every deb since the first release
   shipped "module registry, migration runner, Tauri commands" as the comment
   in the user's application menu, with no category to file it under.
7. Glance at this file: does every row still match reality?

### Linux packages — why one rpm is enough

The Linux job builds an AppImage, a deb and an rpm; adding a format is one word
in `bundle.targets`. The rpm needs **no `rpmbuild` and no new system dependency
on the runner** — tauri-bundler writes the package with the pure-Rust `rpm`
crate.

Its `Requires` come out as **sonames** (`libwebkit2gtk-4.1.so.0()(64bit)`,
`libgtk-3.so.0()(64bit)`) rather than package names. That is what lets a single
artifact install on Fedora, openSUSE and RHEL alike: those distros disagree
about what the packages are *called*, but not about what the libraries are
named. So resist the urge to add per-distro builds until a real install failure
asks for one.

## 7. Dependency advisories and yanked crates

The `audit` CI job runs `cargo-deny` over `Cargo.lock` (advisories only;
`deny.toml` carries every `ignore` with a reason and a revisit condition).

**RESOLVED 2026-08-24, by the cheapest exit in the table below.** From
2026-08-20 this job was deliberately red: `arrayref 0.3.9` was yanked, reached
as arrayref ← tiny-skia/tiny-skia-path ← krilla/resvg ← typst-pdf ←
`terrazgo-report`. It was a *yank*, not a vulnerability — no RustSec advisory
ever existed — and nothing in this repo could fix it, since `tiny-skia-path
0.12.0` requires `arrayref = "0.3.9"` and every 0.3.x from 0.3.5 up was yanked,
so `cargo update -p arrayref` failed outright. Leaving it red was chosen over an
`ignore` entry so the guard stayed honest.

**It went green on its own, and the lockfile proves which exit was taken**:
`arrayref` is still pinned at the same `0.3.9`, so the version did not move —
the yank was lifted upstream. Cost: nothing, not even a lockfile line. That is
row one of the table below, and it is why leaving the job red rather than
ignoring the advisory was the right call: the guard reported the fix by itself.

**Two things worth keeping from the episode.** A permanently red badge hides the
next real failure, so **check WHICH job failed before reading red as "same as
yesterday"**. And detection is inverted for a yank: the job goes green by itself
the day the crate leaves the tree or is un-yanked, but nothing in the repo
announces it — `cargo deny check advisories` locally is how you find out, and
`cargo tree -i arrayref` says whether the crate is still reached at all.

The three exits, kept because the next yank will need them:

| If upstream… | What we do | Cost |
| --- | --- | --- |
| `arrayref` publishes 0.3.10, or un-yanks | `cargo update -p arrayref` | one lockfile line |
| `tiny-skia` drops it in a **0.12.x** patch | `cargo update -p tiny-skia -p tiny-skia-path` | lockfile only |
| `tiny-skia` fixes it only in **0.13** | krilla/resvg and typst-pdf must bump first, then `typst = "0.15"` in `Cargo.toml` | a real dependency change |

None of the three was ever needed: on 2026-08-24 the crate was simply
un-yanked at the version already in the lockfile.

## 8. Android build, on-device findings and the mobile backlog

Moved here from the project instructions on 2026-08-22: build recipes and
machine setup are procedures, which is what this notebook is for. The mobile arc
itself is still an open candidate — see the project instructions' open-items
table.

Collected 2026-07-08, kept in one place rather than scattered across decision
rows.

**Android bootstrap DONE (2026-07-17)** — the app builds and packages for
Android: identifier renamed to `org.terrazgo.app`, Gradle
project generated at `src-tauri/gen/android/` (tracked in git; only
`gen/schemas/` stays ignored), shell crate is `crate-type = ["staticlib",
"cdylib", "rlib"]` with `#[cfg_attr(mobile, tauri::mobile_entry_point)]` on
`run()`. Debug APK (163 MB, debug-keystore-signed, installable):

```
export JAVA_HOME=~/APPS/android-studio/jbr ANDROID_HOME=~/Android/Sdk \
       NDK_HOME=~/Android/Sdk/ndk/28.2.13676358 CARGO_PROFILE_DEV_STRIP=debuginfo
cargo tauri android build --debug --target aarch64 --apk
# → src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
# install: ~/Android/Sdk/platform-tools/adb install <apk>   (USB debugging on)
```

Machine setup that made it work (this dev machine): cmdline-tools +
NDK 28.2.13676358 installed into `~/Android/Sdk`; Java = Android Studio's
bundled JBR (`~/APPS/android-studio/jbr`); rustup Android targets added.
Two gotchas: `CARGO_PROFILE_DEV_STRIP=debuginfo` keeps the debug `.so` at
156 MB instead of ~600 MB (env-only — the repo profile is untouched), and
Gradle's incremental packaging can leave a replaced `.so` as dead space in
the APK (611 MB mystery) — `rm -rf src-tauri/gen/android/app/build` before
judging APK size. Release-build signing is WIRED (2026-07-19, see the Android signing decision): a release build
signs itself when `gen/android/keystore.properties` points at the
developer's keystore, and the storefront `build.yml` has an `android` job
gated on the keystore secrets.

**On-device state** (field-tested on the published v0.1.5 release APK, and on
locally built release APKs for everything landed since).
Working: install, launch, real logo, bottom tab bar, IPC/views, **native
date inputs**, base map over upstream HTTPS, SAF file saves, GPS locate +
live tracking. The two 2026-07-17 field bugs — blank maps (uninitialized
`rustls-platform-verifier` panicking on Android) and 0-byte SAF saves — were
fixed 2026-07-18 and are field-verified; the durable design lives in the TLS trust policy and Android
user-file access decisions. The **native date
inputs** in that list are history — they are owned controls since 2026-08-14,
accepted on a real phone.

**The Android distribution path is proven end to end (2026-08-07)**: build →
sign → Play → device. The Play Console internal-testing track is live, the first
AAB was uploaded by hand (Play requires that), and the app installed *from Play*
onto a phone and ran. Custody and recurring Play chores are in
`docs/maintenance.md` §5; the consequence to remember is in the Android signing
row — a Play install and a sideloaded APK carry different signatures and can
never update over each other, so a test phone is on one channel or the other.

- **gen/android is hand-edited**: `app/build.gradle.kts` (verifier
  Maven repo + dependency + release cleartext + signingConfigs),
  `app/proguard-rules.pro` (keep rule) and
  `app/src/main/java/org/terrazgo/app/MainActivity.kt` (system-bar insets
  padding, 2026-07-23 — keeps the app between the status bar and the
  gesture area; strip color must match `--panel`) carry changes a
  `cargo tauri android init` regeneration would WIPE — never regenerate
  blindly; re-apply from git if the project is ever re-inited.
  `tauri.settings.gradle` stays autogenerated (the CLI added
  `:tauri-plugin-fs` and `:tauri-plugin-geolocation` itself).
- Release-build unknowns RESOLVED by the 2026-07-21 field test of the
  signed release APK: `http://*.localhost` serving works in release
  (protocol interception precedes the cleartext block) and R8 was innocent.
  The real release-only bug was CRL-over-cleartext — release now allows
  cleartext like debug; full root cause in the TLS trust policy decision
  row. Field-confirmed on the published v0.1.5 APK.
- On-device debugging recipe that cracked it (debug builds have WebView
  inspection enabled): `adb shell pidof org.terrazgo.app` → `adb forward
  tcp:9222 localabstract:webview_devtools_remote_<pid>` → page list at
  `http://127.0.0.1:9222/json`; drive the page over raw CDP with Node's
  built-in WebSocket (`Runtime.evaluate` — puppeteer-core's `connect`
  stalls against Android WebView, don't retry it); Rust panics appear in
  `adb logcat` under `RustStdoutStderr`.

- **P5 GPS point query + live tracking — SHIPPED 2026-07-23**:
  `tauri-plugin-geolocation` 2.3
  mobile-gated, platforms-gated `capabilities/mobile.json`, `is_mobile`-gated
  locate button in the map's SIGPAC panel feeding the existing point-lookup
  flow, plus a toolbar follow toggle streaming `watch_position` into a
  dot+accuracy marker on the canvas. Field-tested outdoors: locate and follow
  both work; the camera teleports instead of easing so intermediate frames
  can't flood the tile pipeline. The `plugin:geolocation|*` commands are
  session-stub territory for scripted checks (like `plugin:dialog|save`) —
  never harvested into fixtures.js.
- **Offline municipality packs** (sigpac-integration step 6) — bulk GPKG per
  municipality (INSPIRE ATOM) consulted by the lookups before the network,
  for never-seen-offline areas (the P5 field case). Gated: only if real field
  usage shows the cache-through model isn't enough. Packs cover geometry +
  attributes, NOT zone intersections (query-only services).
- **Stage-1 sync transport** — phone as writing device; export via shared
  storage + USB (Android MTP; iOS `UIFileSharingEnabled`); see Data storage →
  Sync. **Start with a gap analysis, not with code**: the shipped backup
  export/import may already cover most of the phone→desktop mirror, so what this
  needs first is an assessment of what is genuinely missing.
- **Already mobile-ready by design** (verify, don't rebuild): bottom tab bar
  (nav-as-data), native dialog plugin (share sheet on mobile), no blocking JS
  dialogs, `confirmDialog()` everywhere, Svelte bundle size, lazy map chunk.
- **Pre-release/mobile chores**: demo `seed_demo_data` now WORKS in release
  builds (guard removed 2026-07-23 for release-APK field testing; MUST be re-guarded or dropped before the
  stable release); webview differences (verifiers
  drive WebKitGTK/Blink, not WKWebView/Android WebView — re-verify date
  inputs and the `geo://` protocol CORS behaviour per platform; Windows/
  WebView2 field-verified 2026-07-09: `http://geo.localhost` + CORS render
  fine, date inputs unchecked); proj4rs-style pure-Rust deps chosen
  deliberately so mobile targets cross-compile clean — keep that bar for
  new crates.

---

## 9. Re-running the query-scope audit

`data-model.md` → "Indexes and query scope" is the finding; this is how to
produce it again. **A recipe, not a gate**: whether a `SCAN` is a defect or a
six-row lookup being read whole is a judgement, and an automated check would
need an allowlist that drifts — which is a gate that cries wolf and gets waved
through. Run it before tagging a release, and after adding a register.

**The two rules a test already enforces** are the index house pattern
(`src-tauri/tests/index_contract.rs`) and the per-record child query (the
`query_scope.rs` file in each register-owning crate, built on
`terrazgo_testkit::query_cost`). Those fail on their own. What follows is for
the third rule — query *scope* — which needs someone to read the plans.

**The plans.** Concatenate the schema and ask SQLite what it would do:

```
cat crates/terrazgo-core/migrations/0001_core_schema.sql \
    crates/module-cue/migrations/0001_schema.sql \
    crates/module-fertilisation/migrations/0001_schema.sql \
    crates/module-ecoscheme/migrations/0001_schema.sql > /tmp/schema.sql
rm -f /tmp/plan.db && sqlite3 /tmp/plan.db < /tmp/schema.sql
sqlite3 /tmp/plan.db "EXPLAIN QUERY PLAN <the statement>"
```

For a sweep, pull every Rust string literal starting with `SELECT` out of
`crates/*/src` and `src-tauri/src`, bind each `?n` with a dummy, and `EXPLAIN`
it. **Read only the `SCAN` lines, and only for tables that GROW** — a scan of
`unit` or `production_system` is correct, and `SCAN CONSTANT ROW` is an artefact
of `EXISTS`. The 2026-08-24 sweep read 279 statements and produced 12 such
lines, of which 3 were real.

Then ask of each: *is the result set bounded by the question, or by the
history?* That is the judgement no tool makes.

**The timings.** The scaled-data builder is `crates/module-cue/tests/common/scale.rs`
and the measurement rides on it:

```
cargo test -p module-cue --release --test query_scope -- --ignored --nocapture
```

Release, because a debug build measures the wrong thing. It prints the markdown
table the doc records. **Compare slopes, not milliseconds** — the constant is
one machine's, the slope is the finding, and the desktop is the fast tier.

**What a corruption check costs.** The same shape, for the other measurement
this repo keeps `#[ignore]`d — it writes up to ~513 MB and takes minutes, so it
is a gate on nothing and an answer on demand:

```
cargo test -p terrazgo --test quick_check_cost -- --ignored --nocapture
```

It fills the real composed schema with `record_change` rows, because that is the
table that grows without bound, and prints both pragmas side by side. Last run
2026-08-26: `quick_check` 1.6 ms/MB, `integrity_check` 3.4 ms/MB, ratio rising
from 1.5x at 29 MB to 2.1x from 114 MB on. It answers two live questions — what
the weekly check costs a cooperative-scale book, and what the Settings button's
thorough check costs — and it is the cost curve the `record_change` retention
decision will need when it comes due.

---
