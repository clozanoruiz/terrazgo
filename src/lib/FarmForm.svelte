<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The holding's fields — model 1.1 — asked the same way wherever a holding is
  // being written. The farms list creates one and the farm's own page corrects
  // it, and until 2026-09-15 those were two different forms: four questions
  // against twenty-six, because `insert_farm` wrote four columns whatever the
  // payload carried. The backend now writes all of them, so there is one form.
  //
  // The only difference left is the country, and it is a real one: it may be
  // stated at creation and never again, because changing it would re-home the
  // meaning of every country-scoped code already on the holding's records.
  //
  // It binds to the caller's draft and holds no copy — the TreatmentForm
  // arrangement, and for its reason: a form that copies a record has two
  // answers to the same question the moment anything else changes one.
  import DateInput from "./DateInput.svelte";
  import NumberInput from "./NumberInput.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems } from "./selectItems.js";
  import { t } from "../i18n.js";

  let {
    /// The object from farmDraft.js. Mutated in place through `bind:`.
    draft = $bindable(),
    /// The country list, for the select this form owns only while creating.
    countries = [],
    /// Whether this is a new holding. Decides one thing: whether the country can
    /// be chosen or is stated as fixed.
    creating = false,
    /// async () => void, run on submit. Throws to report a refusal, as TzForm
    /// expects.
    onsubmit,
    /// Minted by the panel, so its pinned Save can claim this form.
    formId = "",
  } = $props();
</script>

<TzForm id={formId} {onsubmit}>
  <div class="form-grid">
    <TextInput label={t("farm.name")} required bind:value={draft.name} />
    <TextInput label={t("farm.owner")} bind:value={draft.ownerName} />
    <TextInput label={t("farm.owner_tax_id")} bind:value={draft.ownerTaxId} />
    <!-- Stated once, when the farm is created, and fixed from then on: the
         country decides which coded vocabularies this holding's records speak,
         so changing it would re-home the meaning of everything already
         recorded. A holding that genuinely moved is a new farm. -->
    <TzSelect
      label={t("farm.country")}
      hint={creating ? undefined : t("farm.country_fixed_hint")}
      items={codeItems(countries, "country")}
      value={draft.countryCode}
      disabled={!creating}
      onchange={(code) => (draft.countryCode = code)}
    />
    <TextInput label={t("farm.address")} bind:value={draft.address} />
    <TextInput label={t("farm.location")} bind:value={draft.locationText} />
    <TextInput label={t("farm.postal_code")} bind:value={draft.postalCode} />
    <TextInput label={t("farm.phone_fixed")} bind:value={draft.phoneFixed} />
    <TextInput label={t("farm.phone_mobile")} bind:value={draft.phoneMobile} />
    <TextInput label={t("farm.email")} type="email" bind:value={draft.email} />
    <DateInput
      label={t("farm.opened_on")}
      hint={t("farm.opened_on_hint")}
      bind:value={draft.openedOn}
    />
    <NumberInput label={t("farm.latitude")} min={-90} max={90} bind:value={draft.latitude} />
    <NumberInput label={t("farm.longitude")} min={-180} max={180} bind:value={draft.longitude} />
  </div>

  {#if draft.countryCode === "es"}
    <fieldset class="es-only">
      <legend>{t("farm.es_section")}</legend>
      <div class="form-grid">
        <TextInput label={t("farm.siex")} bind:value={draft.siexCode} />
        <TextInput label={t("farm.rea")} bind:value={draft.reaCode} />
        <TextInput label={t("farm.rega")} bind:value={draft.regaCode} />
        <TextInput label={t("farm.province")} bind:value={draft.provinceCode} />
      </div>
    </fieldset>
  {/if}

  <fieldset>
    <legend>{t("farm.representative_section")}</legend>
    <p class="detail">{t("farm.representative_hint")}</p>
    <div class="form-grid">
      <TextInput label={t("farm.rep_name")} bind:value={draft.repName} />
      <TextInput label={t("farm.rep_tax_id")} bind:value={draft.repTaxId} />
      <TextInput label={t("farm.rep_kind")} bind:value={draft.repKind} />
      <TextInput label={t("farm.rep_address")} bind:value={draft.repAddress} />
      <TextInput label={t("farm.rep_locality")} bind:value={draft.repLocality} />
      <TextInput label={t("farm.rep_province")} bind:value={draft.repProvince} />
      <TextInput label={t("farm.rep_postal_code")} bind:value={draft.repPostalCode} />
      <TextInput label={t("farm.rep_phone")} bind:value={draft.repPhone} />
      <TextInput label={t("farm.rep_email")} type="email" bind:value={draft.repEmail} />
    </div>
  </fieldset>
</TzForm>
