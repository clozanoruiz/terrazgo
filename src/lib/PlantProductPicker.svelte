<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Harvested-produce entry: the shared catalogue type-ahead over FEGA's
  // PROD_VEGETAL, which is NOT the crop catalogue the species picker offers.
  // The two answer different questions about the same plant — "Granos de trigo"
  // is what leaves the holding, "TRIGO BLANDO" is what grew on it — and the
  // register fields that ask for produce (a sale, a postharvest treatment)
  // validate against this list.
  import { t } from "../i18n.js";
  import { invoke } from "./backend.js";
  import CataloguePicker from "./CataloguePicker.svelte";

  let {
    name = $bindable(""),
    code = $bindable(null),
    countryCode = "es",
    required = false,
  } = $props();

  let options = $state([]);

  $effect(() => {
    loadOptions(countryCode);
  });

  async function loadOptions(country) {
    try {
      options = await invoke("list_plant_products", { countryCode: country });
    } catch {
      // A typing aid, not a requirement: the field still takes free text.
      options = [];
    }
  }
</script>

<CataloguePicker
  bind:name
  bind:code
  {options}
  {required}
  placeholder={t("harvest.product_search")}
/>
