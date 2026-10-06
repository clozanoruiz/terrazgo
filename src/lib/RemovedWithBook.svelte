<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // One line on a live book's page whose records were deleted with it and did
  // not come back with it (docs/sync.md → Deleting a book with its records).
  //
  // Two ways in, neither anybody's mistake: the book renamed on another device
  // while this one deleted it — the rename, later on the clock, brings it back
  // live and empty everywhere — or the same campaign opened again, which
  // revives the same book. Either way its records are removed and listed
  // nowhere else, so the page says so and offers them back. Deleting the book
  // again, from the band above, is the other answer.
  //
  // Absent on almost every page: a book never deleted costs the backend one
  // look at its own log to say so.
  import { t } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import { recordKinds, recordTotal, removedNotice } from "./bookRemoval.js";

  let { seasonId, onchanged = null } = $props();

  let removal = $state(null);
  let thisDevice = $state("");
  let busy = $state(false);

  export async function reload() {
    removal = await invoke("removed_with_book", { seasonId });
  }

  run(async () => {
    const [found, peers] = await Promise.all([
      invoke("removed_with_book", { seasonId }),
      invoke("list_sync_peers"),
    ]);
    removal = found;
    thisDevice = peers.this_device;
  });

  /// Bring them back, then read the line again and tell the page: its
  /// registers hold them now. `run` never rethrows, so whether they came back
  /// is carried out in `restored`.
  function restore() {
    let restored = false;
    const count = recordTotal(removal.records);
    run(async () => {
      if (!(await confirmDialog(t("removed_with_book.restore_confirm", { count })))) return;
      busy = true;
      try {
        const back = await invoke("restore_season", { seasonId });
        restored = true;
        notify(t("removed_with_book.restored", { count: back.records }));
      } finally {
        busy = false;
      }
    }).then(
      () =>
        restored &&
        run(async () => {
          await reload();
          onchanged?.();
        }),
    );
  }
</script>

{#if removal}
  <div class="removed-notice" role="status">
    <div class="removed-text">
      <span>{removedNotice(removal, thisDevice)}</span>
      <span class="removed-kinds">{recordKinds(removal.records)}</span>
    </div>
    <button type="button" disabled={busy} onclick={restore}>{t("removed_with_book.restore")}</button
    >
  </div>
{/if}

<style>
  /* The duplicates line's shape, in the conflicts' colour: these records are
     in no book at all, which is worse than a record in one twice. */
  .removed-notice {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-2) 0;
    padding: var(--space-2) var(--space-3);
    border-left: var(--space-1) solid var(--danger);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }

  .removed-text {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    flex: 1 1 18rem;
    min-width: 0;
  }

  .removed-kinds {
    font-size: 0.8125rem;
    color: var(--muted);
  }

  /* A book's page is a framed view on a wide screen: one more fixed band, and
     the frame has no padding of its own to inset it with. */
  @media (min-width: 701px) {
    :global(.view.framed) .removed-notice {
      flex: none;
      margin-inline: var(--space-3);
    }
  }

  @media (pointer: coarse) {
    .removed-notice button {
      min-height: 2.75rem;
      padding-inline: var(--space-4);
    }
  }
</style>
