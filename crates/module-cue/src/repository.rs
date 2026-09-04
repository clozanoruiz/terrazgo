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

pub use alert::{acknowledge_alert, dismiss_alert, list_active_alerts, refresh_alerts};
pub use analysis::{
    get_analysis_record, insert_analysis_record, list_analysis_records,
    list_analysis_records_for_export, soft_delete_analysis_record, update_analysis_record,
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
    subject_kinds_naming_premises, update_non_field_treatment,
};
pub use seed_treatment::{
    get_seed_treatment, insert_seed_treatment, list_seed_treatments,
    list_seed_treatments_for_export, list_seed_treatments_for_sowing, set_seed_treatment_efficacy,
    soft_delete_seed_treatment, update_seed_treatment,
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
    phi_status_for_farm, season_has_treatments, set_treatment_efficacy,
    soft_delete_treatment_record, update_treatment_record, validate_phi_horizon_days,
};
// Export-only query (soft-deleted records included, for the Borrar entries).
// Public since the exporter moved out to terrazgo-siex; the name is the guard
// that its crate visibility used to be.
pub use treatment::list_treatment_records_for_export;

use crate::error::CueError;

/// Whether this season holds ANY record this module owns — the module half of
/// the season-deletion guard, chained by the shell before it calls core's
/// `soft_delete_season` (core owns the `season` row but may never reference a
/// module's table).
///
/// It answers for every register, not just treatments: each one is
/// season-scoped and every record-book view is read through its season, so
/// hiding the season would hide a postharvest treatment, a sowing or an
/// analysis exactly as thoroughly as it would hide a treatment.
pub fn season_has_records(
    conn: &rusqlite::Connection,
    season_id: &str,
) -> crate::error::Result<bool> {
    Ok(treatment::season_has_treatments(conn, season_id)?
        || non_field_treatment::season_has_non_field_treatments(conn, season_id)?
        || seed_treatment::season_has_sowings(conn, season_id)?
        || analysis::season_has_analyses(conn, season_id)?)
}

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> CueError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => CueError::NotFound,
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
