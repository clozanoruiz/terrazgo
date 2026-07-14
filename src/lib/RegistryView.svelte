<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The catalogue: entry UI for the reference data treatments point at —
  // products (with substances and per-country authorisations), operators and
  // machinery. Each section loads and manages its own data.
  import { t } from "../i18n.js";
  import RegistryMachinery from "./RegistryMachinery.svelte";
  import RegistryOperators from "./RegistryOperators.svelte";
  import RegistryProducts from "./RegistryProducts.svelte";
</script>

<section class="view">
  <h2>{t("registry.title")}</h2>
  <RegistryProducts />
  <RegistryOperators />
  <RegistryMachinery />
</section>
