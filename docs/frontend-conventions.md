# Frontend conventions — Svelte 5 + plain JS

> How the `src/` frontend is written and extended — the working reference.
> The architectural rationale is in [architecture.md](architecture.md) →
> "The frontend in one page".

## The two-tier rule

The frontend has a framework-agnostic core that must stay free of Svelte
imports, and views that may use anything Svelte offers:

| Tier | Files | May import Svelte? |
|---|---|---|
| Framework-agnostic | `i18n.js`, `i18n/<locale>/*.js`, `lib/backend.js`, `lib/nav.js`, `lib/mapLayers.js`, `lib/registryHints.js`, `lib/dateValue.js`, `lib/numberValue.js`, `lib/collate.js`, `lib/selectItems.js`, `lib/formValidation.js`, `lib/tabOverflow.js` | **No** |
| Reactive glue | `lib/notifications.svelte.js`, `lib/lookups.svelte.js` (runes modules) | Runes only |
| Views + wiring | `App.svelte`, `lib/*View.svelte`, `lib/*Form.svelte`, `lib/routes.js`, `lib/icons.js` | Yes |

`lib/routes.js` is in the view tier deliberately: it names components, so it
imports Svelte; `lib/formRefusal.js` is there for the same reason, importing
`getContext`/`setContext`; `lib/icons.js` for a third — it resolves the icon
names `nav.js` carries into Lucide components — and `lib/columnResize.js` for a
fourth, since its drag handle became a component and it now imports `mount`.
Its pure half, `lib/columnWidths.js`, stayed agnostic and stayed tested. `nav.js` stays agnostic and says what the navigation *offers*;
`routes.js` says what each route *renders*, and the two lists differ —
`#/farms/<id>` is a route with no nav entry.

The point: business logic lives in Rust behind `invoke`, and the agnostic tier
survives a future framework swap untouched — only views would be rewritten.

**The same line decides what is unit-tested.** `npm test` runs vitest over
`src/**/*.test.js` against the agnostic tier only, which is why
`vitest.config.js` needs no Svelte plugin — a module that would need a component
rendered is, by this table, a view. Views stay manually verified by the scripted
checks. Modules that read `localStorage`/`navigator` at import — `i18n.js` — are
stubbed by the tests that need them, which keeps the suite on the `node`
environment and free of a DOM dependency.

**This is the general JS test tier, not a numbers thing.** Covered today:
`numberValue.js`, `i18n.js` (the display side), `dateValue.js`, `collate.js`,
`selectItems.js`, `nav.js`, `formValidation.js` and `tabOverflow.js` — the second-to-last of which takes
anything that iterates like `form.elements` and returns plain objects, so its
tests build the fixtures by hand and the suite stays on `node` with no DOM. Uncovered and fair game: `backend.js`'s
`errorText`, `mapLayers.js`, `registryHints.js`, `treatmentDraft.js`.

**Test files sit beside the module they test** (`lib/numberValue.test.js`), the
JS convention, and not in a `tests/` folder like the Rust side — the test
travels with its module when one is renamed, and the import path stays `./x.js`.
They are not bundled: nothing imports them, so `dist/` never sees them. They ARE
scanned for SPDX headers and neutral voice like any other source file. The one
exemption is `number_formatting.rs`'s ban list, because a test **names** what it
guards — `numberValue.test.js` asserts that `1,5kg` is refused, and a suite for
that rule would have to write `type="number"` out in full.

## Svelte 5 idioms in use

- **Runes everywhere**: `$state`, `$derived`, `$props`. No stores, no legacy
  `export let`, no `$:` statements, no `createEventDispatcher` — child
  components receive **callback props** (`onSaved`, `onCancel`) instead of
  emitting events.
- **No state-management library either** (reviewed 2026-08-12,
  [stack-choices.md](stack-choices.md) §3): runes already are a fine-grained
  observable graph, so a MobX-style layer would be a second reactivity system
  fighting the first. Shared state, when it is warranted, is module-level
  `$state` in a `.svelte.js` file — `lib/notifications.svelte.js` is the
  pattern.
- **Reference data may live module-level; records never do.** Built 2026-08-13
  as `lib/lookups.svelte.js`: 26 argument-free lists (units, coded
  vocabularies, the model's closed lists) fetched once per session and read as
  `lookups.units` rather than passed as props. `loadLookups()` is warmed in
  `main.js` after the readiness gate and awaited by any view that needs the
  data; `invalidateLookups()` has exactly one caller, the catalogue refresh in
  Settings, which is the only thing that can change these rows.
  **Three things stay out**, and the boundaries are the point: regulatory
  records (refetch on mount — the backend is in-process SQLite, so the query is
  cheap, and a cache would trade that for silently-stale rows in a legal
  document); **user data the app itself edits** (farms, plots, operators,
  advisors, products, materials — caching those buys one invalidation rule per
  mutating command); and **country-scoped lists** (`list_problem_codes`,
  `list_growth_stages`, `list_crop_species`…), which need a key and whose
  caller already knows the argument.
  Measured before and after on the real backend: the record book's cold mount
  went from **30 invokes to 10**, and the 20 that left were being drilled two
  component levels deep. The win is the drill, not the milliseconds — each of
  those lists costs 1-2 ms and returns 3-10 rows. The point at which
  refetch-on-mount stops being sufficient is **sync**, and what answers it then
  is an invalidation event from Rust, not a client cache.
- **Dynamic lists** mutate `$state` arrays in place (`rows.push(...)`,
  `rows.splice(i, 1)`) and key each block on object identity:
  `{#each rows as row (row)}`.
- **Locale switching**: `App.svelte` wraps the routed content in
  `{#key localeVersion}` and bumps the key on `onLocaleChange`, so every `t()`
  call re-evaluates by remount. Components never subscribe to locale
  themselves.
- **Routing** is a hand-rolled hash router: `App.svelte` tracks the hash and
  `lib/routes.js` maps it to a view. The table is **data** (2026-08-13) — each
  entry is a `match(hash)` returning the view's props or null, first match
  wins, and anything unmatched renders the status view. Order is load-bearing:
  `#/farms/<id>` must precede `#/farms`, and prefix routes (`#/map`, which
  carries a query) follow the exact routes they would swallow.
- **Navigation is data**: top-level destinations live in `lib/nav.js`
  (`NAV_ITEMS`: route, i18n label key, SVG icon path) and `App.svelte` renders
  that list twice — as the collapsible sidebar on wide screens and as the
  bottom tab bar on narrow ones (media query at 700px; there is no desktop
  menu bar). `activeRoute(hash)` picks the highlighted entry by longest
  route prefix. Adding a view = one `NAV_ITEMS` entry + one `routes.js` entry;
  never hardcode a nav link in markup. The collapsed-sidebar state persists
  in `localStorage` (`terrazgo.sidebar`), like the locale. Per-view actions
  (buttons) belong in the view itself, not in a global toolbar.
- **Forms**: `<TzForm onsubmit={fn}>`, never a bare `<form>`
  (`form_feedback.rs` refuses one). The handler is a plain `async` function
  taking no event: TzForm gates it on the form's own validity and catches a
  refusal, so neither `event.preventDefault()` nor `run()` belongs in it. It
  reports a refusal by THROWING. `required`/`min`/`max` still declare the
  first-line validation, on the owned controls; the form is still the source of
  truth on save (full-state payloads, not diffs).
- **The view that opens a form owns the form's state.** Every register view
  keeps its fields and fills them in a `showForm(detail = null)` — blank for a
  new entry, from the stored record for a correction. A form split into its own
  component for size (`TreatmentForm.svelte`) does not change that: the view
  passes the draft object down (`src/lib/treatmentDraft.js` holds the shape) and
  the component binds to `draft.*`, holding no copy. **A form must never take
  the record as a prop and copy it into local state at creation** — a component
  keeps its initial values for its whole life, so setting the prop again leaves
  the previous record on screen while the id underneath has moved, and Svelte
  says so with `state_referenced_locally`. Treat that warning as the design
  smell it is; do not silence it with `untrack()` or `svelte-ignore`.
- **Command calls** go through `run(async () => ...)` from
  `lib/notifications.svelte.js`: a boundary error becomes a red notification
  (rendered through `errorText`: localized `error.<code>` key; the `internal`
  code gets the localized `error.internal_intro` line + the raw developer
  message; unknown codes fall back raw) and the bell panel opens itself so
  the failure is seen. **A form's OWN save does not go through `run()`** — see
  the Forms bullet: its failure belongs in the form, not the bell. `run()` is
  for everything else a view calls, which is most of it: the load at mount, the
  map, exports, the catalogue refresh, and every delete.

  Success feedback is pushed with `notify(t("message.…"))`, **but only when it
  says something the screen does not** (2026-09-01). A `<thing>_saved` beside a
  list that just refreshed under a form that just closed is noise, and noise is
  what makes a farmer stop reading the bell. What earns a notification is a
  derived value (a new treatment's plazo de seguridad), a path, a count, or an
  outcome with nothing on screen to show it — which is also why
  `sigpac_boundary_saved` stays: FarmView draws no map, so nothing there says
  the geometry arrived.
  Notifications accumulate in the bell (`NotificationBell.svelte`, one
  instance per layout) until dismissed individually or cleared; a locale
  switch clears them all (they hold interpolated text in the old language).

## i18n rules

- Never hardcode a user-facing string in markup or JS. Add a key to **every**
  locale — the i18n contract test (`src-tauri/tests/i18n_contract.rs`) reads
  them all, so it fails the build on divergent key sets or mismatched
  `{placeholders}` in any locale, including ones added later.
- **Each locale is a directory of area files** (2026-08-13): `src/i18n/es/`
  holds `common`, `errors`, `farm`, `book`, `fertilisation`, `ecoscheme`,
  `map`, `settings` and `external`, and `src/i18n/es.js` is the entry point that merges them and
  holds no entries of its own. A new module adds one file per locale instead of
  editing three thousand-line dictionaries. **A key may live in exactly one area
  file per locale** — the merge would silently prefer the last one — and a
  contract test refuses duplicates.
- `t(key, params)` for normal strings; `tCode(prefix, code)` for schema codes
  (`tCode("unit", "l_ha")` → key `unit.l_ha`) — falls back to the raw code so a
  new schema value degrades gracefully; `formatDate(iso)` for `YYYY-MM-DD`
  values (parses field-by-field to avoid UTC-midnight off-by-one).
- **A number in a sentence inflects the words around it: pass `count`.** `t()`
  reads `params.count`, picks a CLDR category with `Intl.PluralRules`, and looks
  up `<key>.one` / `<key>.other` before the bare key — so "1 días" and "Se han
  añadido 1 líneas" cannot be written. **The contract test enforces the
  structural half**: a `{count}` placed straight before a word must have plural
  forms, because that word agrees with it. Rules that follow from it:
  - **One count per key.** Selection keys on a single number, so a sentence
    whose nouns inflect on two different counts cannot be served by it. Write
    those in the "Label: N" form instead (`"Catálogos: {count} · Códigos:
    {codes}"`), which is grammatical at any figure. Reach for ICU
    MessageFormat only if a string genuinely needs two inflecting counts —
    never merely because a language was added, since `Intl.PluralRules` already
    covers every CLDR category up to Arabic's six.
  - **A variant may leave the count implicit** — "Se ha añadido **una** línea"
    beats "1 línea". `count` is the one placeholder the contract test forgives
    across variants; every other one must still match in all of them.
  - **A key with no variants ignores `count`** and resolves to its bare string,
    so passing it is always safe.
  - **A number that sits before a word without inflecting it is not a `count`**
    — "…y {n} más" — so name it something else and do not pass `count`. That is
    what keeps the test above free of an exemption list.
  - `tCode(prefix, code, count)` for the few codes that are counted **nouns**
    rather than symbols — the intensity units, "1 trampa" beside "2 trampas",
    where "1 l/ha" and "2 l/ha" are the same word. Called without a count (a
    picker listing the code) it resolves to `other`, the citation form.
  - The printed record book answers the same question in Rust with a two-form
    `Plural` type (`crates/terrazgo-recordbook/src/labels.rs`), because its
    language set is Spain's official languages and every one of them is
    two-form. One concept, two containers.
- **Every displayed number goes through `formatNumber(value, digits?)`**, never
  into markup raw and never through `toFixed` — which emits a decimal point in
  every locale, with no way to ask it for another. The decimal separator is a
  comma in Castilian and Catalan and a point in English, so a hand-built number
  is wrong in whichever language it was not written for. Siblings for the cases
  a bare number does not cover: `formatPercent(0–100)`, which also gets the
  space before the sign that Castilian and Catalan want and English does not;
  `formatCoordinates(lat, lon)`, joined by `" / "` because a comma between two
  decimal-comma numbers reads as four numbers; and `formatUnit(value, unit)` for
  quantities in the app's *own* units (cache sizes), never for the farmer's —
  regulatory unit symbols stay on `tCode("unit", …)` and print verbatim.
  - **The two defaults are an app convention: four decimals, no grouping.**
    They hold `formatNumber` to the same PRECISION as the book's
    `format_number` (`crates/terrazgo-recordbook/src/lib.rs`), so the two never
    show a different figure — only, at most, a different separator, since the
    book prints in the holding's language and the screen in the reader's. Two
    decimals would round a dose of
    0,0375 l/ha to "0,04" — a regulatory value silently restated. Grouping is
    off because the printed book has no thousands separator, and leaving it on
    made the two co-official languages disagree with each other: CLDR gives
    Castilian `minimumGroupingDigits=2` and Catalan 1, so 1234,5 grouped in
    Catalan only.
  - **No decree states a precision**, so four is not a rule — it is enough
    because the units already scale (a dose is written in g/ha, not kg/ha).
    Treat it as a display default, never as permission to refuse a figure the
    farmer measured.
  - **Money follows the reader for its CONVENTION and the record for its
    CURRENCY.** No money field exists yet (Costs is unstarted), but the rule is
    written beside the formatters because a region setting makes it easy to get
    wrong: pass `currency` explicitly from the data, never let it be inferred
    from the locale, or a Spanish holding's costs print as dollars for a reader
    whose machine is set to the US. What may vary is only the shape around the
    figure — es-ES writes "1.234,56 €", en-US "€1,234.56", both naming euros.
    No currency *setting* is needed: currency is data, not a preference. The
    input stays a plain `NumberInput` with the symbol as a label; nobody types a
    currency sign. Money is also the case that will want a fixed **scale** —
    two decimals, always — which the control does not carry today because
    nothing needs one, and no decree bounds a register's precision.
  - **A nonzero measurement is NEVER rendered as "0".** A value too small for
    four decimals falls back to significant digits, in `formatNumber` and in the
    book's `format_number` alike. Rounding 0,00003 into "0" would put a figure
    in front of an inspector that nobody wrote — the same falsehood a blank cell
    exists to avoid. Coordinates keep their own five-decimal formatter.
  - **A nullish value formats as blank, not "0"** — `Intl` coerces `null` to
    zero, and a printed 0 is a statement the farmer never made.
  - **`count` stays the raw number.** `Intl.PluralRules` selects on a number, so
    a formatted string would break inflection; integers render identically under
    this policy anyway, grouping being off.
  - **A value bound to an input stays raw.** Formatting is for what is read, not
    for what is edited — a numeric field parses what it is given.
  - A contract test (`src-tauri/tests/number_formatting.rs`) bans `toFixed` and
    the bare `toLocale*String` family outside `i18n.js`, and pins the two
    defaults. It cannot see a raw number interpolated into markup —
    `{record.dose_value}` and `{record.notes}` are the same syntax — so that
    half is a review job, which is why it is written here.
- **Two tags, because formatting and language are different questions**
  (2026-08-28). `formatTag()` is what NUMBERS and DATES render under and what
  the owned date/time/number controls parse with — so a figure the app shows and
  a figure it lets you type can never disagree. `languageTag()` is what the app's
  own WORDS resolve under: `Intl.PluralRules` and `Intl.Collator`. Applying a
  Polish machine's plural rules to Spanish strings, or its collation to Spanish
  farm names, is the bug the split prevents; collation staying on the language is
  also what keeps the screen agreeing with the book's Rust collator.
  `localeTag()` is gone: every caller was rendering a value, so they all became
  `formatTag`, and a name meaning "the locale" would now hide which of the two
  a reader is looking at.
- **`formatTag()` follows a per-device setting, defaulting to the machine.**
  `terrazgo.format` in `localStorage` beside `terrazgo.locale`, two values:
  `"system"` (the default — the OS regional format, resolved through
  `Intl.NumberFormat().resolvedOptions().locale` rather than guessed from
  `navigator.language`, since the two can differ) and `"language"`. Settings
  offers both with a live sample of each, because "system" means nothing without
  the figure it produces. Nothing is written until the farmer chooses.
  **The printed book is unaffected either way** — it renders in the holding's
  report language, which is a different axis again.
- **`languageTag()` maps through**
  `FORMAT_LOCALE`, and the entry that matters is **`en` → `en-GB`**: bare `en`
  resolves to US in `Intl`, so English would print `08/03/2026` where every other
  locale in the app prints `03/08/2026`, and English here means European English.
  `formatDate` and the owned fields read the same function, so a date the app
  renders and a date it lets you edit can never disagree.
- **Ordering display lists goes through `lib/collate.js`**, not `.sort()`. SQL
  returns BINARY order (`Á` is U+00C1, so "Ángel" lands after "Zubiri"; "Parcela
  10" before "Parcela 2"). The module mirrors
  `crates/terrazgo-recordbook/src/collate.rs` — same CLDR data, `numeric: true`,
  accents distinguished — so the RULES match. Which language's rules apply does
  not always: the book sorts in its report language, the screen in the one being
  read. Castilian files `ñ` after `n` (*Peña* after *Penz*) where Catalan and
  English fold it beside `n`, so an English reader of a Castilian book gets a
  different order on screen than on the printout.
- User-entered data (farm names, species, notes) is never translated.
- Adding a language = one `SUPPORTED` entry in `i18n.js` + one directory with
  the same area files and the full key set + its row in `required_categories`
  in the contract test, which is what stops it arriving with half its plural
  forms missing.

## Talking to Rust

- `invoke` comes from `lib/backend.js` (re-exported from `window.__TAURI__` —
  `withGlobalTauri: true`, no `@tauri-apps/api` npm dependency).
- Tauri exposes snake_case Rust command arguments as **camelCase** invoke keys:
  Rust `farm_id: String` ⇒ `invoke("list_plots", { farmId })`. Struct payloads
  (`NewFarm`, `NewTreatmentRecord`, …) keep their **snake_case** field names —
  they are deserialized by serde, not by Tauri's argument mapping.
- Optional fields: send `null`, not `undefined`; normalise empty inputs with
  `value.trim() || null` before building the payload.
- Plugins are invoked over the same transport:
  `invoke("plugin:dialog|save", { options: {...} })`.

## Navigation, feedback and confirmations

Three shapes settled early, together, because they answer the same question:
how a non-technical occasional user on a phone finds a function and learns
what happened.

- **Navigation is responsive and data-driven** (2026-07-03): a collapsible
  sidebar on wide screens, a bottom tab bar on narrow ones, and no desktop
  menu bar or global toolbar. Menu bars have no mobile equivalent and hide
  functions behind memorized locations, which is wrong for this audience; a
  top nav or tab bar caps at roughly five items where the sidebar scales with
  the module roadmap. Destinations live as data in `lib/nav.js` and are
  rendered by both layouts, so adding a screen is a one-entry change and the
  two layouts cannot drift. `lib/routes.js` is a *separate* table: one says
  what navigation offers, the other what a route renders. The sidebar defaults
  to expanded labels (the icon-only rail is opt-in and persisted in
  `localStorage`), and app-level entries sink to its foot via `foot: true`.
  Per-view actions stay in the view.
- **A form's problems belong to the form; everything else is the bell**
  (2026-09-01, narrowing the 2026-07-03 rule below). A form reports in two
  places at once, both drawn by one pass: a `.validation-summary` at its top
  listing every problem in the order the fields appear, and the offending
  field's own `.tz-field-error` underneath it. The summary entry names the
  field and focuses it when clicked, which is what a register several screens
  tall needs. A backend refusal lands in the same summary, and in the field too
  where the form's `anchors` map names one.

  **The mechanism is that every problem is a message on a form control**, so
  there is no second error state to keep in step: TzForm carries `novalidate`
  and calls `checkValidity()` itself, which fires `invalid` at each failing
  control (drawing the inline messages) and leaves the list readable off
  `form.elements` (drawing the summary). Suppressing the browser's own bubble is
  the point — it paints one problem, in the OS language.
- **Command feedback is the notification bell** (2026-07-03), not an inline
  message line: messages accumulate until dismissed, so a farmer can review
  what happened after the fact. **Errors open the panel themselves** —
  feedback for a failed command must not hide behind a badge — while successes
  only tick it. One bell instance per layout sharing state
  (`lib/notifications.svelte.js`); a locale switch clears the items, since
  they hold interpolated text in the old language. **Two things are NOT this**:
  a form's own validation and refusal, which the bullet above owns; and domain
  alerts (PHI, licence, ITV), which live on the Status view — the bell is
  transport for command feedback rather than a second alert system.
- **Destructive actions confirm through `confirmDialog()`** in
  `lib/backend.js`, which calls `plugin:dialog|message` with
  `buttons: "OkCancel"` and treats the result `"Ok"` as confirmation.
  `window.confirm` is banned: blocking JS dialogs are not reliably supported
  by the mobile webviews. The confirmation goes *inside* the `run()` async
  block.

  **The lesson that outlives the API**: tauri-plugin-dialog 2.7.0 removed the
  `confirm`/`ask` IPC commands (merging them into `message`), and a lockfile
  update broke every delete confirmation with a raw English error. Raw
  `withGlobalTauri` invokes track a plugin's IPC surface with no wrapper and
  no compiler safety net, so **a manifest pin must floor the minor that
  defines the shape being invoked** (`tauri-plugin-dialog = "2.7"`).

### The settings screen: a contents list beside one scrolling document (2026-09-04)

Settings is the one screen with a navigation of its own — a tree of its
sections down the left, and a search field above the settings themselves. The
settings stay **one document that scrolls as a whole**: the tree scrolls to a
heading and the search narrows what is on screen.

**The tree is navigational, so collapsing a section hides its entry in the list
and never the setting itself** — the document below is untouched. Sections open
by default, and a search opens all of them regardless, because a hit counted
beside a heading the reader cannot see points at nothing; the collapsed state
is left alone while filtering, so clearing the field puts the tree back.

**The search field is a `.tz-control` box, not a `TextInput`.** The hit count
and the clear button belong inside the field, and that box is the app's idiom
for a control holding an input plus adornments — `TzCombobox`'s input sits in
one the same way. Nothing here is a form field: no validity, no submission,
nothing stored, so the owned-control plumbing has nothing to do.

**Structure is declared once, in `lib/settingsTree.js`** — sections, the groups
inside them, and for each setting the i18n **keys** it is findable by. The view
and the tree render from it, the `nav.js`/`icons.js` arrangement: a contents
list that can disagree with what it lists is worse than none. Matching goes
through `collate.js`'s `foldTokens`/`matchRank`, so the settings search and the
catalogue pickers cannot grow two ideas of what a match is.

**A node's searchable text is declared, not scraped.** Reading the rendered DOM
would index whatever happened to be on screen, so a setting inside a closed
panel would stop being findable. `settingsTree.test.js` pins every declared key
against the real Spanish dictionary — a stale key would silently search its own
name.

**A container matching by its own text keeps all of its contents.** "carné" is
written in the alerts hint and in neither field's label; answering it with an
empty heading would be worse than not matching at all.

Four measurements, each of which cost a run:

- **An unstyled `<button>` turns WHITE on hover, and stating the colour once is
  not enough.** `button:hover:enabled` sets `color: var(--on-primary)` at 0,2,1,
  which beats a 0,2,0 base rule declaring the resting colour — so a tree node
  was legible until the pointer touched it. Any button re-skinned as something
  other than an action (`.toc-node`, `.toc-twisty`, `.tz-field-trigger`,
  `.pane-resizer`) needs its colour declared **in a hover rule of its own**, not
  only in its base rule.

- **A sticky element reports its STUCK position from `getBoundingClientRect`.**
  Every group heading was a `.view-head` band, so all of them measured 88px at
  once and the scroll spy stopped at the first, leaving the tree on the opening
  entry however far the reader scrolled. Group headings here are therefore
  **static** (`.settings-pane > .view-head`), and only the search band sticks —
  which costs nothing, because what a stuck band would have said, the tree
  beside it already says. **Never measure a sticky element for scroll position.**
- **`scrollIntoView` scrolls EVERY scrollable ancestor**, and `<main>` is one:
  `overflow: hidden` still permits a programmatic scroll (the same fact the
  app-shaped UI spike recorded from the other direction). The shell slid 319px
  up and stayed there, carrying the sticky band off the top of the window. Jump
  by scrolling the intended scroller explicitly — `viewEl.scrollTo` — never by
  asking an element to bring itself into view.
- **A clicked node has to be pinned until the reader scrolls for themselves.**
  The last sections can never reach the top of the scroller, because there is
  not enough document below them to push them there, so the spy overruled the
  click within the frame: clicking *Mantenimiento* lit *Perfiles*. The pin is
  released by the reader moving the pane rather than by a scroll event, since
  the smooth jump the click started fires plenty of those itself.

  **The two signals are `wheel` on the frame and `focusin` on the pane**, and
  the pair is narrower than it first looks. `touchmove` is unreachable — the
  tree is hidden below 700px, so nothing is ever pinned there. `keydown` on the
  frame is what Svelte's `a11y_no_static_element_interactions` flags, and
  correctly: a container that answers keys should be reachable. `focusin` is
  the honest signal anyway, because focus landing in the pane is both what
  scrolls it for a keyboard reader and the precondition for any key press
  reaching it. It goes on the **pane** and not the frame: the tree is inside the
  frame too, so a frame-level `focusin` would release the pin the click had just
  set.

**Below 700px the tree is gone** and the screen is the single stacked column it
has always been, plus the search band: `<main>` is the document scroller there,
and a second column has nowhere to go.

### The About panel's third-party attribution (2026-09-04)

Three tabs, and the third is the attribution the libraries we ship are owed.
Two data files feed it, and the split is the point:

- **`src/lib/thirdParty.js` is hand-written** — one row per PROJECT, because a
  reader recognises "Tauri" and not `tauri-plugin-geolocation`, and no machine
  can group that for us. It lists the direct dependencies of our own crates plus
  the npm packages whose code reaches the bundle; not the resolved graph (743
  crates), not the build and test tooling (never distributed, so never ours to
  attribute).
- **A third kind of row, `bundled`, exists because the dependency graph is not
  the binary.** SQLite's amalgamation is compiled in through rusqlite's
  `bundled` feature and the four Liberation Sans faces are embedded with
  `include_bytes!` — both ship, neither is a package, and both were missing from
  the list until 2026-09-04. A bundled row names the file its licence is read
  from and, where one exists, the crate whose version tracks it (SQLite's is
  libsqlite3-sys); the manifest and lockfile checks skip them, because nothing
  in cargo or npm will ever mention them.
- **`src/lib/thirdPartyLicences.json` is generated** by
  `npm run gen:licences` and committed. Committed because a build must not
  depend on the cargo registry being populated, and because an offline-first app
  has to be able to show its own attribution offline. It is lazy-loaded — 88 KB,
  fetched only when the tab is opened.

**Keeping it true is enforced, not remembered.** `cargo test` fails, by name and
with the fix in the message, when:

| What you did | What fails | What you do |
| --- | --- | --- |
| Added a dependency | `every_distributed_dependency_is_attributed` | add it to `thirdParty.js`, then `npm run gen:licences` |
| Removed one | `every_listed_package_still_exists` | drop its row |
| Brought in a new licence | `every_licence_has_an_allowlisted_link` | add the SPDX URL it names to `external_links.rs` |
| Edited the list, forgot to regenerate | `the_generated_licence_texts_cover_every_listed_package` | `npm run gen:licences` |
| **Upgraded** a dependency | `the_generated_licence_texts_were_read_from_the_installed_versions` | `npm run gen:licences` |

The last row is the one that is easy to leave out and the one that matters most.
The other four all trigger on the LIST changing, and an upgrade changes nothing
in the list — the package is still there, still covered, still linked, and the
text on screen is silently whatever the old version said. A licence file does
change with its package: a copyright year moves, a project re-licences. So the
generator records the version it read each text from and the test compares those
against `Cargo.lock` and `package-lock.json`, which is the cheap proxy for "this
file may have changed underneath us".

And the generator itself refuses, rather than emitting something incomplete,
when a package ships no licence text and none is vendored, or when a
dual-licensed row can only evidence one of its options.

**Grouped by licence, then by TEXT**, which is not the obvious design and is
forced by a measurement: Apache-2.0's body is boilerplate, but **MIT embeds its
copyright line**, so our packages have 16 different MIT texts. Grouping by
licence alone and showing one text — the intuitive reading, and what "they share
a licence so the text is the same" suggests — would attribute fifteen packages to
one wrong copyright holder. The bucket key ignores whitespace, because several
packages ship the same licence wrapped differently (301 "changed" lines between
two Apache-2.0 files that read identically).

**Three things are not automatable, and each failed silently before it was
caught.** Scraping the copyright line out of a licence file returns prose from
inside the Apache-2.0 body ("copyright notice that is included in or attached to
the work") for a third of the packages. Seven packages publish no licence text at
all, so `third-party/` holds copies taken by hand from each project's own
repository, with provenance in its README, and the generator **refuses** rather
than emitting a package with no notice. And a dual-licensed row needs a text for
*each* option — checking that any one matched hid that `jiff` publishes only its
MIT half.

**cargo-about remains the right tool for a different artifact**: a complete
transitive NOTICE at release time. It covers the crates and knows nothing about
the npm half, and its output is a document rather than panel data.

## Adding a command end-to-end (checklist)

1. Repository function in the owning crate + test alongside
   (`crates/*/tests/repository.rs`).
2. Thin `#[tauri::command]` wrapper in `src-tauri/src/commands/<domain>.rs`
   (one file per crate the commands wrap) — no logic, just `lock_conn` + repo
   call + `?`.
3. Register it in `generate_handler!` in `src-tauri/src/lib.rs`
   (`command_registration.rs` contract test fails otherwise).
4. If it can emit a new `Invalid("code")`, add `error.invalid.<code>` to every
   dictionary (i18n contract test fails otherwise).
5. If it changes alert inputs, call `refresh_alerts` before returning.
6. Frontend: a form's save goes in a `TzForm` handler (plain `async`, throws);
   anything else calls through `run()`. Push a `message.*` key via `notify()`
   on success **only if it says something the screen does not**.
7. If the new `Invalid("code")` names one field of one form, add it to that
   form's `anchors` so the refusal shows under the field as well as in the
   summary. Anchors are per-form: the same code names a different field in each.

## Styling

- One global stylesheet (`src/styles.css`), plain CSS, no preprocessor. Shared
  vocabulary belongs there; **rules that only one component can use belong in
  that component's `<style>` block** (10 of them today, `MapView` the largest
  at 158 lines, `AboutPanel` next at 116) — the earlier "no scoped blocks" line
  stopped matching the code and
  Svelte's scoping is the right tool for the local case.
- **Scoping raises specificity by one class, so a rule the global sheet
  overrides from an ancestor cannot be moved** (measured 2026-09-01, on the
  attempt that failed). `.notif-panel` was `0,1,0` and lost, correctly, to the
  narrow-screen `.topbar .notif-panel` at `0,2,0`. Moved into
  `NotificationBell.svelte` it compiles to `.notif-panel.svelte-hash` — also
  `0,2,0`, and emitted 13 kB later in the bundle, so it won the tie and took
  `width` and `right` with it: the panel stopped stretching edge-to-edge on a
  phone and overflowed the viewport, which is the exact failure the global
  rule's own comment describes. **Svelte does not warn** — the selector is
  used, it just wins something it did not before. So the test for a move is not
  "does one component use this class", it is **"does any rule outside that
  component target the same element"**; if one does, the pair is composition,
  and composition is shared vocabulary. That is what keeps the app-shell and
  bell rules (`.sidebar`, `.topbar`, `.tabbar`, `.main-head`,
  `.bell*`/`.notif*`) in the global sheet despite each having a single
  consumer — `.main-head .bell-wrap` and `.topbar .notif-panel` span two
  components and can be written from neither.
- **Anything on markup a library renders must stay global too**, for the reason
  the next bullet gives: `tz-calendar-*`, `tz-dialog-*` and `tz-trigger` sit on
  Bits UI elements. `.tz-check` stays for the first reason instead — TzCheckbox
  renders its own markup, but `.form-grid`'s two `:not()` exclusions name the
  class from outside.
- Measured 2026-09-01, so the split is a number rather than a feeling: of 1721
  lines, 490 were rules 2+ components use and 336 were element/id/at-rule only.
  Of the 631 that one component could use, ~180 sit on Bits UI markup and ~190
  were genuinely local and have now moved (`AboutPanel`, `Skeleton`,
  `StatusView`, `BookFertilisation`); the rest is the composition described
  above. **Splitting the sheet into partials was considered and rejected** —
  14 banner sections make it navigable, and the thing that caps its growth is
  this rule applied per change, not a directory.
- **A scoped rule cannot style markup another component renders.** Passing
  `class="…"` down as a prop hands the class to a child, but the parent's
  scoping hash never reaches that element, so the rule silently does nothing —
  Svelte says so as `Unused CSS selector`, which is worth treating as an error
  rather than noise. Pass a class the *global* sheet defines (`inline-field` is
  the usual one: label beside its control), or put the rule in `styles.css`.
- **Plain CSS stays, and the effort went into tokens instead** — reviewed
  2026-08-12 against Tailwind and similar, reasoning in
  [stack-choices.md](stack-choices.md) §4; the tokens landed 2026-08-13.
  `:root` now carries, beside the palette: `--surface` and `--on-primary` (a
  raised sheet and the text on a filled `--primary`/`--danger` — two different
  statements where the sheet used to write `#fff` eleven times), the
  interaction surfaces `--surface-hover`/`--surface-active` and
  `--disabled-opacity`, a six-step `--space-*` scale, four `--radius-*`, two
  `--shadow-*`, and `--z-popover`/`--z-sticky`.
  **Use a token where one fits; add a step rather than a one-off value.** The
  point is that a re-theme, a density change or a new floating element is a
  `:root` edit and a reading of a list, not a hunt through 800 lines.
  The handful of spacing values that sit *between* steps (`0.4rem`, `0.6rem`,
  `0.9rem`…) were left alone deliberately, because snapping them moves every
  button and card in the app — a design pass with screenshots, not a side effect
  of naming tokens. **`--focus-ring` and `--focus-ring-offset` arrived with the
  owned controls (2026-08-14)**, which is what the token was waiting for: they
  are focusable `<div>`s and `<button>`s that must draw focus themselves, and
  one `:focus-visible` rule now draws it for everything.
  Note there is **no global `box-sizing: border-box` reset** — it is applied per
  rule, five times — so any rule stating a height or width floor has to say
  which box it means.
- Reuse the existing vocabulary before inventing new classes: `.view`,
  `.view-head`, `.form-grid`, `.form-actions`, `.card-list`/`.card`(+`.stack`),
  `fieldset.es-only` (country-conditional sections), `fieldset.subsection`,
  `.btn-danger`/`.btn-cancel`. App-shell classes (`.sidebar`, `.topbar`,
  `.tabbar`, `.main-head`, `.bell*`/`.notif*`) belong to
  `App.svelte`/`NotificationBell.svelte` — views never use them.
- Icons are **[Lucide](https://lucide.dev) components** (`@lucide/svelte`, ISC),
  imported by name and stroked with `currentColor` — no icon font, no image
  files (CSP: `default-src 'self'`). Read out of the dist on 2026-09-03: the
  component sets SVG **attributes** and a class list and nothing else, so it
  injects no stylesheet and clears the standing rule for a new dependency
  ([stack-choices.md](stack-choices.md) §2). Size it in CSS — the `width`/
  `height` it renders are attributes, which any CSS length overrides.
  `nav.js` names an icon rather than holding one, because it may not import
  components; `lib/icons.js` is the view-tier half that resolves the name.
- Production CSP is `default-src 'self'`: no inline styles/scripts, no CDN
  anything. The dev-only CSP additions (`devCsp`) exist solely for Vite HMR.
  **Measured 2026-08-12, because "no inline styles" is coarser than what the
  policy actually does** (there is no `style-src`, so `style-src-attr` and
  `style-src-elem` both fall back to `'self'`): `setAttribute('style', …)` and a
  runtime-injected `<style>` element are **blocked**, while every CSSOM write —
  `el.style.prop`, `Object.assign(el.style, …)`, `el.style.cssText` — is
  **honoured**. A dynamic `style=` **binding** is safe because Svelte compiles it
  to `cssText`; a **static `style=` attribute is blocked** (corrected
  2026-08-15 — the first pass said otherwise, and `Skeleton.svelte` had been
  losing its bar widths on Android ever since. Write a class.). The rule that
  matters for a new dependency is **no runtime stylesheet injection**, and the
  rule for our own markup is **no literal `style=`**. Every `invoke` also raises
  a `connect-src` violation event for `ipc://localhost/<command>` while working
  normally — expected noise; anything else in the console on Android is worth
  chasing, since that is how both of these were found.

### The UI typeface (2026-09-04)

The app sets in **IBM Plex Sans**, self-hosted as **one variable woff2** at
`src/fonts/IBMPlexSansVar-Roman-subset.woff2` (102 KB). Self-hosting is not a
preference: the production CSP is `default-src 'self'` and the app has to
render identically with no network.

**The measured property that decided it is not about looks:** its digits are
**tabular by construction** — the font has no `tnum` feature because it has no
proportional set to switch away from — so every column of doses and dates lines
up with no CSS.

**The app switches no OpenType feature on.** `liga` and `kern` are on by
default, this face has no `calt`, and the slashed zero is off deliberately:
`zero` was switched on across tables and fields for a while, on the argument
that a transcribed registration number wants 0 and O told apart, and the
preference went the other way — a plain zero reads better, and Plex already
separates the two on shape, a narrow oval against a round O. The feature is
still asserted by `check-font.py`, as a canary rather than because we use it.

Rules that are easy to get wrong:

- **Drive weight through `font-weight`, never `font-variation-settings`.** The
  `@font-face` states `font-weight: 100 700`, which lets the browser move the
  `wght` axis through the ordinary cascade, so every pre-existing rule keeps
  working. `font-variation-settings` bypasses that cascade; it is reserved for
  `wdth` (85–100%), which nothing uses yet.
- **`format("woff2")`, not `format("woff2-variations")`.** The latter is
  deprecated, and an engine that does not recognise it skips the source
  silently — the failure looks like a page that renders fine in system-ui.
- **A subset must pass `--layout-features='*'`.** Dropping features does not
  break rendering, it just quietly removes them: Google's own subset of this
  face keeps `tnum` and loses `zero`, which is how this was found.

**One variable file rather than three static ones** was an 8 KB decision, not a
free one: static subsets of the three weights in use (400/600/700) came to
94 KB against this file's 102 KB. The 8 KB buys any future weight and the
`wdth` axis. If it is ever spent, condense **prose** columns — narrowing a
registration number works against the reason the face was chosen.

**Which upstream, because there are three and only one is right.**
`@ibm/plex-sans-variable` is the vendor's own current package; `@ibm/plex` is
the retired monolith whose variable font is two years older; google/fonts
carries a later font version with a wider `wdth` floor but only as a raw file
on a branch. After subsetting, the IBM and Google builds differ by **8 bytes**,
so the pinned package version wins.

Two scripts keep it honest, both run by hand (they need
`sudo apt install fonttools`, which is a local tool and not a project
dependency — the committed `.woff2` is what the build consumes):
`scripts/subset-font.sh` re-cuts the file, and **`scripts/check-font.py`
asserts what survived** — the axes, the OpenType features, and every non-ASCII
character the frontend can put on screen, with comments stripped so a code
comment's Greek does not demand Greek in the font. That check is what found
four hardcoded glyphs the font does not carry (`✕`, `⚠`, `▰`), now Lucide
icons; `≡` turned out to be comments only. **A character that falls back is
invisible in review and obvious in a screenshot**, which is the whole reason
the check exists.

## Tooling

- **Prettier** formats JS/Svelte/JSON/CSS/HTML (`.prettierrc`: printWidth 100,
  `prettier-plugin-svelte`; markdown is excluded so hand-maintained doc
  tables stay hand-editable). `npm run format` to fix, `npm run format:check` is the CI
  gate.
- **ESLint 9** (flat config, `eslint.config.js`) with `eslint-plugin-svelte`
  catches defects — undefined globals, unused vars, Svelte misuse. Style is
  Prettier's job, so no stylistic ESLint rules. `npm run lint` is the CI gate.
- Destructive confirmations use `confirmDialog(message)` from
  `lib/backend.js` (native dialog via `plugin:dialog|message`, OkCancel) — never
  `window.confirm`, which mobile webviews don't reliably support. Call it
  inside the `run()` block: `if (!(await confirmDialog(...))) return;`.

## Owned controls (2026-08-14)

The app owns its form controls. Every native `<select>`, `<input type="date">`,
`<input type="time">` and `<input type="number">` is gone, and so is the last
hand-rolled one — replaced by components in `src/lib/`, most of them built on
[Bits UI](https://bits-ui.com/llms.txt) (headless Svelte 5 primitives —
behaviour, keyboard and ARIA, no styling of its own):

| Component | Replaces | Call sites |
|---|---|---|
| `DateInput.svelte` | `<input type="date">` | 19 |
| `TimeInput.svelte` | `<input type="time">` | 1 |
| `TzSelect.svelte` | `<select>` | 78 |
| `TzCombobox.svelte` | a filter box beside a `<select>` | 4 |
| `CataloguePicker.svelte` | its own hand-rolled input + `<ul>` | 3 |
| `TzDialog.svelte` (2026-08-26) | nothing — the app had no modal | 1 |
| `NumberInput.svelte` (2026-08-27) | `<input type="number">` | 54 |
| `TzCheckbox.svelte` (2026-09-01) | `<input type="checkbox">` | 17 |
| `TextInput.svelte` (2026-09-01) | `<input>` / `<input type="email">` | 103 |
| `TzForm.svelte` (2026-09-01) | `<form>` | 26 |
| `TzTabs.svelte` | a hand-rolled `role="tab"` strip | 6 |
| `TzTooltip.svelte` (2026-09-03) | the native `title` attribute | 11 |
| `TzMenu.svelte` (2026-09-03) | nothing — the app had no menu | 1 (`TzTabs`) |

**The reason is not that platforms look different.** It is that the native date
picker follows the **OS** locale and so overrides the language the holding chose
— on a field that appears in every register of the record book, in a project
whose `Labels`/`region.rs` design exists to honour exactly that choice. The
WebKitGTK input grab (a popover only a focus change released) is the second
defect, and owning the control is what the 2026-07-03 decision to ban page-side
workarounds always said the real fix would be.

**The numeric field is the same argument, except it CORRUPTS rather than merely
looking wrong.** `<input type="number">` parses with the OS locale too, and on a
mismatch it does not refuse — it reinterprets. Measured in the shipping
WebKitGTK webview on 2026-08-27, typing `1,5`:

| OS locale | native input stores | `NumberInput` stores |
|---|---|---|
| `es_ES` | `1.5` | `1.5` |
| `en_GB` | **`15`** | `1.5` |

So a farmer running the app in Castilian on an English-locale machine enters a
dose of 1,5 l/ha and records ten times that, in a register read at an
inspection, with no error and no empty field to notice.

**`lang` does not fix it, and that was measured too** — `lang="es"` on the
input, on an ancestor, and `document.documentElement.lang` (which `i18n.js`
already sets) all still parse `1,5` as 15. WebKit's number localizer reads the
application locale and ignores the attribute. Do not re-open this.

`NumberInput` is therefore a plain `<input type="text" inputmode="decimal">`
with the parsing in `lib/numberValue.js` — Bits UI ships no number field.
Rules that follow:

- **Both separators are accepted as the decimal point**, because a farmer typing
  `1.5` on a keypad offering only a dot means one and a half in any language.
- **Grouped input is refused, visibly.** There is no thousands separator to
  parse because the app never renders one, so `1.234,5` gets a message rather
  than a guess — the whole difference from the native control's silent 15.
- **An unparseable entry blocks the form and stays on screen.** It publishes
  `""` upward and sets `setCustomValidity`, so it can never be submitted, and
  the text is left for the reader to correct rather than wiped.
- **`onchange` keeps the native event's meaning** — it fires when editing
  settles, not per keystroke, because the settings fields save straight to the
  backend from it.
- **`step` is not reproduced**; `integer` replaces `step="1"`. The other step
  values were spinner hints, and no decree makes a measured width a multiple of
  a centimetre.
- **`decimals` defaults to null — unbounded — and that is deliberate.** It was
  briefly bounded at 4 (the idea IMask spells `scale`, taken after reading its
  source) and that was wrong twice over: it refused the **five-decimal
  coordinates the book itself prints**, and its premise — that more could not be
  rendered honestly — no longer holds, because both renderers now widen rather
  than round a small value into a false "0". Set `decimals` where the **domain**
  has a scale, not where a renderer does: currency will want 2, and `integer` is
  the zero case with its own flag.

**The checkbox is owned for a DIFFERENT reason, and it is worth stating so the
table is not read as one argument.** Every control above corrects something: the
date and number fields parse with the OS locale, the select and the date popover
are platform-drawn. A checkbox does none of that — there is nothing it can
record wrongly. What was broken was the CSS, and it was broken by omission:
`label.inline` had no rule anywhere in the app and `label.check` only a scoped
one in `FarmView`, so seventeen call sites sat inside `.form-grid`, where
`.form-grid label` stacked each tick above its own text and `.form-grid input`
gave the box a text field's 2.25 rem height, border and inset. Section 6's
forty-one good practices — sentences out of the FEGA catalogue — are where that
became unreadable.

**So `TzCheckbox` is the second owned control that is NOT Bits UI**, the first
being `NumberInput`, and for the same kind of reason: the library ships nothing
this needs. Read out of bits-ui 2.18.1 on 2026-09-01, so nobody re-derives it —
`checkbox.svelte` renders `<button {...mergedProps}>` (its `type` prop defaults
to `"button"`), and `checkbox.svelte.js`'s
`shouldRender = Boolean(this.root.trueName)` renders the sr-only form input only
when `name` is set. Its `child` snippet hands you `role="checkbox"`,
`aria-checked` and a toggling `onclick`, so a real input placed there toggles
twice per click. Against that, a native `<input type="checkbox">` under
`appearance: none` keeps the role, the checked state, space-to-toggle, the label
as a click target and constraint validation, all correctly and for free;
`Checkbox.Group`'s array binding is what `bind:group` already does. Not a CSP
question either way — bits-ui's `HiddenInput` passes `style` as an object, but
Svelte routes a spread `style` through `set_style` → `dom.style.cssText`, the
CSSOM write the 2026-08-12 measurement recorded as honoured.

Two consequences of owning the element rather than wrapping a library's:

- **`element.click()` drives it**, unlike every control in the "Known gaps"
  note below — a scripted check can tick a box without synthesising pointer
  events.
- **A component cannot forward `bind:group`**, which is an `<input>`-only
  directive, so the three call sites that had one bind a `group` array prop
  instead (`value` names this box's entry). That is the only call-site change
  the migration was not a straight markup swap.

**The contract is a plain string, and that is what made the migration a markup
swap.** `DateInput` takes and returns `"YYYY-MM-DD"`, `TimeInput` `"HH:MM"`,
`TzSelect`/`TzCombobox` a code or an id, `NumberInput` a number — `""` for unset
in all of them. No view
logic moved; `CalendarDate`/`Time` objects live behind `lib/dateValue.js` and
never escape their component.

### Rules when adding one

- **Build items with `lib/selectItems.js`, and pick the right builder.** They
  differ in one decision: whether the list may be re-ordered. `codeItems(rows,
  prefix)` keeps the backend's order, because coded vocabularies carry meaning
  in it (licence levels run basic → pilot, BBCH stages 0-9, efficacy good →
  poor) and alphabetising them is a regression. `nameItems(rows, …)` collates,
  because entity lists arrive in SQL's BINARY order — which puts "Ángel" after
  "Zubiri".
- **Never render more than ~40 rows in a `TzSelect`.** An owned dropdown paints
  its rows in the webview instead of handing them to the OS, so rows rendered is
  the one real cost, and it is not specific to Bits UI. `TzSelect` carries a
  `console.warn` tripwire above the cap; a longer list belongs in `TzCombobox`,
  whose own input is the trigger so the list narrows before it renders.
- **A truncated list says so**: `form.list_truncated` renders
  "Mostrando 40 de 601 — siga escribiendo". Silence would hide from a farmer
  that the code they want exists below the cut, in a register where the code
  carries legal weight.
- **Matching folds; sorting does not.** `lib/collate.js` holds both, deliberately
  in one file: `fold()` is accent- and case-insensitive (uppercase, because it
  collapses more — "Straße" folds to STRASSE), `searchItems()` is token-AND on
  whitespace and ranked exact > starts-with > starts-a-word > anywhere. So
  "cali" finds CÁLIDO and ALCALI, and "olivo verde" finds "VERDE OLIVO".
  Ranking is what makes the cap safe: with 200 rows containing a query, unranked
  truncation is a coin toss. Sorting is the opposite and uses
  `Intl.Collator(… sensitivity: "variant")`, so "Pena" sorts before "Peña"
  rather than tying — mirroring `crates/terrazgo-recordbook/src/collate.rs`, so
  both sides apply the same rules to whichever language each is rendering.
- **A control states its problem with `setCustomValidity`, never with the
  `required` ATTRIBUTE** (2026-09-01; `form_feedback.rs` refuses the attribute
  on a validity proxy). Both block submit, so this looks like a free choice and
  is not: a bare `required` leaves `validationMessage` as the BROWSER's string,
  which follows the OS language — measured in headless Chrome, "Please fill in
  this field." while the app is in Castilian. That is the same defect that
  retired the native date picker, one layer down, and it went unnoticed at 23
  call sites precisely because nothing failed. `validationMessage` is what
  TzForm's summary reads, so it has to be ours.

  Derive ONE `error` string and let it drive both the inline `<small>` and
  `setCustomValidity`, so the field and the summary cannot disagree. Blocking is
  unaffected, also measured: a `customError` alone refuses a submit exactly as
  `required` did.
- **Constraint validation rides on a real (not `type="hidden"`) input parked
  off-screen as `.tz-validity`** — for the controls whose visible part is a
  `<div>`. Bits UI's own hidden input was not enough: it carries no `min`, and
  the app has exactly one range guard (`TreatmentForm.svelte`, end date not
  before application date). `TextInput`, `TzCheckbox` and `CataloguePicker` need
  no proxy, their visible element being a real input already.
- **A control takes `name` and puts it on whichever element carries the
  validity**, so `form.elements[name]` reaches it — that is how a form's
  `anchors` map hangs a backend refusal on the right field. It also forwards
  `label` as `data-tz-label`, which is what the summary names the field by.
  A backend refusal is rendered as a SECOND `.tz-field-error` line and never
  through `setCustomValidity`: a stale custom validity would refuse the next
  submit until something cleared it, wedging the form.
- **The popover is portalled to `<body>`** and every content element passes
  `preventScroll={false}` explicitly — see the forbidden list below for why that
  is not left to a default.
- **`TzDialog` opts out of *two* body-level layers, and it is the only control
  that has to.** bits-ui renders both `ScrollLock` and `TextSelectionLayer` from
  exactly the same two components — `dialog-content` and `alert-dialog-content`
  — so `Select`, `Combobox`, `DatePicker` and `TimeField` carry neither. The
  second layer sets an **inline** `user-select: text` on the content between
  pointerdown and pointerup, which defeats the app-wide
  `body { user-select: none }` and let a drag select the dialog's own title;
  `preventOverflowTextSelection={false}` turns it off, and the About panel opts
  its technical block back in with `user-select: text`, the exception
  `.notif-panel li span` already takes. **An inline style beats every selector,
  so neither of these could have been fixed in CSS without `!important`** — when
  a library writes through CSSOM, look for its prop before reaching for the
  stylesheet.

### The tab row measures itself, and what does not fit goes into a menu (2026-09-03)

`TzTabs` renders a **`.tabrow`**: the `.tabstrip` of tabs that fit, then — only
when something does not — a divider and a `TzMenu` labelled *More*. It is
GitHub's repo-nav shape, and it replaced `overflow-x: auto` on the strip, which
was the worst of both on the two screens that matter: a sideways scroller shows
nothing that says more tabs exist, and on a phone it fights the page's own
scrolling.

- **The split is arithmetic, and it lives in `lib/tabOverflow.js`** — the
  agnostic tier, so it is unit-tested. The component measures; `visibleTabCount`
  decides.
- **Tabs keep their order.** The menu never promotes the current tab into the
  strip: reordering a strip under the reader is a worse trade than the one thing
  it fixes. The button takes the selected marker (`.tab.is-current`) instead,
  and the row inside the menu is ticked.
- **The menu button is a SIBLING of the tab list, never a child.** A
  `role="tablist"` whose children are not all tabs is a broken ARIA contract.
  Inside, the rows are a `RadioGroup` — `role="menuitemradio"` with
  `aria-checked` — which is how a screen reader learns that the tab it cannot
  see in the strip is the one it is on.
- **Widths are cached, and the cache is self-healing.** A tab can only be
  measured while it is in the DOM, so `visibleTabCount` answers "all of them"
  whenever it has no width to go on; every pass where the row is wide enough to
  hold everything re-measures. That is what survives a font swap or a longer
  locale without a stale number.
- **`resplit()` runs two passes on purpose.** The overflow group is only in the
  DOM while something overflows, so the first pass after a row runs out of room
  measures it as 0 and keeps one tab too many; the second runs with the button
  on screen. It terminates because the inputs stop changing, not because it is
  capped.
- **The ResizeObserver watches the row, never the strip.** The split changes
  what is inside the strip, so observing it would feed the observer its own
  output.

Two measured traps, both silent:

- **`.tabbar` was already taken** — the shell's phone navigation is
  `<nav class="tabbar">`. Naming this row that made the two swap rules in *both*
  directions: the register strip came out 602px inside a 1000px frame, and the
  phone nav inherited a negative margin that scrolled the whole document
  sideways **at 1280px**, a width nobody thinks to check a phone bar at. The
  page-overflow assertion is what caught it.
- **`overflow-x: clip`, not `hidden`.** `hidden` on one axis computes the other
  to `auto`, which clips the global focus ring off the top and bottom of every
  tab. `clip` is the exception the spec allows to leave the other axis visible,
  and `overflow-clip-margin: 3px` then lets the last visible tab's ring finish.
- **A popover outranks the view's sticky bands.** `--z-popover` was 20 against
  `--z-sticky`'s 30 on the premise that "popovers open below sticky chrome",
  which stopped being true the moment one hung off the tab row: the menu's rows
  were painted over by a register's own `.view-head` button. It is 32 now —
  above the view's bands, still under `--z-shell`.

### Tooltips are owned too, and this one is owned for looks (2026-09-03)

`TzTooltip` replaces the native `title` attribute at all 11 sites. A `title` is
drawn by the platform and has **no styling hook at all** — no selector, no
pseudo-element — so it follows the OS: a GTK chip under WebKitGTK, something
else on Android.

**Say plainly what is different about it.** Every other owned control replaced
something that was *wrong*: the date picker overrode the holding's chosen
language, `<input type="number">` corrupted a dose. A tooltip's colour is only
appearance. It is the first owned control taken for looks, and that is worth
knowing before the list grows on the same argument.

Four things it settled, each measured:

- **Bits UI rather than a CSS-only tip.** Several triggers sit inside clipping
  ancestors — `.tabstrip` clips, `.view.framed` clips, the inspector scrolls —
  and a tip parented to its trigger is cut off by every one. This portals.
- **`preventScroll` is absent on purpose**, uniquely: `tooltip-content.svelte`
  hardcodes `preventScroll={false}` itself, so the lock is never constructed.
- **`disableHoverableContent` is set, and it could not have been done in CSS.**
  bits-ui writes `pointer-events` inline off that flag, and an inline style beats
  a stylesheet rule — a `none` in `.tz-tooltip` computed to `auto`. A tip that
  swallows a click on the control beneath it is a defect.
- **Every trigger uses the `child` snippet.** The call sites are existing `<a>`,
  `<button>` and `<span>` elements, and a wrapping trigger button would nest a
  button in a button. The caller spreads `props` onto its own element; where it
  also needs one of the handlers bits-ui supplies, it CALLS the one from `props`
  rather than replacing it (`ColumnResizer` does this for `pointerdown`, which is
  what dismisses the tip as a drag begins — pointer capture means the leave event
  never arrives).

An empty `label` renders no tooltip and the bare trigger, which is what lets the
sidebar show one only while collapsed, and a zone chip only when it has a detail.

### Forbidden Bits UI components — and the one exception, with its evidence

The rule is **not** "these four components are broken". It is:

> **A view never imports bits-ui. An owned control may, and every one of them
> passes `preventScroll={false}` explicitly.**

That was already true of the code — every bits-ui import in `src/` sits inside
an owned wrapper, plus `BitsConfig` in `App.svelte` — and it is the whole of
what keeps the app safe under the production CSP.

**The defect, which is real.** bits-ui's body scroll lock *applies* through
CSSOM (`document.body.style.pointerEvents = "none"`, allowed) and *releases*
through `document.body.setAttribute("style", …)`, which `default-src 'self'`
blocks (`style-src-attr`, see Styling above). A lock that engages therefore
never lifts, leaving `pointer-events: none` on `<body>` for good: the app stops
responding to the mouse until it is restarted.

**Why it is a default and not a component.** Read out of bits-ui 2.18.1 on
2026-08-26:

- `document.body.setAttribute("style", …)` appears **exactly once in the whole
  dist** — `internal/body-scroll-lock.svelte.js`, inside `resetBodyStyle()`;
- `resetBodyStyle()` is reachable **only** from `BodyScrollLock`'s teardown;
- `new BodyScrollLock` appears **exactly once** —
  `utilities/scroll-lock/scroll-lock.svelte`, wrapped in `if (preventScroll)`;
- `<ScrollLock>` is rendered by **three** components — `dialog-content`,
  `alert-dialog-content`, and `utilities/popper-layer/popper-layer-inner.svelte`,
  which **every floating layer goes through**, menus included. The bullet here
  used to say "exactly two … (`DropdownMenu` and `ContextMenu` no longer render
  it at all)", and that was wrong; re-read 2026-09-03 when `TzMenu` needed it.
- popper-layer-inner resolves it as `preventScroll ?? true`, so **an unstated
  prop means the lock IS constructed**. `Select.Content` and `Combobox.Content`
  declare `preventScroll = false` as their own default and are safe unaided;
  `DropdownMenu.Content` declares no default, so stating it there is
  load-bearing rather than belt and braces.

So `preventScroll={false}` means the lock is never **constructed** and the
blocked line is unreachable — the same prop, guard and mechanism `TzSelect` and
`DateInput` have relied on since 2026-08-14. Here it also costs nothing: this
app sets `body { overflow: hidden }`, so there is no body scroll to lock, and
what the lock would otherwise buy — blocking interaction behind the panel —
comes from `Dialog.Overlay` and the focus trap, neither of which is affected.

**`TzDialog.svelte` is the one sanctioned `Dialog` import and `TzMenu.svelte`
the one sanctioned `DropdownMenu`**, each carrying a targeted
`eslint-disable-next-line no-restricted-imports` with its reasoning beside it.
`AlertDialog` and `ContextMenu` stay banned outright: nothing needs them, and an
unused escape hatch is one nobody re-derives the evidence for. `Menubar` is safe
(it passes `preventScroll={false}` itself).

Guard — every hit outside the two owned controls is a defect:

```
grep -rn "Dialog\.\|DropdownMenu\.\|ContextMenu\." src/ \
     --exclude=TzDialog.svelte --exclude=TzMenu.svelte
```

Destructive confirmations keep using `confirmDialog()` (the native dialog
plugin), which is unaffected — and remains the right tool for a yes/no question,
because it is the platform's own.

**Anything here must be re-proved under the production CSP, and only one build
does that**: neither verification tier applies a policy, and not even
`cargo build --release` does, because it still loads `devUrl`. See
[stack-choices.md](stack-choices.md) §2 → "Method note".

### `CataloguePicker` is not `TzCombobox`, and the difference is the contract

Both narrow a long list as you type; they answer different questions.
`TzCombobox` answers *which code?* — the typed text is a throwaway query and the
value is the code alone. `CataloguePicker` answers *what is this called?* — the
typed text **is** a stored value, the name the record book prints, and the code
is optional metadata derived from it. Free text is legal and must survive: a
farmer may grow something the catalogue does not list.

So the rule it exists to enforce is that **a name and a code must never
disagree**. Picking a row sets both; typing away from a picked row keeps the name
and drops the code; retyping the exact name restores it. Nine characterisation
checks pin exactly that, and they are the acceptance gate for any future change
to this component.

Two consequences worth knowing before editing it:

- **The text is driven through `bind:inputValue` on `Combobox.Root`.** Bits UI
  merges the input's `value` last, so it owns that attribute unconditionally and
  passing `value` through props does nothing; the root's two-way `inputValue`
  box is the supported way in. `clearOnDeselect` defaults to false and nothing
  reverts the text on close, which is what makes free text survive at all.
- **`Home` and `End` are reclaimed for the caret.** Bits UI treats them as list
  navigation (jump to first/last row) and `preventDefault`s them, which is right
  for `TzCombobox` and wrong here, where the input holds a name being edited.
  `mergeProps` runs our handler first and stops the chain once the event is
  defaultPrevented, so the component handles those two keys and moves the caret
  itself. `allowDeselect={false}` for the same class of reason: re-picking the
  row already chosen must keep it, not toggle it off and silently strip the code.

### Android acceptance (2026-08-14)

Android was the arc's stated criterion rather than its beneficiary: the native
select there is a full-screen thumb picker and the best control of the three
platforms, so an owned one had to *match* it, not merely replace it. Verified on
a Galaxy A22 (Android 13, WebView 151) — the controls were judged good in use,
and the mechanical half was measured by driving real touch events through the
device's own webview:

- `.tz-option` rows are exactly **44 px** under `@media (pointer: coarse)`
- the listbox caps at **256 px** tall and at the viewport width, with **no
  sideways page scroll** and the popover clear of the bottom tab bar
- tapping a row commits the value; the species picker narrows and announces
  "Showing 40 of 1023 — keep typing"

Two of those numbers only became true during that session: the row floor
(padding alone left a one-line row at 38 px) and the height cap (`.tz-listbox`
named only `--bits-select-*`, so on a combobox — which emits
`--bits-combobox-*` — both the width floor and the height cap were invalid at
computed-value time and silently dropped, leaving a 40-row list 1244 px tall).
Both are fixed; the lesson is that a headless library's own sizing defaults are
part of what you adopt, and they are invisible until a long list meets a narrow
screen.

### Costs, measured rather than estimated

Rows rendered is the cost that matters. Minified, gzipped, in a probe app; the
"open" column is pointerdown to painted, CPU-throttled 6× to stand in for a
phone:

| rows rendered | open @ 1× | open @ 6× |
| --- | --- | --- |
| 40 | 66 ms | 260 ms |
| 602 | 179 ms | 897 ms |
| 2000 | 384 ms | 2 146 ms |

The bundle grew ~65 kB gzip on the entry chunk (73.7 kB → 138.8 kB), against a
budget of 90 kB and a multi-MB installer. Two npm runtime dependencies joined
(`bits-ui`, `@internationalized/date`), the first since the map's three; CI has
gated `npm audit --audit-level=high` since 2026-08-13.

## Known gaps (as of 2026-07-02)

- **UI has no automated tests** — deliberate while the UI is in flux
  (testing strategy #5, architecture.md). Runtime verification is scripted
  though: a headless-Chrome harness over the built bundle (error-stub or
  backend-harvested fixtures) and an app-level harness driving the real
  debug binary in the real webview (screenshot via X11).
- **Driving an owned control needs real pointer events.** A synthetic
  `element.click()` neither opens a trigger nor picks a row, so a scripted check
  written that way passes while asserting nothing. The scripted-check rules live
  in both verifier skills under "Driving an owned control"; the short version is
  pointer events, scroll-settle-then-click (the popover is portalled and
  positioned from a measured anchor), and remember the first row opens already
  highlighted.
