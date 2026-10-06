// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Database setup: pragmas + versioned migrations embedded in the binary.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use terrazgo_core::merge::RowCaption;
use terrazgo_core::sync::TableSync;

/// The ordered migration steps this module contributes to the core's single global
/// sequence. The core's registry collects these from every module and concatenates
/// them into one `Migrations` set — numbering and execution are owned by the core
/// (docs/architecture.md → Migrations: one global sequence). SQL is embedded with
/// `include_str!` so the binary needs no files at runtime (offline-first).
///
/// Pre-release the set is exactly two files — 0001 (DDL) and 0002 (seed DML) — and is
/// squashed freely, recreating dev databases. The moment any database holds real data,
/// this list becomes append-only (docs/architecture.md → Migrations: one global sequence).
pub fn migration_set() -> Vec<M<'static>> {
    vec![
        M::up(include_str!("../migrations/0001_schema.sql")),
        M::up(include_str!("../migrations/0002_seed_reference.sql")),
    ]
}

/// This module's half of the aggregate map (docs/sync.md → The aggregate map):
/// what the merge does with each of its tables. Composed with core's and every
/// other module's by the shell, whose contract test refuses a table nobody
/// declared — the same division as [`BACKUP_SHAPE`].
pub const SYNC_SHAPE: &[TableSync] = &[
    TableSync::root("active_substance"),
    TableSync::root("product"),
    TableSync::child("product_active_substance", "product", "product_id"),
    TableSync::child("product_authorisation", "product", "product_id"),
    TableSync::root("treatment_record"),
    TableSync::child("treatment_plot", "treatment_record", "treatment_record_id"),
    TableSync::child(
        "treatment_problem",
        "treatment_record",
        "treatment_record_id",
    ),
    TableSync::child(
        "treatment_justification",
        "treatment_record",
        "treatment_record_id",
    ),
    TableSync::root("non_field_treatment"),
    TableSync::child(
        "non_field_treatment_problem",
        "non_field_treatment",
        "non_field_treatment_id",
    ),
    TableSync::child(
        "non_field_treatment_justification",
        "non_field_treatment",
        "non_field_treatment_id",
    ),
    TableSync::root("seed_treatment"),
    TableSync::child("seed_treatment_plot", "seed_treatment", "seed_treatment_id"),
    TableSync::slot("register_declaration", REGISTER_DECLARATION_SLOT),
    TableSync::root("analysis_record"),
    TableSync::child("analysis_plot", "analysis_record", "analysis_record_id"),
    TableSync::child(
        "analysis_record_type",
        "analysis_record",
        "analysis_record_id",
    ),
    TableSync::child(
        "analysis_substance",
        "analysis_record",
        "analysis_record_id",
    ),
];

/// How a row of each of this module's tables is named to a person
/// (docs/sync.md → Conflicts as the person sees them): what a conflict over it
/// is called, and what a reference to it says instead of a UUID.
///
/// Composed with core's half and every other module's by the shell, like
/// [`SYNC_SHAPE`] — core may never name a module's tables. A register known
/// only by its date says its date: a treatment is "the one applied on 12/05",
/// and its table holds nothing else a farmer would recognise.
pub const ROW_CAPTIONS: &[RowCaption] = &[
    RowCaption::new("product", "commercial_name"),
    RowCaption::new("active_substance", "name"),
    RowCaption::new("treatment_record", "application_date"),
    RowCaption::new("non_field_treatment", "treated_on"),
    RowCaption::new("seed_treatment", "sown_on"),
    RowCaption::new("analysis_record", "sampled_on"),
];

/// `register_declaration`'s slot: one live declaration per farm, season and
/// register. Declaring, withdrawing and declaring again is one register over
/// time, so two devices doing it offline are two versions of one thing.
pub const REGISTER_DECLARATION_SLOT: &[&str] = &["farm_id", "season_id", "register_code"];

/// The columns a current-version backup must carry for THIS module's tables,
/// checked by `terrazgo_core::backup::validate_backup` alongside the core
/// fingerprint. Same reason and same lifetime as the core list: while the
/// project is pre-release, `0001` is edited in place, so `user_version` cannot
/// tell a backup taken before an edit from one taken after — and a stale file
/// would import cleanly and then fail with `no such column`.
///
/// It lives here rather than in core because core may never name a module's
/// tables; the shell composes the two, exactly as it composes the migrations.
/// Every pre-release edit to `0001` adds its new columns here.
pub const BACKUP_SHAPE: &[terrazgo_core::backup::TableShape] = &[
    (
        "treatment_record",
        &[
            "application_end_date",
            "drying_date",
            "total_quantity_value",
            "total_quantity_unit_code",
        ],
    ),
    (
        "non_field_treatment",
        &[
            "subject_kind_code",
            "subject_description",
            "treated_on",
            "premises_id",
        ],
    ),
    (
        "register_declaration",
        &["farm_id", "season_id", "register_code"],
    ),
    (
        "seed_treatment",
        &[
            "sown_on",
            "species_name",
            "seed_lot",
            "treatment_kind_code",
            "product_name",
        ],
    ),
    (
        "analysis_record",
        &[
            "sampled_on",
            "material_kind_code",
            "lab_tax_id",
            "soil_ph",
            "soil_organic_matter_pct",
            "soil_clay_pct",
        ],
    ),
    (
        "analysis_record_type",
        &["analysis_record_id", "analysis_type_code"],
    ),
    (
        "analysis_substance",
        &["analysis_record_id", "substance_code"],
    ),
];

/// A runnable set for the library's own tests: the CORE's
/// steps followed by this module's, mirroring the shell's composed global
/// sequence (phytosanitary tables reference core tables — farm, plot, country — so the
/// module's SQL cannot run on its own). The app itself never calls this.
pub fn migrations() -> Migrations<'static> {
    let mut steps = terrazgo_core::migration_set();
    steps.extend(migration_set());
    Migrations::new(steps)
}

/// Open an in-memory database with foreign keys enforced and all migrations applied.
/// Used by the repository tests. The app opens its database through the core's
/// composed runner instead.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    terrazgo_core::db::harden(&conn)?;
    migrations().to_latest(&mut conn)?;
    // Every database is a replica of its own (terrazgo_core::open_in_memory).
    terrazgo_core::sync::install_device(&conn, &terrazgo_core::sync::mint_device_id())?;
    terrazgo_core::sync::install_shape(&conn, &[terrazgo_core::sync::CORE_SYNC_SHAPE, SYNC_SHAPE])?;
    Ok(conn)
}

/// Open (or create) a file-backed database: WAL mode + foreign keys + migrations.
/// WAL only applies to file databases, so it lives here rather than in `open_in_memory`.
/// Like `open_in_memory`, this is for the library's tests — not the app.
pub fn open(path: impl AsRef<std::path::Path>) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    terrazgo_core::db::harden(&conn)?;
    migrations().to_latest(&mut conn)?;
    // Every database is a replica of its own (terrazgo_core::open_in_memory).
    terrazgo_core::sync::install_device(&conn, &terrazgo_core::sync::mint_device_id())?;
    terrazgo_core::sync::install_shape(&conn, &[terrazgo_core::sync::CORE_SYNC_SHAPE, SYNC_SHAPE])?;
    Ok(conn)
}
