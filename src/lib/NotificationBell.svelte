<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The bell + badge + dropdown panel over notifications.svelte.js state.
  // App.svelte renders one instance per layout (desktop main head, mobile
  // topbar); they share the open state, so the guard below ignores document
  // clicks reaching the instance that is display:none'd by the media query.
  import { t } from "../i18n.js";
  import { clearAll, dismiss, notifications } from "./notifications.svelte.js";

  // Feather "bell".
  const BELL = "M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9 M13.73 21a2 2 0 0 1-3.46 0";

  const count = $derived(notifications.items.length);
  const hasError = $derived(notifications.items.some((n) => n.isError));

  let root;
  function onDocumentClick(event) {
    if (!notifications.open || !root) return;
    if (root.getClientRects().length === 0) return; // this instance is hidden
    // composedPath, not contains(): dismissing an item detaches the clicked ✕
    // before this bubble listener runs, but the dispatch-time path still
    // proves the click started inside the panel — it must stay open.
    if (!event.composedPath().includes(root)) notifications.open = false;
  }
  $effect(() => {
    document.addEventListener("click", onDocumentClick);
    return () => document.removeEventListener("click", onDocumentClick);
  });
</script>

<div class="bell-wrap" bind:this={root}>
  <button
    type="button"
    class="bell"
    aria-label={t("notif.aria")}
    title={t("notif.aria")}
    onclick={() => (notifications.open = !notifications.open)}
  >
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d={BELL} /></svg>
    {#if count > 0}
      <span class="bell-badge" class:error={hasError}>{count}</span>
    {/if}
  </button>

  {#if notifications.open}
    <div class="notif-panel" aria-live="polite">
      {#if count === 0}
        <p class="notif-empty">{t("notif.empty")}</p>
      {:else}
        <ul>
          {#each notifications.items as n (n.id)}
            <li class:error={n.isError}>
              <span>{n.text}</span>
              <button
                type="button"
                class="notif-dismiss"
                aria-label={t("actions.dismiss")}
                title={t("actions.dismiss")}
                onclick={() => dismiss(n.id)}>✕</button
              >
            </li>
          {/each}
        </ul>
        <button type="button" class="notif-clear" onclick={clearAll}>{t("notif.clear")}</button>
      {/if}
    </div>
  {/if}
</div>
