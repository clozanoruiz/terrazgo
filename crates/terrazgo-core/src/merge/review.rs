// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a person is shown when two devices wrote one register, and what
//! happens when they choose (docs/sync.md → Conflicts as the person sees them).
//!
//! The third file of the merge, and the only one a person ever reaches:
//!
//!   * [`review`] reads both versions out of the log and lines them up field by
//!     field. It writes nothing — the losing branch is never materialised.
//!   * [`resolve`] takes the version a person chose and makes it the register,
//!     everywhere.
//!
//! **Resolving is an ordinary audited write, not a special state.** The change
//! set it makes carries the register's vector, which `WriteTx::register` builds
//! by merging every version the register has been written with — so it descends
//! from both branches, it is the register's only head on every device that
//! receives it, and the conflict closes with no message of its own. That is
//! also why *correcting the record in its own screen* resolves a conflict just
//! as well: this file makes the choice one click instead of two, and adds
//! nothing the log did not already understand.
//!
//! Nothing here is per-register. A register is whatever its rows say it is, and
//! both halves work off the log's images, so a register a future module adds
//! arrives reviewable.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, Statement};
use serde::Serialize;
use serde_json::{Value, json};

use crate::audit;
use crate::error::{CoreError, Result};

use super::head::{self, BranchState, Head};

// ---------------------------------------------------------------------------
// Naming a row to a person
// ---------------------------------------------------------------------------

/// Which column names a row of a table to a person.
///
/// Two questions need this and neither can be answered generically: what to
/// call the register a conflict is about ("the treatment of 12/05"), and what a
/// reference to another row says ("La Vega", not
/// `0192f3a4-0000-7000-8000-0000000000a1`). Both are a property of the table,
/// so both are answered by one declaration.
///
/// A date is a perfectly good name where a register has no other — a treatment
/// is known by the day it was applied, and its table has nothing else a person
/// would recognise.
///
/// Composed the way the aggregate map is: core's half here, each module's in
/// its own crate, joined by the shell — core may never name a module's tables.
/// A table missing from the map costs a name, never correctness: the value
/// falls back to the id it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowCaption {
    pub table: &'static str,
    pub column: &'static str,
}

impl RowCaption {
    pub const fn new(table: &'static str, column: &'static str) -> Self {
        RowCaption { table, column }
    }
}

/// Core's half of the naming map.
///
/// Slot-keyed registers are deliberately absent: nothing holds a reference to
/// one, and a slot names itself by the values in its key rather than by a row.
pub const CORE_ROW_CAPTIONS: &[RowCaption] = &[
    RowCaption::new("farm", "name"),
    RowCaption::new("season", "label"),
    RowCaption::new("plot", "name"),
    RowCaption::new("crop", "species_name"),
    RowCaption::new("operator", "full_name"),
    RowCaption::new("user_profile", "display_name"),
    RowCaption::new("advisor", "name"),
    RowCaption::new("machinery", "name"),
    RowCaption::new("premises", "name"),
    RowCaption::new("plot_water_point", "denomination"),
    RowCaption::new("sowing_record", "sown_on"),
    RowCaption::new("harvest_record", "harvested_on"),
    RowCaption::new("sync_peer", "label"),
];

/// The table a column refers to, if it refers to one that has a name.
///
/// By the schema's own convention — a reference is `<table>_id` — rather than
/// by a declared column-to-table map, which would be a third description of the
/// foreign keys the schema already states. A column whose stem is not a named
/// table (`tax_id`, `lab_tax_id`, `export_alias.entity_id`) simply resolves to
/// nothing and keeps its value.
fn referenced_table<'a>(column: &str, captions: &'a [RowCaption]) -> Option<&'a RowCaption> {
    let stem = column.strip_suffix("_id")?;
    captions.iter().find(|caption| caption.table == stem)
}

/// Resolves references to the names people know rows by, one prepared statement
/// per table and one lookup per distinct row.
///
/// The SQL names a table and a column, so it varies per table and must never
/// reach the shared prepared-statement cache (docs/architecture.md → the
/// statement cache is one small LRU keyed by SQL text). It is held here for the
/// life of one review instead, like the applier's.
struct Names<'conn> {
    conn: &'conn Connection,
    captions: &'conn [RowCaption],
    statements: HashMap<&'static str, Statement<'conn>>,
    resolved: HashMap<(&'static str, String), Option<String>>,
}

impl<'conn> Names<'conn> {
    fn new(conn: &'conn Connection, captions: &'conn [RowCaption]) -> Self {
        Names {
            conn,
            captions,
            statements: HashMap::new(),
            resolved: HashMap::new(),
        }
    }

    /// What `value` names, when the column it sits in refers to a named table.
    ///
    /// `None` whenever the question does not arise or has no answer: a column
    /// that is not a reference, a reference to a table with no name column, a
    /// row this device does not hold (a plot that exists only in a branch
    /// nobody materialised), or a row whose name is NULL.
    fn display(&mut self, column: &str, value: &Value) -> Result<Option<String>> {
        let Some(caption) = referenced_table(column, self.captions) else {
            return Ok(None);
        };
        let Some(id) = value.as_str() else {
            return Ok(None);
        };
        let key = (caption.table, id.to_owned());
        if let Some(known) = self.resolved.get(&key) {
            return Ok(known.clone());
        }
        let statement = match self.statements.entry(caption.table) {
            std::collections::hash_map::Entry::Occupied(held) => held.into_mut(),
            std::collections::hash_map::Entry::Vacant(empty) => {
                let sql = format!(
                    "SELECT {} FROM {} WHERE id = ?1",
                    caption.column, caption.table
                );
                empty.insert(self.conn.prepare(&sql)?)
            }
        };
        let found: Option<Option<String>> =
            statement.query_row([id], |row| row.get(0)).optional()?;
        let name = found.flatten();
        self.resolved.insert(key, name.clone());
        Ok(name)
    }
}

// ---------------------------------------------------------------------------
// What the screen is given
// ---------------------------------------------------------------------------

/// Columns no person compares, whatever register they are on.
///
/// `deleted_at` is NOT among them: one device deleting a register while another
/// corrected it is one of the states most worth reviewing. `created_at` and
/// `updated_at` are the housekeeping stamps two branches almost always differ
/// on and never differ *about*; `id` is the key the two sides are matched by,
/// so it cannot differ. `farm_id` places the register — a record does not move
/// between holdings — and the columns that place a row inside a register are
/// excluded beside it, off the aggregate map ([`crate::sync::placing_columns`]).
///
/// **`season_id` is compared.** A record does move between books: merging two
/// books that turned out to be one campaign re-points one's records into the
/// other (docs/sync.md → Merging two books), so a version made before the move
/// and one made after can disagree about the campaign — and a version filing
/// the record in a book that no longer exists is the one thing the person most
/// needs to see. It reads as the book's name, through `season`'s caption.
///
/// **This list and `syncFields.js`'s coverage are two halves of one rule**: a
/// column the comparison can emit is a column that must have a label, and
/// `sync_fields_contract.rs` exempts exactly what is exempted here. Widen one
/// and the other stops matching — the symptom is a raw column name in front of
/// a person mid-decision.
const STRUCTURAL: &[&str] = &["id", "created_at", "updated_at", "farm_id"];

/// One value on one side of the comparison.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewValue {
    /// The stored value, as the log holds it — a number stays a number, so the
    /// screen formats it in the reader's locale.
    pub value: Value,
    /// What it names, where it names something: a plot's name for a `plot_id`.
    /// `None` leaves the screen with the value itself.
    pub display: Option<String>,
}

/// One line of the comparison: one column of one row, with what each version
/// says about it.
///
/// `values` is in the order of [`ConflictReview::versions`], and `None` means
/// that version does not hold the row at all — a treated plot only one device
/// added, a child the other deleted.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewLine {
    pub table: String,
    /// Which row of `table` the line is about. For two records compared side
    /// by side it is the key their rows were lined up on rather than an id —
    /// see [`compare_records`].
    pub entity_id: String,
    pub column: String,
    /// Whether the row is the register itself rather than one of its children.
    pub root: bool,
    /// Whether the sides disagree. Always true in a conflict's review, which
    /// shows nothing else; two records compared side by side show what they
    /// share as well, and this is what tells the two apart.
    pub differs: bool,
    pub values: Vec<Option<ReviewValue>>,
}

/// Which lines a comparison keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    /// What the sides disagree about. Two versions of one register are mostly
    /// identical, and a screen listing thirty-seven identical columns would
    /// hide the three that matter.
    Differences,
    /// Every field either side states. Two records that may be one operation
    /// are judged on what they share as much as on where they part: the same
    /// product on the same plot is the whole reason they were put side by side.
    Stated,
}

/// One version of the register, as the review names it.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewVersion {
    /// The change set: the device that wrote it and its number there.
    pub device: String,
    pub seq: i64,
    /// What people call that device, if anybody has named it yet
    /// (`sync_peer.label`).
    pub label: Option<String>,
    /// Whether this is the version the book is showing now.
    pub live: bool,
    /// When it was written, as an inspector reads it. Never the HLC, which may
    /// legitimately run ahead of the wall clock.
    pub changed_at: String,
    /// The `user_profile` that wrote it, and that profile's name where this
    /// device holds it. A claim from another device may name a profile this
    /// one has never seen.
    pub actor: Option<String>,
    pub actor_name: Option<String>,
}

/// Both versions of one register, lined up for a person.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictReview {
    pub root_table: String,
    pub root_id: String,
    pub season_id: Option<String>,
    /// What to call the register — the live version's name column, where its
    /// table has one.
    pub caption: Option<String>,
    /// The live version first.
    pub versions: Vec<ReviewVersion>,
    /// Only what the versions disagree about. Two versions of a register are
    /// mostly identical, and a screen listing thirty-seven identical columns
    /// would hide the three that matter.
    pub lines: Vec<ReviewLine>,
}

/// One change set's authorship, for the version headings.
const CHANGE_SET_AUTHOR_SQL: &str = "SELECT changed_at, actor FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2
     ORDER BY id LIMIT 1";

/// A device's label, for naming a version after the phone that wrote it.
const PEER_LABEL_SQL: &str = "SELECT label FROM sync_peer WHERE id = ?1";

/// A profile's name, for naming a version after the person who wrote it.
const ACTOR_NAME_SQL: &str = "SELECT display_name FROM user_profile WHERE id = ?1";

/// The two versions of a conflicted register, field by field.
///
/// Reads the log and nothing else: the losing branch is reconstructed from its
/// change sets ([`head::branch_states`]) rather than materialised, so a person
/// can look at a version the tables have never held and walk away without
/// having changed anything.
///
/// Refuses a register with one head. A conflict can be resolved on another
/// device and arrive while this one's screen is open, and "there is nothing to
/// choose between" is an answer the screen must be able to give.
pub fn review(
    conn: &Connection,
    root_table: &str,
    root_id: &str,
    captions: &[RowCaption],
) -> Result<ConflictReview> {
    let found = head::heads(conn, root_table, root_id)?;
    if found.len() < 2 {
        return Err(CoreError::Invalid("register_not_in_conflict"));
    }
    let live = head::live_head(&found)
        .cloned()
        .ok_or(CoreError::Invalid("register_not_in_conflict"))?;
    // The live version first, so a screen reading left to right shows the book
    // as it stands and then what is waiting.
    let mut ordered = vec![live.clone()];
    ordered.extend(found.into_iter().filter(|head| head != &live));

    let states = head::branch_states(conn, root_table, root_id, &ordered)?;
    let mut names = Names::new(conn, captions);
    let lines = compare(conn, &states, root_table, &mut names, Show::Differences)?;

    let caption = states
        .first()
        .and_then(|state| register_caption(state, root_table, captions));
    let versions = ordered
        .iter()
        .map(|head| version_of(conn, head, head == &live))
        .collect::<Result<Vec<_>>>()?;

    Ok(ConflictReview {
        root_table: root_table.to_owned(),
        root_id: root_id.to_owned(),
        season_id: register_season(&states, root_table),
        caption,
        versions,
        lines,
    })
}

/// One version's heading: who wrote it, on what device, and when.
fn version_of(conn: &Connection, head: &Head, live: bool) -> Result<ReviewVersion> {
    let (changed_at, actor): (String, Option<String>) =
        crate::sql::cached_statement(conn, CHANGE_SET_AUTHOR_SQL)?
            .query_row(rusqlite::params![head.device, head.seq], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
    let label: Option<String> = crate::sql::cached_statement(conn, PEER_LABEL_SQL)?
        .query_row([&head.device], |row| row.get(0))
        .optional()?
        .flatten();
    let actor_name = match &actor {
        Some(id) => crate::sql::cached_statement(conn, ACTOR_NAME_SQL)?
            .query_row([id], |row| row.get(0))
            .optional()?,
        None => None,
    };
    Ok(ReviewVersion {
        device: head.device.clone(),
        seq: head.seq,
        label,
        live,
        changed_at,
        actor,
        actor_name,
    })
}

/// Every row either version holds, register first and then its children, with
/// each row's own key.
///
/// Register first because that is the order a person reads a record in, and the
/// log's own order — alphabetical by table — would open a treatment with its
/// treated plots. A slot-keyed register's rows are all in the root table, so
/// they sort first together.
fn ordered_keys<'a>(states: &'a [BranchState], root_table: &str) -> Vec<&'a (String, String)> {
    let mut keys: BTreeSet<&(String, String)> = BTreeSet::new();
    for state in states {
        keys.extend(state.keys());
    }
    let mut keys: Vec<_> = keys.into_iter().collect();
    keys.sort_by_key(|(table, id)| (table != root_table, table.clone(), id.clone()));
    keys
}

/// The lines of a comparison between two or more states: the ones they
/// disagree about, or every one either side states ([`Show`]).
fn compare(
    conn: &Connection,
    states: &[BranchState],
    root_table: &str,
    names: &mut Names,
    show: Show,
) -> Result<Vec<ReviewLine>> {
    let mut lines = Vec::new();
    for compared in compared_columns(conn, states, root_table)? {
        if show == Show::Differences && !compared.differs {
            continue;
        }
        // Two records side by side are each headed by their book already
        // (`DuplicateRecord::season_label`); a line saying it again is noise.
        // Two versions of one record have no such heading, so there it stays.
        if show == Show::Stated && compared.column == "season_id" {
            continue;
        }
        let mut rendered = Vec::with_capacity(compared.values.len());
        for value in compared.values {
            rendered.push(match value {
                Some(value) => Some(ReviewValue {
                    display: names.display(compared.column, &value)?,
                    value,
                }),
                None => None,
            });
        }
        lines.push(ReviewLine {
            table: compared.key.0.clone(),
            entity_id: compared.key.1.clone(),
            column: compared.column.to_owned(),
            root: compared.key.0 == root_table,
            differs: compared.differs,
            values: rendered,
        });
    }
    Ok(lines)
}

/// Whether two versions of a register say the same thing to a person: the
/// review would show no line between them (docs/sync.md → Versions that say
/// the same thing are not listed).
///
/// **The review's own comparison, not a second one.** Whatever this calls
/// agreement is what the review would have answered with "the two versions say
/// exactly the same", so a conflict left unlisted is one whose review offered a
/// choice that could change nothing visible. It errs one way only: two rows the
/// versions hold under different ids are two keys, and never agree.
///
/// **And two removals agree, whatever the record held.** Removed is removed —
/// all any screen shows — so choosing between them changes nothing visible;
/// and the choice, a write of its own, made the record a removal no book
/// deletion had made, which bringing the book back then left behind (*found by
/// the audit, 2026-10-03*). Brought back, the record returns as the later
/// removal left it; the other stays in the log.
pub(super) fn versions_agree(
    conn: &Connection,
    live: &BranchState,
    other: &BranchState,
    root_table: &str,
    root_id: &str,
) -> Result<bool> {
    if head::state_is_removed(live, root_table, root_id)
        && head::state_is_removed(other, root_table, root_id)
    {
        return Ok(true);
    }
    let states = [live.clone(), other.clone()];
    Ok(!compared_columns(conn, &states, root_table)?
        .iter()
        .any(|compared| compared.differs))
}

/// One column of one row, as each state holds it.
struct Compared<'a> {
    key: &'a (String, String),
    column: &'a str,
    /// In the order of the states; `None` where that state does not hold the
    /// row at all.
    values: Vec<Option<Value>>,
    differs: bool,
}

/// Every column a person could compare across `states`, row by row in reading
/// order, with whether the states disagree about it — what both the review and
/// [`versions_agree`] are made of, so the two cannot mean different things.
///
/// A column no state has a value for is left out: it disagrees only in the
/// arithmetic sense — a treated plot one device added has thirty NULL columns,
/// and none of them is what a person is choosing between.
fn compared_columns<'a>(
    conn: &Connection,
    states: &'a [BranchState],
    root_table: &str,
) -> Result<Vec<Compared<'a>>> {
    let mut found = Vec::new();
    // One lookup per table rather than per row: a register with forty treated
    // plots asks the map once.
    let mut placing: HashMap<String, Vec<String>> = HashMap::new();
    for key in ordered_keys(states, root_table) {
        if !placing.contains_key(&key.0) {
            placing.insert(key.0.clone(), crate::sync::placing_columns(conn, &key.0)?);
        }
        // `None` twice over: the version does not know the row, or knows it and
        // holds it deleted. Both mean "nothing on this side".
        let images: Vec<Option<&'a Value>> = states
            .iter()
            .map(|state| state.get(key).and_then(Option::as_ref))
            .collect();
        let mut columns: BTreeSet<&'a str> = BTreeSet::new();
        for image in images.iter().flatten() {
            columns.extend(
                image
                    .as_object()
                    .into_iter()
                    .flat_map(|object| object.keys().map(String::as_str)),
            );
        }
        for column in columns {
            let places = placing
                .get(&key.0)
                .is_some_and(|held| held.iter().any(|name| name == column));
            if STRUCTURAL.contains(&column) || places {
                continue;
            }
            let values: Vec<Option<Value>> = images
                .iter()
                .map(|image| image.map(|image| image[column].clone()))
                .collect();
            let stated = values
                .iter()
                .any(|value| matches!(value, Some(value) if !value.is_null()));
            if !stated {
                continue;
            }
            found.push(Compared {
                key,
                column,
                differs: disagree(column, &values),
                values,
            });
        }
    }
    Ok(found)
}

/// Whether the states disagree about one column's values.
///
/// **A removal is compared as removed or not, never by its instant.** Two
/// people pressing *keep this one* on the same pair remove the same copy a
/// minute apart; what they say about the record is the same, and the instant
/// each did it is in the log for anyone who asks. A removal against a record
/// still in the book stays the state most worth reviewing.
fn disagree(column: &str, values: &[Option<Value>]) -> bool {
    if column == "deleted_at" {
        let removed: Vec<Option<bool>> = values
            .iter()
            .map(|value| value.as_ref().map(|value| !value.is_null()))
            .collect();
        return removed.iter().any(|state| state != &removed[0]);
    }
    values.iter().any(|value| value != &values[0])
}

/// The register's own row in one version's state — the newest one, for a
/// slot-keyed register whose slot has been refilled.
fn root_image<'a>(state: &'a BranchState, root_table: &str) -> Option<&'a Value> {
    state
        .iter()
        .filter(|((table, _), _)| table == root_table)
        .filter_map(|(_, image)| image.as_ref())
        .next_back()
}

/// What to call the register, off the live version's own row.
fn register_caption(
    state: &BranchState,
    root_table: &str,
    captions: &[RowCaption],
) -> Option<String> {
    let caption = captions.iter().find(|entry| entry.table == root_table)?;
    let image = root_image(state, root_table)?;
    match &image[caption.column] {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// The campaign the register is in, as the versions state it. The live version
/// answers where they disagree — that is the book the register is filed in now.
fn register_season(states: &[BranchState], root_table: &str) -> Option<String> {
    states
        .iter()
        .find_map(|state| root_image(state, root_table))
        .and_then(|image| image["season_id"].as_str().map(str::to_owned))
}

// ---------------------------------------------------------------------------
// Two records of one register
// ---------------------------------------------------------------------------

/// Two records of one register, line by line, for a person judging whether they
/// are one operation recorded twice (docs/sync.md → Duplicate suspects). The
/// lines follow `ids`.
///
/// **The same comparison as a conflict's, over rows lined up differently.** Two
/// versions of one register share their row ids; two records share none — their
/// own rows and every child row have ids of their own. So each side is re-keyed
/// before the comparison sees it: the record's own row by its table alone, and a
/// child row by what makes it that child, the columns of its table's `UNIQUE`
/// key less the one naming the parent ([`ChildKeys`]). That is the plot for a
/// treated plot and the herd and species for a grazing's animals, so "La Vega"
/// on one record sits beside "La Vega" on the other, and a plot only one of them
/// lists stands alone.
///
/// Each side is read off its record's log, as a conflict's versions are — the
/// live version where the record is itself in conflict — and every field either
/// side states is kept, the differing ones marked ([`Show::Stated`]).
pub(crate) fn compare_records(
    conn: &Connection,
    root_table: &str,
    ids: [&str; 2],
    captions: &[RowCaption],
) -> Result<Vec<ReviewLine>> {
    let mut keys = ChildKeys::default();
    let mut states = Vec::with_capacity(ids.len());
    for id in ids {
        let found = head::heads(conn, root_table, id)?;
        let live = head::live_head(&found)
            .cloned()
            .ok_or(CoreError::NotFound)?;
        let state = head::branch_states(conn, root_table, id, &[live])?
            .into_iter()
            .next()
            .ok_or(CoreError::NotFound)?;
        states.push(keys.lined_up(conn, root_table, state)?);
    }
    let mut names = Names::new(conn, captions);
    compare(conn, &states, root_table, &mut names, Show::Stated)
}

/// The columns that line a child row up with its counterpart on another record,
/// per child table: the table's `UNIQUE` key less its placing column.
///
/// Read off the schema rather than declared, for the reason [`referenced_table`]
/// reads references off column names: the schema already states which columns
/// make a child row that child, and a second list would be one more description
/// of it to keep in step. Every child table of a register of the book has such a
/// key. One that had none would line up nothing — each of its rows keyed by its
/// own id, so standing alone on its side — which is plain rather than wrong.
#[derive(Debug, Default)]
struct ChildKeys {
    columns: HashMap<String, Option<Vec<String>>>,
}

/// The unique indexes on a table, as the schema holds them. Bound, so it stays
/// one statement whatever the table.
const UNIQUE_INDEXES_SQL: &str =
    "SELECT name FROM pragma_index_list(?1) WHERE \"unique\" = 1 AND partial = 0 ORDER BY seq";

/// One index's columns, in order.
const INDEX_COLUMNS_SQL: &str = "SELECT name FROM pragma_index_info(?1) ORDER BY seqno";

impl ChildKeys {
    /// `state` with each row keyed the way [`compare_records`] lines rows up:
    /// the record's own row by its table and an empty key, a child row by its
    /// key's values. A row the version holds deleted is left out — it is not
    /// on that side.
    fn lined_up(
        &mut self,
        conn: &Connection,
        root_table: &str,
        state: BranchState,
    ) -> Result<BranchState> {
        let mut lined = BranchState::new();
        for ((table, id), image) in state {
            let Some(image) = image else { continue };
            let key = if table == root_table {
                String::new()
            } else {
                match self.of(conn, &table)? {
                    Some(columns) => {
                        let values: Vec<Value> = columns
                            .iter()
                            .map(|column| image[column.as_str()].clone())
                            .collect();
                        Value::Array(values).to_string()
                    }
                    None => id,
                }
            };
            lined.insert((table, key), Some(image));
        }
        Ok(lined)
    }

    /// The key columns of one child table: `None` for a table with no key to
    /// line up on, and an empty list for one keyed by its parent alone — one
    /// such child per record, which lines up with the other record's one.
    fn of(&mut self, conn: &Connection, table: &str) -> Result<Option<&[String]>> {
        if !self.columns.contains_key(table) {
            let found = key_columns(conn, table)?;
            self.columns.insert(table.to_owned(), found);
        }
        Ok(self.columns.get(table).and_then(|found| found.as_deref()))
    }
}

/// The first full `UNIQUE` index on `table` that includes its placing column,
/// less that column — `treatment_plot`'s `(treatment_record_id, plot_id)` gives
/// `plot_id`. `None` when there is no such index.
fn key_columns(conn: &Connection, table: &str) -> Result<Option<Vec<String>>> {
    let placing = crate::sync::placing_columns(conn, table)?;
    let indexes: Vec<String> = conn
        .prepare(UNIQUE_INDEXES_SQL)?
        .query_map([table], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for index in indexes {
        let columns: Vec<String> = conn
            .prepare(INDEX_COLUMNS_SQL)?
            .query_map([&index], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if columns.iter().any(|column| placing.contains(column)) {
            return Ok(Some(
                columns
                    .into_iter()
                    .filter(|column| !placing.contains(column))
                    .collect(),
            ));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Choosing
// ---------------------------------------------------------------------------

/// Make the version a person chose the register's only version, everywhere.
///
/// The write states the whole difference between the two branches — every row
/// they disagree about, as the kept branch left it — in one change set. That is
/// what makes the choice travel: the change set's vector descends from both
/// branches, so on every device it is the register's only head, and a replay
/// that reaches it lands on the kept version whichever branch the device had
/// been showing.
///
/// **Two branches are compared, not all of them.** A three-way conflict needs
/// no third comparison, because the losing branches were never materialised
/// anywhere: [`head::contributing`] keeps, among mutually concurrent versions,
/// exactly the one [`head::live_head`] picks — so the state every device holds
/// is the live branch, and stating the difference from it is stating it from
/// what is really there.
///
/// `before` is the live version's image rather than what this device's tables
/// hold at the instant of the write — the same thing, since the live version IS
/// what the tables hold, and the honest audit statement either way: the book was
/// showing this, a person chose that.
///
/// Resolving a register with one head does nothing and says so by returning
/// `Ok`: two devices may resolve the same conflict, and the second one to be
/// asked has nothing left to do. A version that is no longer a head is a
/// different matter — somebody resolved it to something else, and quietly
/// applying a stale choice would overwrite their decision, so it is refused.
pub fn resolve(
    conn: &mut Connection,
    root_table: &str,
    root_id: &str,
    keep_device: &str,
    keep_seq: i64,
    actor: Option<&str>,
) -> Result<()> {
    let heads = head::heads(conn, root_table, root_id)?;
    let Some(keep) = heads
        .iter()
        .find(|head| head.device == keep_device && head.seq == keep_seq)
        .cloned()
    else {
        return Err(CoreError::Invalid("conflict_version_gone"));
    };
    if heads.len() == 1 {
        return Ok(());
    }
    let live = head::live_head(&heads)
        .cloned()
        .ok_or(CoreError::Invalid("conflict_version_gone"))?;
    let states = head::branch_states(conn, root_table, root_id, &[live.clone(), keep])?;
    let [live_state, keep_state] = states.as_slice() else {
        return Err(CoreError::Invalid("conflict_version_gone"));
    };

    // Only what differs, a removal on either side included: every device this
    // change set reaches holds both branches to rebuild the rest from, and one
    // that erased the register discards it (docs/sync.md → Bringing a register
    // back writes only what changes).
    let mut changes = Vec::new();
    for key in ordered_keys(&states, root_table) {
        let before = live_state.get(key).cloned().flatten();
        let after = keep_state.get(key).cloned().flatten();
        if before == after {
            continue;
        }
        changes.push((key.clone(), before, after));
    }
    // Two versions can differ in causality and agree in content — two devices
    // naming one phone the same thing, offline. The choice must still be
    // written down, or the conflict outlives the decision, so the register's
    // own row is restated.
    if changes.is_empty() {
        let (key, image) = keep_state
            .iter()
            .rfind(|((table, _), image)| table == root_table && image.is_some())
            .ok_or(CoreError::Invalid("conflict_version_gone"))?;
        changes.push((key.clone(), image.clone(), image.clone()));
    }
    // The register's own row first, then by table and id, as
    // `repository::undo` writes. The two versions can each hold one slot
    // under a row of their own — two devices adding the same plot — and the
    // row leaving has to free it before the one arriving takes it; which row
    // leaves depends on the tables being written, so `settle` orders that on
    // every device, the resolver's included (`head::make_live`).
    changes.sort_by_key(|((table, id), _, _)| (table != root_table, table.clone(), id.clone()));

    // The campaign the kept version states, not the register's newest row: a
    // resolution files the register where the version a person chose puts it,
    // and `audit::write_change` checks every logged image against the stamp.
    let season = root_image(keep_state, root_table)
        .and_then(|image| image["season_id"].as_str().map(str::to_owned));

    let tx = audit::begin(conn, actor)?;
    let stamp = tx.register(root_table, root_id, season.as_deref())?;
    for ((table, id), before, after) in &changes {
        let operation = match (before, after) {
            (None, _) => "insert",
            (_, None) => "delete",
            _ => "update",
        };
        audit::write_change(
            &tx,
            &stamp,
            table,
            id,
            operation,
            json!({ "before": before, "after": after }),
        )?;
    }
    // The tables are brought to the kept version by the same path an incoming
    // bundle takes — the log is read back and materialised — rather than by
    // writing the rows here. A register's tables are a pure function of its
    // log, and a resolution is not the place to keep a second opinion.
    head::settle(&tx, root_table, root_id, Some(&live))?;
    tx.commit()?;
    Ok(())
}
