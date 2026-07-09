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
      <span class="skel-bar" style="width: 22%"></span>
      <span class="skel-bar" style="width: 46%"></span>
    </li>
  {/each}
</ul>
