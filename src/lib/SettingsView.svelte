<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Settings view: device-local preferences. Three storage tiers meet here —
  // the display language (localStorage: per-device, read synchronously at
  // startup by i18n.js), the settings file (settings.json via the backend),
  // and the backup actions (moved from the Status view: maintenance chores
  // belong with settings, alerts stay with status).
  import { locale, locales, nativeName, setLocale, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import SettingsProfiles from "./SettingsProfiles.svelte";
  import Skeleton from "./Skeleton.svelte";

  // { settings, tile_cache_default_bytes } — the default rides along so an
  // unset cap can display its effective value without the frontend hardcoding
  // a copy of the Rust constant.
  let info = $state(null);
  let loading = $state(true);

  run(async () => {
    info = await invoke("get_settings");
  }).finally(() => (loading = false));

  // setLocale is async (it may lazy-load a dictionary); a rejection means a
  // locale file is missing from the bundle — log it, keep the previous language.
  function switchLocale(event) {
    setLocale(event.target.value).catch((err) => {
      console.error(err);
      event.target.value = locale();
    });
  }

  const MIB = 1024 * 1024;
  // Preset cache sizes; the empty select value means "follow the default".
  const CACHE_PRESETS = [256 * MIB, 512 * MIB, 1024 * MIB, 2048 * MIB];

  function formatSize(bytes) {
    if (bytes >= 1024 * MIB) {
      const gib = bytes / (1024 * MIB);
      return `${Number.isInteger(gib) ? gib : gib.toFixed(1)} GB`;
    }
    if (bytes >= MIB) return `${Math.round(bytes / MIB)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} kB`;
  }

  function changeCacheSize(event) {
    const value = event.target.value;
    run(async () => {
      const settings = {
        ...info.settings,
        tile_cache_max_bytes: value === "" ? null : Number(value),
      };
      info = await invoke("update_settings", { settings });
      notify(t("message.settings_saved"));
    });
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

<section class="view">
  <h2>{t("nav.settings")}</h2>

  <h3>{t("settings.general")}</h3>
  <label
    ><span>{t("lang.label")}</span>
    <select aria-label={t("lang.label")} onchange={switchLocale}>
      {#each locales() as code (code)}
        <option value={code} selected={code === locale()}>{nativeName(code)}</option>
      {/each}
    </select>
  </label>

  <h3>{t("settings.map")}</h3>
  {#if loading}
    <Skeleton />
  {:else if info}
    <label
      ><span>{t("settings.cache_size")}</span>
      <select onchange={changeCacheSize}>
        <option value="" selected={info.settings.tile_cache_max_bytes == null}>
          {t("settings.cache_default", { size: formatSize(info.tile_cache_default_bytes) })}
        </option>
        {#each CACHE_PRESETS as bytes (bytes)}
          <option value={bytes} selected={info.settings.tile_cache_max_bytes === bytes}>
            {formatSize(bytes)}
          </option>
        {/each}
      </select>
    </label>
    <p>{t("settings.cache_hint")}</p>
    <div id="cache-actions" aria-label={t("settings.map")}>
      <button type="button" onclick={clearCache}>{t("settings.clear_cache")}</button>
    </div>
  {/if}

  <SettingsProfiles bind:info />

  <h3>{t("backup.title")}</h3>
  <div id="backup-actions" aria-label={t("backup.title")}>
    <button type="button" onclick={exportBackup}>{t("actions.export_backup")}</button>
    <button type="button" onclick={importBackup}>{t("actions.import_backup")}</button>
  </div>
</section>
