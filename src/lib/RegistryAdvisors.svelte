<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Advisors section of the catalogue: list + shared create/edit form.
  // Advisors are not farm-scoped (one advisory entity serves many holdings);
  // which farm is advised, and under which GIP framework, is set on the farm.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";

  // Neither operators nor advisors are farm-scoped, but ROPO is national, so
  // the hint needs a country to resolve against. RegistryView supplies the one
  // it already derives for the materials section — the holdings on file rather
  // than a hardcoded country.
  let { countryCode = null } = $props();

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

  async function submit() {
    const payload = {
      name: name.trim(),
      tax_id: taxId.trim() || null,
      registration_number: registrationNumber.trim() || null,
    };
    if (editingId) {
      await invoke("update_advisor", { advisorId: editingId, update: payload });
    } else {
      await invoke("create_advisor", { advisor: payload });
    }
    hideForm();
    advisors = await invoke("list_advisors");
  }

  function deleteAdvisor(advisor) {
    run(async () => {
      if (!(await confirmDialog(t("advisor.delete_confirm", { name: advisor.name })))) return;
      await invoke("delete_advisor", { advisorId: advisor.id });
      advisors = await invoke("list_advisors");
    });
  }

  // The "·"-joined detail string is gone: each value is its own column, which
  // is what lets a reader scan one down the list.

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(advisors.find((a) => a.id === editingId) ?? null);
</script>

<div class="view-head">
  <button type="button" onclick={() => showForm()}>{t("advisors.new")}</button>
</div>

<TzWorkspace
  open={formOpen}
  title={editingId ? name : t("advisors.new")}
  onclose={hideForm}
  ondelete={editingId ? () => deleteAdvisor(editing) : null}
>
  {#snippet list()}
    {#if loading}
      <Skeleton />
    {:else if advisors.length === 0}
      <p class="table-empty">{t("advisors.empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"advisors"}>
          <thead>
            <tr>
              <th>{t("column.name")}</th>
              <th>{t("column.tax_id")}</th>
              <th>{t("column.registration_number")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedAdvisors as advisor (advisor.id)}
              <tr
                class:selected={editingId === advisor.id}
                onclick={(e) => opensRow(e) && showForm(advisor)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showForm(advisor)}>
                    {advisor.name}
                  </button>
                </td>
                <td class="col-muted">{advisor.tax_id ?? ""}</td>
                <td class="col-muted">{advisor.registration_number ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/snippet}

  {#snippet inspector(formId)}
    <TzForm id={formId} onsubmit={submit}>
      <div class="form-grid">
        <TextInput label={t("advisor.name")} required bind:value={name} />
        <TextInput label={t("advisor.tax_id")} bind:value={taxId} />
        <TextInput label={t("advisor.registration_number")} bind:value={registrationNumber}>
          <RegistryHint country={countryCode} field="advisor.registration_number" />
        </TextInput>
      </div>
    </TzForm>
  {/snippet}

  {#snippet actions(formId)}
    <div class="form-actions">
      <button type="submit" form={formId}>{t("form.save")}</button>
      <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
    </div>
  {/snippet}
</TzWorkspace>
