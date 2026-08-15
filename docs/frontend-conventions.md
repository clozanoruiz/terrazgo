# Frontend conventions — Svelte 5 + plain JS

> How the `src/` frontend is written and extended — the working reference.
> The architectural rationale is in [architecture.md](architecture.md) →
> "The frontend in one page".

## The two-tier rule

The frontend has a framework-agnostic core that must stay free of Svelte
imports, and views that may use anything Svelte offers:

| Tier | Files | May import Svelte? |
|---|---|---|
| Framework-agnostic | `i18n.js`, `i18n/<locale>/*.js`, `lib/backend.js`, `lib/nav.js`, `lib/dateValue.js`, `lib/collate.js`, `lib/selectItems.js` | **No** |
| Reactive glue | `lib/notifications.svelte.js`, `lib/lookups.svelte.js` (runes modules) | Runes only |
| Views + wiring | `App.svelte`, `lib/*View.svelte`, `lib/*Form.svelte`, `lib/routes.js` | Yes |

`lib/routes.js` is in the view tier deliberately: it names components, so it
imports Svelte. `nav.js` stays agnostic and says what the navigation *offers*;
`routes.js` says what each route *renders*, and the two lists differ —
`#/farms/<id>` is a route with no nav entry.

The point: business logic lives in Rust behind `invoke`, and the agnostic tier
survives a future framework swap untouched — only views would be rewritten.

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
- **Forms**: `<form onsubmit={fn}>` with `event.preventDefault()` inside, HTML
  `required`/`min`/`step` for first-line validation, submit via the form so
  browser validation runs. The form is the source of truth on save (full-state
  payloads, not diffs).
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
  the failure is seen. Success feedback is pushed with `notify(t("message.…"))`.
  Notifications accumulate in the bell (`NotificationBell.svelte`, one
  instance per layout) until dismissed individually or cleared; a locale
  switch clears them all (they hold interpolated text in the old language).

## i18n rules

- Never hardcode a user-facing string in markup or JS. Add a key to **every**
  locale — the i18n contract test (`src-tauri/tests/i18n_contract.rs`) reads
  them all, so it fails the build on divergent key sets or mismatched
  `{placeholders}` in any locale, including ones added later.
- **Each locale is a directory of area files** (2026-08-13): `src/i18n/es/`
  holds `common`, `errors`, `farm`, `book`, `fertilisation`, `map` and
  `settings`, and `src/i18n/es.js` is the entry point that merges them and
  holds no entries of its own. A new module adds one file per locale instead of
  editing three thousand-line dictionaries. **A key may live in exactly one area
  file per locale** — the merge would silently prefer the last one — and a
  contract test refuses duplicates.
- `t(key, params)` for normal strings; `tCode(prefix, code)` for schema codes
  (`tCode("unit", "l_ha")` → key `unit.l_ha`) — falls back to the raw code so a
  new schema value degrades gracefully; `formatDate(iso)` for `YYYY-MM-DD`
  values (parses field-by-field to avoid UTC-midnight off-by-one).
- **`localeTag()` is the regional tag for every `Intl` consumer** — date
  formatting, the owned date/time controls, and `Intl.Collator`. It maps through
  `DATE_LOCALE`, and the entry that matters is **`en` → `en-GB`**: bare `en`
  resolves to US in `Intl`, so English would print `08/03/2026` where every other
  locale in the app prints `03/08/2026`, and English here means European English.
  `formatDate` and the owned fields read the same function, so a date the app
  renders and a date it lets you edit can never disagree.
- **Ordering display lists goes through `lib/collate.js`**, not `.sort()`. SQL
  returns BINARY order (`Á` is U+00C1, so "Ángel" lands after "Zubiri"; "Parcela
  10" before "Parcela 2"). The module mirrors
  `crates/terrazgo-recordbook/src/collate.rs` — same CLDR data, `numeric: true`,
  accents distinguished — so the screen and the printed book agree.
- User-entered data (farm names, species, notes) is never translated.
- Adding a language = one `SUPPORTED` entry in `i18n.js` + one directory with
  the same area files and the full key set.

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
6. Frontend: call through `run()`, push a `message.*` key via `notify()` on
   success.

## Styling

- One global stylesheet (`src/styles.css`), plain CSS, no preprocessor. Shared
  vocabulary belongs there; **rules that only one component can use belong in
  that component's `<style>` block** (6 of them today, `MapView` the largest) —
  the earlier "no scoped blocks" line stopped matching the code and Svelte's
  scoping is the right tool for the local case.
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
- Icons are inline SVG path data (24×24 Feather outlines, MIT), stroked with
  `currentColor` — no icon font, no image files (CSP: `default-src 'self'`).
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

The app owns its form controls. Every native `<select>`, `<input type="date">`
and `<input type="time">` is gone, and so is the last hand-rolled one — replaced
by five components in `src/lib/` built on
[Bits UI](https://bits-ui.com/llms.txt) (headless Svelte 5 primitives —
behaviour, keyboard and ARIA, no styling of its own):

| Component | Replaces | Call sites |
|---|---|---|
| `DateInput.svelte` | `<input type="date">` | 19 |
| `TimeInput.svelte` | `<input type="time">` | 1 |
| `TzSelect.svelte` | `<select>` | 78 |
| `TzCombobox.svelte` | a filter box beside a `<select>` | 4 |
| `CataloguePicker.svelte` | its own hand-rolled input + `<ul>` | 3 |

**The reason is not that platforms look different.** It is that the native date
picker follows the **OS** locale and so overrides the language the holding chose
— on a field that appears in every register of the record book, in a project
whose `Labels`/`region.rs` design exists to honour exactly that choice. The
WebKitGTK input grab (a popover only a focus change released) is the second
defect, and owning the control is what the 2026-07-03 decision to ban page-side
workarounds always said the real fix would be.

**The contract is a plain string, and that is what made the migration a markup
swap.** `DateInput` takes and returns `"YYYY-MM-DD"`, `TimeInput` `"HH:MM"`,
`TzSelect`/`TzCombobox` a code or an id — `""` for unset in all four. No view
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
  rather than tying — mirroring `crates/terrazgo-recordbook/src/collate.rs` so a
  picker on screen and a cell in the printed book cannot disagree.
- **`required` and `min` still block submit**, because a real (not
  `type="hidden"`) input parked off-screen as `.tz-validity` carries them and the
  browser runs constraint validation on it as usual. Bits UI's own hidden input
  was not enough — it carries no `min`, and the app has exactly one range guard
  (`TreatmentForm.svelte`, end date not before application date).
- **The popover is portalled to `<body>`** and every content element passes
  `preventScroll={false}` explicitly — see the forbidden list below for why that
  is not left to a default.

### Forbidden Bits UI components

**Never use `Dialog`, `AlertDialog`, `DropdownMenu` or `ContextMenu`.** They
engage bits-ui's body scroll lock, which *releases* by calling
`document.body.setAttribute("style", …)` — blocked by the production CSP
(`style-src-attr`, see Styling above). The lock therefore goes on and never comes
off, leaving `pointer-events: none` on `<body>` for good: the whole app stops
responding to the mouse until it is restarted. Menubar is safe (it passes
`preventScroll={false}` itself). Guard:

```
grep -rn "Dialog\.\|DropdownMenu\.\|ContextMenu\." src/    # must stay empty
```

Destructive confirmations keep using `confirmDialog()` (the native dialog
plugin), which is unaffected.

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
