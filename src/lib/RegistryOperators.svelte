<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Operators section of the catalogue: list + shared create/edit form.
  // Operators are not farm-scoped (the same applicator may work several farms).
  import { formatDate, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { lookups, loadLookups } from "./lookups.svelte.js";
  import { run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import TzSelect from "./TzSelect.svelte";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import { codeItems } from "./selectItems.js";

  // Neither operators nor advisors are farm-scoped, but ROPO is national, so
  // the hint needs a country to resolve against. RegistryView supplies the one
  // it already derives for the materials section — the holdings on file rather
  // than a hardcoded country.
  let { countryCode = null } = $props();

  let operators = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedOperators = $derived(sortedBy(operators, (o) => o.full_name));
  // Session-wide reference data (lib/lookups.svelte.js).
  const licenceLevels = $derived(lookups.licenceLevels);
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let fullName = $state("");
  let taxId = $state("");
  let licenceNumber = $state("");
  let levelCode = $state("");
  let expiryDate = $state("");

  run(async () => {
    [operators] = await Promise.all([invoke("list_operators"), loadLookups()]);
  }).finally(() => (loading = false));

  function showForm(operator = null) {
    editingId = operator?.id ?? null;
    fullName = operator?.full_name ?? "";
    taxId = operator?.tax_id ?? "";
    licenceNumber = operator?.licence_number ?? "";
    levelCode = operator?.licence_level_code ?? "";
    expiryDate = operator?.licence_expiry_date ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  // Plain async: TzForm gates it on the form's own validity and catches a
  // refusal into the summary, so neither preventDefault nor run() belongs here.
  async function submit() {
    const payload = {
      full_name: fullName.trim(),
      tax_id: taxId.trim() || null,
      licence_number: licenceNumber.trim() || null,
      licence_level_code: levelCode || null,
      licence_expiry_date: expiryDate || null,
    };
    if (editingId) {
      await invoke("update_operator", { operatorId: editingId, update: payload });
    } else {
      await invoke("create_operator", { operator: payload });
    }
    hideForm();
    operators = await invoke("list_operators");
  }

  function deleteOperator(operator) {
    run(async () => {
      if (!(await confirmDialog(t("operator.delete_confirm", { name: operator.full_name }))))
        return;
      await invoke("delete_operator", { operatorId: operator.id });
      operators = await invoke("list_operators");
    });
  }

  // The "·"-joined detail string is gone: each value is its own column, which
  // is what lets a reader scan the licence expiry dates down the list instead
  // of hunting for them inside four different sentences.

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(operators.find((o) => o.id === editingId) ?? null);
</script>

<div class="view-head">
  <button type="button" onclick={() => showForm()}>{t("operators.new")}</button>
</div>

<TzWorkspace
  open={formOpen}
  title={editingId ? fullName : t("operators.new")}
  onclose={hideForm}
  ondelete={editingId ? () => deleteOperator(editing) : null}
>
  {#snippet list()}
    {#if loading}
      <Skeleton />
    {:else if operators.length === 0}
      <p class="table-empty">{t("operators.empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"operators"}>
          <thead>
            <tr>
              <th>{t("column.name")}</th>
              <th>{t("column.tax_id")}</th>
              <th>{t("column.licence_number")}</th>
              <th>{t("column.licence_level")}</th>
              <th>{t("column.licence_expiry")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedOperators as operator (operator.id)}
              <tr
                class:selected={editingId === operator.id}
                onclick={(e) => opensRow(e) && showForm(operator)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showForm(operator)}>
                    {operator.full_name}
                  </button>
                </td>
                <td class="col-muted">{operator.tax_id ?? ""}</td>
                <td class="col-muted">{operator.licence_number ?? ""}</td>
                <td class="col-muted">
                  {operator.licence_level_code
                    ? tCode("licence_level", operator.licence_level_code)
                    : ""}
                </td>
                <td class="col-muted">
                  {operator.licence_expiry_date ? formatDate(operator.licence_expiry_date) : ""}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/snippet}

  {#snippet inspector(formId)}
    <TzForm id={formId} onsubmit={submit} anchors={{ "invalid.empty_name": "full_name" }}>
      <div class="form-grid">
        <TextInput
          label={t("operator.full_name")}
          name="full_name"
          required
          bind:value={fullName}
        />
        <TextInput label={t("operator.tax_id")} bind:value={taxId} />
        <TextInput label={t("operator.licence_number")} bind:value={licenceNumber}>
          <RegistryHint country={countryCode} field="operator.licence_number" />
        </TextInput>
        <TzSelect
          label={t("operator.licence_level")}
          items={codeItems(licenceLevels, "licence_level")}
          nullable
          bind:value={levelCode}
        />
        <DateInput label={t("operator.licence_expiry")} bind:value={expiryDate} />
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
