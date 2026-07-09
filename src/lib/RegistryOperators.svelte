<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Operators section of the catalogue: list + shared create/edit form.
  // Operators are not farm-scoped (the same applicator may work several farms).
  import { formatDate, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let operators = $state([]);
  let licenceLevels = $state([]);
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let fullName = $state("");
  let licenceNumber = $state("");
  let levelCode = $state("");
  let expiryDate = $state("");

  run(async () => {
    [operators, licenceLevels] = await Promise.all([
      invoke("list_operators"),
      invoke("list_licence_levels"),
    ]);
  }).finally(() => (loading = false));

  function showForm(operator = null) {
    editingId = operator?.id ?? null;
    fullName = operator?.full_name ?? "";
    licenceNumber = operator?.licence_number ?? "";
    levelCode = operator?.licence_level_code ?? "";
    expiryDate = operator?.licence_expiry_date ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  function submit(event) {
    event.preventDefault();
    const payload = {
      full_name: fullName.trim(),
      licence_number: licenceNumber.trim() || null,
      licence_level_code: levelCode || null,
      licence_expiry_date: expiryDate || null,
    };
    run(async () => {
      if (editingId) {
        await invoke("update_operator", { operatorId: editingId, update: payload });
      } else {
        await invoke("create_operator", { operator: payload });
      }
      notify(t("message.operator_saved", { name: payload.full_name }));
      hideForm();
      operators = await invoke("list_operators");
    });
  }

  function deleteOperator(operator) {
    run(async () => {
      if (!(await confirmDialog(t("operator.delete_confirm", { name: operator.full_name }))))
        return;
      await invoke("delete_operator", { operatorId: operator.id });
      notify(t("message.operator_deleted"));
      operators = await invoke("list_operators");
    });
  }

  function operatorDetail(operator) {
    return [
      operator.licence_number,
      operator.licence_level_code ? tCode("licence_level", operator.licence_level_code) : null,
      operator.licence_expiry_date
        ? t("operator.expiry_detail", { date: formatDate(operator.licence_expiry_date) })
        : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<div class="view-head">
  <h3>{t("operators.title")}</h3>
  <button type="button" onclick={() => showForm()}>{t("operators.new")}</button>
</div>

{#if formOpen}
  <form onsubmit={submit}>
    <div class="form-grid">
      <label><span>{t("operator.full_name")}</span><input required bind:value={fullName} /></label>
      <label>
        <span>{t("operator.licence_number")}</span>
        <input bind:value={licenceNumber} />
      </label>
      <label>
        <span>{t("operator.licence_level")}</span>
        <select bind:value={levelCode}>
          <option value="">—</option>
          {#each licenceLevels as level (level.code)}
            <option value={level.code}>{tCode("licence_level", level.code)}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>{t("operator.licence_expiry")}</span>
        <input type="date" bind:value={expiryDate} />
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
    {#each operators as operator (operator.id)}
      <li class="card">
        <strong>{operator.full_name}</strong>
        <span class="detail">{operatorDetail(operator)}</span>
        <button type="button" onclick={() => showForm(operator)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => deleteOperator(operator)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>
  {#if operators.length === 0}
    <p>{t("operators.empty")}</p>
  {/if}
{/if}
