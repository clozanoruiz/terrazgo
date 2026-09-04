<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Status view: app facts strip + active alerts wired to the CUE module.
  // Backup export/import lives in the Settings view.
  import { formatDate, t, tCode } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import { resizableColumns } from "./columnResize.js";

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

  // The "·"-joined detail line is gone: the due date, the subject and the
  // state are three different questions, and a farmer scanning for what falls
  // due first was reading three sentences to find one date.
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

  <!--
    Only ever shown when the database is damaged. A healthy one says nothing:
    the check runs weekly in the background, and reporting "all fine" every
    time would train the farmer to ignore the one time it is not.
  -->
  {#if status?.integrity && !status.integrity.ok}
    <p class="integrity-warning" role="alert">
      <strong>{t("status.integrity.failed")}</strong>
      {t("status.integrity.restore", { date: formatDate(status.integrity.at) })}
    </p>
  {/if}

  <div id="actions" aria-label={t("actions.aria")}>
    <button type="button" onclick={refresh}>{t("actions.refresh")}</button>
    <button type="button" onclick={seed}>{t("actions.seed")}</button>
  </div>

  <div class="view-head">
    <h2>{t("alerts.title")}</h2>
  </div>
  {#if loading}
    <Skeleton />
  {:else if alerts.length === 0}
    <p class="table-empty">{t("alerts.empty")}</p>
  {:else}
    <!-- `rows-static`: an alert is not a record with a page of its own. What
         there is to do with one is on the row, because acknowledging and
         dismissing are the whole of it — there is nothing to open. The accent
         bar stays on the row: it is what makes the list read as alerts. -->
    <div class="table-wrap">
      <table class="data-table rows-static" id="alerts" use:resizableColumns={"alerts"}>
        <thead>
          <tr>
            <th>{t("column.finding")}</th>
            <th>{t("column.date")}</th>
            <th>{t("column.subject")}</th>
            <th>{t("column.status")}</th>
            <th class="col-actions"></th>
          </tr>
        </thead>
        <tbody>
          {#each alerts as alert (alert.id)}
            <tr class="alert {alert.status}">
              <td class="col-name">{tCode("alert.type", alert.alert_type_code)}</td>
              <td class="col-muted">
                {alert.due_date ? formatDate(alert.due_date) : ""}
              </td>
              <td class="col-muted">{tCode("entity", alert.subject_table)}</td>
              <td class="col-muted">{tCode("alert.status", alert.status)}</td>
              <td class="col-actions">
                <button
                  type="button"
                  disabled={alert.status === "acknowledged"}
                  onclick={() => acknowledge(alert)}>{t("actions.ack")}</button
                >
                <button type="button" onclick={() => dismiss(alert)}>{t("actions.dismiss")}</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  /* Shown only when the weekly corruption check found damage, so it is allowed
     to be loud — a farmer whose record book is failing needs to see it before
     the next backup overwrites a good one. Left border rather than a filled
     block: --danger stays legible on --surface in either theme, while text on a
     filled --danger would need its own light colour. */
  .integrity-warning {
    margin: var(--space-4) 0;
    padding: var(--space-3) var(--space-4);
    border-left: var(--space-1) solid var(--danger);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--danger);
  }

  .integrity-warning strong {
    display: block;
  }
</style>
