# SIEX export

How the app's registers map onto the official CUE exchange format (the SIEX
descriptor), what the serializer in `terrazgo-siex` sends, and why.

> **Status: dormant. The export has no delivery path, but the serializer is
> complete.**
>
> CUECYL answered the onboarding questions (see the end of this file):
> connecting an application to the REACYL or CUECYL web services requires being
> a **company or self-employed professional (autónomo)**, and **CUECYL has no
> way for a farmer to upload a descriptor JSON file**. So the printable PDF
> cuaderno is the app's compliance document, and `AltaDGC`/`CambioCultivoDGC`
> (gap 2) and the web-service client are out of scope.
>
> `terrazgo-siex` stays compiled, schema-validated and tested, and emits every
> block that has a register behind it. It has no UI. **Dormant means no UI and
> no transport, never a serializer that falls behind**: a register added or
> changed must decide, in the same change, what the export does with it, or this
> file must say why it sends nothing.
>
> Bringing the export back means rebuilding its UI panel and its scripted
> checks, and first having one submitting device per farm
> (docs/sync.md → "`export_alias` collisions").
>
> The **national FEGA** services (`/catalogos/*`, `POST /existeNIF`) are not
> affected: they are public and need no authentication. The barrier is specific
> to the regional CUE/REA web services.

## Sources of truth

| What | Where | Version |
| --- | --- | --- |
| Interface spec (methods, auth, envelope) | [FEGA Anexo VI "Interfaz Único Común"](https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex) | **v3.11.4 (Nov 2025)** |
| CUE JSON Schema | Embedded in the Anexo VI docx (OLE object, file `…CUE_3.11.4.json`); vendored copy: [`references/cue-schema-3.11.4.json`](references/cue-schema-3.11.4.json) | 3.11.4 |
| Field meanings / mandatory flags | `BdcSix-DS-DiseñoCUE.xlsx` (embedded in the same docx, sheet `EstructuraCuadernoWS`) + FEGA Anexo V | 3.11.4. **Where the sheet and the JSON Schema disagree, the schema wins** (it is what validates). Known differences: the sheet still shows `MateriaActivaFormulado{}`, `HorasUtilizacion` and `DatosCubierta.ActividadCubierta[]`, none of which is in the schema |
| Code catalogues (crops, units, problems, substances…) | FEGA Anexo VII, public REST API `https://www11.fega.es/bdcsixwsp/` (no auth; guide "BdcSixWsp"); vendored in `crates/terrazgo-core/catalogues/` | see "Storage design" and `maintenance.md` §1 |
| **The documentation from September 2027** | [FEGA "Documentación Técnica Horizontal SIEX"](https://www.fega.gob.es/es/siex/documentacion-tecnica-horizontal-siex): with SIEX 5.9.0 it replaces the agricultural and livestock documentation above. Seven anexos: 01 glossary, 02 information sources, **03 data blocks** (Anexo V's successor), 04 catalogues, 05 interfaces (IUWS), 06 authorisation model, 07 REA management | Anexo 03 is still preliminary; see "The September 2027 model" |
| REA ↔ CUE relationship, CyL onboarding | [CUECYL](https://agriculturaganaderia.jcyl.es/web/es/cuaderno-digital-explotacion-agricola.html) + [REACYL](https://agriculturaganaderia.jcyl.es/web/es/registro-explotaciones-agrarias-castilla.html) pages; [RD 1054/2022](https://www.boe.es/buscar/doc.php?id=BOE-A-2022-23054) | |
| What a farmer can download | [Instrucciones declaración DGC (Junta de CyL, PDF)](https://agriculturaganaderia.jcyl.es/web/jcyl/binarios/169/699/Instrucciones%20declaraci%C3%B3nDGC_ene-2025.pdf); [SIGPAC download service](https://sigpac-hubcloud.es/html/sdsigpac/descServicio.html) + [cultivos-declarados model](https://sigpac-hubcloud.es/html/sdsigpac/modelos/cultivos-declarados-SIGPAC.html) | |
| Other public SIEX services (MDF, `existeNIF`) | The same BdcSixWsp API; see "Other public SIEX services" | |

Transport: REST + JSON; authentication is a qualified legal-person certificate
plus a JWT; `POST /IUWS/crear/` is asynchronous (a request number, then polling
`comprobarEstado`). **A standalone desktop app is expected to produce the JSON
file**; the web-service client would be a separate server-side component,
outside the offline core.

## Target format

```
Root
└─ CUADERNO[]                          ← one entry per farm (explotación)
   ├─ CAExplotacion*  IdTitular*  CodigoRea*  UnidadGestora*   (+ CodigoSIEX, IdCuaderno)
   ├─ DatosExplotacion
   │  ├─ AltaDGC[]            ← register plot+crop units the REA doesn't know
   │  └─ CambioCultivoDGC[]
   └─ ActividadesExplotacion          ← 13 of the 15 blocks are emitted
      ├─ TratamFito[]         (+ OtrasActuacionesFito, AsesorValidacion, FechaSeca)
      ├─ UsoSemillaTratada[]  TratamientosPostCosecha[]  TratamientosEdifInstalaciones[]
      ├─ Analitica[]  ComercializacionVD[]  SiembraPlantacion[]
      ├─ Fertilizacion[]  Riego[]  PlanAbonado[]
      ├─ Pastoreo[]  LaboresCulturales[]  DatosCubierta[]
      └─ Cosecha[]  EnergiaUtilizada[]   ← NOT emitted: no register behind
                                            either, by decision (see "The two
                                            blocks nothing will fill")
```

A **DGC** ("dato geográfico de cultivo") is SIEX's unit of plot + crop + period.
Activities don't point at plots directly: they point at DGCs, either by the REA's
own `CodigoDGC` (obtained by importing the REA) or by a `CodigoDGCAjena` the
client assigns and creates through `AltaDGC`.

## How farm data reaches the cuaderno — the REA-first rule

Under RD 1054/2022 the regional farm registry (the **REA**) is the source of
truth the cuaderno reads from, not the other way round.

### One decree, one system per community

RD 1054/2022 creates three things at once: the national **SIEX**, a **registro
autonómico de explotaciones** and a **cuaderno digital**. Each autonomous
community runs its own version of the last two under its own name, so the names
below are branding for the same concepts, and the app models the concepts:

| Community | Regional registry (REA) | Digital record book |
| --- | --- | --- |
| Castilla y León | REACYL | CUECYL |
| Catalunya | SIDEAC (fed by the DUN declaration) | QIE — *Quadern integrat d'explotacions* |
| … | one each | one each |

Sources: [CUECYL](https://agriculturaganaderia.jcyl.es/web/es/cuaderno-digital-explotacion-agricola.html)
/ [REACYL](https://agriculturaganaderia.jcyl.es/web/es/registro-explotaciones-agrarias-castilla.html);
the DUN 2024 SIDEAC sheet says it plainly: *"El SIDEAC … constitueix el
Registre d'explotacions agràries de Catalunya d'acord amb … el RD 1054/2022 …
pel qual s'estableix el SIEX i el Registre autonòmic d'explotacions agrícoles i
quadern digital"*.

**For the app**: `farm_es_extension.rea_code` is ONE column for every community
(the concept is national, only the name of the system differs), and no text the
user sees may name one community's service. Labels say "registro autonómico";
the export hint says "el cuaderno digital de su comunidad autónoma". Naming the
local system would need a province → service map (like
`terrazgo_recordbook::region`'s province → language map), kept current for
seventeen communities, and nothing needs it.

The rules below are from the Junta de Castilla y León's CUECYL and REACYL
pages. They follow from the decree, so they hold whatever a community calls its
own systems:

- **The farm must be registered in the REA first.** The regional CUE is created
  automatically from that registration ("Primero debe inscribir su explotación
  en el Reacyl y posteriormente se le generará automáticamente el Cuecyl");
  `CodigoRea` and `IdTitular` (gap 4) only exist because of it. Nothing creates
  a farm from the cuaderno side.
- **Each campaign's surfaces and crops flow from the REA to the CUE.** "Desde el
  Reacyl se vuelcan al Cuecyl las superficies y cultivos de la campaña"; the
  cuaderno can't be filled for a campaign whose DGCs are out of date in the REA.
  The REA itself is fed by the holder's declarations (REACYL's "Declaración de
  DGC" module), the PAC solicitud única and the sector registries (Registro
  Vitícola, ROMA, REGA).
- **Plots and crops can also go the other way.** CyL says surfaces and crops
  "se podrán importar … desde un CUE comercial", which is what
  `AltaDGC`/`CambioCultivoDGC` (+ `CodigoDGCAjena`) are for. So plots entered in
  the app can still be exported; only the farm itself has to exist in the REA
  first.
- **Rules in CyL:** a DGC can only be filled by ONE commercial notebook; and when
  a farm is selected for an official control, everything in the commercial
  notebook must already have been transferred (volcado) to the Cuecyl, or the
  control falls back to the paper cuaderno.
- **There is no general download of the REA for a farmer** (REACYL's
  "Consultar mi explotación" is a web page behind a certificate/DNIe login), so
  the only *machine* path into a commercial notebook is the Interfaz Único
  (`exportarREA`), i.e. a server-side component, never the offline core. The DGC
  declaration module does give the holder two manual outputs (see below).

### Farmer-side data paths — no server involved

Three ways DGC-shaped data can reach the app with only the farmer's own work:

1. **REACYL DGC Excel export (the farmer's own official DGCs).** The DGC
   declaration module's sign-and-register screen lets the holder "firmar y
   registrar, así como **exportar las DGC a una hoja Excel**" ([Instrucciones
   declaración DGC](https://agriculturaganaderia.jcyl.es/web/jcyl/binarios/169/699/Instrucciones%20declaraci%C3%B3nDGC_ene-2025.pdf)).
   The farmer logs into `particulares.ayg.jcyl.es` (certificate/DNIe), exports,
   and imports the file into the app. Unknown until a real export is seen: the
   exact columns (does it have `CodigoDGC`? that would ease gap 2), how crops are
   coded, and whether the export is available outside an active declaration. It
   is specific to CyL; other regions' REA apps would each need their own
   adapter. Reading `.xlsx` in Rust needs a new crate (calamine is the pure-Rust
   candidate), which would have to be decided first.
2. **Public "cultivos declarados" downloads (geometry + declared crop, no
   login).** The SIGPAC download service publishes the graphical declaration
   lines as **provincial GeoPackages**, current and previous campaign, CC BY 4.0,
   in the same channel, format and SRS (ETRS89/REGCAN95) as the recinto files the
   importer already reads. The model has the full SIGPAC reference, `EXP_ANO`,
   `PARC_PRODUCTO` (declared crop code), `PARC_SISTEXP` (secano/regadío),
   `PARC_SUPCULT` and the line geometry: enough to *prefill* a season's crops by
   matching the SIGPAC references on plots. It is published declaration data,
   not the REA record: no `CodigoDGC`, and only PAC-declared surfaces. The app's
   declared-crops prefill uses the per-reference OGC API version of this data
   (`sigpac-integration.md` → "Declared crops"), and the map shows the MVT
   version as the `cultivo_declarado` overlay.
3. **Signed DGC document (PDF).** After registering, the holder can "obtener el
   documento firmado que recoge el listado de las DGC": readable by people, not
   an import format.

None of them replaces `exportarREA` for real REA sync (codes, holder data, new
campaigns), which stays server-side. In the export, plots and crops are mapped to
DGCs when exporting: importing the REA's `CodigoDGC` would be a server-side
feature, while `AltaDGC` + `CodigoDGCAjena` works standalone (gap 2 and open
question 5). Farm identifiers (`CodigoRea`, the holder's NIF) are entered by the
user from their REA registration (gap 4) and can't be worked out from anything
else.

## The law outranks the format: required, obligatorio and binding

**The order to use whenever there is a question about what to record. The
decrees come first; the exchange format is only transport.** Mixing the levels
up produces both false gaps and false duties, usually by checking the lowest of
them.

Highest authority first:

1. **The decrees** create the duty to keep the record at all, and decide what a
   register IS. Where a decree requires an entry that no schema carries, **we
   carry it**: RD 1048/2022 anexo IV is the proof, and the record book prints a
   "9.6" that the official model has no page for.
2. **Anexo V's `OBLIGATORIEDAD` column** is FEGA's own duty flag per field. It
   decides what to record *inside a block we have chosen to send* (a real
   requirement even when no decree names it), and it grades whole blocks as
   voluntary: `EnergiaUtilizada` **6/6 Voluntario**, `ComercializacionVD`
   **5/5**, `Analitica` **8/8** ("en caso de haberse realizado"), the REA
   editable block **18/18**.
3. **JSON Schema `required`** only means *structural validity of an entry you
   chose to send*. `ActividadesExplotacion` declares **no required properties at
   all** and every block is `0..n` in the descriptor sheet, so no block is ever
   compulsory. A field required inside `Cosecha` only binds a `Cosecha` entry
   that exists.
4. **The printed model isn't law at all.** It is a layout source (the Andalucía
   v6 PDF, whose 2023 "OPCIONAL" heading on section 6 is older than RD
   1051/2022). Never use it as the reason to record or not record anything.

**Between Anexo V and the Anexo VI descriptor sheet, Anexo V governs.** Neither
is law: both are annexes to the same FEGA resolution (BOE-A-2023-13035), i.e.
technical specifications, not decrees. But V is the *definición de variables*
(what a field means, and its duty flag) while VI describes the web-service
*interface* (how the bytes travel); V is what point 2 above already uses; and our
copy of V is the **2025-11-20 corrección de errores**, published to correct the
earlier documents.

**But that tie-break isn't a licence to throw data away.** Where V *demands*
something, obey it. Where V says a field "debe ir vacío", VI scopes that
differently, and no decree names the field, keep what the farmer recorded.
Refusing to send can be undone and is visible; dropping a stored value is
neither.

**The rule works in both directions.** Never build a register to satisfy the
format (that is why `Cosecha` and `EnergiaUtilizada` stay empty). And never drop
a duty because the format has no member for it (`plot_water_point` exists with
no SIEX counterpart at all, because Anexo III A.1.f–g asks for it).

So the schema is a **superset**: SIEX is the national format for everything a
farm might report, including voluntary sustainability and statistical data, and
the cuaderno decrees use a subset of it. A block existing in the schema is an
offer, not an obligation; and a field FEGA marks `Obligatorio` inside a block we
do send is a real requirement even when no decree names it.

## What changed from 3.3.0 to 3.11.4

Kept as the model for the next comparison (`maintenance.md` §1). The envelope
(root, CUADERNO, required farm ids) didn't change.

- **Three `TratamFito` fields became REQUIRED**: `ProblematicaFito` (≥1 coded
  problem), `Justificaciones[].JustAct` (1..n, catalogue) and `Eficacia`
  (code). So the treatment form records these as codes when the treatment is
  entered; they can't be left to the export.
- **`MateriaActivaFormulado[]` → `MateriaActiva`**, a single number(5) code on
  `ProductosFito`, required **only** for TipoProducto 4 (autorización
  excepcional). Registered products are identified by `NumRegistro` alone.
- **`EquipoAplicador` changed**: `AplicacionManual` (yes/no) is required (it can
  be worked out); `HorasUtilizacion` became `Duracion` (+ optional
  `NumRepeticiones`, `TipoEnergia`, `TipoMaquinariaUNE`, all from catalogues and
  all omitted); `IdEquipoAplicador` (string(50)) names equipment not in
  ROMA/REGANIP, which covers hand tools and unregistered machines.
- **Date patterns are enforced** on all `Fecha*` fields: `dd/mm/yyyy` (or `-`).
  ISO dates are converted when serializing; a wrong format fails schema
  validation.
- **`DGCs[]` gained `CodigoCultivo`** (crop code, next to `CodigoDGC`) and
  `Cubiertas` (ground-cover data).
- **Descriptor rule (in the sheet, not the schema): all DGCs in one `TratamFito`
  must be the same product + variety.** A treatment in the app can cover plots
  with different crops, so the serializer splits it into one `TratamFito` per
  crop, with separate integer aliases (gap 1 keys on (treatment, crop)).
- **New blocks** `LaboresCulturales` and `Riego` (replacing `ActividadAgraria`).
- In `DatosCubierta`, **`AnchuraCubierta`, `AnchuraLibreProy` and
  `TipoCobertura` went from required to optional**, leaving `FecEstablecimientoCub`
  and `DGCs` as the only required content. That matches art. 42.1: the
  establishment date is due within a month and the widths later.
- `Pastoreo.FechaFin` went from optional to **required**.

## The September 2027 model

FEGA's "Documentación Técnica Horizontal" replaces this file's sources with SIEX
5.9.0, planned for September 2027. Its Anexo 03, "Bloques de datos", follows on
from Anexo V: one sheet of about 720 fields, each with its block, catalogue,
units and an obligation graded against RD 1054/2022. It is marked *versión
preliminar en revisión*; compare it again field by field when the final version
appears. From the comparison with Anexo V 3.11's CUE sheet:

- **Already recorded (optional, never demanded, since no decree asks for any of
  it):** which basic substance measure 11 used (field 366,
  `treatment_record.measure_basic_substance_code`); the measure units Anexo V
  field 18 already listed (uds./m², m², m² malla/ha, kg, kg/ha); irrigation good
  practices (field 481, `irrigation_practice`, sent as
  `Riego.BuenasPracticasRiego`).
- **Can't be built yet:** "Formato" on every product line, and a method per soil
  parameter, both coded against catalogues FEGA hasn't published.
- **Waiting for the final model:** the machinery block (UNE type, power, energy,
  repetitions, hours/ha) on five registers, and the energy and direct-sale
  registers, all "según normativa autonómica" in a preliminary model.
- **Nothing to do:** renamed fields, ROMA/REGANIP split into two members,
  obligations relaxed to *condicionado*, a plantation surface the DGC already
  gives, and a premises surface in m² (B.f asks premises for m³).

What it says about fields the app already has:

- **Good practices are read per ámbito**, which is how
  `BUENAS_PRACTICAS_AMBITOS` is published (one row per practice, a SI/NO column
  per ámbito): treatments use the "ámbito fitosanitario" (Voluntario, exclusive
  with the product block, as today), fertilisation its "ámbito fertilización",
  fertigation the practices marked in **both** irrigation and fertilisation, and
  irrigation its "ámbito riego". Fertilisation's and irrigation's become
  *obligatorio condicionado*: irrigation's to regional rules, and
  fertilisation's with a note that a green manuring fills in nothing else of the
  block. The conditions themselves aren't spelled out.
- **Density** is the liquid fertiliser's, in one of five units (kg/m³, g/m³,
  t/m³, kg/L, mg/L) from `UNIDADES_MEDIDA`, conditional on regional rules.
  `density_kg_l` with the fixed unit 12 (kg/L) is still a valid answer, and
  `DETALLE_MATERIAL_FERT`'s `Densidad (g/cm3)` is the same unit.
- **The material block** lists `P2O5 orgánico` and `K2O orgánico` next to
  `N orgánico` (`MACRONUTRIENTES` 15 and 16, which a composition line already
  accepts) and gives heavy metals in **%**. That doesn't settle
  `DETALLE_MATERIAL_FERT`'s metal columns, which mix units within the file
  whatever the model says.
- **`Tipo de labor`** is required and coded against `TIPO_LABOR`.

## Anexo VII catalogue study

The Anexo VII catalogues are served by a **public REST API with no
authentication**, the same data the sede portal shows (guide: "BdcSixWsp: Guía
de Servicios públicos de Siex"; its security section says no authentication is
required):

```
base  https://www11.fega.es/bdcsixwsp/
GET   /catalogos/{idTabla}            one catalogue (CSV default; XLSX, PDF)
GET   /catalogos/zip/                 all catalogues, one ZIP
GET   /catalogos/{idTabla}/fecha      {"fecha":"DD/MM/YYYY"} last-update date
```

The files are `;`-separated CSV with quoted fields, documented as
**ISO-8859-1** but actually **Windows-1252** (UNIDADES_MEDIDA has € at byte
0x80). Most have lifecycle columns `Fecha de alta` / `Fecha de modificación` /
`Fecha de baja`: **codes are never deleted, they get a baja date**, so an old
record's code stays resolvable as long as imports only ever upsert. The
registry of all 287 catalogues, refreshing, and the checks are in
`maintenance.md` §1.

### Catalogues `TratamFito` needs

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
| `OtrasActuacionesFito.TipoMedida` | `TIPO_MEDIDA_FITOSANITARIA` | 14 | code + label; the codes run 1–12, 14, 15: there is no 13 |
| (2027 model only) *Sustancia básica* | `SUSTANCIAS_BASICAS` | 28 | code + substance + function + EU approval date; read by `treatment_record.measure_basic_substance_code` |
| `OtrasActuacionesFito.BuenasPracticas` | `BUENAS_PRACTICAS_AMBITOS` | 60 | code + label + one SI/NO column per ámbito (Fitosanitario, Fertilización, Riego) |
| `DGCs[].EstadoFenologico` | `EST_FENOLOGICO` | 9 | code + BBCH-style stage + label |
| `EquipoAplicador.TipoEnergia` | `TIPENERGIA` | 10 | code + label |
| `EquipoAplicador.TipoMaquinariaUNE` | `TIPO_MAQUINA_UNE` | 689 | **string** code + label, no lifecycle dates |
| `DGCs[].CodigoCultivo` | `PRODUCTOS` | 1119 | code + name + Latin + EPPO + about 25 yes/no attribute columns |
| (prefill/checking) | `CULTIVO_USO_SIGPAC` | 2496 | crop code ↔ SIGPAC uso, the natural cross-check for the declared-crops prefill |
| (variety, for `AltaDGC`) | `VARIEDAD_ESPECIE_TIPO` | ~85k (~10 MB) | not vendored; would come with `AltaDGC` |

Catalogues change on FEGA's own schedule, so a refresh path matters, but the app
has to work offline with the vendored data from the first run.

### Catalogues for the other blocks

Looking only at `TratamFito` misses most of what the other blocks read. FEGA's
registry ([maintenance.md](maintenance.md) §1) lists **287** catalogues; check it
before deciding a field has no catalogue.

| Payload field / reader | idTabla | Rows | Note |
| --- | --- | --- | --- |
| `Analitica.MaterialAnalizado` | `MATERIAL_ANALIZADO` | 4 | **four** values: FEGA separates Cultivo from Producto cosechado |
| `Analitica.TiposAnalisis[]` | `TIPO_ANALISIS` | 6 | including 5 "Parámetros del Suelo" |
| `Analitica.TiposSustancias[]` | `SUST_ACTIVAS` | 283 | substance + **CAS number** + código europeo |
| `UsoSemillaTratada.Tratamiento` | `TIPO_TRATAMIENTO` | 4 | codes start at 2 |
| `ComercializacionVD.ProductoVegetal`, `TratamientosPostCosecha.ProductoVegetal`, `Cosecha.ProductoCosechado` | `PROD_VEGETAL` | 692 | harvested **produce**, NOT the crop catalogue; one row per (produce, crop) |
| `premises.class_code` (3.4) | `EDIFICACIONES_INSTALACIONES` | 109 | keyed on `Código SIEX` (column 2); column 0 is the tipología. Read for REA block 8's `claseInstalacion`, required when a treated building has to be identified for the CUE. All are buildings; no vehicles, which is why `class_code` is buildings only |
| 3.1 bis `treatment_record.measure_code` | `TIPO_MEDIDA_FITOSANITARIA` | 14 | the model's "Tipo de medida", per event |
| — no reader | `MEDIDA_PREVENTIVA_CULTURAL` | 14 | **not 3.1 bis's list**: a FARM-level GIP declaration on `DatosExplotacion`, with no date, plot or intensity, and no column in the printed model |
| `Riego.OrigenAgua[]`, `Fertirrigacion.OrigenAgua[]` | `ORIGEN_AGUA_RIEGO` | 6 | where **irrigation** water comes from; not the drinking-water points of 2.2 (see "Recorded gaps") |
| `CAExplotacion` range; report-language choice | `COMUNIDAD_AUTONOMA`, `PROVINCIA` | 17, 53 | CCAA has **both** catastro and INE codes; we key on INE |
| `crop.irrigation_code` counterpart | `SIST_EXPLOTACION`, `SIST_RIEGO` | 2, 8 | R/S against the 8 irrigation methods; see "Recorded gaps" |
| `crop.growing_environment_code` counterpart | `SIST_CULTIVO` | 33 | 1–4 are AL/M/BP/INV; the other 29 are crop-system distinctions we don't make |
| SIGPAC uso | `USO_SIGPAC` | 32 | the uso codes the provider returns |
| Fertilisation | `MAT_FERTI`, `DETALLE_MATERIAL_FERT`, `MACRONUTRIENTES`, `MICRONUTRIENTES`, `METALES_PESADOS`, `TIPO_FERITILIZACION`, `METODO_APLICACION_FERTILIZANTE`, `TRAT_ESTIERCOLES` | 24, 1243, 16, 7, 7, 3, 7, 9 | section 6 / Anexo III Parte I.C vocabulary |
| Eco-schemes, crops, harvest | `DEST_RES_VEG`, `TIPO_LABOR`, `TIPO_COBERTURA_SUELO`, `ESPECIE_ANIMAL`, `DESTINO_CULTIVO`, `DEST_COSECHA`, `MATERIAL_VEGETAL_REPRODUCCION`, `PROC_VEGETAL`, `REGIMEN_TENENCIA`, `PAIS` | | read by registers, or held for a named future reader (`maintenance.md` §1) |

Not vendored: `VARIEDAD_ESPECIE_TIPO` (about 85,000 rows / 10 MB, would come
with `AltaDGC`) and `ROPO` (about 174,000 rows / 30 MB). Both are registries,
not code lists, and both would dominate the binary.

**`ROPO_NIVEL` and `ROPO_CATE` can't be fetched.** They would be the authority's
own vocabulary behind core's `licence_level` lookup and table 1.2's carné
columns, but the registry marks both `exportable: false` and the endpoint
returns an empty body. FEGA publishes on its own schedule, so check again before
concluding they are unavailable. `ATRIA` and `ENT_ASESORA` are exportable:
registries of GIP groups and advisory entities that could one day prefill the
`advisor` table.

### Recorded gaps

- **SIEX has no entity for a drinking-water abstraction point.** Model section
  2.2 asks for the *puntos de captación de agua para consumo humano* near each
  plot (Anexo III A.1.f–g), and no block of `ActividadesExplotacion` or
  `DatosExplotacion` has a field for it (nothing matching *punto*, *consumo*,
  *capta*, *distancia*, *coordenada* or *masa*). The one water field,
  `OrigenAgua[]`, only appears under `Riego[]` and
  `Fertilizacion[].Fertirrigacion`, and codes where **irrigation** water comes
  from, a different question. FEGA's four water catalogues (`ORIGEN_AGUA_RIEGO`,
  `USOS_AGUA` = the water administration's use taxonomy, `REGANTES` and
  `COMU_REGA` = irrigation-community registries) all belong to that irrigation
  vocabulary. So `plot_water_point` has **no coded field**, nothing extra was
  vendored, and it exists for the printed model alone, like
  `seed_treatment_plot` and `harvest_plot` with the buyer block.
- **Ceuta and Melilla have no `CAExplotacion` LABEL; the code is fine.**
  `CAExplotacion` is documented "según codificación INE", INE's codes for the
  two ciudades autónomas are 18 and 19, and that is what `province_to_ccaa`
  returns (a unit test checks it); the schema limits the field to two characters
  with no list, so it validates. What is missing is only a row to give those
  codes a name: FEGA's `COMUNIDAD_AUTONOMA` only publishes the seventeen
  *comunidades*. Nothing shows a CCAA label today (the printed book never uses
  one, and §1.1's Provincia uses `PROVINCIA`, which does have `51 CEUTA` and `52
  MELILLA`), so a farm there prints a complete book. If something ever needs the
  label, an unresolvable code prints itself, and the Spanish wording would come
  from INE through the report labels, never invented in a core table. **Never
  map the two cities onto `00 Comunidad Desconocida` or a neighbouring
  community**: either would make a record say something false about a farm whose
  location is known. The contract test checks the absence for that reason.
- **`crop.irrigation_code` can't give a `SIST_RIEGO` code.** Our four values
  (`rainfed`, `sprinkler`, `drip`, `gravity`) answer *two* SIEX questions:
  `SIST_EXPLOTACION` (R/S, mapped completely) and `SIST_RIEGO` (the method).
  `sprinkler` falls between "Aspersión fija" and "Aspersión móvil" with nothing
  to choose between them, and `rainfed` has no `SIST_RIEGO` code. **No mapping
  is added**: a lossy one behind a passing contract test would put a statement
  the farmer never made into a regulatory export. The irrigation register
  records `SIST_RIEGO` per watering instead, which is where the question
  belongs.
- **A one-directional mapping needs an anchor test.** Where our list is smaller
  than the catalogue (`growing_environment` → `SIST_CULTIVO` 1–4), "every one of
  ours maps to an active code" stays green even if the provider renumbers and
  silently redirects every record. So the target labels are checked too
  (`growing_environments_map_to_the_siex_growing_system_they_name` anchors each
  of the four on a word of the catalogue's own label).
- **`SUST_ACTIVAS` can't code every analysis result.** It codes phytosanitary
  actives (`TipoAnalisis` 1), so a heavy-metals, nutrients or soil-parameters
  bulletin has nothing to code there. `analysis_record` keeps the free
  `substances_detected` next to the coded `analysis_substance` junction for that
  reason; a serializer sends the codes and leaves the wording to the farmer's
  own records.

### Other public SIEX services

The same API has two more things besides the catalogues (the portal's `/ffii`
section, "Fuentes de Información externas", documented in the same guide):

- **`GET /fuentesInformacion/zip`** (about 30 MB), with two CSVs:
  - **`MDF.csv`**: the Registro de Determinados Medios de Defensa Fitosanitaria,
    *non-chemical* means (biological control organisms, traps, pheromone
    attractants) with target organisms and crops. Not used: a measure's
    registration number is stored as it is. It is small enough to vendor if
    biological-control records ever need it.
  - **`ROPO.csv`**: the national phytosanitary-carné register. **Not used: it is
    a mass of personal data** (names, phones, emails) the app must not copy or
    redistribute, and the file served was years out of date. Checking or
    prefilling carnés would need a different mechanism.
- **`POST /existeNIF`**: given a NIF, returns the holder's farms with
  `Codigo_SIEX` (+ REA code and CCAA when present). This is gap 4's data, and
  could one day prefill the REA code from the NIF through `terrazgo-net`,
  sparing the farmer copying it from their REA papers. Optional and online only;
  the manual fields stay the offline path.

### Storage design

Two generic tables in **terrazgo-core**: `catalogue` (one row per imported
catalogue) and `catalogue_code` (its codes, with the provider's other columns
kept in `attrs` JSON). Reference catalogues serve the whole farm (treatments,
crop prefill, fertilisation, irrigation), and modules only depend on core, so
putting them in core avoids any module reading another. Core stays
country-neutral because the *mechanism* is generic and the Spanish part is data
(as with `geo_feature`). Columns, the import and the refresh: `data-model.md` →
"Derived and infrastructure tables" and `maintenance.md` §1. The reasons:

- **One generic table, not one per catalogue.** About fifty nearly identical
  tables, for data whose only general query is code → label + attributes. A
  catalogue becomes a typed table only when a real query needs its attributes;
  the generic rows keep everything the CSV had, so that is an additive copy and
  the codes never change. No UNIQUE on (catalogue, code): `CULTIVO_USO_SIGPAC`
  repeats a code per SIGPAC uso. Relationship data (say, a MAPA product ↔ crop
  ↔ problem table) would be separate schema either way.
- **Upsert only, never delete**: codes with a baja date must keep resolving for
  old records (with its own test). Pickers only offer codes without a baja date
  (filtered by attributes where relevant, e.g. ámbito), and still name a stored
  one.
- **No SQL foreign key from user data to catalogue codes**: the code is what the
  regulation cares about, and the catalogue row is only for display. Wrong codes
  are caught by a shared Rust check plus the export's schema-validated tests, and
  a re-import can never cascade into user records. The cost: the database itself
  won't reject a wrong code. Labels aren't copied onto records: if the source
  renames a label, showing the new one is right, because the code is what counts.
- **Vendored in the binary and imported at startup**, not shipped as migrations:
  migrations after release are append-only forever, the wrong tool for
  third-party data that changes on its own schedule. Not logged in
  `record_change` (each device imports its own copy).
- **Not in `geo-cache.db`**: regulatory reference data must be in backups, so a
  restored backup still resolves its codes.
- **Parsing**: the `csv` crate (delimiter `b';'`; the notes columns use RFC
  quoting with embedded `;` and newlines). Decoding is done by hand, with no
  encoding crate: UTF-8 is tried first (in case the provider ever switches;
  old accented text is never accidentally valid UTF-8), then Windows-1252 (only
  0x80–0x9F differ from the one-to-one Latin-1 map), with a control-character
  test that fails on any further encoding change instead of importing garbage.
- **A per-catalogue `code_col`**, because three files don't start with their own
  code: `COMUNIDAD_AUTONOMA` starts with the catastro code where SIEX wants INE,
  and `EDIFICACIONES_INSTALACIONES` and `DETALLE_MATERIAL_FERT` start with their
  parent catalogue's.

## Capture design — gaps 1/3/4

The storage rule follows two patterns the codebase already had:

- **Small closed lists with a universal meaning** (efficacy, justification,
  authorisation kind) → English-coded lookup tables + i18n keys, mapped to SIEX
  integers at export (`module_phytosanitary::siex`), like `unit` and
  `reason_category`. The `es` dictionary has the official Castilian wording word
  for word, so Spanish users see exactly the catalogue's terms. A **contract
  test** (`tests/siex_mapping.rs`) checks each mapping against the vendored
  catalogue in both directions, so a refresh that adds a code fails the suite
  instead of silently offering fewer choices.
- **Provider lists too big to own** (the ~1,400 phytosanitary problems) → the
  catalogue code stored as it is, with no foreign key. Size is the usual reason
  but not the only one: `SUST_ACTIVAS` (283 rows) is stored this way because it
  has **CAS numbers**, the key a future French or Italian export would match
  on; inventing English names for chemicals that already have a universal
  identifier would make a worse key.
- **And:** a stored provider code must have **a named reader and a way to show
  its label**; an owned lookup must have **a `Labels` accessor and a two-way
  contract test**. A catalogue nothing reads is dead weight in the binary; a code
  nothing can resolve prints an empty cell in a legal document; and *not*
  vendoring what a field needs leads to concluding the authority publishes no
  list.

Where each part is:

- **Problems (gap 3)**: the `treatment_problem` junction, with a
  `reason_category_code` + `problem_code` per row, ≥1 per record when inserting.
  This IS the "reason for treatment"; `target_organism` stays as optional free
  text. The category picks the catalogue to resolve against (disease →
  ENFERMEDADES, pest → PLAGAS, weed → MALAS_HIERBAS, growth_regulator/other →
  REGULADORES_CRECIMIENTO) and the export bucket. Codes are checked on insert
  against the imported catalogue (only that they exist: retired codes pass, as
  imports never delete); the export's schema-validated tests are the second net.
- **Justifications (gap 3)**: the `treatment_justification` junction, ≥1 per
  record on insert (known when treating, unlike efficacy).
- **Efficacy (gap 3)**: `treatment_record.efficacy_code`, which can be empty: it
  can't be known on the day of application, so it is set later and the export
  precheck lists records still missing it.
- **Product kind (gap 3)**: `product_authorisation.kind_code`
  (`registered`/`common_name`/`parallel_import`/`exceptional`, default
  registered) + `exceptional_substance_code` (an AUTORIZACION_EXCP code,
  required only for an exceptional authorisation: the `MateriaActiva` payload).
  Dose units need no schema: `siex::unit_to_siex` maps each unit to a catalogue
  code plus an exact conversion factor (SIEX has no ml/ha or g/L; the nearest
  units differ by a power of ten).
- **Integer aliases (gap 1)**: `export_alias`, in core.
- **Farm identifiers (gap 4)**: `farm.owner_tax_id` in core and
  `farm_es_extension.rea_code`.

## Gaps found

The four gaps between the app's data and the format, numbered because code
refers to them, with how each is handled.

1. **Integer activity ids.** `IdAjena*` fields are integers (`number(10)`, at
   most 9999999999), not strings, so UUIDv7 TEXT ids can't be sent. The id is
   the key the authority uses to edit and delete, so it must be *stable across
   exports*. `export_alias` (core): `(target, entity_table, entity_id,
   split_key) → alias INTEGER`, created as MAX+1 per target at the first export,
   never updated or deleted. `split_key` tells apart the per-crop `TratamFito`
   splits; an alias existing doubles as the "previously exported" marker that
   drives `Borrar`. Synced and logged (it can't be worked out again). Two devices
   exporting before syncing could create the same integer, which is why only one
   device per farm submits and creates that farm's numbers (sync.md →
   `export_alias` collisions).
2. **DGC linkage.** Pointing at the REA's DGC codes needs the REA import
   (`exportarREA`), which is server-side. The export instead creates a
   `CodigoDGCAjena` integer for each core `crop` row (a crop IS the plot + crop +
   season unit), stable across exports and shared by every treatment on that
   crop. CyL accepts commercial notebooks importing surfaces and crops, so the
   `AltaDGC` path works standalone. Not built: generating the `AltaDGC` blocks
   themselves (they need `CodigoCultivo` from PRODUCTOS) and anything to do with
   the REA's `CodigoDGC`. Out of scope while there is no delivery path.
3. **Anexo VII catalogue codes.** Crops, varieties, units, active substances,
   product types, phytosanitary problems, justifications and efficacy are *coded*
   lists. The treatment form records problems, justifications and efficacy as
   codes, product kinds and exceptional substances are coded on the
   authorisation, and units map at export (see "Capture design"). Crop coding
   (`DGCs[].CodigoCultivo`, from `crop.crop_code`) is sent where the crop has a
   code.
4. **Farm identifiers.** `IdTitular` (NIF) and `CodigoRea` are required. They
   are `farm.owner_tax_id` (in core: a tax or identity number exists in every
   country, with per-country format checks) and `farm_es_extension.rea_code`
   (not `rega_code`, which is the *livestock* registry). Both are entered by the
   user from the farm's REA registration, never worked out. `CAExplotacion` needs
   no column: it comes from `farm_es_extension.province_code` through a fixed
   province → CCAA map. `UnidadGestora` is "Identificador (NIF/CIF) de la Unidad
   gestora": for a notebook the holder uses directly, the export uses
   `owner_tax_id` (open question 7).

## Export module

**`terrazgo-siex`**: the query layer and serializer for one farm and season,
validated in its tests against the vendored 3.11.4 schema (the `jsonschema`
crate, dev-dependency only, with HTTP resolving off). Two entry points:

- **`export_precheck(conn, season, farm)`** lists everything that blocks a valid
  export at once, instead of failing one field at a time (records missing
  efficacy, operators with no licence number, treated plots with no crop, farm
  identity fields missing or unusable, and the rules described below). Only
  active records are checked: a deletion entry can't demand new observations.
- **`build_cuaderno(conn, season, farm)`** builds the descriptor
  (`descriptor::CuadernoExport`, typed serde structs mirroring the schema). It
  refuses while the precheck isn't clean, so nothing is silently dropped or
  invented. **An export that silently leaves a register out is worse than one
  that refuses with a list the farmer can fix.**

`export_cuaderno_precheck` and `export_cuaderno` are still registered commands,
but nothing in the UI calls them: a button producing a file with nowhere to send
it was the wrong thing to show a farmer. So any field added to the precheck in
the meantime isn't exercised end to end; bringing the export back means
rebuilding its UI and its checks, not just calling the command.

### Serialization rules, each checked by a test

- **Per-crop splits.** Plots are grouped by their `(crop_name, variety)`
  snapshot; a record with several groups becomes one `TratamFito` per group,
  aliased on (record, split key). The snapshots are fixed when written, so the
  grouping can't change between exports. A record with one group keeps the empty
  split key. The book and the export split the same way, so the rule is in
  `module_phytosanitary::grouping`.
- **Deletions.** A soft-deleted record produces a full entry with `Borrar: true`
  for each split that has an alias (i.e. was actually exported); splits never
  exported are skipped. A deletion entry still has to meet the schema's required
  fields, so an efficacy never assessed falls back to the schema's default 0 and
  a missing licence to the empty string: the entry identifies the deleted
  activity, it doesn't state observations. **A removed record with an
  `export_alias` is never purged**, because the authority may hold it (sync.md →
  When a register goes).
- **Equipment `oneOf`.** The schema requires exactly one of
  `NumROMA`/`NumREGANIP`/`IdEquipoAplicador`, even for manual application, and
  in every block that has `EquipoAplicador` (the non-field blocks drop
  `AplicacionManual`, but not the `oneOf`). No machinery → `AplicacionManual:
  true` + the fixed value `"manual"`; machinery in both registries → ROMA ("nunca
  ambos"); machinery in neither → its row id as `IdEquipoAplicador` (a free
  string(50) that never changes).
- **Product kind.** Looked up live from the stored authorisation number (product
  + country + `authorisation_number_snapshot`); when no authorisation row matches
  any more, the default kind (registered) applies. `MateriaActiva` (the
  AUTORIZACION_EXCP code) is only sent for kind `exceptional`.
- **Dose, not quantity.** The descriptor allows `Dosis` XOR `Cantidad`. Every
  record has a dose, while a total quantity is missing whenever the dose is a
  concentration, so `TratamFito` sends `Dosis`.
- **Dates** convert from ISO to dd/mm/yyyy (`siex::date_to_siex`);
  `CAExplotacion` comes from the province through `siex::province_to_ccaa`;
  `UnidadGestora` = `owner_tax_id`.
- **`CodigoRea` is exactly 14 characters** (minLength = maxLength = 14, like
  `CodigoSIEX`: ES + 12 digits). The precheck flags a REA code of the wrong
  length the same way as a missing one.
- **The official schema has a typo**: one `$id` reads `"##root/…"` (double `#`,
  under SiembraPlantacion → Maquinaria → items), which draft-07 meta-validation
  rejects. The vendored file stays byte for byte; the tests fix it in their
  in-memory copy only (the `$id`s are decorative; the schema has no `$ref`).

### Where it lives, and why

`terrazgo-siex` is a top-level crate above the modules, next to
`terrazgo-recordbook` and independent of it. **Ten of the format's fifteen
blocks come from `module-fertilisation` and `module-ecoscheme`**, which a module
can't depend on, so the export can't live inside a module. And it isn't part of
the record book either: the two read the same registers but produce different
documents under different rules. The book prints what exists and blocks nothing;
the descriptor is validated by an authority and can't leave out, invent or drop a
required field.

- **`export_alias` is in `terrazgo-core`**: it gives aliases to registers owned
  by core and all three modules. It is part of core's backup fingerprint, so a
  stale backup without the table can't import cleanly and lose every alias.
- **`list_treatment_records_for_export` is `pub`**, and its name is the warning:
  a caller that isn't building an export and wants soft-deleted rows is almost
  certainly making a mistake. The same goes for the `*_for_export` getters
  (`get_fertiliser_material_for_export`, `get_soil_cover_for_export`,
  `find_crop_for_export`): **they deliberately include withdrawn rows**, because
  a record written years ago may name a material, cover or crop that has since
  been removed, and the descriptor must still state its code.
- `terrazgo_siex::db::migrations()` builds the same schema the shell does,
  checked by a contract test in
  `src-tauri/tests/contracts/migration_composition.rs`.

### The two blocks nothing will fill

- **`Cosecha`** ("COSECHA/RECOLECCIÓN/SIEGA") is the harvesting *operation*,
  with thirteen `Obligatorio` fields, including five yes/no fields about keeping
  seed, environmental non-harvest and whether mown residue was left on the
  ground. No decree asks a cuaderno to keep any of it. Our `harvest_record` is
  *what left the farm*, which is `ComercializacionVD`. Block 11 is named for
  siega and has "depositado en el suelo de los restos **segados**", while
  `LaboresCulturales` has "restos **desbrozados**" and "de **poda**"; but
  `TIPO_LABOR` code 5 is literally *"Desbroce y siega"*, and RD 1048/2022 art. 31
  separates *"siega para producción o mantenimiento"*. Model 9.2's register is
  the **maintenance** one, which is where mowing is recorded.
- **`EnergiaUtilizada`** is voluntary in all six of its fields and has no register
  behind it.

Building either would be recording driven by the format rather than by a duty.

### What each block sends

| Block | From | Notes |
| --- | --- | --- |
| `TratamFito` | `treatment_record` + junctions | See the serialization rules above and "`TratamFito`'s sub-blocks" below. `FechaFin` is `application_end_date` when set. `HoraTratamiento` is `application_time` (local `HH:MM`, padded to the seconds Anexo VI's `string(8)` wants). `DGCs[].EstadoFenologico` is `treatment_plot.growth_stage_code` as an integer: the catalogue's code, NOT the BBCH stage the book prints; a code that can't be parsed is left out rather than refused, since the field is optional. `ProductosFito.Cantidad` isn't sent (Dosis is) |
| `TratamientosPostCosecha` | `non_field_treatment` (subject `postharvest`) | `Cantidad` is the produce in **kilograms**: Anexo V fixes the unit and the block has no unit member, while model 3.3 prints tonnes, so the serializer converts (sending the stored number would say 120 kg for 120 t). `ProductosFito` here has no `Dosis` and requires `Cantidad`, which is what this register records |
| `TratamientosEdifInstalaciones` | `non_field_treatment` (subjects `premises`, `transport`) | Keyed on `premises_es_extension.rea_installation_code` (see "Premises and `IdEdificacion`"). Model 3.5's vehicles go in the same block because the WS descriptor has **no transport block at all** |
| `UsoSemillaTratada` | `seed_treatment` | `Producto` is the **crop** (Anexo V field 1: "Cultivo — código del cultivo del catálogo SIEX"), so it takes `crop_code`. Its optional `ProductosFito` child isn't sent: the register stores no amount of product and model 3.2 prints no such column |
| `Analitica` | `analysis_record` + junctions | The schema only requires the material and the date, both always set. `ParametrosSuelo` is left out entirely when the bulletin gave no soil figure: a missing measurement is missing, never zero |
| `ComercializacionVD` | `harvest_record` (core) | One stored date fills both ends. The block has its own `Unidad`, so kg or t travel unconverted. Not sent: `TipoVenta` (optional, Voluntario, not stored: the printed model makes no distinction between comercializada and directa, so claiming one would invent it), and `NumFactura`/`NumLote`, which are in the descriptor SHEET but **not in the JSON Schema**, so `delivery_note_ref` and `lot_number` are only printed |
| `SiembraPlantacion` | `sowing_record` + `sowing_plot` (core) | See "Treated seed and the sowing it was used for" and "`SiembraPlantacion` is a sowing/planting flag". `DGCs[]` is the plot junction with its crop; no `SuperficieCultivada`, because the descriptor says it equals the DGC's surface unless stated, which is exactly what `sowing_plot` having no surface means. `SiembraDirecta` isn't sent (it is a `cultural_operation` of kind `no_tillage`), nor `DosisSiembra`/`MarcoPlantacion`/`DensidadPlantacion`/`UnidadesRemolacha`, which Anexo V makes alternatives to the `Cantidad` this register stores |
| `Fertilizacion` | `fertilisation_record`, `fertiliser_material` + junctions | The composition comes from the material REGISTRY row (`get_fertiliser_material_for_export`, which includes removed materials). `BuenasPracticas` is an **empty array** when none was declared, which is schema-valid (no `minItems`) and what Anexo V's field 6 says. `EquipoAplicador` is left out completely when no machine is named (C.g's "cuando proceda", and the block's `oneOf`, which a half-filled block would fail). `Fertirrigacion`: see "Fertigation: one act, sent twice". `BuenasPracticasRiego` here isn't sent |
| `Riego` | `irrigation_record` + junctions | `OrigenAgua` is a junction and `energy_type_code`/`meter_number` exist. `BuenasPracticasRiego` comes from `irrigation_practice`. Units: Anexo V allows m³ and L, the register stores m³ or m³/ha, and `UNIDADES_MEDIDA` has m³/ha as code 19, so a per-hectare volume is sent as such rather than converted with a surface the record may not have |
| `PlanAbonado` | `fertilisation_plan` + `fertilisation_plan_crop` | Its required set IS art. 5.a's list plus `Herramienta`. `Asesor` (a REGFER code) and `FechaAsesoramiento` aren't stored: art. 6.6's advice requirement is on the plan DOCUMENT, and the record art. 5.a describes names no advisor |
| `Pastoreo` | `grazing_record` + `grazing_plot` + `grazing_animal` | Per line `{REGA, Numero, Especie}`. **`AnimalesPropios` and `AnimalesTerceros` are required yes/no fields** ("Pastoreo con animales de la explotación (S/N)"), not head counts, and are **worked out**: a line whose REGA is the farm's own (`farm_es_extension.rega_code`) is its own animals, any other a third party's. The precheck demands the farm's REGA once a season has a grazing, since without it every line would read as a third party's. The descriptor forbids both being false; `insert_grazing_record` already refuses a grazing with no animals. `FechaFin` is required: see "The crop of an eco-scheme DGC" for open grazings. `species_code` isn't checked on insert (a provider code stored as it is) while `Especie` is a required integer, so `grazings_with_unsendable_species` is where the two meet |
| `LaboresCulturales` | `cultural_operation` + `cultural_operation_plot` | `TipoLabor` through `module_ecoscheme::siex::cultural_operation_kind_to_siex`, **not one-to-one on purpose**: `mowing` and `brush_cutting` both map to `TIPO_LABOR` 5, because model 9.4 prints Siega and Desbrozado as two columns where the catalogue has one code. **`DepositadoSueloDesb` and `DepositadoSueloPoda` are worked out, not stored**: `DEST_RES_VEG` 9 (*poda* residue) and 1 ("Incorporación al suelo o distribución en parcela", `RESIDUE_LEFT_ON_PLOT`) both mean left on the ground, and the kind decides which of the two fields it fills; `pruning_removal` fills neither (`TIPO_LABOR` 11 is *"Eliminación de restos de poda"*). `Maquinaria[]` isn't sent (Voluntario in all six fields, no printed column). Each kind follows what its code says: 7 is *"Poda en verde, incluida la limpieza de tallos, chupones y varetas"* (`green_pruning_with_cleaning`), 12 *"Poda en verde"* (`green_pruning`), so there is no kind for a pruning outside the green season |
| `DatosCubierta` | `soil_cover` + `soil_cover_plot` | `FecEstablecimientoCub`, `AnchuraCubierta`, `AnchuraLibreProy` and `TipoCobertura` map directly; `DGCs[]` is the plot junction. A cover without widths is refused (see "The crop of an eco-scheme DGC"). The maintenance isn't in this block: art. 42.1.c's siega, desbroce and pastoreo travel as their own `LaboresCulturales` and `Pastoreo` entries naming the cover through `DGCs[].Cubiertas[]`. `ActividadCubierta[]` is in the sheet but not the schema, so nothing is sent |

**Recorded by the app, sent by nothing, and why:**

| Field | Why |
| --- | --- |
| `Fertilizacion.BuenasPracticasRiego` | Voluntario in Anexo V and in the 2027 model (field 456, "ámbito riego y fertilización, a la vez"), no printed column. A fertigation's would be the practices claimed on the linked watering, which nothing asks for yet |
| `OtrasActuacionesFito.BuenasPracticas` | Voluntario, no printed column |
| `treatment_record.measure_basic_substance_code` | 3.11.4 has no member for it; the 2027 model adds one (Anexo 03 field 366) |
| `DatosExplotacion.MedidaPreventivaCultural` | the farm-level declaration of which IPM practices it follows (`MEDIDA_PREVENTIVA_CULTURAL`); optional, next to the parked `AltaDGC`, and the printed model has no column for it, so nothing records it. It is **not** 3.1 bis's "Tipo de medida", which is `TIPO_MEDIDA_FITOSANITARIA`, per event |

**Tables with nothing to send:** `plot_water_point` and `plot_water_declaration`
(SIEX has no abstraction-point entity: "Recorded gaps"); and the sync and alert
tables (`sync_peer`, the stamp columns on `record_change`, `sync_group`,
`alert_acknowledgement`, `duplicate_verdict`, `purged_register`, `sync_held`,
`sync_known`). These describe the devices, what the app told the farmer, or
judgements about the book's own records, not the farm's activities. A season
belonging to its farm changes no block either: the descriptor is already built
for one farm and one campaign.

**What a field must meet to be recorded:** a field SIEX *requires* is recorded
even when the printed model has no column; a field FEGA marks `Obligatorio`
(Anexo V) inside a block we send is recorded even when no decree names it
(`PlanAbonado.Herramienta`, `SiembraPlantacion.FechaAdquisicion`,
`Fertilizacion.GestionSostInsu`, `Fertilizacion.BuenasPracticas`); a field
optional in SIEX AND missing from the model is listed here instead.

### `TratamFito`'s sub-blocks

`OtrasActuacionesFito`, `AsesorValidacion` and `FechaSeca`, all fed by existing
columns. **A purely non-chemical action is a full entry**: `TratamFito`'s
required set is `["IdAjenaTratamFito", "FechaInicio", "FechaFin", "DGCs",
"ProblematicaFito", "Justificaciones", "IdentificadorAplicador", "Eficacia"]`,
without `ProductosFito`, so an entry naming no product is schema-valid.
`OtrasActuacionesFito` is an OBJECT, not an array: one measure per action, which
is why the register keeps its columns on the record.

| Member | Our column | State |
| --- | --- | --- |
| `TipoMedida` (required) | `treatment_record.measure_code` | sent; `TIPO_MEDIDA_FITOSANITARIA` runs 1–12, 14, 15: **there is no 13** |
| `Cantidad` / `Unidad` | the measure's intensity value + unit | sent, and **demanded** (see below) |
| `NumRegistroMDF` | `measure_registration_number` | sent whenever stored; the MDF registry isn't vendored, and the number is stored as it is, so nothing needs it to resolve |
| `BuenasPracticas` | not recorded | Voluntario, no printed column |

#### A treatment is a spray or a measure, not both

Anexo V grades all five members of `OtrasActuacionesFito` *"excluyente con el
subbloque siguiente de «Productos fitosanitarios»"*. The descriptor sheet's *"se
debe indicar al menos una «otra actuacion fitosanitaria» o un producto"* is an
OR that would allow both, but it is the weaker document. So a **mixed** record (a
spray and a measure on one row, which the register allows because model 3.1 bis
prints "Alternativas no químicas" and "Alternativas químicas" as two groups of
columns of one row) can't be one entry, and the precheck refuses it
(`records_mixing_product_and_measure`).

**The decree agrees from the other side.** RD 1311/2012 art. 16.1 ties the
record to *"la información especificada en la Parte I del anexo III"*, and Parte I
sección B starts *"Para cada tratamiento que se realice en la explotación…
especificar la información siguiente"* with eleven lettered items, of which g
names a *producto fitosanitario* and i its kilos or litres. **There is no
non-chemical item in that list.** The whole block is the format's own; the
register exists because model 3.1 bis prints those columns for art. 10–11 GIP
compliance, and art. 10 requires *choosing* non-chemical methods, never
recording one. With the decree's unit being *cada tratamiento*, a row with both is
a row with two treatments. **Splitting the row into two entries was rejected**:
`export_alias` is created once and never changed because SIEX keys edits and
deletions on it, so one row would get two aliases, and a later correction that
removed the measure would leave one alias claiming an activity that no longer
exists.

#### Rules from Anexo V's gradings

- **`records_with_unsendable_measure`**: the intensity can be empty in the
  register (a farmer may record that traps were hung before counting them) and
  the book prints such a measure without complaint, while **Anexo V grades fields
  17 and 18 Obligatorio**. The JSON Schema only requires `TipoMedida`, so this is
  the grading deciding over `required`. The rule also catches a measure code
  that isn't an integer, and an intensity unit SIEX can't express.
- **`records_missing_measure_registration`**: Anexo V field 19 grades `Registro
  MDF` Obligatorio for *"suelta de OCB, trampas y otros y feromonas y atrayentes
  para monitoreo"*: `TIPO_MEDIDA_FITOSANITARIA` 1, 14 and 15.
- **`records_missing_advisor_ropo`**: a record naming an advisor with no ROPO
  number. Anexo V grades field 50 Obligatorio **here**, while blocks 1.2 and 1.3
  grade the same field Voluntario, which is why the three non-field registers
  leave the block out instead and this one refuses. `NumROPO` is the only member
  that can be carried, so leaving it out would drop the identification Anexo III
  B.d asks for in the same sentence as the applicator's.

**What isn't enforced, on purpose.** Anexo V's field 19 says that outside its
three kinds *"el campo debe ir vacío"*, and Anexo VI names a **different** set
(OCB, plantas banker, trampas cromotrópicas); no decree names the field at all.
So the *demand* is enforced and the *emptying* isn't: a stored MDF number always
travels, whatever the measure kind, because following either list would silently
throw away something the farmer recorded on the strength of a rule the authority
states two ways.

The same applies to `Unidad`. Anexo V's field 18 narrows the "unidades válidas"
to Unidades, uds./m², uds./ha, m², m² malla/ha, kg and kg/ha, a list without
Trampas and Difusores, which `UNIDADES_MEDIDA` publishes (27, 24, 25, 22) and the
JSON Schema accepts. Answering "12 trampas" with code 11 (Unidades) would drop
*what was counted*, which model 3.1 bis asks for by name, so
`intensity_unit_to_siex` sends the exact code and leaves the narrowing to the
receiver.

#### `FechaSeca` is sent and never required

A plain dd/mm/yyyy from `treatment_record.drying_date`. Anexo V field 4 grades it
**Obligatorio** *"cuando se trate de cultivos bajo agua"*, and the export doesn't
require it, though `sowing_record.flooded_on` would make the condition checkable.
The condition isn't that the crop is flooded but that the field was dried *for
this treatment*: Anexo V's own wording is *"fecha en la que se realiza el secado
**para la realización del tratamiento**"*, which is also why the column is on the
treatment and not on the flooded crop. A rice herbicide applied on water is a
lawful record with no drying date, so requiring it whenever `flooded_on` is set
would refuse records the decree allows. A test checks that a treatment on a
flooded crop needs no drying date. (The Anexo VI sheet words the same duty as
*"Obligatorio para producto arroz (80)"*; Anexo V's *"cultivos bajo agua"* is
wider and governs.)

#### `AsesorValidacion`

Only `NumROPO` is sent. `Validacion`, `Confirmacion`, `Contrato`, `Fecha` and
`Observaciones` stay empty: model 3.1 bis collects the sign-off as a handwritten
signature and the book has no signatures by design, so filling any of them would
invent the one thing the block exists to confirm. Anexo V also grades the
advisor's `Nombre`, apellidos, `Razón social` and `NIF` Obligatorio here, and the
JSON Schema has **no member for any of them**, so there is nothing to do about
it.

### The crop of an eco-scheme DGC

`grazing_plot`, `cultural_operation_plot` and `soil_cover_plot` have a plot and
no crop, because no printed page of section 9 asks for one (9.1 wants the SIGPAC
reference, 9.2 the plot, 9.4 the cover), while a SIEX DGC is a plot + crop unit.
**Anexo V asks for the crop to be worked out**: field 3 of both `Pastoreo` and
`LaboresCulturales` is *"Cultivo/s … Campo calculado"*.

So `crop_on_plot(conn, plot_id, season_id)` folds the live crops of that plot in
that season into three cases:

| Case | What the export does |
| --- | --- |
| exactly one live crop | that crop's `CodigoDGCAjena` and `CodigoCultivo` |
| none | precheck: `ecoscheme_plots_missing_crop` |
| **two or more** | precheck: `ecoscheme_plots_with_ambiguous_crop`, **never resolved** |

A plot with two crops **is** two DGCs and the record names neither, so choosing
one would claim the activity happened on a crop the farmer never stated. A test
checks this (making the rule take the first crop turns it red).

The two halves of the rule live in different places: **the query belongs to the
crate that owns the data, the refusal to the document that refuses.**
`terrazgo_core::repository::crops_on_plot` returns **every** live crop and
chooses nothing (another reader might reasonably split a two-crop plot instead,
and core has no business deciding). `terrazgo_siex::blocks::crop_on_plot` turns
that list into `PlotCrop`, and is where "two crops means refuse" lives, because
that is this document's rule; the record book asks differently and blocks
nothing.

`Superficie` is always left out on these blocks. Neither junction has a surface
column, and the descriptor reads a missing `Superficie` as the DGC's own (*"es
igual a la superficie DGC salvo que se indique lo contrario"*, stated on
`LaboresCulturales` and `SiembraPlantacion`; for `Pastoreo` it is our reading,
not a quotation). Sending the crop's `area_ha` would claim every hectare of it
was grazed or worked.

**Two refusals:**

- **An open grazing is refused, not skipped.** RD 1048/2022 art. 30.2 ter gives
  the farmer a month *"desde la nueva fecha de inicio o fin que haya resultado de
  la modificación"*, so a grazing still going on isn't late, it is unfinished;
  but `Pastoreo.FechaFin` is required. `grazings_without_end` names such records
  and the export refuses, like `records_missing_efficacy` (a field empty because
  it can't be known yet, required by the schema, made a blocker rather than
  silently left out). The cost: a farm with animals out can't export until it
  closes the record.
- **A cover with no widths is refused too, and this is where the two documents
  part.** Art. 42.1.e falls due *"en el mes anterior al final del periodo mínimo
  de cuatro meses"* while 42.1.a is due within a month of establishment, so a
  cover between the two deadlines is a complete record, and the record book
  prints it without complaint. But **Anexo V grades both widths `Obligatorio`**
  for exactly the three cover types this register can hold (*"Solo para los casos
  de cubierta vegetal sembrada, cubierta vegetal espontánea y cubierta inerte de
  restos de poda"*: `PLANT_COVER_TYPES` and `INERT_COVER_TYPES`), and the grading
  decides, even though the schema made the widths optional.

Also from Anexo V and the sheet:

- **`DGCs[].Cubiertas[]` exists on `Pastoreo` and `LaboresCulturales`** (Anexo V's
  "Actividad en la cubierta" sub-block, `Obligatorio`), and `soil_cover_id` is
  exactly when it applies. It is resolved once per entry, which makes the sheet's
  rule (*"No se pueden indicar en la misma actividad DGCs con cubierta y sin
  cubierta"*) hold by construction.
- **A cover's type is read through `get_soil_cover_for_export`**, not cached per
  export: `validated_cover_link` checks the farm but NOT the season, so the
  season's own list of covers can't be relied on to contain it. (The fertigation
  link *is* checked against farm and campaign, so the season's waterings can be
  listed once; the two look alike and aren't.)
- A cover type that isn't an integer joins `covers_missing_fields`.

### Treated seed and the sowing it was used for

`SiembraPlantacion` has `MaterialAdquirido`, `FechaAdquisicion`,
`MaterialTratado` and `NumLote`, which describe the seed of model 3.2's
`seed_treatment`, and the format treats §3.2 as *a sowing that used treated
material* rather than as a register of its own (which is why
`UsoSemillaTratada` has no plots). Three of the four need nothing new:

- **`MaterialAdquirido` is already stored, in FEGA's own coding.**
  `TIPO_TRATAMIENTO` 4 and 5 are literally *"adquisición de semilla tratada con
  producto autorizado en España"* and *"…fuera de España"* (against 2 and 3, seed
  treated on the farm or at a conditioning centre), and
  `seed_treatment.treatment_kind_code` *is* that catalogue.
- **`MaterialTratado`** is whether such a 3.2 record exists: RD 1311/2012 Anexo
  III Parte I B.e (*"si la siembra se realiza con semilla tratada, indicar el
  producto"*) is what `seed_treatment` records.
- **`NumLote`** is `seed_lot`.
- **`FechaAdquisicion`** is `seed_treatment.acquired_on`: what was bought is the
  *seed*, 3.2's subject, and `seed_lot` is already there.

**The link between the two registers is stated by the farmer, not guessed.**
`seed_treatment.sowing_record_id` is a nullable foreign key set on the 3.2 form.
Matching on date and plots was rejected: a farmer whose two registers differed by
a day would silently get `MaterialTratado: false`, with nothing on screen to show
it. The direction is forced twice: a module can reference a core table and never
the reverse, so the column can only be on `seed_treatment`; and one sowing can use
several seed lots (one row per product), each naming it, which a column on
`sowing_record` would limit to one. The descriptor sheet points the same way:
`UsoSemillaTratada` has an optional `IdAjenaSiembraPlant`, *"Identificador de la
actividad de siembra en origen"*. That member isn't in the JSON Schema, so it
isn't sent, but it confirms the format sees two entries with a pointer, not one
merged activity.

**How several lots become one answer.** `MaterialTratado` is true when any live
linked record exists; `MaterialAdquirido` when any of them is a purchase (one
bought sack makes the answer yes); `FechaAdquisicion` is the **earliest**
purchase (the precheck demands a date on every purchased record); and `NumLote` is
only sent when the linked records agree on one: naming one of two lots would be
false about the other, and the member is optional. Each lot still travels as its
own `UsoSemillaTratada` entry.

**The link only points at LIVE records.** It can't point at a soft-deleted sowing,
and a soft-deleted 3.2 record stops saying the sowing used treated seed. It is
also refused for a sowing on another farm or in another campaign: the foreign key
alone would allow both, and a link across farms would put one farmer's treated
seed in another's descriptor.

### `SiembraPlantacion` is a sowing/planting flag

The required member `SiembraPlantacion` isn't the crop. The WS descriptor types it
`number(1)`, *"1 Siembra 0 Plantación"*; Anexo V's "Cultivo" (field 3) is
`DGCs[].CodigoCultivo`, per DGC. So it needs an answer, and the app's form already
asks it: `sowing_record`'s form is called "Siembra y plantación" and asks the
farmer to *"anote cómo empezó cada cultivo"*, so plantings are part of its use. A
constant `1` would misstate every one of them, so `sowing_record.kind_code`
(`sowing_kind`, two values, in core) is `NOT NULL`. No decree asks for a planting
entry: the column records something the form already asked.
(`MATERIAL_VEGETAL_REPRODUCCION` isn't its catalogue.)

### Fertigation: one act, sent twice

A fertigation is recorded twice by the decree (art. 5.d puts the fertiliser in
§6's register and art. 5.e the water in §8's), and the format puts them together
again as `Fertilizacion.Fertirrigacion`. **That sub-block is the only place in the
format for Anexo III C.l's two water-quality figures**:
`irrigation_record.water_nitric_n_mg_l` and `water_soluble_p2o5_mg_l` appear in no
printed column, in no member of `Riego`, and in no other block. Without it, two
columns of a **binding** Anexo III letter would be recorded for nobody.

So it is filled from a stated link: `fertilisation_record.irrigation_record_id`,
optional, set by the farmer on the §6 form, and **refused unless
`application_method.is_fertigation`** (on any other method it would claim a
fertigation that didn't happen). It is checked against the same farm AND
campaign and refused for a removed watering, like `seed_treatment.sowing_record_id`.
A removed watering counts as absent, which a test checks.

The precheck requires the link for a fertigation, which asks nothing new: art.
5.e already requires the irrigation record for that watering. It requires the two
water figures separately, because art. 17.2 makes them conditional and the
register lets them be empty, so a fertigation whose watering gives neither can't
fill a block that requires both.

The same act appears twice in the file, as a `Riego` entry and inside
`Fertilizacion.Fertirrigacion`. That is the format's shape, not a duplication bug:
the two blocks answer different questions, and the decree splits the act the
same way across arts. 5.d and 5.e.

`Fertilizacion.GestionSostInsu` is **Obligatorio** in Anexo V (block 9 field 5:
*"indicar si realizan o no una gestión sostenible de insumos conforme a las
disposiciones normativas vigentes en materia de nutrición sostenible de los
suelos agrarios"*), so it is recorded, as
`fertilisation_record.sustainable_input_management`, although no decree names it.

### Premises and `IdEdificacion`

`TratamientosEdifInstalaciones` requires `Edificaciones[].IdEdificacion` as a
`number(10)`. **It is the REA's own key for an installation registered there**,
not something the app can create:

| Document | What it says |
| --- | --- |
| `descriptor-EstructuraRegistro.tsv` (**REA**) | `instalacionesEdificaciones` has `claseInstalacion` (the catalogue), `referenciaCatastral string(20)`, and **`identificador`, `number(10)`, "Código del edificio/instalación en el REA"** |
| `descriptor-EstructuraCuadernoWS.tsv` (**what our JSON Schema mirrors**) | `Edificaciones { IdEdificacion* number(10), Volumen, Unidad }`: one required field, with the same shape and wording as REA's `identificador` |
| `descriptor-EstructuraCuaderno.tsv` (the non-WS structure) | has **both**: `referenciaCatastral string(20)` as Anexo V field 1, and `identificador number(10)` "Código del edificio/instalación en el REA" |
| Anexo V, CUE sheet, block **1.3** | a sub-block named **"Instalación identificada en el REA"**, whose only identifying field is nº 1, *"Referencia catastral de la edificación/instalación o de la parcela en que se ubica"*, **Obligatorio** |

So it is like `CodigoDGC`, not `CodigoDGCAjena`, and there is no
`IdAjenaEdificacion` to create; a number made up by the client would name a
different building. It is **entered by the user from their own REA papers**, like
`farm.owner_tax_id` and `farm_es_extension.rea_code` (the REA-first rule), in
`premises_es_extension.rea_installation_code`, and the precheck demands it: a
record naming no premises, a premises with no code, or a code that isn't an
integer all block the export with a list to fix. If farmers turn out not to be
able to find those codes at all, nothing was invented and the blocker says so.

The registry also has two buildings-only fields: **`cadastral_reference`**, the
identification Anexo V marks `Obligatorio` in a block we send (and the only one
of the three a farmer can easily give), and **`class_code`**, because Anexo V's
REA block 8 field 1 is required *"en caso de actuación, tratamiento o práctica en
las edificaciones e instalaciones que conlleve su identificación para la
cumplimentación del CUE"*, which is models 3.4/3.5's own case.

Why there is a premises registry at all: RD 1311/2012 Anexo III Parte I B.b asks
for the treated *local o medio de transporte* to be **identified** (with B.f
adding the volume in m³), and a description typed again on each treatment
identifies nothing. A registry of establishments isn't required for our user:
the registry of RD 1311/2012 art. 42–43 is ROPO's, covering supply, treatments
*for third parties*, advice and professional users "en tanto sea con carácter
comercial, industrial o corporativo"; a farmer treating their own store is
outside it, and art. 16 only requires the treatment register. Design:
`docs/data-model.md` → `premises`. (The consolidated BOE page doesn't include
Anexo III, so B.b and B.f rest on our own transcription, and the art. 42–43 scope
is a summary rather than a quotation; re-read them directly before relying on
them further.)

**The WS structure has no transport block.** Searching the descriptor for
*transporte* finds nothing, so model 3.5's vehicles are sent in
`TratamientosEdifInstalaciones`, although they are absent from the class
catalogue.

### Narrower problem vocabularies outside `TratamFito`

Neither non-field block has `MalasHierbas`, and the buildings block has no
`ReguladoresOtros` either. Our registers accept every reason category, so a weed
treatment in a warehouse is something the format can't express: the precheck
refuses it with a named row rather than exporting it with the reason silently
missing.

## Open questions for CUECYL

The questions sent to CUECYL, and the answers. Two answers settle almost
everything:

- **Connecting to the REACYL or CUECYL web services requires being a company or
  an autónomo.** That closes questions 1–3 and 5 for now: no individual, however
  the software is licensed, gets web-service credentials.
- **CUECYL has no file upload for farmers**: no manual submission of the
  descriptor JSON. That answers question 4, the one the export depended on, in
  the negative.

The onboarding path for commercial notebooks in CyL is published: a form for the
test environment, sent to **comercialcuecyl@jcyl.es**, linked to the MAPA "grupo
de trabajo mixto cuaderno digital"; after testing the company moves to production
and is added to a public list. Holders can use a commercial notebook directly,
without an entidad habilitada, if the notebook implements the authorisation flow
the Cuecyl app offers.

1. How to register as a commercial-notebook developer (empresa desarrolladora)
   in Castilla y León; is an autónomo accepted? *Answered: a company or an
   autónomo.*
2. CyL's IUWS endpoint and any CyL-specific documentation. *Closed with 1.*
3. Access to the integration/test environment mentioned in FEGA Anexo VI.
   *Answered: the form, sent to comercialcuecyl@jcyl.es.*
4. Is there any *file* import into CUECYL for farmers (manual upload of the
   descriptor JSON), or is the authorised web service the only path? *Answered:
   no file import.*
5. For DGCs: should commercial notebooks point at REA `CodigoDGC` (via
   `exportarREA`) or create their own via `AltaDGC`/`CodigoDGCAjena`? *The
   `AltaDGC` path is accepted in practice; moot without a delivery path.*
6. The REACYL DGC Excel export: which columns does it have, in particular
   `CodigoDGC`? Is its format stable across campaigns, and can it be used at any
   time or only during an active declaration? *Moot: the crop prefill uses the
   public SIGPAC declared-crops data instead (see "Farmer-side data paths"),
   which needs no login.*
7. `UnidadGestora` ("Identificador (CIF, NIE, CIF) de la Unidad gestora" in the
   descriptor sheet): for a holder who uses a commercial notebook directly (no
   entidad habilitada), is it simply the holder's own NIF, i.e. equal to
   `IdTitular`? *Moot while there is no submission path; the export uses
   `owner_tax_id`.*
