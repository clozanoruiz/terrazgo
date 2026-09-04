// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `TratamientosPostCosecha` and `TratamientosEdifInstalaciones` — model
//! sections 3.3, 3.4 and 3.5.
//!
//! One register feeds two blocks: `non_field_treatment.subject_kind_code`
//! decides which, exactly as it decides which page prints the record. The two
//! blocks share this module because they share almost every field and differ in
//! precisely the three ways the descriptor types record — what identifies the
//! subject, how much of it there was, and which problem buckets exist.
//!
//! Both refuse rather than approximate. A record whose problem category the
//! block cannot express is caught by the precheck, never dropped: `MalasHierbas`
//! exists in neither block, and a building's block has no `ReguladoresOtros`
//! either, so a weed treatment in a warehouse has nowhere to go and the export
//! says so.

use crate::SIEX_TARGET;
use crate::descriptor::*;
use crate::error::{Result, SiexError};
use module_cue::models::{NonFieldTreatment, NonFieldTreatmentDetail, NonFieldTreatmentProblem};
use module_cue::repository::list_non_field_treatments_for_export;
use module_cue::siex;
use rusqlite::Connection;
use terrazgo_core::repository::{ensure_export_alias, find_export_alias};

/// Both blocks in one pass over the register, so it is read once.
pub struct NonFieldBlocks {
    pub post_cosecha: Vec<TratamientoPostCosecha>,
    pub edificaciones: Vec<TratamientoEdifInstalaciones>,
}

pub fn build(
    conn: &mut Connection,
    season_id: &str,
    farm_id: &str,
    actor: Option<&str>,
) -> Result<NonFieldBlocks> {
    let mut out = NonFieldBlocks {
        post_cosecha: Vec::new(),
        edificaciones: Vec::new(),
    };
    for detail in list_non_field_treatments_for_export(conn, season_id, farm_id)? {
        match detail.record.subject_kind_code.as_str() {
            "postharvest" => {
                if let Some(entry) = post_cosecha_entry(conn, &detail, actor)? {
                    out.post_cosecha.push(entry);
                }
            }
            "storage_premises" | "transport" => {
                if let Some(entry) = edificacion_entry(conn, &detail, actor)? {
                    out.edificaciones.push(entry);
                }
            }
            _ => return Err(SiexError::Invalid("export_code_unmappable")),
        }
    }
    Ok(out)
}

fn post_cosecha_entry(
    conn: &mut Connection,
    detail: &NonFieldTreatmentDetail,
    actor: Option<&str>,
) -> Result<Option<TratamientoPostCosecha>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let Some(alias) = alias_for(conn, record, "non_field_treatment", deleted, actor)? else {
        return Ok(None);
    };

    // The produce treated, in kilograms: the block has no unit member and Anexo
    // V fixes it, while the register stores the tonnes model 3.3 prints.
    let quantity = match (
        record.treated_quantity_value,
        &record.treated_quantity_unit_code,
    ) {
        (Some(value), Some(unit)) => {
            siex::mass_in_kg(value, unit).ok_or(SiexError::Invalid("export_code_unmappable"))?
        }
        // Precheck-blocked on active records; a deletion entry only identifies
        // the activity it withdraws.
        _ if deleted => 0.0,
        _ => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let plant_product = match record.subject_product_code.as_deref() {
        Some(code) => code
            .trim()
            .parse::<i64>()
            .map_err(|_| SiexError::Invalid("export_code_unmappable"))?,
        _ if deleted => 0,
        _ => return Err(SiexError::Invalid("export_code_unmappable")),
    };

    let buckets = problem_buckets(&detail.problems)?;
    Ok(Some(TratamientoPostCosecha {
        id_ajena_tratam_postco: alias,
        borrar: deleted.then_some(true),
        fecha_actuacion: date_of(record)?,
        producto_vegetal: plant_product,
        cantidad: quantity,
        problematica_fito: ProblematicaPostCosecha {
            enfermedades: buckets.diseases(),
            artropodos_gasteropodos: buckets.pests(),
            reguladores_otros: buckets.regulators(),
        },
        justificaciones: justifications(detail)?,
        productos_fito: products(conn, record, deleted)?,
        identificador_aplicador: vec![applicator(record)],
        asesor_validacion: advisor(record),
        eficacia: efficacy(record)?,
        observaciones: record.notes.clone(),
    }))
}

fn edificacion_entry(
    conn: &mut Connection,
    detail: &NonFieldTreatmentDetail,
    actor: Option<&str>,
) -> Result<Option<TratamientoEdifInstalaciones>> {
    let record = &detail.record;
    let deleted = record.deleted_at.is_some();
    let Some(alias) = alias_for(conn, record, "non_field_treatment", deleted, actor)? else {
        return Ok(None);
    };

    let buckets = problem_buckets(&detail.problems)?;
    Ok(Some(TratamientoEdifInstalaciones {
        id_ajena_tratam_edif: alias,
        borrar: deleted.then_some(true),
        edificaciones: vec![building(conn, record, deleted)?],
        fecha_actuacion: date_of(record)?,
        problematica_fito: ProblematicaEdificacion {
            enfermedades: buckets.diseases(),
            artropodos_gasteropodos: buckets.pests(),
        },
        justificaciones: justifications(detail)?,
        productos_fito: products(conn, record, deleted)?,
        identificador_aplicador: vec![applicator(record)],
        asesor_validacion: advisor(record),
        eficacia: efficacy(record)?,
        observaciones: record.notes.clone(),
    }))
}

/// The building treated, by REA's own code for it.
///
/// The registry row is where that code lives, and the precheck demands it — so
/// an active record always resolves. A deletion entry may name a premises whose
/// code was since cleared, and it still has to identify the activity, so it
/// falls back to the schema-valid 0 rather than refusing the withdrawal.
fn building(conn: &Connection, record: &NonFieldTreatment, deleted: bool) -> Result<Edificacion> {
    let code = match record.premises_id.as_deref() {
        Some(id) => {
            let detail = terrazgo_core::repository::get_premises_detail(conn, id)?;
            detail
                .es
                .and_then(|es| es.rea_installation_code)
                .and_then(|code| code.trim().parse::<i64>().ok())
        }
        None => None,
    };
    let id_edificacion = match code {
        Some(code) => code,
        None if deleted => 0,
        None => return Err(SiexError::Invalid("export_code_unmappable")),
    };

    // B.f's treated volume, and the unit it was measured in. Absent is normal:
    // the descriptor asks for it only "en caso de que no se haya tratado el
    // total del volumen del edificio".
    let (volumen, unidad) = match (
        record.treated_quantity_value,
        &record.treated_quantity_unit_code,
    ) {
        (Some(value), Some(unit)) => {
            let code = siex::quantity_unit_to_siex(unit)
                .ok_or(SiexError::Invalid("export_code_unmappable"))?;
            (Some(value), Some(code))
        }
        _ => (None, None),
    };
    Ok(Edificacion {
        id_edificacion,
        volumen,
        unidad,
    })
}

/// Mint or find the frozen alias for this record. `None` means a soft-deleted
/// record that was never exported — nothing to withdraw, so no entry at all.
///
/// No split key: unlike a field treatment, one non-field record is always one
/// entry (it treats one subject), so the alias keys on the record alone.
fn alias_for(
    conn: &mut Connection,
    record: &NonFieldTreatment,
    entity_table: &str,
    deleted: bool,
    actor: Option<&str>,
) -> Result<Option<i64>> {
    if deleted {
        Ok(find_export_alias(
            conn,
            SIEX_TARGET,
            entity_table,
            &record.id,
            "",
        )?)
    } else {
        Ok(Some(ensure_export_alias(
            conn,
            SIEX_TARGET,
            entity_table,
            &record.id,
            "",
            actor,
        )?))
    }
}

fn date_of(record: &NonFieldTreatment) -> Result<String> {
    siex::date_to_siex(&record.treated_on).ok_or(SiexError::Invalid("export_code_unmappable"))
}

fn justifications(detail: &NonFieldTreatmentDetail) -> Result<Vec<Justificacion>> {
    detail
        .justifications
        .iter()
        .map(|j| {
            siex::justification_to_siex(&j.justification_code)
                .map(|just_act| Justificacion { just_act })
                .ok_or(SiexError::Invalid("export_code_unmappable"))
        })
        .collect()
}

fn efficacy(record: &NonFieldTreatment) -> Result<i64> {
    match &record.efficacy_code {
        Some(code) => {
            siex::efficacy_to_siex(code).ok_or(SiexError::Invalid("export_code_unmappable"))
        }
        // Only reachable on deletion entries: the precheck demands efficacy on
        // active records, and a withdrawal cannot invent an observation.
        None => Ok(0),
    }
}

/// The product used and how much of it. `Cantidad`, never `Dosis`: these
/// registers record an amount (model 3.3-3.5's "Cantidad utilizada, kg o l"),
/// which is what the post-harvest block requires and what the buildings block
/// accepts.
fn products(
    conn: &Connection,
    record: &NonFieldTreatment,
    deleted: bool,
) -> Result<Vec<ProductoFitoCantidad>> {
    let (quantity, unit_code) = match (
        record.product_quantity_value,
        &record.product_quantity_unit_code,
    ) {
        (Some(value), Some(unit)) => (value, unit.clone()),
        _ if deleted => (0.0, "kg".to_string()),
        _ => return Err(SiexError::Invalid("export_code_unmappable")),
    };
    let unidad = siex::quantity_unit_to_siex(&unit_code)
        .ok_or(SiexError::Invalid("export_code_unmappable"))?;

    let (tipo_producto, materia_activa) = crate::blocks::authorisation_product_kind(
        conn,
        Some(&record.product_id),
        &record.country_code,
        record.authorisation_number_snapshot.as_deref(),
    )?;

    Ok(vec![ProductoFitoCantidad {
        tipo_producto,
        num_registro: record.authorisation_number_snapshot.clone(),
        materia_activa,
        cantidad: quantity,
        unidad,
    }])
}

/// Operator and equipment from the record's frozen snapshots.
///
/// These blocks have no `AplicacionManual` — that member is `TratamFito`'s —
/// but they carry the SAME `oneOf` over the three equipment identifiers, so
/// exactly one must be named even when the treatment was applied by hand. Hence
/// the fixed sentinel, as in `TratamFito`: an empty `EquipoAplicador` is
/// schema-invalid, which the vendored schema itself established here.
fn applicator(record: &NonFieldTreatment) -> IdentificadorAplicadorNoField {
    let licence_number = match &record.operator_licence_snapshot {
        Some(licence) if !licence.trim().is_empty() => licence.clone(),
        // Deletion entries only: the precheck demands the licence on active
        // records.
        _ => String::new(),
    };
    let (num_roma, num_reganip, id_equipo_aplicador) = if record.machinery_id.is_none() {
        (None, None, Some("manual".to_string()))
    } else if record.machinery_roma_snapshot.is_some() {
        // ROMA preferred when a machine carries both ("nunca ambos").
        (record.machinery_roma_snapshot.clone(), None, None)
    } else if record.machinery_reganip_snapshot.is_some() {
        (None, record.machinery_reganip_snapshot.clone(), None)
    } else {
        (None, None, record.machinery_id.clone())
    };
    IdentificadorAplicadorNoField {
        aplicador_empresa: AplicadorEmpresa {
            num_ropo: licence_number,
        },
        equipo_aplicador: EquipoAplicadorNoField {
            num_roma,
            num_reganip,
            id_equipo_aplicador,
        },
    }
}

/// The advisor, when the record names one AND the frozen snapshot carries a
/// ROPO number — the block's one required member. An advisor recorded without
/// one is an identification the format cannot express, and it is left out
/// rather than sent empty: the printed book still shows it.
fn advisor(record: &NonFieldTreatment) -> Option<AsesorValidacion> {
    record
        .advisor_registration_snapshot
        .as_deref()
        .map(str::trim)
        .filter(|ropo| !ropo.is_empty())
        .map(|ropo| AsesorValidacion {
            num_ropo: ropo.to_string(),
        })
}

/// The coded problems, sorted into buckets. Weeds and regulators are kept apart
/// from the rest because the two blocks differ in which of them exist — the
/// caller takes only the buckets its own block declares, and the precheck has
/// already refused any record whose problems would be lost that way.
struct Buckets {
    diseases: Vec<i64>,
    pests: Vec<i64>,
    regulators: Vec<i64>,
}

impl Buckets {
    fn diseases(&self) -> Option<Enfermedades> {
        (!self.diseases.is_empty()).then(|| Enfermedades {
            tipo_enfermedad: self.diseases.clone(),
        })
    }
    fn pests(&self) -> Option<ArtropodosGasteropodos> {
        (!self.pests.is_empty()).then(|| ArtropodosGasteropodos {
            tipo_plaga: self.pests.clone(),
        })
    }
    fn regulators(&self) -> Option<ReguladoresOtros> {
        (!self.regulators.is_empty()).then(|| ReguladoresOtros {
            tipo_regulador: self.regulators.clone(),
        })
    }
}

fn problem_buckets(problems: &[NonFieldTreatmentProblem]) -> Result<Buckets> {
    let mut buckets = Buckets {
        diseases: Vec::new(),
        pests: Vec::new(),
        regulators: Vec::new(),
    };
    for problem in problems {
        let code: i64 = problem
            .problem_code
            .trim()
            .parse()
            .map_err(|_| SiexError::Invalid("export_code_unmappable"))?;
        let bucket = match problem.reason_category_code.as_str() {
            "disease" => &mut buckets.diseases,
            "pest" => &mut buckets.pests,
            "growth_regulator" | "other" => &mut buckets.regulators,
            // Weeds reach neither block, and the precheck refuses such a record
            // before a build starts. Reaching here means the precheck was
            // bypassed, so it is an error rather than a silent drop.
            "weed" => return Err(SiexError::Invalid("export_code_unmappable")),
            _ => return Err(SiexError::Invalid("export_code_unmappable")),
        };
        if !bucket.contains(&code) {
            bucket.push(code);
        }
    }
    Ok(buckets)
}
