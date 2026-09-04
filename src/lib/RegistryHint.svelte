<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // A note under an identifier field, saying which official registry holds
  // that number and offering to open it.
  //
  // The call site names the FIELD, not the registry: this resolves the id
  // through lib/registryHints.js and renders nothing when the country has no
  // entry. That is what keeps "add a country" a data-file edit — no call site
  // grows a second branch, and no Spanish registry is hardcoded into a shared
  // form component (the *_es_extension discipline, applied to the UI).
  //
  // The button is a <button>, never an <a href>. An external href would
  // navigate the app window away from itself (target="_blank" does nothing in
  // a Tauri webview), and the production CSP is `default-src 'self'` anyway.
  // Rust holds the URL — this only ever names a registry id.
  //
  // Renders as the same <small> under a label that the hand-written field
  // hints already use, so it drops in beside a native <input> with no layout
  // work. A whole fieldset (the SIGPAC block) passes `block` instead, which
  // makes it a paragraph above the fields it covers.
  //
  // It carries its own class rather than the `detail` the other section hints
  // use: `detail` is now stated globally (muted, 13px), and this needs its own
  // treatment for the link it holds.
  import { ExternalLink } from "@lucide/svelte";
  import { invoke } from "./backend.js";
  import { registryHint } from "./registryHints.js";
  import { run } from "./notifications.svelte.js";
  import { t } from "../i18n.js";

  let {
    /// The farm's country_code. Null/undefined while a farm is still loading,
    /// which simply renders nothing.
    country = null,
    /// `<entity>.<field>`, the same slug the field's i18n label uses.
    field,
    /// Render as a paragraph above a fieldset rather than a field-level <small>.
    block = false,
  } = $props();

  const id = $derived(registryHint(country, field));

  function open() {
    run(() => invoke("open_external_link", { target: id }));
  }
</script>

{#if id}
  <svelte:element this={block ? "p" : "small"} class="registry-hint" class:block>
    {t(`registry.${id}_hint`)}
    <button type="button" class="link-button" onclick={open}>
      {t(`registry.${id}_open`)}
      <ExternalLink />
    </button>
  </svelte:element>
{/if}
