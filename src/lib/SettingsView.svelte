<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Settings view: device-local preferences. Three storage tiers meet here —
  // the display language (localStorage: per-device, read synchronously at
  // startup by i18n.js), the settings file (settings.json via the backend),
  // and the backup actions (moved from the Status view: maintenance chores
  // belong with settings, alerts stay with status).
  //
  // The screen is ONE scrolling document with a table of contents beside it
  // (SettingsToc.svelte) and a search field above it. Neither hides anything:
  // the tree scrolls to a heading and the search narrows what is on screen, so
  // a setting is always either visible or one keystroke away. Which sections
  // exist, what is inside them and what text each setting is findable by are
  // declared once in settingsTree.js and rendered from there by both — the
  // nav.js arrangement, for the same reason.
  import {
    formatMode,
    formatModes,
    formatUnit,
    languageTag,
    locale,
    locales,
    nativeName,
    setFormatMode,
    setLocale,
    t,
  } from "../i18n.js";
  import { X } from "@lucide/svelte";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import NumberInput from "./NumberInput.svelte";
  import AboutPanel from "./AboutPanel.svelte";
  import SettingsCatalogues from "./SettingsCatalogues.svelte";
  import SettingsProfiles from "./SettingsProfiles.svelte";
  import SettingsToc from "./SettingsToc.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzDialog from "./TzDialog.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { searchSettings, settingsAnchor, settingsAnchors } from "./settingsTree.js";

  // { settings, tile_cache_default_bytes } — the default rides along so an
  // unset cap can display its effective value without the frontend hardcoding
  // a copy of the Rust constant.
  let info = $state(null);
  let loading = $state(true);
  // About is a panel in a dialog rather than a screen of its own: it is
  // read once and dismissed, and a seventh destination would push the
  // phone tab bar past what that layout carries.
  let aboutOpen = $state(false);

  run(async () => {
    info = await invoke("get_settings");
  }).finally(() => (loading = false));

  // --- search and the contents list ------------------------------------------

  let query = $state("");
  const searchId = $props.id();

  /// Clearing puts the focus back in the field: the reader's next move after
  /// emptying a search is almost always to type another one.
  function clearSearch() {
    query = "";
    document.getElementById(searchId)?.focus();
  }

  // The scroller on wide screens (.view carries `overflow-y: auto` there), and
  // the element the scroll spy measures headings against.
  let viewEl = $state(null);
  // The sticky search band, whose height is the strip a jumped-to heading has
  // to clear.
  let bandEl = $state(null);
  let current = $state(settingsAnchors()[0]);

  const result = $derived(searchSettings(query, t));
  /// The one predicate the whole screen uses: a section, a group and a single
  /// setting all answer it the same way.
  const shown = (id) => result.hits[id] > 0;

  /// Which heading the reader is looking at: the last one at or above the top
  /// of the scroller. Deterministic rather than an IntersectionObserver, which
  /// answers "is this in view" — a question with no answer in the gap between
  /// two headings, and gaps are most of a settings screen.
  ///
  /// Cheap enough to do on every frame: nine headings, and it only runs on wide
  /// screens because that is the only place the tree exists to be updated.
  function updateCurrent() {
    if (!viewEl || pinned) return;
    const top = viewEl.getBoundingClientRect().top;
    let best = null;
    for (const id of settingsAnchors()) {
      const el = document.getElementById(settingsAnchor(id));
      // Absent means the search filtered it out; skip rather than stop, since
      // later headings may still be on screen.
      if (!el) continue;
      if (el.getBoundingClientRect().top - top > 8) break; // in document order
      best = id;
    }
    current = best ?? settingsAnchors().find((id) => shown(id)) ?? "";
  }

  // A node the reader clicked stays lit until they scroll for themselves.
  //
  // The spy cannot manage this alone: the last sections can never reach the top
  // of the scroller, because there is not enough document below them to push
  // them there — so a jump to one of them was overruled within the frame by a
  // heading three sections back, and clicking "Mantenimiento" lit "Perfiles"
  // (measured 2026-09-04).
  //
  // Cleared by the reader moving the pane themselves rather than by a scroll
  // event, because the smooth jump the click started fires plenty of those
  // itself. Two signals cover the two ways that happens on a wide screen — the
  // only width where the tree exists at all: a wheel, and focus landing inside
  // the settings, which is what moves the viewport for a keyboard reader and
  // is the only way a key press could scroll this pane in the first place.
  //
  // Deliberately NOT `touchmove` or `keydown`. Touch cannot reach this: the
  // tree is hidden below 700px, so nothing is ever pinned there. And a `keydown`
  // on the frame is what Svelte's a11y rule flags — correctly, since a
  // container that answers keys should be reachable — where `focusin` describes
  // what is actually being watched for.
  //
  // Not $state: nothing renders it.
  let pinned = false;

  function release() {
    pinned = false;
  }

  // One measurement per frame at most: `scroll` fires far faster than the tree
  // can usefully change.
  let ticking = false;

  function onScroll() {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      updateCurrent();
    });
  }

  // Filtering moves every heading, so the current one has to be re-measured
  // after the DOM settles — which is when an effect runs. It also releases a
  // pinned node, which the query may have removed from the tree entirely.
  $effect(() => {
    void result;
    pinned = false;
    updateCurrent();
  });

  /// Jump to a heading by scrolling the view, and ONLY the view.
  ///
  /// Not `scrollIntoView`: it scrolls every scrollable ancestor, and <main> is
  /// one of them — `overflow: hidden` still permits a programmatic scroll, so
  /// the shell itself slid 319px up and stayed there, taking the sticky search
  /// band off the top of the window with it (measured 2026-09-04).
  ///
  /// The band's own height is read rather than assumed: it is the strip the
  /// heading has to clear, and a token could drift from what the band actually
  /// measures once it holds a control.
  function goTo(id) {
    const el = document.getElementById(settingsAnchor(id));
    if (!el || !viewEl) return;
    const offset = el.getBoundingClientRect().top - viewEl.getBoundingClientRect().top;
    const band = bandEl?.getBoundingClientRect().height ?? 0;
    // A jump the reader asked for is not motion they need protecting from, but
    // the preference exists precisely to be honoured without being asked.
    const still = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    viewEl.scrollTo({
      top: viewEl.scrollTop + offset - band,
      behavior: still ? "auto" : "smooth",
    });
    current = id;
    pinned = true;
  }

  // --- the settings themselves -----------------------------------------------

  // setLocale is async (it may lazy-load a dictionary); a rejection means a
  // locale file is missing from the bundle — log it, keep the previous language.
  // Mirrors the active locale so a FAILED switch can put the picker back. On
  // success this component is torn down by the shell remount and rebuilt with
  // the new locale, so nothing here has to undo itself.
  let localeChoice = $state(locale());

  function switchLocale(code) {
    setLocale(code).catch((err) => {
      console.error(err);
      localeChoice = locale();
    });
  }

  let formatChoice = $state(formatMode());

  /// The figure each option would produce, so the choice is legible without
  /// knowing what "system" resolves to on this machine. 1234,5 is the shape
  /// that differs most between conventions while staying a plausible area.
  function sampleFor(mode) {
    const tag = mode === "language" ? languageTag() : undefined;
    return new Intl.NumberFormat(tag, { maximumFractionDigits: 4, useGrouping: false }).format(
      1234.5,
    );
  }

  /// Switching re-renders the whole shell, exactly like a language switch:
  /// every figure on screen has just changed convention.
  function switchFormat(mode) {
    setFormatMode(mode);
    formatChoice = formatMode();
  }

  const MIB = 1024 * 1024;
  // Preset cache sizes; the empty select value means "follow the default".
  const CACHE_PRESETS = [256 * MIB, 512 * MIB, 1024 * MIB, 2048 * MIB];

  // Binary thresholds with the familiar GB/MB/kB labels, as before; only the
  // number is now the reader's ("2,5 GB" in Castilian, "2.5 GB" in English)
  // instead of a hardcoded decimal point.
  function formatSize(bytes) {
    if (bytes >= 1024 * MIB) return formatUnit(bytes / (1024 * MIB), "gigabyte");
    if (bytes >= MIB) return formatUnit(Math.round(bytes / MIB), "megabyte", 0);
    return formatUnit(Math.max(1, Math.round(bytes / 1024)), "kilobyte", 0);
  }

  function changeCacheSize(value) {
    run(async () => {
      const settings = {
        ...info.settings,
        tile_cache_max_bytes: value === "" ? null : Number(value),
      };
      info = await invoke("update_settings", { settings });
    });
  }

  // Day-count settings (alert lead times, the map's treated-plot horizon).
  // Saved on change rather than on a submit button, like the cache size — and
  // NumberInput keeps the native `change` meaning, firing on blur or Enter
  // rather than on every keystroke, so a half-typed "9" of "90" is never sent.
  // It hands over a number or "" where the native input handed over a string;
  // both readings below already covered that. An empty field means "follow the
  // app default", the same null the cache preset uses.
  function changeDays(field, value) {
    run(async () => {
      const settings = { ...info.settings, [field]: value === "" ? null : Number(value) };
      info = await invoke("update_settings", { settings });
    });
  }

  // Check, then compact — one action because the check GATES the compaction:
  // VACUUM rebuilds the file by reading every page, so running it on a damaged
  // database would entrench the damage rather than reveal it. A bad verdict is
  // an outcome, not a command error, so it comes back Ok and is notified as an
  // error here (which opens the panel).
  let checking = $state(false);

  function checkDatabase() {
    checking = true;
    run(async () => {
      const report = await invoke("check_and_compact_database");
      if (!report.integrity.ok) {
        notify(t("message.db_check_failed"), true);
        return;
      }
      const freed = report.size_before_bytes - report.size_after_bytes;
      notify(
        freed > 0
          ? t("message.db_checked_freed", { size: formatSize(freed) })
          : t("message.db_checked_clean"),
      );
    }).finally(() => (checking = false));
  }

  function clearCache() {
    run(async () => {
      if (!(await confirmDialog(t("settings.clear_cache_confirm")))) return;
      const count = await invoke("clear_tile_cache");
      notify(t("message.cache_cleared", { count }));
    });
  }

  // The dialog plugin is invoked directly (plugin:dialog|…) — same transport
  // the official @tauri-apps/plugin-dialog JS wrapper uses, no npm package.
  function exportBackup() {
    run(async () => {
      const stamp = new Date().toISOString().slice(0, 10);
      const path = await invoke("plugin:dialog|save", {
        options: {
          defaultPath: `terrazgo-backup-${stamp}.db`,
          filters: [{ name: "SQLite", extensions: ["db"] }],
        },
      });
      if (!path) return;
      const summary = await invoke("export_backup", { destPath: path });
      notify(
        t("message.backup_saved", { path: summary.path, size: formatSize(summary.size_bytes) }),
      );
    });
  }

  function importBackup() {
    run(async () => {
      const selection = await invoke("plugin:dialog|open", {
        options: {
          multiple: false,
          directory: false,
          filters: [{ name: "SQLite", extensions: ["db"] }],
        },
      });
      const path = Array.isArray(selection) ? selection[0] : selection;
      if (!path) return;
      if (!(await confirmDialog(t("backup.import_confirm")))) return;
      const summary = await invoke("import_backup", { srcPath: path });
      notify(t("message.backup_imported", { path: summary.safety_backup_path }));
    });
  }
</script>

<section class="view settings-view" bind:this={viewEl} onscroll={onScroll} onwheel={release}>
  <div class="settings-layout">
    <SettingsToc hits={result.hits} filtering={result.filtering} {current} onnavigate={goTo} />

    <!-- `focusin` here and not on the frame: the tree is inside the frame too,
         and clicking one of its nodes would release the pin the click just
         set. -->
    <div class="settings-pane" onfocusin={release}>
      <!-- The band spans the settings themselves and not the contents list
           beside them, so it cancels .view-head's full-bleed inset. -->
      <!-- A `.tz-control` box rather than a TextInput: the count and the clear
           button belong INSIDE the field, and that box is the app's existing
           idiom for a control holding an input plus adornments (TzCombobox's
           input sits in one the same way). Nothing here is a form field —
           no validity, no submission, nothing stored — so the owned-control
           plumbing has nothing to do. -->
      <div class="view-head settings-search" bind:this={bandEl}>
        <div class="form-grid">
          <div class="tz-field">
            <label class="tz-label" for={searchId}>{t("settings.search")}</label>
            <div class="tz-control settings-search-box">
              <input id={searchId} type="text" bind:value={query} />
              {#if result.filtering}
                <span class="settings-count">
                  {t("settings.results", { count: result.total })}
                </span>
                <button
                  type="button"
                  class="tz-field-trigger"
                  aria-label={t("settings.clear_search")}
                  onclick={clearSearch}
                >
                  <X />
                </button>
              {/if}
            </div>
          </div>
        </div>
      </div>

      {#if result.filtering && result.total === 0}
        <p class="settings-empty">{t("settings.no_results")}</p>
      {/if}

      <!-- General --------------------------------------------------------- -->
      {#if shown("general")}
        <h3 id={settingsAnchor("general")}>{t("settings.general")}</h3>
      {/if}

      {#if shown("general.language")}
        <div class="view-head" id={settingsAnchor("general.language")}>
          <h4>{t("settings.group.language")}</h4>
        </div>
        {#if shown("language")}
          <!-- The one control whose own change remounts the shell it lives in
               (App.svelte's {#key localeVersion}), so its portalled listbox is
               torn down by that remount rather than left orphaned in <body>. -->
          <TzSelect
            label={t("lang.label")}
            items={locales().map((code) => ({ value: code, label: nativeName(code) }))}
            bind:value={localeChoice}
            onchange={switchLocale}
          />
        {/if}
        {#if shown("format")}
          <!-- Which convention numbers and dates follow. A separate question
               from the language, and defaulted to the machine because that is
               where most people have already answered it — the toggle is for
               the farmer reading a Castilian book on an English-configured
               phone. Each option shows the figure it produces, because "system"
               means nothing without the example. -->
          <TzSelect
            label={t("format.label")}
            items={formatModes().map((mode) => ({
              value: mode,
              label: t(`format.${mode}`, { sample: sampleFor(mode) }),
            }))}
            bind:value={formatChoice}
            onchange={switchFormat}
          />
          <p>{t("format.hint")}</p>
        {/if}
      {/if}

      {#if shown("general.alerts")}
        <div class="view-head" id={settingsAnchor("general.alerts")}>
          <h4>{t("settings.alerts")}</h4>
        </div>
        {#if loading}
          <Skeleton />
        {:else if info}
          <p>
            {t("settings.alerts_hint", {
              licence: info.licence_lead_default_days,
              itv: info.itv_lead_default_days,
            })}
          </p>
          <!-- Bounds match module-cue's validate_lead_days, which is the
               authority: the input keeps an out-of-range value from being sent
               at all, and the backend still refuses one that arrives another
               way. -->
          <div class="form-grid" id="alert-leads" aria-label={t("settings.alerts")}>
            {#if shown("licence_lead")}
              <NumberInput
                label={t("settings.licence_lead")}
                integer
                min={1}
                max={400}
                placeholder={String(info.licence_lead_default_days)}
                value={info.settings.licence_lead_days ?? ""}
                onchange={(v) => changeDays("licence_lead_days", v)}
              />
            {/if}
            {#if shown("itv_lead")}
              <NumberInput
                label={t("settings.itv_lead")}
                integer
                min={1}
                max={400}
                placeholder={String(info.itv_lead_default_days)}
                value={info.settings.itv_lead_days ?? ""}
                onchange={(v) => changeDays("itv_lead_days", v)}
              />
            {/if}
          </div>
        {/if}
      {/if}

      <!-- Map ------------------------------------------------------------- -->
      {#if shown("map")}
        <h3 id={settingsAnchor("map")}>{t("settings.map")}</h3>
      {/if}

      {#if shown("map.offline")}
        <div class="view-head" id={settingsAnchor("map.offline")}>
          <h4>{t("settings.group.offline_maps")}</h4>
        </div>
        {#if loading}
          <Skeleton />
        {:else if info}
          {#if shown("cache_size")}
            <!-- The one select whose option values were numbers. They become
                 strings, which is what the handler already parsed them back
                 from: a native option value is a string too. -->
            <TzSelect
              label={t("settings.cache_size")}
              items={CACHE_PRESETS.map((bytes) => ({
                value: String(bytes),
                label: formatSize(bytes),
              }))}
              nullable
              nullLabel={t("settings.cache_default", {
                size: formatSize(info.tile_cache_default_bytes),
              })}
              value={info.settings.tile_cache_max_bytes == null
                ? ""
                : String(info.settings.tile_cache_max_bytes)}
              onchange={changeCacheSize}
            />
            <p>{t("settings.cache_hint")}</p>
          {/if}
          {#if shown("clear_cache")}
            <div id="cache-actions" aria-label={t("settings.group.offline_maps")}>
              <button type="button" onclick={clearCache}>{t("settings.clear_cache")}</button>
            </div>
          {/if}
        {/if}
      {/if}

      {#if shown("map.treated")}
        <div class="view-head" id={settingsAnchor("map.treated")}>
          <h4>{t("settings.group.treated_plots")}</h4>
        </div>
        {#if loading}
          <Skeleton />
        {:else if info}
          <!-- Bounds match module-cue's validate_phi_horizon_days. The ceiling
               is not cosmetic: this value IS the query's WHERE clause, so it is
               what keeps the tint from reading the whole record book. The
               placeholder is the bare number, not "Default: 90 days" — a number
               input is one column wide and truncated a longer string to
               "Defa". -->
          <div class="form-grid">
            <NumberInput
              label={t("settings.phi_horizon")}
              integer
              min={7}
              max={730}
              placeholder={String(info.phi_recent_default_days)}
              value={info.settings.phi_recent_days ?? ""}
              onchange={(v) => changeDays("phi_recent_days", v)}
            />
          </div>
          <p>{t("settings.phi_horizon_hint", { days: info.phi_recent_default_days })}</p>
        {/if}
      {/if}

      <!-- Data ------------------------------------------------------------ -->
      {#if shown("data")}
        <h3 id={settingsAnchor("data")}>{t("settings.data")}</h3>
      {/if}

      <!-- These two render their own heading, because each carries an action
           in the band beside it and splitting the two apart would put the title
           on one row and its button on the next. They take the anchor id so the
           contents list can still reach them. -->
      {#if shown("data.profiles")}
        <SettingsProfiles bind:info anchorId={settingsAnchor("data.profiles")} />
      {/if}

      {#if shown("data.catalogues")}
        <SettingsCatalogues anchorId={settingsAnchor("data.catalogues")} />
      {/if}

      <!-- Advanced -------------------------------------------------------- -->
      {#if shown("advanced")}
        <h3 id={settingsAnchor("advanced")}>{t("settings.advanced")}</h3>
      {/if}

      {#if shown("advanced.backup")}
        <div class="view-head" id={settingsAnchor("advanced.backup")}>
          <h4>{t("backup.title")}</h4>
        </div>
        <div id="backup-actions" aria-label={t("backup.title")}>
          {#if shown("backup_export")}
            <button type="button" onclick={exportBackup}>{t("actions.export_backup")}</button>
          {/if}
          {#if shown("backup_import")}
            <button type="button" onclick={importBackup}>{t("actions.import_backup")}</button>
          {/if}
        </div>
      {/if}

      {#if shown("advanced.maintenance")}
        <div class="view-head" id={settingsAnchor("advanced.maintenance")}>
          <h4>{t("settings.maintenance")}</h4>
        </div>
        <p>{t("settings.maintenance_hint")}</p>
        <div id="maintenance-actions" aria-label={t("settings.maintenance")}>
          <button type="button" onclick={checkDatabase} disabled={checking}>
            {t("settings.check_db")}
          </button>
        </div>
      {/if}

      {#if shown("advanced.about")}
        <div class="view-head" id={settingsAnchor("advanced.about")}>
          <h4>{t("about.title")}</h4>
        </div>
        <div id="about-actions" aria-label={t("about.title")}>
          <button type="button" onclick={() => (aboutOpen = true)}>{t("about.title")}</button>
        </div>
      {/if}
    </div>
  </div>
</section>

<!-- `fill` because this one has tabs: the panel must not resize as the reader
     moves along the strip. -->
<TzDialog bind:open={aboutOpen} title={t("about.title")} fill>
  <AboutPanel />
</TzDialog>
