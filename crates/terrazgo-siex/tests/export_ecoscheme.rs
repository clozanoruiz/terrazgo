// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `module-ecoscheme`'s three blocks — `Pastoreo`, `LaboresCulturales` and
//! `DatosCubierta` — and the rule they all hit: their junctions carry a plot
//! and no crop, while a SIEX DGC is a plot+crop unit, so the DGC is resolved
//! from the plot and the precheck names a plot it cannot resolve rather than
//! choosing for the farmer.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::repository as repo;
use module_fertilisation::models::NewFertilisationRecord;
use module_fertilisation::repository as fert;
use terrazgo_siex::{SiexError, build_cuaderno, export_precheck};

// ---------------------------------------------------------------------------
// Seam 4 — module-ecoscheme's three blocks, and the DGC plot→crop rule
// ---------------------------------------------------------------------------

use module_ecoscheme::models::{
    GrazingAnimal, NewCulturalOperation, NewGrazingRecord, NewSoilCover,
};
use module_ecoscheme::repository as eco;

fn grazing(fx: &Fixture) -> NewGrazingRecord {
    NewGrazingRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "extensive_grazing".into(),
        plot_group_ref: Some("Grupo norte".into()),
        soil_cover_id: None,
        started_on: "2026-05-12".into(),
        ended_on: Some("2026-09-30".into()),
        notes: None,
        plot_ids: vec![fx.wheat_plot_id.clone()],
        animals: vec![GrazingAnimal {
            id: String::new(),
            grazing_record_id: String::new(),
            // ESPECIE_ANIMAL 2 — ovino.
            species_code: "2".into(),
            rega_code: "ES471820000001".into(),
            animal_count: 120,
        }],
    }
}

fn operation(fx: &Fixture) -> NewCulturalOperation {
    NewCulturalOperation {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "sustainable_mowing".into(),
        operation_kind_code: "mowing".into(),
        performed_on: "2026-06-05".into(),
        performed_end_date: Some("2026-06-07".into()),
        activity_description: None,
        // DEST_RES_VEG 1 — incorporación al suelo o distribución en parcela.
        residue_destination_code: Some("1".into()),
        soil_cover_id: None,
        notes: None,
        plot_ids: vec![fx.wheat_plot_id.clone()],
    }
}

fn cover(fx: &Fixture) -> NewSoilCover {
    NewSoilCover {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        practice_code: "plant_cover".into(),
        // TIPO_COBERTURA_SUELO 3 — cubierta vegetal espontánea.
        cover_type_code: "3".into(),
        established_on: "2026-10-15".into(),
        width_m: Some(1.2),
        free_canopy_width_m: Some(2.5),
        widths_stated_on: Some("2027-01-20".into()),
        notes: None,
        plot_ids: vec![fx.wheat_plot_id.clone()],
        maintenance: vec![],
    }
}

#[test]
fn the_three_ecoscheme_registers_export_their_blocks() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    eco::insert_cultural_operation(&mut conn, operation(&fx), None).unwrap();
    eco::insert_soil_cover(&mut conn, cover(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);

    let pastoreo = &block(&doc, "Pastoreo")[0];
    assert_eq!(pastoreo["FechaInicio"], "12/05/2026");
    assert_eq!(pastoreo["FechaFin"], "30/09/2026");
    assert_eq!(pastoreo["Animales"][0]["REGA"], "ES471820000001");
    assert_eq!(pastoreo["Animales"][0]["Numero"], 120);
    assert_eq!(pastoreo["Animales"][0]["Especie"], 2);

    let labor = &block(&doc, "LaboresCulturales")[0];
    assert_eq!(labor["FechaInicio"], "05/06/2026");
    assert_eq!(labor["FechaFin"], "07/06/2026");
    // TIPO_LABOR 5, "Desbroce y siega" — the code `mowing` maps to.
    assert_eq!(labor["TipoLabor"], 5);

    let cubierta = &block(&doc, "DatosCubierta")[0];
    assert_eq!(cubierta["FecEstablecimientoCub"], "15/10/2026");
    assert_eq!(cubierta["AnchuraCubierta"], 1.2);
    assert_eq!(cubierta["AnchuraLibreProy"], 2.5);
    assert_eq!(cubierta["TipoCobertura"], 3);
}

#[test]
fn an_ecoscheme_dgc_resolves_its_crop_from_the_plot_and_the_season() {
    // Anexo V calls the crop of `Pastoreo` and `LaboresCulturales` a "campo
    // calculado", which is exactly what this is: the three eco-scheme junctions
    // carry a plot and no crop, because no printed page of section 9 asks which
    // crop was on it, while a SIEX DGC is a plot+crop unit.
    let mut conn = db();
    let fx = fixture(&mut conn);
    // Give the crop a PRODUCTOS code so `CodigoCultivo` has something to carry.
    conn.execute(
        "UPDATE crop SET crop_code = '21' WHERE id = ?1",
        [&fx.wheat_crop_id],
    )
    .unwrap();
    eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    eco::insert_cultural_operation(&mut conn, operation(&fx), None).unwrap();
    eco::insert_soil_cover(&mut conn, cover(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);

    // The SAME crop alias in all three blocks, and the same one `TratamFito`
    // would mint: a DGC alias is per crop row, not per register.
    let alias = block(&doc, "Pastoreo")[0]["DGCs"][0]["CodigoDGCAjena"].clone();
    assert!(alias.is_i64());
    assert_eq!(block(&doc, "Pastoreo")[0]["DGCs"][0]["CodigoCultivo"], 21);
    assert_eq!(
        block(&doc, "LaboresCulturales")[0]["DGCs"][0]["CodigoDGCAjena"],
        alias
    );
    assert_eq!(
        block(&doc, "DatosCubierta")[0]["DGCs"][0]["CodigoDGCAjena"],
        alias
    );
    assert_eq!(
        block(&doc, "DatosCubierta")[0]["DGCs"][0]["CodigoCultivo"],
        21
    );
}

#[test]
fn an_ecoscheme_dgc_never_states_a_surface() {
    // Neither junction has a surface column, and the descriptor reads an absent
    // `Superficie` as the DGC's own. Sending the crop's area instead would
    // assert that every hectare of it was grazed or worked.
    let mut conn = db();
    let fx = fixture(&mut conn);
    eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    eco::insert_cultural_operation(&mut conn, operation(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert!(
        block(&doc, "Pastoreo")[0]["DGCs"][0]
            .get("Superficie")
            .is_none()
    );
    assert!(
        block(&doc, "LaboresCulturales")[0]["DGCs"][0]
            .get("Superficie")
            .is_none()
    );
}

#[test]
fn precheck_refuses_an_ecoscheme_plot_carrying_two_crops() {
    // The record names a plot and nothing more. With two live crops on it the
    // plot IS two DGCs, and picking one would assert the activity happened on a
    // crop the farmer never named — so the rule refuses rather than guessing.
    let mut conn = db();
    let fx = fixture(&mut conn);
    insert_crop(&mut conn, &fx.wheat_plot_id, &fx.season_id, "vetch", None);
    eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.ecoscheme_plots_with_ambiguous_crop.len(), 1);
    let found = &check.ecoscheme_plots_with_ambiguous_crop[0];
    assert_eq!(found.register_code, "grazing");
    assert_eq!(found.plot_name, "El Prado");
    assert!(check.ecoscheme_plots_missing_crop.is_empty());
}

#[test]
fn precheck_refuses_an_ecoscheme_plot_with_no_crop() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bare_plot = insert_plot(&mut conn, &fx.farm_id, "El Erial", 2.0);
    let mut operation = operation(&fx);
    operation.plot_ids = vec![bare_plot];
    eco::insert_cultural_operation(&mut conn, operation, None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.ecoscheme_plots_missing_crop.len(), 1);
    assert_eq!(
        check.ecoscheme_plots_missing_crop[0].register_code,
        "cultural_operation"
    );
    assert_eq!(check.ecoscheme_plots_missing_crop[0].plot_name, "El Erial");
}

#[test]
fn precheck_names_a_grazing_that_has_not_ended() {
    // RD 1048/2022 art. 30.2 ter gives the farmer a month from "la nueva fecha
    // de inicio o fin", so a grazing still under way is not late — it is
    // unfinished, and `FechaFin` is required by the format. The export refuses
    // and names it rather than inventing an end or dropping the record.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut open = grazing(&fx);
    open.ended_on = None;
    eco::insert_grazing_record(&mut conn, open, None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.grazings_without_end.len(), 1);
    assert_eq!(check.grazings_without_end[0].started_on, "2026-05-12");
    assert!(matches!(
        build_cuaderno(&mut conn, &fx.season_id, &fx.farm_id, None),
        Err(SiexError::Invalid("export_precheck_failed"))
    ));
}

#[test]
fn precheck_demands_the_holding_rega_once_the_season_holds_a_grazing() {
    // `AnimalesPropios` / `AnimalesTerceros` are derived by comparing each
    // line's REGA with the holding's own, so without it every animal would be
    // reported as a third party's — a claim, not an absence.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let clean = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(clean.is_clean());

    conn.execute(
        "UPDATE farm_es_extension SET rega_code = NULL WHERE farm_id = ?1",
        [&fx.farm_id],
    )
    .unwrap();
    // Still clean while nothing grazes: the field is demanded by the register,
    // not by the farm.
    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.is_clean());

    eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(check.farm_missing_fields, vec!["rega_code"]);
}

#[test]
fn animal_ownership_is_derived_from_each_line_rega() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut own_only = grazing(&fx);
    own_only.started_on = "2026-05-01".into();
    eco::insert_grazing_record(&mut conn, own_only, None).unwrap();

    let mut mixed = grazing(&fx);
    mixed.started_on = "2026-06-01".into();
    mixed.animals.push(GrazingAnimal {
        id: String::new(),
        grazing_record_id: String::new(),
        species_code: "3".into(),
        // A neighbour's holding: same shape, different registry code.
        rega_code: "ES471820000999".into(),
        animal_count: 40,
    });
    eco::insert_grazing_record(&mut conn, mixed, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "Pastoreo");
    assert_eq!(entries[0]["AnimalesPropios"], true);
    assert_eq!(entries[0]["AnimalesTerceros"], false);
    assert_eq!(entries[1]["AnimalesPropios"], true);
    assert_eq!(entries[1]["AnimalesTerceros"], true);
}

#[test]
fn a_grazing_of_only_third_party_animals_says_so() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut visiting = grazing(&fx);
    visiting.animals[0].rega_code = "ES471820000999".into();
    eco::insert_grazing_record(&mut conn, visiting, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    // The descriptor forbids both being false; the register's "at least one
    // animal line" rule is what guarantees it.
    assert_eq!(block(&doc, "Pastoreo")[0]["AnimalesPropios"], false);
    assert_eq!(block(&doc, "Pastoreo")[0]["AnimalesTerceros"], true);
}

#[test]
fn precheck_refuses_an_animal_species_the_format_cannot_carry() {
    // `species_code` is a provider catalogue stored verbatim and deliberately
    // unvalidated at insert, while `Especie` is a required integer here.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let record = eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    conn.execute(
        "UPDATE grazing_animal SET species_code = 'ovino' WHERE grazing_record_id = ?1",
        [&record.record.id],
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.grazings_with_unsendable_species.len(), 1);
}

#[test]
fn precheck_demands_the_cover_widths() {
    // Art. 42.1.e falls due "en el mes anterior al final del periodo mínimo de
    // cuatro meses", so the register keeps the widths nullable and the book
    // prints such a cover. The DESCRIPTOR does not: Anexo V grades both widths
    // Obligatorio for exactly the three cover types this register can hold, and
    // that grading is what decides here.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut unmeasured = cover(&fx);
    unmeasured.width_m = None;
    unmeasured.free_canopy_width_m = None;
    unmeasured.widths_stated_on = None;
    eco::insert_soil_cover(&mut conn, unmeasured, None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.covers_missing_fields.len(), 1);
    assert_eq!(check.covers_missing_fields[0].established_on, "2026-10-15");
}

#[test]
fn precheck_refuses_a_cover_type_the_format_cannot_carry() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let record = eco::insert_soil_cover(&mut conn, cover(&fx), None).unwrap();
    conn.execute(
        "UPDATE soil_cover SET cover_type_code = 'espontanea' WHERE id = ?1",
        [&record.record.id],
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.covers_missing_fields.len(), 1);
}

#[test]
fn cover_maintenance_carries_the_cover_on_every_dgc() {
    // Art. 42.1.c's maintenance is a `cultural_operation` or a `grazing_record`
    // in its own right, so it travels as its own entry — carrying the cover
    // through `Cubiertas` rather than through `DatosCubierta`. The link is per
    // RECORD, which is what satisfies the descriptor's rule that one activity
    // may not mix DGCs with and without a cover.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let record = eco::insert_soil_cover(&mut conn, cover(&fx), None).unwrap();

    let mut mowing = operation(&fx);
    mowing.practice_code = "plant_cover".into();
    mowing.soil_cover_id = Some(record.record.id.clone());
    eco::insert_cultural_operation(&mut conn, mowing, None).unwrap();

    let mut cover_grazing = grazing(&fx);
    cover_grazing.practice_code = "plant_cover".into();
    cover_grazing.soil_cover_id = Some(record.record.id.clone());
    eco::insert_grazing_record(&mut conn, cover_grazing, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(
        block(&doc, "LaboresCulturales")[0]["DGCs"][0]["Cubiertas"][0]["TipoCobertura"],
        3
    );
    assert_eq!(
        block(&doc, "Pastoreo")[0]["DGCs"][0]["Cubiertas"][0]["TipoCobertura"],
        3
    );
}

#[test]
fn an_operation_on_no_cover_carries_no_cubiertas() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    eco::insert_cultural_operation(&mut conn, operation(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert!(
        block(&doc, "LaboresCulturales")[0]["DGCs"][0]
            .get("Cubiertas")
            .is_none()
    );
}

#[test]
fn residue_left_on_the_ground_sets_the_flag_its_kind_names() {
    // Both booleans are Obligatorio, so both are always sent — and each answers
    // for its own kind of residue. DEST_RES_VEG 1 is "Incorporación al suelo o
    // distribución en parcela" and 9 is "Trituración de restos de poda y
    // depositado sobre el terreno"; anything else took the residue away.
    let mut conn = db();
    let fx = fixture(&mut conn);

    let mut mown = operation(&fx);
    mown.performed_on = "2026-06-01".into();
    eco::insert_cultural_operation(&mut conn, mown, None).unwrap();

    let mut pruned = operation(&fx);
    pruned.performed_on = "2026-06-02".into();
    pruned.operation_kind_code = "pruning".into();
    pruned.residue_destination_code = Some("9".into());
    eco::insert_cultural_operation(&mut conn, pruned, None).unwrap();

    let mut removed = operation(&fx);
    removed.performed_on = "2026-06-03".into();
    removed.operation_kind_code = "pruning_removal".into();
    // DEST_RES_VEG 6 — traslado a planta de gestión de restos vegetales.
    removed.residue_destination_code = Some("6".into());
    eco::insert_cultural_operation(&mut conn, removed, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "LaboresCulturales");
    assert_eq!(entries[0]["DepositadoSueloDesb"], true);
    assert_eq!(entries[0]["DepositadoSueloPoda"], false);
    assert_eq!(entries[1]["DepositadoSueloDesb"], false);
    assert_eq!(entries[1]["DepositadoSueloPoda"], true);
    assert_eq!(entries[2]["DepositadoSueloDesb"], false);
    assert_eq!(entries[2]["DepositadoSueloPoda"], false);
}

#[test]
fn a_single_day_operation_fills_both_ends_of_the_exported_dates() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut one_day = operation(&fx);
    one_day.performed_end_date = None;
    eco::insert_cultural_operation(&mut conn, one_day, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(
        block(&doc, "LaboresCulturales")[0]["FechaInicio"],
        "05/06/2026"
    );
    assert_eq!(
        block(&doc, "LaboresCulturales")[0]["FechaFin"],
        "05/06/2026"
    );
}

#[test]
fn withdrawn_ecoscheme_records_become_deletion_entries() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let grazed = eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    let worked = eco::insert_cultural_operation(&mut conn, operation(&fx), None).unwrap();
    let covered = eco::insert_soil_cover(&mut conn, cover(&fx), None).unwrap();

    let first = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let aliases = [
        block(&first, "Pastoreo")[0]["IdAjenaPastoreo"].clone(),
        block(&first, "LaboresCulturales")[0]["IdAjenaLabor"].clone(),
        block(&first, "DatosCubierta")[0]["IdAjenaCubierta"].clone(),
    ];

    eco::soft_delete_grazing_record(&mut conn, &grazed.record.id, None).unwrap();
    eco::soft_delete_cultural_operation(&mut conn, &worked.record.id, None).unwrap();
    eco::soft_delete_soil_cover(&mut conn, &covered.record.id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(block(&doc, "Pastoreo")[0]["Borrar"], true);
    assert_eq!(block(&doc, "Pastoreo")[0]["IdAjenaPastoreo"], aliases[0]);
    assert_eq!(block(&doc, "LaboresCulturales")[0]["Borrar"], true);
    assert_eq!(
        block(&doc, "LaboresCulturales")[0]["IdAjenaLabor"],
        aliases[1]
    );
    assert_eq!(block(&doc, "DatosCubierta")[0]["Borrar"], true);
    assert_eq!(
        block(&doc, "DatosCubierta")[0]["IdAjenaCubierta"],
        aliases[2]
    );
}

#[test]
fn a_never_exported_withdrawal_leaves_no_ecoscheme_trace() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let grazed = eco::insert_grazing_record(&mut conn, grazing(&fx), None).unwrap();
    eco::soft_delete_grazing_record(&mut conn, &grazed.record.id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let activities = &doc["CUADERNO"][0]["ActividadesExplotacion"];
    assert!(activities.get("Pastoreo").is_none());
}

#[test]
fn precheck_ignores_withdrawn_ecoscheme_records() {
    // A deletion entry identifies a previously exported activity; it cannot
    // demand a width that will never be measured or an end that never came.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut open = grazing(&fx);
    open.ended_on = None;
    let open = eco::insert_grazing_record(&mut conn, open, None).unwrap();
    let mut unmeasured = cover(&fx);
    unmeasured.width_m = None;
    unmeasured.free_canopy_width_m = None;
    unmeasured.widths_stated_on = None;
    let unmeasured = eco::insert_soil_cover(&mut conn, unmeasured, None).unwrap();

    eco::soft_delete_grazing_record(&mut conn, &open.record.id, None).unwrap();
    eco::soft_delete_soil_cover(&mut conn, &unmeasured.record.id, None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.is_clean());
}

#[test]
fn a_campaign_with_no_ecoscheme_records_omits_all_three_blocks() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let activities = &doc["CUADERNO"][0]["ActividadesExplotacion"];
    assert!(activities.get("Pastoreo").is_none());
    assert!(activities.get("LaboresCulturales").is_none());
    assert!(activities.get("DatosCubierta").is_none());
}

#[test]
fn a_withdrawn_watering_leaves_its_fertigation_blocked() {
    // The rule a comment had asserted since seam 3 with nothing enforcing it:
    // a withdrawn watering counts as ABSENT, because the statement it carried is
    // retracted and `Fertirrigacion` then has nothing to say. It reports as the
    // missing-irrigation case rather than the missing-figures one — the farmer's
    // fix is to name a live watering, not to edit a withdrawn one.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let watered = fert::insert_irrigation_record(&mut conn, irrigation(&fx), None).unwrap();
    fert::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_sprinkler".into(),
            irrigation_record_id: Some(watered.record.id.clone()),
            ..fertilisation(&fx, &material_id)
        },
        None,
    )
    .unwrap();
    // Clean while the watering stands.
    assert!(
        export_precheck(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_clean()
    );

    fert::soft_delete_irrigation_record(&mut conn, &watered.record.id, None).unwrap();

    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!report.is_clean());
    assert_eq!(report.fertigations_missing_irrigation.len(), 1);
    assert!(report.fertigations_missing_water_figures.is_empty());
}
