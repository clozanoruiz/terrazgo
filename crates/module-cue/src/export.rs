// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! SIEX-aligned cuaderno export (docs/siex-export.md): turns one farm+season
//! into the official CUE descriptor JSON, `TratamFito` block.
//!
//! Two public entry points:
//!   * [`export_precheck`] — what blocks a valid export, for the UI to list
//!     (records missing efficacy or an operator licence, treated plots without
//!     a crop, farm identity fields not yet entered from the REA papers).
//!   * [`build_cuaderno`] — the export itself; refuses while the precheck is
//!     not clean, so nothing is ever silently dropped or invented.
//!
//! Serialization rules (each pinned by tests/export.rs against the vendored
//! schema): a multi-crop treatment splits into one `TratamFito` per crop
//! snapshot (3.11.4 descriptor rule), every entry carries a frozen integer
//! alias (`export_alias` — SIEX keys edits/deletes on it), dates render
//! dd/mm/yyyy, and all codes map through `siex`. Soft-deleted records emit
//! `Borrar` entries under their existing aliases; never-exported deletions
//! leave no trace. DGCs are referenced by client-assigned codes
//! (`CodigoDGCAjena`, one alias per crop row — a core `crop` IS the SIEX
//! plot+crop+season unit) while gap 2 (REA `CodigoDGC` import) stays open.

pub mod descriptor;

use crate::error::{CueError, Result};
use crate::models::{TreatmentPlot, TreatmentProblem, TreatmentRecordWithPlots};
use crate::repository::{
    ensure_export_alias, find_export_alias, list_treatment_records,
    list_treatment_records_for_export,
};
use crate::siex;
use descriptor::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// `export_alias.target` for this export regime; other countries' formats
/// will mint their own sequences.
pub const SIEX_TARGET: &str = "siex";

// ---------------------------------------------------------------------------
// Precheck
// ---------------------------------------------------------------------------

/// A treatment record the precheck points at, with enough context for a list
/// row ("01/05/2026 — Fungitop").
#[derive(Debug, Clone, Serialize)]
pub struct RecordRef {
    pub treatment_record_id: String,
    pub application_date: String,
    /// `None` for a purely non-chemical actuation, which has no product to
    /// name. The caller supplies its own wording for that case — this layer
    /// resolves no catalogue labels.
    pub product_name: Option<String>,
}

/// A treated plot without a crop — the export cannot name the DGC unit.
#[derive(Debug, Clone, Serialize)]
pub struct PlotRef {
    pub treatment_record_id: String,
    pub application_date: String,
    pub plot_id: String,
    pub plot_name: String,
}

/// Everything that blocks a schema-valid export of this farm+season. The
/// fields the farmer must fill are listed rather than errored one at a time.
#[derive(Debug, Clone, Serialize)]
pub struct ExportPrecheck {
    /// Farm identity fields still missing (or unusable): `owner_tax_id`,
    /// `rea_code`, `province_code` — all user-entered from the REA papers.
    pub farm_missing_fields: Vec<&'static str>,
    /// Efficacy is observed after application and nullable at insert; the
    /// schema requires it, so it must be recorded before exporting.
    pub records_missing_efficacy: Vec<RecordRef>,
    /// `AplicadorEmpresa.NumROPO` comes from the operator licence snapshot.
    pub records_missing_operator_licence: Vec<RecordRef>,
    pub plots_missing_crop: Vec<PlotRef>,
    /// Actuations carrying a non-chemical measure, which this serializer cannot
    /// yet represent: the twin carries them in `OtrasActuacionesFito` and
    /// nothing writes that block (see the dormant-export inventory in
    /// docs/siex-export.md).
    ///
    /// Both shapes are refused, and for the same reason. A PURELY non-chemical
    /// actuation would emit a `TratamFito` with an empty `ProductosFito` — a
    /// record asserting that a treatment happened while naming nothing that was
    /// done. A MIXED one (a spray and a measure on the same record) would
    /// export cleanly while silently losing the measure, which is the worse of
    /// the two: nothing in the output would say anything was left behind.
    /// Refusing with a nameable list is the call the builder already makes
    /// elsewhere — an export that silently drops data is worse than one that
    /// says what it cannot carry.
    pub records_with_non_chemical_measure: Vec<RecordRef>,
}

impl ExportPrecheck {
    pub fn is_clean(&self) -> bool {
        self.farm_missing_fields.is_empty()
            && self.records_missing_efficacy.is_empty()
            && self.records_missing_operator_licence.is_empty()
            && self.plots_missing_crop.is_empty()
            && self.records_with_non_chemical_measure.is_empty()
    }
}

/// List what blocks a valid SIEX export of this farm+season. Only active
/// records are checked: deletion entries identify a previously exported
/// activity and cannot demand new observations.
pub fn export_precheck(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
) -> Result<ExportPrecheck> {
    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;

    let mut farm_missing_fields = Vec::new();
    if is_blank(farm.farm.owner_tax_id.as_deref()) {
        farm_missing_fields.push("owner_tax_id");
    }
    let es = farm.es.as_ref();
    // The REA code is exactly 14 characters (schema minLength = maxLength =
    // 14, the national ES+12-digit registry format); anything else blocks the
    // export the same way as an absent one.
    let rea_code = es.and_then(|e| e.rea_code.as_deref()).unwrap_or("").trim();
    if rea_code.len() != 14 {
        farm_missing_fields.push("rea_code");
    }
    // Present but unmappable (not an INE province) blocks the same way as
    // absent: CAExplotacion cannot be derived from it.
    let province = es.and_then(|e| e.province_code.as_deref()).unwrap_or("");
    if siex::province_to_ccaa(province).is_none() {
        farm_missing_fields.push("province_code");
    }

    let mut records_missing_efficacy = Vec::new();
    let mut records_missing_operator_licence = Vec::new();
    let mut plots_missing_crop = Vec::new();
    let mut records_with_non_chemical_measure = Vec::new();
    for rec in list_treatment_records(conn, season_id, farm_id)? {
        let record_ref = || RecordRef {
            treatment_record_id: rec.record.id.clone(),
            application_date: rec.record.application_date.clone(),
            product_name: rec.record.product_name_snapshot.clone(),
        };
        // The schema CHECK `(product_id IS NOT NULL OR measure_code IS NOT
        // NULL)` makes the first clause imply the second, so this is exactly
        // "every record carrying a measure". Both are written anyway: a
        // regulatory export must not depend silently on a schema invariant
        // holding, and if that CHECK is ever relaxed a productless record has
        // to keep being refused.
        if rec.record.product_id.is_none() || rec.record.measure_code.is_some() {
            records_with_non_chemical_measure.push(record_ref());
        }
        if rec.record.efficacy_code.is_none() {
            records_missing_efficacy.push(record_ref());
        }
        if is_blank(rec.record.operator_licence_snapshot.as_deref()) {
            records_missing_operator_licence.push(record_ref());
        }
        for plot in &rec.plots {
            if plot.crop_id.is_none() {
                let plot_name: String = conn.query_row(
                    "SELECT name FROM plot WHERE id = ?1",
                    [&plot.plot_id],
                    |r| r.get(0),
                )?;
                plots_missing_crop.push(PlotRef {
                    treatment_record_id: rec.record.id.clone(),
                    application_date: rec.record.application_date.clone(),
                    plot_id: plot.plot_id.clone(),
                    plot_name,
                });
            }
        }
    }

    Ok(ExportPrecheck {
        farm_missing_fields,
        records_missing_efficacy,
        records_missing_operator_licence,
        plots_missing_crop,
        records_with_non_chemical_measure,
    })
}

fn is_blank(value: Option<&str>) -> bool {
    value.unwrap_or("").trim().is_empty()
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

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
        return Err(CueError::Invalid("export_precheck_failed"));
    }

    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;
    // The precheck just guaranteed these; the fallbacks only keep the
    // no-unwrap rule honest.
    let missing = || CueError::Invalid("export_precheck_failed");
    let owner_tax_id = farm
        .farm
        .owner_tax_id
        .ok_or_else(missing)?
        .trim()
        .to_string();
    let es = farm.es.ok_or_else(missing)?;
    let autonomous_community = siex::province_to_ccaa(es.province_code.as_deref().unwrap_or(""))
        .ok_or_else(missing)?
        .to_string();
    let rea_code = es.rea_code.ok_or_else(missing)?.trim().to_string();

    let mut activities = Vec::new();
    for record in list_treatment_records_for_export(conn, season_id, farm_id)? {
        append_record(conn, &mut activities, &record, actor)?;
    }

    Ok(CuadernoExport {
        cuaderno: vec![CuadernoEntry {
            ca_explotacion: autonomous_community,
            id_titular: owner_tax_id.clone(),
            codigo_rea: rea_code,
            // Titular-driven notebook: the managing entity is the titular
            // (docs/siex-export.md → open question 7).
            unidad_gestora: owner_tax_id,
            actividades_explotacion: ActividadesExplotacion {
                tratam_fito: activities,
            },
        }],
    })
}

/// Serialize one record into its `TratamFito` entries (one per crop group).
/// Soft-deleted records contribute `Borrar` entries for the groups that were
/// previously exported and nothing for the rest.
fn append_record(
    conn: &mut Connection,
    out: &mut Vec<TratamFito>,
    rec: &TreatmentRecordWithPlots,
    actor: Option<&str>,
) -> Result<()> {
    let record = &rec.record;
    let deleted = record.deleted_at.is_some();

    let application_date = siex::date_to_siex(&record.application_date)
        .ok_or(CueError::Invalid("export_code_unmappable"))?;
    // The format demands both ends of the actuation. A treatment that ran over
    // several days states its last one; a single-day one repeats its start,
    // which is the same statement.
    let application_end_date = match &record.application_end_date {
        Some(end) => siex::date_to_siex(end).ok_or(CueError::Invalid("export_code_unmappable"))?,
        None => application_date.clone(),
    };
    let problems = problem_buckets(&rec.problems)?;
    let justifications = rec
        .justifications
        .iter()
        .map(|j| {
            siex::justification_to_siex(&j.justification_code)
                .map(|code| Justificacion { just_act: code })
                .ok_or(CueError::Invalid("export_code_unmappable"))
        })
        .collect::<Result<Vec<_>>>()?;
    let products = product_blocks(conn, rec)?;
    let applicator = vec![applicator_block(rec)];
    let efficacy = match &record.efficacy_code {
        Some(code) => {
            siex::efficacy_to_siex(code).ok_or(CueError::Invalid("export_code_unmappable"))?
        }
        // Only reachable on deletion entries (the precheck demands efficacy on
        // active records): the entry exists to identify the deleted activity,
        // so the schema default stands in for the never-observed value.
        None => 0,
    };

    let groups = crop_groups(&rec.plots);
    let split = groups.len() > 1;
    for (group_key, plots) in groups {
        // A record that fits one TratamFito keeps the empty split key (the
        // 1:1 case); only real splits discriminate by crop. Snapshots are
        // frozen at insert, so the grouping can never drift between exports.
        let split_key = if split { group_key } else { String::new() };
        let alias = if deleted {
            match find_export_alias(
                conn,
                SIEX_TARGET,
                "treatment_record",
                &record.id,
                &split_key,
            )? {
                Some(alias) => alias,
                None => continue, // never exported — nothing to delete
            }
        } else {
            ensure_export_alias(
                conn,
                SIEX_TARGET,
                "treatment_record",
                &record.id,
                &split_key,
                actor,
            )?
        };

        let dgcs = plots
            .iter()
            .map(|plot| dgc(conn, plot, deleted, actor))
            .collect::<Result<Vec<_>>>()?;

        out.push(TratamFito {
            id_ajena_tratam_fito: alias,
            borrar: deleted.then_some(true),
            fecha_inicio: application_date.clone(),
            fecha_fin: application_end_date.clone(),
            // Anexo VI wants HH:MM:SS; the record holds the HH:MM a farmer
            // actually writes down, so the seconds are padded here rather than
            // stored — the same shaping the dates get.
            hora_tratamiento: record
                .application_time
                .as_deref()
                .map(|time| format!("{time}:00")),
            dgcs,
            problematica_fito: problems.clone(),
            justificaciones: justifications.clone(),
            productos_fito: products.clone(),
            identificador_aplicador: applicator.clone(),
            eficacia: efficacy,
            observaciones: record.notes.clone(),
        });
    }
    Ok(())
}

/// Group treated plots by crop snapshot — the 3.11.4 descriptor constraint
/// ("all DGCs in one TratamFito share the crop") splits a multi-crop record
/// into one entry per group. Sorted by key so output order is deterministic.
/// `pub(crate)`: the printable cuaderno (src/report.rs) prints one register
/// row per group, so both outputs split identically.
pub fn crop_groups(plots: &[TreatmentPlot]) -> Vec<(String, Vec<&TreatmentPlot>)> {
    let mut groups: Vec<(String, Vec<&TreatmentPlot>)> = Vec::new();
    for plot in plots {
        // \u{1F} (unit separator) never appears in species/variety text, so
        // the concatenated key cannot collide across groups.
        let key = format!(
            "{}\u{1F}{}",
            plot.crop_name_snapshot.as_deref().unwrap_or(""),
            plot.variety_snapshot.as_deref().unwrap_or("")
        );
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(plot),
            None => groups.push((key, vec![plot])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    groups
}

/// One DGC reference: the crop row's frozen alias (a core `crop` IS the SIEX
/// plot+crop+season unit) plus the surface actually treated on that plot.
fn dgc(
    conn: &mut Connection,
    plot: &TreatmentPlot,
    deleted: bool,
    actor: Option<&str>,
) -> Result<Dgc> {
    let crop_alias = match &plot.crop_id {
        Some(crop_id) if deleted => find_export_alias(conn, SIEX_TARGET, "crop", crop_id, "")?,
        Some(crop_id) => Some(ensure_export_alias(
            conn,
            SIEX_TARGET,
            "crop",
            crop_id,
            "",
            actor,
        )?),
        // Active records are precheck-blocked; a deletion entry may lack the
        // crop and still identify the activity by its own alias.
        None => None,
    };
    Ok(Dgc {
        codigo_dgc_ajena: crop_alias,
        superficie: plot.surface_treated_ha,
        // The catalogue's own code, which the schema types as an integer. An
        // unparseable one is dropped rather than refused: the field is optional
        // in the format, and a code the vendored snapshot cannot type is not a
        // reason to fail an export of everything else.
        estado_fenologico: plot
            .growth_stage_code
            .as_deref()
            .and_then(|code| code.parse::<i64>().ok()),
    })
}

/// Sort the coded problems into the four export buckets, deduplicating within
/// each (growth_regulator and other share ReguladoresOtros).
fn problem_buckets(problems: &[TreatmentProblem]) -> Result<ProblematicaFito> {
    let mut diseases: Vec<i64> = Vec::new();
    let mut pests: Vec<i64> = Vec::new();
    let mut weeds: Vec<i64> = Vec::new();
    let mut regulators: Vec<i64> = Vec::new();
    for problem in problems {
        let code: i64 = problem
            .problem_code
            .trim()
            .parse()
            .map_err(|_| CueError::Invalid("export_code_unmappable"))?;
        let bucket = match problem.reason_category_code.as_str() {
            "disease" => &mut diseases,
            "pest" => &mut pests,
            "weed" => &mut weeds,
            "growth_regulator" | "other" => &mut regulators,
            _ => return Err(CueError::Invalid("export_code_unmappable")),
        };
        if !bucket.contains(&code) {
            bucket.push(code);
        }
    }
    Ok(ProblematicaFito {
        enfermedades: (!diseases.is_empty()).then_some(Enfermedades {
            tipo_enfermedad: diseases,
        }),
        artropodos_gasteropodos: (!pests.is_empty())
            .then_some(ArtropodosGasteropodos { tipo_plaga: pests }),
        malas_hierbas: (!weeds.is_empty()).then_some(MalasHierbas {
            tipo_mala_hierba: weeds,
        }),
        reguladores_otros: (!regulators.is_empty()).then_some(ReguladoresOtros {
            tipo_regulador: regulators,
        }),
    })
}

/// The record's product as `ProductosFito` (every record has exactly one).
/// The authorisation kind is resolved live by the frozen authorisation
/// number — the number is what the record legally cites; when the
/// authorisation row no longer matches it, the default kind (registered)
/// applies, which is also what the pre-kind_code rows were.
fn product_blocks(conn: &Connection, rec: &TreatmentRecordWithPlots) -> Result<Vec<ProductoFito>> {
    let record = &rec.record;
    // The chemical block is all-or-nothing in the schema, and `export_precheck`
    // refuses a build whose records carry any non-chemical measure — so by the
    // time a descriptor is being written, a dose is present.
    let (Some(dose_value), Some(dose_unit_code)) = (record.dose_value, &record.dose_unit_code)
    else {
        return Err(CueError::Invalid("export_code_unmappable"));
    };
    let (unit, factor) =
        siex::unit_to_siex(dose_unit_code).ok_or(CueError::Invalid("export_code_unmappable"))?;

    let kind: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT kind_code, exceptional_substance_code FROM product_authorisation
             WHERE product_id = ?1 AND country_code = ?2 AND authorisation_number = ?3",
            params![
                record.product_id,
                record.country_code,
                record.authorisation_number_snapshot
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (kind_code, exceptional_substance) = kind.unwrap_or(("registered".to_string(), None));
    let authorisation_type = siex::authorisation_kind_to_siex(&kind_code)
        .ok_or(CueError::Invalid("export_code_unmappable"))?;
    let active_substance = if kind_code == "exceptional" {
        // Mandatory exactly for TipoProducto 4 (3.11.4 re-diff): the
        // AUTORIZACION_EXCP catalogue code, required at authorisation entry.
        let code = exceptional_substance.ok_or(CueError::Invalid("export_code_unmappable"))?;
        Some(
            code.trim()
                .parse::<i64>()
                .map_err(|_| CueError::Invalid("export_code_unmappable"))?,
        )
    } else {
        None
    };

    Ok(vec![ProductoFito {
        tipo_producto: authorisation_type,
        num_registro: record.authorisation_number_snapshot.clone(),
        materia_activa: active_substance,
        dosis: dose_value * factor,
        unidad: unit,
    }])
}

/// Operator + equipment identity from the record's frozen snapshots. The
/// schema demands exactly one equipment identifier (`oneOf`) even for manual
/// application, hence the fixed sentinel; machinery in neither ROMA nor
/// REGANIP is named by its stable row id (`IdEquipoAplicador` is a free
/// string(50), and the UUID never drifts between exports).
fn applicator_block(rec: &TreatmentRecordWithPlots) -> IdentificadorAplicador {
    let record = &rec.record;
    let licence_number = match &record.operator_licence_snapshot {
        Some(licence) if !licence.trim().is_empty() => licence.clone(),
        // Active records are precheck-blocked; a deletion entry only needs to
        // identify the activity, so the schema-valid empty string stands in.
        _ => String::new(),
    };
    let (roma, reganip, equipment_id) = if record.machinery_id.is_none() {
        (None, None, Some("manual".to_string()))
    } else if record.machinery_roma_snapshot.is_some() {
        // ROMA preferred when a machine carries both numbers ("nunca ambos").
        (record.machinery_roma_snapshot.clone(), None, None)
    } else if record.machinery_reganip_snapshot.is_some() {
        (None, record.machinery_reganip_snapshot.clone(), None)
    } else {
        (None, None, record.machinery_id.clone())
    };
    IdentificadorAplicador {
        aplicador_empresa: AplicadorEmpresa {
            num_ropo: licence_number,
        },
        equipo_aplicador: EquipoAplicador {
            num_roma: roma,
            num_reganip: reganip,
            id_equipo_aplicador: equipment_id,
            aplicacion_manual: record.machinery_id.is_none(),
        },
    }
}
