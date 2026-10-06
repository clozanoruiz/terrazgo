<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The registers two devices wrote at once, and the screen a person chooses on
  // (docs/sync.md → Conflicts as the person sees them).
  //
  // On the Status view rather than in Settings, and for the Status view's own
  // reason: this is work waiting on the farmer, not a preference. A conflict
  // arrives with an import, and the notification that announces it is gone by
  // the time anybody acts on it — so the queue has to live where the app puts
  // everything else that is waiting.
  //
  // The list is holding-wide because not every register is in a book: a plot,
  // an operator and a machine belong to the holding. Each entry says which
  // campaign it is in when it has one.
  //
  // THE COMPARISON IS GENERIC. The backend hands back lines naming a table and
  // a column, having compared the two versions off the log's own row images,
  // and syncFields.js turns those into words. That is what lets a register a
  // future module adds arrive reviewable instead of waiting for a screen of
  // its own.
  import { formatDate, formatNumber, languageTag, t, tCode } from "../i18n.js";
  import { confirmDialog, invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import TzDialog from "./TzDialog.svelte";
  import { fieldOf, orderedLines, startsRow } from "./syncFields.js";

  let { onresolved = null } = $props();

  let conflicts = $state([]);
  let open = $state(false);
  let review = $state(null);
  let busy = $state(false);

  export async function reload() {
    conflicts = await invoke("list_sync_conflicts");
  }

  run(reload);

  /// What to call a register: its kind, and the name the book knows it by
  /// where its table has one. A register whose table names nothing a person
  /// would recognise — a slot, a row whose live version was deleted — is its
  /// kind alone.
  ///
  /// The backend hands back the caption as the column holds it, because only
  /// the column knows what it is; a register named by its DATE therefore
  /// arrives as `2026-05-12` and is read here the way every other value is.
  function registerTitle(entry) {
    const kind = tCode("entity", entry.root_table);
    const named = entry.caption ? `${kind} · ${asText(entry.caption)}` : kind;
    return capitalise(named);
  }

  function asText(caption) {
    return /^\d{4}-\d{2}-\d{2}$/.test(caption) ? formatDate(caption) : caption;
  }

  /// The `entity.*` names are written for the middle of a sentence ("la parcela
  /// «El Soto»"), which is where the alert cards use them. As a heading the
  /// first letter is capital — in the reader's own language, since that is what
  /// decides the casing rules.
  function capitalise(text) {
    return text ? text[0].toLocaleUpperCase(languageTag()) + text.slice(1) : text;
  }

  /// What to call a device: the name somebody gave it, "this device", or a
  /// plain word — never the UUID, which is the whole reason sync_peer exists.
  function deviceName(device, label) {
    if (label) return label;
    return device === thisDevice ? t("sync.peer_this_device") : t("sync.peer_unnamed");
  }

  let thisDevice = $state("");
  run(async () => {
    ({ this_device: thisDevice } = await invoke("list_sync_peers"));
  });

  function openReview(entry) {
    run(async () => {
      review = await invoke("review_sync_conflict", {
        rootTable: entry.root_table,
        rootId: entry.root_id,
      });
      open = true;
    });
  }

  function keep(version) {
    let refused = false;
    run(async () => {
      const device = deviceName(version.device, version.label);
      if (!(await confirmDialog(t("conflicts.keep_confirm", { device })))) return;
      busy = true;
      try {
        await invoke("resolve_sync_conflict", {
          rootTable: review.root_table,
          rootId: review.root_id,
          keepDevice: version.device,
          keepSeq: version.seq,
        });
      } catch (err) {
        refused = true;
        throw err;
      } finally {
        busy = false;
      }
      open = false;
      review = null;
      notify(t("conflicts.kept", { device }));
      await reload();
      // The record itself changed, so whatever list is showing it is stale.
      onresolved?.();
    }).then(async () => {
      if (!refused) return;
      // A refusal is usually somebody else's decision arriving first, and the
      // queue is what says so — reread it rather than leaving a stale row.
      // `run` reports the refusal and never rethrows, so this hangs off its
      // result rather than off a rejected promise, which never comes.
      open = false;
      review = null;
      await reload();
    });
  }

  /// One value as the screen prints it: the name of the row it refers to, a
  /// catalogue label, a date, a number in the reader's conventions, or a dash
  /// for the two kinds of nothing (the version has no such row, or the column
  /// is unset).
  function shown(value, column) {
    if (!value) return t("conflicts.absent");
    const raw = value.value;
    if (raw === null || raw === undefined || raw === "") return t("conflicts.absent");
    if (value.display) return value.display;
    const field = fieldOf(column.table, column.column);
    if (field.code) return tCode(field.code, raw);
    if (typeof raw === "number") return formatNumber(raw);
    if (typeof raw === "boolean") return raw ? t("conflicts.yes") : t("conflicts.no");
    // A date column holds `YYYY-MM-DD`; an instant holds the whole stamp and is
    // read by its day, which is what a person compares.
    if (typeof raw === "string" && /^\d{4}-\d{2}-\d{2}/.test(raw)) {
      return formatDate(raw.slice(0, 10));
    }
    return String(raw);
  }

  /// A line's own label, or the column name for a register nobody has described
  /// in syncFields.js — plain rather than broken.
  function label(line) {
    const field = fieldOf(line.table, line.column);
    return field.key ? t(field.key) : line.column;
  }

  const lines = $derived(review ? orderedLines(review.lines) : []);
</script>

{#if conflicts.length > 0}
  <div class="view-head">
    <h2>{t("conflicts.title")}</h2>
  </div>
  <p>{t("conflicts.hint")}</p>
  <ul class="conflict-list">
    {#each conflicts as entry (entry.root_table + entry.root_id)}
      {@const title = registerTitle(entry)}
      <li class="conflict-card">
        <div class="conflict-body">
          <h3>{title}</h3>
          <p class="conflict-meta">
            <span
              >{t("conflicts.written_on", {
                devices: entry.devices
                  .map((device) => deviceName(device.device, device.label))
                  .join(" · "),
              })}</span
            >{#if entry.season_label}<span
                >{t("conflicts.in_book", {
                  season: entry.season_label,
                  farm: entry.farm_name ?? "",
                })}</span
              >{/if}
          </p>
        </div>
        <button
          type="button"
          aria-label={`${t("conflicts.review")} — ${title}`}
          onclick={() => openReview(entry)}>{t("conflicts.review")}</button
        >
      </li>
    {/each}
  </ul>
{/if}

<TzDialog bind:open title={review ? registerTitle(review) : t("conflicts.title")} wide>
  {#if review}
    <div class="review">
      <!-- One column per version, headed by the device that wrote it. The live
           one leads, because that is the book as it stands. -->
      <div class="review-grid" style={`--versions: ${review.versions.length}`}>
        <div class="review-head"></div>
        {#each review.versions as version (version.device + version.seq)}
          <div class="review-head" class:live={version.live}>
            <strong>{deviceName(version.device, version.label)}</strong>
            <span class="review-note">
              {version.live ? t("conflicts.version_live") : t("conflicts.version_waiting")}
            </span>
            <span class="review-note">
              {t("conflicts.written_at", { date: formatDate(version.changed_at.slice(0, 10)) })}
            </span>
            {#if version.actor_name}
              <span class="review-note"
                >{t("conflicts.written_by", { person: version.actor_name })}</span
              >
            {/if}
          </div>
        {/each}

        {#each lines as line, at (line.table + line.entity_id + line.column)}
          <!-- Not on the first line: the version headings' own rule is
               right above it, and a second one there reads as a gap. -->
          {@const first = at > 0 && startsRow(line, lines[at - 1])}
          <div class="review-label" class:row-start={first} class:child={!line.root}>
            {label(line)}
          </div>
          {#each line.values as value, side (side)}
            <div class="review-value" class:row-start={first} class:live={side === 0}>
              {shown(value, line)}
            </div>
          {/each}
        {/each}
      </div>

      {#if lines.length === 0}
        <p class="table-empty">{t("conflicts.identical")}</p>
      {/if}
      <p class="review-hint">{t("conflicts.edit_hint")}</p>
    </div>
  {/if}

  {#snippet footer()}
    <div class="form-actions review-actions">
      {#each review?.versions ?? [] as version (version.device + version.seq)}
        <button type="button" disabled={busy} onclick={() => keep(version)}>
          {t("conflicts.keep")} — {deviceName(version.device, version.label)}
        </button>
      {/each}
    </div>
  {/snippet}
</TzDialog>

<style>
  /* Cards, like the alerts above them and for the same reason: each row is a
     different kind of record with one thing to do about it, not a list of one
     shape compared down a column. The leading edge is --danger rather than the
     alerts' --warning — an alert is the app telling the farmer something, this
     is the book holding two answers and waiting. */
  .conflict-list {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }

  .conflict-card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius-sm);
  }

  .conflict-body {
    flex: 1;
    min-width: 0;
  }

  .conflict-card h3 {
    margin: 0;
    font-size: 1rem;
  }

  /* The separator is drawn by CSS rather than written into the text, so a
     single fact does not trail a middot — the alert cards' idiom. */
  .conflict-meta {
    margin: var(--space-1) 0 0;
    color: var(--muted);
    font-size: 0.875rem;
  }

  .conflict-meta span + span::before {
    content: " · ";
  }

  /* The comparison: a label column and one column per version, so the eye runs
     across a field and down a version. It is a grid rather than a table because
     the label column is a heading for its row and the versions are columns of
     equal weight — and because a phone has to fold it, which `minmax` does
     without a second markup. */
  .review-grid {
    display: grid;
    grid-template-columns: minmax(6rem, 1fr) repeat(var(--versions), minmax(6rem, 1.2fr));
    gap: 0 var(--space-3);
    align-items: baseline;
  }

  .review-head {
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
    line-height: 1.3;
  }

  .review-head.live {
    border-bottom-color: var(--accent);
  }

  .review-note {
    display: block;
    color: var(--muted);
    font-size: 0.8125rem;
  }

  .review-label,
  .review-value {
    padding: var(--space-1) 0;
  }

  .review-label {
    color: var(--muted);
  }

  /* A child row reads as a block of its own: the rule is what says "these three
     lines are one treated plot" without a heading nobody could name. */
  .review-label.row-start,
  .review-value.row-start {
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .review-label.child {
    padding-left: var(--space-3);
  }

  .review-value.live {
    font-weight: 600;
  }

  .review-hint {
    margin-top: var(--space-4);
    color: var(--muted);
    font-size: 0.875rem;
  }

  /* One button per version, and they wrap: three devices on a phone is three
     rows of one button, not three cramped ones. */
  .review-actions {
    flex-wrap: wrap;
  }
</style>
