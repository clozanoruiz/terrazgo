// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reference-catalogue reads the record book's coded fields need.
//!
//! Storage and import belong to `terrazgo_core::catalogue`; which catalogue a
//! field speaks belongs to [`crate::siex`]. What lives here is the little that
//! is neither: turning a catalogue into the list a picker offers, for the two
//! catalogues whose raw rows are not that list.

use rusqlite::Connection;
use serde::Serialize;
use terrazgo_core::catalogue::{CatalogueCode, held_picks};

use crate::error::Result;
use crate::siex;

/// One offer in a catalogue-backed picker: the code the record stores, and the
/// name the farmer reads. Core's, since core grew a catalogue-backed field of
/// its own (`premises.class_code`) and two identical structs would serialize
/// the same JSON while drifting independently.
pub use terrazgo_core::catalogue::CataloguePick;

/// The harvested produce a sale or a postharvest treatment can name (FEGA
/// `PROD_VEGETAL`), one entry per code.
///
/// The file carries one row per (produce, crop) pair — 692 rows behind 208
/// codes, because *Aceitunas* is published once for OLIVO and again for
/// ACEBUCHE. A picker must offer each produce once; which crops it can come off
/// is not a question the form asks.
pub fn plant_products(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    picks(conn, siex::plant_product_catalogue(country_code))
}

/// The non-chemical measures model 3.1 bis offers (FEGA
/// `TIPO_MEDIDA_FITOSANITARIA`, fourteen entries).
pub fn measures(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    picks(conn, siex::measure_catalogue(country_code))
}

/// The active substances an analysis can report (FEGA `SUST_ACTIVAS`).
pub fn substances(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    picks(conn, siex::substance_catalogue(country_code))
}

/// The growth stages a treated crop can be recorded at (FEGA
/// `EST_FENOLOGICO`, ten entries), named as [`growth_stage_label`] names them.
///
/// Not [`picks`], which would offer the provider's row number as if it were the
/// BBCH stage.
pub fn growth_stages(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = siex::growth_stage_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    Ok(held_picks(conn, catalogue_id, |_| true, stage_name)?)
}

/// A stored growth-stage code, resolved into the two renderings its readers
/// need. Neither of them is the stored code.
///
/// FEGA numbers `EST_FENOLOGICO`'s rows 1-10 in `Código SIEX` — which is what a
/// record stores, because the twin validates `EstadoFenologico` against the
/// catalogue — and publishes the monograph's own 0-9 beside them in `Estadio
/// bibliografía`. Printing the stored code as a BBCH stage would misstate the
/// monograph by one everywhere it appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthStage {
    /// The BBCH principal stage alone — "5". What a register CELL prints: the
    /// annex asks for the stage "in line with the BBCH monograph", and the
    /// monograph's identifier is this number, with the page's footnote saying
    /// so (the same division as the model's own SEC/ASP/LOC/GRA siglas).
    pub bbch: String,
    /// The stage with FEGA's own wording behind it — "5 · Emergencia de la
    /// inflorescencia (tallo principal)/ espigamiento". What a picker offers
    /// and what the spreadsheet stores, both having room for a sentence that a
    /// 15-column landscape register does not.
    pub label: String,
}

/// The catalogue a stored growth-stage code speaks, for a caller resolving many
/// of them against rows it has already read.
pub const GROWTH_STAGE_CATALOGUE: &str = "EST_FENOLOGICO";

/// Resolve a stored growth-stage code for display, from the catalogue row that
/// carries it.
///
/// The row rather than the connection, because the readers are registers: one
/// stage per treated plot, over and over, and a book must resolve
/// [`GROWTH_STAGE_CATALOGUE`] a bounded number of times rather than once per
/// row (`docs/architecture.md` → "The report engine").
///
/// An unresolvable code stands in for both renderings, the `problem_code` rule:
/// the vendored snapshot rides app releases, and a record written against a
/// later catalogue must stay readable rather than print blank.
pub fn growth_stage_from(row: Option<&CatalogueCode>, code: &str) -> GrowthStage {
    if code.is_empty() {
        return GrowthStage {
            bbch: String::new(),
            label: String::new(),
        };
    }
    match row {
        Some(row) => GrowthStage {
            bbch: stage_bbch(row).unwrap_or_else(|| code.to_string()),
            label: stage_name(row),
        },
        None => GrowthStage {
            bbch: code.to_string(),
            label: code.to_string(),
        },
    }
}

/// The monograph's principal stage as the catalogue publishes it, beside the
/// code of its own.
fn stage_bbch(row: &CatalogueCode) -> Option<String> {
    let bbch = row
        .attrs
        .as_ref()
        .and_then(|attrs| attrs.get("Estadio bibliografía"))
        .and_then(serde_json::Value::as_str)?
        .trim();
    (!bbch.is_empty()).then(|| bbch.to_string())
}

/// The BBCH stage the catalogue publishes beside a row, then its label. The
/// stage column is absent from no row of the vendored file, but a refresh that
/// dropped it must degrade to the label rather than to a stray separator.
fn stage_name(row: &CatalogueCode) -> String {
    match stage_bbch(row) {
        Some(bbch) => format!("{bbch} · {}", row.label),
        None => row.label.clone(),
    }
}

/// The catalogue's active rows as picker entries, first label per code wins.
/// A country with no coded list gets an empty list rather than Spain's — the
/// `list_problem_codes` rule: nothing to offer is not the same as offering the
/// wrong vocabulary.
/// The basic substances a measure "Usos de sustancias básicas" can name, and
/// which measure that is — so the form shows the picker on the right measure
/// without carrying the code itself.
#[derive(Debug, Clone, Serialize)]
pub struct BasicSubstances {
    /// The `TIPO_MEDIDA_FITOSANITARIA` code the substances belong to; `None`
    /// for a country with no coded list, where there is nothing to offer.
    pub measure_code: Option<&'static str>,
    pub substances: Vec<CataloguePick>,
}

/// FEGA `SUSTANCIAS_BASICAS`, in the provider's order, with the measure they
/// are named on.
pub fn basic_substances(conn: &Connection, country_code: &str) -> Result<BasicSubstances> {
    let catalogue = siex::basic_substance_catalogue(country_code);
    Ok(BasicSubstances {
        measure_code: catalogue.map(|_| siex::BASIC_SUBSTANCE_MEASURE),
        substances: picks(conn, catalogue)?,
    })
}

fn picks(conn: &Connection, catalogue_id: Option<&str>) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = catalogue_id else {
        return Ok(Vec::new());
    };
    Ok(held_picks(
        conn,
        catalogue_id,
        |_| true,
        |row| row.label.clone(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut conn = crate::open_in_memory().expect("in-memory database");
        terrazgo_core::catalogue::ensure_catalogues(&mut conn).expect("catalogue import");
        conn
    }

    /// What a reader with one code and no rows in hand does: read the row, then
    /// format it. The book resolves the rows in bulk instead.
    fn growth_stage(conn: &Connection, code: &str) -> GrowthStage {
        let rows = terrazgo_core::catalogue::find_code(conn, GROWTH_STAGE_CATALOGUE, code)
            .expect("catalogue read");
        growth_stage_from(rows.first(), code)
    }

    #[test]
    fn basic_substances_come_with_the_measure_they_are_named_on() {
        let conn = db();
        let offered = basic_substances(&conn, "es").unwrap();
        assert_eq!(offered.measure_code, Some("11"));
        // SUSTANCIAS_BASICAS as published 2026-09-29: 28 substances.
        assert_eq!(offered.substances.len(), 28);
        assert!(
            offered
                .substances
                .iter()
                .any(|s| s.code == "28" && s.name == "Vinagre")
        );
        // No coded list, no measure to hang one on.
        let elsewhere = basic_substances(&conn, "fr").unwrap();
        assert_eq!(elsewhere.measure_code, None);
        assert!(elsewhere.substances.is_empty());
    }

    #[test]
    fn a_retired_stage_or_measure_is_listed_and_not_offered() {
        // A picker must still name the code a record already carries when it
        // no longer offers it: retired by the authority since, or held only by
        // another device's newer catalogue (docs/sync.md → What stays
        // device-local).
        let conn = db();
        for catalogue in [GROWTH_STAGE_CATALOGUE, "TIPO_MEDIDA_FITOSANITARIA"] {
            conn.execute(
                "UPDATE catalogue_code SET retired_on = '2026-01-01'
                 WHERE id = (SELECT MIN(id) FROM catalogue_code WHERE catalogue_id = ?1)",
                [catalogue],
            )
            .unwrap();
        }
        for picks in [
            growth_stages(&conn, "es").unwrap(),
            measures(&conn, "es").unwrap(),
        ] {
            assert!(!picks[0].offered, "{} is retired", picks[0].code);
            assert!(picks[1..].iter().all(|pick| pick.offered));
        }
    }

    #[test]
    fn plant_products_offer_each_produce_once() {
        let conn = db();
        let picks = plant_products(&conn, "es").unwrap();
        // 208 codes behind the file's 692 rows (docs/maintenance.md §1).
        assert_eq!(picks.len(), 208);
        let olives: Vec<&CataloguePick> = picks.iter().filter(|p| p.name == "Aceitunas").collect();
        assert_eq!(
            olives.len(),
            1,
            "Aceitunas is published for OLIVO and for ACEBUCHE, and offered once"
        );
    }

    #[test]
    fn substances_come_from_the_catalogue_that_carries_cas_numbers() {
        let conn = db();
        let picks = substances(&conn, "es").unwrap();
        assert!(picks.iter().any(|p| p.name == "TEBUCONAZOL"));
    }

    #[test]
    fn a_country_with_no_coded_list_is_offered_nothing() {
        // Never Spain's list under another flag: a code means what its own
        // authority says it means.
        let conn = db();
        assert!(plant_products(&conn, "fr").unwrap().is_empty());
        assert!(substances(&conn, "it").unwrap().is_empty());
        assert!(growth_stages(&conn, "fr").unwrap().is_empty());
    }

    #[test]
    fn a_growth_stage_prints_the_bbch_number_not_the_catalogue_code() {
        // The whole point of the formatter. EST_FENOLOGICO numbers its rows
        // 1-10 and carries the BBCH monograph's own principal stage 0-9 in
        // "Estadio bibliografía", so the two ends of the list are off by one:
        // code 1 IS stage 0 (germinación) and code 10 IS stage 9 (senescencia).
        let conn = db();
        let first = growth_stage(&conn, "1");
        assert_eq!(first.bbch, "0", "the first row is BBCH 0, not BBCH 1");
        assert!(first.label.starts_with("0 · "));
        assert!(first.label.contains("Germinación"));

        let last = growth_stage(&conn, "10");
        assert_eq!(last.bbch, "9", "the last row is BBCH 9, not BBCH 10");
        assert!(last.label.starts_with("9 · "));
    }

    #[test]
    fn growth_stages_offer_the_ten_principal_stages() {
        let conn = db();
        let picks = growth_stages(&conn, "es").unwrap();
        // The BBCH monograph's ten principal stages (docs/maintenance.md §1).
        assert_eq!(picks.len(), 10);
        // Every offer names its stage the way the spreadsheet will, so the form
        // and the book cannot disagree about what was chosen.
        for pick in &picks {
            assert_eq!(pick.name, growth_stage(&conn, &pick.code).label);
        }
    }

    #[test]
    fn an_unresolvable_growth_stage_prints_itself() {
        // The vendored snapshot rides app releases; a record written against a
        // later catalogue must stay readable, never blank (the problem_code
        // rule). And nothing is printed for nothing stated.
        let conn = db();
        let unknown = growth_stage(&conn, "77");
        assert_eq!(unknown.bbch, "77");
        assert_eq!(unknown.label, "77");
        // And nothing is printed for nothing stated, in either rendering.
        let unstated = growth_stage(&conn, "");
        assert_eq!(unstated.bbch, "");
        assert_eq!(unstated.label, "");
    }
}
