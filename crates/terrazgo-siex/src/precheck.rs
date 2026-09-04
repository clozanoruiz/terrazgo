// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What blocks a schema-valid export, listed rather than errored one field at
//! a time.
//!
//! The precheck exists because the two documents this app produces answer to
//! opposite rules. The printed record book has **no gate**: it shows what
//! exists and prints missing fields blank, because a farmer must be able to
//! print for an inspection while registry data is incomplete. The descriptor is
//! the reverse — it is *validated* by the authority, so a field the schema
//! demands cannot be blank, invented or quietly dropped.
//!
//! Hence the standing rule: `build_cuaderno` refuses on a dirty precheck rather
//! than skipping the offending records. **An export that silently drops data is
//! worse than one that refuses with a fixable list** — the farmer can act on a
//! list, and cannot act on a file that looks complete and is not.

use crate::error::Result;
use module_cue::repository::{
    list_non_field_treatments, list_seed_treatments, list_treatment_records,
};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;

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

/// A record of one of the three non-field registers, with enough context for a
/// list row ("21/08/2026 — Almacén de grano").
#[derive(Debug, Clone, Serialize)]
pub struct NonFieldRef {
    pub non_field_treatment_id: String,
    /// `postharvest` | `storage_premises` | `transport` — which register, so a
    /// caller can name the model section the farmer has to open.
    pub subject_kind_code: String,
    pub treated_on: String,
    /// What the register states was treated: the composed premises identity or
    /// the produce description.
    pub subject_description: String,
}

/// A sowing of treated seed (model 3.2).
#[derive(Debug, Clone, Serialize)]
pub struct SeedRef {
    pub seed_treatment_id: String,
    pub sown_on: String,
    pub species_name: String,
}

/// A sale of harvested produce (model 5).
#[derive(Debug, Clone, Serialize)]
pub struct HarvestRef {
    pub harvest_record_id: String,
    pub harvested_on: String,
    pub product_name: String,
}

/// A sowing (core's own register), with enough context for a list row.
#[derive(Debug, Clone, Serialize)]
pub struct SowingRef {
    pub sowing_record_id: String,
    pub sown_on: String,
    /// `sowing` | `planting` — which the farmer said it was.
    pub kind_code: String,
}

/// A sown plot without a crop — the export cannot name the DGC unit.
#[derive(Debug, Clone, Serialize)]
pub struct SowingPlotRef {
    pub sowing_record_id: String,
    pub sown_on: String,
    pub plot_id: String,
    pub plot_name: String,
}

/// A fertilisation record (model section 6).
#[derive(Debug, Clone, Serialize)]
pub struct FertilisationRef {
    pub fertilisation_record_id: String,
    pub applied_on: String,
    /// The material as the record froze it, so the row names itself.
    pub material_name: String,
}

/// An irrigation record (model section 8).
#[derive(Debug, Clone, Serialize)]
pub struct IrrigationRef {
    pub irrigation_record_id: String,
    pub irrigated_on: String,
}

/// A plan de abonado (model section 7.1).
#[derive(Debug, Clone, Serialize)]
pub struct PlanRef {
    pub fertilisation_plan_id: String,
    pub drawn_up_on: String,
}

/// A plot on one of the two application registers that carries no crop.
#[derive(Debug, Clone, Serialize)]
pub struct ApplicationPlotRef {
    /// `fertilisation` | `irrigation` — which register, so a caller can name
    /// the model section the farmer has to open.
    pub register_code: String,
    pub record_id: String,
    pub recorded_on: String,
    pub plot_id: String,
    pub plot_name: String,
}

/// A grazing (model 9.1).
#[derive(Debug, Clone, Serialize)]
pub struct GrazingRef {
    pub grazing_record_id: String,
    pub started_on: String,
    /// The farmer's own label for the parcels, when they gave one — model 9.1's
    /// "Id. del grupo de parcelas".
    pub plot_group_ref: Option<String>,
}

/// A cover (models 9.4 and 9.5).
#[derive(Debug, Clone, Serialize)]
pub struct CoverRef {
    pub soil_cover_id: String,
    /// `plant_cover` | `inert_cover` — which of the two pages the farmer has to
    /// open.
    pub practice_code: String,
    pub established_on: String,
}

/// A plot on one of the three eco-scheme registers whose crop the DGC rule could
/// not resolve — either because the plot carries none this season, or because it
/// carries several and the record names no one of them.
#[derive(Debug, Clone, Serialize)]
pub struct EcoschemePlotRef {
    /// `grazing` | `cultural_operation` | `soil_cover` — which register, so a
    /// caller can name the model section the farmer has to open.
    pub register_code: String,
    pub record_id: String,
    pub recorded_on: String,
    pub plot_id: String,
    pub plot_name: String,
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
    /// An actuation carrying a product AND a non-chemical measure — which the
    /// register allows, because model 3.1 bis prints "Alternativas no químicas"
    /// and "Alternativas químicas" as two column groups of one row.
    ///
    /// The format has no shape for it: Anexo V grades all five members of
    /// `OtrasActuacionesFito` *"excluyente con el subbloque siguiente de
    /// «Productos fitosanitarios»"*. Sending both would contradict that in five
    /// fields; sending either alone would lose the other half in silence, which
    /// is the failure this whole struct exists to prevent.
    ///
    /// The decree agrees with the exclusivity from the other side. RD 1311/2012
    /// Anexo III Parte I B — the list art. 16.1 binds the record to — opens
    /// *"Para cada tratamiento… especificar la información siguiente"* and has
    /// no non-chemical member at all, so a row carrying both is a row carrying
    /// two treatments. Splitting it here was rejected: `export_alias` is minted
    /// once and never mutated because SIEX keys edits and deletes on it, so one
    /// row would mint two aliases and a later correction dropping the measure
    /// would strand one asserting an activity that no longer exists.
    ///
    /// **Until 2026-08-22 this rule also refused the purely non-chemical
    /// record**, for want of an `OtrasActuacionesFito` writer. That record now
    /// exports (docs/siex-export.md → "Seam 5").
    pub records_mixing_product_and_measure: Vec<RecordRef>,
    /// An actuation whose measure block cannot be built: the
    /// `TIPO_MEDIDA_FITOSANITARIA` code is not an integer, the intensity was
    /// never stated, or its unit is one SIEX cannot express.
    ///
    /// The intensity is the deliberate part. The register keeps the value+unit
    /// pair nullable — a farmer may record that traps were hung before counting
    /// them — and the record book prints such a measure without complaint,
    /// while **Anexo V grades fields 17 and 18 Obligatorio**. That grading, not
    /// the JSON Schema's `required` (which lists `TipoMedida` alone), is what
    /// decides here: the seam-4 cover-widths case again, and the same reason
    /// the two documents live in separate crates.
    pub records_with_unsendable_measure: Vec<RecordRef>,
    /// A measure whose kind demands an MDF registration number and carries
    /// none. Anexo V field 19 grades `Registro MDF` Obligatorio for *"suelta de
    /// OCB, trampas y otros y feromonas y atrayentes para monitoreo"* — the
    /// three kinds `module_cue::siex::measure_requires_mdf_number` names.
    pub records_missing_measure_registration: Vec<RecordRef>,
    /// A record that names an advisor whose ROPO number is absent.
    ///
    /// `AsesorValidacion`'s only carriable member is `NumROPO` — the block has
    /// no name, surname or NIF member, though Anexo V grades those Obligatorio
    /// too — so a record naming an advisor without one would have to go out
    /// with the block omitted, dropping the identification Anexo III Parte I
    /// B.d asks for in the same sentence as the applicator's. Anexo V grades
    /// field 50 Obligatorio here, where blocks 1.2 and 1.3 grade the same field
    /// Voluntario; that is why the three non-field registers omit the block
    /// instead and this one refuses.
    pub records_missing_advisor_ropo: Vec<RecordRef>,

    // --- the three non-field registers (models 3.3, 3.4, 3.5) --------------
    /// Both blocks require an observed efficacy, like `TratamFito`.
    pub non_field_missing_efficacy: Vec<NonFieldRef>,
    /// `AplicadorEmpresa.NumROPO` is required in both.
    pub non_field_missing_operator_licence: Vec<NonFieldRef>,
    /// `ProductosFito` requires the amount used and its unit — post-harvest
    /// requires `Cantidad` outright, and a unit with no amount states nothing.
    pub non_field_missing_product_quantity: Vec<NonFieldRef>,
    /// 3.3 only: `ProductoVegetal` (the PROD_VEGETAL code) and `Cantidad` (the
    /// weight treated) are both required, and both are nullable in the register.
    pub post_harvest_missing_produce: Vec<NonFieldRef>,
    /// 3.4 / 3.5 only: `Edificaciones[].IdEdificacion` is REA's own code for
    /// the building, which lives on the premises' Spanish extension. A record
    /// naming no premises, or one whose extension carries no code, cannot be
    /// sent — and the code has to be an integer, because the schema types it as
    /// one.
    pub premises_missing_rea_code: Vec<NonFieldRef>,
    /// A problem category neither block can express. Neither carries
    /// `MalasHierbas` and the buildings block has no `ReguladoresOtros` either,
    /// so such a record is refused rather than exported with its reason
    /// silently missing — the `records_mixing_product_and_measure` rule again.
    pub non_field_unexpressible_problem: Vec<NonFieldRef>,

    // --- treated seed (model 3.2) ------------------------------------------
    /// `Tratamiento`, `Producto` (the crop code), `Cantidad` (kg of seed) and
    /// `Eficacia` are all required by the schema and all nullable here.
    pub seed_missing_fields: Vec<SeedRef>,
    /// The descriptor's own cross-field rule for `UsoSemillaTratada`: the lot
    /// number is required when the treatment kind is an acquisition (4 or 5),
    /// because a bought sack is traceable by its lot and nothing else.
    pub seed_acquired_missing_lot: Vec<SeedRef>,
    /// `SiembraPlantacion.FechaAdquisicion` — demanded only of the records that
    /// state an acquisition, which is what makes "the earliest purchase" a
    /// well-defined value when several lots feed one sowing.
    pub seed_acquired_missing_date: Vec<SeedRef>,

    // --- what left the holding (model 5) -----------------------------------
    /// `ProductoVegetal`, `Cantidad` and `Unidad` are all required by the schema
    /// and all nullable in the register, which prints them as blanks the farmer
    /// fills by hand. The produce code must also be an integer and the unit one
    /// SIEX knows.
    pub harvest_missing_fields: Vec<HarvestRef>,

    // --- how a crop began (the sowing register) ----------------------------
    /// `Cantidad` (kg of seed) is required by the schema and nullable here — no
    /// printed page shows it, so it is the one field of this register a farmer
    /// can leave blank without noticing.
    pub sowing_missing_seed_quantity: Vec<SowingRef>,
    /// A sown plot with no crop: both members of its DGC are optional, so such
    /// an entry would serialize as an empty object stating nothing at all.
    pub sowing_plots_missing_crop: Vec<SowingPlotRef>,

    // --- fertilisation, irrigation and the plan (sections 6, 7.1, 8) -------
    /// A fertigation with no irrigation record named. `Fertirrigacion` is the
    /// water side of one act the decree records twice (arts. 5.d and 5.e), and
    /// the only reader anywhere of Anexo III C.l's two water figures — so a
    /// fertigation that names no watering would export with its water silently
    /// missing. Asks for nothing new: art. 5.e already obliges that record.
    pub fertigations_missing_irrigation: Vec<FertilisationRef>,
    /// A named watering that no longer carries the two C.l figures — either it
    /// was withdrawn after the link was made, or the figures were never stated.
    /// Both are required inside `Fertirrigacion`.
    pub fertigations_missing_water_figures: Vec<FertilisationRef>,
    /// `PlanAbonado.CultivoPrecedente` is required and nullable here, because a
    /// unit coming out of fallow has no preceding crop to name.
    pub plans_missing_preceding_crop: Vec<PlanRef>,
    /// A fertilised or irrigated plot with no crop, which would serialize as a
    /// DGC stating nothing — the `sowing_plots_missing_crop` rule, on the two
    /// registers that share a DGC shape.
    pub application_plots_missing_crop: Vec<ApplicationPlotRef>,

    // --- the eco-scheme registers (model section 9) ------------------------
    /// A grazing with no end date. `Pastoreo.FechaFin` is required and
    /// `ended_on` is nullable, because RD 1048/2022 art. 30.2 ter gives the
    /// farmer a month from *"la nueva fecha de inicio o fin"* — so a grazing
    /// still under way is not late, it is unfinished, and the format has no
    /// shape for that. Refused by name rather than skipped: a record that
    /// vanishes from the file with nothing on screen saying so is the one
    /// failure this whole struct exists to prevent.
    pub grazings_without_end: Vec<GrazingRef>,
    /// A grazing whose animal line carries a species code that is not an
    /// integer. `Especie` is a required integer here, while `species_code` is a
    /// provider catalogue stored verbatim and deliberately unvalidated at insert
    /// (the two-tier rule), so this is where such a value is caught.
    pub grazings_with_unsendable_species: Vec<GrazingRef>,
    /// A cover the block cannot carry: its two widths were never stated, or its
    /// `TIPO_COBERTURA_SUELO` code is not an integer.
    ///
    /// The widths are the deliberate part. Art. 42.1.e falls due *"en el mes
    /// anterior al final del periodo mínimo de cuatro meses"* while 42.1.a is
    /// due within a month of establishment, so the register keeps them nullable
    /// and the record book prints such a cover without complaint. **Anexo V
    /// grades both Obligatorio** for exactly the three cover types this register
    /// can hold, and that grading — not the JSON Schema's `required` — is what
    /// decides here (docs/siex-export.md → "The law outranks the
    /// format"). The two documents answer to different readers under different
    /// rules, which is why they are separate crates.
    pub covers_missing_fields: Vec<CoverRef>,
    /// An eco-scheme plot carrying no live crop this season: its DGC would name
    /// neither a unit nor a species, which is an entry stating nothing.
    pub ecoscheme_plots_missing_crop: Vec<EcoschemePlotRef>,
    /// An eco-scheme plot carrying SEVERAL live crops. The plot is then several
    /// DGCs and the record names no one of them, so the rule refuses rather than
    /// choosing — picking one would assert the activity happened on a crop the
    /// farmer never stated.
    pub ecoscheme_plots_with_ambiguous_crop: Vec<EcoschemePlotRef>,
}

impl ExportPrecheck {
    pub fn is_clean(&self) -> bool {
        self.farm_missing_fields.is_empty()
            && self.records_missing_efficacy.is_empty()
            && self.records_missing_operator_licence.is_empty()
            && self.plots_missing_crop.is_empty()
            && self.records_mixing_product_and_measure.is_empty()
            && self.records_with_unsendable_measure.is_empty()
            && self.records_missing_measure_registration.is_empty()
            && self.records_missing_advisor_ropo.is_empty()
            && self.non_field_missing_efficacy.is_empty()
            && self.non_field_missing_operator_licence.is_empty()
            && self.non_field_missing_product_quantity.is_empty()
            && self.post_harvest_missing_produce.is_empty()
            && self.premises_missing_rea_code.is_empty()
            && self.non_field_unexpressible_problem.is_empty()
            && self.seed_missing_fields.is_empty()
            && self.seed_acquired_missing_lot.is_empty()
            && self.seed_acquired_missing_date.is_empty()
            && self.harvest_missing_fields.is_empty()
            && self.sowing_missing_seed_quantity.is_empty()
            && self.sowing_plots_missing_crop.is_empty()
            && self.fertigations_missing_irrigation.is_empty()
            && self.fertigations_missing_water_figures.is_empty()
            && self.plans_missing_preceding_crop.is_empty()
            && self.application_plots_missing_crop.is_empty()
            && self.grazings_without_end.is_empty()
            && self.grazings_with_unsendable_species.is_empty()
            && self.covers_missing_fields.is_empty()
            && self.ecoscheme_plots_missing_crop.is_empty()
            && self.ecoscheme_plots_with_ambiguous_crop.is_empty()
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
    if module_cue::siex::province_to_ccaa(province).is_none() {
        farm_missing_fields.push("province_code");
    }

    let mut records_missing_efficacy = Vec::new();
    let mut records_missing_operator_licence = Vec::new();
    let mut plots_missing_crop = Vec::new();
    let mut records_mixing_product_and_measure = Vec::new();
    let mut records_with_unsendable_measure = Vec::new();
    let mut records_missing_measure_registration = Vec::new();
    let mut records_missing_advisor_ropo = Vec::new();
    for rec in list_treatment_records(conn, season_id, farm_id)? {
        let record_ref = || RecordRef {
            treatment_record_id: rec.record.id.clone(),
            application_date: rec.record.application_date.clone(),
            product_name: rec.record.product_name_snapshot.clone(),
        };
        if let Some(measure) = rec.record.measure_code.as_deref() {
            if rec.record.product_id.is_some() {
                records_mixing_product_and_measure.push(record_ref());
            }
            // A measure code is validated against the closed catalogue at
            // insert, but only when a catalogue has been imported — so this is
            // where a non-integer one is caught rather than serialized.
            let kind = measure.trim().parse::<i64>().ok();
            let intensity_sendable = rec.record.measure_intensity_value.is_some()
                && rec
                    .record
                    .measure_intensity_unit_code
                    .as_deref()
                    .is_some_and(|unit| module_cue::siex::intensity_unit_to_siex(unit).is_some());
            if kind.is_none() || !intensity_sendable {
                records_with_unsendable_measure.push(record_ref());
            }
            if kind.is_some_and(module_cue::siex::measure_requires_mdf_number)
                && is_blank(rec.record.measure_registration_number.as_deref())
            {
                records_missing_measure_registration.push(record_ref());
            }
        }
        if rec.record.advisor_id.is_some()
            && is_blank(rec.record.advisor_registration_snapshot.as_deref())
        {
            records_missing_advisor_ropo.push(record_ref());
        }
        if rec.record.efficacy_code.is_none() {
            records_missing_efficacy.push(record_ref());
        }
        if is_blank(rec.record.operator_licence_snapshot.as_deref()) {
            records_missing_operator_licence.push(record_ref());
        }
        for plot in &rec.plots {
            if plot.crop_id.is_none() {
                let plot_name = plot_name(conn, &plot.plot_id)?;
                plots_missing_crop.push(PlotRef {
                    treatment_record_id: rec.record.id.clone(),
                    application_date: rec.record.application_date.clone(),
                    plot_id: plot.plot_id.clone(),
                    plot_name,
                });
            }
        }
    }

    // --- the three non-field registers -------------------------------------
    let mut non_field_missing_efficacy = Vec::new();
    let mut non_field_missing_operator_licence = Vec::new();
    let mut non_field_missing_product_quantity = Vec::new();
    let mut post_harvest_missing_produce = Vec::new();
    let mut premises_missing_rea_code = Vec::new();
    let mut non_field_unexpressible_problem = Vec::new();
    for detail in list_non_field_treatments(conn, season_id, farm_id)? {
        let record = &detail.record;
        let entry = || NonFieldRef {
            non_field_treatment_id: record.id.clone(),
            subject_kind_code: record.subject_kind_code.clone(),
            treated_on: record.treated_on.clone(),
            subject_description: record.subject_description.clone(),
        };
        if record.efficacy_code.is_none() {
            non_field_missing_efficacy.push(entry());
        }
        if is_blank(record.operator_licence_snapshot.as_deref()) {
            non_field_missing_operator_licence.push(entry());
        }
        if record.product_quantity_value.is_none() || record.product_quantity_unit_code.is_none() {
            non_field_missing_product_quantity.push(entry());
        }
        let is_post_harvest = record.subject_kind_code == "postharvest";
        if is_post_harvest
            && (is_blank(record.subject_product_code.as_deref())
                || record.treated_quantity_value.is_none()
                || record.treated_quantity_unit_code.is_none())
        {
            post_harvest_missing_produce.push(entry());
        }
        if !is_post_harvest && !premises_has_rea_code(conn, record.premises_id.as_deref())? {
            premises_missing_rea_code.push(entry());
        }
        // Weeds fit neither block; regulators fit post-harvest only.
        let unexpressible = detail.problems.iter().any(|problem| {
            matches!(problem.reason_category_code.as_str(), "weed")
                || (!is_post_harvest
                    && matches!(
                        problem.reason_category_code.as_str(),
                        "growth_regulator" | "other"
                    ))
        });
        if unexpressible {
            non_field_unexpressible_problem.push(entry());
        }
    }

    // --- treated seed -------------------------------------------------------
    let mut seed_missing_fields = Vec::new();
    let mut seed_acquired_missing_lot = Vec::new();
    let mut seed_acquired_missing_date = Vec::new();
    for detail in list_seed_treatments(conn, season_id, farm_id)? {
        let record = &detail.record;
        let seed_ref = || SeedRef {
            seed_treatment_id: record.id.clone(),
            sown_on: record.sown_on.clone(),
            species_name: record.species_name.clone(),
        };
        if record.treatment_kind_code.is_none()
            || is_blank(record.crop_code.as_deref())
            || record.seed_quantity_kg.is_none()
            || record.efficacy_code.is_none()
        {
            seed_missing_fields.push(seed_ref());
        }
        // Both rules apply only to the two acquisition kinds, which is why
        // neither can block a farmer who treated their own seed.
        if crate::blocks::siembra_plantacion::is_acquisition(record.treatment_kind_code.as_deref())
        {
            if is_blank(record.seed_lot.as_deref()) {
                seed_acquired_missing_lot.push(seed_ref());
            }
            if is_blank(record.acquired_on.as_deref()) {
                seed_acquired_missing_date.push(seed_ref());
            }
        }
    }

    // --- what left the holding, and how the crop began ----------------------
    let mut harvest_missing_fields = Vec::new();
    for detail in terrazgo_core::repository::list_harvest_records(conn, season_id, farm_id)? {
        let record = &detail.record;
        let produce_sendable = record
            .plant_product_code
            .as_deref()
            .is_some_and(|code| code.trim().parse::<i64>().is_ok());
        let unit_sendable = record
            .quantity_unit_code
            .as_deref()
            .is_some_and(|code| module_cue::siex::quantity_unit_to_siex(code).is_some());
        if !produce_sendable || record.quantity_value.is_none() || !unit_sendable {
            harvest_missing_fields.push(HarvestRef {
                harvest_record_id: record.id.clone(),
                harvested_on: record.harvested_on.clone(),
                product_name: record.product_name.clone(),
            });
        }
    }

    let mut sowing_missing_seed_quantity = Vec::new();
    let mut sowing_plots_missing_crop = Vec::new();
    for detail in terrazgo_core::repository::list_sowing_records(conn, season_id, farm_id)? {
        let record = &detail.record;
        if record.seed_quantity_kg.is_none() {
            sowing_missing_seed_quantity.push(SowingRef {
                sowing_record_id: record.id.clone(),
                sown_on: record.sown_on.clone(),
                kind_code: record.kind_code.clone(),
            });
        }
        for plot in &detail.plots {
            if plot.crop_id.is_none() {
                let plot_name = plot_name(conn, &plot.plot_id)?;
                sowing_plots_missing_crop.push(SowingPlotRef {
                    sowing_record_id: record.id.clone(),
                    sown_on: record.sown_on.clone(),
                    plot_id: plot.plot_id.clone(),
                    plot_name,
                });
            }
        }
    }

    // --- fertilisation, irrigation and the plan ----------------------------
    let mut fertigations_missing_irrigation = Vec::new();
    let mut fertigations_missing_water_figures = Vec::new();
    let mut application_plots_missing_crop = Vec::new();

    // Read once for the whole pass, both of them. Which methods ARE fertigation
    // comes from the lookup rather than from matching the code, so the rule
    // follows the same data the form reads.
    let fertigation_methods: HashSet<String> =
        module_fertilisation::repository::list_application_methods(conn)?
            .into_iter()
            .filter(|method| method.is_fertigation)
            .map(|method| method.code)
            .collect();
    // The season's LIVE waterings, which is the whole population a fertigation
    // can name: `validate_fertigation_link` refuses a link to another farm or
    // another campaign, so a linked record absent from this list is a withdrawn
    // one — exactly the case the rule below has to catch. (The cover link is not
    // season-validated, which is why `cover_type_of` cannot work this way.)
    let irrigations =
        module_fertilisation::repository::list_irrigation_records(conn, season_id, farm_id)?;

    for detail in
        module_fertilisation::repository::list_fertilisation_records(conn, season_id, farm_id)?
    {
        let record = &detail.record;
        let entry = || FertilisationRef {
            fertilisation_record_id: record.id.clone(),
            applied_on: record.applied_on.clone(),
            material_name: record.material_name_snapshot.clone(),
        };
        if fertigation_methods.contains(&record.application_method_code) {
            let watering = record.irrigation_record_id.as_deref().and_then(|id| {
                irrigations
                    .iter()
                    .find(|candidate| candidate.record.id == id)
            });
            match watering {
                // Named nothing, or named a watering since withdrawn: the
                // statement it carried is retracted, so the block has nothing to
                // say and the two cases are the same fixable one.
                None => fertigations_missing_irrigation.push(entry()),
                Some(found)
                    if found.record.water_nitric_n_mg_l.is_none()
                        || found.record.water_soluble_p2o5_mg_l.is_none() =>
                {
                    fertigations_missing_water_figures.push(entry())
                }
                Some(_) => {}
            }
        }
        for plot in &detail.plots {
            if plot.crop_id.is_none() {
                application_plots_missing_crop.push(application_plot_ref(
                    conn,
                    "fertilisation",
                    &record.id,
                    &record.applied_on,
                    &plot.plot_id,
                )?);
            }
        }
    }

    for detail in &irrigations {
        for plot in &detail.plots {
            if plot.crop_id.is_none() {
                application_plots_missing_crop.push(application_plot_ref(
                    conn,
                    "irrigation",
                    &detail.record.id,
                    &detail.record.irrigated_on,
                    &plot.plot_id,
                )?);
            }
        }
    }

    let mut plans_missing_preceding_crop = Vec::new();
    for detail in
        module_fertilisation::repository::list_fertilisation_plans(conn, season_id, farm_id)?
    {
        if is_blank(detail.plan.preceding_crop_code.as_deref()) {
            plans_missing_preceding_crop.push(PlanRef {
                fertilisation_plan_id: detail.plan.id.clone(),
                drawn_up_on: detail.plan.drawn_up_on.clone(),
            });
        }
    }

    // --- the eco-scheme registers (model section 9) ------------------------
    let mut grazings_without_end = Vec::new();
    let mut grazings_with_unsendable_species = Vec::new();
    let mut unresolved = UnresolvedPlots::default();
    let mut season_holds_a_grazing = false;
    for detail in module_ecoscheme::repository::list_grazing_records(conn, season_id, farm_id)? {
        let record = &detail.record;
        season_holds_a_grazing = true;
        let entry = || GrazingRef {
            grazing_record_id: record.id.clone(),
            started_on: record.started_on.clone(),
            plot_group_ref: record.plot_group_ref.clone(),
        };
        if record.ended_on.is_none() {
            grazings_without_end.push(entry());
        }
        if detail
            .animals
            .iter()
            .any(|line| line.species_code.trim().parse::<i64>().is_err())
        {
            grazings_with_unsendable_species.push(entry());
        }
        unresolved.collect(
            conn,
            "grazing",
            &record.id,
            &record.started_on,
            season_id,
            detail.plots.iter().map(|plot| plot.plot_id.clone()),
        )?;
    }

    // `AnimalesPropios`/`AnimalesTerceros` are derived by comparing each animal
    // line's REGA with the holding's own, so without it every animal would be
    // reported as a third party's — a claim rather than an absence, and the
    // descriptor forbids both booleans being false. Demanded only once the
    // season holds a grazing, because it is the register that needs it and not
    // the farm: a holding with no animals owes no REGA to anyone.
    if season_holds_a_grazing && is_blank(es.and_then(|e| e.rega_code.as_deref())) {
        farm_missing_fields.push("rega_code");
    }

    for detail in module_ecoscheme::repository::list_cultural_operations(conn, season_id, farm_id)?
    {
        let record = &detail.record;
        unresolved.collect(
            conn,
            "cultural_operation",
            &record.id,
            &record.performed_on,
            season_id,
            detail.plots.iter().map(|plot| plot.plot_id.clone()),
        )?;
    }

    let mut covers_missing_fields = Vec::new();
    for detail in module_ecoscheme::repository::list_soil_covers(conn, season_id, farm_id)? {
        let record = &detail.record;
        // The widths are an all-or-none triple, so the stated-on date answers
        // for all three — which is exactly what separates a cover measured in
        // June from one never measured.
        let widths_stated = record.width_m.is_some()
            && record.free_canopy_width_m.is_some()
            && !is_blank(record.widths_stated_on.as_deref());
        let cover_type_sendable = record.cover_type_code.trim().parse::<i64>().is_ok();
        if !widths_stated || !cover_type_sendable {
            covers_missing_fields.push(CoverRef {
                soil_cover_id: record.id.clone(),
                practice_code: record.practice_code.clone(),
                established_on: record.established_on.clone(),
            });
        }
        unresolved.collect(
            conn,
            "soil_cover",
            &record.id,
            &record.established_on,
            season_id,
            detail.plots.iter().map(|plot| plot.plot_id.clone()),
        )?;
    }

    // Analytics (model section 4) contribute no rule, and the register is
    // deliberately not read here: the schema requires only the material and the
    // date, both NOT NULL, and Anexo V grades all eight fields Voluntario. Every
    // analysis a farmer can record is already exportable as it stands.

    Ok(ExportPrecheck {
        farm_missing_fields,
        records_missing_efficacy,
        records_missing_operator_licence,
        plots_missing_crop,
        records_mixing_product_and_measure,
        records_with_unsendable_measure,
        records_missing_measure_registration,
        records_missing_advisor_ropo,
        non_field_missing_efficacy,
        non_field_missing_operator_licence,
        non_field_missing_product_quantity,
        post_harvest_missing_produce,
        premises_missing_rea_code,
        non_field_unexpressible_problem,
        seed_missing_fields,
        seed_acquired_missing_lot,
        seed_acquired_missing_date,
        harvest_missing_fields,
        sowing_missing_seed_quantity,
        sowing_plots_missing_crop,
        fertigations_missing_irrigation,
        fertigations_missing_water_figures,
        plans_missing_preceding_crop,
        application_plots_missing_crop,
        grazings_without_end,
        grazings_with_unsendable_species,
        covers_missing_fields,
        ecoscheme_plots_missing_crop: unresolved.missing,
        ecoscheme_plots_with_ambiguous_crop: unresolved.ambiguous,
    })
}

/// Which of the two eco-scheme plot lists a record's plots belong in, gathered
/// for one record at a time.
///
/// The rule is identical in all three registers — their junctions carry a plot
/// and no crop, so the crop is computed — and the two ways that computation can
/// fail are the two lists this fills.
#[derive(Default)]
struct UnresolvedPlots {
    missing: Vec<EcoschemePlotRef>,
    ambiguous: Vec<EcoschemePlotRef>,
}

impl UnresolvedPlots {
    fn collect(
        &mut self,
        conn: &Connection,
        register_code: &str,
        record_id: &str,
        recorded_on: &str,
        season_id: &str,
        plot_ids: impl IntoIterator<Item = String>,
    ) -> Result<()> {
        for plot_id in plot_ids {
            let resolved = crate::blocks::crop_on_plot(conn, &plot_id, season_id)?;
            let target = match resolved {
                crate::blocks::PlotCrop::One { .. } => continue,
                crate::blocks::PlotCrop::None => &mut self.missing,
                crate::blocks::PlotCrop::Ambiguous => &mut self.ambiguous,
            };
            let plot_name = plot_name(conn, &plot_id)?;
            target.push(EcoschemePlotRef {
                register_code: register_code.to_string(),
                record_id: record_id.to_string(),
                recorded_on: recorded_on.to_string(),
                plot_id,
                plot_name,
            });
        }
        Ok(())
    }
}

fn application_plot_ref(
    conn: &Connection,
    register_code: &str,
    record_id: &str,
    recorded_on: &str,
    plot_id: &str,
) -> Result<ApplicationPlotRef> {
    let plot_name = plot_name(conn, plot_id)?;
    Ok(ApplicationPlotRef {
        register_code: register_code.to_string(),
        record_id: record_id.to_string(),
        recorded_on: recorded_on.to_string(),
        plot_id: plot_id.to_string(),
        plot_name,
    })
}

/// Whether the premises this record names carries a REA installation code that
/// the format can send — i.e. an integer, since the schema types
/// `IdEdificacion` as one.
///
/// A record naming no premises at all fails too: `Edificaciones` is `1..n` and
/// its only member is that code, so there is nothing to send. That is the one
/// place the registry's optionality meets the format's requirement, and it
/// resolves the way the arc decided it would — the register stays recordable,
/// the export refuses with a fixable list.
fn premises_has_rea_code(conn: &Connection, premises_id: Option<&str>) -> Result<bool> {
    let Some(id) = premises_id else {
        return Ok(false);
    };
    let detail = terrazgo_core::repository::get_premises_detail(conn, id)?;
    Ok(detail
        .es
        .and_then(|es| es.rea_installation_code)
        .is_some_and(|code| code.trim().parse::<i64>().is_ok()))
}

pub(crate) fn is_blank(value: Option<&str>) -> bool {
    value.unwrap_or("").trim().is_empty()
}

/// A plot's name, for the list row that points the farmer at it.
///
/// **Deliberately not filtered on `deleted_at`, and deliberately not a core
/// accessor.** Plots are soft-deleted, so a record can name a withdrawn one and
/// the row still has to say which plot it means — but this resolves a *label*,
/// not a regulatory value, and core has no by-id plot getter today. Inventing
/// one to serve a display string would grow core's surface for a single
/// consumer, which is the thing the module seam's own rule says to resist. What
/// was worth fixing was the four copies of this query; that is what this is.
fn plot_name(conn: &Connection, plot_id: &str) -> Result<String> {
    let name = conn.query_row("SELECT name FROM plot WHERE id = ?1", [plot_id], |r| {
        r.get(0)
    })?;
    Ok(name)
}
