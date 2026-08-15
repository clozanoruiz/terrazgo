# Stack choices — questions asked, and the answers

> Status: reviewed 2026-08-12. Five candidate changes to the stack were weighed
> on what they would *offer*, not on how much work they would be. **Four of the
> five answers are "no", and that is the reason this document exists**: a
> rejected option with its reasoning written down stays rejected, while one
> that was only ever discussed comes back every few months. Nothing here is
> scheduled; the items that survived are listed at the end as candidates.
>
> The measurements are kept alongside the verdicts. If a premise changes — a
> new raster need, a library that grows a capability it lacks today — the
> numbers are what to re-check, not the conclusion.

## Summary

| Question | Answer |
| --- | --- |
| OpenLayers instead of MapLibre? | **No.** Its three advantages are all out of scope by earlier, unrelated decisions |
| Own the UI controls on Bits UI? | **Yes** — done 2026-08-14: dates, time and every dropdown, the last hand-rolled picker included |
| Svelte stores / a MobX-style layer? | **No library.** One narrow module for reference data |
| Tailwind CSS or similar? | **No.** The gap is a thin token set, not the CSS language |
| An API for third-party extensions? | **One already exists** and it is compile-time. Improve it for our own modules |

---

## 1. The map engine: OpenLayers vs MapLibre

OpenLayers has three genuine advantages over MapLibre: arbitrary projections,
native WMS/WMTS clients, and a first-class `GeoTIFF`/COG source with WebGL band
math. All three are already out of scope here, and each for a reason that has
nothing to do with which library draws the map.

- **Projections.** [sigpac-integration.md](sigpac-integration.md) records the
  original comparison — OpenLayers "would only win if we needed exotic
  projections", and SIGPAC MVT is standard EPSG:3857. The boundary importer
  allowlist is identity-only (4326/4258/4081), and
  [map-layers-roadmap.md](map-layers-roadmap.md) puts reprojection of map
  imagery out of scope.
- **WMS.** The roadmap decides WMS is fetched as **Rust-side grid-snapped 3857
  tiles**, and explicitly rejects client-side bbox pass-through because float
  bbox cache keys bypass the tile LRU cap. OpenLayers' `TileWMS`/`ImageWMS` are
  that rejected design; adopting them would move work back into the webview
  that was deliberately moved into Rust.
- **Band math.** [agro-data-services.md](agro-data-services.md) rules out raw
  Sentinel-2 processing in-app — roughly 1 GB per L2A granule, on a rural
  connection. NDVI arrives as server-rendered tiles via evalscript.

Every planned raster — Catastro, IDEE hydrography, MITECO, ITACYL soil, NDVI
mosaics — arrives as a pre-tiled 256×256 XYZ image through `geo://`. MapLibre's
`raster` source already serves that shape; `src/lib/mapLayers.js` would gain a
`raster()` entry kind of a few lines beside its existing `vector()`.

### What a swap would cost

MapLibre itself is confined to one file (`src/lib/MapCanvas.svelte`, 508 lines),
which makes the swap look cheaper than it is. `src/lib/mapLayers.js` (429 lines,
7 entries) returns **verbatim MapLibre style-spec objects**, and all of it would
be rewritten. Beyond that: `crates/terrazgo-geo/src/style.rs` builds MapLibre
style documents; the scripted-check fixture set stores a MapLibre style as
ground truth; and the planned map snapshot for the PDF assumes a WebGL canvas
that can be read back to PNG.

The vector base map is the real loss. OpenFreeMap publishes only a MapLibre
style for its OSM schema, so OpenLayers would need `ol-mapbox-style` as a bridge
— a fourth runtime dependency, to render what MapLibre renders natively, with
weaker label placement and CPU-canvas drawing on the phone, which is the slowest
device in the stack. Only terra-draw is cheap to move (~25 lines; it ships an
official OpenLayers adapter).

### The two raster needs, and why they point the same way

Two concrete pulls were named: **NDVI earlier than the roadmap schedules it**,
and **the farmer's own rasters** (a drone orthomosaic, a scanned plan). Neither
was recorded anywhere before this review.

**Farmer-supplied GeoTIFF** is the case OpenLayers looks built for, since
`ol/source/GeoTIFF` reads a local file client-side. It is also the case where
reading client-side cannot work on the platforms this app ships to: on Android a
dialog result is a `content://` URI that not even `std::fs` can open — the whole
reason `src-tauri/src/user_files.rs` exists — so the webview certainly cannot.
A drone orthomosaic is also 100 MB–2 GB, which no mobile webview will decode.

The shape that works is the one the architecture already has: **Rust decodes and
tiles the file once on import, then serves XYZ tiles through `geo://`.**
Identical on all three platforms, offline ever after, reusing the tile cache and
its LRU cap. The cost is real — a pure-Rust TIFF decode path (`tiff` covers
LZW/deflate/PackBits; JPEG-in-TIFF and the newer compressions are patchier),
reprojection through the already-vetted `proj4rs`, resampling, and a tile
pyramid — and it needs a storage decision first, because a farm orthophoto is
derived-but-irreplaceable and fits neither the evictable tile cache nor the
backed-up app database cleanly. But it is the same work whichever library
renders the result.

**NDVI** is unaffected by the library choice: rendered tiles land in a `raster`
source either way. Bringing it forward is a scheduling question about the
credential/secrets work, not a map-engine question.

### One thing MapLibre genuinely cannot do

Checked against the installed maplibre-gl 5.24.0 rather than from memory:
`raster-color` and `raster-array` are **absent** from the bundle, while
`raster-opacity`, `raster-hue-rotate` and `hillshade-shadow-color` are present —
so the check is sound. **MapLibre cannot recolour raster pixels client-side.**
Rendered tiles are baked; only opacity, hue-rotate, brightness, saturation and
contrast are available.

This is a real OpenLayers advantage, and it is answered rather than dismissed:
if an NDVI ramp must be adjustable, **cache the values and colour them in
Rust**, so changing the ramp is a local re-render that still works offline.
OpenLayers' client-side equivalent needs the raw raster in the browser, which is
the thing that fails on Android.

---

## 2. Owning the UI controls (Bits UI)

**Verdict: yes, and it is built** (2026-08-14). What shipped, how it is used and
what is still open are in
[frontend-conventions.md](frontend-conventions.md) → "Owned controls". This
section keeps the reasoning and the three costs, now with what each actually
cost.

**The strongest argument is not that controls look different per platform.** It
is that one control contradicts a decision the project treats as load-bearing.
The record book's language is the holding's choice among the languages official
where it sits — a `Labels` struct per language, and a region map deriving the
offer from INE province codes. The native date picker ignores all of it and
follows the **OS** locale, and dates appear on every regulatory record in the
book. That is a correctness-of-presentation defect rather than a preference, and
it is the claim the arc was verified against: with the session locale
`en_GB.UTF-8` and the app set to Catalan, the owned calendar renders
*agost del 2026* over `dl. dt. dc. dj. dv. ds. dg.` in the real WebKitGTK window.

The three costs, as predicted and as they landed:

1. **The scripted checks — much smaller than feared.** The prediction was that
   rewriting every check driving a `<select>` by `.value` would be the largest
   single chunk of the work, across 81 elements in 18 files. In fact `bridge.js`
   contains no `querySelector`, no `dispatchEvent` and no `.value =`: there was
   **zero committed select-driving code**, so the cost was documenting three
   snippets in the verifier skills, not rewriting a file. The trap that does bite
   is subtler and is now written down — a synthetic `element.click()` on a Bits
   UI trigger does nothing, so a check written the old way goes green while
   asserting nothing.
2. **npm supply chain. DONE 2026-08-13** — CI had `cargo-deny` for Rust
   advisories and nothing for npm, and a probe install of Bits UI pulled 44
   packages. The `npm-audit` job now runs `npm audit --audit-level=high`
   alongside `cargo-deny`, lockfile-only like it. Adding it surfaced three high
   advisories already in the tree (`postcss`, `nanoid`, `brace-expansion`, all
   dev-only transitives of vite and eslint), cleared by `npm audit fix` —
   lockfile patch bumps, no manifest change. **`high` deliberately, where
   cargo-deny denies everything**: the npm tree is mostly build tooling whose
   low/moderate advisories are not reachable from the shipped app, and a gate
   that cries wolf gets waved through.
3. **Android touch is a downgrade risk, and it is still open.** The native
   Android select is a full-screen thumb-friendly picker, and is the best of the
   three platforms' controls. "The same everywhere" means Windows and Linux
   improve while Android must be *matched*. `@media (pointer: coarse)` now floors
   a row at 44 px, but hit targets and scrolling on a real phone are the
   acceptance criterion, not a headless check — and no phone has run it yet.

**One cost was not predicted and is worth recording.** Bits UI sizes its
floating wrapper `min-width: max-content`, so a list of long labels lays every
row on one line: the fertiliser-kind picker measured **1696 px** and pushed the
whole page into a sideways scroll at 1280 *and* 390. Capping `.tz-popover` at
`--bits-floating-available-width` fixes it. The general lesson is that adopting a
headless library means adopting its sizing defaults too, and those are invisible
until a list with long labels meets a narrow viewport.

### The CSP question, settled by experiment (2026-08-12)

An owned-control library positions floating elements at runtime, and the
production CSP is `default-src 'self'` with **no `style-src`** — so
`style-src-attr` and `style-src-elem` both fall back to `'self'`. Measured in a
release build, with an external https fetch as the control proving a policy was
actually in force:

| operation | result |
| --- | --- |
| `setAttribute('style', …)` | **blocked** (`style-src-attr`) |
| runtime-injected `<style>` element | **blocked** (`style-src-elem`) |
| `el.style.prop = v` | honoured |
| `Object.assign(el.style, …)` | honoured |
| `el.style.cssText = …` | honoured |
| **static `style=` attribute in component markup** | **blocked** (corrected 2026-08-15) |
| *control:* external https fetch | blocked (`connect-src`) |

**That last row was wrong until 2026-08-15, and it had a live victim.** The
original pass recorded static `style=` attributes as honoured and cited the
skeleton bars as the proof; auditing an Android launch log found two
`style-src-attr` violations whose reported hashes match `width: 22%` and
`width: 46%` — `Skeleton.svelte`'s bars, which had therefore been rendering at
the wrong width under the production CSP the whole time. They are classes now.

What *is* true is the dynamic case: Svelte compiles a `style=` **binding** to
`el.style.cssText`, which is honoured, and that is why the map legend's five
swatches paint their real colours. The likely origin of the error is measuring
the binding and generalising to the literal. **A static `style=` attribute is
not safe; write a class.**

**The risk for an owned-control layer is therefore narrower than "inline styles"
but real: a library that injects a `<style>` element at runtime, or that calls
`setAttribute('style', …)`, fails silently under the production CSP.** Floating
UI — the positioning engine behind Bits UI — writes through
`Object.assign(el.style, …)`, which is honoured, so positioning is expected to
work; runtime stylesheet injection is the thing a spike must check. If a
relaxation is ever needed, `style-src 'self' 'unsafe-inline'` is a narrow
concession here, because the frontend renders no user-controlled markup
anywhere (`{@html}` appears zero times) — but it is a security decision to take
deliberately.

**Method note, because it cost two wrong runs.** *Neither verification tier can
answer a CSP question.* The headless-Chrome tier runs outside Tauri entirely.
The real-window tier runs a debug binary that loads `devUrl` from an external
dev server, and Tauri injects no policy there — measured: an external https
fetch succeeded with zero violation events. Even `cargo build --release` is not
enough, as it still loads `devUrl`. Only a build carrying the
`tauri/custom-protocol` feature — what the Tauri CLI passes for a real build —
serves the embedded frontend under the production CSP.

**Expected noise:** every `invoke` raises a `connect-src` violation event for
`ipc://localhost/<command>`, and IPC works regardless. It is not a fault to
chase.

**Outcome (2026-08-14): the prediction held in both directions, and the second
half is the one that earned this measurement its place.** Floating UI positions
through `Object.assign(el.style, …)`, so every popover in the app places
correctly under the production CSP — as expected. But four Bits UI components
(`Dialog`, `AlertDialog`, `DropdownMenu`, `ContextMenu`) engage a body scroll
lock whose *release* path calls `document.body.setAttribute("style", …)` — the
exact operation this table marks blocked. The lock would go on and never come
off, leaving `pointer-events: none` on `<body>` and an app that ignores the mouse
until restarted. Those four are forbidden, every content element passes
`preventScroll={false}` explicitly rather than trusting a default, and a grep
guard keeps them out (see
[frontend-conventions.md](frontend-conventions.md) → "Forbidden Bits UI
components"). Without this measurement the failure would have been found by a
user, not by a table.

---

## 3. Frontend state: stores, or a MobX-style layer

**A library would be a second reactivity system fighting the first.** Runes are
already a fine-grained observable graph; `$state` in a `.svelte.js` module is
what `makeAutoObservable` would give, with the compiler doing the work, and
Svelte components would not react to foreign observables without an adapter.
Legacy `writable()` stores are redundant for the same reason and are already
ruled out in [frontend-conventions.md](frontend-conventions.md). The sanctioned
pattern exists and is used once, in `src/lib/notifications.svelte.js`. So the
real question is not which library, but which state should move up a level.

**Do not cache the record data.** The backend is in-process SQLite over IPC —
no network, no latency to amortise. Refetch-on-mount is the correct default for
a legal record book: what is on screen came from the database now. A general
cache would convert 42 visible reload call sites into 42 invisible invalidation
rules, trading a cheap query for a class of silently-stale bug in regulatory
records. That is a good trade in a typical web app and a bad one here.

**Reference data is the part worth extracting.** As measured:

- 14 commands are fetched independently by two or more components —
  `list_farms` by five; `list_countries`, `list_operators`, `list_advisors`,
  `list_plots`, `list_problem_codes` and `get_settings` by three each.
- `TreatmentsView.svelte` mounts with 26 `$state` list declarations and 29
  `invoke()` calls, most of them lookups.
- Those lists are then prop-drilled two levels: `BookTreatments` takes 17 props
  and re-drills 17 into `TreatmentForm`.
- `list_problem_codes` is 602 rows, re-fetched in three components, and the
  record book remounts on every farm/season switch.

A `lookups.svelte.js` module holding the session-immutable reference data would
collapse much of that mount, delete most of the two-level drill — the real
maintainability win — and carry exactly **one** invalidation rule, whose owner
already exists: the catalogue refresh in Settings. Nothing else changes these
lists.

Caveat worth keeping: the prop-drill argument stands on its own, but **the
performance argument is unmeasured**. Measure a throttled mount before claiming
a speed win.

**MEASURED, then BUILT (2026-08-13).** The measurement came first and it
deflated the speed half exactly as this caveat warned. On the real backend
(debug build, demo data), the record book's cold mount fired **30 invokes,
painted at 91 ms and settled at ~140 ms**; served on their own the reference
lists cost **1-2 ms each and return 3-10 rows**; and the render half under a 6×
CPU throttle is **20 ms**, so the mount is IPC-bound rather than render-bound.
A recorded figure was wrong too: `list_problem_codes` takes a category, so it
is a per-category fetch, not one 602-row list read by three components.

`lib/lookups.svelte.js` was built on the maintainability case alone, and the
after-measurement is **10 invokes** for the same mount — the 20 that left were
the ones being drilled two levels deep. Two design notes worth keeping. The
lists are **warmed in `main.js` after the readiness gate**, not on first view:
loading them lazily made the first book mount *worse* (37 invokes, 124 ms),
because the module fetches all 26 while the view had fetched only the 21 it
needed. And the tier boundary held under contact with the code — **user data
the app itself edits stayed out** (farms, plots, operators, advisors, products,
materials), as did the country-scoped lists, which need a key their caller
already has.

A second, smaller module for the active farm/season selection would let it be
chosen once instead of per view. Modest for a one-farm holding, real for a
farmer with several; lower priority.

**The rule to carry forward: reference data may live module-level; records never
do.** The point at which refetch-on-mount stops being sufficient is **sync**,
when data changes underneath a view that did not initiate it. What survives that
is a thin invalidation channel — Rust emits an event, the frontend refetches —
not a client-side cache, which is a further reason not to build one now.

---

## 4. Tailwind CSS, or plain CSS

Measured rather than assumed. `src/styles.css` is **809 lines, 104 selectors,
3 media queries**, and **22 literal colour uses in total** — of which 9 *are*
the token definitions and 11 are `#fff`. Essentially every colour already flows
through the `:root` custom properties, as intended. `border-radius` is
effectively a single value (`0.375rem`, 9 of 15 uses). This is a small,
disciplined stylesheet.

Tailwind would not address either problem that actually exists:

1. **The token set is thin for what comes next.** Nine tokens, and no scales for
   spacing, elevation, z-index or interaction surfaces (hover/active/focus/
   disabled). The tell is already in the tree: `CataloguePicker.svelte` reaches
   for `var(--surface-hover, …)` and **`--surface-hover` is not defined**, so
   the fallback wins silently. An owned-control layer needs perhaps 15–20 more.
   That is a token problem, not a CSS-language problem.
2. **Drift.** There are now **6 component-scoped `<style>` blocks totalling
   ~312 lines** (`MapView` alone is 168). The drift is *correct* — Svelte scoped
   styles are the right tool for component-local rules — and the convention that
   forbade them is what is stale.

The one genuinely untidy number is **24 distinct `padding` values**, which a
six-step spacing scale absorbs in six lines of `:root`.

Against that, Tailwind moves styling into markup — in an app whose 800-line form
components are already dense with `{#each}`, `t()` and `bind:value` — and adds
PostCSS, a config and a class-ordering plugin to a toolchain that already
carries Rust, Tauri, Android and Typst. It buys a scale that takes an afternoon
to write.

**Stay on plain CSS; spend the effort on tokens.** Extend `:root` with
spacing/radius/elevation/z-index/interaction-surface scales including the
missing `--surface-hover`, promote `#fff` to a token so a future dark mode
really is a `:root` edit, and update the convention to sanction scoped `<style>`
for component-local rules.

**DONE 2026-08-13**, all of it; the token list is in
[frontend-conventions.md](frontend-conventions.md) → Styling. Two notes for
whoever extends it. The `#fff` uses were **two** tokens, not one — `--surface`
(a raised sheet a dark theme repaints) and `--on-primary` (text on a filled
`--primary`/`--danger`, light in any theme) — and collapsing them would have
made the eventual dark mode repaint the wrong half. And the pass was verified by
building the bundle before and after and pixel-diffing twelve screenshots
(6 routes × 2 widths): **all identical**, the only computed-style difference
being `--surface-hover` going from undefined to `#0000000f`, which is what the
CataloguePicker fallback was already painting. If the discipline is wanted pre-made, **Open Props**
fits far better than Tailwind: plain custom properties, no classes in markup, no
build step, droppable behind the existing tokens. Revisit Tailwind only if the
app outgrows one sheet plus scoped blocks, which at 104 selectors across 30
components it is nowhere near.

---

## 5. An API for third-party extensions

### What exists already

`src-tauri/src/registry.rs` is 84 lines, and the `Module` trait has **two
methods**: `name()` and `migrations()`. Modules are a hardcoded list of three.
The telling detail is that **module-sigpac contributes zero migrations**, and
that modules #2 and #3 both landed without the trait growing at all — the seam
is real but nearly vestigial.

Alongside it sit genuinely plugin-shaped, data-driven registries: `NAV_ITEMS` in
`src/lib/nav.js`, `TILE_SOURCES` and `RESOURCE_BASES` in
`crates/terrazgo-geo/src/sources.rs`, the supported-locale list in
`src/i18n.js`, and best of all `MAP_LAYERS` in `src/lib/mapLayers.js`, which has
a documented entry contract. If extensibility is ever wanted, that is the model
to copy; it does not need inventing.

### Why a runtime third-party API is the wrong goal

- **The command surface is ungated.** `src-tauri/capabilities/default.json`
  records that app-defined commands are not ACL-gated, so all **169** commands —
  including backup import, which swaps the entire database — are reachable by
  anything running in the webview. A JavaScript-extension model would hand a
  third party the whole regulatory dataset with no permission layer, and
  building that layer is a project in its own right.
- **The audit trail has no answer.** `record_change` stamps an actor. If an
  extension writes a `treatment_record`, there is no honest answer to who the
  actor was at an inspection, and the book is a legal document.
- **The licence already settles the ecosystem.** An extension linking the app's
  internals is a derivative work under AGPL, so the realistic ecosystem is
  contributors rather than a proprietary plugin market — and the compile-time
  crate path serves contributors perfectly well.
- **Nothing in the tree supports runtime loading, and the options are poor.** No
  `libloading`, `wasmtime`, `extism`, `mlua` or `rhai` in any workspace manifest
  (the lockfile hits are transitive tray and windowing plumbing). Native dynamic
  plugins mean Rust's unstable ABI, so host and plugin must share an exact
  toolchain and dependency set, and the mobile platforms restrict loading
  executable code. WASM offers a stable sandboxed ABI but needs a host-function
  interface designed for every capability, and addresses none of the UI half,
  since views are components compiled into the bundle.

### The reframe: this is developer experience, not extensibility

The friction a third party would hit is the friction every new module hits, and
**six modules are still unbuilt** (Irrigation in progress; Crop planning, Costs,
Weather, Sensors and Analytics not started). Adding one today means touching
about ten hardcoded points: `registry.rs`, a 2928-line `commands.rs`, the
169-entry `generate_handler!` block, `classify()`'s match, the backup-shape
hand-join, a literal migration step count, a hardcoded crate-directory list in
the i18n contract test, the SPDX source roots, the route chain in `App.svelte`,
and three flat dictionaries.

**DONE 2026-08-13** — all five, taken as one arc, plus the classify move.
What the list got wrong: the SPDX roots were never hardcoded (`["crates",
"src", "src-tauri"]` is directories, so a new crate is already covered), so it
was nine points, not ten. Six are gone, `classify` shrank to one line per
module, and **two stay by choice**: the `registered_modules()` line, which IS
the seam — removing it means linker-section discovery (`inventory`, `linkme`),
fragile exactly where mobile static linking is — and the `generate_handler!`
entry, where the exits are a tt-muncher macro chain or a build script that
regex-scans Rust with a regex, and the drift they would prevent is already
impossible, since `command_registration.rs` checks both directions.

Improvements worth making on their own merits, none of them plugin machinery:

1. **Split `commands.rs` per module** and let the registration contract test
   scan a directory instead of one file. It currently *structurally requires*
   every command to live in a single file, which is both the largest blocker and
   a file well past the size where it is pleasant.
   *Done: 2940 lines → a 200-line boundary file plus seven per-domain files
   (`app`, `core`, `cue`, `fertilisation`, `geo`, `sigpac`, `recordbook`). The
   parent re-exports each child with a glob, so the `commands::<name>` paths in
   `generate_handler!` did not move and a command can change domain without
   being an API change. One shared helper surfaced in the process:
   `reconcile_alerts` is used after writes in four domains, and modules never
   call each other — so chaining module-cue's alert engine is the shell's job,
   and it sits with the locks.*
2. **Move the backup shape onto the `Module` trait.** The trait's own comment
   says to resist adding methods until a second module needs them — two modules
   now ship a shape constant and the shell hand-joins them. That is the second
   consumer the comment anticipates.
   *Done, with **no default implementation**: an empty default would let a
   module that ships tables forget to declare them and still compile, which is
   the hand-joined list's hole moved rather than closed. module-sigpac returns
   `&[]` explicitly, and the compiler asks every future module.*
3. **Derive the assertions instead of hardcoding them.** The composed-migration
   step count and the i18n contract's crate list should come from the registry
   and the workspace members; the lesson is the same one a hardcoded catalogue
   count taught on 2026-08-11.
   *Done — and the i18n one was a live hole, not a chore: it named five crates
   while `module-sigpac` and `src-tauri` were also emitting `Invalid("…")`
   codes and going unchecked. Their keys happened to be present. The derivation
   reads the workspace manifest's own member list and asserts every crate on
   disk is in it, so neither half can drift silently.*
4. **Make the route table data-driven off `nav.js`**, which is already data —
   only the route-to-component mapping is not.
   *Done as `lib/routes.js`, NOT inside nav.js: nav.js is the
   framework-agnostic tier and may not import components. The honest split is
   that nav.js says what the navigation offers and routes.js says what each
   route renders — and the lists genuinely differ, since `#/farms/<id>` is a
   route with no nav entry.*
5. **Per-module i18n dictionaries merged at load.** Keys are already namespaced
   by prefix, so this is a merge step rather than a redesign.
   *Done: each locale is now a directory of seven area files merged by its
   entry point, verified lossless by diffing the merged objects against the
   originals (885 keys, identical values, all three locales). The split
   introduces one hazard — a key defined in two files would be silently
   overwritten by the merge — so it comes with the guard it owes.*

---

## What survived, as candidates

None of these is scheduled.

| Candidate | Size | Note |
| --- | --- | --- |
| ~~npm advisory gate in CI~~ | done 2026-08-13 | `npm audit --audit-level=high`, beside cargo-deny |
| ~~Owned date field on Bits UI~~ | done 2026-08-14 | Grew into the whole control set: dates, time, 78 selects, 4 filtered pickers and the free-text catalogue picker, accepted on a real Android device. See [frontend-conventions.md](frontend-conventions.md) → "Owned controls" |
| ~~Design tokens: spacing, elevation, z-index, interaction surfaces~~ | done 2026-08-13 | Verified by pixel-diffing twelve screenshots |
| ~~`lookups.svelte.js` for reference data~~ | done 2026-08-13 | Measured first: 30 → 10 invokes on the book's mount |
| ~~Module-seam cleanups~~ | done 2026-08-13 | Six of nine hardcoded points gone; two stay by choice |
| Rust-side raster import and tiling | arc | Needs a storage design pass before any code |
