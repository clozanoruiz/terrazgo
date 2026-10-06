<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The books deleted lately, at the foot of the record-book list, each with
  // what went with it and a way to bring it back (docs/sync.md → Deleting a
  // book with its records).
  //
  // A fold that is closed by default and absent when nothing was deleted: the
  // list above is the books in use, and a deleted one is the exception. Where
  // a farmer looks for a book is where a deleted one is found again — the
  // confirmation before deleting says so.
  //
  // Nothing is stored about it: the backend works the list out on every call.
  // What each entry says — when, by whom, where, what went with it, until when
  // — is the backend's.
  //
  // A book past its thirty days cannot come back, and goes for good on its own
  // (docs/sync.md → The purge, as settled). One that has not gone yet stays
  // here, without the button, saying what it waits for: a device to hear of the
  // deletion, something on the Status view, a record elsewhere that names it,
  // or only the next start.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import { bookSpan } from "./bookMerge.js";
  import { recordKinds, removedLine, restorableUntil, restoredBook } from "./bookRemoval.js";
  import { erasingLine } from "./purge.js";

  let { onrestored = null } = $props();

  let removed = $state([]);
  let thisDevice = $state("");
  let busy = $state(false);

  export async function reload() {
    removed = await invoke("list_removed_books");
  }

  run(async () => {
    const [found, peers] = await Promise.all([
      invoke("list_removed_books"),
      invoke("list_sync_peers"),
    ]);
    removed = found;
    thisDevice = peers.this_device;
  });

  /// Bring one back, then read the list again — and the page above it, where
  /// the book is one of the books in use again. `run` never rethrows, so
  /// whether it came back is carried out in `restored`.
  function restore(entry) {
    let restored = false;
    run(async () => {
      const names = { label: entry.season.label, farm: entry.farm_name };
      if (!(await confirmDialog(t("removed_books.restore_confirm", names)))) return;
      busy = true;
      try {
        const back = await invoke("restore_season", { seasonId: entry.season.id });
        restored = true;
        notify(restoredBook(back.season.label, back.records));
      } finally {
        busy = false;
      }
    }).then(
      () =>
        restored &&
        run(async () => {
          await reload();
          onrestored?.();
        }),
    );
  }
</script>

{#if removed.length > 0}
  <details class="removed-books">
    <summary>{t("removed_books.summary", { count: removed.length })}</summary>
    <p class="removed-hint">{t("removed_books.hint")}</p>
    <ul>
      {#each removed as entry (entry.season.id)}
        <li class="removed-book">
          <div class="removed-body">
            <h3>{entry.season.label}</h3>
            <p class="removed-meta">
              <span>{entry.farm_name}</span><span>{bookSpan(entry.season)}</span>
            </p>
            <p class="removed-meta">{removedLine(entry.removal, thisDevice)}</p>
            {#if entry.erasing}
              <p class="removed-meta">{restorableUntil(entry.removal, entry.erasing)}</p>
              <p class="removed-erasing">{erasingLine(entry.erasing, thisDevice)}</p>
            {:else}
              <p class="removed-kinds">{recordKinds(entry.removal.records)}</p>
              <p class="removed-meta">{restorableUntil(entry.removal)}</p>
            {/if}
          </div>
          {#if !entry.erasing}
            <button type="button" disabled={busy} onclick={() => restore(entry)}
              >{t("removed_books.restore")}</button
            >
          {/if}
        </li>
      {/each}
    </ul>
  </details>
{/if}

<style>
  /* Quiet by design: a fold under the list, closed until asked for. The
     entries are cards like the Status view's, without its colour — a deleted
     book is a choice somebody made, not work waiting. */
  .removed-books {
    margin-top: var(--space-3);
    font-size: 0.875rem;
  }

  .removed-books summary {
    cursor: pointer;
    color: var(--muted);
  }

  .removed-hint {
    margin: var(--space-2) 0 0;
    color: var(--muted);
  }

  .removed-books ul {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
  }

  .removed-book {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  /* The description takes the width it needs and the button the rest; on a
     narrow screen the button falls under it. */
  .removed-body {
    flex: 1 1 18rem;
    min-width: 0;
  }

  .removed-book h3 {
    margin: 0;
    font-size: 0.9375rem;
  }

  .removed-meta,
  .removed-kinds,
  .removed-erasing {
    margin: var(--space-1) 0 0;
    font-size: 0.8125rem;
  }

  .removed-meta {
    color: var(--muted);
  }

  .removed-meta span + span::before {
    content: " · ";
  }

  @media (pointer: coarse) {
    .removed-book button {
      min-height: 2.75rem;
      padding-inline: var(--space-4);
    }
  }
</style>
