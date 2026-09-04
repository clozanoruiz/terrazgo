<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Premises section of the catalogue: the stores and vehicles models 3.4 and
  // 3.5 treat.
  //
  // It exists because RD 1311/2012 Anexo III Parte I B.b asks for the "local o
  // medio de transporte tratado" to be IDENTIFIED, and a description retyped on
  // every record identifies nothing — two treatments of one warehouse can spell
  // it differently and nothing ties them together. Registering the place once
  // and naming it from the record is what an identification means.
  //
  // Premises are farm-scoped, so the section has its own farm selector, like
  // machinery. The two kinds carry different fields, because their pages ask
  // different questions: 3.4 wants "tipo y dirección", 3.5 "tipo, modelo y
  // matrícula", and the FEGA class and the Spanish registry identifiers only
  // exist for real estate. Those last two sit in their own fieldset for the
  // same reason machinery's ROMA/REGANIP do: they are what the SPANISH
  // registries say, and they live in an extension row.
  import { formatNumber, t, tCode } from "../i18n.js";
  import { lookups } from "./lookups.svelte.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { run } from "./notifications.svelte.js";
  import NumberInput from "./NumberInput.svelte";
  import RegistryHint from "./RegistryHint.svelte";
  import Skeleton from "./Skeleton.svelte";
  import { sortedBy } from "./collate.js";
  import { resizableColumns } from "./columnResize.js";
  import { opensRow } from "./tableRow.js";
  import TzCombobox from "./TzCombobox.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { codeItems, nameItems } from "./selectItems.js";
  import TextInput from "./TextInput.svelte";
  import TzForm from "./TzForm.svelte";
  import TzWorkspace from "./TzWorkspace.svelte";

  const kinds = $derived(lookups.premisesKinds);

  let farms = $state([]);
  let farmId = $state("");
  let premises = $state([]);
  // Display order is the client's business: SQL orders by BINARY collation.
  const sorted = $derived(sortedBy(premises, (row) => row.premises.name));
  let loading = $state(true);

  // FEGA's EDIFICACIONES_INSTALACIONES, per country — 109 rows, so a combobox
  // rather than a listbox. Not session-wide reference data (it takes a
  // country), so it is fetched here, like the other coded catalogues.
  let classes = $state([]);
  const farmCountry = $derived(farms.find((f) => f.id === farmId)?.country_code);

  // Form; null editingId = the form creates, an id = it edits.
  let formOpen = $state(false);
  let editingId = $state(null);
  let kindCode = $state("building");
  let name = $state("");
  let address = $state("");
  let vehicleModel = $state("");
  let plate = $state("");
  let classCode = $state("");
  let cadastralReference = $state("");
  let reaInstallationCode = $state("");
  let volume = $state("");
  let notes = $state("");

  const isVehicle = $derived(kindCode === "vehicle");
  const farmCountryIsSpain = $derived(farmCountry === "es");

  run(async () => {
    farms = await invoke("list_farms");
    if (farms.length > 0) farmId = farms[0].id;
    await reload();
  }).finally(() => (loading = false));

  async function reload() {
    premises = farmId ? await invoke("list_premises_details", { farmId }) : [];
    classes = farmCountry
      ? await invoke("list_premises_classes", { countryCode: farmCountry })
      : [];
  }

  function selectFarm() {
    formOpen = false;
    run(reload);
  }

  function showForm(detail = null) {
    const row = detail?.premises ?? null;
    editingId = row?.id ?? null;
    kindCode = row?.kind_code ?? "building";
    name = row?.name ?? "";
    address = row?.address ?? "";
    vehicleModel = row?.vehicle_model ?? "";
    plate = row?.plate ?? "";
    classCode = row?.class_code ?? "";
    cadastralReference = detail?.es?.cadastral_reference ?? "";
    reaInstallationCode = detail?.es?.rea_installation_code ?? "";
    volume = row?.volume_m3 ?? "";
    notes = row?.notes ?? "";
    formOpen = true;
  }

  function hideForm() {
    formOpen = false;
    editingId = null;
  }

  /// Switching kind clears the fields the other kind carries, rather than
  /// storing values nothing will ever print or export.
  function onKindChosen() {
    if (isVehicle) {
      address = "";
      classCode = "";
      cadastralReference = "";
      reaInstallationCode = "";
    } else {
      vehicleModel = "";
      plate = "";
    }
  }

  async function submit() {
    const trimmed = name.trim();
    const payload = {
      kind_code: kindCode,
      name: trimmed,
      address: isVehicle ? null : address.trim() || null,
      vehicle_model: isVehicle ? vehicleModel.trim() || null : null,
      plate: isVehicle ? plate.trim() || null : null,
      class_code: isVehicle ? null : classCode || null,
      volume_m3: volume === "" ? null : Number(volume),
      notes: notes.trim() || null,
      // Spanish registry fields; they land in premises_es_extension, which the
      // backend reconciles from exactly what is submitted here.
      cadastral_reference:
        isVehicle || !farmCountryIsSpain ? null : cadastralReference.trim() || null,
      rea_installation_code:
        isVehicle || !farmCountryIsSpain ? null : reaInstallationCode.trim() || null,
    };
    if (editingId) {
      await invoke("update_premises", { premisesId: editingId, update: payload });
    } else {
      await invoke("create_premises", { premises: { farm_id: farmId, ...payload } });
    }
    hideForm();
    await reload();
  }

  function deletePremises(row) {
    run(async () => {
      if (!(await confirmDialog(t("premises.delete_confirm", { name: row.name })))) return;
      await invoke("delete_premises", { premisesId: row.id });
      await reload();
    });
  }

  /// A stored class code resolved through the catalogue — never snapshotted
  /// onto the row, so this is the only place its label is read.
  function className(code) {
    return classes.find((option) => option.code === code)?.name ?? code;
  }

  // Seven values used to be joined into one "·" sentence per row. They are
  // columns now — which is what shows, at a glance, that a vehicle has a plate
  // and a building has an address, instead of leaving the reader to work out
  // which half of the sentence they are looking at.

  /// The row the inspector is editing, so the delete button beside the form
  /// knows which record it is about. Null while creating.
  const editing = $derived(premises.find((d) => d.premises.id === editingId)?.premises ?? null);
</script>

<div class="view-head">
  {#if farms.length > 0}
    <div class="form-grid">
      <TzSelect
        label={t("premises.farm")}
        items={nameItems(farms)}
        bind:value={farmId}
        onchange={selectFarm}
      />
    </div>
  {/if}
  <button type="button" onclick={() => showForm()} disabled={!farmId}>
    {t("premises.new")}
  </button>
</div>

{#if loading}
  <Skeleton />
{:else if farms.length === 0}
  <p>{t("premises.no_farms")} <a href="#/farms">{t("nav.farms")}</a></p>
{:else}
  <TzWorkspace
    open={formOpen}
    title={editingId ? name : t("premises.new")}
    onclose={hideForm}
    ondelete={editingId ? () => deletePremises(editing) : null}
  >
    {#snippet list()}
      {#if premises.length === 0}
        <p class="table-empty">{t("premises.empty")}</p>
      {:else}
        <div class="table-wrap">
          <table class="data-table" use:resizableColumns={"premises"}>
            <thead>
              <tr>
                <th>{t("column.name")}</th>
                <th>{t("column.kind")}</th>
                <th>{t("column.address")}</th>
                <th>{t("column.model")}</th>
                <th>{t("column.plate")}</th>
                <th>{t("column.class")}</th>
                <th>{t("column.cadastral")}</th>
                <th>{t("column.rea")}</th>
                <th class="col-num">{t("column.volume")}</th>
              </tr>
            </thead>
            <tbody>
              {#each sorted as { premises: row, es } (row.id)}
                <tr
                  class:selected={editingId === row.id}
                  onclick={(e) => opensRow(e) && showForm({ premises: row, es })}
                >
                  <td class="col-name">
                    <button
                      type="button"
                      class="row-open"
                      onclick={() => showForm({ premises: row, es })}
                    >
                      {row.name}
                    </button>
                  </td>
                  <td class="col-muted">{tCode("premises_kind", row.kind_code)}</td>
                  <td class="col-muted">{row.address ?? ""}</td>
                  <td class="col-muted">{row.vehicle_model ?? ""}</td>
                  <td class="col-muted">{row.plate ?? ""}</td>
                  <td class="col-muted">{row.class_code ? className(row.class_code) : ""}</td>
                  <td class="col-muted">{es?.cadastral_reference ?? ""}</td>
                  <td class="col-muted">{es?.rea_installation_code ?? ""}</td>
                  <td class="col-muted col-num">
                    <!-- `== null`, not truthiness: a stored 0 is a number the
                         farmer entered, and `0 ? …` would print it as blank —
                         the same "a real value renders empty" failure the
                         composition column had. -->
                    {row.volume_m3 == null ? "" : formatNumber(row.volume_m3)}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/snippet}

    {#snippet inspector(formId)}
      {#if !editingId}
        <p class="detail">{t("premises.intro")}</p>
      {/if}
      <TzForm id={formId} onsubmit={submit}>
        <div class="form-grid">
          <TzSelect
            label={t("premises.kind")}
            items={codeItems(kinds, "premises_kind")}
            required
            bind:value={kindCode}
            onchange={onKindChosen}
          />
          <TextInput label={t("premises.name")} required bind:value={name}>
            <small>{t("premises.name_hint")}</small>
          </TextInput>
          {#if isVehicle}
            <TextInput label={t("premises.vehicle_model")} bind:value={vehicleModel} />
            <TextInput label={t("premises.plate")} bind:value={plate} />
          {:else}
            <TextInput label={t("premises.address")} bind:value={address} />
            <TzCombobox
              label={t("premises.class")}
              hint={t("premises.class_hint")}
              items={classes.map((option) => ({ value: option.code, label: option.name }))}
              bind:value={classCode}
            />
          {/if}
          <NumberInput
            label={t("premises.volume")}
            hint={t("premises.volume_hint")}
            min={0.0001}
            bind:value={volume}
          />
          <TextInput label={t("premises.notes")} bind:value={notes} />
        </div>
        <!-- What the SPANISH registries say about this building: the cadastral
             reference Anexo V asks for, and REA's own code for the installation.
             Buildings only, and Spain only, like machinery's ROMA/REGANIP. -->
        {#if farmCountryIsSpain && !isVehicle}
          <fieldset class="es-only">
            <legend>{t("premises.es_section")}</legend>
            <div class="form-grid">
              <TextInput label={t("premises.cadastral_reference")} bind:value={cadastralReference}>
                <!-- Two different statements, so both stay: which reference to
                     enter (Anexo V 1.3: the building's, or the parcel it sits on)
                     and where to go and find it. -->
                <small>{t("premises.cadastral_hint")}</small>
                <RegistryHint country={farmCountry} field="premises.cadastral_reference" />
              </TextInput>
              <TextInput
                label={t("premises.rea_installation_code")}
                bind:value={reaInstallationCode}
              >
                <small>{t("premises.rea_hint")}</small>
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
