// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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
    // Trust nothing: reopen the copy and verify it is intact and current. No
    // module shape to check — a snapshot just taken from the live database has
    // the running app's shape by construction; the probe is for files that
    // arrive from somewhere else.
    let info = validate_backup(dest, schema_version, &[])?;
    if info.schema_version != schema_version {
        return Err(CoreError::Invalid("backup_invalid"));
    }

    Ok(BackupSummary {
        path: dest_str.to_string(),
        size_bytes: std::fs::metadata(dest)?.len(),
        schema_version,
    })
}

/// One table of a shape fingerprint: its name and the columns it must carry.
pub type TableShape = (&'static str, &'static [&'static str]);

/// Check that `path` is an intact Terrazgo backup importable by an app whose
/// composed migration sequence reaches `max_supported_version`.
///
/// * not SQLite, failed integrity check, or `user_version` 0 (never touched by
///   the migration runner) → `Invalid("backup_invalid")`;
/// * `user_version` beyond what this app knows → `Invalid("backup_newer_schema")`
///   (importing would downgrade the schema and lose data);
/// * an OLDER version passes: reopening the imported file migrates it forward.
///
/// `module_shape` extends the core fingerprint with the tables registered
/// modules own — the same composition the migration runner does, and for the
/// same reason: core cannot name a module's tables. Callers that validate a
/// snapshot of their own live database pass an empty slice.
pub fn validate_backup(
    path: &Path,
    max_supported_version: i64,
    module_shape: &[TableShape],
) -> Result<BackupInfo> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // A backup can arrive from anywhere — a USB stick, an email. The integrity
    // check below catches a DAMAGED file; this catches a CRAFTED one, which
    // would otherwise pass every check here and then become the live database.
    crate::db::harden(&conn)?;

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
    // A backup at the CURRENT version must also have the current shape. While
    // the project is pre-release the migration files are edited in place, which
    // adds columns WITHOUT bumping the migration count — so `user_version`
    // alone cannot tell a backup taken before an edit from one taken after.
    // Left unchecked, such a file imports "successfully" and then fails with
    // `no such column` on the first query, long after the reason is visible.
    // Compare the shape instead. Older versions are exempt: they are migrated
    // forward on reopen, which is what makes them importable at all.
    if schema_version == max_supported_version {
        for (table, columns) in REQUIRED_SHAPE.iter().chain(module_shape) {
            let present = table_columns(&conn, table)?;
            if columns.iter().any(|c| !present.iter().any(|p| p == c)) {
                return Err(CoreError::Invalid("backup_invalid"));
            }
        }
    }

    Ok(BackupInfo { schema_version })
}

/// The columns a current-version backup must carry, as a fingerprint of the
/// squashed schema. Not the whole schema: the point is to catch a stale file
/// cheaply, so this lists the tables whose shape changed most recently, and
/// each pre-release schema edit adds its new columns here. Post-release, when
/// migrations become append-only and `user_version` becomes trustworthy again,
/// this check becomes redundant and can go.
///
/// Core tables only — a module's own tables reach the probe through
/// `validate_backup`'s `module_shape` argument.
const REQUIRED_SHAPE: &[TableShape] = &[
    (
        "farm",
        &[
            "address",
            "postal_code",
            "phone_fixed",
            "phone_mobile",
            "email",
        ],
    ),
    ("farm_es_extension", &["siex_code"]),
    ("farm_representative", &["farm_id", "full_name"]),
    (
        "crop",
        &[
            "area_ha",
            "irrigation_code",
            "growing_environment_code",
            "gip_system_code",
            "crop_code",
            "source",
            "source_campaign",
            "declared_area_ha",
        ],
    ),
    ("operator", &["tax_id"]),
    (
        "premises",
        &[
            "kind_code",
            "name",
            "address",
            "vehicle_model",
            "plate",
            "class_code",
        ],
    ),
    (
        "premises_es_extension",
        &["cadastral_reference", "rea_installation_code"],
    ),
    // Moved here from module-cue on 2026-08-20 with the table itself. It was in
    // neither fingerprint before: a stale backup whose export_alias was missing
    // would have imported cleanly and then lost every frozen alias, which is
    // the one thing about this table that must never happen — SIEX keys its
    // edits and deletions on those integers.
    (
        "export_alias",
        &["target", "entity_table", "entity_id", "split_key", "alias"],
    ),
    ("machinery", &["acquired_on"]),
    ("season", &["deleted_at"]),
    ("advisor", &["id", "name", "registration_number"]),
    (
        "farm_advisor",
        &["farm_id", "advisor_id", "gip_system_code"],
    ),
    (
        "sowing_record",
        &[
            "sown_on",
            "sowing_end_date",
            "flooded_on",
            "seed_quantity_kg",
        ],
    ),
    ("sowing_plot", &["sowing_record_id", "plot_id", "crop_id"]),
    (
        "harvest_record",
        &[
            "harvested_on",
            "product_name",
            "plant_product_code",
            "quantity_value",
            "quantity_unit_code",
            "buyer_name",
            "buyer_registry_number",
        ],
    ),
    ("harvest_plot", &["harvest_record_id", "plot_id", "crop_id"]),
    (
        "plot_water_point",
        &[
            "plot_id",
            "denomination",
            "inside_plot",
            "distance_m",
            "latitude",
            "longitude",
        ],
    ),
    ("plot_water_declaration", &["plot_id", "declared_on"]),
    // Not user data, but the startup catalogue import writes these columns: a
    // backup taken before one of them existed would import cleanly and then
    // fail with `no such column` at the next startup, which is exactly the
    // delayed failure this probe exists to prevent. `absent_since` is on the
    // list because the import clears it on every row a file still carries.
    ("catalogue", &["source_digest", "imported_by_version"]),
    ("catalogue_code", &["absent_since"]),
];

/// Column names of `table`, empty when the table itself is missing.
fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let names = stmt
        .query_map([table], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(names)
}

fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
