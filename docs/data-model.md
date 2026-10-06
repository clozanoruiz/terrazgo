# Data model: the database schema, explained

This is the per-table reference: what each table is, how the tables relate, and
which rules each one takes part in. Why the model looks like this is in
[architecture.md](architecture.md) → "The data model in five ideas".

**The DDL is the source of truth**, and it is well commented. Read it alongside
this file: [`crates/terrazgo-core/migrations/0001_core_schema.sql`](../crates/terrazgo-core/migrations/0001_core_schema.sql),
[`crates/module-phytosanitary/migrations/0001_schema.sql`](../crates/module-phytosanitary/migrations/0001_schema.sql),
and the `0001_schema.sql` of module-fertilisation and module-ecoscheme. Update
this file whenever they change.

## Conventions (once, for every table)

- `snake_case`, **singular** table names, lowercase English enum values.
  Everything is in English; i18n is a display matter, and the Spanish
  regulatory term for each entity is in the table below.
- **User-data primary keys are UUIDv7**, as 36-character TEXT, generated in Rust
  (`Uuid::now_v7()`) at insert, never in SQL. Lookups use short TEXT codes.
- Timestamps are `TEXT` ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`); date-only fields
  are `YYYY-MM-DD`. Surfaces are in hectares (`REAL`). User-data tables have
  `created_at`/`updated_at` (not repeated in the tables below).
- `foreign_keys = ON` and WAL are set when the connection opens, not in the
  schema.

## Entity ↔ Spanish regulatory term

The schema is in English; these are the regulatory concepts each entity stands
for. (Core owns the farm registry; the phytosanitary module gives the entities
their Spanish regulatory meaning.)

| Schema name | Spanish regulatory term | Notes |
|---|---|---|
| `farm` | Explotación | Holder (titular) + tax id; REA/REGA codes in the Spanish extension table |
| `plot` | Parcela / Recinto | SIGPAC reference in the extension table |
| `crop` | Cultivo | Species, variety, production system |
| `treatment_record` | Tratamiento fitosanitario | The central entity; its changes are logged |
| `treatment_plot` | — | Junction: treatment ↔ plot, surface treated per plot |
| `treatment_problem` | Problemática fitosanitaria | Junction: coded problems treated (catalogue codes per category) |
| `treatment_justification` | Justificación de la actuación | Junction: IPM reasons for treating |
| `product` | Producto fitosanitario | Active substances, PHI days |
| `product_authorisation` | Nº de registro | Junction: product ↔ country, the MAPA number in Spain |
| `operator` | Operador / Aplicador | Licence number, level, expiry date |
| `advisor` | Asesor / entidad de asesoramiento | Model 1.4: name or razón social, NIF, registration number (ROPO in Spain) |
| `farm_advisor` | — | Junction: farm ↔ advisor, with the GIP framework (model 1.4's "tipo de explotación") |
| `machinery` | Maquinaria | ROMA/REGANIP numbers, inspection (ITV) dates |
| `season` | Campaña agrícola (one farm's) | One farm's campaign, and so one record book; its dates and name, active/archived. Every record points at one |
| — | Alerta | Not a table: PHI windows, licence and ITV expiry and zone flags are worked out when the list is read (see "Alerts: the settled design") |
| `alert_acknowledgement` | — | One person's act on an alert (seen, or hidden), insert-only, in core; the alert's status is read from these ([sync.md](sync.md) → Alert acknowledgements roam) |
| — | Posible duplicado | Not a table: each register's rule runs when the list is read ([sync.md](sync.md) → Duplicate suspects) |
| `duplicate_verdict` | — | One person's judgement of two records that looked like one operation (both real, or one kept and the other removed in the same change set), insert-only, in core ([sync.md](sync.md) → Duplicate suspects) |
| `record_change` | — | Append-only change log: what sync exchanges, and what a record's history reads. No law requires it (docs/cuaderno-print.md → What the law asks of the entries over time) |
| `export_alias` | — | Integer ids that regulatory exports give to records (SIEX `IdAjena*`) |
| `sync_peer` | — | Every device that has written to this book, keyed by its device id, with the name people know it by ([sync.md](sync.md) → Device identity) |

## Four kinds of table

Every table is of exactly one kind, and the kind answers most questions about
it:

| Kind | Tables | PK | Synced? | Logged in `record_change`? | Soft delete? |
|---|---|---|---|---|---|
| **Reference / lookup**: ships with the app, seeded by migration | `country`, `production_system`, `licence_level`, `irrigation_system`, `growing_environment`, `gip_system`, `zone_type`, `unit`, `reason_category`, `formulation_type`, `efficacy`, `justification`, `authorisation_kind` | TEXT code | no (comes with the app version) | no | no |
| **Imported reference**: a provider catalogue snapshot inside the binary, imported at startup | `catalogue`, `catalogue_code` | TEXT id / INTEGER | no (each device imports its own copy) | no | no: the provider retires codes with a baja date; imports upsert and never delete |
| **User data**: created on a device | `season`, `farm`, `plot`, `crop`, `operator`, `advisor`, `farm_advisor`, `machinery`, `user_profile`, `geo_feature`, `active_substance`, `product`, `product_active_substance`, `product_authorisation`, `treatment_record`, `treatment_plot`, `treatment_problem`, `treatment_justification`, `export_alias`, `sync_peer`, `alert_acknowledgement`, `duplicate_verdict`, `purged_register` | UUIDv7 (`sync_peer`: the device's own id) | yes | yes, full row images | on the regulatory ones (see below) |
| **Regional extension**: one country's attributes of a user-data row | `farm_es_extension`, `plot_es_extension`, `machinery_es_extension` | the parent's id | yes (with the parent) | yes (as its own entity) | no: hard-deleted when the form clears them (a null after-image is logged) |
| **Derived / infrastructure** | `sync_conflict` (derived), `record_change` (infrastructure); `sync_group`, `sync_held`, `sync_known` (device-local: this database's group, what it has held from each device, what it knows each device holds; [sync.md](sync.md) → What stays device-local) | composite / UUIDv7; the device-local three by group row or device id | no / is what sync sends / no | `sync_conflict` and the device-local three: never. `record_change` *is* the log | no |

The question that separates lookup from user data is *"can two devices create
this independently?"* That is why `active_substance` is user data (a farmer
offline must be able to record a substance the app doesn't know), even though it
feels like a catalogue.

Soft delete (`deleted_at`) exists on `farm`, `plot`, `crop`, `operator`,
`advisor`, `farm_advisor`, `machinery`, `user_profile`, `geo_feature`,
`product`, `treatment_record` and `season`. `farm_representative` is an
exception, like the `*_es_extension` rows: it is reconciled from the submitted
form, so removing the block hard-deletes it with a null after-image.

On `season` the two lifecycles are separate and both are needed: `status`
archives a campaign that holds real records, while `deleted_at` removes one
created by mistake. Deleting is refused while the season still has crops or
treatment records, because every record-book view is scoped to a season, so
hiding the season would hide its records too.

Junction rows (`treatment_plot`, `treatment_problem`, `treatment_justification`,
`product_active_substance`, `product_authorisation`) live and die with their
parent (`ON DELETE CASCADE` covers hard deletes; regulatory parents are only
soft-deleted in practice). `export_alias` rows are never updated or deleted: an
alias is the key the authority uses to edit or delete, so it must stay the same
across exports.

## The farm registry (core)

Owned by `terrazgo-core`: land, calendar, people and machines, the entities
every module builds on. `──<` reads "one … has many".

```
country (lookup)
   ▲
   │ country_code (NOT NULL — treatments take their country from here)
  farm ──< plot ──< crop >── season
   │         │        │
   │         │        ├── production_system, irrigation_system,
   │         │        │   growing_environment, gip_system (lookups)
   ├──< season   (the same season: each is ONE farm's campaign, so one book)
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
  operator (standalone — people don't belong to a farm)
   └── licence_level (lookup)

  unit (lookup — every module that records an amount reads it)

  user_profile (standalone — who uses the app; optional operator_id link)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `season` | One farm's campaign (campaña agrícola), e.g. Finca Los Llanos's 2025/2026, and so one record book | On every regulatory record. Has `farm_id` (fixed when created, like `plot.farm_id`), `starts_on` and `ends_on` (both required), an optional `custom_label`, and a DERIVED `label`. It is archived (`status`) when it holds records, and soft-deleted only while empty. **The campaign is the same everywhere; the row is per farm**, because the book is kept per explotación. **The dates name the book**: Rust writes `label` on every insert and update (`season_label`): the farmer's `custom_label` if there is one, otherwise "2025/2026" for a campaign that spans the new year or "2026" for one inside it. So correcting a date renames a book named by its dates, and the inputs are stored next to it. There is no campaign-year column; everything reads the dates or the name. **At most one live book per farm per `label`** (partial unique index + `Invalid("season_name_taken")`), not per year: a farm can keep several campaigns in one year and name the second one itself, but two books nobody can tell apart (in a list, on a printed cover, in an export file name, or created on two devices offline) can't exist. **Every register with `season_id` and `farm_id` ties the pair to one farm with a composite foreign key onto `season(id, farm_id)`**: a record filed under another farm's season would print in neither book. That key needs the full `UNIQUE (farm_id, id)` index, redundant with the primary key but required by SQLite, which only refuses a partial one at the first insert. `src-tauri/tests/contracts/season_link_contract.rs` checks both |
| `farm` | Explotación | `country_code NOT NULL`: the schema refuses a farm with no country, because treatment authorisation checks depend on it. It also **can't be changed**: `UpdateFarm` has no `country_code`, so nothing moves a farm to another country. The country decides which coded vocabularies the farm's records use (licence levels, GIP frameworks, every catalogue a coded field resolves against), so changing it would silently change the *meaning* of everything already recorded. A farm that really moved is a new farm. `owner_tax_id` is the holder's tax or identity number (NIF/CUAA/SIREN…), which every country has and exports need, so it is in core; its format is checked per country |
| `plot` | Parcela / recinto | `farm_id` **can't be changed**: nothing moves a plot to another farm, since that would silently move its history |
| `crop` | What grows on a plot in a season | The (plot, season) pair is what treatments point at, and it is indexed. Has the model's 2.1 agronomic columns: its own `area_ha` (per crop, so a shared plot isn't counted twice), `irrigation_code`, `growing_environment_code` and `gip_system_code`. The last can be empty, and the printed book then uses what `production_system_code` implies (organic → AE, integrated → PI). `crop_code` is the species' code in FEGA's PRODUCTOS catalogue, stored as it is and **without** a foreign key (the code is what matters, the catalogue row is only for display, and re-importing a catalogue must never cascade into records). Free-text species are still valid; they just have no code. `source` (`user`/`sigpac`), `source_campaign` and `declared_area_ha` say whether a row was typed or came from a PAC declaration, and from which campaign. `declared_area_ha` sits **next to** `area_ha`, never instead of it: one is what a third party recorded, the other the farmer's own figure. `UpdateCrop` only sets those three when they are given, so a correction from a form that doesn't carry them can't erase where the row came from. There is no sowing-date column: a crop's sowing date is `sowing_record` joined through `sowing_plot.crop_id` |
| `operator` | Aplicador with a licence | `licence_expiry_date` drives `licence_expiry` alerts |
| `advisor` | Asesor, agrupación or entidad de asesoramiento (model 1.4) | Standalone like `operator`: one advisor serves many farms. Often a company, so it has a `name`, not a person's name. `registration_number` is the model's "Nº de identificación" (the ROPO advisor registration in Spain), named in general terms because core has no regional identifiers. Advising is **not** a `licence_level`: ROPO registers applicators and advisors as separate things, which is why the model has a separate "Asesor" cross in 1.2. The printed book works it out by matching the operator's NIF against this table |
| `farm_advisor` | Farm ↔ advisor, with the GIP framework | The framework belongs to the *relationship*, so it is on the junction, not on `farm`. One ACTIVE link per (farm, advisor) (partial unique index): stating a relationship again updates it instead of printing the advisor twice in table 1.4. Soft-deleted; deleting an advisor removes its links in the same transaction, each logged separately |
| `user_profile` | Who uses the app | Identification, not security: no credentials (real authentication belongs to cloud sync). The id is the author on `record_change.actor` (every repository write takes an `actor` parameter, and the shell passes the device's active profile id), so profiles are only ever soft-deleted. The author is stored as given, never checked: another device's value must survive sync. The optional `operator_id` ("this user is this applicator") must point at an operator that isn't deleted. The ACTIVE profile is a per-device choice in `settings.json`, never in this table |
| `machinery` | Equipment, per farm | `next_inspection_due_date` (ITV) drives `itv_expiry` alerts |
| `premises` | A place or vehicle on the farm that a treatment can be applied to: model 3.4's "local tratado", model 3.5's "vehículo tratado" | **A registry because RD 1311/2012 Anexo III Parte I B.b asks to IDENTIFY it** ("la parcela, o en su caso, local o medio de transporte tratado"), and a description typed again on every record identifies nothing: two treatments of one warehouse can spell it differently and nothing links them. In core because a store is farm infrastructure like `machinery`, and module-fertilisation may need it too (manure and fertiliser stores) without depending on module-phytosanitary. **One table for both kinds**: B.b names them together and the exchange format puts both in one `Edificaciones` block, so two tables would differ in three columns. `kind_code` is core's own (`building` / `vehicle`), because the register's vocabulary (`storage_premises` / `transport`) belongs to module-phytosanitary and core can't reference a module's lookup (as with `sowing_record`). The `name` is also what prints as the model's "tipo"; a second free-text type field would ask twice. `volume_m3` is the CAPACITY; the *treated* volume (B.f) is per treatment and stays on the record. Buildings also have `class_code`, an `EDIFICACIONES_INSTALACIONES` code stored as it is with no foreign key, narrowed by the picker rather than the repository so a class published between releases can still be recorded (like `crop.crop_code`). **It never appears in a treatment's printed cell**: `describe_premises` builds that from name + address (3.4) or name + model + plate (3.5), because including a catalogue label would let a refresh silently change stored records. A unit test checks it is absent. Vehicles have none of this: FEGA's catalogue is all buildings, and a lorry has a matrícula. The Spanish identifiers are in `premises_es_extension` (below) |
| `unit` | Units of measure, with a `dimension` | In core because module-fertilisation records doses and irrigation volumes too, and a module can't depend on another module. `dimension` stops a total being offered where a rate belongs (`dose_rate` / `concentration` / `quantity`), which is why the selectors are two separate lists. Being in core also lets `harvest_record.quantity_unit_code` be a real foreign key |
| `*_es_extension` | Spanish registry identifiers | Regional ids never go in core tables; a French module would add `*_fr_extension` tables, not columns. The farm has both `rea_code` (the farm registry; the SIEX export's CodigoRea) and `rega_code` (the *livestock* registry): different registrations, both entered by the user |
| `premises_es_extension` | What the Spanish registries say about a building | `cadastral_reference` (Anexo V's CUE block 1.3 field 1, its only identifying field, `Obligatorio`; stored trimmed and upper-cased so one building has one spelling) and `rea_installation_code` (REA's own key for the installation, which `Edificaciones[].IdEdificacion` wants and which isn't ours to create). Neither is pattern-checked (like `roma_number` / `rea_code`); the export precheck asks for them instead, so the registry never blocks the duty it serves. The row is reconciled from the submitted form and hard-deleted when both are cleared, like the other extensions. It is an extension and not a core column (unlike `farm.owner_tax_id`) because a tax id is one string in every country, while a cadastral reference isn't: France's is 14 characters with another structure, and **Italy's is three fields** (foglio, particella, subalterno). A future Catastro lookup would fill the reference through the same reviewed-proposal path as SIGPAC's crop prefill (propose → the user confirms → the same repository write), so neither column has a source tag. Building geometry, if it ever comes, would attach through `geo_feature`'s exclusive arc as one nullable `premises_id` column |
| `geo_feature` | Geometry attached to a plot or farm (boundaries) | **Exclusive arc**: one nullable foreign key per subject (`plot_id`/`farm_id`) + a CHECK that exactly one is set. The foreign keys are enforced, unlike `record_change` and `alert_acknowledgement`, which are polymorphic on purpose, because a geometry must go with its subject. GeoJSON in EPSG:4326. Rows from different `source`s (`manual`/`import`/`sigpac`) exist side by side so differences can be shown; partial unique indexes allow one ACTIVE row per (subject, role, source), and replacing one soft-deletes the old, so history is kept. `official_area_ha` is the provider's figure and never overwrites `plot.area_ha`. `properties` holds provider attributes as JSON (turned into real columns only when a real need appears). Fetched geometry can't be fetched again offline, so it syncs and is logged like any user data, unlike map *tiles*, which live in the separate `geo-cache.db` (its own migrations, never in backups or `record_change`) |

## The fertilisation domain (fertilisation module)

Owned by `module-fertilisation`. Model sections 6, 7.1 and 8, under **RD
1051/2022** rather than RD 1311/2012: a second decree feeding the same book, with
its own deadlines. Like every module it may reference core tables and no other
module's.

```
season ──< irrigation_record >── farm       model section 8
              │        └── irrigation_method (lookup, SIST_RIEGO)
              ├──< irrigation_plot >── plot, crop?
              ├──< irrigation_water_origin >── water_origin (lookup)
              └──< irrigation_practice            (BUENAS_PRACTICAS_AMBITOS code, as is)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `irrigation_record` | One irrigation, or one accumulated period of them | The binding field list is RD 1311/2012 Anexo III Parte I **sección C** (a, b, l), which art. 5.d/5.e point to, not the printed model, which is older than the decree. It has a date **interval** (art. 5.f allows adding up a fortnight for intensive and fertigated crops), the volume as value + unit ({m³/ha, m³}, checked by the repository), and C.l's two water-quality figures, which **can be empty**: art. 17.2 only requires them when the basin authority or irrigators' community provides them. **Fully correctable**: it copies no other row's identity |
| `irrigation_method` | The eight `SIST_RIEGO` systems | **Not** core's four-value `irrigation_system`, and both are needed: core's describes the PLOT (A.2.e, and one value is "rainfed"), these describe how one watering was done. A "sprinkler" crop can be watered by a fixed installation one week and a mobile one the next, which is why `crop.irrigation_code` can't be mapped to SIST_RIEGO |
| `irrigation_water_origin` | Where the water came from | A junction, not a column: the SIEX `OrigenAgua` is an array, because one irrigation can mix a river and a borehole |
| `irrigation_practice` | Good practices claimed for the watering | The irrigation counterpart of `fertilisation_practice`, for `Riego.BuenasPracticasRiego`: the `BUENAS_PRACTICAS_AMBITOS` practices marked "SI" under "Ámbito Riego", stored as they are with no foreign key, recorded but never required (Voluntario in Anexo V, obligatorio condicionado to regional rules in the 2027 model). Checked like fertilisation's: deduplicated, sorted numerically, and "0" never next to another code |

**A FERTIGATION is one act the decree records twice.** Art. 5.d puts the
fertiliser in §6's register and art. 5.e puts the water in §8's, while the
exchange format joins them again as `Fertilizacion.Fertirrigacion`. The two
records are linked by `fertilisation_record.irrigation_record_id` (nullable).
That sub-block is the **only reader anywhere in the format** of
`irrigation_record`'s two C.l water-quality figures (no printed column and no
member of `Riego` has them), so without the link two columns of a binding Anexo
III letter would be recorded for nobody. The link is **refused unless
`application_method.is_fertigation`**, because on any other method it would
claim a fertigation that didn't happen; the flag is read from the lookup rather
than by matching the code. It is checked against the same farm and campaign,
and refused for a removed watering, like `seed_treatment.sowing_record_id`.
Reasoning in `siex-export.md` → "Fertigation: one act, sent twice".

**`fertilisation_record.sustainable_input_management`**: Anexo V marks
`Fertilizacion.GestionSostInsu` Obligatorio, no decree names it and no printed
cell has it, so it goes in the spreadsheet's own column next to the sludge
flag.

### Sowing and planting (core)

| Table | What it is | Worth knowing |
|---|---|---|
| `sowing_record` | How a crop began | **The mirror image of harvest, and in core for the same reason**: the two bracket a crop, so core holds all three of `crop`, `sowing_record` and `harvest_record`. It has **no eco-scheme practice code** and can't: core can't reference a module's lookup. What marks a *cultivo bajo agua* is `flooded_on`, a core fact, which also decides whether it reaches model 9.3. `flooded_on` is usually filled by a **correction** weeks after the dry sowing, because art. 45.2 records each activity within a month of it. `seed_quantity_kg` exists only because SIEX requires `Cantidad`; no printed page shows it. `kind_code` (`sowing_kind`: sown or planted) is `NOT NULL`: the register's form has always been called "Siembra y plantación", so plantings are part of its use and the export has to say which. No decree asks for a planting entry, so the column records something the form already asked, not something the format invented |
| `sowing_plot` | Which parcel was sown, and which crop it started | Mirrors `harvest_plot` field for field, **including having no surface column**: model 9.3 asks which parcels, not how much of each. The crop is fixed at sowing time and worked out again when a correction restates the plots |

**`sowing_record` and module-phytosanitary's `seed_treatment` are separate
TABLES**, although the exchange format merges them into one
`SiembraPlantacion`. Their junctions prevent merging:
`seed_treatment_plot.surface_sown_ha` is `NOT NULL` because model 3.2 prints
that column, while model 9.3 asks for no surface. Merging would weaken an
existing register or invent a required field.

**They are linked** by `seed_treatment.sowing_record_id` (nullable), which the
farmer sets on the 3.2 form. The direction is forced: a module can reference a
core table and never the reverse, and one sowing can use several seed lots,
each naming it, which a column on `sowing_record` would limit to one. The SIEX
descriptor points the same way, with `UsoSemillaTratada.IdAjenaSiembraPlant`.
It is checked against the same farm AND campaign, and refused for a
soft-deleted sowing, because the export reads it to state `MaterialTratado`
about that sowing. Reasoning in `cuaderno-print.md`; the serializer's rules in
`siex-export.md` → "Treated seed and the sowing it was used for".

## The eco-scheme domain (eco-scheme module)

Owned by `module-ecoscheme`. Model section 9, under **RD 1048/2022**: a third
decree feeding the same book, through RD 1054/2022 anexo II item 4 ("otros
aspectos que se recojan en la respectiva normativa sectorial").

The tables follow the decree, not the form. Five model pages become three
registers, because the pages hide what the articles ask for: anexo IV's duty has
no page, art. 42 is three entries on three deadlines that one printed row
squeezes together, and model 9.3 prints three of the five dates art. 45.2 names.
The three registers are also the exchange format's own blocks (`Pastoreo`,
`LaboresCulturales`, `DatosCubierta`), which confirms the shape but isn't the
reason for it.

```
season ──< grazing_record >── farm             model 9.1 (no cover)
              │      ├── eco_practice (lookup, ours — FEGA publishes no P1-P7 list)
              │      └── soil_cover?           set ⇒ 9.4's "Pastoreo" instead
              ├──< grazing_plot >── plot
              └──< grazing_animal               ESPECIE_ANIMAL code, as is

season ──< cultural_operation >── farm         model 9.2 + the book's "9.6"
              │      ├── eco_practice          which duty → which printed page
              │      ├── cultural_operation_kind (lookup, ours → TIPO_LABOR)
              │      ├── residue_destination     DEST_RES_VEG code, as is
              │      └── soil_cover?           set ⇒ 9.4's "Siega"/"Desbrozado"
              └──< cultural_operation_plot >── plot

season ──< soil_cover >── farm                 model 9.4 (P6) / 9.5 (P7)
              │      ├── eco_practice          which of the two pages
              │      └── cover_type            TIPO_COBERTURA_SUELO, as is
              └──< soil_cover_plot >── plot
```

Art. 42's **three entries on three deadlines** are why the cover register
reaches into the other two instead of having its own maintenance table:

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
module-phytosanitary   treatment_record.drying_date → "Fecha de seca"
ecoscheme    cultural_operation (levelling|ridging, flooded_biodiversity)
                                          → the two columns the model lacks
```

| Table | What it is | Worth knowing |
|---|---|---|
| `eco_practice` | Which of the decree's six register-level duties a record is evidence for | **Our own, because FEGA publishes no eco-scheme catalogue at all** (not in its 287-entry registry), and `TIPO_COBERTURA_SUELO` can't replace it: its values 1, 5 and 6 belong to neither cover practice. Every register in the module has it, because the same activity means different things under different practices: a mowing is P2's required maintenance on one plot and P6's cover maintenance on another, with different deadlines |
| `cultural_operation_kind` | What was done on the land | Our own lookup, mapped to FEGA `TIPO_LABOR`, and **not one-to-one on purpose**: the catalogue puts "Desbroce y siega" in one code where model 9.4 prints two columns, so `mowing` and `brush_cutting` are two of ours on one of theirs. Having our own list also keeps the Catalan book in Catalan (a provider code has no i18n key). The contract test checks the map both ways, and the second direction catches codes FEGA adds |
| `grazing_record` | One grazing: which animals, which plots, from when to when | **The one-month deadline runs from the END** (art. 30.2 ter, and the model's own footnote), so `ended_on` can be empty, and empty means "still grazing", not "unknown". An open record isn't late, and the book prints an empty end cell rather than inventing one. `plot_group_ref` is free text: the model only asks for it when the plots are more than 10 km from the main livestock installation, which the app can't know (there is no installation entity), so the rule stays in the printed footnote. **`soil_cover_id` decides between two printed pages**: art. 42.1.c counts grazing as one of three ways to maintain a live cover, so a grazing with a cover is model 9.4's Pastoreo column and one without is a row of model 9.1. Printing it on both would show a P6 cover grazing as extensive grazing, which would be false, not just repeated |
| `grazing_plot` | The plots grazed | No surface: model 9.1 asks for the parcel REFERENCE, not an area. The reference is read from `plot_es_extension` at print time and never copied here, so correcting the parcel register can't leave the book disagreeing with it |
| `grazing_animal` | `Pastoreo.Animales[]` = {REGA, Numero, Especie} | One row per (farm of origin, species), which is one printed line: 40 sheep and 12 goats from one farm are two lines. **The REGA is per line, not per record**, because animals from another farm carry their owner's code, and recording them under this farm's would say the wrong animals grazed. The species is the `ESPECIE_ANIMAL` code as it is, with no foreign key, per the catalogue rule |
| `cultural_operation` | One operation on one or more plots | **Four duties in one table, printed on three pages.** Art. 31/31.4.d is model 9.2; anexo IV is the book's own "9.6", a page the printed model doesn't have; art. 45.2's nivelación and caballones join 9.3; and art. 42.1.c's maintenance joins 9.4 through a nullable `soil_cover_id`. `practice_code` decides the page, which is why every row has it and why the repository refuses `extensive_grazing`: art. 30.2 ter's duty is the grazing dates, and a row filed under P1 would print nowhere. `performed_end_date` can be empty: empty means a single day's work, never "unknown", because SIEX tells the two apart. `residue_destination_code` is the `DEST_RES_VEG` code as it is, and its value 9 ("Trituración de restos de poda…") **is** art. 43's P7 practice: an inert cover exists because a pruning row said 9, which is also where the export puts the booleans it works out from it |
| `cultural_operation_plot` | The plots the operation covered | No surface, like `grazing_plot`: model 9.2 prints the plot's own SIGPAC surface, read from the parcel register at print time, and an operation doesn't partly cover a recinto the way a treatment does |
| `soil_cover` | One cover established over one or more plots: art. 42's live one (P6, model 9.4) or art. 43's inert one of shredded pruning residue (P7, model 9.5) | **Art. 42 is three entries on three deadlines, and the table follows that.** `established_on` is the record. The two widths plus `widths_stated_on` are an **all-or-none** group that can be empty (`invalid.incomplete_widths`, like `plot_water_point.distance_m`), because art. 42.1.e is a separate entry with a later deadline. The maintenance is rows in the registers that own those activities. So a cover with no widths is a COMPLETE record whose second entry isn't due yet: the cells print empty, never zero. **`widths_stated_on` is a column neither the decree nor SIEX asks for**: it is what tells "measured in June" from "never measured" in a query, which is the only way an advisory can tell them apart. `cover_type_code` is the `TIPO_COBERTURA_SUELO` code as it is and has **no printed column** (art. 42.1.a records the date, not the kind); it is recorded because `DatosCubierta.TipoCobertura` asks for it. It is narrowed per practice by the PICKER and never by the repository, because the catalogue grows between releases |
| `soil_cover_plot` | The plots the cover was established over | No surface, like the other two junctions: a cover's extent is given by its two widths, which is what both articles ask for |

## The treatment domain (phytosanitary module)

Owned by `module-phytosanitary`. Module tables may reference core tables (module
migrations run after core's), never the reverse.

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
| `active_substance` | Materia activa | `cas_number` is the natural key across devices that a future MAPA import would deduplicate on |
| `product` | Commercial phytosanitary product | `default_phi_days` is only a *default*; the value actually applied is on the record |
| `product_active_substance` | Junction with concentration | Has its own UUID primary key (not a composite) so `record_change` can point at the row; the natural key is kept as UNIQUE |
| `product_authorisation` | Registration per country | A product with no authorisation row for the farm's country can't be used there (`AuthorisationMissing`). `kind_code` says what kind it is (default `registered`; also common name, parallel import, Art. 53 exceptional). An `exceptional` authorisation must name its substance by catalogue code (`exceptional_substance_code`), the SIEX `MateriaActiva` value, required only for that kind |
| `treatment_record` | The central regulatory entity | One farm per record (the cuaderno is per explotación). Six `*_snapshot` columns keep the values the record states; `phi_days_used` (input) sits next to `phi_end_date` (derived). The country comes from the farm and is checked again against the authorisations |
| `treatment_plot` | Junction: record ↔ plots treated | `surface_treated_ha` may be less than the plot's area. `crop_id` + crop/variety snapshots keep the crop per plot, since one treatment can cover plots with different crops. `growth_stage_code` is the crop's stage (see below) |
| `treatment_problem` | The coded problems treated (≥1 per record) | This IS the "reason for treatment": each row is a category (`reason_category` lookup, which picks the catalogue and the export bucket) + the catalogue code as it is (no foreign key, per the catalogue rule). The free-text `target_organism` stays on the record as optional detail |
| `treatment_justification` | IPM reasons (≥1 per record) | Directive 2009/128/CE concepts as English lookup codes (`threshold_exceeded`, `monitoring`…), mapped to each country's export codes when serializing |
| `export_alias` | Integer export ids | Created at the FIRST export (`MAX+1` per target), then never changed: the authority uses them to key edits and deletions. `split_key` tells entries apart when one record becomes several export entries (a treatment with several crops is split per crop). Polymorphic like `record_change`, so no foreign key; synced and logged (it can't be worked out again). Created as `MAX+1` behind one connection lock, so two devices exporting before they sync could collide. That is prevented by design: one device per farm submits, and only it creates that farm's numbers, built with the exporter ([sync.md](sync.md) → `export_alias` collisions) |

Every required field of RD 1311/2012 / Reglamento (UE) 2023/564 maps onto
`treatment_record` + `treatment_plot` columns. The snapshots let the printed
cuaderno be reproduced years later, even if the rows it points at have been
edited.

One field is empty on purpose: `treatment_record.efficacy_code`. Efficacy is
seen *after* the application, and asking for it at insert would make farmers
invent a value. So it is set later (`set_treatment_efficacy`, logged), and the
export precheck lists records still missing it.

### The EU annex's two conditional fields

Reglamento (UE) 2023/564's annex asks for two things RD 1311/2012 anexo III
parte I B doesn't, both only "where relevant": where the product's use is limited
to certain times of day, or to certain growth stages. The duty comes from the EU
regulation alone, which doesn't make it optional.

- **`treatment_record.application_time`**: the start hour, as local clock time
  `HH:MM`. **Not UTC, on purpose**, an exception to the ISO-UTC convention,
  because this is a time *of day* and not an instant. What makes an hour matter
  is the hour on the ground (label limits, bees, wind, heat), no timezone is
  stored anywhere in the schema, and converting to UTC and back would print an
  hour the farmer never recorded. Checked on write: an hour is either well
  formed or unreadable, unlike an observation a farmer may not have yet.
- **`treatment_plot.growth_stage_code`**: an `EST_FENOLOGICO` code, as it is,
  with no foreign key (the catalogue rule). It is on the junction, not the
  record, because the annex puts the stage inside its "Crop or situation/land
  use" column and the exchange format hangs `EstadoFenologico` off each DGC.
  Checked against the catalogue when one is imported: the BBCH monograph's main
  stages are ten and fixed, so an unknown code is a bug, not a newer catalogue.
  With no catalogue there is no check: reference data must never stand between
  a farmer and a lawful record.

**The stored code is not the BBCH stage.** FEGA numbers the catalogue's rows
1–10 and gives the monograph's own 0–9 in a separate column, so every reader
goes through `module_phytosanitary::catalogue::growth_stage`, which returns the
number for a register cell and the full wording for a picker or a spreadsheet.

**Every correctable field of a junction row has to be in `reconcile_plots`'
survivor comparison.** A field left out isn't a visible bug but a silent one:
the row is skipped, nothing is written, and the command reports success for a
correction it threw away.

### The chemical block, and why it is nullable as a unit

RD 1311/2012 art. 10.1 asks professionals to prefer non-chemical methods where
possible, so the register has to be able to record an action that used no
product at all, such as hanging pheromone diffusers against a pest. SIEX agrees:
`TratamFito` requires an applicator, a problem, justifications and an efficacy,
but **not** `ProductosFito`.

So `product_id`, `dose_value`, `dose_unit_code`, `phi_days_used`,
`phi_end_date` and `product_name_snapshot` can be empty, **together**. Two table
CHECKs do the job the six NOT NULLs used to do:

- the chemical columns are all set or all empty, so a product can never be
  stored without its dose or without a `phi_end_date`;
- an action must name a product, a `measure_code`, or both.

The first one matters most. A product application with an empty `phi_end_date`
raises no PHI alert, which is a *silent* wrong answer rather than a visible
gap. Both readers of that column (`current_alerts`' PHI query and
`phi_status_for_farm`) filter `phi_end_date IS NOT NULL` in SQL, where the rule
belongs. Tests check both directions: a measure opens no window, and it doesn't
disturb the module's other alerts.

`treatment_record` also has the advisor of Anexo III Parte I B.d
("identificación del aplicador **y, en su caso, del asesor**") with its own
snapshots, and the non-chemical measure the printed model shows in section 3.1
bis: a `TIPO_MEDIDA_FITOSANITARIA` code as it is, its intensity as value + unit
(the `unit` table's `intensity` dimension plus kg and kg/ha, Anexo V field 18's
list, `list_intensity_units`), the measure's own registration number, and, on
measure 11 "Usos de sustancias básicas" only, which basic substance was used
(`measure_basic_substance_code`, a `SUSTANCIAS_BASICAS` code as it is; optional,
and never refused for being unknown here, since the EU list grows). 3.1 bis is
a printed VIEW of these rows, not a register of its own: Anexo III Parte I B is
one list covering every treatment.

`non_field_treatment` has the same advisor block, for the same reason one
clause further on: B identifies what was treated as "la parcela, **o en su caso,
local o medio de transporte tratado**" (B.b) and asks for the volume in cubic
metres "como tratamiento de locales" (B.f). Sections 3.3–3.5 are B, so B.d's
advisor applies to them too, even though the printed model has no column for it.
That is why the book puts the pair in the applicator cell and the workbook
splits it into its own columns.

### Correcting a stored record

`treatment_record` and `non_field_treatment` can be corrected, like every other
register. Nothing in the sources forbids it: RD 1311/2012 art. 16 says nothing
about changing an entry, Reglamento (UE) 2023/564 nothing about integrity or
change logs, and SIEX models a correction as sending the same `IdAjena*` again
with new values, keeping its `Borrar` flag for removal. Its child arrays
(`DGCs`, `ProductosFito`, `Justificaciones`) have no ids or delete flags of
their own, which means a correction restates them whole. Ours are reconciled
from the submitted form in the same way, and rows that survive keep their id, so
each junction row's history stays in one thread.

**A snapshot is taken again only when its foreign key changes.** Choosing a
different product takes what that product states, because a record naming one
product while citing another's registration number would be worse than the
mistake being fixed. Leaving the product alone keeps what the record already
says, even if the registry row was corrected since: **a value this record states
must not change because of an edit made somewhere else**. That is the whole point
of the `*_snapshot` columns. So correcting a date can't change any stated value
the correction didn't touch. `phi_end_date` is different: it is derived, not
kept, so it is always worked out again, from the END of the interval when there
is one.

Neither update has `season_id` or `farm_id`: a record's own form never moves it
to another campaign or farm (like `UpdateCrop`). The one thing that moves
records is merging two books that turned out to be one campaign, a whole book
at a time (docs/sync.md → Merging two books). `efficacy_code` isn't in the
update either; it keeps its own logged setter because it is observed afterwards.
A non-field record also keeps its `subject_kind_code`: moving one between the
three registers would empty one and fill another, and affect the stored
"APLICA TRATAMIENTO: NO" of both.

### Nothing is ever frozen, and printing is not a state

*When does a record become fixed?* Never, and no part of the app notices
printing.

**The register is the legal record; the PDF is a rendering of it.** RD
1311/2012 art. 16 says the farm *"mantendrá actualizado el registro"*: the duty
is on the data, not on any document made from it. `export_cuaderno_pdf` reads
the current data, writes a file and records nothing about having done so, so
printing the book twice with a correction in between gives two different PDFs,
as it should. It is also why the printed book has no precheck: an incomplete
register must still print.

**Records can be corrected on purpose, and the sources allow it** (see above):
nothing in RD 1311/2012, Reglamento (UE) 2023/564 or SIEX forbids changing an
entry, and SIEX models a correction as sending the same `IdAjena*` again.
`record_change` keeps complete before/after images, so a corrected record can
be traced; one that can't be corrected is just wrong for longer.

**Archived seasons aren't read-only either.** `season.status`
(active/archived) has nothing to do with `deleted_at` or with editing: a
campaign is archived *because* it holds data, and an inspection is the most
likely moment to find an error in last year's book. Locking archived books
would work against corrections in every register, so if it is ever wanted it
needs its own decision, not to arrive as a side effect.

**So `*_snapshot` columns aren't about printing.** They do two things:

1. the record states what was true **at the time of the operation**: Anexo III
   Parte I B asks for the product used and the registration number it had then,
   not whatever the registry says today; and
2. an edit to a registry row must not **silently rewrite records the farmer
   never touched**. That is the failure to prevent, because nobody would see it:
   `record_change` would log the edit to `product`, while the meaning of fifty
   treatment records changed with it and nothing logged that.

So the question to ask about a new snapshot isn't "is this frozen" but **"when
the referenced row changes, was the past record wrong, or did the world
change?"** The answer is the foreign-key rule above: take it again when the
foreign key changes, leave it alone otherwise, and correct the record itself
(logged) when the farmer wants to restate it.

**Example: `non_field_treatment.premises_id`.** Naming a premises builds
`subject_description` when the record is written. Correcting the store's address
later does **not** change the records that named it: they keep saying what they
said, and the farmer can restate any of them. Naming a *different* store does
rebuild it, because a record naming one warehouse while printing another's
address is worse than the mistake being fixed. Clearing the link leaves the last
text as it was: the record still says what it said, and is now free text again.

## Derived and infrastructure tables

**Alerts are not a table.** PHI windows, licence expiry, ITV due dates and a
plot in a zone are worked out from the registers and today's date every time the
list is read, and stored nowhere (see "Alerts: the settled design" below). What a
person said about an alert is user data, in core's `alert_acknowledgement`: one
insert-only row per act, with a polymorphic subject and no foreign key (it must
outlive the condition, which goes away), plus the deadline it was about, so a
renewed licence's next expiry is a new alert. The status shown is the strongest
act about the alert's current deadline, worked out when read ([sync.md](sync.md)
→ Alert acknowledgements roam).

**`catalogue` / `catalogue_code`**: imported regulatory reference catalogues
(storage design in docs/siex-export.md → "Storage design"). Generic:
`catalogue.source` names the provider (`'siex'`: the FEGA Anexo VII catalogues
the SIEX export codes against), and each code's other provider columns are kept
as they are in `attrs` JSON (as with `geo_feature`: a catalogue becomes a typed
table only when a real query needs its attributes).
`terrazgo_core::catalogue::ensure_catalogues` runs at startup and is
**upsert-only**: a code used by an old record must keep resolving forever, so
retired codes get `retired_on` and leave the pickers, never the table
(maintenance.md §1 has when it imports). A code may repeat within a catalogue
when another attribute tells the rows apart (one row per SIGPAC uso, or per
province). There are **no foreign keys from user data to codes**, on purpose:
the code is what matters legally, the catalogue row is only for display, and a
re-import must never cascade into user records. Labels aren't copied onto
records: the code is what is legal, and a renamed label should show its new
text.

**`record_change`**: the append-only change log. Sync sends it, and the
conflict review, the duplicate review and a record's history read it. **It has
one obligation, sync's; the rest are uses.** The law's three years apply to the
entries and their documents, and no text asks for a history of their changes
(docs/cuaderno-print.md → What the law asks of the entries over time).
Polymorphic (`entity_table`, `entity_id`), with **no foreign keys** on purpose:
the log must outlive the rows it describes. `payload` is JSON `{"before": …,
"after": …}` with **complete** row images, written in the same transaction as
the change, through `terrazgo_core::audit`. Inserts log the whole new row; soft
deletes log the whole row before *and* after; extension hard deletes log a null
after-image.

Because the images are complete, a receiving device can build a row from
`after` alone. And **the payload's keys are the column names, on purpose**:
`Machinery.kind` has `#[serde(rename = "type")]` for exactly that reason, since
the column name is a Rust keyword. Sync ([sync.md](sync.md)) relies on this as a
tested contract and uses one generic row applier instead of sixty hand-written
ones.

**The sync stamp** ([sync.md](sync.md) → The change stamp). Every row also says
which register it belongs to (`root_table`, `root_id`), the unit of merge, which
is a register and its children rather than the row; where it came from
(`origin_device`, `origin_seq`, shared by every row of one write transaction,
so the pair *is* the change-set key); the register's `version_vector` after the
write; and an `hlc` used only to order changes already known to conflict.
`changed_at` and `actor` keep their meaning, since the reviews show who wrote
each version and when.

A register is usually a row, but six are keyed on a **slot** (the values of the
UNIQUE key their writes replace), so two devices filling one slot offline are
two versions of one register rather than two rows that can't both be applied:
`farm_advisor`, `geo_feature`, `plot_zone_flag`, `plot_water_declaration`,
`export_alias` and `register_declaration`. For those, `root_id` is the slot's
values as a JSON array.

**Each row is checked against the aggregate map as it is written.** A row filed
under the wrong register, a table no crate has classified, or a table the map
says never syncs is refused and its write is rolled back, so a mistake here can
never reach a database. The map itself is checked against the schema by
`src-tauri/tests/contracts/sync_shape_contract.rs`.

## Integrity that lives in Rust, not in the schema

SQLite enforces the foreign keys, NOT NULLs and UNIQUEs above. A second layer of
rules is enforced in the repositories and is only visible there, so the schema
alone won't stop you:

- Treated plots must belong to the record's farm (`PlotNotOnFarm`).
- A crop's plot and its season must belong to the same farm
  (`Invalid("season_not_on_farm")`, in `insert_crop`). Every register ties its
  season to its farm with a composite key; `crop` can't, because it reaches its
  farm only through `plot_id`, and a mismatched crop would be in no book.
  `insert_crop` is the only place a crop is created and neither side changes
  later, so one check covers every path.
- A farm keeps one live season per name (`Invalid("season_name_taken")` on
  insert and update); the partial unique index backs it up. A season's end
  can't come before its start (`Invalid("invalid_date_interval")`, the same code
  as the registers; a `CHECK` backs it up), and its name comes from its dates
  unless the farmer gives one.
- A treatment needs ≥1 coded problem and ≥1 justification at insert
  (`Invalid("no_problems")` / `Invalid("no_justifications")`); duplicates from
  the form are merged, not refused.
- Problem codes (and the exceptional-authorisation substance code) must exist
  in the reference catalogue the record's country maps them to, whenever that
  catalogue is imported, which in a running app it always is
  (`Invalid("unknown_problem_code")` / `Invalid("unknown_substance_code")`).
  Retired codes pass: providers retire codes with a date rather than delete
  them.
- An `exceptional` product authorisation must name its substance
  (`Invalid("missing_exceptional_substance")`).
- A `country_code` given explicitly must match the farm's (`CountryMismatch`);
  the product must be authorised in that country (`AuthorisationMissing`).
- `phi_end_date` is always worked out from `application_date` +
  `phi_days_used` with `jiff`, never taken from the caller.
- Names must not be empty and areas must be positive (`Invalid("empty_name")`,
  `Invalid("nonpositive_area")`).
- `geo_feature` writes check the arc (`Invalid("geo_subject_missing")` /
  `Invalid("geo_subject_ambiguous")`), require the subject row to be active
  (`NotFound`), and parse the geometry with core's `geojson` validator:
  Polygon/MultiPolygon, closed rings, lon/lat ranges
  (`Invalid("geometry_invalid")`). The range check also catches projected (UTM)
  coordinates passed in as if they were degrees.
- Every write to a synced table adds its `record_change` row in the same
  transaction. A repository that forgets is a bug the repository tests are
  there to catch.

## Changing the schema

Schema changes are high-stakes: design first. While the project is
pre-release, edit the squashed `0001`/`0002` files and recreate dev databases.
After release, add a migration at the end of the global sequence (core and
module steps share **one** version sequence; see architecture.md → Migrations)
and write both migration tests: it applies to a fresh database, and to a
database at the previous version. Then update this file.

**Three questions to answer while the register is still on paper**, because
afterwards you can't see them: the code gives the right answer either way, and
only the cost changes (see "Indexes and query scope" below):

1. **What does a reader ask this table, and is the answer bounded?** A query
   about the current season or about today must say so in SQL. An index makes a
   lookup cheap; it can't make an unbounded result small. A `WHERE` the caller
   could have written and didn't is a defect that grows for as long as the
   farmer uses the app.
2. **Does listing it load children one parent at a time?** Use
   `terrazgo_core::sql::children_by_parent` and pin it with a counting test:
   `terrazgo_testkit::query_cost` reports statements and rows, and the
   `query_scope.rs` file in each crate that owns registers is where those tests
   go.
3. **Does every column a reader FILTERS on have an index that starts with it?**
   The register composite and the cascading-child rule are enforced by
   `src-tauri/tests/contracts/index_contract.rs`, so those can't be forgotten.
   A new *link* column is what the test can't see: index it in the same change
   as the reader that filters on it.

A migration after release that needs data work in Rust (giving UUIDs to a table
that used to have a composite or integer key, for example) uses
`rusqlite_migration`'s `up_with_hook`, which gives the step a `Connection`. IDs
are still generated in Rust: never in SQL, never by `AUTOINCREMENT`, because a
device-local rowid collides across devices when they sync.

## Which crate owns each table

**The rule** (architecture.md → the placement rule): shared data belongs in
`terrazgo-core`, a domain with its own logic in a module, and a module never
depends on another module. In structure the schema follows it: no module's SQL
references another module's table. In meaning not quite: some groups of tables
sit in a module whose domain they aren't. The alerts have been moved (below).
**The other three groups wait for the module that first reads them**, because
what that module needs may change the answer, and moving them now would mean
moving them twice.

| Tables | Where now | Why they might move | When |
| --- | --- | --- | --- |
| `analysis_record`, `analysis_plot`, `analysis_record_type`, `analysis_substance`, and the `analysis_material` / `analysis_type` lookups | module-phytosanitary | They hold any lab bulletin: soil, water, crop, harvested produce; residues, nutrients, heavy metals, microbiology, soil parameters. The A.3 soil block is for the fertilisation domain (RD 1051/2022 arts. 5.b and 6 make a soil analysis an input to the plan de abonado), and its column comment says why it is here anyway. **module-fertilisation can't read it**: only the record book and the SIEX export, which sit above the modules, can. It is shared data, like `unit`, which went to core when a second module needed it | When module-fertilisation first needs to read it (nutrient planning) |
| `irrigation_record`, `irrigation_plot`, `irrigation_water_origin`, `irrigation_practice` | module-fertilisation | Placed there on purpose (RD 1051/2022 art. 5.e puts irrigation doses and dates in that duty), but the Irrigation module's water-usage analysis will read them, and can't | With the Irrigation module |
| `cultural_operation`, `cultural_operation_plot`, `cultural_operation_kind` | module-ecoscheme | SIEX's `LaboresCulturales` is general farm work, and organic farms must record their farm operations. A crop-planning or organic module would read them, and can't | With a crop-planning or organic module |

**What stays where it is**: products, active substances, treatment records,
non-field and seed treatments and their lookups, and `register_declaration`,
whose `register_kind` only covers the four phytosanitary registers. Fertiliser
materials, fertilisation records and plans stay in module-fertilisation;
grazing and soil cover in module-ecoscheme.

**What moving a group involves**, so nothing is left behind:

- the DDL (pre-release, an edit to the squashed files and recreated dev
  databases);
- the repository functions and their tests, and the `query_scope.rs` pins;
- the sync shape entry (a module's `SYNC_SHAPE` → `CORE_SYNC_SHAPE`), the row
  captions and the backup shape;
- the SIEX serializer's and the record book's imports (the exporter moves with
  the registers);
- the Tauri commands, which move file with their owner;
- the i18n keys, whose names don't change.

### Alerts: the settled design

**Alerts are worked out when the list is read, and nothing stores them.** A
stored copy has to be kept current after every save in every module, and
forgetting is silent: a deleted plot's zone alert would stay listed until some
other save ran. A stored list also goes stale at midnight when nothing is saved,
and a refresh at startup runs before the app has its database, so a failing
rule there would leave every screen without it. Instead, each crate that raises
alerts returns the ones that hold now, and the list is put together from those
whenever it is read.

**Core owns everything that works on alerts, and knows no kind:**

- **a standard shape** every crate fills: the kind, which row the alert is
  about (`subject_id`), the subject's name as a farmer knows it ("Los Alcores",
  not "plot"), and the due date;
- **`AlertKind`**: a code, the table its subjects are in, and whether the kind
  is standing. Each crate declares one constant per kind. A standing kind's
  alerts have no due date and a dated kind's always have one; building one the
  other way round is refused. Whether a kind is standing is decided in Rust next
  to its rule, never in the view;
- **the subject's table belongs to the kind**, never an argument: a plazo is
  always about a treatment, a carné always about an operator. Every alert,
  every unchecked record and every act takes it from the kind, so the screen
  doesn't send one back and an act can't be filed under a table its kind is
  never about;
- **`alert_acknowledgement`**, with its rules (docs/sync.md → Alert
  acknowledgements roam). Its `alert_type_code` is not a foreign key (see
  "There is no kinds table" below);
- **putting the list together**: the acts on the alerts that hold now, looked up
  for those alerts and never across the whole history (pinned by a counting
  test), then the strongest act about each alert's current deadline, with the
  dismissed ones dropped;
- **Seen and Hide**, which record what the farmer was shown: the kind (and
  through it the subject's table), the subject and the deadline on screen,
  without checking again that the alert still holds. If it changed in the
  meantime, the act names a deadline that never comes back and affects nothing,
  which is true to what the person saw. An act that changes nothing isn't
  written;
- **the three zone alerts**, as core's own rules. They read core's
  `plot_zone_flag`, and their rule (the latest check says inside) belongs to no
  single domain: a nitrate zone matters to fertilisation, the other two to the
  phytosanitary side.

**module-phytosanitary keeps the rules whose duties come from its decree**: the
PHI window (RD 1311/2012; the map's PHI tint reads the same rule), the
applicator's carné and the equipment's inspection (Anexo III A.1.c and A.1.h put
both in the book), with `AlertConfig`, the lead-time checks, and the names of
treatments, operators and machines. The carné and ITV alerts read core tables,
but the duties behind them are phytosanitary, and the lead times belong with the
duties; in core they would be phytosanitary logic in the shared layer.

**The shell collects them**, in one function that is the only place alerts are
gathered: core's first, then each crate on its list (module-phytosanitary
today). A crate whose rules fail is named next to everyone else's alerts, and the
Status view shows it, never only in a log: a bad row in one module must not hide
another module's PHI window. A contract test checks the list both ways (no code
is declared by two crates, and every declared kind has `alert.type.<code>` in
every locale) and checks each kind's subject table is one the aggregate map
says is synced, since an act names its subject on every device. What it can't
see is a module whose alerts were never wired up: its own tests pass and the app
shows nothing. That takes three separate omissions and breaks nothing else; it
is the cost of a list instead of a `Module` trait method, and accepted.

**What goes wrong is said in a way the farmer can act on.** Three layers, from
the most likely to the least:

- **A bad date is refused when it is saved.** Every date an alert rule reads
  (an operator's carné expiry, a machine's purchase and inspection dates, a
  treatment's dates) goes through `date::validate_dates` on insert and on
  correction, so the form says "Fecha no válida" while the farmer is still in
  it.
- **A record a rule still can't read is named, not fatal.** A bad value can
  still arrive by sync or restore, from a copy that let it through. The rule
  then reports that record as an `UncheckedRecord` (its kind, the record by
  name, the stored value) and works out every other record as usual, so one bad
  carné doesn't hide every PHI window. Each crate answers with an `AlertReport`
  (alerts raised, records unchecked). The Status view shows each unchecked
  record as a card saying where to correct it ("Corríjalo en Catálogo →
  Operadores"), with no Seen button, since there is nothing to acknowledge until
  the value can be read.
- **A crate that fails completely says what to do.** Anything else (the
  database itself, a bug) isn't something the farmer can fix in their data, so
  the notice tells them to update the app and report the problem if it
  continues, with the report link the About panel uses and the error's detail
  folded underneath, ready to copy.

**There is no kinds table.** Kinds are the constants their crates declare, and
no migration ever adds one. What a table would protect, the constants protect: a
code is never typed, only copied from its constant, and two crates can't share
one past the contract test. A table would cost a migration for every new kind
after release (in core, or a module writing into core's table), and a migration
changes `user_version`, which must be equal for two devices to sync
(docs/sync.md → Schema skew). The phone and the laptop would stop syncing until
both were updated, for a new alert.

Codes the database doesn't check exist wherever their list lives outside the
database, and they all follow one rule: **the database stores the code as it
is, Rust checks it where this device writes it, and a row arriving from another
device is kept whatever it says**, because that device may know a list this one
doesn't. `record_change.entity_table` is checked against the aggregate map on
every log write; provider catalogue codes against the device's own catalogue
snapshot, field by field, where the list is closed (docs/sync.md → What stays
device-local); `alert_acknowledgement.subject_table` is never an input at all,
since it comes from the kind; and the kind's code is looked up among the
declared kinds before an act is written. An act about a kind this build doesn't
know (synced from a newer build, or about a kind that has since gone) is kept
and matches nothing.

**Adding an alert to a module** is its kind constant (its code and its subjects'
table), its rule, and its text in each dictionary, plus one line in the shell's
list the first time that module raises any. Core doesn't change and no migration
is written. Alerts and acts aren't registers the SIEX format carries, so the
exporter isn't affected.

**Rules about acts**: insert-only, one row per act, the strongest wins, and an
act names its deadline.

**Other designs considered:**

| Option | Why not |
| --- | --- |
| A stored table refreshed after every save | Every save in every module must remember to refresh, and forgetting is silent; stale at midnight; a failure at startup leaves the whole app without its database |
| A stored table refreshed just before it is read | Still a copy, with code to insert, correct and delete its rows, and every future reader must remember to refresh first |
| One refresh in core running every crate's rules | Core needs an interface each crate implements; a call that leaves a crate out deletes all of that crate's alerts; one failing rule empties every crate's list |
| A `terrazgo-alerts` crate | The migration sequence knows core, then modules; a third kind of crate needs its own place there and in the backup, sync, coverage and CI lists, for nothing core can't hold |
| Leaving the machinery in module-phytosanitary | No other module could ever raise an alert, the AEMET red-warning advisory included |
| Every rule stays in module-phytosanitary | Keeps the zone alerts in a crate whose law isn't theirs |
| The zone, carné and ITV rules all in core | Puts phytosanitary lead times, and their settings, in the shared layer |
| The shell finds alert crates through the `Module` trait | Nothing to forget, but grows the trait before a second module needs it; a list with a contract test was preferred |
| Rules written as SQL that core runs | Moves the date rules out of tested Rust, with the lead times passed into the SQL |
| Database triggers keeping the alerts up to date | A deadline reached because the date changed isn't a row change, so a trigger can never raise it |
| One failing crate fails the whole list | A bad row in one module would empty every module's alerts |

## Name ordering

SQLite sorts text in **BINARY** order, which puts "Álamo" after "Avena". So
lists of names are sorted where they are shown, not in SQL:

- **The record book** sorts with `NameCollator` (`terrazgo-recordbook`'s
  `collate.rs`), so a picker on screen and a cell in the PDF agree: §2.1's plot
  rows and the joined species/variety cells in `zone_rows`, and §6's
  `material_rows`.
- **The views** sort with `Intl.Collator` (`sortedBy` in the registry views,
  `nameItems` in the forms), including the product card's substances (the joined
  line and the management panel come from one sorted list, so they can't
  disagree) and the SIGPAC panel's skipped-plot names.
- **Crops are sorted as structs**, not as two lists of strings: §2.1's species
  and variety cells are joined by position, so sorting them separately would
  pair one crop's species with another crop's variety.
  `several_crops_on_one_plot_print_in_collated_order_not_byte_order` checks
  this.

**In SQL, lists of names are `ORDER BY id`**: stable (UUIDv7 makes it insertion
order) without pretending to an alphabetical order the database can't get
right. An obviously unsorted list also makes a *missing* collation easy to
spot, where a roughly alphabetical one hides it. (`ORDER BY id` is also cheap:
it comes straight from the primary key's index with no sort. `ORDER BY rowid`
would be cheaper still, but `VACUUM INTO`, which backups use, renumbers rowids.)

**Two orderings stay in SQL on purpose.** `active_substances_snapshot`
(`ORDER BY a.name`) is stored on the record when it is written and printed from
there, and sorting a stored legal value again when displaying it would
misrepresent what the record says. And `terrazgo-geo`'s importer orders
GeoPackage layers by `table_name`, which is an identifier, not a person's name.
The rule: **live lists are sorted when displayed; stored snapshots keep the
order they were stored in.**

## Indexes and query scope

**An index makes a lookup cheap; it can't make an unbounded result small.**
When a query answers a question about the current season or about today, limit
it in SQL to that season or that date. A `WHERE` the caller could have written
and didn't is a defect that grows for as long as the farmer uses the app.

The way to check it (every SQL statement through `EXPLAIN QUERY PLAN` against
the composed schema, then the hot paths timed on synthetic data: one farm, 120
plots, 400 treatments per season, 10 to 20 seasons) is in `maintenance.md` §9.
What matters is the slope, not the milliseconds. The hot paths stay flat as
seasons pile up:

| treatments in the database | map PHI tint | phytosanitary alerts (`current_alerts`) |
| --- | --- | --- |
| 4 000 (10 seasons) | 1.2 ms | 0.4 ms |
| 6 000 (15 seasons) | 1.0 ms | 0.3 ms |
| 8 000 (20 seasons) | 1.3 ms | 0.5 ms |

### What keeps the hot paths bounded

- **`phi_status_for_farm`** (the map's PHI tint) is a `JOIN` bounded by date.
  `in_phi` is limited by date across every campaign, and "treated and clear" has
  a horizon, because on a farm worked for a decade it would be true of every
  plot. The horizon is a device setting (`phi_recent_days`, read through
  `module_phytosanitary::repository::phi_horizon_days`; the constant behind it
  is private, so no caller can get round the setting). **Its maximum matters**:
  the horizon IS the `WHERE` clause, so the guarantee is "bounded", not
  "bounded at 90", and `query_scope.rs` checks the read at
  `MAX_PHI_HORIZON_DAYS` as well as at the default.
- **`current_alerts`' PHI query** is bounded by `phi_end_date >= today`. That is
  a **bound** on the candidates, not the window rule, which stays in
  `alerts::phi_window_is_active`. `query_scope.rs` checks it by rows.
- **`plot_zone_flag`** adds rows every campaign by design, so it grows by
  (plots × zone kinds) each year, while every reader only wants the latest
  campaign per (plot, zone kind). `list_zone_flags_for_farm` and
  `list_latest_zone_flags` return that **standing**, one row per pair. Two rules
  there matter: the latest campaign is found per (plot, zone type), never once
  for the whole farm, or a plot nobody checked again would silently lose its
  chip; and within one campaign `'inside'` wins.

**Child rows are loaded per list, not per record.**
`terrazgo_core::sql::children_by_parent` is the one implementation. The caller
writes the whole child query with `{ids}` where the parent ids go, because
several registers order their children by something other than an id
(irrigation joins `water_origin` for its seeded order, fertilisation casts its
practice codes to integers), and a helper that built the SQL would silently
reorder printed cells. Two are loaded per record on purpose: `soil_cover`'s
maintenance, which comes from two *other* registers, and the `machinery` name map
in `terrazgo-recordbook`, which is unscoped on purpose so a record naming a
deleted machine still prints its name.

**Indexes.** Every register has the `(season_id, farm_id)` composite.
`idx_treatment_record_phi` is partial and starts with the date: both readers of
`phi_end_date` ask about today, so the date comes first and the farm after it.
`idx_crop_season_plot` starts with the season, which is what readers filter by;
nothing filters crops by plot alone. `plot` and `machinery` are indexed on
`farm_id`. Junctions don't get a separate index on their parent column: every
junction declares `UNIQUE (<parent>_id, …)`, SQLite indexes that constraint, and
a second index on the parent alone is a duplicate the planner never uses, paid
for on every insert.

**Expect new link columns to arrive unindexed.** A new column comes with a
query, and the index is the part nobody notices is missing, because the query
gives the right answer either way. Every column that points at a register of a
book is indexed (every `crop_id`, `seed_treatment.sowing_record_id`,
`non_field_treatment.premises_id`, `fertilisation_record.irrigation_record_id`,
the soil-cover links), each partial on the column being set, because the purge
erases a record only together with everything that names it, and SQLite asks
that through the column. And every register of a book has an index over its
removed rows (docs/sync.md → The purge, as settled).

### What is not built, on purpose

**`record_change` has no index by time**, though it is the fastest-growing table.
Nothing in the app reads it that way. Sync reads the log as "everything device D
wrote after sequence N", and the UNIQUE `(origin_device, origin_seq,
entity_table, entity_id)` (needed anyway for one row per entity per change set)
starts with exactly that pair: a range scan per device, with gap detection for
free. An index on `changed_at` would be the wrong shape, and would inherit that
column's whole-second precision. Two more indexes serve the stamp every write
takes: `(root_table, root_id, origin_seq)` for a register's history and `hlc` for
the clock, each a single seek however long the log gets (checked by the
full-scan test in `terrazgo_core::audit`).

**`record_change` keeps a live record's whole history**: full JSON row images
for every write, for as long as the record exists. How long the log keeps things
is a sync and product decision, **not a regulatory one**: art. 16.3's three
years are for the entries, not for the log of their changes (docs/cuaderno-print.md
→ What the law asks of the entries over time). What leaves the log is what was
deleted from a book, with its history: the purge runs by itself thirty days
after the deletion, once every device has it, and leaves one `purged_register`
row per register as a marker (docs/sync.md → Retention and real deletion). The
growth benchmark is `src-tauri/tests/contracts/quick_check_cost.rs`.

### The record book list is paged

Every register list is limited to a season and stays at a few hundred rows. The
list of record books is different: a book is a season, a season belongs to one
farm, so the list grows with **farms × years** and no `WHERE` can bound it,
because it is the whole question. In headless Chrome with a 6× CPU throttle,
100 books drew in about 55 ms, 200 in about 90 ms, and 4 000 (200 farms × 20
years) in about 3 s: the rows drawn are the cost. Filtering by year would have
hidden a one-farm user's past books to help a cooperative.

So `list_seasons(limit, offset)` returns a page and the total, limited to
`SEASON_PAGE_MAX` (500). The view asks for 100 and only shows page controls past
the first page, so a user with a few farms never sees them. The first page of
4 000 draws in about 140 ms with the same throttle. In SQL (SQLite 3.45, core's
schema, ordered by end date then farm name), page 1 of 4 000 books takes 2.3 ms
and the last 5 ms; of 40 000 books, 25 ms and 85 ms. The page is chosen over the
narrow sort keys and only then joined back to its rows; sorting whole rows made
the last page of 40 000 cost 204 ms.
`a_page_of_record_books_costs_the_same_at_four_times_the_books` (core's
`query_scope.rs`) checks that a page costs the same statements AND rows however
many books there are.

### What keeps it holding

`src-tauri/tests/contracts/index_contract.rs` reads two rules from the composed
schema: a register has an index starting with `season_id` and `farm_id` (in
either order), and a cascading child is indexed by the parent it lives and dies
with. Next to it, `season_link_contract.rs` checks that every such register ties
its season to its farm with the composite key, and that every foreign key has a
parent index SQLite can use. (The new key needs no register index of its own,
because the `(season_id, farm_id)` one already covers it.) **There is no list to
keep up: the schema is the expectation**, so a register added next year is
checked from the day it exists. The test is in the shell because that is the
only crate that sees the whole schema.

The two rules a test can't enforce (query scope, and how children are loaded)
need a judgement about what the caller asked. They are in "Changing the schema"
above, and repeated in the project's engineering conventions, where someone
adding a register will look.
