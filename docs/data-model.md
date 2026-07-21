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
| **Reference / lookup** — ships with the app, seeded by migration | `country`, `production_system`, `licence_level`, `unit`, `reason_category`, `formulation_type`, `alert_type`, `efficacy`, `justification`, `authorisation_kind` | TEXT code | no (app-versioned) | no | no |
| **Imported reference** — provider catalogue snapshot vendored in the binary, imported at startup | `catalogue`, `catalogue_code` | TEXT id / INTEGER | no (each device imports its own copy) | no | no — the provider retires codes by baja date; imports upsert and never delete |
| **User data** — created on a device | `season`, `farm`, `plot`, `crop`, `operator`, `machinery`, `user_profile`, `geo_feature`, `active_substance`, `product`, `product_active_substance`, `product_authorisation`, `treatment_record`, `treatment_plot`, `treatment_problem`, `treatment_justification`, `export_alias` | UUIDv7 | yes (Stage 2+) | yes, full row images | on the regulatory ones (see below) |
| **Regional extension** — attributes of a user-data row for one country | `farm_es_extension`, `plot_es_extension`, `machinery_es_extension` | parent's id | yes (as part of parent's domain) | yes (own entity) | no — hard-deleted when the form clears them (null after-image logged) |
| **Derived / infrastructure** | `alert` (derived), `record_change` (infrastructure) | UUIDv7 | no / is the sync source | `alert`: never. `record_change`: is the log | no |

The dividing question for lookup vs user data is *"can two devices create
this independently?"* — that is why `active_substance` is user data (an
offline farmer must be able to record an unknown substance) even though it
feels like a catalogue.

Soft delete (`deleted_at`) exists on: `farm`, `plot`, `crop`, `operator`,
`machinery`, `user_profile`, `geo_feature`, `product`, `treatment_record`.
`season` is never
deleted — it archives (`status`). Junction rows (`treatment_plot`,
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
   │         │        └── production_system (lookup)
   ├──< machinery
   │
   ├── farm_es_extension       (1 : 0..1  REA + REGA codes, province)
   │    plot_es_extension      (1 : 0..1  full SIGPAC reference)
   │    machinery_es_extension (1 : 0..1  REGANIP number)
   │
  operator (standalone — people are not owned by a farm)
   └── licence_level (lookup)

  user_profile (standalone — who uses the app; optional operator_id link)
```

| Table | What it is | Worth knowing |
|---|---|---|
| `season` | Campaign (campaña agrícola), e.g. 2025/2026 | On every regulatory record. `campaign_year` + free `label`; archives instead of deleting |
| `farm` | Explotación | `country_code NOT NULL` — the schema itself rejects country-less farms, because treatment authorisation checks derive from it. `owner_tax_id` is the holder's tax/identity number (NIF/CUAA/SIREN…) — a universal concept regulatory exports need, so it lives in core; format validation is per-country |
| `plot` | Parcela / recinto | `farm_id` is **immutable by design** — no API moves a plot between farms, since that would silently re-home its history |
| `crop` | What grows on a plot in a season | The (plot, season) pair is the unit treatments point at; indexed on it |
| `operator` | Aplicador with licence | `licence_expiry_date` drives `licence_expiry` alerts |
| `user_profile` | Who uses the app | Identification, not security: no credentials — real authentication belongs to cloud sync. The id is the author stamp on `record_change.actor` (every repository write takes an `actor` parameter; the shell passes the device's active profile id), so rows are only ever soft-deleted. The stamp is verbatim, never validated: a foreign device's claim must survive sync. Optional `operator_id` link ("this user is this applicator") must point at a non-deleted operator. The ACTIVE profile is a per-device choice in `settings.json`, never in this table |
| `machinery` | Equipment, per farm | `next_inspection_due_date` (ITV) drives `itv_expiry` alerts |
| `*_es_extension` | Spanish registry identifiers | Regional IDs never sit in core tables; a French module would add `*_fr_extension` tables, not columns. Farm carries both `rea_code` (the farm registry — the SIEX export's CodigoRea) and `rega_code` (the *livestock* registry): different registrations, both user-entered |
| `geo_feature` | Geometry attached to a plot or farm (boundaries) | **Exclusive arc**: one nullable FK per subject (`plot_id`/`farm_id`) + CHECK exactly one — real FK enforcement where `record_change`/`alert` deliberately go polymorphic, because a geometry must die with its subject. GeoJSON in EPSG:4326; `source` (`manual`/`import`/future `sigpac`) rows coexist for discrepancy display; partial unique indexes allow one ACTIVE row per (subject, role, source) — replacement soft-deletes, history is kept. `official_area_ha` is provider-declared and never overwrites `plot.area_ha`; `properties` holds provider attributes as JSON (promoted to real columns only on proven need). Fetched geometry cannot be re-derived offline, so it syncs and is audited like any user data — unlike map *tiles*, which live in the separate `geo-cache.db` (own migration runner, never in backups or `record_change`) |

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
| `treatment_plot` | Junction: record ↔ plots treated | `surface_treated_ha` may be less than the plot's area; `crop_id` + crop/variety snapshots capture the per-plot crop — a single treatment can span plots with different crops |
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
