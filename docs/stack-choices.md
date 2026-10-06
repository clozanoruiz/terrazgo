# Stack choices: questions asked, and the answers

This file keeps the answers to "should we switch to X?" questions about the
stack, with the reasons. Most of the answers are "no". Writing the reasons down
is what stops a rejected option from coming back every few months. Each option
was judged on what it would give the app, not on how much work it would be.

Where a measurement backs an answer, it is kept with it. If something changes
(a new raster need, a library that gains a feature it lacks today), re-check
the numbers rather than the conclusion.

## Summary

| Question | Answer |
| --- | --- |
| OpenLayers instead of MapLibre? | **No.** Its three advantages are all things the app already decided not to do, for other reasons |
| Own the UI controls on Bits UI? | **Yes**, and built: dates, time and every dropdown |
| Svelte stores, or a MobX-style layer? | **No library.** One small module for reference data |
| Tailwind CSS or similar? | **No.** What was missing was a few design tokens, not a different way to write CSS |
| An API for third-party extensions? | **There is one already**, at compile time. Make it easier for our own modules |
| An icon library instead of copied SVG paths? | **Yes: Lucide** (`@lucide/svelte`, ISC). It injects no stylesheet, so it works under the CSP |

---

## 1. The map engine: OpenLayers vs MapLibre

OpenLayers has three real advantages over MapLibre: any projection, built-in
WMS/WMTS clients, and a proper `GeoTIFF`/COG source with WebGL band maths. The
app has already ruled out all three, each for reasons that have nothing to do
with which library draws the map.

- **Projections.** [sigpac-integration.md](sigpac-integration.md) has the
  original comparison: OpenLayers "would only win if we needed exotic
  projections", and SIGPAC MVT is plain EPSG:3857. The boundary importer only
  accepts 4326/4258/4081 without reprojection, and
  [map-layers-roadmap.md](map-layers-roadmap.md) leaves reprojecting map images
  out.
- **WMS.** The roadmap fetches WMS as **3857 tiles snapped to a grid in Rust**,
  and rejects passing the bbox through from the client, because float bbox
  cache keys escape the tile cache's size limit. OpenLayers' `TileWMS` and
  `ImageWMS` are that rejected design; using them would move work back into the
  webview that was moved to Rust on purpose.
- **Band maths.** [agro-data-services.md](agro-data-services.md) rules out
  processing raw Sentinel-2 in the app: about 1 GB per L2A granule, over a
  rural connection. NDVI comes as tiles rendered on the server (evalscript).

Every planned raster (Catastro, IDEE hydrography, MITECO, ITACYL soil, NDVI
mosaics) arrives as a ready-made 256×256 XYZ image through `geo://`. MapLibre's
`raster` source already handles that; `src/lib/mapLayers.js` would need a
`raster()` entry kind of a few lines next to its `vector()`.

### What a swap would cost

MapLibre itself is used in one file (`src/lib/MapCanvas.svelte`), which makes
a swap look cheaper than it is. `src/lib/mapLayers.js` returns **MapLibre
style-spec objects as they are**, and all of it would have to be rewritten. So
would `crates/terrazgo-geo/src/style.rs`, which builds MapLibre style
documents; the scripted checks' fixtures, which store a MapLibre style; and the
planned map snapshot for the PDF, which assumes a WebGL canvas that can be read
back as a PNG.

The vector base map is the real loss. OpenFreeMap only publishes a MapLibre
style for its OSM schema, so OpenLayers would need `ol-mapbox-style` in between:
another runtime dependency to draw what MapLibre draws natively, with worse
label placement and CPU canvas drawing on the phone, which is the slowest device
the app runs on. Only terra-draw would be easy to move (about 25 lines; it has
an official OpenLayers adapter).

### The two raster needs, and why they point the same way

Two needs come up: **NDVI sooner than the roadmap has it**, and **the farmer's
own rasters** (a drone orthomosaic, a scanned plan).

**A farmer's own GeoTIFF** looks like the case OpenLayers is built for, since
`ol/source/GeoTIFF` reads a local file in the browser. But it is also the case
where reading in the browser can't work on the platforms the app ships to. On
Android the file dialog returns a `content://` URI that even `std::fs` can't
open (that is why `src-tauri/src/user_files.rs` exists), so the webview
certainly can't. And a drone orthomosaic is 100 MB to 2 GB, which no mobile
webview will decode.

What works is the shape the app already has: **Rust decodes the file and cuts
it into tiles once, on import, and then serves XYZ tiles through `geo://`.** It
is the same on all three platforms, works offline afterwards, and reuses the
tile cache and its size limit. It is real work: a pure-Rust TIFF decoder (`tiff`
handles LZW/deflate/PackBits; JPEG-in-TIFF and newer compressions are patchier),
reprojection with `proj4rs` (already in use), resampling, and a tile pyramid.
It also needs a storage decision first, because a farm orthophoto is derived
data that can't be replaced, and fits neither the tile cache (which evicts) nor
the app database (which is backed up). But the work is the same whichever
library draws the result.

**NDVI** doesn't depend on the library: rendered tiles go into a `raster` source
either way. Bringing it forward is a question of when the credentials work
gets done, not of which map engine to use.

### One thing MapLibre can't do

In maplibre-gl 6.9, `raster-color` and `raster-array` don't exist in the
bundle, while `raster-opacity`, `raster-hue-rotate` and `hillshade-shadow-color`
do (so the search itself is sound). **MapLibre can't recolour raster pixels in
the browser.** Rendered tiles are fixed images; only opacity, hue-rotate,
brightness, saturation and contrast can change. Check again after a MapLibre
upgrade.

This is a real point for OpenLayers, and the answer is: if an NDVI colour ramp
must be adjustable, **cache the values and colour them in Rust**, so changing
the ramp is a local redraw that still works offline. OpenLayers' way of doing
it needs the raw raster in the browser, which is what fails on Android.

---

## 2. Owning the UI controls (Bits UI)

**Answer: yes, and it is built.** What there is and how to use it:
[frontend-conventions.md](frontend-conventions.md) → "Owned controls". This
section keeps the reasons and the costs.

The main reason isn't that native controls look different on each platform. It
is that the native date picker ignores the record book's language. The book's
language is the farm's choice among the official languages where it is (a
`Labels` struct per language, and a region map that works out the choice from
INE province codes). The native date picker follows the **OS** language
instead, and dates are on every record in the book. With the OS in English and
the app in Catalan, the owned calendar shows *agost del 2026* over `dl. dt. dc.
dj. dv. ds. dg.`, as it should.

The costs:

1. **The scripted checks.** Almost nothing to change, because no committed check
   drove a `<select>` by its value. What does matter: a synthetic
   `element.click()` on a Bits UI trigger does nothing, so a check written the
   old way passes while testing nothing.
2. **npm supply chain.** Bits UI brings in many packages, and CI only checked
   Rust advisories. The `npm-audit` job runs `npm audit --audit-level=high`
   next to `cargo-deny`, reading only the lockfile as it does. The level is
   `high`, while cargo-deny denies everything, because the npm tree is mostly
   build tools whose low and moderate advisories can't be reached from the
   shipped app, and a check that fails for nothing teaches people to ignore it.
3. **Android touch.** Android's native select is a good full-screen picker, so
   on Android the owned control has to be just as good, while on Windows and
   Linux it is an improvement. `@media (pointer: coarse)` makes rows at least
   44 px tall. Touch targets and scrolling can only be judged on a real phone.

Bits UI also sets its floating wrapper to `min-width: max-content`, so a list
with long labels puts each row on one line. The fertiliser-kind picker came out
1696 px wide and pushed the whole page sideways, even on a desktop window.
`.tz-popover` is capped at `--bits-floating-available-width` to stop it. A
headless library brings its sizing defaults with it, and they only show when a
list with long labels meets a narrow screen.

### Bits UI and the CSP

An owned-control library positions floating elements at runtime, and the
production CSP is `default-src 'self'` with **no `style-src`**, so
`style-src-attr` and `style-src-elem` both fall back to `'self'`. In a build
under the real CSP (an external https fetch was used to confirm a policy was in
force):

| What the code does | Result |
| --- | --- |
| `setAttribute('style', …)` | **blocked** (`style-src-attr`) |
| a `<style>` element added at runtime | **blocked** (`style-src-elem`) |
| `el.style.prop = v` | works |
| `Object.assign(el.style, …)` | works |
| `el.style.cssText = …` | works |
| **a fixed `style=` attribute in component markup** | **blocked** |
| *check:* external https fetch | blocked (`connect-src`) |

A Svelte `style=` **binding** compiles to `el.style.cssText`, which works; that
is why the map legend's swatches show their real colours. A fixed `style="…"`
attribute is blocked. Use a class.

So the risk for any owned-control library is a library that adds a `<style>`
element at runtime or calls `setAttribute('style', …)`: it fails silently under
the production CSP. Floating UI, which positions Bits UI's popovers, writes
through `Object.assign(el.style, …)`, which works, so every popover in the app
is placed correctly. If the policy ever has to be relaxed, `style-src 'self'
'unsafe-inline'` would be a small concession, because the frontend renders no
markup the user controls (there is no `{@html}`). But it is a security decision
and should be taken on purpose.

**Bits UI's body scroll lock.** Some Bits UI components lock body scrolling,
and the lock's *release* calls `document.body.setAttribute("style", …)`, which
the CSP blocks. The lock would go on and never come off, leaving
`pointer-events: none` on `<body>`: the app would ignore the mouse until
restarted. In bits-ui 2.18.1 that call appears once in the package, only in
`BodyScrollLock`'s teardown; the lock is only created inside `if
(preventScroll)`; and `<ScrollLock>` is only rendered by components that pass a
`preventScroll` prop on. So `preventScroll={false}` means the lock is never
created. In this app it costs nothing, because `body { overflow: hidden }`
means there is no body scroll to lock. The rules that follow
([frontend-conventions.md](frontend-conventions.md) → "Forbidden Bits UI
components"): every content element passes `preventScroll={false}`, even where
that is the default; `TzDialog.svelte` is the only place that imports `Dialog`;
`AlertDialog`, `DropdownMenu` and `ContextMenu` stay out because nothing needs
them, and a grep check enforces it. After a Bits UI upgrade, check the
mechanism again (where the `setAttribute("style")` call is and what creates the
lock), not just the list of names.

**How to check a CSP question.** Neither of the usual checks can answer it. The
headless Chrome checks run outside Tauri. The app-level harness runs a debug
binary that loads `devUrl` from a dev server, and Tauri applies no policy there:
an external https fetch goes through with no violation. `cargo build --release`
isn't enough either, because it still loads `devUrl`. Only a build with the
`tauri/custom-protocol` feature serves the embedded frontend under the real
CSP. `tauri build --debug --no-bundle` has that feature and is several minutes
faster than a release build.

**Expected noise:** every `invoke` raises a `connect-src` violation for
`ipc://localhost/<command>`, and IPC works anyway. Nothing to fix.

---

## 2b. Icons: Lucide instead of copied SVG paths

**Answer: yes.** The icons used to be inline `<svg>` blocks and path strings
in `nav.js`, copied by hand from Feather. Lucide is Feather's maintained
successor, with about 1600 icons against Feather's 290, so adding an icon no
longer means copying from a website into our code.

**It meets the rule for a new frontend dependency: it injects no stylesheet at
runtime** (§2). In `@lucide/svelte`'s package, the shared `Icon.svelte` renders
an `<svg>` with its default attributes and a `class` list, and
`setAttribute("style", …)`, `<style>`, `insertRule` and `.style.` appear nowhere.
There is no CSS for the CSP to block.

Two things to know:

- **The size is an attribute** (`width={size} height={size}`, 24 by default), and
  any CSS size overrides an attribute. So the app's icon CSS still wins, and
  `.sidebar svg` doesn't need `fill`, `stroke`, `stroke-width` or linecap: the
  library sets all four.
- **`nav.js` names an icon; it doesn't hold one.** `nav.js` is in the
  framework-agnostic tier and can't import components, so its `icon` is a Lucide
  name and `lib/icons.js`, in the view tier, turns it into the component. It is
  the same split as `nav.js` and `routes.js`, for the same reason.

Cost: about +2.3 kB raw / +0.5 kB gzipped on the main chunk for the icons in
use, so tree-shaking works.

---

## 3. Frontend state: stores, or a MobX-style layer

**A library would be a second reactivity system competing with Svelte's.**
Runes already are a fine-grained observable graph: `$state` in a `.svelte.js`
module gives what `makeAutoObservable` would, with the compiler doing the work,
and Svelte components wouldn't react to another library's observables without
an adapter. Old-style `writable()` stores are redundant for the same reason and
are ruled out in [frontend-conventions.md](frontend-conventions.md). The pattern
to use already exists (`src/lib/notifications.svelte.js`). So the real question
is which state should move up a level, not which library to use.

**Don't cache records.** The backend is SQLite in the same process, over IPC:
no network and no latency to save. Fetching again when a view mounts is the
right default for a legal record book: what is on screen came from the database
just now. A general cache would turn dozens of visible reloads into dozens of
invisible invalidation rules, saving a cheap query at the cost of stale data
in regulatory records. That trade suits a typical web app, not this one.

**Reference data is the part worth sharing.** `lib/lookups.svelte.js` holds
the lists that can't change while the app runs (units, coded vocabularies,
closed lists from the model). It has **one** invalidation rule, and its owner
already existed: the catalogue refresh in Settings, the only thing that changes
those rows.

It was built for maintainability, not speed. Measured on the real backend, the
record book's first mount was limited by IPC, not rendering: each reference list
took 1–2 ms and returned a handful of rows, and rendering took about 20 ms even
with the CPU throttled 6×. What the module removed was each view fetching these
lists and passing them two levels down as props to reach a form. With it, the
book's mount went from 30 invokes to 10.

Things to keep when changing it:

- The lists are **loaded in `main.js` after the readiness gate**, not on first
  use. Loading them lazily made the first book mount *slower*, because the
  module fetches all of its lists while the view had only needed most of them.
- **Data the app itself edits stays out** (farms, plots, operators, advisors,
  products, materials), and so do lists scoped by country, which need an
  argument the calling view already has.
- A shared farm/season selection isn't needed: a season belongs to one farm, and
  the record book is a page opened from a list of them, so there is no
  selection to share.

**The rule: reference data may live at module level; records never do.** If
data starts changing under an open view (a live sync transport, say), the
answer is a thin invalidation channel (Rust sends an event, the frontend fetches
again), not a client-side cache.

---

## 4. Tailwind CSS, or plain CSS

The stylesheet is small and tidy. Nearly every colour goes through the `:root`
custom properties, and border-radius is effectively one value. Tailwind wouldn't
fix either of the two real problems:

1. **The token set was too thin** for owned controls: no scales for spacing,
   elevation, z-index or interaction states (hover/active/focus/disabled). One
   component was even using `var(--surface-hover, …)` when `--surface-hover`
   didn't exist, so the fallback was silently used. That is a token problem, not
   a problem with CSS itself.
2. **Component-scoped `<style>` blocks** had appeared against the old
   convention. They are the right tool for rules that only concern one
   component; the convention was what needed changing.

Tailwind would also move styling into the markup, in forms already full of
`{#each}`, `t()` and `bind:value`, and add PostCSS, a config and a class-sorting
plugin to a toolchain that already has Rust, Tauri, Android and Typst. What it
offers is a scale that takes an afternoon to write.

**So: plain CSS, with a fuller set of tokens.** `:root` has scales for spacing,
radius, elevation, z-index and interaction states (including
`--surface-hover`), and scoped `<style>` is allowed for rules that belong to one
component. The token list is in [frontend-conventions.md](frontend-conventions.md)
→ Styling. One thing to know when extending it: what used to be `#fff` is
**two** tokens, not one. `--surface` is a raised sheet that a dark theme would
repaint; `--on-primary` is text on a filled `--primary`/`--danger`, light in any
theme. Merging them would make a dark mode repaint the wrong half.

If a ready-made set of tokens is ever wanted, **Open Props** fits much better
than Tailwind: plain custom properties, no classes in markup, no build step, and
it can sit behind the existing tokens. Only reconsider Tailwind if the app
outgrows one stylesheet plus scoped blocks, which it is far from doing.

---

## 5. An API for third-party extensions

### What exists already

Modules are a fixed list in `src-tauri/src/registry.rs`, and the `Module` trait
is small: `name()`, `migrations()`, `backup_shape()`, `sync_shape()` and
`row_captions()`. Not every module uses every part (module-sigpac has no
migrations).

Next to it are registries that already work like plugins, driven by data:
`NAV_ITEMS` in `src/lib/nav.js`, `TILE_SOURCES` and `RESOURCE_BASES` in
`crates/terrazgo-geo/src/sources.rs`, the locale list in `src/i18n.js`, and the
best one, `MAP_LAYERS` in `src/lib/mapLayers.js`, which has a documented entry
contract. If extensibility is ever wanted, that is the model to copy.

### Why a runtime API for third parties is the wrong goal

- **Commands have no permission layer.** `src-tauri/capabilities/default.json`
  notes that app-defined commands aren't ACL-gated, so every command, backup
  import included (which replaces the whole database), can be called by
  anything running in the webview. A JavaScript extension model would give a
  third party the whole regulatory dataset with no permissions, and building
  that layer is a project of its own.
- **The change log couldn't say who wrote it.** `record_change` records an
  actor. If an extension writes a `treatment_record`, there is no honest answer
  to who wrote it, in the reviews or in a record's history, and the book is a
  legal document.
- **The licence already decides what the ecosystem looks like.** An extension
  that links the app's internals is a derivative work under the AGPL, so the
  realistic ecosystem is contributors, not a market of proprietary plugins. The
  compile-time crate path suits contributors fine.
- **Loading code at runtime is not supported, and the options are poor.**
  There is no `libloading`, `wasmtime`, `extism`, `mlua` or `rhai` in the
  workspace. Native plugins mean Rust's unstable ABI, so host and plugin must
  share the exact toolchain and dependencies, and the mobile platforms restrict
  loading executable code. WASM gives a stable sandboxed ABI but needs a host
  interface designed for every capability, and does nothing for the UI, since
  views are components compiled into the bundle.

### What matters instead: adding our own modules easily

What a third party would struggle with is what every new module struggles with,
and several modules are still to be built. So the work went into making that
easier, with no plugin machinery:

1. **Commands are split by domain.** `commands.rs` is a small boundary file and
   each domain has its own file (`app`, `core`, `phytosanitary`,
   `fertilisation`, `geo`, `sigpac`, `recordbook`, …). The parent re-exports
   each one with a glob, so the `commands::<name>` paths in `generate_handler!`
   don't depend on the file, and moving a command to another domain isn't an
   API change. `reconcile_alerts` runs after writes in several domains, and
   modules never call each other, so calling module-phytosanitary's alert
   engine is the shell's job; it sits with the locks.
2. **The backup and sync shapes are on the `Module` trait, with no default.**
   An empty default would let a module with tables forget to declare them and
   still compile. module-sigpac returns `&[]` explicitly, and the compiler asks
   every new module.
3. **Assertions are derived, not hardcoded.** The composed-migration step count
   and the i18n contract's crate list come from the registry and the workspace
   members. The i18n test reads the workspace manifest's member list and checks
   that every crate on disk is in it, so neither side can drift.
4. **Routes are data.** `lib/routes.js` says what each route renders; `nav.js`
   (framework-agnostic, can't import components) says what the navigation
   offers. The lists really are different: `#/farms/<id>` is a route with no
   nav entry.
5. **i18n dictionaries are split by area** and merged when loaded. Keys are
   already namespaced by prefix. A key defined in two files would be silently
   overwritten by the merge, so a test checks for that.

Three points stay written by hand on purpose. The `registered_modules()` line
*is* the seam; removing it would mean linker-section discovery (`inventory`,
`linkme`), which is fragile with the static linking mobile uses. `classify()`
needs one line per module. And each command goes in `generate_handler!`: the
alternatives are a chain of recursive macros or a build script that searches
Rust with a regex, and `command_registration.rs` already checks both
directions, so nothing can drift.

---

## Still open

Rust-side raster import and tiling (§1). It needs a storage design before any
code.
