// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rust structs mirroring the schema, plus the `New*` / `Update*` inputs the
//! Tauri commands deserialize into.
//!
//! `Lookup` is re-exported from core so a selector's shape is identical
//! whichever crate filled it.

pub use terrazgo_core::models::Lookup;

use serde::{Deserialize, Serialize};

/// A reusable fertiliser, manure or amendment: the registry a record points at.
/// SIEX twin: `Fertilizacion.MaterialFertilizante`.
#[derive(Debug, Clone, Serialize)]
pub struct FertiliserMaterial {
    pub id: String,
    pub name: String,
    /// FEGA `MAT_FERTI`, verbatim (Anexo III C.d, first level).
    pub material_code: String,
    /// FEGA `DETALLE_MATERIAL_FERT`, verbatim (C.d, second level).
    pub material_detail_code: Option<String>,
    pub supplier_name: Option<String>,
    /// C.e's three mutually exclusive supplier registries: a livestock
    /// holding's REGA, a manure management centre's NIF, a waste manager's
    /// NIMA. At most one is set.
    pub supplier_rega: Option<String>,
    pub supplier_tax_id: Option<String>,
    pub supplier_nima: Option<String>,
    pub manure_treatment_code: Option<String>,
    /// kg/L, the unit every fertiliser label states a density in.
    pub density_kg_l: Option<f64>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// One line of a material's composition: a percentage against one of the three
/// FEGA nutrient catalogues, chosen by `kind_code`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialNutrient {
    #[serde(default)]
    pub id: String,
    /// `macro` | `micro` | `heavy_metal` — which catalogue `nutrient_code`
    /// indexes.
    pub kind_code: String,
    pub nutrient_code: String,
    pub percentage: f64,
}

/// A material with its composition, which is how both the registry form and
/// the record book need it.
#[derive(Debug, Clone, Serialize)]
pub struct FertiliserMaterialDetail {
    pub material: FertiliserMaterial,
    pub nutrients: Vec<MaterialNutrient>,
}

#[derive(Debug, Deserialize)]
pub struct NewFertiliserMaterial {
    pub name: String,
    pub material_code: String,
    #[serde(default)]
    pub material_detail_code: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    #[serde(default)]
    pub supplier_rega: Option<String>,
    #[serde(default)]
    pub supplier_tax_id: Option<String>,
    #[serde(default)]
    pub supplier_nima: Option<String>,
    #[serde(default)]
    pub manure_treatment_code: Option<String>,
    #[serde(default)]
    pub density_kg_l: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Reconciled from the submitted state on update, like a product's
    /// substance links.
    #[serde(default)]
    pub nutrients: Vec<MaterialNutrient>,
}

/// Full-row correction. A material is a registry entry, not a record of an
/// event, so everything about it is correctable; past applications are immune
/// because they snapshot the name and the printed richness at write time.
#[derive(Debug, Deserialize)]
pub struct UpdateFertiliserMaterial {
    pub id: String,
    pub name: String,
    pub material_code: String,
    #[serde(default)]
    pub material_detail_code: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    #[serde(default)]
    pub supplier_rega: Option<String>,
    #[serde(default)]
    pub supplier_tax_id: Option<String>,
    #[serde(default)]
    pub supplier_nima: Option<String>,
    #[serde(default)]
    pub manure_treatment_code: Option<String>,
    #[serde(default)]
    pub density_kg_l: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub nutrients: Vec<MaterialNutrient>,
}

/// Model section 6 — one fertiliser application (or one accumulated period of
/// them) over a set of plots. SIEX twin: `Fertilizacion`.
#[derive(Debug, Clone, Serialize)]
pub struct FertilisationRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// Start of the interval, or the single day it happened (Anexo III C.a).
    pub applied_on: String,
    /// End of the interval; `None` for a single day.
    pub application_end_date: Option<String>,
    /// C.c — fondo / cobertera / enmienda.
    pub fertilisation_type_code: String,
    /// C.f — how it was applied, fertigation included. A separate legal field
    /// from the type, which the printed model's single letter merges.
    pub application_method_code: String,
    /// C.j, per hectare.
    pub dose_value: f64,
    pub dose_unit_code: String,
    pub fertiliser_material_id: String,
    /// Frozen at write time, so correcting the registry never rewrites a past
    /// legal record. Only what section 6 prints is snapshotted; the full C.h
    /// composition stays on the (soft-deleted, always resolvable) registry row.
    pub material_name_snapshot: String,
    /// Anexo III C.d's coded kind, frozen beside the name: the model's own
    /// "Tipo de abono/producto" column prints it.
    pub material_code_snapshot: String,
    pub richness_n_snapshot: Option<f64>,
    pub richness_p2o5_snapshot: Option<f64>,
    pub richness_k2o_snapshot: Option<f64>,
    /// C.i / art. 5.g — whether sewage sludge was applied.
    pub sludge_application: bool,
    /// C.g, explicitly optional.
    pub machinery_id: Option<String>,
    /// C.k — the service company and its REGFER number, when the applicator is
    /// not the holding's own.
    pub service_company: Option<String>,
    pub service_regfer_number: Option<String>,
    pub delivery_note_ref: Option<String>,
    pub yield_estimated_kg_ha: Option<f64>,
    pub yield_final_kg_ha: Option<f64>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// One fertilised plot, with the crop on it at the time and the surface.
#[derive(Debug, Clone, Serialize)]
pub struct FertilisationPlot {
    pub id: String,
    pub fertilisation_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub fertilised_area_ha: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FertilisationRecordDetail {
    pub record: FertilisationRecord,
    pub plots: Vec<FertilisationPlot>,
    /// `BUENAS_PRACTICAS_AMBITOS` codes in the "Fertilización" ámbito, stored
    /// verbatim. Required by the twin, absent from the printed model, so
    /// captured and never demanded.
    pub practices: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewFertilisationPlot {
    pub plot_id: String,
    #[serde(default)]
    pub crop_id: Option<String>,
    #[serde(default)]
    pub fertilised_area_ha: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct NewFertilisationRecord {
    pub season_id: String,
    pub farm_id: String,
    pub applied_on: String,
    #[serde(default)]
    pub application_end_date: Option<String>,
    pub fertilisation_type_code: String,
    pub application_method_code: String,
    pub dose_value: f64,
    pub dose_unit_code: String,
    pub fertiliser_material_id: String,
    #[serde(default)]
    pub sludge_application: bool,
    #[serde(default)]
    pub machinery_id: Option<String>,
    #[serde(default)]
    pub service_company: Option<String>,
    #[serde(default)]
    pub service_regfer_number: Option<String>,
    #[serde(default)]
    pub delivery_note_ref: Option<String>,
    #[serde(default)]
    pub yield_estimated_kg_ha: Option<f64>,
    #[serde(default)]
    pub yield_final_kg_ha: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewFertilisationPlot>,
    #[serde(default)]
    pub practices: Vec<String>,
}

/// Full-row update; `season_id` and `farm_id` are absent for the reason
/// `UpdateIrrigationRecord`'s are. The material CAN change, and changing it
/// re-takes the snapshot: a record that names one fertiliser while printing
/// another's richness would be worse than either.
#[derive(Debug, Deserialize)]
pub struct UpdateFertilisationRecord {
    pub id: String,
    pub applied_on: String,
    #[serde(default)]
    pub application_end_date: Option<String>,
    pub fertilisation_type_code: String,
    pub application_method_code: String,
    pub dose_value: f64,
    pub dose_unit_code: String,
    pub fertiliser_material_id: String,
    #[serde(default)]
    pub sludge_application: bool,
    #[serde(default)]
    pub machinery_id: Option<String>,
    #[serde(default)]
    pub service_company: Option<String>,
    #[serde(default)]
    pub service_regfer_number: Option<String>,
    #[serde(default)]
    pub delivery_note_ref: Option<String>,
    #[serde(default)]
    pub yield_estimated_kg_ha: Option<f64>,
    #[serde(default)]
    pub yield_final_kg_ha: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewFertilisationPlot>,
    #[serde(default)]
    pub practices: Vec<String>,
}

/// Model section 7.1 — what the record book carries about the plan de abonado.
///
/// Much less than the plan itself: RD 1051/2022 art. 6 defines a document (the
/// parcels, the soil parameters, the water available, the recommended dose of
/// each nutrient with its moment, material, form of application and machinery,
/// and the anexo V emission measures), while art. 5.a defines what is written
/// into the book — the four things below. SIEX twin: `PlanAbonado`, whose
/// required set is that same list.
#[derive(Debug, Clone, Serialize)]
pub struct FertilisationPlan {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// Unidades fertilizantes, kg/ha of each (the model's footnote 2).
    pub needs_n_kg_ha: f64,
    pub needs_p2o5_kg_ha: f64,
    pub needs_k2o_kg_ha: f64,
    /// "Rendimiento esperado", kg/ha of produce.
    pub expected_yield_kg_ha: f64,
    /// A `PRODUCTOS` code, verbatim; `None` when the unit came out of fallow.
    pub preceding_crop_code: Option<String>,
    pub drawn_up_on: String,
    /// Twin-only: whether a calculation tool produced the plan.
    pub tool_generated: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A plan with the crops of the production unit it covers.
#[derive(Debug, Clone, Serialize)]
pub struct FertilisationPlanDetail {
    pub plan: FertilisationPlan,
    /// `crop` ids — an array because `PlanAbonado.DGCs` is one, and a unidad de
    /// producción may be several plots carrying the same crop.
    pub crop_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewFertilisationPlan {
    pub season_id: String,
    pub farm_id: String,
    pub needs_n_kg_ha: f64,
    pub needs_p2o5_kg_ha: f64,
    pub needs_k2o_kg_ha: f64,
    pub expected_yield_kg_ha: f64,
    #[serde(default)]
    pub preceding_crop_code: Option<String>,
    pub drawn_up_on: String,
    #[serde(default)]
    pub tool_generated: bool,
    #[serde(default)]
    pub notes: Option<String>,
    pub crop_ids: Vec<String>,
}

/// Full-row update. Art. 6 lets a plan be adjusted during the campaign to
/// follow the crop and the weather, so correcting one is the normal case
/// rather than the exception — `drawn_up_on` moves with it.
#[derive(Debug, Deserialize)]
pub struct UpdateFertilisationPlan {
    pub id: String,
    pub needs_n_kg_ha: f64,
    pub needs_p2o5_kg_ha: f64,
    pub needs_k2o_kg_ha: f64,
    pub expected_yield_kg_ha: f64,
    #[serde(default)]
    pub preceding_crop_code: Option<String>,
    pub drawn_up_on: String,
    #[serde(default)]
    pub tool_generated: bool,
    #[serde(default)]
    pub notes: Option<String>,
    pub crop_ids: Vec<String>,
}

/// Model section 8 — one irrigation (or one accumulated period of them) over a
/// set of plots. SIEX twin: `Riego`.
#[derive(Debug, Clone, Serialize)]
pub struct IrrigationRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// Start of the interval, or the single day it happened.
    pub irrigated_on: String,
    /// End of the interval; `None` for a single day. RD 1051/2022 art. 5.f
    /// allows fortnightly accumulation for intensive and fertigated crops.
    pub irrigation_end_date: Option<String>,
    pub irrigation_method_code: String,
    pub volume_value: f64,
    pub volume_unit_code: String,
    /// Anexo III C.l, conditional under RD 1051/2022 art. 17.2 — recorded only
    /// when the basin authority or irrigators' community supplies the figure,
    /// voluntary from the holder's own analysis.
    pub water_nitric_n_mg_l: Option<f64>,
    pub water_soluble_p2o5_mg_l: Option<f64>,
    /// FEGA TIPENERGIA code, verbatim and without a foreign key.
    pub energy_type_code: Option<String>,
    pub meter_number: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// One irrigated plot, with the crop on it at the time and the surface watered.
#[derive(Debug, Clone, Serialize)]
pub struct IrrigationPlot {
    pub id: String,
    pub irrigation_record_id: String,
    pub plot_id: String,
    pub crop_id: Option<String>,
    pub irrigated_area_ha: Option<f64>,
}

/// A record with everything the book and the forms need to show it.
#[derive(Debug, Clone, Serialize)]
pub struct IrrigationRecordDetail {
    pub record: IrrigationRecord,
    pub plots: Vec<IrrigationPlot>,
    /// `water_origin` codes; an array in the twin, because one irrigation can
    /// draw on more than one source.
    pub water_origins: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewIrrigationPlot {
    pub plot_id: String,
    #[serde(default)]
    pub crop_id: Option<String>,
    #[serde(default)]
    pub irrigated_area_ha: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct NewIrrigationRecord {
    pub season_id: String,
    pub farm_id: String,
    pub irrigated_on: String,
    #[serde(default)]
    pub irrigation_end_date: Option<String>,
    pub irrigation_method_code: String,
    pub volume_value: f64,
    pub volume_unit_code: String,
    #[serde(default)]
    pub water_nitric_n_mg_l: Option<f64>,
    #[serde(default)]
    pub water_soluble_p2o5_mg_l: Option<f64>,
    #[serde(default)]
    pub energy_type_code: Option<String>,
    #[serde(default)]
    pub meter_number: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewIrrigationPlot>,
    #[serde(default)]
    pub water_origins: Vec<String>,
}

/// Full-row update. `season_id` and `farm_id` are deliberately absent: an
/// irrigation never moves campaign or holding, so correcting that means delete
/// and re-enter (the `UpdateCrop` precedent). Plots and water origins are
/// reconciled from the submitted state.
///
/// This register is **fully correctable** from the start, for the reason
/// `seed_treatment` is: it holds no snapshot of another row's identity, so
/// there is nothing a later edit elsewhere could rewrite underneath it.
#[derive(Debug, Deserialize)]
pub struct UpdateIrrigationRecord {
    pub id: String,
    pub irrigated_on: String,
    #[serde(default)]
    pub irrigation_end_date: Option<String>,
    pub irrigation_method_code: String,
    pub volume_value: f64,
    pub volume_unit_code: String,
    #[serde(default)]
    pub water_nitric_n_mg_l: Option<f64>,
    #[serde(default)]
    pub water_soluble_p2o5_mg_l: Option<f64>,
    #[serde(default)]
    pub energy_type_code: Option<String>,
    #[serde(default)]
    pub meter_number: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub plots: Vec<NewIrrigationPlot>,
    #[serde(default)]
    pub water_origins: Vec<String>,
}
