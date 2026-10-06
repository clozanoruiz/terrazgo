// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The foundations stage-2 sync stands on (docs/sync.md → Part 2): which device
//! a connection writes as, the hybrid logical clock, version vectors, and the
//! classification every table carries for the merge.
//!
//! Nothing here merges anything. What it provides is what every write needs
//! *today* so that a merge is possible later: `audit::begin` reads the device
//! from here, stamps the clock from here, and computes a register's vector with
//! the type defined here.

use std::collections::BTreeMap;
use std::fmt;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::error::{CoreError, Result};

// ---------------------------------------------------------------------------
// Device identity
// ---------------------------------------------------------------------------

/// A new device id: a UUIDv7, minted once per installation and kept in
/// `settings.json` (docs/sync.md → Device identity).
///
/// Also minted afresh whenever a backup is imported. A restored database is a
/// different replica from the one that kept writing after the snapshot was
/// taken, and giving it a new name is what makes it impossible for the two to
/// reuse one change-set number for different content.
pub fn mint_device_id() -> String {
    Uuid::now_v7().to_string()
}

/// Whether `id` is usable as a device id: a UUID in the canonical hyphenated,
/// lowercase form every id in this schema uses.
///
/// Strict on purpose. The value comes from `settings.json`, which a person can
/// edit, and it is written into every log row forever after; `{…}`, `urn:` or
/// upper-case spellings of one UUID would name the same device in three ways.
pub fn is_device_id(id: &str) -> bool {
    is_canonical_uuid(id)
}

/// Whether `id` is a UUID in the one spelling this schema uses: hyphenated and
/// lower-case. Shared by the two identities that arrive from outside the code —
/// the device id off `settings.json`, and the sync group off a peer's bundle.
fn is_canonical_uuid(id: &str) -> bool {
    Uuid::try_parse(id).is_ok_and(|uuid| uuid.hyphenated().to_string() == id)
}

/// Tell this connection which device it writes as.
///
/// The identity lives in a TEMP table, which is the property wanted: it
/// belongs to the connection, not to the file. It is therefore never in a
/// `VACUUM INTO` snapshot, so a backup cannot carry one device's name onto
/// another machine, and a database opened without it cannot log a change —
/// [`installed_device`] refuses rather than guessing.
///
/// It is connection state rather than a parameter of every write, which the
/// crate's own docs argue against for the actor. The difference is who
/// supplies it: the actor changes per command, while the device is fixed for
/// the life of the connection, so it is installed by the code that opens the
/// connection — the shell's `open_app_db`, which takes it as an argument, and
/// each crate's in-memory test opener. Calling this again replaces the identity.
pub fn install_device(conn: &Connection, device_id: &str) -> Result<()> {
    if !is_device_id(device_id) {
        return Err(CoreError::Stamp("invalid_device_id"));
    }
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS sync_device (id TEXT NOT NULL);
         DELETE FROM temp.sync_device;",
    )?;
    conn.execute("INSERT INTO temp.sync_device (id) VALUES (?1)", [device_id])?;
    Ok(())
}

/// Whether the TEMP table holding the device identity exists at all.
const DEVICE_INSTALLED_SQL: &str = "SELECT EXISTS(SELECT 1 FROM sqlite_temp_schema
                                    WHERE type = 'table' AND name = 'sync_device')";

/// The identity itself. One row or none.
const DEVICE_ID_SQL: &str = "SELECT id FROM temp.sync_device";

/// The device this connection writes as, or `Stamp("no_device_identity")`.
///
/// Two queries rather than one because the table itself may not exist, and
/// naming a missing table is an error at prepare time that would otherwise
/// have to be told apart from every other SQLite error by its message. Both
/// are cached: every write that logs runs them.
pub fn installed_device(conn: &Connection) -> Result<String> {
    let installed: bool =
        crate::sql::cached_statement(conn, DEVICE_INSTALLED_SQL)?.query_row([], |r| r.get(0))?;
    if !installed {
        return Err(CoreError::Stamp("no_device_identity"));
    }
    crate::sql::cached_statement(conn, DEVICE_ID_SQL)?
        .query_row([], |r| r.get(0))
        .optional()?
        .ok_or(CoreError::Stamp("no_device_identity"))
}

/// Tell this connection how every table merges — the aggregate map, composed
/// from core's half and each module's — so that every logged row is checked
/// against it AS IT IS WRITTEN (`audit::write_change`).
///
/// This is what makes filing a row under the wrong register impossible to
/// ship rather than merely unlikely: every write path in every test, and in
/// the running app, is checked on every write, with no list of cases to keep
/// in step. Like the device identity it lives in a TEMP table, belongs to the
/// connection rather than the file, and a connection without it cannot log.
/// Declaring one table twice is refused.
///
/// It also marks every table a foreign key from outside its own register
/// points at — what the purge never erases (docs/sync.md → What can go: what
/// nothing else can point at). Read off the schema here, once per connection,
/// so it runs after the migrations, as every caller does: read on every purge
/// instead, it cost every start and import 1.7 ms for an answer only the
/// schema changes.
pub fn install_shape(conn: &Connection, shapes: &[&[TableSync]]) -> Result<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS sync_shape (
             table_name TEXT PRIMARY KEY,
             role       TEXT NOT NULL,   -- 'root' | 'child' | 'slot' | 'local'
             root       TEXT,            -- a child's register table
             fk         TEXT,            -- a child's column naming its register row
             columns    TEXT,            -- a slot's columns, as a JSON array
             pointed_at INTEGER NOT NULL DEFAULT 0  -- named from outside its register
         );
         DELETE FROM temp.sync_shape;",
    )?;
    let mut insert = conn.prepare(
        "INSERT INTO temp.sync_shape (table_name, role, root, fk, columns)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for entry in shapes.iter().flat_map(|shape| shape.iter()) {
        let (role, root, fk, columns) = match entry.role {
            SyncRole::Root => ("root", None, None, None),
            SyncRole::Child { root, fk } => ("child", Some(root), Some(fk), None),
            SyncRole::Slot { columns } => {
                ("slot", None, None, Some(serde_json::to_string(columns)?))
            }
            SyncRole::Local => ("local", None, None, None),
        };
        insert
            .execute(rusqlite::params![entry.table, role, root, fk, columns])
            .map_err(|err| match err.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    CoreError::ShapeViolation(format!("{} is declared twice", entry.table))
                }
                _ => err.into(),
            })?;
    }
    conn.execute(MARK_POINTED_AT_SQL, [])?;
    Ok(())
}

/// Mark every table some foreign key points at — less the one link that makes a
/// child part of its register, which goes with it by definition. One statement
/// over the schema, grouped by key so a composite key counts once.
const MARK_POINTED_AT_SQL: &str = "UPDATE temp.sync_shape SET pointed_at = 1
     WHERE table_name IN (
       SELECT keys.target FROM (
         SELECT m.name AS source, fk.\"table\" AS target,
                COUNT(*) AS width, MIN(fk.\"from\") AS link
         FROM sqlite_schema AS m, pragma_foreign_key_list(m.name) AS fk
         WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%'
         GROUP BY m.name, fk.id
       ) AS keys
       WHERE NOT EXISTS (
         SELECT 1 FROM temp.sync_shape AS own
         WHERE own.table_name = keys.source AND own.role = 'child'
           AND own.root = keys.target AND own.fk = keys.link AND keys.width = 1))";

/// Whether the TEMP table holding the aggregate map exists at all.
const SHAPE_INSTALLED_SQL: &str = "SELECT EXISTS(SELECT 1 FROM sqlite_temp_schema
                                   WHERE type = 'table' AND name = 'sync_shape')";

/// One table's classification, by name. A primary-key seek.
const DECLARED_SHAPE_SQL: &str =
    "SELECT role, root, fk, columns FROM temp.sync_shape WHERE table_name = ?1";

/// Whether [`install_shape`] has run on this connection.
pub(crate) fn shape_installed(conn: &Connection) -> Result<bool> {
    Ok(crate::sql::cached_statement(conn, SHAPE_INSTALLED_SQL)?.query_row([], |r| r.get(0))?)
}

/// The columns of `table` that PLACE a row inside its register rather than
/// describing it: a child's link to the register it belongs to, or a
/// slot-keyed register's own key.
///
/// They are the one part of a row that is never a statement about the world,
/// so nothing that compares two versions of a register has anything to say
/// about them — a treated plot's `treatment_record_id` names the treatment
/// being compared, and its value is the same on both sides by construction.
///
/// Read off the installed map rather than listed here, so it answers for a
/// module's tables as well as core's.
pub(crate) fn placing_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let declared = crate::sql::cached_statement(conn, DECLARED_SHAPE_SQL)?
        .query_row([table], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .optional()?;
    Ok(match declared {
        Some((role, Some(fk), _)) if role == "child" => vec![fk],
        Some((role, _, Some(columns))) if role == "slot" => serde_json::from_str(&columns)?,
        _ => Vec::new(),
    })
}

/// A table whose rows belong to one book: a register of its own carrying a
/// `season_id`, or a slot-keyed register whose key names the book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookTable {
    pub table: String,
    /// A slot's key columns, in the map's order; `None` for a register of its
    /// own, whose rows are each a register.
    pub slot: Option<Vec<String>>,
    /// Whether the table names its farm beside its book. Every register does
    /// but `crop`, which reaches its farm through its plot — and a book's rows
    /// are read on the index that leads with the columns it has.
    pub farm_scoped: bool,
    /// Whether the purge may erase its registers: no foreign key from outside
    /// a register points at its table or at a table of its children
    /// (docs/sync.md → What can go: what nothing else can point at). A crop,
    /// which sowings and treatments name, is not; a treatment is.
    pub erasable: bool,
}

/// Which of a book's rows [`BookTable::rows_sql`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookRows {
    /// Live and removed alike.
    Every,
    /// Live only — what a merge moves and a deletion removes.
    Live,
    /// Removed only — what bringing a book back looks through.
    Removed,
}

impl BookTable {
    /// The ids of the table's rows in one book — `?1` the book, `?2` its farm
    /// when the table names one — filtered by `rows`. Seeks the table's book
    /// index; `book_merge_contract.rs` holds it to that.
    pub fn rows_sql(&self, rows: BookRows) -> String {
        let mut sql = format!(
            "SELECT id FROM {} WHERE season_id = ?1",
            crate::merge::quote_ident(&self.table)
        );
        if self.farm_scoped {
            sql.push_str(" AND farm_id = ?2");
        }
        match rows {
            BookRows::Every => {}
            BookRows::Live => sql.push_str(" AND deleted_at IS NULL"),
            BookRows::Removed => sql.push_str(" AND deleted_at IS NOT NULL"),
        }
        sql.push_str(" ORDER BY id");
        sql
    }

    /// The table's live rows in removed books of live farms, as `(book, row,
    /// caption)` — `caption` the column naming a row to a person, when the
    /// table has one. Starts from the removed books (`idx_season_removed`, empty
    /// on almost every device) and seeks the table on its book index, so what
    /// it costs does not grow with the books and records still in use.
    ///
    /// **The loop order is pinned, not left to the planner.** With no
    /// statistics it may just as well walk a whole register and look each
    /// row's book up (it did, for `crop`), and a `CROSS JOIN` is SQLite's
    /// documented way to say which table is the outer loop
    /// (sqlite.org/optoverview.html → Manual Control Of Query Plans Using
    /// CROSS JOIN). Unordered for the same reason: ordering by the book's id
    /// walked every book in id order. The caller groups and sorts in memory
    /// what comes back — a handful of rows, or none.
    pub fn stray_sql(&self, caption: Option<&str>) -> String {
        let named = caption.map_or_else(
            || "NULL".to_owned(),
            |column| format!("t.{}", crate::merge::quote_ident(column)),
        );
        let same_farm = if self.farm_scoped {
            " AND t.farm_id = s.farm_id"
        } else {
            ""
        };
        format!(
            "SELECT s.id, t.id, {named}
             FROM season AS s
             CROSS JOIN {} AS t ON t.season_id = s.id{same_farm}
             JOIN farm AS f ON f.id = s.farm_id AND f.deleted_at IS NULL
             WHERE s.deleted_at IS NOT NULL AND t.deleted_at IS NULL",
            crate::merge::quote_ident(&self.table)
        )
    }
}

/// Every table hanging off a book, read off the installed map and the schema
/// rather than listed: what merging two books moves, what deleting a book
/// removes and bringing it back restores, and what the list of records in a
/// removed book reads (docs/sync.md → Merging two books, Deleting a book with
/// its records).
///
/// Read rather than declared so that a register a module adds next year is
/// moved with the rest the day it exists — the shell's `book_merge_contract.rs`
/// holds every table carrying `season_id` to being found here. `season` itself
/// is left out: it is the book, not something in one.
pub fn book_tables(conn: &Connection) -> Result<Vec<BookTable>> {
    let mut stmt = conn.prepare(BOOK_TABLES_SQL)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, bool>(2)?,
            row.get::<_, bool>(3)?,
        ))
    })?;
    let mut found = Vec::new();
    for row in rows {
        let (table, columns, farm_scoped, erasable) = row?;
        let slot = columns
            .map(|columns| serde_json::from_str::<Vec<String>>(&columns))
            .transpose()?;
        found.push(BookTable {
            table,
            slot,
            farm_scoped,
            erasable,
        });
    }
    Ok(found)
}

/// The tables [`book_tables`] answers with: registers and slots of the
/// installed map that carry a `season_id`, and whether each can be erased —
/// itself and its children unmarked by [`install_shape`]. Not through the
/// statement cache — it runs once per merge or list, and its table-valued
/// pragmas would hold a slot the writes need.
const BOOK_TABLES_SQL: &str = "SELECT shape.table_name, shape.columns,
            EXISTS(SELECT 1 FROM pragma_table_info(shape.table_name) WHERE name = 'farm_id'),
            NOT shape.pointed_at AND NOT EXISTS(
              SELECT 1 FROM temp.sync_shape AS child
              WHERE child.role = 'child' AND child.root = shape.table_name
                AND child.pointed_at)
     FROM temp.sync_shape AS shape
     WHERE shape.role IN ('root', 'slot') AND shape.table_name <> 'season'
       AND EXISTS(SELECT 1 FROM pragma_table_info(shape.table_name) WHERE name = 'season_id')
     ORDER BY shape.table_name";

/// One table's row of the installed map, as [`install_shape`] stored it.
struct Declared {
    role: String,
    root: Option<String>,
    fk: Option<String>,
    columns: Option<String>,
}

/// The register a logged row belongs to, derived from the installed map and
/// the row's own image — what [`crate::audit`] compares each stamp against.
///
/// `image` is the row as logged: the after-image, or the before-image of a
/// hard delete. Returns `(root_table, root_id)`.
///
/// On the way it checks the row is the one the log call names: `id` is the
/// row's own key as its image states it — `id`, or for a regional extension
/// keyed by its parent, that column. A log row addressed to one row but
/// carrying another's image is what a receiving device would materialise
/// under the wrong key.
pub(crate) fn register_of(
    conn: &Connection,
    table: &str,
    id: &str,
    image: &serde_json::Value,
) -> Result<(String, String)> {
    let declared = crate::sql::cached_statement(conn, DECLARED_SHAPE_SQL)?
        .query_row([table], |r| {
            Ok(Declared {
                role: r.get(0)?,
                root: r.get(1)?,
                fk: r.get(2)?,
                columns: r.get(3)?,
            })
        })
        .optional()?;
    let Some(Declared {
        role,
        root,
        fk,
        columns,
    }) = declared
    else {
        return Err(CoreError::ShapeViolation(format!(
            "{table} is not in the aggregate map — declare it in its crate's sync shape"
        )));
    };
    let missing =
        |column: &str| CoreError::ShapeViolation(format!("{table}'s logged image has no {column}"));
    if role == "local" {
        return Err(CoreError::ShapeViolation(format!(
            "{table} is never synced, so it may not be logged"
        )));
    }
    let own_key = match (image.get("id"), fk.as_deref()) {
        (Some(key), _) => key.as_str(),
        (None, Some(parent_column)) => image.get(parent_column).and_then(serde_json::Value::as_str),
        (None, None) => None,
    };
    if own_key != Some(id) {
        return Err(CoreError::ShapeViolation(format!(
            "{table} row logged as {id}, but its image names {own_key:?}"
        )));
    }
    match (role.as_str(), root, fk, columns) {
        ("root", ..) => Ok((table.to_owned(), id.to_owned())),
        ("child", Some(root), Some(fk), _) => {
            let parent = image
                .get(&fk)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| missing(&fk))?;
            Ok((root, parent.to_owned()))
        }
        ("slot", _, _, Some(columns)) => {
            let columns: Vec<String> = serde_json::from_str(&columns)?;
            let values = columns
                .iter()
                .map(|column| image.get(column).cloned().ok_or_else(|| missing(column)))
                .collect::<Result<Vec<_>>>()?;
            Ok((table.to_owned(), slot_id(&values)))
        }
        _ => Err(CoreError::ShapeViolation(format!(
            "{table} has a malformed entry in the aggregate map"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Group identity
// ---------------------------------------------------------------------------

/// The holding this device syncs within, or `None` if it has not joined one.
///
/// Where the device id says *who wrote this*, the group says *whose book this
/// is* — which is the only thing that can make a bundle from a neighbour's
/// holding refusable (docs/sync.md → Device identity). It lives in the
/// database, so a restored backup is still the same holding, where a restored
/// device is deliberately a new replica with a new device id.
pub fn sync_group(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .prepare("SELECT group_id FROM sync_group WHERE row_id = 1")?
        .query_row([], |row| row.get(0))
        .optional()?)
}

/// This device's group, minting one if it has none.
///
/// **Called by the export, and nowhere earlier.** Minting at first launch
/// would give two devices set up separately two groups before anything had
/// decided what joining means, and they would then refuse each other for ever.
/// By the time a device writes a bundle, it is claiming to be a holding's
/// device, which is the first moment the question has an answer.
pub fn ensure_sync_group(conn: &Connection) -> Result<String> {
    if let Some(held) = sync_group(conn)? {
        return Ok(held);
    }
    let minted = Uuid::now_v7().to_string();
    conn.execute(
        "INSERT INTO sync_group (row_id, group_id, joined_at) VALUES (1, ?1, ?2)",
        rusqlite::params![minted, crate::date::now_utc_iso()],
    )?;
    Ok(minted)
}

/// Take `group_id` as this device's group — **joining**, which is the whole of
/// what joining is.
///
/// It replaces whatever was there, because the two cases a caller faces need
/// the same act and differ only in how alarming they are: a device that has
/// never exported has no group and is simply being paired, while one that
/// minted its own is leaving that group for this one. Which of those is
/// happening is visible to the caller — the import says
/// `sync_not_paired` or `sync_group_mismatch` — and how loudly to ask is a
/// question for the screen, not for here.
pub fn join_sync_group(conn: &Connection, group_id: &str) -> Result<()> {
    if !is_canonical_uuid(group_id) {
        return Err(CoreError::Invalid("sync_group_invalid"));
    }
    conn.execute(
        "INSERT INTO sync_group (row_id, group_id, joined_at) VALUES (1, ?1, ?2)
         ON CONFLICT(row_id) DO UPDATE SET group_id = excluded.group_id,
                                           joined_at = excluded.joined_at",
        rusqlite::params![group_id, crate::date::now_utc_iso()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// What this database has held, and what this device knows of the others
// ---------------------------------------------------------------------------

/// The highest change-set number this database holds from one device, or held
/// before a purge took it out of the log: the log's own highest, or
/// `sync_held`'s, whichever is greater. Two seeks — the leading pair of
/// `record_change`'s UNIQUE, and `sync_held`'s key.
///
/// **What every question about "how far does this device have" asks**: the
/// stamp's next number, the manifest's `seen`, and whether a file reaches back
/// far enough to apply here (docs/sync.md → What a database has held, and what
/// a file starts from). The log alone stops answering it the moment a purge
/// takes a device's last change set away — that number would then be handed to
/// a different change set.
pub(crate) const HELD_THROUGH_SQL: &str = "SELECT max(
            ifnull((SELECT MAX(origin_seq) FROM record_change WHERE origin_device = ?1), 0),
            ifnull((SELECT through FROM sync_held WHERE device = ?1), 0))";

/// See [`HELD_THROUGH_SQL`]. 0 for a device this database holds nothing of.
pub fn held_through(conn: &Connection, device: &str) -> Result<i64> {
    Ok(crate::sql::cached_statement(conn, HELD_THROUGH_SQL)?.query_row([device], |r| r.get(0))?)
}

/// Raise what this database has held of a device, never lowering it.
const RAISE_HELD_SQL: &str = "INSERT INTO sync_held (device, through) VALUES (?1, ?2)
     ON CONFLICT(device) DO UPDATE SET through = max(through, excluded.through)";

/// Raise what this database has held of every device `seen` names to at least
/// what it names — after a file applies, when this database holds everything
/// its sender held, erased change sets included; and before a purge takes
/// change sets out of the log.
pub(crate) fn raise_held(conn: &Connection, seen: &VersionVector) -> Result<()> {
    // Once per import or purge, so prepared here rather than kept in the
    // shared cache the writes need.
    let mut stmt = conn.prepare(RAISE_HELD_SQL)?;
    for (device, through) in seen.iter() {
        stmt.execute(rusqlite::params![device, through])?;
    }
    Ok(())
}

/// What this device knows each device of the group holds — the highest `seen`
/// heard from it, in its own file or passed on in another's (docs/sync.md →
/// What each device knows of the others). Itself not included: what it holds
/// is [`crate::bundle::seen_by`].
pub fn known(conn: &Connection) -> Result<BTreeMap<String, VersionVector>> {
    let mut stmt = conn.prepare("SELECT device, seen FROM sync_known ORDER BY device")?;
    let mut rows = stmt.query([])?;
    let mut known = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let device: String = row.get(0)?;
        let seen: String = row.get(1)?;
        known.insert(device, VersionVector::from_json(&seen)?);
    }
    Ok(known)
}

/// Raise what this device knows of `device` to include `seen`. Called only once
/// a file has applied: before then this device does not hold what the file's
/// sender held, and knowledge must never run ahead of the data it is about.
pub(crate) fn raise_known(conn: &Connection, device: &str, seen: &VersionVector) -> Result<()> {
    let held: Option<String> = conn
        .query_row(
            "SELECT seen FROM sync_known WHERE device = ?1",
            [device],
            |r| r.get(0),
        )
        .optional()?;
    let mut raised = match held {
        Some(text) => VersionVector::from_json(&text)?,
        None => VersionVector::default(),
    };
    raised.merge(seen);
    conn.execute(
        "INSERT INTO sync_known (device, seen) VALUES (?1, ?2)
         ON CONFLICT(device) DO UPDATE SET seen = excluded.seen",
        rusqlite::params![device, raised.to_json()?],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Hybrid logical clock
// ---------------------------------------------------------------------------

/// A hybrid logical clock value ([Kulkarni et al., 2014]): wall-clock
/// milliseconds and a counter, packed so that comparing two values as integers
/// compares the milliseconds first and the counter second.
///
/// ```text
///   bit 63     bits 62..16                   bits 15..0
///   0          ms since the Unix epoch       counter
/// ```
///
/// Bit 63 is always zero, so SQLite — whose INTEGER is signed — stores every
/// value as a non-negative number and sorts it correctly. That leaves 47 bits
/// of milliseconds, enough until roughly the year 6400.
///
/// **It orders changes already known to conflict, and nothing else**
/// (docs/sync.md → Hybrid logical clocks). Whether two changes conflict is the
/// version vector's question. And it is never a time to show anybody: its
/// millisecond part can legitimately run ahead of the wall clock.
///
/// It serialises as a decimal STRING, never a JSON number. A JSON number that
/// reaches JavaScript becomes a double, exact only to 2^53, and this value is
/// larger than that today. A frontend that needs to compare two reads them
/// with `BigInt(value)`. Putting the rule in the type means no code path can
/// send the number by accident.
///
/// [Kulkarni et al., 2014]: https://cse.buffalo.edu/tech-reports/2014-04.pdf
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc(u64);

impl Hlc {
    /// Low bits holding the counter.
    pub const COUNTER_BITS: u32 = 16;

    /// The counter's bits, all set: the largest counter, and the mask that
    /// reads it. A constant, so an out-of-range shift here would be a compile
    /// error rather than a runtime one.
    const COUNTER_MAX: u64 = (1 << Self::COUNTER_BITS) - 1;

    /// The largest value the encoding admits: bit 63 clear, which is also the
    /// largest value SQLite's signed INTEGER can hold. (A positive `i64` is
    /// exactly representable as `u64`, so the cast loses nothing.)
    pub const MAX: u64 = i64::MAX as u64;

    /// The largest millisecond the encoding holds — about the year 6400.
    const PHYSICAL_MAX: u64 = Self::MAX >> Self::COUNTER_BITS;

    /// A value read from storage or from another device, refused if it could
    /// not have been produced by [`Hlc::next`].
    pub fn from_raw(raw: u64) -> Result<Hlc> {
        if raw > Self::MAX {
            return Err(CoreError::Stamp("hlc_out_of_range"));
        }
        Ok(Hlc(raw))
    }

    /// The packed value.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// The millisecond part. Close to the wall clock of the device that made
    /// it, but NOT a timestamp — see the type's documentation.
    pub fn physical_ms(self) -> u64 {
        self.0 >> Self::COUNTER_BITS
    }

    /// The counter part: how many stamps shared this millisecond before it.
    pub fn counter(self) -> u64 {
        self.0 & Self::COUNTER_MAX
    }

    /// How far ahead of the receiving device's clock an incoming stamp may be
    /// before the bundle carrying it is refused: **two hours**
    /// (docs/sync.md → How far ahead a stamp may be).
    ///
    /// The receive rule is what makes the clock causality-respecting — a device
    /// adopts any stamp ahead of its own — and that is also the hazard: a phone
    /// whose date reads 2030 would drag every device it syncs with to 2030, for
    /// good. Since nothing beyond this bound is ever adopted, the worst skew
    /// this device can carry is two hours, and real time erases it in two
    /// hours. Two rather than one because a clock set to Spanish local time in
    /// the belief it is UTC lands at exactly +1 or +2.
    pub const MAX_SKEW_MS: i64 = 2 * 60 * 60 * 1000;

    /// Milliseconds this stamp's physical part runs ahead of `now_ms`, or 0 if
    /// it does not run ahead at all.
    ///
    /// The counter plays no part: it orders stamps inside one millisecond and
    /// is not time. Reported rather than merely compared so a refusal can tell
    /// the farmer how wrong the other device's clock is.
    pub fn ahead_of(self, now_ms: i64) -> i64 {
        let physical = i64::try_from(self.physical_ms()).unwrap_or(i64::MAX);
        physical.saturating_sub(now_ms).max(0)
    }

    /// Whether this stamp is close enough to `now_ms` to be adopted — the
    /// question [`Self::MAX_SKEW_MS`] exists to answer. Inclusive at the bound.
    pub fn within_skew(self, now_ms: i64) -> bool {
        self.ahead_of(now_ms) <= Self::MAX_SKEW_MS
    }

    /// Pack two parts the caller has already range-checked.
    fn pack(physical_ms: u64, counter: u64) -> Hlc {
        debug_assert!(physical_ms <= Self::PHYSICAL_MAX && counter <= Self::COUNTER_MAX);
        Hlc((physical_ms << Self::COUNTER_BITS) | counter)
    }

    /// The stamp for a new change set, from the wall clock and `latest` — the
    /// highest value this device has seen, other devices' included.
    ///
    /// This is the HLC send rule (Kulkarni et al., §3), part by part:
    ///
    /// * the wall clock is past everything seen → its millisecond, counter 0;
    /// * otherwise → `latest`'s millisecond, and its counter plus one.
    ///
    /// Because `latest` includes stamps received from other devices, the
    /// receive rule comes for free: a device whose clock is behind still stamps
    /// after everything it has seen.
    ///
    /// **A full counter carries into the milliseconds**, on purpose. It fills
    /// only while `latest` stays ahead of the wall clock for 65 536 change sets
    /// — typically a phone whose date was set back after it had been running a
    /// year ahead, whose every write then counts on that future millisecond.
    /// Refusing would stop that phone saving records until real time caught
    /// up; carrying costs the stamp one millisecond of lead per 65 536 change
    /// sets and keeps every stamp after the last. (A PEER's clock far ahead is
    /// bounded where its stamps are received, not here — docs/sync.md,
    /// slice 2.)
    ///
    /// A clock set before 1970 counts as 0 rather than failing the write:
    /// `latest` alone still gives a value past everything seen. A clock beyond
    /// what 47 bits hold is refused, as is carrying past it.
    pub fn next(now_ms: i64, latest: Option<Hlc>) -> Result<Hlc> {
        let wall = u64::try_from(now_ms).unwrap_or(0);
        if wall > Self::PHYSICAL_MAX {
            return Err(CoreError::Stamp("clock_out_of_range"));
        }
        let Some(latest) = latest else {
            return Ok(Self::pack(wall, 0));
        };
        if wall > latest.physical_ms() {
            return Ok(Self::pack(wall, 0));
        }
        if latest.counter() < Self::COUNTER_MAX {
            return Ok(Self::pack(latest.physical_ms(), latest.counter() + 1));
        }
        let carried = latest.physical_ms() + 1;
        if carried > Self::PHYSICAL_MAX {
            return Err(CoreError::Stamp("clock_out_of_range"));
        }
        Ok(Self::pack(carried, 0))
    }
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Hlc {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hlc {
    /// Accepts only the string form. A bare number is refused rather than
    /// tolerated: if one arrives, something upstream already treated the value
    /// as a double and may have rounded it.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Hlc, D::Error> {
        let text = String::deserialize(deserializer)?;
        let raw: u64 = text.parse().map_err(serde::de::Error::custom)?;
        Hlc::from_raw(raw).map_err(serde::de::Error::custom)
    }
}

impl ToSql for Hlc {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        // Cannot fail — every constructor keeps the value at or under
        // `i64::MAX` — but the conversion is checked rather than cast, so a
        // future constructor that forgot would error instead of storing a
        // negative number that sorts before every real stamp.
        let signed = i64::try_from(self.0)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        Ok(ToSqlOutput::from(signed))
    }
}

impl FromSql for Hlc {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let signed = i64::column_result(value)?;
        u64::try_from(signed)
            .map(Hlc)
            .map_err(|_| FromSqlError::OutOfRange(signed))
    }
}

// ---------------------------------------------------------------------------
// Version vectors
// ---------------------------------------------------------------------------

/// How one version of a register stands to another — the whole of what decides
/// whether a merge has a conflict on its hands.
///
/// Read `left.compare(&right)` as a sentence about `left`: [`Causality::Newer`]
/// means left knows everything right does and more, so right can be discarded.
/// "Newer" is CAUSAL, not chronological — it says left's device had seen
/// right's version when it wrote, which no clock can tell you. That is the
/// division of labour in docs/sync.md → Hybrid logical clocks: the vector says
/// whether there is a conflict, the HLC only says which side goes live while a
/// person looks at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Causality {
    /// The same version, entry for entry. Nothing to do.
    Same,
    /// This version descends from the other: apply it, discard the other.
    Newer,
    /// The other version descends from this one: already held, change nothing.
    Older,
    /// Neither descends from the other — each device wrote without seeing the
    /// other's change. **This is a conflict**, and the only case that needs a
    /// tie-break and a person.
    Concurrent,
}

/// A register's version vector: for each device, the highest of its change
/// sets that has touched the register (docs/sync.md → Version vectors decide
/// whether there is a conflict).
///
/// A `BTreeMap` so the JSON form has its keys sorted: two devices holding the
/// same vector write the same text, which is what lets the log be compared
/// row for row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionVector(BTreeMap<String, i64>);

impl VersionVector {
    /// The entry for `device`; a device that never touched the register is 0.
    pub fn get(&self, device: &str) -> i64 {
        self.0.get(device).copied().unwrap_or(0)
    }

    /// How this vector stands to `other` — see [`Causality`].
    ///
    /// Componentwise, over every device either side names: a device absent from
    /// a vector counts as 0, so knowing about a device at all is knowing more.
    /// If each side leads on some device, neither saw the other and the result
    /// is [`Causality::Concurrent`].
    pub fn compare(&self, other: &VersionVector) -> Causality {
        let mut ahead = false;
        let mut behind = false;
        for device in self.0.keys().chain(other.0.keys()) {
            let (mine, theirs) = (self.get(device), other.get(device));
            ahead |= mine > theirs;
            behind |= theirs > mine;
            if ahead && behind {
                return Causality::Concurrent;
            }
        }
        match (ahead, behind) {
            (false, false) => Causality::Same,
            (true, false) => Causality::Newer,
            (false, true) => Causality::Older,
            (true, true) => Causality::Concurrent,
        }
    }

    /// Whether this version already includes `device`'s change set `seq`.
    ///
    /// A vector entry is a high-water mark, so everything at or below it has
    /// been seen. This is what decides whether a change set belongs to a
    /// branch's own history or to the history both branches share.
    pub fn has_seen(&self, device: &str, seq: i64) -> bool {
        self.get(device) >= seq
    }

    /// Where two versions parted: the componentwise minimum.
    ///
    /// Everything at or below this vector is history both sides hold, and
    /// everything above it is one branch's own work — which is exactly the line
    /// a rewind undoes back to and a replay applies forward from
    /// (docs/sync.md → Applying a winner needs rewind and replay).
    ///
    /// A device only one side has heard of contributes nothing, because the
    /// other side's entry for it is 0.
    pub fn common_ancestor(&self, other: &VersionVector) -> VersionVector {
        let shared = self
            .0
            .iter()
            .filter_map(|(device, &seq)| {
                let theirs = other.get(device);
                let lowest = seq.min(theirs);
                (lowest > 0).then(|| (device.clone(), lowest))
            })
            .collect();
        VersionVector(shared)
    }

    /// What every one of `vectors` had seen: [`Self::common_ancestor`] taken
    /// across all of them.
    ///
    /// This is what a reply answering several devices at once may leave out —
    /// the consolidation laptop that imports two phones and copies one file to
    /// both. Trimmed to it, the file carries everything above what the least
    /// informed of them holds, so it is complete at each (docs/sync.md → A
    /// trimmed reply is safe only for the device it answers).
    ///
    /// None at all is nothing seen, rather than the "everything" a strict
    /// minimum over no vectors would be: a reply that answers nobody carries the
    /// whole log.
    pub fn common_ancestor_of(vectors: &[VersionVector]) -> VersionVector {
        let Some((first, rest)) = vectors.split_first() else {
            return VersionVector::default();
        };
        rest.iter()
            .fold(first.clone(), |shared, next| shared.common_ancestor(next))
    }

    /// Every device this vector names, with its entry, in device order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, i64)> {
        self.0.iter().map(|(device, seq)| (device.as_str(), *seq))
    }

    /// Raise `device`'s entry to `seq`. Never lowers it: a vector only grows.
    pub fn observe(&mut self, device: &str, seq: i64) {
        let entry = self.0.entry(device.to_owned()).or_insert(0);
        *entry = (*entry).max(seq);
    }

    /// Componentwise maximum — the vector of a state that has seen both.
    pub fn merge(&mut self, other: &VersionVector) {
        for (device, seq) in &other.0 {
            self.observe(device, *seq);
        }
    }

    /// The form stored in `record_change.version_vector`.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Read a stored vector back.
    pub fn from_json(text: &str) -> Result<VersionVector> {
        Ok(serde_json::from_str(text)?)
    }
}

// ---------------------------------------------------------------------------
// The aggregate map
// ---------------------------------------------------------------------------

/// What the merge does with one table (docs/sync.md → The aggregate map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRole {
    /// A register of its own, identified by its row id.
    Root,
    /// Part of another register, merged with it as one statement: `root` is
    /// that register's table and `fk` the column naming its row. Treated plots,
    /// problems and justifications are children of their treatment record.
    Child {
        root: &'static str,
        fk: &'static str,
    },
    /// A register identified by a slot rather than a row: every row sharing
    /// these column values, over time, is one register, and its `root_id` is
    /// [`slot_id`] over them in this order.
    ///
    /// For the tables whose UNIQUE index allows one live row per slot and whose
    /// writes replace that row. Two devices filling the slot offline then write
    /// two versions of one register, which the merge can see and show a person,
    /// instead of two unrelated rows the second apply cannot insert.
    Slot { columns: &'static [&'static str] },
    /// Never logged, never merged: derived on each device (`sync_conflict`),
    /// or device-local by design (the reference catalogues).
    Local,
}

/// One table's entry in the aggregate map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSync {
    pub table: &'static str,
    pub role: SyncRole,
}

impl TableSync {
    pub const fn root(table: &'static str) -> Self {
        TableSync {
            table,
            role: SyncRole::Root,
        }
    }

    pub const fn child(table: &'static str, root: &'static str, fk: &'static str) -> Self {
        TableSync {
            table,
            role: SyncRole::Child { root, fk },
        }
    }

    pub const fn slot(table: &'static str, columns: &'static [&'static str]) -> Self {
        TableSync {
            table,
            role: SyncRole::Slot { columns },
        }
    }

    pub const fn local(table: &'static str) -> Self {
        TableSync {
            table,
            role: SyncRole::Local,
        }
    }
}

/// The `root_id` of a slot-keyed register: its slot's values as a JSON array,
/// in the order the table's [`SyncRole::Slot`] lists the columns.
///
/// JSON rather than a delimiter-joined string so that no value can contain the
/// delimiter, and so a NULL (one side of `geo_feature`'s farm-or-plot arc) is
/// `null` and never an empty string that another value could also produce.
pub fn slot_id(values: &[serde_json::Value]) -> String {
    serde_json::Value::Array(values.to_vec()).to_string()
}

/// The `root_id` of the slot-keyed register `row` belongs to, read off the same
/// serialised image the log stores.
///
/// Write paths use this with the SAME column constant their table's
/// [`SyncRole::Slot`] declares, rather than rebuilding the key from whatever
/// variables are in scope: the declaration and the stamp then cannot disagree,
/// and a merge deriving the key from a logged payload gets the identical
/// string. A column the image lacks is refused — the payload is required to
/// carry every column, so a missing one is a defect, not a NULL.
pub fn slot_id_of<T: Serialize>(row: &T, columns: &[&str]) -> Result<String> {
    let image = serde_json::to_value(row)?;
    let values = columns
        .iter()
        .map(|column| {
            image
                .get(column)
                .cloned()
                .ok_or(CoreError::Stamp("slot_column_missing"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(slot_id(&values))
}

/// `farm_advisor`'s slot: one live link per farm and advisor.
pub const FARM_ADVISOR_SLOT: &[&str] = &["farm_id", "advisor_id"];
/// `geo_feature`'s slot: one live geometry per subject, role and source. Both
/// arms of the farm-or-plot arc are in it, so a farm outline and a plot
/// outline can never share one.
pub const GEO_FEATURE_SLOT: &[&str] = &["farm_id", "plot_id", "role", "source"];
/// `plot_zone_flag`'s slot: exactly its UNIQUE index, because a check replaces
/// one zone type at a time. One provider check therefore writes several
/// registers in one change set — and a conflict over one zone type cannot roll
/// back another device's fresh result for a different one.
pub const ZONE_FLAG_SLOT: &[&str] = &["plot_id", "zone_type_code", "campaign", "source"];
/// `plot_water_declaration`'s slot: one live declaration per plot.
pub const WATER_DECLARATION_SLOT: &[&str] = &["plot_id"];
/// `export_alias`'s slot: one alias per exported entry. The other UNIQUE,
/// (target, alias), is kept from being filled twice by one submitting device
/// per farm (docs/sync.md → `export_alias` collisions).
pub const EXPORT_ALIAS_SLOT: &[&str] = &["target", "entity_table", "entity_id", "split_key"];

/// Core's half of the aggregate map. Every module declares its own through the
/// shell's `Module` trait, and the shell's contract test refuses a table that
/// nobody declared — the same division as the backup shape.
///
/// Lookup tables keyed by `code` are not listed: they are seeded by migrations
/// and therefore identical on two devices at one schema version, which delta
/// exchange requires anyway. Neither is `record_change`, which is the log
/// itself rather than something logged.
pub const CORE_SYNC_SHAPE: &[TableSync] = &[
    TableSync::root("farm"),
    TableSync::child("farm_es_extension", "farm", "farm_id"),
    TableSync::child("farm_representative", "farm", "farm_id"),
    // A season is a register of its own. Its (farm, label) UNIQUE is NOT a
    // slot, because the label is edited: its id is derived from the farm and
    // dates, so two devices opening one campaign with the same dates write one
    // register, and a name two books reach apart is refused at import for a
    // person to settle (docs/sync.md → Seasons created on two devices).
    TableSync::root("season"),
    TableSync::root("plot"),
    TableSync::child("plot_es_extension", "plot", "plot_id"),
    TableSync::root("crop"),
    TableSync::root("operator"),
    TableSync::root("user_profile"),
    TableSync::root("advisor"),
    TableSync::slot("farm_advisor", FARM_ADVISOR_SLOT),
    TableSync::root("machinery"),
    TableSync::child("machinery_es_extension", "machinery", "machinery_id"),
    TableSync::root("premises"),
    TableSync::child("premises_es_extension", "premises", "premises_id"),
    TableSync::slot("geo_feature", GEO_FEATURE_SLOT),
    TableSync::slot("plot_zone_flag", ZONE_FLAG_SLOT),
    TableSync::root("plot_water_point"),
    TableSync::slot("plot_water_declaration", WATER_DECLARATION_SLOT),
    TableSync::root("sowing_record"),
    TableSync::child("sowing_plot", "sowing_record", "sowing_record_id"),
    TableSync::root("harvest_record"),
    TableSync::child("harvest_plot", "harvest_record", "harvest_record_id"),
    TableSync::slot("export_alias", EXPORT_ALIAS_SLOT),
    TableSync::root("sync_peer"),
    // What a person said about an alert, which no device can re-derive. A
    // register per act, never updated, so two devices acting on one alert
    // never write the same register and can never conflict. The alerts
    // themselves are not a table at all: each crate works out its own when
    // the list is read.
    TableSync::root("alert_acknowledgement"),
    // What a person said about two records that looked like one — the same
    // shape for the same reason: a register per act, so two devices judging
    // one pair never conflict. The suspicions are not a table: each rule is
    // run when the list is read.
    TableSync::root("duplicate_verdict"),
    // A register erased for good, and the version that removed it — one row
    // per register, never updated, so two devices erasing one register never
    // conflict, and every stamp reads it as a marker (docs/sync.md → The
    // purge, as settled).
    TableSync::root("purged_register"),
    // Which holding THIS device syncs within — device-local by definition: a
    // group that travelled would be a group nobody had to agree to join.
    TableSync::local("sync_group"),
    TableSync::local("catalogue"),
    TableSync::local("catalogue_code"),
    // Derived from the log on each device: every device computes
    // the same heads from the same rows, so there is nothing to send.
    TableSync::local("sync_conflict"),
    // What this database has held of each device, and what this device knows
    // each device holds — both about THIS file and device, so neither travels
    // as a register: the second is carried in every manifest instead.
    TableSync::local("sync_held"),
    TableSync::local("sync_known"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: &str = "0192f3a4-0000-7000-8000-00000000000a";

    // --- device identity ---------------------------------------------------

    #[test]
    fn a_minted_device_id_is_a_device_id() {
        assert!(is_device_id(&mint_device_id()));
    }

    #[test]
    fn only_the_canonical_spelling_is_a_device_id() {
        assert!(is_device_id(DEVICE));
        for spelling in [
            "0192F3A4-0000-7000-8000-00000000000A",
            "{0192f3a4-0000-7000-8000-00000000000a}",
            "urn:uuid:0192f3a4-0000-7000-8000-00000000000a",
            // The "simple" form, which the uuid crate itself parses happily.
            "0192f3a400007000800000000000000a",
            "",
            "laptop",
        ] {
            assert!(!is_device_id(spelling), "{spelling:?} is not canonical");
        }
    }

    #[test]
    fn a_connection_with_no_identity_says_so() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(matches!(
            installed_device(&conn),
            Err(CoreError::Stamp("no_device_identity"))
        ));
    }

    #[test]
    fn an_installed_identity_reads_back_and_a_second_install_replaces_it() {
        let conn = Connection::open_in_memory().unwrap();
        install_device(&conn, DEVICE).unwrap();
        assert_eq!(installed_device(&conn).unwrap(), DEVICE);

        let other = mint_device_id();
        install_device(&conn, &other).unwrap();
        assert_eq!(installed_device(&conn).unwrap(), other);
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM temp.sync_device", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "replaced, not appended");
    }

    #[test]
    fn a_malformed_identity_is_refused_and_installs_nothing() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(matches!(
            install_device(&conn, "laptop"),
            Err(CoreError::Stamp("invalid_device_id"))
        ));
        assert!(installed_device(&conn).is_err());
    }

    #[test]
    fn the_identity_is_not_part_of_the_database_file() {
        // The property the TEMP table was chosen for: a snapshot of the file
        // must not carry one device's name onto another machine.
        let conn = Connection::open_in_memory().unwrap();
        install_device(&conn, DEVICE).unwrap();
        let in_main: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'sync_device')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!in_main);
    }

    // --- hybrid logical clock ----------------------------------------------

    const NOW: i64 = 1_789_000_000_000; // 2026-09-10, in ms

    fn at(ms: u64, counter: u64) -> Hlc {
        Hlc::from_raw((ms << Hlc::COUNTER_BITS) | counter).unwrap()
    }

    #[test]
    fn the_first_stamp_is_the_wall_clock_with_a_zero_counter() {
        let first = Hlc::next(NOW, None).unwrap();
        assert_eq!(first.physical_ms(), NOW.unsigned_abs());
        assert_eq!(first.counter(), 0);
    }

    // --- how far ahead a received stamp may be -----------------------------
    //
    // docs/sync.md → "How far ahead a stamp may be": ε = 2 hours, because a
    // clock set to Spanish local time thinking it is UTC lands at +1 or +2.

    #[test]
    fn a_stamp_at_or_behind_the_clock_is_not_ahead_at_all() {
        assert_eq!(at(NOW.unsigned_abs(), 0).ahead_of(NOW), 0);
        assert_eq!(at(NOW.unsigned_abs() - 60_000, 0).ahead_of(NOW), 0);
        // The counter is not time: a full counter on this millisecond is still
        // this millisecond.
        assert_eq!(at(NOW.unsigned_abs(), Hlc::COUNTER_MAX).ahead_of(NOW), 0);
    }

    #[test]
    fn a_stamp_ahead_reports_the_milliseconds_it_leads_by() {
        let ahead = at(NOW.unsigned_abs() + 90_000, 0);
        assert_eq!(ahead.ahead_of(NOW), 90_000);
    }

    #[test]
    fn two_hours_is_the_bound_and_it_is_inclusive() {
        let epsilon = Hlc::MAX_SKEW_MS;
        assert_eq!(epsilon, 2 * 60 * 60 * 1000);
        // The mistake worth absorbing: a clock set to UTC+1 or UTC+2.
        for offset in [60 * 60 * 1000, epsilon] {
            let stamp = at(NOW.unsigned_abs() + offset.unsigned_abs(), 0);
            assert!(
                stamp.within_skew(NOW),
                "a clock {offset} ms ahead must still sync"
            );
        }
        let beyond = at(NOW.unsigned_abs() + epsilon.unsigned_abs() + 1, 0);
        assert!(!beyond.within_skew(NOW));
    }

    #[test]
    fn a_clock_set_years_ahead_is_refused() {
        let year_ms = 365 * 24 * 60 * 60 * 1000u64;
        let wrong = at(NOW.unsigned_abs() + 4 * year_ms, 0);
        assert!(!wrong.within_skew(NOW));
        assert_eq!(wrong.ahead_of(NOW), (4 * year_ms) as i64);
    }

    #[test]
    fn a_clock_before_the_epoch_never_makes_a_stamp_look_ahead() {
        // `next` already floors a pre-1970 clock at 0; a receiver whose own
        // clock is negative must not conclude every stamp is unusable.
        assert_eq!(at(0, 0).ahead_of(-5_000), 5_000);
        assert!(at(0, 0).within_skew(-5_000));
    }

    #[test]
    fn a_later_clock_wins_and_resets_the_counter() {
        let seen = at(NOW.unsigned_abs() - 5, 3);
        let next = Hlc::next(NOW, Some(seen)).unwrap();
        assert_eq!(
            (next.physical_ms(), next.counter()),
            (NOW.unsigned_abs(), 0)
        );
    }

    #[test]
    fn within_one_millisecond_the_counter_orders_the_stamps() {
        let first = Hlc::next(NOW, None).unwrap();
        let second = Hlc::next(NOW, Some(first)).unwrap();
        assert!(second > first);
        assert_eq!(second.physical_ms(), first.physical_ms());
        assert_eq!(second.counter(), 1);
    }

    #[test]
    fn a_clock_that_jumps_backwards_does_not_move_the_stamp_back() {
        // An NTP correction, or somebody fixing the phone's date: the stamp
        // must still follow everything this device has already seen.
        let seen = Hlc::next(NOW, None).unwrap();
        let after_the_jump = Hlc::next(NOW - 60_000, Some(seen)).unwrap();
        assert!(after_the_jump > seen);
        assert_eq!(after_the_jump.physical_ms(), seen.physical_ms());
    }

    #[test]
    fn a_stamp_received_from_a_device_ahead_is_followed() {
        // The receive rule, carried by `latest` including other devices' rows.
        let from_the_future = at(NOW.unsigned_abs() + 3_600_000, 7);
        let next = Hlc::next(NOW, Some(from_the_future)).unwrap();
        assert!(next > from_the_future);
    }

    #[test]
    fn a_full_counter_carries_into_the_milliseconds() {
        let full = at(NOW.unsigned_abs(), (1 << Hlc::COUNTER_BITS) - 1);
        let next = Hlc::next(NOW, Some(full)).unwrap();
        assert!(next > full);
        assert_eq!(
            (next.physical_ms(), next.counter()),
            (NOW.unsigned_abs() + 1, 0)
        );
    }

    /// The HLC send rule as Kulkarni et al. state it (§3, "send or local
    /// event"), on separate parts and with an unbounded counter — the
    /// reference the packed implementation is held to.
    fn textbook(wall: u64, latest: (u64, u64)) -> (u64, u64) {
        let (l, c) = latest;
        let next_l = l.max(wall);
        if next_l == l {
            (next_l, c + 1)
        } else {
            (next_l, 0)
        }
    }

    #[test]
    fn it_is_the_textbook_rule_whenever_the_counter_has_room() {
        // Every ordering of wall clock against what was seen — behind, level,
        // ahead — at counters from empty to one short of full.
        let base = NOW.unsigned_abs();
        for wall in [base - 1_000, base - 1, base, base + 1, base + 1_000] {
            for counter in [0, 1, 7, 40_000, Hlc::COUNTER_MAX - 1] {
                let latest = at(base, counter);
                let wall_ms = i64::try_from(wall).unwrap();
                let ours = Hlc::next(wall_ms, Some(latest)).unwrap();
                assert_eq!(
                    (ours.physical_ms(), ours.counter()),
                    textbook(wall, (base, counter)),
                    "wall {wall}, latest ({base}, {counter})"
                );
            }
        }
    }

    #[test]
    fn a_phone_whose_date_was_set_back_keeps_saving() {
        // The case the carry exists for. The phone ran a year ahead, wrote,
        // and then had its date corrected: every stamp from now on counts on
        // that future millisecond. More change sets than the counter holds
        // must neither fail nor ever step backwards.
        let year_ms: u64 = 365 * 24 * 3600 * 1000;
        let mut latest = at(NOW.unsigned_abs() + year_ms, 0);
        for _ in 0..70_000 {
            let next = Hlc::next(NOW, Some(latest)).unwrap();
            assert!(next > latest);
            latest = next;
        }
        assert_eq!(
            latest.physical_ms(),
            NOW.unsigned_abs() + year_ms + 1,
            "one carry: 70 000 stamps is a millisecond of lead, no more"
        );
    }

    #[test]
    fn the_counter_mask_is_the_low_sixteen_bits() {
        // Two spellings of the same mask; the constant is evaluated at compile
        // time, where an out-of-range shift is an error rather than a panic.
        assert_eq!(Hlc::COUNTER_MAX, 0xFFFF);
        assert_eq!(
            Hlc::COUNTER_MAX,
            u64::MAX >> (u64::BITS - Hlc::COUNTER_BITS)
        );
        assert_eq!(Hlc::MAX, i64::MAX.unsigned_abs());
    }

    #[test]
    fn a_clock_before_1970_still_stamps_past_what_was_seen() {
        let seen = Hlc::next(NOW, None).unwrap();
        assert!(Hlc::next(-1, Some(seen)).unwrap() > seen);
        assert_eq!(Hlc::next(-1, None).unwrap().raw(), 0);
    }

    #[test]
    fn bit_63_is_never_set() {
        // 2^47 ms is the first clock reading the encoding cannot hold.
        let last_ms = (1_i64 << 47) - 1;
        let last = Hlc::next(last_ms, None).unwrap();
        assert!(i64::try_from(last.raw()).is_ok());
        assert!(matches!(
            Hlc::next(1_i64 << 47, None),
            Err(CoreError::Stamp("clock_out_of_range"))
        ));
        assert!(matches!(
            Hlc::next(NOW, Some(Hlc::from_raw(Hlc::MAX).unwrap())),
            Err(CoreError::Stamp("clock_out_of_range"))
        ));
        assert!(Hlc::from_raw(Hlc::MAX + 1).is_err());
    }

    #[test]
    fn integer_order_is_milliseconds_then_counter() {
        assert!(at(10, 65_535) < at(11, 0));
        assert!(at(10, 1) < at(10, 2));
    }

    #[test]
    fn it_serialises_as_a_string_and_refuses_a_number() {
        let stamp = at(NOW.unsigned_abs(), 5);
        let json = serde_json::to_string(&stamp).unwrap();
        assert_eq!(json, format!("\"{}\"", stamp.raw()));
        assert_eq!(serde_json::from_str::<Hlc>(&json).unwrap(), stamp);

        // The value is past 2^53, so as a JSON number JavaScript would round it.
        assert!(stamp.raw() > 1 << 53);
        assert!(serde_json::from_str::<Hlc>(&stamp.raw().to_string()).is_err());
        assert!(serde_json::from_str::<Hlc>(&format!("\"{}\"", Hlc::MAX + 1)).is_err());
    }

    #[test]
    fn it_round_trips_through_sqlite_as_a_non_negative_integer() {
        let conn = Connection::open_in_memory().unwrap();
        let stamp = at(NOW.unsigned_abs(), 9);
        let (back, kind): (Hlc, String) = conn
            .query_row("SELECT ?1, typeof(?1)", [stamp], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(back, stamp);
        assert_eq!(kind, "integer");
        assert!(
            conn.query_row("SELECT CAST(-1 AS INTEGER)", [], |r| r.get::<_, Hlc>(0))
                .is_err()
        );
    }

    // --- version vectors ---------------------------------------------------

    fn vv(entries: &[(&str, i64)]) -> VersionVector {
        let mut vector = VersionVector::default();
        for (device, seq) in entries {
            vector.observe(device, *seq);
        }
        vector
    }

    #[test]
    fn an_unseen_device_is_zero() {
        assert_eq!(VersionVector::default().get("a"), 0);
    }

    #[test]
    fn observing_never_lowers_an_entry() {
        let mut vector = vv(&[("a", 5)]);
        vector.observe("a", 3);
        assert_eq!(vector.get("a"), 5);
    }

    #[test]
    fn merging_takes_the_componentwise_maximum() {
        let mut left = vv(&[("a", 5), ("b", 1)]);
        left.merge(&vv(&[("b", 4), ("c", 2)]));
        assert_eq!(left, vv(&[("a", 5), ("b", 4), ("c", 2)]));
    }

    #[test]
    fn the_stored_form_has_sorted_keys_whatever_the_insertion_order() {
        let one = vv(&[("b", 2), ("a", 1)]);
        let other = vv(&[("a", 1), ("b", 2)]);
        assert_eq!(one.to_json().unwrap(), r#"{"a":1,"b":2}"#);
        assert_eq!(one.to_json().unwrap(), other.to_json().unwrap());
        assert_eq!(VersionVector::from_json(r#"{"a":1,"b":2}"#).unwrap(), one);
    }

    // --- causality ---------------------------------------------------------
    //
    // The four cases of docs/sync.md → "Version vectors decide whether there is
    // a conflict": E ≥ L applies, L ≥ E is already held, neither is a conflict.

    #[test]
    fn identical_vectors_are_the_same_version() {
        assert_eq!(vv(&[("a", 2)]).compare(&vv(&[("a", 2)])), Causality::Same);
        assert_eq!(
            VersionVector::default().compare(&VersionVector::default()),
            Causality::Same
        );
    }

    #[test]
    fn a_vector_ahead_on_every_device_is_newer() {
        assert_eq!(
            vv(&[("a", 3), ("b", 2)]).compare(&vv(&[("a", 1), ("b", 2)])),
            Causality::Newer
        );
        assert_eq!(
            vv(&[("a", 1), ("b", 2)]).compare(&vv(&[("a", 3), ("b", 2)])),
            Causality::Older
        );
    }

    #[test]
    fn a_device_the_other_side_never_saw_counts_as_zero() {
        // Knowing about `b` at all is knowing more, since an absent entry is 0.
        assert_eq!(
            vv(&[("a", 1), ("b", 1)]).compare(&vv(&[("a", 1)])),
            Causality::Newer
        );
        assert_eq!(
            VersionVector::default().compare(&vv(&[("a", 1)])),
            Causality::Older
        );
    }

    #[test]
    fn each_side_ahead_on_a_different_device_is_a_conflict() {
        assert_eq!(
            vv(&[("a", 2), ("b", 1)]).compare(&vv(&[("a", 1), ("b", 2)])),
            Causality::Concurrent
        );
        // Disjoint devices are the same thing: each knows one the other does not.
        assert_eq!(
            vv(&[("a", 1)]).compare(&vv(&[("b", 1)])),
            Causality::Concurrent
        );
    }

    #[test]
    fn a_vector_has_seen_every_set_up_to_its_entry() {
        let seen = vv(&[("a", 3)]);
        assert!(seen.has_seen("a", 1) && seen.has_seen("a", 3));
        assert!(!seen.has_seen("a", 4));
        assert!(!seen.has_seen("b", 1), "a device it never heard of");
    }

    #[test]
    fn the_common_ancestor_is_what_both_sides_had_seen() {
        // Where the two branches parted: everything at or below this is shared
        // history, everything above it is one branch's own.
        let ours = vv(&[("a", 5), ("b", 2)]);
        let theirs = vv(&[("a", 2), ("b", 7), ("c", 1)]);
        assert_eq!(ours.common_ancestor(&theirs), vv(&[("a", 2), ("b", 2)]));
        assert_eq!(
            theirs.common_ancestor(&ours),
            ours.common_ancestor(&theirs),
            "it cannot depend on which side asks"
        );
    }

    #[test]
    fn a_device_only_one_side_knows_is_not_shared_history() {
        assert_eq!(
            vv(&[("a", 1)]).common_ancestor(&vv(&[("b", 1)])),
            VersionVector::default()
        );
    }

    #[test]
    fn the_ancestor_of_a_version_and_its_descendant_is_the_older_one() {
        let older = vv(&[("a", 2)]);
        let newer = vv(&[("a", 5), ("b", 1)]);
        assert_eq!(newer.common_ancestor(&older), older);
    }

    #[test]
    fn the_ancestor_of_several_is_what_every_one_had_seen() {
        // Three phones read by one laptop: a reply to all of them may leave out
        // only what the least informed held, device by device.
        let phones = [
            vv(&[("laptop", 4), ("p", 3), ("q", 1)]),
            vv(&[("laptop", 2), ("p", 3), ("q", 5)]),
            vv(&[("laptop", 6), ("p", 1), ("q", 5), ("r", 2)]),
        ];
        assert_eq!(
            VersionVector::common_ancestor_of(&phones),
            vv(&[("laptop", 2), ("p", 1), ("q", 1)]),
            "and a device one of them has never heard of is not shared at all"
        );

        let mut reordered = phones.clone();
        reordered.reverse();
        assert_eq!(
            VersionVector::common_ancestor_of(&reordered),
            VersionVector::common_ancestor_of(&phones),
            "it cannot depend on which phone was imported first"
        );
    }

    #[test]
    fn the_ancestor_of_one_vector_is_that_vector() {
        // One import, one reply: exactly the precise reply there was before a
        // session could answer several.
        let only = vv(&[("a", 3), ("b", 1)]);
        assert_eq!(
            VersionVector::common_ancestor_of(std::slice::from_ref(&only)),
            only
        );
    }

    #[test]
    fn the_ancestor_of_no_vectors_is_nothing_seen() {
        // A reply that answers nobody — no import yet this session — carries
        // the whole log, never nothing.
        assert_eq!(
            VersionVector::common_ancestor_of(&[]),
            VersionVector::default()
        );
    }

    #[test]
    fn a_vector_lists_its_entries_in_device_order() {
        let seen = vv(&[("b", 2), ("a", 5)]);
        assert_eq!(seen.iter().collect::<Vec<_>>(), vec![("a", 5), ("b", 2)]);
    }

    #[test]
    fn comparison_is_symmetric() {
        let pairs = [
            (vv(&[("a", 1)]), vv(&[("a", 1)])),
            (vv(&[("a", 2)]), vv(&[("a", 1)])),
            (vv(&[("a", 2), ("b", 1)]), vv(&[("a", 1), ("b", 2)])),
        ];
        for (left, right) in pairs {
            let mirrored = match left.compare(&right) {
                Causality::Same => Causality::Same,
                Causality::Newer => Causality::Older,
                Causality::Older => Causality::Newer,
                Causality::Concurrent => Causality::Concurrent,
            };
            assert_eq!(right.compare(&left), mirrored);
        }
    }

    // --- aggregate map -----------------------------------------------------

    #[test]
    fn a_slot_id_is_a_json_array_and_null_is_not_empty() {
        use serde_json::json;
        assert_eq!(
            slot_id(&[json!("p1"), json!(2026), json!("sigpac")]),
            r#"["p1",2026,"sigpac"]"#
        );
        assert_ne!(
            slot_id(&[json!(null), json!("p1")]),
            slot_id(&[json!(""), json!("p1")])
        );
        // No delimiter can be smuggled into a value.
        assert_ne!(
            slot_id(&[json!("a,b"), json!("c")]),
            slot_id(&[json!("a"), json!("b,c")])
        );
    }

    #[test]
    fn core_declares_each_table_once() {
        let mut names: Vec<&str> = CORE_SYNC_SHAPE.iter().map(|t| t.table).collect();
        let declared = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), declared);
    }
}
