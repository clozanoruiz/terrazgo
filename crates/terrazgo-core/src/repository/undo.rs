// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bringing a removed register back by undoing what removed it (docs/sync.md →
//! Bringing a register back writes only what changes).
//!
//! Every write that brings a removed register back goes through [`undo`]:
//! bringing back one of two records removed by opposite verdicts, and bringing
//! back a book with what was removed with it. **It writes only what changes**
//! — each row the removal wrote, put back as it was before — because every
//! device the write can reach holds the register's history to rebuild the rest
//! from: a file arrives only after everything it was built on, and a device
//! that erased the register discards a write that never saw the purge (An
//! import meets the purge).
//!
//! *Changed 2026-10-04*: a restore stated every row of the register, and the
//! removed rows it pointed at, for a device that had erased the register and
//! held nothing to rebuild from. Since the purge wins, no such device keeps
//! the write.

use std::collections::BTreeMap;

use rusqlite::params;
use serde_json::{Value, json};

use crate::audit::{WriteTx, write_change};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::merge::{Head, branch_states, heads, live_head, settle};

/// The rows one change set wrote in one register, and what each was before it.
/// A seek on the leading pair of `record_change`'s UNIQUE index — which also
/// says a change set writes a row at most once.
const UNDONE_ROWS_SQL: &str = "SELECT entity_table, entity_id, payload FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2 AND root_table = ?3 AND root_id = ?4";

/// Undo what `undone` wrote in one register, in the caller's change set; the
/// tables then follow through `settle`, as after any write that goes through
/// the log.
///
/// Each row `undone` wrote is written as it was before it — a removal cleared,
/// a row it added taken away — with a fresh `updated_at`. A row that already
/// stands that way is left out, and so is every row `undone` did not write.
///
/// `undone` must be a version of the register nothing has been written on
/// top of — one of its heads. Undoing a change set the register has moved
/// past would overwrite what came after it, so it is refused; every caller
/// checks first, so reaching the refusal is a defect.
///
/// `season` is the book the register is filed in, which every row stating one
/// must agree with (`audit::write_change` checks).
pub(super) fn undo(
    tx: &WriteTx,
    root_table: &str,
    root_id: &str,
    season: Option<&str>,
    undone: &Head,
) -> Result<()> {
    let current = heads(tx, root_table, root_id)?;
    if !current.contains(undone) {
        return Err(CoreError::ShapeViolation(format!(
            "{root_table} {root_id}: a restore may only undo a current version"
        )));
    }
    let live = live_head(&current).cloned().ok_or_else(|| {
        CoreError::ShapeViolation(format!("{root_table} {root_id} has no version to undo"))
    })?;
    let state = branch_states(tx, root_table, root_id, std::slice::from_ref(&live))?
        .into_iter()
        .next()
        .unwrap_or_default();

    let mut earlier: BTreeMap<(String, String), Value> = BTreeMap::new();
    {
        let mut stmt = tx.prepare(UNDONE_ROWS_SQL)?;
        let mut rows = stmt.query(params![undone.device, undone.seq, root_table, root_id])?;
        while let Some(row) = rows.next()? {
            let payload: String = row.get(2)?;
            let payload: Value = serde_json::from_str(&payload)?;
            earlier.insert((row.get(0)?, row.get(1)?), payload["before"].clone());
        }
    }

    let now = now_utc_iso();
    let mut writes: Vec<Undone> = Vec::new();
    for (key, before) in earlier {
        let standing = state.get(&key).cloned().flatten();
        let mut stated = match before {
            Value::Null => None,
            before => Some(before),
        };
        if stated == standing {
            continue;
        }
        if let Some(image) = stated.as_mut().and_then(Value::as_object_mut)
            && image.contains_key("updated_at")
        {
            image.insert("updated_at".to_owned(), Value::from(now.as_str()));
        }
        writes.push(Undone {
            key,
            standing,
            stated,
        });
    }
    if writes.is_empty() {
        return Ok(());
    }
    // The register's own row before the rows belonging to it, then by table
    // and id, so the change set reads alike wherever it is written. Which row
    // leaves a slot before another takes it is the apply's to order, on the
    // tables it writes (`merge::make_live`).
    writes.sort_by(|left, right| {
        let order = |write: &Undone| {
            (
                write.key.0 != root_table,
                write.key.0.clone(),
                write.key.1.clone(),
            )
        };
        order(left).cmp(&order(right))
    });

    let stamp = tx.register(root_table, root_id, season)?;
    for write in &writes {
        let operation = match (&write.standing, &write.stated) {
            (None, _) => "insert",
            (_, None) => "delete",
            _ => "update",
        };
        let (table, id) = &write.key;
        write_change(
            tx,
            &stamp,
            table,
            id,
            operation,
            json!({ "before": write.standing, "after": write.stated }),
        )?;
    }
    settle(tx, root_table, root_id, Some(&live))?;
    Ok(())
}

/// One row an undo writes: as it stands, and as it was before the change set
/// being undone.
struct Undone {
    key: (String, String),
    standing: Option<Value>,
    stated: Option<Value>,
}
