// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `module-fertilisation`'s three blocks: `Fertilizacion` (the applications and
//! the material registry behind them), `PlanAbonado` (the plan as art. 5.a
//! records it) and `Riego`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::repository as repo;
use serde_json::Value;
use terrazgo_siex::export_precheck;

// ---------------------------------------------------------------------------
// Seam 3 — module-fertilisation's three blocks
// ---------------------------------------------------------------------------

use module_fertilisation::models::*;
use module_fertilisation::repository as fert;

fn plan(fx: &Fixture) -> NewFertilisationPlan {
    NewFertilisationPlan {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        needs_n_kg_ha: 140.0,
        needs_p2o5_kg_ha: 60.0,
        needs_k2o_kg_ha: 80.0,
        expected_yield_kg_ha: 5200.0,
        preceding_crop_code: Some("28".into()),
        drawn_up_on: "2025-09-30".into(),
        tool_generated: true,
        notes: None,
        crop_ids: vec![fx.wheat_crop_id.clone()],
    }
}

#[test]
fn an_irrigation_exports_its_system_volume_and_water_origin() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    fert::insert_irrigation_record(&mut conn, irrigation(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "Riego");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["FechaInicio"], "14/06/2026");
    // Art. 5.f lets an intensive or fertigated crop accumulate the record over a
    // fortnight, which is why the register carries an interval at all.
    assert_eq!(entries[0]["FechaFin"], "28/06/2026");
    assert_eq!(entries[0]["SistemaRiego"], 6); // SIST_RIEGO drip
    assert_eq!(entries[0]["Cantidad"], 320.0);
    // Anexo V names m³ and L as valid units; the catalogue carries m³/ha as its
    // own code, so a per-hectare volume states itself rather than being
    // converted with a surface the record may not carry.
    assert_eq!(entries[0]["UnidadMedida"], 19);
    assert_eq!(entries[0]["OrigenAgua"][0]["IdOrigenAgua"], 2);
    assert_eq!(entries[0]["NumContador"], "C-4471");
    assert_eq!(entries[0]["DGCs"][0]["Superficie"], 3.5);
    assert!(entries[0]["DGCs"][0]["CodigoDGCAjena"].is_i64());
}

#[test]
fn an_irrigation_of_one_day_sends_its_start_as_both_ends() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let single = NewIrrigationRecord {
        irrigation_end_date: None,
        ..irrigation(&fx)
    };
    fert::insert_irrigation_record(&mut conn, single, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "Riego")[0];
    assert_eq!(entry["FechaInicio"], "14/06/2026");
    assert_eq!(entry["FechaFin"], "14/06/2026");
}

#[test]
fn a_fertilisation_carries_its_material_composition_from_the_registry() {
    // The record freezes only what section 6 PRINTS; C.h's eight values and
    // C.i's heavy metals stay on the registry row, which is what the twin asks
    // for. The three arrays differ only in which catalogue their integer
    // indexes — which is exactly what `kind_code` records.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "Fertilizacion");
    assert_eq!(entries.len(), 1);
    let mat = &entries[0]["MaterialFertilizante"];
    assert_eq!(mat["Material"], 5);
    assert_eq!(mat["Macronutrientes"][0]["TipoMacroN"], 3);
    assert_eq!(mat["Micronutrientes"][0]["TipoMicroN"], 2);
    assert_eq!(mat["MetalesPesados"][0]["TipoMetalP"], 3);
    // The same integer means a different substance in each catalogue, so a
    // nutrient never carries two of the three type members.
    assert!(mat["Macronutrientes"][0].get("TipoMetalP").is_none());
    // A supplier is identified by exactly one registry number ("excluyente").
    assert_eq!(mat["REGA"], "ES471820000123");
    assert!(mat.get("NifEmpresa").is_none());
    assert!(mat.get("NIMA").is_none());
    assert_eq!(mat["TratamientoEstiercoles"], 5);
    // The density carries the unit the column does not: kg/L on every label.
    assert_eq!(mat["Densidad"], 1.03);
    assert_eq!(mat["UnidadesMedida"], 12);
}

#[test]
fn a_fertilisation_states_its_two_legal_fields_and_the_insumos_flag() {
    // C.c and C.f are separate legal fields that the printed model's
    // "(F)/(AF)/(AC)" footnote merges into one letter, and the twin keeps them
    // apart. `GestionSostInsu` has no decree and no printed box: Anexo V marks
    // it Obligatorio inside a block we do send.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "Fertilizacion")[0];
    assert_eq!(entry["GestionSostInsu"], true);
    assert_eq!(entry["BuenasPracticas"][0]["TipoBPF"], 4);
    let app = &entry["AplicacionMaterialFertilizante"];
    assert_eq!(app["TipoFertilizacion"], 2); // cobertera
    assert_eq!(app["MetodoFertilizacion"], 1); // a voleo
    assert_eq!(app["Dosis"], 250.0);
    assert_eq!(app["Unidad"], 17); // kg/ha
    assert_eq!(app["AplicacionLodos"], false);
    // C.k: the decree attaches the REGFER number to the service company; the
    // twin carries only the number, so the company's NAME stays printed-only.
    assert_eq!(app["EmpresaServicios"], "REGFER-4471");
}

#[test]
fn a_record_with_no_good_practice_sends_an_empty_array_rather_than_failing() {
    // `BuenasPracticas` is in the schema's `required` list but carries no
    // `minItems`, and Anexo V's own field 6 says the field "irá vacío" when none
    // was declared. So an empty array is the correct statement, and the register
    // stays fillable by a farmer working from the printed model, which has no
    // such column.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let bare = NewFertilisationRecord {
        practices: vec![],
        ..fertilisation(&fx, &material_id)
    };
    fert::insert_fertilisation_record(&mut conn, bare, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let practices = block(&doc, "Fertilizacion")[0]["BuenasPracticas"]
        .as_array()
        .unwrap();
    assert!(practices.is_empty());
}

#[test]
fn an_application_with_no_machine_omits_the_equipment_block_entirely() {
    // C.g makes the machine optional ("cuando proceda") and the block's `oneOf`
    // makes a half-filled one invalid, so omission is the only correct way to
    // say "no machine".
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert!(
        block(&doc, "Fertilizacion")[0]
            .get("EquipoAplicador")
            .is_none()
    );
}

#[test]
fn a_machine_outside_roma_is_named_by_its_own_row_id() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let sprayer = insert_machinery(&mut conn, &fx.farm_id, Some("ROMA-8891"), None);
    let unregistered = insert_machinery(&mut conn, &fx.farm_id, None, None);

    for machinery_id in [&sprayer, &unregistered] {
        fert::insert_fertilisation_record(
            &mut conn,
            NewFertilisationRecord {
                machinery_id: Some(machinery_id.clone()),
                ..fertilisation(&fx, &material_id)
            },
            None,
        )
        .unwrap();
    }

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "Fertilizacion");
    let equipment: Vec<&Value> = entries.iter().map(|e| &e["EquipoAplicador"]).collect();
    assert!(equipment.iter().any(|e| e["NumROMA"] == "ROMA-8891"));
    assert!(
        equipment
            .iter()
            .any(|e| e["IdEquipoAplicador"] == unregistered.as_str())
    );
    // Exactly one identifier each, which is what the `oneOf` demands.
    for e in equipment {
        assert_eq!(
            e.get("NumROMA").is_some() as u8 + e.get("IdEquipoAplicador").is_some() as u8,
            1
        );
    }
}

#[test]
fn a_fertigation_restates_the_water_from_the_watering_it_names() {
    // One act the decree records twice — art. 5.d the fertiliser, art. 5.e the
    // water — which the format re-joins. `DosisN`/`DosisP` are Anexo III C.l's
    // two water-quality figures, and this block is their ONLY reader anywhere in
    // the format: no printed column and no member of `Riego` carries them.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let watering = fert::insert_irrigation_record(&mut conn, irrigation(&fx), None).unwrap();
    fert::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_localised".into(),
            irrigation_record_id: Some(watering.record.id.clone()),
            ..fertilisation(&fx, &material_id)
        },
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let fertigation = &block(&doc, "Fertilizacion")[0]["Fertirrigacion"];
    assert_eq!(fertigation["SistemaRiego"], 6);
    assert_eq!(fertigation["Cantidad"], 320.0);
    assert_eq!(fertigation["DosisN"], 12.5);
    assert_eq!(fertigation["DosisP"], 1.8);
    // mg/L, code 20 — the columns carry no unit because C.l fixes it.
    assert_eq!(fertigation["UnidadDosisN"], 20);
    assert_eq!(fertigation["UnidadDosisP"], 20);
    assert_eq!(fertigation["OrigenAgua"][0]["IdOrigenAgua"], 2);
    assert_eq!(fertigation["NumContador"], "C-4471");
    // The watering still travels as its own Riego entry: the two blocks answer
    // different questions about one act.
    assert_eq!(block(&doc, "Riego").len(), 1);
}

#[test]
fn an_ordinary_application_carries_no_fertigation_block() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert!(
        block(&doc, "Fertilizacion")[0]
            .get("Fertirrigacion")
            .is_none()
    );
}

#[test]
fn precheck_demands_a_watering_for_a_fertigation_and_its_two_water_figures() {
    // Asks for nothing new: art. 5.e already obliges the irrigation record for
    // that watering, so the list is genuinely fixable.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_sprinkler".into(),
            ..fertilisation(&fx, &material_id)
        },
        None,
    )
    .unwrap();
    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.fertigations_missing_irrigation.len(), 1);
    assert_eq!(
        report.fertigations_missing_irrigation[0].material_name,
        "Purín de porcino"
    );
    assert!(!report.is_clean());

    // A watering that states no water figures blocks for the other reason: both
    // are required inside `Fertirrigacion`.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let dry = fert::insert_irrigation_record(
        &mut conn,
        NewIrrigationRecord {
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            ..irrigation(&fx)
        },
        None,
    )
    .unwrap();
    fert::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            application_method_code: "fertigation_sprinkler".into(),
            irrigation_record_id: Some(dry.record.id.clone()),
            ..fertilisation(&fx, &material_id)
        },
        None,
    )
    .unwrap();
    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(report.fertigations_missing_irrigation.is_empty());
    assert_eq!(report.fertigations_missing_water_figures.len(), 1);
}

#[test]
fn an_ordinary_application_is_subject_to_neither_fertigation_rule() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();
    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(report.fertigations_missing_irrigation.is_empty());
    assert!(report.fertigations_missing_water_figures.is_empty());
    assert!(report.is_clean());
}

#[test]
fn the_plan_exports_article_5as_four_figures_and_nothing_more() {
    // The twin's required set IS art. 5.a's list plus the tool flag — the
    // exchange format agreeing with the article is what confirmed the book
    // carries the summary and not the document art. 6 defines.
    let mut conn = db();
    let fx = fixture(&mut conn);
    fert::insert_fertilisation_plan(&mut conn, plan(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "PlanAbonado");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["NecesidadUFN"], 140.0);
    assert_eq!(entries[0]["NecesidadUFP2O5"], 60.0);
    assert_eq!(entries[0]["NecesidadUFK2O"], 80.0);
    assert_eq!(entries[0]["ObjetivoProduccion"], 5200.0);
    assert_eq!(entries[0]["CultivoPrecedente"], 28);
    assert_eq!(entries[0]["Herramienta"], true);
    assert_eq!(entries[0]["FechaGeneracion"], "30/09/2025");
    // Its DGC carries no surface: a plan states doses per hectare.
    assert!(entries[0]["DGCs"][0].get("Superficie").is_none());
    assert!(entries[0]["DGCs"][0]["CodigoDGCAjena"].is_i64());
}

#[test]
fn precheck_lists_a_plan_with_no_preceding_crop() {
    // Required by the schema and nullable here, because a unit coming out of
    // fallow has none — and inventing one would state a rotation that did not
    // happen.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let fallow = NewFertilisationPlan {
        preceding_crop_code: None,
        ..plan(&fx)
    };
    fert::insert_fertilisation_plan(&mut conn, fallow, None).unwrap();

    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.plans_missing_preceding_crop.len(), 1);
    assert_eq!(
        report.plans_missing_preceding_crop[0].drawn_up_on,
        "2025-09-30"
    );
    assert!(!report.is_clean());
}

#[test]
fn precheck_lists_fertilised_and_irrigated_plots_with_no_crop() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_irrigation_record(
        &mut conn,
        NewIrrigationRecord {
            plots: vec![NewIrrigationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
                irrigated_area_ha: None,
            }],
            ..irrigation(&fx)
        },
        None,
    )
    .unwrap();
    fert::insert_fertilisation_record(
        &mut conn,
        NewFertilisationRecord {
            plots: vec![NewFertilisationPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: None,
                fertilised_area_ha: None,
            }],
            ..fertilisation(&fx, &material_id)
        },
        None,
    )
    .unwrap();

    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.application_plots_missing_crop.len(), 2);
    let registers: Vec<&str> = report
        .application_plots_missing_crop
        .iter()
        .map(|r| r.register_code.as_str())
        .collect();
    assert!(registers.contains(&"fertilisation"));
    assert!(registers.contains(&"irrigation"));
}

#[test]
fn withdrawn_fertilisations_irrigations_and_plans_become_deletion_entries() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    let applied =
        fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None)
            .unwrap();
    let watered = fert::insert_irrigation_record(&mut conn, irrigation(&fx), None).unwrap();
    let planned = fert::insert_fertilisation_plan(&mut conn, plan(&fx), None).unwrap();

    let first = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let aliases = [
        block(&first, "Fertilizacion")[0]["IdAjenaFerti"].clone(),
        block(&first, "Riego")[0]["IdAjenaRiego"].clone(),
        block(&first, "PlanAbonado")[0]["IdAjenaPlan"].clone(),
    ];

    fert::soft_delete_fertilisation_record(&mut conn, &applied.record.id, None).unwrap();
    fert::soft_delete_irrigation_record(&mut conn, &watered.record.id, None).unwrap();
    fert::soft_delete_fertilisation_plan(&mut conn, &planned.plan.id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(block(&doc, "Fertilizacion")[0]["Borrar"], true);
    assert_eq!(block(&doc, "Fertilizacion")[0]["IdAjenaFerti"], aliases[0]);
    assert_eq!(block(&doc, "Riego")[0]["Borrar"], true);
    assert_eq!(block(&doc, "Riego")[0]["IdAjenaRiego"], aliases[1]);
    assert_eq!(block(&doc, "PlanAbonado")[0]["Borrar"], true);
    assert_eq!(block(&doc, "PlanAbonado")[0]["IdAjenaPlan"], aliases[2]);
}

#[test]
fn a_retired_material_still_resolves_for_a_past_record() {
    // The registry row is soft-deleted precisely so a decade-old record can
    // still resolve the composition it never froze. The ordinary getter filters
    // those out, which is right for a picker and wrong for an export.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let material_id = material(&mut conn);
    fert::insert_fertilisation_record(&mut conn, fertilisation(&fx, &material_id), None).unwrap();
    fert::soft_delete_fertiliser_material(&mut conn, &material_id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(
        block(&doc, "Fertilizacion")[0]["MaterialFertilizante"]["Material"],
        5
    );
}

#[test]
fn a_campaign_with_none_of_the_three_registers_omits_all_three_blocks() {
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
    assert!(activities.get("Fertilizacion").is_none());
    assert!(activities.get("Riego").is_none());
    assert!(activities.get("PlanAbonado").is_none());
}
