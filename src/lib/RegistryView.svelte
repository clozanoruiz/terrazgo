<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The catalogue: entry UI for the reference data the record book points at —
  // products (with substances and per-country authorisations), operators,
  // machinery, advisors and fertiliser materials. Each section loads and
  // manages its own data.
  import { t } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import RegistryAdvisors from "./RegistryAdvisors.svelte";
  import RegistryMachinery from "./RegistryMachinery.svelte";
  import RegistryMaterials from "./RegistryMaterials.svelte";
  import RegistryOperators from "./RegistryOperators.svelte";
  import RegistryProducts from "./RegistryProducts.svelte";

  // Materials are not farm-scoped, but the coded lists Anexo III sección C
  // asks them to speak are national, so the section needs a country to resolve
  // against. It follows the holdings on file rather than being hardcoded; with
  // none yet, Spain — the only country whose record book this app implements.
  let countryCode = $state("es");

  run(async () => {
    const farms = await invoke("list_farms");
    if (farms.length > 0) countryCode = farms[0].country_code;
  });
</script>

<section class="view">
  <h2>{t("registry.title")}</h2>
  <RegistryProducts />
  <RegistryOperators />
  <RegistryMachinery />
  <RegistryAdvisors />
  <RegistryMaterials {countryCode} />
</section>
