// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The registers whose records can be one operation recorded twice, each with
//! the rule that says so and the delete that removes a copy (docs/sync.md →
//! Duplicate suspects).
//!
//! The list is the shell's because only the shell sees every crate — core may
//! never name a module's table, and a module may never name another. Each rule
//! is a constant in the crate that owns the register; what the shell adds is
//! the pairing with that register's own delete, which core needs for *keep
//! this one* and cannot name.
//!
//! **A register of the book is added here, once.**
//! `tests/contracts/duplicate_rules_contract.rs` refuses any table carrying a
//! `season_id` that is missing, and checks every rule against the schema and
//! against the query planner.

use terrazgo_core::audit::WriteTx;
use terrazgo_core::duplicates::DuplicatePolicy;

/// One register, its rule, and how a copy of it is removed.
pub struct DuplicateRegister {
    pub policy: DuplicatePolicy,
    /// The register's own delete, inside the transaction that also writes the
    /// verdict. A plain function rather than a trait, like `AlertCrate`'s: each
    /// entry adapts its crate's delete to one signature.
    pub remove: fn(&WriteTx, &str) -> anyhow::Result<()>,
}

/// Every register compared for duplicates, core's first.
pub const DUPLICATE_REGISTERS: &[DuplicateRegister] = &[
    DuplicateRegister {
        policy: terrazgo_core::duplicates::CROP_DUPLICATES,
        remove: remove_crop,
    },
    DuplicateRegister {
        policy: terrazgo_core::duplicates::SOWING_DUPLICATES,
        remove: remove_sowing,
    },
    DuplicateRegister {
        policy: terrazgo_core::duplicates::HARVEST_DUPLICATES,
        remove: remove_harvest,
    },
    DuplicateRegister {
        policy: module_phytosanitary::duplicates::TREATMENT_DUPLICATES,
        remove: remove_treatment,
    },
    DuplicateRegister {
        policy: module_phytosanitary::duplicates::NON_FIELD_DUPLICATES,
        remove: remove_non_field_treatment,
    },
    DuplicateRegister {
        policy: module_phytosanitary::duplicates::SEED_TREATMENT_DUPLICATES,
        remove: remove_seed_treatment,
    },
    DuplicateRegister {
        policy: module_phytosanitary::duplicates::ANALYSIS_DUPLICATES,
        remove: remove_analysis,
    },
    DuplicateRegister {
        policy: module_fertilisation::duplicates::FERTILISATION_DUPLICATES,
        remove: remove_fertilisation,
    },
    DuplicateRegister {
        policy: module_fertilisation::duplicates::IRRIGATION_DUPLICATES,
        remove: remove_irrigation,
    },
    DuplicateRegister {
        policy: module_fertilisation::duplicates::PLAN_DUPLICATES,
        remove: remove_fertilisation_plan,
    },
    DuplicateRegister {
        policy: module_ecoscheme::duplicates::GRAZING_DUPLICATES,
        remove: remove_grazing,
    },
    DuplicateRegister {
        policy: module_ecoscheme::duplicates::CULTURAL_OPERATION_DUPLICATES,
        remove: remove_cultural_operation,
    },
    DuplicateRegister {
        policy: module_ecoscheme::duplicates::SOIL_COVER_DUPLICATES,
        remove: remove_soil_cover,
    },
];

/// Every register's rule — what the list runs.
pub fn policies() -> Vec<DuplicatePolicy> {
    DUPLICATE_REGISTERS
        .iter()
        .map(|register| register.policy)
        .collect()
}

/// The register a screen names by its table, among the listed ones — `None`
/// for a table that is not. What a screen sends is looked up here and the
/// table used afterwards is the constant's, never the screen's text.
pub fn register_by_table(table: &str) -> Option<&'static DuplicateRegister> {
    DUPLICATE_REGISTERS
        .iter()
        .find(|register| register.policy.table == table)
}

fn remove_crop(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(terrazgo_core::repository::soft_delete_crop_tx(tx, id)?)
}

fn remove_sowing(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(terrazgo_core::repository::soft_delete_sowing_record_tx(
        tx, id,
    )?)
}

fn remove_harvest(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(terrazgo_core::repository::soft_delete_harvest_record_tx(
        tx, id,
    )?)
}

fn remove_treatment(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_phytosanitary::repository::soft_delete_treatment_record_tx(tx, id)?)
}

fn remove_non_field_treatment(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_phytosanitary::repository::soft_delete_non_field_treatment_tx(tx, id)?)
}

fn remove_seed_treatment(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_phytosanitary::repository::soft_delete_seed_treatment_tx(tx, id)?)
}

fn remove_analysis(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_phytosanitary::repository::soft_delete_analysis_record_tx(tx, id)?)
}

fn remove_fertilisation(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_fertilisation::repository::soft_delete_fertilisation_record_tx(tx, id)?)
}

fn remove_irrigation(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_fertilisation::repository::soft_delete_irrigation_record_tx(tx, id)?)
}

fn remove_fertilisation_plan(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_fertilisation::repository::soft_delete_fertilisation_plan_tx(tx, id)?)
}

fn remove_grazing(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_ecoscheme::repository::soft_delete_grazing_record_tx(
        tx, id,
    )?)
}

fn remove_cultural_operation(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_ecoscheme::repository::soft_delete_cultural_operation_tx(tx, id)?)
}

fn remove_soil_cover(tx: &WriteTx, id: &str) -> anyhow::Result<()> {
    Ok(module_ecoscheme::repository::soft_delete_soil_cover_tx(
        tx, id,
    )?)
}
