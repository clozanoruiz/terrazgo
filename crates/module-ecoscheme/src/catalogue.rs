// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reference-catalogue reads section 9's coded fields need.
//!
//! Storage and import belong to `terrazgo_core::catalogue`; which catalogue a
//! field speaks belongs to [`crate::siex`]. What lives here is the little that
//! is neither — turning a catalogue into the list a picker offers.

use rusqlite::Connection;
use serde::Serialize;
use terrazgo_core::catalogue::active_codes;

use crate::error::Result;
use crate::siex;

/// One offer in a catalogue-backed picker: the code the record stores, and the
/// name the farmer reads. Deliberately the same shape the other modules'
/// pickers use, so one Svelte component serves them all.
#[derive(Debug, Clone, Serialize)]
pub struct CataloguePick {
    pub code: String,
    pub name: String,
}

/// The animals a grazing can name (FEGA `ESPECIE_ANIMAL`, 198 species from
/// bovinos to camellos). Model 9.1's "Especie animal que pasta";
/// `Pastoreo.Animales[].Especie`.
///
/// Every row is offered: the file carries no lifecycle columns at all, so
/// there is nothing to retire — `active_codes` returns all of it. Well past
/// the 40-row threshold at which an owned dropdown becomes a combobox, which
/// is the form's problem and not this function's.
pub fn animal_species(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = siex::animal_species_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    Ok(active_codes(conn, catalogue_id)?
        .into_iter()
        .map(|row| CataloguePick {
            code: row.code,
            name: row.label,
        })
        .collect())
}

/// Where a cultural operation's plant residue went (FEGA `DEST_RES_VEG`, nine
/// destinations from incorporation into the soil to controlled burning).
///
/// Model 9.2 does not print this column — it is the twin's
/// (`LaboresCulturales`) and art. 43's evidence chain: the destination
/// [`siex::RESIDUE_LEFT_ON_GROUND`] is what turns a pruning into a P7 inert
/// cover, so the field is captured even though no page shows it.
pub fn residue_destinations(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = siex::residue_destination_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    Ok(active_codes(conn, catalogue_id)?
        .into_iter()
        .map(|row| CataloguePick {
            code: row.code,
            name: row.label,
        })
        .collect())
}

/// What a cover is made of (FEGA `TIPO_COBERTURA_SUELO`, six kinds).
///
/// `DatosCubierta.TipoCobertura`, and stored for it: neither model 9.4 nor 9.5
/// prints a column for the kind, because arts. 42.1.a and 43.1.a annotate the
/// *date* a cover was established rather than which of the two it was.
///
/// The list is narrowed to what the practice can plausibly be — art. 42.1.a's
/// *"espontánea o sembrada"* for a plant cover, art. 43.1.a's *"restos de
/// poda"* for an inert one — because a picker offering all six would let a
/// farmer describe a P7 cover as bare soil.
///
/// **Narrowing here rather than refusing at write time** is the two-tier rule:
/// this catalogue grows between releases and a user's own refresh can carry a
/// code this build has never seen, so a picker may offer less than the record
/// accepts, never the reverse. A practice this function does not recognise gets
/// the whole list, so a caller can never be handed nothing by surprise.
pub fn cover_types(
    conn: &Connection,
    country_code: &str,
    practice_code: &str,
) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = siex::cover_type_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    let wanted = match practice_code {
        "plant_cover" => Some(siex::PLANT_COVER_TYPES),
        "inert_cover" => Some(siex::INERT_COVER_TYPES),
        _ => None,
    };
    Ok(active_codes(conn, catalogue_id)?
        .into_iter()
        .filter(|row| wanted.is_none_or(|codes| codes.contains(&row.code.as_str())))
        .map(|row| CataloguePick {
            code: row.code,
            name: row.label,
        })
        .collect())
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
    fn every_species_is_offered_because_the_file_retires_none() {
        let conn = db();
        let picks = animal_species(&conn, "es").unwrap();
        // 198 rows, and the file has no Fecha de baja column at all — so a
        // number that drops here means the snapshot lost rows, not that FEGA
        // retired a species (docs/maintenance.md §1).
        assert_eq!(picks.len(), 198);
        assert!(
            picks.iter().any(|p| p.code == "03" && p.name == "Ovinos"),
            "sheep are the species a Spanish extensive grazing is most likely to name"
        );
    }

    #[test]
    fn a_country_with_no_coded_list_gets_nothing_rather_than_spains() {
        let conn = db();
        assert!(animal_species(&conn, "fr").unwrap().is_empty());
        assert!(residue_destinations(&conn, "fr").unwrap().is_empty());
        assert!(cover_types(&conn, "fr", "plant_cover").unwrap().is_empty());
    }

    #[test]
    fn each_cover_practice_is_offered_only_what_its_article_names() {
        let conn = db();

        // RD 1048/2022 art. 42.1.a establishes "la cubierta vegetal espontánea
        // o sembrada": TIPO_COBERTURA_SUELO 3 and 2.
        let plant = cover_types(&conn, "es", "plant_cover").unwrap();
        let codes: Vec<&str> = plant.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(codes, siex::PLANT_COVER_TYPES);
        assert!(plant.iter().any(|p| p.name.contains("sembrada")));

        // Art. 43.1.a establishes "la cubierta inerte de restos de poda" —
        // code 4, and specifically NOT 5, which is other materials (nutshells,
        // stones).
        let inert = cover_types(&conn, "es", "inert_cover").unwrap();
        let codes: Vec<&str> = inert.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(codes, siex::INERT_COVER_TYPES);
        assert!(
            inert[0]
                .name
                .starts_with("Cubierta inerte de restos de poda")
        );

        // A practice this build does not know gets everything rather than
        // nothing: the six of TIPO_COBERTURA_SUELO, none of them retired.
        assert_eq!(cover_types(&conn, "es", "whatever").unwrap().len(), 6);
    }

    #[test]
    fn the_residue_destination_that_creates_an_inert_cover_is_offered() {
        let conn = db();
        let picks = residue_destinations(&conn, "es").unwrap();
        // DEST_RES_VEG, nine destinations, none of them retired.
        assert_eq!(picks.len(), 9);
        // The one with meaning beyond display: RD 1048/2022 art. 43's inert
        // cover is triturated pruning residue left on the ground, so this code
        // is the evidence that a cover came into being.
        let left_on_ground = picks
            .iter()
            .find(|p| p.code == siex::RESIDUE_LEFT_ON_GROUND)
            .expect("the trituración destination is what art. 43 rests on");
        assert!(
            left_on_ground
                .name
                .starts_with("Trituración de restos de poda"),
            "unexpected label for the P7 destination: {}",
            left_on_ground.name
        );
    }
}
