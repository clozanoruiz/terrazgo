<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // Status view: the alerts every alert crate raises, assembled by core when
  // the list is read, the corruption warning when there is one, and what is
  // waiting on a decision — conflicts and possible duplicates. Backup
  // export/import lives in the Settings view.
  //
  // It opened with a strip of app facts — database path, schema version, app
  // version — until 2026-09-06. They are the About panel's technical tab now
  // (Settings → About): three constants that never change while the app is
  // open were furniture above the one list on this screen that does.
  import { formatDate, t, tCode } from "../i18n.js";
  import { invoke } from "./backend.js";
  import { notify, run } from "./notifications.svelte.js";
  import Skeleton from "./Skeleton.svelte";
  import DuplicateSuspects from "./DuplicateSuspects.svelte";
  import StrayRecords from "./StrayRecords.svelte";
  import SyncConflicts from "./SyncConflicts.svelte";

  let status = $state(null);
  let alerts = $state([]);
  // Records whose value a rule could not read — named, so the farmer can open
  // the record and correct it. Every other record's alerts are listed anyway.
  let unchecked = $state([]);
  // The alert crates that could not report at all ({ source, detail }). Not
  // empty means the list is incomplete, and the view says so rather than
  // letting it pass for the whole answer.
  let unavailable = $state([]);
  let loading = $state(true);

  // Worked out on every call — nothing is stored — so this is the only reload
  // there is: a save elsewhere, an imported bundle or midnight passing all show
  // here the next time it runs, with no refresh button to remember.
  async function reloadAlerts() {
    const list = await invoke("list_alerts");
    alerts = list.alerts;
    unchecked = list.unchecked;
    unavailable = list.unavailable;
  }

  run(async () => {
    status = await invoke("get_status");
    await reloadAlerts();
  }).finally(() => (loading = false));

  // The lists of what waits on a decision can change each other: a conflict
  // resolved to the version filing a record in a removed book strands it, a
  // removed copy of a duplicate restored into one does too, and records moved
  // back into a book may pair with what is there.
  let strays = $state(null);
  let duplicates = $state(null);

  /// A conflict resolved or a duplicate decided: a record changed, and it may
  /// have raised or ended an alert, or landed in a removed book.
  function recordDecided() {
    run(async () => {
      await reloadAlerts();
      await strays?.reload();
    });
  }

  function strayMoved() {
    run(async () => {
      await reloadAlerts();
      await duplicates?.reload();
    });
  }

  function seed() {
    run(async () => {
      const summary = await invoke("seed_demo_data");
      notify(
        summary.seeded
          ? t("message.seeded", { season: summary.season_label, farm: summary.farm_name })
          : t("message.already_seeded"),
      );
      await reloadAlerts();
    });
  }

  // An alert has no id: nothing stores it. It is its condition — the kind and
  // what it is about — which is also what the card's key is made of.
  const alertKey = (alert) => `${alert.alert_type_code}|${alert.subject_table}|${alert.subject_id}`;

  // What an act is filed under: the alert as this card showed it, date
  // included, so the act means "this deadline" and a moved one comes back.
  // No table: the kind names it, in Rust. Tauri exposes snake_case Rust
  // command arguments as camelCase in JS.
  const shown = (alert) => ({
    alertTypeCode: alert.alert_type_code,
    subjectId: alert.subject_id,
    dueDate: alert.due_date,
  });

  function acknowledge(alert) {
    run(async () => {
      await invoke("acknowledge_alert", shown(alert));
      await reloadAlerts();
    });
  }

  function dismiss(alert) {
    run(async () => {
      await invoke("dismiss_alert", shown(alert));
      await reloadAlerts();
    });
  }

  // ONE control per card, in two stages (2026-09-10). The two states behind it
  // are not alternatives a farmer should have to choose between: acknowledging
  // keeps the alert listed because the condition it reports still holds, and
  // dismissing is what hides one whose condition never lapses on its own — the
  // standing zone flags. So the card offers "seen" first and only offers to hide
  // it afterwards, which reads as a progression instead of two similar buttons
  // whose difference lives in the repository.
  const seen = (alert) => alert.status === "acknowledged";

  // Where a farmer corrects a record of each kind: the screen, then its tab,
  // named with those screens' own labels so the hint follows any renaming. A
  // kind of record not listed here gets the generic hint rather than a wrong
  // one. Presentation, not a domain rule: which record is broken is Rust's
  // answer, where the form for it lives is this app's layout.
  const PLACES = {
    operator: ["nav.registry", "tab.operators"],
    machinery: ["nav.registry", "tab.machinery"],
    treatment_record: ["nav.record_book", "book.tab_treatments"],
  };

  function fixHint(record) {
    const place = PLACES[record.subject_table];
    return place
      ? t("alerts.unchecked.fix", { place: place.map((key) => t(key)).join(" → ") })
      : t("alerts.unchecked.fix_generic");
  }

  // A crate that failed whole is nothing the farmer can correct in their data,
  // so the notice says what they can do instead: update, and report it with
  // the detail below — through the same allowlisted link the About panel uses.
  function reportProblem() {
    run(() => invoke("open_external_link", { target: "issues" }));
  }

  const technicalDetail = $derived(
    unavailable.map((failed) => `${failed.source}: ${failed.detail}`).join("\n"),
  );

  // What the card says instead of a date column. A standing alert has no date
  // to end on, so it is phrased as a standing condition rather than left blank;
  // an overdue one — an expired carné, an ITV past its date — says the date has
  // passed, never "until". Both calls are Rust's rather than this file's: each
  // kind declares whether it is standing beside the rule that raises it, and
  // core decides `overdue` from the day the list was read.
  function when(alert) {
    if (alert.standing) return t("alert.standing");
    if (!alert.due_date) return "";
    const date = formatDate(alert.due_date);
    return alert.overdue ? t("alert.overdue_since", { date }) : t("alert.until", { date });
  }
</script>

<section class="view">
  <!--
    Only ever shown when the database is damaged. A healthy one says nothing:
    the check runs weekly in the background, and reporting "all fine" every
    time would train the farmer to ignore the one time it is not.
  -->
  {#if status?.integrity && !status.integrity.ok}
    <p class="integrity-warning" role="alert">
      <strong>{t("status.integrity.failed")}</strong>
      {t("status.integrity.restore", { date: formatDate(status.integrity.at) })}
    </p>
  {/if}

  <div id="actions" aria-label={t("actions.aria")}>
    <button type="button" onclick={seed}>{t("actions.seed")}</button>
  </div>

  <!-- Above the alerts, and deliberately: an alert is the app telling the
       farmer something, a conflict is the book holding two answers and waiting
       for a decision. The section draws nothing when the queue is empty, which
       is almost always. -->
  <SyncConflicts onresolved={recordDecided} />

  <!-- Below the conflicts, because a record with a conflict waiting cannot be
       moved until it is decided; above the duplicates, because a record in a
       removed book is missing from every book, which is worse than being in
       one twice. -->
  <StrayRecords bind:this={strays} onchanged={strayMoved} />

  <!-- Below the conflicts and above the alerts: like a conflict it waits on a
       decision, but it is a suspicion rather than two answers the book holds.
       Keeping a record removes the other, whose plazo may have been raising an
       alert — so the alerts are reread after any decision here. -->
  <DuplicateSuspects bind:this={duplicates} onchanged={recordDecided} />

  <div class="view-head">
    <h2>{t("alerts.title")}</h2>
  </div>
  {#if loading}
    <Skeleton />
  {:else}
    {#if unavailable.length > 0}
      <!-- Visible, never only a log line: a list missing one crate's alerts
           must not read as "nothing is wrong". And it says what to do, since
           the farmer cannot correct this one in their data. -->
      <div class="alerts-unavailable" role="status">
        <p>
          {t("alerts.unavailable", {
            sources: unavailable.map((failed) => tCode("alert.source", failed.source)).join(", "),
          })}
        </p>
        <p>{t("alerts.unavailable_hint")}</p>
        <button type="button" onclick={reportProblem}>{t("about.link_issues")}</button>
        <details>
          <summary>{t("alerts.unavailable_detail")}</summary>
          <pre>{technicalDetail}</pre>
        </details>
      </div>
    {/if}
    {#if unchecked.length > 0}
      <!-- One card per record a rule could not read: which record, the value
           as stored, and where to correct it. No Seen button — there is no
           condition to acknowledge until the value can be read. -->
      <ul class="alert-list">
        {#each unchecked as record (alertKey(record))}
          <li class="alert-card unchecked">
            <div class="alert-body">
              <h3>
                {t("alerts.unchecked.title", {
                  kind: tCode("alert.type", record.alert_type_code),
                })}
              </h3>
              <p class="alert-meta">
                <span class="alert-when"
                  >{t("alerts.unchecked.value", { value: record.value })}</span
                ><span class="alert-subject" class:kind-only={!record.subject_label}
                  >{record.subject_label ?? tCode("entity", record.subject_table)}</span
                >
              </p>
              <p class="alert-fix">{fixHint(record)}</p>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    {#if alerts.length === 0}
      <!-- "No alerts" only when the list is whole and nothing needs correcting;
           otherwise what is above has said what it could. -->
      {#if unavailable.length === 0 && unchecked.length === 0}
        <p class="table-empty">{t("alerts.empty")}</p>
      {/if}
    {:else}
      <ul class="alert-list">
        {#each alerts as alert (alertKey(alert))}
          {@const title = tCode("alert.type", alert.alert_type_code)}
          {@const action = seen(alert) ? t("actions.dismiss") : t("actions.ack")}
          {@const dateLine = when(alert)}
          <li class="alert-card" class:seen={seen(alert)} class:overdue={alert.overdue}>
            <div class="alert-body">
              <h3>{title}</h3>
              <!-- The subject is the NAME of the thing when the backend could
                 resolve one: "Los Alcores" answers the question, "parcela" only
                 repeats what the title already said. The kind is the fallback for
                 an id whose row is momentarily gone.
                 The two spans close against the next tag (`</span\n>`) because
                 the separator is drawn by CSS: a newline between them collapses
                 to a space, and two of those either side of the middot is what
                 made the gap read as a gutter. -->
              <p class="alert-meta">
                {#if dateLine}<span class="alert-when" class:overdue={alert.overdue}
                    >{dateLine}</span
                  >{/if}<span class="alert-subject" class:kind-only={!alert.subject_label}
                  >{alert.subject_label ?? tCode("entity", alert.subject_table)}</span
                >
              </p>
            </div>
            {#if seen(alert)}
              <span class="alert-tag">{tCode("alert.status", alert.status)}</span>
            {/if}
            <!-- The visible word alone ("Descartar") does not say WHAT is being
               dismissed once a screen reader has moved past the heading, so the
               label names the alert. It opens with the visible text, which is
               what keeps a spoken command matching the button (WCAG 2.5.3). -->
            <button
              type="button"
              aria-label={`${action} — ${title}`}
              onclick={() => (seen(alert) ? dismiss(alert) : acknowledge(alert))}>{action}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  /* Shown only when the weekly corruption check found damage, so it is allowed
     to be loud — a farmer whose record book is failing needs to see it before
     the next backup overwrites a good one. Left border rather than a filled
     block: --danger stays legible on --surface in either theme, while text on a
     filled --danger would need its own light colour. */
  .integrity-warning {
    margin: var(--space-4) 0;
    padding: var(--space-3) var(--space-4);
    border-left: var(--space-1) solid var(--danger);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--danger);
  }

  .integrity-warning strong {
    display: block;
  }

  /* Quieter than the integrity warning — nothing is damaged, one crate's
     alerts just could not be worked out this time — but on the same leading
     rule, so it reads as the app speaking rather than as an alert. */
  .alerts-unavailable {
    margin: var(--space-3) 0 0;
    padding: var(--space-2) var(--space-4);
    border-left: var(--space-1) solid var(--warning);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }

  .alerts-unavailable p {
    margin: 0 0 var(--space-2);
  }

  .alerts-unavailable details {
    margin-top: var(--space-2);
    color: var(--muted);
    font-size: 0.8125rem;
  }

  /* The detail is for a report, so it must survive being selected and copied
     whole: wrapped rather than scrolled, and never cut off at the card edge. */
  .alerts-unavailable pre {
    margin: var(--space-1) 0 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  /* A record to correct rather than a condition to act on: the same card, its
     rule dashed and in --danger, and one more line saying where to fix it. */
  .alert-card.unchecked {
    border-left-style: dashed;
    border-left-color: var(--danger);
  }

  .alert-fix {
    margin: var(--space-1) 0 0;
    font-size: 0.8125rem;
  }

  /* Alerts are cards again (2026-09-10), and the register tables stay tables.
     What earns a table is a list whose rows are the same shape and get compared
     down a column; these six alert kinds are not that. Three of them carry no
     real date, the useful sentence differs per kind, and there is one thing to do
     with each — so the columns were mostly holding blanks and one repeated word.

     Scoped rather than global: no rule outside this component targets an alert
     card, which is the test for where a rule lives (frontend-conventions.md →
     Styling). */
  .alert-list {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }

  /* No fill: --surface equals --bg by design, so a card reads by its rule, plus
     the gold bar on the leading edge that made the table rows read as alerts. */
  .alert-card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-left: 3px solid var(--warning);
    border-radius: var(--radius-sm);
  }

  /* `min-width: 0` is what lets a long title wrap instead of pushing the button
     past the card's trailing edge: a flex item's default floor is its content. */
  .alert-body {
    flex: 1;
    min-width: 0;
  }

  .alert-card h3 {
    margin: 0;
    font-size: 0.9375rem;
  }

  .alert-meta {
    margin: var(--space-1) 0 0;
    color: var(--muted);
    font-size: 0.8125rem;
  }

  /* Both halves of the line are facts the farmer needs — when, and which one —
     so both read at full strength. Only the fallback recedes: naming the KIND of
     thing is what the line says when it could not name the thing. */
  .alert-when,
  .alert-subject {
    color: var(--text);
  }

  .alert-subject.kind-only {
    color: var(--muted);
  }

  /* An overdue alert is the most urgent state on the list — a carné that has
     already expired, an ITV past its date — so its accent and its date turn to
     --danger, and the date is what carries the change: the title names the
     kind, which is the same before and after. */
  .alert-card.overdue {
    border-left-color: var(--danger);
  }

  .alert-when.overdue {
    color: var(--danger);
    font-weight: 600;
  }

  /* The separator belongs to the pair, not to the subject: an alert with no date
     line renders no .alert-when at all, and would otherwise open with a stray
     middot. A sibling selector rather than `:empty` on a rendered-but-blank span,
     because an empty text node is not reliably `:empty`. */
  .alert-when + .alert-subject::before {
    content: "·";
    margin: 0 var(--space-1);
    color: var(--muted);
  }

  .alert-tag {
    color: var(--muted);
    font-size: 0.6875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Acknowledged alerts stay listed by design — just subdued, and the accent
     recedes to a hairline. The fade lands on the TEXT rather than the card: the
     button is now the way to hide the alert, and a faded control is exactly what
     a farmer stops finding. */
  .alert-card.seen {
    border-left-color: var(--border);
  }

  .alert-card.seen .alert-body {
    opacity: 0.6;
  }

  /* The card's one control is what a farmer taps on a phone, and a button sized
     by its padding alone comes out around 30px — under the 44 CSS px floor the
     owned controls state for exactly this reason (`.tz-option` in styles.css).
     Padding grows a button with long text; it cannot lift a one-word one, so the
     floor is stated outright. */
  @media (pointer: coarse) {
    .alert-card button {
      min-height: 2.75rem;
      padding-inline: var(--space-4);
    }
  }

  /* On a phone the title takes the width and the action goes under it: a wrapped
     three-line heading squeezed beside a button is how the card stops reading as
     one statement. */
  @media (max-width: 700px) {
    .alert-card {
      flex-wrap: wrap;
    }

    .alert-body {
      flex-basis: 100%;
    }

    .alert-card button {
      margin-left: auto;
    }
  }
</style>
