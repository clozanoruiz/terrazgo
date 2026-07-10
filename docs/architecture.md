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
│  state.rs (AppState + GeoState, connections in Mutex)  │
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
  (`geo_feature`), the audit helpers, date utilities, the pure-parsing
  GeoJSON validator and backup.
- Modules depend on `terrazgo-core`. The CUE module owns the treatment
  domain: products, authorisations, treatment records, alerts.
- `terrazgo-geo` depends on `terrazgo-core` only (for the GeoJSON validator
  and error conventions) and owns **all network I/O in the app** plus the
  boundary-file importers. No user data lives there.
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

1. Resolve the data dir from the app identifier (`org.terrazgo.desktop` →
   `~/.local/share/org.terrazgo.desktop/` on Linux) and open/create
   `terrazgo.db` — WAL mode, `foreign_keys = ON`.
2. Run `composed_migrations()` — core steps first, then each registered
   module's steps in registration order, one global `user_version`.
3. `refresh_alerts(today)` — idempotent reconciliation, so the UI never
   opens on stale alert state.
4. Put `AppState { conn: Mutex<Connection>, db_path, schema_version }` into
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

## Backup and the road to sync

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

## The map tier: geo://, two databases, layers as data

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
base map or the future SIGPAC MVT layer is a new entry). Upstream styles are
rewritten in Rust (`terrazgo_geo::style`) so no external URL ever reaches
the webview; responses carry `Access-Control-Allow-Origin` because the page
origin is cross-origin to `geo://localhost` and MapLibre uses `fetch()`.

**Two databases, two natures.** Geometry a user attaches to a plot is *user
data*: the core `geo_feature` table (exclusive-arc FKs, audit-logged,
soft-deleted, synced, in backups). Tiles and styles are *derived and
re-fetchable*: `geo-cache.db`, a separate file with its own tiny migration
runner, deliberately outside `VACUUM INTO` backups, `record_change` and any
future sync. Deleting it loses warm caches, nothing else. Offline with a
cold cache the map degrades to a plain background with stored geometry —
the app never stops working.

**Layers as data.** `src/lib/mapLayers.js` mirrors the `nav.js` philosophy:
a module contributes a map overlay by adding one entry (id, label key,
`load()` via invoke, MapLibre style specs). `MapCanvas.svelte` is the
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

1. **Business logic & compliance rules — test-first (TDD).** PHI end dates,
   licence/ITV expiry, alert generation, record validation: the failing test
   is written from the regulatory requirement, then implemented. Edge cases
   (leap years, campaign boundaries, multi-plot treatments) are in scope.
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
Windows NSIS + portable `.exe`) plus the **complete source of that version** —
one snapshot commit per release, so the AGPL source-offer travels with every
distributed binary. The installers are built by that repository's own
`build.yml` workflow from the tagged source itself, so every binary comes from
exactly the source published next to it. Release notes are written by hand
before a draft release is published.

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
