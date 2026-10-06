// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Duplicate suspects: the rules every crate that owns a register of the book
//! declares, how the records they compare are fetched, and the comparison
//! itself. No database here beyond building SQL —
//! [`crate::repository::list_duplicates`] runs it.
//!
//! Two workers each recording "applied product X on plot Y yesterday" write two
//! valid rows with two UUIDs, and no sync algorithm can see a conflict in that:
//! from the data's point of view there is none. So each register declares a
//! **rule** saying when two of its live records, on one farm, look like one
//! operation recorded twice, and a person decides. A rule raises a suspicion;
//! it never identifies anything, and nothing is ever dropped by a machine
//! (docs/sync.md → Duplicate suspects).
//!
//! **Nothing stores a suspicion.** The list is worked out whenever it is read,
//! like the alerts, so an edit, a deletion, an import or a verdict from another
//! device is reflected the next time anybody looks. What is stored is what a
//! person said — `duplicate_verdict`, one insert-only row per act.
//!
//! **Core knows no register but its own.** A crate declares a
//! [`DuplicatePolicy`] per register as a constant, and the shell lists them;
//! core never names another crate's table. Core's own are the three registers
//! it owns: `crop`, `sowing_record` and `harvest_record`.
//!
//! # SQL fetches, Rust pairs — and why
//!
//! Finding the pairs is two steps, and the split is deliberate:
//!
//!   1. **SQL decides which records are read.** [`candidates_sql`] and
//!      [`overlap_sql`] fetch the register's records in scope — plus, for a
//!      dated rule, the farm's other records close enough in time to pair with
//!      them — and the child values the rules compare. Each is one statement
//!      per register, scoped by a `WHERE` on the books and read through an
//!      index; the shell's contract test holds both under `EXPLAIN QUERY PLAN`.
//!   2. **Rust decides which of them pair.** [`find_pairs`] groups the fetched
//!      records by what must be equal, sorts each group by day, and walks it
//!      once, comparing each record only with the ones inside its window.
//!
//! The first version asked SQLite for the pairs directly, as a self-join with
//! the plot overlap as a correlated `EXISTS`, and was measured before it was
//! kept (2026-09-24): SQLite spends about a microsecond on every record inside
//! another's window, most of it running the `EXISTS`. On a farm recording
//! 4 000 treatments a campaign with one product — every spray inside a day of
//! another is a candidate, about 800 000 comparisons for the current books —
//! the Status view took 1.1 s. In memory those comparisons take about 7 ms,
//! because each record's plots are looked up once and then compared as two
//! short sorted lists of numbers; the whole list, fetching and naming
//! included, took 99 ms on that farm and 44 ms on the same farm with ordinary
//! data (docs/sync.md → What it costs).
//!
//! This is not filtering in Rust in the sense the house rule forbids: nothing
//! is read to be thrown away. SQL still decides what is read — the current
//! books and their neighbours, never the history — and Rust does the part SQL
//! can only do one candidate at a time.

use std::collections::{BTreeSet, HashMap};

use rusqlite::types::Value;
use serde::Serialize;

/// What one register declares: its rules, or that it has none and why.
///
/// Every register of the book — a table carrying a `season_id` — must declare
/// one, and the shell's contract test refuses one that did not: a new crate's
/// register cannot be forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicatePolicy {
    /// The register's table. Also what a verdict about two of its records is
    /// filed under, so it comes from here and never from a screen.
    pub table: &'static str,
    pub detection: Detection,
}

/// Whether a register's records are compared at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detection {
    /// Two records matching ANY of these rules are a suspected pair.
    Rules(&'static [DuplicateRule]),
    /// The register is never compared, for the reason given — a register whose
    /// records cannot describe one operation twice. Stated rather than left
    /// out, so the contract test can tell "decided" from "forgotten".
    Never { because: &'static str },
}

/// When two records of one register look like one operation recorded twice.
/// Every test must hold, and both records must be live and on one farm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateRule {
    pub when: When,
    /// Columns that must hold the same value on both records.
    pub same: &'static [Same],
    /// Child sets that must share at least one value — the plots treated, a
    /// grazing's herds.
    pub overlaps: &'static [Overlap],
}

/// How two records' dates must relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// In the same book, whatever their dates — for a register with no date
    /// worth comparing, or one done once per campaign.
    SameBook,
    /// Their periods come within `days` of each other: they overlap, or the gap
    /// between the end of one and the start of the other is at most `days`.
    /// `days` is the slip a rule forgives: entry is batchy and late, and
    /// whoever types the operation the next evening types the wrong day.
    ///
    /// Every dated rule of one register compares the same period; the shell's
    /// contract test holds that, and [`DuplicatePolicy::period`] relies on it.
    Within { period: Period, days: u8 },
}

/// Where a record keeps its dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// One day per record.
    Day { on: &'static str },
    /// A first day and an optional last one; an unset last day means the
    /// operation took one day — a treatment, an application.
    Days {
        from: &'static str,
        to: &'static str,
    },
    /// A first day and an optional last one; an unset last day means it is
    /// still going on — a grazing whose flock has not left.
    Ongoing {
        from: &'static str,
        to: &'static str,
    },
}

impl Period {
    /// The column holding the first day — what the index every dated register
    /// carries for its rule leads with, after the farm.
    pub const fn first_day(self) -> &'static str {
        match self {
            Period::Day { on } => on,
            Period::Days { from, .. } | Period::Ongoing { from, .. } => from,
        }
    }

    /// The column holding the last day, for a period that has one.
    pub const fn last_day(self) -> Option<&'static str> {
        match self {
            Period::Day { .. } => None,
            Period::Days { to, .. } | Period::Ongoing { to, .. } => Some(to),
        }
    }

    /// SQL for the last day of `alias`'s period as stored: the first day for a
    /// one-day operation, and NULL for a period still going on.
    fn last_day_sql(self, alias: &str) -> String {
        match self {
            Period::Day { on } => format!("{alias}.{on}"),
            Period::Days { from, to } => format!("coalesce({alias}.{to}, {alias}.{from})"),
            Period::Ongoing { to, .. } => format!("{alias}.{to}"),
        }
    }
}

/// One column two records must agree on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    /// Equal, two empty values counting as equal: two non-chemical treatments
    /// both name no product, and that is agreement.
    Value(&'static str),
    /// Equal AND stated: two empty values are not a match. Two harvests with no
    /// delivery note are not thereby one load.
    Stated(&'static str),
}

impl Same {
    pub const fn column(self) -> &'static str {
        match self {
            Same::Value(column) | Same::Stated(column) => column,
        }
    }

    const fn stated(self) -> bool {
        matches!(self, Same::Stated(_))
    }
}

/// A child set two records must share a value in: rows of `table` whose
/// `parent` column names each record, compared on `column`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Overlap {
    pub table: &'static str,
    pub parent: &'static str,
    pub column: &'static str,
}

impl DuplicatePolicy {
    /// The rules, or none for a register declared [`Detection::Never`].
    pub fn rules(&self) -> &'static [DuplicateRule] {
        match self.detection {
            Detection::Rules(rules) => rules,
            Detection::Never { .. } => &[],
        }
    }

    /// The period the register's dated rules compare, and the widest slack any
    /// of them forgives — what decides how far around the records in scope the
    /// fetch reaches. `None` for a register compared by book alone.
    pub fn period(&self) -> Option<(Period, u8)> {
        let mut found: Option<(Period, u8)> = None;
        for rule in self.rules() {
            if let When::Within { period, days } = rule.when {
                found = Some(match found {
                    None => (period, days),
                    Some((first, widest)) => (first, widest.max(days)),
                });
            }
        }
        found
    }

    /// The column holding the day the operation began, for the registers whose
    /// rules compare one — what the list orders pairs by. `None` for a register
    /// compared by book alone.
    pub fn day_column(&self) -> Option<&'static str> {
        self.period().map(|(period, _)| period.first_day())
    }

    /// Every column any rule compares, once each, in the order the rules name
    /// them — the order of [`Candidate::values`].
    pub fn compared_columns(&self) -> Vec<&'static str> {
        let mut columns: Vec<&'static str> = Vec::new();
        for same in self.rules().iter().flat_map(|rule| rule.same) {
            if !columns.contains(&same.column()) {
                columns.push(same.column());
            }
        }
        columns
    }

    /// Every child set any rule compares, once each.
    pub fn overlaps(&self) -> Vec<Overlap> {
        let mut overlaps: Vec<Overlap> = Vec::new();
        for overlap in self.rules().iter().flat_map(|rule| rule.overlaps) {
            if !overlaps.contains(overlap) {
                overlaps.push(*overlap);
            }
        }
        overlaps
    }
}

/// Which records a list is about. The other record of a pair may be in any
/// book of the farm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    /// What the Status view asks: pairs with a record in a book whose campaign
    /// ended less than a year before `today` (`YYYY-MM-DD`), or has not ended —
    /// the campaign under way and the one before, which late entries and
    /// delayed imports still land in. A date rather than `season.status`,
    /// because nothing in the app archives a book.
    Current { today: &'a str },
    /// What a book's own page asks: pairs with a record in this book, however
    /// old it is.
    Book { season_id: &'a str },
}

impl<'a> Scope<'a> {
    /// The books in scope, as a subquery bound to `?1`. For the Status view a
    /// seek on `idx_season_ends_active`: the current books, and never the
    /// campaigns behind them.
    pub fn seasons_sql(self) -> &'static str {
        match self {
            Scope::Current { .. } => {
                "SELECT id FROM season WHERE ends_on >= date(?1, '-1 year') AND deleted_at IS NULL"
            }
            Scope::Book { .. } => "SELECT ?1",
        }
    }

    /// The value bound to `?1`.
    pub fn parameter(self) -> &'a str {
        match self {
            Scope::Current { today } => today,
            Scope::Book { season_id } => season_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Step 1: what SQL fetches
// ---------------------------------------------------------------------------

/// The query fetching the records one register's rules compare in `scope`.
///
/// Columns, in order: `id`, `season_id`, `farm_id`, the first and last day as
/// day numbers, then each of [`DuplicatePolicy::compared_columns`]. For a
/// register compared by book alone the farm and the days are NULL.
///
/// **What it reaches, and no further** (docs/sync.md → Which records are
/// compared):
///
///   * for a register compared by book alone, the live records of the books in
///     scope — a pair is always within one book;
///   * for a dated register, every live record of each farm in scope whose
///     first day falls inside that farm's span: from the earliest first day of
///     its records in scope to the latest last day, widened by the rule's
///     slack. That span is the current campaigns, give or take a day, however
///     many campaigns lie behind them — read on the `(farm_id, <first day>)`
///     index. It also brings in the records of another book that fall inside
///     it, which is how two books kept for one campaign on two devices are
///     compared.
///
/// Days come back as SQLite's own day numbers (`julianday`, whole), so the
/// comparison in Rust is integer arithmetic and never parses a date. A date
/// the column holds in any other shape comes back NULL, and that record is
/// left out of the dated rules rather than guessed at.
pub fn candidates_sql(policy: &DuplicatePolicy, scope: Scope) -> String {
    candidates_query(policy, scope, &candidate_columns(policy), "")
}

/// What a candidate row holds, in [`Candidate`]'s order — shared by the list's
/// fetch and a record's, so the two cannot read a record differently.
fn candidate_columns(policy: &DuplicatePolicy) -> String {
    let (farm, first, last) = match policy.period() {
        Some((period, _)) => (
            "y.farm_id".to_owned(),
            format!("CAST(julianday(y.{}) AS INTEGER)", period.first_day()),
            format!("CAST(julianday({}) AS INTEGER)", period.last_day_sql("y")),
        ),
        None => ("NULL".to_owned(), "NULL".to_owned(), "NULL".to_owned()),
    };
    let mut select = vec![
        "y.id".to_owned(),
        "y.season_id".to_owned(),
        farm,
        first,
        last,
    ];
    select.extend(
        policy
            .compared_columns()
            .into_iter()
            .map(|column| format!("y.{column}")),
    );
    select.join(", ")
}

/// The query fetching one child set's values for the same records
/// [`candidates_sql`] fetches: `(parent id, value)` rows. The child's
/// `UNIQUE (parent, column, …)` makes it a seek per record.
pub fn overlap_sql(policy: &DuplicatePolicy, overlap: &Overlap, scope: Scope) -> String {
    let Overlap {
        table,
        parent,
        column,
    } = *overlap;
    candidates_query(
        policy,
        scope,
        &format!("c.{parent}, c.{column}"),
        &format!("JOIN {table} c ON c.{parent} = y.id"),
    )
}

/// The FROM and WHERE both fetches share, so they cannot disagree about which
/// records are compared. `select` names what to return, and `join` adds a
/// child table.
///
/// Built per table, so it is prepared plainly and never through the shared
/// statement cache (`crate::sql::cached_statement`).
fn candidates_query(policy: &DuplicatePolicy, scope: Scope, select: &str, join: &str) -> String {
    let table = policy.table;
    let seasons = scope.seasons_sql();
    match policy.period() {
        None => format!(
            "SELECT {select} FROM {table} y {join}
             WHERE y.deleted_at IS NULL AND y.season_id IN ({seasons})"
        ),
        Some((period, days)) => {
            let first = period.first_day();
            let last = period.last_day_sql("x");
            // `span` holds one row per farm in scope. A period still going on
            // has no last day, and reaches every day after it began — which the
            // latest date the ISO form can write stands for; `date()` of it
            // plus a day is NULL, hence the second `coalesce`.
            format!(
                "WITH span AS (
                     SELECT x.farm_id AS farm_id,
                            MIN(x.{first}) AS lo,
                            MAX(coalesce({last}, '9999-12-31')) AS hi
                     FROM {table} x
                     WHERE x.deleted_at IS NULL AND x.season_id IN ({seasons})
                     GROUP BY x.farm_id)
                 SELECT {select} FROM span
                 JOIN {table} y
                   ON y.farm_id = span.farm_id
                  AND y.{first} BETWEEN date(span.lo, '-{days} days')
                                    AND coalesce(date(span.hi, '+{days} days'), '9999-12-31')
                 {join}
                 WHERE y.deleted_at IS NULL"
            )
        }
    }
}

/// The query listing the pairs of one register that were **both removed, by
/// opposite verdicts** — *keep A* on one device and *keep B* on another,
/// offline, which leaves the operation in the book zero times (docs/sync.md →
/// Two people removing opposite copies). `(first_id, second_id)` rows, found
/// from a removed record in `scope` that somebody's verdict had kept — a seek
/// on `idx_duplicate_verdict_kept` per removed record, and on the pair after.
pub fn both_removed_sql(table: &str, scope: Scope) -> String {
    format!(
        "SELECT v1.first_id, v1.second_id FROM {table} x
         JOIN duplicate_verdict v1
           ON v1.kept_id = x.id AND v1.verdict = 'duplicate' AND v1.subject_table = '{table}'
         JOIN duplicate_verdict v2
           ON v2.first_id = v1.first_id AND v2.second_id = v1.second_id
          AND v2.verdict = 'duplicate' AND v2.kept_id <> v1.kept_id
         JOIN {table} y ON y.id = v2.kept_id
         WHERE x.deleted_at IS NOT NULL AND y.deleted_at IS NOT NULL
           AND x.season_id IN ({})",
        scope.seasons_sql()
    )
}

/// The query fetching the records one register's rules could pair with ONE
/// record, bound as `?1` — what is asked right after a save, where the list
/// asks about books. Columns as [`candidates_sql`].
///
/// **Exactly the records the record's book page would compare it with, and
/// only those.** The book page fetches every record of the farm whose first
/// day falls in the book's span, and pairs the ones inside each other's
/// window. Of those, the ones that can pair with this record are:
///
///   * for a one-day register, those within the rule's slack of its day — the
///     farm's `(farm_id, <first day>)` index read over a few days;
///   * for a register of periods, those starting between the book's earliest
///     start and this record's end, and still going on when it began. A long
///     period that began weeks earlier — an irrigation for the whole of May —
///     is reached; what is read to find it is the book's first days on the
///     farm index, and what comes back is the handful that overlap;
///   * for a rule of the book, the book's records holding the values the rule
///     needs equal — read on the book index, the equality in SQL.
///
/// The branches are joined with `UNION`, so a record two rules reach comes
/// back once. `find_pairs` over the result, with only this record in scope,
/// finds the pairs its book page lists for it; each crate's tests hold the two
/// to that.
///
/// Built per table, so it is prepared plainly and never through the shared
/// statement cache (`crate::sql::cached_statement`).
pub fn record_candidates_sql(policy: &DuplicatePolicy) -> String {
    record_query(policy, &candidate_columns(policy), "")
}

/// One child set's values for the records [`record_candidates_sql`] fetches:
/// `(parent id, value)` rows, as [`overlap_sql`] returns them for a list.
pub fn record_overlap_sql(policy: &DuplicatePolicy, overlap: &Overlap) -> String {
    let Overlap {
        table,
        parent,
        column,
    } = *overlap;
    record_query(
        policy,
        &format!("c.{parent}, c.{column}"),
        &format!("JOIN {table} c ON c.{parent} = y.id"),
    )
}

/// The FROM and WHERE both of a record's fetches share. `r0` is the record
/// itself; each branch returns `select` over the records `y` it reaches.
fn record_query(policy: &DuplicatePolicy, select: &str, join: &str) -> String {
    let table = policy.table;
    let mut branches: Vec<String> = Vec::new();

    if let Some((period, days)) = policy.period() {
        let first = period.first_day();
        let last_r0 = period.last_day_sql("r0");
        let last_y = period.last_day_sql("y");
        // How far back the fetch starts. A one-day record meets only records
        // within the slack of its own day. A record of periods also meets any
        // that began earlier and is still going on — which the book page finds
        // from the book's earliest start, and so does this.
        let (from, still_going) = match period {
            Period::Day { .. } => ("r.first".to_owned(), String::new()),
            Period::Days { .. } | Period::Ongoing { .. } => (
                format!(
                    "coalesce((SELECT MIN(x.{first}) FROM {table} x
                               WHERE x.season_id = r.season_id AND x.deleted_at IS NULL), r.first)"
                ),
                format!(" AND coalesce({last_y}, '9999-12-31') >= date(r.first, '-{days} days')"),
            ),
        };
        branches.push(format!(
            "SELECT {select} FROM r
             JOIN {table} y
               ON y.farm_id = r.farm_id
              AND y.{first} BETWEEN date({from}, '-{days} days')
                                AND coalesce(date(r.last, '+{days} days'), '9999-12-31')
             {join}
             WHERE y.deleted_at IS NULL{still_going}"
        ));
        let with = format!(
            "WITH r AS (SELECT r0.farm_id AS farm_id, r0.season_id AS season_id,
                               r0.{first} AS first, {last_r0} AS last
                        FROM {table} r0 WHERE r0.id = ?1)"
        );
        // Every book rule too, each a branch of its own.
        branches.extend(book_branches(policy, select, join));
        return format!("{with} {}", branches.join(" UNION "));
    }

    branches.extend(book_branches(policy, select, join));
    branches.join(" UNION ")
}

/// A branch per rule of the book: the record's book, narrowed to the records
/// holding what that rule needs equal. A value the rule needs stated and the
/// record leaves empty matches nothing, so its branch returns nothing.
fn book_branches(policy: &DuplicatePolicy, select: &str, join: &str) -> Vec<String> {
    let table = policy.table;
    policy
        .rules()
        .iter()
        .filter(|rule| rule.when == When::SameBook)
        .map(|rule| {
            let equal: String = rule
                .same
                .iter()
                .map(|same| {
                    let column = same.column();
                    if same.stated() {
                        format!(" AND r0.{column} IS NOT NULL AND y.{column} = r0.{column}")
                    } else {
                        format!(" AND y.{column} IS r0.{column}")
                    }
                })
                .collect();
            format!(
                "SELECT {select} FROM {table} r0
                 JOIN {table} y ON y.season_id = r0.season_id
                 {join}
                 WHERE r0.id = ?1 AND y.deleted_at IS NULL{equal}"
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Step 2: what Rust compares
// ---------------------------------------------------------------------------

/// One stored value as the comparison holds it: SQLite's value, made something
/// a `HashMap` can key on (a float cannot be hashed as it is, so it is kept as
/// its bits).
///
/// Two values are equal exactly when SQL's `IS` would call them equal for one
/// column: every column has one declared type, so SQLite stores its values in
/// one representation, and two NULLs are equal — which is what
/// [`Same::Value`] means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Null,
    Integer(i64),
    Real(u64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<Value> for Key {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Key::Null,
            Value::Integer(number) => Key::Integer(number),
            // -0.0 and 0.0 are one number to a person and two bit patterns to a
            // hash; adding 0.0 turns the first into the second.
            Value::Real(number) => Key::Real((number + 0.0).to_bits()),
            Value::Text(text) => Key::Text(text),
            Value::Blob(bytes) => Key::Blob(bytes),
        }
    }
}

/// One record as [`find_pairs`] compares it — a row of [`candidates_sql`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub season_id: String,
    /// `None` for a register compared by book alone.
    pub farm_id: Option<String>,
    /// Day numbers: whole days, so a later day is a bigger number. `None` for
    /// a register compared by book alone, or a date that could not be read.
    pub first_day: Option<i64>,
    /// The last day; `None` for a period still going on (or unreadable).
    pub last_day: Option<i64>,
    /// The values of [`DuplicatePolicy::compared_columns`], in that order.
    pub values: Vec<Key>,
    /// Whether it is in one of the books the list is about. A pair is listed
    /// when at least one of its records is; the others were fetched only to be
    /// compared with them.
    pub in_scope: bool,
}

/// The values each record holds in each child set a rule compares.
///
/// **Numbers, not ids.** Each distinct value (a plot's id, a herd's REGA code)
/// is given a small number the first time it is seen, and each record keeps its
/// numbers sorted. "Do these two share a plot?" is then two short sorted lists
/// walked side by side — no allocation, no hashing of long strings, run for
/// every candidate pair.
#[derive(Debug, Default)]
pub struct ChildSets {
    sets: HashMap<Overlap, HashMap<String, Vec<u32>>>,
    numbers: HashMap<Key, u32>,
}

impl ChildSets {
    /// Record that `record_id` holds `value` in `overlap`'s child set.
    pub fn insert(&mut self, overlap: Overlap, record_id: String, value: Key) {
        let next = u32::try_from(self.numbers.len()).unwrap_or(u32::MAX);
        let number = *self.numbers.entry(value).or_insert(next);
        let held = self
            .sets
            .entry(overlap)
            .or_default()
            .entry(record_id)
            .or_default();
        // Kept sorted as it grows, so the comparison can walk two lists at once.
        if let Err(at) = held.binary_search(&number) {
            held.insert(at, number);
        }
    }

    /// `record_id`'s values in this child set, sorted — empty for a record
    /// with no rows in it, which therefore shares nothing.
    fn of(&self, overlap: &Overlap, record_id: &str) -> &[u32] {
        self.sets
            .get(overlap)
            .and_then(|set| set.get(record_id))
            .map_or(&[], Vec::as_slice)
    }
}

/// Whether two sorted lists hold a number in common: advance whichever side is
/// behind until they meet or one runs out. A handful of steps for a record's
/// plots, and no allocation.
fn share(left: &[u32], right: &[u32]) -> bool {
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => return true,
        }
    }
    false
}

/// The pairs `policy`'s rules raise among `candidates`, each named smaller id
/// first, with at least one record in scope. Verdicts are not consulted here —
/// a pair judged distinct is still a pair to this function, and the caller
/// drops it.
///
/// For each rule:
///
///   1. **Group** the records by what must be equal: the farm (or the book, for
///      a book rule) and each compared column. A record whose [`Same::Stated`]
///      column is empty joins no group — it can match nothing. A record whose
///      day could not be read joins no dated group.
///   2. **Within a group**, a book rule compares every two records — its groups
///      are small by construction (one plot's crops, one delivery note). A
///      dated rule sorts the group by first day and walks it once: each record
///      is compared with the ones after it until one starts later than its last
///      day plus the slack. Sorted by first day, nothing after that can meet
///      it either, so each record costs the records inside its own window and
///      no more.
///   3. **Two records that meet** are a pair if they share a value in every
///      child set the rule compares.
///
/// Each record is compared with the ones after it, so every pair is looked at
/// once, from its earlier record, whichever of the two is in scope.
pub fn find_pairs(
    policy: &DuplicatePolicy,
    candidates: &[Candidate],
    children: &ChildSets,
) -> BTreeSet<(String, String)> {
    let columns = policy.compared_columns();
    let ongoing = matches!(policy.period(), Some((Period::Ongoing { .. }, _)));
    // The last day a record reaches: its own, its first for a one-day record,
    // or every day after it began for one still going on.
    let last_day = |candidate: &Candidate, first: i64| match candidate.last_day {
        Some(last) => last,
        None if ongoing => i64::MAX,
        None => first,
    };

    // Each record's child values, looked up once per child set and then read
    // by position. The walk below checks two records' plots once per pair
    // inside a window — hundreds of thousands of times on a large farm — and a
    // lookup by id there would hash two 36-character ids every time. Measured:
    // that alone was more than half the list's cost at 4 000 sprays a
    // campaign.
    let overlaps = policy.overlaps();
    let held: Vec<Vec<&[u32]>> = overlaps
        .iter()
        .map(|overlap| {
            candidates
                .iter()
                .map(|candidate| children.of(overlap, &candidate.id))
                .collect()
        })
        .collect();

    let mut pairs = BTreeSet::new();
    for rule in policy.rules() {
        // Which of `held` each of this rule's child sets is.
        let rule_sets: Vec<&Vec<&[u32]>> = rule
            .overlaps
            .iter()
            .filter_map(|overlap| {
                let at = overlaps.iter().position(|known| known == overlap)?;
                Some(&held[at])
            })
            .collect();

        // Where in `values` each of this rule's columns is, and whether it
        // must be stated.
        let tests: Vec<(usize, bool)> = rule
            .same
            .iter()
            .filter_map(|same| {
                let at = columns.iter().position(|column| *column == same.column())?;
                Some((at, same.stated()))
            })
            .collect();

        // 1. Group. The key borrows from `candidates` rather than copying.
        let mut groups: HashMap<(Option<&str>, Vec<&Key>), Vec<usize>> = HashMap::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if tests
                .iter()
                .any(|&(at, stated)| stated && candidate.values[at] == Key::Null)
            {
                continue;
            }
            let place = match rule.when {
                When::SameBook => Some(candidate.season_id.as_str()),
                When::Within { .. } => {
                    if candidate.first_day.is_none() {
                        continue;
                    }
                    candidate.farm_id.as_deref()
                }
            };
            let equal: Vec<&Key> = tests.iter().map(|&(at, _)| &candidate.values[at]).collect();
            groups.entry((place, equal)).or_default().push(index);
        }

        // 2 and 3. Walk each group.
        let mut consider = |a: usize, b: usize| {
            if !(candidates[a].in_scope || candidates[b].in_scope) {
                return;
            }
            if rule_sets.iter().all(|sets| share(sets[a], sets[b])) {
                let (first, second) = ordered_pair(&candidates[a].id, &candidates[b].id);
                pairs.insert((first.to_owned(), second.to_owned()));
            }
        };
        for members in groups.values_mut() {
            match rule.when {
                When::SameBook => {
                    for (n, &a) in members.iter().enumerate() {
                        for &b in &members[n + 1..] {
                            consider(a, b);
                        }
                    }
                }
                When::Within { days, .. } => {
                    // Every member has a first day: step 1 left out those that
                    // do not.
                    let first = |index: usize| candidates[index].first_day.unwrap_or(i64::MAX);
                    members.sort_by_key(|&index| first(index));
                    for (n, &a) in members.iter().enumerate() {
                        let reach =
                            last_day(&candidates[a], first(a)).saturating_add(i64::from(days));
                        for &b in &members[n + 1..] {
                            if first(b) > reach {
                                break;
                            }
                            consider(a, b);
                        }
                    }
                }
            }
        }
    }
    pairs
}

/// A pair as every device names it: the smaller id first.
pub fn ordered_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// What a person said about a suspected pair.
///
/// Serialised as its code (`"distinct"`), which is also what
/// `duplicate_verdict.verdict` stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Both are real: two operations, not one recorded twice.
    Distinct,
    /// One operation recorded twice; one record was kept and the other removed.
    Duplicate,
}

impl Verdict {
    /// The code this verdict is stored and sent as.
    pub fn code(self) -> &'static str {
        match self {
            Verdict::Distinct => "distinct",
            Verdict::Duplicate => "duplicate",
        }
    }

    /// The verdict a stored code names, or `None` for a code that names none.
    pub fn from_code(code: &str) -> Option<Verdict> {
        match code {
            "distinct" => Some(Verdict::Distinct),
            "duplicate" => Some(Verdict::Duplicate),
            _ => None,
        }
    }
}

// --- core's own registers ----------------------------------------------------

/// Two crops on one plot in one book: two devices each adding what grows
/// there. By the catalogue code where both have one, or by the name — the
/// same crop typed without its code on one device. A second cycle of the same
/// species is legitimate, and one "both are real" away.
pub const CROP_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "crop",
    detection: Detection::Rules(&[
        DuplicateRule {
            when: When::SameBook,
            same: &[Same::Value("plot_id"), Same::Stated("crop_code")],
            overlaps: &[],
        },
        DuplicateRule {
            when: When::SameBook,
            same: &[Same::Value("plot_id"), Same::Value("species_name")],
            overlaps: &[],
        },
    ]),
};

/// Two sowings of one kind on overlapping plots within a day of each other.
pub const SOWING_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "sowing_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Days {
                from: "sown_on",
                to: "sowing_end_date",
            },
            days: 1,
        },
        same: &[Same::Value("kind_code")],
        overlaps: &[Overlap {
            table: "sowing_plot",
            parent: "sowing_record_id",
            column: "plot_id",
        }],
    }]),
};

/// One load recorded twice. Several loads a day off one plot are ordinary,
/// each with its own delivery note — so on one day it takes the same product
/// and the same weight, and in one book the same delivery note alone.
pub const HARVEST_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "harvest_record",
    detection: Detection::Rules(&[
        DuplicateRule {
            when: When::Within {
                period: Period::Day { on: "harvested_on" },
                days: 0,
            },
            same: &[
                Same::Value("plant_product_code"),
                Same::Value("quantity_value"),
                Same::Value("quantity_unit_code"),
            ],
            overlaps: &[Overlap {
                table: "harvest_plot",
                parent: "harvest_record_id",
                column: "plot_id",
            }],
        },
        DuplicateRule {
            when: When::SameBook,
            same: &[Same::Stated("delivery_note_ref")],
            overlaps: &[],
        },
    ]),
};

#[cfg(test)]
mod tests {
    use super::*;

    const PLOTS: Overlap = Overlap {
        table: "applied_plot",
        parent: "application_id",
        column: "plot_id",
    };

    /// A register shaped like the treatment rule: a period of days, a product
    /// that two empty values agree on, a note that must be stated, and plots.
    const APPLICATION: DuplicatePolicy = DuplicatePolicy {
        table: "application",
        detection: Detection::Rules(&[DuplicateRule {
            when: When::Within {
                period: Period::Days {
                    from: "applied_on",
                    to: "applied_until",
                },
                days: 1,
            },
            same: &[Same::Value("product"), Same::Stated("note")],
            overlaps: &[PLOTS],
        }]),
    };

    /// A day number that reads like the date it stands for: `day(12)` is the
    /// 12th of some month.
    fn day(n: i64) -> Option<i64> {
        Some(2_461_000 + n)
    }

    fn record(id: &str, first: i64, last: Option<i64>, product: Key, note: Key) -> Candidate {
        Candidate {
            id: id.into(),
            season_id: "book".into(),
            farm_id: Some("farm".into()),
            first_day: day(first),
            last_day: last.and_then(day),
            values: vec![product, note],
            in_scope: true,
        }
    }

    fn text(value: &str) -> Key {
        Key::Text(value.into())
    }

    /// Every record on the same one plot.
    fn on_one_plot(records: &[Candidate]) -> ChildSets {
        let mut children = ChildSets::default();
        for record in records {
            children.insert(PLOTS, record.id.clone(), text("La Vega"));
        }
        children
    }

    fn pairs(policy: &DuplicatePolicy, records: &[Candidate]) -> Vec<(String, String)> {
        find_pairs(policy, records, &on_one_plot(records))
            .into_iter()
            .collect()
    }

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.into(), b.into())
    }

    #[test]
    fn a_pair_is_named_smaller_id_first_whichever_side_asks() {
        assert_eq!(ordered_pair("b", "a"), ("a", "b"));
        assert_eq!(ordered_pair("a", "b"), ("a", "b"));
    }

    #[test]
    fn a_verdict_reads_back_from_the_code_it_is_stored_as() {
        for verdict in [Verdict::Distinct, Verdict::Duplicate] {
            assert_eq!(Verdict::from_code(verdict.code()), Some(verdict));
            // The spelling serde puts in the log payload, which is what a
            // receiving device writes into the column.
            assert_eq!(
                serde_json::to_value(verdict).unwrap(),
                serde_json::Value::from(verdict.code())
            );
        }
        assert_eq!(Verdict::from_code("maybe"), None);
    }

    // --- the window -------------------------------------------------------------

    #[test]
    fn records_a_day_apart_meet_and_two_days_apart_do_not() {
        let records = [
            record("a", 10, None, text("X"), text("n")),
            record("b", 11, None, text("X"), text("n")),
            record("c", 13, None, text("X"), text("n")),
        ];
        assert_eq!(pairs(&APPLICATION, &records), vec![pair("a", "b")]);
    }

    #[test]
    fn a_period_meets_a_day_inside_it_or_within_its_slack_after_it() {
        let records = [
            record("spread", 10, Some(15), text("X"), text("n")),
            record("inside", 13, None, text("X"), text("n")),
            record("after", 16, None, text("X"), text("n")),
            record("later", 17, None, text("X"), text("n")),
        ];
        // `after` is a day past the period; `later` two. `inside` and `after`
        // are three days apart, and `after`/`later` a day apart.
        assert_eq!(
            pairs(&APPLICATION, &records),
            vec![
                pair("after", "later"),
                pair("after", "spread"),
                pair("inside", "spread"),
            ]
        );
    }

    #[test]
    fn the_walk_does_not_stop_at_a_short_record_inside_a_long_one() {
        // Sorted by first day: the long period starts first, a short one ends
        // early, and a third still falls inside the long one. The walk from
        // the long period must reach it past the short one.
        let records = [
            record("long", 1, Some(20), text("X"), text("n")),
            record("short", 2, None, text("X"), text("n")),
            record("third", 18, None, text("X"), text("n")),
        ];
        let found = pairs(&APPLICATION, &records);
        assert!(found.contains(&pair("long", "third")), "{found:?}");
        assert!(found.contains(&pair("long", "short")));
        assert!(!found.contains(&pair("short", "third")));
    }

    #[test]
    fn a_period_still_going_on_meets_every_later_record() {
        const GRAZING: DuplicatePolicy = DuplicatePolicy {
            table: "grazing",
            detection: Detection::Rules(&[DuplicateRule {
                when: When::Within {
                    period: Period::Ongoing {
                        from: "started_on",
                        to: "ended_on",
                    },
                    days: 0,
                },
                same: &[],
                overlaps: &[PLOTS],
            }]),
        };
        let mut open = record("open", 1, None, Key::Null, Key::Null);
        open.values.clear();
        let mut late = record("late", 900, Some(905), Key::Null, Key::Null);
        late.values.clear();
        assert_eq!(pairs(&GRAZING, &[open, late]), vec![pair("late", "open")]);
    }

    // --- what must be equal -------------------------------------------------------

    #[test]
    fn two_empty_values_agree_but_two_empty_stated_values_do_not() {
        let no_product = [
            record("a", 10, None, Key::Null, text("n")),
            record("b", 10, None, Key::Null, text("n")),
        ];
        assert_eq!(pairs(&APPLICATION, &no_product), vec![pair("a", "b")]);
        let no_note = [
            record("a", 10, None, text("X"), Key::Null),
            record("b", 10, None, text("X"), Key::Null),
        ];
        assert!(pairs(&APPLICATION, &no_note).is_empty());
    }

    #[test]
    fn a_different_value_or_another_farm_is_never_a_pair() {
        let mut elsewhere = record("c", 10, None, text("X"), text("n"));
        elsewhere.farm_id = Some("neighbour".into());
        let records = [
            record("a", 10, None, text("X"), text("n")),
            record("b", 10, None, text("Y"), text("n")),
            elsewhere,
        ];
        assert!(pairs(&APPLICATION, &records).is_empty());
    }

    #[test]
    fn a_float_is_one_value_whatever_its_sign_of_zero() {
        assert_eq!(Key::from(Value::Real(0.0)), Key::from(Value::Real(-0.0)));
        assert_ne!(Key::from(Value::Real(1.5)), Key::from(Value::Real(1.49)));
    }

    // --- the child sets -------------------------------------------------------------

    #[test]
    fn two_records_on_different_plots_are_not_a_pair() {
        let records = [
            record("a", 10, None, text("X"), text("n")),
            record("b", 10, None, text("X"), text("n")),
        ];
        let mut children = ChildSets::default();
        children.insert(PLOTS, "a".into(), text("La Vega"));
        children.insert(PLOTS, "a".into(), text("El Soto"));
        children.insert(PLOTS, "b".into(), text("La Loma"));
        assert!(find_pairs(&APPLICATION, &records, &children).is_empty());
        children.insert(PLOTS, "b".into(), text("El Soto"));
        assert_eq!(
            find_pairs(&APPLICATION, &records, &children)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![pair("a", "b")]
        );
    }

    #[test]
    fn a_record_with_no_child_rows_shares_nothing() {
        let records = [
            record("a", 10, None, text("X"), text("n")),
            record("b", 10, None, text("X"), text("n")),
        ];
        let mut children = ChildSets::default();
        children.insert(PLOTS, "a".into(), text("La Vega"));
        assert!(find_pairs(&APPLICATION, &records, &children).is_empty());
    }

    // --- scope, books and several rules -----------------------------------------------

    #[test]
    fn a_pair_needs_at_least_one_record_in_scope() {
        let mut outside = [
            record("a", 10, None, text("X"), text("n")),
            record("b", 10, None, text("X"), text("n")),
        ];
        outside[0].in_scope = false;
        assert_eq!(pairs(&APPLICATION, &outside), vec![pair("a", "b")]);
        outside[1].in_scope = false;
        assert!(pairs(&APPLICATION, &outside).is_empty());
    }

    #[test]
    fn a_book_rule_compares_within_a_book_and_any_rule_is_enough() {
        // Two rules, as the harvest has: one dated, one by a stated note in the
        // same book.
        const LOADS: DuplicatePolicy = DuplicatePolicy {
            table: "load",
            detection: Detection::Rules(&[
                DuplicateRule {
                    when: When::Within {
                        period: Period::Day { on: "on" },
                        days: 0,
                    },
                    same: &[Same::Value("weight")],
                    overlaps: &[],
                },
                DuplicateRule {
                    when: When::SameBook,
                    same: &[Same::Stated("note")],
                    overlaps: &[],
                },
            ]),
        };
        let load = |id: &str, on: i64, weight: i64, note: &str, book: &str| Candidate {
            id: id.into(),
            season_id: book.into(),
            farm_id: Some("farm".into()),
            first_day: day(on),
            last_day: None,
            values: vec![Key::Integer(weight), text(note)],
            in_scope: true,
        };
        let records = [
            load("same-day", 1, 900, "A-1", "book"),
            load("same-weight", 1, 900, "A-2", "book"),
            load("same-note", 20, 500, "A-1", "book"),
            load("note-elsewhere", 30, 700, "A-1", "other book"),
        ];
        let found: Vec<_> = find_pairs(&LOADS, &records, &ChildSets::default())
            .into_iter()
            .collect();
        assert_eq!(
            found,
            vec![
                pair("same-day", "same-note"),
                pair("same-day", "same-weight")
            ]
        );
    }

    // --- what a register declares ------------------------------------------------------

    #[test]
    fn a_register_names_each_compared_column_and_child_set_once() {
        assert_eq!(
            CROP_DUPLICATES.compared_columns(),
            vec!["plot_id", "crop_code", "species_name"]
        );
        assert_eq!(HARVEST_DUPLICATES.overlaps().len(), 1);
        assert!(CROP_DUPLICATES.overlaps().is_empty());
    }

    #[test]
    fn the_period_is_the_dated_rules_and_the_slack_their_widest() {
        assert_eq!(
            SOWING_DUPLICATES.period(),
            Some((
                Period::Days {
                    from: "sown_on",
                    to: "sowing_end_date"
                },
                1
            ))
        );
        assert_eq!(HARVEST_DUPLICATES.day_column(), Some("harvested_on"));
        assert_eq!(CROP_DUPLICATES.period(), None);
    }

    #[test]
    fn a_register_declared_never_has_no_rules() {
        let never = DuplicatePolicy {
            table: "plan",
            detection: Detection::Never {
                because: "one per campaign by construction",
            },
        };
        assert!(never.rules().is_empty());
        assert_eq!(never.day_column(), None);
    }

    #[test]
    fn a_book_register_is_fetched_by_book_and_a_dated_one_by_its_span() {
        let crops = candidates_sql(&CROP_DUPLICATES, Scope::Book { season_id: "s" });
        assert!(crops.contains("y.season_id IN (SELECT ?1)"), "{crops}");
        assert!(!crops.contains("span"));
        let sowings = candidates_sql(&SOWING_DUPLICATES, Scope::Current { today: "t" });
        let flat = sowings.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("y.sown_on BETWEEN date(span.lo, '-1 days')"),
            "{flat}"
        );
        assert!(flat.contains("CAST(julianday(y.sown_on) AS INTEGER)"));
    }
}
