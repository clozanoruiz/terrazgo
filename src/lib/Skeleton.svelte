<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Loading placeholder: card-shaped shimmer rows shown while a view's first
  // fetch is in flight, so lists read as "coming" instead of blank — and the
  // "no X yet" empty message can't flash before the data lands.
  let { rows = 2 } = $props();
</script>

<ul class="card-list" aria-hidden="true">
  {#each { length: rows }, i (i)}
    <li class="card skeleton">
      <!-- Widths come from classes, never a style attribute: the production CSP
           blocks style attributes outright (see .skel-short in styles.css). -->
      <span class="skel-bar skel-short"></span>
      <span class="skel-bar skel-long"></span>
    </li>
  {/each}
</ul>
