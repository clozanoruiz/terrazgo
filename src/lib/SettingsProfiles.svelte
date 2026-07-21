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
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";

  let { info = $bindable() } = $props();

  let profiles = $state([]);
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

  function submit(event) {
    event.preventDefault();
    const payload = {
      display_name: displayName.trim(),
      operator_id: operatorId || null,
    };
    run(async () => {
      if (editingId) {
        await invoke("update_user_profile", { profileId: editingId, update: payload });
      } else {
        await invoke("create_user_profile", { profile: payload });
      }
      notify(t("message.profile_saved", { name: payload.display_name }));
      hideForm();
      profiles = await invoke("list_user_profiles");
    });
  }

  function deleteProfile(profile) {
    run(async () => {
      if (!(await confirmDialog(t("profile.delete_confirm", { name: profile.display_name }))))
        return;
      await invoke("delete_user_profile", { profileId: profile.id });
      notify(t("message.profile_deleted"));
      profiles = await invoke("list_user_profiles");
      // The backend cleared the active-profile setting if it pointed at the
      // deleted row; re-read so the picker reflects it.
      info = await invoke("get_settings");
    });
  }

  function changeActive(event) {
    const value = event.target.value;
    run(async () => {
      const settings = { ...info.settings, active_user_id: value || null };
      info = await invoke("update_settings", { settings });
      notify(t("message.settings_saved"));
    });
  }

  function profileDetail(profile) {
    const operator = operators.find((candidate) => candidate.id === profile.operator_id);
    return operator ? t("profile.operator_detail", { name: operator.full_name }) : "";
  }
</script>

<div class="view-head">
  <h3>{t("settings.profiles")}</h3>
  <button type="button" onclick={() => showForm()}>{t("profiles.new")}</button>
</div>
<p>{t("profiles.hint")}</p>

{#if formOpen}
  <form onsubmit={submit}>
    <div class="form-grid">
      <label>
        <span>{t("profile.display_name")}</span>
        <input required bind:value={displayName} />
      </label>
      <label>
        <span>{t("profile.operator_link")}</span>
        <select bind:value={operatorId}>
          <option value="">—</option>
          {#each operators as operator (operator.id)}
            <option value={operator.id}>{operator.full_name}</option>
          {/each}
        </select>
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
  {#if info}
    <label>
      <span>{t("profiles.active_label")}</span>
      <select onchange={changeActive}>
        <option value="" selected={info.settings.active_user_id == null}>
          {t("profiles.active_none")}
        </option>
        {#each profiles as profile (profile.id)}
          <option value={profile.id} selected={info.settings.active_user_id === profile.id}>
            {profile.display_name}
          </option>
        {/each}
      </select>
    </label>
  {/if}

  <ul class="card-list">
    {#each profiles as profile (profile.id)}
      <li class="card">
        <strong>{profile.display_name}</strong>
        <span class="detail">{profileDetail(profile)}</span>
        <button type="button" onclick={() => showForm(profile)}>{t("form.edit")}</button>
        <button type="button" class="btn-danger" onclick={() => deleteProfile(profile)}>
          {t("form.delete")}
        </button>
      </li>
    {/each}
  </ul>
  {#if profiles.length === 0}
    <p>{t("profiles.empty")}</p>
  {/if}
{/if}
