# SIEX-aligned export — design notes

> Status: **DORMANT — the export has no delivery path, and is complete anyway.**
>
> **Parked 2026-08-02.** CUECYL answered the onboarding questions below:
> connecting an application to the REACYL or CUECYL web services requires being
> a **company or self-employed professional (autónomo)**, and **CUECYL offers no
> farmer-facing utility to upload a descriptor JSON file**. The printable PDF
> cuaderno is the app's compliance artifact instead, and
> `AltaDGC`/`CambioCultivoDGC` (gap 2) plus the web-service client stay out of
> scope — no delivery path exists for either.
>
> **Finished 2026-08-20 → 08-22.** "Parked" was read for a while as "frozen",
> and by August the serializer emitted **one** of the format's fifteen activity
> blocks while twelve had registers behind them. That is the failure the park
> was never meant to license: capture kept moving, the descriptor did not, and
> nothing said so. The "finish the serializer" arc (below) took it to
> **thirteen** — every block with a register behind it, `Cosecha` and
> `EnergiaUtilizada` having none by decision — plus `TratamFito`'s three
> sub-blocks. `terrazgo-siex` stays compiling, schema-validated and tested.
>
> **So dormancy means no UI and no transport, never a stale serializer.** A
> register added from here owes its twin the same day, or this document owes an
> explicit row saying why it has none.
>
> Not blocked by the CUECYL answer: the **national FEGA** services below
> (`/catalogos/*`, `POST /existeNIF`) are public and no-auth — the barrier is
> specific to the regional CUE/REA web services.
>
> History: design 2026-07-04; re-diffed against schema v3.11.4 on 2026-07-14;
> capture schema built 2026-07-15, export module built 2026-07-16, extracted to
> its own crate 2026-08-20 — see "Export module" below. This document maps
> Terrazgo's registers onto the official CUE exchange format.

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
   └─ ActividadesExplotacion          ← 13 of 15 blocks emitted since 2026-08-22
      ├─ TratamFito[]         (+ OtrasActuacionesFito, AsesorValidacion, FechaSeca)
      ├─ UsoSemillaTratada[]  TratamientosPostCosecha[]  TratamientosEdifInstalaciones[]
      ├─ Analitica[]  ComercializacionVD[]  SiembraPlantacion[]
      ├─ Fertilizacion[]  Riego[]  PlanAbonado[]
      ├─ Pastoreo[]  LaboresCulturales[]  DatosCubierta[]
      └─ Cosecha[]  EnergiaUtilizada[]   ← NOT emitted: no register behind either,
                                            by decision (see "The two blocks
                                            nothing will fill")
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
| `premises.class_code` (3.4, since 2026-08-21) | `EDIFICACIONES_INSTALACIONES` | 109 | keyed on `Código SIEX` (col 2); col 0 is the tipología. **This row used to read "`IdEdificacion` typing", which was a guess and is now known wrong** — the real consumer is REA bloque 8's `claseInstalacion`, obligatory when a treated building must be identified for the CUE. All 109 are real estate; no vehicle appears, which is why `class_code` is buildings-only |
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
  `source_digest` (content hash of the bytes that produced the stored rows,
  vendored or fetched — what a refresh compares to recognise a copy it
  already holds), `imported_by_version` (the app version whose vendored
  snapshot was last imported here — what *startup* compares, so a user's
  refresh survives restarts but not an update; docs/maintenance.md §1),
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

## Export module (built 2026-07-16; moved to its own crate 2026-08-20)

**`terrazgo-siex`** — the query layer + serializer for one farm+season,
schema-validated in `crates/terrazgo-siex/tests/export.rs` against the vendored
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

### Where it lives, and why it had to move (2026-08-20)

It was `module_cue::export` until the "finish the serializer" arc started. That
was tenable while `TratamFito` was the only block; it stopped being tenable the
moment the inventory below was counted properly. **The format declares fifteen
activity blocks and ten of them come from `module-fertilisation` and
`module-ecoscheme`** — crates a module may never depend on. Same wall that
produced `terrazgo-recordbook`, so the same answer: a top-layer consumer above
the modules, and `terrazgo-recordbook`'s **sibling** rather than part of it.
Neither depends on the other. They read the same registers and project
different documents for different readers under different rules — the book
prints what exists and gates nothing, the descriptor is validated by an
authority and may not blank, invent or drop a required field.

Three things moved with it rather than being duplicated:

- **`export_alias` → `terrazgo-core`.** It aliased `crop` — a core row — on the
  day it shipped, and now aliases registers owned by core and all three
  modules. One module's table keying two other modules' rows is the coupling
  the layering exists to prevent; "shared DATA → core" is the placement rule's
  first case, and `unit` moved for exactly this reason on 2026-08-07. It also
  joined **core's backup fingerprint**, which it had never been in: a stale
  backup missing that table would have imported cleanly and silently lost every
  frozen alias, which is the one thing about this table that must never happen.
- **`crop_groups` → `module_cue::grouping`.** Both documents split a multi-crop
  treatment identically and *must* — the book prints one 3.1 row per crop
  group, the export emits one `TratamFito` per group — so the rule belongs to
  the domain they both read, not to whichever consumer was written first.
- **`list_treatment_records_for_export` became `pub`.** Its name is the guard
  its crate visibility used to be: a caller that is not building an export and
  wants soft-deleted rows is almost certainly making a mistake.

`terrazgo_siex::db::migrations()` composes the same schema the shell does,
pinned by a contract test in `src-tauri/tests/migration_composition.rs` written
**with** the crate and verified by breaking it — the record book's equivalent
spent eleven days as a doc comment describing a test that did not exist.

### The law outranks the format: required, obligatorio and binding (2026-08-20, restated 2026-08-22)

**The precedence to reach for whenever a capture question is open. The decrees
are paramount; the exchange format is transport.** Conflating these produces both
false gaps and false duties, and it has been conflated twice while building this
arc — each time by testing the lowest of the three.

Listed **highest authority first**, which is the reverse of the order they get
noticed in:

1. **The decrees** create the duty to keep the record at all, and decide what a
   register IS. Where a decree obliges an annotation no schema carries, **we
   carry it**: RD 1048/2022 anexo IV is the proof, and the record book prints a
   "9.6" the official model has no page for.
2. **Anexo V's `OBLIGATORIEDAD` column** is FEGA's own per-field duty flag. It
   decides what to capture *inside a block we have chosen to send* — a real
   requirement even when no decree names it — and it grades whole blocks
   voluntary: `EnergiaUtilizada` **6/6 Voluntario**, `ComercializacionVD`
   **5/5**, `Analitica` **8/8** ("en caso de haberse realizado"), the REA
   editable block **18/18**.
3. **JSON Schema `required`** is *structural validity of an entry you chose to
   send*. `ActividadesExplotacion` declares **no required properties at all**
   and every block is `0..n` in the descriptor sheet, so no block is ever
   obligatory. A field marked required inside `Cosecha` binds only a `Cosecha`
   entry that exists.
4. **The printed model is not law at all.** It is a layout source — a downloaded
   Andalucía v6 PDF whose 2023 "OPCIONAL" heading on section 6 predated RD
   1051/2022 and sat in this repo's docs as law until the 2026-08-07 audit.
   Never justify capturing or not capturing anything from it.

**Where the Anexo VI descriptor sheet sits, added 2026-08-22 when seam 5 found
the two annexes contradicting each other.** Neither is law: both are annexes to
the same FEGA resolution (BOE-A-2023-13035), technical specifications rather
than decrees. **Anexo V outranks Anexo VI** — V is the *definición de variables*
(what a field means, and its duty flag) while VI is the web-service *interface*
descriptor, i.e. how bytes travel; V is what tier 2 above already names; and our
copy of V is the **2025-11-20 corrección de errores**, published to fix what the
earlier documents said. Seam 5 hit the disagreement twice, on
`OtrasActuacionesFito`'s exclusivity and on `NumRegistroMDF`'s scope.

**But a tie-break is not a licence to destroy data.** Where V *demands*
something, obey it; where V says a field "debe ir vacío" and VI scopes that
emptying differently and no decree names the field at all, keep what the farmer
recorded. Refusing to send is reversible and visible; dropping a stored value is
neither.

**The corollary cuts both ways, and both halves have bitten.** Never build a
register to satisfy the format — that is the inversion the record book exists to
avoid, and why `Cosecha` and `EnergiaUtilizada` stay unfilled. And never drop a
duty because the format has no member for it — `plot_water_point` exists with no
twin at all, because Anexo III A.1.f–g asks for it.

So the schema is a deliberate **superset**: SIEX is the national
activity-exchange format for everything a holding might report, including
voluntary sustainability and statistical data, and the cuaderno decrees pick a
subset of it. A block existing in the schema is an offer, not an obligation —
and a field FEGA marks `Obligatorio` inside a block we do send is a real
requirement even when no decree names it (the standing line below).

### The two blocks nothing will fill, and why that is a decision (2026-08-20)

- **`Cosecha`** ("COSECHA/RECOLECCIÓN/SIEGA") is the harvesting *operation*,
  with thirteen `Obligatorio` fields including five booleans about seed
  retention, environmental non-harvest and whether the mown residue was left on
  the ground. No decree asks a cuaderno to keep any of that. Our
  `harvest_record` is *what left the holding*, which is `ComercializacionVD`.
  **Checked, because it looked like a seam-2 error and is not**: block 11 is
  named for siega and carries "depositado en el suelo de los restos **segados**"
  while block 5 `LaboresCulturales` carries "restos **desbrozados**" and "de
  **poda**" — but `TIPO_LABOR` code 5 is literally *"Desbroce y siega"*, and RD
  1048/2022 art. 31 distinguishes *"siega para producción o mantenimiento"*.
  Model 9.2's register is the **maintenance** one, which is where we put it.
- **`EnergiaUtilizada`** is voluntary in every one of its six fields and has no
  register behind it.

Building either would be capture driven by the exchange format rather than by a
duty — the exact inversion the record book is built to avoid.

### What the serializer does NOT emit (dormant-export inventory, 2026-08-07)

The 2026-08-02 CUECYL answer parked this module: it stays compiling,
schema-validated and tested. The park promised that *capture* stays SIEX-shaped
so a future un-parking rebuilds the arrays from stored columns — **not** that the
serializer keeps pace, and by 2026-08-07 slice 8 had added five registers that
none of it reached. This table existed so that gap was visible rather than
implied. **The "finish the serializer" arc closed it** (2026-08-20 → 08-22): the
rows below now record what each block sends and what it deliberately does not,
which is the more useful thing for the table to be.

`ActividadesExplotacion` carries **thirteen** blocks since 2026-08-22 (seams
1-4): `TratamFito`, `TratamientosPostCosecha`, `TratamientosEdifInstalaciones`,
`UsoSemillaTratada`, `Analitica`, `ComercializacionVD`, `SiembraPlantacion`,
`Fertilizacion`, `Riego`, `PlanAbonado`, `Pastoreo`, `LaboresCulturales` and
`DatosCubierta` — **every block with a register behind it**, since `Cosecha` and
`EnergiaUtilizada` have none by decision. Seam 5 closed the last three rows the
same day: they were sub-blocks of `TratamFito` rather than blocks of their own,
so nothing below reads "not emitted" any more unless it says why it never will.

| Captured since | Our tables | The twin it would fill | State |
| --- | --- | --- | --- |
| seam 1 (2026-08-04) | `treatment_record.application_end_date` | `TratamFito.FechaFin` | **emitted** — no longer falls back to the start date |
| seam 1 (2026-08-04) | `treatment_record.total_quantity_value` + `_unit_code` | `ProductosFito.Cantidad` | not emitted, **by choice**: the descriptor allows `Dosis` XOR `Cantidad` and the dose is the value every record carries, while a total is absent whenever the dose is a concentration |
| seam 2 (2026-08-04) | `non_field_treatment` (subject `postharvest`) | `TratamientosPostCosecha` | **emitted since 2026-08-21**. Its `Cantidad` is the produce in **kilograms** — Anexo V fixes the unit and the block carries no unit member, while model 3.3 prints tonnes, so the serializer converts. `ProductosFito` here has no `Dosis` at all and requires `Cantidad`, which is what this register captures |
| seam 2 (2026-08-04) | `non_field_treatment` (subjects `premises`, `transport`) | `TratamientosEdifInstalaciones` | **emitted since 2026-08-21**, keyed on `premises_es_extension.rea_installation_code`. Model 3.5's vehicles ride in the same block for want of any other: the WS descriptor has **no transport block at all** |
| seam 3 (2026-08-04) | `seed_treatment` | `UsoSemillaTratada` | **emitted since 2026-08-21**. `Producto` is the **crop** (Anexo V field 1: "Cultivo — código del cultivo del catálogo SIEX"), so it takes `crop_code`; its optional `ProductosFito` child is emitted by nothing, because the register stores no amount of product and model 3.2 prints no such column |
| seam 4 (2026-08-04) | `analysis_record` + its junctions | `Analitica` | **emitted since 2026-08-21**, and the only block of the four that added **no precheck rule**: the schema requires just the material and the date, both NOT NULL here |
| seam 4 (2026-08-04) | `harvest_record` (core) | `ComercializacionVD` | **emitted since 2026-08-21**. One stored date fills both ends, and the block carries its own `Unidad`, so the stored kg or t travels unconverted — the opposite of `TratamientosPostCosecha`, whose unit Anexo V fixes. Three members our register holds are **not** sent: `TipoVenta` is optional, Voluntario and unstored (the printed model draws no comercializada/directa distinction, so claiming one would invent it), while `NumFactura` and `NumLote` are in the descriptor SHEET and **not in the JSON Schema**, so `delivery_note_ref` and `lot_number` stay printed-only |
| seam 5 (2026-08-07) | `plot_water_point`, `plot_water_declaration` | **none exists** | nothing to emit — SIEX has no captación entity at any level (see "Recorded gaps") |
| slice 8.5 c2 (2026-08-06) | `analysis_type`, `analysis_substance`, `seed_treatment.treatment_kind_code`, `harvest_record.plant_product_code` | fields inside the blocks above | **all emitted since 2026-08-21**; `plant_product_code` joined the rest when seam 2 landed `ComercializacionVD` |
| slice B seam 1 (2026-08-07) | `irrigation_record` + its junctions | `Riego` | **emitted since 2026-08-21**, and the seam's easiest block: the register was SIEX-shaped from the day it shipped, so `OrigenAgua` was already a junction and `energy_type_code`/`meter_number` already existed. `BuenasPracticasRiego` is still emitted by nothing — Voluntario in Anexo V on this block AND on `Fertilizacion`, with no column in either printed section. Note the unit: Anexo V names m³ and L as valid, the register stores m³ or m³/ha, and `UNIDADES_MEDIDA` carries m³/ha as code 19 — so a per-hectare volume states itself rather than being converted with a surface the record may not carry |
| slice B seam 2 (2026-08-08) | `fertilisation_record`, `fertiliser_material` + their junctions | `Fertilizacion` | **emitted since 2026-08-21**, with its `Fertirrigacion` sub-block (see below). The composition comes from the REGISTRY row rather than the record — which is why `get_fertiliser_material_for_export` exists: the ordinary getter filters soft-deleted materials, and a decade-old record must still resolve the material it named. `BuenasPracticas` sends an **empty array** when none was declared, which is schema-valid (no `minItems`) and is what Anexo V's own field 6 says the field does. `EquipoAplicador` is omitted entirely when no machine is named — C.g's "cuando proceda" and the block's `oneOf`, which a half-filled block would fail |
| slice B seam 3 (2026-08-08) | `fertilisation_plan` + `fertilisation_plan_crop` | `PlanAbonado` | **emitted since 2026-08-21**. Its required set IS art. 5.a's list plus `Herramienta`, which is the corroboration the register's design already leant on. Two optional members are stored by nothing on purpose: `Asesor` (a REGFER code) and `FechaAsesoramiento` — art. 6.6's advice requirement is on the DOCUMENT that is kept, and the record art. 5.a describes names no advisor |
| slice B seam 4 (2026-08-09) | `analysis_record`'s soil block | `Analitica.ParametrosSuelo` | **emitted since 2026-08-21**, and omitted entirely when the bulletin stated no soil figure — an absent measurement is absent, never zero |
| slice C (2026-08-09) | `treatment_record.measure_code` + intensity + `measure_registration_number` | `TratamFito.OtrasActuacionesFito` | **emitted since 2026-08-22**, and a purely non-chemical actuation is now a first-class entry — `ProductosFito` is absent from `TratamFito`'s required set, and the member is omitted rather than sent empty. What did NOT happen is the retirement the plan promised: Anexo V grades all five members *"excluyente con el subbloque siguiente de «Productos fitosanitarios»"*, so a MIXED record (a spray and a measure on one row, which model 3.1 bis prints side by side) still has no shape, and the rule narrowed to `records_mixing_product_and_measure`. The decree agrees from the other side — Anexo III Parte I B lists no non-chemical member at all, so a row carrying both is a row carrying two treatments. `BuenasPracticas` is emitted by nothing: Voluntario, no printed column, and its catalogue repeats each code per ámbito, so one integer cannot say which row was meant. See "Seam 5" |
| slice C (2026-08-09) | `treatment_record.advisor_id` + its snapshots | `TratamFito.AsesorValidacion` | **emitted since 2026-08-22**. `NumROPO` fills the one required member and is the only one sent; `Validacion`/`Fecha`/`Confirmacion`/`Contrato` are **not** captured, because model 3.1 bis asks for a handwritten signature and the book has no signature capability by design. A record naming an advisor with no ROPO is refused rather than sent without the block — Anexo V grades field 50 Obligatorio here, where blocks 1.2 and 1.3 grade it Voluntario, which is why the non-field builders omit it instead |
| eco-scheme seam 3 (2026-08-19) | `sowing_record` + `sowing_plot` (**core**) | `SiembraPlantacion` | **emitted since 2026-08-21**, and still narrower than the twin. `FechaInicio`/`FechaFin`, `FechaInundacion` and `Cantidad` map directly; `DGCs[]` is the plot junction with its crop, and it sends no `SuperficieCultivada` because the descriptor defines that as equal to the DGC's surface unless stated otherwise — which is exactly what `sowing_plot`'s missing surface column means. The required `SiembraPlantacion` member is **`sowing_record.kind_code`, not the crop**: the 2026-08-19 reading of it as Anexo V's "Cultivo" was wrong, and the WS descriptor types it `number(1)`, "1 Siembra 0 Plantación" (**`MATERIAL_VEGETAL_REPRODUCCION` is still not its catalogue** — that file stays orphaned). The seed-provenance members come from 3.2 through `seed_treatment.sowing_record_id`; `SiembraDirecta` is still captured by nothing, being already a `cultural_operation` of kind `no_tillage`, and so are `DosisSiembra`/`MarcoPlantacion`/`DensidadPlantacion`/`UnidadesRemolacha`, which Anexo V makes mutually exclusive alternatives to the `Cantidad` this register stores |
| eco-scheme seam 3 (2026-08-19) | `treatment_record.drying_date` | `TratamFito.FechaSeca` | **emitted since 2026-08-22**, and deliberately never gated. Model 9.3's fourth column, and one of art. 45.2's five dates. It sits on the treatment because Anexo V says *"fecha en la que se realiza el secado para la realización del tratamiento"* — the field is dried in order to spray. Anexo V grades it Obligatorio *"cuando se trate de cultivos bajo agua"* and `sowing_record.flooded_on` would make that checkable, but the condition is the drying rather than the flooding: a rice herbicide applied on water is a lawful record with no date to state |
| eco-scheme seam 2 (2026-08-19) | `cultural_operation` + `cultural_operation_plot` | `LaboresCulturales` | **emitted since 2026-08-22.** `FechaInicio`/`FechaFin` and `TipoLabor` map directly — the last through `module_ecoscheme::siex::cultural_operation_kind_to_siex`, whose map is **deliberately not injective**: our `mowing` and `brush_cutting` both answer to `TIPO_LABOR` 5, because model 9.4 prints Siega and Desbrozado as two columns where the catalogue has one code. **`DepositadoSueloDesb` and `DepositadoSueloPoda` are DERIVED, not stored**, and seam 4 had to widen the reading: code 9's own label names *poda* residue, so `DEST_RES_VEG` **1** ("Incorporación al suelo o distribución en parcela") joins it as `RESIDUE_LEFT_ON_PLOT` — 9 alone would leave a desbroce left on the ground with no code to answer to, and an Obligatorio boolean stuck at false for a farmer who did exactly what art. 42.1.c describes. The kind then decides which of the two booleans it fills, and `pruning_removal` fills neither: `TIPO_LABOR` 11 is *"Eliminación de restos de poda"*. That the twin hangs those booleans off `LaboresCulturales` rather than off `DatosCubierta` is what settled where the P7 evidence chain lives. `Maquinaria[]` is emitted by nothing — Voluntario in all six of its Anexo V fields, and no printed column |
| eco-scheme seam 1 (2026-08-18) | `grazing_record` + `grazing_plot` + `grazing_animal` | `Pastoreo` | **emitted since 2026-08-22.** Every member has a column behind it except two, which are **derived**, and the per-line `{REGA, Numero, Especie}` the animal junction mirrors. `AnimalesPropios`/`AnimalesTerceros` are **required booleans** — Anexo V reads "Pastoreo con animales de la explotación (S/N)" — **not** the head-count split two seam-1 columns of the same name assumed; those columns were dropped on 2026-08-20 and the serializer computes the booleans from each line's REGA against `farm_es_extension.rega_code`, which the precheck therefore demands of a season holding grazings. `FechaFin` is **required** in 3.11.4 (it was optional in 3.3.0) while `ended_on` is nullable, and the 2026-08-19 note saying a serializer must SKIP such records was **overruled on 2026-08-22**: the precheck names them and the export refuses, because a record vanishing from the file with nothing on screen saying so is what the precheck exists to prevent. See "Seam 4" below |
| eco-scheme seam 4 (2026-08-19) | `soil_cover` + `soil_cover_plot` | `DatosCubierta` | **emitted since 2026-08-22.** `FecEstablecimientoCub`, `AnchuraCubierta`, `AnchuraLibreProy` and `TipoCobertura` map directly, and `DGCs[]` is the plot junction. The width members are **nullable as a group** in the register — art. 42.1.e falls due later than 42.1.a's, so a cover between the two is a complete record with no widths — and **the export nonetheless refuses one**: Anexo V grades both `Obligatorio` for exactly the three cover types this register can hold. That is the point where the two documents part company, and it is the arc's own rule applied to the gate rather than only to capture (see "Seam 4"). The cover's **maintenance is not in this block**: art. 42.1.c's siega, desbroce and pastoreo travel as their own `LaboresCulturales` and `Pastoreo` entries naming the cover through `DGCs[].Cubiertas[]`, which is where the twin puts them |

`DatosExplotacion.MedidaPreventivaCultural` joins the same list from the other
side: the twin's holding-level declaration of which IPM practices the farm
follows (catalogue `MEDIDA_PREVENTIVA_CULTURAL`, 14 rows, vendored). It is
optional in the twin, sits beside the parked `AltaDGC`/`CambioCultivoDGC`, and
the printed model has no column for it anywhere — so nothing captures it, and
the catalogue stays consumer-less. It is emphatically **not** what model 3.1
bis's "Tipo de medida" speaks in: that is `TIPO_MEDIDA_FITOSANITARIA`, per
event, with a date, a plot and an intensity.

### One column pair modelled on a stale reading — found, then dropped (2026-08-20)

`grazing_record.own_animals` and `third_party_animals` were `INTEGER` head
counts whose schema comment said the twin "splits the head count by ownership".
**It does not.** Schema 3.11.4 types `AnimalesPropios` and `AnimalesTerceros` as
**booleans**, both required, and Anexo V spells them out: *"Pastoreo con
animales de la explotación (S/N)"* and *"Pastoreo con animales de terceros
(S/N)"*. They ask **whether**, not **how many**.

The two columns were added in seam 1 against the 3.3.0 descriptor, which carried
neither field; the reading came from the member names alone. Found on 2026-08-19
when the descriptors were refreshed to 3.11.4, and **dropped in the arc's
closing seam** — the `crop.sown_on` precedent, and for the same three reasons:

- **Nothing printed them.** Model 9.1's "Nº animales desplazados al pasto" is a
  per-line figure and comes from `grazing_animal.animal_count`.
- **The booleans the twin wants are derivable** from what is already stored: a
  line whose `rega_code` is the holding's own is its own animals, and any other
  is a third party's. Storing them would be derived state that can drift, which
  the data-model rules forbid. `third_party_animals_keep_their_owners_rega` pins
  that the two REGAs survive on their lines, which is what a serializer reads.
- So they were captured, printed nowhere and read by nothing, on a misreading.

**A serializer therefore computes both booleans**, from each line's `rega_code`
against `farm_es_extension.rega_code`; neither is a stored column and neither
should become one.

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

**This table is now empty, and that is the point of having kept it.** All three
twins have left it, in the order the eco-scheme seams shipped:

| The twin | Left on | Into |
| --- | --- | --- |
| `Pastoreo` | 2026-08-18 | `grazing_record` and its two junctions (model 9.1) |
| `LaboresCulturales` | 2026-08-19 | `cultural_operation` — one table serving model 9.2, the book's own "9.6" for anexo IV, 9.3's two unprinted dates and 9.4's cover maintenance |
| `DatosCubierta` | 2026-08-19 | `soil_cover` and its junction (models 9.4 and 9.5) |

**And all three are emitted since 2026-08-22** (seam 4), which closes the loop
this table was opened to keep visible: every block the printed book records has a
register behind it, and every one of those registers reaches the descriptor.

Their vocabularies came with them. `TIPO_COBERTURA_SUELO` gained its consumer
with the cover register, `TIPO_LABOR` and `DEST_RES_VEG` with the
cultural-operation one, and `ESPECIE_ANIMAL` was vendored with the grazing one.
`RAZAS` is published but **deliberately not vendored** — neither model 9.1 nor
`Pastoreo.Animales[]` asks for a breed (`maintenance.md` §1 records the
negative).

**One descriptor-versus-schema drift is worth naming here, because reading the
wrong artifact would reopen a settled decision.** The descriptor sheet gives
`DatosCubierta` a required `ActividadCubierta[]` child — `{ActSobreCubierta,
Fecha}` against a "catálogo de actividad de la cubierta" — and **the JSON Schema
has no such member**. This is a *live* disagreement inside 3.11.4, not a change
between versions: the sheet has carried it since 3.3.0 and the schema has never
had it. The standing rule from the 2026-07-14 re-diff applies, and this is its
third instance after `MateriaActivaFormulado` and `HorasUtilizacion`: **the JSON
Schema is what validates, so the schema wins.**

The real corroboration for the cover design is elsewhere in the same block, and
it is a genuine version change (3.3.0 → 3.11.4, confirmed in both the sheet and
the schema): **`AnchuraCubierta`, `AnchuraLibreProy` and `TipoCobertura` were
REQUIRED and are now optional**, leaving `FecEstablecimientoCub` and `DGCs` as
the only mandatory content. That is exactly the shape art. 42.1 describes — the
establishment date is due within a month and the widths on a later deadline, so
a cover between the two is a complete record with no widths — and it is why
`soil_cover` keeps them nullable and prints them blank rather than as zeros.

One member is captured by nothing and is worth naming rather than rediscovering:
**`LaboresCulturales.Maquinaria[]`** is optional in the twin AND absent from the
printed model, so the cultural-operation register records no machinery.

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

One alignment note, and it became a decision on 2026-08-19.
`SiembraPlantacion` carries `MaterialTratado: boolean` and `NumLote`, so the
exchange format models our §3.2 as *a sowing that used treated material* rather
than as a register of its own — which is why `UsoSemillaTratada` carries no
plots, and why the treated-seed seam had to take the plot linkage from the
printed model instead.

**The eco-scheme arc's sowing register did NOT merge with it.** The two stay
separate tables with no link, and the reason is in their junctions:
`seed_treatment_plot.surface_sown_ha` is `NOT NULL` because model 3.2 prints
"Superficie sembrada (ha)", while model 9.3 prints only "Id. Parcelas" and
dates. Merging the records means merging the plot sets, and then either 3.2
loses a guarantee it has shipped with or 9.3 demands a surface no decree asks
for. The printed model keeps them as two tables filled independently; only this
format merges them, and a register derives from the decree, never from the form
and never from the exchange format.

**So merging them was a SERIALIZER's job, not the schema's — and seam 2 settled
how** (2026-08-21, above). The matching-on-date-and-plot-set guess this
paragraph originally proposed was declined: it would have produced a silent
`MaterialTratado: false` for a farmer whose two registers disagreed by a day,
with nothing on screen to reveal it. The farmer states the link instead, on the
3.2 form, through `seed_treatment.sowing_record_id`. The double entry a P5 rice
grower still faces (the sowing date typed in both registers) is exactly what the
paper form already asks of them.

### The precheck now has no renderer (2026-08-11)

`export_cuaderno_precheck` and `export_cuaderno` stay registered, compiled,
schema-validated and tested, but **nothing in the interface calls them**: the
export panel was removed from the record book's export tab, which now offers the
PDF and the spreadsheet over the completeness advisory. A button producing a
file with nowhere to go was the wrong thing to show a farmer.

That is worth recording rather than just doing, because it recreates the exact
condition that hid the non-chemical-measure refusal until the 2026-08-09
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
5. **Advisor.** `AsesorValidacion` carries the GIP advisor. **Closed** — and it
   was never optional: the 2026-08-09 audit found Anexo III Parte I B.d reads
   *"Identificación del aplicador y, en su caso, del asesor"*, one sentence
   naming two identifications, so the advisor is a **binding** field on the
   treatment and "fine to omit" was wrong. Core's `advisor` table and the
   snapshots on `treatment_record` landed with the record-book completion arc;
   the block is emitted since 2026-08-22 (seam 5), sending `NumROPO` and
   nothing else.

## The "finish the serializer" arc (2026-08-20 → 08-22, COMPLETE)

Started because the developer wanted the descriptor **on par with the schema
and the rest of the cuaderno**: twelve blocks had capture behind them and one
was emitted. Scope settled in conversation — **13 of the 15 blocks**, with
`Cosecha` and `EnergiaUtilizada` recorded above as having no register behind
them. Each seam ships descriptor types + builder + precheck rules +
schema-validated tests, the pattern `TratamFito` already follows.

| Seam | Content | State |
| --- | --- | --- |
| 0 | `terrazgo-siex` crate; the export moves out wholesale; `export_alias` → core; `crop_groups` → `module_cue::grouping` | **done 2026-08-20** |
| 0.5 | `premises` registry in core (see below), `non_field_treatment.premises_id`, its commands, `RegistryPremises.svelte` and the 3.4/3.5 picker | **done 2026-08-21**, with `cadastral_reference` + `class_code` added once `IdEdificacion` was settled |
| 1 | module-cue's blocks: `TratamientosPostCosecha`, `TratamientosEdifInstalaciones`, `UsoSemillaTratada`, `Analitica` | **done 2026-08-21** — five of fifteen blocks now emitted; `IdEdificacion` is filled from `premises_es_extension.rea_installation_code` |
| 2 | core's: `ComercializacionVD`, `SiembraPlantacion` + `MaterialAdquirido`/`FechaAdquisicion` | **done 2026-08-21** — seven of fifteen blocks emitted; the contradiction is settled below, and it cost one column fewer than either side of it proposed |
| 3 | fertilisation's: `Fertilizacion`, `Riego`, `PlanAbonado` + `Herramienta` | **done 2026-08-21** — ten of fifteen blocks emitted, plus `Fertirrigacion` and `GestionSostInsu`, which a 2026-08-08 note had recorded as deliberate gaps under a rule seam 3 had to correct (below) |
| 4 | eco-scheme's: `Pastoreo`, `LaboresCulturales`, `DatosCubierta`, and the DGC plot→crop rule | **done 2026-08-22** — thirteen of fifteen blocks emitted, which is the arc's whole scope; the plot→crop rule refuses rather than guesses, and Anexo V turned out to ask for it in as many words |
| 5 | `TratamFito`'s sub-blocks: `OtrasActuacionesFito`, `AsesorValidacion`, `FechaSeca` | **done 2026-08-22** — the purely non-chemical actuation exports, which is what the seam was for. It did **not** retire the widest refusal as planned: Anexo V makes the two sub-blocks *excluyente*, so the rule narrowed to the mixed record instead, and three new ones joined it (below) |
| 6 | Closure: docs, coverage, the project instructions | **done 2026-08-22** — the coverage map put `terrazgo-siex` on the standing command for the first time and surfaced two real gaps, below |

**Two fields get captured that no decree asks for**, because Anexo V marks them
`Obligatorio` *inside blocks we do send* — the standing line that already put
`Fertilizacion.BuenasPracticas` in `fertilisation_practice`:
`PlanAbonado.Herramienta` (whether a digital nutrient-advice tool produced the
plan), and `SiembraPlantacion.FechaAdquisicion`. The list said three until
2026-08-21; `MaterialAdquirido` left it on the evidence below.

**`BuenasPracticas` gained a rule on 2026-09-01, and the serializer needed no
change** — recorded because the parallel-move convention says a register change
is only settled once the twin's answer is written down. Code `0` is
"No realiza buenas prácticas", so the register now refuses to store it beside
another code; `blocks/fertilizacion.rs` goes on mapping whatever it is given to
`BuenaPracticaFertilizante { tipo_bpf }`. The contradiction was previously
exportable — two entries saying opposite things — and is now unreachable from
upstream, which is the better place to close it: Anexo V has no member that
could have expressed the exclusion, and refusing at export would have refused a
record the farmer could not then fix.

### Seam 5: the block no decree asks for, and the exclusivity that outlived the plan (2026-08-22)

Three sub-blocks of `TratamFito`, all fed by columns that already existed. The
seam shipped what it was for — **a purely non-chemical actuation is now a
first-class entry** — and reversed its own headline promise on the way, which is
the fourth time in this arc that re-reading the sources beat the plan written
from them.

**What the plan got right.** `TratamFito`'s required set is
`["IdAjenaTratamFito", "FechaInicio", "FechaFin", "DGCs", "ProblematicaFito",
"Justificaciones", "IdentificadorAplicador", "Eficacia"]` — `ProductosFito` is
**not** in it, so an entry naming no product is schema-valid, and that is what
makes the seam possible at all. `OtrasActuacionesFito` is an OBJECT, not an
array: one measure per actuation, which is why the register hangs its four
columns off the record.

| Member | Our column | State |
| --- | --- | --- |
| `TipoMedida` (required) | `treatment_record.measure_code` | emitted; `TIPO_MEDIDA_FITOSANITARIA` runs 1-12, 14, 15 — **there is no 13** |
| `Cantidad` / `Unidad` | the measure's intensity value + unit | emitted, and **demanded** — see below |
| `NumRegistroMDF` | `measure_registration_number` | emitted whenever stored; the MDF registry (*Medios de Defensa Fitosanitaria*, 1,235 rows) is **not vendored** — the code is stored verbatim, so nothing needs it to resolve |
| `BuenasPracticas` | captured by nothing | Voluntario, no printed column, and `BUENAS_PRACTICAS_AMBITOS` repeats each code once per ámbito — **composite identity**, so one integer cannot say which row was meant |

#### The refusal narrowed instead of retiring

The plan said building the block **retires** `records_with_non_chemical_measure`,
the precheck's widest rule. It does not. Anexo V grades all five members of
`OtrasActuacionesFito` *"excluyente con el subbloque siguiente de «Productos
fitosanitarios»"* — five separate fields, all saying the same thing. The
descriptor SHEET's *"se debe indicar al menos una «otra actuacion fitosanitaria»
o un producto"* is an OR that permits both, and it is the weaker document (see
"Which annex governs" below).

So a **mixed** record — a spray and a measure on one row, which the register
allows because model 3.1 bis prints "Alternativas no químicas" and "Alternativas
químicas" as two column groups of one row — has no single-entry shape. The rule
narrowed to `records_mixing_product_and_measure`, and the purely non-chemical
record walked free.

**The decree agrees with the exclusivity from the other side, and that is the
finding that settled it.** RD 1311/2012 art. 16.1 binds the record to *"la
información especificada en la Parte I del anexo III"*, and Parte I sección B
opens *"Para cada tratamiento que se realice en la explotación… especificar la
información siguiente"* followed by eleven lettered items, of which g names a
*producto fitosanitario* and i its kilos or litros. **There is no non-chemical
member anywhere in that list** — no medida, no intensidad, no registro MDF. The
whole block is the format's own; the register exists because model 3.1 bis
prints those columns for art. 10-11 GIP compliance, and art. 10 obliges the
*choice* of non-chemical methods, never an annotation of one.

The decree's unit being *cada tratamiento*, a row carrying both is a row
carrying two treatments. **Splitting one row into two entries was rejected**:
`export_alias` is minted once and never mutated because SIEX keys edits and
deletes on it, so one row would mint two aliases, and a later correction
dropping the measure would strand one asserting an activity that no longer
exists.

#### Three rules joined it, all from Anexo V gradings

- **`records_with_unsendable_measure`** — the intensity is nullable as a pair in
  the register (a farmer may record that traps were hung before counting them)
  and the book prints such a measure without complaint, while **Anexo V grades
  fields 17 and 18 Obligatorio**. The JSON Schema requires `TipoMedida` alone,
  so this is the grading deciding over `required`: the seam-4 cover-widths case
  again, and the same reason the two documents live in separate crates. The rule
  also catches a measure code that is not an integer and an intensity unit SIEX
  cannot express.
- **`records_missing_measure_registration`** — Anexo V field 19 grades `Registro
  MDF` Obligatorio for *"suelta de OCB, trampas y otros y feromonas y atrayentes
  para monitoreo"*: `TIPO_MEDIDA_FITOSANITARIA` 1, 14 and 15.
- **`records_missing_advisor_ropo`** — a record naming an advisor with no ROPO
  number. Anexo V grades field 50 Obligatorio **here** where blocks 1.2 and 1.3
  grade the same field Voluntario, which is exactly why the three non-field
  registers omit the block instead and this one refuses; `NumROPO` is the only
  carriable member, so omitting it would drop the identification Anexo III B.d
  asks for in the same sentence as the applicator's.

#### Which annex governs, and where the line was NOT drawn

**Neither Anexo V nor Anexo VI is law.** Both are annexes to the same FEGA
resolution (BOE-A-2023-13035): technical specifications, not decrees. Between
them **Anexo V governs**, on three counts — it is the *definición de variables*
(what each field means, and its `OBLIGATORIEDAD`) where VI is the web-service
*interface* descriptor; the standing precedence already ranks V at ② and does
not rank VI at all; and our copy of V is the **2025-11-20 corrección de
errores**, the authority fixing what the earlier documents said.

That still leaves one thing unenforced, deliberately. Anexo V's field 19 says
that outside its three kinds *"el campo debe ir vacío"* — and Anexo VI names a
**different** set (OCB, plantas banker, trampas cromotrópicas). No decree names
the field at all. So the *demand* is enforced and the *emptying* is not: a
stored MDF number always travels, whatever the measure kind, because honouring
either list would silently discard something the farmer recorded on the strength
of a rule the authority states two ways.

The same asymmetry decided `Unidad`. Anexo V's field 18 narrows the "unidades
válidas" per measure kind to Unidades, uds./m², uds./ha, m², m² malla/ha, kg and
kg/ha — a list omitting Trampas and Difusores, which `UNIDADES_MEDIDA` publishes
(27, 24, 25, 22) and which the JSON Schema accepts, asking only that the code be
in the catalogue. Answering "12 trampas" with code 11 (Unidades) would drop
*what was counted*, the one thing model 3.1 bis asks for by name — so
`intensity_unit_to_siex` sends the exact code and leaves the narrowing to the
receiver.

#### `FechaSeca` is emitted and never gated

A plain dd/mm/yyyy from `treatment_record.drying_date`. Anexo V field 4 grades it
**Obligatorio** *"cuando se trate de cultivos bajo agua"* — and the export does
not gate on it, though `sowing_record.flooded_on` would make the condition
checkable. The condition is not that the crop is flooded but that the field was
dried *for this treatment*: Anexo V's own wording is *"fecha en la que se realiza
el secado **para la realización del tratamiento**"*, which is also why the column
sits on the treatment rather than on the flooded crop's calendar. A rice
herbicide applied on water is a lawful record with no drying date to state, so a
gate keyed on `flooded_on` would refuse records the decree permits. A test pins
that a treatment on a flooded crop needs no drying date, so the gate cannot creep
in later.

(The Anexo VI sheet words the same obligation as *"Obligatorio para producto
arroz (80)"*. Anexo V's *"cultivos bajo agua"* is both wider and the one that
governs — another instance of the tie-break above.)

#### `AsesorValidacion`

Its descriptor struct was already written, emitted by nothing but the non-field
blocks; seam 5 wired it to `TratamFito`. Only `NumROPO` is sent. `Validacion`,
`Confirmacion`, `Contrato`, `Fecha` and `Observaciones` stay unfilled — model
3.1 bis collects the sign-off as a handwritten signature and the book has no
signature capability by design, so claiming any of them would invent the one
thing the block exists to attest. Anexo V grades the advisor's `Nombre`,
apellidos, `Razón social` and `NIF` Obligatorio here too, and the JSON Schema has
**no member for any of them** — a grading demanding what the format cannot carry,
with nothing to be done about it.

### Seam 6: what the coverage map found, and what the arc leaves behind (2026-08-22)

Closure, and it earned its place: the standing `cargo llvm-cov` command had
never listed `terrazgo-siex` — the crate was extracted at seam 0 and the command
was not updated, so the arc's own 2,107 lines were invisible to the map the
whole way through. Adding it is the durable fix; **a new library crate joins
that command in the same commit that creates it**, or its coverage silently
does not exist.

**The map, 2026-08-22** (`--summary-only`, line coverage, eight library crates):

| Crate | Lines | Before the arc | Now |
| --- | --- | --- | --- |
| `terrazgo-core` | 4,572 | — | **94.5** |
| `module-cue` | 5,440 | — | **94.1** |
| `module-sigpac` | 780 | — | **97.2** |
| core+cue+sigpac combined | 10,792 | 94.4 | **94.5** |
| `terrazgo-geo` | 896 | 88.5 | **88.5** |
| `terrazgo-recordbook` | 4,452 | 96.2 | **96.2** |
| `module-fertilisation` | 2,722 | 93.4 | **94.2** |
| `module-ecoscheme` | 1,835 | 93.0 | **93.6** |
| `terrazgo-siex` | 2,107 | *unmeasured* | **92.7** |
| all eight | 22,804 | — | **94.3** |

Fertilisation and eco-scheme rose without either crate being touched: the
serializer's tests drive their repositories, and llvm-cov attributes that to the
crate under test. Worth knowing before reading a movement as a change in the
module itself.

**Two artifacts, so the numbers are not misread.** `terrazgo-siex/src/error.rs`
and `terrazgo-recordbook/src/error.rs` both read **0.00%** — their `classify`
and `Display` paths are called only from the shell's command boundary, which is
deliberately outside the coverage set (Testing strategy, categories 4-5). That
is the crate selection showing through, not untested logic, and it is the same
class of arithmetic as the `demo` feature trap already recorded on the command.

**Two real gaps, both found by the map and both closed here.**

- **REGANIP-only machinery was never exercised.** The applicator test covered
  manual application, a machine in both registries (ROMA wins, "nunca ambos")
  and a machine in neither — but not the aircraft/fixed-installation case,
  which is the entire reason two columns exist rather than one. A fourth record
  now pins it.
- **`non_field_missing_operator_licence` was a precheck rule nothing ran.** Its
  test set every other field to `None` while leaving the licensed operator in
  place. Now the record names an operator holding no licence number.

Both are the kind of thing coverage is good at and review is bad at: a rule that
compiles, reads correctly, and has never once executed.

**What the arc leaves behind.** Thirteen blocks emitted, `Cosecha` and
`EnergiaUtilizada` deliberately unfilled, the descriptor at parity with every
register the app captures. What did **not** change: the export is still dormant,
still has no delivery path, and still has no UI. Un-parking means rebuilding the
panel *and* its scripted checks — and the fixture snapshot in the frontend
verifier still holds the pre-seam-5 precheck shape, harmless only because
nothing calls the command.

**The arc's method, five seams out of six.** On seams 2, 3, 4 and 5 the plan
written from the sources was corrected by going back to them — and on seam 5 the
correction came from the *decree*, not from FEGA's annexes, which is where it
should have been read first. The pattern is specific enough to name: **a note
written while reading one document is evidence about that document, never about
the duty.** The precedence at the top of this file exists because that mistake
kept being available.

### How seam 2's contradiction was settled (2026-08-21)

**The contradiction, as it stood.** The dormant-export inventory said
`MaterialAdquirido`, `FechaAdquisicion`, `MaterialTratado` and `NumLote` were
"captured by nothing on purpose", because they restate model 3.2's
`seed_treatment`. The scope note said two of them get captured, because Anexo V
marks them `Obligatorio` inside a block we send. Both were written on
2026-08-19/20 and they could not both be acted on. A third option was on the
table — *"merging them is a SERIALIZER's job, not the schema's"*, matching a
`sowing_record` to a `seed_treatment` on date and plot set.

**Re-reading the sources moved three of the four out of the argument, and each
finding is worth more than the choice it settled.**

- **`MaterialAdquirido` is already stored, in FEGA's own coding.**
  `TIPO_TRATAMIENTO` 4 and 5 are literally *"adquisición de semilla tratada con
  producto autorizado en España"* and *"…fuera de España"*, against 2 and 3 for
  seed treated on the holding or at a conditioning centre. Our
  `seed_treatment.treatment_kind_code` **is** that catalogue, so a boolean column
  would have been a second store of one fact — the defect `crop.sown_on` was
  dropped for on 2026-08-19.
- **`MaterialTratado` is a duty already discharged.** RD 1311/2012 Anexo III
  Parte I B.e (*"si la siembra se realiza con semilla tratada, indicar el
  producto"*) is exactly what `seed_treatment` records. The question the member
  asks is answered by whether such a record exists.
- **`NumLote` is `seed_lot`**, on the same table.
- **Only `FechaAdquisicion` had no store anywhere**, and its home is
  `seed_treatment`, not `sowing_record`: what was acquired is the *seed*, which
  is 3.2's subject, and `seed_lot` — the other purchase-traceability field —
  already sits there. So the capture is one column, `acquired_on`.

**What remained was the JOIN, and it is stated rather than guessed.** The
matching heuristic was declined: a farmer who typed different dates into the two
registers would have got a silent `MaterialTratado: false`, and no user action
could have revealed it. Instead `seed_treatment.sowing_record_id` is a nullable
FK the farmer sets on the 3.2 form.

**Its direction is forced twice over, which is what makes it not an arbitrary
choice.** A module may reference a core table and never the reverse, so the
column can only live on `seed_treatment`; and one sowing may use several seed
lots — the register is one row per product — each naming it, which a single
column on `sowing_record` would cap at one. **The descriptor sheet points the
same way**: `UsoSemillaTratada` carries an optional `IdAjenaSiembraPlant`,
*"Identificador de la actividad de siembra en origen"* — the seed activity names
the sowing. That member is **absent from the JSON Schema**, so nothing is
emitted for it (schema wins, the 2026-07-14 re-diff rule), but it settles that
the format models the two as separate entries with a pointer rather than as one
merged activity.

**Three collapse rules, because the block has one slot each for facts the 3.2
register holds per lot.** `MaterialTratado` is true when any live linked record
exists; `MaterialAdquirido` when any of them is an acquisition (one bought sack
makes the answer yes); `FechaAdquisicion` is the **earliest** purchase, which is
well defined because the precheck demands a date on every acquired record; and
`NumLote` is sent only when the linked records agree on one — naming one of two
lots would be a false statement about the other, and the member is optional, so
silence is available. Each lot still travels on its own `UsoSemillaTratada`
entry.

**The link is a statement about a LIVE register, in both directions.** It cannot
be made to a soft-deleted sowing, and a soft-deleted 3.2 record stops asserting
that the sowing used treated material. The guard also refuses a sowing on
another farm or in another campaign — the FK alone would allow both, and the
export reads this link to state `MaterialTratado` on that sowing, so a
cross-farm link would put one farmer's treated seed in another's descriptor.

### Seam 3: two recorded gaps that should not have been gaps (2026-08-21)

Seam 3's three blocks were nearly all wiring — `module-fertilisation` was
designed against them, down to `irrigation_water_origin` being a junction
because `OrigenAgua` is an array. What it had to settle was a **contradiction
between two rules this project wrote twelve days apart**, and the correction
matters more than the two fields it moved.

**The older rule** (`cuaderno-print.md`, 2026-08-08): *"required in the twin →
capture it; optional in the twin AND absent from the model → record it as a
gap."* Under it, `Fertirrigacion`, `GestionSostInsu` and `BuenasPracticasRiego`
were all recorded as deliberate gaps.

**The newer rule** (this document, 2026-08-20): *"a field FEGA marks
`Obligatorio` inside a block we do send is a real requirement even when no
decree names it"* — the line that put `PlanAbonado.Herramienta` and
`SiembraPlantacion.FechaAdquisicion` in the schema.

**The newer one supersedes, and the reason is the "required, obligatorio and
binding" section above.** The older rule tests the JSON Schema's `required`,
which is *only structural validity of an entry you chose to send*. Anexo V's
`OBLIGATORIEDAD` column is a different question — FEGA's own per-field duty flag
— and it is the one that decides capture. Re-graded against it:

- **`GestionSostInsu` is Obligatorio** (Anexo V block 9 field 5: *"indicar si
  realizan o no una gestión sostenible de insumos conforme a las disposiciones
  normativas vigentes en materia de nutrición sostenible de los suelos
  agrarios"*). Captured, as `fertilisation_record.sustainable_input_management`.
- **`BuenasPracticasRiego` is Voluntario**, on this block and on `Riego`, and no
  page prints it. It stays a recorded gap — now for a reason that survives the
  correction rather than one that happened to agree with it.

**`Fertirrigacion` needed no rule at all, only a second look.** The 2026-08-08
note reasoned that the model has no fertigation columns, that
`application_method_code` already records *that* it was a fertigation, and that
the water side is §8's. All true, and all beside the point: the sub-block is the
**only reader anywhere in the format** of Anexo III **C.l**'s two water-quality
figures. `irrigation_record.water_nitric_n_mg_l` and `water_soluble_p2o5_mg_l`
appear in no printed column, in no member of `Riego`, and in no other block —
this document had already said so in 2026-08-08's own sección C notes. Not
building it left two columns of a **binding** letter captured for nobody.

**So it is built, from a stated link.** `fertilisation_record.irrigation_record_id`
is nullable, set by the farmer on the §6 form, and **refused unless
`application_method.is_fertigation`** — on any other method it would assert a
fertigation that did not happen. The flag is read from the lookup rather than
matched on the code, which is exactly what that column's own comment anticipated
("what a future `Fertirrigacion` block would key on"). The link is validated
against the same farm AND campaign and refused for a withdrawn watering, the
`seed_treatment.sowing_record_id` rules one seam earlier.

**The precheck demands the link for a fertigation, and it asks for nothing
new**: art. 5.e already obliges the irrigation record for that watering, so the
list is genuinely fixable. It demands the two water figures separately, because
art. 17.2 makes them conditional and the register therefore leaves them nullable
— a fertigation whose watering states neither cannot fill a block that requires
both.

**One asymmetry worth naming**: the same act travels twice in the file, as a
`Riego` entry and inside `Fertilizacion.Fertirrigacion`. That is the format's
shape, not a duplication bug — the two blocks answer different questions, and
the decree splits the act the same way across arts. 5.d and 5.e.

### `SiembraPlantacion` the member is not the crop (corrected 2026-08-21)

The inventory row below said the required `SiembraPlantacion: integer` "needs no
column — Anexo V defines it as *Cultivo sembrado/plantado, según catálogo
SIEX*", so it is the crop, derived. **That was wrong, and the WS descriptor says
so in one line**: `SiembraPlantacion … number(1) … "1 Siembra 0 Plantación"`.
Anexo V's "Cultivo" (field 3) is `DGCs[].CodigoCultivo`, which sits per-DGC and
is where this serializer puts it. The member is a sowing/planting flag.

**And that made it a real capture question rather than a formality**, answered
by the app's own interface rather than by the format: **`sowing_record`'s form is
titled "Siembra y plantación"** and asks the farmer to *"anote cómo empezó cada
cultivo"*, so recording an orchard planting in it is its documented use. A
constant `1` would state something false about every one of them. `sowing_kind`
is therefore a two-value core lookup and `sowing_record.kind_code` is `NOT NULL`.

**No decree asks for a planting annotation**, and that stays true — the whole
duty table has exactly one clause naming this kind of act (RD 1048/2022 art.
45.2's *"las fechas de … siembra …"* for cultivos bajo agua, which is rice, an
annual sown crop) plus art. 42.1.a's cover *establishment* date, which has its
own register and its own block. So the column is not "a register derived from
the exchange format": the register already existed and already invited both
answers, and this makes the one it was collecting implicitly answerable.

**Rules that carry across the remaining seams.** The builder keeps its
all-or-nothing precheck: an export that silently drops a register is worse than
one that refuses with a fixable list. `AltaDGC`, `CambioCultivoDGC` and the WS
client stay out of scope (the CUECYL answer). And three blocks — `Pastoreo`,
`LaboresCulturales`, `DatosCubierta` — reference DGCs while their junctions
carry **no `crop_id`**, because no printed page asks for one; that plot→crop
resolution is seam 4's real work and needs a rule written test-first, refusing
rather than guessing where a plot carries several crops.

### Seam 4: the crop the authority calls a calculated field (2026-08-22)

Three blocks, and the only one of them that was not wiring is the rule the arc
had been carrying since it opened: **`grazing_plot`, `cultural_operation_plot`
and `soil_cover_plot` carry a plot and no crop, while a SIEX DGC is a plot+crop
unit.** They carry no crop because no printed page of section 9 asks for one —
model 9.1 wants the SIGPAC reference, 9.2 the plot, 9.4 the cover.

**Anexo V asks for the computation in as many words.** Field 3 of both
`Pastoreo` and `LaboresCulturales` is *"Cultivo/s … Campo calculado"* — the
authority's own description of the field, and the corroboration that resolving
the crop from the plot is the intended reading rather than a workaround.

So `crop_on_plot(conn, plot_id, season_id)` folds the live crops of that plot in
that season into three cases, and **the third is the point of writing it
test-first**:

| Case | What the export does |
| --- | --- |
| exactly one live crop | that crop's `CodigoDGCAjena` and `CodigoCultivo` |
| none | precheck: `ecoscheme_plots_missing_crop` |
| **two or more** | precheck: `ecoscheme_plots_with_ambiguous_crop` — **never resolved** |

A plot carrying two crops **is** two DGCs and the record names neither, so
choosing one would assert the activity happened on a crop the farmer never
stated. The test that pins it was verified by breaking the rule: making it take
the first crop turns the suite red.

`Superficie` is always omitted on both blocks. Neither junction has a surface
column, and the descriptor reads an absent `Superficie` as the DGC's own (*"es
igual a la superficie DGC salvo que se indique lo contrario"*, stated on
`LaboresCulturales` and on `SiembraPlantacion`, not on `Pastoreo` — that last is
a **reading**, not a quotation, and is recorded as such). Sending the crop's
`area_ha` instead would assert that every hectare of it was grazed or worked.

#### Two refusals, and the second corrects the reading this seam opened with

Both were settled against the decrees and Anexo V rather than by preference, and
between them they decide what the precheck contains.

- **An open grazing is refused, not skipped.** RD 1048/2022 art. 30.2 ter gives
  the farmer a month *"desde la nueva fecha de inicio o fin que haya resultado de
  la modificación"*, so a grazing still under way is not late — it is
  unfinished — while `Pastoreo.FechaFin` is required. The 2026-08-19 note said a
  serializer must SKIP such records. It is overruled: `grazings_without_end`
  names them and the export refuses. The precedent is `records_missing_efficacy`,
  which is the same shape — a field that is nullable because it is *not yet
  knowable*, schema-required, and made a blocker rather than a silent omission.
  Cost, accepted: a holding with animals out cannot export until it closes the
  record.
- **A cover with no widths is refused too, and this is where the two documents
  part company.** Art. 42.1.e falls due *"en el mes anterior al final del periodo
  mínimo de cuatro meses"* while 42.1.a is due within a month of establishment,
  so a cover between the two deadlines is a complete record — which is exactly
  what `widths_stated_on` exists to make visible, and the record book prints it
  without complaint. **Anexo V grades both widths `Obligatorio`** for exactly the
  three cover types this register can hold (its own wording: *"Solo para los
  casos de cubierta vegetal sembrada, cubierta vegetal espontánea y cubierta
  inerte de restos de poda"* — `PLANT_COVER_TYPES` and `INERT_COVER_TYPES`), and
  that grading decides.

  **The first reading of this went the other way, and it was wrong for a reason
  this document already names.** It argued that every existing precheck rule keys
  off a field the JSON Schema *requires*, and that 3.11.4 deliberately made these
  two optional. That is testing `required` — *structural validity of an entry you
  chose to send* — which is the exact mistake the "The law outranks the
  format" section was written to stop, and which seam 3 had already had to
  correct once. `OBLIGATORIEDAD` is the column that decides. So the precheck now
  has its first rule on a schema-optional field, and it is right that it does.

#### Where the two halves of the rule live

The rule splits, and the split is the placement rule applied one level down:
**the query belongs to the crate that owns the data, the refusal belongs to the
document that refuses.**

- `terrazgo_core::repository::crops_on_plot(conn, plot_id, season_id)` returns
  **every** live crop and says nothing about choosing. That is deliberate: a
  future reader of the same question might reasonably split a two-crop plot
  instead of refusing it, and core has no business deciding.
- `terrazgo_siex::blocks::crop_on_plot` folds that list into `PlotCrop` and is
  where "two crops means refuse" lives, because it is *this document's* rule —
  the record book asks the question differently and gates on nothing.

Two cross-crate reads went through the same tidy-up while the rule was written,
and both bought more than tidiness:

- **`cover_type_of` now calls `module_ecoscheme::repository::get_soil_cover_for_export`**
  rather than selecting `cover_type_code` itself — the
  `get_fertiliser_material_for_export` precedent, and for the same two reasons.
  The ordinary getter filters soft-deleted rows while a deletion entry must
  still resolve the cover it named; and **`validated_cover_link` checks the farm
  but NOT the season**, so a caller cannot safely resolve the cover from a
  season's own list either. That second fact is what rules out caching the
  covers per export, which would otherwise have been the faster shape.
- **`dgc_identity` lost a query.** Reading the crop through core's typed `Crop`
  gives it `crop_code` for free, so the separate `SELECT crop_code FROM crop`
  it used to issue per DGC is gone.
- **The four remaining `SELECT crop_code FROM crop` reads went the same way**
  (`dgc_superficie` and the `siembra_plantacion`, `analitica` and `plan_abonado`
  blocks, all from seams 1–3), behind core's `find_crop_for_export` and one
  `crop_code_of` helper. **The point is not the duplication — it is that nothing
  said those reads deliberately include WITHDRAWN crops.** Crop deletion is
  always allowed, so a record written years ago routinely names a crop that is no
  longer live and the descriptor must still state its PRODUCTOS code; the intent
  was expressed only by the absence of an `AND deleted_at IS NULL`, which the
  next reader to tidy one of the four would not have seen. Now it has a name, a
  comment and a test. `find_` rather than `get_`, the `find_export_alias`
  convention, so a missing row stays `None` and no behaviour changed — every
  `crop_id` that reaches it carries a real foreign key anyway.

**The rest of the crate's raw reads went the same way on the same day**, once
counting them properly turned up six rather than the three first reported:

| Was | Now | What it bought beyond tidiness |
| --- | --- | --- |
| `SELECT is_fertigation FROM application_method` per record | `list_application_methods` once, into a set | the module's own lookup API, and N queries fewer |
| `SELECT … FROM irrigation_record WHERE … deleted_at IS NULL` per fertigation | the season's live waterings, listed once | **safe only because `validate_fertigation_link` checks farm AND campaign**, so the list is the whole population a link can name. The cover link is *not* season-validated, which is why `cover_type_of` cannot work this way — the two look alike and are not |
| `SELECT roma_number FROM machinery_es_extension` | core's `find_machinery_es` (its private `get_extension`, promoted) | a regulatory value, read live and unsnapshotted, now named |
| `SELECT kind_code, exceptional_substance_code FROM product_authorisation`, **twice** | `module_cue::repository::find_product_authorisation` behind one `authorisation_product_kind` helper | the two blocks had copied the query, the fallback, the code mapping *and* the substance parse — four duplicated decisions, now one |
| `SELECT name FROM plot`, **four times** | one local `plot_name` helper | the duplication only. **Deliberately not a core accessor**: it resolves a display label, core has no by-id plot getter, and inventing one for a single consumer is what the module seam's rule says to resist |

Two things that pass came out of it and are worth keeping. The withdrawn-watering
rule — *"a withdrawn watering counts as absent"* — **had been asserted by a
comment since seam 3 with no test behind it**; refactoring the path is what
exposed that, and `a_withdrawn_watering_leaves_its_fertigation_blocked` now pins
it. And every one of those files then reported an unused `OptionalExtension`
import, which is the cheapest possible confirmation that the raw queries were the
only thing needing it.

#### What else the sheet and Anexo V settled

- **`DGCs[].Cubiertas[]` exists on `Pastoreo` and `LaboresCulturales`** (Anexo V's
  "Actividad en la cubierta" subloque, `Obligatorio`), and `soil_cover_id` is
  exactly when it applies. It is resolved once per entry, which is what makes the
  sheet's rule — *"No se pueden indicar en la misma actividad DGCs con cubierta y
  sin cubierta"* — hold by construction rather than by a check.
- **The descriptor forbids `AnimalesPropios` and `AnimalesTerceros` both being
  false.** Nothing in the serializer enforces that: `insert_grazing_record`
  already refuses `no_animals`, and the precheck demands
  `farm_es_extension.rega_code` once a season holds a grazing — without it every
  line reads as a third party's, which is a *claim*, not an absence. Demanded by
  the register rather than by the farm, so a holding with no animals still owes
  nobody a REGA.
- **`species_code` is unvalidated at insert on purpose** (a provider registry
  stored verbatim) while `Especie` is a required integer, so
  `grazings_with_unsendable_species` is where that meets the format. Same for a
  cover type that is not an integer, which joins `covers_missing_fields`.
- **`ActividadCubierta[]` stays emitted by nothing.** The sheet declares it
  `1..n` and the JSON Schema has no such member — a live disagreement inside
  3.11.4, and the schema wins, for the third time after `MateriaActivaFormulado`
  and `HorasUtilizacion`.

### The premises registry, and a claim that did not survive checking

`TratamientosEdifInstalaciones` requires `Edificaciones[].IdEdificacion` as a
`number(10)`, and models 3.4/3.5 stored only free text. The first framing —
"a premises registry is driven by the exchange format" — **was wrong**, and
checking the sources produced a better reason:

- **RD 1311/2012 Anexo III Parte I B.b creates the identification duty**:
  "la parcela, o en su caso, local o medio de transporte tratado", with B.f
  adding the volume in m³. Identifying the local is a decree requirement.
- The model structures it further — 3.4 "local tratado (**tipo y dirección**)",
  3.5 "vehículo tratado (**tipo, modelo y matrícula**)".
- But a premises **registry** is genuinely not mandated for our user: RD
  1311/2012 art. 42–43's establishment registry is ROPO's, covering supply,
  treatments *for third parties*, advice and professional users "en tanto sea
  con carácter comercial, industrial o corporativo". A farmer treating their
  own store is outside it, and art. 16 requires only the treatment register.

So the justification is: **the decree requires the local to be identified, and
a description retyped per treatment identifies nothing.** Design and the
snapshot behaviour: `docs/data-model.md` → `premises`.

**And that is the whole justification, because the format-side half collapsed
on inspection (2026-08-20).** The first framing also claimed the registry would
give `Edificaciones[].IdEdificacion` a stable integer to alias. It does nothing
of the sort — see the next section, which settles what the field is.

*Confidence note*: the BOE consolidated page does not carry Anexo III, so B.b
and B.f rest on our own live transcription of 2026-08-09; the art. 42–43 scope
is a summarised read rather than a verbatim quote. Worth re-reading directly
before anything else leans on it.

### `IdEdificacion` is REA's key, and the farmer's answer is the referencia catastral (settled 2026-08-21)

Three readings were recorded on 2026-08-20 — a minted client alias, an
`EDIFICACIONES_INSTALACIONES` type code, the referencia catastral — and
**none of them was right**, though the third is right about the *datum*. The
answer came from reading the REA structure beside the CUE one, which is where
this block's own subloque points:

| Artifact | What it says |
| --- | --- |
| `descriptor-EstructuraRegistro.tsv` (**REA**) | `instalacionesEdificaciones` carries `claseInstalacion` (the catalogue), `referenciaCatastral string(20)`, and **`identificador`, `number(10)`, "Código del edificio/instalación en el REA"** |
| `descriptor-EstructuraCuadernoWS.tsv` (**what our JSON Schema mirrors**) | `Edificaciones { IdEdificacion* number(10), Volumen, Unidad }` — one required field, the same shape and the same wording as REA's `identificador` |
| `descriptor-EstructuraCuaderno.tsv` (the non-WS structure) | carries **both**: `referenciaCatastral string(20)` as Anexo V field 1, and `identificador number(10)` "Código del edificio/instalación en el REA" |
| Anexo V, CUE sheet, block **1.3** | subloque literally named **"Instalación identificada en el REA"**, whose only identifying field is nº 1, *"Referencia catastral de la edificación/instalación o de la parcela en que se ubica"*, **Obligatorio** |

So `IdEdificacion` is **the authority's own key for an installation already
registered in REA** — the `CodigoDGC` situation, not the `CodigoDGCAjena` one,
and there is no `IdAjenaEdificacion` anywhere to mint. The type code reading was
`claseInstalacion`, a different REA field; the cadastral reading named the right
datum but the wrong member.

**What this decided for seam 0.5, and what it leaves for seam 1.** `premises`
gained two columns on 2026-08-21, both **buildings-only** because a lorry has a
matrícula and FEGA's catalogue of edificaciones holds no vehicles:

- **`cadastral_reference`** — the identification Anexo V marks `Obligatorio`
  inside a block we do send, which is the standing line that already put
  `Fertilizacion.BuenasPracticas` and `PlanAbonado.Herramienta` in the schema.
  It is also the only one of the three a farmer can actually supply.
- **`class_code`** — Anexo V's REA bloque 8 field 1 is obligatory *"en caso de
  actuación, tratamiento o práctica en las edificaciones e instalaciones que
  conlleve su identificación para la cumplimentación del CUE"*, which is models
  3.4/3.5's own case. This is what gave `EDIFICACIONES_INSTALACIONES` a real
  consumer, after its recorded one ("IdEdificacion typing") turned out to be a
  guess.

**How `IdEdificacion` is filled — settled in seam 1 (2026-08-21).** The app
cannot *derive* a REA code, and minting one was never an option: the block's own
subloque says the installation is identified **in REA**, so a client-assigned
number would name a different building. It is therefore **user-entered from the
farmer's own REA papers**, exactly like `farm.owner_tax_id` and
`farm_es_extension.rea_code` — the REA-first rule this document has carried
since 2026-07-11, applied one level down. It lives in
`premises_es_extension.rea_installation_code`, and the export **precheck demands
it**: a record naming no premises, or one whose extension has no code, or a code
that is not an integer, all block the export with a fixable list rather than
being sent as a guess. If it turns out farmers cannot read those codes at all,
nothing was invented and the blocker names itself.

**One more finding for seam 1, from the same reading: the WS structure has no
transport block at all.** Searching the descriptor for *transporte* returns
nothing, so model 3.5's register — `non_field_treatment` with subject
`transport` — has **no twin**, exactly like `plot_water_point`. Either vehicles
fold into `TratamientosEdifInstalaciones` as instalaciones, which their absence
from the class catalogue argues against, or the format simply does not carry
them.

### What seam 1 learned from the schema rather than from the sheet (2026-08-21)

Four things the descriptor summary does not show, each of which would have been
a wrong guess:

- **`EquipoAplicador` carries the same `oneOf` in every block.** The non-field
  blocks drop `TratamFito`'s required `AplicacionManual`, which reads like "no
  equipment identifier is required here" — and is wrong. Exactly one of
  `NumROMA` / `NumREGANIP` / `IdEquipoAplicador` is still demanded, so a hand
  application names the same `"manual"` sentinel `TratamFito` uses. The vendored
  schema caught this on the first test run, which is what schema-validated tests
  are for.
- **The two non-field blocks have NARROWER problem vocabularies.** Neither
  carries `MalasHierbas`, and the buildings block has no `ReguladoresOtros`
  either. Our registers accept every reason category, so a weed treatment in a
  warehouse is capture the format cannot express — refused by the precheck with
  a nameable row, never exported with the reason silently missing (the
  refuse-rather-than-drop rule, second instance).
- **Units differ between the printed model and the format, in one direction
  that matters.** `TratamientosPostCosecha.Cantidad` is *"peso en kg del
  producto vegetal tratado"* with no unit member, while model 3.3 prints tonnes
  — which is what the register stores. Sending the stored number would state
  120 kg where the farmer treated 120 t.
- **`UsoSemillaTratada.Producto` is the CROP.** Anexo V's field 1 reads
  "Cultivo — código del cultivo del catálogo SIEX", so the member's name is
  misleading and it takes `seed_treatment.crop_code`. Its `ProductosFito` child
  stays unfilled: the register captures no amount of product because model 3.2
  prints no such column, and the child is optional in the schema — the
  required/obligatorio/binding distinction, applied.


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
   2026-07-16, and the UI half was REMOVED 2026-08-11** —
   `export_cuaderno_precheck` + `export_cuaderno` commands remain (the latter
   async, backup-command pattern: build → write to the dialog-chosen path,
   returns path/size/entry count; the suggested filename sanitizes the season
   label, "2025/2026" carrying a path separator). The record-book view's
   "Exportación oficial (SIEX)" section is gone: a button producing a file with
   nowhere to go was the wrong thing to show a farmer. See "The precheck now has
   no renderer".
5. **Widen the serializer to every block with a register behind it. Done
   2026-08-20 → 08-22** — the "finish the serializer" arc, thirteen of fifteen
   blocks. This step was not in the original plan; it exists because the park
   was read as "frozen" and capture outran the descriptor for five months.
6. Server-side WS client — separate component, after developer authorization
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
