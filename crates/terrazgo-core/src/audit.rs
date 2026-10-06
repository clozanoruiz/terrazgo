// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Append-only `record_change` helpers — the sync delta source, and the
//! history the reviews read. No law asks for it: RD 1311/2012 art. 16.3's three
//! years are for the entries, not their history (docs/cuaderno-print.md → What
//! the law asks of the entries over time). Public: every crate that writes synced user data (the core
//! repository, every module's) logs through these, inside the same
//! transaction as the write itself.
//!
//! # Stamping a write
//!
//! Every log row carries values computed ONCE per transaction and shared by
//! every row in it — the device, its change-set number, the clock — plus the
//! register the row belongs to (docs/sync.md → The change stamp). So a write
//! that logs opens its transaction through [`begin`], which opens the change
//! set with it, and stamps each register before logging to it:
//!
//! ```text
//! let tx = audit::begin(conn, actor)?;
//! let stamp = tx.register("treatment_record", &record.id, Some(&record.season_id))?;
//! audit::log_insert(&tx, &stamp, "treatment_plot", &plot.id, &plot)?;
//! audit::log_insert(&tx, &stamp, "treatment_record", &record.id, &record)?;
//! tx.commit()?;
//! ```
//!
//! A transaction that changes a SECOND register — recording a seed treatment
//! withdraws the season's "no seed treatments" declaration — registers that
//! one too, and a helper that writes a whole register on a caller's behalf
//! takes the [`WriteTx`] and registers its own. One transaction is one change
//! set however many registers it touches; each register gets its own version
//! vector.
//!
//! **The rules are carried by the types, not by this comment:**
//!
//! * *One change set per transaction.* The change set is born with the
//!   transaction in [`begin`] and there is no other way to make one. A second
//!   would need a second [`WriteTx`], which needs the connection the first
//!   still holds — the compiler refuses.
//! * *Every log row names its register.* The log helpers take a
//!   [`ChangeStamp`], whose fields are private and whose only constructor,
//!   [`WriteTx::register`], names the register. Only a whole log call can be
//!   forgotten, which every crate's repository tests already catch.
//! * *A stamp dies with its transaction.* It borrows the [`WriteTx`], which
//!   `commit()` consumes, so using a stamp afterwards does not compile.
//! * *The register named is the right one.* Checked on every row as it is
//!   written — see [`write_change`].

use std::ops::Deref;

use rusqlite::{Connection, Transaction};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::sync::{self, Hlc, VersionVector};

/// The highest clock value this log holds, other devices' included — which is
/// what makes the hybrid clock follow what it has seen. A seek on
/// `idx_record_change_hlc`.
const LATEST_HLC_SQL: &str = "SELECT MAX(hlc) FROM record_change";

/// Every distinct vector a register has been written with — and the version
/// that removed it, wherever the purge erased it. A seek on
/// `idx_record_change_root`, bounded by how often THIS register was edited —
/// never by the size of the log — and one on `idx_purged_register_root`.
/// DISTINCT because every row of one change set repeats one vector.
///
/// **The second half is the marker** (docs/sync.md → It travels as a row per
/// register — and the row is a marker). A slot keyed by a book — a declaration
/// made again — writes the same register again after it was erased, on a
/// device that holds nothing of its history; stamped on top of the version that
/// removed it, the new write is newer than the deletion everywhere rather than
/// a rival to it. **And on top of the purge itself**: the marker's own change
/// set joins the vector, which is how an import tells a write made knowing the
/// register was purged — kept — from one made where the purge had not arrived,
/// which the purge wins over (docs/sync.md → An import meets the purge).
const REGISTER_VECTORS_SQL: &str = "SELECT version_vector FROM record_change
     WHERE root_table = ?1 AND root_id = ?2
     UNION
     SELECT json_set(p.removal, '$.\"' || c.origin_device || '\"', c.origin_seq)
     FROM purged_register p
     JOIN record_change c ON c.root_table = 'purged_register' AND c.root_id = p.id
     WHERE p.root_table = ?1 AND p.root_id = ?2";

/// A register's waiting-for-review rows, which a new head makes untrue.
/// A seek on `sync_conflict`'s primary key, against a table that is empty on
/// almost every device almost always.
const CLEAR_REGISTER_CONFLICT_SQL: &str =
    "DELETE FROM sync_conflict WHERE root_table = ?1 AND root_id = ?2";

/// One row of the append-only log. The most-run write statement in the app:
/// every logged row of every transaction goes through this one.
const WRITE_CHANGE_SQL: &str = "INSERT INTO record_change
       (id, entity_table, entity_id, season_id, operation, changed_at, actor, payload,
        root_table, root_id, origin_device, origin_seq, version_vector, hlc)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

/// What one transaction's log rows share: which device wrote them, that
/// device's change-set number, the clock, the instant and the author. Private:
/// it exists only inside a [`WriteTx`].
#[derive(Debug)]
struct ChangeSet {
    device: String,
    seq: i64,
    hlc: Hlc,
    changed_at: String,
    actor: Option<String>,
}

/// A write transaction and its change set, opened together by [`begin`].
///
/// It is a `Transaction` for every SQL purpose: `Deref` lets `tx.execute(…)`
/// and any function taking `&Transaction` accept it unchanged. What it adds is
/// the change set, and [`WriteTx::register`] — the only way to get the stamp
/// a log call needs. It deliberately does NOT hand out `&mut Transaction`, so
/// no savepoint can open a second scope inside it.
///
/// Dropping it without [`WriteTx::commit`] rolls the transaction back, like a
/// `Transaction`.
#[derive(Debug)]
pub struct WriteTx<'conn> {
    tx: Transaction<'conn>,
    set: ChangeSet,
}

/// A [`WriteTx`]'s change set applied to one register: what every log call
/// takes.
///
/// `season_id` belongs here rather than on each log call because a register's
/// children are in its season — a treated plot is logged under its treatment
/// record's campaign.
///
/// Used inside its transaction, it is an ordinary value:
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use terrazgo_core::audit;
/// let mut conn = terrazgo_core::open_in_memory()?;
/// let tx = audit::begin(&mut conn, None)?;
/// let stamp = tx.register("farm", "f1", None)?;
/// audit::log_insert(&tx, &stamp, "farm", "f1", &serde_json::json!({ "id": "f1" }))?;
/// tx.commit()?;
/// # Ok(()) }
/// ```
///
/// Carried past `commit()` into a later transaction — where its change-set
/// number would name a second change set — it does not compile: the stamp
/// still borrows the first transaction, which `commit()` consumes (E0505).
///
/// ```compile_fail,E0505
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use terrazgo_core::audit;
/// let mut conn = terrazgo_core::open_in_memory()?;
/// let tx = audit::begin(&mut conn, None)?;
/// let stamp = tx.register("farm", "f1", None)?;
/// tx.commit()?;
/// let later = audit::begin(&mut conn, None)?;
/// audit::log_insert(&later, &stamp, "farm", "f1", &serde_json::json!({ "id": "f1" }))?;
/// # Ok(()) }
/// ```
///
/// And a second change set cannot be opened inside a transaction: the first
/// [`WriteTx`] holds the connection mutably for as long as it lives (E0499).
///
/// ```compile_fail,E0499
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use terrazgo_core::audit;
/// let mut conn = terrazgo_core::open_in_memory()?;
/// let tx = audit::begin(&mut conn, None)?;
/// let second = audit::begin(&mut conn, None)?;
/// tx.commit()?;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct ChangeStamp<'tx> {
    set: &'tx ChangeSet,
    root_table: String,
    root_id: String,
    season_id: Option<String>,
    /// The register's vector after this write, already serialised: every row
    /// the stamp logs repeats it.
    version_vector: String,
}

/// Open a write transaction and its change set — the one way a write that logs
/// begins.
///
/// `actor` is the author stamp: the `user_profile.id` of the device's active
/// profile at write time, passed down from the shell. `None` means "recorded
/// with no active profile" — including every row written before profiles
/// existed. The id is stamped verbatim, never validated here: profiles are
/// soft-deleted only, so it resolves at inspection time, and a claim from a
/// foreign device must survive sync even if this device can't resolve it yet.
///
/// Fails with `Stamp("no_device_identity")` on a connection nobody installed a
/// device on (see [`sync::install_device`]) — loudly, because a row with no
/// origin could never be merged — and with `Stamp("no_sync_shape")` on one
/// without the aggregate map every logged row is checked against
/// ([`sync::install_shape`]).
///
/// A transaction that ends up logging nothing (a write refused by validation,
/// an alias that already existed) consumes no change-set number: the number is
/// the log's maximum plus one, and nothing was logged.
pub fn begin<'conn>(conn: &'conn mut Connection, actor: Option<&str>) -> Result<WriteTx<'conn>> {
    let tx = conn.transaction()?;
    let device = sync::installed_device(&tx)?;
    if !sync::shape_installed(&tx)? {
        return Err(CoreError::Stamp("no_sync_shape"));
    }
    // Cached, like every statement a write runs on the way to the log: parsing
    // and planning them afresh on every write cost more than running them.
    // Past everything this database has held from itself, not only what the
    // log still holds: the purge may have taken this device's last change set
    // out, and its number must not name a second one.
    let last_seq = sync::held_through(&tx, &device)?;
    let latest: Option<Hlc> =
        crate::sql::cached_statement(&tx, LATEST_HLC_SQL)?.query_row([], |r| r.get(0))?;
    let set = ChangeSet {
        device,
        seq: last_seq + 1,
        hlc: Hlc::next(crate::date::now_ms(), latest)?,
        changed_at: now_utc_iso(),
        actor: actor.map(str::to_owned),
    };
    Ok(WriteTx { tx, set })
}

impl<'conn> Deref for WriteTx<'conn> {
    type Target = Transaction<'conn>;

    fn deref(&self) -> &Transaction<'conn> {
        &self.tx
    }
}

impl WriteTx<'_> {
    /// The stamp for one register this transaction writes.
    ///
    /// `root_table`/`root_id` name the REGISTER, not necessarily the row about
    /// to be logged: a treated plot's register is its treatment record. For a
    /// slot-keyed register (`SyncRole::Slot`) `root_id` is `sync::slot_id` over
    /// the slot's values — `sync::slot_id_of` reads them off a row.
    ///
    /// Computes the register's version vector: every vector it has been
    /// written with, and the version that removed it if the purge erased it
    /// (the marker), merged, and raised to this change set. Merging them all —
    /// rather than taking the newest row — is what makes a write on a device
    /// holding two conflicting versions dominate BOTH, which is how resolving a
    /// conflict closes it everywhere (docs/sync.md → Conflicts as the person
    /// sees them). Registering one register twice gives the same stamp.
    pub fn register(
        &self,
        root_table: &str,
        root_id: &str,
        season_id: Option<&str>,
    ) -> Result<ChangeStamp<'_>> {
        let mut vector = VersionVector::default();
        let mut stmt = crate::sql::cached_statement(&self.tx, REGISTER_VECTORS_SQL)?;
        let mut rows = stmt.query([root_table, root_id])?;
        while let Some(row) = rows.next()? {
            let stored: String = row.get(0)?;
            vector.merge(&VersionVector::from_json(&stored)?);
        }
        vector.observe(&self.set.device, self.set.seq);
        Ok(ChangeStamp {
            set: &self.set,
            root_table: root_table.to_owned(),
            root_id: root_id.to_owned(),
            season_id: season_id.map(str::to_owned),
            version_vector: vector.to_json()?,
        })
    }

    /// Commit the transaction, and with it the change set.
    pub fn commit(self) -> rusqlite::Result<()> {
        self.tx.commit()
    }
}

/// Append one row to the append-only `record_change` log. `payload` is the full
/// `{"before": ..., "after": ...}` document for the change.
///
/// **The row is checked against the aggregate map before it is written**: the
/// register the stamp names must be the one the row's own values place it in
/// (a treated plot under the treatment its `treatment_record_id` names, a
/// declaration under its slot). A mismatch is refused as a
/// `ShapeViolation`, and the caller's transaction rolls back with it. A row
/// filed under the wrong register would compile, pass every test that does not
/// look at the log, and be merged as part of the wrong statement forever after.
pub fn write_change(
    tx: &Transaction,
    stamp: &ChangeStamp,
    entity_table: &str,
    entity_id: &str,
    operation: &str,
    payload: Value,
) -> Result<()> {
    // The row as it stands after the change, or as it stood before a hard
    // delete — whichever image exists carries the columns that place it.
    let image = match &payload["after"] {
        Value::Null => &payload["before"],
        after => after,
    };
    let (root_table, root_id) = sync::register_of(tx, entity_table, entity_id, image)?;
    // A row that states its campaign must be stamped with it: the log is read
    // per season, and a register's children inherit the stamp's.
    if let Some(season) = image.get("season_id")
        && season.as_str() != stamp.season_id.as_deref()
    {
        return Err(CoreError::ShapeViolation(format!(
            "{entity_table} {entity_id} is in season {season}, but was stamped for {:?}",
            stamp.season_id
        )));
    }
    if (root_table.as_str(), root_id.as_str())
        != (stamp.root_table.as_str(), stamp.root_id.as_str())
    {
        return Err(CoreError::ShapeViolation(format!(
            "{entity_table} {entity_id} belongs to {root_table} {root_id}, \
             but was stamped for {} {}",
            stamp.root_table, stamp.root_id
        )));
    }
    let mut stmt = crate::sql::cached_statement(tx, WRITE_CHANGE_SQL)?;
    stmt.execute(rusqlite::params![
        Uuid::now_v7().to_string(),
        entity_table,
        entity_id,
        stamp.season_id,
        operation,
        stamp.set.changed_at,
        stamp.set.actor,
        payload.to_string(),
        stamp.root_table,
        stamp.root_id,
        stamp.set.device,
        stamp.set.seq,
        stamp.version_vector,
        stamp.set.hlc,
    ])?;
    // This row is a new head for its register, and the stamp's vector merges
    // every head the register had — so whatever conflict was waiting on it is
    // now resolved, by exactly the ordinary write the design says resolves one
    // (docs/sync.md → Conflicts as the person sees them).
    //
    // Cleared here rather than by the merge, because a local correction never
    // goes near the merge: `settle` runs when rows ARRIVE. `sync_conflict` is
    // derived from the log, so whoever writes the log owes it this.
    crate::sql::cached_statement(tx, CLEAR_REGISTER_CONFLICT_SQL)?
        .execute(rusqlite::params![stamp.root_table, stamp.root_id])?;
    Ok(())
}

/// Log an insert: `before` is null, `after` is the serialized new row.
///
/// `after` must be the complete domain struct, never a hand-picked subset: the log
/// doubles as the sync delta source, and a receiving device has to be able to
/// materialise the row from this payload alone.
pub fn log_insert<T: Serialize>(
    tx: &Transaction,
    stamp: &ChangeStamp,
    table: &str,
    id: &str,
    after: &T,
) -> Result<()> {
    write_change(
        tx,
        stamp,
        table,
        id,
        "insert",
        json!({ "before": Value::Null, "after": serde_json::to_value(after)? }),
    )
}

/// Log an update: complete before- and after-images of the row.
pub fn log_update<T: Serialize>(
    tx: &Transaction,
    stamp: &ChangeStamp,
    table: &str,
    id: &str,
    before: &T,
    after: &T,
) -> Result<()> {
    write_change(
        tx,
        stamp,
        table,
        id,
        "update",
        json!({ "before": serde_json::to_value(before)?, "after": serde_json::to_value(after)? }),
    )
}

/// Log a delete. For a soft delete `after` is the row with `deleted_at` set
/// (both images complete); for a hard delete (extension rows only — regulatory
/// records are never hard-deleted) `after` is `None`, serialized as null.
pub fn log_delete<T: Serialize>(
    tx: &Transaction,
    stamp: &ChangeStamp,
    table: &str,
    id: &str,
    before: &T,
    after: Option<&T>,
) -> Result<()> {
    let after = match after {
        Some(row) => serde_json::to_value(row)?,
        None => Value::Null,
    };
    write_change(
        tx,
        stamp,
        table,
        id,
        "delete",
        json!({ "before": serde_json::to_value(before)?, "after": after }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, StatementStatus};

    /// A log several thousand change sets long, from two devices, so a query
    /// that scans instead of seeking has something to scan.
    fn long_log() -> Connection {
        let conn = crate::open_in_memory().unwrap();
        conn.execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4000)
             INSERT INTO record_change
               (id, entity_table, entity_id, operation, changed_at, payload,
                root_table, root_id, origin_device, origin_seq, version_vector, hlc)
             SELECT printf('row-%d', i), 'farm', printf('farm-%d', i % 50), 'update',
                    '2026-09-18T10:00:00Z', '{}', 'farm', printf('farm-%d', i % 50),
                    CASE i % 2 WHEN 0 THEN 'device-a' ELSE 'device-b' END, i,
                    '{}', i * 65536
             FROM n",
        )
        .unwrap();
        conn
    }

    /// Run one of the stamp's queries and report how many rows SQLite stepped
    /// through in full table scans to answer it. Zero means every table it read
    /// was reached through an index.
    fn full_scan_steps(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> i32 {
        let mut stmt = conn.prepare(sql).unwrap();
        let mut rows = stmt.query(params).unwrap();
        while rows.next().unwrap().is_some() {}
        drop(rows);
        stmt.get_status(StatementStatus::FullscanStep)
    }

    #[test]
    fn stamping_a_write_seeks_and_never_scans_the_log() {
        // `record_change` is the fastest-growing table in the schema and every
        // write reads it three times before logging anything. A missing index
        // would change no answer and would make every save slower for as long
        // as the farmer keeps using the app.
        let conn = long_log();
        // The control: the same instrument does see a scan when there is one.
        // `changed_at` is deliberately unindexed.
        assert!(
            full_scan_steps(&conn, "SELECT MAX(changed_at) FROM record_change", &[]) > 1000,
            "the counter must be able to fail"
        );
        assert_eq!(
            full_scan_steps(&conn, sync::HELD_THROUGH_SQL, &[&"device-a"]),
            0
        );
        assert_eq!(full_scan_steps(&conn, LATEST_HLC_SQL, &[]), 0);
        assert_eq!(
            full_scan_steps(&conn, REGISTER_VECTORS_SQL, &[&"farm", &"farm-7"]),
            0
        );
    }
}
