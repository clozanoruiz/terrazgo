<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Record book, export tab: the printable cuaderno as PDF and as a
  // spreadsheet, over a completeness advisory that tells and never blocks. The
  // shell owns the farm/season selectors and resolves which languages the
  // holding may print in.
  //
  // The SIEX descriptor export is deliberately absent: it has had no delivery
  // path since the platform answer of 2026-08-02, so the command and its
  // precheck stay compiled and tested while nothing offers them (see
  // docs/siex-export.md).
  import { formatDate, formatNumber, t } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzSelect from "./TzSelect.svelte";

  let { farmId, seasonId, seasonLabel, reportLanguages, defaultLanguage } = $props();

  // The farmer's pick, or the backend's default until they make one. It cannot
  // be a plain `$state(defaultLanguage)`: the shell resolves which languages a
  // holding may print in asynchronously, and switching farm remounts this panel
  // before that answer arrives — the snapshot would keep the previous holding's
  // default. Deriving also drops a choice the new holding is not offered, so
  // the export can never be asked for a language it may not print.
  let chosenLanguage = $state(null);
  const reportLanguage = $derived(
    reportLanguages.some((option) => option.code === chosenLanguage)
      ? chosenLanguage
      : defaultLanguage,
  );

  // --- book completeness advisory ------------------------------------------
  // Read on mount and after every export, and deliberately NOT wired into the
  // print path: the printed book shows what exists, and a farmer must be able
  // to print for an inspection while some registry data is still incomplete.
  // This tells; it never blocks.
  let advisory = $state(null);

  $effect(() => {
    const [season, farm] = [seasonId, farmId];
    if (!season || !farm) return;
    run(async () => {
      advisory = await invoke("book_advisory", { seasonId: season, farmId: farm });
    });
  });

  // The three verdicts of RD 1051/2022 art. 4.1, worded per verdict rather than
  // as one sentence with a hole in it: "binding" and "possibly exempt" are
  // different statements, and "undetermined" is the app admitting it cannot
  // measure the holding.
  function dutySentence(gap) {
    // Only the "undetermined" wording counts plots, and only it has plural
    // variants; the other two ignore `count` and resolve to their bare key.
    return t(`advisory.duty_${gap.duty}`, {
      surface: formatNumber(gap.arable_permanent_ha),
      irrigated: formatNumber(gap.irrigated_ha),
      count: gap.plots_without_land_use + gap.plots_without_area,
    });
  }

  function advisoryIsClean(check) {
    return (
      check.farm_missing_fields.length === 0 &&
      check.treatments_missing_crop.length === 0 &&
      check.treatments_missing_efficacy.length === 0 &&
      check.operators_missing_licence.length === 0 &&
      check.registers_undeclared.length === 0 &&
      check.fertilisation_absent === null &&
      check.irrigation_absent === null &&
      check.covers_missing_widths.length === 0 &&
      check.inert_covers_established_late.length === 0 &&
      check.covers_missing_maintenance.length === 0 &&
      check.grazing_records_without_end.length === 0
    );
  }

  // Section 9's findings name a record by the date that identifies it in its
  // own register — the establishment date for a cover, the start of grazing for
  // a grazing — which is what the register lists them by.
  function coverDates(refs) {
    return refs.map((ref) => formatDate(ref.established_on)).join("; ");
  }

  // --- printable cuaderno (PDF) --------------------------------------------
  // No precheck: the printed record book shows the current state, and fields
  // the official model asks for but the data lacks print blank — so the only
  // step is choosing where to save.
  function exportCuadernoPdf() {
    run(async () => {
      const path = await invoke("plugin:dialog|save", {
        options: {
          defaultPath: exportFileName("cuaderno", "pdf", reportLanguage),
          filters: [{ name: "PDF", extensions: ["pdf"] }],
        },
      });
      if (!path) return;
      const summary = await invoke("export_cuaderno_pdf", {
        seasonId,
        farmId,
        destPath: path,
        language: reportLanguage,
      });
      notify(t("message.cuaderno_pdf_exported", { path: summary.path, pages: summary.pages }));
    });
  }

  // --- the same book as a spreadsheet ---------------------------------------
  // Same content and same no-precheck rule as the PDF; the difference is typed
  // cells (real dates and numbers) so the sheet can be sorted, filtered and
  // summed — or handed to a gestoría.
  function exportCuadernoXlsx() {
    run(async () => {
      const path = await invoke("plugin:dialog|save", {
        options: {
          defaultPath: exportFileName("cuaderno", "xlsx", reportLanguage),
          filters: [{ name: "Excel", extensions: ["xlsx"] }],
        },
      });
      if (!path) return;
      const summary = await invoke("export_cuaderno_xlsx", {
        seasonId,
        farmId,
        destPath: path,
        language: reportLanguage,
      });
      notify(t("message.cuaderno_xlsx_exported", { path: summary.path, sheets: summary.sheets }));
    });
  }

  /// Suggested name for an export: `<documento>_<campaña>[_<idioma>]_<fecha>`.
  ///
  /// Underscores separate the fields because the campaign label already
  /// contains hyphens once sanitised ("2025/2026" → "2025-2026"), and the date
  /// is compact (YYYYMMDD) so it cannot be misread as a second year range.
  /// The language code rides along whenever the book has one, so the two
  /// language versions of the same season never look like the same file.
  ///
  /// The date is not decoration: re-exporting a season would otherwise always
  /// propose the same name. On Android that means colliding with the previous
  /// file, and the SAF picker renames the collision badly — it appends the
  /// counter AFTER the extension ("cuaderno.pdf (2)"), because
  /// tauri-plugin-dialog sends `intent.type = "*/*"` and Android then cannot
  /// tell where the extension starts. Distinct names sidestep it, and telling
  /// two exports of the same season apart is worth having regardless.
  function exportFileName(document, extension, language) {
    // Season labels can carry path-hostile characters ("2025/2026").
    const label = (seasonLabel || seasonId).replace(/[^\p{L}\p{N}._-]+/gu, "-");
    const stamp = new Date().toISOString().slice(0, 10).replaceAll("-", "");
    const parts = [document, label, language, stamp].filter(Boolean);
    return `${parts.join("_")}.${extension}`;
  }

  // Advisory findings that name a treatment print it as date + product. A
  // purely non-chemical actuation names no product, so the backend sends null
  // and the wording is ours (the same fallback BookTreatments uses for its
  // record headings).
  function recordLabel(ref) {
    return `${formatDate(ref.application_date)} — ${ref.product_name ?? t("treatment.non_chemical")}`;
  }

  /// The advisory as ROWS rather than eleven hand-written blocks: each finding
  /// is a title and what it is about, in the order the book is read. Built as
  /// data because the table takes one shape and the list is what varies —
  /// eleven `{#if}` blocks each spelling out its own markup was how a finding
  /// came to be worded three different ways.
  ///
  /// `key` is the field name the backend answered under, so it is stable and
  /// unique without inventing an id.
  const findings = $derived.by(() => {
    if (!advisory) return [];
    const rows = [];
    const add = (key, list, detail) => {
      const present = Array.isArray(list) ? list.length > 0 : list !== null;
      if (present) rows.push({ key, title: t(`advisory.${key}`), detail: detail() });
    };

    add("farm_fields", advisory.farm_missing_fields, () =>
      advisory.farm_missing_fields.map((f) => t(`advisory.field_${f}`)).join(", "),
    );
    add("missing_crop", advisory.treatments_missing_crop, () =>
      advisory.treatments_missing_crop
        .map((ref) => `${ref.plot_name} (${formatDate(ref.application_date)})`)
        .join("; "),
    );
    add("missing_efficacy", advisory.treatments_missing_efficacy, () =>
      advisory.treatments_missing_efficacy.map(recordLabel).join("; "),
    );
    add("missing_licence", advisory.operators_missing_licence, () =>
      advisory.operators_missing_licence.map((o) => o.full_name).join("; "),
    );
    add("registers_undeclared", advisory.registers_undeclared, () =>
      advisory.registers_undeclared.map((code) => t(`register_kind.${code}`)).join(", "),
    );
    add("fertilisation_absent", advisory.fertilisation_absent, () =>
      dutySentence(advisory.fertilisation_absent),
    );
    add("irrigation_absent", advisory.irrigation_absent, () =>
      dutySentence(advisory.irrigation_absent),
    );
    // Section 9's findings. All four are record-triggered: a holding that
    // claimed no eco-scheme records nothing here and hears nothing.
    add(
      "covers_missing_widths",
      advisory.covers_missing_widths,
      () =>
        `${t("advisory.covers_missing_widths_hint")} ${coverDates(advisory.covers_missing_widths)}`,
    );
    add(
      "inert_covers_established_late",
      advisory.inert_covers_established_late,
      () =>
        `${t("advisory.inert_covers_established_late_hint")} ${coverDates(
          advisory.inert_covers_established_late,
        )}`,
    );
    add("covers_missing_maintenance", advisory.covers_missing_maintenance, () =>
      coverDates(advisory.covers_missing_maintenance),
    );
    add(
      "grazing_records_without_end",
      advisory.grazing_records_without_end,
      () =>
        `${t("advisory.grazing_records_without_end_hint")} ${advisory.grazing_records_without_end
          .map((ref) => formatDate(ref.started_on))
          .join("; ")}`,
    );
    return rows;
  });
</script>

<div class="view-head">
  <h3>{t("export.pdf_title")}</h3>
  <div class="selector-buttons">
    <!-- One language on offer is not a choice: the chooser appears only where
         the holding's region makes a second one official. -->
    {#if reportLanguages.length > 1}
      <!-- Not nameItems: the offer is already ordered by the region map, with
           Castilian first, and native names are shown untranslated. -->
      <TzSelect
        class="inline-field"
        label={t("export.language")}
        items={reportLanguages.map((option) => ({
          value: option.code,
          label: option.native_name,
        }))}
        value={reportLanguage}
        onchange={(code) => (chosenLanguage = code)}
      />
    {/if}
    <button type="button" onclick={exportCuadernoPdf}>
      {t("export.pdf_run")}
    </button>
    <button type="button" onclick={exportCuadernoXlsx}>
      {t("export.xlsx_run")}
    </button>
  </div>
</div>
<p class="detail">{t("export.pdf_hint")}</p>
<p class="detail">{t("export.xlsx_hint")}</p>
{#if reportLanguages.length > 1}
  <p class="detail">{t("export.language_hint")}</p>
{/if}

{#if advisory && !advisoryIsClean(advisory)}
  <div class="view-head">
    <h3>{t("advisory.title")}</h3>
  </div>
  <p class="detail">{t("advisory.hint")}</p>
  <!-- `rows-static`: a finding is a statement about the book, not a record to
       open. What the farmer does about one is go to the register it names. -->
  <div class="table-wrap">
    <table class="data-table rows-static">
      <thead>
        <tr>
          <th>{t("column.finding")}</th>
          <th>{t("column.detail")}</th>
        </tr>
      </thead>
      <tbody>
        {#each findings as finding (finding.key)}
          <tr>
            <td class="col-name">{finding.title}</td>
            <td class="col-muted">{finding.detail}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if advisory.farm_missing_fields.length > 0}
    <p class="detail"><a href="#/farms">{t("nav.farms")}</a></p>
  {/if}
{/if}
