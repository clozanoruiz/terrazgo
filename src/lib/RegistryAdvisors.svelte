<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Advisors section of the catalogue: list + shared create/edit form.
  // Advisors are not farm-scoped (one advisory entity serves many holdings);
  // which farm is advised, and under which GIP framework, is set on the farm.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";

  let advisors = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedAdvisors = $derived(sortedBy(advisors, (a) => a.name));
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let name = $state("");
  let taxId = $state("");
  let registrationNumber = $state("");

  run(async () => {
    advisors = await invoke("list_advisors");
  }).finally(() => (loading = false));

  function showForm(advisor = null) {
    editingId = advisor?.id ?? null;
    name = advisor?.name ?? "";
    taxId = advisor?.tax_id ?? "";
    registrationNumber = advisor?.registration_number ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  function submit(event) {
    event.preventDefault();
    const payload = {
      name: name.trim(),
      tax_id: taxId.trim() || null,
      registration_number: registrationNumber.trim() || null,
    };
    run(async () => {
      if (editingId) {
        await invoke("update_advisor", { advisorId: editingId, update: payload });
      } else {
        await invoke("create_advisor", { advisor: payload });
      }
      notify(t("message.advisor_saved", { name: payload.name }));
      hideForm();
      advisors = await invoke("list_advisors");
    });
  }

  function deleteAdvisor(advisor) {
    run(async () => {
      if (!(await confirmDialog(t("advisor.delete_confirm", { name: advisor.name })))) return;
      await invoke("delete_advisor", { advisorId: advisor.id });
      notify(t("message.advisor_deleted"));
      advisors = await invoke("list_advisors");
    });
  }

  function advisorDetail(advisor) {
    return [advisor.tax_id, advisor.registration_number].filter(Boolean).join(" · ");
  }
</script>

<div class="view-head">
  <h3>{t("advisors.title")}</h3>
  <button type="button" onclick={() => showForm()}>{t("advisors.new")}</button>
</div>

{#if formOpen}
  <form onsubmit={submit}>
    <div class="form-grid">
      <label><span>{t("advisor.name")}</span><input required bind:value={name} /></label>
      <label><span>{t("advisor.tax_id")}</span><input bind:value={taxId} /></label>
      <label>
        <span>{t("advisor.registration_number")}</span>
        <input bind:value={registrationNumber} />
      </label>
    </div>
    <div class="form-actions">
      <button type="submit">{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
    </div>
  </form>
{/if}

{#if loading}
  <Skeleton />
{:else}
  <ul class="card-list">
    {#each sortedAdvisors as advisor (advisor.id)}
      <li class="card">
        <strong>{advisor.name}</strong>
        <span class="detail">{advisorDetail(advisor)}</span>
        <button type="button" onclick={() => showForm(advisor)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => deleteAdvisor(advisor)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>
  {#if advisors.length === 0}
    <p>{t("advisors.empty")}</p>
  {/if}
{/if}
