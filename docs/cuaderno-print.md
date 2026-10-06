<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# The printable cuaderno — legal basis and section map

The printed record book is the app's main compliance document: there is no
submission path (see `siex-export.md`'s status banner). This file ties every
section of the official model to its legal source, keeps the exact field lists,
and says what is binding content and what is only the form's convenience, so
schema, report and test decisions can all point to one place. Related:
`data-model.md` (the schema itself) and `architecture.md` → "The report engine".

## Sources of truth

| What | Where | Notes |
| --- | --- | --- |
| Binding content list | [RD 1311/2012](https://www.boe.es/buscar/act.php?id=BOE-A-2012-11605), art. 16 + **Anexo III Parte I** (consolidated text) | Art. 16.1: every farm keeps the treatment register with Anexo III Parte I's information; electronic registration is mandatory (paper allowed until 31/12/2026, under UE 2023/564 and the 2025/2203 postponement Spain took). Art. 16.2: any record book carrying at least Parte I's data complies, so **the content is binding and the layout isn't** |
| What must be kept | RD 1311/2012 art. 16.3 | The register itself, plus advisory documentation (art. 11.2), equipment inspection certificates, service contracts (Ley 43/2002 art. 41.2.c), invoices and supporting documents, and residue-analysis results: *"Los registros y documentos se conservarán al menos durante los 3 años siguientes a su fecha de emisión."* **At least 3 years**, for the entries and documents, not for a history of their changes (see "What the law asks of the entries over time" below) |
| **Fertilisation + irrigation content** | [RD 1051/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23052) art. 4–6, amended by [RD 934/2025](https://www.boe.es/diario_boe/txt.php?id=BOE-A-2025-21211) | A **second decree** feeding the same cuaderno. Art. 5 creates its fertilisation section, **in force since 1 Jan 2026**, recorded within one month of each operation, and art. 5.e puts irrigation doses and dates in the same duty. Art. 4.2 + 6 add the plan de abonado from **1 Sep 2026**. Thresholds are in the section map below. This is why sections 6, 7.1 and 8 are binding, not optional |
| **National minimum content of the cuaderno digital** | [RD 1054/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23054) **Anexo II**, made binding by art. 9.1 | Only four items: (1) crop data per parcela agrícola, from the REA; (2) treatments per RD 1311/2012 anexo III; (3) fertilisation per the sectoral rules; and **(4) "otros aspectos que se recojan en la respectiva normativa sectorial"**. Items 1–3 are what sections 1–8 carry. **Item 4 is an open door, and RD 1048/2022 uses it** (see the eco-scheme section below) |
| **Eco-scheme annotation duties** | [RD 1048/2022](https://www.boe.es/buscar/act.php?id=BOE-A-2022-23048) consolidated, arts. 30–45 + anexo IV | Ten clauses telling the farmer to annotate a practice **in the cuaderno**, mostly within one month. They bind any farm claiming the eco-scheme concerned |
| Layout (orientative) | "Modelo de Cuaderno de Explotación", Junta de Andalucía v6 (2023), a regional reprint of the Comité Fitosanitario Nacional model | The layout the Typst template follows (`crates/terrazgo-recordbook/templates/cuaderno.typ`); the field lists below are copied from it. **The model is older than RD 1051/2022**, so its "OPCIONAL" heading on section 6 no longer states the law; where the two disagree, the decree wins. It is a source of *layout*, never of duties: RD 1048/2022's anexo IV duty has no page anywhere in it |
| EU baseline | [Reglamento (UE) 2023/564](https://eur-lex.europa.eu/eli/reg_impl/2023/564/oj), art. 1–3 + **annex** | Record content for professional users, the same in every member state; applies **from 1 Jan 2026**, with Spain taking Reglamento (UE) 2025/2203's one-year postponement. Its annex is a three-row table by *type of use* (surface areas, closed spaces, seeds) and asks for two things no Spanish form has a column for: the **start hour** where relevant, and the **BBCH growth stage** where relevant. See "What the EU annex adds" below |
| SIEX exchange format | `references/cue-schema-3.11.4.json` (vendored) | Named for each register below, so that if the export is brought back its columns line up |

## What the law asks of the entries over time — and what it does not

**What it asks:**

- **Keep them.** *"Los registros y documentos se conservarán al menos durante
  los 3 años siguientes a su fecha de emisión"* (RD 1311/2012 art. 16.3): the
  entries of the register and the documents behind them. The duty is the
  farm's, as with a paper book.
- **Keep them up to date, and on time.** The farm *"mantendrá actualizado el
  registro"* (art. 16.1); each treatment is recorded electronically *"con un
  plazo de volcado de esta información de un mes desde la fecha de realización
  de los tratamientos"* (art. 16.1), and the data is provided *"al menos, con
  carácter mensual"* (art. 16.5, through RD 1054/2022). Reglamento (UE)
  2023/564 asks for each use to be recorded *"sin demora indebida"*, in a
  machine-readable format, within thirty days when first kept some other way
  (arts. 2–3). RD 1051/2022 gives fertilisation the same month (art. 5).
- **Hand them over when asked** (Reglamento (UE) 2023/564 art. 4).

**What it doesn't ask: anything about how an entry changes.** None of the three
texts says anything about correcting an entry, deleting one, keeping its earlier
versions or keeping a history of changes: not RD 1311/2012 art. 16, not
Reglamento (UE) 2023/564 (four articles: content, format, timing and handing
over), and not RD 1054/2022, which presents the cuaderno digital as the farm's
own electronic system and sets no rule on keeping or changing it. So:

- **A history of edits and a store of deleted entries are the app's choices,
  never a legal duty.** `record_change` exists because sync needs it
  (docs/sync.md) and because showing a record's history is useful; how long it
  is kept is up to those two, not to art. 16.3.
- **Deleting an entry is the farmer's decision**, as crossing one out of a paper
  book is. What the law holds them to is that the book, as it stands, has an
  entry for every operation of the last three years.
- **What was sent is what the farm answers for.** Once an entry is in the
  authority's system, the authority holds what was sent; a correction is sent
  again under the same `IdAjena*`, and a removal carries `Borrar`
  (docs/siex-export.md). What the app owes itself is a record of each
  submission (what, when, under which ids), which is the exporter's to keep.

## Section map — model section → legal basis → status

Classes: **binding** = Anexo III Parte I content, **or** required by RD
1051/2022 for the fertilisation and irrigation sections; **conditional** =
binding only when the activity happens (the model's "APLICA TRATAMIENTO: SÍ/NO"
checkboxes); **recommended** = in the model but not in Anexo III Parte I (kept
because the model is what inspectors know, and art. 16.3 requires keeping the
documents behind it).

> **Three decrees feed one book.** RD 1311/2012 covers the phytosanitary
> registers (1.x, 2.x, 3.x, 4); RD 1051/2022 covers fertilisation, irrigation
> and soil (6, 7.1, 8, A.3); RD 1048/2022 covers the eco-scheme annotations
> (section 9), reaching the cuaderno through RD 1054/2022 anexo II item 4. They
> have different deadlines and different exemption thresholds, and the printed
> model only reflects the first. The third is also the one whose registers have
> to be taken from the decree rather than from the form (see "The eco-scheme
> registers" below).

| Model section | Legal basis | Class | How the book covers it |
| --- | --- | --- | --- |
| 1.1 Datos generales | Anexo III A.1.a–b | binding | Identity, both registry numbers (national `siex_code` + regional `rea_code`), full postal contact, the titular-o-representante block, and the box for a handwritten signature |
| 1.2 Personas que intervienen | A.1.c | binding | NIF, the Piloto carné, and the Asesor cross, which isn't a carné level but a NIF match against the advisor table |
| 1.3 Equipos de aplicación | A.1.h | binding | `machinery.acquired_on` prints next to the inspection date; A.1.h accepts either, so equipment needing no ITV still has a date |
| 1.4 Asesor / entidad | A.1.d | binding (when advised) | `advisor` + `farm_advisor` (name or razón social, NIF, nº de identificación, and the tipo de explotación as the six GIP abbreviations) |
| 2.1 Parcelas | A.2.a–f | binding | Uso SIGPAC and Superficie SIGPAC (from the provider's boundary, next to the user's own figure); Superficie cultivada is left blank on plots with several crops instead of repeating the plot's area. Secano/Regadío and aire libre/protegido print as the model's four-value abbreviations, and `crop.area_ha` gives each crop its own surface. GIP is stated per crop (`crop.gip_system_code`, the full art. 10 framework); a crop that states nothing falls back to the AE/PI its production system implies |
| — soil characteristics | A.3 + RD 1051/2022 art. 5.b | conditional | The nine `Analitica.ParametrosSuelo` figures on `analysis_record`. Not binding yet: A.3's minimum data (pH, P₂O₅, K₂O, organic matter) becomes required one year after MAPA publishes its sampling and analysis guides; heavy metals only when sludge is applied. **RD 1051/2022 art. 5.b already asks for soil organic matter, nutrients and contaminants**, and art. 6 makes soil data an input to the plan de abonado. The Andalucía v6 model is older than A.3 and has no soil page |
| 3.1 Registro de actuaciones | B.a–k | binding | The date is an *interval* when the work took several days (`application_end_date`, so the export's `FechaInicio`/`FechaFin` are both real), and **B.i's total kg or l used** is recorded (`total_quantity_value` + `_unit_code`) rather than worked out, because a concentration dose (g/l, ml/l, %) can't give one. **The plazo de seguridad counts from the END of the interval**, since the plazo is the time between the *last* application and harvest, so `phi_end_date` is worked out from `application_end_date` when it is set |
| 3.1 bis (cultivos asesorados) | B.d "y, en su caso, del asesor"; the rest is art. 10–11 (GIP) as the model shows it | binding only for advised crops | **Anexo III Parte I B is ONE list (a–k) covering every treatment**, so this is a second VIEW of `treatment_record`, not a second register. B.d's advisor is the binding part; the non-chemical alternative, the written justification and the two signature boxes are art. 10–11 compliance as Andalucía presents it. SIEX agrees: `AsesorValidacion` and `OtrasActuacionesFito` both hang off `TratamFito`, which does **not** require `ProductosFito`, so an action with no product is a full record |
| 3.2 Semilla tratada | B.e ("si la siembra se realiza con semilla tratada, indicar el producto") | conditional | `seed_treatment` + `seed_treatment_plot`. The product is FREE text (name, nº registro, materia activa) with an optional link to the product registry: treated seed names a product the farmer never bought as such. **The plots come from the printed model, not from SIEX**: `UsoSemillaTratada` has none, and the model is the compliance document |
| 3.3 Postcosecha | B.b/B.f ("local o medio de transporte tratado", volume in m³) | conditional | `non_field_treatment`, one table for all three sections (SIEX: `TratamientosPostCosecha`) |
| 3.4 Locales de almacenamiento | B.b/B.f | conditional | `non_field_treatment` (SIEX: `TratamientosEdifInstalaciones`) |
| 3.5 Medios de transporte | B.b/B.f | conditional | `non_field_treatment` (SIEX: `TratamientosEdifInstalaciones`) |
| 2.2 Medioambiental (agua + zonas) | A.1.e–g (water bodies and drinking-water abstraction points, with the distance when outside the parcel; art. 35 zones) | binding | Zones half: the latest campaign per (plot, zone kind), and "Totalmente" only when every zone affecting the plot covers all of it (unknown coverage counts as partial). Water half (`plot_water_point` + `plot_water_declaration`): several points on one plot are lined up by position across the four cells, so the columns read across. **Both halves print in three states, for the same reason**: a stated negative ("Sin afección — campaña YYYY", "Sin captaciones — DD/MM/YYYY") is evidence for an inspection, a plot nobody checked prints blank, and saying nothing isn't the same claim. The model's "Coordenadas UTM" column prints the stored WGS84 lat/lon pair and is relabelled accordingly |
| 4 Análisis de productos fito | art. 16.3 (keep residue-analysis results) | recommended | `analysis_record` + `analysis_plot`, details only: the register says an analysis exists and where its bulletin can be found, and never holds the bulletin (SIEX: `Analitica`) |
| 5 Cosecha comercializada | traceability (Ley 43/2002; food-chain rules); not Anexo III Parte I | recommended | `harvest_record` + `harvest_plot`, in **core**. Neither section 4 nor 5 has an "APLICA TRATAMIENTO" line: they are recommended registers, not conditional ones, so no `register_declaration` code backs them (SIEX: `ComercializacionVD`, **not** `Cosecha`) |
| 6 Fertilización | Anexo III **Parte I.C** + **RD 1051/2022 art. 5**, amended by RD 934/2025 | **binding since 1 Jan 2026** | `fertiliser_material` + `fertilisation_record` and its junctions, in `module-fertilisation`. **The model's "OPCIONAL (EXCEPTO ZONAS VULNERABLES)" heading is OUT OF DATE** (Andalucía v6 is from 2023, before RD 1051/2022): this is a NATIONAL duty with a size threshold, not a nitrate-zone matter. Exempt only: ≤5 ha of arable and permanent crops (temporary pastures excluded) **and** ≤1 ha irrigated, or pasture only and not fertilised. **The size exemption is partial** (art. 4.1): a farm exempt under the ≤5 ha limb that fertilises pastures, or has greenhouses totalling more than 0,1 ha under cover, must still record **those surfaces**. Deadline: **one month from each operation**. SIEX: `Fertilizacion` |
| 7.1 Plan de abonado | **RD 1051/2022 art. 4.2 + art. 6** (not only for PAC eco-schemes) | **binding from 1 Sep 2026** | `fertilisation_plan` holds art. 5.a's four items (what the BOOK records); art. 6's plan document is kept, not printed. The table's aportadas and acumuladas are worked out from section 6. Earlier, **1 Jan 2026**, for irrigated production units sown or planted between 1 March and 30 June. Exempt: pasture only and not fertilised; ≤10 ha of secano or fodder for the farm's own use. SIEX: `PlanAbonado` |
| 8 Riego | Anexo III C.l + **RD 1051/2022 art. 5.e** | **binding since 1 Jan 2026** | `irrigation_record` + its junctions in `module-fertilisation`, NOT the Irrigation module: art. 5.e puts *doses and dates of irrigation* inside the SAME cuaderno duty as fertilisation, with the same one-month deadline. SIEX `Riego` requires `SistemaRiego` per EVENT and carries `OrigenAgua`, which is where `SIST_RIEGO` and `ORIGEN_AGUA_RIEGO` are read. The Irrigation module keeps *planning* (schedules, ETo); this is the *record* |
| 9.1 (P1) Pastoreo extensivo | RD 1048/2022 art. 30.2 ter | conditional (farms claiming the eco-scheme) | `grazing_record` + `grazing_plot` + `grazing_animal` in `module-ecoscheme`. **The deadline runs from the END of grazing**, so an open record (`ended_on` NULL) isn't late and prints an empty end cell rather than an invented date. One printed line per group of animals: the model's last three columns describe one group while the dates describe the grazing, so sheep and goats on one pasture are two lines. Column 2 asks for the SIGPAC **reference**, not the table-2.1 cross-reference the other registers use; the plot's name prints instead when it has no complete reference (SIEX: `Pastoreo`) |
| 9.2 (P2) Siega sostenible / islas de biodiversidad | RD 1048/2022 art. 31 + 31.4.d | conditional | `cultural_operation` + `cultural_operation_plot` in `module-ecoscheme`, ONE table behind four duties on three pages. **The printed page is a PIVOT of the register**: the model's row is a *plot* (with the SIGPAC parts and the surface in their own columns, like table 2.1) and its cells collect dates, so two cuts are one row, and one operation on two plots is two rows. `no_tillage` doesn't print under "Laboreo", on purpose: a date there says the ground was worked. The **Siembra column stays empty**: `TIPO_LABOR` has no sowing code, and a sowing is its own register (SIEX: `LaboresCulturales`) |
| 9.3 (P5) Espacios de biodiversidad: cultivos bajo agua | RD 1048/2022 art. 45.2 | conditional | **It prints FIVE date columns where the model prints three.** Art. 45.2 names *"las fechas de nivelación, siembra, inundación y secas, y construcción de caballones"*; the form has no column for nivelación or caballones, so a book following the form wouldn't meet the article. The layout is orientative and the content binds. The two extra columns sit where the ARTICLE names them, which keeps the model's own three in their original order. The row is a plot, and its five cells come from **three tables in three crates**: `sowing_record` (core), `treatment_record.drying_date` (module-phytosanitary) and `cultural_operation` (module-ecoscheme); only `terrazgo-recordbook` can read all three. **A plot appears on the page when there is evidence it is a cultivo bajo agua** (a flooded sowing, a `flooded_biodiversity` operation, or a treatment that dried the field); once it appears, every sowing on it prints, which keeps a dry sowing visible in the month before the flooding is recorded |
| 9.4 (P6) Cubiertas vegetales en leñosos | RD 1048/2022 art. 42.1.a, 42.1.c, 42.1.e | conditional | `soil_cover` + `soil_cover_plot` in `module-ecoscheme`. **The row here is the COVER, not the plot**, unlike 9.2 and 9.3: one establishment date and one pair of widths however many plots it covers, so there is nothing to pivot. Art. 42 is **three entries with three deadlines**, which the model's single row squeezes together and the schema keeps apart: `established_on` is the record, the two widths are an all-or-none group (with their own `widths_stated_on`) that can be empty, and the maintenance is rows in the registers those activities already belong to. So the three "mantenimiento por medios mecánicos" columns are read from elsewhere: Siega and Desbrozado from `cultural_operation`, Pastoreo from `grazing_record`, both linked by `soil_cover_id` and looked up once per book. `TIPO_COBERTURA_SUELO` is recorded but has **no printed column**: art. 42.1.a records the date, not which of the two kinds of cover it was (SIEX: `DatosCubierta`) |
| 9.5 (P7) Cubiertas inertes de restos de poda | RD 1048/2022 art. 43.1.a–b | conditional | The same `soil_cover` register under `practice_code = 'inert_cover'`, printed three columns shorter because **art. 43 asks for no maintenance at all**. The register refuses a maintenance line against an inert cover rather than store something no page would print. The 15 April limit is an advisory check, not a refusal when saving: the book records what happened and doesn't decide whether an aid was earned (SIEX: `DatosCubierta`) |
| "9.6" pastos comunales | RD 1048/2022 **anexo IV** | conditional | **On a page the model doesn't have.** The same `cultural_operation` rows under `practice_code = 'communal_pasture'`, printed as a subsection the book numbers **9.6** itself, with a footnote saying the official model has no page for it, because a reader comparing the two must find the difference explained. One row per operation, not 9.2's per-plot pivot: there is no official layout to follow, so the register's own shape is the honest default. Putting it into 9.2 was rejected: that page's footnotes are about P2's two cuts and its 300 m threshold, which would sit wrongly over an anexo IV row. The invoices the annex asks for are on the documents-to-keep page (`item_communal_invoices`); the book holds no attachments by design. This is the clearest proof that duties come from the decree, not the form |
| 10 Determinadas ayudas asociadas | RD 1048/2022 arts. 49.h, 51.e, 53.e, 54.d, 61.4 | recommended | **Nothing to build.** The page has no fields; it points to sections 3, 7 and 8, because the aid requires "la aplicación de la gestión sostenible de insumos conforme a las disposiciones normativas vigentes en materia de nutrición sostenible de los suelos agrarios", i.e. RD 1051/2022 compliance, which those sections already record. The page prints one sentence saying so, or a reader would think the book skipped a section |
| Documentación a conservar (annex page) | art. 16.3 | binding (as a duty, not a table) | A plain numbered list of the model's seven items plus the three-year retention sentence, never tick boxes (it reminds the farmer what to file away; it isn't something to fill in). The book holds no attachments by design |

## What the advisory checks

The printed book has no gate: it shows what exists, and fields the model asks
for that the data doesn't have print blank, because a farmer must be able to
print for an inspection while registry data is still incomplete. That rules out
*blocking*, not *telling*. (`export_precheck` serves the parked SIEX export, not
the book.)

`terrazgo_recordbook::advisory::book_advisory` reports these, all as advice;
none can stop a print or an export:

| Finding | Source |
| --- | --- |
| Farm address, holder's name, holder's tax id | Anexo III Parte I A.1.a–b |
| Treated plots with no crop stated | Anexo III Parte I B.e |
| Treatments with no efficacy assessed | Anexo III Parte I B.j (seen after the application, so advice here and refused at export instead) |
| Applicators with no ROPO / licence number | table 1.2, and B.d's identification |
| Conditional registers ticked neither SÍ nor NO | the model's own "APLICA TRATAMIENTO" boxes: of the three states, saying nothing is the one that says nothing |
| Sections 6 and 8 empty | RD 1051/2022 art. 5.d and 5.e |
| Covers with no widths stated | RD 1048/2022 arts. 42.1.e / 43.1.b |
| Inert covers established after 15 April | art. 43.1.a |
| Live covers with no maintenance recorded | art. 42.1.c |
| Grazings still open in a closed campaign | art. 30.2 ter |
| Codes printed without a name, because no catalogue on this device names them | none: a finding about the device, not the book. Counted by running the book's own assembly, so it matches the page; the fix is a catalogue refresh ([sync.md](sync.md) → What stays device-local) |

It takes `today` as a parameter instead of reading the clock (like the alert
rules), because one of the section-9 checks is a date rule, and a rule that
can't be tested can't be pinned.

### Section 9's checks are record-triggered, and that is the design

The four eco-scheme findings each start from a record the farmer **chose to
create**, so none of them reaches a farm outside the scheme. There is **no
`SectionGap` and no "section 9 is empty" finding**, on purpose: the app can't know
which eco-schemes were claimed in the solicitud única, so an empty section 9 is
normal for most farms. It is the same as the plan de abonado, and more so, since
here the duty is per claimed practice.

There is nowhere to read a claim from. The Nube de SIGPAC OGC API publishes five
collections and none is an eco-scheme layer; `cultivo_declarado` carries
solicitud-única aid *lines* (`CL`, `VI`, `PT`), not practices; FEGA's
287-catalogue registry has no list of practices at all; and the CUE exchange
schema models activities, not entitlements. **The design leaves room for it**:
`practice_code` is on every record, so a farm-level "claimed P6, recorded
nothing" finding would be one advisory field away.

Two of the four are worded carefully, because the obvious wording would be
false:

- **Missing widths aren't late.** Art. 42.1.e falls due after 42.1.a, so a cover
  between the two deadlines is a *complete* record with an entry still to make.
  `widths_stated_on` is what makes the two distinguishable at all; it is why
  that column exists, though no source asks for it.
- **An open grazing isn't late either.** The month runs from the END of grazing
  (art. 30.2 ter, and the model's own 9.1 footnote), so the honest finding is
  that the book can't show the entry *finished*, and only once the campaign has
  ended. Every season has an end date, because the dates are what name the book.

The 15 April limit is checked against the year of the record's **own**
`established_on`, never `season.label`, which is a name (the farmer's own, or
the campaign's two calendar years) and couldn't answer it anyway. And there is
**no communal-pasture invoice check**: anexo IV asks for invoices kept as
evidence, and the book holds no attachments by design.

### Why it never says "exempt"

RD 1051/2022 art. 4.1 exempts a farm with **≤5 ha** of permanent crops and
arable land (pastures excluded) **and ≤1 ha** irrigated, or one that only has
pasture and doesn't fertilise it. But read word for word, the article **takes
some of that back**: a farm exempt under a) still records:

1. pastures where fertilisers ARE applied, and
2. **invernaderos con superficie total bajo cubierta superior a 0,1 ha**.

The first is unknowable for exactly the farm being advised (one that records
nothing tells the app nothing about what it applies), so the advisory says
`possibly_exempt`, never `exempt`, worded as "check it: the exemption has
conditions the app can't see". The greenhouse clause CAN be checked
(`crop.growing_environment_code`, or SIGPAC's own `IV` use), and it is checked
**first**, because it survives the exemption rather than qualifying it.

The third answer, `undetermined`, applies when any plot has no SIGPAC land use or
no area. Land use only comes from a *checked* boundary, so a farm that never ran
the SIGPAC check can't be measured, and saying so is better than excusing it.
Where the data is only ambiguous, the rule leans towards reporting the duty: a
temporary pasture sown on arable land counts towards the 5 ha, because SIGPAC
calls that plot `TA` and nothing we hold says otherwise. Asking a farmer to check
a rule that may not apply is the cheap mistake; telling one who is bound that
they aren't is the expensive one.

**There is no advisory for the plan de abonado.** Art. 6's duty has exemptions
per *unidad de producción* (art. 4.2), a unit the schema doesn't model (a plan
covers a set of crops), so a check would either nag every small farm or excuse a
large one.

## The eco-scheme registers

A third decree writes into the cuaderno. RD 1054/2022 anexo II ends with *"otros
aspectos que se recojan en la respectiva normativa sectorial"*, and **RD
1048/2022** is that sectoral rule for anyone claiming an ecorrégimen: ten
clauses telling the farmer to annotate something **in the cuaderno de
explotación agrícola**, most within one month of the activity.

The table is ordered by **decree and article**, with the model's pages in the
last column rather than as the organising principle, because the duties exist
whether or not a form prints them, and one of them has no printed page at all.

| Article | What must be annotated | Deadline | Model page |
| --- | --- | --- | --- |
| 30.2 ter | the new grazing start/end dates, when they differ from those in the solicitud única | 1 month from the new date | 9.1 |
| 31 | *"la fecha y las actividades realizadas"*: pastoreo, siega para producción o mantenimiento, or any other maintenance activity in anexo III.B | 1 month | 9.2 |
| 31.4.d | *"las labores de siega realizadas"* | 1 month after mowing | 9.2 |
| 42.1.a | *"la fecha de establecimiento de la cubierta vegetal espontánea o sembrada con presencia viva sobre el terreno"* | 1 month | 9.4 |
| 42.1.c | *"el tipo de mantenimiento que realiza sobre la cubierta"* | within the month before the solicitud-única modification period ends | 9.4 |
| 42.1.e | *"la anchura de la cubierta y la anchura libre de la proyección de copa"* | within the month before the end of the 4-month live-cover period | 9.4 |
| 43.1.a | *"la fecha de establecimiento de la cubierta inerte"*, which may not be later than 15 April | 1 month | 9.5 |
| 43.1.b | the same two widths | within the month before the modification period ends | 9.5 |
| 45.2 | *"las fechas de nivelación, siembra, inundación y secas, y construcción de caballones"* | 1 month per activity | 9.3 |
| **anexo IV** | the dates of maintenance work on each **pasto comunal** plot, with the invoices kept as evidence | 1 month | **none** |

What the printed form hides, and a schema copied from it would have lost:

1. **Anexo IV's duty has no page.** The model prints five eco-scheme
   sub-registers; the decree has six duties. Reading the form would miss the
   sixth entirely; the book prints it as its own **"9.6"**.
2. **Art. 42 is three entries with three different deadlines**, squeezed into
   one row of columns. A single "cover" record with one date wouldn't meet it,
   which is why `soil_cover` is a record, an all-or-none group of widths that can
   be empty, and rows in two other registers.
3. **Art. 45.2 names five dates and model 9.3 prints three**, so the book prints
   five: the layout is orientative and the content binds.
4. **The cuaderno is the main way to prove the practice, not a convenience.**
   Farmers who don't use it fall back on a paper register they must keep, plus
   georeferenced photographs for a 1 % sample of beneficiaries.

**Two of RD 1048/2022's duties are already covered**: arts. 35.2 and 45.7 ask for
a plan de abonado plus *"las operaciones de aporte de nutrientes y materia
orgánica al suelo agrario y de agua de riego"* in the cuaderno, which are
sections 6, 7.1 and 8.

**Other decrees that add nothing here.** RD 1047/2022 (gestión y control) names
the CUE as a system and creates no annotation duty. RD 1049/2022
(condicionalidad reforzada) has one cuaderno clause and it points to RD
1051/2022: *"todas las operaciones encaminadas a aportar nutrientes o materia
orgánica al suelo deben estar correctamente registradas en el cuaderno de
explotación"*, plus the plan de abonado, which the book already carries.

**Vocabularies.** `TIPO_COBERTURA_SUELO`, `TIPO_LABOR`, `DEST_RES_VEG` and
`ESPECIE_ANIMAL` are vendored and read by these registers. `RAZAS` is published
but **not** vendored: neither model 9.1 nor `Pastoreo.Animales[]` asks for a
breed (`maintenance.md` §1). Two lookups of our own cover what FEGA publishes no
list for: `eco_practice` (the six duties) and `cultural_operation_kind`, whose
codes map onto `TIPO_LABOR` with one pair deliberately not one-to-one, checked
in both directions so a new upstream code makes somebody look. The SIEX
counterparts are `Pastoreo`, `DatosCubierta` and `LaboresCulturales`: all three
are recorded and none is sent (`siex-export.md`).

## What the EU annex adds (Reglamento (UE) 2023/564)

The annex is a table of **three types of use** (treatment of surface areas,
treatment of or in closed spaces, treatment of seeds or plant reproductive
material) against seven columns. Two of its cells ask for data no Spanish form
has a column for, and both are **conditional**:

- **Start time (hour)**, in the surface-areas row only: *"Date and where
  relevant start time (hour)"*, with footnote 4 defining relevance as *"when the
  use of plant protection product is restricted to specific times of the day or
  when the time of use is relevant in the context of the particular use."* The
  other two rows only ask for the date, so sections 3.2, 3.4 and 3.5 owe nothing.
- **Growth stage according to the BBCH monograph**, in the surface-areas and
  closed-spaces rows, inside the **"Crop or situation/land use"** column. So it
  belongs to the treated crop, which is `treatment_plot`, exactly where the
  exchange format puts it (`TratamFito.DGCs[].EstadoFenologico`). Footnote 7
  mirrors footnote 4: relevant when the product's use is limited to particular
  growth stages. The vendored `EST_FENOLOGICO` has the BBCH principal stage in a
  column of its own, so it is the picker.

Neither is in RD 1311/2012 anexo III parte I B, so the duty comes from the EU
regulation alone, which doesn't make it optional. Both are recorded:
`treatment_record.application_time` (local clock time `HH:MM`, not UTC on
purpose: it is a time *of day*, no timezone is stored anywhere, and converting to
UTC and back would print an hour the farmer never recorded) and
`treatment_plot.growth_stage_code`.

Three things about the growth stage:

- **The catalogue's code isn't the BBCH stage.** FEGA numbers `EST_FENOLOGICO`'s
  rows 1–10 in `Código SIEX` (which is what a record stores, because SIEX checks
  `EstadoFenologico` against the catalogue) and gives the monograph's own 0–9
  next to them in `Estadio bibliografía`. Every reader goes through
  `module_phytosanitary::catalogue::growth_stage`, which returns both forms;
  printing the stored code would be off by one everywhere.
- **The register cell prints the NUMBER, not FEGA's wording.** The annex asks for
  the stage "in line with the BBCH monograph", and the monograph identifies a
  stage by its number, while the catalogue's labels are whole sentences
  ("Desarrollo de las partes vegetativas cosechables de la planta o de órganos
  vegetativos de propagación/ embuchamiento") that made one row of the 15-column
  landscape register wrap to fourteen lines. That matches how the model already
  uses abbreviations, and the spreadsheet keeps the full name.
- **One printed row can carry several stages.** A row groups plots with the same
  species and variety, and a treatment over two days can find them at different
  stages, so the cell lists each one ("BBCH 4 / 5"). Naming only the first would
  say something false about the others.

Of the two other annex points, one is met and **one can't be fully met**. The
seed row's *"batch number, where applicable"* is `seed_treatment.seed_lot`. But
**crop names following EPPO codes** (art. 1.3, which leaves the correspondence to
the Member State) can only partly be worked out. In the vendored `PRODUCTOS`,
**151 of 1023 active rows have no EPPO code**, and that is structural rather than
an omission: EPPO codes a plant taxon, and much of the catalogue isn't one.
`BARBECHO TRADICIONAL` and `BARBECHO MEDIOAMBIENTAL` are fallow, `PASTOS
PERMANENTES DE 5 O MÁS AÑOS` is a land use, `FLORES` is a generic group, and
`TRANQUILLÓN` is a wheat-rye mixture naming two taxa. Those rows can never have a
code, which fits the annex calling its own column "Crop or situation/**land
use**". So nothing derives an EPPO code today: a derivation that worked 85% of
the time and looked complete would be worse than none. The count is checked
exactly by a contract test in `terrazgo-core`, so a catalogue refresh that
changes it makes somebody look.

Two notes on framing, neither a defect. Art. 2 requires the records to be **kept
in a machine-readable format** (Directive (UE) 2019/1024 art. 2(13)): the app
meets that by storing them at all, and the spreadsheet is the portable form. A
PDF isn't machine-readable in that sense, so "the printed book is the compliance
document" holds for *inspection* under RD 1311/2012 art. 16.2 without being the
whole answer. Art. 3 requires recording *without undue delay* and putting records
in electronic form within 30 days, relaxed before 2030 to "before 31 January of
the year following the year of use"; a record made in the app meets it when it
is entered.

## Exact field lists (transcribed from the model)

The footnote code lists are part of the form: they are the closed vocabularies
the schema needs.

### 1.1 Datos generales de la explotación

Fecha de apertura del cuaderno · Nombre y apellidos o razón social · NIF ·
Nº Registro de Explotaciones **Nacional** · Nº Registro de Explotaciones
**Autonómico** · Dirección · Localidad · C. Postal · Provincia · Teléfono fijo ·
Teléfono móvil · e-mail. **Titular o representante:** Nombre y apellidos · NIF ·
Dirección · Localidad · C. Postal · Provincia · Tipo de representación ·
Teléfono · e-mail. Signature box (the person signing answers for the data being
true) + date.

How three of these cells are filled:

- **"Fecha de apertura del cuaderno"** is `farm.opened_on`. The book is a
  continuing document for the farm, so the date belongs to the farm, with the
  campaign printed next to it. If it isn't stated, the ruled line stays rather
  than an invented date.
- **The representative's "Provincia"** is `farm_representative.province`, free
  text like the address lines around it. A code would put a Spanish code list in
  a core table, and a company's representative may be outside Spain.
- **The farm's own "Provincia"** comes from `farm_es_extension.province_code`.
  It is looked up in the vendored `PROVINCIA` catalogue when the stored value
  reads as an INE province number, and printed as the farmer typed it when it
  doesn't (the catalogue-label rule), with one refinement: a code that can't be
  resolved prints the farmer's own string, not a zero-padded one ("7" must not
  print as "07"). A bare number in a legal document isn't acceptable.

### 1.2 Personas o empresas que intervienen

Nº de orden · Nombre y apellidos / Empresa de servicios · NIF · Nº inscripción
ROPO / nº carné · Tipo de carné: **Básico / Cualificado / Fumigador / Piloto**
(crosses) · **Asesor** (its own cross column: being an advisor isn't a carné
level; ROPO registers applicators and advisors separately).

### 1.3 Equipos de aplicación

Nº de orden · Descripción del equipo (tipo, marca y modelo) · Nº inscrip. ROMA
(when ROMA registration isn't required: the reference number in the
corresponding census, REGANIP) · **Fecha de adquisición** · Fecha de la última
inspección.

### 1.4 Asesor, agrupación o entidad de asesoramiento

Nombre o razón social · NIF · Nº de identificación · **Tipo de explotación**
(the GIP framework of art. 10): **(AE)** Agricultura Ecológica · **(PI)**
Producción Integrada · **(CP)** Certificación Privada · **(Atrias)** Agrupación
de Tratamiento Integrado en Agricultura · **(AS)** Asistida de un asesor ·
**(NO)** Sin obligación de disponer de asesor en GIP.

### 2.1 Datos identificativos y agronómicos de las parcelas

Nº de orden (consecutive, grouping parcels managed together) · SIGPAC: Código
Provincia · Término municipal (código y nombre) · Código Agregado · Zona · Nº
Polígono · Nº Parcela · Nº Recinto · **Uso SIGPAC** · **Superficie SIGPAC (ha)** ·
Agronómico: Superficie cultivada (ha) · Especie · Variedad (GMO varieties add
"OGM") · **Secano/Regadío**: **(SEC)** secano · **(ASP)** aspersión · **(LOC)**
goteo o localizado · **(GRA)** por gravedad · **Aire libre o protegido**:
**(AL)** aire libre · **(M)** malla · **(BP)** cubierta bajo plástico · **(INV)**
invernadero · **GIP** (the same six codes as 1.4, per row).

> The "Secano/Regadío" column is *not* yes/no: the form's own footnote makes it
> a four-value irrigation-method code, matching A.2.e's "secano o regadío
> (indicando en su caso el sistema de riego)".

**"Término municipal (código y nombre)"** prints both, e.g. "122 · POLLOS". The
SIGPAC reference holds the municipality as a number and the provider returns no
name in any response, so the name comes from the vendored `MUNICIPIO_SIGPAC`
catalogue. Three rules, and the first is the one that matters:

- **The province is part of the key, not context.** Municipality codes are only
  unique *within* a province (001 is Alegría-Dulantzi in Álava and Adalia in
  Valladolid), so the lookup is qualified by `Código de provincia`, which is also
  the catalogue's `identity_attrs`. A plot that states no province gets **no
  name**, never the first of 52 candidates. A test drops the qualifier to watch
  a Valladolid plot print Álava's town.
- **Both parts are used as stored**: a checked plot holds `10`, not `010`,
  because the reference parses its parts as numbers, while the catalogue pads to
  three digits. Anything that doesn't read as a number, and any code the
  snapshot can't resolve, gives no name, so the cell shows the code alone (as
  with `problem_code`) and the book still prints with no snapshot at all.
- **The PDF joins them, the workbook keeps them apart.** The model has one
  column, so the printed cell has "código · nombre"; the sheet has a *Municipio
  (nombre)* column of its own, because a joined string can be read but not
  filtered. The 2.1 column takes a share of the width rather than `auto`,
  because names upstream run to 67 characters and would otherwise push a
  17-column table off the page.

The **province** column next to it prints the code, on purpose: 2.1 asks for
"Código Provincia", where 1.1 asks for "Provincia".

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

**Reglamento (UE) 2023/564's annex adds two more, both conditional**: the
**start hour** of the application (surface treatments only, "where relevant")
and the crop's **BBCH growth stage** ("where relevant", per treated crop, not
per record). Neither has a column in the model, so each goes into the cell the
annex itself places it in (the hour after the date, the stage after the
species), with the page's footnotes (2) and (3) saying so. The spreadsheet gives
each its own column. See "What the EU annex adds" above.

Anexo III B also asks, beyond the model's columns, for the **total kg or l of
product used** (B.i) and the machinery registration number (B.h; ROMA/REGANIP
already print).

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
SIEX also carries a seed lot number (`NumeroLote`).

### 3.3 / 3.4 / 3.5 — one structure, three subjects

"APLICA TRATAMIENTO: ☐SÍ ☐NO" on each · Fecha · *subject* · Problemática
fitosanitaria · *quantity* · Producto (Nombre comercial · Nº Registro ·
Cantidad utilizada, kg o l). The subject/quantity pairs: **3.3** producto
vegetal tratado / Cantidad (t) · **3.4** local tratado (tipo y dirección) /
Volumen (m³) · **3.5** vehículo tratado (tipo, modelo y matrícula) / Volumen
(m³). SIEX also requires coded problems, justifications, applicator and
efficacy.

**These three registers are Anexo III Parte I B**, not a separate list that
looks like it: B.b identifies what was treated as "la parcela, **o en su caso,
local o medio de transporte tratado**", and B.f asks for "el volumen tratado
expresado en metros cúbicos" *como tratamiento de locales*: the model's own
subject and quantity columns, taken straight from the annex.

**B.b's word is "identification", which is why 3.4 and 3.5 use a registry.** A
description typed again on each record identifies nothing: two treatments of one
warehouse can spell it differently and nothing links them, so the book couldn't
answer "what was done in this store this year". Core's `premises` is the
identity. The printed cell is built from it (name + address for a building, name
+ model + plate for a vehicle) and stored on the record, so correcting a store's
address never changes what a past record says. The link can be empty: a farmer
who hasn't filled in the registry can still record a lawful treatment, and it is
the SIEX export's precheck that asks for it (as with efficacy). A postharvest
record names no premises, because it treats produce.

In the app the registry is the catalogue's premises section (farm-scoped, like
machinery), and each register only offers the kind its own page prints: 3.4 the
buildings, 3.5 the vehicles. Choosing one replaces the free-text field rather
than sitting next to it, because the printed cell is built in Rust and a second
editable field would ask twice; the free text remains for a farmer who has
registered nothing yet, with a line pointing to the catalogue. A registry entry
also has two fields the *printed* model never asks for and Anexo V does (a
cadastral reference and FEGA's building class), which stay on the registry row
and out of the printed cell (`data-model.md` → `premises`).

B.d ("identificación del aplicador **y, en su caso, del asesor**") applies here
as it does in 3.1, so the register carries the advisor even though the printed
model has no column for it. The PDF puts the pair in the applicator cell the
model does have (B.d names the two together), and the workbook splits them into
an *Asesor* and a *Nº ROPO asesor* column.

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
Rec., which can be replaced by the 2.1 order number) · Sup. (ha) · **S/R** ·
Cultivo (Especie, Variedad) · Producción (kg/ha): **Estimada** / **Final** ·
Tipo de abono/producto · **Nº de albarán** · **Riqueza N/P/K** · **Dosis N
(kg/ha, m³/ha)** · Tipo de fertilización: **(F)** fertirrigación · **(AF)**
abonado de fondo · **(AC)** abonado de cobertera · Observaciones.

The model's footnote (0) gives a **15-day** recording deadline. RD 1051/2022
art. 4.1 sets **one month**, and the decree is later and national, so the book
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
> = their running total per plot. Recording them twice would let one book state
> two different totals, so 7.1 is **worked out, not entered**. What is stored is
> the recommendation, which is exactly what RD 1051/2022 art. 5.a asks the
> cuaderno to carry.

### 8 Riego

Id. parcelas · Superficie regada (ha) · **Sistema de Riego** · Fecha/Intervalo
de riego · **Volumen de riego (m³/ha)** · **Volumen acumulado (m³/ha)**.
Footnote (1) is the list of irrigation systems, and it is **`SIST_RIEGO` word for
word**: 1 Superficie o Gravedad · 2 Aspersión fija · 3 Aspersión móvil ·
4 Microaspersión · 5 Nebulización · 6 Goteo · 7 Hidroponía a solución perdida ·
8 Hidroponía con recirculación.

> This is **not** the four-value SEC/ASP/LOC/GRA list of 2.1. The two answer
> different questions: 2.1 describes the *plot* (A.2.e), 8 records the system
> used for *this irrigation*, which is why `crop.irrigation_code` can't be mapped
> to `SIST_RIEGO`. Volumen acumulado is a running total, worked out like 7.1's.

### Section 9 — the eco-scheme registers

The numbering is the model's; the sixth duty has no page and the book gives it
one.

**9.1 (P1, art. 30.2 ter)** — Id. del grupo de parcelas⁽¹⁾ · Referencia SIGPAC
de la parcela o grupo⁽²⁾ · Fecha inicio de pastoreo · Fecha fin de pastoreo⁽³⁾ ·
Especie animal que pasta · REGA · N.º animales desplazados al pasto.
⁽¹⁾ only filled in for plots more than 10 km from the main livestock
installation; ⁽²⁾ plots less than 10 km apart may be treated as a group;
⁽³⁾ the entry is due within one month of the **END** of grazing.

**9.2 (P2, art. 31)** — Id. de parcelas · Provincia · Municipio · Polígono ·
Parcela · Recinto · Superficie SIGPAC (ha) · Siega: fecha de cortes⁽¹⁾ · Otras
actividades: Laboreo⁽²⁾ / Siembra⁽³⁾ / Otras activ. de mantenimiento⁽⁴⁾.
⁽¹⁾ two cuts a year, threshold 300 m altitude; ⁽²⁾⁽³⁾ give a date;
⁽⁴⁾ give a date **and** the activity.

**9.3 (P5, art. 45.2)** — Id. Parcelas · Fecha de siembra en seco · Fecha de
inundación · Fecha de seca para tratamiento herbicida o fitosanitario.
**Two of art. 45.2's five dates have no column** (nivelación and construcción
de caballones), so the book adds them, in the ARTICLE's order: nivelación ·
siembra en seco · inundación · seca · caballones. That keeps the model's own
three in their original order.

**9.4 (P6, art. 42)** — Id. Parcelas · Fecha de establecimiento⁽¹⁾ · Anchura de
la cubierta (m) · Anchura libre proyección copa (m) · Mantenimiento por medios
mecánicos: Siega / Desbrozado / Pastoreo⁽²⁾. ⁽¹⁾ live spontaneous or sown
cover; ⁽²⁾ slopes ≥ 10 % and bancales.

**9.5 (P7, art. 43)** — Id. Parcelas · Fecha de establecimiento⁽³⁾ · Anchura de
la cubierta (m) · Anchura libre proyección copa (m). ⁽³⁾ inert cover of
shredded pruning residue.

**"9.6" (anexo IV)** — there is no printed page. The book's own: Id. de
parcelas · Parcelas · Fecha · Fecha fin · Actividad de mantenimiento, with a
footnote saying the official model has no page for the duty.

**Section 10** has no fields; it points to sections 3, 7 and 8 (see the section
map).

> **There is no "APLICA TRATAMIENTO: SÍ / NO" box anywhere in section 9.** The
> `register_declaration` mechanism doesn't extend here and must not be extended:
> a farmer claiming no ecorrégimen isn't declaring the register empty, they are
> outside the scheme, which the section's introduction says in words. Every
> section 9 table therefore uses `blank_rows: 6`, never `0`.

### Anexo III Parte I sección C — the binding list for 6 and 8

RD 1051/2022 art. 5.d **and** 5.e both point here ("la información requerida en
la sección C de la parte I del anexo III del Real Decreto 1311/2012 … según
indica el anexo I"), so this, not the printed model, is the field list that
binds. Copied from the consolidated BOE text.

Recorded **per unidad homogénea de cultivo**: one crop, one titular, one
sistema de explotación (secano/regadío).

| | Field | Note |
| --- | --- | --- |
| a | Fecha de la aplicación | |
| b | Superficie en que se realiza | |
| c | Tipo de tratamiento | enmienda (orgánica, cálcica…), abonado de fondo, abonado de cobertera → **`TIPO_FERITILIZACION`** exactly |
| d | Tipo material empleado | 1º producto fertilizante (RD 506/2013 anexo I type, or Reg. 2019/1009 functional category) · 2º estiércol sólido, indicando especie · 3º purín, indicando especie · 4º otros materiales (RD 1051/2022 anexo VIII) → **`MAT_FERTI`** + **`DETALLE_MATERIAL_FERT`** |
| e | Supplier, for d.2/d.3/d.4 | nombre de la empresa suministradora + **REGA** (livestock farm) · **NIF** (centro de gestión de estiércoles) · **NIMA** (gestor de residuos), only one of them |
| f | Forma de aplicación | "en particular si es por fertirrigación, especificando si es por aspersión, localizada, etc." → **`METODO_APLICACION_FERTILIZANTE`** |
| g | Máquina empleada | **explicitly optional**, with its registration number where applicable |
| h | **Valor agronómico del material** | N total · N orgánico · N ureico · N nítrico · N amoniacal · P₂O₅ total · P₂O₅ soluble en agua · K₂O total: **eight values**, from the label, the material certificate or the art. 13.2 document that comes with manure |
| i | Lodos only | heavy-metal content per RD 1051/2022 anexo IV tabla A.1 |
| j | Dosis | cantidad del producto o material aplicado **por hectárea** |
| k | Empresa de servicios | when the applicator isn't the farm's own, with its **REGFER** registration number |
| l | Regadío only, subject to art. 17 | contenido de N nítrico en el agua de riego · contenido de P₂O₅ soluble en el agua · **cantidad de agua aportada en cada riego (m³/ha)** |

Three things the printed model doesn't show:

- **C.h is eight values, not three.** The model's "Riqueza N/P/K" is a subset,
  and C.i adds heavy metals whenever sludge is applied, so the composition can't
  be three columns.
- **C.k names a third machinery registry, REGFER** (created by RD 1051/2022
  art. 18), next to ROMA and REGANIP.
- **C.l's two water-quality values have no column in the model and no field in
  SIEX's `Riego`.** They appear in SIEX as
  `Fertilizacion.Fertirrigacion.DosisN`/`DosisP`. **Art. 17.2 makes them
  conditional**: required only when the organismo de cuenca, comunidad de
  regantes or similar provides the data, and voluntary when the holder analyses
  the water themselves. So they are accepted when given, never demanded.

### Documentación a conservar (annex page, 3 years)

Facturas/documentos de adquisición de fitosanitarios · contratos con empresas
o personas que realizaron tratamientos · certificados de inspección de los
equipos · justificantes de entrega de envases vacíos · boletines de análisis
de residuos (cultivos, producciones y, en su caso, agua de riego) ·
documentación del asesoramiento · albaranes o facturas de venta de la cosecha.

> **The model's list is wider than art. 16.3.** The article names the register
> of art. 16.1, the art. 11.2 advisory documentation, equipment inspection
> certificates, the Ley 43/2002 art. 41.2.c contracts, invoices and other
> supporting documents, and residue-analysis results, all kept **at least three
> years from their issue**. **Empty-container return receipts** and **harvest
> sale delivery notes** are NOT in it: the first comes from the container-return
> duty, the second from food-chain traceability (section 5's own basis). The page
> prints all seven, because the model is what an inspector knows, and its
> retention sentence cites art. 16.3 for the three years while naming the other
> two sources instead of implying the article covers them.

The page also lists the second decree's three documents: the plan de abonado
itself (art. 6), the *documento de aplicación de los lodos* issued by the
authorised manager (art. 5.g, anexo III of Orden AAA/1072/2013), and the
agronomic-quality document that has to come with manure received from someone
else (art. 13.2; **not needed when the holder supplies their own**, which the
printed item says). The retention sentence says the three years of art. 16.3
cover items 1–6, names the two that rest on other rules, and states that RD
1051/2022 sets no retention period of its own for items 8–10, rather than
letting one citation appear to cover ten items it doesn't.

## Capture design

The schema behind the sections above. Each register's tables, repository, tests,
UI and print are added together, and the migrations still run as one sequence
(`architecture.md`).

How a backup is checked against this schema's shape is explained in
`architecture.md`, in the paragraph on backups.

### Columns added to existing tables

| Table | Columns (can be empty unless stated) | Feeds |
| --- | --- | --- |
| `crop` | `area_ha`; `irrigation_code` → `irrigation_system`; `growing_environment_code` → `growing_environment`; `gip_system_code` → `gip_system`; `crop_code` (PRODUCTOS catalogue code, TEXT, **no foreign key**, as for `treatment_problem.problem_code`); where it came from: `source` (NOT NULL DEFAULT `'user'`), `source_campaign`, `declared_area_ha` | 2.1's agronomic columns; a surface per crop instead of repeating the plot's; the SIGPAC declared-crops prefill (`sigpac-integration.md` → "Declared crops") |
| `operator` | `tax_id` | 1.2 NIF |
| `machinery` | `acquired_on` | 1.3 |
| `farm` | `address`, `postal_code`, `phone_fixed`, `phone_mobile`, `email` | 1.1 (the same everywhere, so in core) |
| `farm_es_extension` | `siex_code` (Nº Registro Nacional; `rea_code` is the regional one) | 1.1 |
| `treatment_record` | `application_end_date` (end of the interval; the export's `FechaFin` falls back to the start date); `total_quantity_value` + `total_quantity_unit_code` → `unit` (Anexo III B.i; can't be worked out from a concentration dose, so the form only prefills dose × surface for per-ha doses) | 3.1 |
| `seed_treatment` | `treatment_kind_code` → `seed_treatment_kind` (can be empty, because the printed model has no such column) | 3.2, and SIEX's required `Tratamiento` |
| `analysis_record` | its material list coded as FEGA's four (`crop`, `harvested_produce`, `soil`, `water`), plus the `analysis_record_type` and `analysis_substance` junctions | 4 |
| `harvest_record` | `plant_product_code` (a PROD_VEGETAL code; see below) | 5 |
| `farm_representative` (new, 1:0..1) | `farm_id` PK/FK; name, tax_id, representation_kind, address, locality, postal_code, phone, email; reconciled from the submitted form like `farm_es_extension` | 1.1 titular-o-representante |

### Lookups (i18n keys only)

`irrigation_system` {rainfed, sprinkler, drip, gravity} ·
`growing_environment` {open_air, mesh, plastic_cover, greenhouse} ·
`gip_system` {organic, integrated_production, private_certification, atria,
advisor_assisted, not_required} · `analysis_material` {crop,
harvested_produce, soil, water} · `analysis_type` {pesticide_residues,
microbiological, heavy_metals, nutrients, soil_parameters, gmo_presence} ·
`seed_treatment_kind` {on_farm, processing_centre, purchased_es,
purchased_abroad}. The last three each map one-to-one onto their FEGA
catalogue, checked by a contract test. `licence_level` has `pilot`. `unit` has a
`quantity` dimension {kg, l, t, m3}: a quantity is an amount, not a rate, so
`list_units` (the dose picker) leaves them out and `list_quantity_units` is
their own list. The Spanish abbreviations (SEC/ASP/LOC/GRA, AL/M/BP/INV,
AE/PI/CP/Atrias/AS/NO, Básico…Piloto) are print-template content, not schema
values. 1.2's "Asesor" cross is **not** a carné level: it prints when the
operator's NIF matches an advisor.

### Core tables (changes logged, soft-deleted)

- `advisor`: name, tax_id, **`registration_number`** (not `ropo_number`: core
  tables carry no regional identifiers, as with `operator.licence_number`, and
  the model's own label is the neutral "Nº de identificación"). `farm_advisor`:
  farm ↔ advisor with `gip_system_code` (1.4's "tipo de explotación"), one
  ACTIVE link per pair, so stating a relationship again updates it instead of
  repeating the 1.4 row; deleting an advisor removes its links in the same
  transaction. 2.1's per-row GIP comes from `crop.gip_system_code`, falling back
  to AE/PI from `production_system`. The advisor isn't scoped to a farm: one
  advises many farms, and 1.2's Asesor cross is a NIF match against the whole
  registry; it says what the person *is*, not which farm they were advising.
- `plot_water_point`: plot FK, denomination, `inside_plot` (bool),
  `distance_m`, lat/lon (voluntary). 2.2's water half (A.1.f–g); the zones half
  reads `plot_zone_flag`. Four decisions:
  - **It comes from the printed model only.** The SIEX 3.11.4 schema has NO
    entity for an abstraction point at any level. Its one water field,
    `OrigenAgua`, sits under `Riego`/`Fertirrigacion` and codes where
    *irrigation* water came from, and FEGA's four water catalogues
    (`ORIGEN_AGUA_RIEGO`, `USOS_AGUA`, `REGANTES`, `COMU_REGA`) all belong to that
    irrigation vocabulary. So there is no SIEX field to mirror and no code to
    store; the requirement is the decree's.
  - **`distance_m` is REQUIRED when the point is outside**
    (`invalid.missing_distance`) and refused when it is inside
    (`invalid.water_point_distance_inside`). A.1.g asks for it in that case, and
    unlike efficacy or a total quantity, it is something the farmer already
    knows, so "accept now, complain at export" doesn't apply. A distance next to
    "included: SÍ" is a wrong answer, not a missing one.
  - **Flat and per plot**, not a separate point plus junction: `inside_plot`
    and `distance_m` describe the *(plot, point)* pair, so a well serving two
    plots would need a junction carrying both anyway. It is entered once per
    plot it concerns, which is exactly what the model's per-plot row says. Real
    geometry waits for the Irrigation module, where it belongs in `geo_feature`.
  - **Coordinates are stored as WGS84/ETRS89 lat/lon**, which the rest of the
    app already uses (SIGPAC queries `recinfobypoint/4326/…`, `geo_feature` is
    4326 GeoJSON, and the importer treats 4326/4258/4081 as the same). The
    model's "Coordenadas UTM" heading is relabelled "Coordenadas (lat, lon)"
    rather than converting behind the farmer's back into a projection nothing
    else uses. A UTM version can be added later from the same two numbers with
    no schema change.
- `plot_water_declaration`: plot FK, `declared_on`, one live row per plot
  (partial unique index). The stored "none" for 2.2's water half: an empty
  register looks the same as one nobody filled in, and only the first is
  evidence the farmer checked. **The rule works both ways**, as in
  `register_declaration` (same shape, different table: that one is
  module-phytosanitary's and scoped by farm and season; this one is core, per
  plot and not per season). Declaring a plot free of points while it has some is
  refused (`invalid.plot_has_water_points`), and recording a point **withdraws
  the declaration in the same transaction**, because a stale "no captaciones"
  printed next to a row contradicting it would be a false proof of checking.
  Withdrawing is a soft delete and stating it again creates a new row, so the
  history still shows what the farmer once declared.
- `harvest_record` + `harvest_plot`: season + farm scoped; date, product name,
  **quantity value + unit code** ({kg, t}: "value + unit code, never free text",
  and SIEX `ComercializacionVD` has a coded `Unidad`), delivery note and lot
  references, buyer (name, tax id, address, `buyer_registry_number`). In core,
  not module-phytosanitary: harvest is whole-farm data (costs, analytics), and
  modules never depend on each other. That is also why the column is the neutral
  `buyer_registry_number`, not `rgseaa_number`: core tables carry no regional
  identifiers, so the Spanish label "Nº RGSEAA" lives in the report labels and
  the UI dictionaries.

  Our `harvest_record` is SIEX's `ComercializacionVD`, not `Cosecha`, which is
  the field operation and is out of scope. `ComercializacionVD` has **no plot
  array and no buyer block**, so `harvest_plot` and the buyer block exist because
  the *printed model* asks for them ("Nº de orden parcela/s de origen",
  "Cliente… Nº de RGSEAA"). SIEX also requires **FechaInicio + FechaFin**, an
  interval; the record stores a **single `harvested_on`**, because the model
  prints one date column and section 5 isn't Anexo III Parte I content. A
  serializer meets SIEX by sending the same date for both ends.
  `ProductoVegetal` is a coded integer, which is why a code sits next to the free
  `product_name`, and `kg` (5) and `t` (6) both exist in `UNIDADES_MEDIDA`, so
  the allowed set maps cleanly.

  **The code is a PROD_VEGETAL code, not a crop.** PRODUCTOS codes the crop (101
  OLIVO); PROD_VEGETAL codes the harvested produce (1 Aceitunas), and the file
  states the relation itself, one row per (produce, crop) pair, so *Aceitunas*
  appears for both OLIVO and ACEBUCHE. SIEX's `ProductoVegetal` (and
  `Cosecha.ProductoCosechado`) point at *"Catálogo de Producto vegetal"*. So the
  column is **`plant_product_code`**, while `crop.crop_code` and
  `seed_treatment.crop_code` really are PRODUCTOS; two identical names against
  different catalogues is the trap the naming rule exists for. It isn't
  `product_code`, because in module-phytosanitary `product_*` always means the
  phytosanitary product. In the UI, `CataloguePicker.svelte` holds the type-ahead
  both fields share: `SpeciesPicker` wraps it with its SIGPAC land-use narrowing,
  and `PlantProductPicker` wraps it over `PROD_VEGETAL` (deduplicated). Section
  3.3's `subject_product_code` uses the same picker, shown for postharvest only,
  since the other two subjects are a building and a vehicle.

### module-phytosanitary tables (changes logged, soft-deleted)

- `non_field_treatment`: one table for 3.3/3.4/3.5. `subject_kind_code`
  {postharvest, storage_premises, transport}, `treated_on`,
  `subject_description` (producto vegetal / local tipo y dirección / vehículo
  tipo, modelo y matrícula), an optional `subject_product_code` for the
  postharvest kind (SIEX `TratamientosPostCosecha.ProductoVegetal` is a
  `PROD_VEGETAL` code, stored as it is with no foreign key), and the treated
  quantity + unit (t or m³; the repository enforces which goes with which,
  because recording a warehouse in tonnes is a different claim, not a unit slip;
  both are optional pairs, because the printed form leaves the cell to be filled
  by hand, and a format requirement belongs in an export precheck). **Same rules
  as `treatment_record`, matching SIEX**: coded problems (≥1) and justifications
  (≥1) in `non_field_treatment_problem` / `_justification` junctions,
  `operator_id` NOT NULL with name + licence snapshots, and `efficacy_code`
  optional (seen afterwards; a logged setter, asked for by print and export
  prechecks, never at insert). Product: foreign key + name/registration
  snapshots + quantity used (kg or l).
- `seed_treatment` + `seed_treatment_plot` (3.2): sown_on, species, variety,
  surface sown (ha), seed quantity (kg), seed lot (SIEX `NumeroLote`), product
  name + registration nº + active substance (free text: seed treated by a
  supplier is often not in the product registry; an optional `product_id` when
  it is), and an optional `efficacy_code` with the same set-later setter as
  `treatment_record` (SIEX `UsoSemillaTratada` requires `Eficacia`). It can be
  **fully corrected** (`update_seed_treatment`, with the sown plots reconciled
  from the submitted form): it holds no snapshot of another row, so there is
  nothing a later edit elsewhere could rewrite, which is exactly the situation
  the `*_snapshot` columns are for.

  Its `ProductosFito[].TipoProducto` is the `TIPO_PRODFITO` code that our
  `authorisation_kind` lookup already holds, so a serializer can supply it with
  no new storage.

  SIEX's required `Tratamiento` integer is **`TIPO_TRATAMIENTO`** ("Tratamiento
  semilla"), four values starting at 2: 2 realizado en la explotación · 3
  realizado en un centro de acondicionamiento · 4 adquisición de semilla tratada
  con producto autorizado en España · 5 adquisición de semilla tratada fuera de
  España. It is the `seed_treatment_kind` lookup and
  `seed_treatment.treatment_kind_code`, which can be empty: the printed model
  has no such column, so a book kept only to the model mustn't be blocked on it,
  while a stated value has to be one the export can send. It goes in the PDF's
  product cell and has its own spreadsheet column. The SIEX descriptor also
  states two rules a future export precheck must check: `NumeroLote` is required
  when the kind is 4 or 5, and `NumRegistro` is only allowed for 2 or 3.
- `analysis_record` + `analysis_plot` (4): sampled_on, `material_kind_code`
  {crop, harvested_produce, soil, water}, bulletin nº, lab name + address, **lab
  tax id** (SIEX `Analitica.Nif`), substances detected. Fully correctable, for
  the same reason as seed treatment.

  - `MaterialAnalizado` is a required **integer code**; `TiposSustancias[]` and
    `TiposAnalisis[]` are **coded arrays**. The three catalogues are
    `MATERIAL_ANALIZADO`, `SUST_ACTIVAS` and `TIPO_ANALISIS`, all vendored (FEGA
    publishes far more catalogues than we carry; check the registry,
    docs/maintenance.md §1, before deciding one doesn't exist).
  - `MATERIAL_ANALIZADO` has **four** values: 1 Cultivo · 2 Producto cosechado ·
    3 Suelo · 4 Agua de riego. FEGA tells the standing crop apart from the
    harvested produce, so our codes are `crop` / `harvested_produce` / `soil` /
    `water` (one-to-one, with `analysis_material_to_siex`). The report prints
    FEGA's own wording, not the model's hint *(vegetal / tierra / agua)*, which
    can't express the split.
  - `SUST_ACTIVAS` lists substances **with their CAS numbers**. The code is
    stored as it is with no foreign key, because CAS is the key a future French
    or Italian export would match on. `substances_detected` stays as free text
    next to the coded junction: SUST_ACTIVAS only codes phytosanitary actives
    (`TipoAnalisis` 1), so a heavy-metals, nutrients or soil bulletin has no
    code there and would otherwise be impossible to record. The junction
    (`analysis_substance`) **accepts a code the vendored snapshot can't
    resolve**: the snapshot comes with app releases, and a laboratory doesn't
    wait for one. The PDF joins the resolved names to the free text in the
    model's single cell; the sheet keeps them in two columns.
  - `TIPO_ANALISIS` has 6 values, including 5 "Parámetros del Suelo". It is the
    `analysis_type` lookup + `analysis_record_type` junction (one-to-one).
  - SIEX has a `Nif` but **no address**, while the printed model asks for
    "Laboratorio (nombre y dirección)", so both are stored and the PDF joins the
    three fields into the model's single cell, skipping whatever is empty.
  - `Analitica.ParametrosSuelo` carries pH, materia orgánica, P, K, N, texture
    and conductivity, so **Anexo III A.3's soil minimums live inside
    `Analitica`**, not in a block of their own (see "A.3's soil block" below).

  Neither junction appears in the printed model, which has no column for
  either: the kinds of analysis go in the material cell and the coded substances
  in the substances cell, and both get a **spreadsheet column of their own**,
  where they can be filtered.

  **No file attachments**, as a standing scope decision: the app has no
  attachments, and adding them has backup, sync and mobile-storage consequences
  of its own. The farmer keeps the bulletin; the documents-to-keep page says so.
- `register_declaration`: the explicit "APLICA TRATAMIENTO: **NO**" per (farm,
  season, register {seed_treatment, postharvest, storage_premises, transport}),
  with a partial unique index on the three where `deleted_at IS NULL`. SÍ comes
  from rows existing; the stored NO is deliberate proof of checking (like
  `plot_zone_flag`'s stored "outside"). The rule works both ways: declaring a
  register empty while it has records is refused (`invalid.register_has_rows`),
  and adding a record to a register declared empty **withdraws the declaration
  in the same transaction**: the record is the stronger statement, and a stale
  NO printed next to it would be a contradiction in a legal document.
  Withdrawing is a soft delete, so the history still shows the farmer once
  declared it, and stating it again creates a new row.

  So a register prints in **three** states, not two: rows tick SÍ, a declaration
  ticks NO, and an untouched register ticks neither. "Nobody filled this in"
  isn't the same claim as "nothing happened".
- **3.1 bis needs no tables of its own.** Anexo III Parte I B is one list a–k
  covering *every* treatment, and B.d reads "Identificación del aplicador **y, en
  su caso, del asesor**": the advisor is a field of the treatment, in the same
  sentence as the applicator. Nothing in the decree asks for a separate
  register, a non-chemical column or a signature. SIEX models it the same way:
  `AsesorValidacion` and `OtrasActuacionesFito` are members of `TratamFito`,
  whose required set (`IdAjenaTratamFito`, `FechaInicio`, `FechaFin`, `DGCs`,
  `ProblematicaFito`, `Justificaciones`, `IdentificadorAplicador`, `Eficacia`)
  pointedly **leaves out `ProductosFito`**.

  So `treatment_record` has three more blocks, and 3.1 bis is printed as a
  filtered view of it:

  - **the advisor** (`advisor_id` + `advisor_name_snapshot` +
    `advisor_registration_snapshot`), kept like the applicator's, so correcting
    an advisor's ROPO number never rewrites a past record;
  - **the non-chemical measure**: `measure_code` against
    **`TIPO_MEDIDA_FITOSANITARIA`**, its intensity as value + unit, and the
    measure's own registration number (SIEX's `NumRegistroMDF`);
  - **the chemical block, which can be empty**, so an action can be a product
    application, a measure, or both, but never neither.

  `MEDIDA_PREVENTIVA_CULTURAL` is a different thing: it hangs off
  `DatosExplotacion` (next to the parked `AltaDGC`/`CambioCultivoDGC`) and
  declares which IPM practices the FARM follows ("rotación de cultivos",
  "asesoramiento por un asesor en GIP"), with no date, no plot, no intensity and
  no column anywhere in the printed model. It has no reader, and is listed in
  `siex-export.md`'s inventory of what isn't sent.

  **The chemical block can only be empty as a WHOLE**, under a table CHECK; why,
  and what keeps a product from being stored without its PHI window, is in
  `data-model.md` → "The chemical block, and why it is nullable as a unit".

  **The intensity is mostly a count, and counts are words.** `UNIDADES_MEDIDA`
  publishes Trampas, Trampas/ha, Difusores, Difusores/ha and Unidades, so the
  `unit` table has an `intensity` dimension with its own picker (a number of
  traps is neither a dose nor an amount of product). Unlike `L/ha`, which reads
  the same in every language, "trampas" has to be translated, so it comes from
  `Labels`, not from `unit_symbol`, and the check that every seeded unit prints
  *something* covers both. **The picker also has the rest of Anexo V field 18's
  list**: uds./m², m² and m² de malla/ha for nets, and the two amounts it allows
  for a measure, kg and kg/ha ("fitosanitarios biológicos"), which print as
  symbols. `list_intensity_units` is both the picker and what the repository
  checks against. Litres aren't on the list, and nothing in either annex puts
  them there.

  **A basic substance is named in the measure's cell**: "Usos de sustancias
  básicas — Vinagre". Measure 11 says a basic substance (Reglamento (CE)
  1107/2009 art. 23) was used; `SUSTANCIAS_BASICAS` says which. No decree asks
  for it (B.g's product is an authorised one with a registration number, and a
  basic substance has neither) and the model has no column, so it is optional
  and goes in the cell next to the measure it describes. The 2027 SIEX model
  asks for it (Anexo 03 field 366).

  **The two validation boxes store nothing.** They appear once per sheet, they
  ask for a handwritten Firma, and the book has no signatures by design (as in
  1.1). The page prefills Asesor and Nº ROPO from its own rows, but only when
  every advised row names the same advisor: putting one of two names against a
  signature nobody gave would be the book claiming what it can't know.

  **The "Justificación de la actuación" column is already recorded, as codes.**
  The model leaves it free text; we hold the `treatment_justification` rows SIEX
  requires, and the cell prints their words. No second free-text field for the
  same fact.

  **Which rows appear** is decided from the record (it names an advisor or a
  measure), not from a flag on the crop. Nothing has to be kept in step for that
  to stay true, and a crop whose treatments name no advisor contributes nothing,
  which is a true statement about what was recorded.

### module-fertilisation tables

Sections 6, 7.1 and 8 are in their own crate, `module-fertilisation`, registered
after module-phytosanitary. It is a second module rather than more of
module-phytosanitary or core because of the placement rule in `architecture.md`:
this is a **domain with its own logic** (dose arithmetic, unidades
fertilizantes, plan against applied), not shared data and not presentation. Two
things follow from that: `unit` is in terrazgo-core (a module can't depend on
another module, and module-fertilisation needs dose and volume units), and
`terrazgo-recordbook` has its own error type, `RecordbookError`, which wraps
`CoreError`, the module errors and `ReportError` and is turned into a
`CommandError` by the shell. Units the sections add: `m3_ha` and `t_ha` (dose
rates); `m3` already exists as a quantity.

- `fertiliser_material`: the **registry**, reused across a campaign: commercial
  or material name, `material_code` (`MAT_FERTI`), `material_detail_code`
  (`DETALLE_MATERIAL_FERT`), supplier name and only one of `supplier_rega` /
  `supplier_tax_id` / `supplier_nima` (C.e), `manure_treatment_code`
  (`TRAT_ESTIERCOLES`), density and its unit. Soft-deleted and logged like
  `product`, for the same reason: a farmer applies one fertiliser many times a
  season, and C.h hangs eight agronomic values off it. Typing those again for
  every application is where wrong data comes from.
- `fertiliser_material_nutrient`: the composition, as **one coded junction**
  covering all three SIEX arrays: `(material_id, kind, code, percentage)` with
  kind ∈ {macro, micro, heavy_metal} against `MACRONUTRIENTES`,
  `MICRONUTRIENTES` and `METALES_PESADOS`. Codes are stored as they are, with no
  foreign key (the catalogue rule). One table rather than eight columns,
  because C.h asks for eight values, C.i adds heavy metals for sludge, and
  micronutrients exist: a fixed set of columns can't hold the last two. **The
  `kind` matters because the same number means a different substance in each of
  the three catalogues** (3 = N nítrico / Cobre / Plomo). The PDF joins N total
  / P₂O₅ total / K₂O into the model's single "Riqueza N/P/K" cell; the workbook
  has a tab with one row per value, where it can be filtered and summed.
- `fertilisation_record` + `fertilisation_plot`: the event (SIEX
  `Fertilizacion`). `applied_on` + `application_end_date` (C.a; art. 5.f allows
  adding up a fortnight for intensive or fertigated crops),
  `fertilisation_type_code` (C.c), `application_method_code` (C.f), dose value
  + unit (C.j), optional `machinery_id` (**C.g says it is optional in so many
  words**), service company + its `regfer_number` (C.k), `sludge_application`
  (C.i / art. 5.g), delivery-note reference (the model's "Nº de albarán"),
  estimated and final yield (the model's "Producción kg/ha"), notes. The
  material by foreign key **plus name and composition snapshots**. The plots
  junction holds the treated surface per plot, like `treatment_plot`.
- `irrigation_record` + `irrigation_plot`: SIEX `Riego`. `irrigated_on` +
  `irrigation_end_date`, `irrigation_system_code` (`SIST_RIEGO`, **per
  event**: the model's own §8 footnote is that catalogue word for word), volume
  value + unit (C.l's m³/ha), optional `energy_type_code` (`TIPENERGIA`) and
  `meter_number`, and the two conditional water-quality values from C.l,
  `water_nitric_n` and `water_soluble_p2o5`, which can be empty under art. 17.2.
  The water's origin (`ORIGEN_AGUA_RIEGO`) is a junction because SIEX's
  `OrigenAgua` is an array: one irrigation can mix sources.
- `fertilisation_plan`: SIEX `PlanAbonado`, whose required set is **exactly RD
  1051/2022 art. 5.a's list**: expected yield (`ObjetivoProduccion`), the
  previous crop (`CultivoPrecedente`, a `PRODUCTOS` code), the N / P₂O₅ / K₂O
  requirements, and the date the plan was drawn up. Scoped to the production
  unit, which for us is the crop row (the same unit the SIEX export already
  treats as the DGC). `Herramienta` (whether a calculation tool produced the
  plan) is a SIEX-only yes/no and is recorded, since it is one column and the
  export requires it.

7.1's aportadas and acumuladas, and 8's volumen acumulado, are **worked out,
never stored**: they are sums over the records above, and a stored copy is a
second number that can disagree with the first (the "don't store derived
values" rule). A.3's soil parameters extend `analysis_record` in
module-phytosanitary; there is no soil table.

### Section 6 in detail

- **The record keeps the material's coded kind as well as its name.** SIEX has
  `AplicacionMaterialFertilizante.NombreProducto` as a plain string next to the
  coded material, which supports keeping the name, but the model's own "Tipo de
  abono/producto" column prints C.d's *kind*, and C.d is binding. A record that
  named a manure must keep saying so after the registry entry is corrected, so
  `material_code_snapshot` is kept with it.
- **REGFER is in two places, and we store the decree's.** C.k attaches the
  REGFER number to the *empresa de servicios*; SIEX splits them, with
  `EmpresaServicios` on the application and `NumREGFER` inside
  `EquipoAplicador`. Both are stored on the record, together, and a serializer
  puts them where SIEX wants them.
- **`EquipoAplicador` has a `oneOf`** (ROMA / REGANIP / an applicator id). Since
  C.g makes the machine optional, an application without one leaves out the
  whole block, which the `oneOf` allows and a half-filled block wouldn't.
- **The printed "Tipo de fertilización" cell carries both legal fields.** The
  model's footnote lists (F) fertirrigación next to (AF) and (AC) as if they
  were one list; they aren't. So the book works out the abbreviation ("F/AC" for
  a fertigated cobertera, empty for an enmienda, which the model gives no
  letter) and spells out the forma de aplicación next to it. `is_fertigation`
  is stored on the lookup rather than matched on the code, so the logic reads it
  from data.
- **Fertigation is linked to its watering.** `Fertilizacion.Fertirrigacion` is
  the **only place in the format** that carries C.l's two water-quality figures,
  which `irrigation_record` records and no printed column and no member of
  `Riego` has. Without it, two columns of a *binding* Anexo III letter would be
  read by nothing. It is filled through `fertilisation_record.irrigation_record_id`,
  which the farmer sets on the §6 form when the method is a fertigation.
- **`GestionSostInsu`** is recorded as `sustainable_input_management`.
- **`BuenasPracticasRiego`** is **Voluntario** in Anexo V on both blocks. The
  watering's own practices are recorded (`irrigation_practice`, sheet "8 Riego"
  only), because the 2027 model makes them obligatorio condicionado to regional
  rules (Anexo 03 field 481); the copy in the fertigation block isn't sent.

**What decides whether a SIEX field is recorded:** not "required in the
schema", which is the JSON Schema's `required` and only means *structural
validity of an entry you chose to send*. It is **Anexo V's own `OBLIGATORIEDAD`
column**: a field FEGA marks `Obligatorio` inside a block we do send is a real
requirement even when no decree names it, and a field it marks `Voluntario` that
no page prints is a recorded gap. `BuenasPracticas` is recorded under both
readings; `GestionSostInsu` only under the second. The three questions this
separates (required, obligatorio and binding) are set out in
`docs/siex-export.md`.

**Code `0` stands alone.** `BUENAS_PRACTICAS_AMBITOS` starts with
`"0";"No realiza buenas prácticas"`, which can be claimed in every ámbito and is
an ordinary row as far as the file's shape goes, so a record could hold it next
to the others, claiming at once that nothing was done and what was done.
`validated_practices` refuses that pair, and the form never offers it. **This
isn't the case the "the picker narrows, never the repository" rule protects**
(see `soil_cover`): that rule exists because a catalogue *grows* between
releases, and refusing a code not on the list would make a lawful practice
impossible to record. Here both codes are known, published and fixed in meaning,
and a register that could hold the contradiction would export it as two
`BuenaPracticaFertilizante` entries. The section stays optional: an empty set and
a bare `0` are both valid answers; only the pair is refused.

### A.3's soil block

**The soil block is on `analysis_record`, in module-phytosanitary**, for two
reasons that agree. SIEX settles the first: `Analitica.ParametrosSuelo` is part
OF an analysis, because soil data reaches a farm as a laboratory bulletin like
any other. The module boundary settles the second: `analysis_record` is
module-phytosanitary's table, and a module can never add columns to another
module's schema. So although soil data is *used* by the fertilisation domain (RD
1051/2022 art. 5.b, and art. 6 makes it an input to the plan), the columns
belong to the crate that owns the register, and the record book reads across
both.

Nine figures, SIEX's own: pH, materia orgánica, P and K asimilables, N total,
conductividad, and **texture as three fractions** (arena / limo / arcilla)
rather than a class name. All optional: A.3's minimums only bind a year after
MAPA publishes its sampling guides, and a bulletin reports what was asked for.
The unit is part of the column name (`soil_available_p_mg_kg`, like
`water_nitric_n_mg_l`), which is safe where a farmer copies a figure into a
labelled field, and wouldn't be when importing a provider's number. The one
arithmetic rule: **the three texture fractions must add up to 100** when all
three are given, ±1 for a lab's rounding, since they are fractions of one soil.

The printed model is older than A.3 and has no soil page, so the figures go in
section 4's findings cell with a footnote saying so, and get a **"4 Suelo"**
workbook tab where each is a column of real numbers.

### Section 7.1 in detail

**Art. 6 defines a DOCUMENT and art. 5.a defines the RECORD.** The plan itself
must identify every recinto of the production unit, include soil parameters,
take rainfall and available irrigation into account, give the recommended dose
of each nutrient with its timing, material, way of applying and machinery, and
describe the anexo V emission measures; it is drawn up (with advice, once art.
6.6's transition ends) and kept. What goes in the book is art. 5.a's four items:
*rendimiento esperado, cultivo precedente, necesidades de N, de P₂O₅ y de K₂O y
fecha de elaboración del plan*. `fertilisation_plan` is art. 5.a, and SIEX's
`PlanAbonado` requires the same four, which confirms it.

- **The plan covers a production unit, not a parcel.** `PlanAbonado.DGCs` is an
  array and art. 4.2 says "por cada unidad de producción", so the covered crops
  are a junction, and the repository keeps a crop in at most one live plan:
  two plans recommending different nitrogen for one crop would make 7.1 print
  two different figures on one row.
- **A plan can be corrected by design**: art. 6 explicitly allows adjusting it
  during the campaign to follow the crop and the weather, so `drawn_up_on`
  changes with the correction.
- **The table is worked out.** Only the recommendation is stored; aportadas =
  dose × riqueza and acumuladas = their running total per production unit, both
  from section 6's own records (`module_fertilisation::agronomy`).
- **A volume dose needs the material's density to become unidades
  fertilizantes**, and when it is missing the cell is empty, never a guess of 1
  kg/L. And **the running total stops there and stays empty**: a total that
  silently left out a slurry application would read as the nitrogen already
  applied, and a farmer comparing it with the recommendation would
  over-fertilise because of it.

### Filling C.h from the catalogue

`DETALLE_MATERIAL_FERT` publishes the composition of each of its named products,
so choosing one fills the material's composition instead of asking a farmer to
copy fourteen figures off the sack. It only happens when asked for with a
button, and a line already entered is never overwritten: the label in the
farmer's hand is the source of truth, and the vendored snapshot comes with app
releases.

The same button fills the **density**, on the same terms (only into an empty
field), and only for a product the file marks `Líquido`: Anexo V asks for it
*"cuando se trate de producto fertilizante líquido"*, and the figure the file
gives for solids is a bulk density, which isn't what the field means. The column
is `Densidad (g/cm3)`, which is kg/L, the unit `density_kg_l` stores. Its `0`
means "not stated", as in the composition columns, including slurries, which are
exactly the volume-dosed materials whose density the farmer must enter.

**What isn't filled matters more than what is.** The file has three traps:

- **The seven heavy-metal columns mix units between rows, with nothing in the
  file to tell them apart.** "BASFOLIAR ZNMN" declares `Zinc (Zn) = 20,1`,
  clearly a percentage for a zinc foliar; "CODA-Ca-L" declares `Cobre (Cu) = 70`,
  `Plomo (Pb) = 45` and `Cromo total (Cr) = 70` on a product whose N, P and K
  are all zero, figures only sensible as mg/kg. Filling either way is wrong for
  the other **by a factor of ten thousand**, so C.i's metals are never proposed
  and are entered by hand from the analysis the farmer has.
- **`P_% TOTAL` and `K_% TOTAL` are elemental**, not oxides: the median ratio of
  `P2O5 % total` to `P_% TOTAL` across the rows is 2,2936, and the conversion
  factor is 2,2914. `MACRONUTRIENTES` only codes oxides, so mapping them would
  understate every product's P₂O₅ by more than half.
- **Copper and zinc as micronutrients** (`MICRONUTRIENTES` 3 and 6) have no
  column of their own: the file's only Cu and Zn columns are in the metals
  block, so a declared micronutrient can't be told from a contaminant.

Nineteen columns are safe and are mapped: the five nitrogen forms, three P₂O₅
forms, two K₂O forms, organic carbon, CaO, MgO, SO₃, and boron, cobalt,
manganese, molybdenum and iron. **A zero is never proposed**: the provider fills
unstated cells with `0`, and empty and zero are different claims. The column
list, the reasons and the tests are in `module_fertilisation::catalogue`.

**Why the mapping can't be automatic, and why dropping units wouldn't help.** The
table maps a column header onto a *catalogue code*, and **none of the
`MACRONUTRIENTES` labels appears as a column header**: "N_% TOTAL" → code 1 is a
human reading, with no string to match. The micronutrient and heavy-metal labels
do match word for word, but `Cobre (Cu)` and `Zinc (Zn)` each appear in **both**
catalogues, so an automatic matcher would have to guess exactly where the data is
ambiguous. And the unit isn't ours to drop: SIEX's field is `Porcentaje`, the
model's column header says "(%)", section 7.1 multiplies richness by a dose, and
RD 1051/2022 anexo IV gives the metal limits in mg/kg de materia seca; a figure
with no unit couldn't be compared with any of those.

Every numeric column must be either mapped or listed in `UNMAPPED_COLUMNS` **with
its reason**, so a refresh that adds a column fails until somebody decides about
it. An excluded column that stops being published fails too, since outdated
reasons go stale like anything else.

### Sections 9.4 and 9.5 in detail

**One register, two pages, split by the practice**, as with 9.2 and the book's
own "9.6". `soil_cover` holds both art. 42's live cover of spontaneous or sown
vegetation (P6, model 9.4) and art. 43's inert cover of shredded pruning residue
(P7, model 9.5). The two articles ask for the same three things,
`DatosCubierta` gives them one block, and `practice_code` decides the page.

**The row is the cover, not the plot.** That is the difference from 9.2 and 9.3,
which pivot onto the parcel: a cover has one establishment date and one pair of
widths however many plots it was established on, so there is nothing to collect
per plot and the register's own row is the printed one. The plots go in the
"Id. Parcelas" cell as table-2.1 cross-references.

#### Art. 42 is three entries, and the schema follows that

The printed model squeezes them into one row of columns. Read from the decree,
they are three facts with three deadlines, which arrive at three different
times:

| Clause | What is annotated | Deadline | Where it is stored |
| --- | --- | --- | --- |
| 42.1.a / 43.1.a | the establishment date | 1 month | `soil_cover.established_on`, the record itself |
| 42.1.e / 43.1.b | the cover width **and** the free canopy width | within the month before the 4-month live-cover period ends | `width_m`, `free_canopy_width_m`, `widths_stated_on`: can be empty, all three or none |
| 42.1.c | the maintenance done | within the month before the solicitud-única modification period ends | `cultural_operation` and `grazing_record` rows with a `soil_cover_id` |

What follows, in each case a place where the form would have misled a schema
copied from it:

- **A cover with no widths is a COMPLETE record.** Its second entry isn't due
  yet. So the cells print empty rather than zero, which would be a statement
  the farmer never made, and the advisory is what says the entry is still to
  make.
- **`widths_stated_on` is a column neither the decree nor SIEX asks for.** It
  exists because the deadline is what the entry is *about*: with it, "measured
  in June" and "never measured" can be told apart in a query. Without it they
  are the same NULL, and no advisory could tell them apart.
- **The widths go together or not at all** (`invalid.incomplete_widths`, like
  `plot_water_point.distance_m`): one width without the other, or a width with no
  date, is a *wrong* answer, not a missing one.

#### The maintenance is in other registers

A siega is a cultural operation and a pastoreo is a grazing, whatever land they
are on, so they stay in the registers that own them, linked back by a nullable
`soil_cover_id`. **SIEX agrees**: `DatosCubierta` in schema 3.11.4 has no
maintenance member at all, while the yes/no fields worked out from it sit on
`LaboresCulturales`.

That link also **splits two printed pages**. Model 9.1 prints the grazings with
no cover, model 9.4's Pastoreo column the ones with one. Without the split, a P6
cover grazing would also print on the P1 extensive-grazing page, which on a
document an inspector reads would be false, not just repeated. That is why
`GRAZING_PRACTICES` includes `plant_cover`.

The cover form still enters all three columns in one place. A maintenance line
only asks for its kind and date, taking the cover's plots, practice, farm and
season, and the repository writes it through the *same* functions the 9.2 and
9.1 forms use, inside the transaction that writes the cover. One validation path,
one logging path, and a book that never holds a cover whose maintenance was half
saved.

**Withdrawing a cover withdraws its maintenance**, each as its own logged soft
delete: those rows are art. 42.1.c's entry *for that cover* and print in its
columns, so a cover withdrawn as a mistake must leave no siega behind pointing at
nothing.

#### Two things the pages don't print, and one the form doesn't offer

- **`cover_type_code` has no printed column.** Art. 42.1.a records the *date* a
  cover was established, not which of "espontánea o sembrada" it was, so the
  distinction is in the printed footnote. The field is recorded because
  `DatosCubierta.TipoCobertura` asks for it, and the workbook has it.
- **Model 9.5 has no maintenance columns**, because art. 43 asks for none. A
  maintenance line against an inert cover is refused
  (`invalid.maintenance_on_an_inert_cover`) rather than stored where no page
  would print it.
- **`TIPO_COBERTURA_SUELO` is narrowed by the picker, never by the repository.**
  Art. 42.1.a's codes are 2 and 3, art. 43.1.a's is 4, and specifically not 5,
  "otros materiales", which is nutshells and stones rather than pruning residue.
  But the catalogue grows between releases, and the in-app refresh runs on the
  user's machine, so refusing an unknown code would stop a farmer recording a
  lawful cover. A picker may offer less than the record accepts, never the
  reverse. A contract test accounts for every active code (claimed by a practice,
  or listed in `NON_COVER_TYPES` with its reason), so an upstream addition makes
  somebody decide.

### The sowing register, and where 9.3's five dates live

Model 9.3 is the only page in the book put together from **three tables in three
crates**, because art. 45.2's five dates are five different kinds of fact:

| Date | Where it is recorded | Why there |
| --- | --- | --- |
| Nivelación | `cultural_operation`, kind `levelling`, practice `flooded_biodiversity` | it is work on the land, and `TIPO_LABOR` 2 is literally "Nivelación en cultivos bajo agua" |
| Siembra en seco | `sowing_record.sown_on` (**core**) | a sowing is a farm event, not an eco-scheme one |
| Inundación | `sowing_record.flooded_on` | it belongs to that sowing: the same seed, watered later |
| Seca | `treatment_record.drying_date` (**module-phytosanitary**) | the model says "fecha de seca **para tratamiento**", and SIEX puts `FechaSeca` on `TratamFito`: the field is dried *in order to* spray |
| Construcción de caballones | `cultural_operation`, kind `ridging` | `TIPO_LABOR` 3, "Caballones y tablas en cultivos bajo agua" |

**`sowing_record` and `sowing_plot` are in `terrazgo-core`**, like
`harvest_record`: sowing is harvest's mirror image, the two bracket a crop, and
crop planning, costs and analytics will all want it. So core holds the crop's
three brackets: `crop`, `sowing_record`, `harvest_record`. `sowing_plot` mirrors
`harvest_plot` field for field, **including having no surface column**: model
9.3 asks which parcels, not how much of each.

**It has no eco-scheme practice code**, and can't: core can't reference a
module's lookup. What marks a sowing as a *cultivo bajo agua* is `flooded_on`, a
core fact, which also decides which plots appear on the page. A sowing with no
flooding date isn't on its own evidence of a flooded crop, or every wheat sowing
on the farm would print on a page about rice.

**`flooded_on` is usually filled by a CORRECTION.** A rice grower dry-sows in
April and floods in May, and each is recorded within a month of its own
activity, so the row has an empty flooding date for a month. That is why the
page also admits a plot on *other* evidence (a nivelación, a seca), and why, once
a plot is in, every sowing on it prints its date.

#### What `SiembraPlantacion` asks for and this register doesn't hold

SIEX asks for more than the duty does, and most of the difference is **recorded
as a gap rather than captured** (`siex-export.md`), following the standing rule:
a field SIEX requires is captured even with no model column; a field optional in
SIEX AND missing from the model is recorded as a gap.

- `Cantidad` (kg of seed) **is** required, so `seed_quantity_kg` exists although
  no page of section 9 prints it.
- `SiembraDirecta` can already be recorded as a `cultural_operation` of kind
  `no_tillage`; recording it twice would be two statements of one fact.
- `MaterialTratado`, `MaterialAdquirido`, `FechaAdquisicion` and `NumLote` repeat
  what model 3.2's `seed_treatment` holds. **The two registers stay separate
  tables**: the printed model keeps them as two, filled separately, and their
  junctions prevent merging them. `seed_treatment_plot.surface_sown_ha` is `NOT
  NULL` because model 3.2 prints "Superficie sembrada (ha)", while model 9.3 asks
  for no surface, so a merge would either weaken an existing register or invent
  a required field. **They are linked**: `seed_treatment` has an optional
  `sowing_record_id` the farmer sets on the 3.2 form, plus `acquired_on` for the
  one member nothing else stored. Three of the four members need no extra
  field: `MaterialAdquirido` is `treatment_kind_code` (FEGA's TIPO_TRATAMIENTO
  4 and 5 *are* "adquisición de semilla tratada"), `MaterialTratado` is whether
  a 3.2 record exists, and `NumLote` is `seed_lot`. Reasoning in
  `siex-export.md` → "Treated seed and the sowing it was used for".
- The required `SiembraPlantacion` member is **`sowing_record.kind_code`**. The
  WS descriptor types it `number(1)`, "1 Siembra 0 Plantación" (Anexo V's
  "Cultivo" is `DGCs[].CodigoCultivo`, per DGC, not this). The form already
  answers it: the register is called "Siembra y plantación" and asks how each
  crop began, so a planting is one of its uses and a constant would misstate
  every one. No decree asks for a planting entry, which is why the column records
  something the form already asked rather than something the format invented.
  (`MATERIAL_VEGETAL_REPRODUCCION` isn't its catalogue; that file has no reader,
  `maintenance.md` §1.)

## Two outputs, one assembly

The book is read from the database **once**, into a typed `Cuaderno`
(`terrazgo_recordbook`), and rendered twice:

| | PDF | .xlsx |
| --- | --- | --- |
| Engine | Typst (`terrazgo_report::render_pdf`) | `terrazgo_report::render_xlsx` over `rust_xlsxwriter` |
| Cells | ready-formatted **strings** (dd/mm/yyyy, decimal commas); the template only does layout | typed **values**: real dates, real numbers |
| Cross-references | order numbers only (models 1.2/1.3/2.1 ↔ 3.1), as the official form prints them | order numbers **and** the names they point to, so the sheet can be filtered on its own and still matches the PDF row for row |
| Layout | the official model's tables | one tab per section, bold frozen header, autofilter, column widths |

Why it matters: a farmer sorting by date, filtering a product or adding up
treated hectares needs values, not display text. Numbers have **no** number
format: Excel shows them in the reader's own locale, which is how a Spanish user
gets decimal commas without the app hardcoding them. Dose value and dose unit are
in separate columns for the same reason: `1,5 L/ha` in one cell can't be summed.

Empty stays empty in both. An unknown surface is an empty cell, never a zero: a
spreadsheet would happily add the zeros up, and the official form leaves the
cell to be filled by hand. Manual application is the exception that proves it:
"Manual" is a value the model defines (3.1 footnote 3), so it is written, not left
out.

Adding a field means changing the assembly once. Both renderers are checked by
tests against the same fixture (`cuaderno_inputs` for the PDF's JSON contract,
`cuaderno_workbook` for the sheet's cells), so neither can drift silently.

## The book is one country's model

Everything *around* the book is scoped by country, and the book deliberately
isn't.

The three abbreviation mappers (`gip_abbrev`: AE/PI/CP/Atrias/AS/NO;
`irrigation_abbrev`: SEC/ASP/LOC/GRA; `environment_abbrev`: AL/M/BP/INV) print the
Spanish official model's own vocabulary with **no country check**, and that is
correct. They are exactly as Spanish as the document that prints them, and
checking the country field by field would swap an obviously wrong document for a
**silently wrong one**: an empty GIP column on an otherwise complete Spanish
model, which is worse than a Spanish model plainly printed for a farm that
should never have asked for it.

**A second country gets a second record book with its own layout.** When there
is one, the check belongs where the renderer starts (a `book_layout(country) ->
Option<Layout>` map, the same shape as `premises_class_catalogue`), never in each
field. Until then such a function would have one `Some` arm and no caller, which
is the kind of thing the project doesn't build before it is needed.

The same goes for `region.rs`: which languages a book can be printed in comes
from INE province codes and statutes of autonomy, which is Spanish policy in a
crate that renders a Spanish document. It becomes a per-country question when
there is a second layout to ask it about.

## Language of the book

The **layout is per country**: the Spanish official model, one template, never
forked. The **language is per region**: Castilian is official across Spain, and
where a statute of autonomy makes another language co-official, the farmer must
be able to hand an inspector the same book in either. Both documents (PDF and
.xlsx) take a language; the choice is next to the export buttons and starts on
the app's language when that language is official for the farm.

Available: Castilian and Catalan. Adding one is a single `Labels` const in
`terrazgo_recordbook::labels`. The region map already lists Galician, Basque and
Valencian, and only offers those that have a dictionary, so a language appears
as soon as it can be printed and not before.

### What translates, and what doesn't

| | Example | Why |
| --- | --- | --- |
| **Text translates** | headings, footnotes, "Buena/Bona", "Cualificado/Qualificat", the PHI phrase, 2.2's "Sin afección/Sense afectació" | it is the form's own wording, and the form is what the reader reads |
| **Codes don't** | SEC/ASP/LOC/GRA, AL/M/BP/INV, AE/PI/CP/Atrias/AS/NO, `L/ha`, FEGA catalogue labels for "problema fitosanitario", SIGPAC land-use codes | the record's legal value is the code; the footnote that explains an abbreviation is what carries the language |
| **User data never** | farm and plot names, species, varieties, notes | it is the farmer's text, in whatever language they wrote it |

So a Catalan book prints Spanish pest names under Catalan headings, on purpose:
those labels are the authority's own catalogue wording, and the export sends the
code anyway.

Dates stay `dd/mm/yyyy` and numbers keep the decimal comma in both languages.
`format_date`/`format_number` are shared, and will only take a language argument
when a language needs different ones.

### Which languages a farm is offered

`terrazgo_recordbook::region` maps INE province codes to the co-official
languages there, using both the farm's registry province
(`farm_es_extension.province_code`) and each plot's SIGPAC province: a farm can
cross a boundary, and offering one language too many costs nothing while
offering one too few hides a right. A farm with **no** province recorded is
offered every available language rather than none: an empty form field says
nothing about what the farmer may print.

Two gaps, each a one-line change once the dictionary exists: Valencian
(provinces 03/12/46) waits for its own entry rather than being offered as
"Català", and Aranese is left out because it is co-official in one valley, not
in province 25.

## Print conventions

- Sections print in the model's order. A register with no rows prints as an
  empty table to fill by hand (several empty rows, not one): an empty
  "postcosecha" table reads as "no postharvest treatments", while a missing
  section reads as an incomplete book.
- The "APLICA TRATAMIENTO: SÍ/NO" boxes are part of the form: SÍ comes from rows
  existing; NO is an explicitly stored declaration (proof of checking, like
  `plot_zone_flag`'s stored "outside").
- Changes from the model are allowed (the content binds, the layout doesn't):
  3.1 has an extra "Plazo de seguridad" column, plus the total quantity used and
  date intervals, per Anexo III B.
- The export file name has the campaign, the language and the date
  (`cuaderno_2025-2026_ca_20260803.pdf`), because the language is chosen per
  export and never saved, so the file itself has to say which one it is.
- **Check a layout change by rendering the page, not by extracting its text.**
  `pdftotext` gives the same words whatever the layout does, so it can't see a
  cell that wrapped to fourteen lines or a table that ran off the sheet. Render
  to an image (`pdftoppm`) and look. Two decisions in this file (the BBCH cell
  printing the number rather than FEGA's sentence, and section 2.1's font size)
  came from looking at the rendered page and wouldn't have come from any text
  dump.
