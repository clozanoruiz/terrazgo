# SIEX-aligned export — design notes

> Status: design (2026-07-04). No code yet. The user-facing feature name is TBD
> ("SIEX" is too technical for farmers). This document maps Terrazgo's treatment
> domain onto the official CUE exchange format and lists what is missing.

## Sources of truth

| What | Where | Version used here |
| --- | --- | --- |
| Interface spec (methods, auth, envelope) | [FEGA Anexo VI "Interfaz Único Común"](https://www.fega.gob.es/es/siex/documentacion-tecnica-agricola-siex) | v3.3.0 (Dec 2024) — **latest is v3.11.4 (Nov 2025); re-diff before implementing** |
| CUE JSON Schema | Embedded in the Anexo VI docx (Anexo 5); vendored copy: [`references/cue-schema-3.3.0.json`](references/cue-schema-3.3.0.json) | 3.3.0 |
| Field semantics / mandatory flags | `BdcSix-DS-DiseñoCUE.xlsx` (embedded in the same docx, sheet `EstructuraCuadernoWS`) + FEGA Anexo V | — |
| Code catalogues (crops, units, problems, substances…) | FEGA Anexo VII (web) | — |

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
      └─ (Fertilizacion, Cosecha, SiembraPlantacion, … — future modules)
```

A **DGC** ("dato geográfico de cultivo") is the SIEX unit of plot+crop+period.
Activities do not reference plots directly: they reference DGCs, either by the
REA's own `CodigoDGC` (obtained by importing the REA) or by a client-assigned
`CodigoDGCAjena` created via `AltaDGC`.

### `TratamFito` (required: IdAjenaTratamFito, FechaInicio, FechaFin, DGCs, IdentificadorAplicador)

| Descriptor field | Terrazgo source | Status |
| --- | --- | --- |
| `IdAjenaTratamFito` (integer) | — needs an integer alias per `treatment_record` | **gap 1** |
| `Borrar` (bool) | soft-deleted records that were previously exported | ok (derive) |
| `FechaInicio` / `FechaFin` | `application_date` (both = same day) | ✓ |
| `HoraTratamiento`, `FechaSeca` | not captured | optional — omit |
| `DGCs[].CodigoDGC` / `CodigoDGCAjena` | `treatment_plot` → plot+crop | **gap 2** |
| `DGCs[].Superficie` | `treatment_plot.surface_treated_ha` | ✓ |
| `ProblematicaFito.*.Tipo*[]` (codes) | `reason_category_code` + free-text `target_organism` | **gap 3** |
| `Justificaciones[].JustAct` (code) | not captured | optional — omit |
| `ProductosFito[].TipoProducto` (code) | not captured (product kind catalogue) | **gap 3** |
| `ProductosFito[].NumRegistro` | `authorisation_number_snapshot` | ✓ |
| `ProductosFito[].MateriaActivaFormulado[].MateriaActiva` (code) | `active_substance` (name + CAS, no SIEX code) | **gap 3** |
| `ProductosFito[].Dosis` / `Cantidad` / `Unidad` (code) | `dose_value` + `dose_unit_code` | code mapping (**gap 3**) |
| `IdentificadorAplicador[].AplicadorEmpresa.NumROPO` | `operator_licence_snapshot` | ✓ |
| `IdentificadorAplicador[].EquipoAplicador.NumROMA` / `NumREGANIP` | `machinery_roma_snapshot` / `machinery_reganip_snapshot` | ✓ (2026-07-04 fix) — but the descriptor xlsx says a payload must carry **exactly one** of NumROMA / NumREGANIP / IdEquipoAplicador ("nunca ambos"): storing both stays correct, the serializer emits one (ROMA preferred) |
| `AsesorValidacion` (advisor ROPO + validation) | no advisor entity yet | optional — omit |
| `Eficacia` (code) | not captured | optional — omit |
| `Observaciones` | `notes` | ✓ |

Envelope requirements per farm: `CAExplotacion` (CCAA code), `IdTitular`
(titular NIF), `CodigoRea` (REA registration code), `UnidadGestora` — see gap 4.

## Gaps found (ordered by design impact)

1. **Integer activity ids.** `IdAjena*` fields are integers (`number(10)`, max
   9999999999 per the descriptor), not strings — our UUIDv7 TEXT ids cannot be
   sent as-is. The id is the edit/delete key on the
   SIEX side, so it must be *stable across exports*. Design direction: a small
   mapping table (entity id → monotonic integer alias, assigned at first
   export) owned by the export module. Needs schema design.
2. **DGC linkage.** Referencing REA DGC codes requires the REA import
   (`exportarREA`) to be built first, OR we always create our own DGCs via
   `AltaDGC` + `CodigoDGCAjena` (works standalone; risks duplicating what REA
   already has). Ask CUECYL which they prefer for commercial notebooks.
3. **Anexo VII catalogue codes.** Crops, varieties, units, active substances,
   product types, phytosanitary problems and efficacy are *coded* lists. We
   store English enum codes / free text today. Needed: import the Anexo VII
   catalogues as lookup data + add code columns (or mapping tables), and
   capture coded problem/crop choices in the UI at record time (a free-text
   `target_organism` cannot be reliably back-coded).
4. **Farm identifiers.** `IdTitular` (NIF) and `CodigoRea` are required and we
   capture neither (`farm_es_extension` has REGA, which is the *livestock*
   registry — same trap as REGANIP/ROMA). Needs: `rea_code` (+ titular NIF,
   probably on `farm`) — schema design.
5. **Advisor (optional).** `AsesorValidacion` supports GIP advisor sign-off;
   no advisor entity exists yet. Fine to omit; future entity if users need it.

## Suggested build order

1. Anexo VII catalogue study → decide storage (lookup tables per catalogue,
   versioned imports) — this also improves the treatment form (coded problems).
2. Schema additions (gaps 1, 3, 4) — one schema design pass, settled before coding.
3. Export module: query layer (season+farm → snapshots+plots) → serializer to
   the descriptor JSON → validate against the vendored schema in tests
   (JSON-Schema validation crate needed — decide deliberately before adding).
4. File export command (async, like backups) + UI entry point.
5. Server-side WS client — separate component, after developer authorization
   with the Junta exists. Not in this repo's core.

## Open questions for CUECYL (cuecyl@jcyl.es)

1. Procedure and requirements to register as a commercial-notebook developer
   (empresa desarrolladora) in Castilla y León; is an autónomo acceptable?
2. CyL's IUWS endpoint and any CyL-specific documentation.
3. Access to the integration/test environment mentioned in FEGA Anexo VI.
4. Is there any farmer-facing *file* import into CUECYL (manual upload of the
   descriptor JSON), or is the authorized web service the only path?
5. For DGCs: should commercial notebooks reference REA `CodigoDGC` (via
   `exportarREA`) or create their own via `AltaDGC`/`CodigoDGCAjena`?
