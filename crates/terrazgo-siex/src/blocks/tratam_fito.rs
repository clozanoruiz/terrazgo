// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `TratamFito` — field phytosanitary treatments (model section 3.1).
//!
//! The one block that was ever emitted, and the pattern the others follow: read
//! the register including its soft-deleted rows, mint or look up a frozen alias
//! per emitted entry, map every code through the owning module's `siex`, and
//! push one descriptor value per entry.
//!
//! Its own peculiarity is the split: the 3.11.4 descriptor constrains all DGCs
//! in one entry to share the crop, so a multi-crop record becomes several
//! entries — the same split `module_cue::crop_groups` gives the printed book,
//! which is why that function lives in the module both documents read.
//!
//! It is also the only block whose two halves are alternatives. `ProductosFito`
//! and `OtrasActuacionesFito` are *excluyente* per Anexo V, and the register
//! allows a record to carry both because model 3.1 bis prints them side by
//! side — so the precheck refuses that record and this builder emits exactly
//! one of the two.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::crop_groups;
use module_cue::models::{
    TreatmentPlot, TreatmentProblem, TreatmentRecord, TreatmentRecordWithPlots,
};
use module_cue::repository::list_treatment_records_for_export;
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

/// Every `TratamFito` entry for this farm+season, in register order.
pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<Vec<TratamFito>> {
    let mut out = Vec::new();
    for record in list_treatment_records_for_export(conn, season_id, farm_id)? {
        append_record(conn, &mut out, &record, actor)?;
    }
    Ok(out)
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
        .ok_or(SiexError::Invalid("export_code_unmappable"))?;
    // The format demands both ends of the actuation. A treatment that ran over
    // several days states its last one; a single-day one repeats its start,
    // which is the same statement.
    let application_end_date = match &record.application_end_date {
        Some(end) => siex::date_to_siex(end).ok_or(SiexError::Invalid("export_code_unmappable"))?,
        None => application_date.clone(),
    };
    let problems = problem_buckets(&rec.problems)?;
    let justifications = rec
        .justifications
        .iter()
        .map(|j| {
            siex::justification_to_siex(&j.justification_code)
                .map(|code| Justificacion { just_act: code })
                .ok_or(SiexError::Invalid("export_code_unmappable"))
        })
        .collect::<Result<Vec<_>>>()?;
    let drying_date = match &record.drying_date {
        Some(dried) => {
            Some(siex::date_to_siex(dried).ok_or(SiexError::Invalid("export_code_unmappable"))?)
        }
        None => None,
    };
    let products = product_blocks(conn, rec)?;
    let measure = measure_block(record)?;
    let advisor = advisor_block(record);
    let applicator = vec![applicator_block(rec)];
    let efficacy = match &record.efficacy_code {
        Some(code) => {
            siex::efficacy_to_siex(code).ok_or(SiexError::Invalid("export_code_unmappable"))?
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
            fecha_seca: drying_date.clone(),
            dgcs,
            problematica_fito: problems.clone(),
            justificaciones: justifications.clone(),
            otras_actuaciones_fito: measure.clone(),
            productos_fito: products.clone(),
            identificador_aplicador: applicator.clone(),
            asesor_validacion: advisor.clone(),
            eficacia: efficacy,
            observaciones: record.notes.clone(),
        });
    }
    Ok(())
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
            .map_err(|_| SiexError::Invalid("export_code_unmappable"))?;
        let bucket = match problem.reason_category_code.as_str() {
            "disease" => &mut diseases,
            "pest" => &mut pests,
            "weed" => &mut weeds,
            "growth_regulator" | "other" => &mut regulators,
            _ => return Err(SiexError::Invalid("export_code_unmappable")),
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

/// The record's product as `ProductosFito` (a record has at most one).
/// The authorisation kind is resolved live by the frozen authorisation
/// number — the number is what the record legally cites; when the
/// authorisation row no longer matches it, the default kind (registered)
/// applies, which is also what the pre-kind_code rows were.
///
/// **Empty for a purely non-chemical actuation**, whose entry then omits the
/// member and carries `OtrasActuacionesFito` instead. The chemical block is
/// all-or-nothing at the schema level (a table CHECK), so an absent dose here
/// means an absent product rather than a half-filled record.
fn product_blocks(conn: &Connection, rec: &TreatmentRecordWithPlots) -> Result<Vec<ProductoFito>> {
    let record = &rec.record;
    if record.product_id.is_none() {
        return Ok(Vec::new());
    }
    let (Some(dose_value), Some(dose_unit_code)) = (record.dose_value, &record.dose_unit_code)
    else {
        return Err(SiexError::Invalid("export_code_unmappable"));
    };
    let (unit, factor) =
        siex::unit_to_siex(dose_unit_code).ok_or(SiexError::Invalid("export_code_unmappable"))?;

    let (authorisation_type, active_substance) = crate::blocks::authorisation_product_kind(
        conn,
        record.product_id.as_deref(),
        &record.country_code,
        record.authorisation_number_snapshot.as_deref(),
    )?;

    Ok(vec![ProductoFito {
        tipo_producto: authorisation_type,
        num_registro: record.authorisation_number_snapshot.clone(),
        materia_activa: active_substance,
        dosis: dose_value * factor,
        unidad: unit,
    }])
}

/// The non-chemical measure as `OtrasActuacionesFito`, or `None` when the
/// actuation was a plain product application.
///
/// Every value it needs is precheck-guaranteed: the measure code parses, the
/// intensity pair is stated, its unit maps, and an MDF number is present
/// wherever Anexo V demands one. The fallbacks below only keep the no-unwrap
/// rule honest, the same shape the rest of this module uses.
fn measure_block(record: &TreatmentRecord) -> Result<Option<OtrasActuacionesFito>> {
    let Some(code) = &record.measure_code else {
        return Ok(None);
    };
    let unmappable = || SiexError::Invalid("export_code_unmappable");
    let tipo_medida: i64 = code.trim().parse().map_err(|_| unmappable())?;
    let (Some(cantidad), Some(unit_code)) = (
        record.measure_intensity_value,
        &record.measure_intensity_unit_code,
    ) else {
        return Err(unmappable());
    };
    Ok(Some(OtrasActuacionesFito {
        tipo_medida,
        cantidad,
        unidad: siex::intensity_unit_to_siex(unit_code).ok_or_else(unmappable)?,
        // Sent whenever stored, whatever the measure kind: Anexo V's "en caso
        // contrario el campo debe ir vacío" rests on a list Anexo VI states
        // differently, and no decree names the field at all — so the demand is
        // enforced (in the precheck) and the emptying is not.
        num_registro_mdf: record
            .measure_registration_number
            .as_deref()
            .map(str::trim)
            .filter(|number| !number.is_empty())
            .map(str::to_string),
    }))
}

/// The advisor the record names, as `AsesorValidacion`. Only `NumROPO` is sent
/// — `Validacion`, `Confirmacion`, `Contrato`, `Fecha` and `Observaciones`
/// describe a sign-off the book cannot hold, model 3.1 bis asking for a
/// handwritten signature and this app having no signature capability by design.
///
/// A record naming an advisor whose ROPO is absent is refused by the precheck
/// rather than emitted without the block, so this returning `None` means the
/// record named nobody.
fn advisor_block(record: &TreatmentRecord) -> Option<AsesorValidacion> {
    record
        .advisor_registration_snapshot
        .as_deref()
        .map(str::trim)
        .filter(|ropo| !ropo.is_empty())
        .map(|ropo| AsesorValidacion {
            num_ropo: ropo.to_string(),
        })
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
