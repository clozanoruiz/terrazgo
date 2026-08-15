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
product re-freezes what that product prints, because a record naming one
product while printing another's registration number would be worse than the
mistake being fixed. Leaving it alone keeps what the record already printed,
even if the registry row was corrected in between — a printed legal value must
not move because of an edit made elsewhere, which is the whole purpose of the
`*_snapshot` columns. So correcting a date cannot shift a single printed value
the correction did not name. `phi_end_date` is the exception that proves it: it
is derived, not frozen, so it is always re-computed — from the interval's END
when there is one.

Neither update carries `season_id` or `farm_id` (a record never moves campaign
or holding — the `UpdateCrop` precedent), nor `efficacy_code`, which keeps its
own audit-logged setter because it is observed after the fact. A non-field
record additionally freezes its `subject_kind_code`: moving one between the
three registers would empty one and fill another, and interact with the stored
"APLICA TRATAMIENTO: NO" of both.

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
