<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // App shell: responsive navigation (sidebar on wide screens, bottom tab bar
  // on narrow ones — both rendered from lib/nav.js), hash router, notification
  // bell. The language selector lives in the Settings view.
  import { onLocaleChange, t } from "./i18n.js";
  import FarmsView from "./lib/FarmsView.svelte";
  import FarmView from "./lib/FarmView.svelte";
  import MapView from "./lib/MapView.svelte";
  import { NAV_ITEMS, activeRoute } from "./lib/nav.js";
  import NotificationBell from "./lib/NotificationBell.svelte";
  import { clearAll } from "./lib/notifications.svelte.js";
  import RegistryView from "./lib/RegistryView.svelte";
  import SettingsView from "./lib/SettingsView.svelte";
  import StatusView from "./lib/StatusView.svelte";
  import TreatmentsView from "./lib/TreatmentsView.svelte";

  let hash = $state(location.hash || "#/status");
  window.addEventListener("hashchange", () => {
    hash = location.hash;
  });

  const active = $derived(activeRoute(hash));

  // The farm detail route belongs to the farms nav entry (prefix match).
  const farmRoute = $derived(hash.match(/^#\/farms\/(.+)$/));

  // Collapsed sidebar is a per-device display preference, like the locale.
  let collapsed = $state(localStorage.getItem("terrazgo.sidebar") === "collapsed");
  function toggleSidebar() {
    collapsed = !collapsed;
    localStorage.setItem("terrazgo.sidebar", collapsed ? "collapsed" : "expanded");
  }

  // Feather chevrons for the collapse toggle.
  const CHEVRONS_LEFT = "M11 17l-5-5 5-5 M18 17l-5-5 5-5";
  const CHEVRONS_RIGHT = "M13 17l5-5-5-5 M6 17l5-5-5-5";

  // A language switch remounts the whole shell via {#key}, so every t()
  // call re-evaluates. Notifications are cleared rather than re-translated —
  // they may hold interpolated stale data.
  let localeVersion = $state(0);
  onLocaleChange(() => {
    clearAll();
    localeVersion += 1;
  });
</script>

{#key localeVersion}
  <div class="app-shell">
    <!-- Narrow screens only (CSS): brand + language on top, tabs at the bottom. -->
    <header class="topbar">
      <h1>Terrazgo</h1>
      <div class="topbar-tools">
        <NotificationBell />
      </div>
    </header>

    <!-- Wide screens only (CSS). -->
    <aside class="sidebar" class:collapsed>
      <div class="brand">
        <h1>Terrazgo</h1>
        <p class="subtitle">{t("app.subtitle")}</p>
      </div>
      <nav aria-label={t("nav.aria")}>
        {#each NAV_ITEMS as item (item.route)}
          <a
            href={item.route}
            class:active={active === item.route}
            class:nav-foot={item.foot}
            title={t(item.labelKey)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.icon} /></svg>
            <span class="nav-label">{t(item.labelKey)}</span>
          </a>
        {/each}
      </nav>
      <div class="sidebar-foot">
        <button
          type="button"
          class="sidebar-toggle"
          onclick={toggleSidebar}
          aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
          title={collapsed ? t("nav.expand") : t("nav.collapse")}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d={collapsed ? CHEVRONS_RIGHT : CHEVRONS_LEFT} />
          </svg>
        </button>
      </div>
    </aside>

    <main>
      <!-- Wide screens only (CSS): sticky utility strip — the bell stays in
           sight while scrolling; future always-visible items go before it.
           The narrow-screen bell lives in the (also sticky) topbar. -->
      <div class="main-head">
        <NotificationBell />
      </div>

      {#if farmRoute}
        <FarmView farmId={farmRoute[1]} />
      {:else if hash === "#/farms"}
        <FarmsView />
      {:else if hash.startsWith("#/map")}
        <!-- Prefix match: #/map?farm=…&plot=… deep links (query parsed inside). -->
        <MapView />
      {:else if hash === "#/treatments"}
        <TreatmentsView />
      {:else if hash === "#/registry"}
        <RegistryView />
      {:else if hash === "#/settings"}
        <SettingsView />
      {:else}
        <StatusView />
      {/if}
    </main>

    <!-- Narrow screens only (CSS). -->
    <nav class="tabbar" aria-label={t("nav.aria")}>
      {#each NAV_ITEMS as item (item.route)}
        <a href={item.route} class:active={active === item.route}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.icon} /></svg>
          <span>{t(item.labelKey)}</span>
        </a>
      {/each}
    </nav>
  </div>
{/key}
