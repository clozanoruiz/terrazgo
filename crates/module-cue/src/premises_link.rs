// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How the non-field register uses core's premises registry.
//!
//! Core owns the *thing* — a building or a vehicle on the holding. Which of
//! them a given register may name is this crate's rule, because only this crate
//! holds the register's own vocabulary (`non_field_subject_kind`), and core may
//! never reference a module's lookup.
//!
//! The pairing is not bookkeeping: model 3.4 prints locales and model 3.5
//! prints vehicles, so a record filed under one while naming the other would
//! print a lorry on the premises page. Anexo III Parte I B.b names them
//! separately ("local **o** medio de transporte tratado") for the same reason.

use crate::error::{CueError, Result};
use terrazgo_core::models::Premises;

/// Which `premises.kind_code` each register may name. `postharvest` treats
/// produce and not a place, so it may name none — the third arm is as
/// load-bearing as the other two.
pub fn required_premises_kind(subject_kind_code: &str) -> Option<&'static str> {
    match subject_kind_code {
        "storage_premises" => Some("building"),
        "transport" => Some("vehicle"),
        _ => None,
    }
}

/// Check a named premises against the register it is being filed under, and
/// against the farm the record belongs to.
pub fn validate_premises(
    premises: Option<&Premises>,
    subject_kind_code: &str,
    farm_id: &str,
) -> Result<()> {
    let wanted = required_premises_kind(subject_kind_code);
    match (premises, wanted) {
        (None, _) => Ok(()),
        // A postharvest record treats produce. Letting it name a warehouse
        // would put a place in a register whose subject is a plant product.
        (Some(_), None) => Err(CueError::Invalid("premises_on_produce_record")),
        (Some(p), Some(kind)) => {
            if p.farm_id != farm_id {
                return Err(CueError::Invalid("premises_not_on_farm"));
            }
            if p.kind_code != kind {
                return Err(CueError::Invalid("premises_kind_mismatch"));
            }
            Ok(())
        }
    }
}

/// Whether a premises may be corrected to `new_kind_code` while the registers
/// in `kinds_in_use` name it.
///
/// A mistyped kind is a typo like any other and must stay correctable, but
/// correcting it under a record filed in the other register would leave a
/// vehicle sitting in model 3.4 — the state [`validate_premises`] refuses at
/// write time, reached from the side core cannot see. The farmer's way out is
/// the same as for a wrong subject kind on a record: delete the record and
/// re-enter it, since moving one between registers empties one and fills
/// another.
pub fn validate_kind_change(kinds_in_use: &[String], new_kind_code: &str) -> Result<()> {
    let clashes = kinds_in_use
        .iter()
        .any(|kind| required_premises_kind(kind) != Some(new_kind_code));
    if clashes {
        return Err(CueError::Invalid("premises_kind_in_use"));
    }
    Ok(())
}

/// The printed subject cell, composed from the registry row.
///
/// The two models ask for different things — 3.4 for "local tratado (tipo y
/// dirección)", 3.5 for "vehículo tratado (tipo, modelo y matrícula)" — and the
/// premises' own name answers the "tipo" both of them want. Parts the farmer
/// left blank are skipped rather than printed as empty separators.
///
/// This is what lands in `subject_description`, which stays the printed truth:
/// composing it at write time and re-taking it only when `premises_id` changes
/// is the snapshot rule (docs/data-model.md → "Nothing is ever frozen").
pub fn describe_premises(premises: &Premises) -> String {
    let mut parts = vec![premises.name.trim()];
    if premises.kind_code == "vehicle" {
        parts.extend(premises.vehicle_model.as_deref());
        parts.extend(premises.plate.as_deref());
    } else {
        parts.extend(premises.address.as_deref());
    }
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn premises(kind: &str) -> Premises {
        Premises {
            id: "p1".into(),
            farm_id: "f1".into(),
            kind_code: kind.into(),
            name: "Almacén de la finca".into(),
            address: Some("Camino de la Vega, 1".into()),
            vehicle_model: Some("Iveco Daily".into()),
            plate: Some("1234 ABC".into()),
            class_code: Some("2".into()),
            volume_m3: None,
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    #[test]
    fn a_building_prints_its_name_and_address() {
        // Model 3.4: "local tratado (tipo y dirección)".
        assert_eq!(
            describe_premises(&premises("building")),
            "Almacén de la finca, Camino de la Vega, 1"
        );
    }

    #[test]
    fn a_vehicle_prints_its_name_model_and_plate_and_never_an_address() {
        // Model 3.5: "vehículo tratado (tipo, modelo y matrícula)".
        let mut lorry = premises("vehicle");
        lorry.name = "Camión frigorífico".into();
        assert_eq!(
            describe_premises(&lorry),
            "Camión frigorífico, Iveco Daily, 1234 ABC"
        );
    }

    /// The drift guard. The class is a catalogue label the user's own refresh
    /// can reword — folding it into the composed cell would silently restate
    /// records nobody touched. (The two Spanish registry identifiers cannot
    /// reach this function at all: they live in the extension row, which this
    /// layer never sees.)
    #[test]
    fn the_class_code_never_reaches_the_printed_cell() {
        let store = premises("building");
        let printed = describe_premises(&store);
        assert_eq!(printed, "Almacén de la finca, Camino de la Vega, 1");
        assert!(!printed.contains('2'), "the class code is not printed");

        let mut lorry = premises("vehicle");
        lorry.name = "Camión frigorífico".into();
        assert_eq!(
            describe_premises(&lorry),
            "Camión frigorífico, Iveco Daily, 1234 ABC"
        );
    }

    #[test]
    fn a_kind_may_be_corrected_while_no_record_or_only_matching_records_name_it() {
        // Nothing names it yet: any correction is a plain typo fix.
        assert!(validate_kind_change(&[], "vehicle").is_ok());
        // The records that name it agree with where it is going.
        assert!(validate_kind_change(&["storage_premises".into()], "building").is_ok());
        assert!(validate_kind_change(&["transport".into()], "vehicle").is_ok());
    }

    #[test]
    fn a_kind_change_is_refused_while_the_other_register_names_it() {
        // Model 3.4 holds a record for this store; turning it into a vehicle
        // would leave a lorry printed on the premises page.
        assert!(matches!(
            validate_kind_change(&["storage_premises".into()], "vehicle"),
            Err(CueError::Invalid("premises_kind_in_use"))
        ));
        assert!(matches!(
            validate_kind_change(&["transport".into()], "building"),
            Err(CueError::Invalid("premises_kind_in_use"))
        ));
    }

    /// A postharvest record can never name a premises ([`validate_premises`]
    /// refuses it), so if one somehow does, no kind satisfies it and the
    /// correction is refused rather than silently allowed.
    #[test]
    fn a_register_that_takes_no_premises_blocks_every_kind() {
        for kind in ["building", "vehicle"] {
            assert!(matches!(
                validate_kind_change(&["postharvest".into()], kind),
                Err(CueError::Invalid("premises_kind_in_use"))
            ));
        }
    }

    #[test]
    fn blank_parts_are_skipped_rather_than_printed_as_separators() {
        // Only the name is required, so a store with no address recorded must
        // not print "Almacén, ".
        let mut bare = premises("building");
        bare.address = None;
        assert_eq!(describe_premises(&bare), "Almacén de la finca");
    }

    #[test]
    fn each_register_accepts_only_the_kind_its_page_prints() {
        // A lorry filed under 3.4 would print on the premises page.
        assert!(validate_premises(Some(&premises("building")), "storage_premises", "f1").is_ok());
        assert!(validate_premises(Some(&premises("vehicle")), "transport", "f1").is_ok());
        assert!(matches!(
            validate_premises(Some(&premises("vehicle")), "storage_premises", "f1"),
            Err(CueError::Invalid("premises_kind_mismatch"))
        ));
        assert!(matches!(
            validate_premises(Some(&premises("building")), "transport", "f1"),
            Err(CueError::Invalid("premises_kind_mismatch"))
        ));
    }

    #[test]
    fn a_postharvest_record_treats_produce_and_names_no_place() {
        assert!(matches!(
            validate_premises(Some(&premises("building")), "postharvest", "f1"),
            Err(CueError::Invalid("premises_on_produce_record"))
        ));
        // And it is perfectly normal for it to name none.
        assert!(validate_premises(None, "postharvest", "f1").is_ok());
    }

    #[test]
    fn a_premises_of_another_holding_is_refused() {
        assert!(matches!(
            validate_premises(
                Some(&premises("building")),
                "storage_premises",
                "other-farm"
            ),
            Err(CueError::Invalid("premises_not_on_farm"))
        ));
    }

    #[test]
    fn naming_no_premises_stays_lawful_on_every_register() {
        // The registry is nullable on purpose: refusing a record because the
        // farmer has not yet created a registry row would be the register
        // blocking the duty it exists to serve (the efficacy precedent). The
        // EXPORT precheck is where the format's requirement belongs.
        for kind in ["storage_premises", "transport", "postharvest"] {
            assert!(validate_premises(None, kind, "f1").is_ok());
        }
    }
}
