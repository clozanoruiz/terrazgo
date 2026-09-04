<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Loading placeholder: shimmer rows shown while a view's first fetch is in
  // flight, so lists read as "coming" instead of blank — and the "no X yet"
  // empty message can't flash before the data lands.
  //
  // Shaped like the tables it stands in for rather than like the cards it used
  // to: what a reader is waiting for is rows of a table, and a placeholder that
  // resolves into a different shape is a flash of layout rather than a
  // promise of one.
  let { rows = 2 } = $props();
</script>

<ul class="skeleton-rows" aria-hidden="true">
  {#each { length: rows }, i (i)}
    <li>
      <!-- Widths come from classes, never a style attribute: the production CSP
           blocks style attributes outright (see .skel-short below). -->
      <span class="skel-bar skel-short"></span>
      <span class="skel-bar skel-long"></span>
    </li>
  {/each}
</ul>

<style>
  .skeleton-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    pointer-events: none;
  }

  /* One data-table row: the same hairline, the same one-line height and the
     same outer cells with no inset. */
  .skeleton-rows li {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) 0;
    border-top: 1px solid var(--border);
  }

  .skel-bar {
    height: 0.9rem;
    border-radius: var(--radius-sm);
    background: linear-gradient(90deg, var(--border) 30%, var(--panel) 50%, var(--border) 70%);
    background-size: 300% 100%;
    animation: skel-shimmer 1.1s linear infinite;
  }

  /* Widths are classes rather than style="width: …" attributes: the production
     CSP is `default-src 'self'` with no `style-src`, so a style attribute is
     blocked (`style-src-attr`) and the bar would silently render full width.
     Found on Android, where the block is reported to the console. */
  .skel-bar.skel-short {
    width: 22%;
  }

  .skel-bar.skel-long {
    width: 46%;
  }

  @keyframes skel-shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0% 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .skel-bar {
      animation: none;
    }
  }
</style>
