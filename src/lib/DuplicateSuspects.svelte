<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Records that may be one operation recorded twice, and the screen a person
  // judges a pair on (docs/sync.md → Duplicate suspects).
  //
  // Two places ask, and they ask different questions — which is the whole of
  // the difference between the two layouts:
  //
  //   * the Status view (no `seasonId`) asks what is waiting now: pairs with a
  //     record in a current book, as cards, like the conflicts above them;
  //   * a book's page asks what is in THAT book, however old it is: one line
  //     above its tabs, opening the same cards in a panel, so a book with
  //     nothing to review spends no height saying so.
  //
  // Nothing is stored about a suspicion: the backend works the list out on
  // every call, so reloading is all there is to keeping it current.
  //
  // THE COMPARISON IS THE CONFLICT REVIEW'S, over two records instead of two
  // versions: the backend lines their rows up and syncFields.js supplies the
  // words. Unlike a conflict, every field either record states is shown — two
  // records are judged on what they share as much as on where they part — and
  // what differs is marked.
  import { formatDate, formatNumber, languageTag, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzDialog from "./TzDialog.svelte";
  import { onSaved } from "./savedCheck.js";
  import { fieldOf, readableLines, startsRow } from "./syncFields.js";

  let { seasonId = null, onchanged = null } = $props();

  let list = $state({ suspects: [], both_removed: [] });
  let thisDevice = $state("");
  let open = $state(false);
  // The pair in the panel — `{ entry, removed }` — or null for the book's list.
  let pair = $state(null);
  let review = $state(null);
  let busy = $state(false);
  // After a form saves: the save being asked about — `{ register, id }` of the
  // record the form saved — and the ids of every record that save wrote, which
  // the review marks as just saved. Null and empty otherwise.
  let afterSave = $state(null);
  let justSaved = $state([]);

  // Fixed for the life of the component: a book's page is remounted per book.
  const inBook = seasonId !== null;
  const waiting = $derived(list.suspects.length + list.both_removed.length);

  export async function reload() {
    list = await invoke("list_duplicates", { seasonId });
  }

  run(async () => {
    const [found, peers] = await Promise.all([
      invoke("list_duplicates", { seasonId }),
      invoke("list_sync_peers"),
    ]);
    list = found;
    thisDevice = peers.this_device;
  });

  const entryKey = (entry) => `${entry.register}|${entry.records[0].id}|${entry.records[1].id}`;

  /// What to call a pair: its register's kind, and the name the book knows
  /// each record by — for most registers its day, and when the two were typed
  /// a day apart, both days.
  function pairTitle(entry) {
    const kind = tCode("entity", entry.register);
    const captions = [...new Set(entry.records.map((r) => r.caption).filter(Boolean))];
    const named = captions.length ? `${kind} · ${captions.map(asText).join(" / ")}` : kind;
    return capitalise(named);
  }

  function asText(caption) {
    return /^\d{4}-\d{2}-\d{2}$/.test(caption) ? formatDate(caption) : caption;
  }

  /// The `entity.*` names are written for the middle of a sentence; as a
  /// heading the first letter is capital, in the reader's own language.
  function capitalise(text) {
    return text ? text[0].toLocaleUpperCase(languageTag()) + text.slice(1) : text;
  }

  /// What to call a device: the name somebody gave it, "this device", or "a
  /// device with no name" — never the UUID. Worded for the middle of a sentence
  /// ("Anotado en…", "Anotados por…"), which is the only place this screen
  /// names one; the Settings list's own words are column values and read badly
  /// there.
  function deviceName(device, label) {
    if (label) return label;
    return device && device === thisDevice ? t("sync.device_this") : t("sync.device_unnamed");
  }

  /// Who wrote a record, as a card names them: the person where the log names
  /// one this device knows, otherwise the device.
  function writer(record) {
    return record.author_name ?? deviceName(record.written_on, record.device_label);
  }

  /// Who removed a record of a pair removed twice over, as the card names them:
  /// the person and the device where the log names both, otherwise the device.
  function remover(record) {
    const device = deviceName(record.removed_on, record.removed_device_label);
    return record.remover_name
      ? t("duplicates.remover", { person: record.remover_name, device })
      : device;
  }

  /// The book a record is in, named with its farm: a book's name is only
  /// unique within its farm, and these lists are read across every farm.
  function bookOf(record) {
    return t("duplicates.in_book", { season: record.season_label, farm: record.farm_name ?? "" });
  }

  /// Each book the pair is in, once.
  const booksOf = (entry) => [...new Set(entry.records.filter((r) => r.season_label).map(bookOf))];

  /// What the red card says after its tag: who removed which copy. The two
  /// copies are usually alike — one operation typed twice — so who WROTE them
  /// tells them apart far less than who removed each.
  function removedMeta(entry) {
    const [first, second] = entry.records.map(remover);
    return [t("duplicates.removed_each", { first, second }), ...booksOf(entry)];
  }

  /// The card's second line: who recorded the two, and in which campaign.
  function meta(entry) {
    const people = [...new Set(entry.records.map(writer))].join(" · ");
    return [t("duplicates.written_by", { people }), ...booksOf(entry)];
  }

  function openPair(entry, removed) {
    run(() => showPair(entry, removed));
  }

  async function showPair(entry, removed) {
    review = await invoke("review_duplicate_pair", {
      register: entry.register,
      firstId: entry.records[0].id,
      secondId: entry.records[1].id,
    });
    pair = { entry, removed };
    open = true;
  }

  function openList() {
    pair = null;
    review = null;
    open = true;
  }

  function closed() {
    pair = null;
    review = null;
    afterSave = null;
    justSaved = [];
  }

  // --- right after a save (docs/sync.md → The same rule, right after the form saves) -------
  // A book's page listens for its registers' forms saving (lib/savedCheck.js).
  // The save has already happened when this runs — nothing here can hold one
  // up or refuse it — so the record is asked about as stored, and a failure
  // here is reported and goes no further than the bell.
  $effect(() => (inBook ? onSaved(checkSaved) : undefined));

  function checkSaved(register, id) {
    return run(async () => {
      // The save may have made a pair or ended one, whatever it resembles.
      await reload();
      const found = await invoke("list_saved_duplicates", { register, recordId: id });
      if (found.suspects.length === 0) return;
      afterSave = { register, id };
      justSaved = found.saved;
      await showPair(found.suspects[0], false);
    });
  }

  /// The next pair the save put its record in, once one is answered — or the
  /// panel closes when there are none left. A record the answer removed is in
  /// no pair any more, so asking again is also how that ends.
  async function nextAfterSave() {
    const found = await invoke("list_saved_duplicates", {
      register: afterSave.register,
      recordId: afterSave.id,
    });
    if (found.suspects.length > 0) {
      justSaved = found.saved;
      await showPair(found.suspects[0], false);
      return;
    }
    open = false;
    closed();
  }

  /// Run one act on the pair under review, then bring everything showing it up
  /// to date. `work` resolves `false` when the person answered no at a
  /// confirmation, and then nothing else happens.
  ///
  /// A refusal is reported by `run` like any failure, and it too is followed by
  /// a reread: it is usually somebody else's decision arriving first — the
  /// record already removed on another device — and the list is what says so.
  /// `run` never rethrows, so whether anything happened is carried out of it in
  /// `settled` rather than read off a rejected promise.
  function act(work, message) {
    let settled = false;
    run(async () => {
      busy = true;
      try {
        if ((await work()) === false) return;
        settled = true;
        notify(message);
      } catch (err) {
        settled = true;
        throw err;
      } finally {
        busy = false;
      }
    }).then(() => settled && run(afterAct));
  }

  async function afterAct() {
    await reload();
    // A record was removed, restored or judged: whatever list shows it, and
    // any alert its date raised, is stale.
    onchanged?.();
    pair = null;
    review = null;
    if (afterSave) {
      await nextAfterSave();
      return;
    }
    // On a book's page the panel goes back to the book's list while there is
    // anything left on it; everywhere else the panel was only the one pair.
    open = inBook && waiting > 0;
  }

  function keep(side) {
    const n = side + 1;
    act(async () => {
      if (!(await confirmDialog(t("duplicates.keep_confirm", { n })))) return false;
      await invoke("keep_duplicate", {
        register: review.register,
        keptId: review.records[side].id,
        removedId: review.records[1 - side].id,
      });
    }, t("duplicates.kept"));
  }

  function bothReal() {
    act(
      () =>
        invoke("mark_duplicates_distinct", {
          register: review.register,
          firstId: review.records[0].id,
          secondId: review.records[1].id,
        }),
      t("duplicates.judged_distinct"),
    );
  }

  function restore(side) {
    act(
      () =>
        invoke("restore_removed_duplicate", {
          register: review.register,
          recordId: review.records[side].id,
        }),
      t("duplicates.restored"),
    );
  }

  /// One value as the screen prints it — the conflict review's rules: the name
  /// of the row it refers to, a catalogue label, a date, a number in the
  /// reader's conventions, or a dash for either kind of nothing.
  function shown(value, line) {
    if (!value) return t("conflicts.absent");
    const raw = value.value;
    if (raw === null || raw === undefined || raw === "") return t("conflicts.absent");
    if (value.display) return value.display;
    const field = fieldOf(line.table, line.column);
    if (field.code) return tCode(field.code, raw);
    if (typeof raw === "number") return formatNumber(raw);
    if (typeof raw === "boolean") return raw ? t("conflicts.yes") : t("conflicts.no");
    if (typeof raw === "string" && /^\d{4}-\d{2}-\d{2}/.test(raw)) {
      return formatDate(raw.slice(0, 10));
    }
    return String(raw);
  }

  /// A line's own label, or the column name for a register nobody has
  /// described in syncFields.js — plain rather than broken.
  function label(line) {
    const field = fieldOf(line.table, line.column);
    return field.key ? t(field.key) : line.column;
  }

  const lines = $derived(
    review
      ? readableLines(review.lines, (line) => line.values.map((value) => shown(value, line)))
      : [],
  );

  const identical = $derived(lines.length > 0 && lines.every((entry) => !entry.differs));

  const panelTitle = $derived(
    pair ? pairTitle(pair.entry) : inBook ? t("duplicates.book_title") : t("duplicates.title"),
  );
</script>

{#snippet cards()}
  <ul class="duplicate-list">
    {#each list.suspects as entry (entryKey(entry))}
      {@const title = pairTitle(entry)}
      <li class="duplicate-card">
        <div class="duplicate-body">
          <h3>{title}</h3>
          <p class="duplicate-meta">
            {#each meta(entry) as part (part)}<span>{part}</span>{/each}
          </p>
        </div>
        <button
          type="button"
          aria-label={`${t("duplicates.review")} — ${title}`}
          onclick={() => openPair(entry, false)}>{t("duplicates.review")}</button
        >
      </li>
    {/each}
    {#each list.both_removed as entry (entryKey(entry))}
      {@const title = pairTitle(entry)}
      <li class="duplicate-card removed">
        <div class="duplicate-body">
          <h3>{title}</h3>
          <p class="duplicate-meta">
            <span class="duplicate-tag">{t("duplicates.both_removed")}</span
            >{#each removedMeta(entry) as part (part)}<span>{part}</span>{/each}
          </p>
        </div>
        <button
          type="button"
          aria-label={`${t("duplicates.review")} — ${title}`}
          onclick={() => openPair(entry, true)}>{t("duplicates.review")}</button
        >
      </li>
    {/each}
  </ul>
{/snippet}

{#if waiting > 0}
  {#if inBook}
    <!-- One line, not the cards: a book's page is its registers, and the
         review is one tap away. -->
    <div class="duplicate-notice" role="status">
      <span>{t("duplicates.book_notice", { count: waiting })}</span>
      <button type="button" onclick={openList}>{t("duplicates.review")}</button>
    </div>
  {:else}
    <div class="view-head">
      <h2>{t("duplicates.title")}</h2>
    </div>
    <p>{t("duplicates.hint")}</p>
    {@render cards()}
  {/if}
{/if}

<TzDialog bind:open title={panelTitle} wide onClose={closed}>
  {#if pair && review}
    {#if afterSave}
      <p>{t("duplicates.saved_hint")}</p>
    {:else if inBook}
      <button type="button" class="link-button duplicate-back" onclick={() => (pair = null)}>
        ← {t("duplicates.back")}
      </button>
    {/if}
    {#if pair.removed}
      <p>{t("duplicates.both_removed_hint")}</p>
    {/if}
    <!-- One column per record, headed by where and by whom it was written: the
         two are otherwise the same kind of thing, and "record 1" is what the
         buttons below can name without a sentence. -->
    <div class="pair-grid">
      <div class="pair-head"></div>
      {#each review.records as record, side (record.id)}
        <div class="pair-head">
          <strong>{t("duplicates.record", { n: side + 1 })}</strong>
          {#if justSaved.includes(record.id)}
            <span class="pair-note just-saved">{t("duplicates.just_saved")}</span>
          {/if}
          <span class="pair-note"
            >{t("duplicates.on_device", {
              device: deviceName(record.written_on, record.device_label),
            })}</span
          >
          {#if record.author_name}
            <span class="pair-note"
              >{t("duplicates.by_person", { person: record.author_name })}</span
            >
          {/if}
          {#if record.written_at}
            <span class="pair-note"
              >{t("duplicates.written_at", {
                date: formatDate(record.written_at.slice(0, 10)),
              })}</span
            >
          {/if}
          {#if record.season_label}
            <span class="pair-note">{bookOf(record)}</span>
          {/if}
          {#if record.removed_at}
            <!-- A copy of a pair removed twice over: who removed it is what
                 tells it from the other. -->
            <span class="pair-note removed"
              >{record.remover_name
                ? t("duplicates.removed_by", {
                    person: record.remover_name,
                    device: deviceName(record.removed_on, record.removed_device_label),
                    date: formatDate(record.removed_at.slice(0, 10)),
                  })
                : t("duplicates.removed_on_device", {
                    device: deviceName(record.removed_on, record.removed_device_label),
                    date: formatDate(record.removed_at.slice(0, 10)),
                  })}</span
            >
          {/if}
        </div>
      {/each}

      {#each lines as entry, at (entry.line.table + entry.line.entity_id + entry.line.column)}
        <!-- Not on the first line: the column headings' own rule is right
             above it, and a second one there reads as a gap. -->
        {@const first = at > 0 && startsRow(entry.line, lines[at - 1].line)}
        <div
          class="pair-label"
          class:row-start={first}
          class:child={!entry.line.root}
          class:differs={entry.differs}
        >
          {label(entry.line)}
        </div>
        {#each entry.shown as text, side (side)}
          <div class="pair-value" class:row-start={first} class:differs={entry.differs}>
            {text}
          </div>
        {/each}
      {/each}
    </div>
    <!-- Two copies of one operation are often identical in every field, and
         two identical columns with a choice under them read as though
         something is being missed. When nothing differs, the screen says the
         choice does not matter; otherwise it says what the marks mean. -->
    <p class="pair-hint">
      {#if identical}
        {pair.removed ? t("duplicates.identical_removed") : t("duplicates.identical")}
      {:else}
        {t("duplicates.differs_hint")}
      {/if}
    </p>
  {:else if inBook}
    {@render cards()}
  {/if}

  {#snippet footer()}
    {#if pair && review}
      <div class="form-actions pair-actions">
        {#each review.records as record, side (record.id)}
          <button
            type="button"
            disabled={busy}
            onclick={() => (pair.removed ? restore(side) : keep(side))}
          >
            {pair.removed
              ? t("duplicates.restore", { n: side + 1 })
              : t("duplicates.keep", { n: side + 1 })}
          </button>
        {/each}
        {#if !pair.removed}
          <button type="button" disabled={busy} onclick={bothReal}
            >{t("duplicates.both_real")}</button
          >
        {/if}
      </div>
    {/if}
  {/snippet}
</TzDialog>

<style>
  /* Cards, like the conflicts above them on the Status view and for the same
     reason: each is a different kind of record with one thing to do about it.
     The leading edge is --warning, the alerts' colour, rather than the
     conflicts' --danger: a conflict is the book holding two answers, a
     suspicion is the app pointing at something that MAY be wrong. */
  .duplicate-list {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }

  .duplicate-card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-left: 3px solid var(--warning);
    border-radius: var(--radius-sm);
  }

  /* An operation removed twice over is not in the book at all, which is worse
     than being in it twice. */
  .duplicate-card.removed {
    border-left-color: var(--danger);
  }

  .duplicate-body {
    flex: 1;
    min-width: 0;
  }

  .duplicate-card h3 {
    margin: 0;
    font-size: 0.9375rem;
  }

  .duplicate-meta {
    margin: var(--space-1) 0 0;
    color: var(--muted);
    font-size: 0.8125rem;
  }

  .duplicate-meta span + span::before {
    content: " · ";
  }

  .duplicate-tag {
    color: var(--danger);
    font-weight: 600;
  }

  /* The book's one line: the leading rule the Status view's notices use, so it
     reads as the app speaking rather than as one more register. */
  .duplicate-notice {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-2) 0;
    padding: var(--space-2) var(--space-3);
    border-left: var(--space-1) solid var(--warning);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }

  .duplicate-notice span {
    flex: 1;
  }

  /* A book's page is a framed view on a wide screen: a flex column whose
     workspace takes what the bands leave. The notice is one more fixed band,
     and the frame has no padding of its own to inset it with. */
  @media (min-width: 701px) {
    :global(.view.framed) .duplicate-notice {
      flex: none;
      margin-inline: var(--space-3);
    }
  }

  .duplicate-back {
    margin-bottom: var(--space-3);
  }

  /* The comparison: the conflict review's grid, a label column and one column
     per record, so the eye runs across a field and down a record. */
  .pair-grid {
    display: grid;
    grid-template-columns: minmax(6rem, 1fr) repeat(2, minmax(6rem, 1.2fr));
    gap: 0 var(--space-3);
    align-items: baseline;
  }

  /* Stretched to the row, so the rule under a heading with one line more — the
     record just saved — lines up with the others instead of stepping down. */
  .pair-head {
    align-self: stretch;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
    line-height: 1.3;
  }

  .pair-note {
    display: block;
    color: var(--muted);
    font-size: 0.8125rem;
  }

  /* Who removed a copy of a pair removed twice over, in the danger colour the
     card uses: it is why the operation is missing. */
  .pair-note.removed {
    color: var(--danger);
  }

  /* Which of the two the person has just typed: the first thing to find in
     a comparison they did not ask for. */
  .pair-note.just-saved {
    color: var(--primary);
    font-weight: 600;
  }

  .pair-label,
  .pair-value {
    padding: var(--space-1) 0;
  }

  /* Every label carries the rule a differing one colours, so marking a line
     never shifts it sideways. */
  .pair-label {
    padding-left: var(--space-2);
    border-left: 3px solid transparent;
    color: var(--muted);
  }

  .pair-label.row-start,
  .pair-value.row-start {
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .pair-label.child {
    padding-left: calc(var(--space-2) + var(--space-3));
  }

  /* What the two do not agree on is what a person decides on, so it carries
     the weight; what they share stays plain and is still there to read. The
     accent is a rule on the label rather than coloured text: --warning is a
     wheat gold that does not hold contrast as text on --bg. */
  .pair-label.differs {
    border-left-color: var(--warning);
    color: var(--text);
  }

  .pair-value.differs {
    font-weight: 600;
  }

  .pair-hint {
    margin-top: var(--space-4);
    color: var(--muted);
    font-size: 0.875rem;
  }

  /* Three buttons, and they wrap: on a phone that is three rows. */
  .pair-actions {
    flex-wrap: wrap;
  }

  @media (pointer: coarse) {
    .duplicate-card button,
    .duplicate-notice button {
      min-height: 2.75rem;
      padding-inline: var(--space-4);
    }
  }

  @media (max-width: 700px) {
    .duplicate-card {
      flex-wrap: wrap;
    }

    .duplicate-body {
      flex-basis: 100%;
    }

    .duplicate-card button {
      margin-left: auto;
    }
  }
</style>
