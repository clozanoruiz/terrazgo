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
`terrazgo-net` crate (the dependency list below names it); its two consumers are
`terrazgo-geo`, which wraps it in cache-through map fetching, and the shell's
catalogue refresh, which the user has to ask for. With no network the app
keeps working.

```
┌────────────────────────────────────────────────────────┐
│  src/  — Svelte 5 frontend (views only)                │
│         framework-agnostic layer: i18n.js, backend.js, │
│         nav.js, mapLayers.js · reactive glue: notifs   │
└──────────┬─────────────────────────┬───────────────────┘
           │ invoke (JSON in/out)    │ geo:// (tiles, styles)
┌──────────▼─────────────────────────▼───────────────────┐
│  src-tauri/  — shell (crate `terrazgo`)                │
│  commands.rs (boundary) + commands/ (one file/domain)  │
│  geo_protocol.rs (geo:// handler) · registry.rs        │
│  db.rs (composed migration runner)                     │
│  state.rs (AppState, GeoState, SettingsState)          │
└───────┬──────────────────┬──────────────────┬──────────┘
        │                  │                  │
      (the shell also consumes the two READ MODELS above the modules:
       terrazgo-recordbook — the printed book — and terrazgo-siex — the
       exchange descriptor. Siblings; neither depends on the other.)

┌───────▼──────────┐ ┌─────▼────────────┐ ┌───▼──────────────────┐
│ crates/          │ │ crates/          │ │ crates/terrazgo-geo  │
│ module-cue       │▶│ terrazgo-core    │◀│ tile/resource cache, │
│ treatment domain │ │ farm registry +  │ │ base-map sources,    │
│ product,         │ │ geo_feature,     │ │ style rewriting,     │
│ treatment, alert │ │ audit, backup,   │ │ boundary-file import │
│ + CUE lookups    │ │ date, geojson    │ │ cache-through fetch  │
└──────────────────┘ └─────┬────────────┘ └───┬──────────────────┘
                     ┌─────▼───────┐   ┌──────▼───────┐  ┌──────────────────┐
                     │ terrazgo.db │   │ geo-cache.db │  │ terrazgo-net     │
                     │ user data,  │   │ tiles/styles │  │ the ONLY HTTP    │
                     │ WAL, FKs on │   │ own runner   │  │ agent + TLS      │
                     │ (Database)  │   │ (Database)   │  │                  │
                     └─────────────┘   └──────────────┘  └──────────────────┘
                        derived, re-fetchable, never in    ▲ used by geo and
                        backups or record_change           │ the shell only
```

Dependency direction is one-way and enforced by the crate graph — the
compiler, not discipline, prevents a core→module import:

- `terrazgo-core` depends on **no module and never on the shell**. It owns
  the farm registry (land, calendar, people, machines, and the premises a
  treatment can be applied to), geometry storage
  (`geo_feature`), the imported reference catalogues (`catalogue` +
  vendored SIEX snapshot), the audit helpers, date utilities, the
  pure-parsing GeoJSON validator, backup and the device-local settings file.
- Modules depend on `terrazgo-core`. The CUE module owns the treatment
  domain: products, authorisations, treatment records, alerts.
- `terrazgo-net` is the **network seam** (2026-08-09): the process-wide HTTP
  agent, its TLS trust policy, the Android bootstrap that policy needs, and the
  offline diagnosis. It depends on no other workspace crate and knows nothing
  about caching, tiles or catalogues — it answers "fetch these bytes, or say
  why not". **Core and the modules must never name it**: core having no HTTP
  crate anywhere in its dependency tree is the build-enforced form of the
  offline-first rule.
- `terrazgo-geo` depends on `terrazgo-core` (for the GeoJSON validator and
  error conventions) and on `terrazgo-net`, and owns the map's cache-through
  fetching plus the boundary-file importers. No user data lives there.
- `terrazgo-report` depends on no other workspace crate — pure
  infrastructure: in-process **PDF** generation via Typst (Liberation Sans
  faces embedded in the binary) and in-process **spreadsheet** generation via
  `rust_xlsxwriter` (added 2026-08-02). Templates and workbook descriptions
  live with whoever owns the document; both are rendered here, so document
  technology never leaks into a domain crate (see "The report engine" below).
- `terrazgo-recordbook` is the **record book itself** (2026-08-07): the data
  assembly, the per-language labels, the region → language map, the Typst
  template and both renderers. It is a **read model** — it holds no state and
  writes nothing, it reads core and the domain modules and projects one
  document. That is why it may depend on several modules where a module never
  may: it is a top-layer *consumer*, like the shell, and a **leaf** that
  nothing depends on but the shell, so it cannot create cycles. It was
  extracted from `module_cue::report`, whose assembly already read
  `terrazgo_core::` 18 times against its own domain 4 — the book was a core
  reader living in the treatment module, and a second contributing module
  would have forced everything the book touches into the core. It owns its own
  `RecordbookError` (2026-08-07): the book borrowed `module_cue::Result` when
  it was first extracted, and a document that reads several domains must not
  report failures as though it were any one of them.
- `terrazgo-siex` is the **SIEX descriptor export** (2026-08-20), and the
  record book's **sibling**: the second top-layer consumer, on identical terms.
  It reads core and every domain module and projects one exchange document —
  the official CUE descriptor of FEGA Anexo VI — with frozen integer aliases, a
  precheck and JSON-Schema validation. Neither it nor the record book depends
  on the other; they share their *sources* and nothing else, because a Spanish
  form for a human inspector and a machine exchange format are not the same
  document and do not fail the same way. It lived in `module_cue::export` until
  ten of the format's fifteen activity blocks turned out to come from
  `module-fertilisation` and `module-ecoscheme`, which a module may never
  depend on — the same wall that produced `terrazgo-recordbook`, hit a second
  time, and the reason the placement rule now has two instances rather than
  one. It owns `SiexError` for the record book's reason. **Dormant**: no
  interface calls it (docs/siex-export.md), and it is kept compiling and
  validated so the descriptor cannot silently fall behind the registers the
  app captures — which is exactly what it had done by 2026-08-20, emitting one
  of fifteen activity blocks against twelve registers that had twins. The
  "finish the serializer" arc closed that gap on 2026-08-22: **thirteen blocks
  emitted**, every one with a register behind it.
- `module-fertilisation` is the record book's **second decree** (2026-08-07):
  fertilisation, irrigation and soil records under RD 1051/2022, whose art. 5
  has been binding since 1 January 2026. It depends on `terrazgo-core` and on
  no other module — where it needs something module-cue also needs (units of
  measure), that thing lives in core. The *record* of irrigation is here rather
  than in a future Irrigation module because art. 5.e puts irrigation doses and
  dates inside the same cuaderno duty as fertilisation; the Irrigation module
  keeps planning.
- `module-ecoscheme` is the record book's **third decree** (2026-08-18): the
  eco-scheme annotations of RD 1048/2022, which reach the cuaderno through
  RD 1054/2022 anexo II item 4 and print as the model's section 9. Like the
  other modules it depends on `terrazgo-core` alone. Its registers are shaped
  by the DECREE rather than by the printed form — three tables against five
  model pages — because the form hides duties the decree creates: anexo IV's
  has no page at all, art. 42 is three annotations on three deadlines that one
  printed row collapses, and model 9.3 prints three of the five dates art. 45.2
  names.
- The shell depends on all of these and owns everything Tauri-specific: command
  wrappers, the migration runner, the `geo://` protocol, managed state, the
  window.
- In the frontend, `i18n.js`, the locale dictionaries, `lib/backend.js` and
  `lib/nav.js` are plain JS with **no Svelte imports** — a future framework
  swap rewrites only the views ([frontend-conventions.md](frontend-conventions.md)).

The mental model for the split: **the core is the farm registry; a module is
a regulatory or functional domain built on top of it.** CUE gives the core
entities their Spanish phytosanitary meaning; a future irrigation module
would give plots an irrigation meaning without the core changing.

**Where a new thing goes**, so the question is answered once:

- shared **data** → the core (harvest, water points, advisors)
- a **domain** with its own logic → a module (treatments, fertilisation, SIGPAC)
- shared **presentation** → a consumer crate above the modules (the record book)

A document that spans domains does *not* justify moving those domains into the
core. The alternative considered and rejected was a `Module` trait method
letting each module contribute its own section: the record book is a **fixed
legal form** whose sections cross-reference each other (3.1 prints the plot
order numbers computed in 2.1), so no module can emit its section without
knowing the whole book, and the single template that serves every language
would fragment across crates. If a module ever needs to contribute without the
book knowing it, that method can be added **over** the consumer crate rather
than instead of it.

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

**4. The command wrapper** (`src-tauri/src/commands/cue.rs`) is deliberately
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

- Repositories return typed errors (`CoreError`, `CueError`, `GeoError` —
  `thiserror`); the record book returns `RecordbookError`.
- The command's `?` converts them into `CommandError(anyhow::Error)` via a
  blanket `From` impl.
- `CommandError` serializes as `{ code, params, message }`: `classify()`
  downcasts the `anyhow::Error` back to the domain error and asks it for its
  code. **The mapping lives in the crate that owns the error** (the
  `terrazgo_core::Classify` impl beside each enum, since 2026-08-13), so the
  exhaustive match is next to the variants: adding one is a compile error in
  the crate where it was added rather than a silent fall-through to `internal`
  in a shell file the module author never opens. The shell keeps only the
  downcast chain, which is irreducible — `anyhow::downcast_ref` needs a
  concrete type.
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
5. Put `AppState { db: Database, db_path, schema_version }` into Tauri managed
   state and register the commands.

`Database` (`terrazgo_core::db`) is the handle both long-lived connections are
held by — the app database and the geo cache. It is a `Mutex<Option<Connection>>`
and a pair of accessors, and each half earns its place.

The **lock** is there because Tauri runs commands on a thread pool and anything
in managed state must be `Send + Sync`, while a rusqlite `Connection` is
`!Sync` — it must never be used from two threads at once. So the mutex
serialises all database access through the single connection. For a single-user
desktop app that is exactly right; if a long query ever blocks the UI, the
upgrade path is a connection pool (r2d2), not removing the lock.

The **`Option`** is there so the connection can be closed. See "Shutdown".

A command reaches the connection in two steps, and each step is the only place
one of the two failures can be seen:

```rust
let db = state.db.lock()?;   // Err(Unavailable::Poisoned) — a panic mid-write
let conn = db.conn()?;       // Err(Unavailable::Closed)   — shutdown, or a
                             //                               failed import
```

**A re-entrant lock panics in debug builds instead of hanging.** Functions that
take a `&Database` lock it themselves — `terrazgo_geo::fetch` does it on every
call, since its contract is that the lock is *released* across network I/O — so
a caller already holding a guard would deadlock on re-entry. `std::sync::Mutex`
is not reentrant, and a deadlock does not fail a test, it hangs it. `lock()`
therefore checks a thread-local of currently-held databases and panics naming
the mistake. **The whole mechanism is `#[cfg(debug_assertions)]`**: a release
build has no thread-local, no field on `DbGuard`, no `Drop`, and no check —
verifiable with `nm` on the two rlibs.

`conn()` / `conn_mut()` follow the standard library's pairing, so a command says
which it needs. `Deref` would read better — `MutexGuard` itself does it — but
`deref(&self) -> &Connection` cannot return a `Result`, so a closed database
would have to panic; `MutexGuard` can only get away with it because it has no
empty state.

## Hardening, and checking for corruption

Every connection the app opens — ours and imported alike — goes through
`terrazgo_core::db::harden`, which switches off the SQL features a database
file could otherwise use against us. It costs nothing here, because the schema
defines **no views and no triggers** and the app registers **no custom SQL
functions, collations or virtual tables**:

| flag | what it stops |
|---|---|
| `DEFENSIVE` | `writable_schema`, and `PRAGMA journal_mode=OFF` |
| `ENABLE_TRIGGER = false` | a trigger in an imported file firing |
| `ENABLE_VIEW = false` | reading a view |
| `trusted_schema = OFF` | trusting expressions embedded in a schema |

Every one of those was measured rather than assumed, and the tests beside
`harden` keep them measured. Three results are worth knowing:

- **Foreign-key actions are unaffected.** The schema has 33 `ON DELETE CASCADE`
  clauses and they keep working with triggers off.
- **`DEFENSIVE` protects WAL** rather than threatening it: it refuses
  `journal_mode=OFF`, so nothing running in SQL can undo what Shutdown fixes.
- **`CREATE VIEW` and `CREATE TRIGGER` still succeed** — only *using* them is
  refused. That is why `src-tauri/tests/schema_features.rs` asserts the composed
  schema defines neither: without it a migration adding one would apply
  cleanly, ship, and fail at read time far from the cause.

### Three layers, because the flags do not travel

`harden`'s settings live on the connection, not in the database file. So a
hostile file cannot arrive with them pre-disabled — but equally they do not
travel, and **every open site has to apply them or nobody does**. Nothing in
the type system distinguishes a hardened `Connection` from a plain one, so
three mechanisms overlap:

1. **Each opener hardens.** `open_app_db` is public and returns a bare
   `Connection`; its contract is "returns a hardened connection", not "returns
   one that becomes safe if you remember to wrap it". It also means migrations
   run under the same restrictions the app later queries under.
2. **`Database::new` hardens.** The two long-lived connections — the app book
   and the geo cache — cannot be held in an unhardened state, because the
   constructor is the only way in. The compiler carries that rule.
3. **`src-tauri/tests/hardened_connections.rs` scans for the rest.** Three
   connections are short-lived and never become a `Database` (a backup being
   validated, an imported GeoPackage, the corruption check's reader), and a
   future crate could add a fourth. The test is a **rule, not an allowlist**:
   any shipping file that opens a connection must also harden it, so adding a
   crate or a feature never requires editing it.

That third one matches per file rather than per call site, deliberately:
`terrazgo-geo`'s `try_open` hardens one call away inside
`apply_pragmas_and_migrate`, which is correct factoring that an adjacency rule
would flag. The cost is a blind spot — a file that opens two connections and
hardens one passes — so it is a tripwire against forgetting wholesale, not a
proof of coverage.

The two connections that matter most are the ones opened on a file **someone
else wrote** — a GeoPackage the farmer imports, and a backup that could arrive
on a USB stick. Both are read-only, hardened, and integrity-checked before
anything in them is believed. `validate_backup`'s check catches a *damaged*
file; the hardening is what catches a *crafted* one, which would otherwise pass
every check and then become the live database.

Three settings are deliberately left at their defaults, all of them the kind
someone "optimises" later: `synchronous` stays FULL (this is a legal record,
not a cache), `mmap_size` stays 0 (with memory-mapped I/O a stray pointer
anywhere in the process can corrupt the file), and `cell_size_check` stays off.

### The weekly corruption check

`PRAGMA quick_check` runs on the startup worker, at most once every
`INTEGRITY_CHECK_INTERVAL_DAYS` (7). The verdict goes in `settings.json` — not
in the database, which is the point: a file too corrupt to read still has a
readable verdict beside it — and `get_status` reports it. The Status view warns
only when it says `ok: false`; a healthy database says nothing, because
reporting "all fine" weekly trains a farmer to ignore the one time it is not.

Three choices behind that shape, all from measurement
(`src-tauri/tests/quick_check_cost.rs` re-runs it):

- **`quick_check`, not `integrity_check`.** About half the cost (measured
  1.5–2.1x across 29–513 MB, converging on 2.1x) and it catches structural
  page damage. The thorough check still
  happens on every backup, where `VACUUM INTO` reads every page and the copy is
  verified — and since 2026-08-26 whenever the farmer asks, below.
- **Throttled, not per-launch.** Cost is linear at ~1.7 ms/MB: ~10 ms for a
  smallholder, but approaching a second for a cooperative-scale book after a
  decade, and `record_change` grows without bound until sync gives it a
  retention policy.
- **Its own read-only connection.** Holding the shared lock for that long would
  freeze every command at startup; WAL admits a second reader without
  disturbing writers. The cost is a narrow race — closing the window mid-check
  leaves the sidecars behind that once.

### Check and compact, on request

Settings offers one button (`check_and_compact_database`, 2026-08-26). It is
**one action rather than two, because the check gates the compaction**:
`VACUUM` rebuilds the file by reading every page and writing a fresh one, so on
a damaged database it would copy the damage forward instead of revealing it. A
failed check therefore stops with the file untouched and tells the farmer to
restore a backup.

The button runs **`integrity_check`**, not the weekly `quick_check`, and the two
are asked for different reasons: the weekly one is a cheap structural screen
nobody requested, this one is a person asking, and a person asking will wait. It
adds index-against-table consistency and the UNIQUE/NOT NULL/CHECK constraints,
at roughly twice the cost (~3.5 ms/MB — 20 ms on a smallholder's book, a couple
of seconds on a cooperative-scale one). Because they differ,
`IntegrityCheck.thorough` records **which** check produced a verdict; without
it, "checked three days ago, fine" would mean two different things. A verdict
written before that field existed loads as the quick check it was.

Two shapes worth keeping. A bad verdict is an **outcome, not an error** — it
returns `Ok` with `integrity.ok == false`, the way a refused catalogue refresh
does, because failing the command would hand the farmer an error instead of the
answer they asked for. And size is reported as `page_count * page_size`, not the
file's length: in WAL mode the pages are still in the sidecar when `VACUUM`
returns, so the file on disk is not yet the honest number.

Unlike the weekly check this runs on the **live** connection, holding the
database lock — it has to, because `VACUUM` needs the writer. The lock is
released before the settings lock is taken to record the verdict: the invariant
everything obeys is that no thread ever holds both at once, which
`active_actor` reaches from the other direction.

## Shutdown

SQLite in WAL mode keeps a `-wal` and a `-shm` file beside the database and
**deletes them when the last connection closes cleanly**. Nothing else does:
`wal_checkpoint(TRUNCATE)` empties the write-ahead log but leaves both files on
disk.

Nothing closed them, and the reason is structural: **Tauri never drops managed
state**, because the platform event loop ends the process with
`std::process::exit` (tao's GTK loop does exactly this after dispatching
`LoopDestroyed`). So `Connection::drop` never runs, and a WAL grew without
bound — measured at 6.3 MB beside a 6.0 MB database.

The fix is a `RunEvent::Exit` handler:

```rust
app.run(|handle, event| {
    if let tauri::RunEvent::Exit = event {
        close_databases(handle);
    }
});
```

which is why `run()` ends in `.build(ctx)` + `app.run(callback)` rather than the
one-liner `.run(ctx)`. `RunEvent::Exit` is delivered before `process::exit`, and
`close_databases` takes each connection out of its `Database` and calls
`Connection::close`.

Three things about that hook are deliberate:

- **Explicit `close()`, not `Drop`.** rusqlite's `Drop` discards the close
  error; `Connection::close` flushes the prepared-statement cache and reports
  what failed.
- **`try_state`, not `state`.** `initialise` runs on a worker thread, so a
  window closed during startup can arrive here before either database has been
  managed. That exit leaves the sidecars behind, and it is the one case that
  still does.
- **Not gated to desktop.** Android never reaches `RunEvent::Exit` — the system
  kills the process — which is exactly what WAL is for. Gating would add a `cfg`
  for a branch that already never runs.

A close blocks on the lock, so a command still running (a report render, say)
finishes first. That is the right order: closing under a live command is worse
than waiting for it.

### On Android the webview starts first, and the first reply waits for a second message

Desktop creates its window after `setup()` returns, so nothing can call a
command too early. **Android does not**: the activity builds the webview while
`setup()` is still running, so the frontend is live and invoking before managed
state exists. `src/main.js` therefore gates `mount()` behind a poll of
`app_ready`, a command that deliberately takes `AppHandle` rather than `State`
so it is callable before anything is managed, and reports readiness by probing
the `SetupComplete` marker that `setup()` manages **last**.

The gate polls rather than awaits once, and that is load-bearing for a reason
measured on-device (Galaxy A22, Android 13, WebView 151, fresh data dir):

```
+0.000s  rust: app_ready executed #1 -> false
+0.670s  rust: setup hook COMPLETE
+6.069s  js:   probe #1 finally resolves false   <- 10ms after probe #2 was posted
```

The command runs immediately and its answer is ready at once; setup finishes
well before the answer is delivered. Widening the probe timeout from 2s to 6s
moved that delivery from +2.07s to +6.07s in step with it, always ~10 ms after
the *next* probe was posted. **A pending reply is delivered when the next IPC
message is posted** — not when the command returns, not when the main thread
frees.

So a single awaited `invoke` at startup deadlocks against itself: its answer is
waiting for a next message, and the caller only sends one after receiving the
answer. That is what produced a permanently blank first launch on a fresh
install, recoverable only by relaunching.

**The fix is the startup order, not the retry.** `setup()` now returns in
microseconds and does its real work — open and migrate the database, import the
catalogues, refresh alerts, load settings, open the geo cache — on a worker in
`initialise()`, managing `SetupComplete` last exactly as before. The event loop
is therefore running *before the webview exists*, so nothing can be queued into
the window where replies are parked, and the defect is unreachable rather than
worked around. The 2 s probe timeout stays as a belt: it costs one timer, and on
a launch slow enough to exceed it (the first start after an install, while ART
compiles) the retry still posts the message that delivers the previous answer.

The cost of returning early is that the window now appears before the app can
render, so `src/index.html` carries a wordless spinner that `main.js` removes
just before `mount()`. Wordless because it renders before i18n has resolved a
locale.

**The same hazard has a second instance, so treat it as a class rather than one
bug.** `tauri::webview_version()` is implemented on Android by spinning
`loop { first_activity_id(); sleep(100 ms) }` and then blocking on the main pipe
(wry's `android/mod.rs`). Anything shaped like that — a call that waits on the
activity or the webview — must stay off the startup path. That is why the
version strings the About panel prints live in their own `get_about_info`
command rather than joining `get_status`: it can only be reached by tapping a
button inside a webview, which is proof the thing being waited for already
exists. **The general rule: before calling a platform API during startup, check
whether its Android implementation waits for the activity.**

The consequence for anything else that runs early: **do not await a lone invoke
before the gate has completed.** Everything downstream is safe by construction —
`loadLookups()` and every view's mount-time fetch run after `await
waitForBackend()`, by which point at least two messages have been exchanged and
replies flow in ~10 ms — but code that moves ahead of the gate loses that
guarantee, and a parked promise raises nothing for `run()` to report.

**Where the defect actually lives — measured, after two wrong guesses.** It is
in **tao**, not in wry and not in this app. A command result returned from a
non-main thread is dispatched through tao's `EventLoopProxy`
(`tauri-runtime-wry`, `send_user_message`), and on Android **a user event sent
before `event_loop.run()` is queued but never wakes the loop**; it arrives only
when some later event flushes the whole backlog. Reproduced standalone in ~90
lines of pure tao — no wry, no webview, no Tauri — on both tao 0.35.3 and
0.36.0, while the identical pattern delivers on desktop. It needs concurrent
ndk_glue traffic to bite, which an activity starting up always produces; with a
quiet pipe the same event is delivered in 1 ms.

Two earlier explanations were wrong and are recorded so they are not re-derived.
It is **not** wry's `MainPipe`: probes sent down it (`JniHandle::exec`) are
delivered in 0–1 ms in every configuration, including with the Android UI thread
deliberately blocked. And it is **not** a blocked main thread as such: blocking
either thread *while the loop is already running* delays delivery correctly and
no more. Both claims were asserted before being measured, and both died on the
first measurement.

**Tried and rejected, so it is not retried:** making `app_ready` an `async fn`.
Tauri dispatches sync commands to the main thread and async ones to the runtime
pool, so "the main thread is contended during Android startup" was the obvious
suspect. Measured with the retry pump disabled, it changes nothing — Rust logs
the command executing while the frontend's promise is still pending 15 s later.
The parking is in reply **delivery**, not in command dispatch.

Also rejected as the explanation: wry issue #1551, whose `rx.recv_timeout`
TODO looks like exactly this symptom. It sits on the custom-protocol path,
and the `MainPipe` probes above exonerate it.

One anomaly met on the way is not a bug at all, recorded so it is not chased
again: `Event::Resumed` never arrives on a first launch, because tao drops the
first resume deliberately ("to match the iOS implementation"). Background the
app and return and the second one lands in 1 ms.

The upstream fix is [tao#1304](https://github.com/tauri-apps/tao/pull/1304),
open since 2026-08-08, which names the mechanism these measurements could only
bound from outside: `ALooper_pollAll` returns an fd event instead of
`ALOOPER_POLL_WAKE` when both are ready in the same epoll batch, so
`EventSource::User` never runs and the receiver is never drained. That is the
measured co-factor — pending ndk_glue traffic — reached from the other end. Its
patch was applied locally and verified on device: the event arrives at +3000 ms
instead of being lost until +5003. The app-side startup ordering above is
correct whichever way the PR goes.

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
release-ritual step, and users can also fetch the provider's current copy
from Settings (docs/maintenance.md §1), through the same parser and the same
upsert.

The importer is idempotent — it runs the vendored snapshot on the first
launch of each app version and skips on the ones after, so the steady-state
startup cost is a handful of probes and no parsing — and **upsert-only**: the
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

The export itself lives in the `terrazgo-siex` crate (added 2026-07-16 as
`module_cue::export`, extracted 2026-08-20; mapping design in
docs/siex-export.md): `export_precheck` lists what blocks a valid export and
`build_cuaderno` turns one farm+season into the official CUE descriptor JSON,
refusing while the precheck is not clean so nothing is silently dropped.

**It emits thirteen of the format's fifteen activity blocks** since the
"finish the serializer" arc closed on 2026-08-22 — every block with a register
behind it, `Cosecha` and `EnergiaUtilizada` having none by decision. One block
builder per file under `src/blocks/`, each reading the module that owns its
register.

Three rules run through all of them. Multi-crop treatments split into one
`TratamFito` per crop snapshot (3.11.4 descriptor rule), each split keyed by
its own frozen alias; a core `crop` row is the SIEX plot+crop+season unit
(DGC) and is referenced by a client-assigned integer (`CodigoDGCAjena`,
aliases again), which the eco-scheme blocks resolve from the plot and refuse
to guess at when a plot carries several crops. Soft-deleted records emit
`Borrar` entries under their existing aliases; never-exported deletions leave
no trace.

**The precheck is where the export's ethics live.** It refuses rather than
dropping: every rule exists because some record could otherwise have gone out
with a value silently missing, or not gone out at all with nothing on screen
saying so. Several of its rules come from FEGA's Anexo V `OBLIGATORIEDAD`
grading rather than from the JSON Schema's `required`, which is a weaker
statement — docs/siex-export.md → "The law outranks the format" carries that
precedence, and it is worth reading before adding or relaxing a rule here.

The serializer's output is schema-validated in tests against the vendored
official JSON Schema (the `jsonschema` crate, dev-dependency only — never in
the shipped binary). The shell exposes both entry points as commands
(`export_cuaderno_precheck`, async `export_cuaderno` writing the JSON to a
dialog-chosen path) — but **nothing in the interface calls them** since
2026-08-11: the export has no delivery path, so a button producing a file with
nowhere to go was the wrong thing to show a farmer. Un-parking it means
rebuilding the panel *and* its scripted checks.

**Backup** (in `terrazgo_core::backup`): export is a `VACUUM INTO` snapshot
— consistent and compact while the app runs, no WAL sidecars — which is then
re-opened and integrity-checked before success is reported; an unverified
backup of regulatory records is worse than none. Import validates, exports
an automatic safety copy of the live db first, then swaps and re-migrates.
Older backups are fine (migrated forward on open); newer-than-app backups
are refused. A backup at the *current* version is also shape-checked, because
while the project is pre-release the migration files are edited in place and
`user_version` therefore cannot tell a file taken before an edit from one
taken after — left unchecked it would import cleanly and fail later with
`no such column`. **That fingerprint composes the way the migration sequence
does**: core owns its own tables, each module contributes its own through
`validate_backup`'s `module_shape` argument, and the shell passes them
together — core may never name a module's table, not even in a constant.
Details: [backup-restore.md](backup-restore.md).

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

### Spreadsheets: the second renderer

`render_xlsx` (2026-08-02) is the same shape of seam as `render_pdf`: callers
describe *what* a document contains — `Workbook` → `Sheet` → `Cell` — and the
engine owns *how* it looks (bold frozen header, autofilter, column widths,
`dd/mm/yyyy` dates). No module touches `rust_xlsxwriter`, so the look stays
consistent across the reports modules will add, and the crate could be swapped
without touching a caller.

`rust_xlsxwriter` was picked (2026-08-02, `default-features = false`) as
write-only, which is all a report engine needs: pure Rust, MIT/Apache-2.0, and
13 transitive crates that are all permissive and C-free (zlib-rs rather than
zlib), so Android cross-compilation is untouched. Rejected: `umya-spreadsheet`,
which reads as well as writes and is correspondingly heavier; hand-rolled
OOXML; and CSV, which cannot carry the book's ~18 sections, has no typed cells,
and hands Excel decimal commas it will mangle.

The design rule that matters is **typed cells**. The Typst template consumes
pre-formatted Spanish strings because it only does layout; a spreadsheet must
not, because sorting by date, filtering a product and summing hectares all
need real values. So a document's assembly produces a typed intermediate and
each renderer formats from it — one read of the database, two presentations,
and a new field is added in one place. Numbers deliberately carry no number
format: Excel renders them in the reader's own locale, which is how a Spanish
user gets decimal commas without the app hard-coding them.

Empty stays empty: an unknown value is a blank cell, never a zero (a
spreadsheet would add zeros up, and official forms leave blanks for
hand-filling). Excel's tab-name rules — forbidden characters, a 31-character
cap, no duplicates — are repaired by the engine rather than pushed onto
callers, because a report must never fail over a tab name.

## The report engine: Typst in-process

Printable documents (the official cuaderno first; fertilisation plans, cost
reports and analytics dossiers later) are rendered by `crates/terrazgo-report`
(added 2026-07-16): **Typst as a library**, not a webview `print()` (never
wired on Linux/wry or Android) and not a low-level PDF writer (no layout
engine — an unbounded treatments table needs per-cell wrapping, cross-page
row breaking and repeating headers).

The whole pipeline is offline by construction:

- **Templates** are Typst source owned by the consuming module, embedded via
  `include_str!`. A template holds layout, not prose: its labels are
  per-country document content — never UI i18n keys — but they arrive as
  ordinary inputs, so one template serves every language the document may be
  printed in (see "The book's language" below).
- **Fonts**: the four Liberation Sans faces (~1.6 MB, OFL-1.1 — licence
  vendored alongside in `crates/terrazgo-report/fonts/`) are embedded with
  `include_bytes!`. Liberation Sans is metric-compatible with Arial, the
  look of the Spanish administrative forms. No system-font scanning: output
  is identical on every platform. Vendoring them by hand was chosen over
  typst's own `typst-kit-embed-fonts` feature, which costs ~10 MB for a
  bookish Libertinus look — and, more importantly, ships a default-font
  fallback that would quietly satisfy the zero-warnings tripwire below
  instead of exposing broken font wiring. Never add `ttf-parser` as a direct
  dependency to inspect these faces: it trips cargo-deny's unmaintained
  policy (RUSTSEC-2026-0192), and typst's own `Font` parser reads them.
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
more rule worth copying: `terrazgo_recordbook::cuaderno_inputs` pre-formats
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

**Catalogue labels resolve once per book, never once per row** (2026-08-13).
Assembly used to read every coded cell with its own point query — each one a
single indexed lookup, harmless on a smallholder's book, and exactly the shape
the scale rule warns about: *"keep report assembly to a bounded number of
queries rather than one per row"*. All of it now goes through one private
`CatalogueCache`, created per book and threaded beside the connection, so the
reads a book makes are bounded by the **distinct codes it prints** rather than
by how many rows print them.

**The design choice is memoise per code, NOT preload per catalogue.** Preloading
is the obvious fix while the vocabularies are small (`EST_FENOLOGICO` has ten
rows, `TIPO_MEDIDA_FITOSANITARIA` fourteen), but the same funnel resolves
`MUNICIPIO_SIGPAC` (8 434 rows) and `DETALLE_MATERIAL_FERT` (1 243) while a book
names a handful of towns and materials — reading a whole catalogue to resolve
three of its rows costs more than the queries it saves. Memoising is bounded at
either size, so one mechanism serves every catalogue and no call site has to
choose. A code that resolves to nothing is remembered as such, or a book written
against a catalogue this installation never imported would re-ask for every row.

Counting the sites corrected what this section previously recorded, and the
corrections are the useful part. There were **nine**, not three — the shape had
spread through `catalogue_label` and through three functions that called
`find_code` themselves. The per-row lookup on a plot row is the **término
municipal**, not the province: the holding's province is read once for the 1.1
header, while `MUNICIPIO_SIGPAC` is asked per plot. And "nothing else in
assembly has this shape" was wrong twice over — the fertilisation register
resolved its machinery name with a query per row, and `list_plots` ran a third
time to build a plot-name map `plot_rows` already had the data for (the order
numbers and the names now travel together as one `PlotIndex`).

The invariant is pinned by assembling the demo book twice, the second time with
four times the treatments, and asserting the asks multiply while the reads stand
still.

### The book's language

The record book's **layout is per country** (the Spanish official model, which
never forks) while its **language is per region**: where a co-official language
exists, a farmer must be able to hand an inspector the same book in either one.
So the document owns a `Labels` struct per language
(`terrazgo_recordbook::labels`), serialized into the template's `sys.inputs` and
read by the spreadsheet renderer as well.

Three properties are worth keeping when a second document needs this:

- **A Rust struct, not a dictionary file.** A missing translation becomes a
  compile error, which is stronger than the key-parity contract test the
  frontend dictionaries need — and serde produces the template's dictionary
  for free.
- **Prose translates, codes do not.** The model's own siglas (SEC/ASP/LOC/GRA,
  AE/PI/CP/…), dose-unit symbols and the FEGA catalogue labels resolved for
  "problema fitosanitario" are payload printed verbatim in every language; the
  footnote that explains a sigla is what translates. The assembly therefore
  holds no prose at all — even the 2.2 zone summary is stored as values and
  worded at render time.
- **Which languages are offered is derived, not configured.**
  `terrazgo_recordbook::region` maps INE province codes (the farm's registry
  province plus each plot's SIGPAC reference) to the languages co-official
  there, intersected with the ones that have a dictionary — so a co-official
  language with no dictionary yet simply does not appear, and adding one is a
  single `Labels` const. A holding with no province recorded is offered every
  language rather than none: an unfilled form field is not a statement about
  what the farmer may print.

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

## The map tier

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
that too. The default was reviewed on 2026-08-26 against device storage —
the constraint it had been held open for — and kept at 512 MiB: it is a
ceiling and not a reservation, so a phone holds only the tiles it browsed,
and the cap is a user setting on every platform.

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

## Pages the app points at, and never talks to

Spain's agricultural registries publish no machine interface: ASPAFITOS is a
server-rendered ASP.NET app, REGMAQ-ROMA answers a form POST with HTML, and
ROPO's bulk download stopped updating in 2024. Scraping is not an acceptable
interface here, so the app cannot look a farmer's ROMA number up for them.
What it can do is say which registry holds it and open that registry — which
is what `src-tauri/src/external_links.rs` and `tauri-plugin-opener` are for
(added 2026-08-26).

**This is not a network seam.** Nothing in that path fetches anything; it hands
a URL to the platform browser and forgets it. `terrazgo-net` remains the one
place the app itself speaks HTTP, and neither core nor any module gains a
dependency from this — the offline-first rule is untouched.

**Rust owns the URLs, and that is the design rather than a detail.** The
allowlist is a `const` table of `(id, url)` in `external_links.rs`; the
`open_external_link` command resolves an id or fails with
`Invalid("unknown_link")`. The webview passes `"roma"`, never a URL, so the
opener plugin is registered with **no `opener:allow-open-url` granted** — the
`user_files.rs` precedent, and the reason the ACL files did not change when
this landed.

The split across the boundary is deliberate:

| Where | What it knows | Why there |
|---|---|---|
| `src-tauri/src/external_links.rs` | id → URL | It is a permission decision. One source of truth for a destination, and the webview cannot name one. |
| `src/lib/registryHints.js` | country + field → id | It is presentation: which *field* earns a hint is a UI question, and it is per country so no Spanish registry is hardcoded into a shared form component. |
| `src/i18n/<locale>/external.js` | id → the sentence | Registry proper nouns (ROMA, ROPO, SIGPAC) do not translate; the sentence around them does. |

`src-tauri/tests/registry_hints.rs` keeps the three in step, because the failure
mode is otherwise invisible until a farmer taps a button.

**What is deliberately absent.** REGA, SIEX, MDF, RGSEAA, REGFER and NIMA have
columns in the schema but no link: none resolves to a stable public lookup
page, and a hint that leads nowhere is worse than a bare label.
`farm_es_extension.rea_code` can never gain one — REA is a per-community
registry (REACYL, SIDEAC, …) and [siex-export.md](siex-export.md) → "the
REA-first rule" forbids any user-facing string naming one community's service.

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
- **Feedback** has two surfaces, split by whether a form is involved. A
  form's own problems — its empty required fields and the backend's refusal to
  save it — are drawn by `TzForm`: a summary at the top of that form listing
  every one, plus each field's own inline message. Everything else flows
  through the notification bell, where `run()` turns boundary errors into red
  notifications (panel auto-opens) and successes tick the badge. A success is
  only pushed when it says something the screen does not already show.
- **i18n**: every user-facing string is a key present in *every* locale
  dictionary (a contract test enforces it); schema codes are translated at
  display time via `tCode`; user-entered data is never translated.
- **The form controls are owned, not the platform's** (2026-08-14): dates,
  time and every dropdown are the `.tz-*` component family on Bits UI
  (headless primitives — behaviour and ARIA, no styling). The reason is
  correctness, not looks: the native date picker follows the **OS** locale and
  would override the language the holding chose, on a field that appears in
  every register of the book. Each takes and returns a plain string, so the
  migration moved no view logic. Rules, the 40-row cap and the four Bits UI
  components that must never be used are in
  [frontend-conventions.md](frontend-conventions.md) → "Owned controls".
- **No `@tauri-apps/api` dependency** — `withGlobalTauri` exposes
  `window.__TAURI__`, and plugin calls ride the same transport
  (`invoke("plugin:dialog|save")`).
- **Build shape.** Vite's root is `src/` (`index.html`, `main.js`,
  `App.svelte`) with `vite.config.js` and `package.json` at the repository
  root; the output is `dist/`, which is gitignored and is what
  `tauri.conf.json`'s `frontendDist` points at. **`npm run build` must run
  before the first `cargo check` or `cargo test`**, because tauri's codegen
  embeds `dist/` at compile time — on a fresh clone the Rust build fails
  without it, and the error does not say so.

### What the webview is allowed to call

`src-tauri/capabilities/` holds the Tauri 2 ACL, and it is deliberately
small. App-defined commands registered through `generate_handler!` are **not**
ACL-gated at all, so the files only cover what the injected global API bundle
and the plugins need:

- `default.json` — `core:default` for the events/window plumbing, plus the
  three dialog permissions (`save`, `open`, `message`) behind backup
  export/import and the destructive-action confirmations.
- `mobile.json` — the geolocation permissions for the map's GPS lookup,
  behind a `platforms` gate so desktop builds never see a plugin that does
  not exist there.

**No filesystem permission and no opener permission is granted to the webview,
and that is the point.** Two plugins are registered with nothing in the ACL at
all, because both are driven entirely from Rust:

- `tauri-plugin-fs` — every dialog-chosen read and write goes through
  `src-tauri/src/user_files.rs` (see "Files the user picks" above);
- `tauri-plugin-opener` — every outbound link goes through
  `src-tauri/src/external_links.rs` (see "Pages the app points at" below).

The shape is the same in both cases and is what makes the claim real rather
than nominal: **the frontend names a thing, Rust decides what that means.** A
webview granted `opener:allow-open-url` could open any URL it was talked into
building; one that can only pass `"roma"` cannot. So the webview's permission
surface stays at zero.

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

### How the suite is organised, audited 2026-08-17, rebuilt 2026-08-24

**It is the cargo convention with no bespoke harness** — 60 integration files
across the crates' `tests/` directories, 26 inline `#[cfg(test)]` modules, 1 084
tests including doc-tests. Five properties are worth naming so they are not
traded away by a later tidy-up:

- **Tests run against the real migrations.** Each crate's `open_in_memory()`
  applies the actual composed migration set, so a repository test meets the
  shipping schema's CHECK constraints and FKs rather than a fixture schema that
  drifts from it.
- **Assertions are semantic, and there is no snapshot library anywhere.** The
  book's tests assert `rows[0]["species"] == "wheat"`, not a rendered blob.
  Snapshot testing carries an "accept the new output" step that blesses
  regressions by reflex, which is the wrong affordance to have anywhere near a
  document with legal value.
- **Test names state behaviour** (`rejects_an_interval_that_ends_before_it_starts`),
  which is what keeps a suite this size readable years later.
- **The source-scanning contract tests** (`i18n_contract`,
  `command_registration`, `spdx_headers`, `neutral_voice`) encode as tests what
  most projects leave to a CI grep, so they run before a push rather than after.
- **`migration_composition.rs` pins both consumer crates' hand-written
  `db::migrations()` to the module registry.** It is the structural guard that
  makes a fourth module safe rather than hopeful: a module registered in the
  shell but forgotten in the record book's or the descriptor's composition
  fails here rather than as "no such table" in a report.

#### Where a shared test helper goes

Three homes, and the question that picks one is *what schema does it need?*

- **Only `terrazgo-core` → `crates/terrazgo-testkit`**, a dev-dependency-only
  crate. It holds `farm_with_plots` (the season, farm, two plots and the
  second farm's plot every `PlotNotOnFarm` test needs), `last_change` (the
  latest `record_change` row as `(operation, before, after)`) and `TempFile` (a
  temp path with an RAII guard that also sweeps the `-wal`/`-shm` sidecars).
  **It depends on core and nothing else, and that is the design.** A testkit
  that grew a `module-cue` dependency would put module-cue's schema inside
  module-fertilisation's test graph, and each module's
  `the_module_runs_on_core_alone` guard would go on passing while it was open.
- **This crate's own schema, and two or more test files use it →
  `tests/common/mod.rs` in that crate.** Seven crates have one. It re-exports
  the testkit so a test file needs one `mod common;` and one `use`, and adds
  what is local: the `db()` and `db_with_catalogues()` openers, and the
  crate's shared fixture. One test file means no `common` — `terrazgo-geo`
  and `terrazgo-report` have none, and `terrazgo-net` has no `tests/` at all.
- **Another crate's schema → it cannot be shared; duplicate it and say why.**
  `terrazgo-recordbook/tests/common` and `terrazgo-siex/tests/common` build
  field-for-field twins of the same export-ready Spanish farm, ~60 lines each,
  and both say so at the top. Closing that would mean the testkit reaching into
  module-cue, which is the back door above; and it is the same trade the crates
  themselves make, since the book and the descriptor read the same registers
  and share no code.

#### The catalogue fixture is explicit

**A test that resolves a catalogue code to a label opens through
`db_with_catalogues()`; every other test opens through `db()`, and the
difference is deliberate.** It used to be an ad-hoc `ensure_catalogues` call at
47 sites, so within one file some tests resolved codes to prose and some
printed the bare code, and which kind you wrote depended on which test you
copied. Importing the vendored snapshot parses 1.6 MB of CSV per call, but the
cost is the lesser reason — the statement is.

The exception is core's tests OF `ensure_catalogues` itself (idempotency,
upsert-never-delete), which still call it by hand: there the import is the
subject rather than the setup.

#### Doc-tests: where the example IS the specification

- **Pure functions get a worked example each** — core's `date` and `geojson`,
  module-fertilisation's `agronomy`, module-cue's `alerts`. There the example
  is the specification, and it costs nothing to run.
- **Each crate root gets one end-to-end example.** core's opens, writes and
  reads back; terrazgo-report's renders a real PDF and asserts the warnings
  list is empty.
- **Repository functions get none.** A doc example per repository function
  would spin a database each, for documentation whose real specification is the
  test beside it.

#### The six deviations, and where each landed

1. **No shared test support — CLOSED.** `terrazgo-testkit` plus a
   `tests/common/mod.rs` in seven crates, per the homes above. `fixture` was
   defined 15 times and `last_change` 8; the plumbing now grows per crate
   rather than per test file.
2. **The migration upgrade-path test cannot yet prove what its name promises —
   PARKED, correctly.** `applies_cleanly_on_top_of_previous_version` steps to
   version 1 and then to latest, which while the migrations are squashed into
   `0001`/`0002` only exercises "DDL, then seeds". It is the right test to have
   standing and it becomes load-bearing at the first append-only migration;
   until then **its green tick is not an upgrade guarantee.**
3. **Zero doc-tests — CLOSED.** Eleven, under the rule above.
4. **`open_in_memory()` is public API on the shipping crates — DECLINED, with
   reasons.** A `#[cfg(feature = "testing")]` gate collides head-on with
   deviation 3: a doc example needs `open_in_memory()` callable from a
   doc-test, and gating it makes every such example either `ignore`d — a
   doc-test that never runs — or forced to hand-roll `Connection::open_in_memory()`
   plus `migrations().to_latest()` inline. Supporting: no `publish` key exists
   anywhere in the workspace and nothing consumes these crates outside it, so
   the surface is workspace-internal; shipping code that wants a scratch
   connection already calls rusqlite's own; and the shell enables module-cue's
   `demo` feature *unconditionally* because cargo features cannot be
   debug-profile-conditional, so a `testing` feature would end up always on —
   which is no gate at all.
5. **`std::env::temp_dir()` and `tempfile` — DECLINED as a dependency, FIXED as
   a defect.** The real problem was never the std temp dir: it was three files
   with three cleanup disciplines, two of them removing their files with
   explicit calls that a failing assertion skips. `terrazgo-geo`'s import tests
   had already settled this once with an RAII `Drop` guard under a comment
   reading *"no tempfile dev-dependency"*. That guard is now the testkit's
   `TempFile`, used by all three — geo included, so the crate that wrote it
   consumes it back rather than keeping a private copy.
6. **Four oversized files — CLOSED.** `report.rs` (5 817), core's
   `repository.rs` (4 137), module-cue's `repository.rs` (3 637) and
   `export.rs` (3 591) were 53 % of the integration suite. They split along
   their existing banner sections into 22 files, the largest 1 226 lines:
   `report_*.rs` by section of the book, `repository_*.rs` by entity (matching
   `src/repository/`'s layout), and `export_*.rs` **by SIEX block rather than by
   arc seam** — finding how `Pastoreo` is serialized no longer requires knowing
   it landed in seam 4.

The pass moved blocks verbatim: no assertion changed and no test was renamed,
so the count was identical before and after, per crate as well as in total. The
only deletions were two tests in `module-cue/tests/migrations.rs` that
re-asserted core's schema (`farm_without_country_is_rejected_by_the_schema`,
`foreign_keys_are_enforced`) word for word from
`terrazgo-core/tests/migrations.rs`.

## Releases

Releases live at
[github.com/clozanoruiz/terrazgo](https://github.com/clozanoruiz/terrazgo),
together with the issue tracker: per-platform installers (Linux AppImage/deb/rpm,
Windows NSIS + portable `.zip`, Android APK) plus the **complete source of that
version** —
one snapshot commit per release, so the AGPL source-offer travels with every
distributed binary. The installers are built by that repository's own
`build.yml` workflow from the tagged source itself, so every binary comes from
exactly the source published next to it. Each artifact carries signed SLSA
build provenance and every release ships a CycloneDX SBOM attested against
the installers — verify any download with
`gh attestation verify <file> --repo clozanoruiz/terrazgo`. Release notes are
written by hand before a draft release is published.

The snapshot is produced by `packaging/`, which holds the export script and
the public-facing Spanish README and issue templates it drops in. The script
strips the development-only trees and then **fails the release** if a
case-insensitive grep finds any surviving reference to them, so a stray
mention in a crate source or a doc stops the publish rather than shipping.
`packaging/` is itself stripped from the snapshot. Procedure and the full
release ritual: [maintenance.md](maintenance.md) §6.

## Recipes — where to start when you want to…

- **Add a command end-to-end** → checklist in
  [frontend-conventions.md](frontend-conventions.md#adding-a-command-end-to-end-checklist)
  (repository + test → thin wrapper → `generate_handler!` → i18n keys →
  `refresh_alerts` if inputs changed → `run()` + `notify()`).
- **Add a view** → one `NAV_ITEMS` entry in `src/lib/nav.js` + one entry in
  `src/lib/routes.js` (the router table is data, not a branch chain); keys in
  the area file of every locale.
- **Add a module** → new crate under `crates/` depending on `terrazgo-core`;
  implement `Module` (`name`, `migrations`, `backup_shape`); register it at the
  END of `registered_modules()`; add `src-tauri/src/commands/<module>.rs` and
  list its commands in `lib.rs`'s `generate_handler!`; add a `Classify` impl for
  its error type and one line to the shell's downcast chain; add one area file
  per locale. The core does not change. For its tests: take
  `terrazgo-testkit` as a dev-dependency and build fixtures on
  `farm_with_plots`; add a `tests/common/mod.rs` once a second test file needs
  to share something (see "Where a shared test helper goes" above); and if the
  crate is a library, add it to the `cargo llvm-cov` line in the same commit or
  it is simply not measured.
  *This used to be about ten hardcoded points; six were removed on 2026-08-13
  (the single 2940-line `commands.rs`, the backup-shape hand-join, the composed
  migration count, the i18n contract's crate list, the router branch chain and
  the three flat dictionaries), and `classify` shrank to one line per module.
  Two remain ON PURPOSE: the registration line — which IS the seam, and whose
  removal would mean linker-section discovery, fragile exactly where mobile
  static linking is — and the `generate_handler!` entry, because the ways out
  are a tt-muncher macro chain or a build script that regex-scans sources, and
  the drift it would prevent is already impossible (`command_registration.rs`
  checks both directions). The reasoning, and why a third-party plugin API is
  the wrong goal instead, is in* [stack-choices.md](stack-choices.md) *§5.*
- **Change the schema** → high-stakes: design first.
  Pre-release, edit the squashed `0001`/`0002` and recreate dev databases;
  post-release, append a migration and write both migration tests.
- **Add a language** → one `SUPPORTED` entry in `src/i18n.js` + one
  dictionary file with the full key set (the contract test enforces
  completeness).
- **Re-theme** → edit the CSS variables in `:root` (`src/styles.css`);
  nothing else references raw colors.
