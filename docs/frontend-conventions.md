# Frontend conventions: Svelte 5 + plain JS

How the `src/` frontend is written and extended. The architectural reasons are
in [architecture.md](architecture.md) → "The frontend in one page".

## The two-tier rule

The frontend has a framework-agnostic core that must not import Svelte, and
views that can use anything Svelte offers:

| Tier | Files | May import Svelte? |
|---|---|---|
| Framework-agnostic | `i18n.js`, `i18n/<locale>/*.js`, `lib/backend.js`, `lib/nav.js`, `lib/mapLayers.js`, `lib/registryHints.js`, `lib/dateValue.js`, `lib/numberValue.js`, `lib/collate.js`, `lib/selectItems.js`, `lib/formValidation.js`, `lib/tabOverflow.js`, `lib/byteSize.js`, `lib/seasonDraft.js`, `lib/syncFields.js`, `lib/savedCheck.js`, `lib/bookMerge.js` | **No** |
| Reactive glue | `lib/notifications.svelte.js`, `lib/lookups.svelte.js`, `lib/pageTitle.svelte.js` (runes modules) | Runes only |
| Views + wiring | `App.svelte`, `lib/*View.svelte`, `lib/*Form.svelte`, `lib/routes.js`, `lib/icons.js` | Yes |

A few plain `.js` files are in the view tier because they import Svelte:
`lib/routes.js` names components; `lib/formRefusal.js` imports
`getContext`/`setContext`; `lib/icons.js` turns the icon names `nav.js` holds
into Lucide components; and `lib/columnResize.js` imports `mount` for its drag
handle. Its pure half, `lib/columnWidths.js`, is agnostic and tested. `nav.js`
says what the navigation *offers*; `routes.js` says what each route *renders*,
and the two lists differ: `#/farms/<id>` is a route with no nav entry.

The point: business logic lives in Rust behind `invoke`, and the agnostic tier
would survive a change of framework untouched; only the views would be
rewritten.

**The same line decides what is unit-tested.** `npm test` runs vitest over
`src/**/*.test.js`, for the agnostic tier only, which is why `vitest.config.js`
needs no Svelte plugin: a module that needs a component rendered is, by this
table, a view. Views are checked by the scripted checks instead. Modules that
read `localStorage`/`navigator` when imported (`i18n.js`) are stubbed by the
tests that need them, which keeps the suite on the `node` environment with no
DOM.

**It is the general JS test tier.** Covered today: `numberValue.js`, `i18n.js`
(the display side), `dateValue.js`, `collate.js`, `selectItems.js`, `nav.js`,
`formValidation.js`, `tabOverflow.js` and `byteSize.js`. `formValidation.js`
takes anything that iterates like `form.elements` and returns plain objects, so
its tests build the fixtures by hand and still need no DOM. Not covered yet:
`backend.js`'s `errorText`, `mapLayers.js`, `registryHints.js`,
`treatmentDraft.js`.

**Test files sit next to the module they test** (`lib/numberValue.test.js`), as
is usual in JS, not in a `tests/` folder like the Rust side: the test moves with
its module when one is renamed, and the import path stays `./x.js`. They aren't
bundled (nothing imports them). They ARE checked for SPDX headers and neutral
voice like any other source file. The one exception is `number_formatting.rs`'s
ban list, because a test has to **name** what it guards: `numberValue.test.js`
checks that `1,5kg` is refused, and a test of that rule has to write
`type="number"` out in full.

## Svelte 5 idioms in use

- **Runes everywhere**: `$state`, `$derived`, `$props`. No stores, no old-style
  `export let`, no `$:` statements, no `createEventDispatcher`. Child components
  receive **callback props** (`onSaved`, `onCancel`) instead of emitting events.
- **No state-management library** ([stack-choices.md](stack-choices.md) §3):
  runes already are a fine-grained observable graph, so a MobX-style layer would
  be a second reactivity system competing with the first. Shared state, when
  needed, is module-level `$state` in a `.svelte.js` file;
  `lib/notifications.svelte.js` shows how.
- **Reference data may live at module level; records never do.**
  `lib/lookups.svelte.js` holds the argument-free lists (units, coded
  vocabularies, the model's closed lists), fetched once per session and read as
  `lookups.units` instead of passed as props. `loadLookups()` is started in
  `main.js` after the readiness gate and awaited by any view that needs the
  data; `invalidateLookups()` has one caller, the catalogue refresh in Settings,
  the only thing that can change these rows. **Three things stay out**:
  regulatory records (fetched again on mount: the backend is SQLite in the same
  process, so the query is cheap, and a cache would trade that for stale rows in
  a legal document); **user data the app itself edits** (farms, plots,
  operators, advisors, products, materials: caching those would need an
  invalidation rule per command that changes them); and **lists scoped by
  country** (`list_problem_codes`, `list_growth_stages`,
  `list_crop_species`…), which need an argument the caller already knows. The
  gain is not speed (each list costs 1–2 ms) but not passing twenty lists two
  components down as props. If data ever changes under an open view (a live
  sync), the answer is an invalidation event from Rust, not a client cache.
- **Dynamic lists** change `$state` arrays in place (`rows.push(...)`,
  `rows.splice(i, 1)`) and key each block on object identity:
  `{#each rows as row (row)}`.
- **Switching language**: `App.svelte` wraps the routed content in
  `{#key localeVersion}` and bumps the key on `onLocaleChange`, so every `t()`
  call is evaluated again by remounting. Components never watch the locale
  themselves.
- **Routing** is a small hash router: `App.svelte` tracks the hash and
  `lib/routes.js` maps it to a view. The table is **data**: each entry is a
  `match(hash)` that returns the view's props or null, the first match wins, and
  anything unmatched shows the status view. Order matters: `#/farms/<id>` must
  come before `#/farms`, and prefix routes (`#/map`, which carries a query) come
  after the exact routes they would otherwise catch.
- **Navigation is data**: the top-level destinations are in `lib/nav.js`
  (`NAV_ITEMS`: route, i18n label key, icon name), and `App.svelte` renders that
  list twice: as the collapsible sidebar on wide screens and as the bottom tab
  bar on narrow ones (media query at 700px; there is no desktop menu bar).
  `activeRoute(hash)` picks the highlighted entry by the longest route prefix.
  Adding a view is one `NAV_ITEMS` entry plus one `routes.js` entry; never
  hardcode a nav link in markup. Whether the sidebar is collapsed is saved in
  `localStorage` (`terrazgo.sidebar`), like the language. A view's own actions
  (buttons) belong in the view, not in a global toolbar.
- **Forms**: `<TzForm onsubmit={fn}>`, never a bare `<form>`
  (`form_feedback.rs` refuses one). The handler is a plain `async` function that
  takes no event: TzForm only calls it when the form is valid and catches a
  refusal, so neither `event.preventDefault()` nor `run()` belongs in it. It
  reports a refusal by THROWING. `required`/`min`/`max` still declare the first
  checks, on the owned controls; the form is still the source of truth on save
  (it sends the whole state, not a diff).
- **The view that opens a form owns the form's state.** Every register view
  keeps its fields and fills them in a `showForm(detail = null)`: empty for a new
  entry, from the stored record for a correction. A form split into its own
  component for size (`TreatmentForm.svelte`) doesn't change that: the view
  passes the draft object down (`src/lib/treatmentDraft.js` has its shape) and
  the component binds to `draft.*` without keeping a copy. **A form must never
  take the record as a prop and copy it into local state when it is created**: a
  component keeps its initial values for its whole life, so setting the prop
  again leaves the previous record on screen while the id underneath has
  changed. Svelte warns about this with `state_referenced_locally`. Treat that
  warning as a real problem; don't silence it with `untrack()` or
  `svelte-ignore`.
- **Command calls** go through `run(async () => ...)` from
  `lib/notifications.svelte.js`: an error from the backend becomes a red
  notification (shown through `errorText`: the localized `error.<code>` key; the
  `internal` code gets the localized `error.internal_intro` line plus the raw
  developer message; unknown codes are shown raw), and the bell panel opens by
  itself so the failure is seen. **A form's OWN save doesn't go through `run()`**
  (see Forms): its failure belongs in the form, not the bell. `run()` is for
  everything else a view calls, which is most of it: loading at mount, the map,
  exports, the catalogue refresh, and every delete.

  Success messages are added with `notify(t("message.…"))`, **but only when they
  say something the screen doesn't.** A `<thing>_saved` next to a list that has
  just refreshed under a form that has just closed is noise, and noise is what
  makes a farmer stop reading the bell. Worth a notification: a derived value (a
  new treatment's plazo de seguridad), a path, a count, or a result with nothing
  on screen to show it. That is why `sigpac_boundary_saved` stays: FarmView draws
  no map, so nothing there says the boundary arrived. Notifications collect in
  the bell (`NotificationBell.svelte`, one per layout) until dismissed one by one
  or cleared; switching language clears them all, since they hold text in the
  old language.

## i18n rules

- Never hardcode a user-facing string in markup or JS. Add a key to **every**
  locale. The i18n contract test (`src-tauri/tests/contracts/i18n_contract.rs`)
  reads them all, so it fails the build when key sets differ or `{placeholders}`
  don't match in any locale, including ones added later.
- **Each locale is a folder of area files**: `src/i18n/es/` has `common`,
  `errors`, `farm`, `book`, `fertilisation`, `ecoscheme`, `map`, `settings` and
  `external`, and `src/i18n/es.js` merges them and has no entries of its own. A
  new module adds one file per locale. **A key may be in only one area file per
  locale** (the merge would silently keep the last one), and a contract test
  refuses duplicates.
- `t(key, params)` for normal strings; `tCode(prefix, code)` for schema codes
  (`tCode("unit", "l_ha")` → key `unit.l_ha`), which falls back to the raw code
  so a new schema value still shows something; `formatDate(iso)` for
  `YYYY-MM-DD` values (it parses field by field to avoid the UTC-midnight
  off-by-one).
- **A number in a sentence changes the words around it: pass `count`.** `t()`
  reads `params.count`, picks a CLDR category with `Intl.PluralRules`, and looks
  up `<key>.one` / `<key>.other` before the bare key, so "1 días" and "Se han
  añadido 1 líneas" can't happen. **The contract test enforces the structure**:
  a `{count}` placed right before a word must have plural forms, because that
  word agrees with it. What follows:
  - **One count per key.** The form is chosen from a single number, so a
    sentence whose nouns agree with two different counts can't be done this
    way. Write those as "Label: N" instead (`"Catálogos: {count} · Códigos:
    {codes}"`), which works with any figure. Only use ICU MessageFormat if a
    string really needs two counts, never just because a language was added:
    `Intl.PluralRules` already covers every CLDR category, up to Arabic's six.
  - **A variant may leave the count out**: "Se ha añadido **una** línea" is
    better than "1 línea". `count` is the one placeholder the contract test
    allows to be missing from a variant; every other one must match in all of
    them.
  - **A key with no variants ignores `count`** and gives its plain string, so
    passing it is always safe.
  - **A number before a word that doesn't change with it isn't a `count`**
    ("…y {n} más"), so give it another name and don't pass `count`. That keeps
    the test above without a list of exceptions.
  - `tCode(prefix, code, count)` for the few codes that are counted **nouns**
    rather than symbols (the intensity units: "1 trampa" next to "2 trampas",
    where "1 l/ha" and "2 l/ha" are the same word). Called without a count (a
    picker listing the code) it gives `other`, the dictionary form.
  - The printed record book handles the same thing in Rust with a two-form
    `Plural` type (`crates/terrazgo-recordbook/src/labels.rs`), because its
    languages are Spain's official ones, all of which have two forms.
- **Every number shown goes through `formatNumber(value, digits?)`**, never
  straight into markup and never through `toFixed`, which writes a decimal point
  in every language with no way to change it. The decimal separator is a comma
  in Castilian and Catalan and a point in English, so a number built by hand is
  wrong in whichever language it wasn't written for. For other cases:
  `formatPercent(0–100)`, which also adds the space before the sign that
  Castilian and Catalan want and English doesn't; `formatCoordinates(lat,
  lon)`, joined by `" / "` because a comma between two decimal-comma numbers
  reads as four numbers; and `formatUnit(value, unit)` for quantities in the
  app's *own* units (cache sizes), never the farmer's. Regulatory unit symbols
  use `tCode("unit", …)` and print as they are.
  - **The defaults are four decimals and no grouping.** That keeps
    `formatNumber` at the same PRECISION as the book's `format_number`
    (`crates/terrazgo-recordbook/src/lib.rs`), so the two never show a different
    figure, at most a different separator (the book prints in the farm's
    language, the screen in the reader's). Two decimals would round a dose of
    0,0375 l/ha to "0,04": a regulatory value silently changed. Grouping is off
    because the printed book has no thousands separator, and with it on the two
    co-official languages disagreed: CLDR gives Castilian
    `minimumGroupingDigits=2` and Catalan 1, so 1234,5 was grouped in Catalan
    only.
  - **No decree sets a precision**, so four isn't a rule. It is enough because
    the units already scale (a dose is written in g/ha, not kg/ha). Treat it as
    a display default, never as a reason to refuse a figure the farmer measured.
  - **Money: the format follows the reader, the currency follows the record.**
    There is no money field yet (Costs isn't started), but the rule is written
    here because a region setting makes it easy to get wrong: pass `currency`
    explicitly from the data, never infer it from the locale, or a Spanish
    farm's costs would print in dollars for a reader whose machine is set to the
    US. Only the shape around the figure may change: es-ES writes "1.234,56 €",
    en-US "€1,234.56", both in euros. No currency *setting* is needed: currency
    is data, not a preference. The input stays a plain `NumberInput` with the
    symbol as a label; nobody types a currency sign. Money will also want a fixed
    **scale** (always two decimals), which the control doesn't support today
    because nothing needs it.
  - **A nonzero measurement is NEVER shown as "0".** A value too small for four
    decimals falls back to significant digits, in `formatNumber` and in the
    book's `format_number` alike. Rounding 0,00003 to "0" would show an
    inspector a figure nobody wrote, the same false statement an empty cell is
    there to avoid. Coordinates have their own five-decimal formatter.
  - **An empty value is shown blank, not "0"**: `Intl` turns `null` into zero,
    and a printed 0 is a statement the farmer never made.
  - **`count` stays the raw number.** `Intl.PluralRules` picks from a number, so
    a formatted string would break it; integers look the same either way, since
    grouping is off.
  - **A value bound to an input stays raw.** Formatting is for reading, not
    editing: a numeric field parses what it is given.
  - A contract test (`src-tauri/tests/contracts/number_formatting.rs`) bans
    `toFixed` and the bare `toLocale*String` family outside `i18n.js`, and checks
    the two defaults. It can't see a raw number put into markup
    (`{record.dose_value}` and `{record.notes}` look the same), so that part
    depends on review, which is why it is written here.
- **Two tags, because formatting and language are different questions.**
  `formatTag()` is what NUMBERS and DATES are shown with and what the owned
  date/time/number controls parse with, so a figure the app shows and a figure
  it lets you type can never disagree. `languageTag()` is what the app's own
  WORDS use: `Intl.PluralRules` and `Intl.Collator`. Applying a Polish machine's
  plural rules to Spanish text, or its sorting to Spanish farm names, is the bug
  the split prevents. Keeping sorting on the language also keeps the screen in
  agreement with the book's Rust collator.
- **`formatTag()` follows a per-device setting, defaulting to the machine's.**
  `terrazgo.format` in `localStorage`, next to `terrazgo.locale`, has two values:
  `"system"` (the default: the OS regional format, read through
  `Intl.NumberFormat().resolvedOptions().locale` rather than guessed from
  `navigator.language`, since the two can differ) and `"language"`. Settings
  offers both with a live sample of each, because "system" means nothing without
  the figure it produces. Nothing is saved until the farmer chooses. **The
  printed book isn't affected either way**: it uses the farm's report language,
  which is another matter again.
- **`languageTag()` maps through `FORMAT_LOCALE`**, and the entry that matters is
  **`en` → `en-GB`**: plain `en` means US in `Intl`, so English would print
  `08/03/2026` where every other language in the app prints `03/08/2026`, and
  English here means European English. `formatDate` and the owned fields use the
  same function, so a date the app shows and a date it lets you edit can never
  disagree.
- **Sort display lists with `lib/collate.js`**, not `.sort()`. SQL returns BINARY
  order (`Á` is U+00C1, so "Ángel" comes after "Zubiri"; "Parcela 10" before
  "Parcela 2"). The module mirrors `crates/terrazgo-recordbook/src/collate.rs`
  (same CLDR data, `numeric: true`, accents distinguished), so the RULES match.
  Which language's rules apply doesn't always: the book sorts in its report
  language, the screen in the one being read. Castilian puts `ñ` after `n`
  (*Peña* after *Penz*), while Catalan and English put it with `n`, so an English
  reader of a Castilian book sees a different order on screen than on paper.
- Data the user enters (farm names, species, notes) is never translated.
- Adding a language is one `SUPPORTED` entry in `i18n.js`, plus one folder with
  the same area files and every key, plus its row in `required_categories` in
  the contract test, which stops it arriving with half its plural forms missing.

## Talking to Rust

- `invoke` comes from `lib/backend.js` (re-exported from `window.__TAURI__`;
  `withGlobalTauri: true`, no `@tauri-apps/api` npm dependency).
- Tauri exposes snake_case Rust command arguments as **camelCase** invoke keys:
  Rust `farm_id: String` ⇒ `invoke("list_plots", { farmId })`. Struct payloads
  (`NewFarm`, `NewTreatmentRecord`, …) keep their **snake_case** field names,
  because serde reads them, not Tauri's argument mapping.
- Optional fields: send `null`, not `undefined`; turn empty inputs into null
  with `value.trim() || null` before building the payload.
- Plugins are called the same way: `invoke("plugin:dialog|save", { options:
  {...} })`.

## Navigation, feedback and confirmations

These three go together, because they answer the same question: how does a
non-technical, occasional user on a phone find a function and find out what
happened?

- **Navigation adapts to the screen and is data-driven**: a collapsible sidebar
  on wide screens, a bottom tab bar on narrow ones, and no desktop menu bar or
  global toolbar. Menu bars have no mobile equivalent and hide functions behind
  locations people have to remember, which is wrong for these users; a top nav
  or tab bar holds about five items at most, while the sidebar grows with the
  modules. Destinations are data in `lib/nav.js`, drawn by both layouts, so
  adding a screen is one entry and the two layouts can't drift apart.
  `lib/routes.js` is a *separate* table: one says what navigation offers, the
  other what a route shows. The sidebar shows labels by default (the icon-only
  rail is optional and saved in `localStorage`), and app-level entries go to its
  foot with `foot: true`. A view's own actions stay in the view.
- **A detail page has a back button and is titled with its record's name.** A
  route in `lib/routes.js` with a `parent` gets, in both the wide band and the
  phone's top bar, a left-chevron link to that parent (the bell's size and
  outline, at the other end of the band), followed by the name the view
  publishes through `lib/pageTitle.svelte.js` ("Finca Los Llanos"), or the
  section's name while it loads. **It isn't a breadcrumb**: the sidebar and tab
  bar already highlight the section, so a trail would repeat it, and no page is
  more than two levels deep. The title is stamped with the hash it was set on
  and only shown while that hash is current, so the shell never has to clear it;
  clearing on navigation would race the view that is mounting and publishing
  its own.
- **A form's problems belong to the form; everything else goes to the bell.** A
  form reports in two places at once, both drawn in one pass: a
  `.validation-summary` at the top listing every problem in field order, and the
  field's own `.tz-field-error` under it. Clicking a summary entry focuses its
  field, which a register several screens tall needs. A refusal from the backend
  is shown once: under the field when the form's `anchors` map names one that is
  on screen (focus moves there, and a hidden `role="alert"` region reads it
  out), and in the summary otherwise.

  **Every problem is a message on a form control**, so there is no second error
  state to keep in step. TzForm has `novalidate` and calls `checkValidity()`
  itself, which fires `invalid` at each failing control (drawing the inline
  messages) and leaves the list readable from `form.elements` (drawing the
  summary). Hiding the browser's own bubble is deliberate: it shows only one
  problem, in the OS language.
- **Command feedback goes to the notification bell**, not an inline message:
  messages stay until dismissed, so a farmer can look back at what happened.
  **Errors open the panel by themselves** (a failed command must not hide behind
  a badge), while successes only add to the count. There is one bell per layout,
  sharing state (`lib/notifications.svelte.js`); switching language clears the
  items, since they hold text in the old language. **Two things don't go here**:
  a form's own validation and refusals (see above), and domain alerts (PHI,
  licence, ITV), which are on the Status view. The bell is for command feedback,
  not a second alert system.
- **Destructive actions confirm with `confirmDialog()`** in `lib/backend.js`,
  which calls `plugin:dialog|message` with `buttons: "OkCancel"` and treats the
  result `"Ok"` as yes. `window.confirm` is banned: blocking JS dialogs aren't
  reliably supported by mobile webviews. Put the confirmation *inside* the
  `run()` block: `if (!(await confirmDialog(...))) return;`.

  **Pin plugins to the minor version that defines what you call.**
  tauri-plugin-dialog 2.7.0 removed the `confirm`/`ask` IPC commands (merging
  them into `message`), and a lockfile update broke every delete confirmation
  with a raw English error. Raw `withGlobalTauri` invokes depend on a plugin's IPC
  commands with no wrapper and no compiler check, so **the manifest must require
  at least the minor that defines the call** (`tauri-plugin-dialog = "2.7"`).

### The settings screen: a contents list beside one scrolling document

Settings is the one screen with its own navigation: a tree of its sections on
the left and a search field above the settings. The settings are **one document
that scrolls as a whole**: the tree scrolls to a heading and the search narrows
what is shown.

**The tree is for navigating, so collapsing a section hides its entries in the
tree, never the settings themselves.** Sections are open by default, and a search
opens all of them, because a hit counted next to a heading the reader can't see
points at nothing. The collapsed state is left alone while searching, so
clearing the field puts the tree back as it was.

**The search field is a `.tz-control` box, not a `TextInput`.** The hit count and
the clear button go inside the field, and that box is how the app builds a
control with an input plus extras (`TzCombobox`'s input sits in one the same
way). Nothing here is a form field (no validity, no submit, nothing saved), so
the owned-control plumbing has nothing to do.

**The structure is declared once, in `lib/settingsTree.js`**: the sections, the
groups inside them, and for each setting the i18n **keys** it can be found by.
The view and the tree are both drawn from it, as with `nav.js`/`icons.js`: a
contents list that can disagree with what it lists is worse than none. Matching
uses `collate.js`'s `foldTokens`/`matchRank`, so the settings search and the
catalogue pickers can't end up with two ideas of what a match is.

**What a setting can be found by is declared, not read from the screen.** Reading
the rendered DOM would only index what happens to be on screen, so a setting
inside a closed panel couldn't be found. `settingsTree.test.js` checks every
declared key against the real Spanish dictionary, because a stale key would
silently search for its own name.

**A section that matches by its own text keeps all its contents.** "carné" is in
the alerts hint and in neither field's label; showing an empty heading would be
worse than no match at all.

Four things learnt the hard way:

- **An unstyled `<button>` turns WHITE on hover, and setting its colour once
  isn't enough.** `button:hover:enabled` sets `color: var(--on-primary)` at
  0,2,1, which beats a 0,2,0 base rule giving the normal colour, so a tree node
  was readable until the pointer touched it. Any button styled as something
  other than an action (`.toc-node`, `.toc-twisty`, `.tz-field-trigger`,
  `.pane-resizer`) needs its colour set **in a hover rule of its own**, not only
  in its base rule.
- **A sticky element reports its STUCK position from `getBoundingClientRect`.**
  If the group headings were sticky bands, they would all measure the same
  position and the scroll spy would stop at the first. So group headings here are
  **static** (`.settings-pane > .view-head`), and only the search band sticks.
  That costs nothing, because the tree next to it already says where you are.
  **Never measure a sticky element to find the scroll position.**
- **`scrollIntoView` scrolls EVERY scrollable ancestor**, and `<main>` is one:
  `overflow: hidden` still allows a scroll from code. The whole shell slid up and
  stayed there, taking the sticky band off the top of the window. To jump, scroll
  the intended scroller directly (`viewEl.scrollTo`); never ask an element to
  bring itself into view.
- **A clicked node has to stay selected until the reader scrolls.** The last
  sections can never reach the top of the scroller, because there isn't enough
  document below them, so the scroll spy would override the click straight away:
  clicking *Mantenimiento* would light *Perfiles*. The selection is released when
  the reader moves the pane, not on a scroll event, since the smooth scroll the
  click started fires plenty of those itself.

  **The two signals are `wheel` on the frame and `focusin` on the pane.**
  `touchmove` isn't needed: the tree is hidden below 700px, so nothing is ever
  pinned there. `keydown` on the frame is what Svelte's
  `a11y_no_static_element_interactions` warns about, and rightly: a container
  that answers keys should be reachable. `focusin` is the right signal anyway,
  because focus landing in the pane is what scrolls it for a keyboard user and
  what has to happen before any key reaches it. It goes on the **pane**, not the
  frame: the tree is inside the frame too, so `focusin` on the frame would release
  the pin the click had just set.

**Below 700px the tree is gone**, and the screen is a single column plus the
search band: `<main>` is the scroller there, and a second column has nowhere to
go.

### The About panel's third-party attribution

Three tabs; the third is the attribution owed to the libraries we ship. Two data
files feed it, for different reasons:

- **`src/lib/thirdParty.js` is written by hand**, one row per PROJECT, because a
  reader recognises "Tauri", not `tauri-plugin-geolocation`, and no tool can do
  that grouping. It lists the direct dependencies of our own crates plus the npm
  packages whose code reaches the bundle; not the whole resolved graph, and not
  the build and test tools (never distributed, so not ours to attribute).
- **A third kind of row, `bundled`, exists because the dependency graph isn't
  the binary.** SQLite's amalgamation is compiled in through rusqlite's `bundled`
  feature, and the four Liberation Sans faces are embedded with `include_bytes!`.
  Both ship, and neither is a package. A bundled row names the file its licence
  is read from and, where there is one, the crate whose version follows it
  (SQLite's is libsqlite3-sys); the manifest and lockfile checks skip these rows,
  because nothing in cargo or npm will ever mention them.
- **`src/lib/thirdPartyLicences.json` is generated** by `npm run gen:licences`
  and committed: a build shouldn't depend on the cargo registry being present,
  and an offline-first app has to be able to show its attribution offline. It is
  loaded lazily (about 90 KB), only when the tab is opened.

**It is kept accurate by tests, not by memory.** `cargo test` fails, by name and
with the fix in the message, when:

| What you did | What fails | What to do |
| --- | --- | --- |
| Added a dependency | `every_distributed_dependency_is_attributed` | add it to `thirdParty.js`, then `npm run gen:licences` |
| Removed one | `every_listed_package_still_exists` | remove its row |
| Brought in a new licence | `every_licence_has_an_allowlisted_link` | add the SPDX URL it names to `external_links.rs` |
| Edited the list and forgot to regenerate | `the_generated_licence_texts_cover_every_listed_package` | `npm run gen:licences` |
| **Upgraded** a dependency | `the_generated_licence_texts_were_read_from_the_installed_versions` | `npm run gen:licences` |

The last row is the easy one to miss and the one that matters most. The other
four react to the LIST changing, and an upgrade doesn't change the list: the
package is still there, still covered, still linked, and the text on screen is
silently the old version's. A licence file does change with its package (a
copyright year, a change of licence). So the generator records the version it
read each text from, and the test compares those with `Cargo.lock` and
`package-lock.json`, a cheap way to know "this file may have changed".

The generator also refuses, rather than producing something incomplete, when a
package ships no text for the licence it is shown under and none is vendored.

**One licence per library.** A dual licence (`MIT OR Apache-2.0`, `Unlicense OR
MIT`) is the licensor offering a choice ("Licensed under either of … at your
option"), and whoever takes one option can redistribute under that one alone.
The FSF reads it the same way ("each user could choose to use and redistribute
Perl under one license or the other", gnu.org/licenses/license-compatibility.html).
So each library is shown under ONE licence. `licenceShown` in `thirdParty.js`
takes **MIT wherever it is offered**, and every row with a choice offers it: it
is compatible with every GPL version (Apache-2.0 only with v3, though
AGPL-3.0-or-later accepts either), its text is a kilobyte against eleven, and it
has no NOTICE-file duty. The row still records the whole offer, because that is
what the package states and what makes taking one option lawful. The contract
test checks both halves (each package under exactly one licence, and one its row
offers), and a new dual-licensed dependency that doesn't offer MIT stops the
generator and `npm test` until someone decides which option to take.

**Grouped by licence, then by TEXT.** Apache-2.0's body is standard text, but
**MIT includes its copyright line**, so our packages have many different MIT
texts. Grouping by licence alone and showing one text would attribute most of
the MIT packages to the wrong copyright holder. The grouping key ignores
whitespace, because several packages ship the same licence wrapped differently.

**Two things can't be automated.** Pulling the copyright line out of a licence
file returns prose from inside the Apache-2.0 body ("copyright notice that is
included in or attached to the work") for a third of the packages. And some
packages publish no licence text at all, so `third-party/` holds copies taken by
hand from each project's own repository, with where they came from in its README;
the generator **refuses** rather than produce a package with no notice.

**cargo-about is still the right tool for something else**: a complete
transitive NOTICE file at release time. It covers the crates and knows nothing
about npm, and it produces a document, not panel data.

### The technical tab answers four questions, not sixteen

It is the block a tester is asked to read out, so it carries what a bug report
needs, organised so it isn't a wall of rows. Four groups, each its own `<dl>`
under a heading (a heading isn't valid inside a `<dl>`), sharing a fixed
`minmax(0, 8.5rem)` label column so the values line up across all four. An
`auto` column would size to each group's longest label and jump in and out down
the tab.

- **Build**: the version *and* the build stamp in one row, which is the point
  of the stamp: during a pre-release the version doesn't change, so "0.1.7"
  alone doesn't identify a build. Then the identifier next to its packaging (an
  AppImage, a Flatpak and a `.deb` put the same id in three different data
  folders), Tauri, the web engine, SQLite.
- **Machine**: OS, the architecture the BINARY was built for, processor, free
  and total memory, this process's resident memory and its peak, and the
  monitor.
- **Data**: both databases, each with path, schema version and size on disk.
  Side by side so the disposable cache and the irreplaceable database can be told
  apart before anyone deletes the wrong file.
- **Regional**: the machine's locale, the app's language, and the format mode
  with a live sample (`es-ES · 1234,5 · 08/03/2026`). Three separate questions,
  and a report saying "the dates are wrong" needs all three.

**The frame rate is measured, not reported, and it is the only figure here that
is**: no browser API gives a rate and Tauri's `Monitor` has none, so the panel
times `requestAnimationFrame` itself. Two things about it, which is why the row
says `~62 fps` and not `62 Hz`:

- **It is frames per second, NOT the screen's refresh rate.** On WebKitGTK the
  webview paints about 62 fps on a 60 Hz screen, because its rAF runs on a timer
  rather than on vsync. Showing "62 Hz" next to the display's resolution would
  state something about the hardware that the hardware contradicts; as fps it is
  the useful number anyway, the answer to "why does this feel slow".
- **Use the mean over the window, never the median of the gaps.** WebKitGTK
  rounds the rAF timestamp to whole milliseconds, so a true 16.67 ms frame
  arrives as a run of 16s with the odd 17, whose median is exactly 16 ms (62.5
  fps) however long you sample. Dividing the elapsed time by the frames in it
  spreads one millisecond of rounding over all of them.

**Every number goes through `formatNumber`**, including the scale factor and the
pixel counts: a scale is `1,5` in Castilian and `1.5` in English. Sizes go
through `lib/byteSize.js`, shared with the Settings view's cache size.

## Adding a command end-to-end (checklist)

1. Repository function in the owning crate, with a test
   (`crates/*/tests/repository.rs`).
2. A thin `#[tauri::command]` wrapper in `src-tauri/src/commands/<domain>.rs`
   (one file per crate the commands wrap): no logic, just `lock_conn` + the
   repository call + `?`.
3. Register it in `generate_handler!` in `src-tauri/src/lib.rs` (the
   `command_registration.rs` contract test fails otherwise).
4. If it can return a new `Invalid("code")`, add `error.invalid.<code>` to every
   dictionary (the i18n contract test fails otherwise).
5. Nothing for alerts: the list is worked out when read, so a write that changes
   what an alert depends on needs no extra call.
6. Frontend: a form's save goes in a `TzForm` handler (plain `async`, throws);
   anything else goes through `run()`. Add a `message.*` key with `notify()` on
   success **only if it says something the screen doesn't**.
7. If the new `Invalid("code")` is about one field of one form, add it to that
   form's `anchors` so the refusal shows under the field. Anchors are per form:
   the same code can name a different field in each.

## Styling

- One global stylesheet (`src/styles.css`), plain CSS, no preprocessor. Shared
  vocabulary goes there; **rules only one component can use go in that
  component's `<style>` block**. Svelte's scoping is the right tool for the
  local case.
- **Scoping adds one class of specificity, so a rule the global sheet overrides
  from an ancestor can't be moved into a component.** For example,
  `.notif-panel` (`0,1,0`) correctly loses to the narrow-screen `.topbar
  .notif-panel` (`0,2,0`). Moved into `NotificationBell.svelte`, it compiles to
  `.notif-panel.svelte-hash`, also `0,2,0` and later in the bundle, so it wins
  the tie and takes `width` and `right` with it: the panel no longer stretches
  edge to edge on a phone and overflows the screen. **Svelte doesn't warn**: the
  selector is used, it just wins something it didn't before. So the test for
  moving a rule isn't "does one component use this class" but **"does any rule
  outside that component target the same element"**. If one does, the pair is
  composition, and composition is shared vocabulary. That is why the app-shell
  and bell rules (`.sidebar`, `.topbar`, `.tabbar`, `.main-head`,
  `.bell*`/`.notif*`) stay in the global sheet although each has one user:
  `.main-head .bell-wrap` and `.topbar .notif-panel` span two components and
  can't be written from either.
- **Anything on markup a library renders has to stay global too**, for the
  reason two bullets down: `tz-calendar-*`, `tz-dialog-*` and `tz-trigger` sit on
  Bits UI elements. `.tz-check` stays global for the first reason instead:
  TzCheckbox renders its own markup, but `.form-grid`'s two `:not()` exclusions
  name the class from outside.
- **Splitting the sheet into partials was considered and rejected**: its banner
  sections make it easy to find your way, and what limits its growth is applying
  this rule on every change, not a folder of files.
- **No scroller bounces, and each one opts out on the axis it scrolls and
  nowhere else.** Chromium's overscroll effect (the Android 12+ stretch, the glow
  before it) animates whichever box was overscrolled, so a scroller left at
  `auto` stretches its own content at the ends. Use `none`, not `contain`:
  `contain` still shows the local bounce.

  **`none` also stops scroll chaining, which is why it must not be set
  everywhere.** Every `overflow: hidden` box is a scroll container, and a touch
  scroll latches to the innermost one under the finger: Android System WebView
  ends the gesture at any container that says `none` on the swiped axis, whether
  or not it has anything to scroll. With `* { overscroll-behavior: none }`, on a
  phone every table cell (hidden, for the ellipsis), every `.table-wrap` and the
  map's side panel became dead zones, and a sideways swipe on a table scrolled
  nothing. So the opt-out goes in the rule that makes the box scroll, on that
  axis only (`.table-wrap` says `overscroll-behavior-x`, never the shorthand),
  and a box that only scrolls in one layout (`.map-side`, beside the map) sets
  its `overflow` inside that layout's media query.
  `src-tauri/tests/contracts/overscroll_contract.rs` reads every stylesheet and
  holds each block to exactly that. How to check it on a phone is in
  maintenance.md §8; headless Chrome shows the trap too, but only with real touch
  events.
- **A scoped rule can't style markup another component renders.** Passing
  `class="…"` down as a prop gives the class to a child, but the parent's scoping
  hash never reaches that element, so the rule silently does nothing. Svelte
  reports `Unused CSS selector`, which is worth treating as an error. Pass a class
  the *global* sheet defines (`inline-field` is the usual one: a label beside its
  control), or put the rule in `styles.css`.
- **Plain CSS, with design tokens** (why not Tailwind:
  [stack-choices.md](stack-choices.md) §4). `:root` has, besides the palette:
  `--surface` and `--on-primary` (a raised sheet, and the text on a filled
  `--primary`/`--danger`: two different things that used to both be `#fff`), the
  interaction states `--surface-hover`/`--surface-active` and
  `--disabled-opacity`, a six-step `--space-*` scale, four `--radius-*`, two
  `--shadow-*`, `--z-popover`/`--z-sticky`, and `--focus-ring` /
  `--focus-ring-offset` (owned controls are focusable `<div>`s and `<button>`s
  that draw their own focus, and one `:focus-visible` rule draws it for all of
  them). **Use a token where one fits; add a step rather than a one-off value**,
  so a re-theme, a density change or a new floating element is a `:root` edit,
  not a hunt through the sheet. A few spacing values sit *between* steps
  (`0.4rem`, `0.6rem`, `0.9rem`…) and were left alone on purpose: snapping them
  moves every button and card in the app, which needs a design pass with
  screenshots. There is **no global `box-sizing: border-box` reset**; it is set
  per rule, so any rule that sets a minimum height or width has to say which box
  it means.
- Reuse the existing vocabulary before inventing classes: `.view`, `.view-head`,
  `.form-grid`, `.form-actions`, `.table-wrap`/`.data-table` (+`.rows-static`,
  `.col-name`/`.col-num`/`.col-actions`) and `.table-empty`, `fieldset.es-only`
  (sections shown only for one country), `fieldset.subsection`,
  `.btn-danger`/`.btn-cancel`. Every list of records is a table. The one card in
  the app is the Status view's alert card, scoped inside `StatusView.svelte` (see
  "A list of alerts is not a table"). App-shell classes (`.sidebar`, `.topbar`,
  `.tabbar`, `.main-head`, `.bell*`/`.notif*`) belong to
  `App.svelte`/`NotificationBell.svelte`; views never use them.
- Icons are **[Lucide](https://lucide.dev) components** (`@lucide/svelte`, ISC),
  imported by name and drawn with `currentColor`: no icon font, no image files
  (CSP: `default-src 'self'`). The component only sets SVG **attributes** and a
  class list, so it injects no stylesheet ([stack-choices.md](stack-choices.md)
  §2b). Size it in CSS: the `width`/`height` it renders are attributes, which any
  CSS size overrides. `nav.js` names an icon instead of holding one, because it
  can't import components; `lib/icons.js` in the view tier turns the name into
  the component.
- The production CSP is `default-src 'self'`: no inline styles or scripts, no
  CDNs. The dev-only additions (`devCsp`) exist only for Vite's hot reload.
  "No inline styles" is less precise than what the policy actually does (there
  is no `style-src`, so `style-src-attr` and `style-src-elem` both fall back to
  `'self'`): `setAttribute('style', …)` and a `<style>` element added at runtime
  are **blocked**, while every CSSOM write (`el.style.prop`,
  `Object.assign(el.style, …)`, `el.style.cssText`) **works**. A Svelte `style=`
  **binding** is safe because it compiles to `cssText`; a **fixed `style=`
  attribute is blocked**. Write a class. The rule for a new dependency is **no
  stylesheet injected at runtime**, and the rule for our own markup is **no
  literal `style=`**. Every `invoke` also raises a `connect-src` violation for
  `ipc://localhost/<command>` while working normally: expected noise. Anything
  else in the console on Android is worth looking into. Details:
  [stack-choices.md](stack-choices.md) §2.

### The UI typeface

The app uses **IBM Plex Sans**, self-hosted as **one variable woff2** at
`src/fonts/IBMPlexSansVar-Roman-subset.woff2` (about 100 KB). It has to be
self-hosted: the production CSP is `default-src 'self'`, and the app must look
the same with no network.

**What decided it isn't looks:** its digits are **always tabular** (the font has
no `tnum` feature because it has no proportional digits to switch from), so
every column of doses and dates lines up with no CSS.

**The app turns on no OpenType feature.** `liga` and `kern` are on by default,
this face has no `calt`, and the slashed zero is off: a plain zero reads better,
and Plex already tells 0 and O apart by shape (a narrow oval against a round O).
`check-font.py` still checks the `zero` feature is there, as a canary rather
than because it is used.

Rules that are easy to get wrong:

- **Set weight with `font-weight`, never `font-variation-settings`.** The
  `@font-face` declares `font-weight: 100 700`, which lets the browser move the
  `wght` axis through the normal cascade, so every existing rule keeps working.
  `font-variation-settings` bypasses that cascade; it is kept for `wdth`
  (85–100%), which nothing uses yet.
- **`format("woff2")`, not `format("woff2-variations")`.** The latter is
  deprecated, and an engine that doesn't recognise it skips the source silently;
  the failure looks like a page that renders fine in system-ui.
- **A subset must pass `--layout-features='*'`.** Dropping features doesn't
  break rendering, it just quietly removes them (Google's own subset of this face
  keeps `tnum` and loses `zero`).

**One variable file rather than three static ones** costs a little: static
subsets of the three weights in use (400/600/700) come to about 8 KB less. Those
8 KB buy any future weight and the `wdth` axis. If `wdth` is ever used, narrow
**prose** columns; narrowing a registration number goes against the reason the
face was chosen.

**Which upstream.** There are three: `@ibm/plex-sans-variable` is the vendor's
own current package; `@ibm/plex` is the retired all-in-one package, whose
variable font is two years older; google/fonts has a later font version with a
wider `wdth` range, but only as a raw file on a branch. After subsetting, the IBM
and Google builds differ by **8 bytes**, so the pinned package version is used.

Two scripts keep it correct, both run by hand (they need `sudo apt install
fonttools`, a local tool, not a project dependency; the committed `.woff2` is
what the build uses): `scripts/subset-font.sh` cuts the file again, and
**`scripts/check-font.py` checks what survived**: the axes, the OpenType
features, and every non-ASCII character the frontend can put on screen, with
comments stripped so a code comment's Greek doesn't demand Greek in the font. **A
character that falls back to another font is invisible in review and obvious in
a screenshot**, which is why the check exists. Symbols the font doesn't have
(`✕`, `⚠`, `▰`) are Lucide icons instead.

### A list of alerts is not a table

Every list of *records* is a `.data-table`. The Status view's domain alerts are
the one list shown as cards, because they fail the test a table exists to pass:
**columns are worth it when rows have the same shape and get compared down a
column**. Six alert kinds don't.

- Three of the six are **zone flags**, with no date, so a Date column would be
  empty for half the list.
- A Subject column could only show the entity *type* ("parcela"), never which
  one. The card answers what a farmer actually asks: `Alert.subject_label` names
  the plot, the operator, the machine, or the plots a treatment covered. It is
  worked out when read from `(subject_table, subject_id)`, which has no foreign
  key on purpose, so it can be `None`; the view then names the kind instead, the
  one case where that line is shown muted.
- Each alert has exactly **one thing to do with it**, and nothing to open. There
  is no record behind the row, so no reason for a row.

**The card phrases each kind in its own words**, and what decides the phrasing
comes from Rust: `Alert.standing` says whether the condition ends by itself.
Each kind declares it where it is raised (`StandingKind` or `DatedKind`, in the
crate that owns the rule). A standing alert says "condición permanente" and has
no date; one that ends shows the date: "hasta el …" while it is ahead, "venció
el …" in `--danger` once it has passed (`Alert.overdue`, decided in core from
the day the list was read). The titles of kinds that can pass their date are
worded to fit both states ("Caducidad del carné de aplicador", not "a punto de
caducar"), so the date line alone carries the change.

Two things in the list aren't alerts, and look different. **A record a rule
couldn't read** is a card with a dashed `--danger` border: what couldn't be
checked, the stored value, the record's name, and where to correct it (the
screen and tab named with those screens' own labels, `PLACES` in
`StatusView.svelte`, or a general hint for other kinds of record). It has no Seen
button: there is nothing to acknowledge until the value can be read. **A crate
that failed completely** is the notice above the list, which says to update the
app and report it, with the About panel's report link and the error's detail in
a folded `<details>`. Whether a kind is standing is decided next to its rule,
never in the view: a view that hardcoded it would be a second copy of a domain
rule with nothing checking it against the first.

**One control, in two steps.** Acknowledging and dismissing aren't alternatives:
an acknowledged alert stays listed because its condition still holds, and
dismissing is how you clear one that never ends by itself. So the card offers
"seen" first, and only then offers to hide it. The faded look of an acknowledged
alert applies to the card's **text**, never the whole card: the button is the
only way to hide the alert, and a faded control is one a farmer stops finding.

The rules are scoped inside `StatusView.svelte`, since no rule outside it targets
an alert card (the test in Styling above). Two details: a card needs `min-width:
0` on its flex text column, or a long title pushes the button past the edge; and
the button has a `(pointer: coarse)` minimum of 2.75rem, because a button sized
by its padding comes out around 30px, under the 44 CSS px minimum the owned
controls follow for the same reason.

### A register's form is a modal, and its dropdowns go inside it

Every data-entry form opens in `TzDialog`, through `TzFormDialog`. A form shown
as a side pane would be one more thing on a screen that already has a toolbar, a
table, a splitter and a pinned bar, and on a phone it would just be more page
under the table, with nothing showing where the list ends and the questions
begin. A dialog is a flex column with a real footer.

**The z-index problem, and why the ladder doesn't change.** `--z-popover` (32)
is below `--z-modal` (40) on purpose: a popover must be above the view's sticky
bands but not above the shell's top band. Portalled to `<body>`, a `.tz-popover`
is a *sibling* of the panel at a lower step, so every select, combobox,
catalogue picker and date picker opened inside a form would be drawn underneath
the panel. Raising the ladder would have cost two steps (`--z-tooltip` sits
between them) for every popover in the app, including those that never meet a
modal, and it couldn't be done in a stylesheet anyway: `use-floating-layer`
copies the **content's computed z-index** onto the wrapper it positions, so there
is no outer element at a neutral step left for a selector to raise.

So the layers move instead. `TzDialog` nests a second `BitsConfig` whose
`defaultPortalTo` is the dialog's own content element, and inside that stacking
context a descendant at 32 is drawn above the panel's content. Three details
matter:

- the target is `.tz-dialog`, never `.tz-dialog-body`, which is `overflow-y:
  auto` and would cut a dropdown off at its edge;
- the prop is `dialogEl ?? undefined`, because `undefined` falls back to the
  shell's `"body"`, while `null` makes `portal.svelte` throw while the panel is
  closed;
- `.tz-dialog` has no `transform` (it is centred with `inset: 0; margin: auto`),
  because a transform is the containing block for `position: fixed`
  descendants, and a floating wrapper is fixed by default.

This works on all three engines the app runs in (Blink, WebKitGTK, Android
WebView): the first option row is the topmost element at its own centre, and
clicking it picks it. **Check with a hit test**: `getComputedStyle(...).zIndex`
says `32` whether it works or not. It also works under the production CSP: no
`securitypolicyviolation` events while opening a panel, opening a dropdown inside
it, picking a row and closing both, and `<body>`'s `pointer-events` stays `auto`
at every step. (`preventScroll={false}` is unchanged, and `BitsConfig` renders
`{@render children}` with no layer of its own.)

Escape order, outside clicks and focus are managed by bits-ui through
`globalThis` registries and a focus-scope manager, none of which looks at the
portal target. An open select takes the Escape before the panel does.

**Dismissing and focus.** Escape closes; a click on the overlay doesn't
(`dismissible={false}` → `onInteractOutside` preventing the default, *not*
`interactOutsideBehavior`, which `dialog-content.svelte` doesn't pick out and
would pass to the div as a stray attribute). A stray click next to a long
correction form isn't a decision to throw it away. Escape goes through `onclose`,
which at every call site is the same function Cancel calls: that is what makes
Escape mean Cancel, including resetting the draft.

Focus goes to the panel body, not the first control. bits-ui's focus scope takes
the first *tabbable* element, which here is the close button, where Enter (the
reflex after opening anything) would close the form. Cancel would be no better.
The first field is worse on a phone, where it raises the keyboard over a form
nobody has read yet; the ARIA practices allow focusing the dialog element itself
for exactly that case.

**The panel is controlled**, so `open` stays a plain prop: a call site may pass
an expression (`createOpen || openId !== null`), and `bind:` can't take one. What
bits-ui gets is a *writable* `$derived` of it, and that matters: a dismissal
writes to the derived, the caller's `onclose` decides, and **a handler that
doesn't close keeps the panel open**. No caller refuses today, but a controlled
dialog that overrode its caller would be wrong the day one does. (A derived
`open` that stays `true` pushes no new value down, so without this, nothing
would put the panel back after Escape and the screen would be left stuck.)

**44rem is arithmetic.** `.form-grid` is `repeat(auto-fill, minmax(13rem, 1fr))`
with a 12px gap, so the number of columns is `floor((content + 12) / 220)`. 44rem
is the first width that gives **3**, and it stays at three up to 57rem, so one
number works at every wide screen. Not `fill`, which fixes the height so a box
doesn't resize when *tab* content changes; a form's content doesn't change that
way.

**The footer has `flex-shrink: 0`**, for the reason noted at
`.tz-dialog-head`: a flex column shrinks every item in proportion to its base
size. On a real phone this made the head different heights across the About
panel's three tabs, and it can't be reproduced in a desktop browser at any size.
With a footer the column has three items, so the head's fixed height is checked
across a short form, a long one, the About panel and each of its tabs.

### A required field says so, and says it once

A required field has to be marked before submitting, not only refused after. The
controls carry `required`, but it only drives `setCustomValidity`, and because
they avoid the `required` **attribute** on purpose, a screen reader wouldn't be
told either.

Three parts, because an unexplained asterisk is the usual mistake (W3C WAI and
the GOV.UK Design System both say so, and the latter is worth reading before
changing how a form asks a question):

1. `RequiredMark.svelte` next to the label: `aria-hidden`, in `--danger`, the
   colour of the invalid border and the validation summary, so the mark and the
   message it predicts read as one thing.
2. `aria-required="true"` on the control. ARIA only: it states the meaning
   **without** the native validation and the OS-language bubble that got the
   attribute banned. `form_feedback.rs` bans the attribute, not this.
3. The legend `TzForm` shows once per form, only when the form has a required
   field, read from the rendered DOM rather than declared by every caller.

The asterisk marks the **minority**, which here is the required fields by a
wide margin. If that ever flips on some form, mark the optional ones on that
form.

One trap: `.form-grid label.tz-label { display: block }` has to come **after**
the rule it overrides. `:not()` takes the specificity of its argument, so
`.form-grid label:not(.tz-check)` weighs the same, and if written earlier in the
sheet it wins the tie: the mark ends up on a line of its own.

## Tooling

- **Prettier** formats JS/Svelte/JSON/CSS/HTML (`.prettierrc`: printWidth 100,
  `prettier-plugin-svelte`; markdown is excluded so hand-written doc tables stay
  editable). `npm run format` fixes, `npm run format:check` is the CI check.
- **ESLint 9** (flat config, `eslint.config.js`) with `eslint-plugin-svelte`
  catches mistakes: undefined globals, unused variables, Svelte misuse. Style is
  Prettier's job, so there are no style rules in ESLint. `npm run lint` is the CI
  check.
- Destructive confirmations use `confirmDialog(message)` from `lib/backend.js`
  (see "Navigation, feedback and confirmations").

## Owned controls

The app owns its form controls. There is no native `<select>`, `<input
type="date">`, `<input type="time">` or `<input type="number">`; they are
replaced by components in `src/lib/`, most of them built on
[Bits UI](https://bits-ui.com/llms.txt) (headless Svelte 5 parts: behaviour,
keyboard and ARIA, no styling of their own):

| Component | Replaces |
|---|---|
| `DateInput.svelte` | `<input type="date">` |
| `TimeInput.svelte` | `<input type="time">` |
| `TzSelect.svelte` | `<select>` |
| `TzCombobox.svelte` | a filter box next to a `<select>` |
| `CataloguePicker.svelte` | a hand-made input + `<ul>` |
| `TzDialog.svelte` | nothing: the app had no modal |
| `NumberInput.svelte` | `<input type="number">` |
| `TzCheckbox.svelte` | `<input type="checkbox">` |
| `TextInput.svelte` | `<input>` / `<input type="email">` |
| `TzForm.svelte` | `<form>` |
| `TzTabs.svelte` | a hand-made `role="tab"` strip |
| `TzTooltip.svelte` | the native `title` attribute |
| `TzMenu.svelte` | nothing: the app had no menu (used by `TzTabs`) |
| `TzFormDialog.svelte` | a form pane beside the list |
| `RequiredMark.svelte` | nothing: nothing said a field was required |
| `TzPagination.svelte` | nothing: no list was paged (used by the record books) |

To count how often one is used, grep for it rather than keeping a number here:

```
grep -rhoP '<TzSelect(?=[\s/>])' src/ | wc -l
```

**`TzPagination` pages a list that grows with farms × years**; today only the
record books (data-model.md → "Indexes and query scope"). Three choices about
it. **It replaces the rows rather than adding to them**, which is why it is a
page control and not a "show more" button: a list that adds rows grows back into
the thousands that paging is there to avoid. **It shows nothing while the list
fits on one page**, so a user with a few farms sees a plain table. **Its words
are ours**: bits-ui labels each page button "Page 3" in English whatever the
app's language, so every button goes through the `child` snippet and sets its
own `aria-label` after the spread, where it wins. It needs no `preventScroll`:
Pagination renders plain buttons, with no floating layer to lock the body.

**Why own the controls: not because platforms look different.** The native date
picker follows the **OS** language and so overrides the language the farm chose,
on a field that appears in every register of the record book, in a project whose
`Labels`/`region.rs` design exists to respect that choice. WebKitGTK's date
input also held an input grab that only a focus change released, which no
page-side workaround could fix.

**The number field has the same problem, except it CORRUPTS data rather than
just looking wrong.** `<input type="number">` also parses with the OS locale,
and when they don't match it doesn't refuse, it reinterprets. In the WebKitGTK
webview, typing `1,5`:

| OS locale | native input stores | `NumberInput` stores |
|---|---|---|
| `es_ES` | `1.5` | `1.5` |
| `en_GB` | **`15`** | `1.5` |

So a farmer using the app in Castilian on a machine set to English enters a
dose of 1,5 l/ha and records ten times that, in a register read at an
inspection, with no error and no empty field to notice.

**`lang` doesn't fix it**: `lang="es"` on the input, on an ancestor, or on
`document.documentElement` (which `i18n.js` already sets) all still parse `1,5`
as 15. WebKit's number parser reads the application locale and ignores the
attribute. Don't reopen this.

So `NumberInput` is a plain `<input type="text" inputmode="decimal">` with the
parsing in `lib/numberValue.js` (Bits UI has no number field). Rules:

- **Both separators are accepted as the decimal point**, because a farmer typing
  `1.5` on a keypad that only has a dot means one and a half in any language.
- **Grouped input is refused, with a message.** The app never shows a thousands
  separator, so there is none to parse: `1.234,5` gets a message rather than a
  guess, which is the whole difference from the native control's silent 15.
- **An entry that can't be read blocks the form and stays on screen.** It sends
  `""` up and sets `setCustomValidity`, so it can't be submitted, and the text is
  left for the reader to correct rather than wiped.
- **`onchange` keeps its native meaning**: it fires when editing is finished, not
  on every key, because the settings fields save straight to the backend from it.
- **`step` isn't reproduced**; `integer` replaces `step="1"`. The other step
  values were spinner hints, and no decree makes a measured width a multiple of a
  centimetre.
- **`decimals` defaults to null (no limit), on purpose.** A limit of 4 would
  refuse the **five-decimal coordinates the book itself prints**, and the reason
  for a limit (that more digits couldn't be shown honestly) doesn't hold: both
  renderers widen a small value rather than round it to a false "0". Set
  `decimals` where the **domain** has a scale, not where a renderer does:
  currency will want 2, and `integer` is the zero case, with its own flag.

**The checkbox is owned for a DIFFERENT reason**, worth saying so the table
isn't read as one argument. Every control above fixes something: the date and
number fields parse with the OS locale, and the select and the date popover are
drawn by the platform. A checkbox can't record anything wrongly. What was broken
was the CSS, by omission: there were no shared rules for a checkbox with its
label, so inside `.form-grid` each tick sat above its own text and the box got a
text field's height, border and padding. Section 6's long list of good
practices (sentences from the FEGA catalogue) is where that became unreadable.

**So `TzCheckbox` is the second owned control that isn't Bits UI**, after
`NumberInput`, for the same kind of reason: the library has nothing this needs.
In bits-ui 2.18.1, `checkbox.svelte` renders `<button {...mergedProps}>` (its
`type` prop defaults to `"button"`), and `checkbox.svelte.js` only renders the
hidden form input when `name` is set (`shouldRender =
Boolean(this.root.trueName)`). Its `child` snippet gives you `role="checkbox"`,
`aria-checked` and a toggling `onclick`, so a real input placed there toggles
twice per click. A native `<input type="checkbox">` under `appearance: none`
keeps the role, the checked state, space to toggle, the label as a click target
and constraint validation, all correct and for free; `Checkbox.Group`'s array
binding is what `bind:group` already does. (No CSP issue either way: bits-ui's
`HiddenInput` passes `style` as an object, and Svelte sends a spread `style`
through `set_style` → `dom.style.cssText`, a CSSOM write, which works.)

Two consequences of owning the element instead of wrapping a library's:

- **`element.click()` works on it**, unlike every control in "Known gaps" below,
  so a scripted check can tick a box without synthesising pointer events.
- **A component can't forward `bind:group`**, which only works on `<input>`, so
  call sites that need it bind a `group` array prop instead (`value` names this
  box's entry).

**The contract is a plain string.** `DateInput` takes and returns
`"YYYY-MM-DD"`, `TimeInput` `"HH:MM"`, `TzSelect`/`TzCombobox` a code or an id,
`NumberInput` a number, and `""` means unset in all of them.
`CalendarDate`/`Time` objects live behind `lib/dateValue.js` and never leave
their component.

### Rules when adding one

- **Build items with `lib/selectItems.js`, and pick the right builder.** They
  differ in one decision: whether the list may be reordered. `codeItems(rows,
  prefix)` keeps the backend's order, because coded vocabularies carry meaning in
  their order (licence levels run basic → pilot, BBCH stages 0–9, efficacy good
  → poor) and sorting them alphabetically would be a regression.
  `nameItems(rows, …)` sorts, because entity lists arrive in SQL's BINARY order,
  which puts "Ángel" after "Zubiri".
- **A field that stores a catalogue code uses `catalogueItems` and the
  `catalogue` prop.** The backend's picker lists have every code the device
  holds, each with `offered`; the control only offers those, still shows the code
  the record has whatever its state (`pickerOptions`), and shows a code this
  device's catalogue doesn't have as the code itself, with one line saying where
  to update. Without this, a record synced from a device with newer catalogues,
  or one whose code the authority has since retired, would look like an empty
  field, and a farmer "filling it in" would replace a real value. A list shown in
  name order goes through `nameItems`, which keeps the flag; a checkbox list does
  the same by hand (`shownPractices`). Never clear a stored code because a loaded
  list doesn't have it; clear it on the change that makes it wrong.
- **Never show more than about 40 rows in a `TzSelect`.** An owned dropdown draws
  its rows in the webview instead of handing them to the OS, so the number of rows
  drawn is the real cost (see the costs table below). `TzSelect` logs a
  `console.warn` above the limit; a longer list belongs in `TzCombobox`, whose own
  input is the trigger, so the list narrows before it is drawn.
- **A shortened list says so**: `form.list_truncated` shows "Mostrando 40 de 601
  — siga escribiendo". Saying nothing would hide from a farmer that the code they
  want is below the cut, in a register where the code has legal weight.
- **Matching folds; sorting doesn't.** `lib/collate.js` has both, in one file on
  purpose. `fold()` ignores accents and case (it uppercases, because that folds
  more: "Straße" becomes STRASSE), and `searchItems()` requires every word
  (split on spaces) and ranks exact > starts with > a word starts with >
  anywhere. So "cali" finds CÁLIDO and ALCALI, and "olivo verde" finds "VERDE
  OLIVO". Ranking is what makes the limit safe: with 200 rows containing a query,
  cutting an unranked list is a coin toss. Sorting is the opposite and uses
  `Intl.Collator(… sensitivity: "variant")`, so "Pena" sorts before "Peña"
  instead of tying, mirroring `crates/terrazgo-recordbook/src/collate.rs` so both
  sides apply the same rules to whichever language each is showing.
- **A control states its problem with `setCustomValidity`, never with the
  `required` ATTRIBUTE** (`form_feedback.rs` refuses the attribute on a validity
  proxy). Both block a submit, so it looks like a free choice, but it isn't: a
  bare `required` leaves `validationMessage` as the BROWSER's text, in the OS
  language ("Please fill in this field." while the app is in Castilian). That is
  the same defect that ruled out the native date picker, one level down, and
  `validationMessage` is what TzForm's summary reads, so it has to be ours.

  Work out ONE `error` string and let it drive both the inline `<small>` and
  `setCustomValidity`, so the field and the summary can't disagree. Blocking
  still works: a `customError` alone refuses a submit just as `required` did.
- **Constraint validation uses a real input (not `type="hidden"`) placed off
  screen as `.tz-validity`**, for the controls whose visible part is a `<div>`.
  Bits UI's own hidden input isn't enough: it has no `min`, and the app has a
  range check (`TreatmentForm.svelte`: the end date can't be before the
  application date). `TextInput`, `TzCheckbox` and `CataloguePicker` need no
  proxy, since their visible element is already a real input.
- **A control takes `name` and puts it on whichever element holds the
  validity**, so `form.elements[name]` reaches it. That is how a form's `anchors`
  map shows a backend refusal on the right field. It also passes `label` on as
  `data-tz-label`, which is the name the summary uses for the field. A backend
  refusal is shown as a SECOND `.tz-field-error` line, never through
  `setCustomValidity`: a leftover custom validity would refuse the next submit
  until something cleared it, and the form would be stuck.
- **The popover is portalled to `<body>`, or, inside `TzDialog`, to the dialog's
  own content box.** The dialog nests a second `BitsConfig` to change the target
  (see "A register's form is a modal"); a control needs no `to` prop of its own.
  Every content element passes `preventScroll={false}` explicitly; the forbidden
  list below says why that isn't left to a default.
- **`TzDialog` opts out of *two* body-level layers, and it is the only control
  that has to.** bits-ui renders both `ScrollLock` and `TextSelectionLayer` from
  the same two components (`dialog-content` and `alert-dialog-content`), so
  `Select`, `Combobox`, `DatePicker` and `TimeField` have neither. The second
  layer puts an **inline** `user-select: text` on the content between pointerdown
  and pointerup, which defeats the app-wide `body { user-select: none }` and lets
  a drag select the dialog's own title. `preventOverflowTextSelection={false}`
  turns it off, and the About panel turns selection back on for its technical
  block with `user-select: text`, as `.notif-panel li span` does. **An inline
  style beats every selector, so neither could be fixed in CSS without
  `!important`.** When a library writes through CSSOM, look for its prop before
  reaching for the stylesheet.

### The tab row measures itself, and what doesn't fit goes into a menu

`TzTabs` renders a **`.tabrow`**: the `.tabstrip` of tabs that fit, then (only
when something doesn't fit) a divider and a `TzMenu` labelled *More*. It is the
same shape as GitHub's repository tabs. A sideways-scrolling strip would show
nothing to say there are more tabs, and on a phone it would fight the page's own
scrolling.

- **The split is arithmetic, in `lib/tabOverflow.js`**, in the agnostic tier, so
  it is unit-tested. The component measures; `visibleTabCount` decides.
- **Tabs keep their order.** The menu never moves the current tab into the strip:
  reordering the strip under the reader is worse than what it would fix. The
  button gets the selected marker (`.tab.is-current`) instead, and the row inside
  the menu is ticked.
- **The menu button is a SIBLING of the tab list, never a child.** A
  `role="tablist"` whose children aren't all tabs breaks the ARIA contract.
  Inside, the rows are a `RadioGroup` (`role="menuitemradio"` with
  `aria-checked`), which is how a screen reader learns that the tab it can't see
  in the strip is the current one.
- **Widths are cached, and the cache repairs itself.** A tab can only be measured
  while it is in the DOM, so `visibleTabCount` says "all of them" when it has no
  width to go on, and every pass where the row is wide enough to hold everything
  measures again. That survives a font change or a longer language without a
  stale number.
- **`resplit()` runs two passes on purpose.** The overflow group is only in the
  DOM while something overflows, so the first pass after a row runs out of room
  measures it as 0 and keeps one tab too many; the second runs with the button on
  screen. It stops because the inputs stop changing, not because of a limit.
- **The ResizeObserver watches the row, never the strip.** The split changes
  what is in the strip, so watching the strip would feed the observer its own
  output.

Three traps, all silent:

- **`.tabbar` is already taken**: it is the shell's phone navigation (`<nav
  class="tabbar">`). Reusing the name made the two swap rules in *both*
  directions, and the phone bar pushed the whole page sideways even on a 1280px
  desktop window.
- **`overflow-x: clip`, not `hidden`.** `hidden` on one axis makes the other
  `auto`, which clips the focus ring off the top and bottom of every tab. `clip`
  is the one value allowed to leave the other axis visible, and
  `overflow-clip-margin: 3px` lets the last visible tab's ring show in full.
- **A popover is above the view's sticky bands.** `--z-popover` (32) is above
  `--z-sticky` (30), because a menu hanging off the tab row would otherwise be
  drawn under a register's own `.view-head` button. It is still under
  `--z-shell`.

### Tooltips are owned too, and this one is owned for looks

`TzTooltip` replaces the native `title` attribute. A `title` is drawn by the
platform and has **no styling hook at all** (no selector, no pseudo-element), so
it follows the OS: a GTK chip on WebKitGTK, something else on Android.

**This is the first owned control taken for looks.** Every other one replaced
something that was *wrong* (the date picker overrode the farm's language,
`<input type="number">` corrupted a dose); a tooltip's colour is only
appearance. Worth knowing before more controls are added on the same argument.

What it settled:

- **Bits UI rather than a CSS-only tip.** Several triggers sit inside ancestors
  that clip (`.tabstrip` clips, `.view.framed` clips, the dialog body scrolls),
  and a tip attached to its trigger would be cut off by each. This one portals.
- **`preventScroll` isn't set, on purpose**, and only here:
  `tooltip-content.svelte` sets `preventScroll={false}` itself, so the lock is
  never created.
- **`disableHoverableContent` is set, and it couldn't be done in CSS.** bits-ui
  writes `pointer-events` inline from that flag, and an inline style beats a
  stylesheet rule: a `none` in `.tz-tooltip` came out as `auto`. A tip that
  swallows a click on the control under it is a bug.
- **Every trigger uses the `child` snippet.** The call sites are existing `<a>`,
  `<button>` and `<span>` elements, and a wrapping trigger button would put a
  button inside a button. The caller spreads `props` onto its own element; where
  it also needs one of the handlers bits-ui supplies, it CALLS the one from
  `props` instead of replacing it (`ColumnResizer` does this for `pointerdown`,
  which is what hides the tip as a drag starts, since pointer capture means the
  leave event never arrives).

An empty `label` renders no tooltip, just the trigger, which is how the sidebar
shows one only while collapsed, and a zone chip only when it has a detail.

### Forbidden Bits UI components — and the one exception, with its evidence

The rule isn't "these four components are broken". It is:

> **A view never imports bits-ui. An owned control may, and every one of them
> passes `preventScroll={false}` explicitly.**

Every bits-ui import in `src/` is inside an owned wrapper (plus `BitsConfig` in
`App.svelte`), and that is what keeps the app safe under the production CSP.

**The defect.** bits-ui's body scroll lock is *applied* through CSSOM
(`document.body.style.pointerEvents = "none"`, allowed) and *released* through
`document.body.setAttribute("style", …)`, which `default-src 'self'` blocks
(`style-src-attr`, see Styling). A lock that is applied therefore never comes
off, leaving `pointer-events: none` on `<body>`: the app stops responding to the
mouse until it is restarted.

**Why it is about a default and not a component.** In bits-ui 2.18.1:

- `document.body.setAttribute("style", …)` appears **once in the whole package**,
  in `internal/body-scroll-lock.svelte.js`, inside `resetBodyStyle()`;
- `resetBodyStyle()` is only reachable from `BodyScrollLock`'s teardown;
- `new BodyScrollLock` appears **once**, in
  `utilities/scroll-lock/scroll-lock.svelte`, inside `if (preventScroll)`;
- `<ScrollLock>` is rendered by **three** components: `dialog-content`,
  `alert-dialog-content`, and `utilities/popper-layer/popper-layer-inner.svelte`,
  which **every floating layer goes through**, menus included;
- popper-layer-inner uses `preventScroll ?? true`, so **leaving the prop out
  means the lock IS created**. `Select.Content` and `Combobox.Content` default
  `preventScroll` to `false` themselves and are safe on their own;
  `DropdownMenu.Content` has no default, so setting it there is required.

So `preventScroll={false}` means the lock is never **created** and the blocked
line can't be reached. It costs nothing here: the app sets `body { overflow:
hidden }`, so there is no body scroll to lock, and what the lock would otherwise
give (blocking interaction behind the panel) comes from `Dialog.Overlay` and the
focus trap, which aren't affected. After a bits-ui upgrade, check these facts
again rather than trusting the list of names.

**`TzDialog.svelte` is the only allowed `Dialog` import and `TzMenu.svelte` the
only allowed `DropdownMenu`**, each with a targeted `eslint-disable-next-line
no-restricted-imports` and the reason beside it. `AlertDialog` and `ContextMenu`
stay banned outright: nothing needs them, and an unused exception is one nobody
re-checks. `Menubar` is safe (it passes `preventScroll={false}` itself).

Check: every hit outside the two owned controls is a bug:

```
grep -rn "Dialog\.\|DropdownMenu\.\|ContextMenu\." src/ \
     --exclude=TzDialog.svelte --exclude=TzMenu.svelte
```

Destructive confirmations still use `confirmDialog()` (the native dialog
plugin), which isn't affected, and it is still the right tool for a yes/no
question because it is the platform's own.

**Anything here has to be checked again under the production CSP, and only one
kind of build does that**: neither the scripted frontend checks nor the
app-level harness apply a policy, and `cargo build --release` doesn't either,
because it still loads `devUrl`. See [stack-choices.md](stack-choices.md) §2 →
"How to check a CSP question".

### `CataloguePicker` is not `TzCombobox`, and the difference is the contract

Both narrow a long list as you type, but they answer different questions.
`TzCombobox` answers *which code?*: the typed text is just a search and the value
is the code alone. `CataloguePicker` answers *what is this called?*: the typed
text **is** a stored value, the name the record book prints, and the code is
optional extra information that comes from it. Free text is allowed and must
survive: a farmer may grow something the catalogue doesn't list.

So the rule it exists for is that **a name and a code must never disagree**.
Picking a row sets both; typing away from a picked row keeps the name and drops
the code; typing the exact name again brings the code back. Nine checks cover
exactly that, and they must pass for any change to this component.

Two things to know before editing it:

- **The text goes through `bind:inputValue` on `Combobox.Root`.** Bits UI merges
  the input's `value` last, so it always owns that attribute and passing `value`
  through props does nothing; the root's two-way `inputValue` is the supported
  way in. `clearOnDeselect` defaults to false and nothing resets the text on
  close, which is what lets free text survive at all.
- **`Home` and `End` move the caret.** Bits UI uses them to jump to the first and
  last row and calls `preventDefault`, which is right for `TzCombobox` and wrong
  here, where the input holds a name being edited. `mergeProps` runs our handler
  first and stops the chain once the event has had its default prevented, so the
  component handles those two keys and moves the caret itself.
  `allowDeselect={false}` for a similar reason: picking the row already chosen
  must keep it, not toggle it off and silently drop the code.

### Android

On Android the native select is a full-screen picker made for thumbs, the best
of the three platforms' controls, so an owned control has to *match* it, not just
replace it. On a real phone (Android WebView, driven with real touch events):

- `.tz-option` rows are exactly **44 px** tall under `@media (pointer: coarse)`;
- the list is at most **256 px** tall and as wide as the screen, with **no
  sideways page scroll**, and the popover clears the bottom tab bar;
- tapping a row picks the value; the species picker narrows and says "Showing 40
  of 1023 — keep typing".

Two things to keep: padding alone gives a one-line row of 38 px, so the 44 px
minimum has to be set; and `.tz-listbox` must name both `--bits-select-*` and
`--bits-combobox-*` variables, because a combobox emits the second set and with
only the first, the width minimum and height limit are dropped (a 40-row list
then came out 1244 px tall). A headless library's own sizing defaults are part
of what you adopt, and you only see them when a long list meets a narrow screen.

### Costs

The number of rows drawn is the cost that matters. Minified, gzipped, in a test
app; "open" is from pointerdown to painted, with the CPU throttled 6× to stand in
for a phone:

| rows drawn | open @ 1× | open @ 6× |
| --- | --- | --- |
| 40 | 66 ms | 260 ms |
| 602 | 179 ms | 897 ms |
| 2000 | 384 ms | 2 146 ms |

Bits UI added about 65 kB gzipped to the entry chunk, against a budget of 90 kB
and an installer of several MB. It brought two npm runtime dependencies
(`bits-ui`, `@internationalized/date`), and CI checks `npm audit
--audit-level=high`.

## Known gaps

- **The UI has no automated tests**, on purpose while it keeps changing (testing
  strategy #5, architecture.md). It is checked by scripts, though: a
  headless-Chrome harness over the built bundle (with stubbed or recorded
  backend data) and an app-level harness driving the real debug binary in the
  real webview (screenshots through X11).
- **Driving an owned control needs real pointer events.** A synthetic
  `element.click()` neither opens a trigger nor picks a row, so a scripted check
  written that way passes while checking nothing. In short: use pointer events,
  let scrolling settle before clicking (the popover is portalled and positioned
  from a measured anchor), and remember that the first row opens already
  highlighted.
