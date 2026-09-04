// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Database setup: versioned migrations embedded in the binary, plus this
//! module's contribution to the backup shape probe.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

/// The ordered migration steps this module contributes to the core's single
/// global sequence. The shell's `composed_migrations()` collects these from
/// every registered module and concatenates them; numbering and execution are
/// owned by the core (docs/architecture.md → Migrations: one global sequence).
/// SQL is embedded with `include_str!` so the binary needs no files at runtime.
pub fn migration_set() -> Vec<M<'static>> {
    vec![
        M::up(include_str!("../migrations/0001_schema.sql")),
        M::up(include_str!("../migrations/0002_seed_reference.sql")),
    ]
}

/// The columns a current-version backup must carry for THIS module's tables.
/// Composed with the core fingerprint and every other module's list by the
/// shell, exactly as the migrations are — core may never name a module's
/// tables. Every pre-release edit to `0001` adds its new columns here.
///
/// The lookups are deliberately absent: the probe is about USER data surviving
/// a restore, and seeded reference rows are rebuilt by the migrations on every
/// open.
pub const BACKUP_SHAPE: &[terrazgo_core::backup::TableShape] = &[
    (
        "grazing_record",
        &[
            "practice_code",
            "plot_group_ref",
            "soil_cover_id",
            "started_on",
            "ended_on",
        ],
    ),
    ("grazing_plot", &["grazing_record_id", "plot_id"]),
    (
        "grazing_animal",
        &[
            "grazing_record_id",
            "species_code",
            "rega_code",
            "animal_count",
        ],
    ),
    (
        "cultural_operation",
        &[
            "practice_code",
            "operation_kind_code",
            "performed_on",
            "performed_end_date",
            "activity_description",
            "residue_destination_code",
            "soil_cover_id",
        ],
    ),
    (
        "cultural_operation_plot",
        &["cultural_operation_id", "plot_id"],
    ),
    (
        "soil_cover",
        &[
            "practice_code",
            "cover_type_code",
            "established_on",
            "width_m",
            "free_canopy_width_m",
            "widths_stated_on",
        ],
    ),
    ("soil_cover_plot", &["soil_cover_id", "plot_id"]),
];

/// A runnable set for this library's own tests: the CORE's steps followed by
/// this module's, mirroring the shell's composed global sequence (its tables
/// reference season, farm and plot, so the module's SQL cannot run on its own).
/// The app itself never calls this.
///
/// Deliberately NOT the other modules' steps: this module depends on neither,
/// and a test set that quietly pulled them in would hide the day some code here
/// started relying on a treatment or an irrigation table.
pub fn migrations() -> Migrations<'static> {
    let mut steps = terrazgo_core::migration_set();
    steps.extend(migration_set());
    Migrations::new(steps)
}

/// Open an in-memory database with foreign keys enforced and all migrations
/// applied. Used by the repository tests.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Tests run on the same connection configuration the app does.
    terrazgo_core::db::harden(&conn)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}
