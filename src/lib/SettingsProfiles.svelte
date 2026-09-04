<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // User-profiles section of the Settings view: profile list + create/edit
  // form, and the active-profile picker. Profiles are farm data (synced,
  // audit-logged); the ACTIVE profile is a device preference stored in
  // settings.json — hence the bindable settings `info` shared with the
  // parent view, so both sections update the same object and a save from
  // one can never clobber the other's change.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";

  let {
    info = $bindable(),
    /// The DOM id the settings contents list scrolls to. It goes on the band
    /// rather than on a heading of the parent's, because the band holds this
    /// section's own action too and splitting the two would put the title on
    /// one row and its button on the next (settingsTree.js).
    anchorId = "",
  } = $props();

  let profiles = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation,
  // which puts "Ángel" after "Zubiri".
  const sortedProfiles = $derived(sortedBy(profiles, (p) => p.display_name));
  let operators = $state([]);
  let loading = $state(true);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let displayName = $state("");
  let operatorId = $state("");

  run(async () => {
    [profiles, operators] = await Promise.all([
      invoke("list_user_profiles"),
      invoke("list_operators"),
    ]);
  }).finally(() => (loading = false));

  function showForm(profile = null) {
    editingId = profile?.id ?? null;
    displayName = profile?.display_name ?? "";
    operatorId = profile?.operator_id ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  async function submit() {
    const payload = {
      display_name: displayName.trim(),
      operator_id: operatorId || null,
    };
    if (editingId) {
      await invoke("update_user_profile", { profileId: editingId, update: payload });
    } else {
      await invoke("create_user_profile", { profile: payload });
    }
    hideForm();
    profiles = await invoke("list_user_profiles");
  }

  function deleteProfile(profile) {
    run(async () => {
      if (!(await confirmDialog(t("profile.delete_confirm", { name: profile.display_name }))))
        return;
      await invoke("delete_user_profile", { profileId: profile.id });
      hideForm();
      profiles = await invoke("list_user_profiles");
      // The backend cleared the active-profile setting if it pointed at the
      // deleted row; re-read so the picker reflects it.
      info = await invoke("get_settings");
    });
  }

  function changeActive(value) {
    run(async () => {
      const settings = { ...info.settings, active_user_id: value || null };
      info = await invoke("update_settings", { settings });
    });
  }

  /// The operator a profile is linked to, as the table's own cell. The
  /// "vinculado a …" sentence the card carried is gone: the column is headed
  /// with what it holds.
  function operatorName(profile) {
    return operators.find((candidate) => candidate.id === profile.operator_id)?.full_name ?? "";
  }

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which profile it is about. Null while creating.
  const editing = $derived(profiles.find((p) => p.id === editingId) ?? null);
</script>

<div class="view-head" id={anchorId}>
  <h4>{t("settings.profiles")}</h4>
  <button type="button" onclick={() => showForm()}>{t("profiles.new")}</button>
</div>
<p>{t("profiles.hint")}</p>

{#if info}
  <TzSelect
    label={t("profiles.active_label")}
    items={nameItems(profiles, (p) => p.display_name)}
    nullable
    nullLabel={t("profiles.active_none")}
    value={info.settings.active_user_id ?? ""}
    onchange={changeActive}
  />
{/if}

<TzWorkspace
  open={formOpen}
  title={editingId ? displayName : t("profiles.new")}
  onclose={hideForm}
  ondelete={editing ? () => deleteProfile(editing) : null}
>
  {#snippet list()}
    {#if loading}
      <Skeleton />
    {:else if profiles.length === 0}
      <p class="table-empty">{t("profiles.empty")}</p>
    {:else}
      <div class="table-wrap">
        <table class="data-table" use:resizableColumns={"profiles"}>
          <thead>
            <tr>
              <th>{t("column.name")}</th>
              <th>{t("column.operator")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedProfiles as profile (profile.id)}
              <tr
                class:selected={editingId === profile.id}
                onclick={(e) => opensRow(e) && showForm(profile)}
              >
                <td class="col-name">
                  <button type="button" class="row-open" onclick={() => showForm(profile)}>
                    {profile.display_name}
                  </button>
                </td>
                <td class="col-muted">{operatorName(profile)}</td>
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
        <TextInput label={t("profile.display_name")} required bind:value={displayName} />
        <TzSelect
          label={t("profile.operator_link")}
          items={nameItems(operators, (o) => o.full_name)}
          nullable
          bind:value={operatorId}
        />
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
