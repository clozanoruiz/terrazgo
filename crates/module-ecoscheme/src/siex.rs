// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mapping between this module's neutral codes and the Spanish SIEX coding
//! (FEGA catalogues; design in docs/siex-export.md).
//!
//! Same two-tier rule as the other modules' `siex`: small closed lists are
//! owned as English lookups and mapped here, while provider lists too large to
//! own are stored verbatim as catalogue codes and only named.
//!
//! # What has no map, and why that is not an omission
//!
//! [`crate::repository::list_eco_practices`]' vocabulary maps to nothing.
//! **FEGA publishes no P1–P7 catalogue** — verified across all 287 entries of
//! its catalogue registry — and the CUE exchange schema has no eco-scheme claim
//! member anywhere: it models *activities*, not entitlements. So `eco_practice`
//! is ours alone, used to discriminate which duty a record evidences and to
//! decide which table the book prints it in. Nothing to map, and nothing to
//! watch for upstream.
//!
//! A contract test (`tests/siex_mapping.rs`) checks the mapping that does exist
//! against the vendored catalogue snapshot in BOTH directions, so a snapshot
//! refresh that adds or retires a code fails the suite instead of drifting
//! silently.

/// SIEX `LaboresCulturales.TipoLabor` code (catalogue `TIPO_LABOR`).
///
/// **Deliberately not injective.** `mowing` and `brush_cutting` both answer to
/// 5, "Desbroce y siega", because the provider folds into one code what model
/// 9.4 prints as two columns — a farmer who mowed a cover has not brush-cut it,
/// and art. 42.1.c asks which maintenance was performed. Splitting is safe in
/// the direction that matters (both export as 5) while merging would lose a
/// distinction the printed form asks for. [`SHARED_SIEX_CODES`] pins the pair
/// so it reads as a decision rather than an oversight.
pub fn cultural_operation_kind_to_siex(code: &str) -> Option<i64> {
    match code {
        "no_tillage" => Some(0),
        "tillage" => Some(1),
        "levelling" => Some(2),
        "ridging" => Some(3),
        "weeding" => Some(4),
        "mowing" => Some(5),
        "brush_cutting" => Some(5),
        "drainage" => Some(6),
        "pruning" => Some(7),
        "thinning" => Some(8),
        "staking" => Some(9),
        "grafting" => Some(10),
        "pruning_removal" => Some(11),
        "green_pruning" => Some(12),
        "rolling" => Some(13),
        _ => None,
    }
}

/// Every group of our codes that deliberately shares one SIEX code, pinned so
/// the contract test can assert surjectivity onto the catalogue while allowing
/// exactly these collisions — and refusing any other.
///
/// Read by `tests/siex_mapping.rs` rather than at runtime: it is a decision
/// record, the `UNMAPPED_COLUMNS` precedent in module-fertilisation.
pub const SHARED_SIEX_CODES: &[&[&str]] = &[&["mowing", "brush_cutting"]];

/// Catalogue that `cultural_operation.operation_kind_code` is mapped onto — the
/// vocabulary of land operations, in the country whose registry publishes one.
/// A country with no coded list gets `None`, and the export simply omits the
/// field rather than sending Spain's coding.
pub fn cultural_operation_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("TIPO_LABOR")
}

/// Catalogue that `grazing_animal.species_code` is stored against — tier 2, so
/// the code is kept verbatim rather than mapped: 198 species is a provider list
/// this module does not own, and the code IS the payload.
pub fn animal_species_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("ESPECIE_ANIMAL")
}

/// Catalogue that `cultural_operation.residue_destination_code` is stored
/// against — tier 2 like the species, though only nine rows: what settles it is
/// not the size but that the twin sends this code and the decree enumerates
/// nothing, so there is no closed list of ours to own.
///
/// Its value `"9"` — *"Trituración de restos de poda y depositado sobre el
/// terreno de los mismos"* — is the one code with meaning beyond display: it is
/// what turns a pruning into the P7 inert cover of art. 43. Named here rather
/// than left as a literal at each reader, so the identity is stated once. See
/// also [`RESIDUE_LEFT_ON_PLOT`], which is the wider question of whether the
/// residue stayed on the ground at all.
pub fn residue_destination_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("DEST_RES_VEG")
}

/// `DEST_RES_VEG`'s code for triturating pruning residue onto the ground.
///
/// The evidence chain art. 43.1.a wants: an inert cover exists BECAUSE a poda
/// was left on the land. The twin agrees and puts the booleans it derives —
/// `DepositadoSueloDesb` / `DepositadoSueloPoda` — on `LaboresCulturales`
/// rather than on `DatosCubierta`, so this code is their source.
pub const RESIDUE_LEFT_ON_GROUND: &str = "9";

/// Every `DEST_RES_VEG` code under which the residue STAYS on the plot, which is
/// the question `DepositadoSueloDesb` and `DepositadoSueloPoda` ask.
///
/// Two, and each is named by its own label: 1 is *"Incorporación al suelo o
/// distribución en parcela (previo picado o no)"* and 9 is
/// [`RESIDUE_LEFT_ON_GROUND`]. The other seven all take the residue away — sold,
/// eaten, burnt, composted or hauled to a plant — so under any of them both
/// booleans are false.
///
/// 9 alone would not do: its label names *poda* residue, leaving a desbroce left
/// on the ground with no code to answer to, and an Obligatorio boolean stuck at
/// false for a farmer who did exactly what art. 42.1.c's *"depositado sobre el
/// terreno de los restos a modo de mulching"* describes.
pub const RESIDUE_LEFT_ON_PLOT: &[&str] = &["1", RESIDUE_LEFT_ON_GROUND];

/// Operation kinds whose residue is *desbrozado*, so a "left on the plot"
/// destination fills `DepositadoSueloDesb`.
///
/// Both of them, because `TIPO_LABOR` 5 is one code — *"Desbroce y siega"* — for
/// what model 9.4 prints as two columns.
pub const RESIDUE_KINDS_BRUSH: &[&str] = &["mowing", "brush_cutting"];

/// Operation kinds whose residue is *de poda*, filling `DepositadoSueloPoda`.
///
/// **`pruning_removal` is deliberately absent**: `TIPO_LABOR` 11 is literally
/// *"Eliminación de restos de poda"*, so its residue left the plot by
/// definition. A record combining it with a "left on the plot" destination
/// contradicts itself, and this list resolves that the only way that does not
/// invent an answer — by leaving both booleans false, which is what the removal
/// says.
pub const RESIDUE_KINDS_PRUNING: &[&str] = &["pruning", "green_pruning"];

/// Catalogue that `soil_cover.cover_type_code` is stored against — tier 2,
/// verbatim, because `DatosCubierta.TipoCobertura` sends this very code.
pub fn cover_type_catalogue(country_code: &str) -> Option<&'static str> {
    (country_code == "es").then_some("TIPO_COBERTURA_SUELO")
}

/// Which `TIPO_COBERTURA_SUELO` codes each cover practice can plausibly be,
/// read by the FORM to narrow its picker — never by the repository to refuse a
/// record.
///
/// The split is the decree's own wording. Art. 42.1.a establishes *"la cubierta
/// vegetal espontánea o sembrada"*, which is codes 2 and 3; art. 43.1.a
/// establishes *"la cubierta inerte de restos de poda"*, which is code 4 and
/// specifically not 5, "otros materiales".
///
/// It narrows rather than validates because this is a provider registry that
/// grows between releases — it gained code 6 in 2024, and a user's own
/// catalogue refresh can carry a code this build has never seen. Refusing one
/// would lock a farmer out of recording a lawful cover; offering a shorter list
/// merely means the rare case is typed rather than picked.
pub const PLANT_COVER_TYPES: &[&str] = &["2", "3"];

/// See [`PLANT_COVER_TYPES`].
pub const INERT_COVER_TYPES: &[&str] = &["4"];

/// The `TIPO_COBERTURA_SUELO` codes that belong to NEITHER cover practice,
/// pinned so the contract test can account for every active row in the
/// catalogue and fail when FEGA adds one nobody has classified.
///
/// 1 is bare soil, 5 is an inert cover of something other than pruning residue
/// (nutshells, stones) and 6 is the regeneration of permanent pasture. None of
/// the three is what art. 42 or art. 43 asks a holding to establish, which is
/// also why `eco_practice` could not be derived from this catalogue.
pub const NON_COVER_TYPES: &[&str] = &["1", "5", "6"];
