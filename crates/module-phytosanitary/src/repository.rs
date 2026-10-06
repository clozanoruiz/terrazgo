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
//! The alerts this module raises are not stored at all: `current_alerts` works
//! them out from the registers when the list is read, and writes nothing.
//!
//! Writes take `&mut Connection` because `conn.transaction()` needs a mutable borrow;
//! reads take `&Connection`.

mod alert;
mod analysis;
mod lookup;
mod non_field_treatment;
mod product;
mod seed_treatment;
mod treatment;

// The audit helpers live in terrazgo-core (every crate that writes synced user
// data logs through them). Imported as a module so the entity submodules keep
// addressing them as `super::audit::log_insert`.
use terrazgo_core::audit;

pub use alert::current_alerts;
pub use analysis::{
    get_analysis_record, insert_analysis_record, list_analysis_records,
    list_analysis_records_for_export, soft_delete_analysis_record, soft_delete_analysis_record_tx,
    update_analysis_record,
};
pub use lookup::{
    list_analysis_materials, list_analysis_types, list_authorisation_kinds, list_efficacies,
    list_formulation_types, list_justifications, list_non_field_subject_kinds,
    list_reason_categories, list_register_kinds, list_seed_treatment_kinds,
};
pub use non_field_treatment::{
    clear_register_declaration, get_non_field_treatment, insert_non_field_treatment,
    list_non_field_treatments, list_non_field_treatments_for_export, list_register_declarations,
    set_non_field_efficacy, set_register_declaration, soft_delete_non_field_treatment,
    soft_delete_non_field_treatment_tx, subject_kinds_naming_premises, update_non_field_treatment,
};
pub use seed_treatment::{
    get_seed_treatment, insert_seed_treatment, list_seed_treatments,
    list_seed_treatments_for_export, list_seed_treatments_for_sowing, set_seed_treatment_efficacy,
    soft_delete_seed_treatment, soft_delete_seed_treatment_tx, update_seed_treatment,
};
// The unit lists moved to core with the `unit` table (2026-08-07). Re-exported
// so the treatment form's selectors keep one entry point, exactly as the
// farm-registry moves of 2026-06-12 were.
pub use terrazgo_core::repository::{list_intensity_units, list_quantity_units, list_units};
// The farm-registry repositories moved to the core (2026-06-12); re-exported so
// existing callers (demo seeding, tests) keep one repository entry point.
pub use product::{
    add_product_active_substance, add_product_authorisation, find_product_authorisation,
    insert_active_substance, insert_product, insert_product_with_authorisation,
    list_active_substances, list_product_details, list_products_authorised,
    remove_product_active_substance, remove_product_authorisation, soft_delete_product,
    update_product,
};
pub use terrazgo_core::repository::{
    insert_crop, insert_farm, insert_machinery, insert_operator, insert_plot, insert_season,
    list_crops, list_machinery, list_operators, list_seasons,
};
pub use treatment::{
    MAX_PHI_HORIZON_DAYS, MIN_PHI_HORIZON_DAYS, crop_ids_with_treatments, default_phi_horizon_days,
    get_treatment_record, insert_treatment_record, list_treatment_records, phi_horizon_days,
    phi_status_for_farm, set_treatment_efficacy, soft_delete_treatment_record,
    soft_delete_treatment_record_tx, update_treatment_record, validate_phi_horizon_days,
};
// Export-only query (soft-deleted records included, for the Borrar entries).
// Public since the exporter moved out to terrazgo-siex; the name is the guard
// that its crate visibility used to be.
pub use treatment::list_treatment_records_for_export;

use crate::error::PhytosanitaryError;

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> PhytosanitaryError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => PhytosanitaryError::NotFound,
        other => other.into(),
    }
}

/// The advisor's printed pair, frozen at write time.
///
/// Anexo III Parte I B.d asks for "identificación del aplicador y, en su caso,
/// del asesor" on every treatment the holding makes, which is why both
/// registers resolve it through here rather than each rolling its own query.
/// `None` in, `(None, None)` out: most treatments are not advised.
pub(crate) fn advisor_snapshot(
    conn: &rusqlite::Connection,
    advisor_id: Option<&str>,
) -> crate::error::Result<(Option<String>, Option<String>)> {
    match advisor_id {
        Some(id) => conn
            .query_row(
                "SELECT name, registration_number FROM advisor
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| Ok((Some(r.get::<_, String>(0)?), r.get::<_, Option<String>>(1)?)),
            )
            .map_err(no_rows_to_not_found),
        None => Ok((None, None)),
    }
}

/// Whether `code` exists in an imported reference catalogue. `Ok(None)` means
/// the catalogue itself is not imported — nothing to check against (in a
/// running app the vendored snapshot is imported at startup, so this only
/// happens for countries without catalogue data). Retired codes count as
/// existing: providers baja-date codes rather than delete them.
///
/// **Asked only of a code a save adds or changes, never of one the record
/// already carries.** A record synced from a device whose catalogues are newer
/// can name a code this device's copy lacks (docs/sync.md → What stays
/// device-local); it was checked where it was written, and a correction to any
/// other field must still save — reference data never stands between a farmer
/// and a lawful record. A code a save adds can only come from this device's
/// own pickers, so an unknown one there is a mistake.
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
