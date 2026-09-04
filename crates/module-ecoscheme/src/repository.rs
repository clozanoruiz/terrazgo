// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository for the eco-scheme domain, one submodule per register, with the
//! public functions re-exported here.
//!
//! Same invariant as every other repository in the workspace: each write to a
//! synced user-data table also appends a COMPLETE row image to `record_change`
//! inside the same transaction, junctions logged individually, `actor` threaded
//! through from the shell's active profile.
//!
//! Writes take `&mut Connection` because `conn.transaction()` needs a mutable
//! borrow; reads take `&Connection`.

mod cultural_operation;
mod grazing;
mod lookup;
mod soil_cover;

// The audit helpers live in terrazgo-core (every crate that writes synced user
// data logs through them), imported as a module so the submodules keep
// addressing them as `super::audit::log_insert`.
use rusqlite::OptionalExtension;
use terrazgo_core::audit;

pub use cultural_operation::{
    get_cultural_operation, insert_cultural_operation, list_cultural_operations,
    list_cultural_operations_for_export, soft_delete_cultural_operation, update_cultural_operation,
};
pub use grazing::{
    get_grazing_record, insert_grazing_record, list_grazing_records,
    list_grazing_records_for_export, soft_delete_grazing_record, update_grazing_record,
};
pub use lookup::{list_cultural_operation_kinds, list_eco_practices};
pub use soil_cover::{
    get_soil_cover, get_soil_cover_for_export, insert_soil_cover, list_soil_covers,
    list_soil_covers_for_export, soft_delete_soil_cover, update_soil_cover,
};

use crate::error::EcoschemeError;

/// Whether any record of THIS module hangs off a season — the module's arm of
/// the guard the shell chains before deleting one. Every register this crate
/// owns has to be here: a season holding nothing but a grazing record would
/// otherwise be deletable, and its records would vanish from a book that is
/// read season by season.
pub fn season_has_records(conn: &rusqlite::Connection, season_id: &str) -> crate::Result<bool> {
    Ok(grazing::season_has_grazing(conn, season_id)?
        || cultural_operation::season_has_operations(conn, season_id)?
        || soil_cover::season_has_covers(conn, season_id)?)
}

/// Map `rusqlite::Error::QueryReturnedNoRows` to our `NotFound`, pass
/// everything else through.
pub(crate) fn no_rows_to_not_found(e: rusqlite::Error) -> EcoschemeError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => EcoschemeError::NotFound,
        other => other.into(),
    }
}

/// Resolve the optional link from a maintenance record back to the cover it
/// maintained (RD 1048/2022 art. 42.1.c).
///
/// Shared by both registers that can carry one, because the two rules are the
/// same in either: the cover must be a live row on the SAME farm, and the
/// record's practice must be the cover's own. That second rule is what keeps
/// the printed pages honest — model 9.4 is the P6 page and 9.5 the P7 one, so a
/// siega filed under `sustainable_mowing` but pointed at a plant cover would
/// claim to be P2's duty while printing as P6's maintenance.
///
/// A blank string is the frontend's "no cover", and is folded to `None` rather
/// than looked up.
pub(crate) fn validated_cover_link(
    tx: &rusqlite::Transaction,
    soil_cover_id: Option<&str>,
    farm_id: &str,
    practice_code: &str,
) -> crate::Result<Option<String>> {
    let Some(cover_id) = soil_cover_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(None);
    };

    let found: Option<(String, String)> = tx
        .query_row(
            "SELECT farm_id, practice_code FROM soil_cover
             WHERE id = ?1 AND deleted_at IS NULL",
            [cover_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((cover_farm_id, cover_practice)) = found else {
        return Err(EcoschemeError::Invalid("cover_not_found"));
    };
    if cover_farm_id != farm_id {
        return Err(EcoschemeError::Invalid("cover_on_another_farm"));
    }
    if cover_practice != practice_code {
        return Err(EcoschemeError::Invalid("cover_practice_mismatch"));
    }
    Ok(Some(cover_id.to_string()))
}
