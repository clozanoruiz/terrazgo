<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // App shell: responsive navigation (sidebar on wide screens, bottom tab bar
  // on narrow ones — both rendered from lib/nav.js), hash router, notification
  // bell. The language selector lives in the Settings view.
  import TzTooltip from "./lib/TzTooltip.svelte";
  import { BitsConfig } from "bits-ui";
  import { formatTag, onLocaleChange, t } from "./i18n.js";
  import { ChevronsLeft, ChevronsRight } from "@lucide/svelte";

  import { NAV_ICONS } from "./lib/icons.js";
  import { NAV_ITEMS, activeRoute } from "./lib/nav.js";
  import NotificationBell from "./lib/NotificationBell.svelte";
  import { clearAll } from "./lib/notifications.svelte.js";
  import { resolveRoute } from "./lib/routes.js";

  let hash = $state(location.hash || "#/status");
  window.addEventListener("hashchange", () => {
    hash = location.hash;
  });

  const active = $derived(activeRoute(hash));

  // Which view answers this hash, and with what props — the table is data in
  // lib/routes.js, so a new module screen adds an entry there instead of
  // another branch here.
  const route = $derived(resolveRoute(hash));

  // Collapsed sidebar is a per-device display preference, like the locale.
  let collapsed = $state(localStorage.getItem("terrazgo.sidebar") === "collapsed");
  function toggleSidebar() {
    collapsed = !collapsed;
    localStorage.setItem("terrazgo.sidebar", collapsed ? "collapsed" : "expanded");
  }

  // A language switch remounts the whole shell via {#key}, so every t()
  // call re-evaluates. Notifications are cleared rather than re-translated —
  // they may hold interpolated stale data.
  let localeVersion = $state(0);
  onLocaleChange(() => {
    clearAll();
    localeVersion += 1;
  });
</script>

<!-- Inside the {#key}, so a language switch rebuilds every owned control with
     the new locale — segment ORDER changes, not just the labels. Each wrapper
     also passes locale itself, so it stays correct when a harness mounts a view
     outside this shell. -->
{#key localeVersion}
  <BitsConfig defaultLocale={formatTag()} defaultPortalTo="body">
    <div class="app-shell">
      <!-- Narrow screens only (CSS): the screen's name on top, tabs at the
           bottom. It says where you ARE rather than what the app is called —
           the tab bar below already carries the app's identity, and on a phone
           the band is the only place a title can live at all (.main-head is
           hidden at this width). -->
      <header class="topbar">
        <h1>{t(route.titleKey)}</h1>
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
            {@const Icon = NAV_ICONS[item.icon]}
            <!-- A tip only while the rail is collapsed: expanded, the label is
                 already beside the icon, and a tooltip repeating it is noise. -->
            <TzTooltip label={collapsed ? t(item.labelKey) : ""} side="right">
              {#snippet trigger(props)}
                <a
                  {...props}
                  href={item.route}
                  class:active={active === item.route}
                  class:nav-foot={item.foot}
                >
                  <Icon />
                  <span class="nav-label">{t(item.labelKey)}</span>
                </a>
              {/snippet}
            </TzTooltip>
          {/each}
        </nav>
        <div class="sidebar-foot">
          <TzTooltip label={collapsed ? t("nav.expand") : t("nav.collapse")} side="right">
            {#snippet trigger(props)}
              <button
                {...props}
                type="button"
                class="sidebar-toggle"
                onclick={(event) => {
                  props.onclick?.(event);
                  toggleSidebar();
                }}
                aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
              >
                {#if collapsed}<ChevronsRight />{:else}<ChevronsLeft />{/if}
              </button>
            {/snippet}
          </TzTooltip>
        </div>
      </aside>

      <main>
        <!-- Wide screens only (CSS): the frame's top band. It names the screen
           and holds what governs the whole window; the narrow-screen equivalent
           is the topbar above. The title comes from the route table rather than
           from the view, so a view never states its own name twice.

           Controls that govern one screen (the farm and campaign pickers, say)
           deliberately do NOT come here: this band does not exist below 700px,
           and a control that vanishes on a phone is not a control. They live in
           the view's own first toolbar band, which exists at both widths. -->
        <div class="main-head">
          <h2 class="view-title">{t(route.titleKey)}</h2>
          <NotificationBell />
        </div>

        <route.component {...route.props} />
      </main>

      <!-- Narrow screens only (CSS). -->
      <nav class="tabbar" aria-label={t("nav.aria")}>
        {#each NAV_ITEMS as item (item.route)}
          {@const Icon = NAV_ICONS[item.icon]}
          <a href={item.route} class:active={active === item.route}>
            <Icon />
            <span>{t(item.labelKey)}</span>
          </a>
        {/each}
      </nav>
    </div>
  </BitsConfig>
{/key}
