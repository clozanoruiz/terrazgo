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
pub const BACKUP_SHAPE: &[terrazgo_core::backup::TableShape] = &[
    (
        "irrigation_record",
        &[
            "irrigated_on",
            "irrigation_method_code",
            "volume_value",
            "volume_unit_code",
        ],
    ),
    (
        "irrigation_plot",
        &["irrigation_record_id", "plot_id", "irrigated_area_ha"],
    ),
    (
        "irrigation_water_origin",
        &["irrigation_record_id", "origin_code"],
    ),
    (
        "fertiliser_material",
        &["name", "material_code", "supplier_rega", "density_kg_l"],
    ),
    (
        "fertiliser_material_nutrient",
        &[
            "fertiliser_material_id",
            "kind_code",
            "nutrient_code",
            "percentage",
        ],
    ),
    (
        "fertilisation_record",
        &[
            "applied_on",
            "fertilisation_type_code",
            "application_method_code",
            "dose_value",
            "dose_unit_code",
            "fertiliser_material_id",
            "material_name_snapshot",
            "material_code_snapshot",
            "sludge_application",
        ],
    ),
    (
        "fertilisation_plot",
        &["fertilisation_record_id", "plot_id", "fertilised_area_ha"],
    ),
    (
        "fertilisation_practice",
        &["fertilisation_record_id", "practice_code"],
    ),
    (
        "fertilisation_plan",
        &[
            "needs_n_kg_ha",
            "needs_p2o5_kg_ha",
            "needs_k2o_kg_ha",
            "expected_yield_kg_ha",
            "preceding_crop_code",
            "drawn_up_on",
            "tool_generated",
        ],
    ),
    (
        "fertilisation_plan_crop",
        &["fertilisation_plan_id", "crop_id"],
    ),
];

/// A runnable set for this library's own tests: the CORE's steps followed by
/// this module's, mirroring the shell's composed global sequence (these tables
/// reference season, farm, plot, crop and unit, so the module's SQL cannot run
/// on its own). The app itself never calls this.
///
/// Deliberately NOT module-cue's steps: this module does not depend on that
/// one, and a test set that quietly pulled it in would hide the day some code
/// here started relying on a treatment table.
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
