<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The catalogue: entry UI for the reference data the record book points at —
  // products (with substances and per-country authorisations), operators,
  // machinery, premises, advisors and fertiliser materials. Each section loads
  // and manages its own data.
  //
  // ONE SECTION AT A TIME, picked from a tab strip. Stacked, the six read as a
  // single undifferentiated list of things: six tables of unrelated shapes
  // scrolling past each other, with nothing to say where one ended and the next
  // began. Showing one is also what lets the view be `framed` — a screen with a
  // single subject has a workspace that can fill the frame, so the table and
  // its inspector each get a height to scroll inside.
  //
  // The strip is the same `.tabstrip` the record book uses for its registers,
  // rather than a new control: reuse the vocabulary
  // (docs/frontend-conventions.md → Styling).
  import { t } from "../i18n.js";
  import TzTabs from "./TzTabs.svelte";
  import { invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import RegistryAdvisors from "./RegistryAdvisors.svelte";
  import RegistryMachinery from "./RegistryMachinery.svelte";
  import RegistryMaterials from "./RegistryMaterials.svelte";
  import RegistryOperators from "./RegistryOperators.svelte";
  import RegistryPremises from "./RegistryPremises.svelte";
  import RegistryProducts from "./RegistryProducts.svelte";

  // Materials are not farm-scoped, but the coded lists Anexo III sección C
  // asks them to speak are national, so the section needs a country to resolve
  // against. It follows the holdings on file rather than being hardcoded; with
  // none yet, Spain — the only country whose record book this app implements.
  // Operators and advisors take it for the same reason: they are not
  // farm-scoped either, and ROPO — which holds both their numbers — is
  // national, so their registry hints resolve against this too.
  let countryCode = $state("es");

  // The sections as data, the nav.js philosophy: adding one is an entry here.
  // `component` is what the chosen tab renders; `needsCountry` says whether it
  // takes the resolved country, so the render below stays one branch.
  const SECTIONS = [
    { id: "products", labelKey: "tab.products", component: RegistryProducts },
    { id: "operators", labelKey: "tab.operators", component: RegistryOperators, country: true },
    { id: "machinery", labelKey: "tab.machinery", component: RegistryMachinery },
    { id: "premises", labelKey: "tab.premises", component: RegistryPremises },
    { id: "advisors", labelKey: "tab.advisors", component: RegistryAdvisors, country: true },
    { id: "materials", labelKey: "tab.materials", component: RegistryMaterials, country: true },
  ];

  let sectionId = $state(SECTIONS[0].id);
  const tabItems = $derived(SECTIONS.map((s) => ({ value: s.id, label: t(s.labelKey) })));

  run(async () => {
    const farms = await invoke("list_farms");
    if (farms.length > 0) countryCode = farms[0].country_code;
  });
</script>

<section class="view framed">
  <TzTabs items={tabItems} bind:value={sectionId} framed>
    {#snippet panel(item)}
      {@const entry = SECTIONS.find((s) => s.id === item.value)}
      {#if entry.country}
        <entry.component {countryCode} />
      {:else}
        <entry.component />
      {/if}
    {/snippet}
  </TzTabs>
</section>
