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
    soft_delete_fertilisation_record, update_fertilisation_record,
};
pub use fertiliser_material::{
    get_fertiliser_material, insert_fertiliser_material, list_fertiliser_materials,
    soft_delete_fertiliser_material, update_fertiliser_material,
};
pub use plan::{
    get_fertilisation_plan, insert_fertilisation_plan, list_fertilisation_plans,
    soft_delete_fertilisation_plan, update_fertilisation_plan,
};

pub use irrigation::{
    get_irrigation_record, insert_irrigation_record, list_irrigation_records,
    soft_delete_irrigation_record, update_irrigation_record,
};
pub use lookup::{
    list_application_methods, list_fertilisation_types, list_irrigation_methods,
    list_manure_treatments, list_nutrient_kinds, list_water_origins,
};
// The unit lists live in core with the `unit` table, so both modules that
// record an amount read the same vocabulary. Re-exported to keep one
// repository entry point, the module-cue precedent.
pub use terrazgo_core::repository::{list_fertiliser_dose_units, list_irrigation_volume_units};

use crate::error::FertilisationError;

/// Whether any record of THIS module hangs off a season — the module's arm of
/// the guard the shell chains before deleting one. Every register this crate
/// owns has to be here: a season holding nothing but a fertilisation record
/// would otherwise be deletable, and its records would vanish from a book that
/// is read season by season (the gap seam 4 of the previous slice closed in
/// module-cue, kept closed here by construction).
pub fn season_has_records(conn: &rusqlite::Connection, season_id: &str) -> crate::Result<bool> {
    Ok(irrigation::season_has_irrigation(conn, season_id)?
        || fertilisation::season_has_fertilisation(conn, season_id)?
        || plan::season_has_plans(conn, season_id)?)
}

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass
/// everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> FertilisationError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => FertilisationError::NotFound,
        other => other.into(),
    }
}
