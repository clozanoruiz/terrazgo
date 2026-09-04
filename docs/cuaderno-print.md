<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# The printable cuaderno — legal basis and section map

> Added 2026-08-02, when the printable record book became the app's primary
> compliance artifact (no submission path exists — see `siex-export.md`'s
> status banner). This document pins every section of the official model to its
> legal source, records the exact field lists, and classifies what is binding
> content versus form convenience — so schema, report and test decisions can
> cite one place. Companion docs: `data-model.md` (the schema itself),
> `architecture.md` → "The report engine".

## Sources of truth

| What | Where | Notes |
| --- | --- | --- |
| Binding content list | [RD 1311/2012](https://www.boe.es/buscar/act.php?id=BOE-A-2012-11605), art. 16 + **Anexo III Parte I** (consolidated text, checked 2026-08-02) | Art. 16.1: every farm keeps the treatment register with Anexo III Parte I's information; electronic registration mandatory (paper allowed until 2026-12-31 per UE 2023/564 + the 2025/2203 postponement Spain took). Art. 16.2: any record book carrying at least Parte I's data complies — **content is binding, layout is not** |
| Conservation duties | RD 1311/2012 art. 16.3 | Advisory documentation (art. 11.2), equipment inspection certificates, service contracts (Ley 43/2002 art. 41.2.c), invoices and supporting documents, residue-analysis results. **3 years minimum** |
| **Fertilisation + irrigation content** | [RD 1051/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23052) art. 4–6, amended by [RD 934/2025](https://www.boe.es/diario_boe/txt.php?id=BOE-A-2025-21211) (checked 2026-08-07) | A **second decree** feeding the same cuaderno. Art. 5 creates its fertilisation section — **live since 1 Jan 2026**, recorded within one month of each operation — and art. 5.e puts irrigation doses and dates in the same duty. Art. 4.2 + 6 add the plan de abonado from **1 Sep 2026**. Thresholds in the section map below. This is why sections 6, 7.1 and 8 are binding, not optional |
| **National minimum content of the cuaderno digital** | [RD 1054/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23054) **Anexo II**, made binding by art. 9.1 (checked 2026-08-11) | Four items only: (1) crop data per parcela agrícola, from the REA; (2) treatments per RD 1311/2012 anexo III; (3) fertilisation per the sectoral rules; and **(4) "otros aspectos que se recojan en la respectiva normativa sectorial"**. Items 1–3 are what sections 1–8 carry. **Item 4 is an open door, and RD 1048/2022 walks through it** — see the eco-scheme section below |
| **Eco-scheme annotation duties** | [RD 1048/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23048) consolidated, arts. 30–45 + anexo IV (checked 2026-08-11) | Ten clauses ordering the farmer to annotate a practice **in the cuaderno**, mostly within one month. They bind any holding claiming the eco-scheme concerned, and **the app carries none of them** |
| Orientative layout | "Modelo de Cuaderno de Explotación", Junta de Andalucía v6 (2023) — regional reprint of the Comité Fitosanitario Nacional model | The layout our Typst template follows (`crates/terrazgo-recordbook/templates/cuaderno.typ`). Field lists below are transcribed from it. **The model predates RD 1051/2022**, so its "OPCIONAL" heading on section 6 no longer states the law — where the two disagree, the decree wins. It is a *layout* source and never a source of duties: RD 1048/2022's anexo IV duty has no printed page anywhere in it |
| EU baseline | [Reglamento (UE) 2023/564](https://eur-lex.europa.eu/eli/reg_impl/2023/564/oj), art. 1–3 + **annex** (read verbatim 2026-08-11) | Record content for professional users, shared by all member states; applicable **from 1 Jan 2026**, with Spain taking Reglamento (UE) 2025/2203's one-year postponement. Its annex is a three-row table by *type of use* — surface areas, closed spaces, seeds — and asks for two things no Spanish form has a column for: the **start hour** where relevant, and the **BBCH growth stage** where relevant. See "What the EU annex adds" below |
| SIEX exchange twin | `references/cue-schema-3.11.4.json` (vendored) | Named per register below so a future un-parking of the export finds aligned columns |

## Section map — model section → legal basis → status

Classification: **binding** = Anexo III Parte I content, **or** required by
RD 1051/2022 for the fertilisation/irrigation sections; **conditional** =
binding only when the activity happens (the model's "APLICA TRATAMIENTO:
SÍ/NO" checkboxes); **recommended** = in the model but not in Anexo III Parte I
(kept because the model is what inspectors know, and art. 16.3 obliges keeping
the underlying documents).

> **Three decrees feed one book.** RD 1311/2012 governs the phytosanitary
> registers (1.x, 2.x, 3.x, 4); RD 1051/2022 governs fertilisation, irrigation
> and soil (6, 7.1, 8, A.3); RD 1048/2022 governs the eco-scheme annotations
> (section 9), reaching the cuaderno through RD 1054/2022 anexo II item 4. They
> carry different deadlines and different exemption thresholds, and the printed
> model reflects only the first. The third is also the one whose registers must
> be derived from the decree rather than the form — see "The eco-scheme
> registers" below.

| Model section | Legal basis | Class | Coverage |
| --- | --- | --- | --- |
| 1.1 Datos generales | Anexo III A.1.a–b | binding | **complete since 2026-08-02** — identity, both registry numbers (national `siex_code` + autonómico `rea_code`), full postal contact, the titular-o-representante block and the hand-signed signature box |
| 1.2 Personas que intervienen | A.1.c | binding | **complete since 2026-08-02** — NIF, the Piloto carné and the Asesor cross, which is not a carné level but a NIF match against the advisor table |
| 1.3 Equipos de aplicación | A.1.h | binding | **complete since 2026-08-02** — `machinery.acquired_on` prints beside the inspection date; A.1.h accepts either, so equipment needing no ITV is still datable |
| 1.4 Asesor / entidad | A.1.d | binding (when advised) | **complete since 2026-08-02** — `advisor` + `farm_advisor` (name/razón social, NIF, nº de identificación, tipo de explotación as the six GIP siglas) |
| 2.1 Parcelas | A.2.a–f | binding | **Uso SIGPAC + Superficie SIGPAC print since 2026-08-02** (from the provider boundary, beside the user's own figure); Superficie cultivada now blanks on multi-crop plots instead of repeating the plot area. **Secano/Regadío and aire libre/protegido print since 2026-08-02** as the model's four-value siglas, and `crop.area_ha` gives each crop its own surface. **GIP is stated per crop since 2026-08-02** (`crop.gip_system_code`, the full art. 10 framework); a crop that states nothing still falls back to the AE/PI its production system implies, so older books keep printing |
| — soil characteristics | A.3 + RD 1051/2022 art. 5.b | conditional | **complete since 2026-08-09** — the nine `Analitica.ParametrosSuelo` figures on `analysis_record`; not yet binding: A.3's minimum data (pH, P₂O₅, K₂O, organic matter) becomes obligatory one year after MAPA publishes its sampling/analysis guides; heavy metals only when sludge is applied. **RD 1051/2022 art. 5.b already asks for soil organic matter, nutrients and contaminants**, and art. 6 makes soil data an input to the plan de abonado — so it lands with `module-fertilisation`, **extending `analysis_record`** rather than inventing a soil table (`Analitica.ParametrosSuelo`, see below). The Andalucía v6 model predates A.3 and has no soil page |
| 3.1 Registro de actuaciones | B.a–k | binding | **complete since 2026-08-04**: the date is an *interval* when the actuation ran over several days (`application_end_date`; the export's `FechaInicio`/`FechaFin` are both real now) and **B.i's total kg or l used** is captured (`total_quantity_value` + `_unit_code`) rather than derived, because a concentration dose (g/l, ml/l, %) cannot yield one. **The plazo de seguridad is counted from the interval's END** — the plazo is the time between the *last* application and harvest — so `phi_end_date` derives from `application_end_date` when it is set |
| 3.1 bis (cultivos asesorados) | B.d "y, en su caso, del asesor"; the rest is art. 10-11 (GIP) as the model renders it | binding only for advised crops | **complete since 2026-08-09** — and the audit reframed it: **Anexo III Parte I B is ONE list (a-k) covering every treatment**, so this is a second VIEW of `treatment_record`, not a second register. B.d's advisor was the only binding gap; the non-chemical alternative, the justification narrative and the two signature boxes are art. 10-11 compliance as Andalucía presents it. The twin agrees — `AsesorValidacion` and `OtrasActuacionesFito` both hang off `TratamFito`, which does **not** require `ProductosFito`, so an actuation with no product is a first-class record |
| 3.2 Semilla tratada | B.e ("si la siembra se realiza con semilla tratada, indicar el producto") | conditional | **complete since 2026-08-04** — `seed_treatment` + `seed_treatment_plot`. The product is FREE capture (name, nº registro, materia activa) with an optional link to the product registry: treated seed names a product the farmer never bought as such. **The plots come from the printed model, not from SIEX** — `UsoSemillaTratada` carries none at all, and the model is the compliance artifact |
| 3.3 Postcosecha | B.b/B.f ("local o medio de transporte tratado", volume in m³) | conditional | **complete since 2026-08-04** — `non_field_treatment`, one table for all three sections (SIEX: `TratamientosPostCosecha`) |
| 3.4 Locales de almacenamiento | B.b/B.f | conditional | **complete since 2026-08-04** (SIEX: `TratamientosEdifInstalaciones`) |
| 3.5 Medios de transporte | B.b/B.f | conditional | **complete since 2026-08-04** (SIEX: `TratamientosEdifInstalaciones`) |
| 2.2 Medioambiental (agua + zonas) | A.1.e–g (water bodies + abstraction points for human consumption, with distance when outside the parcel; art. 35 zones) | binding | **complete since 2026-08-07**. Zones half (2026-08-02): latest campaign per (plot, zone kind), Totalmente only when every affecting zone covers the whole plot (unknown coverage counts as partial). Water half (`plot_water_point` + `plot_water_declaration`): several points on one plot join positionally across the four cells, so the columns read across. **Both halves print in three states, and for the same reason** — a stated negative ("Sin afección — campaña YYYY", "Sin captaciones — DD/MM/YYYY") is inspection evidence, an unasked plot prints blank, and silence is not the same claim. The model's "Coordenadas UTM" column prints the stored WGS84 lat/lon pair and is relabelled accordingly |
| 4 Análisis de productos fito | art. 16.3 (keep residue-analysis results) | recommended | **complete since 2026-08-04** — `analysis_record` + `analysis_plot`, metadata only: the register says an analysis exists and where its bulletin can be found, never holds the bulletin (SIEX: `Analitica`) |
| 5 Cosecha comercializada | traceability (Ley 43/2002; food-chain rules); not Anexo III Parte I | recommended | **complete since 2026-08-04** — `harvest_record` + `harvest_plot`, in **core**. Neither section carries an "APLICA TRATAMIENTO" line — they are recommended registers, not conditional ones, so no `register_declaration` code backs them (SIEX: `ComercializacionVD`, **not** `Cosecha`) |
| 6 Fertilización | Anexo III **Parte I.C** + **RD 1051/2022 art. 5**, amended by RD 934/2025 | **binding since 1 Jan 2026** | **complete since 2026-08-08** — `fertiliser_material` + `fertilisation_record` and its junctions in `module-fertilisation`. **The model's "OPCIONAL (EXCEPTO ZONAS VULNERABLES)" heading is STALE** (Andalucía v6 is from 2023 and predates RD 1051/2022): this is a NATIONAL duty with a size threshold, not a nitrate-vulnerable-zone matter. Exempt only: ≤5 ha arable+permanent (temporary pastures excluded) **and** ≤1 ha irrigated, or unfertilised pasture-only. **The size exemption is partial, not total** (art. 4.1, checked 2026-08-07): a holding exempt under the ≤5 ha limb that has fertilised pastures, or greenhouses totalling >0,1 ha under cover, must still record **those surfaces**. Recording deadline **one month from each operation**. SIEX: `Fertilizacion` |
| 7.1 Plan de abonado | **RD 1051/2022 art. 4.2 + art. 6** (not merely PAC ecoschemes) | **binding from 1 Sep 2026** | **complete since 2026-08-08** — `fertilisation_plan` carries art. 5.a's four (what the BOOK records); art. 6's plan document is kept, not printed. The table's aportadas and acumuladas are assembled from section 6. Earlier, **1 Jan 2026**, for irrigated production units sown or planted 1 March–30 June. Exempt: unfertilised pasture-only; ≤10 ha secano or fodder for self-consumption. SIEX: `PlanAbonado` |
| 8 Riego | Anexo III C.l + **RD 1051/2022 art. 5.e** | **binding since 1 Jan 2026** | **complete since 2026-08-07** — `irrigation_record` + its junctions in `module-fertilisation`, NOT the Irrigation module: art. 5.e puts *doses and dates of irrigation* inside the SAME cuaderno duty as fertilisation, on the same one-month deadline. SIEX `Riego` requires `SistemaRiego` per EVENT and carries `OrigenAgua` — which finally gives `SIST_RIEGO` and `ORIGEN_AGUA_RIEGO` their consumer and closes the recorded `crop.irrigation_code` gap where it belongs, on the form. The Irrigation module keeps *planning* (schedules, ETo); this is the *record* |
| 9.1 (P1) Pastoreo extensivo | RD 1048/2022 art. 30.2 ter | conditional (holdings claiming the eco-scheme) | **complete since 2026-08-18** — `grazing_record` + `grazing_plot` + `grazing_animal` in `module-ecoscheme`. **The deadline runs from the END of grazing**, so an open record (`ended_on` NULL) is not late and prints a blank end cell rather than an invented date. One printed line per animal group: the model's last three columns describe one group while the dates describe the grazing, so sheep and goats on one pasture are two lines. Column 2 asks for the SIGPAC **reference**, not the table-2.1 cross-reference the other registers use — the plot's name prints instead when it carries no complete reference. `ESPECIE_ANIMAL` vendored with it (SIEX: `Pastoreo`) |
| 9.2 (P2) Siega sostenible / islas de biodiversidad | RD 1048/2022 art. 31 + 31.4.d | conditional | **complete since 2026-08-19** — `cultural_operation` + `cultural_operation_plot` in `module-ecoscheme`, ONE table behind four duties on three pages. **The printed page is a PIVOT of the register**: the model's row is a *plot* (it carries the SIGPAC parts and the surface in columns of their own, like table 2.1) and its cells accumulate dates, so two cuts are one row and one operation on two plots is two. `no_tillage` deliberately does **not** print under "Laboreo" — a date there states the ground was worked. The **Siembra column stays blank until seam 3**: `TIPO_LABOR` publishes no siembra code, and a sowing is its own register. `DEST_RES_VEG` gains its consumer (SIEX: `LaboresCulturales`) |
| 9.3 (P5) Espacios de biodiversidad: cultivos bajo agua | RD 1048/2022 art. 45.2 | conditional | **complete since 2026-08-19, and it prints FIVE date columns where the model prints three.** Art. 45.2 names *"las fechas de nivelación, siembra, inundación y secas, y construcción de caballones"*; the form has no column for nivelación or caballones, so a book following the form would not satisfy the article — the layout is orientativo and the content binds (the PHI-column precedent). The two added columns sit where the ARTICLE names them, which leaves the model's own three in their original relative order. The row is a plot, and its five cells come from **three tables in three crates**: `sowing_record` (core), `treatment_record.drying_date` (module-cue) and `cultural_operation` (module-ecoscheme) — only `terrazgo-recordbook` can read all three. **A plot enters the page on evidence of being a cultivo bajo agua** (a flooded sowing, a `flooded_biodiversity` operation, or a treatment that dried the field); once in, every sowing on it prints, which is what keeps a dry sowing visible in the month before the flooding is annotated |
| 9.4 (P6) Cubiertas vegetales en leñosos | RD 1048/2022 art. 42.1.a, 42.1.c, 42.1.e | conditional | **complete since 2026-08-19** — `soil_cover` + `soil_cover_plot` in `module-ecoscheme`. **The row here is the COVER, not the plot**, unlike 9.2 and 9.3: one establishment date and one pair of widths however many plots it covers, so there is nothing to pivot. Art. 42 is **three annotations on three deadlines**, which the model's single row collapses and the schema therefore splits — `established_on` is the record, the two widths are a nullable all-or-none triple carrying their own `widths_stated_on`, and the maintenance is rows in the registers those activities already belong to. So the three "mantenimiento por medios mecánicos" columns are cross-read: Siega and Desbrozado from `cultural_operation`, Pastoreo from `grazing_record`, both keyed on `soil_cover_id` and resolved once per book. `TIPO_COBERTURA_SUELO` gains its consumer, though **no printed column**: art. 42.1.a annotates the date, not which of the two kinds it was (SIEX: `DatosCubierta`) |
| 9.5 (P7) Cubiertas inertes de restos de poda | RD 1048/2022 art. 43.1.a–b | conditional | **complete since 2026-08-19** — the same `soil_cover` register under `practice_code = 'inert_cover'`, printed three columns shorter because **art. 43 asks for no maintenance at all**. The register refuses a maintenance line against one rather than storing something no page would print. The 15 April limit is advisory work, not a write-time refusal: the book records what happened and does not decide whether an aid was earned (SIEX: `DatosCubierta`) |
| "9.6" pastos comunales | RD 1048/2022 **anexo IV** | conditional | **complete since 2026-08-19, on a page the model does not have.** The same `cultural_operation` rows under `practice_code = 'communal_pasture'`, printed as a subsection the book numbers **9.6** itself — with a footnote saying the official model carries no page for it, because a reader comparing the two must find the difference explained. One row per operation, not 9.2's per-plot pivot: there is no official layout to follow, so the register shape the rest of the book uses is the honest default. Folding it into 9.2 was rejected — that page's footnotes are about P2's two cuts and its 300 m threshold, which would sit wrongly over an anexo IV row. The invoices the annex asks for join the conservation annex (`item_communal_invoices`); the book holds no attachments by design. The clearest proof that duties come from the decree, not the form |
| 10 Determinadas ayudas asociadas | RD 1048/2022 arts. 49.h, 51.e, 53.e, 54.d, 61.4 | recommended | **nothing to build.** The page carries no fields — it redirects to sections 3, 7 and 8, because the aid requires "la aplicación de la gestión sostenible de insumos conforme a las disposiciones normativas vigentes en materia de nutrición sostenible de los suelos agrarios", i.e. RD 1051/2022 compliance, which those sections already record |
| Documentación a conservar (annex page) | art. 16.3 | binding (as a duty, not a table) | **complete since 2026-08-07** — a plain numbered list of the model's seven items plus the three-year retention sentence, never tick boxes (it is a reminder of what to file away, not something the farmer fills in). The book holds no attachments by design (the seam-4 scope decision) |

## What the advisory checks (2026-08-10)

The printed book has no gate: it shows what exists, and fields the model asks
for that the data lacks print blank, because a farmer must be able to print for
an inspection while registry data is still incomplete. That forbids *blocking*,
not *telling* — and until this slice nothing told, since `export_precheck`
serves the parked SIEX export rather than the artifact that carries legal
weight.

`terrazgo_recordbook::advisory::book_advisory` reports ten things, all
advisory, none of them able to stop a print or an export:

| Finding | Source |
| --- | --- |
| Holding address, holder name, holder tax id | Anexo III Parte I A.1.a-b |
| Treated plots with no crop stated | Anexo III Parte I B.e |
| Treatments with no efficacy assessed | Anexo III Parte I B.j (observed after the application, hence advisory here and refused at export instead) |
| Applicators with no ROPO / licence number | table 1.2, and B.d's identification |
| Conditional registers ticked neither SÍ nor NO | the model's own "APLICA TRATAMIENTO" boxes — silence is the one state of the three that says nothing |
| Sections 6 and 8 empty | RD 1051/2022 art. 5.d and 5.e |
| Covers with no widths stated | RD 1048/2022 arts. 42.1.e / 43.1.b |
| Inert covers established after 15 April | art. 43.1.a |
| Live covers with no maintenance recorded | art. 42.1.c |
| Grazings still open on a closed campaign | art. 30.2 ter |

It takes `today` as a parameter rather than reading the clock, the
`refresh_alerts` precedent, because one of the four section-9 checks is a date
rule and a rule that cannot be tested is not pinned.

### Section 9's checks are record-triggered, and that is the design (2026-08-20)

The four eco-scheme findings each key off a record the farmer **chose to
create**, so none of them reaches a holding outside the regime. There is
deliberately **no `SectionGap` and no "section 9 is empty" finding**: the app
cannot know which eco-schemes were claimed in the solicitud única, so an empty
section 9 is the normal state of most holdings. It is the plan-de-abonado
precedent and stronger, since here the duty itself is per claimed practice.

Four routes to a claim were checked and all are closed — recorded so the
question is not re-derived. The Nube de SIGPAC OGC API publishes five
collections and none is an eco-scheme layer; `cultivo_declarado` carries
solicitud-única aid *lines* (`CL`, `VI`, `PT`), not practices; FEGA's
287-catalogue registry publishes no practice list at all; and the CUE exchange
schema models activities, not entitlements. **The design does not preclude
it**: `practice_code` sits on every record, so a holding-level "claimed P6,
recorded nothing" finding would be one advisory field away.

Two of the four are worded carefully, because the obvious wording would be a
false statement:

- **Missing widths are not late.** Art. 42.1.e falls due after 42.1.a, so a
  cover between the two deadlines is a *complete* record with an annotation
  still to make. `widths_stated_on` is what makes the two distinguishable at
  all — it is why that column exists, though no source asks for it.
- **An open grazing is not late either.** The month runs from the END of
  grazing (art. 30.2 ter, and the model's own 9.1 footnote), so the honest
  finding is that the book cannot show the annotation *finished* — and only
  once the campaign has closed. A season with no `ends_on` says nothing about
  that and produces no finding.

The 15 April boundary is derived from the record's **own** `established_on`
year, never from `season.label`, which is free text and spans two calendar
years anyway. And there is **no communal-pasture invoice check**: anexo IV asks
for invoices kept as evidence, and the book holds no attachments by design.

### Why it never says "exempt"

RD 1051/2022 art. 4.1 exempts a holding with **≤5 ha** of permanent crops and
arable land (pastures excluded) **and ≤1 ha** irrigated, or one that has only
pasture and does not fertilise it. Read verbatim, though, the article **carves
that back**: a holding exempt under a) still records for

1. pastures on which fertilisers ARE applied, and
2. **invernaderos con superficie total bajo cubierta superior a 0,1 ha**.

The first carve-back is unknowable for exactly the holding being advised — one
that records nothing tells the app nothing about what it applies — so the
advisory reports a verdict of `possibly_exempt`, never `exempt`, worded as
"check it, the exemption has conditions the app cannot see". The greenhouse
clause IS checkable (`crop.growing_environment_code`, or SIGPAC's own `IV`
use), and it is evaluated **first**, because it survives the exemption rather
than qualifying it.

The third verdict, `undetermined`, fires when any plot has no SIGPAC land use
or no area. Land use reaches us only from a *verified* boundary, so a holding
that never ran the SIGPAC check cannot be measured — and saying so beats
excusing it. Where the data is merely ambiguous the rule leans the other way,
toward reporting the duty: a temporary pasture sown on arable land counts
toward the 5 ha, because SIGPAC calls that plot `TA` and nothing we hold says
otherwise. Inviting a farmer to check a rule that may not apply to them is the
cheap error; assuring one who is bound that they are not is the expensive one.

**The plan de abonado is deliberately absent** from the advisory. Art. 6's duty
is exempted per *unidad de producción* (art. 4.2), a unit the schema does not
model — a plan covers a set of crops — so an advisory over it would either nag
every smallholder or excuse a large one.

## The eco-scheme registers (found 2026-08-11, built by 2026-08-20)

A third decree writes into the cuaderno. RD 1054/2022 anexo II ends with *"otros
aspectos que se recojan en la respectiva normativa sectorial"*, and **RD
1048/2022** is that sectoral norm for anyone claiming an ecorrégimen: ten
clauses ordering an annotation **in the cuaderno de explotación agrícola**, most
of them within one month of the activity. A completeness audit found them
missing; the arc that followed built all six duties, and the table below stayed
as its spec.

It is ordered by **decree and article**, with the model's pages in the last
column rather than as the organising principle — because the duties exist
whether or not a form prints them, and one of these has no printed page at all.

| Article | What must be annotated | Deadline | Model page |
| --- | --- | --- | --- |
| 30.2 ter | the new grazing start/end dates, when they change from those in the solicitud única | 1 month from the new date | 9.1 |
| 31 | *"la fecha y las actividades realizadas"* — pastoreo, siega para producción o mantenimiento, or any other maintenance activity of anexo III.B | 1 month | 9.2 |
| 31.4.d | *"las labores de siega realizadas"* | 1 month after mowing | 9.2 |
| 42.1.a | *"la fecha de establecimiento de la cubierta vegetal espontánea o sembrada con presencia viva sobre el terreno"* | 1 month | 9.4 |
| 42.1.c | *"el tipo de mantenimiento que realiza sobre la cubierta"* | within the month before the solicitud-única modification period ends | 9.4 |
| 42.1.e | *"la anchura de la cubierta y la anchura libre de la proyección de copa"* | within the month before the end of the 4-month live-cover period | 9.4 |
| 43.1.a | *"la fecha de establecimiento de la cubierta inerte"*, which may not be later than 15 April | 1 month | 9.5 |
| 43.1.b | the same two widths | within the month before the modification period ends | 9.5 |
| 45.2 | *"las fechas de nivelación, siembra, inundación y secas, y construcción de caballones"* | 1 month per activity | 9.3 |
| **anexo IV** | the dates of maintenance activities on each **pasto comunal** plot, with the invoices kept as evidence | 1 month | **none** |

Three things the printed form hides, and a schema derived from it would have
lost — each of which the built registers had to answer:

1. **Anexo IV's duty has no page.** The model prints five eco-scheme
   sub-registers; the decree carries six duties. Reading the form would have
   missed the sixth entirely — the book prints it as a **"9.6"** of its own.
2. **Art. 42 is three annotations with three different deadlines**, collapsed
   into one row of columns. A single "cover" record with one date would not
   satisfy it, which is why `soil_cover` splits into a record, a nullable
   all-or-none width triple and rows in two other registers.
3. **The cuaderno is the primary evidence route, not a convenience.** Farmers
   who do not use it fall back to a paper register they must custody plus
   georeferenced photographs on a 1 % sample of beneficiaries.

A fourth surfaced while building: **art. 45.2 names five dates and model 9.3
prints three**, so the book prints five — the layout is orientativo and the
content binds, the PHI column precedent.

**Two of RD 1048/2022's duties are already satisfied**: arts. 35.2 and 45.7 ask
for a plan de abonado plus *"las operaciones de aporte de nutrientes y materia
orgánica al suelo agrario y de agua de riego"* in the cuaderno — sections 6, 7.1
and 8.

**Negative results, so they are not re-derived.** RD 1047/2022 (gestión y
control) names the CUE as a system and creates no annotation duty. RD 1049/2022
(condicionalidad reforzada) has one cuaderno clause and it redirects to RD
1051/2022: *"todas las operaciones encaminadas a aportar nutrientes o materia
orgánica al suelo deben estar correctamente registradas en el cuaderno de
explotación"*, plus the plan de abonado — already carried.

**The vocabularies needed nothing invented, and now all have consumers.**
`TIPO_COBERTURA_SUELO`, `TIPO_LABOR` and `DEST_RES_VEG` were vendored and read
by nothing; the three registers gave them readers, and `ESPECIE_ANIMAL` was
vendored with the grazing one. `RAZAS` is published and deliberately **not**
vendored — neither model 9.1 nor `Pastoreo.Animales[]` asks for a breed
(`maintenance.md` §1 records the negative). Two owned lookups carry what FEGA
publishes no list for: `eco_practice` (the six duties) and
`cultural_operation_kind`, whose 15 codes map onto `TIPO_LABOR`'s 14 with a
deliberately non-injective pair, pinned in both directions so a new upstream
code makes somebody look. The SIEX twins are `Pastoreo`, `DatosCubierta` and
`LaboresCulturales`, all three now captured and none emitted
(`siex-export.md`).

## What the EU annex adds (Reglamento (UE) 2023/564)

Read verbatim 2026-08-11. The annex is a table of **three types of use** —
treatment of surface areas, treatment of or in closed spaces, treatment of seeds
or plant reproductive material — against seven columns. Two of its cells ask for
data no Spanish form has a column for, and both are **conditional**:

- **Start time (hour)**, in the surface-areas row only: *"Date and where
  relevant start time (hour)"*, footnote 4 defining relevance as *"when the use
  of plant protection product is restricted to specific times of the day or when
  the time of use is relevant in the context of the particular use."* The other
  two rows ask for the date alone — so sections 3.2, 3.4 and 3.5 owe nothing.
- **Growth stage in line with the BBCH monograph**, in the surface-areas and
  closed-spaces rows, and placed inside the **"Crop or situation/land use"**
  column — so it belongs to the treated crop, which is `treatment_plot`, exactly
  where the exchange format puts it (`TratamFito.DGCs[].EstadoFenologico`).
  Footnote 7 mirrors footnote 4: relevant when the product's use is restricted
  to particular growth stages. The vendored `EST_FENOLOGICO` carries the BBCH
  principal stage in a column of its own, so it is the picker.

Neither is in RD 1311/2012 anexo III parte I B, so the duty arrives from the EU
regulation alone — which does not make it optional. **Both are built (2026-08-12)**:
`treatment_record.application_time` (local wall-clock `HH:MM`, deliberately not
UTC — this is a time *of day*, no timezone is stored anywhere, and a UTC
round-trip would print back an hour the farmer never recorded) and
`treatment_plot.growth_stage_code`.

Three things about the growth stage are worth keeping. **The catalogue's code is
not the BBCH stage**: FEGA numbers `EST_FENOLOGICO`'s rows 1-10 in `Código SIEX`
— which is what a record stores, because the twin validates `EstadoFenologico`
against the catalogue — and publishes the monograph's own 0-9 beside them in
`Estadio bibliografía`. Every reader resolves through
`module_cue::catalogue::growth_stage`, which returns both renderings; printing
the stored code would misstate the monograph by one everywhere. **The register
cell prints the NUMBER, not FEGA's wording**, because the annex asks for the
stage "in line with the BBCH monograph" and the monograph's identifier is the
number, while the catalogue's labels are whole sentences ("Desarrollo de las
partes vegetativas cosechables de la planta o de órganos vegetativos de
propagación/ embuchamiento") that wrapped one row of the 15-column landscape
register to fourteen lines — found by rendering the page and looking at it. That
is the division the model's own siglas already use, and the spreadsheet keeps the
full name. **And the stage belongs to the treated crop, so one printed row can
carry several**: a row groups plots sharing a species and variety, and an
actuation spanning two days can catch them at different stages, so the cell lists
every distinct one ("BBCH 4 / 5"). Naming the first would state something false
about the others.

Of the two further annex points, one is met and **one turns out not to be**.
The seed row's *"batch number, where applicable"* is `seed_treatment.seed_lot`,
captured since 2026-08-04. But **crop names following EPPO codes** (art. 1.3,
which puts the correspondence on the Member State) is only partly derivable, and
the earlier "worth measuring before relying on it" was the right instinct.
Measured 2026-08-12 over the vendored `PRODUCTOS`: **151 of its 1023 active rows
carry no EPPO code at all**, and the gap is structural rather than an omission —
EPPO codes a plant taxon, and much of the catalogue is not one. `BARBECHO
TRADICIONAL` and `BARBECHO MEDIOAMBIENTAL` are fallow, `PASTOS PERMANENTES DE 5 O
MÁS AÑOS` is a land use, `FLORES` is a generic group, and `TRANQUILLÓN` is a
wheat-rye mixture naming two taxa. Those rows can never acquire a code, which is
consistent with the annex heading its own column "Crop or situation/**land
use**". So nothing derives an EPPO code today: a derivation that silently
succeeded 85% of the time and presented itself as complete would be worse than
none. The count is pinned by a contract test in `terrazgo-core`, exact on
purpose, so a catalogue refresh that moves it makes somebody look.

Two framing notes, neither a defect. Art. 2 requires the records **kept in a
machine-readable format** (Directive (UE) 2019/1024 art. 2(13)): the app
satisfies that by storing them at all, and the spreadsheet is the portable form
— a PDF is not machine-readable in that sense, so "the printed book is the
compliance artifact" holds for *inspection* under RD 1311/2012 art. 16.2 without
being the whole answer. Art. 3 requires recording *without undue delay* and
transfer into electronic format within 30 days, relaxed before 2030 to "before
31 January of the year following the year of use" — an in-app record meets it on
entry.

## Exact field lists (transcribed from the model)

Footnote code lists are part of the form — they are the closed vocabularies the
schema needs.

### 1.1 Datos generales de la explotación

Fecha de apertura del cuaderno · Nombre y apellidos o razón social · NIF ·
Nº Registro de Explotaciones **Nacional** · Nº Registro de Explotaciones
**Autonómico** · Dirección · Localidad · C. Postal · Provincia · Teléfono fijo ·
Teléfono móvil · e-mail. **Titular o representante:** Nombre y apellidos · NIF ·
Dirección · Localidad · C. Postal · Provincia · Tipo de representación ·
Teléfono · e-mail. Signature box (person signing answers for the data's
veracity) + date.

**All of it prints from data since 2026-08-10.** Three cells used not to:
"Fecha de apertura del cuaderno" printed a ruled line with no field behind it
(now `farm.opened_on` — the book is a continuing document for the holding, so
the date belongs to the farm, with the campaign printed beside it, and an
unstated one keeps the ruled line rather than inventing a date); the
representative's "Provincia" printed a label with no value expression at all
(now `farm_representative.province`, free text like the address lines it sits
with — coding it would put a Spanish code list in a core table, and a company
representative may sit outside Spain); and the holding's own "Provincia"
printed `farm_es_extension.province_code` **verbatim**, so a farmer who typed
`47` got a bare number in a legal document. It now resolves against the vendored
`PROVINCIA` catalogue when the stored value reads as an INE province number, and
prints what the farmer typed when it does not — the catalogue-label rule, with
the one refinement that an unresolved code prints their own string rather than
the zero-padded form ("7" must not silently print as "07").

### 1.2 Personas o empresas que intervienen

Nº de orden · Nombre y apellidos / Empresa de servicios · NIF · Nº inscripción
ROPO / nº carné · Tipo de carné: **Básico / Cualificado / Fumigador / Piloto**
(cross marks) · **Asesor** (separate cross column — advisor capacity is not a
carné level; ROPO registers applicators and advisors as different conditions).

### 1.3 Equipos de aplicación

Nº de orden · Descripción del equipo (tipo, marca y modelo) · Nº inscrip. ROMA
(when ROMA registration is not obligatory: the reference number in the
corresponding census — REGANIP) · **Fecha de adquisición** · Fecha de la última
inspección.

### 1.4 Asesor, agrupación o entidad de asesoramiento

Nombre o razón social · NIF · Nº de identificación · **Tipo de explotación**
(the GIP framework of art. 10): **(AE)** Agricultura Ecológica · **(PI)**
Producción Integrada · **(CP)** Certificación Privada · **(Atrias)** Agrupación
de Tratamiento Integrado en Agricultura · **(AS)** Asistida de un asesor ·
**(NO)** Sin obligación de disponer de asesor en GIP.

### 2.1 Datos identificativos y agronómicos de las parcelas

Nº de orden (correlative, grouping parcels under the same management) ·
SIGPAC: Código Provincia · Término municipal (código y nombre) · Código
Agregado · Zona · Nº Polígono · Nº Parcela · Nº Recinto · **Uso SIGPAC** ·
**Superficie SIGPAC (ha)** · Agronómico: Superficie cultivada (ha) · Especie ·
Variedad (GMO varieties add "OGM") · **Secano/Regadío**: **(SEC)** secano ·
**(ASP)** aspersión · **(LOC)** goteo o localizado · **(GRA)** por gravedad ·
**Aire libre o protegido**: **(AL)** aire libre · **(M)** malla · **(BP)**
cubierta bajo plástico · **(INV)** invernadero · **GIP** (same six codes as
1.4, per row).

> The "Secano/Regadío" column is *not* a boolean: the form's own footnote makes
> it a four-value irrigation-method code, matching A.2.e's "secano o regadío
> (indicando en su caso el sistema de riego)".

**"Término municipal (código y nombre)" prints both since 2026-08-11.** It had
printed the bare code: the SIGPAC reference parses `municipio` as a number and
the provider returns no name in any response, so the stored value is all the
book had — the same defect slice D fixed for the holding's Provincia one column
to the left, that pass having corrected the farm header and left the plot table
alone. `MUNICIPIO_SIGPAC` (8 434 rows) is now vendored and the cell reads
"122 · POLLOS".

Three rules, and the first is the one that matters:

- **The province is part of the key, not context.** Municipality codes are
  unique only *within* a province — 001 is Alegría-Dulantzi in Álava and Adalia
  in Valladolid — so the lookup is qualified by `Código de provincia`, which is
  also the catalogue's `identity_attrs`. A plot that states no province gets **no
  name**, never the first of 52 candidates. A test drops the qualifier to watch
  a Valladolid plot print Álava's town.
- **Both parts arrive as stored**: a verified plot holds `10`, not `010`,
  because the reference parses its parts as numbers, while the catalogue
  zero-pads to three. Anything that does not read as a number, and any code the
  snapshot cannot resolve, yields no name — so the cell falls back to the code
  alone (the `problem_code` rule) and the book still renders with no snapshot
  at all.
- **The PDF joins them, the workbook keeps them apart.** The model has one
  column, so the printed cell carries "código · nombre"; the sheet gains a
  *Municipio (nombre)* column of its own, because a joined string can be read
  but not filtered (the 2.2 captaciones precedent). The 2.1 column takes a
  width share rather than `auto`, since upstream names run to 67 characters
  and would otherwise push a 17-column table off the page.

The **province** column beside it keeps printing the code, deliberately: 2.1
asks for "Código Provincia", where 1.1 asks for "Provincia".

### 2.2 Datos identificativos medioambientales

Id. parcelas ("TODAS" allowed) · Cultivo (Especie, Variedad) · Puntos de
captación de agua para consumo humano: Incluido en la parcela (SÍ/NO) ·
Distancia (m, when outside the parcel) · Coordenadas UTM (voluntary) ·
Denominación · Parcelas en zonas específicas (art. 35): Totalmente (SÍ/NO) ·
Parcialmente (SÍ/NO, hectares affected if known).

### 3.1 Registro de actuaciones fitosanitarias

Id. parcelas ("TODAS" allowed) · Cultivo (Especie, Variedad) · **Intervalo de
fechas** (interval or single date) · Superficie tratada (ha) · Problema
fitosanitario · Aplicador (order nº from 1.2) · Equipo (order nº from 1.3) ·
Producto (Nombre comercial / Sustancia activa · Nº Registro · Dosis kg/ha o
l/ha) · Eficacia (buena / regular / mala) · Observaciones.

**Reglamento (UE) 2023/564's annex adds two more, both conditional and both
captured since 2026-08-12**: the **start hour** of the application (surface
treatments only, "where relevant") and the crop's **BBCH growth stage** ("where
relevant", and it attaches per treated crop, not per record). Neither has a
column in the model, so each folds into the cell the annex itself places it in —
the hour behind the date, the stage behind the species — with the page's
footnotes (2) and (3) saying so. The spreadsheet gives each a column of its own.
See "What the EU annex adds" above.

Anexo III B adds,
beyond the model's columns: **total kg or l of product used** (B.i) and
machinery registration number (B.h — we already print ROMA/REGANIP).

### 3.1 bis Registro por parcela (cultivos objeto de asesoramiento)

Cultivo (Especie, Variedad) · Id. parcelas · Superficie cultivada / tratada ·
Plaga · Justificación de la actuación (thresholds, weather…) · **Alternativas
no químicas** (Tipo de medida · Intensidad — nº de trampas, nº de difusores ·
Fecha) · **Alternativas químicas** (Nombre comercial / Sustancia activa · Nº
registro · Dosis · Fecha) · Eficacia · Observaciones. Two validation boxes:
**VALIDACIÓN INTERMEDIA** and **VALIDACIÓN FINAL** (Firma · Asesor · Nº
inscripción ROPO · Fecha / Fecha fin de campaña).

### 3.2 Registro de uso de semilla tratada

"APLICA TRATAMIENTO: ☐SÍ ☐NO" · Fecha de siembra · Id. parcelas · Cultivo
(Especie, Variedad) · Superficie sembrada (ha) · Cantidad de semilla (kg) ·
Producto fitosanitario (Materia activa / Nombre comercial · Nº registro).
SIEX twin additionally carries a seed lot number (`NumeroLote`).

### 3.3 / 3.4 / 3.5 — one structure, three subjects

"APLICA TRATAMIENTO: ☐SÍ ☐NO" on each · Fecha · *subject* · Problemática
fitosanitaria · *quantity* · Producto (Nombre comercial · Nº Registro ·
Cantidad utilizada, kg o l). The subject/quantity pairs: **3.3** producto
vegetal tratado / Cantidad (t) · **3.4** local tratado (tipo y dirección) /
Volumen (m³) · **3.5** vehículo tratado (tipo, modelo y matrícula) / Volumen
(m³). The SIEX twins additionally require coded problems, justifications,
applicator and efficacy.

**These three registers are Anexo III Parte I B**, not a separate list that
resembles it: B.b identifies what was treated as "la parcela, **o en su caso,
local o medio de transporte tratado**", and B.f asks for "el volumen tratado
expresado en metros cúbicos" *como tratamiento de locales* — the model's own
subject and quantity columns, straight out of the annex.

**B.b's word is "identification", which is why 3.4 and 3.5 gained a registry
(2026-08-20).** The subject cell had been free text retyped on each record, and
that identifies nothing — two treatments of one warehouse can spell it
differently and nothing ties them together, so the book cannot answer "what was
done in this store this year". Core's `premises` is the identity; the printed
cell is composed from it (name + address for a local, name + model + plate for
a vehicle) and frozen onto the record, so correcting a store's address never
rewrites what a past record states. The link is **nullable**: a farmer who has
not built the registry yet still records a lawful treatment, and it is the SIEX
export's precheck that demands it — the `efficacy_code` precedent. A
postharvest record names no premises at all, because it treats produce.

**The registry reached the interface on 2026-08-21**, which is what makes it an
identification rather than a table: `RegistryPremises.svelte` is the catalogue's
sixth section (farm-scoped, like machinery), and each register offers only the
kind its own page prints — 3.4 the buildings, 3.5 the vehicles. Choosing one
replaces the free-text field rather than sitting beside it, because the printed
cell is composed in Rust and a second editable field would be asking twice; the
free text stays for a farmer who has registered nothing yet, with a line
pointing at the catalogue. The registry entry also carries two fields the
*printed* model never asks for and Anexo V does — a cadastral reference and
FEGA's class of building — which are recorded on the registry row and
deliberately kept out of the composed cell (`data-model.md` → `premises`).

So B.d ("identificación
del aplicador **y, en su caso, del asesor**") binds here as it does in 3.1, and
the register carries the advisor even though the printed model shows no column
for it (2026-08-10). The PDF folds the pair into the applicator cell the model
does give — B.d names the two in one breath — and the workbook splits them into
an *Asesor* and a *Nº ROPO asesor* column, the analysis-kinds precedent.

### 4 Registro de análisis

Fecha · Material analizado (**vegetal / tierra / agua**) · Cultivo o cosecha
muestreados (parcel order nºs) · Nº boletín de análisis · Laboratorio (nombre
y dirección) · Sustancias activas detectadas.

### 5 Registro de cosecha comercializada

Fecha · Producto · Cantidad (kg) · Nº de orden parcela/s de origen · Nº de
albarán o factura (voluntary) · Nº de lote (voluntary) · Cliente: Nombre o
razón social · NIF · Dirección · Nº de RGSEAA (voluntary).

### 6 Registro de fertilización

Fecha (puntual o intervalo) · Referencia SIGPAC (Prov. · Mun. · Pol. · Par. ·
Rec., substitutable by the 2.1 order number) · Sup. (ha) · **S/R** · Cultivo
(Especie, Variedad) · Producción (kg/ha): **Estimada** / **Final** · Tipo de
abono/producto · **Nº de albarán** · **Riqueza N/P/K** · **Dosis N (kg/ha,
m³/ha)** · Tipo de fertilización: **(F)** fertirrigación · **(AF)** abonado de
fondo · **(AC)** abonado de cobertera · Observaciones.

Footnote (0) of the model states a **15-day** recording deadline. RD 1051/2022
art. 4.1 sets **one month**, and the decree is later and national — the book
follows the decree.

### 7.1 Plan de abonado

Id. parcelas · Cultivos (Especie, Variedad) · Caracterización de la aplicación:
Fecha · Superficie fertilizada (ha) · Descripción del fertilizante: Nombre
comercial · N · P₂O₅ · K₂O · Dosis de fertilizante (kg/ha) · Unidades
fertilizantes **APORTADAS** (N, P₂O₅, K₂O) · **ACUMULADAS** (N, P₂O₅, K₂O) ·
**RECOMENDADAS** (N, P₂O₅, K₂O). Footnote (2): UF are kg/ha of N, of P₂O₅ and
of K₂O.

> **Only the RECOMENDADAS block is new data.** Every other column of this table
> is section 6's own record seen again: aportadas = dose × riqueza, acumuladas
> = their running sum per plot. Capturing them twice would let one book state
> two different totals, so 7.1 is **assembled, not captured** — what is stored
> is the recommendation, which is exactly what RD 1051/2022 art. 5.a asks the
> cuaderno to carry.

### 8 Riego

Id. parcelas · Superficie regada (ha) · **Sistema de Riego** · Fecha/Intervalo
de riego · **Volumen de riego (m³/ha)** · **Volumen acumulado (m³/ha)**.
Footnote (1) is the irrigation-system list, and it is **`SIST_RIEGO` verbatim**:
1 Superficie o Gravedad · 2 Aspersión fija · 3 Aspersión móvil ·
4 Microaspersión · 5 Nebulización · 6 Goteo · 7 Hidroponía a solución perdida ·
8 Hidroponía con recirculación.

> Note this is **not** the four-value SEC/ASP/LOC/GRA vocabulary of 2.1. The
> two answer different questions — 2.1 characterises the *plot* (A.2.e), 8
> records the system used for *this irrigation* — which is why the recorded
> `crop.irrigation_code → SIST_RIEGO` gap was never closable on the crop.
> Volumen acumulado is a running sum, assembled like 7.1's.

### Section 9 — the eco-scheme registers (transcribed 2026-08-17)

Not in any doc before the arc that builds them. Numbering is the model's; the
sixth duty has no page and the book gives it one.

**9.1 (P1, art. 30.2 ter)** — Id. del grupo de parcelas⁽¹⁾ · Referencia SIGPAC
de la parcela o grupo⁽²⁾ · Fecha inicio de pastoreo · Fecha fin de pastoreo⁽³⁾ ·
Especie animal que pasta · REGA · N.º animales desplazados al pasto.
⁽¹⁾ completed only for plots more than 10 km from the main livestock
installation; ⁽²⁾ plots less than 10 km apart may be treated as a group;
⁽³⁾ the annotation is due within one month of the **END** of grazing.

**9.2 (P2, art. 31)** — Id. de parcelas · Provincia · Municipio · Polígono ·
Parcela · Recinto · Superficie SIGPAC (ha) · Siega: fecha de cortes⁽¹⁾ · Otras
actividades: Laboreo⁽²⁾ / Siembra⁽³⁾ / Otras activ. de mantenimiento⁽⁴⁾.
⁽¹⁾ two cuts a year, threshold 300 m altitude; ⁽²⁾⁽³⁾ indicate a date;
⁽⁴⁾ indicate a date **and** the activity.

**9.3 (P5, art. 45.2)** — Id. Parcelas · Fecha de siembra en seco · Fecha de
inundación · Fecha de seca para tratamiento herbicida o fitosanitario.
**Two of art. 45.2's five dates have no column** — nivelación and construcción
de caballones — so the book adds them: the layout is orientativo, the content
binds (the PHI-column precedent). As built, the printed order is the ARTICLE's
— nivelación · siembra en seco · inundación · seca · caballones — which leaves
the model's own three in their original relative order.

**9.4 (P6, art. 42)** — Id. Parcelas · Fecha de establecimiento⁽¹⁾ · Anchura de
la cubierta (m) · Anchura libre proyección copa (m) · Mantenimiento por medios
mecánicos: Siega / Desbrozado / Pastoreo⁽²⁾. ⁽¹⁾ live spontaneous or sown
cover; ⁽²⁾ slopes ≥ 10 % and bancales.

**9.5 (P7, art. 43)** — Id. Parcelas · Fecha de establecimiento⁽³⁾ · Anchura de
la cubierta (m) · Anchura libre proyección copa (m). ⁽³⁾ inert cover from
trituración of pruning residue.

**"9.6" (anexo IV)** — no printed page exists. The book's own: Id. de parcelas ·
Parcelas · Fecha · Fecha fin · Actividad de mantenimiento, with a footnote
stating that the official model carries no page for the duty.

**Section 10** carries no fields — it redirects to sections 3, 7 and 8. Nothing
to build, but the page prints one sentence saying so, or a reader concludes the
book skipped a section.

> **No "APLICA TRATAMIENTO: SÍ / NO" box appears anywhere in section 9.** The
> `register_declaration` mechanism does not extend here and must not be
> extended: a farmer claiming no ecorrégimen is not declaring the register
> empty, they are outside the regime — which the section's intro says in prose.
> Every section 9 table therefore uses `blank_rows: 6`, never `0`.

### Anexo III Parte I sección C — the binding list for 6 and 8

RD 1051/2022 art. 5.d **and** 5.e both redirect here ("la información requerida
en la sección C de la parte I del anexo III del Real Decreto 1311/2012 … según
indica el anexo I"), so this — not the printed model — is the field list that
binds. Transcribed from the consolidated BOE text, checked 2026-08-07.

Recorded **per unidad homogénea de cultivo**: one crop, one titular, one
sistema de explotación (secano/regadío).

| | Field | Note |
| --- | --- | --- |
| a | Fecha de la aplicación | |
| b | Superficie en que se realiza | |
| c | Tipo de tratamiento | enmienda (orgánica, cálcica…), abonado de fondo, abonado de cobertera → **`TIPO_FERITILIZACION`** exactly |
| d | Tipo material empleado | 1º producto fertilizante (RD 506/2013 anexo I type, or Reg. 2019/1009 functional category) · 2º estiércol sólido, indicando especie · 3º purín, indicando especie · 4º otros materiales (RD 1051/2022 anexo VIII) → **`MAT_FERTI`** + **`DETALLE_MATERIAL_FERT`** |
| e | Supplier, for d.2/d.3/d.4 | nombre de la empresa suministradora + **REGA** (livestock holding) · **NIF** (centro de gestión de estiércoles) · **NIMA** (gestor de residuos) — mutually exclusive |
| f | Forma de aplicación | "en particular si es por fertirrigación, especificando si es por aspersión, localizada, etc." → **`METODO_APLICACION_FERTILIZANTE`** |
| g | Máquina empleada | **explicitly optional**, with its registration number where applicable |
| h | **Valor agronómico del material** | N total · N orgánico · N ureico · N nítrico · N amoniacal · P₂O₅ total · P₂O₅ soluble en agua · K₂O total — **eight values**, per the label, the material certificate or the art. 13.2 document accompanying manure |
| i | Lodos only | heavy-metal content per RD 1051/2022 anexo IV tabla A.1 |
| j | Dosis | cantidad del producto o material aplicado **por hectárea** |
| k | Empresa de servicios | when the applicator is not the holding's own, with its **REGFER** registration number |
| l | Regadío only, subject to art. 17 | contenido de N nítrico en el agua de riego · contenido de P₂O₅ soluble en el agua · **cantidad de agua aportada en cada riego (m³/ha)** |

Three consequences the printed model does not show:

- **C.h is eight values, not three.** The model's "Riqueza N/P/K" is a subset.
  C.i adds heavy metals on top whenever sludge is applied, so the composition
  cannot be three columns.
- **C.k names a third machinery registry, REGFER** (created by RD 1051/2022
  art. 18), alongside ROMA and REGANIP.
- **C.l's two water-quality values have no column in the model and no field in
  the `Riego` twin.** They surface in SIEX as
  `Fertilizacion.Fertirrigacion.DosisN`/`DosisP`. **art. 17.2 makes them
  conditional**: obligatory only when the organismo de cuenca, comunidad de
  regantes or equivalent supplies the data, and voluntary when the holder
  analyses the water themselves — so they are captured permissively, never
  demanded.

### Documentación a conservar (annex page, 3 years)

Facturas/documentos de adquisición de fitosanitarios · contratos con empresas
o personas que realizaron tratamientos · certificados de inspección de los
equipos · justificantes de entrega de envases vacíos · boletines de análisis
de residuos (cultivos, producciones y, en su caso, agua de riego) ·
documentación del asesoramiento · albaranes o facturas de venta de la cosecha.

> **The model's list is wider than art. 16.3** (checked against the
> consolidated BOE-A-2012-11605, 2026-08-07). The article itself names the
> register of art. 16.1, the art. 11.2 advisory documentation, equipment
> inspection certificates, the Ley 43/2002 art. 41.2.c contracts, invoices and
> other supporting documents, and residue-analysis results — all kept **at
> least three years from their issue**. **Empty-container return receipts** and
> **harvest sale delivery notes** are NOT in it: the first answers the
> container-return duty, the second food-chain traceability (section 5's own
> basis). The page prints all seven, because the model is what an inspector
> knows, and its retention sentence cites art. 16.3 for the three years while
> naming the other two footings rather than implying the article covers them.

## Capture design (settled 2026-08-02)

The schema that data-backs the sections above. Landed **per slice** (each
slice edits the pre-release `0001` files for its own tables and ships schema +
repository + tests + UI + print together); the composed sequence stays one
runner (`architecture.md`). The first `0001` edit of this arc also adds a
schema-shape probe to `terrazgo_core::backup::validate_backup` — pre-release
squashing does not bump `user_version`, so a stale backup must be rejected by
comparing its table/column fingerprint against a freshly migrated in-memory
database instead of importing cleanly and failing later.

That probe **composes like the migrations do** (2026-08-04): core owns its own
fingerprint, a module contributes its tables through `validate_backup`'s
`module_shape` argument (`module_cue::BACKUP_SHAPE`), and the shell passes the
two together. Core naming a module's table would have inverted the dependency
direction for a string constant. Every pre-release edit to a module's `0001`
extends that module's list.

### Existing tables

| Table | New columns (nullable unless said) | Feeds |
| --- | --- | --- |
| `crop` | `area_ha`; `irrigation_code` → `irrigation_system`; `growing_environment_code` → `growing_environment`; `gip_system_code` → `gip_system` (**landed 2026-08-02**); `crop_code` (PRODUCTOS catalogue code, TEXT, **no FK** — same rationale as `treatment_problem.problem_code`); provenance `source` (NOT NULL DEFAULT `'user'`), `source_campaign`, `declared_area_ha` (**all landed 2026-08-03**) | 2.1 agronomic columns; per-crop surface ends the repeated-plot-area output; provenance backs the SIGPAC declared-crops prefill (`sigpac-integration.md` → "Declared crops") |
| `operator` | `tax_id` | 1.2 NIF |
| `machinery` | `acquired_on` | 1.3 |
| `farm` | `address`, `postal_code`, `phone_fixed`, `phone_mobile`, `email` | 1.1 (universal → core) |
| `farm_es_extension` | `siex_code` (Nº Registro Nacional; `rea_code` stays the autonómico) | 1.1 |
| `treatment_record` | `application_end_date` (interval end; export's `FechaFin` falls back to the start date); `total_quantity_value` + `total_quantity_unit_code` → `unit` (Anexo III B.i — not derivable from concentration doses; the form prefills dose × surface for per-ha doses only, and offers nothing for a concentration dose). **All three landed 2026-08-04** | 3.1 |
| `seed_treatment` | `treatment_kind_code` → `seed_treatment_kind` (**landed 2026-08-05**; nullable, because the printed model has no such column) | 3.2, and the twin's required `Tratamiento` |
| `analysis_record` | its material list re-coded to FEGA's four (`plant` → `crop` + `harvested_produce`), plus the `analysis_record_type` and `analysis_substance` junctions (**all landed 2026-08-05**) | 4 |
| `harvest_record` | `crop_code` **renamed** `plant_product_code` — it always meant PROD_VEGETAL, not PRODUCTOS (**2026-08-05**) | 5 |
| `farm_representative` (new, 1:0..1) | `farm_id` PK/FK; name, tax_id, representation_kind, address, locality, postal_code, phone, email — reconciled from submitted state like `farm_es_extension` | 1.1 titular-o-representante |

### New lookups (i18n keys only)

`irrigation_system` {rainfed, sprinkler, drip, gravity} ·
`growing_environment` {open_air, mesh, plastic_cover, greenhouse} ·
`gip_system` {organic, integrated_production, private_certification, atria,
advisor_assisted, not_required} (**landed 2026-08-02**) ·
`analysis_material` {crop, harvested_produce, soil, water} ·
`analysis_type` {pesticide_residues, microbiological, heavy_metals, nutrients,
soil_parameters, gmo_presence} ·
`seed_treatment_kind` {on_farm, processing_centre, purchased_es,
purchased_abroad} (**the three landed 2026-08-05**, each bijective against its
FEGA catalogue and pinned by a contract test) · `licence_level` gains
`pilot` · `unit` gains
a `quantity` dimension {kg, l, t, m3} (**landed 2026-08-04**; a quantity is an
amount, not a rate, so `list_units` — the dose picker — excludes them and
`list_quantity_units` is their own list). The Spanish siglas (SEC/ASP/LOC/GRA,
AL/M/BP/INV, AE/PI/CP/Atrias/AS/NO, Básico…Piloto) are print-template
content, not schema values. 1.2's "Asesor" cross is **not** a carné level: it
prints when the operator's NIF matches an advisor row.

### New core tables (audit-logged, soft-deleted)

- `advisor` — name, tax_id, **`registration_number`** (**landed 2026-08-02**;
  the design said `ropo_number`, but core tables carry no regional identifiers —
  the `operator.licence_number` precedent — and the model's own label is the
  neutral "Nº de identificación"). `farm_advisor` — farm ↔ advisor with
  `gip_system_code` (1.4's "tipo de explotación"), one ACTIVE link per pair so
  restating a relationship updates it instead of duplicating the 1.4 row;
  deleting an advisor detaches its links in the same transaction. 2.1's per-row
  GIP comes from `crop.gip_system_code`, deriving AE/PI from
  `production_system` when unset. The advisor entity is standalone, not
  farm-scoped: one entity advises many holdings, and 1.2's Asesor cross is a
  NIF match against the whole registry — the claim is what the person *is*,
  not which holding they were advising.
- `plot_water_point` — plot FK, denomination, `inside_plot` (bool),
  `distance_m`, lat/lon (voluntary). 2.2's water half (A.1.f–g); the zones half
  reads from `plot_zone_flag`. **Landed 2026-08-07**, with four calls worth
  recording. **(a) Printed-model-only, verified rather than assumed**: the SIEX
  3.11.4 schema has NO captación entity at any level — its one water field,
  `OrigenAgua`, sits under `Riego`/`Fertirrigacion` and codes the provenance of
  *irrigation* water — and the live FEGA registry's four water catalogues
  (`ORIGEN_AGUA_RIEGO`, `USOS_AGUA`, `REGANTES`, `COMU_REGA`) all belong to that
  same irrigation vocabulary. So there is no twin to mirror and no code to
  carry; the requirement here is the decree's, not the interface's (seam 3 and
  seam 4's finding, a third time). **(b) `distance_m` is REQUIRED when the point
  lies outside** (`invalid.missing_distance`) and refused when it lies inside
  (`invalid.water_point_distance_inside`) — A.1.g asks for it in that case, and
  unlike efficacy or a total quantity used it is knowledge the farmer already
  has, so the "capture permissively, complain at export" precedent does not
  apply; a distance beside "included: SÍ" is a wrong answer, not a missing one.
  **(c) Flat and per plot, not a normalized point + junction**: `inside_plot`
  and `distance_m` describe the *(plot, point)* pair, so a well serving two
  plots would need a junction carrying both anyway — it is entered once per plot
  it concerns, which is exactly what the model's per-plot row states. Real
  geometry stays out until the Irrigation module wants it, where it belongs in
  `geo_feature`. **(d) Coordinates are stored as WGS84/ETRS89 lat/lon**, what the
  whole app already speaks (SIGPAC queries `recinfobypoint/4326/…`,
  `geo_feature` geometry is 4326 GeoJSON, and the importer treats 4326/4258/4081
  as one identity class); the model's "Coordenadas UTM" heading is relabelled
  "Coordenadas (lat, lon)" rather than converting behind the farmer's back into
  a projection nothing else uses. A UTM rendering can be added later from the
  same two numbers with no schema change.
- `plot_water_declaration` — plot FK, `declared_on`, one live row per plot
  (partial unique index). The stored negative for 2.2's water half: an empty
  register looks exactly like an unfilled one, and only the first is evidence
  the farmer asked. **The invariant runs both ways**, as in `register_declaration`
  (whose *shape* this copies but not its table — that one is module-cue's and
  farm+season scoped, this is core, per plot and season-less): declaring a plot
  free of points while it holds them is refused (`invalid.plot_has_water_points`),
  and recording a point **withdraws the declaration in the same transaction**,
  because a stale "no captaciones" printing beside a contradicting row would
  forge proof-of-check. Withdrawal is a soft delete and restating mints a new
  row, so the trail keeps saying what the farmer once declared.
- `harvest_record` + `harvest_plot` — season + farm scoped; date, product
  name, **quantity value + unit code** ({kg, t} — the "value + unit code,
  never free text" convention, and SIEX `ComercializacionVD` carries a coded
  `Unidad`; re-audit 2026-08-04), albarán/lote refs, buyer (name, tax id,
  address, `buyer_registry_number`). Core, not module-cue: harvest is
  whole-farm data (costs, analytics), and modules never depend on each other —
  which is also why the column is the neutral `buyer_registry_number` and not
  `rgseaa_number`: core tables carry no regional identifiers, so the Spanish
  label "Nº RGSEAA" lives in the report labels and the UI dictionaries. For
  the same reason the unit is **not** an FK (`unit` is a module-cue lookup);
  the {kg, t} set is enforced in the repository.

  NB our `harvest_record` is the SIEX `ComercializacionVD` twin, not `Cosecha`
  — the latter is the field operation, which is out of scope.

  **Landed 2026-08-04.** The re-audit of `ComercializacionVD` found it carries
  **no plot array and no buyer block at all** — so `harvest_plot` and the whole
  client block exist because the *printed model* asks for them ("Nº de orden
  parcela/s de origen", "Cliente… Nº de RGSEAA"). That is seam 3's situation
  repeating, and it is written down here so it does not read as an oversight.
  The twin also requires **FechaInicio + FechaFin**, an interval; the record
  stores a **single `harvested_on`**, because the model prints one date column
  and section 5 is not Anexo III Parte I content — a serializer satisfies the
  twin by sending the same date as both ends, the fallback the export already
  used for treatments before seam 1. `ProductoVegetal` is a coded integer, which
  is why a code sits verbatim beside the free `product_name`, and `kg` (5)
  and `t` (6) both exist in `UNIDADES_MEDIDA`, so the enforced set maps cleanly.

  **Correction (2026-08-05) — it is not the crop catalogue.** The column was
  named `crop_code` and documented against `PRODUCTOS`. `PROD_VEGETAL` is a
  separate catalogue, and the field descriptor points `ProductoVegetal` (and
  `Cosecha.ProductoCosechado`) at *"Catálogo de Producto vegetal"*. The two
  answer different questions about the same plant: PRODUCTOS codes the crop
  (101 OLIVO), PROD_VEGETAL the harvested produce (1 Aceitunas) — and the
  file states the relation itself, one row per (produce, crop) pair, so
  *Aceitunas* appears for both OLIVO and ACEBUCHE. `harvest_record.crop_code`
  was therefore renamed **`plant_product_code`** on 2026-08-05: `crop.crop_code`
  and `seed_treatment.crop_code` really do mean PRODUCTOS, and two identical
  names against different catalogues is the trap the naming rule exists for.
  It is not `product_code`, because in module-cue `product_*` always means the
  phytosanitary product. The UI carried the same error live —
  `BookHarvest.svelte` offered `SpeciesPicker`'s **crop** values for this
  field — so the fix was a picker, not a comment: `CataloguePicker.svelte`
  now holds the type-ahead both fields share, `SpeciesPicker` wraps it with
  its SIGPAC land-use narrowing, and `PlantProductPicker` wraps it over
  `PROD_VEGETAL` (208 offers deduped from 692 rows). Section 3.3's
  `subject_product_code`, which nothing had ever populated, gets the same
  picker — shown for postharvest only, since the other two subjects are a
  building and a vehicle.

### New module-cue tables (audit-logged, soft-deleted)

- `non_field_treatment` — one table for 3.3/3.4/3.5: `subject_kind_code`
  {postharvest, storage_premises, transport}, `treated_on`,
  `subject_description` (producto vegetal / local tipo y dirección / vehículo
  tipo, modelo y matrícula), an optional `subject_product_code` for the
  postharvest kind (**re-audit 2026-08-04**: SIEX
  `TratamientosPostCosecha.ProductoVegetal` is a catalogue code, not free text
  — stored verbatim with no FK, the `problem_code` rule; **corrected
  2026-08-05**: the catalogue is `PROD_VEGETAL`, not `PRODUCTOS`, and nothing
  populated the column yet, so commit 2 wires the same picker section 5
  gets), treated
  quantity + unit (t | m³ — the repository enforces the pairing, since
  recording a warehouse in tonnes is a different claim, not a unit slip; both
  quantities are nullable pairs, because the printed form leaves the cell
  hand-fillable and a format requirement belongs in an export precheck, the
  efficacy precedent). **Same discipline as `treatment_record`, matching the
  SIEX twins**: coded problems
  (≥1) and justifications (≥1) in `non_field_treatment_problem` /
  `_justification` junctions, `operator_id` NOT NULL with name + licence
  snapshots, `efficacy_code` nullable (observed after the fact; an
  audit-logged setter, demanded by print/export prechecks, never at insert).
  Product: FK + name/registration snapshots + quantity used (kg | l).
- `seed_treatment` + `seed_treatment_plot` — 3.2: sown_on, species, variety,
  surface sown (ha), seed quantity (kg), seed lot (SIEX `NumeroLote`), product
  name + registration nº + active substance (free capture — supplier-treated
  seed is often not in the product registry; optional `product_id` when it is),
  and a nullable `efficacy_code` with the same assessed-later setter as
  `treatment_record` (**re-audit 2026-08-04**: SIEX `UsoSemillaTratada` lists
  `Eficacia` as required). **Landed 2026-08-04.** Unlike every other register
  here this record is **fully correctable** (`update_seed_treatment`, with the
  sown plots reconciled from the submitted state): it holds no snapshot of
  another row, so there is nothing a later edit elsewhere could rewrite — which
  is precisely the condition the `*_snapshot` columns exist to handle.

  Its `ProductosFito[].TipoProducto` is the `TIPO_PRODFITO` code our
  `authorisation_kind` lookup already owns, which a serializer can supply
  without new storage.

  **Correction (2026-08-05).** The twin's required `Tratamiento` integer was
  recorded here as having "no catalogue in the vendored FEGA set", and that
  claim was about our snapshot, not about FEGA: **`TIPO_TRATAMIENTO` exists**
  ("Tratamiento semilla") and is now vendored. Four values, starting at 2:
  2 realizado en la explotación · 3 realizado en un centro de acondicionamiento
  · 4 adquisición de semilla tratada con producto autorizado en España ·
  5 adquisición de semilla tratada fuera de España. **Landed 2026-08-05** as
  the tier-1 `seed_treatment_kind` lookup and a NULLABLE
  `seed_treatment.treatment_kind_code`: the printed model has no such column,
  so a book kept to the model alone must not be blocked on it, while a stated
  value has to be one the export can speak. It rides in the PDF's product cell
  and has its own spreadsheet column. The descriptor also states two
  cross-field rules a future export precheck owes: `NumeroLote` is required
  when the kind is 4 or 5, and `NumRegistro` is enabled only for 2 or 3.
- `analysis_record` + `analysis_plot` — 4: sampled_on, `material_kind_code`
  {crop, harvested_produce, soil, water}, bulletin nº, lab name + address, **lab tax id**
  (re-audit 2026-08-04: SIEX `Analitica.Nif`), substances detected.
  **Landed 2026-08-04**, fully correctable for the treated-seed reason (it
  holds no snapshot of another row's identity). The re-audit of `Analitica`
  settled four things:

  * `MaterialAnalizado` is a required **integer code**, `TiposSustancias[]`
    and `TiposAnalisis[]` are **coded arrays**. All three were recorded here
    as having no catalogue in the vendored FEGA set — **wrong, and corrected
    2026-08-05**: `MATERIAL_ANALIZADO`, `SUST_ACTIVAS` and `TIPO_ANALISIS` all
    exist upstream and are now vendored. See the correction block below.
  * The twin carries a `Nif` but **no address**, while the printed model asks
    for "Laboratorio (nombre y dirección)" — so both columns exist and the PDF
    joins the three fields into the model's single cell, skipping whatever is
    blank. The model is the compliance artifact.
  * `Analitica.ParametrosSuelo` carries pH, materia orgánica, P, K, N, texture
    and conductivity — i.e. **Anexo III A.3's soil minimums live inside
    `Analitica`**, not in a block of their own. A.3 stays deferred to the
    Fertilization & soil module, but that module **extends `analysis_record`**
    rather than inventing a soil table.

  No file attachments, and that is a standing scope decision rather than an
  omission: the app has no attachment capability, and giving it one has
  backup, sync and mobile-storage consequences of its own. The farmer keeps
  the bulletin; the annex page (seam 6) is what says so.

  **Correction (2026-08-05) — the catalogues exist.** FEGA publishes 287
  catalogues; we had vendored 16, and this document three times mistook
  "absent from our snapshot" for "not published by the authority". All three
  are now vendored (docs/maintenance.md §1 has the registry that makes this
  checkable in future):

  * `MATERIAL_ANALIZADO` — **four** values, where our lookup had three:
    1 Cultivo · 2 Producto cosechado · 3 Suelo · 4 Agua de riego. So the
    three-value list was not merely incomplete, it was *wrong*: FEGA
    distinguishes the standing crop from the harvested produce, and `plant`
    conflated them. **Landed 2026-08-05** as `crop` / `harvested_produce` /
    `soil` / `water`, tier 1 with `analysis_material_to_siex`. The report
    prints FEGA's own wording rather than the model's parenthetical hint
    *(vegetal / tierra / agua)*, which cannot express the split.
  * `SUST_ACTIVAS` — 283 substances **with their CAS numbers**. Tier 2 (code
    stored verbatim, no FK) not because 283 is large but because CAS is the
    cross-country key a future French or Italian export would match on.
    `substances_detected` **stays** alongside the coded junction: SUST_ACTIVAS
    only codes phytosanitary actives (`TipoAnalisis` 1), so a heavy-metals,
    nutrients or soil-parameter bulletin has no code there and would otherwise
    become unrecordable. **Landed 2026-08-05** as `analysis_substance`, whose
    code is stored verbatim with no FK *and accepted even when the vendored
    snapshot cannot resolve it* — the snapshot rides app releases and a
    laboratory does not wait for one. The PDF joins the resolved names to the
    free text in the model's single cell; the sheet keeps them in two columns.
  * `TIPO_ANALISIS` — 6 values, including 5 "Parámetros del Suelo", which
    independently confirms the `ParametrosSuelo` finding above. **Landed
    2026-08-05** as the `analysis_type` lookup + `analysis_record_type`
    junction (tier 1, bijective mapping), because vendoring a catalogue with
    no consumer is the same mistake in the other direction.

  Neither junction shows in the printed model, which has no column for either:
  the kinds of analysis ride in the material cell and the coded substances in
  the substances cell, and both get a **spreadsheet column of their own**,
  where a value can be filtered instead of read.
- `register_declaration` — the explicit "APLICA TRATAMIENTO: **NO**" per
  (farm, season, register {seed_treatment, postharvest, storage_premises,
  transport}), a partial unique index on the triple where `deleted_at IS NULL`.
  SÍ derives from rows existing; the stored NO is deliberate proof-of-check
  (the `plot_zone_flag` negative philosophy). **Landed 2026-08-04** with the
  invariant enforced in both directions: declaring a register empty while it
  holds records is refused (`invalid.register_has_rows`), and inserting a
  record into a register already declared empty **withdraws the declaration in
  the same transaction** — the record is the stronger statement, and a stale NO
  printed beside it would be a contradiction in a legal document. Withdrawal is
  a soft delete, so the audit trail keeps saying the farmer once declared it,
  and restating mints a new row rather than resurrecting the old one.

  So a register prints in **three** states, not two: rows tick SÍ, a declaration
  ticks NO, and an untouched register ticks neither — "nobody filled this in"
  is not the same claim as "nothing happened".
- **3.1 bis needs no tables of its own** (settled 2026-08-09, after reading
  Anexo III Parte I B verbatim). The apartado is one lettered list a-k covering
  *every* treatment, and B.d reads "Identificación del aplicador **y, en su
  caso, del asesor**" — the advisor is a field on the treatment, in the same
  sentence as the applicator. Nothing in the decree asks for a separate
  register, a non-chemical column or a signature. The SIEX twin models it the
  same way: `AsesorValidacion` and `OtrasActuacionesFito` are members of
  `TratamFito`, whose required set (`IdAjenaTratamFito`, `FechaInicio`,
  `FechaFin`, `DGCs`, `ProblematicaFito`, `Justificaciones`,
  `IdentificadorAplicador`, `Eficacia`) pointedly **omits `ProductosFito`**.

  So `treatment_record` grows three blocks and 3.1 bis is printed as a filtered
  view of it:

  - **the advisor** (`advisor_id` + `advisor_name_snapshot` +
    `advisor_registration_snapshot`), snapshotted like the applicator so
    correcting an advisor's ROPO number never rewrites a past record;
  - **the non-chemical measure** — `measure_code` against
    **`TIPO_MEDIDA_FITOSANITARIA`**, its intensity as a value + unit pair, and
    the measure's own registration number (the twin's `NumRegistroMDF`);
  - **the chemical block made nullable**, so an actuation may be a product
    application, a measure, or both — never neither.

  **The catalogue is not the one the earlier sketch named.**
  `MEDIDA_PREVENTIVA_CULTURAL` backs a different question entirely: it hangs
  off `DatosExplotacion` (beside the parked `AltaDGC`/`CambioCultivoDGC`) and
  declares which IPM practices the HOLDING follows — "rotación de cultivos",
  "asesoramiento por un asesor en GIP" — with no date, no plot, no intensity,
  and no column anywhere in the printed model. It stays consumer-less, recorded
  in `siex-export.md`'s dormant inventory rather than built.

  **The chemical block is nullable as a UNIT**, under a table CHECK that admits
  only all-present or all-absent. Six columns going nullable on the register
  with the highest legal risk is the real cost of this design, and the CHECK is
  what buys most of it back: without it a product could be stored with no
  `phi_end_date`, and a product application that raises no PHI alert is a
  *silent* wrong answer rather than a visible gap. Both PHI readers filter
  `phi_end_date IS NOT NULL` in SQL, and tests pin the rule in both directions
  — the failure that would matter is a missing alert, not a spurious one.

  **The intensity is a count, and counts are prose.** `UNIDADES_MEDIDA`
  publishes Trampas, Trampas/ha, Difusores, Difusores/ha and Unidades, so the
  `unit` table gains an `intensity` dimension with its own picker (a number of
  traps is neither a dose nor an amount of product). Unlike `L/ha`, which reads
  the same in every language, "trampas" has to translate — so it comes from
  `Labels`, not from `unit_symbol`, and the guard that every seeded unit prints
  *something* now covers both renderings.

  **The two validation boxes store nothing.** They sit once per sheet, they ask
  for a handwritten Firma, and the book has no signature capability by design
  (the 1.1 signature-box rule). The page prefills Asesor and Nº ROPO from its
  own rows — and only when every advised row names the same advisor, because
  putting one of two names against a signature nobody gave would be the book
  asserting what it cannot know.

  **The "Justificación de la actuación" column is already captured, coded.**
  The model leaves it free text; we hold the `treatment_justification` rows the
  twin requires, and the cell prints their resolved words. No second free-text
  field for the same fact.

  **Which rows appear** is derived from the record — it carries an advisor or a
  measure — rather than from a flag on the crop. Nothing has to be kept in sync
  for that to stay true, and a crop whose treatments name no advisor simply
  contributes nothing, which is a true statement about what was recorded.

### New module-fertilisation tables (design settled 2026-08-07)

Sections 6, 7.1 and 8 land in their own crate, `module-fertilisation`,
registered after module-cue. The reasoning for a second module rather than
growth in module-cue or core is the placement rule in `architecture.md`: this
is a **domain with its own logic** (dose arithmetic, unidades fertilizantes,
plan-versus-applied), not shared data and not presentation.

**Two structural moves land first, as a separate pure-refactor step**, because
both are prerequisites and neither adds behaviour:

- **`unit` moves from module-cue to terrazgo-core.** module-fertilisation
  needs dose and volume units, and a module may never depend on another
  module. Promoting the lookup keeps module-cue's five foreign keys working
  (module → core is the permitted direction), gives `harvest_record` the real
  foreign key its own comment records it cannot have, and gives the new module
  one too. A measurement vocabulary is universal, not a treatment concept —
  the same argument that moved farm/plot/season to core. Pre-release makes the
  move free.
- **`terrazgo-recordbook` gets its own error type.** Slice A left the book
  returning `module_cue::Result`; a document that reads two modules and
  returns one of their error types is plainly wrong. `RecordbookError` wraps
  `CoreError`, the module errors and `ReportError`, and the shell's
  `CommandError` boundary downcasts it — so the `error.*` keys and the
  `i18n_contract` test are in scope for that step.

New units the sections need: `m3_ha` and `t_ha` (dose rates), and `m3` already
exists as a quantity.

- `fertiliser_material` — the **registry**, reusable across a campaign:
  commercial/material name, `material_code` (`MAT_FERTI`),
  `material_detail_code` (`DETALLE_MATERIAL_FERT`), supplier name and the
  mutually exclusive `supplier_rega` / `supplier_tax_id` / `supplier_nima`
  (C.e), `manure_treatment_code` (`TRAT_ESTIERCOLES`), density and its unit.
  Soft-deleted and audit-logged like `product`, and for the same reason: a
  farmer applies one fertiliser many times a season, and C.h hangs eight
  agronomic values off it — retyping those per application is where wrong data
  comes from.
- `fertiliser_material_nutrient` — the composition, as **one coded junction**
  covering all three SIEX arrays: `(material_id, kind, code, percentage)` with
  kind ∈ {macro, micro, heavy_metal} against `MACRONUTRIENTES`,
  `MICRONUTRIENTES` and `METALES_PESADOS`. Codes stored verbatim without a
  foreign key (the catalogue rule). One table rather than eight columns
  because C.h asks for eight values, C.i adds heavy metals for sludge and
  micronutrients exist — a fixed column set can carry none of the last two.
  The PDF joins N total / P₂O₅ total / K₂O into the model's single "Riqueza
  N/P/K" cell; the workbook gets a tab with one row per value, where it can be
  filtered and summed.
- `fertilisation_record` + `fertilisation_plot` — the event (SIEX
  `Fertilizacion`): `applied_on` + `application_end_date` (C.a, and art. 5.f
  explicitly allows fortnightly accumulation for intensive or fertigated
  crops), `fertilisation_type_code` (C.c), `application_method_code` (C.f),
  dose value + unit (C.j), optional `machinery_id` (**C.g says optional in so
  many words**), service company + its `regfer_number` (C.k),
  `sludge_application` boolean (C.i / art. 5.g), delivery-note reference (the
  model's "Nº de albarán"), estimated and final yield (the model's "Producción
  kg/ha"), notes. Material by FK **plus name and composition snapshots**, the
  Legal value capture rule. The plots junction carries the treated surface per
  plot, exactly like `treatment_plot`.
- `irrigation_record` + `irrigation_plot` — SIEX `Riego`: `irrigated_on` +
  `irrigation_end_date`, `irrigation_system_code` (`SIST_RIEGO`, **per event** —
  the model's own §8 footnote is that catalogue verbatim), volume value + unit
  (C.l's m³/ha), optional `energy_type_code` (`TIPENERGIA`) and `meter_number`,
  and the two conditional water-quality values from C.l — `water_nitric_n` and
  `water_soluble_p2o5`, nullable under art. 17.2. Water origin
  (`ORIGEN_AGUA_RIEGO`) is a junction because the twin's `OrigenAgua` is an
  array: one irrigation can mix sources.
- `fertilisation_plan` — SIEX `PlanAbonado`, and its required set is **exactly
  RD 1051/2022 art. 5.a's list**: expected yield (`ObjetivoProduccion`),
  preceding crop (`CultivoPrecedente`, a `PRODUCTOS` code), the N / P₂O₅ / K₂O
  requirements, and the plan's generation date. Scoped to the production unit,
  which for us is the crop row — the same unit the SIEX export already treats
  as the DGC. `Herramienta` (whether a calculation tool produced the plan) is a
  twin-only boolean and is captured, since it costs one column and the export
  requires it.

7.1's aportadas, acumuladas and 8's volumen acumulado are **assembled, never
stored**: they are sums over the records above, and a stored copy is a second
number that can disagree with the first (the "do not store derived values"
rule). A.3's soil parameters extend `analysis_record` in module-cue, per the
seam-4 finding — no soil table.

### What the section-6 re-audit added (2026-08-08, as built)

Re-reading the `Fertilizacion` twin before writing the code corrected the
design above in four places, and recorded two gaps rather than building them.

- **The record snapshots the material's coded kind as well as its name.** The
  twin carries `AplicacionMaterialFertilizante.NombreProducto` as a plain
  string beside the coded material, which confirms the name snapshot — but the
  model's own "Tipo de abono/producto" column prints C.d's *kind*, and C.d is
  a binding field. A record that named a manure must go on saying so after the
  registry entry is corrected, so `material_code_snapshot` freezes with it.
- **REGFER lives in two places, and the decree's is the one we store.** C.k
  attaches the REGFER number to the *empresa de servicios*; the twin splits
  them, carrying `EmpresaServicios` on the application and `NumREGFER` inside
  `EquipoAplicador`. Both are stored on the record, together, and a serializer
  places them where the twin wants.
- **`EquipoAplicador` has a `oneOf`** (ROMA / REGANIP / an applicator id),
  unlike the non-field twin's no-required-members block. Since C.g makes the
  machine optional, an application without one omits the whole block — which
  the `oneOf` permits and a half-filled block would not.
- **The printed cell for "Tipo de fertilización" carries both legal fields.**
  The model's footnote lists (F) fertirrigación beside (AF) and (AC) as though
  they were one list; they are not, so the book derives the sigla — "F/AC" for
  a fertigated cobertera, blank for an enmienda, which the model gives no
  letter — and spells out the forma de aplicación beside it. `is_fertigation`
  is stored on the lookup rather than matched on the code, so the derivation
  reads from data.

Three things were recorded here as deliberately not built, and **seam 3 built
two of them on 2026-08-21** — because the rule this paragraph stated was too
coarse, and correcting it is worth more than either field.

- **`Fertirrigacion`** is BUILT. The original reasoning — "the model has no
  fertigation columns, `application_method_code` already records *that* it was
  fertigation, and the water side is §8's" — missed that the sub-block is the
  **only reader anywhere in the format** of C.l's two water-quality figures,
  which `irrigation_record` captures and no printed column and no member of
  `Riego` carries. Not building it left two columns of a *binding* Anexo III
  letter with no consumer at all. It is filled through
  `fertilisation_record.irrigation_record_id`, which the farmer sets on the §6
  form when the method is a fertigation.
- **`GestionSostInsu`** is BUILT, as `sustainable_input_management`.
- **`BuenasPracticasRiego`** is still a recorded gap, and now for a reason that
  survives: it is **Voluntario** in Anexo V on both blocks, not merely optional
  in the schema.

**The rule that decided them was wrong, and the corrected one is:** the test is
not "required in the twin" — that is the JSON Schema's `required`, which is only
*structural validity of an entry you chose to send*. It is **Anexo V's own
`OBLIGATORIEDAD` column**: a field FEGA marks `Obligatorio` inside a block we do
send is a real requirement even when no decree names it, and a field it marks
`Voluntario` and no page prints is a recorded gap. `BuenasPracticas` is captured
under both readings; `GestionSostInsu` only under the corrected one. The three
questions this untangles — required, obligatorio and binding — are set out in
`docs/siex-export.md`.

**Code `0` is exclusive (2026-09-01).** Every ámbito of
`BUENAS_PRACTICAS_AMBITOS` opens with `"0";"No realiza buenas prácticas"`, an
ordinary row that the file's shape says nothing special about — so a record
could hold it beside the other forty, claiming at once that nothing was done and
what was done. `validated_practices` refuses that pair, and the form never offers
it. **This is not the case `soil_cover`'s "the picker narrows, never the
repository" rule protects**: that rule exists because the catalogue *grows*
between releases and refusing an unlisted code would make a lawful practice
unrecordable. Here both codes are known, published and fixed in meaning, and a
register that can hold the contradiction exports it as two
`BuenaPracticaFertilizante` entries. The section stays optional throughout — an
empty set and a bare `0` are both legal answers, and only the pair is refused.

### A.3's soil block and the annex additions (2026-08-09)

**The soil block lives on `analysis_record`, in module-cue.** Two separate
reasons, and they point the same way. The twin settles the first:
`Analitica.ParametrosSuelo` is a sub-object OF an analysis, because soil data
reaches a holding as a laboratory bulletin like any other. The module boundary
settles the second: `analysis_record` is module-cue's table and a module may
never add columns to another module's schema — so although the *consumer* of
soil data is the fertilisation domain (RD 1051/2022 art. 5.b, and art. 6 makes
it an input to the plan), the columns belong to the crate that owns the
register, and the record book reads across both. The earlier note that this
"lands with `module-fertilisation`" was imprecise on that point.

Nine figures, the twin's own: pH, materia orgánica, P and K asimilables, N
total, conductividad, and **texture as three fractions** (arena / limo /
arcilla) rather than a class name. All nullable — A.3's minimums bind only a
year after MAPA publishes its sampling guides, and a bulletin reports what was
asked for. Units are fixed by the column name (`soil_available_p_mg_kg`), the
`water_nitric_n_mg_l` precedent: safe here, where a farmer copies a figure into
a labelled field, in a way it would not be when importing a provider's number.
The one arithmetic rule: **the three texture fractions must sum to 100** when
all three are given, ±1 for a lab's rounding — they are fractions of one soil.

The printed model predates A.3 and has no soil page, so the figures ride in
section 4's findings cell with a footnote saying so, and take a **"4 Suelo"**
workbook tab where each is a column of real numbers.

**The annex page gains the second decree's three documents**, verified against
the BOE on 2026-08-09: the plan de abonado itself (art. 6 — seam 3 deliberately
left this here), the *documento de aplicación de los lodos* issued by the
authorised manager (art. 5.g, anexo III of Orden AAA/1072/2013), and the
agronomic-quality document that must accompany manure received from a third
party (art. 13.2 — **explicitly not needed when the holder supplies their
own**, which the printed item says). The retention sentence was rescoped in the
same pass: it now says the three years of art. 16.3 cover items 1–6, names the
two that rest on other footings, and states that RD 1051/2022 sets no retention
period of its own for items 8–10 — rather than letting one citation appear to
cover ten items it does not.

### Section 7.1 as built (2026-08-08)

Checking art. 6 against the BOE before coding drew the line the whole seam
rests on: **art. 6 defines a DOCUMENT and art. 5.a defines the RECORD.** The
plan itself must identify every recinto of the production unit, carry soil
parameters, account for rainfall and available irrigation, give the recommended
dose of each nutrient with its moment, material, form of application and
machinery, and describe the anexo V emission measures — and it is drawn up
(with advice, once art. 6.6's transition elapses) and kept. What goes in the
book is art. 5.a's four: *rendimiento esperado, cultivo precedente, necesidades
de N, de P₂O₅ y de K₂O y fecha de elaboración del plan*. `fertilisation_plan`
is art. 5.a, and the SIEX `PlanAbonado` required set is the same four —
the twin agreeing with the article is the confirmation.

Consequences worth keeping:

- **The plan covers a production unit, not a parcel.** `PlanAbonado.DGCs` is an
  array and art. 4.2 says "por cada unidad de producción", so the covered crops
  are a junction — and the repository keeps a crop in at most one live plan,
  because two plans recommending different nitrogen for one crop would make
  7.1 print two different figures on one row.
- **A plan is correctable by design**, not by concession: art. 6 explicitly
  allows adjusting it during the campaign to follow the crop and the weather,
  so `drawn_up_on` moves with the correction.
- **The table is assembled.** Only the recommendation is stored; aportadas =
  dose × riqueza and acumuladas = their running sum per production unit, both
  computed from section 6's own records (`module_fertilisation::agronomy`).
- **A volume dose needs the material's density to become unidades
  fertilizantes**, and when it is missing the cell is blank — never a guess of
  1 kg/L. More: **the running total stops there and stays blank**, because a
  total that silently omitted a slurry application would read as the nitrogen
  already applied, and a farmer comparing it against the recommendation would
  over-fertilise on the strength of it.

### Filling C.h from the catalogue (2026-08-08)

`DETALLE_MATERIAL_FERT` publishes the composition of each of its 1243 named
products, so choosing one fills the material's composition instead of asking a
farmer to copy fourteen figures off the sack. Explicitly asked for by a button,
and a line already entered is never overwritten: the label in the farmer's hand
is the source of truth, and the vendored snapshot rides app releases.

**What is NOT filled matters more than what is.** Reading the file before
mapping it turned up three traps:

- **The seven heavy-metal columns mix units across rows, with nothing in the
  file to tell them apart.** "BASFOLIAR ZNMN" declares `Zinc (Zn) = 20,1`,
  plainly a percentage for a zinc foliar; "CODA-Ca-L" declares `Cobre (Cu) =
  70`, `Plomo (Pb) = 45` and `Cromo total (Cr) = 70` on a product whose N, P
  and K are all zero — figures only sane as mg/kg. Filling either way is wrong
  for the other **by a factor of ten thousand**, so C.i's metals are never
  proposed and stay hand-entered from the analysis the farmer holds.
- **`P_% TOTAL` and `K_% TOTAL` are elemental**, not oxides — the median
  `P2O5 % total` / `P_% TOTAL` ratio across 816 rows is 2,2936, the conversion
  factor 2,2914. `MACRONUTRIENTES` codes oxides only, so mapping them would
  understate every product's P₂O₅ by more than half.
- **Copper and zinc as micronutrients** (`MICRONUTRIENTES` 3 and 6) have no
  column of their own: the file's only Cu and Zn columns sit in the metals
  block, so a declared micronutrient cannot be told from a contaminant.

Nineteen columns are safe and are mapped: the five nitrogen forms, three
P₂O₅ forms, two K₂O forms, organic carbon, CaO, MgO, SO₃, and boron, cobalt,
manganese, molybdenum and iron. A **zero is never proposed** — the provider
fills unstated cells with `0`, and blank and zero are different claims. The
column list, its reasoning and the tests that pin it live in
`module_fertilisation::catalogue`.

**Why the mapping cannot be derived, and why dropping units would not remove
it** (asked 2026-08-08): the table maps a provider column header onto a
*catalogue code*, and **none of the 16 `MACRONUTRIENTES` labels appears as a
column header** — "N_% TOTAL" → code 1 is a human reading, with no string to
match on. The micronutrient and heavy-metal labels do match verbatim, but
`Cobre (Cu)` and `Zinc (Zn)` each appear in **both** catalogues, so an
automatic matcher would have to guess exactly where the data is ambiguous. And
the unit is not ours to drop: SIEX's field is `Porcentaje`, the model's column
header says "(%)", section 7.1 multiplies richness by a dose, and RD 1051/2022
anexo IV states the metal limits in mg/kg de materia seca — a figure with no
unit could be compared against none of those.

What the question did surface is a real gap, now closed: the suite caught a
renamed or removed column but not an ADDED one, so a refresh could quietly
introduce a nutrient the fill never offers. Every numeric column must now be
either mapped or listed in `UNMAPPED_COLUMNS` **with its reason**, and a
declared exclusion that stops being published fails too — stale reasoning
being its own kind of rot.

### Sections 9.4 and 9.5 as built (2026-08-19)

**One register, two pages, split by the practice** — the shape 9.2 and the
book's own "9.6" already use. `soil_cover` carries both: art. 42's live cover of
spontaneous or sown vegetation (P6, model 9.4) and art. 43's inert cover of
triturated pruning residue (P7, model 9.5). The two articles ask for the same
three things, `DatosCubierta` gives them one block, and `practice_code` decides
the page.

**The row here is the cover, not the plot.** That is the difference from 9.2 and
9.3, which pivot onto the parcel: a cover has one establishment date and one
pair of widths however many plots it was established over, so there is nothing
to accumulate per plot and the register's own row is already the printed one.
The plots ride in the "Id. Parcelas" cell as table-2.1 cross-references.

#### Art. 42 is three annotations, and the schema is shaped like that

The printed model collapses them into one row of columns. Read from the decree
instead, they are three facts with three deadlines, and they arrive at three
different times:

| Clause | What is annotated | Deadline | Where it is stored |
| --- | --- | --- | --- |
| 42.1.a / 43.1.a | the establishment date | 1 month | `soil_cover.established_on` — the record itself |
| 42.1.e / 43.1.b | the cover width **and** the free canopy width | within the month before the 4-month live-cover period ends | `width_m`, `free_canopy_width_m`, `widths_stated_on` — nullable, all three or none |
| 42.1.c | the maintenance performed | within the month before the solicitud-única modification period ends | `cultural_operation` and `grazing_record` rows carrying a `soil_cover_id` |

Three consequences worth stating, because each is a place the form would have
misled a schema derived from it:

- **A cover with no widths is a COMPLETE record.** Its second annotation is not
  due yet. So the cells print blank rather than as a zero, which would be a
  statement the farmer never made, and the advisory is what says the annotation
  is outstanding.
- **`widths_stated_on` is a column neither the decree nor the twin asks for.**
  It exists because the deadline is what the annotation is *about*: with it,
  "measured in June" and "never measured" are distinguishable at query time.
  Without it they are the same NULL, and no advisory could tell them apart.
- **The widths move together or not at all** (`invalid.incomplete_widths`), the
  `plot_water_point.distance_m` pairing: one width without the other, or a width
  with no date, is a *wrong* answer rather than a missing one.

#### The maintenance is not this register's rows

A siega is a cultural operation and a pastoreo is a grazing, whichever land they
happen on — so they stay in the registers that own them, linked back by a
nullable `soil_cover_id`. **The twin agrees**: `DatosCubierta` in schema 3.11.4
carries no maintenance member at all, while the booleans it derives sit on
`LaboresCulturales`.

That link also **partitions two printed pages**. Model 9.1 prints the grazings
with no cover, model 9.4's Pastoreo column the ones with one. Without the
partition a P6 cover grazing would print on the P1 extensive-grazing page as
well, which on a document an inspector reads is a false statement rather than a
duplicate. `GRAZING_PRACTICES` gained `plant_cover` for the same reason.

The cover form still enters all three columns in one place — a maintenance line
asks only for its kind and date, inheriting the cover's plots, practice, farm
and season, and the repository writes it through the *same* functions the 9.2
and 9.1 forms use, inside the transaction that writes the cover. One validation
path, one audit path, and a book that never holds a cover whose maintenance
half-saved.

**Withdrawing a cover withdraws its maintenance**, each as its own audited soft
delete: those rows are art. 42.1.c's annotation *of that cover* and print in its
columns, so a cover withdrawn as a mistake must leave no siega behind pointing
at nothing.

#### Two things the pages do not print, and one the form does not offer

- **`cover_type_code` has no printed column.** Art. 42.1.a annotates the *date*
  a cover was established, not which of "espontánea o sembrada" it was, so the
  distinction lives in the printed footnote. The field is captured because
  `DatosCubierta.TipoCobertura` asks for it, and the workbook carries it.
- **Model 9.5 has no maintenance columns**, because art. 43 asks for none. A
  maintenance line against an inert cover is refused
  (`invalid.maintenance_on_an_inert_cover`) rather than stored somewhere no
  page would print it.
- **`TIPO_COBERTURA_SUELO` is narrowed by the picker, never by the repository.**
  Art. 42.1.a's codes are 2 and 3, art. 43.1.a's is 4 — and specifically not 5,
  "otros materiales", which is nutshells and stones rather than pruning
  residue. But the catalogue is a provider registry that grows between releases
  (it gained code 6 in 2024) and the in-app refresh runs on the user's machine,
  so refusing an unknown code would lock a farmer out of recording a lawful
  cover. A picker may offer less than the record accepts, never the reverse.
  A contract test accounts for every active code — claimed by a practice or
  pinned in `NON_COVER_TYPES` with its reason — so an upstream addition makes
  somebody decide rather than passing unnoticed.


### The sowing register, and where 9.3's five dates live (2026-08-19)

Model 9.3 is the only page in the book assembled from **three tables in three
crates**, and the reason is that art. 45.2's five dates are five different kinds
of fact:

| Date | Where it is recorded | Why there |
| --- | --- | --- |
| Nivelación | `cultural_operation`, kind `levelling`, practice `flooded_biodiversity` | it is work done on the land, and `TIPO_LABOR` 2 is literally "Nivelación en cultivos bajo agua" |
| Siembra en seco | `sowing_record.sown_on` (**core**) | a sowing is a farm event, not an eco-scheme one |
| Inundación | `sowing_record.flooded_on` | it is an attribute of that sowing — the same seed, later watered |
| Seca | `treatment_record.drying_date` (**module-cue**) | the model says "fecha de seca **para tratamiento**", and the twin puts `FechaSeca` on `TratamFito`: the field is dried *in order to* spray |
| Construcción de caballones | `cultural_operation`, kind `ridging` | `TIPO_LABOR` 3, "Caballones y tablas en cultivos bajo agua" |

**`sowing_record` and `sowing_plot` are in `terrazgo-core`**, the `harvest_record`
precedent: sowing is harvest's mirror image, the two bracket a crop, and crop
planning, costs and analytics will all want it. Core therefore holds the crop's
three brackets — `crop`, `sowing_record`, `harvest_record`. `sowing_plot` mirrors
`harvest_plot` field for field, **including the absence of a surface column**:
model 9.3 asks which parcels, not how much of each.

**It carries no eco-scheme practice code**, and cannot: core may not reference a
module's lookup. What marks a sowing as a *cultivo bajo agua* is `flooded_on`, a
core-native fact — which also decides which plots reach the page. A sowing with
no flooding date is not on its own evidence of a flooded crop, or every wheat
sowing on the holding would print on a page about rice.

**`flooded_on` is normally filled by a CORRECTION.** A rice grower dry-sows in
April and floods in May, and each is annotated within a month of its own
activity — so the row exists with a NULL flooding date for a month. That is why
the page also admits a plot on *other* evidence (a nivelación, a seca), and why
once a plot is in, every sowing on it prints its date.

#### What `SiembraPlantacion` asked for and this register deliberately does not hold

The twin is wider than the duty, and most of the difference is **recorded rather
than captured** (`siex-export.md`), on the standing line: a field required by the
twin is captured even with no model column; a field optional in the twin AND
absent from the model is recorded.

- `Cantidad` (kg of seed) **is** required, so `seed_quantity_kg` exists although
  no page of section 9 prints it.
- `SiembraDirecta` is already recordable as a `cultural_operation` of kind
  `no_tillage` — capturing it twice would be two statements of one fact.
- `MaterialTratado`, `MaterialAdquirido`, `FechaAdquisicion` and `NumLote`
  restate what model 3.2's `seed_treatment` holds. **The two registers stay
  separate tables** (settled 2026-08-19) — the printed model keeps them as two,
  filled independently, and merging them is blocked by their junctions:
  `seed_treatment_plot.surface_sown_ha` is `NOT NULL` because model 3.2 prints
  "Superficie sembrada (ha)", while model 9.3 asks for no surface at all, so a
  merge would either weaken a shipped register or invent a required field.
  **They stopped being *unlinked* on 2026-08-21**: `seed_treatment` gained a
  nullable `sowing_record_id` the farmer sets on the 3.2 form, plus `acquired_on`
  for the one member nothing stored. Three of those four members needed no
  capture at all — `MaterialAdquirido` is `treatment_kind_code` (FEGA's
  TIPO_TRATAMIENTO 4 and 5 *are* "adquisición de semilla tratada"),
  `MaterialTratado` is whether a 3.2 record exists, and `NumLote` is `seed_lot`.
  Reasoning in `siex-export.md` → "How seam 2's contradiction was settled".
- The required `SiembraPlantacion` member is **`sowing_record.kind_code`**, and
  the reading recorded here on 2026-08-19 — Anexo V's *"Cultivo
  sembrado/plantado, según catálogo SIEX. Será un campo calculado"*, so the crop,
  derived — **was wrong**. The WS descriptor types it `number(1)`, "1 Siembra 0
  Plantación"; Anexo V's "Cultivo" is `DGCs[].CodigoCultivo`, per-DGC. That made
  it a capture question, and **the form had already answered it**: this register
  is titled "Siembra y plantación" and asks how each crop began, so a planting is
  its documented use and a constant would misstate every one of them. No decree
  asks for a planting annotation, which is why the register was not *derived*
  from the member — it already invited both answers. (`MATERIAL_VEGETAL_REPRODUCCION`
  is still not its catalogue; that file stays orphaned — `maintenance.md` §1.)

## Two outputs, one assembly (2026-08-02)

The book is read out of the database **once**, into a typed `Cuaderno`
(`terrazgo_recordbook`), and rendered twice:

| | PDF | .xlsx |
| --- | --- | --- |
| Engine | Typst (`terrazgo_report::render_pdf`) | `terrazgo_report::render_xlsx` over `rust_xlsxwriter` |
| Cells | pre-formatted **strings** (dd/mm/yyyy, decimal commas) — the template only does layout | typed **values**: real dates, real numbers |
| Cross-references | order numbers only (models 1.2/1.3/2.1 ↔ 3.1), as the official form prints them | order numbers **and** resolved names, so the sheet filters on its own and still reconciles with the PDF row for row |
| Layout | the official model's tables | one tab per section, bold frozen header, autofilter, column widths |

Why the split matters: a farmer sorting by date, filtering a product or summing
treated hectares needs values, not display text. Numbers carry **no** number
format — Excel renders them in the reader's own locale, which is how a Spanish
user gets decimal commas without the app hard-coding them. Dose value and dose
unit live in separate columns for the same reason: `1,5 L/ha` in one cell is
not summable.

Blank stays blank in both. An unknown surface is an empty cell, never a zero —
a spreadsheet would happily add zeros up, and the official form leaves the cell
for hand-filling. Manual application is the exception that proves it: "Manual"
is a value the model defines (3.1 footnote 3), so it is written, not left out.

Adding a field means touching the assembly once. Both renderers are pinned by
tests against the same fixture — `cuaderno_inputs` for the PDF's JSON contract,
`cuaderno_workbook` for the sheet's cells — so neither can drift silently.

## Language of the book (2026-08-03)

The **layout is per country** — the Spanish official model, one template, never
forked. The **language is per region**: Castilian is official across the state,
and where a statute of autonomy makes another language co-official the farmer
must be able to hand an inspector the same book in either one. Both documents
(PDF and .xlsx) take a language; the chooser sits beside the export buttons and
starts on the UI language when that language is official for the holding.

Shipped today: Castilian and Catalan. Adding one is a single `Labels` const in
`terrazgo_recordbook::labels` — the region map already lists Galician, Basque
and Valencian, and intersects itself with what has a dictionary, so a language
appears the moment it can be printed and not before.

### What translates, and what does not

| | Example | Why |
| --- | --- | --- |
| **Prose translates** | headings, footnotes, "Buena/Bona", "Cualificado/Qualificat", the PHI phrase, 2.2's "Sin afección/Sense afectació" | it is the form's own wording, and the form is what the reader reads |
| **Codes do not** | SEC/ASP/LOC/GRA, AL/M/BP/INV, AE/PI/CP/Atrias/AS/NO, `L/ha`, FEGA catalogue labels for "problema fitosanitario", SIGPAC land-use codes | the record's legal value is the code; the footnote that expands a sigla is what carries the language |
| **User data never** | farm and plot names, species, varieties, notes | it is the farmer's text, in whatever language they wrote it |

So a Catalan book prints Spanish pest names under Catalan headings. That is
deliberate: those labels are the authority's own catalogue wording, and the
export sends the code regardless.

Dates stay `dd/mm/yyyy` and numbers keep the decimal comma in both languages —
`format_date`/`format_number` are shared, and gain a language argument only
when a language proves it needs different ones.

### Which languages a holding is offered

`terrazgo_recordbook::region` maps INE province codes to the co-official
languages there, taking the union of the farm's registry province
(`farm_es_extension.province_code`) and each plot's SIGPAC province — a holding
can straddle a boundary, and offering one language too many costs nothing while
offering one too few hides a right. A holding with **no** province recorded is
offered every shipped language rather than none: an unfilled form field is not
a statement about what the farmer may print.

Two deliberate gaps, each a one-line change when the dictionary exists:
Valencian (provinces 03/12/46) waits for its own entry rather than being
offered as "Català", and Aranese is absent because it is co-official in one
valley, not in province 25.

## Print conventions

- Sections print in model order; a register with no rows prints as a blank
  hand-fillable table (several blank rows, not one) — an empty "postcosecha"
  table reads as "no postharvest treatments", a missing section reads as an
  incomplete book.
- The "APLICA TRATAMIENTO: SÍ/NO" checkboxes are part of the form: SÍ is
  derived from rows existing; NO is an explicit stored declaration
  (proof-of-check, the `plot_zone_flag` negative-result philosophy).
- Deviations from the model stay allowed (content binding, layout not): the
  extra "Plazo de seguridad" column in 3.1 already ships; total-quantity-used
  and interval dates join it per Anexo III B.
- The export filename carries the campaign, the language and the date —
  `cuaderno_2025-2026_ca_20260803.pdf` — because the language is chosen per
  export and never persisted, so the file itself has to say which one it is.
- **Check a layout change by rendering the page, not by extracting its text.**
  `pdftotext` reports the same words whatever the layout does, so it cannot see
  a cell that wrapped to fourteen lines or a table that ran off the sheet;
  render to an image (`pdftoppm`) and look. Two design decisions in this
  document — the BBCH register printing the number rather than FEGA's sentence,
  and section 2.1's font size — came from looking at the rendered page and
  would not have come from any text dump.
