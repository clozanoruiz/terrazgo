// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The treatment registers that are not a field spray, and the analyses:
//! `UsoSemillaTratada` (model 3.2), `TratamientosPostCosecha` (3.3),
//! `TratamientosEdifInstalaciones` (3.4/3.5) and `Analitica` (4).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_siex::{SiexError, build_cuaderno, export_precheck};

// ---------------------------------------------------------------------------
// Seam 1: module-cue's other four blocks — models 3.2, 3.3, 3.4/3.5 and 4.
// ---------------------------------------------------------------------------

/// A building with the REA code the format demands, plus the cadastral
/// reference Anexo V asks for.
fn insert_premises(conn: &mut Connection, farm_id: &str, rea: Option<&str>) -> String {
    terrazgo_core::repository::insert_premises(
        conn,
        terrazgo_core::models::NewPremises {
            farm_id: farm_id.into(),
            kind_code: "building".into(),
            name: "Almacén de grano".into(),
            address: Some("Camino de la Vega, 4".into()),
            vehicle_model: None,
            plate: None,
            class_code: Some("3".into()),
            volume_m3: Some(1200.0),
            notes: None,
            cadastral_reference: Some("47170A00500123 0000WX".into()),
            rea_installation_code: rea.map(str::to_string),
        },
        None,
    )
    .unwrap()
    .premises
    .id
}

/// A complete post-harvest record (model 3.3): everything the block requires.
fn post_harvest(fx: &Fixture) -> NewNonFieldTreatment {
    NewNonFieldTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        country_code: None,
        subject_kind_code: "postharvest".into(),
        premises_id: None,
        treated_on: "2026-08-20".into(),
        subject_description: "Trigo blando, silo 2".into(),
        // PROD_VEGETAL 85 = Granos de trigo.
        subject_product_code: Some("85".into()),
        treated_quantity_value: Some(120.0),
        treated_quantity_unit_code: Some("t".into()),
        product_id: fx.product_id.clone(),
        product_quantity_value: Some(2.4),
        product_quantity_unit_code: Some("kg".into()),
        operator_id: fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        problems: vec![NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: Some("good".into()),
        notes: Some("Fumigación preventiva.".into()),
    }
}

/// The same, filed as a premises treatment (model 3.4).
fn premises_treatment(fx: &Fixture, premises_id: &str) -> NewNonFieldTreatment {
    NewNonFieldTreatment {
        subject_kind_code: "storage_premises".into(),
        premises_id: Some(premises_id.into()),
        subject_product_code: None,
        treated_quantity_value: Some(800.0),
        treated_quantity_unit_code: Some("m3".into()),
        ..post_harvest(fx)
    }
}

fn analysis(fx: &Fixture) -> NewAnalysisRecord {
    NewAnalysisRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sampled_on: "2026-06-18".into(),
        material_kind_code: "soil".into(),
        bulletin_number: Some("B-2026/1187".into()),
        lab_name: Some("Laboratorio Agroalimentario".into()),
        lab_address: Some("Ctra. Burgos km 118".into()),
        lab_tax_id: Some("Q4700123B".into()),
        substances_detected: Some("Lambda cihalotrín 0,01 mg/kg".into()),
        soil: Default::default(),
        notes: None,
        plots: vec![NewAnalysisPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
        }],
        analysis_type_codes: vec!["soil_parameters".into()],
        substance_codes: vec!["170".into()],
    }
}

#[test]
fn a_post_harvest_treatment_states_its_produce_in_kilograms() {
    // Anexo V block 1.2 field 2: "Peso en kg del producto vegetal tratado",
    // and the block carries NO unit member — while model 3.3 prints tonnes,
    // which is what the register stores. 120 t must leave as 120000 kg.
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(&mut conn, post_harvest(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "TratamientosPostCosecha");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["Cantidad"], 120_000.0);
    assert_eq!(entries[0]["ProductoVegetal"], 85);
    assert_eq!(entries[0]["FechaActuacion"], "20/08/2026");
    // The amount of product used, never a dose: this block has no Dosis.
    let product = &entries[0]["ProductosFito"][0];
    assert_eq!(product["Cantidad"], 2.4);
    assert_eq!(product["Unidad"], 5); // UNIDADES_MEDIDA 5 = kg
    assert!(product.get("Dosis").is_none());
    // These blocks carry no AplicacionManual — but they DO carry the same
    // oneOf over the three equipment identifiers, so a hand application still
    // has to name exactly one. The sentinel is the same as TratamFito's.
    let equipment = &entries[0]["IdentificadorAplicador"][0]["EquipoAplicador"];
    assert_eq!(equipment["IdEquipoAplicador"], "manual");
    assert!(equipment.get("AplicacionManual").is_none());
}

#[test]
fn a_premises_treatment_is_identified_by_the_rea_code_of_its_building() {
    // `Edificaciones[].IdEdificacion` is REA's own key for the installation —
    // not a client alias — so it comes from the premises' Spanish extension.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let premises_id = insert_premises(&mut conn, &fx.farm_id, Some("4700123456"));
    repo::insert_non_field_treatment(&mut conn, premises_treatment(&fx, &premises_id), None)
        .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "TratamientosEdifInstalaciones");
    assert_eq!(entries.len(), 1);
    let building = &entries[0]["Edificaciones"][0];
    assert_eq!(building["IdEdificacion"], 4_700_123_456i64);
    // B.f's treated volume, in the unit the register measured it in.
    assert_eq!(building["Volumen"], 800.0);
    assert_eq!(building["Unidad"], 3); // UNIDADES_MEDIDA 3 = m3
    // And it does NOT appear in the post-harvest block: one register, two
    // blocks, split by what was treated.
    assert!(
        doc["CUADERNO"][0]["ActividadesExplotacion"]
            .get("TratamientosPostCosecha")
            .is_none()
    );
}

#[test]
fn a_building_without_its_rea_code_blocks_the_export_rather_than_inventing_one() {
    // The registry stays optional for the RECORD — a farmer must be able to
    // note a treatment before filling in registry papers — so the refusal
    // belongs here, with a fixable list, and never at insert time.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let premises_id = insert_premises(&mut conn, &fx.farm_id, None);
    repo::insert_non_field_treatment(&mut conn, premises_treatment(&fx, &premises_id), None)
        .unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.premises_missing_rea_code.len(), 1);
    assert_eq!(
        precheck.premises_missing_rea_code[0].subject_description,
        "Almacén de grano, Camino de la Vega, 4"
    );
    assert!(!precheck.is_clean());
    assert!(matches!(
        build_cuaderno(&mut conn, &fx.season_id, &fx.farm_id, None).unwrap_err(),
        SiexError::Invalid("export_precheck_failed")
    ));

    // A record naming no premises at all is refused the same way: Edificaciones
    // is 1..n and its only member is that code.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut unnamed = premises_treatment(&fx, "");
    unnamed.premises_id = None;
    repo::insert_non_field_treatment(&mut conn, unnamed, None).unwrap();
    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.premises_missing_rea_code.len(), 1);
}

#[test]
fn a_rea_code_that_is_not_an_integer_blocks_the_export() {
    // The schema types IdEdificacion as an integer; the registry never
    // pattern-checks what the farmer types (the roma_number precedent), so the
    // check has to live here.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let premises_id = insert_premises(&mut conn, &fx.farm_id, Some("no es un número"));
    repo::insert_non_field_treatment(&mut conn, premises_treatment(&fx, &premises_id), None)
        .unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.premises_missing_rea_code.len(), 1);
}

#[test]
fn a_problem_the_block_cannot_express_is_refused_rather_than_dropped() {
    // Neither non-field block carries MalasHierbas, and the buildings block has
    // no ReguladoresOtros either. Exporting such a record would state that a
    // treatment happened while losing the reason it happened for — the same
    // failure the non-chemical-measure rule refuses.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut weeds = post_harvest(&fx);
    weeds.problems = vec![NewTreatmentProblem {
        reason_category_code: "weed".into(),
        problem_code: "45".into(),
    }];
    repo::insert_non_field_treatment(&mut conn, weeds, None).unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.non_field_unexpressible_problem.len(), 1);
    assert!(!precheck.is_clean());

    // A growth regulator IS expressible post-harvest (that block has
    // ReguladoresOtros) and is NOT in a building.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut regulated = post_harvest(&fx);
    regulated.problems = vec![NewTreatmentProblem {
        reason_category_code: "growth_regulator".into(),
        problem_code: "3".into(),
    }];
    repo::insert_non_field_treatment(&mut conn, regulated, None).unwrap();
    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(precheck.non_field_unexpressible_problem.is_empty());

    let premises_id = insert_premises(&mut conn, &fx.farm_id, Some("4700123456"));
    let mut regulated_building = premises_treatment(&fx, &premises_id);
    regulated_building.problems = vec![NewTreatmentProblem {
        reason_category_code: "growth_regulator".into(),
        problem_code: "3".into(),
    }];
    repo::insert_non_field_treatment(&mut conn, regulated_building, None).unwrap();
    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.non_field_unexpressible_problem.len(), 1);
}

#[test]
fn the_non_field_registers_demand_what_their_blocks_require() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    // An operator holding no licence number: `AplicadorEmpresa.NumROPO` is
    // required in both non-field blocks, and the snapshot is frozen from this
    // row at insert.
    let unlicensed = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Ana Ruiz".into(),
            tax_id: None,
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap()
    .id;
    let mut bare = post_harvest(&fx);
    bare.efficacy_code = None;
    bare.subject_product_code = None;
    bare.treated_quantity_value = None;
    bare.treated_quantity_unit_code = None;
    bare.product_quantity_value = None;
    bare.product_quantity_unit_code = None;
    bare.operator_id = unlicensed;
    repo::insert_non_field_treatment(&mut conn, bare, None).unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.non_field_missing_efficacy.len(), 1);
    assert_eq!(precheck.non_field_missing_operator_licence.len(), 1);
    assert_eq!(precheck.non_field_missing_product_quantity.len(), 1);
    assert_eq!(precheck.post_harvest_missing_produce.len(), 1);
    // The register that has no produce column is not asked for one.
    assert!(precheck.premises_missing_rea_code.is_empty());
    assert!(!precheck.is_clean());
}

#[test]
fn treated_seed_names_the_crop_and_never_the_product() {
    // Anexo V block 1.4 field 1 is "Cultivo — código del cultivo del catálogo
    // SIEX", so `Producto` is the PRODUCTOS code of what was sown. The
    // phytosanitary product the sack was treated with is free text in the
    // register, and no ProductosFito child is emitted for it.
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "UsoSemillaTratada");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["Producto"], 1);
    assert_eq!(entries[0]["Cantidad"], 1800.0);
    assert_eq!(entries[0]["NumeroLote"], "L-2025-4471");
    assert_eq!(entries[0]["Fecha"], "15/10/2025");
    assert!(
        entries[0].get("ProductosFito").is_none(),
        "the register stores no product amount, so none is invented"
    );
}

#[test]
fn treated_seed_demands_the_four_values_its_block_requires() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut bare = seed_treatment(&fx);
    bare.crop_code = None;
    bare.seed_quantity_kg = None;
    bare.treatment_kind_code = None;
    bare.efficacy_code = None;
    repo::insert_seed_treatment(&mut conn, bare, None).unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(precheck.seed_missing_fields.len(), 1);
    assert_eq!(precheck.seed_missing_fields[0].species_name, "Trigo blando");
    assert!(!precheck.is_clean());
}

#[test]
fn an_analysis_exports_as_recorded_and_blocks_nothing() {
    // The one block of the four with no precheck rule: Anexo V grades all eight
    // fields Voluntario and the schema requires only the material and the date,
    // both NOT NULL in the register.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut soil = analysis(&fx);
    soil.soil = module_cue::models::SoilParameters {
        ph: Some(7.8),
        organic_matter_pct: Some(1.9),
        available_p_mg_kg: Some(18.0),
        ..Default::default()
    };
    repo::insert_analysis_record(&mut conn, soil, None).unwrap();

    let precheck = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(precheck.is_clean());

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "Analitica");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["MaterialAnalizado"], 3); // MATERIAL_ANALIZADO soil
    assert_eq!(entries[0]["Fecha"], "18/06/2026");
    assert_eq!(entries[0]["NumBoletin"], "B-2026/1187");
    assert_eq!(entries[0]["Nif"], "Q4700123B");
    assert_eq!(entries[0]["TiposSustancias"][0]["TipoSustancia"], 170);
    // The soil block carries only what the bulletin stated: a figure it did not
    // measure is absent, never zero.
    let soil = &entries[0]["ParametrosSuelo"];
    assert_eq!(soil["Ph"], 7.8);
    assert_eq!(soil["MateriaOrganica"], 1.9);
    assert!(soil.get("NitrogenoTotal").is_none());
    // And its DGC names the sampled crop.
    assert!(entries[0]["DGCs"][0]["CodigoDGCAjena"].is_i64());
}

#[test]
fn an_analysis_with_no_soil_figures_omits_the_soil_block_entirely() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_analysis_record(&mut conn, analysis(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert!(block(&doc, "Analitica")[0].get("ParametrosSuelo").is_none());
}

#[test]
fn withdrawn_records_of_every_register_become_deletion_entries() {
    // The alias is frozen at first export and SIEX keys its deletions on it, so
    // a withdrawal re-sends the same integer with Borrar — and a record that was
    // never exported leaves no trace at all.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let premises_id = insert_premises(&mut conn, &fx.farm_id, Some("4700123456"));
    let produce = repo::insert_non_field_treatment(&mut conn, post_harvest(&fx), None).unwrap();
    let building =
        repo::insert_non_field_treatment(&mut conn, premises_treatment(&fx, &premises_id), None)
            .unwrap();
    let seed = repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();
    let sample = repo::insert_analysis_record(&mut conn, analysis(&fx), None).unwrap();

    // First export mints the aliases.
    let before = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let alias_of =
        |doc: &Value, name: &str, key: &str| -> i64 { block(doc, name)[0][key].as_i64().unwrap() };
    let produce_alias = alias_of(&before, "TratamientosPostCosecha", "IdAjenaTratamPostco");
    let building_alias = alias_of(
        &before,
        "TratamientosEdifInstalaciones",
        "IdAjenaTratamEdif",
    );
    let seed_alias = alias_of(&before, "UsoSemillaTratada", "IdAjenaSemillaTrat");
    let analysis_alias = alias_of(&before, "Analitica", "IdAjenaAna");

    repo::soft_delete_non_field_treatment(&mut conn, &produce.record.id, None).unwrap();
    repo::soft_delete_non_field_treatment(&mut conn, &building.record.id, None).unwrap();
    repo::soft_delete_seed_treatment(&mut conn, &seed.record.id, None).unwrap();
    repo::soft_delete_analysis_record(&mut conn, &sample.record.id, None).unwrap();

    let after = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&after);
    for (name, key, alias) in [
        (
            "TratamientosPostCosecha",
            "IdAjenaTratamPostco",
            produce_alias,
        ),
        (
            "TratamientosEdifInstalaciones",
            "IdAjenaTratamEdif",
            building_alias,
        ),
        ("UsoSemillaTratada", "IdAjenaSemillaTrat", seed_alias),
        ("Analitica", "IdAjenaAna", analysis_alias),
    ] {
        let entries = block(&after, name);
        assert_eq!(entries.len(), 1, "{name} keeps its deletion entry");
        assert_eq!(entries[0][key], alias, "{name} re-sends its frozen alias");
        assert_eq!(entries[0]["Borrar"], true, "{name} is withdrawn");
    }
}

#[test]
fn a_record_deleted_before_it_was_ever_exported_leaves_no_entry() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let seed = repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();
    repo::soft_delete_seed_treatment(&mut conn, &seed.record.id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert!(
        doc["CUADERNO"][0]["ActividadesExplotacion"]
            .get("UsoSemillaTratada")
            .is_none(),
        "nothing was ever sent, so there is nothing to withdraw"
    );
}

#[test]
fn a_campaign_with_no_records_of_a_block_omits_it_rather_than_sending_an_empty_array() {
    // No block is required and every one is 0..n, so absence is how "none
    // happened" is stated — an empty array would be a different claim.
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(&mut conn, post_harvest(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let activities = &doc["CUADERNO"][0]["ActividadesExplotacion"];
    assert!(activities.get("TratamientosPostCosecha").is_some());
    for absent in [
        "TratamFito",
        "TratamientosEdifInstalaciones",
        "UsoSemillaTratada",
        "Analitica",
    ] {
        assert!(activities.get(absent).is_none(), "{absent} must be absent");
    }
}

#[test]
fn an_advised_non_field_record_carries_the_advisor_ropo_and_nothing_it_cannot_attest() {
    // Anexo III B.d reaches these registers through B.b and B.f. Only NumROPO
    // is required and only NumROPO is sent: Validacion, Confirmacion and
    // Contrato describe a sign-off the book has no signature capability to hold.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "ATRIA Cerealista".into(),
            tax_id: Some("G47654321".into()),
            registration_number: Some("ROPO-AS-47-0912".into()),
        },
        None,
    )
    .unwrap();
    let mut advised = post_harvest(&fx);
    advised.advisor_id = Some(advisor.id.clone());
    repo::insert_non_field_treatment(&mut conn, advised, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "TratamientosPostCosecha")[0];
    assert_eq!(entry["AsesorValidacion"]["NumROPO"], "ROPO-AS-47-0912");
    assert!(entry["AsesorValidacion"].get("Validacion").is_none());
    assert!(entry["AsesorValidacion"].get("Confirmacion").is_none());
    assert!(entry["AsesorValidacion"].get("Contrato").is_none());
}

#[test]
fn an_unadvised_record_sends_no_advisor_block_at_all() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_non_field_treatment(&mut conn, post_harvest(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert!(
        block(&doc, "TratamientosPostCosecha")[0]
            .get("AsesorValidacion")
            .is_none()
    );
}
