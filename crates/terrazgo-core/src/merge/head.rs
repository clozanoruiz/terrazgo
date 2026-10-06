// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which version of a register is current, which one is live, and what it takes
//! to change that.
//!
//! The upper of the two layers in docs/sync.md → "Two layers, and only one of
//! them has policy" — **this is the one with policy**, and all of it:
//!
//!   * [`heads`] reads a register's log and returns the change sets nothing has
//!     seen. One head is the ordinary case; two is a conflict.
//!   * [`live_head`] picks which of them the book shows, by the clock and then
//!     by device id. Deterministic, so every device shows the same thing before
//!     anybody acts.
//!   * [`make_live`] rewinds the branch that is materialised and replays the
//!     one that should be.
//!
//! **A person may merge; an algorithm may not.** Nothing here resolves a
//! conflict — it makes one side live so the book is never broken and never
//! blocks, and leaves both in the log for a person to choose between.
//! Resolving is an ordinary audited write whose stamp merges both heads, which
//! is why `audit::begin` computes a register's vector from every version it has
//! been written with rather than from the newest row.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, Transaction};
use serde_json::Value;

use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::sql::cached_statement;
use crate::sync::{Hlc, VersionVector};

use super::Applier;

/// Every version a register has been written with: one row per change set that
/// touched it, newest clock last.
///
/// Bounded by how often THIS register was edited, never by the size of the log
/// — a seek on `idx_record_change_root`. DISTINCT because a change set writes
/// one vector across all the rows it logs.
pub const REGISTER_HEADS_SQL: &str =
    "SELECT DISTINCT origin_device, origin_seq, hlc, version_vector
     FROM record_change
     WHERE root_table = ?1 AND root_id = ?2
     ORDER BY hlc, origin_device";

/// Every row of a register's log, oldest first — what a rewind and a replay
/// read. Ordered by the clock across change sets and by `id` within one, which
/// is the order the rows were written (a UUIDv7 minted in one process) and so
/// the same answer on every device.
const REGISTER_ROWS_SQL: &str = "SELECT origin_device, origin_seq, entity_table, entity_id, payload
     FROM record_change
     WHERE root_table = ?1 AND root_id = ?2
     ORDER BY hlc, id";

/// Every row a register's log has ever named, which is every row its tables
/// can hold. A seek on `idx_record_change_root`. What an erasure takes out of
/// the tables — the purge's, and an import's when a marker arrives — each
/// preparing it once for the registers it erases: both are occasional, so it
/// stays out of the shared cache.
pub(crate) const REGISTER_ROWS_NAMED_SQL: &str = "SELECT DISTINCT entity_table, entity_id
     FROM record_change WHERE root_table = ?1 AND root_id = ?2";

/// What a register's conflict rows said before this settle. Always run, because
/// a resolution's entire visible effect is that they disappear.
pub(crate) const CLEAR_CONFLICT_SQL: &str =
    "DELETE FROM sync_conflict WHERE root_table = ?1 AND root_id = ?2";

/// Whether a register has a branch in the review queue: a seek on
/// `sync_conflict`'s primary key.
const WAITING_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM sync_conflict WHERE root_table = ?1 AND root_id = ?2)";

/// One losing branch, waiting for a person.
const RECORD_CONFLICT_SQL: &str = "INSERT INTO sync_conflict
       (root_table, root_id, live_device, live_seq, other_device, other_seq,
        season_id, detected_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

/// The campaign a register is in NOW, as its newest log row states it.
///
/// Newest rather than any row: a register can be moved between books — merging
/// two books that turned out to be one campaign re-points everything in one of
/// them — and after that its older rows name the book it left.
const REGISTER_SEASON_SQL: &str = "SELECT season_id FROM record_change
     WHERE root_table = ?1 AND root_id = ?2 AND season_id IS NOT NULL
     ORDER BY hlc DESC, id DESC
     LIMIT 1";

/// A register's newest image, whatever table it is in — what a pre-apply check
/// reads to see what is about to be written. A seek on
/// `idx_record_change_entity`.
const LATEST_IMAGE_SQL: &str = "SELECT payload FROM record_change
     WHERE entity_table = ?1 AND entity_id = ?2
     ORDER BY hlc DESC, id DESC
     LIMIT 1";

/// One change set that touched a register, and the version it left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The device that wrote the change set, and its number there — together
    /// the change set's name, everywhere, forever.
    pub device: String,
    pub seq: i64,
    /// The clock it was stamped with. Decides which head is LIVE and nothing
    /// else: it cannot tell you whether two versions conflict.
    pub hlc: Hlc,
    /// What this version had seen, itself included.
    pub vector: VersionVector,
}

/// Every change set that has touched a register, oldest clock first.
///
/// Bounded by this register's own edit history, and the same read [`heads`] and
/// [`make_live`] both work from — which is why they take it as a slice rather
/// than running the query twice.
fn register_sets(conn: &Connection, root_table: &str, root_id: &str) -> Result<Vec<Head>> {
    let mut stmt = cached_statement(conn, REGISTER_HEADS_SQL)?;
    let mut rows = stmt.query([root_table, root_id])?;
    let mut sets = Vec::new();
    while let Some(row) = rows.next()? {
        let stored: String = row.get("version_vector")?;
        sets.push(Head {
            device: row.get("origin_device")?,
            seq: row.get("origin_seq")?,
            hlc: row.get("hlc")?,
            vector: VersionVector::from_json(&stored)?,
        });
    }
    Ok(sets)
}

/// Whether neither of two change sets had seen the other — the definition of a
/// conflict, and the only question a version vector answers.
fn concurrent(left: &Head, right: &Head) -> bool {
    !left.vector.has_seen(&right.device, right.seq)
        && !right.vector.has_seen(&left.device, left.seq)
}

/// The current versions of a register: the change sets no other change set has
/// seen.
///
/// **One head is the ordinary case.** Two means two devices wrote without
/// seeing each other, which is the definition of a conflict; more than two
/// means three or more did. A register with no log at all has no heads, which
/// is the state of a register this device has never held.
///
/// A change set is not a head when some *other* set's vector has seen it. Its
/// own vector always has — `WriteTx::register` observes the set it belongs to —
/// so the comparison skips itself, by identity rather than by value.
///
/// **A log with rows but no head is refused**, not returned empty. Causality is
/// a DAG, so sets cannot each have seen the other; two that appear to have done
/// are two change sets wearing one `(origin_device, origin_seq)` — the one
/// rewind path docs/sync.md records as unpreventable, two live replicas sharing
/// an identity because the whole app-data directory was copied. Returning
/// nothing would let the apply report success while materialising nothing at
/// all, which is the silent corruption that section says must fail loudly.
/// Slice 3 detects it properly, by change-set hash, and can say so.
pub fn heads(conn: &Connection, root_table: &str, root_id: &str) -> Result<Vec<Head>> {
    let sets = register_sets(conn, root_table, root_id)?;
    let found = heads_of(&sets);
    if found.is_empty() && !sets.is_empty() {
        return Err(CoreError::Invalid("register_has_no_head"));
    }
    Ok(found)
}

/// The heads among sets already read.
fn heads_of(sets: &[Head]) -> Vec<Head> {
    sets.iter()
        .filter(|candidate| {
            !sets.iter().any(|other| {
                !std::ptr::eq(*candidate, other)
                    && other.vector.has_seen(&candidate.device, candidate.seq)
            })
        })
        .cloned()
        .collect()
}

/// The change sets one version's state is made of: the ones it has seen, minus
/// the ones a concurrent set beat.
///
/// **This is what keeps a register's tables a pure function of its log**, and it
/// exists because a rewind is not written down. When a branch loses, the device
/// holding it undoes its rows and the log keeps no record of that — deliberately,
/// since the log is only appended to: its rows are replicated between devices
/// as a set union, and one rewritten here would disagree with every other
/// copy. So the losing set
/// stays in the history, and once a later write has seen both branches it stops
/// being a head and becomes ordinary ancestry inside the winner's lineage. A
/// replay that took the whole lineage would put the rewound rows back, on every
/// device except the one that rewound them (docs/sync.md → A rewind is not
/// written down, so a replay has to infer it).
///
/// Taking the sets newest-clock-first and keeping one only when it is
/// comparable to everything already kept reproduces the sequence of decisions a
/// device actually made: the same `(hlc, device)` order [`live_head`] picks by,
/// applied at every step of the history rather than only at the end. The result
/// is therefore a chain — any two of its sets are causally ordered — which is
/// what makes the before- and after-images compose.
///
/// A set is beaten only by one that is itself kept. That matters: a set can be
/// concurrent with a version that lost long ago, and dropping it for that would
/// take away history the sets after it were written on top of.
fn contributing<'a>(sets: &'a [Head], head: &Head) -> BTreeSet<(&'a str, i64)> {
    let mut history: Vec<&Head> = sets
        .iter()
        .filter(|set| head.vector.has_seen(&set.device, set.seq))
        .collect();
    history.sort_by(|left, right| {
        right
            .hlc
            .cmp(&left.hlc)
            .then_with(|| right.device.cmp(&left.device))
    });
    let mut chain: Vec<&Head> = Vec::new();
    for candidate in history {
        if !chain.iter().any(|kept| concurrent(kept, candidate)) {
            chain.push(candidate);
        }
    }
    chain
        .into_iter()
        .map(|set| (set.device.as_str(), set.seq))
        .collect()
}

/// What one version of a register holds: every row its branch ever touched,
/// against the image that branch left — `None` where the branch does not hold
/// the row at all, because it never created it or deleted it outright.
///
/// Keyed by `(entity_table, entity_id)`, so two versions' states are compared
/// key by key without either side's row order mattering.
pub type BranchState = BTreeMap<(String, String), Option<Value>>;

/// The state each of `heads` holds, in the order the heads were given.
///
/// **This is the live branch read without writing it, and the losing branch
/// read without ever writing it** — the two things a person is shown, and the
/// two states a resolution takes its before- and after-images from. It is the
/// forward half of [`make_live`] collected into a map instead of into the
/// tables, and it uses the same [`contributing`] chain, so what the review
/// shows and what an apply would materialise cannot drift apart.
///
/// The log is read ONCE for all the heads rather than per head: both reads are
/// bounded by this register's own edit history, and a three-way conflict would
/// otherwise run them three times over.
pub fn branch_states(
    conn: &Connection,
    root_table: &str,
    root_id: &str,
    heads: &[Head],
) -> Result<Vec<BranchState>> {
    let sets = register_sets(conn, root_table, root_id)?;
    let rows = register_rows(conn, root_table, root_id)?;
    Ok(heads
        .iter()
        .map(|head| {
            let chain = contributing(&sets, head);
            let branch = rows
                .iter()
                .filter(|row| chain.contains(&(row.device.as_str(), row.seq)));
            last_touch(branch)
                .into_iter()
                .map(|row| {
                    let image = match &row.payload["after"] {
                        Value::Null => None,
                        after => Some(after.clone()),
                    };
                    ((row.entity_table.clone(), row.entity_id.clone()), image)
                })
                .collect()
        })
        .collect())
}

/// Whether a version of a register says the register is removed: its own row
/// removed or absent — or, for a slot, which has no row of its own, no row of
/// it standing.
///
/// What decides whether two versions agree — removed is removed, whatever the
/// record held — and whether the purge may take a register.
pub(crate) fn state_is_removed(state: &BranchState, root_table: &str, root_id: &str) -> bool {
    match state.get(&(root_table.to_owned(), root_id.to_owned())) {
        Some(own) => !is_standing(own),
        None => !state.values().any(is_standing),
    }
}

/// Whether a row's image is in the book: held, and not removed.
pub(crate) fn is_standing(image: &Option<Value>) -> bool {
    image.as_ref().is_some_and(image_stands)
}

/// [`is_standing`] for an image as a log payload holds it, where `null` is a
/// row not held at all.
fn image_stands(image: &Value) -> bool {
    !image.is_null() && image.get("deleted_at").is_none_or(Value::is_null)
}

/// Which head the book shows while a person decides: the greatest clock, ties
/// broken by device id.
///
/// The tie-break has to be total and identical everywhere, or two devices would
/// show different books and the "never broken, never blocks" promise would be
/// a per-device promise. An HLC never ties within a device; the device id
/// settles the rest.
///
/// This is the whole of what the clock decides. It is NOT a claim that the
/// winner is right, or even later in any real sense — only that every device
/// picks the same one (docs/sync.md → Hybrid logical clocks decide which side
/// goes live).
pub fn live_head(heads: &[Head]) -> Option<&Head> {
    heads.iter().max_by(|left, right| {
        left.hlc
            .cmp(&right.hlc)
            .then_with(|| left.device.cmp(&right.device))
    })
}

/// Make `winner` the version of this register that the tables hold, given that
/// `materialised` is the version they hold now.
///
/// Undo the branch that is live back to where the two parted, then replay the
/// winner's branch forward (docs/sync.md → Applying a winner needs rewind and
/// replay). A change set logs only what CHANGED, so a winner's full state
/// cannot be read off one set — which is why this walks the log rather than
/// applying the winning set alone.
///
/// Both directions use the same applier, and in the overwhelmingly common case
/// this is one set undone and one applied. **The log is never rewritten**;
/// only the materialised tables move.
///
/// A branch is [`contributing`]'s chain rather than the head's whole ancestry:
/// a set that lost a conflict was rewound out of the tables and must stay out,
/// even after a later write has absorbed both branches into one lineage.
///
/// Call it inside the apply transaction, with `PRAGMA defer_foreign_keys = ON`:
/// a register may reference rows another device created, so no order of rows
/// satisfies every constraint along the way.
pub fn make_live(
    tx: &Transaction,
    root_table: &str,
    root_id: &str,
    winner: &Head,
    materialised: Option<&Head>,
) -> Result<()> {
    if materialised == Some(winner) {
        return Ok(());
    }
    let sets = register_sets(tx, root_table, root_id)?;
    let rows = register_rows(tx, root_table, root_id)?;
    let winner_chain = contributing(&sets, winner);
    // `None` is a register this device has never held — an incoming farm, a
    // treatment recorded on the other phone. Nothing to rewind, so the whole
    // winner is replayed.
    let live_chain = match materialised {
        Some(current) => contributing(&sets, current),
        None => BTreeSet::new(),
    };

    // Which branch a change set belongs to: its own chain and not the other's.
    // **Shared history is what both CHAINS hold, not what both heads have
    // seen.** A set both heads have seen can be in one chain only — beaten on
    // one side, kept on the other — and a split by the vectors' common ancestor
    // then neither rewinds nor replays it: a plot the live branch never had
    // stays out of the tables, or one the winner dropped stays in, on this
    // device alone (docs/sync.md → Applying a winner needs rewind and replay).
    //
    // What the two chains share is a common prefix: below the newest set both
    // keep, each chain keeps exactly what that set's own chain keeps. So the
    // first touch of a row in the live branch's own part was written on top
    // of that prefix, and its before-image is the prefix's value.
    let branch_of =
        |own: &BTreeSet<(&str, i64)>, other: &BTreeSet<(&str, i64)>, row: &LoggedRow| {
            let set = (row.device.as_str(), row.seq);
            own.contains(&set) && !other.contains(&set)
        };

    let mut applier = Applier::new(tx);
    // Backwards out of the live branch, applying what each row looked like
    // BEFORE it was written.
    if materialised.is_some() {
        let branch = rows
            .iter()
            .filter(|row| branch_of(&live_chain, &winner_chain, row));
        let undone = first_touch(branch)
            .into_iter()
            .rev()
            .map(|row| (row, &row.payload["before"]));
        apply_leaving_first(&mut applier, undone)?;
    }
    // Forwards into the winner's, each row as the branch left it.
    let branch = rows
        .iter()
        .filter(|row| branch_of(&winner_chain, &live_chain, row));
    let replayed = last_touch(branch)
        .into_iter()
        .map(|row| (row, &row.payload["after"]));
    apply_leaving_first(&mut applier, replayed)
}

/// Write each row as `images` pairs it — the rows that leave the book first,
/// then the ones that stand in it, each group in the order given.
///
/// **A row leaving a UNIQUE slot has to free it before another takes it**, and
/// the order of a branch's own change sets does not promise that once each row
/// is reduced to one touch ([`first_touch`], [`last_touch`]). A row's leaving
/// moves to where that row was last touched, which can be after another row's
/// arriving: two declarations of one plot, the one this device holds withdrawn
/// by a phone and then deleted by a resolution, the other brought back by that
/// resolution first. Both end states agree; the partial index, checked per
/// statement, refuses the moment in between (docs/sync.md → A branch is
/// applied a row at a time). Within one register no row standing before and
/// after changes a UNIQUE key — a child row keeps its plot, a slot row its
/// slot — so leaving first is all the order a register needs.
fn apply_leaving_first<'a>(
    applier: &mut Applier,
    images: impl Iterator<Item = (&'a LoggedRow, &'a Value)>,
) -> Result<()> {
    let (leaving, standing): (Vec<_>, Vec<_>) = images.partition(|(_, image)| !image_stands(image));
    for (row, image) in leaving.into_iter().chain(standing) {
        match image {
            Value::Null => applier.delete(&row.entity_table, &row.entity_id)?,
            image => applier.upsert(&row.entity_table, image)?,
        }
    }
    Ok(())
}

/// The first time a branch touched each row, in the branch's own order.
///
/// **A branch is applied a row at a time, not a step at a time.** The states in
/// between are history — the log keeps them, and the reviews read them
/// there — but they are not places the tables have to go
/// through, and going through them is not free: a partial UNIQUE index is
/// checked per statement, so replaying a book that was created and then deleted
/// would take the live name for as long as one statement, and collide with a
/// book that legitimately holds it (docs/sync.md → A branch is applied a row at
/// a time). It is also the difference between an import costing a register's
/// whole edit history and costing its rows.
///
/// A rewind wants this end: what each row was BEFORE the branch touched it.
fn first_touch<'a>(branch: impl Iterator<Item = &'a LoggedRow>) -> Vec<&'a LoggedRow> {
    let mut seen = BTreeSet::new();
    branch
        .filter(|row| seen.insert((row.entity_table.as_str(), row.entity_id.as_str())))
        .collect()
}

/// The last time a branch touched each row, in the branch's own order — what a
/// replay wants, since it is what the branch left behind. See [`first_touch`].
fn last_touch<'a>(branch: impl DoubleEndedIterator<Item = &'a LoggedRow>) -> Vec<&'a LoggedRow> {
    let mut kept = first_touch(branch.rev());
    kept.reverse();
    kept
}

/// Bring one register's tables in line with its log, and say what state it is
/// in afterwards.
///
/// This is what an import calls per register once the incoming rows are in the
/// log. `was_live` is the head the tables held BEFORE those rows arrived —
/// which the importer gets by asking [`live_head`] over [`heads`] *first*,
/// since nothing records it. That is deliberate: a stored "currently
/// materialised" marker would be a second source of truth that a crash between
/// two writes could leave disagreeing with the tables, and the log can always
/// answer the question.
///
/// The returned heads are the register's state after settling: **one means
/// settled, more than one means a conflict**, with the live one first.
///
/// It also brings `sync_conflict` in line — a row per losing branch that says
/// something the live one does not, and none at all once a register has a
/// single head again ([`waiting`] asks). That table is a notepad over the log
/// rather than a record of its own (see its comment in the schema), so it is
/// rewritten from what the log now says and never patched.
pub fn settle(
    tx: &Transaction,
    root_table: &str,
    root_id: &str,
    was_live: Option<&Head>,
) -> Result<Vec<Head>> {
    let mut current = heads(tx, root_table, root_id)?;
    let Some(winner) = live_head(&current).cloned() else {
        return Ok(current);
    };
    refuse_if_unappliable(tx, root_table, root_id)?;
    make_live(tx, root_table, root_id, &winner, was_live)?;
    // Live first, so a caller reading this does not have to ask twice which
    // side the book is showing.
    current.sort_by_key(|head| head != &winner);
    record_conflict(tx, root_table, root_id, &current)?;
    Ok(current)
}

/// Refuse, before anything is written, a register the tables cannot hold.
///
/// **One table is named here, and it is the only one.** `season` carries
/// `UNIQUE (farm_id, label)` over a column a person edits, which is the one
/// shape a merge can never satisfy: two devices can reach the same name
/// independently, and neither fusing the books (two different campaigns would
/// become one) nor renaming one (the label is printed on the cuaderno and sent
/// in the SIEX export) is an answer an algorithm may give. So the apply stops
/// and a person decides — merge the two books if they are one campaign, or
/// rename one to keep both (docs/sync.md → Seasons created on two devices).
///
/// Checked here rather than caught as a constraint failure because the error
/// has to say what happened. `UNIQUE constraint failed: season.farm_id,
/// season.label` is not something a farmer can act on.
///
/// Every other UNIQUE on a synced table is either a slot key — the register's
/// own identity, so two devices filling it write one register — or scoped to a
/// register a merge replaces whole. `sync_shape_contract.rs` holds that line.
///
/// **It asks exactly what the index asks, and no more.** The index is partial —
/// `idx_season_farm_label_active` is `WHERE deleted_at IS NULL` — because a
/// deleted book frees its name, so a book arriving deleted collides with
/// nothing and a check that stopped it would refuse a whole bundle the schema
/// would have taken. `book_using_label` is partial on the other side of the
/// same question.
///
/// It reads the tables, so it answers for the register in front of it and not
/// for the bundle: a delivery where one book releases a name and another takes
/// it depends on which register settles first (docs/sync.md → The collision
/// check sees one register, not the bundle). Resolving that needs the whole
/// bundle in view, which is the importer's, in slice 3.
fn refuse_if_unappliable(tx: &Transaction, root_table: &str, root_id: &str) -> Result<()> {
    if root_table != "season" {
        return Ok(());
    }
    let Some(image) = latest_image(tx, "season", root_id)? else {
        return Ok(());
    };
    // A book on its way out collides with nothing.
    if !image["deleted_at"].is_null() {
        return Ok(());
    }
    let (Some(farm_id), Some(label)) = (image["farm_id"].as_str(), image["label"].as_str()) else {
        return Ok(());
    };
    if crate::repository::book_using_label(tx, farm_id, label, Some(root_id))?.is_some() {
        // A live book of this farm is on this device, so the farm is too.
        let farm = tx
            .query_row("SELECT name FROM farm WHERE id = ?1", [farm_id], |row| {
                row.get(0)
            })
            .optional()?
            .unwrap_or_default();
        return Err(CoreError::SeasonLabelCollision {
            farm,
            label: label.to_owned(),
        });
    }
    Ok(())
}

/// The newest `after` image logged for one row, or `None` when the newest thing
/// that happened to it was a deletion.
fn latest_image(conn: &Connection, entity_table: &str, entity_id: &str) -> Result<Option<Value>> {
    let payload: Option<String> = cached_statement(conn, LATEST_IMAGE_SQL)?
        .query_row([entity_table, entity_id], |row| row.get(0))
        .optional()?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let payload: Value = serde_json::from_str(&payload)?;
    Ok(match &payload["after"] {
        Value::Null => None,
        after => Some(after.clone()),
    })
}

/// Clear what this register's `sync_conflict` rows said and write what is true
/// now: nothing when it has one head, a row per losing branch that says
/// something the live one does not when it has more.
///
/// Cleared first and unconditionally, because a resolution's whole visible
/// effect is that the rows go away.
///
/// **A branch that says what the live one says is not listed** (docs/sync.md →
/// Versions that say the same thing are not listed): its review would read
/// "the two versions say exactly the same", a choice that changes nothing
/// visible. It stays a head in the log, and the next write to the register
/// merges it like any other. Comparing costs a read of this register's log,
/// and only a register with more than one head pays it.
fn record_conflict(
    tx: &Transaction,
    root_table: &str,
    root_id: &str,
    current: &[Head],
) -> Result<()> {
    cached_statement(tx, CLEAR_CONFLICT_SQL)?.execute([root_table, root_id])?;
    let Some((live, losers)) = current.split_first() else {
        return Ok(());
    };
    if losers.is_empty() {
        return Ok(());
    }
    let states = branch_states(tx, root_table, root_id, current)?;
    let Some((live_state, loser_states)) = states.split_first() else {
        return Ok(());
    };
    let season = register_season(tx, root_table, root_id)?;
    let detected_at = now_utc_iso();
    for (loser, state) in losers.iter().zip(loser_states) {
        if super::review::versions_agree(tx, live_state, state, root_table, root_id)? {
            continue;
        }
        cached_statement(tx, RECORD_CONFLICT_SQL)?.execute(rusqlite::params![
            root_table,
            root_id,
            live.device,
            live.seq,
            loser.device,
            loser.seq,
            season,
            detected_at,
        ])?;
    }
    Ok(())
}

/// Whether a register has a branch waiting for a person — a row in the review
/// queue, which is not the same as having more than one head: a branch that
/// says what the live one says is never listed ([`record_conflict`]).
///
/// What an import counts as a conflict, so its message and the queue agree.
pub fn waiting(conn: &Connection, root_table: &str, root_id: &str) -> Result<bool> {
    Ok(cached_statement(conn, WAITING_SQL)?.query_row([root_table, root_id], |row| row.get(0))?)
}

/// The campaign a register belongs to, as its own log rows state it — which is
/// where the review queue gets the book to file a conflict under. NULL for a
/// register that belongs to the holding rather than to one book.
fn register_season(conn: &Connection, root_table: &str, root_id: &str) -> Result<Option<String>> {
    Ok(cached_statement(conn, REGISTER_SEASON_SQL)?
        .query_row([root_table, root_id], |row| row.get(0))
        .optional()?
        .flatten())
}

/// One logged row, as a rewind or a replay needs it.
struct LoggedRow {
    device: String,
    seq: i64,
    entity_table: String,
    entity_id: String,
    payload: Value,
}

/// A register's log rows, oldest first — each checked against the aggregate map
/// before anything is done with it.
///
/// Read whole and partitioned in memory rather than queried once per branch:
/// the `WHERE` that matters is already here — one register — and which branch a
/// row belongs to is a question about version vectors that SQL cannot ask. The
/// result is bounded by this register's own edit history, which is the same set
/// [`heads`] already reads.
///
/// **The check is the receiving half of `audit::write_change`'s.** That one
/// holds every row a device WRITES to the map (`sync::register_of`): the row's
/// table must be one the map classifies as synced, the image must name the row
/// the log row is addressed to, and the register must be the one the row's own
/// values place it in. A row that ARRIVES has had none of that done to it by
/// this device — it is bytes off a bundle, and the table name in it is spliced
/// into SQL by the applier. Checked on both sides, the same map answers for
/// every row in the database however it got there; checked on one, the
/// guarantee stops at the device boundary, which is the one place sync exists
/// to cross.
fn register_rows(conn: &Connection, root_table: &str, root_id: &str) -> Result<Vec<LoggedRow>> {
    let mut stmt = cached_statement(conn, REGISTER_ROWS_SQL)?;
    let mut rows = stmt.query([root_table, root_id])?;
    let mut logged = Vec::new();
    while let Some(row) = rows.next()? {
        let payload: String = row.get("payload")?;
        let logged_row = LoggedRow {
            device: row.get("origin_device")?,
            seq: row.get("origin_seq")?,
            entity_table: row.get("entity_table")?,
            entity_id: row.get("entity_id")?,
            payload: serde_json::from_str(&payload)?,
        };
        // The image that carries the columns placing the row: what it became,
        // or what it was before a hard delete took it away.
        let image = match &logged_row.payload["after"] {
            Value::Null => &logged_row.payload["before"],
            after => after,
        };
        let placed =
            crate::sync::register_of(conn, &logged_row.entity_table, &logged_row.entity_id, image)?;
        if (placed.0.as_str(), placed.1.as_str()) != (root_table, root_id) {
            return Err(CoreError::ShapeViolation(format!(
                "a log row for {} {} arrived filed under {root_table} {root_id}, \
                 but its own values place it in {} {}",
                logged_row.entity_table, logged_row.entity_id, placed.0, placed.1
            )));
        }
        logged.push(logged_row);
    }
    Ok(logged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{install_device, install_shape};
    use rusqlite::params;
    use serde_json::json;
    use uuid::Uuid;

    const DEVICE_A: &str = "0192f3a4-0000-7000-8000-0000000000a1";
    const DEVICE_B: &str = "0192f3a4-0000-7000-8000-0000000000b2";
    const DEVICE_C: &str = "0192f3a4-0000-7000-8000-0000000000c3";

    /// A database at the core schema. The log rows are written BY HAND here on
    /// purpose: two devices editing without seeing each other cannot be staged
    /// through the repositories on one connection, because the second write
    /// would see the first one's log and follow it instead of diverging.
    fn db() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        install_device(&conn, DEVICE_A).unwrap();
        install_shape(&conn, &[crate::sync::CORE_SYNC_SHAPE]).unwrap();
        conn
    }

    fn vv(entries: &[(&str, i64)]) -> VersionVector {
        let mut vector = VersionVector::default();
        for (device, seq) in entries {
            vector.observe(device, *seq);
        }
        vector
    }

    /// One change set, without a log behind it — enough for the questions
    /// [`contributing`] and [`live_head`] answer, which are about vectors and
    /// clocks and nothing else.
    fn set(device: &str, seq: i64, hlc_ms: u64, vector: &[(&str, i64)]) -> Head {
        Head {
            device: device.to_owned(),
            seq,
            hlc: Hlc::from_raw(hlc_ms << Hlc::COUNTER_BITS).unwrap(),
            vector: vv(vector),
        }
    }

    /// Log one row of a change set, with the vector and clock it is to carry.
    #[allow(clippy::too_many_arguments)]
    fn log(
        conn: &Connection,
        device: &str,
        seq: i64,
        hlc_ms: u64,
        table: &str,
        id: &str,
        payload: Value,
        vector: &VersionVector,
    ) {
        conn.execute(
            "INSERT INTO record_change
               (id, entity_table, entity_id, season_id, operation, changed_at, actor, payload,
                root_table, root_id, origin_device, origin_seq, version_vector, hlc)
             VALUES (?1, ?2, ?3, NULL, ?4, '2026-09-20T10:00:00Z', NULL, ?5,
                     ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                Uuid::now_v7().to_string(),
                table,
                id,
                if payload["after"].is_null() {
                    "delete"
                } else if payload["before"].is_null() {
                    "insert"
                } else {
                    "update"
                },
                payload.to_string(),
                "farm",
                id,
                device,
                seq,
                vector.to_json().unwrap(),
                Hlc::from_raw(hlc_ms << Hlc::COUNTER_BITS).unwrap(),
            ],
        )
        .unwrap();
    }

    /// A whole `farm` row as the log carries one — every column, because a
    /// partial image is exactly what the applier refuses.
    fn farm(id: &str, name: &str) -> Value {
        json!({
            "id": id, "name": name, "owner_name": null, "owner_tax_id": null,
            "location_text": null, "address": null, "postal_code": null,
            "phone_fixed": null, "phone_mobile": null, "email": null,
            "opened_on": null, "latitude": null, "longitude": null,
            "country_code": "es", "created_at": "2026-09-20T10:00:00Z",
            "updated_at": "2026-09-20T10:00:00Z", "deleted_at": null
        })
    }

    /// One farm created by A, then edited concurrently by A and B.
    fn diverged(conn: &Connection) {
        let created = vv(&[(DEVICE_A, 1)]);
        log(
            conn,
            DEVICE_A,
            1,
            1000,
            "farm",
            "f1",
            json!({ "before": null, "after": farm("f1", "Los Llanos") }),
            &created,
        );
        // A edits, having seen only its own.
        log(
            conn,
            DEVICE_A,
            2,
            2000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": farm("f1", "La Vega") }),
            &vv(&[(DEVICE_A, 2)]),
        );
        // B edits the same farm, having seen A's creation but not A's edit.
        log(
            conn,
            DEVICE_B,
            1,
            3000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": farm("f1", "El Soto") }),
            &vv(&[(DEVICE_A, 1), (DEVICE_B, 1)]),
        );
    }

    // --- heads -------------------------------------------------------------

    #[test]
    fn a_register_written_once_has_one_head() {
        let conn = db();
        log(
            &conn,
            DEVICE_A,
            1,
            1000,
            "farm",
            "f1",
            json!({ "before": null, "after": farm("f1", "Los Llanos") }),
            &vv(&[(DEVICE_A, 1)]),
        );

        let found = heads(&conn, "farm", "f1").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].device.as_str(), found[0].seq), (DEVICE_A, 1));
    }

    #[test]
    fn a_set_that_saw_the_one_before_it_replaces_it_as_head() {
        let conn = db();
        log(
            &conn,
            DEVICE_A,
            1,
            1000,
            "farm",
            "f1",
            json!({ "before": null, "after": farm("f1", "Los Llanos") }),
            &vv(&[(DEVICE_A, 1)]),
        );
        log(
            &conn,
            DEVICE_A,
            2,
            2000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": farm("f1", "La Vega") }),
            &vv(&[(DEVICE_A, 2)]),
        );

        let found = heads(&conn, "farm", "f1").unwrap();
        assert_eq!(found.len(), 1, "a linear history has one head");
        assert_eq!(found[0].seq, 2);
    }

    #[test]
    fn two_devices_writing_without_seeing_each_other_are_two_heads() {
        let conn = db();
        diverged(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        assert_eq!(found.len(), 2, "that is what a conflict IS");
        let named: Vec<_> = found.iter().map(|h| (h.device.as_str(), h.seq)).collect();
        assert!(named.contains(&(DEVICE_A, 2)) && named.contains(&(DEVICE_B, 1)));
    }

    #[test]
    fn three_devices_can_conflict_at_once() {
        let conn = db();
        diverged(&conn);
        log(
            &conn,
            DEVICE_C,
            1,
            4000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": farm("f1", "Las Eras") }),
            &vv(&[(DEVICE_A, 1), (DEVICE_C, 1)]),
        );

        assert_eq!(heads(&conn, "farm", "f1").unwrap().len(), 3);
    }

    #[test]
    fn a_resolution_that_saw_both_branches_closes_the_conflict() {
        // The shape of a person resolving: an ordinary write whose vector
        // merges both heads, so it dominates everywhere.
        let conn = db();
        diverged(&conn);
        log(
            &conn,
            DEVICE_A,
            3,
            5000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "La Vega"), "after": farm("f1", "La Vega") }),
            &vv(&[(DEVICE_A, 3), (DEVICE_B, 1)]),
        );

        let found = heads(&conn, "farm", "f1").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].device.as_str(), found[0].seq), (DEVICE_A, 3));
    }

    #[test]
    fn a_register_with_no_log_has_no_heads() {
        assert!(heads(&db(), "farm", "nobody").unwrap().is_empty());
    }

    #[test]
    fn a_log_with_rows_but_no_head_is_refused_rather_than_ignored() {
        // Two change sets wearing one `(origin_device, origin_seq)`, which is
        // what two live replicas sharing an identity write — the rewind path
        // docs/sync.md records as unpreventable. Each one's vector has seen the
        // other's name, so each eliminates the other and nothing is left.
        //
        // `record_change`'s UNIQUE stops the pair colliding on one row, so they
        // have to land on two rows of one register: the farm and its
        // representative.
        let conn = db();
        log(
            &conn,
            DEVICE_A,
            1,
            1000,
            "farm",
            "f1",
            json!({ "before": null, "after": farm("f1", "Los Llanos") }),
            &vv(&[(DEVICE_A, 1)]),
        );
        log(
            &conn,
            DEVICE_A,
            1,
            2000,
            "farm_representative",
            "f1",
            json!({ "before": { "farm_id": "f1" }, "after": null }),
            &vv(&[(DEVICE_A, 1), (DEVICE_B, 4)]),
        );

        assert!(
            matches!(
                heads(&conn, "farm", "f1"),
                Err(CoreError::Invalid("register_has_no_head"))
            ),
            "silently returning nothing would let an apply report success while \
             materialising nothing at all"
        );
    }

    // --- what a version's state is made of ---------------------------------

    #[test]
    fn a_set_beaten_by_a_concurrent_one_is_not_part_of_the_state() {
        // The rewind, inferred. A's edit lost to B's, so the devices holding it
        // took it back out; once a resolution has seen both, nothing else says
        // so, and a replay of the whole lineage would put it back.
        let a_created = set(DEVICE_A, 1, 1000, &[(DEVICE_A, 1)]);
        let a_edit = set(DEVICE_A, 2, 2000, &[(DEVICE_A, 2)]);
        let b_edit = set(DEVICE_B, 1, 3000, &[(DEVICE_A, 1), (DEVICE_B, 1)]);
        let resolution = set(DEVICE_A, 3, 5000, &[(DEVICE_A, 3), (DEVICE_B, 1)]);
        let sets = vec![
            a_created.clone(),
            a_edit.clone(),
            b_edit.clone(),
            resolution.clone(),
        ];

        let state = contributing(&sets, &resolution);
        assert!(state.contains(&(DEVICE_A, 1)), "the creation is shared");
        assert!(state.contains(&(DEVICE_B, 1)), "B's edit went live");
        assert!(state.contains(&(DEVICE_A, 3)), "and the resolution itself");
        assert!(
            !state.contains(&(DEVICE_A, 2)),
            "A's edit lost and was rewound, so it is not part of any state again"
        );
    }

    #[test]
    fn a_set_is_beaten_only_by_one_that_is_itself_part_of_the_state() {
        // The case that makes the rule "beaten by a KEPT set" rather than
        // "beaten by any concurrent set with a later clock". A's first edit
        // lost to B's, but A then wrote a second edit on top of its own — still
        // without seeing B — and that one won. The first edit is what the
        // second was written on top of, so dropping it because a version that
        // itself lost had a later clock would take away history the state
        // depends on.
        let a_created = set(DEVICE_A, 1, 1000, &[(DEVICE_A, 1)]);
        let a_first = set(DEVICE_A, 2, 2000, &[(DEVICE_A, 2)]);
        let b_edit = set(DEVICE_B, 1, 3000, &[(DEVICE_A, 1), (DEVICE_B, 1)]);
        let a_second = set(DEVICE_A, 3, 4000, &[(DEVICE_A, 3)]);
        let resolution = set(DEVICE_B, 2, 5000, &[(DEVICE_A, 3), (DEVICE_B, 2)]);
        let sets = vec![
            a_created,
            a_first.clone(),
            b_edit.clone(),
            a_second.clone(),
            resolution.clone(),
        ];

        let state = contributing(&sets, &resolution);
        assert!(
            state.contains(&(DEVICE_A, 2)) && state.contains(&(DEVICE_A, 3)),
            "A's branch went live whole, first edit included"
        );
        assert!(
            !state.contains(&(DEVICE_B, 1)),
            "and B's, which A's second edit beat, did not"
        );
    }

    #[test]
    fn a_state_is_a_chain() {
        // Every pair in it is causally ordered, which is what lets the
        // before- and after-images compose: each was written on top of the one
        // before it.
        let a_created = set(DEVICE_A, 1, 1000, &[(DEVICE_A, 1)]);
        let a_edit = set(DEVICE_A, 2, 2000, &[(DEVICE_A, 2)]);
        let b_edit = set(DEVICE_B, 1, 3000, &[(DEVICE_A, 1), (DEVICE_B, 1)]);
        let c_edit = set(DEVICE_C, 1, 4000, &[(DEVICE_A, 1), (DEVICE_C, 1)]);
        let resolution = set(
            DEVICE_A,
            3,
            5000,
            &[(DEVICE_A, 3), (DEVICE_B, 1), (DEVICE_C, 1)],
        );
        let sets = vec![a_created, a_edit, b_edit, c_edit, resolution.clone()];

        let state = contributing(&sets, &resolution);
        let kept: Vec<&Head> = sets
            .iter()
            .filter(|s| state.contains(&(s.device.as_str(), s.seq)))
            .collect();
        for left in &kept {
            for right in &kept {
                assert!(
                    !concurrent(left, right),
                    "{}/{} and {}/{} are both in the state and concurrent",
                    left.device,
                    left.seq,
                    right.device,
                    right.seq
                );
            }
        }
        assert_eq!(
            kept.len(),
            3,
            "the creation, the version that won, and the resolution"
        );
    }

    // --- which one is live -------------------------------------------------

    #[test]
    fn the_greatest_clock_goes_live() {
        let conn = db();
        diverged(&conn);
        let found = heads(&conn, "farm", "f1").unwrap();
        let live = live_head(&found).unwrap();
        assert_eq!(live.device, DEVICE_B, "B's edit carries the later clock");
    }

    #[test]
    fn an_equal_clock_is_broken_by_device_id() {
        // An HLC never ties within one device, but two devices can stamp the
        // same millisecond with the same counter. Every device must still pick
        // the same winner, or the book differs per device.
        let same = Hlc::from_raw(9000 << Hlc::COUNTER_BITS).unwrap();
        let contenders = vec![
            Head {
                device: DEVICE_B.into(),
                seq: 1,
                hlc: same,
                vector: vv(&[(DEVICE_B, 1)]),
            },
            Head {
                device: DEVICE_A.into(),
                seq: 1,
                hlc: same,
                vector: vv(&[(DEVICE_A, 1)]),
            },
        ];
        assert_eq!(live_head(&contenders).unwrap().device, DEVICE_B);

        let reversed: Vec<_> = contenders.into_iter().rev().collect();
        assert_eq!(
            live_head(&reversed).unwrap().device,
            DEVICE_B,
            "and not by which order they were read in"
        );
    }

    #[test]
    fn no_heads_means_nothing_is_live() {
        assert!(live_head(&[]).is_none());
    }

    // --- rewind and replay -------------------------------------------------

    fn farm_name(conn: &Connection) -> Option<String> {
        conn.query_row("SELECT name FROM farm WHERE id = 'f1'", [], |r| r.get(0))
            .ok()
    }

    /// The tables as device A left them: it wrote both of its own sets.
    fn materialise_as_a(conn: &Connection) {
        conn.execute(
            "INSERT INTO farm (id, name, country_code, created_at, updated_at)
             VALUES ('f1', 'La Vega', 'es', '2026-09-20T10:00:00Z', '2026-09-20T10:00:00Z')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn making_the_other_branch_live_rewinds_and_replays() {
        let mut conn = db();
        diverged(&conn);
        materialise_as_a(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        let winner = found.iter().find(|h| h.device == DEVICE_B).unwrap().clone();
        let loser = found.iter().find(|h| h.device == DEVICE_A).unwrap().clone();

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &winner, Some(&loser)).unwrap();
        tx.commit().unwrap();

        assert_eq!(farm_name(&conn).as_deref(), Some("El Soto"));
    }

    #[test]
    fn the_shared_history_is_not_undone() {
        // Both branches descend from A's creation, so the farm must still be
        // there after the rewind — only the edit is taken back.
        let mut conn = db();
        diverged(&conn);
        materialise_as_a(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        let winner = found.iter().find(|h| h.device == DEVICE_B).unwrap().clone();
        let loser = found.iter().find(|h| h.device == DEVICE_A).unwrap().clone();

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &winner, Some(&loser)).unwrap();
        tx.commit().unwrap();

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM farm WHERE id = 'f1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1, "the creation is shared history, not A's branch");
    }

    #[test]
    fn making_live_the_branch_already_live_changes_nothing() {
        let mut conn = db();
        diverged(&conn);
        materialise_as_a(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        let a = found.iter().find(|h| h.device == DEVICE_A).unwrap().clone();

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &a, Some(&a)).unwrap();
        tx.commit().unwrap();

        assert_eq!(farm_name(&conn).as_deref(), Some("La Vega"));
    }

    #[test]
    fn switching_branches_and_back_returns_the_first_state() {
        // The property a review screen depends on: choosing the other version
        // and changing your mind has to land exactly where you started.
        let mut conn = db();
        diverged(&conn);
        materialise_as_a(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        let a = found.iter().find(|h| h.device == DEVICE_A).unwrap().clone();
        let b = found.iter().find(|h| h.device == DEVICE_B).unwrap().clone();

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &b, Some(&a)).unwrap();
        tx.commit().unwrap();
        assert_eq!(farm_name(&conn).as_deref(), Some("El Soto"));

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &a, Some(&b)).unwrap();
        tx.commit().unwrap();
        assert_eq!(farm_name(&conn).as_deref(), Some("La Vega"));
    }

    #[test]
    fn a_branch_that_deleted_the_row_takes_it_away_and_gives_it_back() {
        let mut conn = db();
        let created = vv(&[(DEVICE_A, 1)]);
        log(
            &conn,
            DEVICE_A,
            1,
            1000,
            "farm",
            "f1",
            json!({ "before": null, "after": farm("f1", "Los Llanos") }),
            &created,
        );
        // B hard-deletes what A created; A meanwhile renames it.
        log(
            &conn,
            DEVICE_A,
            2,
            2000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": farm("f1", "La Vega") }),
            &vv(&[(DEVICE_A, 2)]),
        );
        log(
            &conn,
            DEVICE_B,
            1,
            3000,
            "farm",
            "f1",
            json!({ "before": farm("f1", "Los Llanos"), "after": null }),
            &vv(&[(DEVICE_A, 1), (DEVICE_B, 1)]),
        );
        materialise_as_a(&conn);

        let found = heads(&conn, "farm", "f1").unwrap();
        let a = found.iter().find(|h| h.device == DEVICE_A).unwrap().clone();
        let b = found.iter().find(|h| h.device == DEVICE_B).unwrap().clone();

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &b, Some(&a)).unwrap();
        tx.commit().unwrap();
        assert_eq!(farm_name(&conn), None, "B's branch removed it");

        let tx = conn.transaction().unwrap();
        make_live(&tx, "farm", "f1", &a, Some(&b)).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            farm_name(&conn).as_deref(),
            Some("La Vega"),
            "and A's branch puts it back as A had it"
        );
    }
}
