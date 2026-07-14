# SIEX-aligned export — design notes

> Status: design (2026-07-04; re-diffed against schema v3.11.4 on 2026-07-14).
> No code yet. The user-facing feature name is TBD
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
| `IdAjenaTratamFito` (integer) | — needs an integer alias per `treatment_record` | **gap 1** |
| `Borrar` (bool) | soft-deleted records that were previously exported | ok (derive) |
| `FechaInicio` / `FechaFin` | `application_date` (both = same day) — 3.11.4 enforces `dd/mm/yyyy` (or `-`) via pattern; serializer converts from ISO | ✓ (format at export) |
| `HoraTratamiento`, `FechaSeca`, `Actividad` | not captured (`Actividad` = cover maintenance/elimination, cubierta treatments only) | optional — omit |
| `DGCs[].CodigoDGC` / `CodigoDGCAjena` | `treatment_plot` → plot+crop | **gap 2** |
| `DGCs[].CodigoCultivo` (new 3.11.x) | crop of the DGC — "indicar junto con CodigoDGC" | with gap 2 |
| `DGCs[].Superficie` | `treatment_plot.surface_treated_ha` | ✓ |
| **Constraint (descriptor):** all DGCs in one `TratamFito` must share product+variety | `treatment_plot` allows different crops per plot (by design) | serializer **splits** a multi-crop treatment into one `TratamFito` per crop |
| `ProblematicaFito.*.Tipo*[]` (codes) | `reason_category_code` + free-text `target_organism` | **gap 3 — now REQUIRED** (≥1 problem) |
| `Justificaciones[].JustAct` (code) | not captured | **gap 3 — now REQUIRED** (1..n) |
| `ProductosFito[].TipoProducto` (code) | not captured (product kind catalogue) | **gap 3** |
| `ProductosFito[].NumRegistro` | `authorisation_number_snapshot` | ✓ |
| `ProductosFito[].MateriaActiva` (code, number(5)) | 3.11.4 dropped the `MateriaActivaFormulado[]` wrapper; single code, **mandatory only for TipoProducto 4 "autorización excepcional"** — registered products are covered by `NumRegistro` | softened: omit unless exceptional authorisation (**gap 3** only for that case) |
| `ProductosFito[].Dosis` / `Cantidad` / `Unidad` (code) | `dose_value` + `dose_unit_code`; Dosis XOR Cantidad ("nunca ambas") | code mapping (**gap 3**) |
| **Constraint (descriptor):** ≥1 of `ProductosFito` / `OtrasActuacionesFito` | every treatment record has a product | ✓ |
| `IdentificadorAplicador[].AplicadorEmpresa.NumROPO` | `operator_licence_snapshot` | ✓ |
| `IdentificadorAplicador[].EquipoAplicador.NumROMA` / `NumREGANIP` / `IdEquipoAplicador` | `machinery_roma_snapshot` / `machinery_reganip_snapshot`; `IdEquipoAplicador` (string(50), free id) covers equipment not registrable in ROMA/REGANIP | ✓ — exactly one of the three ("nunca ambos"); serializer emits ROMA preferred |
| `IdentificadorAplicador[].EquipoAplicador.AplicacionManual` (bool) | **REQUIRED in 3.11.4** — derive: true when no machinery on the record, false otherwise | ✓ (derive) |
| `…EquipoAplicador.Duracion`/`NumRepeticiones`/`TipoEnergia`/`TipoMaquinariaUNE` | not captured (3.11.4 replaced `HorasUtilizacion` with `Duracion`) | optional — omit |
| `AsesorValidacion` (advisor ROPO + validation) | no advisor entity yet | optional — omit |
| `Eficacia` (code) | not captured | **gap 3 — now REQUIRED** |
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
4. **Farm identifiers.** `IdTitular` (NIF) and `CodigoRea` are required and we
   capture neither (`farm_es_extension` has REGA, which is the *livestock*
   registry — same trap as REGANIP/ROMA). Needs: `rea_code` (+ titular NIF,
   probably on `farm`) — schema design. Both values come from the farm's REA
   registration (see the REA-first section): user-entered, never derived.
5. **Advisor (optional).** `AsesorValidacion` supports GIP advisor sign-off;
   no advisor entity exists yet. Fine to omit; future entity if users need it.

## Suggested build order

1. Anexo VII catalogue study → catalogue storage — this also improves the
   treatment form (coded problems). **Done 2026-07-14** (study, settled
   design AND implementation — see "Storage design" above).
2. Schema additions (gaps 1, 3, 4) — one schema design pass, settled before
   coding. **This is the next step.**
3. Export module: query layer (season+farm → snapshots+plots) → serializer to
   the descriptor JSON → validate against the vendored schema in tests
   (JSON-Schema validation crate needed — decide deliberately before adding).
4. File export command (async, like backups) + UI entry point.
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
