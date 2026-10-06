<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The devices that have written in this record book, and what they are called
  // (docs/sync.md → Device identity).
  //
  // A `sync_peer` row is a register like any other: the name given to a phone
  // here reaches every other device at the next sync, which is the point — a
  // conflict that says "Móvil de María" is a question a farmer can answer, and
  // one that says a UUID is not.
  //
  // In Settings and not on the Status view, unlike the conflict queue beside
  // it: naming a device is housekeeping nobody is waiting on.
  import { formatDate, t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let {
    /// The DOM id the settings contents list scrolls to, on the band rather
    /// than on a heading of the parent's (settingsTree.js).
    anchorId = "",
  } = $props();

  let peers = $state([]);
  let thisDevice = $state("");
  let loading = $state(true);

  // The form names one device; there is nothing to create here, because a row
  // appears when a device writes.
  let formOpen = $state(false);
  let editingId = $state(null);
  let label = $state("");

  async function reload() {
    ({ peers, this_device: thisDevice } = await invoke("list_sync_peers"));
  }

  run(reload).finally(() => (loading = false));

  function showForm(peer) {
    editingId = peer.id;
    label = peer.label ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  async function submit() {
    await invoke("rename_sync_peer", {
      deviceId: editingId,
      label: label.trim() || null,
    });
    hideForm();
    await reload();
  }

  function retire(peer) {
    run(async () => {
      const retiring = !peer.deleted_at;
      if (retiring && !(await confirmDialog(t("sync.peer_retire_confirm", { name: name(peer) })))) {
        return;
      }
      await invoke("retire_sync_peer", { deviceId: peer.id, retired: retiring });
      hideForm();
      await reload();
    });
  }

  /// What to call a device in the table: its name, or a plain word. Never the
  /// id — it is in the table's own column for anybody who needs it.
  function name(peer) {
    return peer.label || t("sync.peer_unnamed");
  }

  const editing = $derived(peers.find((peer) => peer.id === editingId) ?? null);
</script>

<div class="view-head" id={anchorId}>
  <h4>{t("sync.peers_title")}</h4>
</div>
<p>{t("sync.peers_hint")}</p>

<TzWorkspace
  open={formOpen}
  title={editing ? name(editing) : t("sync.peers_title")}
  onclose={hideForm}
>
  {#snippet list()}
    {#if loading}
      <Skeleton />
    {:else if peers.length === 0}
      <p class="table-empty">{t("sync.peers_empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"sync-peers"}>
          <thead>
            <tr>
              <th>{t("column.name")}</th>
              <th>{t("sync.peer_known_since")}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each peers as peer (peer.id)}
              <tr
                class:selected={editingId === peer.id}
                onclick={(e) => opensRow(e) && showForm(peer)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showForm(peer)}>
                    {name(peer)}
                  </button>
                  {#if peer.id === thisDevice}
                    <span class="peer-tag">{t("sync.peer_this_device")}</span>
                  {/if}
                  {#if peer.deleted_at}
                    <span class="peer-tag">{t("sync.peer_retired")}</span>
                  {/if}
                </td>
                <td class="col-muted">{formatDate(peer.created_at.slice(0, 10))}</td>
                <td class="col-muted">
                  <!-- Not offered for this device: retiring it would say the
                       book should stop expecting changes from the device
                       making them. The backend refuses it too. -->
                  {#if peer.id !== thisDevice}
                    <button type="button" class="btn-cancel" onclick={() => retire(peer)}>
                      {peer.deleted_at ? t("sync.peer_restore") : t("sync.peer_retire")}
                    </button>
                  {/if}
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
        <TextInput label={t("sync.peer_label")} bind:value={label} />
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

<style>
  /* A quiet marker beside the name, not a column of its own: two of the three
     states are absent most of the time, and a column holding blanks is what the
     alert cards were moved away from. */
  .peer-tag {
    margin-left: var(--space-2);
    color: var(--muted);
    font-size: 0.8125rem;
  }
</style>
