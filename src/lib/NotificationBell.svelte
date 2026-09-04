<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The bell + badge + dropdown panel over notifications.svelte.js state.
  // App.svelte renders one instance per layout (desktop main head, mobile
  // topbar); they share the open state, so the guard below ignores document
  // clicks reaching the instance that is display:none'd by the media query.
  import TzTooltip from "./TzTooltip.svelte";
  import { Bell, X } from "@lucide/svelte";
  import { t } from "../i18n.js";
  import { clearAll, dismiss, notifications } from "./notifications.svelte.js";

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
  <TzTooltip label={t("notif.aria")} side="bottom">
    {#snippet trigger(props)}
      <button
        {...props}
        type="button"
        class="bell"
        aria-label={t("notif.aria")}
        onclick={(event) => {
          props.onclick?.(event);
          notifications.open = !notifications.open;
        }}
      >
        <Bell />
        {#if count > 0}
          <span class="bell-badge" class:error={hasError}>{count}</span>
        {/if}
      </button>
    {/snippet}
  </TzTooltip>

  {#if notifications.open}
    <div class="notif-panel" aria-live="polite">
      {#if count === 0}
        <p class="notif-empty">{t("notif.empty")}</p>
      {:else}
        <ul>
          {#each notifications.items as n (n.id)}
            <li class:error={n.isError}>
              <span>{n.text}</span>
              <TzTooltip label={t("actions.dismiss")}>
                {#snippet trigger(props)}
                  <button
                    {...props}
                    type="button"
                    class="notif-dismiss"
                    aria-label={t("actions.dismiss")}
                    onclick={(event) => {
                      props.onclick?.(event);
                      dismiss(n.id);
                    }}
                  >
                    <X />
                  </button>
                {/snippet}
              </TzTooltip>
            </li>
          {/each}
        </ul>
        <button type="button" class="notif-clear" onclick={clearAll}>{t("notif.clear")}</button>
      {/if}
    </div>
  {/if}
</div>
