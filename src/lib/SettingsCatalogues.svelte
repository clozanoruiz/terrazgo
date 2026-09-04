<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Reference-catalogue section of the Settings view: what the app currently
  // holds, and a button to ask the provider for a newer copy.
  //
  // Manual by design. The catalogues ship inside the app and import at first
  // run, so nothing here is needed for the app to work — this is for the user
  // whose provider published a code after the last release. A background
  // fetch rewriting the vocabulary a legal record resolves against, on a rural
  // connection, is the wrong default.
  //
  // Only the interesting lines are listed afterwards: a refresh where all 47
  // files are already current says so in one line, because 47 rows of "sin
  // cambios" is not a report, it is noise.
  import { formatDate, t } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import { invalidateLookups } from "./lookups.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  /// The DOM id the settings contents list scrolls to (settingsTree.js).
  let { anchorId = "" } = $props();

  let status = $state([]);
  let loading = $state(true);
  let busy = $state(false);
  // Per-file outcomes of the last refresh in this session; the unchanged ones
  // are counted, not listed.
  let notable = $state([]);
  let unchanged = $state(0);

  run(async () => {
    status = await invoke("catalogue_status");
  }).finally(() => (loading = false));

  const codes = $derived(status.reduce((total, row) => total + row.codes, 0));
  // The newest adoption across the files: on a fresh install they share the
  // startup import's timestamp, and after a partial refresh this is the last
  // time anything actually moved. ISO timestamps sort lexically.
  const lastImport = $derived(
    status
      .map((row) => row.imported_at)
      .filter(Boolean)
      .sort()
      .at(-1) ?? null,
  );

  function refresh() {
    busy = true;
    run(async () => {
      const reports = await invoke("refresh_catalogues");
      status = await invoke("catalogue_status");
      // The one thing in the app that can change the session-wide reference
      // lists, so it is the one place that invalidates them — a picker opened
      // after a refresh must offer what was just adopted, not the snapshot the
      // app started with.
      await invalidateLookups();
      notable = reports.filter((report) => report.outcome.kind !== "unchanged");
      unchanged = reports.length - notable.length;
      notify(
        t("message.catalogues_refreshed", {
          updated: notable.filter((r) => r.outcome.kind === "updated").length,
          refused: notable.filter((r) => r.outcome.kind === "refused").length,
          unchanged,
        }),
      );
    }).finally(() => (busy = false));
  }

  // A refusal is a localized sentence plus the provider's own specifics (a
  // column name, a row count) verbatim — the error.internal_intro pattern:
  // orientation the user can read, and the raw detail that makes it
  // reportable.
  function refusalText(outcome) {
    const reason = t(`catalogues.refused.${outcome.reason}`);
    return outcome.detail ? `${reason} (${outcome.detail})` : reason;
  }
</script>

<div class="view-head" id={anchorId}>
  <h4>{t("settings.catalogues")}</h4>
</div>
<p>{t("catalogues.hint")}</p>
{#if loading}
  <Skeleton />
{:else}
  <p id="catalogue-state">
    {t("catalogues.state", { count: status.length, codes })}
    {#if lastImport}
      · {t("catalogues.updated_at", { date: formatDate(lastImport.slice(0, 10)) })}
    {:else}
      · {t("catalogues.never")}
    {/if}
  </p>
  <div id="catalogue-actions" aria-label={t("settings.catalogues")}>
    <button type="button" onclick={refresh} disabled={busy}>
      {busy ? t("catalogues.refreshing") : t("catalogues.refresh")}
    </button>
  </div>
  {#if notable.length || unchanged}
    <ul id="catalogue-reports">
      {#each notable as report (report.id)}
        <li class:refused={report.outcome.kind === "refused"}>
          <strong>{report.id}</strong>
          {#if report.outcome.kind === "updated"}
            {t("catalogues.updated", {
              added: report.outcome.added,
              corrected: report.outcome.corrected,
            })}
            {#if report.outcome.withdrawn}
              · {t("catalogues.withdrawn", { count: report.outcome.withdrawn })}
            {/if}
            {#if report.outcome.extra_columns.length}
              · {t("catalogues.extra_columns", {
                columns: report.outcome.extra_columns.join(", "),
                count: report.outcome.extra_columns.length,
              })}
            {/if}
          {:else}
            {refusalText(report.outcome)}
          {/if}
        </li>
      {/each}
      {#if unchanged}
        <li>{t("catalogues.unchanged", { count: unchanged })}</li>
      {/if}
    </ul>
  {/if}
{/if}

<style>
  #catalogue-reports {
    margin: 0.5rem 0 0;
    padding-left: 1.2rem;
    font-size: 0.9rem;
  }
  #catalogue-reports li.refused {
    color: var(--danger);
  }
</style>
