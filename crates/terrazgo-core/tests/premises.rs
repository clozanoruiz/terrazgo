// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The premises registry: the places and vehicles models 3.4 and 3.5 treat.
//!
//! The rule under test throughout is why the registry exists at all — RD
//! 1311/2012 Anexo III Parte I B.b requires the "local o medio de transporte
//! tratado" to be IDENTIFIED, and an identity has to survive being referenced
//! by several records and being corrected.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo_core::models::*;
use terrazgo_core::open_in_memory;
use terrazgo_core::repository as repo;

fn farm(conn: &mut Connection, name: &str) -> String {
    repo::insert_farm(
        conn,
        NewFarm {
            name: name.into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id
}

fn building(farm_id: &str) -> NewPremises {
    NewPremises {
        farm_id: farm_id.into(),
        kind_code: "building".into(),
        name: "Almacén de la finca".into(),
        address: Some("Camino de la Vega, 1".into()),
        vehicle_model: None,
        plate: None,
        // EDIFICACIONES_INSTALACIONES 2 = "Almacén de maquinaria".
        class_code: Some("2".into()),
        volume_m3: Some(420.0),
        notes: None,
        // Both Spanish registry fields ride flat and land in the extension.
        cadastral_reference: Some("1234567AB1234C0001XY".into()),
        rea_installation_code: Some("4700123456".into()),
    }
}

fn vehicle(farm_id: &str) -> NewPremises {
    NewPremises {
        farm_id: farm_id.into(),
        kind_code: "vehicle".into(),
        name: "Camión frigorífico".into(),
        address: None,
        vehicle_model: Some("Iveco Daily".into()),
        plate: Some("1234 ABC".into()),
        class_code: None,
        volume_m3: Some(18.0),
        notes: None,
        cadastral_reference: None,
        rea_installation_code: None,
    }
}

/// A full-row correction that keeps everything but the two registry fields,
/// which the caller supplies — the form is the source of truth for both.
fn update_of(
    premises: &Premises,
    cadastral_reference: Option<&str>,
    rea_installation_code: Option<&str>,
) -> UpdatePremises {
    UpdatePremises {
        kind_code: premises.kind_code.clone(),
        name: premises.name.clone(),
        address: premises.address.clone(),
        vehicle_model: premises.vehicle_model.clone(),
        plate: premises.plate.clone(),
        class_code: premises.class_code.clone(),
        volume_m3: premises.volume_m3,
        notes: premises.notes.clone(),
        cadastral_reference: cadastral_reference.map(str::to_string),
        rea_installation_code: rea_installation_code.map(str::to_string),
    }
}

#[test]
fn a_building_and_a_vehicle_are_stored_with_the_fields_their_page_prints() {
    // Model 3.4 asks for "tipo y dirección", model 3.5 for "tipo, modelo y
    // matrícula" — so the two kinds carry different columns, and the name
    // answers the "tipo" both of them want.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let store = repo::insert_premises(&mut conn, building(&farm_id), Some("tester"))
        .unwrap()
        .premises;
    assert_eq!(store.kind_code, "building");
    assert_eq!(store.address.as_deref(), Some("Camino de la Vega, 1"));
    assert_eq!(store.plate, None);

    let lorry = repo::insert_premises(&mut conn, vehicle(&farm_id), None)
        .unwrap()
        .premises;
    assert_eq!(lorry.vehicle_model.as_deref(), Some("Iveco Daily"));
    assert_eq!(lorry.plate.as_deref(), Some("1234 ABC"));

    let listed = repo::list_premises(&conn, &farm_id).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn a_premises_needs_a_name_because_the_name_is_the_identification() {
    // B.b asks for the local to be identified; an unnamed row identifies
    // nothing, which is the one thing this table exists to prevent.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut blank = building(&farm_id);
    blank.name = "   ".into();
    assert!(matches!(
        repo::insert_premises(&mut conn, blank, None).unwrap_err(),
        terrazgo_core::CoreError::Invalid("empty_name")
    ));
}

#[test]
fn an_unknown_kind_is_refused_by_the_lookup() {
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut wrong = building(&farm_id);
    wrong.kind_code = "warehouse".into();
    assert!(repo::insert_premises(&mut conn, wrong, None).is_err());
}

#[test]
fn a_stated_volume_has_to_be_a_real_capacity() {
    // A zero or negative volume is a typo, not a building. Nullable stays
    // fine: the capacity is not asked for by any decree.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    for bad in [0.0, -5.0] {
        let mut wrong = building(&farm_id);
        wrong.volume_m3 = Some(bad);
        assert!(
            matches!(
                repo::insert_premises(&mut conn, wrong, None).unwrap_err(),
                terrazgo_core::CoreError::Invalid("nonpositive_volume")
            ),
            "{bad} m³ must be refused"
        );
    }

    let mut none = building(&farm_id);
    none.volume_m3 = None;
    assert!(repo::insert_premises(&mut conn, none, None).is_ok());
}

#[test]
fn correcting_a_premises_keeps_its_identity_and_is_audited() {
    // The whole point of the registry: the same warehouse stays the same row
    // when its address is fixed, so every record that named it still names it.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");
    let store = repo::insert_premises(&mut conn, building(&farm_id), None).unwrap();

    let corrected = repo::update_premises(
        &mut conn,
        &store.premises.id,
        UpdatePremises {
            kind_code: "building".into(),
            name: "Almacén de la finca".into(),
            address: Some("Camino de la Vega, 3".into()),
            vehicle_model: None,
            plate: None,
            class_code: Some("2".into()),
            volume_m3: Some(420.0),
            notes: None,
            cadastral_reference: Some("1234567AB1234C0001XY".into()),
            rea_installation_code: Some("4700123456".into()),
        },
        Some("tester"),
    )
    .unwrap();
    assert_eq!(
        corrected.premises.id, store.premises.id,
        "a correction is not a new building"
    );
    assert_eq!(
        corrected.premises.address.as_deref(),
        Some("Camino de la Vega, 3")
    );

    let changes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM record_change WHERE entity_table = 'premises'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(changes, 2, "the insert and the correction are both logged");
}

#[test]
fn a_deleted_premises_leaves_the_registry_but_keeps_its_row() {
    // Soft delete, like every other registry entity: records that named it must
    // keep resolving, and its audit history has to survive.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");
    let store = repo::insert_premises(&mut conn, building(&farm_id), None).unwrap();

    repo::soft_delete_premises(&mut conn, &store.premises.id, None).unwrap();
    assert!(repo::list_premises(&conn, &farm_id).unwrap().is_empty());

    let still_there: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM premises WHERE id = ?1",
            [&store.premises.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(still_there, 1);
}

#[test]
fn a_building_carries_the_two_things_the_spanish_registries_say_about_it() {
    // Anexo V's CUE block 1.3 sits under a subloque named "Instalación
    // identificada en el REA" and gives it one identifying field, Obligatorio:
    // the referencia catastral of the building or of the plot it stands on.
    // Beside it, `Edificaciones[].IdEdificacion` wants REA's own code for the
    // installation — the same field REA's own structure calls "Código del
    // edificio/instalación en el REA", and never ours to mint. Both are
    // Spanish-registry data, so both live in the extension while the FEGA class
    // code stays in core (a catalogue code, the `crop.crop_code` precedent).
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let stored = repo::insert_premises(&mut conn, building(&farm_id), None).unwrap();
    assert_eq!(stored.premises.class_code.as_deref(), Some("2"));
    let es = stored
        .es
        .expect("an extension row when either field is present");
    assert_eq!(
        es.cadastral_reference.as_deref(),
        Some("1234567AB1234C0001XY")
    );
    assert_eq!(es.rea_installation_code.as_deref(), Some("4700123456"));

    // And both survive a re-read, which is what the export resolves against.
    let listed = repo::list_premises_details(&conn, &farm_id).unwrap();
    let es = listed[0].es.as_ref().expect("extension");
    assert_eq!(
        es.cadastral_reference.as_deref(),
        Some("1234567AB1234C0001XY")
    );
    assert_eq!(es.rea_installation_code.as_deref(), Some("4700123456"));
}

#[test]
fn a_cadastral_reference_is_stored_upper_cased_so_one_building_has_one_spelling() {
    // Anexo V types it string(20) over the pattern NNNNNNNNNNNNNNNNNNNN, and a
    // reference is a canonical upper-case code: two spellings of one reference
    // would defeat the identification B.b asks the registry to give.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut typed = building(&farm_id);
    typed.cadastral_reference = Some("  1234567ab1234c0001xy  ".into());
    let stored = repo::insert_premises(&mut conn, typed, None).unwrap();
    assert_eq!(
        stored.es.unwrap().cadastral_reference.as_deref(),
        Some("1234567AB1234C0001XY")
    );
}

#[test]
fn no_registry_identifier_is_demanded_nor_pattern_checked_here() {
    // The registry never blocks the duty it serves: a farmer who does not know
    // the reference must still be able to identify the store by name and
    // address, and the EXPORT precheck is where the format's requirement
    // belongs (the efficacy precedent). The shape is not checked either — the
    // precedent for an external registry identifier is roma_number / rea_code.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut bare = building(&farm_id);
    bare.cadastral_reference = None;
    bare.rea_installation_code = None;
    bare.class_code = None;
    let stored = repo::insert_premises(&mut conn, bare, None).unwrap();
    assert_eq!(stored.premises.class_code, None);
    assert!(
        stored.es.is_none(),
        "no extension row when the holding has nothing registry-side to say"
    );

    let mut odd = building(&farm_id);
    odd.cadastral_reference = Some("not a reference".into());
    odd.rea_installation_code = Some("not a number".into());
    assert!(repo::insert_premises(&mut conn, odd, None).is_ok());
}

#[test]
fn the_extension_row_is_reconciled_from_the_submitted_state() {
    // The farm/plot/machinery contract: the form is the source of truth, so
    // clearing both identifiers hard-deletes the row (logged with a null
    // after-image) and re-entering one brings it back.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");
    let stored = repo::insert_premises(&mut conn, building(&farm_id), None).unwrap();
    let id = stored.premises.id.clone();

    let cleared = repo::update_premises(
        &mut conn,
        &id,
        update_of(&stored.premises, None, None),
        None,
    )
    .unwrap();
    assert!(cleared.es.is_none(), "both cleared → the row is gone");
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM premises_es_extension WHERE premises_id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);

    let restored = repo::update_premises(
        &mut conn,
        &id,
        update_of(&stored.premises, None, Some("4700999999")),
        None,
    )
    .unwrap();
    let es = restored
        .es
        .expect("re-entering one identifier restores the row");
    assert_eq!(es.cadastral_reference, None);
    assert_eq!(es.rea_installation_code.as_deref(), Some("4700999999"));
}

#[test]
fn a_class_code_this_release_does_not_know_is_stored_anyway() {
    // The two-tier rule's second tier: EDIFICACIONES_INSTALACIONES is a
    // published list of 109 rows that the user's own catalogue refresh can
    // grow, so it is narrowed by the PICKER and never by the repository —
    // refusing an unknown code would make a lawful premises unrecordable
    // between releases (the TIPO_COBERTURA_SUELO rule).
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut future = building(&farm_id);
    future.class_code = Some("9999".into());
    let stored = repo::insert_premises(&mut conn, future, None).unwrap();
    assert_eq!(stored.premises.class_code.as_deref(), Some("9999"));
}

#[test]
fn blank_fields_are_stored_as_absent_rather_than_as_empty_strings() {
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");

    let mut blanks = building(&farm_id);
    blanks.cadastral_reference = Some("   ".into());
    blanks.rea_installation_code = Some("".into());
    blanks.class_code = Some("".into());
    let stored = repo::insert_premises(&mut conn, blanks, None).unwrap();
    assert_eq!(stored.premises.class_code, None);
    assert!(stored.es.is_none(), "blank is absent, not an empty row");
}

#[test]
fn the_audit_log_carries_the_core_row_and_the_extension_separately() {
    // The log is the Stage-2/3 sync delta source: a receiving device rebuilds
    // each row from `after` alone, so a field missing from a payload is a bug —
    // and the extension is its own synced entity, keyed on the premises id.
    let mut conn = open_in_memory().unwrap();
    let farm_id = farm(&mut conn, "Finca La Vega");
    let stored = repo::insert_premises(&mut conn, building(&farm_id), None).unwrap();
    let id = stored.premises.id.clone();

    let image = |table: &str| -> serde_json::Value {
        let payload: String = conn
            .query_row(
                "SELECT payload FROM record_change
                 WHERE entity_table = ?1 AND entity_id = ?2",
                rusqlite::params![table, &id],
                |r| r.get(0),
            )
            .unwrap();
        serde_json::from_str(&payload).unwrap()
    };
    assert_eq!(image("premises")["after"]["class_code"], "2");
    let ext = image("premises_es_extension");
    assert_eq!(ext["after"]["cadastral_reference"], "1234567AB1234C0001XY");
    assert_eq!(ext["after"]["rea_installation_code"], "4700123456");
}

#[test]
fn premises_are_listed_per_farm_and_never_across_holdings() {
    let mut conn = open_in_memory().unwrap();
    let mine = farm(&mut conn, "Finca La Vega");
    let neighbour = farm(&mut conn, "Finca del Vecino");
    repo::insert_premises(&mut conn, building(&mine), None).unwrap();
    repo::insert_premises(&mut conn, building(&neighbour), None).unwrap();

    assert_eq!(repo::list_premises(&conn, &mine).unwrap().len(), 1);
    assert_eq!(repo::list_premises(&conn, &neighbour).unwrap().len(), 1);
}
