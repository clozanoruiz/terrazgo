// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Rust structs mirroring the schema, plus the `New*` / `Update*` inputs the
//! Tauri commands deserialize into.
//!
//! `Lookup` is re-exported from core so a selector's shape is identical
//! whichever crate filled it.

pub use terrazgo_core::models::Lookup;

use serde::{Deserialize, Serialize};

/// One grazing: which animals grazed which plots, and between which dates.
/// Model 9.1; SIEX twin `Pastoreo` (RD 1048/2022 art. 30.2 ter).
#[derive(Debug, Clone, Serialize)]
pub struct GrazingRecord {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// Which of the decree's duties this evidences.
    pub practice_code: String,
    /// Model 9.1's "Id. del grupo de parcelas" — a free label, asked for only
    /// when the plots lie more than 10 km from the main livestock installation.
    pub plot_group_ref: Option<String>,
    /// Set when the animals grazed a cover: art. 42.1.c's maintenance, model
    /// 9.4's Pastoreo column. It also decides which page prints the record —
    /// 9.1 shows the grazings with no cover, 9.4 the ones with one.
    pub soil_cover_id: Option<String>,
    pub started_on: String,
    /// `None` = still grazing. The one-month deadline runs from here, so an
    /// open record is not late — it is simply not finished.
    pub ended_on: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A plot this grazing covered. The SIGPAC reference the model prints is
/// resolved from the plot at print time, never frozen here.
#[derive(Debug, Clone, Serialize)]
pub struct GrazingPlot {
    pub id: String,
    pub grazing_record_id: String,
    pub plot_id: String,
}

/// One line of `Pastoreo.Animales[]`: how many animals of which species, from
/// which livestock holding. Model 9.1's last three columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrazingAnimal {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub grazing_record_id: String,
    /// FEGA `ESPECIE_ANIMAL`, verbatim.
    pub species_code: String,
    /// The REGA of the holding the animals belong to — this farm's for its own
    /// animals, the owner's for third-party ones.
    pub rega_code: String,
    pub animal_count: i64,
}

/// A grazing with its children, which is how both the form and the record book
/// need it.
#[derive(Debug, Clone, Serialize)]
pub struct GrazingRecordDetail {
    pub record: GrazingRecord,
    pub plots: Vec<GrazingPlot>,
    pub animals: Vec<GrazingAnimal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewGrazingRecord {
    pub season_id: String,
    pub farm_id: String,
    pub practice_code: String,
    pub plot_group_ref: Option<String>,
    /// Optional, and set by the cover form rather than typed: the cover this
    /// grazing maintained.
    #[serde(default)]
    pub soil_cover_id: Option<String>,
    pub started_on: String,
    pub ended_on: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
    pub animals: Vec<GrazingAnimal>,
}

/// Full-row correction. Carries neither `season_id` nor `farm_id`: re-homing a
/// record would take its plots with it, and correcting a wrong farm means
/// deleting and re-creating — the `plot.farm_id` precedent.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateGrazingRecord {
    pub practice_code: String,
    pub plot_group_ref: Option<String>,
    #[serde(default)]
    pub soil_cover_id: Option<String>,
    pub started_on: String,
    pub ended_on: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
    pub animals: Vec<GrazingAnimal>,
}

/// One operation carried out on the land. Model 9.2 and the book's "9.6";
/// SIEX twin `LaboresCulturales` (RD 1048/2022 arts. 31, 31.4.d and anexo IV,
/// with arts. 45.2 and 42.1.c joining in later seams).
#[derive(Debug, Clone, Serialize)]
pub struct CulturalOperation {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// Which duty this evidences — and therefore which page of section 9 the
    /// row prints on.
    pub practice_code: String,
    pub operation_kind_code: String,
    pub performed_on: String,
    /// `None` = a single day's work, never "unknown".
    pub performed_end_date: Option<String>,
    /// Model 9.2 footnote (4): the date **and the activity**, for the
    /// open-ended maintenance the kind codes cannot name.
    pub activity_description: Option<String>,
    /// FEGA `DEST_RES_VEG`, verbatim. Value 9 is what turns a pruning into a
    /// P7 inert cover.
    pub residue_destination_code: Option<String>,
    /// Set when this operation maintained a cover: art. 42.1.c, printed as
    /// model 9.4's Siega and Desbrozado columns.
    pub soil_cover_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A plot the operation covered.
#[derive(Debug, Clone, Serialize)]
pub struct CulturalOperationPlot {
    pub id: String,
    pub cultural_operation_id: String,
    pub plot_id: String,
}

/// An operation with its plots, which is how both the form and the record book
/// need it.
#[derive(Debug, Clone, Serialize)]
pub struct CulturalOperationDetail {
    pub record: CulturalOperation,
    pub plots: Vec<CulturalOperationPlot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewCulturalOperation {
    pub season_id: String,
    pub farm_id: String,
    pub practice_code: String,
    pub operation_kind_code: String,
    pub performed_on: String,
    pub performed_end_date: Option<String>,
    pub activity_description: Option<String>,
    pub residue_destination_code: Option<String>,
    /// Optional, and set by the cover form rather than typed.
    #[serde(default)]
    pub soil_cover_id: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
}

/// Full-row correction, on the same terms as [`UpdateGrazingRecord`].
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCulturalOperation {
    pub practice_code: String,
    pub operation_kind_code: String,
    pub performed_on: String,
    pub performed_end_date: Option<String>,
    pub activity_description: Option<String>,
    pub residue_destination_code: Option<String>,
    #[serde(default)]
    pub soil_cover_id: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
}

/// A cover established over one or more plots. Model 9.4 (live, P6, art. 42)
/// and model 9.5 (inert, P7, art. 43); SIEX twin `DatosCubierta`.
#[derive(Debug, Clone, Serialize)]
pub struct SoilCover {
    pub id: String,
    pub season_id: String,
    pub farm_id: String,
    /// `plant_cover` or `inert_cover` — which of the two pages this prints on.
    pub practice_code: String,
    /// FEGA `TIPO_COBERTURA_SUELO`, verbatim. Stored for the twin and the
    /// workbook; neither printed page has a column for it.
    pub cover_type_code: String,
    /// Art. 42.1.a / 43.1.a — the first of the article's three annotations.
    pub established_on: String,
    /// Art. 42.1.e / 43.1.b — the second, on its own deadline. All three
    /// together or none.
    pub width_m: Option<f64>,
    pub free_canopy_width_m: Option<f64>,
    /// When the widths were stated. Neither the decree nor the twin asks for
    /// it; it is what separates "measured in June" from "never measured".
    pub widths_stated_on: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// A plot the cover was established over. `DatosCubierta.DGCs[]`.
#[derive(Debug, Clone, Serialize)]
pub struct SoilCoverPlot {
    pub id: String,
    pub soil_cover_id: String,
    pub plot_id: String,
}

/// One line of art. 42.1.c's maintenance — the third of art. 42's annotations,
/// and model 9.4's last three columns.
///
/// It is a line of the cover form, but never a table: a siega or a desbroce is
/// stored as a `cultural_operation` and a pastoreo as a `grazing_record`,
/// because that is what each of them is and what the exchange format already
/// calls them. This struct is the shape the form sends and reads back, and the
/// repository writes it through the very same functions the 9.2 and 9.1 forms
/// use, so one register cannot validate what the other does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverMaintenanceLine {
    /// Empty on a line the form has just added; the underlying record's id once
    /// it exists, which is what a correction reconciles on.
    #[serde(default)]
    pub id: String,
    /// `mowing` or `brush_cutting` — both `cultural_operation_kind` codes — or
    /// [`GRAZING_MAINTENANCE`], which is a register of its own rather than a
    /// kind of operation.
    pub kind_code: String,
    pub performed_on: String,
    /// `None` = a single day's work, as everywhere else in this module.
    pub performed_end_date: Option<String>,
    /// Grazing lines only, and required on them: the decree asks for the animal
    /// groups on every grazing, so a cover grazing states them like any other
    /// rather than being excused because it was entered from another form.
    #[serde(default)]
    pub animals: Vec<GrazingAnimal>,
}

/// The maintenance kind that is a grazing rather than a cultural operation.
///
/// Deliberately not a `cultural_operation_kind` row: `TIPO_LABOR` publishes no
/// pastoreo code, a grazing carries animal groups that no operation has, and
/// `Pastoreo` is its twin whichever land it happens on.
pub const GRAZING_MAINTENANCE: &str = "grazing";

/// A cover with everything the form and the printed pages need.
#[derive(Debug, Clone, Serialize)]
pub struct SoilCoverDetail {
    pub record: SoilCover,
    pub plots: Vec<SoilCoverPlot>,
    /// Ordered by date, whichever table each line came from.
    pub maintenance: Vec<CoverMaintenanceLine>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewSoilCover {
    pub season_id: String,
    pub farm_id: String,
    pub practice_code: String,
    pub cover_type_code: String,
    pub established_on: String,
    pub width_m: Option<f64>,
    pub free_canopy_width_m: Option<f64>,
    pub widths_stated_on: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
    /// Written in the same transaction as the cover, so a book never holds a
    /// cover whose maintenance half-saved.
    #[serde(default)]
    pub maintenance: Vec<CoverMaintenanceLine>,
}

/// Full-row correction, on the same terms as [`UpdateGrazingRecord`].
///
/// The maintenance lines reconcile like `grazing_animal` does: a line carrying
/// an id is corrected in place, one without is created, and one the form no
/// longer sends is withdrawn — as a soft delete, because each line is a
/// regulatory record in its own register and its history has to survive.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSoilCover {
    pub practice_code: String,
    pub cover_type_code: String,
    pub established_on: String,
    pub width_m: Option<f64>,
    pub free_canopy_width_m: Option<f64>,
    pub widths_stated_on: Option<String>,
    pub notes: Option<String>,
    pub plot_ids: Vec<String>,
    #[serde(default)]
    pub maintenance: Vec<CoverMaintenanceLine>,
}
