// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository for the fertilisation domain, one submodule per register, with
//! the public functions re-exported here.
//!
//! Same invariant as every other repository in the workspace: each write to a
//! synced user-data table also appends a COMPLETE row image to `record_change`
//! inside the same transaction, junctions logged individually, `actor` threaded
//! through from the shell's active profile.
//!
//! Writes take `&mut Connection` because `conn.transaction()` needs a mutable
//! borrow; reads take `&Connection`.

mod fertilisation;
mod fertiliser_material;
mod irrigation;
mod lookup;
mod plan;

// The audit helpers live in terrazgo-core (every crate that writes synced user
// data logs through them), imported as a module so the submodules keep
// addressing them as `super::audit::log_insert`.
use terrazgo_core::audit;

pub use fertilisation::{
    get_fertilisation_record, insert_fertilisation_record, list_fertilisation_records,
    list_fertilisation_records_for_export, soft_delete_fertilisation_record,
    soft_delete_fertilisation_record_tx, update_fertilisation_record,
};
pub use fertiliser_material::{
    get_fertiliser_material, get_fertiliser_material_for_export, insert_fertiliser_material,
    list_fertiliser_materials, soft_delete_fertiliser_material, update_fertiliser_material,
};
pub use plan::{
    get_fertilisation_plan, insert_fertilisation_plan, list_fertilisation_plans,
    list_fertilisation_plans_for_export, soft_delete_fertilisation_plan,
    soft_delete_fertilisation_plan_tx, update_fertilisation_plan,
};

pub use irrigation::{
    get_irrigation_record, insert_irrigation_record, list_irrigation_records,
    list_irrigation_records_for_export, soft_delete_irrigation_record,
    soft_delete_irrigation_record_tx, update_irrigation_record,
};
pub use lookup::{
    list_application_methods, list_fertilisation_types, list_irrigation_methods,
    list_manure_treatments, list_nutrient_kinds, list_water_origins,
};
// The unit lists live in core with the `unit` table, so both modules that
// record an amount read the same vocabulary. Re-exported to keep one
// repository entry point, the module-phytosanitary precedent.
pub use terrazgo_core::repository::{list_fertiliser_dose_units, list_irrigation_volume_units};

use crate::error::FertilisationError;

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass
/// everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> FertilisationError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => FertilisationError::NotFound,
        other => other.into(),
    }
}
