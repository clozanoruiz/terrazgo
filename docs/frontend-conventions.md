# Frontend conventions — Svelte 5 + plain JS

> How the `src/` frontend is written and extended — the working reference.
> The architectural rationale is in [architecture.md](architecture.md) →
> "The frontend in one page".

## The two-tier rule

The frontend has a framework-agnostic core that must stay free of Svelte
imports, and views that may use anything Svelte offers:

| Tier | Files | May import Svelte? |
|---|---|---|
| Framework-agnostic | `i18n.js`, `i18n/<locale>.js`, `lib/backend.js`, `lib/nav.js` | **No** |
| Reactive glue | `lib/notifications.svelte.js` (runes module) | Runes only |
| Views | `App.svelte`, `lib/*View.svelte`, `lib/*Form.svelte` | Yes |

The point: business logic lives in Rust behind `invoke`, and the agnostic tier
survives a future framework swap untouched — only views would be rewritten.

## Svelte 5 idioms in use

- **Runes everywhere**: `$state`, `$derived`, `$props`. No stores, no legacy
  `export let`, no `$:` statements, no `createEventDispatcher` — child
  components receive **callback props** (`onSaved`, `onCancel`) instead of
  emitting events.
- **Dynamic lists** mutate `$state` arrays in place (`rows.push(...)`,
  `rows.splice(i, 1)`) and key each block on object identity:
  `{#each rows as row (row)}`.
- **Locale switching**: `App.svelte` wraps the routed content in
  `{#key localeVersion}` and bumps the key on `onLocaleChange`, so every `t()`
  call re-evaluates by remount. Components never subscribe to locale
  themselves.
- **Routing** is a hand-rolled hash router in `App.svelte` (`#/status`,
  `#/farms`, `#/farms/<id>`, `#/treatments`). Fine at this size; revisit only
  if routes multiply.
- **Navigation is data**: top-level destinations live in `lib/nav.js`
  (`NAV_ITEMS`: route, i18n label key, SVG icon path) and `App.svelte` renders
  that list twice — as the collapsible sidebar on wide screens and as the
  bottom tab bar on narrow ones (media query at 700px; there is no desktop
  menu bar). `activeRoute(hash)` picks the highlighted entry by longest
  route prefix. Adding a view = one `NAV_ITEMS` entry + a router branch;
  never hardcode a nav link in markup. The collapsed-sidebar state persists
  in `localStorage` (`terrazgo.sidebar`), like the locale. Per-view actions
  (buttons) belong in the view itself, not in a global toolbar.
- **Forms**: `<form onsubmit={fn}>` with `event.preventDefault()` inside, HTML
  `required`/`min`/`step` for first-line validation, submit via the form so
  browser validation runs. The form is the source of truth on save (full-state
  payloads, not diffs).
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
  locale file (`src/i18n/es.js`, `en.js`) — the i18n contract test
  (`src-tauri/tests/i18n_contract.rs`) fails the build on divergent key sets or
  mismatched `{placeholders}`.
- `t(key, params)` for normal strings; `tCode(prefix, code)` for schema codes
  (`tCode("unit", "l_ha")` → key `unit.l_ha`) — falls back to the raw code so a
  new schema value degrades gracefully; `formatDate(iso)` for `YYYY-MM-DD`
  values (parses field-by-field to avoid UTC-midnight off-by-one).
- User-entered data (farm names, species, notes) is never translated.
- Adding a language = one `SUPPORTED` entry in `i18n.js` + one dictionary file.

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
2. Thin `#[tauri::command]` wrapper in `src-tauri/src/commands.rs` — no logic,
   just `lock_conn` + repo call + `?`.
3. Register it in `generate_handler!` in `src-tauri/src/lib.rs`
   (`command_registration.rs` contract test fails otherwise).
4. If it can emit a new `Invalid("code")`, add `error.invalid.<code>` to every
   dictionary (i18n contract test fails otherwise).
5. If it changes alert inputs, call `refresh_alerts` before returning.
6. Frontend: call through `run()`, push a `message.*` key via `notify()` on
   success.

## Styling

- One global stylesheet (`src/styles.css`), plain CSS, no preprocessor and no
  component-scoped `<style>` blocks so far — keep it that way until there's a
  reason not to.
- Reuse the existing vocabulary before inventing new classes: `.view`,
  `.view-head`, `.form-grid`, `.form-actions`, `.card-list`/`.card`(+`.stack`),
  `fieldset.es-only` (country-conditional sections), `fieldset.subsection`,
  `.btn-danger`/`.btn-cancel`. App-shell classes (`.sidebar`, `.topbar`,
  `.tabbar`, `.lang-select`, `.main-head`, `.bell*`/`.notif*`) belong to
  `App.svelte`/`NotificationBell.svelte` — views never use them.
- Icons are inline SVG path data (24×24 Feather outlines, MIT), stroked with
  `currentColor` — no icon font, no image files (CSP: `default-src 'self'`).
- Production CSP is `default-src 'self'`: no inline styles/scripts, no CDN
  anything. The dev-only CSP additions (`devCsp`) exist solely for Vite HMR.

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

## Known gaps (as of 2026-07-02)

- **UI has no automated tests** — deliberate while the UI is in flux
  (testing strategy #5, architecture.md). Runtime verification is scripted
  though: a headless-Chrome harness over the built bundle (error-stub or
  backend-harvested fixtures) and an app-level harness driving the real
  debug binary in the real webview (screenshot via X11).
