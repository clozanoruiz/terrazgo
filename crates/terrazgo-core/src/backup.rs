// SPDX-License-Identifier: AGPL-3.0-or-later

//! Database backup: export a consistent snapshot of the live database and
//! validate a file before importing it.
//!
//! Export uses `VACUUM INTO` (chosen over a WAL checkpoint and the online
//! backup API): a single statement that writes a consistent,
//! compacted, self-contained copy while the connection stays open — no WAL
//! sidecar files, no torn reads. The copy is verified (integrity check) before
//! success is reported: an unverified backup of regulatory records is worse
//! than none.
//!
//! Validation errors use `CoreError::Invalid` machine codes (`backup_invalid`,
//! `backup_newer_schema`) so the command boundary maps them to i18n keys.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::error::{CoreError, Result};

/// What an export produced; shown to the user by the UI.
#[derive(Debug, Clone, Serialize)]
pub struct BackupSummary {
    pub path: String,
    pub size_bytes: u64,
    pub schema_version: i64,
}

/// What validation learned about a backup file before an import.
#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub schema_version: i64,
}

/// Export a consistent snapshot of the live database to `dest`, replacing any
/// existing file (the save dialog already confirmed the overwrite). The copy
/// is validated before returning.
pub fn export_backup(conn: &Connection, dest: &Path) -> Result<BackupSummary> {
    let dest_str = dest.to_str().ok_or(CoreError::Invalid("backup_invalid"))?;

    // VACUUM INTO refuses to overwrite; the dialog's confirmation makes the
    // removal safe. A leftover -wal from a previous copy method would corrupt
    // the fresh snapshot, so clear sidecars too.
    for suffix in ["", "-wal", "-shm"] {
        let path = format!("{dest_str}{suffix}");
        if Path::new(&path).exists() {
            std::fs::remove_file(&path)?;
        }
    }

    conn.execute("VACUUM INTO ?1", [dest_str])?;

    let schema_version = schema_version(conn)?;
    // Trust nothing: reopen the copy and verify it is intact and current.
    let info = validate_backup(dest, schema_version)?;
    if info.schema_version != schema_version {
        return Err(CoreError::Invalid("backup_invalid"));
    }

    Ok(BackupSummary {
        path: dest_str.to_string(),
        size_bytes: std::fs::metadata(dest)?.len(),
        schema_version,
    })
}

/// Check that `path` is an intact Terrazgo backup importable by an app whose
/// composed migration sequence reaches `max_supported_version`.
///
/// * not SQLite, failed integrity check, or `user_version` 0 (never touched by
///   the migration runner) → `Invalid("backup_invalid")`;
/// * `user_version` beyond what this app knows → `Invalid("backup_newer_schema")`
///   (importing would downgrade the schema and lose data);
/// * an OLDER version passes: reopening the imported file migrates it forward.
pub fn validate_backup(path: &Path, max_supported_version: i64) -> Result<BackupInfo> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let intact = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .map(|verdict| verdict == "ok")
        // "file is not a database" and friends land here, not at open() —
        // SQLite opens lazily.
        .unwrap_or(false);
    if !intact {
        return Err(CoreError::Invalid("backup_invalid"));
    }

    let schema_version = schema_version(&conn)?;
    if schema_version == 0 {
        return Err(CoreError::Invalid("backup_invalid"));
    }
    if schema_version > max_supported_version {
        return Err(CoreError::Invalid("backup_newer_schema"));
    }

    Ok(BackupInfo { schema_version })
}

fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
