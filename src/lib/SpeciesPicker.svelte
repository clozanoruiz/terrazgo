<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Crop species entry: the shared catalogue type-ahead over the FEGA PRODUCTOS
  // catalogue, plus the one thing only this field does — narrowing the list to
  // what a plot's verified SIGPAC land use plausibly grows. The backend decides
  // that and says so, and "show all" is always one click away: the filter is a
  // convenience, never a gate.
  import { t } from "../i18n.js";
  import { invoke } from "./backend.js";
  import CataloguePicker from "./CataloguePicker.svelte";

  let { name = $bindable(""), code = $bindable(null), plotId = null, required = false } = $props();

  let options = $state([]);
  let landUse = $state(null);
  let filterByPlot = $state(true);

  $effect(() => {
    const wanted = filterByPlot ? plotId : null;
    loadOptions(wanted);
  });

  async function loadOptions(wanted) {
    try {
      const catalogue = await invoke("list_crop_species", { plotId: wanted ?? null });
      options = catalogue.options;
      landUse = catalogue.land_use;
    } catch {
      // The catalogue is a typing aid, not a requirement: if it cannot be
      // read the field still works as a plain text input.
      options = [];
      landUse = null;
    }
  }

  function showAll() {
    filterByPlot = false;
  }
</script>

{#snippet filterChip()}
  {t("crop.species_filter_use", { use: landUse })}
  <button type="button" class="link-button" onclick={showAll}>
    {t("crop.species_show_all")}
  </button>
{/snippet}

<CataloguePicker
  bind:name
  bind:code
  {options}
  {required}
  placeholder={t("crop.species_search")}
  footer={landUse ? filterChip : null}
/>

<style>
  .link-button {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
</style>
