# SIEX-aligned export — design notes

> Status: design (2026-07-04; re-diffed against schema v3.11.4 on 2026-07-14);
> capture schema built 2026-07-15, export module (`module_cue::export`) built
> 2026-07-16 — see "Export module" below. The user-facing feature name is TBD
> ("SIEX" is too technical for farmers). This document maps Terrazgo's treatment
> domain onto the official CUE exchange format and lists what is missing.

## Sources of truth

| What | Where | Version used here |
| --- | --- | --- |
| Interface spec (methods, auth, envelope) | [FEGA Anexo VI "Interfaz Único Común"](https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex) | **v3.11.4 (Nov 2025)** — re-diffed 2026-07-14 (still the latest); see "Re-diff 3.3.0 → 3.11.4" below |
| CUE JSON Schema | Embedded in the Anexo VI docx (OLE object, filename `…CUE_3.11.4.json`); vendored copy: [`references/cue-schema-3.11.4.json`](references/cue-schema-3.11.4.json) (the superseded 3.3.0 copy was dropped 2026-07-14 once the re-diff below recorded its findings) | 3.11.4 |
| Field semantics / mandatory flags | `BdcSix-DS-DiseñoCUE.xlsx` (embedded in the same docx, sheet `EstructuraCuadernoWS`) + FEGA Anexo V | v3.11.4 sheet — **where sheet and JSON Schema disagree, the schema wins** (it is what validates); known drift: the sheet still shows `MateriaActivaFormulado{}` and `HorasUtilizacion`, both gone from the schema |
| Code catalogues (crops, units, problems, substances…) | FEGA Anexo VII — public REST API `https://www11.fega.es/bdcsixwsp/` (no auth; guide "BdcSixWsp" v4.5.0); the 16 treatment-relevant CSVs vendored in `crates/terrazgo-core/catalogues/` | stored 2026-07-14 — see "Storage design" |
| REA ↔ CUE relationship, CyL onboarding | [CUECYL](https://agriculturaganaderia.jcyl.es/web/es/cuaderno-digital-explotacion-agricola.html) + [REACYL](https://agriculturaganaderia.jcyl.es/web/es/registro-explotaciones-agrarias-castilla.html) pages; [RD 1054/2022](https://www.boe.es/buscar/doc.php?id=BOE-A-2022-23054) | checked 2026-07-11 |
| Farmer-side DGC outputs | [Instrucciones declaración DGC (Junta de CyL, PDF)](https://agriculturaganaderia.jcyl.es/web/jcyl/binarios/169/699/Instrucciones%20declaraci%C3%B3nDGC_ene-2025.pdf); [SIGPAC download service](https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html) + [cultivos-declarados model](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html) | checked 2026-07-12 |
| Other public SIEX services (MDF, `existeNIF`) | Same BdcSixWsp API — see "Other public SIEX services" below | checked live 2026-07-15 |

Transport recap: REST + JSON; auth =
qualified legal-person certificate + JWT; `POST /IUWS/crear/` is asynchronous
(request number + `comprobarEstado` polling). **Standalone desktop apps are
expected to generate the JSON file**; the WS client is a separate server-side
component (future, outside this repo's offline core).

## Target format (what we must produce)

```
Root
└─ CUADERNO[]                          ← one entry per farm (explotación)
   ├─ CAExplotacion*  IdTitular*  CodigoRea*  UnidadGestora*   (+ CodigoSIEX, IdCuaderno)
   ├─ DatosExplotacion
   │  ├─ AltaDGC[]            ← register plot+crop units the REA doesn't know
   │  └─ CambioCultivoDGC[]
   └─ ActividadesExplotacion
      ├─ TratamFito[]         ← OUR BLOCK (field phytosanitary treatments)
      ├─ UsoSemillaTratada[]  TratamientosPostCosecha[]  TratamientosEdifInstalaciones[]
      └─ (Fertilizacion, Cosecha, SiembraPlantacion, Riego, LaboresCulturales, … — future modules)
```

A **DGC** ("dato geográfico de cultivo") is the SIEX unit of plot+crop+period.
Activities do not reference plots directly: they reference DGCs, either by the
REA's own `CodigoDGC` (obtained by importing the REA) or by a client-assigned
`CodigoDGCAjena` created via `AltaDGC`.

## How farm data reaches the cuaderno — the REA-first rule (checked 2026-07-11)

Under RD 1054/2022 the regional farm registry (the **REA** — REACYL in
Castilla y León) is the source of truth the cuaderno consumes, not the other
way around. Verified against the Junta de Castilla y León's CUECYL and REACYL
pages (agriculturaganaderia.jcyl.es):

- **The explotación must be registered in the REA first.** The regional CUE is
  generated automatically from that registration ("Primero debe inscribir su
  explotación en el Reacyl y posteriormente se le generará automáticamente el
  Cuecyl"); `CodigoRea` and `IdTitular` (gap 4) exist only as a result of it.
  Nothing creates a farm from the cuaderno side.
- **Campaign surfaces and crops flow REA → CUE.** "Desde el Reacyl se vuelcan
  al Cuecyl las superficies y cultivos de la campaña"; the cuaderno cannot be
  filled for a campaign whose DGCs are stale in the REA. The REA itself is fed
  by the titular's declarations (REACYL's "Declaración de DGC" module), the
  PAC solicitud única and the sectoral registries (Registro Vitícola, ROMA,
  REGA).
- **The reverse path for plots+crops is real, not just schema.** CyL states
  that surfaces and crops "se podrán importar … desde un CUE comercial" —
  which is what `AltaDGC`/`CambioCultivoDGC` (+ `CodigoDGCAjena`) exist for.
  Locally entered plots therefore stay exportable; only the farm itself must
  pre-exist in the REA.
- **Operational rules (CyL):** a given DGC may be filled by only ONE
  commercial notebook; and when a farm is selected for an official control,
  all commercial-notebook annotations must already be transferred (volcadas)
  into the Cuecyl — otherwise the control falls back to the paper cuaderno.
- **No generic farmer-facing REA download exists** (REACYL's "Consultar mi
  explotación" is a web view behind certificate/DNIe login), so the only
  *machine* path into a commercial notebook is the Interfaz Único
  (`exportarREA`) — i.e. the future server-side component, never the offline
  core. **Refined 2026-07-12:** the REACYL *DGC declaration module* does give
  the titular two manual outputs — see "Farmer-side data paths" below.

### Farmer-side data paths — no server involved (checked 2026-07-12)

Three ways DGC-shaped data can reach the app with only the farmer's own work,
found while probing whether `exportarREA` could be avoided:

1. **REACYL DGC Excel export (the farmer's own authoritative DGCs).** The
   DGC-declaration module's sign-and-register screen lets the titular "firmar
   y registrar, así como **exportar las DGC a una hoja Excel**" (Junta's
   [Instrucciones declaración DGC, Jan 2025](https://agriculturaganaderia.jcyl.es/web/jcyl/binarios/169/699/Instrucciones%20declaraci%C3%B3nDGC_ene-2025.pdf)).
   Flow: farmer logs into `particulares.ayg.jcyl.es` (certificate/DNIe),
   exports, imports the file here. Unknowns until a real export is inspected:
   exact columns (does it carry `CodigoDGC`? that would soften gap 2), crop
   coding, whether the export is reachable outside an active declaration.
   CyL-specific — other regions' REA apps need their own adapter, like the
   submission side. Reading `.xlsx` in Rust = a new crate (calamine is the
   pure-Rust candidate) — decide deliberately before adding.
2. **Public "cultivos declarados" downloads (geometry + declared crop, no
   login).** The SIGPAC download service
   ([sigpac-hubcloud](https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html))
   publishes the graphical declaration lines as **provincial GeoPackages**,
   current + previous campaign, CC BY 4.0 — same channel, format and SRS
   (ETRS89/REGCAN95) as the recinto files the importer already reads. The
   [model](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html)
   carries the full SIGPAC ref, `EXP_ANO`, `PARC_PRODUCTO` (declared crop
   code), `PARC_SISTEXP` (secano/regadío), `PARC_SUPCULT` and the line
   geometry — enough to *prefill* a season's crops by matching the SIGPAC
   refs stored on plots. It is published declaration data, not the REA
   record: no `CodigoDGC`, PAC-declared surfaces only, and the publication
   cadence after declarations close is a pre-flight check. The same dataset's
   MVT twin is map-layers phase 2's `cultivo_declarado` overlay.
3. **Signed DGC document (PDF).** After registering, the titular can "obtener
   el documento firmado que recoge el listado de las DGC" — human-readable
   fallback, not an import format.

Consequence: a future "load my crops" feature can exist standalone — path 2
alone prefills crop+surface per plot from public data; path 1 upgrades it to
the farmer's authoritative list where available. Neither replaces
`exportarREA` for true REA sync (codes, titular data, rollover), which stays
server-side.

Design consequences: plots/crops in the app map to DGCs at export time —
importing the REA's `CodigoDGC` is a server-side capability for later, while
`AltaDGC` + `CodigoDGCAjena` is the viable standalone path (see gap 2 and open
question 5). Farm-level identifiers (`CodigoRea`, titular NIF) are entered by
the user from their REA registration — gap 4's schema additions — and cannot
be derived from anything else.

### `TratamFito` (required in 3.11.4: IdAjenaTratamFito, FechaInicio, FechaFin, DGCs, **ProblematicaFito, Justificaciones**, IdentificadorAplicador, **Eficacia**)

| Descriptor field | Terrazgo source | Status |
| --- | --- | --- |
| `IdAjenaTratamFito` (integer) | `export_alias` (minted at first export, keyed (treatment, split)) | ✓ (2026-07-15) |
| `Borrar` (bool) | soft-deleted records that were previously exported | ok (derive) |
| `FechaInicio` / `FechaFin` | `application_date` (both = same day) — 3.11.4 enforces `dd/mm/yyyy` (or `-`) via pattern; serializer converts from ISO | ✓ (format at export) |
| `HoraTratamiento`, `FechaSeca`, `Actividad` | not captured (`Actividad` = cover maintenance/elimination, cubierta treatments only) | optional — omit |
| `DGCs[].CodigoDGC` / `CodigoDGCAjena` | `CodigoDGCAjena` minted per core `crop` row (a crop IS the plot+crop+season unit) via `export_alias` | ✓ (2026-07-16) — REA `CodigoDGC` + `AltaDGC` generation stay **gap 2** |
| `DGCs[].CodigoCultivo` (new 3.11.x) | crop of the DGC — "indicar junto con CodigoDGC"; needs PRODUCTOS coding | with gap 2 (omitted for now — optional in schema) |
| `DGCs[].Superficie` | `treatment_plot.surface_treated_ha` | ✓ |
| **Constraint (descriptor):** all DGCs in one `TratamFito` must share product+variety | `treatment_plot` allows different crops per plot (by design) | serializer **splits** a multi-crop treatment into one `TratamFito` per crop |
| `ProblematicaFito.*.Tipo*[]` (codes) | `treatment_problem` junction: category + catalogue code, ≥1 per record | ✓ (2026-07-15; bucket = category) |
| `Justificaciones[].JustAct` (code) | `treatment_justification` junction (≥1 per record), English lookup → SIEX int at export | ✓ (2026-07-15) |
| `ProductosFito[].TipoProducto` (code) | `product_authorisation.kind_code` (default 'registered') → SIEX 1..4 | ✓ (2026-07-15) |
| `ProductosFito[].NumRegistro` | `authorisation_number_snapshot` | ✓ |
| `ProductosFito[].MateriaActiva` (code, number(5)) | `product_authorisation.exceptional_substance_code` (AUTORIZACION_EXCP code, required iff kind = 'exceptional') | ✓ (2026-07-15) — emitted only for TipoProducto 4 |
| `ProductosFito[].Dosis` / `Cantidad` / `Unidad` (code) | `dose_value` + `dose_unit_code`; Dosis XOR Cantidad ("nunca ambas") — our rate units emit Dosis | ✓ — `siex::unit_to_siex` map (code + exact conversion factor), contract-tested |
| **Constraint (descriptor):** ≥1 of `ProductosFito` / `OtrasActuacionesFito` | every treatment record has a product | ✓ |
| `IdentificadorAplicador[].AplicadorEmpresa.NumROPO` | `operator_licence_snapshot` | ✓ |
| `IdentificadorAplicador[].EquipoAplicador.NumROMA` / `NumREGANIP` / `IdEquipoAplicador` | `machinery_roma_snapshot` / `machinery_reganip_snapshot`; `IdEquipoAplicador` (string(50), free id) covers equipment not registrable in ROMA/REGANIP | ✓ — exactly one of the three ("nunca ambos"); serializer emits ROMA preferred |
| `IdentificadorAplicador[].EquipoAplicador.AplicacionManual` (bool) | **REQUIRED in 3.11.4** — derive: true when no machinery on the record, false otherwise | ✓ (derive) |
| `…EquipoAplicador.Duracion`/`NumRepeticiones`/`TipoEnergia`/`TipoMaquinariaUNE` | not captured (3.11.4 replaced `HorasUtilizacion` with `Duracion`) | optional — omit |
| `AsesorValidacion` (advisor ROPO + validation) | no advisor entity yet | optional — omit |
| `Eficacia` (code) | `treatment_record.efficacy_code` (nullable — observed after application; export precheck demands it) | ✓ (2026-07-15) |
| `Observaciones` | `notes` | ✓ |

Envelope requirements per farm: `CAExplotacion` (CCAA code), `IdTitular`
(titular NIF), `CodigoRea` (REA registration code), `UnidadGestora` — see gap 4.

## Re-diff 3.3.0 → 3.11.4 (2026-07-14)

Verified v3.11.4 (Nov 2025) is still the latest; schema extracted from the
docx's OLE object and vendored. Envelope (root/CUADERNO/required farm ids)
unchanged. What changed for us:

- **Three TratamFito fields became REQUIRED**: `ProblematicaFito` (≥1 coded
  problem), `Justificaciones[].JustAct` (1..n, catalogue) and `Eficacia`
  (code). All three are Anexo VII catalogue codes we don't capture → they harden
  gap 3 from "code mapping at export" into "the treatment form must capture
  coded choices at record time". `Justificaciones` and `Eficacia` were
  previously "optional — omit".
- **`MateriaActivaFormulado[]` → `MateriaActiva`** (single number(5) code on
  `ProductosFito`), mandatory **only** for TipoProducto 4 (autorización
  excepcional). Softens gap 3 for substances: registered products are
  identified by `NumRegistro` alone.
- **`EquipoAplicador` reshaped**: `AplicacionManual` (bool) is required
  (derivable); `HorasUtilizacion` replaced by `Duracion` (+ optional
  `NumRepeticiones`, `TipoEnergia`, `TipoMaquinariaUNE` — all catalogue-backed,
  omittable); `IdEquipoAplicador` (string(50)) now names non-ROMA/REGANIP
  equipment — a clean escape hatch for hand tools/unregistered gear.
- **Date patterns enforced** on all `Fecha*` fields: `dd/mm/yyyy` (or `-`
  separators). Our ISO dates convert at serialization; a pattern-violating
  payload now fails schema validation instead of the WS.
- **`DGCs[]` grew `CodigoCultivo`** (crop code, alongside `CodigoDGC`) and
  `Cubiertas` (ground-cover data, permanent crops — not our domain yet).
- **Descriptor constraint (sheet, not schema): all DGCs in one `TratamFito`
  must be the same product+variety.** Terrazgo deliberately allows one
  treatment to span plots with different crops (`treatment_plot` decision), so
  the serializer must split such records into one `TratamFito` per crop —
  same `IdAjena` family, distinct integer aliases (gap 1's mapping table must
  key on (treatment, crop), not treatment alone).
- **New activity blocks** `LaboresCulturales` and `Riego` (replacing
  `ActividadAgraria`): not TratamFito's concern, but they are the SIEX target
  blocks for the future crop-planning and **irrigation** modules — the export
  architecture should keep per-block serializers pluggable.
- Sheet/schema drift note: the 3.11.4 xlsx sheet still shows the old
  `MateriaActivaFormulado{}` and `HorasUtilizacion`; the JSON Schema (what
  validates) removed both. Schema wins.

## Anexo VII catalogue study (2026-07-14)

The Anexo VII catalogues turn out to be served by a **public, unauthenticated
REST API** — the same data the sede portal browses (guide: "BdcSixWsp: Guía de
Servicios públicos de Siex" v4.5.0, saved locally; the securización section
states outright that no authentication is required):

```
base  https://www11.fega.es/bdcsixwsp/
GET   /catalogos/{idTabla}            one catalogue (CSV default; XLSX, PDF)
GET   /catalogos/zip/                 all catalogues, one ZIP (~1.4 MB, 122 files)
GET   /catalogos/{idTabla}/fecha      {"fecha":"DD/MM/YYYY"} last-update probe
```

File format (live-verified on all treatment-relevant catalogues, 2026-07-14):
`;`-separated CSV, fields quoted, documented as **ISO-8859-1** (the guide's own
client example was corrected from UTF-8 to ISO-8859-1 in its v1.5.1 errata) —
but the real files are **Windows-1252**: UNIDADES_MEDIDA carries € (byte 0x80,
a control character in true ISO-8859-1), found 2026-07-14. Most catalogues carry
lifecycle columns `Fecha de alta` / `Fecha de modificación` / `Fecha de baja` —
**codes are never deleted, they are baja-dated**, so an old record's code stays
resolvable forever if imports only ever upsert.

### Catalogues `TratamFito` needs (idTabla live-verified against the API)

| Payload field | idTabla | Rows | Shape |
| --- | --- | --- | --- |
| `Eficacia` | `EFICACIA_TRATAMIENTO` | 3 | code + label |
| `Justificaciones[].JustAct` | `JUSTIFICACION_ACTUACION` | 5 | code + label |
| `ProductosFito[].TipoProducto` | `TIPO_PRODFITO` | 3 | code + label |
| `ProductosFito[].Unidad` | `UNIDADES_MEDIDA` | 81 | code + label |
| `ProblematicaFito.Enfermedades` | `ENFERMEDADES` | 600 | code + hierarchical nº + category + scientific name + **EPPO code** + notes |
| `ProblematicaFito.ArtropodosGasteropodos` | `PLAGAS` | 528 | same shape |
| `ProblematicaFito.MalasHierbas` | `MALAS_HIERBAS` | 203 | same shape |
| `ProblematicaFito.ReguladoresOtros` | `REGULADORES_CRECIMIENTO` | 55 | same shape |
| `ProductosFito[].MateriaActiva` | `AUTORIZACION_EXCP` | 73 | code + substance + product (exceptional authorisations only) |
| `OtrasActuacionesFito.TipoMedida` | `TIPO_MEDIDA_FITOSANITARIA` | 13 | code + label |
| `OtrasActuacionesFito.BuenasPracticas` | `BUENAS_PRACTICAS_AMBITOS` | 97 | code + label + **ámbito** (code repeats per ámbito — composite identity) |
| `DGCs[].EstadoFenologico` | `EST_FENOLOGICO` | 9 | code + BBCH-style stage + label |
| `EquipoAplicador.TipoEnergia` | `TIPENERGIA` | 10 | code + label |
| `EquipoAplicador.TipoMaquinariaUNE` | `TIPO_MAQUINA_UNE` | 689 | **string** code + label, no lifecycle dates |
| `DGCs[].CodigoCultivo` | `PRODUCTOS` | 1119 | code + name + Latin + EPPO + ~25 boolean attribute columns |
| (prefill/validation) | `CULTIVO_USO_SIGPAC` | 2496 | crop code ↔ SIGPAC uso — the natural cross-check for the declared-crops prefill |
| (variety, `AltaDGC` later) | `VARIEDAD_ESPECIE_TIPO` | ~40k (9.7 MB) | defer until `AltaDGC` is built |
| (`AltaDGC` later) | `SIST_CULTIVO` | 32 | code + label |

Catalogues move on FEGA's own cadence (fecha probes ranged 2023 → **2026-07-14
itself** across this list), so a refresh path matters — but snapshot-first:
the app must work offline with vendored data from first run.

### Other public SIEX services (checked live 2026-07-15)

The same no-auth API exposes two more things beyond the catalogues (the
portal's `/ffii` section = "Fuentes de Información externas"; documented in
the same BdcSixWsp guide):

- **`GET /fuentesInformacion/zip`** (~30 MB) — exactly two CSVs:
  - **`MDF.csv`** (1,235 rows): Registro de Determinados Medios de Defensa
    Fitosanitaria — *non-chemical* defense means (biological control
    organisms, traps, pheromone attractants) with target organisms and
    crops. Not needed for `TratamFito` v1 (chemical products ride
    `NumRegistro`), but the natural source when biological-control /
    organic-farm records arrive; small enough to vendor like the
    catalogues.
  - **`ROPO.csv`** (1.33 M rows, 228 MB): the national phytosanitary-carnet
    register — **excluded (2026-07-15): it is a mass personal-data dump**
    (names, phones, emails) the app must not vendor or redistribute, and
    the served snapshot was two years stale (2024-01-25). If carnet
    validation/prefill is ever wanted, it needs a different mechanism, not
    this file.
- **`POST /existeNIF`** (public, live-verified): given a NIF, returns the
  holder's explotaciones with `Codigo_SIEX` (+ REA code and CCAA when
  present). This is gap 4's data — a future "look up my REA code from my
  NIF" prefill through the sanctioned network seam, sparing the farmer the
  transcription from their REA papers. Optional and online-only; the manual
  fields stay the offline path.

### Storage design (settled 2026-07-14; implemented same day)

**Implemented as designed** — schema in core `0001` (`catalogue` +
`catalogue_code`), importer + query API in `terrazgo_core::catalogue`
(`ensure_catalogues` runs at startup; `active_codes` for pickers,
`find_code` for resolution), vendored snapshot in
`crates/terrazgo-core/catalogues/` (16 files, idTabla names), tests against
the real FEGA files in `crates/terrazgo-core/tests/catalogue.rs`. The rest
of this section is the design rationale, kept as decision history.

**Two generic tables owned by terrazgo-core.** Reference catalogues serve the
whole farm domain (treatments now; crop prefill, fertilisation, irrigation
later), and modules only depend on core — putting them in core dissolves any
cross-module read. Core stays country-neutral because the *mechanism* is
generic and the Spanish-ness is data: the `geo_feature` pattern.

- `catalogue` — one row per imported catalogue: `id` TEXT PK (the idTabla),
  `source` TEXT (`'siex'` now; other countries' registries later),
  `source_updated_at` (the fecha value / max row date at import),
  `imported_at`.
- `catalogue_code` — INTEGER PK (shipped reference data — the UUID rule
  applies to user data only), `catalogue_id` FK, `code` TEXT (integer codes
  for all but `TIPO_MAQUINA_UNE`), `label` TEXT, `attrs` JSON (category,
  scientific name, EPPO, ámbito, hierarchical nº, boolean crop attributes… —
  the `geo_feature` precedent: **promote a catalogue to a typed table only
  when a real query needs its attributes**, e.g. crops for the prefill; the
  generic rows keep everything the CSV had, so promotion is an additive copy
  and code values never change). No UNIQUE on (catalogue, code):
  `BUENAS_PRACTICAS_AMBITOS` legitimately repeats a code per ámbito.
- Import semantics: **upsert only, never delete** — baja'd codes must keep
  resolving for old records (invariant gets its own test). UI pickers filter
  `baja IS NULL` (and by attrs where relevant, e.g. ámbito).
- **No SQL FK from user data to catalogue codes** (settled): the code value
  is the regulatory payload, the catalogue row is display metadata. Bogus
  codes are caught by a shared Rust validation helper plus the export's
  schema-validated tests — two nets; and reimports can never cascade into
  user records. Accepted cost: the DB itself won't reject a wrong code.
  Labels are deliberately NOT snapshotted onto records: if the source renames
  a label, showing the new one is correct — the code is what's legal.
- **Shipping (stage 1)**: vendor the ~16 needed CSVs (≈0.7 MB raw), imported
  by an idempotent `ensure_catalogues(conn)` at startup (the `refresh_alerts`
  pattern) when a catalogue is missing or older than the vendored snapshot.
  Catalogue updates ride app releases — refreshing the vendored snapshot is a
  release-ritual step (one public GET). Excluded from `record_change`
  (shipped reference data; each device imports its own copy). Not migrations:
  post-release migrations are append-only forever — wrong tool for
  third-party data on its own cadence.
- **Refresh (stage 2, later, optional)**: async command through terrazgo-geo's
  fetch — `/fecha` staleness probe, then `/catalogos/{idTabla}` — same
  parser, same upsert. The sanctioned network seam; never required. Staleness
  in between is mild: new codes can't be picked until an update, existing
  records stay valid.
- **Parsing**: the `csv` crate (settled 2026-07-14; delimiter `b';'` — the
  notes columns use RFC quoting with embedded `;`/newlines); decoding
  hand-rolled, no encoding crate — UTF-8 accepted first (fallback for a
  future provider encoding switch; legacy accented text is never
  accidentally valid UTF-8), then Windows-1252 (the files' real encoding —
  the € finding above; only 0x80–0x9F differs from the 1:1 Latin-1 map),
  with a control-character tripwire test that fails the suite on any
  further encoding drift instead of importing garbage.
- Rejected: per-catalogue typed tables (~16 near-identical tables for data
  whose only universal query is code→label+attrs — the *relationship* data a
  future recommender needs, e.g. a MAPA-registry product↔crop↔problem table,
  is separate first-class schema under either option, so the choice doesn't
  constrain it); module-cue ownership (blocks other modules — they depend
  only on core); storing catalogues in geo-cache.db (regulatory reference
  data must survive in backups — a restored backup must still resolve codes).

What this does NOT cover (the design pass after the storage lands, gaps
1/3/4): the columns/junctions on `treatment_record` that *capture* coded
choices at record time (efficacy, justifications 1..n, problems per type),
the integer export aliases, and `rea_code` + titular NIF.

## Capture design — gaps 1/3/4 (settled + implemented 2026-07-15)

One schema pass, all pre-release `0001`/`0002` edits. The storage principle
mirrors the codebase's two existing precedents:

- **Small closed lists with universal meaning** (efficacy, justification,
  authorisation kind) → English-coded lookup tables + i18n keys, mapped to
  SIEX integers at export (`module_cue::siex`) — the `unit`/`reason_category`
  pattern. The `es` dictionary carries the official Castilian wording
  verbatim, so Spanish users see exactly the catalogue terms. A **contract
  test** (`tests/siex_mapping.rs`) checks each mapping against the vendored
  catalogue snapshot in both directions, so a snapshot refresh that adds a
  code (JUSTIFICACION_ACTUACION grew 5 → 6 rows in 2025/26) fails the suite
  instead of silently under-offering choices.
- **Provider lists too large to own** (the ~1,400 phytosanitary problems) →
  the catalogue code stored verbatim, no FK (the settled catalogue rule).

What landed where:

- **Problems (gap 3)** — `treatment_problem` junction: per-row
  `reason_category_code` + `problem_code`, ≥1 per record enforced at insert
  (this IS the "reason for treatment"; the record-level
  `reason_category_code` column was dropped, `target_organism` stays as
  optional free text). The category picks the resolution catalogue
  (disease → ENFERMEDADES, pest → PLAGAS, weed → MALAS_HIERBAS,
  growth_regulator/other → REGULADORES_CRECIMIENTO) and the export bucket.
  Codes are validated at insert against the imported catalogue (existence
  only — retired codes pass, matching upsert-never-delete); the export's
  schema-validated tests are the second net.
- **Justifications (gap 3)** — `treatment_justification` junction, ≥1 per
  record at insert (known at treatment time, unlike efficacy).
- **Efficacy (gap 3)** — nullable `treatment_record.efficacy_code`:
  unknowable on application day, so it is recorded later via
  `set_treatment_efficacy` (the ONE edit a stored treatment allows,
  audit-logged) and the export precheck lists records still missing it.
- **Product kind (gap 3)** — `product_authorisation.kind_code`
  (`registered`/`common_name`/`parallel_import`/`exceptional`, default
  registered) + `exceptional_substance_code` (AUTORIZACION_EXCP code,
  required iff exceptional — the `MateriaActiva` payload). Dose units need
  no schema: `siex::unit_to_siex` maps each unit to a catalogue code plus an
  exact conversion factor (SIEX has no ml/ha or g/L — nearest units differ
  by a power of ten).
- **Integer aliases (gap 1)** — `export_alias` (module-cue):
  `(target, entity_table, entity_id, split_key) → alias INTEGER`, minted
  MAX+1 per target at first export, never updated or deleted. `split_key`
  discriminates the per-crop `TratamFito` splits; alias existence doubles as
  the "previously exported" marker driving `Borrar`. Synced + audited (not
  re-derivable). **Recorded limit:** two devices exporting independently
  before syncing could mint colliding integers — a sync-stage-2 design item
  (same family as alert-acknowledgement roaming); today one device exports.
- **Farm identifiers (gap 4)** — `farm.owner_tax_id` in core (holder
  tax/identity number: a universal concept — NIF/CUAA/SIREN — with
  per-country format validation) and `farm_es_extension.rea_code`. Both
  user-entered from the REA papers. `CAExplotacion` needs no column — it
  derives from `farm_es_extension.province_code` via a static province→CCAA
  map at export. `UnidadGestora` is "Identificador (NIF/CIF) de la Unidad
  gestora" per the descriptor sheet: for a titular-driven notebook the
  export defaults it to `owner_tax_id` (question 7 below confirms the
  reading); a column arrives only if entidades habilitadas become a use
  case.

## Export module (built 2026-07-16)

`module_cue::export` — the query layer + serializer for one farm+season,
schema-validated in `crates/module-cue/tests/export.rs` against the vendored
3.11.4 schema (the `jsonschema` crate, dev-dependency only, HTTP-resolver
features off). Two entry points:

- **`export_precheck(conn, season, farm)`** — lists what blocks a valid
  export instead of erroring one field at a time: records missing
  `efficacy_code` (schema-required), records whose operator has no licence
  number (`NumROPO`), treated plots without a crop (no DGC unit to name),
  and farm identity fields missing or unusable (`owner_tax_id`, `rea_code`,
  `province_code`). Only active records are checked — deletion entries
  cannot demand new observations.
- **`build_cuaderno(conn, season, farm)`** — the descriptor itself
  (`descriptor::CuadernoExport`, typed serde structs mirroring the schema).
  Refuses while the precheck is not clean, so nothing is silently dropped or
  invented.

Serialization decisions, each pinned by a test:

- **Per-crop splits.** Plots group by `(crop_name, variety)` snapshot; a
  multi-group record emits one `TratamFito` per group, aliased on
  (record, split key). Snapshots are frozen at insert, so grouping can never
  drift between exports. Single-group records keep the empty split key.
- **DGC linkage (pending gap 2's real answer).** A core `crop` row is
  exactly the SIEX plot+crop+season unit, so each treated plot's crop gets a
  `CodigoDGCAjena` minted from `export_alias` (`entity_table='crop'`) —
  stable across exports and shared by every treatment on that crop. If
  CUECYL mandates REA `CodigoDGC` instead, the minted aliases go unused.
  `AltaDGC` block generation (registering those DGCs) is not built yet.
- **Deletions.** A soft-deleted record emits a full entry with
  `Borrar: true` for each split that had an alias (= was actually exported);
  splits never exported are skipped. Deletion entries must still satisfy the
  schema's required fields, so a never-assessed efficacy falls back to the
  schema default 0 and a missing licence to the empty string — the entry
  exists to identify the deleted activity, not to assert observations.
- **Equipment `oneOf`.** The schema demands exactly one of
  `NumROMA`/`NumREGANIP`/`IdEquipoAplicador` even for manual application:
  no machinery → `AplicacionManual: true` + the fixed sentinel `"manual"`;
  machinery with both registry numbers → ROMA ("nunca ambos"); machinery in
  neither registry → its row id as `IdEquipoAplicador` (free string(50),
  never drifts).
- **Product kind.** Resolved live by the frozen authorisation number
  (product + country + `authorisation_number_snapshot`); when the
  authorisation row no longer matches, the default kind (registered)
  applies. `MateriaActiva` (the AUTORIZACION_EXCP code) is emitted only for
  kind `exceptional`.
- **Dates** convert ISO → dd/mm/yyyy (`siex::date_to_siex`); `CAExplotacion`
  derives from the province via `siex::province_to_ccaa` (INE relation,
  unit-tested); `UnidadGestora` = `owner_tax_id` (open question 7).

Two findings from validating against the real schema:

- **`CodigoRea` is exactly 14 characters** (minLength = maxLength = 14, like
  `CodigoSIEX` — the national ES+12-digit registry format). The precheck
  flags a present-but-wrong-length REA code the same way as an absent one.
- **The official schema has a typo**: one `$id` reads `"##root/…"` (double
  `#`, under SiembraPlantacion → Maquinaria → items), which draft-07
  meta-validation rejects as an invalid uri-reference. The vendored file
  stays byte-exact; the test harness normalizes the typo in its in-memory
  copy only (the `$id`s are decorative — the schema contains no `$ref`).
  Check whether a future schema release fixes it.

The file-export command + UI entry point landed the same day (build-order
step 4 below). Still not built: the `AltaDGC`/`CambioCultivoDGC` blocks
(gap 2) and the server-side WS client.

## Gaps found (ordered by design impact)

1. **Integer activity ids.** `IdAjena*` fields are integers (`number(10)`, max
   9999999999 per the descriptor), not strings — our UUIDv7 TEXT ids cannot be
   sent as-is. The id is the edit/delete key on the
   SIEX side, so it must be *stable across exports*. Design direction: a small
   mapping table (entity id → monotonic integer alias, assigned at first
   export) owned by the export module. Needs schema design. **3.11.4 note:**
   because one Terrazgo treatment can split into several `TratamFito` entries
   (same-crop DGC rule), the alias must key on (treatment, crop), not the
   treatment alone.
2. **DGC linkage.** Referencing REA DGC codes requires the REA import
   (`exportarREA`) to be built first, OR we always create our own DGCs via
   `AltaDGC` + `CodigoDGCAjena` (works standalone; risks duplicating what REA
   already has). Ask CUECYL which they prefer for commercial notebooks.
   Update 2026-07-11: CyL confirms commercial notebooks may import surfaces
   and crops into the Cuecyl (see the REA-first section), so the `AltaDGC`
   path is viable standalone; the duplication question stands.
   Update 2026-07-16: the export module references DGCs via minted
   `CodigoDGCAjena` integers (one per core `crop` row — see "Export module");
   what remains of this gap is generating the `AltaDGC` blocks themselves
   (needs `CodigoCultivo` from the PRODUCTOS catalogue) and the REA
   `CodigoDGC` question.
3. **Anexo VII catalogue codes.** Crops, varieties, units, active substances,
   product types, phytosanitary problems, justifications and efficacy are
   *coded* lists. We store English enum codes / free text today. Needed:
   import the Anexo VII catalogues as lookup data + add code columns (or
   mapping tables), and capture coded problem/crop choices in the UI at record
   time (a free-text `target_organism` cannot be reliably back-coded).
   **Raised by the 3.11.4 re-diff:** `ProblematicaFito`, `Justificaciones` and
   `Eficacia` are now *required* — the treatment form must offer these coded
   choices, they cannot be deferred to export time. Softened for substances:
   `MateriaActiva` codes are only needed for exceptional authorisations.
   **Done 2026-07-15** (capture columns/junctions + form + validation — see
   "Capture design"). Still open within gap 3: crop coding
   (`DGCs[].CodigoCultivo`, PRODUCTOS catalogue) rides with gap 2.
4. **Farm identifiers.** `IdTitular` (NIF) and `CodigoRea` are required and we
   captured neither (`farm_es_extension` had REGA, which is the *livestock*
   registry — same trap as REGANIP/ROMA). **Done 2026-07-15**
   (`farm.owner_tax_id` + `farm_es_extension.rea_code` — see "Capture
   design"). Both values come from the farm's REA registration (see the
   REA-first section): user-entered, never derived.
5. **Advisor (optional).** `AsesorValidacion` supports GIP advisor sign-off;
   no advisor entity exists yet. Fine to omit; future entity if users need it.

## Suggested build order

1. Anexo VII catalogue study → catalogue storage — this also improves the
   treatment form (coded problems). **Done 2026-07-14** (study, settled
   design AND implementation — see "Storage design" above).
2. Schema additions (gaps 1, 3, 4) — one schema design pass, settled before
   coding. **Done 2026-07-15** (design settled and implemented the same day —
   see "Capture design").
3. Export module: query layer (season+farm → snapshots+plots) → serializer to
   the descriptor JSON → validate against the vendored schema in tests.
   **Done 2026-07-16** (see "Export module"; `jsonschema` settled as the
   dev-only validation crate).
4. File export command (async, like backups) + UI entry point. **Done
   2026-07-16** — `export_cuaderno_precheck` + `export_cuaderno` commands
   (the latter async, backup-command pattern: build → write to the
   dialog-chosen path, returns path/size/entry count); the record-book view
   gained an "Exportación oficial (SIEX)" section whose button runs the
   precheck first and renders the blockers as a fix-it list (farm fields
   link to the farms view), opening the save dialog only when clean. The
   suggested filename sanitizes the season label ("2025/2026" carries a
   path separator). Feature name stays provisional.
5. Server-side WS client — separate component, after developer authorization
   with the Junta exists. Not in this repo's core.

## Open questions for CUECYL

Contact update (2026-07-11): the commercial-notebook onboarding path in CyL is
published — a test-environment access form emailed to **comercialcuecyl@jcyl.es**
(more specific than the generic cuecyl@jcyl.es), tied to the MAPA "grupo de
trabajo mixto cuaderno digital"; after the test phase the company is moved to
production and added to a public list. Titulares can use a commercial notebook
directly, without an entidad habilitada, if the notebook implements the
authorization flow offered by the Cuecyl app. This answers most of question 3
and part of question 1; the rest still needs the email.

1. Procedure and requirements to register as a commercial-notebook developer
   (empresa desarrolladora) in Castilla y León; is an autónomo acceptable?
   (Partly answered 2026-07-11 — see the contact update; the autónomo
   question and the MAPA working-group prerequisite remain open.)
2. CyL's IUWS endpoint and any CyL-specific documentation.
3. Access to the integration/test environment mentioned in FEGA Anexo VI.
   (Answered 2026-07-11: form → comercialcuecyl@jcyl.es.)
4. Is there any farmer-facing *file* import into CUECYL (manual upload of the
   descriptor JSON), or is the authorized web service the only path?
5. For DGCs: should commercial notebooks reference REA `CodigoDGC` (via
   `exportarREA`) or create their own via `AltaDGC`/`CodigoDGCAjena`?
   (Evidence 2026-07-11 that the `AltaDGC` path is accepted in practice —
   see the REA-first section — but the preference question stands.)
6. The REACYL DGC Excel export (2026-07-12 finding): which columns does it
   contain — in particular, does it include `CodigoDGC`? Is its format stable
   across campaigns, and is the export reachable any time the titular enters
   the module, or only during an active declaration?
7. `UnidadGestora` (2026-07-15, from the descriptor sheet: "Identificador
   (CIF, NIE, CIF) de la Unidad gestora"): for a titular who drives a
   commercial notebook directly (no entidad habilitada), is it simply the
   titular's own NIF — i.e. equal to `IdTitular`?
