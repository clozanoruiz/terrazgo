<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Machinery section of the catalogue. Machinery is farm-scoped, so the
  // section has its own farm selector; the Spanish registry numbers (ROMA for
  // mobile machinery, REGANIP for aircraft/fixed installations) only apply to
  // Spanish farms (extension row, like SIGPAC on plots).
  import { formatDate, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let farms = $state([]);
  let farmId = $state("");
  let machines = $state([]);
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let name = $state("");
  let kind = $state("");
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

  function submit(event) {
    event.preventDefault();
    const trimmed = name.trim();
    const payload = {
      name: trimmed,
      kind: kind.trim() || null,
      last_inspection_date: lastInspection || null,
      next_inspection_due_date: nextInspection || null,
      roma_number: farmCountry === "es" ? roma.trim() || null : null,
      reganip_number: farmCountry === "es" ? reganip.trim() || null : null,
    };
    run(async () => {
      if (editingId) {
        await invoke("update_machinery", { machineryId: editingId, update: payload });
      } else {
        await invoke("create_machinery", { machinery: { farm_id: farmId, ...payload } });
      }
      notify(t("message.machinery_saved", { name: trimmed }));
      hideForm();
      await reload();
    });
  }

  function deleteMachinery(machinery) {
    run(async () => {
      if (!(await confirmDialog(t("machinery.delete_confirm", { name: machinery.name })))) return;
      await invoke("delete_machinery", { machineryId: machinery.id });
      notify(t("message.machinery_deleted"));
      await reload();
    });
  }

  function machineryDetail(machinery, es) {
    return [
      machinery.type,
      es?.roma_number ? `ROMA ${es.roma_number}` : null,
      es?.reganip_number ? `REGANIP ${es.reganip_number}` : null,
      machinery.next_inspection_due_date
        ? t("machinery.itv_detail", { date: formatDate(machinery.next_inspection_due_date) })
        : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<div class="view-head">
  <h3>{t("machinery.title")}</h3>
  <button type="button" onclick={() => showForm()} disabled={!farmId}>
    {t("machinery.new")}
  </button>
</div>

{#if loading}
  <Skeleton />
{:else if farms.length === 0}
  <p>{t("machinery.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
{:else}
  <div class="form-grid">
    <label>
      <span>{t("machinery.farm")}</span>
      <select bind:value={farmId} onchange={selectFarm}>
        {#each farms as farm (farm.id)}
          <option value={farm.id}>{farm.name}</option>
        {/each}
      </select>
    </label>
  </div>

  {#if formOpen}
    <form onsubmit={submit}>
      <div class="form-grid">
        <label><span>{t("machinery.name")}</span><input required bind:value={name} /></label>
        <label><span>{t("machinery.kind")}</span><input bind:value={kind} /></label>
        <label>
          <span>{t("machinery.last_inspection")}</span>
          <input type="date" bind:value={lastInspection} />
        </label>
        <label>
          <span>{t("machinery.next_inspection")}</span>
          <input type="date" bind:value={nextInspection} />
        </label>
      </div>
      {#if farmCountry === "es"}
        <fieldset class="es-only">
          <legend>{t("machinery.es_section")}</legend>
          <div class="form-grid">
            <label><span>{t("machinery.roma")}</span><input bind:value={roma} /></label>
            <label><span>{t("machinery.reganip")}</span><input bind:value={reganip} /></label>
          </div>
        </fieldset>
      {/if}
      <div class="form-actions">
        <button type="submit">{t("form.save")}</button>
        <button type="button" class="btn-cancel" onclick={hideForm}>{t("form.cancel")}</button>
      </div>
    </form>
  {/if}

  <ul class="card-list">
    {#each machines as { machinery, es } (machinery.id)}
      <li class="card">
        <strong>{machinery.name}</strong>
        <span class="detail">{machineryDetail(machinery, es)}</span>
        <button type="button" onclick={() => showForm(machinery, es)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => deleteMachinery(machinery)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>
  {#if machines.length === 0}
    <p>{t("machinery.empty")}</p>
  {/if}
{/if}
