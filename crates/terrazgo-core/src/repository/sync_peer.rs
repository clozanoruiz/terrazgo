// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The devices that have written to this book, one `sync_peer` row each
//! (docs/sync.md → Device identity). Synced and logged like any register, so
//! the label somebody gives a phone on the laptop reaches every other device.

use crate::audit::{begin, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::models::SyncPeer;
use crate::sync::installed_device;
use rusqlite::{Connection, OptionalExtension, Row, params};

/// Make sure the device this connection writes as has its own row. Returns
/// that row, created or found. Idempotent, so the shell calls it on every
/// open without asking whether it is needed.
///
/// A new row carries no label: the device has no way to know what people call
/// it. A row that already exists is left as it is, including a label someone
/// gave it elsewhere.
pub fn register_this_device(conn: &mut Connection, actor: Option<&str>) -> Result<SyncPeer> {
    let device = installed_device(conn)?;
    let tx = begin(conn, actor)?;
    let existing = tx
        .query_row(
            "SELECT * FROM sync_peer WHERE id = ?1",
            [&device],
            map_sync_peer,
        )
        .optional()?;
    if let Some(peer) = existing {
        return Ok(peer);
    }

    let now = now_utc_iso();
    let peer = SyncPeer {
        id: device,
        label: None,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO sync_peer (id, label, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![peer.id, peer.label, peer.created_at, peer.updated_at],
    )?;
    let stamp = tx.register("sync_peer", &peer.id, None)?;
    log_insert(&tx, &stamp, "sync_peer", &peer.id, &peer)?;
    tx.commit()?;
    Ok(peer)
}

/// Name a device, or clear the name somebody gave it.
///
/// **The label is the whole point of the table**: a conflict that says "María's
/// phone" is a question a farmer can answer, and one that says
/// `0192f3a4-0000-7000-8000-0000000000a1` is not. It is a register like any
/// other, so the name reaches every device at the next sync — and two devices
/// naming one phone differently is an ordinary conflict over a one-column
/// register, not a special case.
///
/// An empty label is stored as NULL rather than as an empty string: unnamed is
/// a state the table already has, and two spellings of it would print
/// differently in a list.
pub fn rename_sync_peer(
    conn: &mut Connection,
    id: &str,
    label: Option<&str>,
    actor: Option<&str>,
) -> Result<SyncPeer> {
    let label = label.map(str::trim).filter(|label| !label.is_empty());
    let tx = begin(conn, actor)?;
    let before = tx
        .query_row("SELECT * FROM sync_peer WHERE id = ?1", [id], map_sync_peer)
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.label = label.map(str::to_owned);
    after.updated_at = now_utc_iso();
    tx.execute(
        "UPDATE sync_peer SET label = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, after.label, after.updated_at],
    )?;
    let stamp = tx.register("sync_peer", id, None)?;
    log_update(&tx, &stamp, "sync_peer", id, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Retire a device, or bring a retired one back.
///
/// Soft, like every row the log refers to: a lost phone's changes stay in the
/// book and stay attributed to it, and a review or a record's history reading
/// one of its old changes still finds the device that wrote it. What retiring says is
/// only that nobody expects to sync with it again.
///
/// Reversible because the mistake is easy to make and costly to live with: a
/// device retired by accident would otherwise have to be re-imported to come
/// back, and it is the same phone either way.
pub fn retire_sync_peer(
    conn: &mut Connection,
    id: &str,
    retired: bool,
    actor: Option<&str>,
) -> Result<SyncPeer> {
    let device = installed_device(conn)?;
    if retired && device == id {
        // Retiring the device you are holding says the book should stop
        // expecting changes from the device that is making them.
        return Err(CoreError::Invalid("sync_peer_is_this_device"));
    }
    let tx = begin(conn, actor)?;
    let before = tx
        .query_row("SELECT * FROM sync_peer WHERE id = ?1", [id], map_sync_peer)
        .optional()?
        .ok_or(CoreError::NotFound)?;

    let mut after = before.clone();
    after.updated_at = now_utc_iso();
    after.deleted_at = retired.then(|| after.updated_at.clone());
    tx.execute(
        "UPDATE sync_peer SET deleted_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, after.deleted_at, after.updated_at],
    )?;
    let stamp = tx.register("sync_peer", id, None)?;
    log_update(&tx, &stamp, "sync_peer", id, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Take over from the replica this device replaces: retire its row, and carry
/// the name people gave it onto this device's own.
///
/// **Called after a backup import, which mints this device a new id.** That
/// makes the old id a replica which can never write again — `settings.json`
/// holds a different one now — so retiring it is a fact rather than a guess,
/// and it is the only moment anything can know it. Left alone, a device
/// restored a few times accumulates rows in the list that are all really the
/// same machine, and the conflict review goes on naming versions after them.
///
/// The name moves rather than being copied away: the retired row KEEPS it, so a
/// version that replica wrote still reads "Portátil de la oficina" in a
/// comparison, and the live row gains it, so the list reads one row per machine
/// with the old ones marked retired.
///
/// Does nothing, and writes nothing, in the two cases where there is nothing to
/// take over: a backup from ANOTHER device never knew this machine's previous
/// id, and a second import changes a row that is already retired and already
/// named. Both are ordinary, so neither is an error.
pub fn succeed_sync_peer(
    conn: &mut Connection,
    replaced: &str,
    actor: Option<&str>,
) -> Result<Option<SyncPeer>> {
    let device = installed_device(conn)?;
    if device == replaced {
        return Ok(None);
    }
    let tx = begin(conn, actor)?;
    let read = |id: &str| {
        tx.query_row("SELECT * FROM sync_peer WHERE id = ?1", [id], map_sync_peer)
            .optional()
    };
    let (Some(before), Some(mine)) = (read(replaced)?, read(&device)?) else {
        return Ok(None);
    };

    let now = now_utc_iso();
    let mut retired = before.clone();
    // An earlier retirement keeps its own date: this one is not a second event.
    retired.deleted_at = Some(before.deleted_at.clone().unwrap_or_else(|| now.clone()));
    let mut after = mine.clone();
    after.label = mine.label.clone().or_else(|| before.label.clone());
    if (retired.deleted_at.as_deref(), after.label.as_deref())
        == (before.deleted_at.as_deref(), mine.label.as_deref())
    {
        return Ok(None);
    }

    retired.updated_at = now.clone();
    after.updated_at = now;
    for row in [&retired, &after] {
        tx.execute(
            "UPDATE sync_peer SET label = ?2, deleted_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![row.id, row.label, row.deleted_at, row.updated_at],
        )?;
    }
    // Two rows, two registers, one change set: the handover is one act, and a
    // device receiving it sees both halves or neither.
    let stamp = tx.register("sync_peer", &retired.id, None)?;
    log_update(&tx, &stamp, "sync_peer", &retired.id, &before, &retired)?;
    let stamp = tx.register("sync_peer", &after.id, None)?;
    log_update(&tx, &stamp, "sync_peer", &after.id, &mine, &after)?;
    tx.commit()?;
    Ok(Some(after))
}

/// Every device known to this book, retired ones included — a retired
/// phone's changes are still in the log and still need a name.
pub fn list_sync_peers(conn: &Connection) -> Result<Vec<SyncPeer>> {
    let mut stmt = conn.prepare("SELECT * FROM sync_peer ORDER BY created_at, id")?;
    let peers = stmt
        .query_map([], map_sync_peer)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(peers)
}

fn map_sync_peer(row: &Row) -> rusqlite::Result<SyncPeer> {
    Ok(SyncPeer {
        id: row.get("id")?,
        label: row.get("label")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}
