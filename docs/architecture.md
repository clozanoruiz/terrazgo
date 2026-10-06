# Architecture — how Terrazgo hangs together

For contributors **and** for day-to-day work on the app. It explains the
structure, the rules the code relies on, and how a request actually travels
through the app. It doesn't describe every file: the code and its doc comments do
that.

## The big picture

Terrazgo is an offline-first desktop and mobile app: a Svelte webview talking to
a Rust backend over Tauri's IPC, with SQLite as the single source of truth on
the device. No code in core or in any module makes network calls; that is a
firm rule. The only network path is the `terrazgo-net` crate, used by two
things: `terrazgo-geo`, which wraps it in a cache for the map, and the shell's
catalogue refresh, which only runs when the user asks. With no network, the app
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
│  machine.rs (OS/CPU/memory + packaging, About panel)   │
│  instance_lock.rs (one running copy per data folder)   │
└───────┬──────────────────┬──────────────────┬──────────┘
        │                  │                  │
      (the shell also uses the two READ MODELS above the modules:
       terrazgo-recordbook — the printed book — and terrazgo-siex — the
       exchange descriptor. Siblings; neither depends on the other.)

┌───────▼──────────┐ ┌─────▼────────────┐ ┌───▼──────────────────┐
│ crates/module-   │ │ crates/          │ │ crates/terrazgo-geo  │
│ phytosanitary    │▶│ terrazgo-core    │◀│ tile/resource cache, │
│ treatment domain │ │ farm registry +  │ │ base-map sources,    │
│ product,         │ │ geo_feature,     │ │ style rewriting,     │
│ treatment, its   │ │ audit, backup,   │ │ boundary-file import │
│ alert rules +    │ │ date, geojson,   │ │ cache-through fetch  │
│ its lookups      │ │ alert machinery  │ │                      │
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

Dependencies go one way, and the crate graph enforces it: the compiler, not
good intentions, stops core from importing a module.

- `terrazgo-core` depends on **no module and never on the shell**. It owns the
  farm registry (land, calendar, people, machines, and the premises a treatment
  can be applied to), geometry storage (`geo_feature`), the imported reference
  catalogues (`catalogue` + the vendored SIEX snapshot), the logging helpers,
  date utilities, the GeoJSON validator, backup, the device-local settings file,
  the sync merge layer, and the alert machinery every crate that raises alerts
  feeds.
- Modules depend on `terrazgo-core`. The phytosanitary module owns the treatment
  domain: products, authorisations, treatment records, and the rules behind the
  alerts it raises.
- `terrazgo-net` is the **network path**: the shared HTTP agent, its TLS trust
  policy, the Android setup that policy needs, and the offline diagnosis. It
  depends on no other workspace crate and knows nothing about caching, tiles or
  catalogues: it answers "fetch these bytes, or say why not". **Core and the
  modules must never name it**: core having no HTTP crate in its dependency tree
  is how the build enforces the offline-first rule.
- `terrazgo-geo` depends on `terrazgo-core` (for the GeoJSON validator and
  error conventions) and on `terrazgo-net`, and owns the map's cached fetching
  and the boundary-file importers. No user data lives there.
- `terrazgo-report` depends on no other workspace crate. It is pure
  infrastructure: **PDF** generation with Typst (Liberation Sans fonts embedded
  in the binary) and **spreadsheet** generation with `rust_xlsxwriter`, both in
  process. Templates and workbook descriptions belong to whoever owns the
  document; both are rendered here, so document technology never leaks into a
  domain crate (see "The report engine").
- `terrazgo-recordbook` is the **record book itself**: the data assembly, the
  labels per language, the region → language map, the Typst template and both
  renderers. It is a **read model**: it holds no state and writes nothing; it
  reads core and the domain modules and produces one document. That is why it
  can depend on several modules when a module never may: it is a top-level
  *consumer*, like the shell, and nothing depends on it but the shell, so it
  can't create cycles. A document that reads several domains can't live inside
  one of them. It has its own `RecordbookError`, so it doesn't report failures
  as if it were one of the domains it reads.
- `terrazgo-siex` is the **SIEX descriptor export**, the record book's
  **sibling**: the second top-level consumer, on the same terms. It reads core
  and every domain module and produces one exchange document (the official CUE
  descriptor of FEGA Anexo VI), with fixed integer aliases, a precheck and JSON
  Schema validation. Neither it nor the record book depends on the other: a
  Spanish form for a human inspector and a machine exchange format are different
  documents and fail in different ways. It owns `SiexError`. **Dormant**: no UI
  calls it (docs/siex-export.md), but it is kept compiling and tested so the
  descriptor can't fall behind the registers.
- `module-fertilisation` is the record book's **second decree**: fertilisation,
  irrigation and soil records under RD 1051/2022. It depends on `terrazgo-core`
  and on no other module; where it needs something module-phytosanitary also
  needs (units of measure), that thing lives in core. The *record* of irrigation
  is here rather than in a future Irrigation module because art. 5.e puts
  irrigation doses and dates in the same cuaderno duty as fertilisation; the
  Irrigation module would handle planning.
- `module-ecoscheme` is the record book's **third decree**: the eco-scheme
  entries of RD 1048/2022, which reach the cuaderno through RD 1054/2022 anexo II
  item 4 and print as the model's section 9. Like the other modules it depends on
  `terrazgo-core` alone. Its registers follow the DECREE rather than the printed
  form (three tables for five model pages), because the form hides duties the
  decree creates (docs/cuaderno-print.md → "The eco-scheme registers").
- `module-sigpac` is the Spanish parcel provider (see "The map tier").
- The shell depends on all of these and owns everything Tauri-specific: command
  wrappers, the migration runner, the `geo://` protocol, managed state, the
  window.
- In the frontend, `i18n.js`, the locale dictionaries, `lib/backend.js` and
  `lib/nav.js` are plain JS with **no Svelte imports**, so changing framework
  would only mean rewriting the views
  ([frontend-conventions.md](frontend-conventions.md)).

The way to think about the split: **the core is the farm registry; a module is a
regulatory or functional domain built on top of it.** The phytosanitary module
gives the core entities their Spanish phytosanitary meaning; a future irrigation
module would give plots an irrigation meaning without the core changing.

**Where a new thing goes:**

- shared **data** → the core (harvest, water points, advisors)
- a **domain** with its own logic → a module (treatments, fertilisation, SIGPAC)
- shared **presentation** → a consumer crate above the modules (the record book)

A document that spans domains doesn't justify moving those domains into the
core. The other option, a `Module` trait method letting each module add its own
section, was rejected: the record book is a **fixed legal form** whose sections
refer to each other (3.1 prints the plot order numbers worked out in 2.1), so no
module could produce its section without knowing the whole book, and the single
template that serves every language would be split across crates. If a module
ever needs to contribute without the book knowing it, that method can be added
**on top of** the consumer crate rather than instead of it.

## Life of a command

The most useful thing to understand. Take "the user saves a treatment" and
follow it down and back up. Everything else in the codebase is a variation of
this path.

**1. The view collects a payload** (`TreatmentForm.svelte`). Svelte 5 runes
(`$state`, `$derived`) hold the form's state; on submit the component builds a
plain JS object, `NewTreatmentRecord`, with `snake_case` fields, because serde on
the Rust side reads struct payloads by field name.

**2. `TzForm` calls the save.** A form's own save goes through its `TzForm`
handler, which only runs when the form is valid and shows a refusal inside the
form. Every other call a view makes goes through `run()`
(`lib/notifications.svelte.js`):

```js
run(async () => {
  const saved = await invoke("create_treatment_record", { record, plots: treatedPlots });
  notify(t("message.treatment_saved", { date: formatDate(saved.phi_end_date) }));
});
```

`run()` is the app's error funnel for everything that isn't a form's own save:
any rejection becomes a red notification (the bell panel opens by itself), shown
through `errorText`. Views never `try/catch` command calls one by one.

**3. Tauri IPC.** `invoke` turns the arguments into JSON, crosses the webview
boundary, and Tauri routes the name to the Rust function registered in
`lib.rs`'s `generate_handler!` list. Plain arguments are camelCase on the JS
side (`farmId`), but struct payloads keep their snake_case field names, because
serde reads those, not Tauri.

**4. The command wrapper** (`src-tauri/src/commands/phytosanitary.rs`) is thin
on purpose: lock, delegate, `?`:

```rust
#[tauri::command]
pub fn create_treatment_record(
    state: State<'_, AppState>,
    record: NewTreatmentRecord,
    plots: Vec<NewTreatmentPlot>,
    settings_state: State<'_, state::SettingsState>,
) -> CmdResult<TreatmentRecord> {
    let actor = active_actor(&settings_state)?;
    let mut db = state.db.lock()?;
    let conn = db.conn_mut()?;
    Ok(repository::insert_treatment_record(conn, record, plots, actor.as_deref())?)
}
```

`State<AppState>` is Tauri's dependency injection: the struct put into managed
state at startup is handed to any command that asks for it. The command reads
the active profile (the author every write records), locks the connection,
calls the repository function, and returns. Nothing else is needed for alerts:
the list is worked out when it is read (see "Alerts").

**5. The repository does the real work**
(`crates/module-phytosanitary/src/repository/treatment.rs`), inside **one SQLite
transaction**:

- takes the country from the farm (`farm.country_code` is the source of truth;
  a country passed in that disagrees is a `CountryMismatch`),
- checks that every treated plot belongs to that farm (`PlotNotOnFarm`) and that
  the product is authorised in that country (`AuthorisationMissing`),
- works out the PHI end date with `jiff` (date maths with legal weight is never
  written by hand) and stores `phi_days_used` next to it,
- generates UUIDv7 ids in Rust (`Uuid::now_v7()`, never in SQL),
- fills the `*_snapshot` columns (product name, MAPA number, operator licence,
  crop…); see idea 3 in "The data model in five ideas",
- adds complete row images to `record_change`, each row stamped with the write's
  change set and the treatment's register; see idea 2.

Either all of it is committed or none of it is.

**6. The result travels back.** `Ok(TreatmentRecord)` becomes JSON and resolves
the JS promise; the view shows a success message and reloads its list. An `Err`
is where the error boundary does its job:

- Repositories return typed errors (`CoreError`, `PhytosanitaryError`,
  `GeoError`, made with `thiserror`); the record book returns `RecordbookError`.
- The command's `?` turns them into `CommandError(anyhow::Error)` through a
  blanket `From` impl.
- `CommandError` is serialized as `{ code, params, message }`: `classify()`
  downcasts the `anyhow::Error` back to the domain error and asks it for its
  code. **The mapping lives in the crate that owns the error** (a
  `terrazgo_core::Classify` impl next to each enum), so the exhaustive match is
  next to the variants: adding a variant is a compile error in the crate where it
  was added, not a silent fall-through to `internal` in a shell file the module
  author never opens. The shell only keeps the chain of downcasts, which can't be
  avoided: `anyhow::downcast_ref` needs a concrete type.
- The frontend shows the `error.<code>` i18n key with `params` filled in.
  `internal` (any error that isn't a domain error) has **no** dictionary entry on
  purpose: the localized `error.internal_intro` line is put in front of the raw
  developer message, so nothing is ever hidden.

This is the `thiserror` in crates / `anyhow` at the boundary split: typed and
matchable where callers make decisions, type-erased where everything becomes
JSON anyway.

## Startup, before the window shows

`src-tauri/src/lib.rs::run()`, in order. Any failure stops startup, which is
right for "the database didn't open or migrate":

0. **Desktop: hold the data folder, or leave.** The setup hook locks
   `instance.lock` in the data folder (`instance_lock.rs`); if another copy of
   the app already holds it, this copy exits before its window exists, so a
   second launch opens nothing. For that, the window is declared with
   `"create": false` in `tauri.conf.json` and built by the hook after the lock:
   Tauri builds the windows it creates itself *before* calling the hook, so a
   second copy's window would flash on screen otherwise.

   Two copies on one folder would each keep their own `settings.json` in memory
   and write it back over the other's, and a backup restored in one would copy a
   file under the other's open write-ahead log and leave it writing under a
   retired device id. The system releases the lock when the process ends,
   however it ends, so a crash never leaves the folder held. A filesystem that
   can't lock logs a warning and starts unguarded rather than keeping the farmer
   from their records. Not on Android, which already runs an app as one process.

   It works per **folder**, not per app: a copy with its own `XDG_DATA_HOME` (the
   app-level harness) runs next to yours, while `cargo tauri dev` next to an
   installed copy doesn't: both use `~/.local/share/org.terrazgo.app/`, so the
   dev copy exits at once with a line in the terminal saying why.
1. Find the data folder from the app identifier (`org.terrazgo.app` →
   `~/.local/share/org.terrazgo.app/` on Linux) and open or create
   `terrazgo.db`: WAL mode, `foreign_keys = ON`.
2. Run `composed_migrations()`: core's steps first, then each registered module's
   steps in registration order, with one global `user_version`.
3. `terrazgo_core::catalogue::ensure_catalogues` imports the reference
   catalogues vendored in the binary, on the first launch of each app version
   only (upsert-only; see "Reference catalogues").
4. Put `AppState { db: Database, db_path, schema_version }` into Tauri's managed
   state and register the commands.

(On both platforms the window appears before steps 1–4 finish: see "On Android
the webview starts first".)

`Database` (`terrazgo_core::db`) is what holds both long-lived connections: the
app database and the geo cache. It is a `Mutex<Option<Connection>>` with a pair
of accessors.

The **lock** is there because Tauri runs commands on a thread pool and anything
in managed state must be `Send + Sync`, while a rusqlite `Connection` is `!Sync`:
it must never be used from two threads at once. So the mutex lets one command at
a time use the single connection. For a single-user app that is right; if a long
query ever blocks the UI, the next step is a connection pool (r2d2), not removing
the lock.

The **`Option`** is there so the connection can be closed (see "Shutdown").

A command reaches the connection in two steps, and each step is the only place
one of the two failures can show:

```rust
let db = state.db.lock()?;   // Err(Unavailable::Poisoned) — a panic mid-write
let conn = db.conn()?;       // Err(Unavailable::Closed)   — shutdown, or a
                             //                               failed import
```

**Locking twice panics in debug builds instead of hanging.** Functions that take
a `&Database` lock it themselves (`terrazgo_geo::fetch` does on every call,
because it promises to *release* the lock during network I/O), so a caller
already holding the lock would deadlock. `std::sync::Mutex` isn't reentrant, and
a deadlock doesn't fail a test, it hangs it. So `lock()` keeps a thread-local
list of databases currently held and panics naming the mistake. **All of this is
`#[cfg(debug_assertions)]`**: a release build has no thread-local, no field on
`DbGuard`, no `Drop` and no check.

`conn()` / `conn_mut()` follow the standard library's naming, so a command says
which one it needs. `Deref` would read better (`MutexGuard` uses it), but
`deref(&self) -> &Connection` can't return a `Result`, so a closed database
would have to panic; `MutexGuard` gets away with it because it has no empty
state.

## Hardening, and checking for corruption

Every connection the app opens, ours or imported, goes through
`terrazgo_core::db::harden`, which switches off the SQL features a database file
could otherwise use against us. It costs nothing here, because the schema has
**no views and no triggers** and the app registers **no custom SQL functions,
collations or virtual tables**:

| flag | what it stops |
|---|---|
| `DEFENSIVE` | `writable_schema`, and `PRAGMA journal_mode=OFF` |
| `ENABLE_TRIGGER = false` | a trigger in an imported file running |
| `ENABLE_VIEW = false` | reading a view |
| `trusted_schema = OFF` | trusting expressions stored in a schema |

The tests next to `harden` check each of these. Three things to know:

- **Foreign-key actions still work.** `ON DELETE CASCADE` keeps working with
  triggers off.
- **`DEFENSIVE` protects WAL**: it refuses `journal_mode=OFF`, so nothing running
  in SQL can undo what "Shutdown" relies on.
- **`CREATE VIEW` and `CREATE TRIGGER` still succeed**; only *using* them is
  refused. That is why `src-tauri/tests/contracts/schema_features.rs` checks the
  composed schema has neither: otherwise a migration adding one would apply
  cleanly, ship, and fail when read, far from the cause.

### Three layers, because the flags don't travel

`harden`'s settings belong to the connection, not to the database file. So a
hostile file can't arrive with them already off, but they also don't travel with
the file: **every place that opens a connection has to apply them, or nobody
does**. Nothing in the type system tells a hardened `Connection` from a plain
one, so three mechanisms overlap:

1. **Each opener hardens.** `open_app_db` is public and returns a bare
   `Connection`; it promises to return a hardened one, not one that becomes safe
   if you remember to wrap it. It also means migrations run under the same
   restrictions the app later queries under.
2. **`Database::new` hardens.** The two long-lived connections (the app database
   and the geo cache) can't be held unhardened, because the constructor is the
   only way in. The compiler enforces that.
3. **`src-tauri/tests/contracts/hardened_connections.rs` checks the rest.**
   Three connections are short-lived and never become a `Database` (a backup
   being checked, an imported GeoPackage, the corruption check's reader), and a
   future crate could add a fourth. The test is a **rule, not a list**: any
   shipping file that opens a connection must also harden it, so adding a crate
   or a feature never needs the test edited.

The test works per file, not per call: `terrazgo-geo`'s `try_open` hardens one
call away, inside `apply_pragmas_and_migrate`, which is fine, and a rule about
adjacent lines would flag it. The cost is a blind spot (a file that opens two
connections and hardens one passes), so it catches forgetting altogether, not
every case.

The connections that matter most are those opened on a file **someone else
wrote**: a GeoPackage the farmer imports, and a backup that may arrive on a USB
stick. Both are read-only, hardened, and integrity-checked before anything in
them is trusted. `validate_backup`'s check catches a *damaged* file; hardening is
what catches a *crafted* one, which would otherwise pass every check and become
the live database.

Three settings stay at their defaults, on purpose, and they are the kind someone
might later "optimise": `synchronous` stays FULL (this is a legal record, not a
cache), `mmap_size` stays 0 (with memory-mapped I/O a stray pointer anywhere in
the process can damage the file), and `cell_size_check` stays off.

### The weekly corruption check

`PRAGMA quick_check` runs on the startup worker, at most once every
`INTEGRITY_CHECK_INTERVAL_DAYS` (7). The result goes in `settings.json`, not in
the database, so a file too damaged to read still has a readable result next to
it, and `get_status` reports it. The Status view only warns when it says `ok:
false`; a healthy database says nothing, because saying "all fine" every week
trains a farmer to ignore the one time it isn't.

Three choices behind it (the costs are re-measured by
`src-tauri/tests/contracts/quick_check_cost.rs`; maintenance.md §9):

- **`quick_check`, not `integrity_check`.** A fraction of the cost, and it
  catches damage to the page structure. The full check happens on every backup,
  where `VACUUM INTO` reads every page and the copy is checked, and whenever the
  farmer asks (below).
- **Once a week, not on every launch.** The cost grows with the file: about
  10 ms for a small farm, but near a second for a cooperative-sized book after
  some years.
- **Its own read-only connection.** Holding the shared lock that long would
  freeze every command at startup; WAL allows a second reader without disturbing
  writers. The cost is a small race: closing the window mid-check leaves the
  sidecar files behind that one time.

### Check and compact, on request

Settings has one button (`check_and_compact_database`). It is **one action, not
two, because the check decides whether to compact**: `VACUUM` rebuilds the file
by reading every page and writing a new one, so on a damaged database it would
copy the damage forward instead of revealing it. A failed check stops with the
file untouched and tells the farmer to restore a backup.

The button runs **`integrity_check`**, not the weekly `quick_check`: the weekly
one is a cheap check nobody asked for, while this is a person asking, who will
wait. It adds checking indexes against tables and the UNIQUE/NOT NULL/CHECK
constraints, at a few times the cost. Because they differ,
`IntegrityCheck.thorough` records **which** check gave a result; otherwise
"checked three days ago, fine" would mean two different things.

A bad result is **an outcome, not an error**: it returns `Ok` with `integrity.ok
== false`, as a refused catalogue refresh does, because failing the command would
give the farmer an error instead of the answer they asked for. Size is reported
as `page_count * page_size`, not the file's length: in WAL mode the pages are
still in the sidecar when `VACUUM` returns, so the file on disk isn't yet the
real number.

Unlike the weekly check, this one uses the **live** connection and holds the
database lock, because `VACUUM` needs the writer. The lock is released before the
settings lock is taken to save the result: the rule everything follows is that no
thread ever holds both at once (`active_actor` follows it from the other side).

## Shutdown

SQLite in WAL mode keeps a `-wal` and a `-shm` file next to the database and
**deletes them when the last connection closes cleanly**. Nothing else does:
`wal_checkpoint(TRUNCATE)` empties the write-ahead log but leaves both files on
disk.

**Tauri never drops managed state**, because the platform event loop ends the
process with `std::process::exit` (tao's GTK loop does exactly this after
`LoopDestroyed`). So `Connection::drop` would never run, and the WAL would grow
without limit. The fix is a `RunEvent::Exit` handler:

```rust
app.run(|handle, event| {
    if let tauri::RunEvent::Exit = event {
        close_databases(handle);
    }
});
```

which is why `run()` ends with `.build(ctx)` + `app.run(callback)` instead of the
one-line `.run(ctx)`. `RunEvent::Exit` arrives before `process::exit`, and
`close_databases` takes each connection out of its `Database` and calls
`Connection::close`.

Three things about that hook:

- **Explicit `close()`, not `Drop`.** rusqlite's `Drop` throws away the close
  error; `Connection::close` flushes the prepared-statement cache and reports
  what failed.
- **`try_state`, not `state`.** `initialise` runs on a worker thread, so a window
  closed during startup can arrive here before either database is in managed
  state. That exit leaves the sidecars behind; it is the one case that still
  does.
- **Not limited to desktop.** Android never reaches `RunEvent::Exit` (the system
  kills the process), which is what WAL is for. Limiting it would add a `cfg`
  for a branch that never runs anyway.

A close waits for the lock, so a command still running (a report render, say)
finishes first. That is the right order: closing under a running command is
worse than waiting for it.

### On Android the webview starts first, so the frontend waits for the backend

On both platforms the frontend is live before managed state exists. Desktop
builds its window in the setup hook, straight after holding the data folder,
while the database work goes on in `initialise`. On **Android** the activity
builds the webview while `setup()` is still running, so the frontend is calling
commands before even the hook has finished.

So `src/main.js` holds back `mount()` and polls `app_ready`, a command that takes
`AppHandle` rather than `State` (so it can be called before anything is managed)
and reports readiness by checking for the `SetupComplete` marker that `setup()`
manages **last**. Meanwhile `src/index.html` shows a spinner without words
(i18n hasn't picked a language yet), which `main.js` removes just before
`mount()`.

`setup()` returns almost at once and does the real work (open and migrate the
database, import the catalogues, load settings, open the geo cache) on a worker
in `initialise()`, managing `SetupComplete` last. So the window appears at once
and nothing waits on the database to show it.

**Why the gate polls instead of waiting on one call.** Before tao 0.37 (Tauri
2.12), on Android a reply sent before the event loop was running could be parked
until the *next* IPC message was posted, so a single awaited `invoke` at startup
could wait for itself forever and leave the first launch blank. The cause was in
tao (`ALooper_pollAll` returning an fd event instead of `ALOOPER_POLL_WAKE` when
both were ready, fixed upstream in
[tao#1304](https://github.com/tauri-apps/tao/pull/1304)). Polling with a 2 s
timeout per probe, and starting the event loop before the webview exists, both
stay: the second is how startup should be ordered anyway, and the timeout keeps
an answer that never arrives from holding the gate past its 30 s fail-open
deadline. (Making `app_ready` async doesn't help: the delay was in delivering the
reply, not in running the command.)

**Don't await a lone invoke before the gate has completed.** Everything after it
is safe (`loadLookups()` and every view's fetches run after `await
waitForBackend()`), but code moved ahead of the gate loses that, and a parked
promise gives `run()` nothing to report.

**Keep anything that waits for the Android activity off the startup path.**
`tauri::webview_version()` on Android loops on `first_activity_id()` with 100 ms
sleeps and then blocks on the main pipe (wry's `android/mod.rs`). That is why the
version strings the About panel shows come from their own `get_about_info`
command rather than from `get_status`: it can only be reached by tapping a button
inside a webview, which proves the thing being waited for exists. Before calling
a platform API during startup, check whether its Android implementation waits
for the activity.

`get_about_info` returns everything the technical tab shows: the build stamp
`build.rs` writes, the bundle identifier and the packaging the binary came from
(`machine.rs`), the machine's OS, processor and memory (`sysinfo`, plus
`/proc/self/status` for the peak resident memory, which sysinfo doesn't give),
the monitor the window is on, and both databases' path, schema version and size
on disk. Every field falls back to `None` on its own: a panel that reports the
machine must never be the reason the panel won't open.

**Naming the OS needs a different recipe on each platform**, all in
`machine.rs::os()`. On Android `System::name()` is `ro.product.model`, the
DEVICE, so the usual name + version would print "SM-A226B 13" and name no OS;
`long_os_version()` gives "Android 13 on SM-A226B". Windows uses that call for
the edition and `kernel_version()` for the build, "Windows 11 Pro (26100)",
because neither half has the other and the build is what a bug report is matched
against. Linux keeps name + version, because `long_os_version()` wraps a complete
answer in the kernel's name ("Linux (Linux Mint 22.3)").

One Android quirk that isn't a bug: `Event::Resumed` never arrives on a first
launch, because tao drops the first resume on purpose ("to match the iOS
implementation"). Background the app and return, and the second one arrives.

## The data model in five ideas

Not a list of tables: the five ideas that explain why the schema looks the way it
does. The per-table reference (relationships, kinds of table, which rules each
takes part in) is [data-model.md](data-model.md), which also maps each entity to
its Spanish regulatory term.

**1. User data vs reference data.** User-data rows (farms, plots, treatments…)
get UUIDv7 TEXT primary keys generated in Rust when inserted: safe to sync
between devices, and in insertion order. Lookup tables that ship with the app
(`unit`, `reason_category`, `country`…) use short text codes and are seeded by
migration. The question that separates them is "can two devices create this
independently?", which is why `active_substance` is user data.

**2. The change log is what sync is built on.** `record_change` rows hold
complete before/after row images for every synced table, and another device
replays them, which is why "always complete row images" can't be relaxed. The
conflict and duplicate reviews read the same rows, and a record's history would
too. No law requires the log: the three years of RD 1311/2012 art. 16.3 are for
the entries and their documents (docs/cuaderno-print.md → What the law asks of
the entries over time).

So every write opens a **change stamp** before it logs anything
(`terrazgo_core::audit::begin`): which device wrote it, that device's change-set
number (shared by every row of one transaction), a hybrid clock value, and the
**register** the row belongs to, which is what a merge works on (a treated plot
belongs to its treatment record). The device comes through the connection:
`open_app_db` takes the id from `settings.json` and installs it, so a connection
opened without one can't log at all. The connection also carries the **aggregate
map** (every table's role in the merge, declared by core and by each module), and
each log row is checked against it as it is written, so a row filed under the
wrong register is refused and its write rolled back. The full design is in
[sync.md](sync.md) Part 2.

**3. A record keeps what it stated.** `treatment_record`/`treatment_plot` copy
product name, registration number, substances, operator licence, crop and REGANIP
into `*_snapshot` columns when written, *and* keep the foreign keys. A later edit
to a product row must never silently change a past record. (This isn't about
printing: nothing is ever frozen, and a record can always be corrected;
data-model.md → "Nothing is ever frozen".) The same goes for derived values:
never store one (the PHI end date) without the inputs it came from
(`phi_days_used`).

**4. The crop is on the junction.** A treatment can cover plots growing different
crops, so "the crop at the time of the treatment" is on `treatment_plot` (per
plot), not on the record. Junctions across several plots and countries are where
this data model gets complicated, and they are what the tests cover explicitly.

**5. Synced records are soft-deleted.** `deleted_at` hides a record, and the
deletion travels to every device. Thirty days later, once every device holds the
deletion, the purge erases what nothing else points at (docs/sync.md → "The
purge, as settled"). Regional extension rows (`*_es_extension`) are the exception:
they are hard-deleted when a form clears them (logged with an empty after-image),
because they are attributes of a live row, not records of their own.

## Migrations: one global sequence

Each crate keeps its SQL files (`crates/*/migrations/`, embedded in the binary
with `include_str!`) and exposes `migration_set() -> Vec<M>`. The shell joins
them, **core first, then modules in registration order**, into a single
`rusqlite_migration` runner with one global `user_version`. So:

- Module tables can reference core tables, never the reverse.
- The registration order in `registry.rs` matters.
- A module never has its own migration version table.

Pre-release, the sequence can be squashed (currently `0001` DDL + `0002` seed
data) and dev databases are recreated, not migrated. **As soon as any database
holds real data, the composed sequence becomes append-only as a whole**: new
migrations go at the end of the global sequence whatever crate owns them, and
each must pass two tests: it applies to a fresh database, and to a database at
the previous version.

## Alerts: worked out when read, never stored

Alerts (a PHI window open, a carné or an ITV coming due, a plot inside a zone)
depend only on the registers and today's date, and **nothing stores them**. Each
crate that raises alerts returns the ones that hold now, in core's standard shape
(`terrazgo_core::alerts::RaisedAlert`), and core puts the list together
(`repository::list_alerts`) whenever it is read. There is no refresh to call after
a write and no copy to go stale: a save, an imported file or midnight passing all
show the next time the list is read. Design and alternatives: docs/data-model.md →
"Alerts: the settled design".

- **Core owns the machinery and knows no kind but its own.** It has the standard
  shape, the status rules, the seen/hidden acts and the list, plus the three zone
  alerts, whose rule (the latest check says inside) belongs to no single domain.
  A kind is a constant its crate declares (`DatedKind` or `StandingKind`, two
  types so the compiler refuses a dated alert without its date), naming its code
  and the table its subjects are in, so no alert and no act ever takes the table
  from a caller. There is no kinds table, so a module adds an alert without a
  migration and without touching core.
- **module-phytosanitary raises the three its decree states**: the PHI window,
  the applicator's carné, and the equipment's inspection, with their lead times
  (`AlertConfig`, 60 / 30 days by default; **these aren't regulatory values**).
- **The shell collects them** from one list, `src-tauri/src/alerts.rs` →
  `ALERT_CRATES`, the only place an `AlertConfig` is built from the device's
  settings. Each crate answers with an `AlertReport`: the alerts it raises, and the
  records whose value it couldn't read, which the Status view names with where to
  correct them, so one bad record never hides the rest. A crate that fails
  completely is named in the list's `unavailable`, with the error's detail, and the
  Status view tells the farmer to update and report it; it never empties another
  crate's alerts. `alert_kinds_contract.rs` checks the list against the
  dictionaries both ways, and each kind's subject table against the aggregate map.
  The dates the rules read are also checked when saved (`date::validate_dates`),
  so a bad one is caught on the form rather than on the Status view.

What a person said about an alert (seen, or hidden) can't be worked out, so it is
stored: `alert_acknowledgement` in core, one insert-only row per act, synced and
logged like any register. An alert's status comes from those acts
(`alerts::alert_status`, the strongest act about the alert's current deadline)
and is stored nowhere. Why one row per act, and why each names a deadline:
docs/sync.md → Alert acknowledgements roam.

## Reference catalogues: vendored, imported, upsert-only

Regulatory exports have to use the provider's codes: for Spain, the FEGA SIEX
"Anexo VII" catalogues (efficacy, justification, crop and phytosanitary-problem
codes, units, machinery types…). The catalogue CSVs are vendored **inside the
binary** (`crates/terrazgo-core/catalogues/`, a snapshot of FEGA's public
catalogue API), and `terrazgo_core::catalogue::ensure_catalogues` imports them
into `catalogue` + `catalogue_code` on the first launch of each app version.
Codes resolve from the first run with no network; the snapshot is refreshed as a
release step, and users can also fetch FEGA's current copy from Settings, through
the same parser and the same upsert (docs/maintenance.md §1, which also has the
checks).

The import is **upsert-only**: FEGA retires codes with a baja date instead of
deleting them, and so do we, because a code on an old treatment record must still
resolve at an inspection. Pickers read `held_picks` (every code the device holds,
each marked `offered`), so they only offer live codes but still name a stored one
that isn't (retired since, or only known to another device's newer catalogue;
docs/sync.md → "What stays device-local"); looking a code up uses `find_code` (any
state). Storage design: docs/siex-export.md → "Storage design".

How records *use* catalogues depends on the size of the list (per-table detail in
docs/data-model.md, design in docs/siex-export.md → "Capture design"). Small
closed lists with a universal meaning (efficacy, IPM justification, authorisation
kind) are **our own lookup tables with English codes**, translated to the
provider's integers at export by `module_phytosanitary::siex`: records stay
country-neutral, the `es` dictionary has the official Castilian wording word for
word, and a two-way contract test against the vendored CSVs fails when the
provider adds or retires a code. Lists too large to own (the ~1,400
phytosanitary problems) store the **catalogue code as it is** on the record
(`treatment_problem`), checked on insert against the imported catalogue (only that
it exists: retired codes stay valid). Integer export ids live in `export_alias`:
created at the first export and never changed, because the authority keys edits
and deletions on them.

The export itself is the `terrazgo-siex` crate (docs/siex-export.md):
`export_precheck` lists what blocks a valid export, and `build_cuaderno` turns one
farm and season into the official CUE descriptor JSON, refusing while the
precheck isn't clean so nothing is silently dropped. It emits every block that has
a register behind it, one builder per file under `src/blocks/`, each reading the
module that owns its register. **The precheck refuses rather than drops**: each
rule exists because a record could otherwise have gone out with a value silently
missing, or not gone out at all with nothing on screen saying so. Several rules
come from FEGA's Anexo V `OBLIGATORIEDAD` grading rather than from the JSON
Schema's `required`, which says less; docs/siex-export.md → "The law outranks the
format" sets out that order, and is worth reading before adding or relaxing a
rule. The output is checked in tests against the vendored official JSON Schema
(the `jsonschema` crate, a dev-dependency only, never in the shipped binary). The
shell still registers the commands (`export_cuaderno_precheck`, async
`export_cuaderno`), but nothing in the UI calls them: the export has nowhere to go.

**Backup** (in `terrazgo_core::backup`): export is a `VACUUM INTO` snapshot
(consistent and compact while the app runs, no WAL sidecars), which is reopened
and integrity-checked before success is reported; an unchecked backup of
regulatory records is worse than none. Import checks the file, saves a safety
copy of the live database, then swaps and re-migrates. Older backups are fine
(migrated forward when opened); backups from a newer app are refused. A backup at
the *current* version is also checked for shape, because pre-release the
migration files are edited in place and `user_version` can't tell a file taken
before an edit from one taken after; unchecked, it would import cleanly and fail
later with `no such column`. **That fingerprint is put together like the
migrations**: core owns its own tables, each module contributes its own (its
`Module::backup_shape`), and the shell passes them together. Core may never name a
module's table, not even in a constant. Details:
[backup-restore.md](backup-restore.md).

**Sync** is planned in stages:

1. *One-way mirror* (phone exports, laptop imports a replaceable copy). Not
   built: the backup export/import covers its mechanics, and stage 2 replaces its
   model.
2. *Two-way local sync*: changes come from `record_change` and are exchanged as
   files (a live link later). Built; [sync.md](sync.md) Part 2.
3. *Cloud*: the same change format and merge rules; only the transport would be
   new.

UUIDv7 everywhere and full row images in `record_change` are what let stages 2–3
work without changing the schema. The live database file must never be put on a
network drive or a file-sync service (WAL breaks across network filesystems);
sync travels through exported files only.

### Conflicts and duplicates

Two different problems:

1. *The same register edited on two devices.* One person fixes a note on the
   phone while another corrects a dose on the laptop. This one is mechanical:
   causality per **register** (the root and its children) in a version vector, a
   hybrid clock only to order two sides already known to be concurrent, the
   winner's complete register live on every device, and the loser in a review
   queue ([sync.md](sync.md) → Merge). **Merging field by field was rejected**: it
   mixes two people's edits into a record neither wrote.
2. *Two different rows describing the same real event.* Two workers each record
   "applied product X on plot Y yesterday" on their own phones. Different UUIDs,
   both rows valid: **no sync algorithm can resolve this**, because as far as the
   data goes there is no conflict. The same property that makes merging
   collision-free (UUIDs) guarantees both records survive it.

For the second, no single layer is enough:

- **How people work.** The applicator records their own treatment. Most
  duplicates are a question of who is responsible, not a technical one.
- **One rule per register, at the form and in the list.** Each register of the
  book declares when two of its records look like one operation recorded twice
  (treatment: one farm, application dates within a day, one authorisation number,
  overlapping plots): a constant in the crate that owns it, listed by the shell,
  with a contract test that refuses a register that declared nothing. The form
  asks the rule right after saving (never before, so a suspicion can never hold up
  or refuse a save) and shows the person both records; the list asks it whenever
  it is read, so an import, an edit, or a record that skipped the form is listed
  too. Nothing stores a suspicion.
- **A person decides, never a machine.** *Both are real* and *keep this one* are
  acts in `duplicate_verdict`, one insert-only row each, so they travel without
  ever conflicting. Keeping one removes the other through its register's own
  delete, in the same change set as the act, so the log shows the removal and its
  reason together.
- **Never delete automatically.** Ids derived from content (hashing the natural
  key so duplicates merge themselves) were rejected: fragile (a dose of 1.5 against
  1.49, and the hash misses exactly when it is needed) and wrong for near-matches
  that are legitimate (two real applications of the same product on different
  recintos the same day). Machines detect; people decide.

Natural keys for *matching*, UUIDs for *identity*. What a rule compares is what
the record stored, never another user row's id: two phones that each add one
product from the catalogue hold two product rows and one authorisation number.
The details are in [sync.md](sync.md) → Duplicate suspects.

## The report engine: Typst in-process

Printable documents (the official cuaderno first; fertilisation plans, cost
reports and analytics later) are rendered by `crates/terrazgo-report`: **Typst as
a library**, not a webview `print()` (not available on Linux/wry or Android) and
not a low-level PDF writer (which has no layout engine; an unbounded treatments
table needs wrapping per cell, rows broken across pages and repeated headers).

It works offline by design:

- **Templates** are Typst source owned by the module that uses them, embedded
  with `include_str!`. A template holds layout, not text: its labels are document
  content per country (never UI i18n keys), but they arrive as ordinary inputs, so
  one template serves every language the document can be printed in (see "The
  book's language").
- **Fonts**: the four Liberation Sans faces (~1.6 MB, OFL-1.1, with the licence in
  `crates/terrazgo-report/fonts/`) are embedded with `include_bytes!`. Liberation
  Sans has the same metrics as Arial, the look of Spanish administrative forms. No
  system fonts are scanned, so the output is identical on every platform. They are
  vendored by hand rather than through Typst's `typst-kit-embed-fonts` feature,
  which costs ~10 MB for the Libertinus look and, more importantly, has a default
  font fallback that would quietly satisfy the zero-warnings check below instead of
  showing broken font wiring. Never add `ttf-parser` as a direct dependency to
  inspect these faces: it trips cargo-deny's unmaintained policy
  (RUSTSEC-2026-0192), and Typst's own `Font` parser reads them.
- **No package resolution**: typst-as-lib's network features stay off, so an
  `@preview` import in a template fails to compile instead of reaching for the
  network.

The API is one function: `render_pdf(template, &serde_json::Value)` →
`RenderedPdf { bytes, page_count, warnings }`. The inputs must be a JSON object
and arrive in the template as `sys.inputs` (strings, ints, floats, bools,
`null`→`none`, arrays, nested objects). Two rules for template authors:

- **Pin the font family** (`#set text(font: "Liberation Sans")`) and check in the
  template's tests that the render produced **zero warnings**. Typst treats an
  unknown font family as a warning plus a silent fallback, and the warnings list is
  where that shows. The crate's own tests check this (an unknown family must give a
  warning; the embedded faces must be listed under exactly that family name and
  cover the Spanish characters).
- **A failed `#assert` in a template stops compilation**, so templates can check
  the shape of their `sys.inputs` and turn a broken data contract into a test
  failure instead of a wrong document.

Rendering is synchronous and uses the CPU; commands that call it are `async fn`,
like other long-running commands.

### What Typst carries that no template uses

Typst 0.15.1 has no switch to leave any of this out. `typst` and `typst-library`
declare no features and no optional dependencies, and `typst-library` takes
`hayagriva` with its defaults on, so the bibliography engine comes with its
bundled style archive. Only the crate that declares a dependency can turn its
defaults off; typst-as-lib's optional features are already off. Measured on the
test binary from `cargo test --release -p terrazgo-report --no-run` (release
defaults, no LTO), grouping symbols by crate with `nm -C -S --size-sort`; that
binary was 56.9 MB, 43.5 MB stripped and 16.8 MB gzipped:

| Typst part | Approx. size | Of which embedded data |
| --- | --- | --- |
| Bibliographies (`hayagriva`, `biblatex`, `citationberg`) | 4.4 MB | 3.1 MB of CSL styles and locales |
| Code highlighting (`syntect`, `two-face`) | 2.2 MB | 1.8 MB, the syntax set twice (with and without newlines) |
| Plugins (`wasmi`) | 1.4 MB | — |
| Images: PDF-in-PDF, raster, SVG | 3.7 MB | — |
| HTML export | 0.1 MB | — |

The figures are approximate, because grouping by symbol name misses
`typst-library`'s own code for each element. Images are the one part a document
might want later (a plot map, a farm logo). The cost is download size, install
size and build time, not speed: the OS only loads a binary's pages when they are
used, and nothing of Typst runs before a render. Ways to shed it:

- **A patched `typst-library` without those elements** would save 8–12 MB. They
  run through the standard library, realisation and layout, so the patch would
  span several crates and have to be redone at every 0.x release. Not done.
- **Stubbing the embedded data in `hayagriva` and `two-face`** with
  `[patch.crates-io]` would save about 5 MB, but leaves two vendored crates to
  re-sync by hand at every update instead of a version bump. Not done.
- **A release profile** isn't set up yet. The workspace has no
  `[profile.release]`, so symbol tables ship, and stripping alone took the binary
  above from 56.9 to 43.5 MB. LTO, one codegen unit and a size-tuned `opt-level`
  aren't measured, and they affect the whole app, not just Typst; the price is
  slower release builds. Android's Gradle build usually strips native libraries
  already. Measure on the real app binary and the release APK, with build time
  next to size, before choosing.

If Typst ever puts these parts behind features, dropping them becomes one line in
a manifest.

### The printable cuaderno

The first user of the engine. `terrazgo_recordbook::cuaderno_inputs` formats
EVERYTHING into strings beforehand (dd/mm/yyyy dates, decimal-comma numbers, the
official Spanish words for closed lists), so the template only does layout, and
the data contract is checked as plain JSON in the tests without parsing a PDF.
The document follows the official model's sections and its cross-reference scheme
(the treatments register names operators, equipment and plots by the order
numbers of the earlier tables, all built from the same records, so a reference
can't point at nothing), prints missing fields blank like the paper form (no
precheck: unlike the SIEX export, a farmer can always print the current state),
and adds a plazo-de-seguridad column the model doesn't have (the content list of
RD 1311/2012 Anexo III is what binds, and the PHI is on it).

**Catalogue labels are looked up once per book, never once per row.** Every coded
cell goes through one private `CatalogueCache`, created per book and passed
alongside the connection, so the queries a book makes depend on the **distinct
codes it prints**, not on how many rows print them. It **remembers each code it
has looked up; it doesn't preload whole catalogues.** Preloading looks like the
obvious fix while the vocabularies are small (`EST_FENOLOGICO` has ten rows), but
the same path resolves `MUNICIPIO_SIGPAC` (8 434 rows) and
`DETALLE_MATERIAL_FERT` (1 243), and a book names a handful of towns and
materials: reading a whole catalogue to resolve three rows costs more than it
saves. Remembering per code stays bounded at any size, so one mechanism serves
every catalogue. A code that resolves to nothing is remembered as such, or a book
written against a catalogue this device never imported would ask again for every
row. The same applies to other per-row lookups in assembly: plot order numbers and
names travel together as one `PlotIndex`, and machinery names come from one map.
A test assembles the demo book twice, the second time with four times the
treatments, and checks that the lookups asked for grow while the queries made stay
the same.

### The book's language

The record book's **layout is per country** (the Spanish official model, never
forked) while its **language is per region**: where there is a co-official
language, a farmer must be able to hand an inspector the same book in either. So
the document has a `Labels` struct per language (`terrazgo_recordbook::labels`),
serialized into the template's `sys.inputs` and read by the spreadsheet renderer
as well.

Worth keeping if a second document needs this:

- **A Rust struct, not a dictionary file.** A missing translation is a compile
  error, which is stronger than the key-parity test the frontend dictionaries
  need, and serde produces the template's dictionary for free.
- **Text translates, codes don't.** The model's own abbreviations
  (SEC/ASP/LOC/GRA, AE/PI/CP/…), dose-unit symbols and the FEGA catalogue labels
  for "problema fitosanitario" print as they are in every language; the footnote
  that explains an abbreviation is what translates. So the assembly holds no text
  at all; even the 2.2 zone summary is stored as values and worded when rendered.
- **Which languages are offered is worked out, not configured.**
  `terrazgo_recordbook::region` maps INE province codes (the farm's registry
  province plus each plot's SIGPAC reference) to the languages co-official there,
  keeping only those that have a dictionary. So a co-official language with no
  dictionary yet doesn't appear, and adding one is a single `Labels` const. A farm
  with no province recorded is offered every language rather than none: an empty
  form field says nothing about what the farmer may print.

### Spreadsheets: the second renderer

`render_xlsx` works like `render_pdf`: callers describe *what* a document contains
(`Workbook` → `Sheet` → `Cell`) and the engine decides *how* it looks (bold frozen
header, autofilter, column widths, `dd/mm/yyyy` dates). No module touches
`rust_xlsxwriter`, so the look stays the same across reports, and the crate could
be replaced without touching a caller.

`rust_xlsxwriter` (`default-features = false`) only writes, which is all a report
engine needs: pure Rust, MIT/Apache-2.0, and its few dependencies are all
permissive and free of C (zlib-rs rather than zlib), so Android cross-compiling
isn't affected. Not chosen: `umya-spreadsheet`, which reads as well as writes and
is heavier; writing OOXML by hand; and CSV, which can't hold the book's sections,
has no typed cells, and gives Excel decimal commas it will mangle.

The rule that matters is **typed cells**. The Typst template takes ready-formatted
Spanish strings because it only does layout; a spreadsheet must not, because
sorting by date, filtering a product and adding up hectares all need real values.
So a document's assembly produces typed data and each renderer formats from it:
one read of the database, two presentations, and a new field is added in one
place. Numbers have no number format, on purpose: Excel shows them in the reader's
own locale, which is how a Spanish user gets decimal commas without the app
hardcoding them.

Empty stays empty: an unknown value is an empty cell, never a zero (a spreadsheet
would add the zeros up, and official forms leave cells to be filled by hand).
Excel's tab-name rules (forbidden characters, 31 characters at most, no
duplicates) are fixed by the engine rather than left to callers, because a report
must never fail over a tab name.

## Device-local settings

App settings live in `settings.json` next to the databases, not in either of them
(`terrazgo_core::settings`). The same test that keeps `geo-cache.db` a separate
file applies: settings are preferences of one device, with no change log, no
sync, and **not in backups** on purpose (a backup exists so regulatory records
survive a lost device; it mustn't force the old device's cache size on a new one).

The file is one flat serde struct. Defaults live in code: a missing file or field
means "use the default" (`#[serde(default)]`), so adding a setting is adding a
struct field: no migrations, and old and new versions read each other's files.
Fields whose default belongs elsewhere are `Option` (`None` = follow the owner's
constant, e.g. the tile-cache size defaulting to
`terrazgo_geo::db::TILE_CACHE_MAX_BYTES`), so a later change to the default still
reaches users who never touched the setting. Writes are atomic (temp file +
rename); an unreadable file falls back to defaults, since settings are the one
store where fixing itself is better than reporting damage. Each setting is checked
by the crate that owns it (the cache-size range check is in terrazgo-geo).

Two things are kept out: the display language stays in `localStorage` (the
frontend has to know it before the first render, and the i18n layer stays
independent of the backend), and **secrets never go in this file** (it is plain
text; credentials such as CDSE accounts need their own storage).

**User profiles** are split across both stores by the same test. The profiles
themselves (`user_profile`: display name, optional operator link) are farm data:
synced, logged, only ever soft-deleted, because a profile id is the author on
`record_change.actor` and must resolve in old log rows on any device. But *which*
profile is active belongs to the device ("who is using THIS phone"), so it is
`active_user_id` in `settings.json`. It is allowed to point at nothing (a profile
deleted elsewhere, a backup restored on a new install): the shell treats that as
"no active profile" and never fails. Deleting the active profile clears the
setting in the same command. Profiles identify people; they aren't security:
there are no credentials. Real authentication would come with cloud sync, and a
local password guarding a SQLite file the user owns would protect nothing.

**The author.** Every repository write function (core and every module) takes an
`actor: Option<&str>` and passes it to `audit::begin`, whose change stamp writes it
to `record_change.actor` on every row of that write. The actor is a person; the
stamp's device is a copy of the database, and the two are never mixed up.
module-sigpac is included because checking a plot writes: `verify_plot` stores the
fetched boundary as a `geo_feature` row and the zone results as `plot_zone_flag`
rows, and "who checked this plot" is attribution like any other. The shell's write
commands read the active profile id from `SettingsState` on each call
(`active_actor`, releasing the settings lock before taking any other lock) and
pass it down; the demo seed passes `None`. Passing it explicitly was chosen over
attaching it to the connection: the backup import swaps the connection in the
middle of a session, which would silently drop an attached actor, while a
parameter can't be forgotten without the compiler noticing. The value is stored as
given, unchecked: profiles are only soft-deleted, so it resolves at an inspection,
and another device's value must survive sync even if it can't be resolved here.
`None` stays NULL, meaning "no active profile". Each log row records who made THAT
write, not who first created the row.

In the UI, the Settings view has the language selector, the offline-map cache
size (applied at once: shrinking evicts straight away), clearing stored maps, the
user profiles, the devices of the sync group, and backup export/import.

## Files the user picks: paths on desktop, content URIs on Android

Every file a user chooses in a native dialog (backup export/import, the PDF and
spreadsheet exports, boundary-file import, sync files) goes through
`src-tauri/src/user_files.rs`. The reason is Android: there the dialogs are the
system document picker (Storage Access Framework), which *creates* the
destination itself and returns a `content://` URI, and `std::fs` can't open that.
The fs plugin (`tauri-plugin-fs`, used from Rust only; no fs commands are granted
to the webview) turns a content URI into an ordinary file descriptor through the
platform's `ContentResolver`; desktop paths take the `std::fs` route in the same
call, so commands have one code path.

Three helpers cover every caller:

- `write_user_file`: bytes in memory → destination, truncating.
- `stage_dest` + `copy_to_user_file`: for producers that need a real path to write
  to (SQLite's `VACUUM INTO`). The checked snapshot goes to a private staging file
  and is then streamed out.
- `stage_user_source` (reading): plain paths pass through untouched; a URI is
  streamed into a staging copy first, because rusqlite and the GPKG reader need
  real paths.

Both writers only return once the bytes are in the file (`fill_user_file`'s
`sync_all`; see `docs/sync.md` → "How long the file sits incomplete"). Staging
files live in the app's *cache* folder and delete themselves when dropped, so they
are temporary by design and never in backups.

## The map tier

Mapping is infrastructure for the whole app (plots today; irrigation, zone flags
and treatments as overlays), not a SIGPAC feature. What each source shows is in
[map-data-sources.md](map-data-sources.md); SIGPAC itself is in
[sigpac-integration.md](sigpac-integration.md).

**One network path.** The webview never talks to the internet: the production CSP
stays `default-src 'self'` plus the `geo:` scheme. MapLibre loads everything
(tiles, style JSON, glyphs, sprites) from `geo://…/tiles/{source}/{z}/{x}/{y}` and
`geo://…/res/{prefix}/{rest}`, served by `src-tauri/src/geo_protocol.rs` →
`terrazgo_geo::fetch`: look in `geo-cache.db`, on a miss fetch through
`terrazgo-net` (the lock is **never** held during network I/O, and bursts of tiles
are fetched in parallel), store, answer. Only listed upstreams exist
(`terrazgo_geo::sources`; a new base map or overlay is a new entry). Upstream styles
are rewritten in Rust (`terrazgo_geo::style`) so no external URL ever reaches the
webview; responses carry `Access-Control-Allow-Origin` because the page's origin
differs from `geo://localhost` and MapLibre uses `fetch()`.

**Android TLS setup.** The platform verifier uses the Android trust store through
JNI, and panics on first use if it was never given the JVM and app context (which
would kill a worker mid-fetch and leave a blank map). `terrazgo-net`'s `android`
module (only compiled for Android) sets it up lazily at the top of `http_get`, the
one place every network request passes through. Doing it at the first fetch,
rather than at startup, matters: the Rust main thread starts from the process's
`onCreate` and races the activity's own `onCreate`, where tao captures the activity
context; but a fetch can only happen once the webview exists, and the webview lives
inside the activity, so by then the context is there. The JNI handles come from
tao's `main_android_context()` (the same tao copy the Tauri runtime links; the
version pin matters, because its statics hold the context). The verifier's Kotlin
half is pulled into the APK by `src-tauri/gen/android/app/build.gradle.kts`, which
finds the crate's bundled Maven repository through `cargo metadata`, so the Kotlin
version follows the Rust crate; a ProGuard keep rule stops release shrinking from
removing the class, which is only reached through JNI (maintenance.md §1). A setup
failure shows through the normal `geo_offline {reason}` diagnosis.

**WebGL2 is required.** MapLibre dropped WebGL1 in v6, so a webview without WebGL2
can't draw a map: the `Map` constructor throws `GPUInitializationError`, which
`MapCanvas.svelte`'s load catch reports as `map.engine_unavailable`, and every
other view keeps working. MapLibre is also ESM-only since v6 and can't find its
own worker under a bundler, so the worker is emitted as its own chunk (Vite's
`?worker&url`) and passed in with `setWorkerUrl` before the first map is created.
That worker has the same origin, so the CSP's `worker-src` is just `'self'` (no
`blob:`). `connect-src` names Tauri's own `ipc:` and `http://ipc.localhost`:
without them the CSP refuses the first invoke on desktop, and Tauri switches to
its postMessage transport for the rest of the session. Android doesn't use that
transport, so the two entries change nothing there.

**Two databases, two kinds of data.** Geometry a user attaches to a plot is *user
data*: the core `geo_feature` table (exclusive-arc foreign keys, logged,
soft-deleted, synced, in backups). Tiles and styles are *derived and can be
fetched again*: `geo-cache.db`, a separate file with its own small migration
runner, left out of backups, `record_change` and sync on purpose. Deleting it only
loses warm caches, which is why its schema check is to recreate it: `open_cache`
checks for the current shape and deletes and rebuilds an out-of-date cache file,
so pre-release schema squashes never break an installed cache. Offline with an
empty cache, the map falls back to a plain background with the stored geometry;
the app never stops working.

**The tile cache has a size limit.** Serving a tile updates `last_used_at` (at most
once per UTC day, so bursts don't turn reads into writes), and at startup (off the
critical path) the shell evicts the least recently used tiles past the limit and
reclaims the space with `VACUUM`. Only tiles count towards it: the `resource`
table also holds the SIGPAC lookup and zone-check responses that keep a checked
plot checkable offline, and evicting those would break that for a few kilobytes.
The limit is a user setting (Settings; `tile_cache_max_bytes` in `settings.json`;
unset means `TILE_CACHE_MAX_BYTES`, 512 MiB), applied immediately, and the same
view can clear the tile cache altogether (`resource` rows survive that too). The
default is a ceiling, not a reservation: a phone only holds the tiles it has
looked at.

**Layers as data.** `src/lib/mapLayers.js` follows `nav.js`'s idea: a module adds
a map overlay with one entry, either a GeoJSON layer (id, label key, `load()` via
invoke, MapLibre style specs) or a vector-tile layer (`vector()` returning the
source spec: `geo://` tile template, zoom range, attribution). An entry can also
declare `vectors()` (several sources behind one toggle; each style spec picks its
source with `sourceKey`), `minZoom` (the layer panel warns "zoom in to see" below
it), `inspect(props)` (rows for the "At this point" panel, which lists what every
*visible* overlay shows where the map was clicked), `defaultVisible: false`, and a
`legend`. `MapCanvas.svelte` is the embeddable map (base-layer switch, selection,
terra-draw drawing); `MapView.svelte` is the page around it (farm selector, layer
panel, drawing and import, `#/map?farm=…&plot=…` links); FarmView embeds the same
canvas read-only. MapLibre and terra-draw are loaded with `import()` only when
needed, so form views never pay for the map.

Boundary files (GeoJSON, and GeoPackage, which *is* SQLite, read with rusqlite +
geozero for the WKB geometry) are imported through `terrazgo_geo::import`: a light
list for the picker, then one checked geometry. Every geometry, whatever its
source, is checked by core's `geojson` validator when written. Only geographic
SRS are accepted: 4326, 4258 (ETRS89) and 4081 (REGCAN95, the Canary SIGPAC
datum; its registered shift to WGS84 is 0,0,0, so treating it as WGS84 is exact).
Projected files fail with a fixed error (sigpac-integration.md → "Projected
files").

**The Spanish provider: `crates/module-sigpac`.** A normal module (registered in
`registry.rs`, no migrations of its own) that turns the 7-part reference
`plot_es_extension` stores into live data from FEGA's Nube de SIGPAC (the service
FEGA offers for third-party apps, CC BY 4.0). `reference.rs` checks and
round-trips the reference, `client.rs` looks a recinto up by code or by point,
and the module has **no HTTP dependency**: every request goes through
`terrazgo_geo::fetch::cached_resource`, so responses are cached in `geo-cache.db`
and a lookup seen once works offline afterwards. An unknown reference answers HTTP
200 with an empty FeatureCollection, which the client turns into `Ok(None)`, never
an error. The tests run offline against real responses in `tests/fixtures/`.

On top of the client, `storage.rs` turns a fetched recinto into a `geo_feature`
with `source='sigpac'` and the official area next to it (never overwriting the
user's `plot.area_ha`), and has the query that matches stored references
numerically to avoid duplicates; `service.rs` has the combined operations the
shell's three async commands wrap (look up by reference, look up by point, check a
plot). The UI has three ways into the ONE plot-creation flow (the plot form's
check and prefill, the map's pick-a-point → create or attach, and the import
picker's "create plot from recinto"), and all three end in the same
`create_plot`/`save_geo_feature` writes: a plot from SIGPAC is an ordinary plot
with one more geometry source.

**Zone flags** come from the same check: once the boundary is stored, the module
queries the three regulatory layers (nitrate-vulnerable, phytosanitary
restriction, Natura 2000) and writes core's `plot_zone_flag` (one row per plot,
zone kind, campaign and source, *including* "outside" rows, which prove the check
ran and was clear). Unlike alerts, flags can't be worked out offline, so they are
user data: logged, synced, backed up. Core's zone alerts read them and raise one
standing alert per (plot, zone kind) whose latest campaign says 'inside'; the
subject is the plot, so dismissing it survives new checks and new campaigns. A
zone check that fails after the boundary is stored is reported
(`zone_check_error`), never fatal, and the plot cards show the flags as chips.

**SIGPAC tile overlays** are ordinary entries in the source list, keyed by
campaign: the tile URL has no campaign year (the fixed path serves the *current*
campaign, or the previous one for `cultivo_declarado`), so cache rows are keyed
`sigpac-recintos@{campaign}` using the same campaign lookup as the zone checks, and
storing the first tile of a new campaign evicts the old campaign's rows. Tiles with
nothing in them answer HTTP 404, which the fetch layer caches and serves as an
*empty* tile, so known-empty countryside costs no repeat requests and reads as
empty offline, not as an error. **That has one blind spot**: the z12 recinto set is
missing tiles where recintos are dense, and a missing tile answers the same 404,
byte for byte, so a hole would be cached as empty and parcels would stop at a tile
edge. Nothing in the response tells them apart, so the recinto overlay starts at
z13 (z13 has far fewer holes, and FEGA's own viewer only draws recintos from its
level 16, MapLibre's z15). A contract test checks the shared SIGPAC tile shape
(pbf, z12–15, CC BY 4.0, keyed by campaign, 404 as empty).

**The farm's own data as overlays**, with no network: `phi-status` tints each
treated plot by whether a PHI window covers today (red = harvest restricted, green
= treated and clear), from `list_phi_status` → module-phytosanitary's
`phi_status_for_farm`, worked out when read (the same `[application_date,
phi_end_date)` rule as the alerts, and tested against it). `zone-flags` tints plots
by the stored zone checks (the latest campaign's 'inside' per plot and zone kind,
as for the chips), one translucent fill per zone kind so overlapping zones blend.
Both are plain GeoJSON `mapLayers.js` entries that join `list_geo_features` with
their status command, one feature per plot (stacked boundary sources would double
the tint).

## Pages the app points at, and never talks to

Spain's agricultural registries have no machine interface: ASPAFITOS is a
server-rendered ASP.NET app, REGMAQ-ROMA answers a form POST with HTML, and ROPO's
bulk download is out of date. Scraping isn't acceptable here, so the app can't look
a farmer's ROMA number up for them. What it can do is say which registry holds it
and open that registry, which is what `src-tauri/src/external_links.rs` and
`tauri-plugin-opener` are for.

**This isn't a network path.** Nothing here fetches anything; it hands a URL to the
system browser and forgets it. `terrazgo-net` is still the one place the app itself
speaks HTTP, and neither core nor any module gains a dependency from this.

**Rust owns the URLs.** The list is a `const` table of `(id, url)` in
`external_links.rs`; the `open_external_link` command resolves an id or fails with
`Invalid("unknown_link")`. The webview passes `"roma"`, never a URL, so the opener
plugin is registered with **no `opener:allow-open-url` permission**, as with
`user_files.rs`.

| Where | What it knows | Why there |
|---|---|---|
| `src-tauri/src/external_links.rs` | id → URL | It is a permission decision: one source of truth for a destination, and the webview can't name one. |
| `src/lib/registryHints.js` | country + field → id | It is presentation: which *field* gets a hint is a UI question, and it is per country, so no Spanish registry is hardcoded into a shared form component. |
| `src/i18n/<locale>/external.js` | id → the sentence | Registry names (ROMA, ROPO, SIGPAC) don't translate; the sentence around them does. |

`src-tauri/tests/contracts/registry_hints.rs` keeps the three in step, because
otherwise the failure would only show when a farmer taps a button.

**What has no link, on purpose.** REGA, SIEX, MDF, RGSEAA, REGFER and NIMA have
columns in the schema but no link: none has a stable public lookup page, and a hint
that leads nowhere is worse than a plain label. `farm_es_extension.rea_code` can
never have one: REA is a per-community registry (REACYL, SIDEAC, …) and
[siex-export.md](siex-export.md) → "the REA-first rule" forbids any text naming one
community's service.

## The frontend in one page

Full conventions in [frontend-conventions.md](frontend-conventions.md); the
outline:

- **Two tiers.** Framework-agnostic plain JS (`i18n.js`, dictionaries,
  `backend.js`, `nav.js`) would survive a change of framework; Svelte views sit on
  top. Business logic lives in Rust: the frontend collects input and shows results.
- **Routing** is a small hash router: `App.svelte` follows the hash and
  `lib/routes.js` maps it to a view (the table is data). Navigation destinations
  are data too (`lib/nav.js`), drawn twice: a collapsible sidebar on wide screens,
  a bottom tab bar on phones. Adding a view is one entry in each.
- **Feedback** has two places, depending on whether a form is involved. A form's
  own problems (empty required fields, and the backend refusing to save it) are
  drawn by `TzForm`: a summary at the top of the form listing all of them, plus each
  field's own message. Everything else goes to the notification bell, where `run()`
  turns backend errors into red notifications (the panel opens by itself) and
  successes add to the badge. A success is only added when it says something the
  screen doesn't already show.
- **i18n**: every user-facing string is a key present in *every* locale dictionary
  (a contract test enforces it); schema codes are translated when shown, with
  `tCode`; data the user enters is never translated.
- **The form controls are the app's own**: dates, time and every dropdown are the
  `.tz-*` components on Bits UI (headless parts: behaviour and ARIA, no styling).
  The reason is correctness, not looks: the native date picker follows the **OS**
  language and would override the language the farm chose, on a field that appears
  in every register of the book. Each takes and returns a plain string. Rules, the
  40-row limit and which Bits UI components are off limits are in
  [frontend-conventions.md](frontend-conventions.md) → "Owned controls".
- **No `@tauri-apps/api` dependency**: `withGlobalTauri` exposes
  `window.__TAURI__`, and plugin calls use the same transport
  (`invoke("plugin:dialog|save")`).
- **Build layout.** Vite's root is `src/` (`index.html`, `main.js`, `App.svelte`),
  with `vite.config.js` and `package.json` at the repository root; the output is
  `dist/`, which is gitignored and is what `tauri.conf.json`'s `frontendDist`
  points at. **`npm run build` must run before the first `cargo check` or `cargo
  test`**, because Tauri's codegen embeds `dist/` at compile time: on a fresh clone
  the Rust build fails without it, and the error doesn't say so.

### What the webview is allowed to call

`src-tauri/capabilities/` holds the Tauri 2 ACL, and it is small on purpose.
App-defined commands registered through `generate_handler!` **aren't** covered by
the ACL at all, so the files only cover what the injected global API and the
plugins need:

- `default.json`: `core:default` for the events and window plumbing, plus the
  three dialog permissions (`save`, `open`, `message`) used by file dialogs and
  destructive-action confirmations.
- `mobile.json`: the geolocation permissions for the map's GPS lookup, behind a
  `platforms` condition so desktop builds never see a plugin that doesn't exist
  there.

**No filesystem and no opener permission is granted to the webview, and that is
the point.** Two plugins are registered with nothing in the ACL, because both are
driven entirely from Rust:

- `tauri-plugin-fs`: every dialog-chosen read and write goes through
  `src-tauri/src/user_files.rs` (see "Files the user picks" above);
- `tauri-plugin-opener`: every outbound link goes through
  `src-tauri/src/external_links.rs` (see "Pages the app points at" above).

The pattern is the same in both: **the frontend names a thing, and Rust decides
what that means.** A webview with `opener:allow-open-url` could open any URL it was
talked into building; one that can only pass `"roma"` can't. So the webview's
permissions stay at zero.

## What guards all of this

Most of the rules above are invisible to the compiler, so tests hold the line (see
the testing strategy below; compliance rules are written test-first):

| Guard | Where |
|---|---|
| Repository behaviour, including the change-log payload | `crates/*/tests/repository*.rs` |
| Compliance rules (PHI maths, alert windows), test-first | `crates/terrazgo-core/src/date.rs` tests, `crates/module-phytosanitary/src/alerts.rs` tests |
| Migrations apply fresh AND from the previous version | `crates/*/tests/migrations.rs`, `src-tauri/tests/` |
| Every command registered ↔ every registration has a command | `src-tauri/tests/contracts/command_registration.rs` |
| Locale dictionaries in step ↔ error codes covered | `src-tauri/tests/contracts/i18n_contract.rs` |
| No `unwrap`/`expect` outside tests | `[workspace.lints.clippy]` in `Cargo.toml` + `clippy.toml` |
| fmt / clippy `-D warnings` / prettier / eslint / tests on every push and PR | `.github/workflows/ci.yml` |
| RustSec advisories on the dependency tree | `deny.toml` + the CI `audit` job |

### Testing strategy

Test-first where it pays, by kind of code:

1. **Domain and business logic: test-first, regulatory or not.** Compliance rules
   (PHI end dates, licence and ITV expiry, alerts, record checks) and equally any
   module's calculations, such as future irrigation recommendations or analytics:
   the failing test is written from the requirement's source of truth (a
   regulation, or a technical reference like FAO-56), then the code. Edge cases
   (leap years, campaign boundaries, multi-plot treatments) are included.
2. **Repositories and the data layer: tested alongside.** Every public repository
   function runs against an in-memory SQLite database with the migrations applied.
3. **Migrations: always tested.** Each migration applies cleanly to a fresh database
   AND to a database at the previous version.
4. **Tauri commands: thin, lightly tested.** The logic lives in the crates and is
   tested at levels 1–2; commands are wiring.
5. **UI: no unit tests while it keeps changing.** It is checked by scripts instead:
   a headless-browser harness drives the built bundle with a stubbed `invoke`
   (stubbed or recorded backend data), and an app-level harness drives the real
   debug binary in the real webview.

### How the test suite is organised

It follows the cargo convention, with no custom harness: integration tests in each
crate's `tests/`, plus inline `#[cfg(test)]` modules and doc-tests. Things to keep:

- **Tests run against the real migrations.** Each crate's `open_in_memory()`
  applies the actual composed migrations, so a repository test meets the shipping
  schema's CHECKs and foreign keys, not a test schema that drifts from it.
- **Assertions are about meaning, and there is no snapshot library.** The book's
  tests check `rows[0]["species"] == "wheat"`, not a rendered blob. Snapshot testing
  has an "accept the new output" step that blesses regressions by habit, which is
  the wrong thing to have near a document with legal value.
- **Test names say what is tested** (`rejects_an_interval_that_ends_before_it_starts`),
  which keeps a large suite readable.
- **Large test files are split by subject**: `report_*.rs` by section of the book,
  `repository_*.rs` by entity (like `src/repository/`), and `export_*.rs` by SIEX
  block.
- **The source-scanning contract tests** (`i18n_contract`, `command_registration`,
  `spdx_headers`, `neutral_voice` and the rest) turn into tests what most projects
  leave to a grep in CI, so they run before a push rather than after.
- **They all live in ONE test binary** (`src-tauri/tests/contracts/`, with `main.rs`
  as its root). A new one is a file there plus a `mod` line, never a file directly
  in `tests/`. Cargo compiles each file directly under `tests/` as its own crate
  linked against the whole graph, which here is Tauri plus Typst: hundreds of MB
  per binary, mostly debug info. Many of them filled the CI runner's disk (`ld`
  maps its output into memory, the filesystem fills up, and touching the mapped
  pages raises SIGBUS rather than a clear ENOSPC). Run one with `cargo test -p
  terrazgo --test contracts <filter>`. **A file put directly in `tests/` still
  compiles and passes, which is why this is written down.**
- **`migration_composition.rs` checks that both consumer crates' hand-written
  `db::migrations()` match the module registry**, so a module registered in the
  shell but forgotten in the record book's or the descriptor's migrations fails
  here instead of as "no such table" in a report.
- **The migration upgrade test can't yet prove what its name promises.**
  `applies_cleanly_on_top_of_previous_version` goes to version 1 and then to the
  latest, which while the migrations are squashed into `0001`/`0002` only tests
  "schema, then seed data". It becomes meaningful with the first append-only
  migration; until then **passing isn't an upgrade guarantee.**
- **`open_in_memory()` is public on the shipping crates**, on purpose. Putting it
  behind a `testing` feature would break the doc-tests that use it, and the shell
  turns on module-phytosanitary's `demo` feature always (features can't depend on
  the build profile), so a `testing` feature would end up always on too. No crate is
  published, so the surface stays inside the workspace.

#### Where a shared test helper goes

Three places, and the question that picks one is *what schema does it need?*

- **Only `terrazgo-core` → `crates/terrazgo-testkit`**, a dev-dependency-only crate.
  It has `farm_with_plots` (a farm with two plots and its season, and a second farm
  with the plot every `PlotNotOnFarm` test needs and its own season; a season
  belongs to one farm, so a test writing a record on the second farm files it under
  `other_season_id`), `last_change` (the latest `record_change` row as `(operation,
  before, after)`), `last_stamp` (which change set and register that row landed
  in), `query_cost` (statement and row counting) and `TempFile` (a temp path that
  deletes itself, `-wal`/`-shm` sidecars included). **It depends on core and nothing
  else, and that is the point.** A testkit that depended on `module-phytosanitary`
  would put that module's schema into module-fertilisation's tests, and each
  module's `the_module_runs_on_core_alone` check would keep passing while that was
  true.
- **This crate's own schema, used by two or more test files →
  `tests/common/mod.rs` in that crate.** It re-exports the testkit, so a test file
  needs one `mod common;` and one `use`, and adds what is local: the `db()` and
  `db_with_catalogues()` openers and the crate's shared fixture. A crate with one
  test file has no `common`.
- **Another crate's schema → it can't be shared; copy it and say why.**
  `terrazgo-recordbook/tests/common` and `terrazgo-siex/tests/common` each build the
  same export-ready Spanish farm, and both say so at the top. Sharing it would mean
  the testkit reaching into module-phytosanitary, the back door above; and it is the
  same trade the two crates make, since they read the same registers and share no
  code.

#### The catalogue fixture is explicit

**A test that turns a catalogue code into a label opens the database with
`db_with_catalogues()`; every other test uses `db()`**, so whether a test sees
labels or bare codes is stated, not inherited from whichever test was copied.
(Importing the vendored snapshot also parses 1.6 MB of CSV each time.) The exception
is core's tests OF `ensure_catalogues` itself (idempotency, upsert never deletes),
which call it by hand: there the import is what is being tested.

#### Doc-tests: where the example IS the specification

- **Pure functions get a worked example each**: core's `date` and `geojson`,
  module-fertilisation's `agronomy`, module-phytosanitary's `alerts`. There the
  example is the specification, and it costs nothing to run.
- **Each crate root gets one end-to-end example.** Core's opens, writes and reads
  back; terrazgo-report's renders a real PDF and checks the warnings list is empty.
- **Repository functions get none.** A doc example per repository function would
  start a database each, for documentation whose real specification is the test
  next to it.

## Releases

Releases are at
[github.com/clozanoruiz/terrazgo](https://github.com/clozanoruiz/terrazgo), with the
issue tracker: installers per platform (Linux AppImage/deb/rpm, Windows NSIS +
portable `.zip`, Android APK) plus the **complete source of that version**, one
snapshot commit per release, so the AGPL source offer comes with every binary. The
installers are built by that repository's own `build.yml` workflow from the tagged
source, so every binary comes from exactly the source published next to it. Each
artifact has signed SLSA build provenance and every release has a CycloneDX SBOM
attested against the installers; check any download with `gh attestation verify
<file> --repo clozanoruiz/terrazgo`. Release notes are written by hand before a
draft release is published.

The snapshot is produced by `packaging/`, which holds the export script and the
public Spanish README and issue templates it adds. The script removes the
development-only folders and then **fails the release** if a case-insensitive grep
finds any remaining reference to them, so a stray mention in a crate or a doc stops
the publish. `packaging/` itself is removed from the snapshot. Procedure and the
full release checklist: [maintenance.md](maintenance.md) §6.

## Recipes — where to start when you want to…

- **Add a command end to end** → the checklist in
  [frontend-conventions.md](frontend-conventions.md#adding-a-command-end-to-end-checklist)
  (repository + test → thin wrapper → `generate_handler!` → i18n keys → `TzForm` or
  `run()` + `notify()`).
- **Add a view** → one `NAV_ITEMS` entry in `src/lib/nav.js` + one entry in
  `src/lib/routes.js`; keys in the area file of every locale.
- **Add a module** → a new crate under `crates/` depending on `terrazgo-core`;
  implement `Module` (`name`, `migrations`, `backup_shape`, `sync_shape`,
  `row_captions`; the shapes have **no default**, so a module with tables can't
  forget to declare them and still compile); register it at the END of
  `registered_modules()`; add `src-tauri/src/commands/<module>.rs` and list its
  commands in `lib.rs`'s `generate_handler!`; add a `Classify` impl for its error
  type and one line to the shell's downcast chain; add one area file per locale.
  The core doesn't change. `sync_shape` says how each of the module's tables merges
  (a register of its own, a child of one, a slot-keyed register, or never synced;
  [sync.md](sync.md) → The aggregate map), and the module's test opener installs it
  with a device id, so its own tests check every log row against it. For tests:
  take `terrazgo-testkit` as a dev-dependency and build fixtures on
  `farm_with_plots`; add a `tests/common/mod.rs` once a second test file needs to
  share something; and if the crate is a library, add it to the `cargo llvm-cov`
  line in the same commit, or it isn't measured. Three things stay written by hand
  on purpose (the registration line, `classify`'s line and the `generate_handler!`
  entries); why, and why a third-party plugin API is the wrong goal, is in
  [stack-choices.md](stack-choices.md) §5.
- **Change the schema** → high-stakes: design first. Pre-release, edit the squashed
  `0001`/`0002` and recreate dev databases; after release, add a migration and write
  both migration tests. **A new user table is also classified for the merge** in its
  crate's sync shape; `sync_shape_contract.rs` refuses the schema otherwise, and
  refuses a UNIQUE key two devices could fill without the merge seeing it.
- **Add a language** → one `SUPPORTED` entry in `src/i18n.js` + one folder of area
  files with every key (the contract test checks it's complete).
- **Re-theme** → edit the CSS variables in `:root` (`src/styles.css`); nothing else
  uses raw colours.
