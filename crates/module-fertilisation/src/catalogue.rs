// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reference-catalogue reads section 6's coded fields need.
//!
//! Storage and import belong to `terrazgo_core::catalogue`; which catalogue a
//! field speaks belongs to [`crate::siex`]. What lives here is the little that
//! is neither — turning a catalogue into the list a picker offers, for the
//! three whose raw rows are not that list.

use rusqlite::Connection;
use serde::Serialize;
use terrazgo_core::catalogue::{CatalogueCode, all_codes, held_picks};

use crate::error::Result;
use crate::siex;

/// One code in a catalogue-backed picker — core's type, the one every module's
/// picker list returns, so one Svelte component serves them all.
pub use terrazgo_core::catalogue::CataloguePick;

/// Anexo III C.d's first level: the kind of material (FEGA `MAT_FERTI`, 24
/// values from "Estiércol sólido de ovino" to "Lodos EDAR").
pub fn fertiliser_materials(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    picks(
        conn,
        siex::fertiliser_material_catalogue(country_code),
        None,
    )
}

/// C.d's second level: the named commercial product (FEGA
/// `DETALLE_MATERIAL_FERT`, 1243 rows).
///
/// Narrowed by the chosen first level when there is one, because the file
/// carries each product under its parent `MAT_FERTI` code and a picker over all
/// 1243 is unusable. A parent that narrows to nothing falls back to the full
/// list — the `SpeciesPicker` rule: a filter that hides everything is worse
/// than no filter.
pub fn fertiliser_material_details(
    conn: &Connection,
    country_code: &str,
    material_code: Option<&str>,
) -> Result<Vec<CataloguePick>> {
    let catalogue = siex::fertiliser_detail_catalogue(country_code);
    let narrowed = picks(conn, catalogue, material_code)?;
    if narrowed.is_empty() {
        return picks(conn, catalogue, None);
    }
    Ok(narrowed)
}

/// The nutrients, micronutrients or heavy metals a composition line can name —
/// one of three catalogues, chosen by `kind_code`.
pub fn nutrients(
    conn: &Connection,
    country_code: &str,
    kind_code: &str,
) -> Result<Vec<CataloguePick>> {
    picks(
        conn,
        siex::nutrient_catalogue(country_code, kind_code),
        None,
    )
}

/// One composition figure a chosen catalogue product publishes, in the shape
/// `fertiliser_material_nutrient` stores.
#[derive(Debug, Clone, Serialize)]
pub struct CompositionLine {
    pub kind_code: String,
    pub nutrient_code: String,
    pub percentage: f64,
}

/// What the catalogue publishes about one named product that the material form
/// can take instead of copying it off the sack — see [`material_proposal`].
#[derive(Debug, Clone, Serialize)]
pub struct MaterialProposal {
    pub composition: Vec<CompositionLine>,
    /// For a liquid product only, in kg/L — `fertiliser_material.density_kg_l`.
    pub density_kg_l: Option<f64>,
}

/// The `DETALLE_MATERIAL_FERT` columns the density proposal reads. Grams per
/// cubic centimetre is kilograms per litre, so the figure needs no conversion.
const DENSITY_COLUMN: &str = "Densidad (g/cm3)";
const AGGREGATION_COLUMN: &str = "Estado de agregación";
const LIQUID: &str = "Líquido";

/// The columns of `DETALLE_MATERIAL_FERT` that can be read as a percentage of
/// the material, paired with the nutrient catalogue code they answer.
///
/// **Deliberately not the whole file.** Three groups of columns are left out,
/// and the reasons are worth stating because each would otherwise put a wrong
/// number into a legal record:
///
///   * **The seven heavy-metal columns** (Cd, Cu, Pb, Ni, Zn, Hg, Cr) mix
///     units across rows with nothing in the file to tell them apart. A foliar
///     zinc fertiliser declares `Zinc (Zn) = 20,1`, plainly a percentage,
///     while "CODA-Ca-L" declares `Cobre (Cu) = 70`, `Plomo (Pb) = 45` and
///     `Cromo total (Cr) = 70` on a product whose N, P and K are all zero —
///     figures that are only sane read as mg/kg. Prefilling either way would
///     be wrong for the other by a factor of ten thousand, so C.i's metals are
///     always entered by hand, from the analysis the farmer holds.
///   * **`P_% TOTAL` and `K_% TOTAL`** are ELEMENTAL phosphorus and potassium
///     (the median `P2O5 % total` / `P_% TOTAL` ratio across 816 rows is
///     2,2936 — the oxide conversion factor 2,2914). `MACRONUTRIENTES` codes
///     oxides only, so there is nothing to map them onto.
///   * Copper and zinc as **micronutrients** (`MICRONUTRIENTES` 3 and 6): the
///     file's only Cu and Zn columns sit inside the metals block, so a
///     declared micronutrient content cannot be told from a contaminant one.
///
/// Everything kept is unambiguous: the headers say "%", and the extremes check
/// out against real products (N total peaks at 82, which is anhydrous ammonia,
/// 82-0-0).
const COMPOSITION_COLUMNS: &[(&str, &str, &str)] = &[
    // (provider column header, nutrient kind, catalogue code)
    ("N_% TOTAL", "macro", "1"),
    (
        "N orgánico % (en fertilizantes orgánico-minerales)",
        "macro",
        "2",
    ),
    ("N % nítrico", "macro", "3"),
    ("N % amoniacal", "macro", "4"),
    ("N % ureico", "macro", "5"),
    ("P2O5 % total", "macro", "6"),
    ("P2O5 % soluble en agua", "macro", "7"),
    (
        "P2O5 % soluble en citrato amónico neutro y agua",
        "macro",
        "8",
    ),
    ("K2O % total", "macro", "9"),
    ("K2O % soluble en agua", "macro", "10"),
    ("% Corg", "macro", "11"),
    ("Ca (CaO)", "macro", "12"),
    ("Mg (MgO)", "macro", "13"),
    ("S (SO3)", "macro", "14"),
    ("Boro (B)", "micro", "1"),
    ("Cobalto (Co)", "micro", "2"),
    ("Manganeso (Mn)", "micro", "4"),
    ("Molibdeno (Mo)", "micro", "5"),
    ("Hierro (Fe)", "micro", "7"),
];

/// The numeric columns [`COMPOSITION_COLUMNS`] deliberately leaves out, each
/// with the reason, so the exclusion is a stated decision rather than an
/// oversight — and so `the_mapping_accounts_for_every_column_of_the_file`
/// can fail the day a snapshot refresh adds a column nobody has looked at.
///
/// Note the metals cannot be rescued by dropping units: SIEX's field is
/// literally `Porcentaje`, the model's column header says "(%)", section 7.1
/// multiplies richness by a dose, and RD 1051/2022 anexo IV states the limits
/// in mg/kg de materia seca. A figure with no unit could be compared against
/// none of those.
/// Test-only: nothing reads these at runtime, which is the point being pinned.
/// The code and label columns are deliberately absent — the importer lifts
/// them out of `attrs`, so they never reach this mapping.
///
/// Together with [`COMPOSITION_COLUMNS`] and the two density columns
/// ([`DENSITY_COLUMN`], [`AGGREGATION_COLUMN`]) this accounts for the file
/// EXACTLY, so a provider adding a column cannot pass as a nutrient nobody
/// decided about. Checked against the header row `terrazgo-core` pins, not against
/// sampled data.
#[cfg(test)]
const UNMAPPED_COLUMNS: &[(&str, &str)] = &[
    // --- identifiers and classification, not figures ---
    (
        "Código SIEX",
        "the parent MAT_FERTI code, not a composition figure",
    ),
    (
        "Tipo material fertilizantes según lista SIEX",
        "the parent material's label",
    ),
    ("D_CLASIFICA_NIVEL1", "the provider's own classification"),
    ("D_CLASIFICA_NIVEL2", "the provider's own classification"),
    (
        "Nombre producto",
        "trade name; the picker's label carries it",
    ),
    ("Fabricante", "manufacturer, not a composition figure"),
    ("D_GRUPO_CONSUMO", "the provider's own consumption grouping"),
    // --- numeric, but not mappable onto a nutrient catalogue ---
    (
        "P_% TOTAL",
        "elemental phosphorus; MACRONUTRIENTES codes the oxide (P₂O₅) only",
    ),
    (
        "K_% TOTAL",
        "elemental potassium; MACRONUTRIENTES codes the oxide (K₂O) only",
    ),
    (
        "¿Inhibidor de la nitrificación? Si/No",
        "a yes/no property of the product, not a percentage",
    ),
    (
        "¿Inhibidor de la ureasa? Si/No",
        "a yes/no property of the product, not a percentage",
    ),
    // --- the heavy metals: one column, two units, nothing to tell them apart ---
    ("Cadmio (Cd)", "heavy metal: the column mixes % and mg/kg"),
    ("Cobre (Cu)", "heavy metal: the column mixes % and mg/kg"),
    ("Plomo (Pb)", "heavy metal: the column mixes % and mg/kg"),
    ("Níquel (Ni)", "heavy metal: the column mixes % and mg/kg"),
    ("Zinc (Zn)", "heavy metal: the column mixes % and mg/kg"),
    ("Mercurio (Hg)", "heavy metal: the column mixes % and mg/kg"),
    (
        "Cromo total (Cr)",
        "heavy metal: the column mixes % and mg/kg",
    ),
];

/// What the catalogue publishes about one named product, offered to the
/// material form so Anexo III C.h's eight values — and a liquid's density —
/// need not be copied off the sack by hand.
///
/// A **proposal**, never a record: the caller applies it explicitly and may
/// edit or drop any of it, because the label in the farmer's hand is the
/// source of truth and this snapshot rides app releases.
///
/// A zero is NOT proposed. The provider fills unstated cells with `0`, and our
/// own rule is that blank and zero are different claims — "contains no
/// potassium" is a statement, "did not say" is not.
///
/// The density is proposed for a liquid only: Anexo V asks for it "cuando se
/// trate de producto fertilizante líquido", and what the file states for a
/// solid is a bulk density the field does not mean.
pub fn material_proposal(
    conn: &Connection,
    country_code: &str,
    detail_code: &str,
) -> Result<MaterialProposal> {
    let nothing = MaterialProposal {
        composition: Vec::new(),
        density_kg_l: None,
    };
    let Some(catalogue_id) = siex::fertiliser_detail_catalogue(country_code) else {
        return Ok(nothing);
    };
    let Some(row) = terrazgo_core::catalogue::find_code(conn, catalogue_id, detail_code)?
        .into_iter()
        .next()
    else {
        return Ok(nothing);
    };
    Ok(MaterialProposal {
        composition: composition(&row),
        density_kg_l: liquid_density(&row),
    })
}

/// The composition lines one catalogue row states, zeros left out.
fn composition(row: &CatalogueCode) -> Vec<CompositionLine> {
    let mut lines = Vec::new();
    for (header, kind_code, nutrient_code) in COMPOSITION_COLUMNS {
        let Some(percentage) = attr(row, header).and_then(parse_percentage) else {
            continue;
        };
        if percentage <= 0.0 {
            continue;
        }
        lines.push(CompositionLine {
            kind_code: (*kind_code).to_string(),
            nutrient_code: (*nutrient_code).to_string(),
            percentage,
        });
    }
    lines
}

/// A liquid product's stated density, in kg/L; `None` for a solid, or when
/// the file leaves it at `0`.
fn liquid_density(row: &CatalogueCode) -> Option<f64> {
    if attr(row, AGGREGATION_COLUMN) != Some(LIQUID) {
        return None;
    }
    attr(row, DENSITY_COLUMN)
        .and_then(parse_decimal)
        .filter(|density| *density > 0.0)
}

/// The provider writes decimals with a point in this file, but a refreshed
/// snapshot spelling one with a comma must not silently become a different
/// number — so both are read, and anything else is skipped rather than guessed.
fn parse_decimal(raw: &str) -> Option<f64> {
    let value: f64 = raw.trim().replace(',', ".").parse().ok()?;
    value.is_finite().then_some(value)
}

fn parse_percentage(raw: &str) -> Option<f64> {
    parse_decimal(raw).filter(|value| (0.0..=100.0).contains(value))
}

/// The good practices a fertilisation record can claim (FEGA
/// `BUENAS_PRACTICAS_AMBITOS`, 41 of its 60 marked "SI" under "Ámbito
/// Fertilización").
///
/// Every practice is listed and only those are offered. The file is one list
/// for three ámbitos, so offering all of it would put irrigation and
/// phytosanitary practices on a fertilisation record. But it can only take a
/// practice out of an ámbito by turning its "SI" into "NO" — there is no baja
/// date per ámbito — and a record already claiming one must still name it
/// rather than show a bare code, which is what a list that dropped it would
/// leave.
pub fn fertilisation_practices(
    conn: &Connection,
    country_code: &str,
) -> Result<Vec<CataloguePick>> {
    practices_in_scope(conn, country_code, siex::FERTILISATION_SCOPE_KEY)
}

/// The good practices an irrigation record can claim — the same list, offered
/// where it marks "SI" under "Ámbito Riego", for the same reasons
/// [`fertilisation_practices`] lists all and offers some.
pub fn irrigation_practices(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    practices_in_scope(conn, country_code, siex::IRRIGATION_SCOPE_KEY)
}

/// Every practice, offered only where `scope_key` says "SI".
fn practices_in_scope(
    conn: &Connection,
    country_code: &str,
    scope_key: &str,
) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = siex::good_practice_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    // One row per code: the file has one per practice, and the import keeps
    // one per identity, which is the code alone.
    Ok(all_codes(conn, catalogue_id)?
        .into_iter()
        .map(|row| CataloguePick {
            offered: row.offered && attr(&row, scope_key) == Some(siex::IN_SCOPE),
            name: row.label,
            code: row.code,
        })
        .collect())
}

/// The catalogue's active rows as picker entries, first label per code wins.
///
/// `parent` narrows to the rows whose parent-code attribute matches, for the
/// one catalogue published as a child list. A country with no coded list gets
/// an empty list rather than Spain's — nothing to offer is not the same as
/// offering the wrong vocabulary.
fn picks(
    conn: &Connection,
    catalogue_id: Option<&str>,
    parent: Option<&str>,
) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = catalogue_id else {
        return Ok(Vec::new());
    };
    Ok(held_picks(
        conn,
        catalogue_id,
        |row| match parent {
            Some(parent) => attr(row, siex::MATERIAL_PARENT_KEY) == Some(parent),
            None => true,
        },
        |row| row.label.clone(),
    )?)
}

/// One provider column of a catalogue row, by its verbatim header. The importer
/// stores every column it did not take as code or label in `attrs`, keys
/// exactly as the provider spells them — accents included.
fn attr<'a>(row: &'a CatalogueCode, key: &str) -> Option<&'a str> {
    row.attrs.as_ref()?.get(key)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut conn = crate::open_in_memory().expect("in-memory database");
        terrazgo_core::catalogue::ensure_catalogues(&mut conn).expect("catalogue import");
        conn
    }

    #[test]
    fn material_kinds_are_the_whole_short_list() {
        let conn = db();
        let picks = fertiliser_materials(&conn, "es").unwrap();
        // MAT_FERTI: 24 rows, codes 0..=23 (docs/maintenance.md §1).
        assert_eq!(picks.len(), 24);
        assert!(
            picks.iter().any(|p| p.code == "22"),
            "sewage sludge (code 22) is the material Anexo III C.i hangs on"
        );
    }

    #[test]
    fn a_retired_material_kind_is_listed_and_not_offered() {
        // A picker must still name the code a record already carries when it
        // no longer offers it: retired by the authority since, or held only by
        // another device's newer catalogue (docs/sync.md → What stays
        // device-local).
        let conn = db();
        conn.execute(
            "UPDATE catalogue_code SET retired_on = '2026-01-01'
             WHERE catalogue_id = 'MAT_FERTI' AND code = '22'",
            [],
        )
        .unwrap();
        let picks = fertiliser_materials(&conn, "es").unwrap();
        assert_eq!(picks.len(), 24);
        let sludge = picks.iter().find(|pick| pick.code == "22").unwrap();
        assert!(!sludge.offered);
        assert_eq!(picks.iter().filter(|pick| pick.offered).count(), 23);
    }

    #[test]
    fn material_details_narrow_to_their_parent() {
        let conn = db();
        let all = fertiliser_material_details(&conn, "es", None).unwrap();
        let inorganic = fertiliser_material_details(&conn, "es", Some("14")).unwrap();
        assert!(
            inorganic.len() < all.len(),
            "a parent material must narrow the 1243-row product list"
        );
        assert!(
            inorganic.iter().any(|p| p.name.contains("NITRATO AMÓNICO")),
            "MAT_FERTI 14 is 'abonos inorgánicos', where ammonium nitrate lives"
        );
    }

    #[test]
    fn an_unknown_parent_falls_back_to_the_full_list() {
        let conn = db();
        let all = fertiliser_material_details(&conn, "es", None).unwrap();
        // Every one of MAT_FERTI's 24 kinds does have published products today
        // (1042 of the 1243 under kind 14 alone), so the fallback only fires
        // for a parent the snapshot does not know — a record entered against a
        // newer catalogue, say. It must still offer a usable picker.
        let fallback = fertiliser_material_details(&conn, "es", Some("99")).unwrap();
        assert_eq!(fallback.len(), all.len());
        assert_eq!(
            all.len(),
            1243,
            "codes are unique across the file's parents"
        );
    }

    #[test]
    fn nutrient_lists_differ_per_kind() {
        let conn = db();
        // Sizes are the vendored files': 16 macro, 7 micro, 7 heavy metals.
        assert_eq!(nutrients(&conn, "es", "macro").unwrap().len(), 16);
        assert_eq!(nutrients(&conn, "es", "micro").unwrap().len(), 7);
        assert_eq!(nutrients(&conn, "es", "heavy_metal").unwrap().len(), 7);
        assert!(nutrients(&conn, "es", "nonsense").unwrap().is_empty());
    }

    #[test]
    fn code_three_means_three_different_things_across_the_nutrient_lists() {
        let conn = db();
        let name = |kind: &str| {
            nutrients(&conn, "es", kind)
                .unwrap()
                .into_iter()
                .find(|p| p.code == "3")
                .map(|p| p.name)
                .expect("code 3")
        };
        // This is why `fertiliser_material_nutrient` needs `kind_code`: the
        // integer alone does not identify a nutrient.
        assert_eq!(name("macro"), "N nítrico");
        assert_eq!(name("micro"), "Cobre (Cu)");
        assert_eq!(name("heavy_metal"), "Plomo (Pb)");
    }

    #[test]
    fn practices_are_offered_only_in_the_fertilisation_scope() {
        let conn = db();
        let picks = fertilisation_practices(&conn, "es").unwrap();
        // BUENAS_PRACTICAS_AMBITOS: 60 practices, 41 of them "SI" under
        // "Ámbito Fertilización".
        assert_eq!(picks.len(), 60);
        assert_eq!(picks.iter().filter(|p| p.offered).count(), 41);
        let pick = |code: &str| picks.iter().find(|p| p.code == code).unwrap();
        assert!(
            pick("1").offered,
            "slurry applied in bands is a fertilisation practice"
        );
        // 23 is irrigation's alone: listed, so a record claiming it names it,
        // and never offered on a fertilisation.
        assert_eq!(pick("23").name, "Riego localizado.");
        assert!(!pick("23").offered);
    }

    #[test]
    fn irrigation_is_offered_the_practices_marked_for_riego() {
        let conn = db();
        let picks = irrigation_practices(&conn, "es").unwrap();
        // BUENAS_PRACTICAS_AMBITOS: 31 of its 60 are "SI" under "Ámbito Riego".
        assert_eq!(picks.len(), 60);
        assert_eq!(picks.iter().filter(|p| p.offered).count(), 31);
        let pick = |code: &str| picks.iter().find(|p| p.code == code).unwrap();
        assert!(pick("23").offered, "riego localizado");
        assert!(
            pick("0").offered,
            "'No realiza buenas prácticas' in every ámbito"
        );
        // Slurry in bands is fertilisation's alone: listed, never offered.
        assert!(!pick("1").offered);
        assert!(irrigation_practices(&conn, "fr").unwrap().is_empty());
    }

    #[test]
    fn a_retired_practice_is_not_offered_even_in_scope() {
        let conn = db();
        conn.execute(
            "UPDATE catalogue_code SET retired_on = '2026-01-01'
             WHERE catalogue_id = 'BUENAS_PRACTICAS_AMBITOS' AND code = '1'",
            [],
        )
        .unwrap();
        let picks = fertilisation_practices(&conn, "es").unwrap();
        let slurry = picks.iter().find(|p| p.code == "1").unwrap();
        assert!(!slurry.offered);
        assert_eq!(picks.iter().filter(|p| p.offered).count(), 40);
    }

    /// The code of one product by its published name, so the composition tests
    /// read as claims about real catalogue entries.
    fn detail_code(conn: &Connection, name: &str) -> String {
        fertiliser_material_details(conn, "es", None)
            .unwrap()
            .into_iter()
            .find(|p| p.name.contains(name))
            .unwrap_or_else(|| panic!("no catalogue product matching '{name}'"))
            .code
    }

    #[test]
    fn a_products_published_composition_becomes_nutrient_lines() {
        let conn = db();
        let lines = material_proposal(&conn, "es", &detail_code(&conn, "Urea 46"))
            .unwrap()
            .composition;
        let get = |kind: &str, code: &str| {
            lines
                .iter()
                .find(|l| l.kind_code == kind && l.nutrient_code == code)
                .map(|l| l.percentage)
        };
        // Urea is 46-0-0, all of its nitrogen in the ureic form.
        assert_eq!(get("macro", "1"), Some(46.0));
        assert_eq!(get("macro", "5"), Some(46.0));
        // Zero is not a proposal: the provider fills unstated cells with 0, and
        // blank and zero are different claims.
        assert_eq!(get("macro", "6"), None, "P₂O₅ is 0 in the file");
        assert_eq!(get("macro", "9"), None, "K₂O is 0 in the file");
    }

    #[test]
    fn heavy_metals_are_never_proposed_because_the_file_mixes_their_units() {
        // "CODA-Ca-L" declares Cu 70, Pb 45, Ni 25, Cr 70 on a product whose
        // N, P and K are all zero — mg/kg, plainly. A foliar zinc fertiliser in
        // the same columns declares 20,1 as a percentage. Nothing in the file
        // separates the two, so C.i's metals are always entered by hand.
        let conn = db();
        for product in ["CODA-Ca-L", "BASFOLIAR ZNMN", "Lodos calizos"] {
            let lines = material_proposal(&conn, "es", &detail_code(&conn, product))
                .unwrap()
                .composition;
            assert!(
                lines.iter().all(|l| l.kind_code != "heavy_metal"),
                "'{product}' proposed a heavy metal"
            );
            // Copper and zinc live only in that block, so they cannot be
            // offered as micronutrients either.
            assert!(
                !lines.iter().any(|l| l.kind_code == "micro"
                    && (l.nutrient_code == "3" || l.nutrient_code == "6")),
                "'{product}' proposed copper or zinc from the metals block"
            );
        }
    }

    #[test]
    fn elemental_phosphorus_and_potassium_are_not_mapped_onto_the_oxide_codes() {
        // `P_% TOTAL` is elemental P (the file's median P2O5/P ratio is 2,2936,
        // the oxide factor), and MACRONUTRIENTES codes oxides only. Mapping it
        // would understate the P₂O₅ of every product by more than half.
        let conn = db();
        let lines = material_proposal(&conn, "es", &detail_code(&conn, "NITRATO AMÓNICO CÁLCICO"))
            .unwrap()
            .composition;
        assert!(lines.iter().all(|l| l.percentage <= 100.0));
        // Every proposed code exists in the catalogue it names.
        for line in &lines {
            let catalogue = nutrients(&conn, "es", &line.kind_code).unwrap();
            assert!(
                catalogue.iter().any(|c| c.code == line.nutrient_code),
                "{}/{} is not a published code",
                line.kind_code,
                line.nutrient_code
            );
        }
    }

    #[test]
    fn every_mapped_column_exists_in_the_vendored_file_and_names_a_real_code() {
        // A snapshot refresh that renames a column would otherwise make the
        // prefill silently stop offering that nutrient.
        let conn = db();
        let row = terrazgo_core::catalogue::find_code(
            &conn,
            "DETALLE_MATERIAL_FERT",
            &detail_code(&conn, "NITRATO AMÓNICO CÁLCICO"),
        )
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        for (header, kind_code, nutrient_code) in COMPOSITION_COLUMNS {
            assert!(
                attr(&row, header).is_some(),
                "DETALLE_MATERIAL_FERT no longer publishes '{header}'"
            );
            let catalogue = nutrients(&conn, "es", kind_code).unwrap();
            assert!(
                catalogue.iter().any(|c| &c.code == nutrient_code),
                "{kind_code}/{nutrient_code} ('{header}') is not a published code"
            );
        }
    }

    #[test]
    fn the_mapping_accounts_for_every_column_of_the_file() {
        // The decision record, checked against the header row terrazgo-core
        // pins rather than against sampled data. Core catches a column that
        // MOVED or was RENAMED; this catches the quieter case it cannot judge
        // — a column that arrived and which nobody has decided the meaning of.
        // Without it, a new nutrient is simply never offered by the fill.
        let pinned = terrazgo_core::catalogue::vendored_headers("DETALLE_MATERIAL_FERT")
            .expect("DETALLE_MATERIAL_FERT is vendored");
        let (code_header, label_header) =
            terrazgo_core::catalogue::vendored_key_headers("DETALLE_MATERIAL_FERT")
                .expect("DETALLE_MATERIAL_FERT is vendored");

        let mapped: std::collections::HashSet<&str> = COMPOSITION_COLUMNS
            .iter()
            .map(|(h, _, _)| *h)
            .chain([DENSITY_COLUMN, AGGREGATION_COLUMN])
            .collect();
        let declared: std::collections::HashSet<&str> =
            UNMAPPED_COLUMNS.iter().map(|(h, _)| *h).collect();

        let unexplained: Vec<&&str> = pinned
            .iter()
            .filter(|h| ***h != *code_header && ***h != *label_header)
            .filter(|h| !mapped.contains(**h) && !declared.contains(**h))
            .collect();
        assert!(
            unexplained.is_empty(),
            "DETALLE_MATERIAL_FERT has columns this mapping neither uses nor explains: \
             {unexplained:?}. Map them in COMPOSITION_COLUMNS, or say why not in \
             UNMAPPED_COLUMNS — silence would mean a nutrient the fill never offers."
        );

        // And the reverse, both ways: reasoning about a column that no longer
        // exists is stale, and a mapping that names one is broken.
        for header in mapped.iter().chain(declared.iter()) {
            assert!(
                pinned.contains(header),
                "'{header}' is named by this mapping but is no longer a column of the file"
            );
        }
    }

    #[test]
    fn an_unknown_product_proposes_nothing_rather_than_erroring() {
        let conn = db();
        for (country, code) in [("es", "999999"), ("fr", "1")] {
            let proposal = material_proposal(&conn, country, code).unwrap();
            assert!(proposal.composition.is_empty());
            assert_eq!(proposal.density_kg_l, None);
        }
    }

    // Density is a LIQUID product's: Anexo V 3.11, Fertilización field 17,
    // "Densidad del material fertilizante cuando se trate de producto
    // fertilizante líquido" (the CUE descriptor and the 2027 Anexo 03 say the
    // same). `DETALLE_MATERIAL_FERT` states it in g/cm³, which is kg/L.

    #[test]
    fn a_liquid_products_stated_density_is_proposed() {
        let conn = db();
        let code = detail_code(&conn, "SOLUCIÓN NITROGENADA (32-00-00)");
        let proposal = material_proposal(&conn, "es", &code).unwrap();
        assert_eq!(proposal.density_kg_l, Some(1.32));
        assert!(!proposal.composition.is_empty(), "and its nitrogen too");
    }

    #[test]
    fn a_solids_density_is_never_proposed() {
        // The file states 1,1 for this granulated NPK — a bulk density, which
        // is not what the field asks for.
        let conn = db();
        let code = detail_code(&conn, "Forterra NPK 8-15-15");
        let proposal = material_proposal(&conn, "es", &code).unwrap();
        assert_eq!(proposal.density_kg_l, None);
    }

    #[test]
    fn a_liquid_stating_zero_proposes_no_density() {
        // The provider fills unstated cells with 0, as in the composition
        // columns — and slurry is exactly the material whose density a farmer
        // must state, by volume dose, so a guess here would be worst.
        let conn = db();
        let code = detail_code(&conn, "Purín de cerdos engorde/cebo");
        let proposal = material_proposal(&conn, "es", &code).unwrap();
        assert_eq!(proposal.density_kg_l, None);
    }

    #[test]
    fn a_country_with_no_coded_lists_gets_nothing_rather_than_spains() {
        let conn = db();
        assert!(fertiliser_materials(&conn, "fr").unwrap().is_empty());
        assert!(fertilisation_practices(&conn, "fr").unwrap().is_empty());
    }
}
