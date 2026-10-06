<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // A record book's campaign, asked the same way wherever one is written: the
  // record book list creates a book, and the book's own page corrects its
  // campaign. The FarmForm arrangement, and it binds the caller's draft
  // (lib/seasonDraft.js) rather than holding a copy, for FarmForm's reason.
  //
  // The campaign is its two dates, and they name the book ("2025/2026",
  // "2026") unless the farmer gives it a name of their own — which a holding
  // keeping two campaigns in one year needs, since a farm cannot have two books
  // of the same name. No campaign year is asked: the dates already say it.
  //
  // The farm is asked only when creating. A season is one holding's campaign,
  // and moving it would carry its whole book to another holding, so the book's
  // own page names its farm in the title and offers no way to change it.
  import DateInput from "./DateInput.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { nameItems } from "./selectItems.js";
  import { t } from "../i18n.js";

  let {
    /// The object from seasonDraft.js. Mutated in place through `bind:`.
    draft = $bindable(),
    /// The farms a new book may be created for.
    farms = [],
    /// Whether this is a new book — decides whether the farm is asked.
    creating = false,
    /// async () => void, run on submit. Throws to report a refusal, as TzForm
    /// expects.
    onsubmit,
    /// Minted by the panel, so its pinned Save can claim this form.
    formId = "",
  } = $props();
</script>

<!-- The backend's refusals name the field to change: a name another book of
     this farm already has, dates another book of this farm already covers, or
     an end before the start. -->
<TzForm
  id={formId}
  {onsubmit}
  anchors={{
    "invalid.season_name_taken": "custom_label",
    "invalid.season_dates_taken": "starts_on",
    "invalid.invalid_date_interval": "ends_on",
  }}
>
  <div class="form-grid">
    {#if creating}
      <TzSelect
        label={t("season.farm")}
        items={nameItems(farms)}
        required
        bind:value={draft.farmId}
      />
    {/if}
    <DateInput label={t("season.starts")} name="starts_on" required bind:value={draft.startsOn} />
    <DateInput
      label={t("season.ends")}
      name="ends_on"
      required
      min={draft.startsOn}
      bind:value={draft.endsOn}
    />
    <TextInput
      label={t("season.custom_label")}
      hint={t("season.custom_label_hint")}
      name="custom_label"
      bind:value={draft.customLabel}
    />
  </div>
</TzForm>
