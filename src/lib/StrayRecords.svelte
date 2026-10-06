<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Records still live in a book that is gone (docs/sync.md → Records in a
  // removed book): one card per removed book, and the two ways out — move its
  // records into a live book of the same farm, or bring the book back.
  //
  // A live record in a removed book is shown by no book page, no print and no
  // export, which makes it worse than a duplicate: nothing else on screen says
  // it exists. It gets there without anybody doing anything wrong — a book
  // merged or deleted on one device while another went on recording in it —
  // so the card explains rather than blames.
  //
  // Nothing is stored about it: the backend works the list out on every call,
  // so reloading is all there is to keeping it current. The book each card
  // suggests is the backend's too — the live book whose campaign overlaps the
  // removed one's most, which after a merge is the book it went into.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzSelect from "./TzSelect.svelte";
  import { bookItem, bookSpan, kindName, recordName, recordsByKind } from "./bookMerge.js";

  let { onchanged = null } = $props();

  /// How many of a book's records the card names one by one. A book removed
  /// with a whole campaign still in it — two devices merging one pair in
  /// opposite directions — can hold hundreds, and the count line above the
  /// names already says how many there are of each kind.
  const NAMED = 20;

  let strays = $state([]);
  /// Farm id → its live books, for each farm a card is about.
  let booksByFarm = $state({});
  /// Removed book id → the live book its records are to move into.
  let into = $state({});
  let busy = $state(false);

  export async function reload() {
    const found = await invoke("list_stray_records");
    const farms = [...new Set(found.map((entry) => entry.season.farm_id))];
    const lists = await Promise.all(farms.map((farmId) => invoke("list_farm_seasons", { farmId })));
    booksByFarm = Object.fromEntries(farms.map((farmId, at) => [farmId, lists[at]]));
    into = Object.fromEntries(found.map((entry) => [entry.season.id, entry.suggested?.id ?? ""]));
    strays = found;
  }

  run(reload);

  const kinds = (entry) =>
    recordsByKind(entry.records)
      .map(({ table, n }) => t("strays.kind_count", { kind: kindName(table), n }))
      .join(" · ");

  /// Run one act on a card, then bring everything showing it up to date.
  /// `work` resolves `false` when the person answered no at a confirmation.
  /// A refusal is followed by a reread too: it is usually somebody else's act
  /// arriving first, and the list is what says so. `run` never rethrows, so
  /// whether anything happened is carried out of it in `settled`.
  function act(work) {
    let settled = false;
    run(async () => {
      busy = true;
      try {
        if ((await work()) === false) return;
        settled = true;
      } catch (err) {
        settled = true;
        throw err;
      } finally {
        busy = false;
      }
    }).then(
      () =>
        settled &&
        run(async () => {
          await reload();
          // Records went back into a book: they may pair with what is there,
          // and they print again.
          onchanged?.();
        }),
    );
  }

  function move(entry) {
    const target = (booksByFarm[entry.season.farm_id] ?? []).find(
      (book) => book.id === into[entry.season.id],
    );
    if (!target) return;
    act(async () => {
      const asking = {
        count: entry.records.length,
        from: entry.season.label,
        into: target.label,
      };
      if (!(await confirmDialog(t("strays.move_confirm", asking)))) return false;
      await invoke("move_stray_records", {
        fromSeasonId: entry.season.id,
        intoSeasonId: target.id,
      });
      notify(t("strays.moved", { into: target.label, count: entry.records.length }));
    });
  }

  function restore(entry) {
    act(async () => {
      const names = { label: entry.season.label, farm: entry.farm_name };
      if (!(await confirmDialog(t("strays.restore_confirm", names)))) return false;
      await invoke("restore_season", { seasonId: entry.season.id });
      notify(t("strays.restored", names));
    });
  }
</script>

{#if strays.length > 0}
  <div class="view-head">
    <h2>{t("strays.title")}</h2>
  </div>
  <p>{t("strays.hint")}</p>
  <ul class="stray-list">
    {#each strays as entry (entry.season.id)}
      {@const books = booksByFarm[entry.season.farm_id] ?? []}
      {@const named = entry.records.slice(0, NAMED)}
      <li class="stray-card">
        <div class="stray-body">
          <h3>{entry.season.label}</h3>
          <p class="stray-meta">
            <span class="stray-tag">{t("strays.removed_book")}</span><span>{entry.farm_name}</span
            ><span>{bookSpan(entry.season)}</span>
          </p>
          <p class="stray-kinds">{kinds(entry)}</p>
          <details>
            <summary>{t("strays.show_records")}</summary>
            <ul class="stray-records">
              {#each named as record (record.table + record.id)}
                <li>{recordName(record)}</li>
              {/each}
            </ul>
            {#if entry.records.length > named.length}
              <p class="stray-more">
                {t("strays.and_more", { more: entry.records.length - named.length })}
              </p>
            {/if}
          </details>
        </div>
        <div class="stray-actions">
          {#if books.length > 0}
            <!-- Pre-selected with the backend's suggestion; empty when no live
                 book shares a day with the removed one, so the person picks
                 rather than accepting a guess. -->
            <TzSelect
              label={t("strays.into")}
              items={books.map(bookItem)}
              placeholder={t("book_merge.pick")}
              bind:value={into[entry.season.id]}
            />
            <button
              type="button"
              disabled={busy || !into[entry.season.id]}
              onclick={() => move(entry)}>{t("strays.move")}</button
            >
          {:else}
            <p class="stray-none">{t("strays.no_book")}</p>
          {/if}
          <button type="button" disabled={busy} onclick={() => restore(entry)}
            >{t("strays.restore")}</button
          >
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  /* Cards, like the conflicts and the duplicates beside them, and in the
     conflicts' --danger: these records are missing from every book, printed
     or exported, which is worse than a record being in one twice. */
  .stray-list {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }

  .stray-card {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius-sm);
  }

  /* The description takes the width it needs and the controls the rest; on a
     narrow screen they fall under it rather than squeezing it. */
  .stray-body {
    flex: 1 1 18rem;
    min-width: 0;
  }

  .stray-card h3 {
    margin: 0;
    font-size: 0.9375rem;
  }

  .stray-meta,
  .stray-kinds {
    margin: var(--space-1) 0 0;
    font-size: 0.8125rem;
  }

  .stray-meta {
    color: var(--muted);
  }

  .stray-meta span + span::before {
    content: " · ";
  }

  .stray-tag {
    color: var(--danger);
    font-weight: 600;
  }

  .stray-card details {
    margin-top: var(--space-1);
    font-size: 0.8125rem;
  }

  .stray-card summary {
    cursor: pointer;
    color: var(--muted);
  }

  .stray-records {
    margin: var(--space-1) 0 0;
    padding-left: var(--space-4);
  }

  .stray-more,
  .stray-none {
    margin: var(--space-1) 0 0;
    color: var(--muted);
  }

  .stray-none {
    font-size: 0.8125rem;
  }

  .stray-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-2);
    flex: 1 1 16rem;
    justify-content: flex-end;
  }

  .stray-actions :global(.tz-field) {
    flex: 1 1 12rem;
    margin: 0;
  }

  @media (pointer: coarse) {
    .stray-actions button {
      min-height: 2.75rem;
      padding-inline: var(--space-4);
    }
  }
</style>
