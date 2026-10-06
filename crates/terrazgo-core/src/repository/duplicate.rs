// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Duplicate suspects: the list, and what people say about the pairs in it
//! (docs/sync.md → Duplicate suspects).
//!
//! **Nothing here stores a suspicion.** [`list_duplicates`] runs every
//! register's rules whenever it is read, and [`review_pair`] puts two records
//! side by side. What is stored is what a person said, as one insert-only
//! `duplicate_verdict` row per act:
//!
//!   * [`mark_distinct`] — both records are real;
//!   * [`keep_duplicate`] — they are one operation: one is kept, and the other
//!     is removed by its own register's delete in the same change set as the
//!     act, so the audit trail reads the removal and its reason together;
//!   * [`restore_removed_duplicate`] — two people removed opposite copies,
//!     offline, and one comes back.
//!
//! The register's table always comes from its [`DuplicatePolicy`], a constant
//! in the crate that owns it — never from a screen.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::audit::{WriteTx, begin, log_insert};
use crate::date::now_utc_iso;
use crate::duplicates::{
    Candidate, ChildSets, DuplicatePolicy, Key, Overlap, Scope, Verdict, both_removed_sql,
    candidates_sql, find_pairs, ordered_pair, overlap_sql, record_candidates_sql,
    record_overlap_sql,
};
use crate::error::{CoreError, Result};
use crate::merge::{ReviewLine, RowCaption};
use crate::models::DuplicateVerdict;
use crate::sql::children_by_parent;

use super::season::BookName;

/// Whether a pair already has a "both are real" act. A seek on
/// `idx_duplicate_verdict_pair`.
const DISTINCT_HELD_SQL: &str = "SELECT EXISTS(SELECT 1 FROM duplicate_verdict
     WHERE first_id = ?1 AND second_id = ?2 AND subject_table = ?3 AND verdict = 'distinct')";

/// The acts that kept one record. A seek on `idx_duplicate_verdict_kept`.
const KEPT_BY_SQL: &str =
    "SELECT id, subject_table, first_id, second_id, verdict, kept_id, created_at
     FROM duplicate_verdict
     WHERE kept_id = ?1 AND verdict = 'duplicate' AND subject_table = ?2";

/// Whether an act kept this record of this pair.
const PAIR_KEPT_SQL: &str = "SELECT EXISTS(SELECT 1 FROM duplicate_verdict
     WHERE first_id = ?1 AND second_id = ?2 AND kept_id = ?3 AND verdict = 'duplicate')";

/// Every row one change set logged, in the order it wrote them. A seek on the
/// leading pair of `record_change`'s UNIQUE index.
const CHANGE_SET_ROWS_SQL: &str = "SELECT entity_table, season_id, root_table, root_id, payload
     FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2
     ORDER BY id";

const INSERT_VERDICT_SQL: &str = "INSERT INTO duplicate_verdict
       (id, subject_table, first_id, second_id, verdict, kept_id, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

/// Read a verdict out of `duplicate_verdict.verdict`. The column's CHECK allows
/// only the two codes, so an unknown one is a damaged row, and saying so beats
/// reading it as either.
impl FromSql for Verdict {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let code = value.as_str()?;
        Verdict::from_code(code)
            .ok_or_else(|| FromSqlError::Other(format!("unknown verdict {code:?}").into()))
    }
}

fn map_verdict(row: &Row<'_>) -> rusqlite::Result<DuplicateVerdict> {
    Ok(DuplicateVerdict {
        id: row.get("id")?,
        subject_table: row.get("subject_table")?,
        first_id: row.get("first_id")?,
        second_id: row.get("second_id")?,
        verdict: row.get("verdict")?,
        kept_id: row.get("kept_id")?,
        created_at: row.get("created_at")?,
    })
}

// ---------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------

/// One record of a listed pair, named the way a person knows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateRecord {
    pub id: String,
    /// The book it is in, that book's name, and its farm's — two farms may
    /// each keep a book called "2025/2026", and a list read across books says
    /// which.
    pub season_id: Option<String>,
    pub season_label: Option<String>,
    pub farm_name: Option<String>,
    /// What its register's name column holds ([`RowCaption`]) — for most
    /// registers the day, as stored. `None` for a register nobody named.
    pub caption: Option<String>,
    /// The day the operation began, for a register whose rules compare one.
    pub day: Option<String>,
    /// Who wrote it first, from the log: the device and its name, the profile
    /// and its name, and when. A claim from another device may name a device
    /// or a profile this one has not been told the name of.
    pub written_on: Option<String>,
    pub device_label: Option<String>,
    pub written_by: Option<String>,
    pub author_name: Option<String>,
    pub written_at: Option<String>,
    /// For a pair removed twice over, who removed THIS record: the device and
    /// its name, the profile and its name, and when — read off the act that
    /// kept the other one. The records of such a pair are often identical, and
    /// what tells them apart for a person is who removed which. `None` for any
    /// other record.
    pub removed_on: Option<String>,
    pub removed_device_label: Option<String>,
    pub removed_by: Option<String>,
    pub remover_name: Option<String>,
    pub removed_at: Option<String>,
}

/// Two live records a register's rules say may be one operation recorded
/// twice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuspectedDuplicate {
    /// The register — its table, from its policy.
    pub register: &'static str,
    /// Smaller id first, as a verdict names them.
    pub records: [DuplicateRecord; 2],
    /// Whether they were first written on two devices or by two people — far
    /// likelier one operation recorded twice than two genuine passes, so these
    /// are listed first. It ranks; it never decides.
    pub apart: bool,
}

/// Two records of one operation that were BOTH removed, by opposite verdicts
/// made on two devices before either heard of the other — the operation is in
/// the book zero times, and one should be restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BothRemoved {
    pub register: &'static str,
    pub records: [DuplicateRecord; 2],
}

/// What the list holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DuplicateList {
    pub suspects: Vec<SuspectedDuplicate>,
    pub both_removed: Vec<BothRemoved>,
}

/// Every pair `policies`' rules raise in `scope`, minus the ones somebody
/// judged distinct, and every pair removed twice over.
///
/// **Worked out when read, stored nowhere** (docs/sync.md → Worked out when
/// read). Per register, in three steps (the reasoning is in
/// [`crate::duplicates`] → SQL fetches, Rust pairs):
///
///   1. **fetch** the records its rules compare and the child values they read
///      — one statement each, scoped to the books in `scope` and their
///      neighbours in time, on an index;
///   2. **pair** them in memory ([`find_pairs`]);
///   3. **drop** the pairs somebody judged distinct — one statement for the
///      register's pairs, seeking the acts by pair.
///
/// Then a bounded number of statements to name the records, however many
/// pairs there are. What is read grows with the current books, never with the
/// campaigns behind them — `duplicates.rs` in module-phytosanitary's tests
/// counts it.
///
/// Ordered: pairs written apart first, then the most recent operations, then by
/// register and id so two reads list them alike.
pub fn list_duplicates(
    conn: &Connection,
    policies: &[DuplicatePolicy],
    captions: &[RowCaption],
    scope: Scope,
) -> Result<DuplicateList> {
    let books = books_in(conn, scope)?;
    let mut suspected: BTreeSet<(&'static str, String, String)> = BTreeSet::new();
    let mut removed: BTreeSet<(&'static str, String, String)> = BTreeSet::new();
    for policy in policies {
        if policy.rules().is_empty() {
            continue;
        }
        let candidates = fetch_candidates(
            conn,
            policy,
            &candidates_sql(policy, scope),
            scope.parameter(),
            |season, _| books.contains(season),
        )?;
        let children = fetch_children(
            conn,
            policy,
            |overlap| overlap_sql(policy, overlap, scope),
            scope.parameter(),
        )?;
        suspected.extend(unjudged_pairs(conn, policy, &candidates, &children)?);
        let sql = both_removed_sql(policy.table, scope);
        for (first, second) in id_pairs(conn, &sql, scope.parameter())? {
            removed.insert((policy.table, first, second));
        }
    }
    named(conn, policies, captions, suspected, removed)
}

/// What a save left waiting: the pairs the records it wrote are in now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SavedDuplicates {
    /// The records the save wrote that a rule compares — the one saved, and
    /// whatever its change set wrote alongside it: a soil cover's save writes
    /// its maintenance as a mowing and a grazing, two registers with rules of
    /// their own. What a screen marks as "just saved".
    pub saved: Vec<String>,
    /// Ordered as the list orders them.
    pub suspects: Vec<SuspectedDuplicate>,
}

/// The pairs the last save of `record_id` put its records in — asked right
/// after a form saves (docs/sync.md → The same rule, right after the form saves).
///
/// **The save is not asked before it happens, and it is never refused.** The
/// form saves as it always did, and then this is read: the rule runs on the
/// record as stored, snapshots and trimming included, so nothing is derived a
/// second time for the check. A defect here cannot stop a treatment being
/// recorded, and what it finds is shown for a person to judge.
///
/// **Every register the save wrote is asked about**, read off the change set
/// that is the record's head now: a save may write several — a soil cover and
/// its maintenance — and a register a later module adds that saves the same
/// way is asked about with nothing added here.
///
/// Per register, the list's own steps over a record's fetch
/// ([`record_candidates_sql`]): its pairs are exactly the ones its book page
/// lists for it, minus the ones somebody judged distinct. An edit is asked
/// about the same way — a pair judged distinct stays answered.
pub fn list_saved_duplicates(
    conn: &Connection,
    policies: &[DuplicatePolicy],
    captions: &[RowCaption],
    table: &str,
    record_id: &str,
) -> Result<SavedDuplicates> {
    let heads = crate::merge::heads(conn, table, record_id)?;
    let Some(save) = crate::merge::live_head(&heads) else {
        return Ok(SavedDuplicates::default());
    };
    let written: Vec<(String, String)> = crate::sql::cached_statement(conn, WRITTEN_BY_SQL)?
        .query_map(params![save.device, save.seq], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut saved = Vec::new();
    let mut suspected: BTreeSet<(&'static str, String, String)> = BTreeSet::new();
    for (root_table, root_id) in written {
        let Some(policy) = policies
            .iter()
            .find(|policy| policy.table == root_table && !policy.rules().is_empty())
        else {
            continue;
        };
        if liveness(conn, policy.table, &root_id)? != Some(true) {
            continue;
        }
        let candidates = fetch_candidates(
            conn,
            policy,
            &record_candidates_sql(policy),
            &root_id,
            |_, id| id == root_id,
        )?;
        let children = fetch_children(
            conn,
            policy,
            |overlap| record_overlap_sql(policy, overlap),
            &root_id,
        )?;
        suspected.extend(unjudged_pairs(conn, policy, &candidates, &children)?);
        saved.push(root_id);
    }
    let list = named(conn, policies, captions, suspected, BTreeSet::new())?;
    Ok(SavedDuplicates {
        saved,
        suspects: list.suspects,
    })
}

/// The registers one change set wrote. A seek on the leading pair of
/// `record_change`'s UNIQUE index.
const WRITTEN_BY_SQL: &str = "SELECT DISTINCT root_table, root_id FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2";

/// Steps 2 and 3 for one register: pair the fetched records, and keep the
/// pairs nobody has judged distinct.
fn unjudged_pairs(
    conn: &Connection,
    policy: &DuplicatePolicy,
    candidates: &[Candidate],
    children: &ChildSets,
) -> Result<Vec<(&'static str, String, String)>> {
    let found = find_pairs(policy, candidates, children);
    let judged = judged_distinct(conn, policy.table, &found)?;
    Ok(found
        .into_iter()
        .filter(|pair| !judged.contains(pair))
        .map(|(first, second)| (policy.table, first, second))
        .collect())
}

/// The pairs, named and in the list's order: written apart first, then the
/// most recent operations, then by register and id so two reads list them
/// alike. A bounded number of statements however many pairs there are.
fn named(
    conn: &Connection,
    policies: &[DuplicatePolicy],
    captions: &[RowCaption],
    suspected: BTreeSet<(&'static str, String, String)>,
    removed: BTreeSet<(&'static str, String, String)>,
) -> Result<DuplicateList> {
    let mut names = RecordNames::default();
    for (table, first, second) in suspected.iter().chain(&removed) {
        names.want(table, first);
        names.want(table, second);
    }
    for (table, first, second) in &removed {
        names.want_removals(table, first, second);
    }
    names.resolve(conn, policies, captions)?;

    let mut suspects: Vec<SuspectedDuplicate> = suspected
        .into_iter()
        .map(|(register, first, second)| {
            let records = [
                names.record(register, &first),
                names.record(register, &second),
            ];
            let apart = written_apart(&records[0], &records[1]);
            SuspectedDuplicate {
                register,
                records,
                apart,
            }
        })
        .collect();
    suspects.sort_by(|a, b| {
        b.apart
            .cmp(&a.apart)
            .then_with(|| latest_day(&b.records).cmp(&latest_day(&a.records)))
            .then_with(|| a.register.cmp(b.register))
            .then_with(|| a.records[0].id.cmp(&b.records[0].id))
    });
    let both_removed = removed
        .into_iter()
        .map(|(register, first, second)| BothRemoved {
            register,
            records: [
                names.record(register, &first),
                names.record(register, &second),
            ],
        })
        .collect();
    Ok(DuplicateList {
        suspects,
        both_removed,
    })
}

/// The ids of the books `scope` is about — what marks a fetched record as one
/// the list is about rather than one fetched only to be compared with them.
fn books_in(conn: &Connection, scope: Scope) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(scope.seasons_sql())?;
    let books = stmt
        .query_map([scope.parameter()], |row| row.get(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(books)
}

/// Step 1: the records one register's rules compare, as rows of `sql` —
/// [`candidates_sql`] for a list, [`record_candidates_sql`] for one record,
/// whose shared column order this reads by position. `in_scope` says, from a
/// record's book and id, whether it is one the question is about rather than
/// one fetched only to be compared with them.
fn fetch_candidates(
    conn: &Connection,
    policy: &DuplicatePolicy,
    sql: &str,
    parameter: &str,
    in_scope: impl Fn(&str, &str) -> bool,
) -> Result<Vec<Candidate>> {
    let compared = policy.compared_columns().len();
    let mut stmt = conn.prepare(sql)?;
    let candidates = stmt
        .query_map([parameter], |row| {
            let id: String = row.get(0)?;
            let season_id: String = row.get(1)?;
            let mut values = Vec::with_capacity(compared);
            for at in 0..compared {
                values.push(Key::from(row.get::<_, rusqlite::types::Value>(5 + at)?));
            }
            Ok(Candidate {
                in_scope: in_scope(&season_id, &id),
                id,
                season_id,
                farm_id: row.get(2)?,
                first_day: row.get(3)?,
                last_day: row.get(4)?,
                values,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(candidates)
}

/// Step 1, the other half: the child values the register's rules compare, one
/// statement per child set — `sql_of` builds it ([`overlap_sql`] or
/// [`record_overlap_sql`]).
fn fetch_children(
    conn: &Connection,
    policy: &DuplicatePolicy,
    sql_of: impl Fn(&Overlap) -> String,
    parameter: &str,
) -> Result<ChildSets> {
    let mut children = ChildSets::default();
    for overlap in policy.overlaps() {
        let mut stmt = conn.prepare(&sql_of(&overlap))?;
        let mut rows = stmt.query([parameter])?;
        while let Some(row) = rows.next()? {
            let record: String = row.get(0)?;
            let value: rusqlite::types::Value = row.get(1)?;
            children.insert(overlap, record, Key::from(value));
        }
    }
    Ok(children)
}

/// Step 3: which of `pairs` somebody judged distinct. The acts are found by
/// each pair's first id, a seek on `idx_duplicate_verdict_pair` — one
/// statement per 500 pairs, never one per pair.
fn judged_distinct(
    conn: &Connection,
    table: &str,
    pairs: &BTreeSet<(String, String)>,
) -> Result<HashSet<(String, String)>> {
    let firsts = unique(pairs.iter().map(|(first, _)| first.clone()));
    let sql = format!(
        "SELECT first_id, second_id FROM duplicate_verdict
         WHERE first_id IN ({{ids}}) AND subject_table = '{table}' AND verdict = 'distinct'"
    );
    let judged = children_by_parent(
        conn,
        &sql,
        &firsts,
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        |(first, _)| first.clone(),
    )?;
    Ok(judged.into_values().flatten().collect())
}

/// Whether a book holds a pair removed twice over — two opposite *keep this
/// one* acts that left the operation in the book zero times.
///
/// Asked before merging the book into another or deleting it (docs/sync.md →
/// Merging two books, Deleting a book with its records): either removes the
/// book, and the list of duplicates reads live books only, so the pair would
/// leave it with nothing to say the operation is gone. The list's own query
/// for one book, and nothing else of the list — no pairing, no names.
pub fn book_has_both_removed(
    conn: &Connection,
    policies: &[DuplicatePolicy],
    season_id: &str,
) -> Result<bool> {
    let scope = Scope::Book { season_id };
    for policy in policies {
        if policy.rules().is_empty() {
            continue;
        }
        let sql = both_removed_sql(policy.table, scope);
        if !id_pairs(conn, &sql, scope.parameter())?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The id pairs a list query returns.
fn id_pairs(conn: &Connection, sql: &str, parameter: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(sql)?;
    let pairs = stmt
        .query_map([parameter], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(pairs)
}

/// Two records of one register side by side, for the person deciding whether
/// they are one operation recorded twice.
#[derive(Debug, Clone, Serialize)]
pub struct PairReview {
    /// The register — its table, from its policy.
    pub register: &'static str,
    /// Smaller id first, as a verdict names them, each named the way the list
    /// names it: its book, its day, who wrote it and where.
    pub records: [DuplicateRecord; 2],
    /// Every field either record states, the register's own first and then its
    /// children, with `values` in the order of `records` and the differing
    /// ones marked. A child row sits beside the other record's child of the
    /// same key — the treated plot, the herd — or alone when only one record
    /// has it.
    pub lines: Vec<ReviewLine>,
}

/// Two records of `policy`'s register, side by side.
///
/// Writes nothing: each side comes off its record's log, like a conflict's
/// versions, so looking changes nothing. It does not ask
/// whether the rule still pairs them — a record corrected since the list was
/// read may no longer match, and the person looking at the two is who decides
/// what they are. A record already removed is shown as it was, which is how a
/// pair removed twice over is looked at before one of it is restored.
pub fn review_pair(
    conn: &Connection,
    policy: &DuplicatePolicy,
    a: &str,
    b: &str,
    captions: &[RowCaption],
) -> Result<PairReview> {
    if a == b {
        return Err(CoreError::Invalid("duplicate_pair_one_record"));
    }
    let (first, second) = ordered_pair(a, b);
    let mut both_removed = true;
    for id in [first, second] {
        match liveness(conn, policy.table, id)? {
            None => return Err(CoreError::NotFound),
            Some(live) => both_removed &= !live,
        }
    }
    let lines = crate::merge::compare_records(conn, policy.table, [first, second], captions)?;
    let mut names = RecordNames::default();
    names.want(policy.table, first);
    names.want(policy.table, second);
    if both_removed {
        names.want_removals(policy.table, first, second);
    }
    names.resolve(conn, std::slice::from_ref(policy), captions)?;
    Ok(PairReview {
        register: policy.table,
        records: [
            names.record(policy.table, first),
            names.record(policy.table, second),
        ],
        lines,
    })
}

/// Two devices, or two known people. A profile the log does not name is not a
/// second person: most writes before profiles existed name none.
fn written_apart(a: &DuplicateRecord, b: &DuplicateRecord) -> bool {
    let devices = a.written_on != b.written_on;
    let people = matches!((&a.written_by, &b.written_by), (Some(x), Some(y)) if x != y);
    devices || people
}

/// The later of a pair's days — ISO text orders as the dates do.
fn latest_day(records: &[DuplicateRecord; 2]) -> Option<&str> {
    records.iter().filter_map(|r| r.day.as_deref()).max()
}

/// What a record's own row says, for naming it.
#[derive(Debug, Clone, Default)]
struct RowFacts {
    season_id: Option<String>,
    caption: Option<String>,
    day: Option<String>,
}

/// Who wrote a register's first change set.
#[derive(Debug, Clone)]
struct FirstWrite {
    device: String,
    actor: Option<String>,
    at: String,
}

/// Names for every record in the list, fetched a table at a time rather than a
/// record at a time — the rule for hydrating a list
/// (`crate::sql::children_by_parent`).
#[derive(Debug, Default)]
struct RecordNames {
    wanted: BTreeMap<&'static str, BTreeSet<String>>,
    /// Pairs removed twice over, per register, whose removals are named too.
    removal_pairs: BTreeMap<&'static str, BTreeSet<(String, String)>>,
    facts: HashMap<(&'static str, String), RowFacts>,
    first_writes: HashMap<(&'static str, String), FirstWrite>,
    /// Who removed each record of those pairs: the first write of the act that
    /// kept the other record.
    removals: HashMap<(&'static str, String), FirstWrite>,
    /// Each book's name and its farm's, by the book's id.
    seasons: HashMap<String, BookName>,
    devices: HashMap<String, String>,
    people: HashMap<String, String>,
}

impl RecordNames {
    fn want(&mut self, table: &'static str, id: &str) {
        self.wanted.entry(table).or_default().insert(id.to_owned());
    }

    /// Name who removed each record of a pair removed twice over, as well as
    /// who wrote it.
    fn want_removals(&mut self, table: &'static str, first: &str, second: &str) {
        self.removal_pairs
            .entry(table)
            .or_default()
            .insert((first.to_owned(), second.to_owned()));
    }

    fn resolve(
        &mut self,
        conn: &Connection,
        policies: &[DuplicatePolicy],
        captions: &[RowCaption],
    ) -> Result<()> {
        for (&table, ids) in &self.wanted {
            let ids: Vec<String> = ids.iter().cloned().collect();
            let caption = captions
                .iter()
                .find(|caption| caption.table == table)
                .map(|caption| caption.column);
            let day = policies
                .iter()
                .find(|policy| policy.table == table)
                .and_then(DuplicatePolicy::day_column);
            // A caption column may hold a number; the list shows text.
            let sql = format!(
                "SELECT id, season_id, CAST({} AS TEXT), {} FROM {table} WHERE id IN ({{ids}})",
                caption.unwrap_or("NULL"),
                day.unwrap_or("NULL"),
            );
            let rows = children_by_parent(
                conn,
                &sql,
                &ids,
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        RowFacts {
                            season_id: row.get(1)?,
                            caption: row.get(2)?,
                            day: row.get(3)?,
                        },
                    ))
                },
                |(id, _)| id.clone(),
            )?;
            for (id, mut found) in rows {
                if let Some((_, facts)) = found.pop() {
                    self.facts.insert((table, id), facts);
                }
            }
            for (id, first) in first_writes_of(conn, table, &ids)? {
                self.first_writes.insert((table, id), first);
            }
        }

        // The act that removed each record of a pair removed twice over is the
        // one that kept the other — found by the pair's first id, a seek on
        // `idx_duplicate_verdict_pair` — and who made it is that act's first
        // write. If more than one act kept the other record, the earliest
        // removed this one.
        for (&table, pairs) in &self.removal_pairs {
            let firsts = unique(pairs.iter().map(|(first, _)| first.clone()));
            let sql = format!(
                "SELECT first_id, second_id, kept_id, id FROM duplicate_verdict
                 WHERE first_id IN ({{ids}}) AND subject_table = '{table}' AND verdict = 'duplicate'
                 ORDER BY id"
            );
            let acts = children_by_parent(
                conn,
                &sql,
                &firsts,
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
                |(first, ..)| first.clone(),
            )?;
            let mut removed_by_act: HashMap<String, String> = HashMap::new();
            for (first, second, kept, act) in acts.into_values().flatten() {
                if !pairs.contains(&(first.clone(), second.clone())) {
                    continue;
                }
                let removed = if kept.as_deref() == Some(first.as_str()) {
                    second
                } else {
                    first
                };
                removed_by_act.entry(removed).or_insert(act);
            }
            let act_ids = unique(removed_by_act.values().cloned());
            let writes = first_writes_of(conn, "duplicate_verdict", &act_ids)?;
            for (record, act) in removed_by_act {
                if let Some(write) = writes.get(&act) {
                    self.removals.insert((table, record), write.clone());
                }
            }
        }

        let seasons: Vec<String> = unique(self.facts.values().filter_map(|f| f.season_id.clone()));
        self.seasons = super::season::book_names(conn, &seasons)?;
        let writes = || self.first_writes.values().chain(self.removals.values());
        let devices: Vec<String> = unique(writes().map(|w| w.device.clone()));
        self.devices = pairs_by_id(
            conn,
            "SELECT id, label FROM sync_peer WHERE id IN ({ids}) AND label IS NOT NULL",
            &devices,
        )?;
        let people: Vec<String> = unique(writes().filter_map(|w| w.actor.clone()));
        self.people = pairs_by_id(
            conn,
            "SELECT id, display_name FROM user_profile WHERE id IN ({ids})",
            &people,
        )?;
        Ok(())
    }

    fn record(&self, table: &'static str, id: &str) -> DuplicateRecord {
        let key = (table, id.to_owned());
        let facts = self.facts.get(&key).cloned().unwrap_or_default();
        let first = self.first_writes.get(&key);
        let removal = self.removals.get(&key);
        let book = facts
            .season_id
            .as_ref()
            .and_then(|season| self.seasons.get(season));
        DuplicateRecord {
            id: id.to_owned(),
            season_label: book.map(|book| book.label.clone()),
            farm_name: book.map(|book| book.farm.clone()),
            season_id: facts.season_id,
            caption: facts.caption,
            day: facts.day,
            written_on: first.map(|w| w.device.clone()),
            device_label: first.and_then(|w| self.devices.get(&w.device).cloned()),
            written_by: first.and_then(|w| w.actor.clone()),
            author_name: first
                .and_then(|w| w.actor.as_ref())
                .and_then(|actor| self.people.get(actor).cloned()),
            written_at: first.map(|w| w.at.clone()),
            removed_on: removal.map(|w| w.device.clone()),
            removed_device_label: removal.and_then(|w| self.devices.get(&w.device).cloned()),
            removed_by: removal.and_then(|w| w.actor.clone()),
            remover_name: removal
                .and_then(|w| w.actor.as_ref())
                .and_then(|actor| self.people.get(actor).cloned()),
            removed_at: removal.map(|w| w.at.clone()),
        }
    }
}

/// Who wrote each of `ids`' first change set, in register `table` — its whole
/// history read oldest first, bounded by how often THESE registers were
/// edited, on `idx_record_change_root`.
fn first_writes_of(
    conn: &Connection,
    table: &str,
    ids: &[String],
) -> Result<HashMap<String, FirstWrite>> {
    let sql = format!(
        "SELECT root_id, origin_device, actor, changed_at FROM record_change
         WHERE root_table = '{table}' AND root_id IN ({{ids}})
         ORDER BY root_id, hlc, id"
    );
    let writes = children_by_parent(
        conn,
        &sql,
        ids,
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                FirstWrite {
                    device: row.get(1)?,
                    actor: row.get(2)?,
                    at: row.get(3)?,
                },
            ))
        },
        |(id, _)| id.clone(),
    )?;
    Ok(writes
        .into_iter()
        .filter_map(|(id, history)| history.into_iter().next().map(|(_, first)| (id, first)))
        .collect())
}

fn unique(values: impl Iterator<Item = String>) -> Vec<String> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

/// `id → name` for a two-column query over `ids`.
fn pairs_by_id(conn: &Connection, sql: &str, ids: &[String]) -> Result<HashMap<String, String>> {
    let grouped = children_by_parent(
        conn,
        sql,
        ids,
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        |(id, _)| id.clone(),
    )?;
    Ok(grouped
        .into_iter()
        .filter_map(|(id, mut rows)| rows.pop().map(|(_, name)| (id, name)))
        .collect())
}

// ---------------------------------------------------------------------------
// What a person says
// ---------------------------------------------------------------------------

/// Whether a record of `table` is live: `Some(true)` live, `Some(false)`
/// removed, `None` not there at all.
fn liveness(conn: &Connection, table: &str, id: &str) -> Result<Option<bool>> {
    Ok(conn
        .query_row(
            &format!("SELECT deleted_at IS NULL FROM {table} WHERE id = ?1"),
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Record that two records are both real: two operations, not one recorded
/// twice. The pair is never listed again, on any device once they have synced.
///
/// **An act that changes nothing is not written** — a pair already judged
/// distinct, a double tap. Two devices judging before they sync still write
/// two rows, which is harmless: either one hides the pair.
pub fn mark_distinct(
    conn: &mut Connection,
    policy: &DuplicatePolicy,
    a: &str,
    b: &str,
    actor: Option<&str>,
) -> Result<()> {
    if a == b {
        return Err(CoreError::Invalid("duplicate_pair_one_record"));
    }
    let (first, second) = ordered_pair(a, b);
    let tx = begin(conn, actor)?;
    for id in [first, second] {
        if liveness(&tx, policy.table, id)?.is_none() {
            return Err(CoreError::NotFound);
        }
    }
    let judged: bool = tx.query_row(
        DISTINCT_HELD_SQL,
        params![first, second, policy.table],
        |row| row.get(0),
    )?;
    if judged {
        return Ok(());
    }
    write_verdict(&tx, policy.table, first, second, Verdict::Distinct, None)?;
    tx.commit()?;
    Ok(())
}

/// Record that two records are one operation: keep `kept`, remove `removed`.
///
/// **The removal is the register's own delete**, passed in as `remove`,
/// because a register's delete may do more than stamp `deleted_at` — removing
/// a soil cover withdraws its maintenance lines in two other registers — and
/// only the crate that owns it knows. Core cannot name that function, so the
/// caller (the shell, which sees every crate) hands it over.
///
/// **One change set holds the removal and the act**, so the audit trail reads
/// what was removed and why under one change-set key, and a failure anywhere
/// leaves neither. No register carries a reason column: the act is the reason
/// (docs/sync.md → The reason is the verdict).
///
/// Generic over the caller's error type `E`: `remove` returns whatever its
/// crate's error is, and every such error converts from a [`CoreError`] (the
/// `E: From<CoreError>` bound), so the `?`s on core's own steps below turn into
/// `E` on the way out.
pub fn keep_duplicate<E>(
    conn: &mut Connection,
    policy: &DuplicatePolicy,
    kept: &str,
    removed: &str,
    actor: Option<&str>,
    remove: impl FnOnce(&WriteTx, &str) -> std::result::Result<(), E>,
) -> std::result::Result<(), E>
where
    E: From<CoreError>,
{
    if kept == removed {
        return Err(CoreError::Invalid("duplicate_pair_one_record").into());
    }
    let tx = begin(conn, actor)?;
    match liveness(&tx, policy.table, kept)? {
        None => return Err(CoreError::NotFound.into()),
        Some(false) => return Err(CoreError::Invalid("duplicate_kept_gone").into()),
        Some(true) => {}
    }
    remove(&tx, removed)?;
    // The remover is the caller's, paired with this policy by hand: a remover
    // for another register would leave `removed` untouched here and delete
    // something else, which must not be committed under this act.
    if liveness(&tx, policy.table, removed)? != Some(false) {
        return Err(CoreError::ShapeViolation(format!(
            "the removal for {} did not remove {removed}",
            policy.table
        ))
        .into());
    }
    let (first, second) = ordered_pair(kept, removed);
    write_verdict(
        &tx,
        policy.table,
        first,
        second,
        Verdict::Duplicate,
        Some(kept),
    )?;
    tx.commit().map_err(CoreError::from)?;
    Ok(())
}

/// Write one act, as a register of its own. No season: an act is about a pair,
/// and a pair may span two books.
fn write_verdict(
    tx: &WriteTx,
    table: &str,
    first: &str,
    second: &str,
    verdict: Verdict,
    kept: Option<&str>,
) -> Result<()> {
    let act = DuplicateVerdict {
        id: Uuid::now_v7().to_string(),
        subject_table: table.to_owned(),
        first_id: first.to_owned(),
        second_id: second.to_owned(),
        verdict,
        kept_id: kept.map(str::to_owned),
        created_at: now_utc_iso(),
    };
    tx.execute(
        INSERT_VERDICT_SQL,
        params![
            act.id,
            act.subject_table,
            act.first_id,
            act.second_id,
            act.verdict.code(),
            act.kept_id,
            act.created_at,
        ],
    )?;
    let stamp = tx.register("duplicate_verdict", &act.id, None)?;
    log_insert(tx, &stamp, "duplicate_verdict", &act.id, &act)?;
    Ok(())
}

/// Bring back a record that was removed as a duplicate of a record that was
/// ALSO removed — two people, two devices, opposite verdicts, and the operation
/// in the book zero times (docs/sync.md → Two people removing opposite copies).
///
/// **It undoes the change set that removed the record**, register by register,
/// rather than clearing one `deleted_at`: that change set is the register's own
/// delete plus the act, and a delete may have withdrawn rows in other registers
/// too — a soil cover's maintenance lines. In each register that change set
/// wrote, what it wrote is put back as it was before (`super::undo`), as one
/// audited change set, and `settle` brings the tables in line, as
/// `merge::resolve` does. The act stays: it is history.
///
/// Refused as `duplicate_restore_refused` unless all of this holds, because
/// anything else is either not this situation or no longer it:
///
///   * the record is removed, and so is the record it was paired with;
///   * a `duplicate` act kept each of them — the two opposite verdicts;
///   * the change set this record's register is at is the removal, holding the
///     act that kept its pair-mate;
///   * nothing has written to any register that change set touched since. A
///     restore would otherwise overwrite that later write.
pub fn restore_removed_duplicate(
    conn: &mut Connection,
    policy: &DuplicatePolicy,
    record_id: &str,
    actor: Option<&str>,
) -> Result<()> {
    let refused = || CoreError::Invalid("duplicate_restore_refused");
    let table = policy.table;
    match liveness(conn, table, record_id)? {
        None => return Err(CoreError::NotFound),
        Some(true) => return Err(refused()),
        Some(false) => {}
    }

    // The pair-mate: removed too, and kept by an act of its own.
    let kept_this = conn
        .prepare(KEPT_BY_SQL)?
        .query_map(params![record_id, table], map_verdict)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut mate = None;
    for act in &kept_this {
        let other = if act.first_id == record_id {
            &act.second_id
        } else {
            &act.first_id
        };
        let other_kept: bool = conn.query_row(
            PAIR_KEPT_SQL,
            params![act.first_id, act.second_id, other],
            |row| row.get(0),
        )?;
        if other_kept && liveness(conn, table, other)? == Some(false) {
            mate = Some((act.first_id.clone(), act.second_id.clone(), other.clone()));
            break;
        }
    }
    let Some((first, second, other)) = mate else {
        return Err(refused());
    };

    // The change set the record's register is at, and what it wrote.
    let heads = crate::merge::heads(conn, table, record_id)?;
    let [removal] = heads.as_slice() else {
        return Err(refused());
    };
    let rows = change_set_rows(conn, &removal.device, removal.seq)?;
    let removed_this = rows.iter().any(|row| {
        row.entity_table == "duplicate_verdict"
            && row.payload["after"]["first_id"] == first.as_str()
            && row.payload["after"]["second_id"] == second.as_str()
            && row.payload["after"]["kept_id"] == other.as_str()
    });
    if !removed_this {
        return Err(refused());
    }
    // Each register the removal wrote, and the book it was filed in there.
    let registers: BTreeMap<(String, String), Option<String>> = rows
        .iter()
        .filter(|row| row.entity_table != "duplicate_verdict")
        .map(|row| {
            (
                (row.root_table.clone(), row.root_id.clone()),
                row.season_id.clone(),
            )
        })
        .collect();
    for (root_table, root_id) in registers.keys() {
        if crate::merge::heads(conn, root_table, root_id)?.as_slice()
            != std::slice::from_ref(removal)
        {
            return Err(refused());
        }
    }

    let tx = begin(conn, actor)?;
    for ((root_table, root_id), season) in &registers {
        super::undo::undo(&tx, root_table, root_id, season.as_deref(), removal)?;
    }
    tx.commit()?;
    Ok(())
}

/// One logged row of a change set, as a restore reads it.
struct LoggedRow {
    entity_table: String,
    season_id: Option<String>,
    root_table: String,
    root_id: String,
    payload: Value,
}

fn change_set_rows(conn: &Connection, device: &str, seq: i64) -> Result<Vec<LoggedRow>> {
    let mut stmt = conn.prepare(CHANGE_SET_ROWS_SQL)?;
    let mut rows = stmt.query(params![device, seq])?;
    let mut logged = Vec::new();
    while let Some(row) = rows.next()? {
        let payload: String = row.get("payload")?;
        logged.push(LoggedRow {
            entity_table: row.get("entity_table")?,
            season_id: row.get("season_id")?,
            root_table: row.get("root_table")?,
            root_id: row.get("root_id")?,
            payload: serde_json::from_str(&payload)?,
        });
    }
    Ok(logged)
}
