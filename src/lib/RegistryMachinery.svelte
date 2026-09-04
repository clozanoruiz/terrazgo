<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Machinery section of the catalogue. Machinery is farm-scoped, so the
  // section has its own farm selector; the Spanish registry numbers (ROMA for
  // mobile machinery, REGANIP for aircraft/fixed installations) only apply to
  // Spanish farms (extension row, like SIGPAC on plots).
  import { formatDate, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import DateInput from "./DateInput.svelte";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TzSelect from "./TzSelect.svelte";
  import { nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";

  let farms = $state([]);
  let farmId = $state("");
  let machines = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedMachines = $derived(sortedBy(machines, (m) => m.machinery.name));
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let name = $state("");
  let kind = $state("");
  let acquiredOn = $state("");
  let lastInspection = $state("");
  let nextInspection = $state("");
  let roma = $state("");
  let reganip = $state("");

  const farmCountry = $derived(farms.find((f) => f.id === farmId)?.country_code);

  run(async () => {
    farms = await invoke("list_farms");
    if (farms.length > 0) farmId = farms[0].id;
    await reload();
  }).finally(() => (loading = false));

  async function reload() {
    machines = farmId ? await invoke("list_machinery_details", { farmId }) : [];
  }

  function selectFarm() {
    formOpen = false;
    run(reload);
  }

  function showForm(machinery = null, es = null) {
    editingId = machinery?.id ?? null;
    name = machinery?.name ?? "";
    kind = machinery?.type ?? "";
    acquiredOn = machinery?.acquired_on ?? "";
    lastInspection = machinery?.last_inspection_date ?? "";
    nextInspection = machinery?.next_inspection_due_date ?? "";
    roma = es?.roma_number ?? "";
    reganip = es?.reganip_number ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  async function submit() {
    const trimmed = name.trim();
    const payload = {
      name: trimmed,
      kind: kind.trim() || null,
      acquired_on: acquiredOn || null,
      last_inspection_date: lastInspection || null,
      next_inspection_due_date: nextInspection || null,
      roma_number: farmCountry === "es" ? roma.trim() || null : null,
      reganip_number: farmCountry === "es" ? reganip.trim() || null : null,
    };
    if (editingId) {
      await invoke("update_machinery", { machineryId: editingId, update: payload });
    } else {
      await invoke("create_machinery", { machinery: { farm_id: farmId, ...payload } });
    }
    hideForm();
    await reload();
  }

  function deleteMachinery(machinery) {
    run(async () => {
      if (!(await confirmDialog(t("machinery.delete_confirm", { name: machinery.name })))) return;
      await invoke("delete_machinery", { machineryId: machinery.id });
      await reload();
    });
  }

  // The "·"-joined detail string is gone: each value is its own column. The two
  // registry numbers earn separate columns rather than a prefix inside a
  // sentence — ROMA and REGANIP cover different equipment and are normally
  // exclusive, so an empty cell says something a missing phrase did not.

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(machines.find((m) => m.machinery.id === editingId)?.machinery ?? null);
</script>

<div class="view-head">
  {#if farms.length > 0}
    <div class="form-grid">
      <TzSelect
        label={t("machinery.farm")}
        items={nameItems(farms)}
        bind:value={farmId}
        onchange={selectFarm}
      />
    </div>
  {/if}
  <button type="button" onclick={() => showForm()} disabled={!farmId}>
    {t("machinery.new")}
  </button>
</div>

{#if loading}
  <Skeleton />
{:else if farms.length === 0}
  <p>{t("machinery.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
{:else}
  <TzWorkspace
    open={formOpen}
    title={editingId ? name : t("machinery.new")}
    onclose={hideForm}
    ondelete={editingId ? () => deleteMachinery(editing) : null}
  >
    {#snippet list()}
      {#if machines.length === 0}
        <p class="table-empty">{t("machinery.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"machinery"}>
            <thead>
              <tr>
                <th>{t("column.name")}</th>
                <th>{t("column.kind")}</th>
                <th>{t("column.roma")}</th>
                <th>{t("column.reganip")}</th>
                <th>{t("column.next_inspection")}</th>
              </tr>
            </thead>
            <tbody>
              {#each sortedMachines as { machinery, es } (machinery.id)}
                <tr
                  class:selected={editingId === machinery.id}
                  onclick={(e) => opensRow(e) && showForm(machinery, es)}
                >
                  <td class="col-name">
                    <button type="button" class="row-open" onclick={() => showForm(machinery, es)}>
                      {machinery.name}
                    </button>
                  </td>
                  <td class="col-muted">{machinery.type ?? ""}</td>
                  <td class="col-muted">{es?.roma_number ?? ""}</td>
                  <td class="col-muted">{es?.reganip_number ?? ""}</td>
                  <td class="col-muted">
                    {machinery.next_inspection_due_date
                      ? formatDate(machinery.next_inspection_due_date)
                      : ""}
                  </td>
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
          <TextInput label={t("machinery.name")} required bind:value={name} />
          <TextInput label={t("machinery.kind")} bind:value={kind} />
          <DateInput label={t("machinery.acquired_on")} bind:value={acquiredOn} />
          <DateInput label={t("machinery.last_inspection")} bind:value={lastInspection} />
          <DateInput label={t("machinery.next_inspection")} bind:value={nextInspection} />
        </div>
        <!-- Section-level rather than under a field: these are DateInputs, whose
             `hint` prop takes a plain string and cannot carry a link. It also
             annotates no identifier — it says where the inspection is done. -->
        <RegistryHint country={farmCountry} field="machinery.inspection" block />
        {#if farmCountry === "es"}
          <fieldset class="es-only">
            <legend>{t("machinery.es_section")}</legend>
            <div class="form-grid">
              <TextInput label={t("machinery.roma")} bind:value={roma}>
                <RegistryHint country={farmCountry} field="machinery.roma" />
              </TextInput>
              <TextInput label={t("machinery.reganip")} bind:value={reganip}>
                <RegistryHint country={farmCountry} field="machinery.reganip" />
              </TextInput>
            </div>
          </fieldset>
        {/if}
      </TzForm>
    {/snippet}

    {#snippet actions(formId)}
      <div class="form-actions">
        <button type="submit" form={formId}>{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
      </div>
    {/snippet}
  </TzWorkspace>
{/if}
