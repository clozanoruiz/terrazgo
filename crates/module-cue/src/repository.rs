// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Thin repository layer: CRUD for `TreatmentRecord` and the entities it depends on,
//! one submodule per entity group. The public functions are re-exported here, so
//! callers keep writing `repository::insert_farm(...)`.
//!
//! Two invariants are enforced here so callers can't get them wrong:
//!   1. Every write to a synced user-data table also appends to `record_change`
//!      (audit trail + future sync delta source), inside the same transaction.
//!      The payload is always the COMPLETE row image — Stage-2/3 sync must be able
//!      to rebuild a row from the log alone, so a partial payload is a bug.
//!   2. `TreatmentRecord` freezes its legally-printed values (`*_snapshot`) at write
//!      time, and stores `phi_days_used` (input) next to the derived `phi_end_date`.
//!
//! Exception to invariant 1: `alert` rows are derived state, owned by `refresh_alerts`
//! and re-derivable on any device — they are never logged to `record_change`.
//!
//! Writes take `&mut Connection` because `conn.transaction()` needs a mutable borrow;
//! reads take `&Connection`.

mod alert;
mod export_alias;
mod lookup;
mod product;
mod treatment;

// The audit helpers live in terrazgo-core (every crate that writes synced user
// data logs through them). Imported as a module so the entity submodules keep
// addressing them as `super::audit::log_insert`.
use terrazgo_core::audit;

pub use alert::{acknowledge_alert, dismiss_alert, list_active_alerts, refresh_alerts};
pub use export_alias::{ensure_export_alias, find_export_alias};
pub use lookup::{
    list_authorisation_kinds, list_efficacies, list_formulation_types, list_justifications,
    list_reason_categories, list_units,
};
// The farm-registry repositories moved to the core (2026-06-12); re-exported so
// existing callers (demo seeding, tests) keep one repository entry point.
pub use product::{
    add_product_active_substance, add_product_authorisation, insert_active_substance,
    insert_product, insert_product_with_authorisation, list_active_substances,
    list_product_details, list_products_authorised, remove_product_active_substance,
    remove_product_authorisation, soft_delete_product, update_product,
};
pub use terrazgo_core::repository::{
    insert_crop, insert_farm, insert_machinery, insert_operator, insert_plot, insert_season,
    list_crops, list_machinery, list_operators, list_seasons,
};
pub use treatment::{
    get_treatment_record, insert_treatment_record, list_treatment_records, phi_status_for_farm,
    set_treatment_efficacy, soft_delete_treatment_record,
};
// Export-only query (soft-deleted records included, for the Borrar entries);
// crate-visible so the seam stays the export module, not general callers.
pub(crate) use treatment::list_treatment_records_for_export;

use crate::error::CueError;

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> CueError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => CueError::NotFound,
        other => other.into(),
    }
}

/// Whether `code` exists in an imported reference catalogue. `Ok(None)` means
/// the catalogue itself is not imported — nothing to check against (in a
/// running app the vendored snapshot is imported at startup, so this only
/// happens for countries without catalogue data). Retired codes count as
/// existing: providers baja-date codes rather than delete them.
pub(crate) fn resolve_in_catalogue(
    conn: &rusqlite::Connection,
    catalogue_id: &str,
    code: &str,
) -> crate::error::Result<Option<bool>> {
    let imported: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalogue WHERE id = ?1)",
        [catalogue_id],
        |r| r.get(0),
    )?;
    if !imported {
        return Ok(None);
    }
    let known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalogue_code WHERE catalogue_id = ?1 AND code = ?2)",
        rusqlite::params![catalogue_id, code],
        |r| r.get(0),
    )?;
    Ok(Some(known))
}
