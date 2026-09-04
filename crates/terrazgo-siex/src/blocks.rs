// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One module per activity block of `ActividadesExplotacion`.
//!
//! The format declares fifteen blocks and none of them is required: the
//! container has no required properties and every block is `0..n`, so a block
//! is simply absent when the campaign holds none of its records. Each module
//! here exposes one `build` returning its block's entries, and
//! [`crate::build_cuaderno`] assembles the envelope around them.
//!
//! Two blocks have no module and never will, and that is a decision rather
//! than a gap (docs/siex-export.md): `Cosecha` describes a harvesting
//! *operation* with five mandatory booleans about residue and seed retention
//! that no decree asks a cuaderno to keep — our `harvest_record` is what left
//! the holding, which is `ComercializacionVD` — and `EnergiaUtilizada` is
//! graded **Voluntario** in every one of its six fields by Anexo V.

pub mod analitica;
pub mod comercializacion_vd;
pub mod datos_cubierta;
pub mod fertilizacion;
pub mod labores_culturales;
pub mod non_field;
pub mod pastoreo;
pub mod plan_abonado;
pub mod riego;
pub mod siembra_plantacion;
pub mod tratam_fito;
pub mod uso_semilla_tratada;

use crate::SIEX_TARGET;
use crate::descriptor::{Cubierta, DgcActividad, DgcCubierta, DgcSuperficie};
use crate::error::{Result, SiexError};
use module_cue::repository::find_product_authorisation;
use module_ecoscheme::repository::get_soil_cover_for_export;
use rusqlite::Connection;
use terrazgo_core::repository::{
    crops_on_plot, ensure_export_alias, find_crop_for_export, find_export_alias,
};

/// One plot+crop unit as `Fertilizacion` and `Riego` both spell it — the two
/// application registers whose DGC arrays are identical in the schema.
///
/// The crop's PRODUCTOS code is read LIVE rather than snapshotted (the
/// `Analitica` rule): it identifies the species for the authority, while the
/// junction's frozen crop name is what the BOOK prints. A plot with no crop
/// names neither member — the precheck refuses that on an active record, so it
/// happens only on a deletion entry.
pub(crate) fn dgc_superficie(
    conn: &mut Connection,
    crop_id: Option<&str>,
    area_ha: Option<f64>,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcSuperficie> {
    let Some(crop_id) = crop_id else {
        return Ok(DgcSuperficie {
            codigo_dgc_ajena: None,
            codigo_cultivo: None,
            superficie: area_ha,
        });
    };
    let codigo_dgc_ajena = if deleted {
        find_export_alias(conn, SIEX_TARGET, "crop", crop_id, "")?
    } else {
        Some(ensure_export_alias(
            conn,
            SIEX_TARGET,
            "crop",
            crop_id,
            "",
            actor,
        )?)
    };
    Ok(DgcSuperficie {
        codigo_dgc_ajena,
        codigo_cultivo: crop_code_of(conn, crop_id)?,
        superficie: area_ha,
    })
}

/// A crop's PRODUCTOS code as an integer, for the `CodigoCultivo` every DGC
/// shape carries.
///
/// Read LIVE rather than snapshotted (the `Analitica` rule): it identifies the
/// species for the authority, while the junction's frozen crop name is what the
/// BOOK prints. Withdrawn crops resolve too — see
/// [`find_crop_for_export`]. `None` when the crop carries no catalogue match, or
/// carries one this format cannot send as an integer.
pub(crate) fn crop_code_of(conn: &Connection, crop_id: &str) -> Result<Option<i64>> {
    Ok(find_crop_for_export(conn, crop_id)?
        .and_then(|crop| crop.crop_code)
        .and_then(|code| code.trim().parse::<i64>().ok()))
}

/// `ProductosFito.TipoProducto` and `MateriaActiva`, resolved together.
///
/// Shared by `TratamFito` and the two non-field blocks, which asked this
/// identical question with identical code in two places until 2026-08-22 — the
/// query, the fallback, the code mapping and the substance parse, all four.
///
/// **Resolved LIVE by the number the record froze**, because that number is what
/// the record legally cites; an authorisation row that no longer matches falls
/// back to `registered`, which is what every record predating `kind_code` was.
/// `MateriaActiva` is emitted only for kind `exceptional` — TipoProducto 4 —
/// where the 3.11.4 re-diff makes the AUTORIZACION_EXCP code mandatory.
pub(crate) fn authorisation_product_kind(
    conn: &Connection,
    product_id: Option<&str>,
    country_code: &str,
    authorisation_number: Option<&str>,
) -> Result<(i64, Option<i64>)> {
    let unmappable = || SiexError::Invalid("export_code_unmappable");
    // Either half absent matches no row, exactly as binding a NULL did before:
    // `treatment_record.product_id` is nullable for a purely non-chemical
    // actuation, and a record can have frozen no authorisation number.
    let found = match (product_id, authorisation_number) {
        (Some(product), Some(number)) => {
            find_product_authorisation(conn, product, country_code, number)?
        }
        _ => None,
    };
    let (kind_code, exceptional_substance) = match found {
        Some(auth) => (auth.kind_code, auth.exceptional_substance_code),
        None => (DEFAULT_AUTHORISATION_KIND.to_string(), None),
    };
    let tipo_producto =
        module_cue::siex::authorisation_kind_to_siex(&kind_code).ok_or_else(unmappable)?;
    let materia_activa = if kind_code == "exceptional" {
        let code = exceptional_substance.ok_or_else(unmappable)?;
        Some(code.trim().parse::<i64>().map_err(|_| unmappable())?)
    } else {
        None
    };
    Ok((tipo_producto, materia_activa))
}

/// What an unmatched authorisation falls back to — the schema default, and what
/// every row written before `kind_code` existed meant.
const DEFAULT_AUTHORISATION_KIND: &str = "registered";

/// What the eco-scheme registers' plots resolve to, since none of their
/// junctions carries a crop.
///
/// **The rule seam 4 owes, and Anexo V asks for it in as many words**: the crop
/// of `Pastoreo` and `LaboresCulturales` is *"un campo calculado"*. The three
/// junctions store a plot and nothing else, because no printed page of section 9
/// asks which crop was on it — model 9.1 wants the SIGPAC reference, 9.2 the
/// plot, 9.4 the cover — while a SIEX DGC is a plot+crop unit.
///
/// So the crop is computed from the plot and the record's own season. The one
/// case with no honest answer is a plot carrying two live crops: it IS two DGCs,
/// the record names neither, and choosing one would put the activity on a crop
/// the farmer never stated. That is refused rather than guessed, and the precheck
/// names the plot so the farmer can say which — the standing rule that an export
/// refuses with a fixable list instead of inventing.
pub(crate) enum PlotCrop {
    /// The plot carries exactly one live crop this season: the DGC.
    ///
    /// Carries the two fields a DGC is made of rather than the whole `Crop`,
    /// which is a ~384-byte row against two empty variants. Narrowing beats
    /// boxing here because it also says what a DGC actually needs from a crop —
    /// its identity, to alias, and its PRODUCTOS code, to name the species.
    One {
        crop_id: String,
        /// `crop.crop_code`, read LIVE rather than snapshotted (the `Analitica`
        /// rule) and `None` for a free-text species with no catalogue match.
        crop_code: Option<String>,
    },
    /// No live crop — nothing to name the DGC with.
    None,
    /// Two or more. Never resolved here; see the type's own docs.
    Ambiguous,
}

/// Apply the rule to one plot, over core's own [`crops_on_plot`].
///
/// The query lives in core because the DATA is core's; the refusal lives here
/// because the rule is this document's. A future reader of the same question
/// might reasonably split a two-crop plot instead of refusing it, which is
/// exactly why core returns every match and says nothing about what to do with
/// them.
pub(crate) fn crop_on_plot(conn: &Connection, plot_id: &str, season_id: &str) -> Result<PlotCrop> {
    Ok(match crops_on_plot(conn, plot_id, season_id)?.as_slice() {
        [] => PlotCrop::None,
        [only] => PlotCrop::One {
            crop_id: only.id.clone(),
            crop_code: only.crop_code.clone(),
        },
        _ => PlotCrop::Ambiguous,
    })
}

/// The alias and PRODUCTOS code of one resolved DGC.
///
/// The crop code is read LIVE rather than snapshotted (the `Analitica` rule):
/// it identifies the species for the authority, while a frozen name is what the
/// BOOK prints. A plot the rule could not resolve names neither member — the
/// precheck refuses that on an active record, so it happens only on a deletion
/// entry.
fn dgc_identity(
    conn: &mut Connection,
    plot_id: &str,
    season_id: &str,
    deleted: bool,
    actor: Option<&str>,
) -> Result<(Option<i64>, Option<i64>)> {
    let PlotCrop::One { crop_id, crop_code } = crop_on_plot(conn, plot_id, season_id)? else {
        return Ok((None, None));
    };
    let codigo_dgc_ajena = if deleted {
        find_export_alias(conn, SIEX_TARGET, "crop", &crop_id, "")?
    } else {
        Some(ensure_export_alias(
            conn,
            SIEX_TARGET,
            "crop",
            &crop_id,
            "",
            actor,
        )?)
    };
    Ok((
        codigo_dgc_ajena,
        crop_code
            .as_deref()
            .and_then(|code| code.trim().parse::<i64>().ok()),
    ))
}

/// The `TIPO_COBERTURA_SUELO` code of the cover a maintenance record names
/// (art. 42.1.c), resolved once per entry rather than once per plot.
///
/// Goes through module-ecoscheme's own unfiltered getter rather than reading the
/// table: withdrawing a cover withdraws its maintenance lines in the same
/// transaction, so a live record always points at a live cover — but a deletion
/// entry must still resolve the cover it named, which the ordinary getter
/// filters out.
pub(crate) fn cover_type_of(conn: &Connection, soil_cover_id: Option<&str>) -> Result<Option<i64>> {
    let Some(id) = soil_cover_id else {
        return Ok(None);
    };
    let cover = get_soil_cover_for_export(conn, id)?;
    Ok(cover.cover_type_code.trim().parse::<i64>().ok())
}

/// One plot of a `Pastoreo` or `LaboresCulturales` entry.
///
/// `cover_type` is the cover this record maintained (art. 42.1.c), already
/// resolved once for the whole entry — which is what makes the descriptor's rule
/// that an activity may not mix DGCs with and without a cover hold by
/// construction. An unparseable code carries nothing rather than failing the
/// export: `Cubiertas` is optional, unlike `DatosCubierta.TipoCobertura` which
/// the precheck demands.
pub(crate) fn dgc_actividad(
    conn: &mut Connection,
    plot_id: &str,
    season_id: &str,
    cover_type: Option<i64>,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcActividad> {
    let (codigo_dgc_ajena, codigo_cultivo) =
        dgc_identity(conn, plot_id, season_id, deleted, actor)?;
    Ok(DgcActividad {
        codigo_dgc_ajena,
        codigo_cultivo,
        cubiertas: cover_type
            .map(|tipo_cobertura| Cubierta { tipo_cobertura })
            .into_iter()
            .collect(),
    })
}

/// One plot of a `DatosCubierta` entry.
pub(crate) fn dgc_cubierta(
    conn: &mut Connection,
    plot_id: &str,
    season_id: &str,
    deleted: bool,
    actor: Option<&str>,
) -> Result<DgcCubierta> {
    let (codigo_dgc_ajena, codigo_cultivo) =
        dgc_identity(conn, plot_id, season_id, deleted, actor)?;
    Ok(DgcCubierta {
        codigo_dgc_ajena,
        codigo_cultivo,
    })
}
