# SIEX-aligned export — design notes

> Status: **PARKED 2026-08-02 — the export has no delivery path.** CUECYL
> answered the onboarding questions below: connecting an application to the
> REACYL or CUECYL web services requires being a **company or self-employed
> professional (autónomo)**, and **CUECYL offers no farmer-facing utility to
> upload a descriptor JSON file**. So the shipped serializer
> (`module_cue::export`) stays in the codebase — compiling, schema-validated
> and tested — as future-proofing for the day that changes (a non-profit entity
> is not discarded), but it is not extended: `AltaDGC`/`CambioCultivoDGC`
> (gap 2) and the web-service client are out of scope, and the printable PDF
> cuaderno becomes the app's compliance artifact instead.
>
> Not blocked by that answer: the **national FEGA** services below
> (`/catalogos/*`, `POST /existeNIF`) are public and no-auth — the barrier is
> specific to the regional CUE/REA web services.
>
> History: design 2026-07-04; re-diffed against schema v3.11.4 on 2026-07-14;
> capture schema built 2026-07-15, export module built 2026-07-16 — see
> "Export module" below. This document maps Terrazgo's treatment domain onto
> the official CUE exchange format and lists what is missing.

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

Under RD 1054/2022 the regional farm registry (the **REA**) is the source of
truth the cuaderno consumes, not the other way around.

### One decree, one system per community (2026-08-03)

RD 1054/2022 creates three things at once: the national **SIEX**, a
**registro autonómico de explotaciones** and a **cuaderno digital**. Each
autonomous community runs its own instance of the last two, under its own
name — so the names below are branding, not different concepts, and the app
models the concepts:

| Community | Regional registry (REA) | Digital record book |
| --- | --- | --- |
| Castilla y León | REACYL | CUECYL |
| Catalunya | SIDEAC (fed by the DUN declaration) | QIE — *Quadern integrat d'explotacions* |
| … | one each | one each |

Sources: [CUECYL](https://agriculturaganaderia.jcyl.es/web/es/cuaderno-digital-explotacion-agricola.html)
/ [REACYL](https://agriculturaganaderia.jcyl.es/web/es/registro-explotaciones-agrarias-castilla.html);
the DUN 2024 SIDEAC sheet, which states it plainly — *"El SIDEAC … constitueix
el Registre d'explotacions agràries de Catalunya d'acord amb … el RD
1054/2022 … pel qual s'estableix el SIEX i el Registre autonòmic
d'explotacions agrícoles i quadern digital"* (checked 2026-08-03).

**Consequence for the app**: `farm_es_extension.rea_code` is ONE column for
every community — the concept is national, only the issuing system's name
varies — and no user-facing string may name a single community's service.
Labels say "registro autonómico"; the export hint says "el cuaderno digital de
su comunidad autónoma". Naming the local system would need a province →
service map, which is the same seam as `terrazgo_recordbook::region`'s
province → language map; nothing needs it yet, and it would have to be kept
current for seventeen communities. The rule below is verified against the Junta de Castilla y León's CUECYL and
REACYL pages (agriculturaganaderia.jcyl.es); it is the decree's mechanism, so
it holds under whatever name a community gives its own systems:

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
| `HoraTratamiento` | `treatment_record.application_time` (local `HH:MM`; the serializer pads the seconds Anexo VI's `string(8)` wants) | ✓ (2026-08-12) |
| `FechaSeca`, `Actividad` | not captured (`Actividad` = cover maintenance/elimination, cubierta treatments only) | optional — omit |
| `DGCs[].CodigoDGC` / `CodigoDGCAjena` | `CodigoDGCAjena` minted per core `crop` row (a crop IS the plot+crop+season unit) via `export_alias` | ✓ (2026-07-16) — REA `CodigoDGC` + `AltaDGC` generation stay **gap 2** |
| `DGCs[].CodigoCultivo` (new 3.11.x) | crop of the DGC — "indicar junto con CodigoDGC"; needs PRODUCTOS coding | with gap 2 (omitted for now — optional in schema) |
| `DGCs[].Superficie` | `treatment_plot.surface_treated_ha` | ✓ |
| `DGCs[].EstadoFenologico` | `treatment_plot.growth_stage_code` as an integer — the catalogue's code, NOT the BBCH stage the book prints | ✓ (2026-08-12) |
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
| `OtrasActuacionesFito.TipoMedida` | `TIPO_MEDIDA_FITOSANITARIA` | 14 | code + label; the codes run 1-12, 14, 15 — there is no 13 |
| `OtrasActuacionesFito.BuenasPracticas` | `BUENAS_PRACTICAS_AMBITOS` | 97 | code + label + **ámbito** (code repeats per ámbito — composite identity) |
| `DGCs[].EstadoFenologico` | `EST_FENOLOGICO` | 9 | code + BBCH-style stage + label |
| `EquipoAplicador.TipoEnergia` | `TIPENERGIA` | 10 | code + label |
| `EquipoAplicador.TipoMaquinariaUNE` | `TIPO_MAQUINA_UNE` | 689 | **string** code + label, no lifecycle dates |
| `DGCs[].CodigoCultivo` | `PRODUCTOS` | 1119 | code + name + Latin + EPPO + ~25 boolean attribute columns |
| (prefill/validation) | `CULTIVO_USO_SIGPAC` | 2496 | crop code ↔ SIGPAC uso — the natural cross-check for the declared-crops prefill |
| (variety, `AltaDGC` later) | `VARIEDAD_ESPECIE_TIPO` | ~40k (9.7 MB) | defer until `AltaDGC` is built |

Catalogues move on FEGA's own cadence (fecha probes ranged 2023 → **2026-07-14
itself** across this list), so a refresh path matters — but snapshot-first:
the app must work offline with vendored data from first run.

### The catalogues the 2026-07-14 study missed (added 2026-08-05)

That study scoped itself to `TratamFito`, which was right for the SIEX arc and
wrong as a standing snapshot: seams 2–4 of the cuaderno arc then recorded four
times that a coded field had "no catalogue in the vendored FEGA set", when what
was true is that the field was outside `TratamFito`. The
[registry](maintenance.md) lists **287** catalogues; we now vendor 47.

| Payload field / consumer | idTabla | Rows | Note |
| --- | --- | --- | --- |
| `Analitica.MaterialAnalizado` | `MATERIAL_ANALIZADO` | 4 | **four** values — FEGA splits Cultivo from Producto cosechado |
| `Analitica.TiposAnalisis[]` | `TIPO_ANALISIS` | 6 | incl. 5 "Parámetros del Suelo" |
| `Analitica.TiposSustancias[]` | `SUST_ACTIVAS` | 283 | substance + **CAS number** + código europeo |
| `UsoSemillaTratada.Tratamiento` | `TIPO_TRATAMIENTO` | 4 | codes start at 2 |
| `ComercializacionVD.ProductoVegetal`, `TratamientosPostCosecha.ProductoVegetal`, `Cosecha.ProductoCosechado` | `PROD_VEGETAL` | 692 | harvested **produce**, NOT the crop catalogue; one row per (produce, crop) |
| `Edificaciones[].IdEdificacion` typing (3.4) | `EDIFICACIONES_INSTALACIONES` | 109 | keyed on `Código SIEX` (col 2); col 0 is the tipología |
| 3.1 bis `treatment_record.measure_code` (since 2026-08-09) | `TIPO_MEDIDA_FITOSANITARIA` | 14 | the model's "Tipo de medida", per event |
| ~~3.1 bis~~ — no consumer | `MEDIDA_PREVENTIVA_CULTURAL` | 14 | **not 3.1 bis's list**: a HOLDING-level GIP declaration on `DatosExplotacion`, with no date, plot or intensity and no column in the printed model |
| Irrigation module (`Riego.OrigenAgua[]`, `Fertirrigacion.OrigenAgua[]`) | `ORIGEN_AGUA_RIEGO` | 6 | procedencia del agua **de riego** — see the gap below: NOT seam 5's consumer |
| `CAExplotacion` range; report-language offer | `COMUNIDAD_AUTONOMA`, `PROVINCIA` | 17, 53 | CCAA carries **both** catastro and INE codes; we key on INE |
| `crop.irrigation_code` counterpart | `SIST_EXPLOTACION`, `SIST_RIEGO` | 2, 8 | R/S vs the 8 irrigation methods — see the gap below |
| `crop.growing_environment_code` counterpart | `SIST_CULTIVO` | 33 | 1–4 are AL/M/BP/INV; the other 29 are crop-system distinctions we do not make |
| SIGPAC uso resolution | `USO_SIGPAC` | 32 | the uso codes the provider JSON returns |
| Fertilization module | `MAT_FERTI`, `DETALLE_MATERIAL_FERT`, `MACRONUTRIENTES`, `MICRONUTRIENTES`, `METALES_PESADOS`, `TIPO_FERITILIZACION`, `METODO_APLICACION_FERTILIZANTE`, `TRAT_ESTIERCOLES` | 24, 1243, 16, 7, 7, 3, 7, 9 | section 6 / Anexo III Parte I.C vocabulary |
| Crop planning, irrigation, harvest | `DESTINO_CULTIVO`, `DEST_COSECHA`, `DEST_RES_VEG`, `TIPO_LABOR`, `TIPO_COBERTURA_SUELO`, `MATERIAL_VEGETAL_REPRODUCCION`, `PROC_VEGETAL`, `REGIMEN_TENENCIA`, `PAIS` | 29, 16, 9, 14, 6, 30, 3, 6, 259 | named future consumers |

Two catalogues named by the spec stay **unvendored**: `VARIEDAD_ESPECIE_TIPO`
(84,565 rows / 9.9 MB, deferred with `AltaDGC`) and `ROPO` (173,554 rows /
30 MB) — both are registries, not code lists, and both would dominate the
binary.

**`ROPO_NIVEL` and `ROPO_CATE` are not fetchable today** (checked 2026-08-11).
They would be the authority's own vocabulary behind core's `licence_level`
lookup and table 1.2's carné columns — the natural target for a bidirectional
contract test under the two-tier rule — but the registry marks both
`exportable: false` and the data endpoint returns an empty body. That is a dated
observation, not a verdict: FEGA publishes on its own cadence, so recheck before
concluding they are unavailable. A third sweep of the 287-entry registry the
same day also found `ATRIA` and `ENT_ASESORA` exportable — registries of GIP
groups and advisory entities that could prefill the `advisor` table one day.
Available, not needed.

### Recorded gaps

- **SIEX has no water-abstraction entity at all.** Model section 2.2 asks for
  the *puntos de captación de agua para consumo humano* near each plot
  (Anexo III A.1.f–g), and the seam-5 audit (2026-08-07, against the live
  3.11.4 schema — confirmed current on FEGA's documentation page) found nothing
  to mirror it: every `ActividadesExplotacion` and `DatosExplotacion` block was
  enumerated and no field matches *punto*, *consumo*, *capta*, *distancia*,
  *coordenada* or *masa*. The single water field, `OrigenAgua[]`, appears only
  under `Riego[]` and `Fertilizacion[].Fertirrigacion`, and codes the
  provenance of **irrigation** water — a different question with a different
  subject. The live registry's four water catalogues (`ORIGEN_AGUA_RIEGO`,
  `USOS_AGUA` = the water administration's use taxonomy, `REGANTES` and
  `COMU_REGA` = irrigation-community registries) all belong to that same
  irrigation vocabulary, so `plot_water_point` carries **no coded field** and
  nothing new was vendored: a catalogue is vendored when a named part of the
  app reads it. `plot_water_point` therefore exists for the PRINTED model
  alone — the third time this has happened, after `seed_treatment_plot` (seam 3)
  and `harvest_plot` + the buyer block (seam 4), and it is recorded here rather
  than left to read as an oversight.
- **Ceuta and Melilla have no `CAExplotacion` LABEL — the code is fine.**
  `CAExplotacion` is documented "según codificación INE", INE's ciudad-autónoma
  codes are 18 and 19, and that is exactly what `province_to_ccaa` returns, with
  a unit test pinning it; the schema bounds the field at two characters with no
  enum, so it validates. What is missing is only a row to resolve those codes to
  a name: they are *ciudades* autónomas and FEGA's `COMUNIDAD_AUTONOMA`
  publishes the seventeen *comunidades* only. Nothing displays a CCAA label
  today — the printed book never touches one, and §1.1's Provincia resolves from
  `PROVINCIA`, which does carry `51 CEUTA` and `52 MELILLA` — so a holding there
  prints a complete book. If something ever needs the label, the standing rule
  applies (an unresolvable code prints itself) and the Spanish wording would
  come from INE via the report labels, never invented in a core table.
  **Never map the two cities onto `00 Comunidad Desconocida` or onto a
  neighbouring comunidad**: both would make a record assert something false
  about a holding whose location is perfectly known. The contract test asserts
  the absence for that reason. Found 2026-08-05 by the province ↔ CCAA
  domain/range test; re-read and corrected 2026-08-11, the earlier wording
  ("no value") having overstated it.
- **`crop.irrigation_code` cannot produce a `SIST_RIEGO` code.** Our four
  values (`rainfed`, `sprinkler`, `drip`, `gravity`) answer *two* SIEX
  questions: `SIST_EXPLOTACION` (R/S — total and lossless, mapped) and
  `SIST_RIEGO` (the method). `sprinkler` sits between "Aspersión fija" and
  "Aspersión móvil" with nothing in our schema to choose between them, and
  `rainfed` has no `SIST_RIEGO` code at all. **No mapping is added**, because
  a lossy one behind a green contract test would bake a statement the farmer
  never made into a regulatory export. Closing it needs either a fifth and
  sixth value or splitting the column into `is_irrigated` +
  `irrigation_method_code`; that belongs to the Irrigation module, whose form
  is where a farmer would actually be asked.
- **One-directional mappings need an anchor test.** Where our list is smaller
  than the catalogue (`growing_environment` → `SIST_CULTIVO` 1–4), "every one
  of ours maps to an active code" stays green through a provider
  renumbering that silently redirects every record. Pin the target labels too.
  *(Done 2026-08-05: `growing_environments_map_to_the_siex_growing_system_they_name`
  anchors each of the four on a word of the catalogue's own label.)*
- **`SUST_ACTIVAS` cannot carry every analysis finding.** It codes
  phytosanitary actives — `TipoAnalisis` 1 — so a heavy-metals, nutrients or
  soil-parameters bulletin has nothing to code there. `analysis_record` keeps
  the free `substances_detected` beside the coded `analysis_substance`
  junction for exactly that reason; a serializer sends the codes and leaves
  the wording to the farmer's own folder.

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
`crates/terrazgo-core/catalogues/` (47 files, idTabla names), tests against
the real FEGA files in `crates/terrazgo-core/tests/catalogue.rs`. The rest
of this section is the design rationale, kept as decision history.

Two importer changes since (2026-08-05): a per-catalogue **`code_col`**,
because three files do not lead with their own code (`COMUNIDAD_AUTONOMA`
leads with the catastro code where SIEX wants INE;
`EDIFICACIONES_INSTALACIONES` and `DETALLE_MATERIAL_FERT` lead with their
parent catalogue's), and **`catalogue.source_digest`**, which replaced a
fast path that compared lifecycle dates — that one had to parse every file
before deciding to skip it, and could not see a refresh that corrected a
label without moving a date.

**Two generic tables owned by terrazgo-core.** Reference catalogues serve the
whole farm domain (treatments now; crop prefill, fertilisation, irrigation
later), and modules only depend on core — putting them in core dissolves any
cross-module read. Core stays country-neutral because the *mechanism* is
generic and the Spanish-ness is data: the `geo_feature` pattern.

- `catalogue` — one row per imported catalogue: `id` TEXT PK (the idTabla),
  `source` TEXT (`'siex'` now; other countries' registries later),
  `source_updated_at` (the fecha value / max row date at import),
  `source_digest` (content hash of the vendored bytes — what the startup
  fast path compares), `imported_at`.
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
- **Refresh (shipped 2026-08-09)**: a manual Settings button behind an async
  command — one `GET /catalogos/{idTabla}` per vendored catalogue through the
  `terrazgo-net` seam, then the same parser and the same upsert. Never
  automatic and never required; staleness in between is mild (new codes cannot
  be picked until an update, existing records stay valid). The `/fecha`
  staleness probe was dropped in favour of the content digest the importer
  already keeps: identical bytes cost nothing to detect, and a probe cannot
  see a label corrected without a date moving. Validation runs entirely before
  the write, because the upsert never deletes — see
  [maintenance.md](maintenance.md) §1.
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
  Size is the usual reason but not the only one: `SUST_ACTIVAS` (283 rows)
  is tier 2 because it carries **CAS numbers**, the cross-country key a
  future French or Italian export would match on — owning English names for
  chemicals with a universal identifier would be inventing a worse key.
- **Third clause, added 2026-08-05, because both halves were violated at
  once:** a tier-2 catalogue must have a **named consumer and a display
  resolver**; a tier-1 lookup must have a **`Labels` accessor and a
  bidirectional contract test**. Vendoring a catalogue nothing reads is dead
  weight in the binary; owning a code without a resolver prints a blank cell
  in a legal document; and *not* vendoring what a seam needs is how the same
  seam concludes the authority publishes no list (see "The catalogues the
  2026-07-14 study missed").

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

### What the serializer does NOT emit (dormant-export inventory, 2026-08-07)

The 2026-08-02 CUECYL answer parked this module: it stays compiling,
schema-validated and tested, but is **not extended**. What that park promised
is that *capture* stays SIEX-shaped so a future un-parking rebuilds the arrays
from stored columns — **not** that the serializer keeps pace. Slice 8 then
added five registers, and none of them is serialized. The gap is deliberate;
this table exists so it is visible rather than implied.

`ActividadesExplotacion` currently carries **one** block, `TratamFito`.

| Captured since | Our tables | The twin it would fill | State |
| --- | --- | --- | --- |
| seam 1 (2026-08-04) | `treatment_record.application_end_date` | `TratamFito.FechaFin` | **emitted** — no longer falls back to the start date |
| seam 1 (2026-08-04) | `treatment_record.total_quantity_value` + `_unit_code` | `ProductosFito.Cantidad` | not emitted, **by choice**: the descriptor allows `Dosis` XOR `Cantidad` and the dose is the value every record carries, while a total is absent whenever the dose is a concentration |
| seam 2 (2026-08-04) | `non_field_treatment` (subject `postharvest`) | `TratamientosPostCosecha` | not emitted |
| seam 2 (2026-08-04) | `non_field_treatment` (subjects `premises`, `transport`) | `TratamientosEdifInstalaciones` | not emitted |
| seam 3 (2026-08-04) | `seed_treatment` | `UsoSemillaTratada` | not emitted |
| seam 4 (2026-08-04) | `analysis_record` + its junctions | `Analitica` | not emitted |
| seam 4 (2026-08-04) | `harvest_record` (core) | `ComercializacionVD` | not emitted |
| seam 5 (2026-08-07) | `plot_water_point`, `plot_water_declaration` | **none exists** | nothing to emit — SIEX has no captación entity at any level (see "Recorded gaps") |
| slice 8.5 c2 (2026-08-06) | `analysis_type`, `analysis_substance`, `seed_treatment.treatment_kind_code`, `harvest_record.plant_product_code` | fields inside the four blocks above | not emitted, because their blocks are not |
| slice B seam 1 (2026-08-07) | `irrigation_record` + its junctions | `Riego` | not emitted |
| slice B seam 2 (2026-08-08) | `fertilisation_record`, `fertiliser_material` + their junctions | `Fertilizacion` | not emitted |
| slice B seam 3 (2026-08-08) | `fertilisation_plan` + `fertilisation_plan_crop` | `PlanAbonado` | not emitted |
| slice B seam 4 (2026-08-09) | `analysis_record`'s soil block | `Analitica.ParametrosSuelo` | not emitted, because `Analitica` is not |
| slice C (2026-08-09) | `treatment_record.measure_code` + intensity + `measure_registration_number` | `TratamFito.OtrasActuacionesFito` | not emitted — **and the precheck says so for every record that carries a measure**, not just the productless ones (widened 2026-08-10). A PURELY non-chemical actuation would serialize as a `TratamFito` with an empty `ProductosFito`, a record asserting that a treatment happened while naming nothing that was done. A MIXED one — a spray and a measure on the same record, which the form allows — would export cleanly while silently losing the measure, and that is the worse of the two: nothing in the output would say anything had been left behind. `records_with_non_chemical_measure` refuses both with a nameable list |
| slice C (2026-08-09) | `treatment_record.advisor_id` + its snapshots | `TratamFito.AsesorValidacion` | not emitted. `NumROPO` is stored and would fill the one required member; the twin's `Validacion`/`Fecha`/`Confirmacion`/`Contrato` are **not** captured, because model 3.1 bis asks for a handwritten signature and the book has no signature capability by design |

`DatosExplotacion.MedidaPreventivaCultural` joins the same list from the other
side: the twin's holding-level declaration of which IPM practices the farm
follows (catalogue `MEDIDA_PREVENTIVA_CULTURAL`, 14 rows, vendored). It is
optional in the twin, sits beside the parked `AltaDGC`/`CambioCultivoDGC`, and
the printed model has no column for it anywhere — so nothing captures it, and
the catalogue stays consumer-less. It is emphatically **not** what model 3.1
bis's "Tipo de medida" speaks in: that is `TIPO_MEDIDA_FITOSANITARIA`, per
event, with a date, a plot and an intensity.

Three twin fields are captured by **nothing**, and that is a decision rather
than an oversight. The line: a field required by the twin is captured even
when the printed model has no column for it (`Fertilizacion.BuenasPracticas`
is, in `fertilisation_practice`); a field optional in the twin AND absent from
the model is recorded here instead.

| The twin's field | Why nothing stores it |
| --- | --- |
| `Fertilizacion.Fertirrigacion` (its own `SistemaRiego`, `DosisN`/`DosisP`, `OrigenAgua`, `NumContador`) | the printed §6 has no fertigation columns, `application_method_code` already records *that* it was fertigation (`METODO_APLICACION_FERTILIZANTE` 5 and 6), and the water side is §8's register |
| `Fertilizacion.GestionSostInsu` | twin-only boolean, optional, no model column and no duty behind it |
| `BuenasPracticasRiego` (on **both** `Fertilizacion` and `Riego`) | optional in both, and the "Riego" ámbito of `BUENAS_PRACTICAS_AMBITOS` has no column in either printed section |

Bringing the serializer up to the schema is roughly a seam's worth of work
(five blocks, their precheck rules and schema-validated tests) for a format
with no delivery path today, so it stays an open decision rather than a
scheduled slice. Nothing here blocks the printed book, which is the
compliance artifact.

### Blocks with no capture at all — the eco-scheme registers (2026-08-11)

The table above lists twins we fill from stored columns but do not emit. These
are the other kind: nothing in the schema feeds them, because the practices they
describe are not recorded anywhere in the app. They are the SIEX side of the
printed model's **section 9**, whose annotation duties come from RD 1048/2022
(the full article-by-article inventory is in `cuaderno-print.md`).

| The twin | The register it serves | The duty behind it |
| --- | --- | --- |
| `Pastoreo` (`FechaInicio`/`FechaFin`, `AnimalesPropios`/`Terceros`, `Animales[]` = `{REGA, Numero, Especie}`) | model 9.1, extensive grazing | RD 1048/2022 art. 30.2 ter |
| `DatosCubierta` (`FecEstablecimientoCub`, `AnchuraCubierta`, `AnchuraLibreProy`, `TipoCobertura`) | model 9.4 and 9.5, plant and inert covers — it matches those pages field for field | arts. 42.1.a/c/e and 43.1.a–b |
| `LaboresCulturales` (`FechaInicio`/`FechaFin`, `TipoLabor`, `DepositadoSueloDesb`/`Poda`) | model 9.2's maintenance activities and 9.3's levelling and caballones | arts. 31, 31.4.d, 45.2 |

Their vocabularies are already to hand: `TIPO_COBERTURA_SUELO` and `TIPO_LABOR`
are vendored (and read by nothing today — see `maintenance.md` §1), and
`ESPECIE_ANIMAL` and `RAZAS` are published in the FEGA registry.

**`EST_FENOLOGICO` → `TratamFito.DGCs[].EstadoFenologico` was the one that
belonged to a table we already had, and it is BUILT (2026-08-12).** The field
hangs off the treated crop — our `treatment_plot.growth_stage_code` — and
Reglamento (UE) 2023/564's annex asks for the growth stage "where relevant", so
the duty existed whatever the twin's optionality said. Two notes for a reader of
the payload: the value is the catalogue's own code 1-10 and **not** the BBCH
stage (the monograph's 0-9 is a separate column, and the book prints that one),
and an unparseable code is dropped rather than refused — the field is optional in
the format, so it must not fail an export of everything else. The conditionality
rules are in `cuaderno-print.md` → "What the EU annex adds".

One alignment note. `SiembraPlantacion` carries `MaterialTratado: boolean` and
`NumLote`, so the exchange format models our §3.2 as *a sowing that used treated
material* rather than as a register of its own — which is why
`UsoSemillaTratada` carries no plots, and why seam 3 had to take the plot
linkage from the printed model instead. Nothing follows from it; it explains a
shape already chosen.

### The precheck now has no renderer (2026-08-11)

`export_cuaderno_precheck` and `export_cuaderno` stay registered, compiled,
schema-validated and tested, but **nothing in the interface calls them**: the
export panel was removed from the record book's export tab, which now offers the
PDF and the spreadsheet over the completeness advisory. A button producing a
file with nowhere to go was the wrong thing to show a farmer.

That is worth recording rather than just doing, because it recreates the exact
condition that hid `records_with_non_chemical_measure` until the 2026-08-09
review: a field on a command's response that no scripted check renders. So
un-parking the export means rebuilding **its UI and its checks**, not merely
calling the command again — and any field added to the precheck meanwhile is
unexercised end to end by construction.

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

## Open questions for CUECYL — ANSWERED 2026-08-02

CUECYL replied to the email. Two answers settle the whole section:

1. **Connecting to REACYL or CUECYL web services requires being a company or an
   autónomo.** That closes questions 1–3 and 5 for now: no individual, however
   the software is licensed, gets web-service credentials.
2. **CUECYL has no farmer-facing file upload** — no manual submission of the
   descriptor JSON. That answers question 4, the one the shipped export
   depended on, in the negative.

Consequences recorded in the status banner at the top of this document: the
serializer is parked, `AltaDGC` (gap 2) and the IUWS client are out of scope,
and the printable PDF becomes the compliance artifact. Questions 6 (REACYL DGC
Excel columns) and 7 (`UnidadGestora`) are moot while no submission path
exists — 6 doubly so, since the REACYL Excel import is dropped in favour of
the public SIGPAC *cultivos declarados* service (see "Farmer-side data paths"),
which needs no login at all.

The questions as originally sent, kept for the record:

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
