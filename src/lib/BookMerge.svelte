<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Making this book and another of its farm one (docs/sync.md → Merging two
  // books that turned out to be one campaign): which other book, and which of
  // the two stays. A button in the book's band and the panel it opens.
  //
  // OFFERED ONLY WITH A BOOK WHOSE CAMPAIGN OVERLAPS THIS ONE'S — the backend's
  // `list_merge_candidates`. Two books of one campaign always share days, and
  // a farm's consecutive years never do, so a farm with years of books sees no
  // third button on every book's band, and the button appearing is itself the
  // sign that this book has a twin.
  //
  // THE BOOK THAT STAYS COMES PRE-SELECTED, AND ALIKE ON EVERY DEVICE. Two
  // people merging one pair on two devices in opposite directions would delete
  // both books, so the backend's `kept_book_by_default` answers from what both
  // books' rows say — the one named by its dates, else the older — and the
  // panel says why leaving it is the safe choice. The person may still change
  // it: it is their book.
  //
  // Every check is the backend's. A merge it refuses — a conflict or two
  // opposite removals waiting in the book that goes — is reported in the form,
  // naming where to decide it.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzForm from "./TzForm.svelte";
  import TzFormDialog from "./TzFormDialog.svelte";
  import TzSelect from "./TzSelect.svelte";
  import { bookItem, bookName, onlyOther } from "./bookMerge.js";

  let {
    /// The book whose page this is.
    season,
    /// (kept, absorbedId) => void, once a merge is made.
    onmerged = null,
  } = $props();

  /// The farm's other live books sharing days with this one.
  let others = $state([]);
  let open = $state(false);
  let draft = $state({ otherId: "", keptId: "" });

  const other = $derived(others.find((book) => book.id === draft.otherId) ?? null);

  // Asked again whenever this book's dates change: correcting a campaign typed
  // with the wrong year is what makes its twin overlap it.
  $effect(() => {
    void [season.starts_on, season.ends_on];
    run(loadCandidates);
  });

  async function loadCandidates() {
    others = await invoke("list_merge_candidates", { seasonId: season.id });
  }

  /// Asked again on every opening — cheap, and a book made or merged since the
  /// page opened is then offered or not as it stands.
  function start() {
    run(async () => {
      await loadCandidates();
      await pickOther(onlyOther(others));
      open = true;
    });
  }

  /// The other book chosen, and the pre-selection that follows from it.
  async function pickOther(id) {
    draft.otherId = id;
    draft.keptId = id
      ? await invoke("kept_book_by_default", { firstId: season.id, secondId: id })
      : season.id;
  }

  const keptItems = $derived(
    other
      ? [
          { value: season.id, label: t("book_merge.this_book", { book: bookName(season) }) },
          bookItem(other),
        ]
      : [],
  );

  /// Which of the two stays and which goes, as the draft now says.
  const sides = $derived(
    other
      ? draft.keptId === other.id
        ? { kept: other, absorbed: season }
        : { kept: season, absorbed: other }
      : null,
  );

  /// Whether a merge is under way: it can take seconds on a large book, and a
  /// second press would merge a book that is already gone.
  let merging = $state(false);

  async function submit() {
    const { kept, absorbed } = sides;
    const names = { kept: kept.label, absorbed: absorbed.label };
    if (!(await confirmDialog(t("book_merge.confirm", names)))) return;
    merging = true;
    let merged;
    try {
      merged = await invoke("merge_books", { keptId: kept.id, absorbedId: absorbed.id });
    } finally {
      merging = false;
    }
    open = false;
    // Said, because the page may not show it: the book that went is gone from
    // a list this page does not draw, and when this is the book that went, the
    // page is about to open the other.
    notify(
      merged.moved === 0
        ? t("book_merge.done_empty", names)
        : t("book_merge.done", { ...names, count: merged.moved }),
    );
    onmerged?.(merged.kept, absorbed.id);
    // When this is the book that stays, the page stays too, and the book that
    // went is no longer one to offer.
    run(loadCandidates);
  }
</script>

{#if others.length > 0}
  <button type="button" onclick={start}>{t("record_book.merge")}</button>
{/if}

<TzFormDialog
  {open}
  title={t("book_merge.title")}
  onclose={() => (open = false)}
  body={mergeFields}
  actions={mergeActions}
/>

{#snippet mergeFields(formId)}
  <!-- Refusals name no field: each is about what waits in the book that goes,
       and says where to decide it. -->
  <TzForm id={formId} onsubmit={submit}>
    <p class="merge-hint">{t("book_merge.hint")}</p>
    <div class="form-grid merge-fields">
      <TzSelect
        label={t("book_merge.other")}
        items={others.map(bookItem)}
        placeholder={t("book_merge.pick")}
        required
        value={draft.otherId}
        onchange={(id) => run(() => pickOther(id))}
      />
      {#if other}
        <TzSelect
          label={t("book_merge.kept")}
          hint={t("book_merge.kept_hint")}
          items={keptItems}
          required
          bind:value={draft.keptId}
        />
      {/if}
    </div>
    {#if sides}
      <p class="merge-outcome">
        {t("book_merge.outcome", { kept: sides.kept.label, absorbed: sides.absorbed.label })}
      </p>
    {/if}
  </TzForm>
{/snippet}

{#snippet mergeActions(formId)}
  <div class="form-actions">
    <button type="submit" form={formId} disabled={merging}>{t("book_merge.submit")}</button>
    <button type="button" class="btn-cancel" onclick={() => (open = false)}>
      {t("form.cancel")}
    </button>
  </div>
{/snippet}

<style>
  .merge-hint {
    margin: 0 0 var(--space-3);
  }

  /* One field under the other, each the panel's width: a book is told from its
     twin by the dates at the END of its name, which a half-width trigger cut
     off. */
  .merge-fields {
    grid-template-columns: 1fr;
  }

  /* What pressing the button will do, in the book names the person just
     picked: the last thing read before it. */
  .merge-outcome {
    margin: var(--space-3) 0 0;
    font-weight: 600;
  }
</style>
