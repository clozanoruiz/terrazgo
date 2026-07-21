# Architecture — how Terrazgo hangs together

> For contributors **and** for the developer working on it day to day. It
> explains the structure, the invariants the code relies on, how a request
> actually travels through the app, and — because this project is also a
> Rust apprenticeship — what the load-bearing Rust constructs mean for
> someone coming from JavaScript/SQL. It deliberately does *not* describe
> every file: the code and its doc comments do that.

## The big picture

Terrazgo is an offline-first desktop/mobile app: a Svelte webview talking to
a Rust backend over Tauri's IPC, with SQLite as the single source of truth on
the device. No network calls exist in any core or module code path — that is
a hard rule, not an accident of youth. The one sanctioned network seam is the
`terrazgo-geo` crate (map tiles and providers, see "The map tier" below), and
even that is cache-through: with no network the app keeps working.

```
┌────────────────────────────────────────────────────────┐
│  src/  — Svelte 5 frontend (views only)                │
│         framework-agnostic layer: i18n.js, backend.js, │
│         nav.js, mapLayers.js · reactive glue: notifs   │
└──────────┬─────────────────────────┬───────────────────┘
           │ invoke (JSON in/out)    │ geo:// (tiles, styles)
┌──────────▼─────────────────────────▼───────────────────┐
│  src-tauri/  — shell (crate `terrazgo`)                │
│  commands.rs (thin wrappers + error boundary)          │
│  geo_protocol.rs (geo:// handler) · registry.rs        │
│  db.rs (composed migration runner)                     │
│  state.rs (AppState, GeoState, SettingsState)          │
└───────┬──────────────────┬──────────────────┬──────────┘
        │                  │                  │
┌───────▼──────────┐ ┌─────▼────────────┐ ┌───▼──────────────────┐
│ crates/          │ │ crates/          │ │ crates/terrazgo-geo  │
│ module-cue       │▶│ terrazgo-core    │◀│ tile/resource cache, │
│ treatment domain │ │ farm registry +  │ │ base-map sources,    │
│ product,         │ │ geo_feature,     │ │ style rewriting,     │
│ treatment, alert │ │ audit, backup,   │ │ boundary-file import │
│ + CUE lookups    │ │ date, geojson    │ │ (ALL network I/O)    │
└──────────────────┘ └─────┬────────────┘ └───┬──────────────────┘
                     ┌─────▼───────┐   ┌──────▼───────┐
                     │ terrazgo.db │   │ geo-cache.db │  derived, re-fetchable,
                     │ user data,  │   │ tiles/styles │  never in backups or
                     │ WAL, FKs on │   │ own runner   │  record_change
                     └─────────────┘   └──────────────┘
```

Dependency direction is one-way and enforced by the crate graph — the
compiler, not discipline, prevents a core→module import:

- `terrazgo-core` depends on **no module and never on the shell**. It owns
  the farm registry (land, calendar, people, machines), geometry storage
  (`geo_feature`), the imported reference catalogues (`catalogue` +
  vendored SIEX snapshot), the audit helpers, date utilities, the
  pure-parsing GeoJSON validator, backup and the device-local settings file.
- Modules depend on `terrazgo-core`. The CUE module owns the treatment
  domain: products, authorisations, treatment records, alerts.
- `terrazgo-geo` depends on `terrazgo-core` only (for the GeoJSON validator
  and error conventions) and owns **all network I/O in the app** plus the
  boundary-file importers. No user data lives there.
- `terrazgo-report` depends on no other workspace crate — pure
  infrastructure: in-process PDF generation via Typst, with the Liberation
  Sans faces embedded in the binary. Modules own their `.typ` templates and
  depend on this crate to render them (see "The report engine" below); the
  first consumer is the CUE printable cuaderno (`module_cue::report` +
  `crates/module-cue/templates/cuaderno.typ`).
- The shell depends on all three and owns everything Tauri-specific: command
  wrappers, the migration runner, the `geo://` protocol, managed state, the
  window.
- In the frontend, `i18n.js`, the locale dictionaries, `lib/backend.js` and
  `lib/nav.js` are plain JS with **no Svelte imports** — a future framework
  swap rewrites only the views ([frontend-conventions.md](frontend-conventions.md)).

The mental model for the split: **the core is the farm registry; a module is
a regulatory or functional domain built on top of it.** CUE gives the core
entities their Spanish phytosanitary meaning; a future irrigation module
would give plots an irrigation meaning without the core changing.

## Life of a command

The single most useful thing to internalise. Take "the user saves a
treatment" and follow it down and back up. Everything else in the codebase
is a variation of this path.

**1. The view collects a payload** (`TreatmentForm.svelte`). Svelte 5 runes
(`$state`, `$derived`) hold form state; on submit the component builds a
plain JS object — `NewTreatmentRecord` — with `snake_case` fields, because
serde on the Rust side deserializes struct payloads by field name.

**2. `run()` wraps the call** (`lib/notifications.svelte.js`):

```js
run(async () => {
  const saved = await invoke("create_treatment_record", { record, plots: treatedPlots });
  notify(t("message.treatment_saved", { date: formatDate(saved.phi_end_date) }));
});
```

`run()` is the app's one error funnel: any rejection becomes a red
notification (the bell panel opens itself) rendered through `errorText`.
Views never `try/catch` command calls individually.

**3. Tauri IPC.** `invoke` serializes the arguments to JSON, crosses the
webview boundary, and Tauri routes the name to the Rust function registered
in `lib.rs`'s `generate_handler!` list. Argument names arrive camelCase on
the JS side for plain arguments (`farmId`), but struct payloads keep their
snake_case field names — they are serde's business, not Tauri's.

**4. The command wrapper** (`src-tauri/src/commands.rs`) is deliberately
thin — lock, delegate, `?`:

```rust
#[tauri::command]
pub fn create_treatment_record(
    state: State<'_, AppState>,
    record: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
) -> CmdResult<TreatmentRecord> {
    let mut conn = lock_conn(&state)?;
    let record = repository::insert_treatment_record(&mut conn, record, plots)?;
    repository::refresh_alerts(&mut conn, &today_utc(), &AlertConfig::default())?;
    Ok(record)
}
```

`State<AppState>` is Tauri's dependency injection: the struct placed in
managed state at startup is handed to any command that asks for it. The
command locks the connection mutex, calls the repository function, refreshes
alerts (this command changes alert inputs), and returns.

**5. The repository does the real work**
(`crates/module-cue/src/repository/treatment.rs`), inside **one SQLite
transaction**:

- derives the country from the farm (`farm.country_code` is the source of
  truth; a caller-supplied country that disagrees is a `CountryMismatch`),
- validates every treated plot belongs to that farm (`PlotNotOnFarm`) and
  that the product is authorised in that country (`AuthorisationMissing`),
- computes the PHI end date with `jiff` (compliance-critical date maths is
  never hand-rolled) and stores `phi_days_used` next to it,
- generates UUIDv7 ids in Rust (`Uuid::now_v7()` — never in SQL),
- freezes the `*_snapshot` columns (product name, MAPA number, operator
  licence, crop…) — see invariant 3,
- appends complete row images to `record_change` — see invariant 2.

Either all of it commits or none of it does.

**6. The result travels back.** `Ok(TreatmentRecord)` serializes to JSON and
resolves the JS promise; the view shows a success notification and reloads
its list. An `Err` is where the error boundary earns its keep:

- Repositories return typed errors (`CoreError`, `CueError` — `thiserror`).
- The command's `?` converts them into `CommandError(anyhow::Error)` via a
  blanket `From` impl.
- `CommandError` serializes as `{ code, params, message }`: `classify()`
  downcasts the `anyhow::Error` back to the domain error and maps variants
  to stable machine codes (`authorisation_missing`, `invalid.empty_name`…).
- The frontend renders the `error.<code>` i18n key with `params`
  interpolated. `internal` (any non-domain error) deliberately has **no**
  dictionary entry: the localized `error.internal_intro` line is prefixed to
  the raw developer message, so nothing is ever swallowed.

This is the `thiserror`-in-crates / `anyhow`-at-the-boundary division: typed
and matchable where callers make decisions, type-erased where everything
becomes JSON anyway.

## Startup, before the window shows

`src-tauri/src/lib.rs::run()`, in order — any failure aborts startup, which
is the correct behaviour for "the database didn't open or migrate":

1. Resolve the data dir from the app identifier (`org.terrazgo.app` →
   `~/.local/share/org.terrazgo.app/` on Linux) and open/create
   `terrazgo.db` — WAL mode, `foreign_keys = ON`.
2. Run `composed_migrations()` — core steps first, then each registered
   module's steps in registration order, one global `user_version`.
3. `terrazgo_core::catalogue::ensure_catalogues` — imports/reconciles the
   reference catalogues vendored in the binary (upsert-only; see "Reference
   catalogues" below). After first run this is a handful of date probes.
4. `refresh_alerts(today)` — idempotent reconciliation, so the UI never
   opens on stale alert state.
5. Put `AppState { conn: Mutex<Connection>, db_path, schema_version }` into
   Tauri managed state and register the commands.

Why a `Mutex`? Tauri runs commands on a thread pool, and anything in managed
state must be `Send + Sync`. A rusqlite `Connection` is `!Sync` — it must
never be used from two threads at once — so the mutex serialises all
database access through the single connection. For a single-user desktop app
that is exactly right; if a long query ever blocks the UI, the upgrade path
is a connection pool (r2d2), not removing the lock.

## The data model in five ideas

Not a table catalogue — the five concepts that explain why the schema looks
the way it does. The per-table reference (relationships, categories, which
rules each table participates in) is [data-model.md](data-model.md), which
also maps each entity to its Spanish regulatory term.

**1. User data vs reference data.** User-data rows (farms, plots,
treatments…) get UUIDv7 TEXT primary keys generated in Rust at insert time —
sync-safe across devices, insertion-ordered. Lookup tables that ship with
the app (`unit`, `reason_category`, `country`…) use short text codes and are
seeded by migration. The dividing question is "can two devices create this
independently?" — that is why `active_substance` was *promoted* from lookup
to user data when the answer turned out to be yes.

**2. The audit log is also the sync mechanism.** `record_change` rows carry
complete before/after row images for every synced table. Today that is the
regulatory audit trail (records must never silently mutate; 3-year
retention). At sync Stage 2/3 the very same log becomes the delta source a
peer device replays. One design, two obligations — which is why "complete
row images, always" is non-negotiable.

**3. Snapshot what the law will print.** The cuaderno is a legal document.
`treatment_record`/`treatment_plot` freeze product name, registration
number, substances, operator licence, crop, REGANIP into `*_snapshot`
columns at write time *and* keep the FKs. Editing a product row later must
never alter a past official record. Corollary: never store a derived value
(PHI end date) without the inputs used to derive it (`phi_days_used`).

**4. Crop lives on the junction.** A treatment can span plots growing
different crops, so "crop at treatment time" sits on `treatment_plot`
(per plot), not on the record. Multi-plot and multi-country junctions are
where this data model earns its complexity — they are also the cases the
tests cover explicitly.

**5. Soft delete, always, on regulatory data.** `deleted_at` hides; nothing
regulatory is ever hard-deleted, so history keeps resolving. The one
exception: regional extension rows (`*_es_extension`) are hard-deleted when
a form clears them (logged with a null after-image) — they are attributes of
a live row, not records in their own right.

## Migrations: one global sequence

Each crate keeps its SQL files (`crates/*/migrations/`, embedded into the
binary via `include_str!`) and exposes `migration_set() -> Vec<M>`. The
shell concatenates them — **core first, then modules in registration
order** — into a single `rusqlite_migration` runner with one global
`user_version`. Consequences:

- Module tables may reference core tables, never the reverse.
- Registration order in `registry.rs` is load-bearing.
- A module never has its own migration version table.

Pre-release, the sequence may be squashed (currently `0001` DDL + `0002`
seed DML) and dev databases are recreated, not migrated. **The moment any
database holds real data, the composed sequence becomes append-only as a
whole**: new migrations join at the global tail regardless of owning crate,
and every one must pass two tests — applies to a fresh database, and applies
to a database at the previous version.

## Alerts: derived, reconciled, never resurrected

Alerts (PHI window open, licence expiring, ITV due) are pure derivations of
(source tables, today). `refresh_alerts(conn, today, config)` reconciles:
inserts missing alerts, corrects drifted fields, deletes lapsed ones — and
never touches `status`, so a dismissal cannot come back. Anything that
changes alert inputs (creating/deleting a treatment, importing a backup,
startup) calls it immediately after.

Two deliberate exclusions: alerts are **not** audit-logged (each device
re-derives its own; logging them would pollute the sync delta source), and
lead times (60 d licence / 30 d ITV) are config defaults, not regulatory
values.

## Reference catalogues: vendored, imported, upsert-only

Regulatory exports must speak the provider's coded vocabulary — for Spain,
the FEGA SIEX "Anexo VII" catalogues (efficacy, justification, crop and
phytosanitary-problem codes, units, machinery types…). The 16
treatment-relevant catalogue CSVs are vendored **inside the binary**
(`crates/terrazgo-core/catalogues/`, a dated snapshot of FEGA's public
catalogue API) and `terrazgo_core::catalogue::ensure_catalogues` imports
them at startup into `catalogue` + `catalogue_code` (added 2026-07-14;
design history in docs/siex-export.md → "Storage design"). Offline-first:
codes resolve from first run, no network — refreshing the snapshot is a
release-ritual step, and an in-app refresh through terrazgo-geo's fetch is a
possible later addition (same parser, same upsert).

The importer is idempotent (a per-catalogue lifecycle-date fast path makes
the steady-state startup cost a handful of probes) and **upsert-only**: the
provider retires codes by baja date instead of deleting them, and so do we —
a code referenced by a years-old treatment record must keep resolving at
inspection time. Pickers offer `active_codes` (not retired); resolution uses
`find_code` (any lifecycle state). The files are parsed with the `csv` crate
(`;`-separated, RFC quoting) plus a hand-rolled decode, no encoding crate:
UTF-8 accepted first (future-proofing — legacy accented text is never
accidentally valid UTF-8), then Windows-1252 (what the files really are —
they carry € at 0x80, despite being documented as ISO-8859-1; only the
0x80–0x9F range differs from the 1:1 Latin-1 map). A control-character
tripwire test turns any future encoding drift into a loud test failure at
the snapshot refresh. Tests run against the real
vendored FEGA files; the upsert-never-delete invariant has its own test.

How records *use* the catalogues splits by list size (2026-07-15; per-table
detail in docs/data-model.md, design in docs/siex-export.md → "Capture
design"). Small closed lists with universal meaning (efficacy, IPM
justification, authorisation kind) are **owned as English-coded lookup
tables** and translated to the provider's integers at export by
`module_cue::siex` — records stay country-neutral, the `es` dictionary
carries the official Castilian wording verbatim, and a bidirectional
contract test against the vendored CSVs fails the suite when the provider
adds or retires a code. Provider lists too large to own (the ~1,400
phytosanitary problems) store the **catalogue code verbatim** on the record
(`treatment_problem`), validated at insert against the imported catalogue
(existence only — retired codes stay legal). Integer export identities live
in `export_alias`: minted at first export, frozen forever, because the
authority keys edits and deletions on them.

The export itself lives in `module_cue::export` (added 2026-07-16; mapping
design in docs/siex-export.md): `export_precheck` lists what blocks a valid
export (records missing efficacy or an operator licence, treated plots
without a crop, farm identity fields not yet entered from the REA papers)
and `build_cuaderno` turns one farm+season into the official CUE descriptor
JSON (`TratamFito` block), refusing while the precheck is not clean so
nothing is silently dropped. Multi-crop treatments split into one
`TratamFito` per crop snapshot (3.11.4 descriptor rule), each split keyed by
its own frozen alias; a core `crop` row is the SIEX plot+crop+season unit
(DGC) and is referenced by a client-assigned integer (`CodigoDGCAjena`,
aliases again). Soft-deleted records emit `Borrar` entries under their
existing aliases; never-exported deletions leave no trace. The serializer's
output is schema-validated in tests against the vendored official JSON
Schema (the `jsonschema` crate, dev-dependency only — never in the shipped
binary). The shell exposes both as commands (`export_cuaderno_precheck`,
async `export_cuaderno` writing the JSON to a dialog-chosen path), and the
record-book view runs precheck-then-export, rendering the blockers as a
fix-it list.

**Backup** (in `terrazgo_core::backup`): export is a `VACUUM INTO` snapshot
— consistent and compact while the app runs, no WAL sidecars — which is then
re-opened and integrity-checked before success is reported; an unverified
backup of regulatory records is worse than none. Import validates, exports
an automatic safety copy of the live db first, then swaps and re-migrates.
Older backups are fine (migrated forward on open); newer-than-app backups
are refused. Details: [backup-restore.md](backup-restore.md).

**Sync** is staged, and the stages explain several present-day choices:

1. *One-way mirror* — phone exports, laptop imports a replaceable copy. No
   merge logic; ships early. (The backup machinery above is most of it.)
2. *Bidirectional local sync* — deltas derived from `record_change`,
   exchanged by file copy or LAN. The conflict rules defined here…
3. *Cloud* — …are reused unchanged; only the transport is new.

UUIDv7 everywhere and full row images in `record_change` exist precisely so
Stages 2–3 need no schema rework. The live db file itself must never be
placed on a network drive or file-sync service — WAL breaks across network
filesystems; sync travels through exported bundles only.

## The report engine: Typst in-process

Printable documents (the official cuaderno first; fertilisation plans, cost
reports and analytics dossiers later) are rendered by `crates/terrazgo-report`
(added 2026-07-16): **Typst as a library**, not a webview `print()` (never
wired on Linux/wry or Android) and not a low-level PDF writer (no layout
engine — an unbounded treatments table needs per-cell wrapping, cross-page
row breaking and repeating headers).

The whole pipeline is offline by construction:

- **Templates** are Typst source owned by the consuming module, embedded via
  `include_str!`. Report labels are per-country template content (Spanish
  for the official cuaderno), never UI i18n keys.
- **Fonts**: the four Liberation Sans faces (~1.6 MB, OFL-1.1 — licence
  vendored alongside in `crates/terrazgo-report/fonts/`) are embedded with
  `include_bytes!`. Liberation Sans is metric-compatible with Arial, the
  look of the Spanish administrative forms. No system-font scanning: output
  is identical on every platform.
- **No package resolution**: typst-as-lib's network-capable features stay
  off, so an `@preview` import in a template fails the compile loudly
  instead of reaching for the network.

The API is one function: `render_pdf(template, &serde_json::Value)` →
`RenderedPdf { bytes, page_count, warnings }`. Inputs must be a JSON object
and arrive in the template as `sys.inputs` (strings, ints, floats, bools,
`null`→`none`, arrays, nested objects). Two contracts matter for template
authors:

- **Pin the family** (`#set text(font: "Liberation Sans")`) and assert the
  render produced **zero warnings** in the template's tests. Typst treats an
  unknown font family as a warning plus silent fallback — the warnings list
  is where that surfaces, and the crate's own tests pin the tripwire (an
  unknown family must produce a warning; the embedded faces must index under
  exactly that family name and cover the Spanish glyph set).
- **A failed template `#assert` aborts compilation** — templates can assert
  on their `sys.inputs` shape, turning data-contract drift into a test
  failure instead of a wrong document.

Rendering is synchronous and CPU-bound; commands that call it follow the
long-running-command rule (`async fn`).

**The printable cuaderno** (first consumer, added 2026-07-16) follows one
more rule worth copying: `module_cue::report::cuaderno_inputs` pre-formats
EVERYTHING into strings (dd/mm/yyyy dates, decimal-comma numbers, the
official Spanish words for closed lookups) so the template does layout only,
and the data contract is pinned as plain JSON in `tests/report.rs` without
parsing a PDF. The document mirrors the official model's sections 1, 2.1 and
3.1 with its cross-reference scheme (the treatments register names
operators, equipment and plots by the order numbers of the earlier tables —
all built from the same records, so a reference cannot dangle), prints
missing fields blank like the paper form (no precheck — unlike the SIEX
export, a farmer can always print the current state), and adds a
plazo-de-seguridad column the model lacks (the content list of RD 1311/2012
Anexo III is what binds, and PHI is on it).

## Device-local settings

App settings live in `settings.json` beside the databases, not in either of
them (`terrazgo_core::settings`, added 2026-07-11). The reasoning is the
same lifecycle test that keeps `geo-cache.db` a separate file: settings are
device-local preferences — no audit trail, no sync, and deliberately **not
in backups** (a backup exists so regulatory records survive a lost device;
it must not impose the old device's cache cap on a new one).

The file is one flat serde struct. Defaults live in code: a missing file or
field means "use the default" (`#[serde(default)]`), so adding a setting is
adding a struct field — no migrations, and old and new versions read each
other's files. Fields whose default is owned elsewhere are `Option` (`None`
= follow the owner's constant, e.g. the tile-cache cap defaulting to
`terrazgo_geo::db::TILE_CACHE_MAX_BYTES`), which keeps a future default
change effective for users who never touched the knob. Writes are atomic
(temp file + rename); an unreadable file falls back to defaults — settings
are the one store where self-healing beats surfacing corruption. Validation
belongs to each setting's owning crate (the cache cap range check lives in
terrazgo-geo).

Two deliberate exclusions: the display language stays in `localStorage`
(the frontend must resolve it synchronously before first render, and the
i18n layer stays backend-independent — revisit if settings ever roam), and
**secrets never go in this file** (it is plain text; future credentials
such as CDSE accounts need their own storage decision).

In the UI, the Settings view hosts the language selector, the offline-map
cache size (applied immediately — shrinking evicts on the spot), the
clear-stored-maps action, the user-profiles section and the backup
export/import moved from the Status view.

**User profiles** (added 2026-07-17) split across both stores by the same
lifecycle test. The profiles themselves (`user_profile`: display name,
optional operator link) are farm data in the full sense — synced,
`record_change`-logged, soft-deleted only, because a profile id is the
author stamp on `record_change.actor` and must resolve in years-old
audit rows on any device. But *which* profile is active is a property of
the device ("who is using THIS phone"), so it is `active_user_id` in
`settings.json` — tolerated when dangling (profile deleted elsewhere,
backup restored onto a new install): the shell degrades to "no active
profile", never errors. Deleting the active profile clears the setting in
the same command. Profiles are identification, not security — no
credentials; real authentication arrives with cloud sync, and a local
password guarding a SQLite file the user owns would be theatre.

**The author stamp** (wired 2026-07-17): every repository write function —
core, module-cue, module-sigpac — takes an `actor: Option<&str>` parameter
and hands it to the audit helpers, which write it to `record_change.actor`.
module-sigpac is in that list despite being "the lookup module" because
verification writes: `verify_plot` persists the fetched boundary as a
`geo_feature` row and the zone results as `plot_zone_flag` rows — synced,
audit-logged user data — and "who verified this plot" is attribution like
any other.
The shell's write commands read the active profile id from `SettingsState`
per call (`active_actor`, settings lock released before any other lock is
taken) and pass it down; the demo seed passes `None`. Explicit threading
was chosen over connection-attached session state deliberately: the backup
import swaps the connection mid-session, which would silently drop an
attached actor, while a parameter cannot be forgotten without the compiler
noticing. The stamp is verbatim and unvalidated — profiles are soft-deleted
only, so it resolves at inspection time, and a foreign device's claim must
survive sync even where it can't be resolved locally. `None` stays NULL:
the honest "no active profile" state, shared by every row written before
profiles existed. Each log row records who performed THAT write, not the
row's original creator.

**Conflicts are two different problems** (Stage-2 design notes, 2026-07-05
— nothing here is built yet, but the strategy is decided):

1. *The same row edited on two devices.* User A fixes a note on the phone
   while user B corrects a dose on the tablet. This is the classic sync
   conflict and it is mechanical: merge rules over `record_change` (per-field
   last-writer-wins, or flag-for-review on regulatory fields — policy picked
   at Stage-2 design time). The full row images exist so a device can diff
   both states and apply the rule deterministically.
2. *Two different rows describing the same real-world event.* Two workers
   each record "applied product X on plot Y yesterday" on their own phones.
   Distinct UUIDs, both rows internally valid — **no sync algorithm can
   resolve this**, because from the data's point of view there is no
   conflict. The very property that makes the merge collision-free (UUIDs)
   guarantees both records survive it.

The strategy for problem 2 is layered, because no single layer is airtight:

- **Workflow prevention.** Multi-user means `created_by` on records and the
  convention that the applicator records their own treatment. Most
  duplicates are an accountability gap, not a technical one.
- **Entry-time warning.** A cheap local query at form-save time: same
  plot(s) + date + product already known to this device? Warn. Porous while
  offline, nearly free.
- **Merge-time detection by natural key, resolution by human.** Each
  regulatory record type gets a *natural key* (treatment: farm + plots +
  application date + authorisation number). An incoming record matching an
  existing one on the natural key under a different UUID is flagged into a
  **duplicate-suspect review queue** — never auto-dropped. The confirmed
  loser is soft-deleted with a reason ("duplicate of …"), which
  `record_change` logs, so the audit trail shows the dedup itself.
- **Never auto-delete.** Content-derived IDs (hash the natural key so
  duplicates collapse themselves) are rejected: brittle (dose 1.5 vs 1.49
  and the hash misses exactly when relied upon) and wrong for near-matches
  that are legitimate (two real applications of the same product on
  different recintos the same day). Machines detect; humans decide.

Natural keys for *matching*, UUIDs for *identity* — the same split already
used once: `active_substance` dedupes across devices by `cas_number`. The
Stage-2 design list therefore carries: pick the natural key per regulatory
table, spec the suspect queue, and (already parked there) decide whether
alert acknowledgements roam.

## Files the user picks: paths on desktop, content URIs on Android

Every file a user chooses in a native dialog (backup export/import, the SIEX
and PDF exports, boundary-file import) flows through
`src-tauri/src/user_files.rs`. The reason is Android (2026-07-18): there the
dialogs are the system document picker (Storage Access Framework), which
*creates* the destination itself and returns a `content://` URI — `std::fs`
cannot open one, which is how the first on-device exports produced 0-byte
files in Downloads plus an os-error-2 notification. The fs plugin
(`tauri-plugin-fs`, Rust-side only — no fs commands are granted to the
webview) resolves a content URI into an ordinary file descriptor through the
platform `ContentResolver`; plain desktop paths take the `std::fs` route
inside the same call, so commands have one code path.

Three helpers cover every caller: `write_user_file` (in-memory bytes →
destination, truncating), `stage_dest` + `copy_to_user_file` (for producers
that need a real filesystem path to write to — SQLite's `VACUUM INTO` — the
verified snapshot lands in a private staging file and is then streamed out),
and `stage_user_source` (read side: plain paths pass through untouched; a
URI is streamed into a staging copy first, because rusqlite and the GPKG
reader need real paths). Staging files live under the app *cache* dir and
delete themselves on drop — transient by construction, never in backups.

Mapping is whole-app infrastructure (plots today; irrigation, zone flags,
treatments as overlays later), not a SIGPAC feature. Three pieces
(implemented 2026-07-07; design history in
[sigpac-integration.md](sigpac-integration.md)):

**One network seam.** The webview never talks to the internet — production
CSP stays `default-src 'self'` plus the `geo:` scheme. MapLibre loads
everything (tiles, style JSON, glyphs, sprites) from
`geo://…/tiles/{source}/{z}/{x}/{y}` and `geo://…/res/{prefix}/{rest}`,
served by `src-tauri/src/geo_protocol.rs` → `terrazgo_geo::fetch`:
cache lookup in `geo-cache.db`, miss → `ureq` GET (lock **never** held
across network I/O; tile bursts fetch in parallel), store, respond. Only
allowlisted upstreams exist (`terrazgo_geo::sources`, data not code — a new
base map or overlay tile source is a new entry). Upstream styles are
rewritten in Rust (`terrazgo_geo::style`) so no external URL ever reaches
the webview; responses carry `Access-Control-Allow-Origin` because the page
origin is cross-origin to `geo://localhost` and MapLibre uses `fetch()`.

Terrazgo-geo hosting the app's only HTTP client is deliberate today (one
consumer: the map tier plus the SIGPAC lookups riding it) and has a
pre-agreed evolution (2026-07-14): when a second in-app network consumer
becomes real (catalogue refresh, weather, CDSE), the *generic* layer —
agent construction with the platform-verifier TLS policy, timeouts, the
offline/error diagnosis — extracts into its own small networking crate;
the cache-through semantics, source registry and style rewriting stay
geo. terrazgo-core never gains a network dependency: core having no HTTP
crate in its tree is the structural enforcement of "no network calls in
core or module code paths", not an accident.

**Android TLS bootstrap (2026-07-18).** The platform verifier delegates to
the Android trust store over JNI and panics on first use if it was never
handed the JVM + app context — on the first on-device test that panic killed
a tokio worker mid-fetch and left a silently blank map. `terrazgo-geo`'s
`android` module (Android-only compile target) initializes it lazily at the
top of `fetch::http_get`, the single chokepoint every network request passes
through. Lazy-at-fetch rather than at-startup is load-bearing: the Rust main
thread is spawned from the process-lifecycle `onCreate` and races the
activity's own `onCreate`, where tao captures the activity context — but a
fetch can only be triggered once the webview exists, and the webview lives
inside the activity, so by then the context is guaranteed. The JNI handles
come from tao's `main_android_context()` (the same tao copy the Tauri
runtime links — the version pin matters, its statics hold the context). The
verifier's Kotlin half is pulled into the APK by
`src-tauri/gen/android/app/build.gradle.kts`, which locates the crate's
bundled Maven repository via `cargo metadata` so the Kotlin version tracks
the Rust crate automatically; a ProGuard keep rule protects the
JNI-only-reachable class from release-build shrinking. An init failure
surfaces through the normal `geo_offline {reason}` diagnosis instead of a
dead worker.

**Two databases, two natures.** Geometry a user attaches to a plot is *user
data*: the core `geo_feature` table (exclusive-arc FKs, audit-logged,
soft-deleted, synced, in backups). Tiles and styles are *derived and
re-fetchable*: `geo-cache.db`, a separate file with its own tiny migration
runner, deliberately outside `VACUUM INTO` backups, `record_change` and any
future sync. Deleting it loses warm caches, nothing else — which is why its
schema guard is recreation: `open_cache` probes for the current shape and
deletes + rebuilds a stale cache file, so pre-release schema squashes never
strand a deployed cache (2026-07-11). Offline with a cold cache the map
degrades to a plain background with stored geometry — the app never stops
working.

The tile cache is size-capped (2026-07-11): serving a tile touches
`last_used_at` (at most once per UTC day, so bursts don't turn reads into
writes), and at startup — off the critical path — the shell evicts
least-recently-used tiles past `TILE_CACHE_MAX_BYTES` (512 MiB) and
reclaims the space with `VACUUM`. Only tiles are capped: the `resource`
table also holds the SIGPAC lookup and zone-check responses that keep a
verified plot verifiable offline, and evicting those would silently break
that promise for kilobytes of savings. Since 2026-07-11 the cap is a user
setting (Settings view; `tile_cache_max_bytes` in `settings.json`, unset =
the `TILE_CACHE_MAX_BYTES` default, changes enforced immediately), and the
same view offers clearing the tile cache outright — `resource` rows survive
that too. Revisit the default at the mobile milestone, where device storage
is the real constraint.

**Layers as data.** `src/lib/mapLayers.js` mirrors the `nav.js` philosophy:
a module contributes a map overlay by adding one entry — either a GeoJSON
layer (id, label key, `load()` via invoke, MapLibre style specs) or a
vector-tile layer (`vector()` returning the source spec: `geo://` tile
template, zoom bounds, attribution). `MapCanvas.svelte` is the
embeddable engine wrapper (base-layer switch, selection, terra-draw
drawing); `MapView.svelte` is the routed workspace around it (farm selector,
layer panel, draw/import workflows, `#/map?farm=…&plot=…` deep links);
FarmView embeds the same canvas read-only. MapLibre and terra-draw are
`import()`-ed lazily so form views never pay for the map chunk.

Boundary files (GeoJSON, and GeoPackage — it *is* SQLite, read with rusqlite
+ geozero for the WKB blobs) import through `terrazgo_geo::import`: a light
list for the picker, then one validated geometry. Every geometry is checked
by core's pure-parsing `geojson` validator at the write path, whatever its
origin. Accepted SRS are geographic only — 4326, 4258 (ETRS89) and 4081
(REGCAN95, the Canary SIGPAC datum; its EPSG-registered shift to WGS84 is
0,0,0, so identity is correct). Projected files fail with a stable error;
the agreed-but-dormant escape hatch is a proj4rs-backed EPSG registry
(decision 2026-07-08).

**The Spanish provider: `crates/module-sigpac`** (P3, shipped 2026-07-08).
A normal module — registered in `registry.rs`, empty `migration_set()` for
now — that turns the 7-part reference `plot_es_extension` already stores
into live data from FEGA's Nube de SIGPAC (the sanctioned third-party
surface, CC BY 4.0). `reference.rs` validates/round-trips the ref,
`client.rs` looks a recinto up by code or by point, and crucially the module
has **no HTTP dependency**: every request rides
`terrazgo_geo::fetch::cached_resource`, so responses cache in `geo-cache.db`
and a lookup seen once works offline forever. Service quirk worth knowing:
an unknown ref answers HTTP 200 with an empty FeatureCollection — the client
maps that to `Ok(None)`, never an error. Tests run fully offline against
harvested real responses in `tests/fixtures/`.

On top of the client sit `storage.rs` (fetched recinto → `geo_feature` with
`source='sigpac'`, official area alongside — never overwriting the user's
declared `plot.area_ha` — and the dedup query matching stored refs
numerically) and `service.rs` (the composed operations the shell's three
async commands wrap: lookup by reference, lookup by point, verify-a-plot).
The UI opens three doors into the ONE plot-creation flow: the plot form's
verify/prefill (FarmView), the map's pick-a-point → create-or-attach
(MapView + a `picking` mode on MapCanvas), and the import picker's
"create plot from recinto" for SIGPAC files. All three converge on the same
`create_plot`/`save_geo_feature` write paths — a SIGPAC-born plot is an
ordinary plot plus one more geometry source.

**Zone flags** (P4, 2026-07-08) ride the same verification tap: after the
boundary stores, the module queries the three regulatory layers (nitrate-
vulnerable, phytosanitary restriction, Natura 2000) and writes core's
`plot_zone_flag` — one row per (plot, zone kind, campaign, source),
*negatives included* (an 'outside' row proves the check ran and was clear).
Unlike alerts, flags cannot be re-derived offline, so they are user data:
audit-logged, synced, backed up. The campaign year comes from the provider's
download-directory listing (the only machine-readable statement of it). The
alert engine (module-cue) reads the flags from core — never from
module-sigpac — and raises one standing alert per (plot, zone kind) whose
latest campaign says 'inside'; the subject is the plot, so a dismissal
survives re-checks and campaign rollovers. A zone-check failure after the
boundary stored is reported (`zone_check_error`), never fatal, and the plot
cards show the flags as chips.

**The recinto overlay** (2026-07-11): SIGPAC's official parcel fabric as a
toggleable vector-tile layer over both base maps — the Nube de SIGPAC MVT
service (pbf, z12–15, single source-layer `recinto`), one `sigpac-recintos`
entry in the source allowlist and one vector entry in `mapLayers.js`, with
`SIGPAC © FEGA (CC BY 4.0)` shown while active. Two service quirks shape
the caching: the tile URL carries no campaign year (the fixed path always
serves the *current* campaign), so cache rows are keyed
`sigpac-recintos@{campaign}` using the same campaign resolution the zone
checks use — a re-resolve at rollover (any plot verification does one)
switches the key, and storing the first new-campaign tile evicts the old
campaign's rows; and tiles with no recintos answer HTTP 404, which the
fetch layer caches and serves as an *empty* payload (a valid empty vector
tile), so known-empty countryside costs no repeat requests and reads as
empty — not as an error — offline.

**The remaining Nube MVT overlays** (2026-07-12, phase 2 of
[map-layers-roadmap.md](map-layers-roadmap.md)): declared-crop lines
(`cultivo_declarado` — the service's fixed path serves the *previous*
campaign, the label says so) and landscape elements. The latter spans three
tile services (area/line/point) behind one toggle, which grew the
`mapLayers.js` contract minimally: an entry may declare `vectors(base)` —
several keyed source specs — instead of `vector(base)`, and style specs pick
theirs with `sourceKey`. All four sources are ordinary campaign-keyed,
empty-on-404 registry entries; a registry contract test pins the shared
SIGPAC service shape (pbf, z12–15, CC BY 4.0, campaign-keyed, 404-as-empty).

**Point inspect + zoom hints** (2026-07-12). Clicking the map lists what
every *visible* overlay renders at that point in an "At this point" panel in
the side column: entries opt in with `inspect(props)` (label/value rows the
view translates), `MapCanvas` dedupes `queryRenderedFeatures` results per
feature, and the recinto overlay gained an invisible fill so polygon
interiors are hit-testable, not just their outlines. Tile overlays also
declare `minZoom`, and the layer panel warns ("zoom in to see: …") while
such a layer is toggled on below it — before this, an on-toggle below z12
silently rendered nothing, which reads as a broken layer. Live-service
quirk the panel corrects for: MVT attribute surfaces are **m²** while the
REST lookups speak hectares (verified on the same recinto: 1152241 vs
115.2241).

**Own-data overlays** (2026-07-12, phase 1 of
[map-layers-roadmap.md](map-layers-roadmap.md)): the app's own records as
plot tints, no network involved. `phi-status` tints each treated plot by
whether a PHI window contains today (red = harvest restricted, green =
treated and clear), backed by `list_phi_status` → module-cue's
`phi_status_for_farm` — derived on read from the treatment records (same
`[application_date, phi_end_date)` rule as the alerts, tested against it),
never stored. `zone-flags` tints plots by the stored zone checks (latest
campaign's 'inside' per plot and zone kind — the chip rule), one translucent
fill per zone kind so overlapping memberships blend. Both are plain GeoJSON
`mapLayers.js` entries that join `list_geo_features` with their status
command, one feature per plot (stacked boundary sources would double the
tint). They start toggled off (`defaultVisible: false`) and declare a
`legend` (color swatch + label pairs) the layer panel shows while visible.

## The frontend in one page

Full conventions in [frontend-conventions.md](frontend-conventions.md); the
architectural skeleton:

- **Two tiers.** Framework-agnostic plain JS (`i18n.js`, dictionaries,
  `backend.js`, `nav.js`) survives any framework swap; Svelte views sit on
  top. Business logic lives in Rust — the frontend collects input and
  renders results.
- **Routing** is a hand-rolled hash router in `App.svelte`. Navigation
  destinations are data (`lib/nav.js`), rendered twice: collapsible sidebar
  on wide screens, bottom tab bar on phones. Adding a view = one entry +
  one router branch.
- **Feedback** flows through the notification bell: `run()` turns boundary
  errors into red notifications (panel auto-opens), successes tick the
  badge. There is no other error surface.
- **i18n**: every user-facing string is a key present in *every* locale
  dictionary (a contract test enforces it); schema codes are translated at
  display time via `tCode`; user-entered data is never translated.
- **No `@tauri-apps/api` dependency** — `withGlobalTauri` exposes
  `window.__TAURI__`, and plugin calls ride the same transport
  (`invoke("plugin:dialog|save")`).

## Rust, for the JavaScript developer who lives here

Not a tutorial — a map from constructs this codebase actually uses to the
nearest JS mental model, with pointers to real examples.

**Ownership & borrowing show up as repository signatures.** Reads take
`&Connection` (shared borrow — many readers fine), writes take
`&mut Connection` (exclusive borrow — the compiler guarantees nobody else
touches the connection mid-transaction). Where JS would document "don't call
this concurrently", Rust makes it unrepresentable.

**`Result` + `?` replace exceptions.** Every fallible function returns
`Result<T, E>`. The `?` operator is "return the error to my caller if this
failed" — like `throw`, but visible in the signature and checked by the
compiler. When the error type changes across a boundary (`CoreError` inside
a CUE repository, `CueError` out), `?` silently applies the `From`
conversion — that is why `From<CoreError> for CueError` being
variant-preserving matters: callers still match on what actually happened.

**`Option<T>` replaces `null`/`undefined`.** `machinery_id: Option<String>`
must be unwrapped to be used; there is no "forgot to check" path. On the
JSON wire it is just `null`, which is why the frontend normalises empty
inputs with `value.trim() || null`.

**Traits are interfaces — two flavours.** The `Module` trait is used as a
*trait object* (`Vec<Box<dyn Module>>` in `registry.rs`): different concrete
types behind one interface, dispatched at runtime, like a JS array of
objects sharing a shape. Derived traits (`#[derive(Serialize, Clone)]`) are
compile-time code generation, closer to decorators that write the
boilerplate for you. `thiserror` derives error boilerplate the same way.

**Macros run at compile time.** `tauri::generate_handler![...]` needs the
literal function paths — which is why commands cannot be registered
dynamically through the `Module` trait, and why a contract test keeps the
manual list honest. `include_str!` embeds the SQL files into the binary at
compile time: the shipped app has no loose files to lose.

**Cargo features are compile-time flags.** The `demo` feature gates the
seeding code; `seed_demo_data` additionally refuses at runtime in release
builds. Workspace dependencies (`[workspace.dependencies]`) pin one version
of everything for all crates — load-bearing here because `Connection`
crosses crate boundaries and `libsqlite3-sys` cannot appear twice.

**`unwrap()` is banned outside tests** — mechanically, by a workspace-level
clippy lint. Where a JS codebase would sprinkle "should never happen", here
the error must be handled or propagated. Tests are exempt: a failed unwrap
in a test *is* the test failing.

## What guards all of this

Most invariants above are invisible to the compiler, so tests hold the line
(the testing strategy below; compliance rules are written test-first):

| Guard | Where |
|---|---|
| Repository behaviour incl. audit payload contract | `crates/*/tests/repository.rs` |
| Compliance rules (PHI maths, alert windows) — test-first | `crates/terrazgo-core/src/date.rs` tests, `crates/module-cue/src/alerts.rs` tests |
| Migrations apply fresh AND from the previous version | `crates/*/tests/migrations.rs`, `src-tauri/tests/` |
| Every command registered ↔ every registration has a command | `src-tauri/tests/command_registration.rs` |
| Locale dictionaries in sync ↔ error codes covered | `src-tauri/tests/i18n_contract.rs` |
| No `unwrap`/`expect` outside tests | `[workspace.lints.clippy]` in `Cargo.toml` + `clippy.toml` |
| fmt / clippy `-D warnings` / prettier / eslint / tests on every push & PR | `.github/workflows/ci.yml` |
| RustSec advisories on the dependency tree | `deny.toml` + the CI `audit` job |

### Testing strategy

Selective test-first, by code category:

1. **Domain/business logic — test-first (TDD), regulatory or not.**
   Compliance rules (PHI end dates, licence/ITV expiry, alert generation,
   record validation) and equally any module's computational core, such as
   future irrigation recommendations or analytics: the failing test is
   written from the requirement's source of truth — a regulation, a
   technical reference like FAO-56 — then implemented. Edge cases (leap
   years, campaign boundaries, multi-plot treatments) are in scope.
2. **Repository / data layer — test-alongside.** Every public repository
   function runs against an in-memory SQLite database with migrations applied.
3. **Migrations — always tested.** Each migration applies cleanly to a fresh
   database AND to a database at the previous version.
4. **Tauri commands — thin, lightly tested.** Logic lives in the crates and is
   tested at layers 1–2; commands are wiring.
5. **UI — no unit tests while it is in flux.** Runtime verification is
   scripted instead: a headless-browser harness drives the built bundle with a
   stubbed `invoke` (error-stub or backend-harvested fixtures), and an
   app-level harness drives the real debug binary in the real webview.

## Releases

Releases live at
[github.com/clozanoruiz/terrazgo](https://github.com/clozanoruiz/terrazgo),
together with the issue tracker: per-platform installers (Linux AppImage/deb,
Windows NSIS + portable `.zip`) plus the **complete source of that version** —
one snapshot commit per release, so the AGPL source-offer travels with every
distributed binary. The installers are built by that repository's own
`build.yml` workflow from the tagged source itself, so every binary comes from
exactly the source published next to it. Each artifact carries signed SLSA
build provenance and every release ships a CycloneDX SBOM attested against
the installers — verify any download with
`gh attestation verify <file> --repo clozanoruiz/terrazgo`. Release notes are
written by hand before a draft release is published.

## Recipes — where to start when you want to…

- **Add a command end-to-end** → checklist in
  [frontend-conventions.md](frontend-conventions.md#adding-a-command-end-to-end-checklist)
  (repository + test → thin wrapper → `generate_handler!` → i18n keys →
  `refresh_alerts` if inputs changed → `run()` + `notify()`).
- **Add a view** → one `NAV_ITEMS` entry in `src/lib/nav.js` + a router
  branch in `App.svelte`; keys in every locale file.
- **Add a module** → new crate under `crates/` depending on
  `terrazgo-core`; implement `Module` (name + `migration_set()`); register
  it at the END of `registered_modules()`; list its commands manually in
  `lib.rs`. The core does not change.
- **Change the schema** → high-stakes: design first.
  Pre-release, edit the squashed `0001`/`0002` and recreate dev databases;
  post-release, append a migration and write both migration tests.
- **Add a language** → one `SUPPORTED` entry in `src/i18n.js` + one
  dictionary file with the full key set (the contract test enforces
  completeness).
- **Re-theme** → edit the CSS variables in `:root` (`src/styles.css`);
  nothing else references raw colors.
