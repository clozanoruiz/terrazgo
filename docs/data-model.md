# Data model — the database schema, explained

> Companion to [architecture.md](architecture.md) (which explains *why* the
> model looks like this — see "The data model in five ideas"). This file is
> the per-table reference: what each table is, how they relate, and which
> rules each one participates in.
>
> **The DDL is the source of truth**, and it is deliberately well-commented —
> read it alongside this doc:
> [`crates/terrazgo-core/migrations/0001_core_schema.sql`](../crates/terrazgo-core/migrations/0001_core_schema.sql)
> and [`crates/module-cue/migrations/0001_schema.sql`](../crates/module-cue/migrations/0001_schema.sql).
> Update this file whenever those change.

## Conventions (once, for every table)

- `snake_case`, **singular** table names, lowercase English enum values.
  English throughout — i18n is a display concern; the Spanish regulatory
  term for each entity is mapped in the table below.
- **User-data PKs are UUIDv7** as 36-char TEXT, generated in Rust
  (`Uuid::now_v7()`) at insert — never in SQL. Lookups use short TEXT codes.
- Timestamps `TEXT` ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`); date-only fields
  `YYYY-MM-DD`. Surfaces in hectares (`REAL`). User-data tables carry
  `created_at`/`updated_at` (not repeated in the tables below).
- `foreign_keys = ON` and WAL are set at connection time, not in the schema.

## Entity ↔ Spanish regulatory term

The schema is English; these are the regulatory concepts each entity models.
(Core owns the farm registry; the CUE module gives the entities their Spanish
regulatory meaning.)

| Schema name | Spanish regulatory term | Notes |
|---|---|---|
| `farm` | Explotación | Holder (titular) + tax id; REA/REGA codes in Spanish extension table |
| `plot` | Parcela / Recinto | SIGPAC ref in extension table |
| `crop` | Cultivo | Species, variety, production system |
| `treatment_record` | Tratamiento fitosanitario | Central entity; audit-trailed |
| `treatment_plot` | — | Junction: treatment ↔ plot, surface treated per plot |
| `treatment_problem` | Problemática fitosanitaria | Junction: coded problems treated (catalogue codes per category) |
| `treatment_justification` | Justificación de la actuación | Junction: IPM justifications for treating |
| `product` | Producto fitosanitario | Active substances, PHI days |
| `product_authorisation` | Nº de registro | Junction: product ↔ country, MAPA number for ES |
| `operator` | Operador / Aplicador | Licence number, level, expiry date |
| `advisor` | Asesor / entidad de asesoramiento | Model 1.4: name or razón social, NIF, registration nº (ROPO in Spain) |
| `farm_advisor` | — | Junction: farm ↔ advisor, carrying the GIP framework (model 1.4's "tipo de explotación") |
| `machinery` | Maquinaria | ROMA/REGANIP numbers, inspection (ITV) dates |
| `season` | Campaña agrícola | Year, active/archived — referenced by every record |
| `alert` | Alerta | Derived: PHI windows, licence expiry, ITV expiry |
| `record_change` | — | Append-only audit log for regulatory records |
| `export_alias` | — | Integer ids regulatory exports assign to records (SIEX `IdAjena*`) |

## Four kinds of table

Every table belongs to exactly one category, and the category answers most
questions about it:

| Category | Tables | PK | Synced? | Audited in `record_change`? | Soft delete? |
|---|---|---|---|---|---|
| **Reference / lookup** — ships with the app, seeded by migration | `country`, `production_system`, `licence_level`, `irrigation_system`, `growing_environment`, `gip_system`, `zone_type`, `unit`, `reason_category`, `formulation_type`, `alert_type`, `efficacy`, `justification`, `authorisation_kind` | TEXT code | no (app-versioned) | no | no |
| **Imported reference** — provider catalogue snapshot vendored in the binary, imported at startup | `catalogue`, `catalogue_code` | TEXT id / INTEGER | no (each device imports its own copy) | no | no — the provider retires codes by baja date; imports upsert and never delete |
| **User data** — created on a device | `season`, `farm`, `plot`, `crop`, `operator`, `advisor`, `farm_advisor`, `machinery`, `user_profile`, `geo_feature`, `active_substance`, `product`, `product_active_substance`, `product_authorisation`, `treatment_record`, `treatment_plot`, `treatment_problem`, `treatment_justification`, `export_alias` | UUIDv7 | yes (Stage 2+) | yes, full row images | on the regulatory ones (see below) |
| **Regional extension** — attributes of a user-data row for one country | `farm_es_extension`, `plot_es_extension`, `machinery_es_extension` | parent's id | yes (as part of parent's domain) | yes (own entity) | no — hard-deleted when the form clears them (null after-image logged) |
| **Derived / infrastructure** | `alert` (derived), `record_change` (infrastructure) | UUIDv7 | no / is the sync source | `alert`: never. `record_change`: is the log | no |

The dividing question for lookup vs user data is *"can two devices create
this independently?"* — that is why `active_substance` is user data (an
offline farmer must be able to record an unknown substance) even though it
feels like a catalogue.

Soft delete (`deleted_at`) exists on: `farm`, `plot`, `crop`, `operator`,
`advisor`, `farm_advisor`, `machinery`, `user_profile`, `geo_feature`,
`product`, `treatment_record` and `season`. `farm_representative` is the exception alongside the `*_es_extension`
rows: reconciled from the submitted form state, so removing the block hard-deletes
it with a null after-image. On `season` the two lifecycles are orthogonal and both exist for a
reason: `status` archives a campaign that holds real records, while
`deleted_at` removes one created by mistake — and deletion is refused while the
season still has crops or treatment records, because every record-book view is
season-scoped, so hiding the season would hide its records with it. Junction rows (`treatment_plot`,
`treatment_problem`, `treatment_justification`, `product_active_substance`,
`product_authorisation`) live and die with their parent (`ON DELETE CASCADE`
guards the pre-release hard-delete path; in practice regulatory parents are
only soft-deleted). `export_alias` rows are never updated or deleted at all —
an alias is the edit/delete key on the authority's side, so stability across
exports is the entire point.

## The farm registry (core)

Owned by `terrazgo-core`: land, calendar, people, machines — the entities
every module builds on. `──<` reads "one … has many".

```
country (lookup)
   ▲
   │ country_code (NOT NULL — treatments derive their country from here)
  farm ──< plot ──< crop >── season
   │         │        │
   │         │        ├── production_system, irrigation_system,
   │         │        │   growing_environment, gip_system (lookups)
   ├──< machinery
   │
   ├──< farm_advisor >── advisor   (advisory relationship + its GIP framework)
   │         └── gip_system (lookup)
   │
   ├── farm_representative     (1 : 0..1  who signs when not the holder)
   ├── farm_es_extension       (1 : 0..1  REA + REGA + SIEX codes, province)
   │    plot_es_extension      (1 : 0..1  full SIGPAC reference)
   │    machinery_es_extension (1 : 0..1  ROMA / REGANIP numbers)
   │
  operator (standalone — people are not owned by a farm)
   └── licence_level (lookup)

  unit (lookup — every module that records an amount reads it)

  user_profile (standalone — who uses the app; optional operator_id link)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `season` | Campaign (campaña agrícola), e.g. 2025/2026 | On every regulatory record. `campaign_year` + free `label`; archives (`status`) when it holds records, soft-deletes only while empty |
| `farm` | Explotación | `country_code NOT NULL` — the schema itself rejects country-less farms, because treatment authorisation checks derive from it. `owner_tax_id` is the holder's tax/identity number (NIF/CUAA/SIREN…) — a universal concept regulatory exports need, so it lives in core; format validation is per-country |
| `plot` | Parcela / recinto | `farm_id` is **immutable by design** — no API moves a plot between farms, since that would silently re-home its history |
| `crop` | What grows on a plot in a season | The (plot, season) pair is the unit treatments point at; indexed on it. Carries the model's 2.1 agronomic columns: own `area_ha` (per crop, so a shared plot is not double-counted), `irrigation_code`, `growing_environment_code` and `gip_system_code` — the last is nullable and the printed book falls back to what `production_system_code` implies (organic → AE, integrated → PI). `crop_code` is the species' code in the FEGA PRODUCTOS catalogue, stored verbatim and deliberately **without** a foreign key (the `treatment_problem.problem_code` rationale: the code is the payload, the catalogue row is display metadata, and a reimport must never cascade into records); free-text species stay valid and simply carry no code. Provenance — `source` (`user`/`sigpac`), `source_campaign`, `declared_area_ha` — records whether a row was typed or came from a PAC declaration, and which campaign's. `declared_area_ha` sits **beside** `area_ha`, never instead of it: one is what a third party recorded, the other is the farmer's own figure. `UpdateCrop` treats the three provenance fields as set-if-present, so a manual correction through a form that does not carry them cannot erase where the row came from |
| `operator` | Aplicador with licence | `licence_expiry_date` drives `licence_expiry` alerts |
| `advisor` | Asesor, agrupación or entidad de asesoramiento (model 1.4) | Standalone like `operator`: one advisory entity serves many holdings. Frequently a company, so the identity is a `name`, not a person's name; `registration_number` is the model's "Nº de identificación" (the ROPO advisor inscription in Spain), named generically because core carries no regional identifiers. Advising is **not** a `licence_level`: ROPO registers applicators and advisors as separate conditions, which is why the model prints a separate "Asesor" cross in 1.2 — the printed book derives it by matching the operator's NIF against this table |
| `farm_advisor` | Farm ↔ advisor, with the GIP framework | The framework belongs to the *relationship*, so it lives on the junction, not on `farm`. One ACTIVE link per (farm, advisor) (partial unique index): restating a relationship updates it in place instead of printing the advisor twice in table 1.4. Soft-deleted, and deleting an advisor detaches its links in the same transaction, each logged on its own |
| `user_profile` | Who uses the app | Identification, not security: no credentials — real authentication belongs to cloud sync. The id is the author stamp on `record_change.actor` (every repository write takes an `actor` parameter; the shell passes the device's active profile id), so rows are only ever soft-deleted. The stamp is verbatim, never validated: a foreign device's claim must survive sync. Optional `operator_id` link ("this user is this applicator") must point at a non-deleted operator. The ACTIVE profile is a per-device choice in `settings.json`, never in this table |
| `machinery` | Equipment, per farm | `next_inspection_due_date` (ITV) drives `itv_expiry` alerts |
| `premises` | A place or vehicle on the holding that a treatment can be applied to: model 3.4's "local tratado", model 3.5's "vehículo tratado" | **A registry because RD 1311/2012 Anexo III Parte I B.b asks for an IDENTIFICATION** — "la parcela, o en su caso, local o medio de transporte tratado" — and a description retyped on every record identifies nothing: two treatments of one warehouse can spell it differently and nothing ties them together. In core because a store is holding infrastructure like `machinery`, and module-fertilisation is a plausible second consumer (manure and fertiliser stores) that may never depend on module-cue. **One table for both kinds**: B.b names them in one breath and the exchange format folds both into a single `Edificaciones` block, so two tables would differ in three columns. `kind_code` is core-native (`building` / `vehicle`) because the register's own vocabulary (`storage_premises` / `transport`) is module-cue's and core may not reference a module lookup — the `sowing_record` precedent. The `name` is also what prints as the model's "tipo": a second free-text type field beside it would be asking twice. `volume_m3` is the CAPACITY; B.f's *treated* volume is per treatment and stays on the record. **Buildings-only fields arrived on 2026-08-21** once `Edificaciones[].IdEdificacion` was settled as REA's own key (`docs/siex-export.md`). In core: `class_code`, an `EDIFICACIONES_INSTALACIONES` code stored verbatim with no FK, narrowed by the picker rather than the repository so a class published between releases stays recordable — country-neutral by construction, the `crop.crop_code` precedent. **It never reaches a treatment's printed cell**: `describe_premises` composes that from name + address (3.4) or name + model + plate (3.5), and folding a catalogue label in would let a refresh silently restate stored records; a unit test pins its absence. A vehicle carries none of this — FEGA's catalogue is all real estate and a lorry has a matrícula |
| `unit` | Units of measure, with a `dimension` | Moved here from module-cue on 2026-08-07: module-fertilisation records doses and irrigation volumes, and a module may never depend on another module, so the shared vocabulary had to sit below both. `dimension` is what stops a total being offered where a rate belongs (`dose_rate` / `concentration` / `quantity`), which is why the selectors are two separate lists. The move also let `harvest_record.quantity_unit_code` become a real foreign key |
| `*_es_extension` | Spanish registry identifiers | Regional IDs never sit in core tables; a French module would add `*_fr_extension` tables, not columns. Farm carries both `rea_code` (the farm registry — the SIEX export's CodigoRea) and `rega_code` (the *livestock* registry): different registrations, both user-entered |
| `geo_feature` | Geometry attached to a plot or farm (boundaries) | **Exclusive arc**: one nullable FK per subject (`plot_id`/`farm_id`) + CHECK exactly one — real FK enforcement where `record_change`/`alert` deliberately go polymorphic, because a geometry must die with its subject. GeoJSON in EPSG:4326; `source` (`manual`/`import`/future `sigpac`) rows coexist for discrepancy display; partial unique indexes allow one ACTIVE row per (subject, role, source) — replacement soft-deletes, history is kept. `official_area_ha` is provider-declared and never overwrites `plot.area_ha`; `properties` holds provider attributes as JSON (promoted to real columns only on proven need). Fetched geometry cannot be re-derived offline, so it syncs and is audited like any user data — unlike map *tiles*, which live in the separate `geo-cache.db` (own migration runner, never in backups or `record_change`) |

## The fertilisation domain (fertilisation module)

Owned by `module-fertilisation` (2026-08-07). Model sections 6, 7.1 and 8, under
**RD 1051/2022** rather than RD 1311/2012 — a second decree feeding the same
book, with its own deadlines. Like every module it may reference core tables and
no other module's.

```
season ──< irrigation_record >── farm       model section 8
              │        └── irrigation_method (lookup, SIST_RIEGO)
              ├──< irrigation_plot >── plot, crop?
              └──< irrigation_water_origin >── water_origin (lookup)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `irrigation_record` | One irrigation, or one accumulated period of them | The binding field list is RD 1311/2012 Anexo III Parte I **sección C** (a, b, l), which art. 5.d/5.e redirect to — not the printed model, which predates the decree. Carries a date **interval** (art. 5.f allows fortnightly accumulation for intensive and fertigated crops), the volume as value + unit ({m³/ha, m³}, repository-enforced), and C.l's two water-quality figures, which are **nullable by design**: art. 17.2 requires them only when the basin authority or irrigators' community supplies them. **Fully correctable**, the `seed_treatment` condition — it snapshots no other row's identity |
| `irrigation_method` | The eight `SIST_RIEGO` systems | **Not** core's four-value `irrigation_system`, and both exist for that reason: core's characterises the PLOT (A.2.e, and one of its values is "rainfed"), these describe how one watering was done. A "sprinkler" crop can be watered by a fixed installation one week and a mobile one the next — which is why the recorded `crop.irrigation_code → SIST_RIEGO` mapping gap was never closable on the crop |
| `irrigation_water_origin` | Where the water came from | A junction, not a column: the SIEX twin's `OrigenAgua` is an array, because one irrigation can mix a river and a borehole |

> **A FERTIGATION is one act the decree records twice**, and the two records are
> linked since 2026-08-21 by `fertilisation_record.irrigation_record_id`
> (nullable): art. 5.d puts the fertiliser in §6's register and art. 5.e puts the
> water in §8's, while the exchange format re-joins them as
> `Fertilizacion.Fertirrigacion`.
>
> The link exists because that sub-block is the **only reader anywhere in the
> format** of `irrigation_record`'s two C.l water-quality figures — no printed
> column and no member of `Riego` carries them — so without it two columns of a
> binding Anexo III letter are captured for nobody. It is **refused unless
> `application_method.is_fertigation`**, because on any other method it would
> assert a fertigation that did not happen; that flag is read from the lookup
> rather than matched on the code, which is what the column was put there for.
> Validated against the same farm and campaign and refused for a withdrawn
> watering, like `seed_treatment.sowing_record_id`. Reasoning in
> `siex-export.md` → "Seam 3: two recorded gaps that should not have been gaps".
>
> **`fertilisation_record.sustainable_input_management`** arrived with it: Anexo
> V marks `Fertilizacion.GestionSostInsu` Obligatorio, no decree names it and no
> printed cell carries it, so it rides in the spreadsheet's own column beside the
> sludge flag.

### Sowing and planting (core)

| Table | What it is | Worth knowing |
|---|---|---|
| `sowing_record` | How a crop began | **Harvest's mirror image, and in core for the same reason** — the two bracket a crop, so core holds all three of `crop`, `sowing_record` and `harvest_record`. It carries **no eco-scheme practice code** and cannot: core may not reference a module's lookup. What marks one as a *cultivo bajo agua* is `flooded_on`, a core-native fact, which is also what decides whether it reaches model 9.3. `flooded_on` is normally filled by a **correction** weeks after the dry sowing, because art. 45.2 annotates each activity within a month of itself. `seed_quantity_kg` exists only because the SIEX twin requires `Cantidad`; no printed page shows it. `kind_code` (`sowing_kind`: sown or planted) is `NOT NULL` — added 2026-08-21 because the register's form has always been titled "Siembra y plantación", so a planting is its documented use and the export has to say which; no decree asks for a planting annotation, so the column answers a question the form was already collecting rather than one the format invented |
| `sowing_plot` | Which parcel was sown, and which crop it started | Mirrors `harvest_plot` field for field, **including the absence of a surface column**: model 9.3 asks which parcels, not how much of each. The crop is frozen at sowing time and re-resolved when a correction restates the plots |

> **`crop` carries no sowing date.** It had a `sown_on` column until
> 2026-08-19, captured in the crops form and **printed on no page** — model 2.1
> has no such column — and read by nothing: not the book, not the advisory, not
> the export. `sowing_record` made it a second store of one fact, with the same
> label in the same tab, so it was dropped. A crop's sowing date is
> `sowing_record` joined through `sowing_plot.crop_id`.

> **`sowing_record` and module-cue's `seed_treatment` stay separate TABLES**
> (settled 2026-08-19), although the exchange format merges them into one
> `SiembraPlantacion`. Their junctions block it:
> `seed_treatment_plot.surface_sown_ha` is `NOT NULL` because model 3.2 prints
> that column, while model 9.3 asks for no surface — merging would weaken a
> shipped register or invent a required field.
>
> **They are LINKED since 2026-08-21**, by `seed_treatment.sowing_record_id`
> (nullable), which the farmer sets on the 3.2 form. The direction is forced:
> a module may reference a core table and never the reverse, and one sowing can
> use several seed lots — each naming it — which a column on `sowing_record`
> would cap at one. The descriptor points the same way, with
> `UsoSemillaTratada.IdAjenaSiembraPlant`. It is validated against the same farm
> AND campaign, and refused for a soft-deleted sowing, because the export reads
> it to state `MaterialTratado` about that sowing. Reasoning in
> `cuaderno-print.md`; the serializer's rules in `siex-export.md` → "How seam
> 2's contradiction was settled".

## The eco-scheme domain (eco-scheme module)

Owned by `module-ecoscheme` (2026-08-18; the cultural-operation register 2026-08-19). Model section 9, under **RD 1048/2022**
— a third decree feeding the same book, reaching it through RD 1054/2022 anexo II
item 4 ("otros aspectos que se recojan en la respectiva normativa sectorial").

The shape is the decree's, not the form's. Five model pages become three
registers, because the pages hide what the articles ask for: anexo IV's duty has
no page, art. 42 is three annotations on three deadlines that one printed row
collapses, and model 9.3 prints three of the five dates art. 45.2 names. The
three registers are also the exchange format's own blocks (`Pastoreo`,
`LaboresCulturales`, `DatosCubierta`), which is corroboration rather than the
reason.

```
season ──< grazing_record >── farm             model 9.1 (no cover)
              │      ├── eco_practice (lookup, ours — FEGA publishes no P1-P7 list)
              │      └── soil_cover?           set ⇒ 9.4's "Pastoreo" instead
              ├──< grazing_plot >── plot
              └──< grazing_animal               ESPECIE_ANIMAL code, verbatim

season ──< cultural_operation >── farm         model 9.2 + the book's "9.6"
              │      ├── eco_practice          which duty → which printed page
              │      ├── cultural_operation_kind (lookup, ours → TIPO_LABOR)
              │      ├── residue_destination     DEST_RES_VEG code, verbatim
              │      └── soil_cover?           set ⇒ 9.4's "Siega"/"Desbrozado"
              └──< cultural_operation_plot >── plot

season ──< soil_cover >── farm                 model 9.4 (P6) / 9.5 (P7)
              │      ├── eco_practice          which of the two pages
              │      └── cover_type            TIPO_COBERTURA_SUELO, verbatim
              └──< soil_cover_plot >── plot
```

Art. 42's **three annotations on three deadlines** are why the cover register
reaches into the other two rather than growing a maintenance table:

```
42.1.a  established_on                          the record itself
42.1.e  width_m + free_canopy_width_m           all three or none
        + widths_stated_on                      (a column no source asks for)
42.1.c  cultural_operation (mowing|brush_cutting) ─┐ keyed on soil_cover_id,
        grazing_record                            ─┘ resolved once per book
```

Model **9.3** has no table of its own. Its five dates come from three crates,
and only `terrazgo-recordbook` can read all three:

```
core         sowing_record.sown_on        → "Fecha de siembra en seco"
core         sowing_record.flooded_on     → "Fecha de inundación"
module-cue   treatment_record.drying_date → "Fecha de seca"
ecoscheme    cultural_operation (levelling|ridging, flooded_biodiversity)
                                          → the two columns the model lacks
```

| Table | What it is | Worth knowing |
|---|---|---|
| `eco_practice` | Which of the decree's six register-level duties a record evidences | **Owned, because FEGA publishes no eco-scheme catalogue at all** (verified across its 287-entry registry), and `TIPO_COBERTURA_SUELO` cannot stand in: its values 1, 5 and 6 belong to neither cover practice. Every register in the module carries it, because the same activity means different things under different practices — a mowing is P2's mandated maintenance on one plot and P6's cover maintenance on another, on different deadlines |
| `cultural_operation_kind` | What was done on the land | An owned tier-1 lookup mapped to FEGA `TIPO_LABOR`, **deliberately not injective**: the catalogue folds "Desbroce y siega" into one code where model 9.4 prints two columns, so `mowing` and `brush_cutting` are two of ours over one of theirs. Owning it also keeps the Catalan book Catalan — a verbatim provider code carries no i18n key. The contract test pins the map both ways, and the second direction is a watchdog for codes FEGA adds |
| `grazing_record` | One grazing: which animals, which plots, from when to when | **The one-month deadline runs from the END** (art. 30.2 ter, and the model's own footnote), so `ended_on` is nullable and NULL means "still grazing", not "unknown" — an open record is not late, and the book prints a blank end cell rather than inventing one. `plot_group_ref` is free capture: the model asks for it only when the plots lie more than 10 km from the main livestock installation, which the app cannot know (no installation entity), so the rule stays in the printed footnote — the `efficacy_code` precedent. **`soil_cover_id` partitions two printed pages**: art. 42.1.c counts pastoreo as one of three ways a live cover is maintained, so a grazing with a cover is model 9.4's Pastoreo column and one without is model 9.1's own row. Printing it on both would show a P6 cover grazing as extensive grazing, which is a false statement rather than a duplicate |
| `grazing_plot` | The plots grazed | Carries no surface: model 9.1 asks for the parcel REFERENCE, not an area. The reference itself is resolved from `plot_es_extension` at print time and never frozen here, so a corrected parcel register cannot leave the book disagreeing with it |
| `grazing_animal` | `Pastoreo.Animales[]` = {REGA, Numero, Especie} | One row per (holding, species), which is one printed line: 40 sheep and 12 goats from one holding are two lines. **The REGA is per line, not per record**, because third-party animals carry their owner's code — recording them under this farm's would misstate whose animals grazed. The species is the `ESPECIE_ANIMAL` code verbatim with no foreign key, per the catalogue rule |
| `cultural_operation` | One operation carried out on one or more plots | **Four duties in one table, printed on three pages.** Art. 31/31.4.d is model 9.2; anexo IV is the book's own "9.6", a page the printed model does not have; art. 45.2's nivelación and caballones join 9.3, and art. 42.1.c's maintenance joins 9.4 through a nullable `soil_cover_id`. `practice_code` is what decides the page, which is why it is on every row and why the repository refuses `extensive_grazing` — art. 30.2 ter's duty is the grazing dates, and a row filed against P1 would print nowhere. `performed_end_date` is nullable: NULL is a single day's work, never "unknown", because the twin distinguishes the two. `residue_destination_code` is `DEST_RES_VEG` verbatim, and its value 9 ("Trituración de restos de poda…") **is** art. 43's P7 practice — an inert cover exists because a poda row said 9, which is also where the twin puts the booleans it derives |
| `cultural_operation_plot` | The plots the operation covered | Carries no surface, like `grazing_plot`: model 9.2 prints the plot's own SIGPAC surface, read from the parcel register at print time, and an operation does not partially cover a recinto the way a treatment does |
| `soil_cover` | One cover established over one or more plots: art. 42's live one (P6, model 9.4) or art. 43's inert one of triturated pruning residue (P7, model 9.5) | **Art. 42 is three annotations on three deadlines, and the table is shaped like that.** `established_on` is the record; the two widths plus `widths_stated_on` are a nullable **all-or-none** triple (`invalid.incomplete_widths`, the `plot_water_point.distance_m` pairing) because art. 42.1.e is one annotation on a later deadline; and the maintenance is rows in the registers that own those activities. So a cover with no widths is a COMPLETE record whose second annotation is not due — the cells print blank, never zero. **`widths_stated_on` is a column neither decree nor twin asks for**: it is what makes "measured in June" distinguishable from "never measured" at query time, which is the only way an advisory can tell them apart. `cover_type_code` is `TIPO_COBERTURA_SUELO` verbatim and has **no printed column** — art. 42.1.a annotates the date, not which kind — captured because `DatosCubierta.TipoCobertura` asks for it; it is narrowed per practice by the PICKER and never by the repository, because the catalogue grows between releases |
| `soil_cover_plot` | The plots the cover was established over | Carries no surface, like the other two junctions: a cover's extent is stated by its two widths, which is what both articles ask for |

## The treatment domain (CUE module)

Owned by `module-cue`. Module tables may reference core tables (module
migrations run after core's) — never the reverse.

```
active_substance >──< product          (via product_active_substance,
                         │              concentration value + unit per pair)
                         ├──< product_authorisation >── country
                         │      (per-country authorisation nº — MAPA for ES —
                         ▼       + its kind: registered/parallel/exceptional…)
season ──< treatment_record >── farm       + operator, machinery?, unit,
                         │                   efficacy? (lookups/FKs)
                         ├──< treatment_plot >── plot
                         │          │             (surface treated per plot)
                         │          └── crop?     (crop AT TREATMENT TIME)
                         ├──< treatment_problem   (coded problems treated:
                         │       category lookup + catalogue code, no FK)
                         └──< treatment_justification >── justification (lookup)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `active_substance` | Materia activa | `cas_number` is the natural cross-device key a future MAPA import will dedupe on |
| `product` | Commercial phytosanitary product | `default_phi_days` is only a *default* — the value actually applied lives on the record |
| `product_active_substance` | Junction with concentration | Has its own UUID PK (not a composite) so `record_change` can address the row; the natural key survives as UNIQUE |
| `product_authorisation` | Per-country registration | A product with no authorisation row for the farm's country cannot be used there (`AuthorisationMissing`). `kind_code` classifies its nature (default `registered`; also common-name, parallel import, Art. 53 exceptional); an `exceptional` authorisation must name its substance by catalogue code (`exceptional_substance_code`) — the SIEX `MateriaActiva` value, required only for that kind |
| `treatment_record` | The central regulatory entity | One farm per record (the cuaderno is per explotación). Six `*_snapshot` columns freeze the legally-printed values; `phi_days_used` (input) sits next to `phi_end_date` (derived). Country is derived from the farm and re-checked against authorisations |
| `treatment_plot` | Junction: record ↔ plots treated | `surface_treated_ha` may be less than the plot's area; `crop_id` + crop/variety snapshots capture the per-plot crop — a single treatment can span plots with different crops. `growth_stage_code` is the crop's BBCH stage (see below) |
| `treatment_problem` | The coded problems treated (≥1 per record) | This IS the "reason for treatment": each row is a category (`reason_category` lookup — picks the catalogue and the export bucket) + the catalogue code verbatim (no FK, per the catalogue rule). Free-text `target_organism` stays on the record as optional nuance |
| `treatment_justification` | IPM justifications (≥1 per record) | Directive 2009/128/CE concepts stored as English lookup codes (`threshold_exceeded`, `monitoring`…), mapped to each country's export coding at serialization |
| `export_alias` | Integer export ids | Minted at FIRST export (`MAX+1` per target), then frozen forever — the authority keys edits/deletions on them. `split_key` discriminates when one record maps to several export entries (a multi-crop treatment splits per crop). Polymorphic like `record_change`, so no FK; synced and audited (not re-derivable) |

Every mandatory field of RD 1311/2012 / Reglamento (UE) 2023/564 maps onto
`treatment_record` + `treatment_plot` columns; the snapshots exist so the
printed cuaderno can be reproduced years later even if referenced rows were
edited since.

One deliberate nullable: `treatment_record.efficacy_code`. Efficacy is
observed *after* application — demanding it at insert would make farmers
invent a value — so it is recorded later through the one edit a stored
treatment allows (`set_treatment_efficacy`, audit-logged), and the export
precheck lists records still missing it.

### The EU annex's two conditional fields (2026-08-12)

Reglamento (UE) 2023/564's annex asks for two things RD 1311/2012 anexo III
parte I B does not, both only "where relevant" — where the product's use is
restricted to particular times of day, or to particular growth stages. The duty
comes from the EU regulation alone, which does not make it optional.

- **`treatment_record.application_time`** — the start hour, as local wall-clock
  `HH:MM`. **Deliberately not UTC**, an exception to the ISO-UTC convention
  because this is a time *of day* rather than an instant: what makes an hour
  relevant is the hour on the ground (label restrictions, bees, wind, heat), no
  timezone is stored anywhere in the schema, and a UTC round-trip would print
  back an hour the farmer never recorded. Validated on write — an hour is either
  well formed or unreadable, unlike an observation a farmer may not have yet.
- **`treatment_plot.growth_stage_code`** — an `EST_FENOLOGICO` code, verbatim
  and with no FK (the catalogue rule). It sits on the junction, not the record,
  because the annex places the stage inside its "Crop or situation/land use"
  column and the exchange format hangs `EstadoFenologico` off each DGC. Validated
  against the catalogue when one is imported: the BBCH monograph's principal
  stages are ten and closed, so an unrecognised code is a bug rather than a
  newer catalogue (the `MAT_FERTI` side of the two-tier rule). An absent
  catalogue has no opinion — reference data must never stand between a farmer
  and a lawful record.

**The stored code is not the BBCH stage.** FEGA numbers the catalogue's rows 1-10
and publishes the monograph's own 0-9 in a column of its own, so every reader
resolves through `module_cue::catalogue::growth_stage`, which returns the number
for a register cell and the full wording for a picker or a spreadsheet.

Adding the stage meant extending `reconcile_plots`' survivor comparison, and that
is the rule to remember: **every correctable field of a junction row has to be in
that equality test.** One left out is not a visible bug but a silent one — the row
is skipped, nothing is written, and the command reports success on a correction it
discarded.

### The chemical block, and why it is nullable as a unit (2026-08-09)

RD 1311/2012 art. 10.1 asks professionals to prefer non-chemical methods where
possible, so the register has to be able to record an actuation that used no
product at all — hanging pheromone diffusers against a pest. The SIEX twin
agrees: `TratamFito` requires an applicator, a problem, justifications and an
efficacy, but **not** `ProductosFito`.

So `product_id`, `dose_value`, `dose_unit_code`, `phi_days_used`,
`phi_end_date` and `product_name_snapshot` are nullable — **together**. Two
table CHECKs carry the weight the six NOT NULLs used to:

- the chemical columns are all present or all absent, so a product can never be
  stored without its dose or without a `phi_end_date`;
- an actuation must state a product, a `measure_code`, or both.

The first is the load-bearing one. A product application whose `phi_end_date`
is NULL raises no PHI alert, which is a *silent* wrong answer rather than a
visible gap — and the two readers of that column (`refresh_alerts`'
candidate query and `phi_status_for_farm`) both filter
`phi_end_date IS NOT NULL` in SQL, where the rule belongs. Tests pin it in both
directions: a measure opens no window, and it must not disturb the rest of a
refresh either.

Alongside them, `treatment_record` carries the advisor of Anexo III Parte I
B.d ("identificación del aplicador **y, en su caso, del asesor**") with its own
snapshots, and the non-chemical measure the printed model shows in section
3.1 bis — a `TIPO_MEDIDA_FITOSANITARIA` code stored verbatim, its intensity as
a value + unit pair over the `unit` table's `intensity` dimension, and the
measure's own registration number. 3.1 bis is a printed VIEW of these rows,
not a register of its own: Anexo III Parte I B is one list covering every
treatment.

`non_field_treatment` carries the same advisor block, and for the same reason
read one clause further: B identifies what was treated as "la parcela, **o en su
caso, local o medio de transporte tratado**" (B.b) and asks for the volume in
cubic metres "como tratamiento de locales" (B.f). Sections 3.3–3.5 are B, not a
register that resembles it, so B.d's advisor reaches them too — even though the
printed model gives them no such column, which is why the book folds the pair
into the applicator cell and the workbook splits it into columns of its own.

### Correcting a stored record (2026-08-10)

`treatment_record` and `non_field_treatment` are correctable, like every other
register. Nothing in the sources forbids it — RD 1311/2012 art. 16 has no
provision on modifying an entry, Reglamento (UE) 2023/564 none on integrity or
change logs, and the SIEX exchange models a correction as re-sending the same
`IdAjena*` with new values, reserving its `Borrar` flag for withdrawal. Its
child arrays (`DGCs`, `ProductosFito`, `Justificaciones`) carry no ids and no
delete flags of their own, which is the authority saying a correction restates
them whole; ours are reconciled from the submitted state accordingly, survivors
keeping their row id so each junction's audit history stays one thread.

**A snapshot is re-taken only when its FK changes.** Choosing a different
product re-freezes what that product states, because a record naming one
product while citing another's registration number would be worse than the
mistake being fixed. Leaving it alone keeps what the record already states,
even if the registry row was corrected in between — **a value this record
states must not move because of an edit made elsewhere**, which is the whole
purpose of the `*_snapshot` columns. So correcting a date cannot shift a single
stated value the correction did not name.

That sentence used to read "a *printed* legal value must not move", which was
wrong in a way worth naming: it implied printing is a state, and it is not —
see "Nothing is ever frozen" below. `phi_end_date` is the exception that proves it: it
is derived, not frozen, so it is always re-computed — from the interval's END
when there is one.

Neither update carries `season_id` or `farm_id` (a record never moves campaign
or holding — the `UpdateCrop` precedent), nor `efficacy_code`, which keeps its
own audit-logged setter because it is observed after the fact. A non-field
record additionally freezes its `subject_kind_code`: moving one between the
three registers would empty one and fill another, and interact with the stored
"APLICA TRATAMIENTO: NO" of both.

### Nothing is ever frozen, and printing is not a state (2026-08-20)

A question that recurs and had no written answer: *when does a record become
fixed?* It never does, and no part of the app observes printing.

**The register is the legal artifact; the PDF is a rendering of it.** RD
1311/2012 art. 16 says the holding *"mantendrá actualizado el registro"* — the
duty attaches to the data, not to any document produced from it.
`export_cuaderno_pdf` reads current data, writes a file and records nothing
about having done so, so printing the book twice with a correction in between
yields two different PDFs by design. It is also why the printed book has no
precheck gate: an incomplete register must still render.

**Correctability is deliberate and sourced** (see above): nothing in RD
1311/2012, in Reglamento (UE) 2023/564 or in the SIEX exchange forbids
amending an entry, and SIEX models a correction as re-sending the same
`IdAjena*`. Integrity comes from `record_change`'s complete before/after
images, not from immutability — an amended record is auditable, an
un-amendable one is merely wrong for longer.

**Archived seasons are not read-only either.** `season.status`
(active/archived) is orthogonal to `deleted_at` and to editability: a campaign
is archived precisely *because* it holds data, and an inspection is the event
most likely to reveal an error in last year's book. A freeze-on-archive rule
would fight correctability across every register, so if it is ever wanted it
needs a decision of its own rather than arriving as a side effect.

**So `*_snapshot` columns are not about printing.** They do two things:

1. the record states what was true **at the operation** — Anexo III Parte I B
   asks for the product used and the registration number it bore, not for
   whatever the registry says today; and
2. an edit to a registry row must not **silently rewrite records the farmer
   never touched**. That is the failure worth preventing, because it is
   invisible: `record_change` would faithfully log an edit to `product`, while
   the meaning of fifty treatment records moved with it and nothing logged
   that.

The test to apply when a new snapshot is proposed is therefore not "is this
frozen" but **"when the referenced row changes, was the past record wrong, or
did the world change?"** — and the answer is the FK rule above: re-take on FK
change, leave alone otherwise, and correct the record itself (audited) when the
farmer means to restate it.

**`premises_es_extension`** (2026-08-21) holds what the SPANISH registries say
about a building: `cadastral_reference` (Anexo V's CUE block 1.3 field 1, its
only identifying field, `Obligatorio` — stored trimmed and upper-cased so one
building has one spelling) and `rea_installation_code` (REA's own key for the
installation, which `Edificaciones[].IdEdificacion` wants and which is never
ours to mint). Neither is pattern-checked, the `roma_number` / `rea_code`
precedent; the export precheck demands them instead, so the registry never
blocks the duty it serves. The row is reconciled from the submitted state and
hard-deleted when both are cleared — the machinery/farm/plot contract.

*Why an extension and not core columns*, since `farm.owner_tax_id` sets a
precedent the other way: a tax id genuinely is one string in every country,
while a cadastral reference is not — France's is 14 characters with another
structure and **Italy's is three fields** (foglio, particella, subalterno). A
core column no second country could fill is Spanish weight in core, which is
what the extension-table rule exists to prevent. **Recorded, not built**: a
future Catastro lookup fills the reference through the reviewed-proposal path
SIGPAC's crop prefill uses (propose → the user confirms → the same repository
write), which is why neither column carries a source tag; and if building
geometry ever follows, it attaches through `geo_feature`'s exclusive arc as one
nullable `premises_id` column, which that design already anticipates.

**Worked example, `non_field_treatment.premises_id` (2026-08-20).** A named
premises composes `subject_description` at write time. Correcting the store's
address afterwards does **not** reach records that named it — they keep
stating what they stated, and the farmer can restate any of them deliberately.
Naming a *different* store does re-compose, because a record naming one
warehouse while printing another's address is worse than the mistake being
fixed. Clearing the link leaves the last composed text standing: the record
still states what it stated, and is now free text again.

## Derived and infrastructure tables

**`alert`** — PHI windows, licence expiry, ITV due. Owned by the reconciling
`refresh_alerts`: derived from source tables + today, corrected or deleted
as conditions change, `status` never touched by the refresh (a dismissal
cannot resurrect). `UNIQUE (alert_type_code, subject_table, subject_id)`
makes the reconciliation idempotent *by construction*. `subject_table` /
`subject_id` are polymorphic — alerts point at treatments, operators or
machinery without FKs. Excluded from audit and sync: every device
re-derives its own.

**`catalogue` / `catalogue_code`** — imported regulatory reference catalogues
(added 2026-07-14; design history in docs/siex-export.md → "Storage design").
Generic by design: `catalogue.source` tags the provider (`'siex'` — the FEGA
Anexo VII catalogues the SIEX export codes against), and each code's remaining
provider columns ride verbatim in `attrs` JSON (the `geo_feature` precedent —
promote a catalogue to a typed table only when a real query needs its
attributes). `terrazgo_core::catalogue::ensure_catalogues` runs at every
startup: idempotent, **upsert-only** (a code referenced by an old record must
keep resolving forever; retired codes carry `retired_on` and drop out of
pickers, never out of the table). A code may repeat within a catalogue when a
qualifying attribute distinguishes the rows (one row per ámbito / per SIGPAC
uso). Deliberately **no FKs from user data to codes**: the code value is the
regulatory payload, the catalogue row is display metadata, and a reimport must
never cascade into user records. Labels are not snapshotted onto records —
the code is what's legal; a renamed label should display its new text.

**`record_change`** — append-only audit log *and* future sync delta source
(one design, two obligations). Polymorphic (`entity_table`, `entity_id`),
deliberately **no foreign keys** — the log must outlive the rows it
describes. `payload` is JSON `{"before": …, "after": …}` with **complete**
row images, written in the same transaction as the change, through
`terrazgo_core::audit`. Inserts log the full new row; soft deletes log full
before *and* after; extension hard-deletes log a null after-image.

## Integrity that lives in Rust, not in the schema

SQLite enforces the FKs, NOT NULLs and UNIQUEs above. A second layer of
invariants is enforced in the repositories and only visible there — worth
knowing because the schema alone won't stop you:

- Treated plots must belong to the record's farm (`PlotNotOnFarm`).
- A treatment needs ≥1 coded problem and ≥1 justification at insert
  (`Invalid("no_problems")` / `Invalid("no_justifications")`); duplicates
  from the form are folded, not rejected.
- Problem codes (and the exceptional-authorisation substance code) must
  exist in the reference catalogue the record's country maps them to,
  whenever that catalogue is imported — which in a running app it always is
  (`Invalid("unknown_problem_code")` / `Invalid("unknown_substance_code")`).
  Retired codes pass: providers baja-date codes rather than delete them.
- An `exceptional` product authorisation must name its substance
  (`Invalid("missing_exceptional_substance")`).
- An explicit `country_code` must match the farm's (`CountryMismatch`);
  the product must be authorised in that country (`AuthorisationMissing`).
- `phi_end_date` is always recomputed from `application_date` +
  `phi_days_used` via `jiff` — never trusted from the caller.
- Names must be non-empty, areas positive (`Invalid("empty_name")`,
  `Invalid("nonpositive_area")`).
- `geo_feature` writes validate the arc (`Invalid("geo_subject_missing")` /
  `Invalid("geo_subject_ambiguous")`), require the subject row to be active
  (`NotFound`), and parse the geometry with core's `geojson` validator —
  Polygon/MultiPolygon, closed rings, lon/lat ranges
  (`Invalid("geometry_invalid")`); the range check also catches projected
  (UTM) coordinates smuggled in as if they were degrees.
- Every write to a synced table appends its `record_change` row in the same
  transaction — a repository that forgets is a bug the repository tests
  are designed to catch.

## Changing the schema

High-stakes by convention: design first. While
pre-release, edit the squashed `0001`/`0002` files and recreate dev
databases. Post-release, append a migration at the global tail (core and
module steps share **one** version sequence — see architecture.md →
Migrations) and write both migration tests: applies to a fresh database,
and applies to a database at the previous version. Then update this file.

**Three questions to answer while the register is still on paper**, because
each of them is invisible afterwards — the code returns the right answer either
way and only the cost moves (see "Indexes and query scope" below):

1. **What does a reader ask this table, and is the answer bounded?** A query
   answering a question about the current season or about today must say so in
   SQL. An index makes a lookup cheap; it cannot make an unbounded result set
   small, and a `WHERE` the caller could have written and didn't is a defect
   that grows for as long as the farmer keeps using the app.
2. **Does listing it hydrate children one parent at a time?** Hoist with
   `terrazgo_core::sql::children_by_parent` and pin it with a counting test —
   `terrazgo_testkit::query_cost` reports statements and rows, and the
   `query_scope.rs` file in each register-owning crate is where those live.
3. **Does every column a reader FILTERS on have an index leading with it?** The
   register composite and the cascading-child rule are enforced by
   `src-tauri/tests/index_contract.rs`, so those two cannot be forgotten. A new
   *link* column is what the test cannot see: `premises_id` and
   `sowing_record_id` each arrived with a reader that filters on them and went a
   week unindexed.

A post-release migration that needs Rust-side data work — backfilling UUIDs
onto a table that used to have a composite or integer key, for instance — uses
`rusqlite_migration`'s `up_with_hook`, which hands the step a `Connection`.
IDs stay generated in Rust either way: never in SQL, never by `AUTOINCREMENT`,
because a device-local rowid collides across devices at Stage-2 sync.

## Name ordering: SQL sorts, and who actually decides

Audited 2026-08-15, after asking whether the `ORDER BY <name>` clauses are dead
weight now that the frontend collates with `Intl.Collator`. Of 94 `ORDER BY`
clauses in the repositories, most order by `id`, `rowid`, a date or a code —
determinism and chronology, which nothing downstream re-sorts. **Thirteen order by
a human name** — the first pass said eleven, having truncated its own grep and
missed `advisor.rs` — and they split three ways.

**Redundant (6).** `list_machinery`, `list_user_profiles`, `list_product_details`,
`list_operators`, `list_advisors` and `list_active_substances`: every consumer re-collates — `sortedBy` in the registry
views, `nameItems` in the forms, and the one Rust reader (`advisory.rs`) uses the
result as a `.find()` lookup. The SQL sort is computed and discarded.

**Live, and sorted by the wrong rule (8).** These reached a printed or displayed
cell in SQLite's **BINARY** order, which puts "Álamo" after "Avena":

| query | where the order shows |
| --- | --- |
| `list_crops` | §2.1's plot rows, and the joined species/variety cells in `zone_rows` |
| `list_fertiliser_materials` | §6's `material_rows` |
| `product_substances` | the product card's `substances.join(" · ")` and the book's substance cell |
| `active_substances_snapshot` | frozen onto each treatment, printed in 3.1 |
| `farm_plot_references` | the SIGPAC panel's skipped-plot lists |
| `list_crops` (again) | `BookCrops`'s crop card list |
| `list_farm_advisors` | §1.4's advisor table, and `FarmView`'s linked advisors |
| `list_advisors` | (already collated on screen by `RegistryAdvisors`) |

That contradicted `terrazgo-recordbook`'s `collate.rs`, whose whole purpose is
that "a picker on screen and a cell in the PDF agree" — for accented names the
screen was right and the book was not.

**Fixed 2026-08-15, by collating in the consumer rather than changing the SQL.**
`NameCollator` now orders §2.1's plot rows and the joined species/variety cells
in `zone_rows`, and `material_rows`; `sortedBy` orders the product card's
substances (both the joined line and the management panel, from one derived
list so they cannot disagree) and the SIGPAC panel's skipped-plot names. The
crops are sorted **as structs**, not as two lists of strings: §2.1's species and
variety cells join positionally, so sorting them separately would pair a species
with another crop's variety. Pinned by
`several_crops_on_one_plot_print_in_collated_order_not_byte_order`, which was
verified to fail without the fix — it reports `["Avena", "Boj", "Álamo"]`,
byte order exactly.

**One case deliberately left alone: `active_substances_snapshot`.** Its order is
frozen onto the record at write time and printed from there. Re-sorting a stored
legal value at display time would misrepresent what the record said. The rule
that falls out of this: **live lists are collated when displayed; frozen
snapshots keep the order they were stored with.**

**Test-only (1).** `list_products_authorised` has no production consumer at all.

**Performance is not the argument, and points the other way from what you would
guess.** Measured on 20 000 rows: `ORDER BY full_name, id` costs a temp B-tree
sort (31.9 ms), while `ORDER BY id` is served straight from the primary key's
implicit unique index with **no sort at all** (27.8 ms). `ORDER BY rowid` is
cheapest (20.3 ms) but rowids are renumbered by `VACUUM INTO`, which the backup
path uses; UUIDv7 makes `id` insertion-ordered anyway.

**The SQL was changed too, 2026-08-15.** Every name ordering is now `ORDER BY
id` — deterministic (UUIDv7 makes it insertion order) without implying an
alphabet the database cannot produce correctly. Four tests pinned byte order as
a contract
and were rewritten to pin what the repositories actually promise: the filter
they exist for (per-country, excludes-deleted) plus a stable insertion order.
They had been asserting BINARY order, which is the order this audit concludes is
wrong.

**The remaining argument for having done it**, since the performance gain is
negligible: an obviously-unsorted list makes a *missing* collation visible,
where an approximately-alphabetical one hides it — which is precisely how the
book's defect survived this long.

**Two orderings deliberately survive.** `active_substances_snapshot`
(`ORDER BY a.name`) freezes onto the record at write time, and
`terrazgo-geo`'s importer orders GeoPackage layers by `table_name`, which is an
identifier rather than a person's name.

## Indexes and query scope, measured to twenty years

Audited 2026-08-17 after asking whether the indexes hold as a database
accumulates seasons; **built 2026-08-24**, which also re-ran the audit over the
registers that had arrived in between. Method both times: every SQL statement
extracted from the crates and run through `EXPLAIN QUERY PLAN` against the
composed schema, then the hot paths timed on synthetic data — one farm, 120
plots, 400 treatments per season, 10 to 20 seasons.

**The finding was never a constant; it was a slope.** Two hot paths read the
whole history to answer a question about today, so both climbed with the record
book and had no plateau:

| treatments in the database | map PHI tint | alert refresh |
| --- | --- | --- |
| 4 000 (10 seasons) | 123.2 ms → **1.1 ms** | 2.9 ms → **0.9 ms** |
| 6 000 (15 seasons) | 189.3 ms → **1.0 ms** | 3.9 ms → **0.9 ms** |
| 8 000 (20 seasons) | 254.2 ms → **1.1 ms** | 5.2 ms → **0.9 ms** |

Flat. Reproduce it with the recipe in `maintenance.md` §9; the numbers are one
machine's, so compare slopes rather than milliseconds.

**The rule the audit distilled, and the one to keep if only one survives: an
index makes a lookup cheap, it cannot make an unbounded result set small.** When
a query answers a question about the current season or about today, scope it in
SQL to that season or that date. A `WHERE` the caller could have written and
didn't is a defect that grows for as long as the farmer keeps using the app.

### What was wrong, and what it is now

**Query scope — the half no index can fix.**

- `phi_status_for_farm` read every treatment the holding had ever recorded to
  decide which plots are restricted *today*, and its
  `IN (SELECT id FROM plot …)` never materialised into a lookup — the planner
  drove `treatment_plot` through its covering index once per candidate. It is a
  `JOIN` now, and date-bounded. **The scope fix changed what the map means**:
  `in_phi` stays date-scoped across every campaign, while "treated and clear"
  gained a horizon, because on a holding farmed for a decade it was true of
  every plot. Since 2026-08-26 that horizon is a device setting
  (`phi_recent_days`, resolved through `module_cue::repository::phi_horizon_days`;
  the constant behind it is private so no caller can reach past the setting).
  **Its ceiling is load-bearing rather than cosmetic** — the horizon IS the
  `WHERE` clause, so the guarantee is "bounded", not "bounded at 90", and
  `query_scope.rs` pins the read at `MAX_PHI_HORIZON_DAYS` as well as at the
  default.
- `refresh_alerts` scanned `treatment_record` whole from ten call sites,
  startup and every treatment write among them. Its candidate query is bounded
  by `phi_end_date >= today` — a candidate **bound**, not the window rule, which
  stays in `alerts::phi_window_is_active`. Narrowing candidates is also *how* a
  lapsed alert disappears, since `reconcile` deletes what it did not re-derive,
  so the alert tests pin it rather than assuming it.
- `plot_zone_flag` appends across campaigns by design and grows by (plots × zone
  kinds) every year, while all three readers reduce it to the latest campaign
  per (plot, zone kind) — twice in JavaScript, once with a correlated
  `MAX(campaign)` per row. `list_zone_flags_for_farm` and
  `list_latest_zone_flags` now answer the **standing**, one row per pair. Two
  rules there are load-bearing: the latest campaign is resolved per (plot, zone
  type) and never once for the holding, or a plot nobody re-verified would
  silently lose its chip; and within one campaign `'inside'` wins, which the
  alert engine already did on its own.

**Per-record child queries.** The audit named `with_details`; the shape was in
eighteen list functions across four crates, and 400 records meant 1 200
statements. `terrazgo_core::sql::children_by_parent` is the one implementation
— the caller writes the whole child query with `{ids}` where the parent ids go,
because several registers order their children by something other than an id
(irrigation joins `water_origin` for the seeded order, fertilisation casts its
practice codes to integers) and a helper that composed the SQL would have
silently reordered printed cells. Two hydrations stay per record on purpose:
`soil_cover`'s maintenance, which is assembled from two *other* registers, and
the `machinery` name map in `terrazgo-recordbook`, which is deliberately
unscoped so a record naming a deleted machine keeps printing the name.

**Indexes.** `treatment_record` was the one register that missed the house
`(season_id, farm_id)` composite. `idx_treatment_record_phi` is partial and
date-first — both readers of `phi_end_date` ask about today, so the date leads
and the farm rides behind it. `idx_crop_season_plot` **replaced**
`idx_crop_plot_season`: the old column order served neither season-first reader,
and no query filters crops by plot alone. `plot` and `machinery` gained
`farm_id`; the 2026-08-17 audit had caught the first and missed the second.

**Twelve indexes were removed**, found by the contract test below rather than by
the sweep: every junction declares `UNIQUE (<parent>_id, …)`, SQLite indexes
that constraint, and a second `CREATE INDEX` on the parent alone is a duplicate
the planner never picks — a write on every insert, forever. Verified identical
in `EXPLAIN QUERY PLAN` before and after.

**The re-audit's own findings**, all three on columns younger than the first
audit: `non_field_treatment.premises_id` and `seed_treatment.sowing_record_id`
each arrived with a reader that filters on them and no index, and `machinery`
had never had one on `farm_id`. **That is the shape to expect** — a new column
arrives with a query, and the index is the half nobody notices is missing,
because the query is right either way.

### What is deliberately not built

**`record_change` has no index by time**, and it is the fastest-growing table in
the schema. Nothing in the app reads it that way, so nothing is slow: the first
"changes since X" belongs to the Stage-2 sync design, along with the index that
query will want. Building one now would be guessing at a query nobody has
written. **`record_change` also has no retention policy** — full JSON row images
accumulate for every write — and pruning an append-only log that doubles as the
sync delta source is a sync decision *and* a regulatory one (3-year minimum,
RD 1311/2012 art. 16.3). Both are carried on the project's gated-work list —
what actually gets read when the next arc is chosen — rather than left here,
because a finding filed in a doc nobody opens is a finding that is lost.

**`fertilisation_record.irrigation_record_id` is unindexed on purpose.** It is
written and read as a value, never filtered on. No reader, no index.

**No pagination anywhere.** Every register list is season-scoped and bounded at
a few hundred rows; the one list that was unbounded is the zone standing above.

### What keeps it holding

`src-tauri/tests/index_contract.rs` reads two rules off the composed schema: a
register carries an index leading with `season_id` and `farm_id` (either order),
and a cascading child is indexed by the parent it lives and dies with. **No list
to maintain — the schema is the expectation**, so a register added next year is
checked the day it exists. It is in the shell because that is the only crate
that sees the whole schema.

The two rules a test cannot enforce — query scope, and the hydration shape —
need a judgement about what the caller asked. They are stated in "Changing the
schema" above and repeated in the project's engineering conventions, which is
where someone adding a register will be looking.
