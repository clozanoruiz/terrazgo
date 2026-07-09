<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Status view: app facts strip + active alerts wired to the CUE module.
  import { formatDate, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let status = $state(null);
  let alerts = $state([]);
  let loading = $state(true);

  async function reloadAlerts() {
    alerts = await invoke("list_alerts");
  }

  run(async () => {
    status = await invoke("get_status");
    await reloadAlerts();
  }).finally(() => (loading = false));

  function refresh() {
    run(async () => {
      alerts = await invoke("refresh_alerts");
      notify(t("message.refreshed"));
    });
  }

  function seed() {
    run(async () => {
      const summary = await invoke("seed_demo_data");
      notify(
        summary.seeded
          ? t("message.seeded", { season: summary.season_label, farm: summary.farm_name })
          : t("message.already_seeded"),
      );
      await reloadAlerts();
    });
  }

  // Tauri exposes snake_case Rust command arguments as camelCase in JS:
  // the Rust parameter `alert_id` is invoked as `alertId`.
  function acknowledge(alert) {
    run(async () => {
      await invoke("acknowledge_alert", { alertId: alert.id });
      await reloadAlerts();
    });
  }

  function dismiss(alert) {
    run(async () => {
      await invoke("dismiss_alert", { alertId: alert.id });
      await reloadAlerts();
    });
  }

  function formatSize(bytes) {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} kB`;
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
      status = await invoke("get_status");
      await reloadAlerts();
    });
  }

  function alertDetail(alert) {
    return [
      alert.due_date ? t("alerts.due", { date: formatDate(alert.due_date) }) : null,
      tCode("entity", alert.subject_table),
      tCode("alert.status", alert.status),
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<section class="view">
  <div id="status-strip" aria-label={t("status.aria")}>
    <dl>
      <div>
        <dt>{t("status.database")}</dt>
        <dd>{status?.db_path ?? "…"}</dd>
      </div>
      <div>
        <dt>{t("status.schema_version")}</dt>
        <dd>{status?.schema_version ?? "…"}</dd>
      </div>
      <div>
        <dt>{t("status.app_version")}</dt>
        <dd>{status?.app_version ?? "…"}</dd>
      </div>
    </dl>
  </div>

  <div id="actions" aria-label={t("actions.aria")}>
    <button type="button" onclick={refresh}>{t("actions.refresh")}</button>
    <button type="button" onclick={seed}>{t("actions.seed")}</button>
  </div>

  <h2>{t("alerts.title")}</h2>
  {#if loading}
    <Skeleton />
  {:else}
    <ul id="alerts">
      {#each alerts as alert (alert.id)}
        <li class="alert {alert.status}">
          <strong>{tCode("alert.type", alert.alert_type_code)}</strong>
          <span class="detail">{alertDetail(alert)}</span>
          <button
            type="button"
            disabled={alert.status === "acknowledged"}
            onclick={() => acknowledge(alert)}>{t("actions.ack")}</button
          >
          <button type="button" onclick={() => dismiss(alert)}>{t("actions.dismiss")}</button>
        </li>
      {/each}
    </ul>
    {#if alerts.length === 0}
      <p>{t("alerts.empty")}</p>
    {/if}
  {/if}

  <h2>{t("backup.title")}</h2>
  <div id="backup-actions" aria-label={t("backup.title")}>
    <button type="button" onclick={exportBackup}>{t("actions.export_backup")}</button>
    <button type="button" onclick={importBackup}>{t("actions.import_backup")}</button>
  </div>
</section>
