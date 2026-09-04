<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The settings screen's table of contents: every section and the groups
  // inside it, as a tree beside the screen it lists.
  //
  // Navigational only. Clicking a node scrolls the screen to that heading and
  // changes nothing else — the settings themselves are one document that
  // scrolls as a whole, so collapsing a section here hides its entry in this
  // list and never the setting itself.
  //
  // Sections open by default, and a SEARCH opens all of them regardless: a hit
  // counted beside a heading the reader cannot see is a count that points at
  // nothing. The collapsed state is left untouched while filtering, so clearing
  // the field puts the tree back the way it was.
  //
  // Buttons rather than links, deliberately: an <a href="#..."> would be a
  // route change to this app's hash router, which navigates away from the very
  // screen the node points into. The twisty is its own button beside the label
  // rather than around it — a button inside a button is not markup.
  //
  // Wide screens only — it is hidden under 700px by the layout, where the
  // screen is a single column and a second pane has nowhere to go.
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { t } from "../i18n.js";
  import { SETTINGS_TREE } from "./settingsTree.js";

  let {
    /// Node id → hit count, from searchSettings.
    hits = {},
    /// Whether a query is narrowing anything; counts only mean something then.
    filtering = false,
    /// The anchor the reader is looking at, from the screen's scroll spy.
    current = "",
    onnavigate = null,
  } = $props();

  /// Sections the reader has collapsed. Absent means open, so the default needs
  /// no seeding and a new section in settingsTree.js arrives expanded.
  let collapsed = $state({});

  const isOpen = (section) => filtering || !collapsed[section.id];

  function toggle(section) {
    collapsed[section.id] = !collapsed[section.id];
  }

  /// A section stays lit while any of its own groups is the current anchor, so
  /// the reader can see which branch they are in and not only which leaf.
  const inSection = (section) => current === section.id || current.startsWith(`${section.id}.`);
</script>

<nav class="settings-toc" aria-label={t("settings.toc")}>
  <ul>
    {#each SETTINGS_TREE as section (section.id)}
      {#if hits[section.id] > 0}
        {@const open = isOpen(section)}
        <li>
          <div class="toc-row">
            <button
              type="button"
              class="toc-twisty"
              aria-expanded={open}
              aria-controls="toc-{section.id}"
              aria-label={t(open ? "settings.collapse_section" : "settings.expand_section", {
                section: t(section.labelKey),
              })}
              onclick={() => toggle(section)}
            >
              {#if open}<ChevronDown />{:else}<ChevronRight />{/if}
            </button>
            <button
              type="button"
              class="toc-node toc-section"
              class:current={inSection(section)}
              onclick={() => onnavigate?.(section.id)}
            >
              <span class="toc-label">{t(section.labelKey)}</span>
              {#if filtering}<span class="toc-count">{hits[section.id]}</span>{/if}
            </button>
          </div>

          <ul id="toc-{section.id}" hidden={!open}>
            {#each section.groups as group (group.id)}
              {#if hits[group.id] > 0}
                <li>
                  <button
                    type="button"
                    class="toc-node toc-group"
                    class:current={current === group.id}
                    onclick={() => onnavigate?.(group.id)}
                  >
                    <span class="toc-label">{t(group.labelKey)}</span>
                    {#if filtering}<span class="toc-count">{hits[group.id]}</span>{/if}
                  </button>
                </li>
              {/if}
            {/each}
          </ul>
        </li>
      {/if}
    {/each}
  </ul>
</nav>

<style>
  .settings-toc ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* The second level is indented by the rail rather than by a bullet: a tree
     drawn with a hairline reads as structure, where a bullet reads as a list of
     unrelated things. The inset lines it up under its section's label, past the
     twisty. */
  .settings-toc ul ul {
    margin: 0 0 var(--space-2) var(--space-3);
    border-left: 1px solid var(--border);
  }

  .toc-row {
    display: flex;
    align-items: center;
  }

  /* The app's button skin has to be undone on both of these: they are places to
     go and a disclosure, not actions. Without it they inherit a green border
     and fill solid green on hover — the `.tz-field-trigger` problem. */
  .toc-node,
  .toc-twisty {
    padding: 0.3rem var(--space-2);
    border: 0;
    border-radius: var(--radius);
    background: none;
    font: inherit;
    color: var(--muted);
    cursor: pointer;
  }

  .toc-node {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    width: 100%;
    text-align: left;
  }

  .toc-twisty {
    display: flex;
    align-items: center;
    flex: none;
    padding-inline: 0;
  }

  .toc-twisty :global(svg) {
    width: 1rem;
    height: 1rem;
  }

  .toc-section {
    font-weight: 600;
    color: var(--text);
  }

  /* `color` is declared here and not left to the base rule: `button:hover:enabled`
     is 0,2,1 and sets `color: var(--on-primary)`, which beat the 0,2,0 base rule
     and turned an unselected node white on hover (measured 2026-09-04). Stating
     the colour in a rule of the same shape is what settles it. */
  .toc-node:hover:enabled,
  .toc-twisty:hover:enabled {
    background: var(--surface-hover);
    color: var(--text);
  }

  .toc-node:active:enabled,
  .toc-twisty:active:enabled {
    background: var(--surface-active);
  }

  .toc-node.current,
  .toc-node.current:hover:enabled {
    color: var(--primary);
    font-weight: 600;
  }

  /* Long section names wrap rather than being cut: the pane is narrow and a
     truncated heading is a heading the reader cannot match to the screen. */
  .toc-label {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  /* The count is a figure, not a badge — a filled pill here would compete with
     the current-node highlight, which is the one thing this pane must say
     loudly. */
  .toc-count {
    flex: none;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
</style>
