// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! SIEX-aligned cuaderno export (docs/siex-export.md): turns one farm+season
//! into the official CUE descriptor JSON.
//!
//! **A top-layer consumer, and a sibling of `terrazgo-recordbook`.** Both read
//! core and every domain module and project one document; neither depends on
//! the other, and nothing depends on either but the shell. The two documents
//! answer to different readers under different rules — the book is the Spanish
//! official form for a human inspector, this is a machine exchange format with
//! frozen aliases, a precheck and schema validation — so they share their
//! *sources* and nothing else.
//!
//! It lived in `module_cue::export` until 2026-08-20. That was tenable while
//! `TratamFito` was the only block; it stopped being tenable at the point ten
//! of the format's fifteen activity blocks came from `module-fertilisation` and
//! `module-ecoscheme`, which a module may never depend on. The same wall that
//! produced `terrazgo-recordbook`, hit a second time.
//!
//! Two public entry points:
//!   * [`export_precheck`] — what blocks a valid export, for a caller to list
//!     (records missing efficacy or an operator licence, treated plots without
//!     a crop, farm identity fields not yet entered from the REA papers).
//!   * [`build_cuaderno`] — the export itself; refuses while the precheck is
//!     not clean, so nothing is ever silently dropped or invented.
//!
//! Serialization rules (each pinned by the tests against the vendored schema):
//! a multi-crop treatment splits into one `TratamFito` per crop snapshot
//! (3.11.4 descriptor rule, and the same split the printed book makes — see
//! `module_cue::crop_groups`), every entry carries a frozen integer alias
//! (`export_alias` — SIEX keys edits and deletes on it), dates render
//! dd/mm/yyyy, and all codes map through each module's own `siex` module.
//! Soft-deleted records emit `Borrar` entries under their existing aliases;
//! never-exported deletions leave no trace. DGCs are referenced by
//! client-assigned codes (`CodigoDGCAjena`, one alias per crop row — a core
//! `crop` IS the SIEX plot+crop+season unit) while gap 2 (REA `CodigoDGC`
//! import) stays open.
//!
//! **The export is dormant**: it has had no delivery path since the CUECYL
//! answer of 2026-08-02, and no interface calls it. It stays compiled,
//! schema-validated and tested so that the day a path appears the work is
//! already done — and so the descriptor keeps pace with the registers the app
//! captures instead of silently falling behind them.

pub mod blocks;
pub mod db;
pub mod descriptor;
pub mod error;
pub mod precheck;

pub use db::{migrations, open_in_memory};
pub use descriptor::CuadernoExport;
pub use error::{Result, SiexError};
pub use precheck::{ExportPrecheck, PlotRef, RecordRef, export_precheck};

use descriptor::{ActividadesExplotacion, CuadernoEntry};
use rusqlite::Connection;

/// `export_alias.target` for this export regime; other countries' formats
/// will mint their own sequences.
pub const SIEX_TARGET: &str = "siex";

/// Build the descriptor for one farm+season. Takes `&mut Connection` because
/// first-time exports mint aliases (transactional inserts); re-exports only
/// read and produce byte-identical output.
pub fn build_cuaderno(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<CuadernoExport> {
    if !export_precheck(conn, season_id, farm_id)?.is_clean() {
        return Err(SiexError::Invalid("export_precheck_failed"));
    }

    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;
    // The precheck just guaranteed these; the fallbacks only keep the
    // no-unwrap rule honest.
    let missing = || SiexError::Invalid("export_precheck_failed");
    let owner_tax_id = farm
        .farm
        .owner_tax_id
        .ok_or_else(missing)?
        .trim()
        .to_string();
    let es = farm.es.ok_or_else(missing)?;
    let autonomous_community =
        module_cue::siex::province_to_ccaa(es.province_code.as_deref().unwrap_or(""))
            .ok_or_else(missing)?
            .to_string();
    let rea_code = es.rea_code.ok_or_else(missing)?.trim().to_string();

    let tratam_fito = blocks::tratam_fito::build(conn, season_id, farm_id, actor)?;
    // One pass over the non-field register fills two blocks: which one a record
    // lands in is its subject kind, exactly as on the printed page.
    let non_field = blocks::non_field::build(conn, season_id, farm_id, actor)?;
    let uso_semilla_tratada = blocks::uso_semilla_tratada::build(conn, season_id, farm_id, actor)?;
    let analitica = blocks::analitica::build(conn, season_id, farm_id, actor)?;
    let comercializacion_vd = blocks::comercializacion_vd::build(conn, season_id, farm_id, actor)?;
    // Reads TWO registers: core's sowing record for the dates, plots and
    // amount, and module-cue's treated seed for the provenance members the
    // format hangs off the sowing.
    let siembra_plantacion = blocks::siembra_plantacion::build(conn, season_id, farm_id, actor)?;
    // module-fertilisation's three. `Fertilizacion` reads the irrigation
    // register too, for the fertigations whose water side the format asks it to
    // restate.
    let fertilizacion = blocks::fertilizacion::build(conn, season_id, farm_id, actor)?;
    let riego = blocks::riego::build(conn, season_id, farm_id, actor)?;
    let plan_abonado = blocks::plan_abonado::build(conn, season_id, farm_id, actor)?;
    // module-ecoscheme's three. Their junctions carry a plot and no crop, so
    // each DGC resolves its crop from the plot and the season — the rule Anexo V
    // asks for in as many words ("campo calculado") and the one piece of this
    // arc that was not wiring.
    let pastoreo = blocks::pastoreo::build(conn, season_id, farm_id, actor)?;
    let labores_culturales = blocks::labores_culturales::build(conn, season_id, farm_id, actor)?;
    let datos_cubierta = blocks::datos_cubierta::build(conn, season_id, farm_id, actor)?;

    Ok(CuadernoExport {
        cuaderno: vec![CuadernoEntry {
            ca_explotacion: autonomous_community,
            id_titular: owner_tax_id.clone(),
            codigo_rea: rea_code,
            // Titular-driven notebook: the managing entity is the titular
            // (docs/siex-export.md → open question 7).
            unidad_gestora: owner_tax_id,
            actividades_explotacion: ActividadesExplotacion {
                tratam_fito,
                tratamientos_post_cosecha: non_field.post_cosecha,
                tratamientos_edif_instalaciones: non_field.edificaciones,
                uso_semilla_tratada,
                analitica,
                comercializacion_vd,
                siembra_plantacion,
                fertilizacion,
                riego,
                plan_abonado,
                pastoreo,
                labores_culturales,
                datos_cubierta,
            },
        }],
    })
}
