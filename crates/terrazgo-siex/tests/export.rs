// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The SIEX cuaderno export (docs/siex-export.md), and the rules that hold for
//! EVERY activity block: the headline contract that the output validates
//! against the vendored official CUE JSON Schema v3.11.4 — the same artifact
//! the authority validates with — plus the serialization rules (dd/mm/yyyy
//! dates, catalogue code mapping, dose conversion), the per-crop splits and
//! frozen integer aliases, deletion entries, and the precheck that lists what
//! blocks a farm from exporting.
//!
//! One block per sibling file, named for the blocks it emits rather than for
//! the arc seam that added them: `export_phytosanitary.rs`,
//! `export_treatment_registers.rs`, `export_sowing_and_sales.rs`,
//! `export_fertilisation.rs`, `export_ecoscheme.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use terrazgo_core::models::FarmEsFields;
use terrazgo_siex::export_precheck;

// ---------------------------------------------------------------------------
// The headline contract: the output validates against the official schema
// ---------------------------------------------------------------------------

#[test]
fn export_validates_against_the_official_schema() {
    let mut conn = db();
    let fx = fixture(&mut conn);

    // A representative spread: a manual single-crop record, a machinery
    // multi-crop record (splits), and one with several problems/justifications.
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-04-10"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let machinery_id = insert_machinery(&mut conn, &fx.farm_id, Some("47-00123"), None);
    let mut multi = treatment(&fx, "2026-05-01");
    multi.machinery_id = Some(machinery_id);
    multi.justifications = vec!["monitoring".into(), "advisor_recommendation".into()];
    multi.problems.push(NewTreatmentProblem {
        reason_category_code: "pest".into(),
        problem_code: "135".into(),
    });
    multi.notes = Some("Aplicación en banda".into());
    repo::insert_treatment_record(
        &mut conn,
        multi,
        vec![
            on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5),
            on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0),
        ],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    // Sanity: the multi-crop record split, so 1 + 2 TratamFito entries.
    assert_eq!(treatment_activities(&doc).len(), 3);
}

#[test]
fn envelope_carries_the_farm_identity() {
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
    let entry = &doc["CUADERNO"][0];
    // CAExplotacion derives from the province (47 → Castilla y León, INE 07);
    // UnidadGestora defaults to the titular NIF (CUECYL question 7).
    assert_eq!(entry["CAExplotacion"], "07");
    assert_eq!(entry["IdTitular"], "12345678Z");
    assert_eq!(entry["CodigoRea"], "ES244700000123");
    assert_eq!(entry["UnidadGestora"], "12345678Z");
}

// ---------------------------------------------------------------------------
// Serialization rules
// ---------------------------------------------------------------------------

#[test]
fn dates_are_rendered_dd_mm_yyyy() {
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
    let t = &treatment_activities(&doc)[0];
    // One application day: FechaInicio = FechaFin, in the schema's dd/mm/yyyy.
    assert_eq!(t["FechaInicio"], "01/05/2026");
    assert_eq!(t["FechaFin"], "01/05/2026");
}

/// The exchange format requires both ends of the actuation. A treatment that
/// really ran over several days now says so, instead of repeating its start —
/// the same interval Anexo III Parte I B allows the paper register to record.
#[test]
fn an_actuation_interval_fills_both_ends_of_the_exported_dates() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.application_end_date = Some("2026-05-03".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let t = &treatment_activities(&doc)[0];
    assert_eq!(t["FechaInicio"], "01/05/2026");
    assert_eq!(t["FechaFin"], "03/05/2026");
}

#[test]
fn codes_map_through_the_siex_catalogues() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.justifications = vec!["monitoring".into(), "alert_device".into()];
    new.efficacy_code = Some("fair".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let t = &treatment_activities(&doc)[0];
    // JUSTIFICACION_ACTUACION: monitoring → 2, alert_device → 6;
    // EFICACIA_TRATAMIENTO: fair → 2; TIPO_PRODFITO: registered → 1.
    let justs: Vec<i64> = t["Justificaciones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["JustAct"].as_i64().unwrap())
        .collect();
    assert_eq!(justs, vec![2, 6]);
    assert_eq!(t["Eficacia"], 2);
    let p = &t["ProductosFito"][0];
    assert_eq!(p["TipoProducto"], 1);
    assert_eq!(p["NumRegistro"], "ES-25.123");
    assert!(
        p.get("MateriaActiva").is_none(),
        "registered products ride NumRegistro alone (3.11.4 re-diff)"
    );
}

#[test]
fn problems_land_in_their_export_buckets() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    new.problems = vec![
        NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "weed".into(),
            problem_code: "10".into(),
        },
        // growth_regulator and other share the ReguladoresOtros bucket — the
        // same code arriving through both must not repeat in the payload.
        NewTreatmentProblem {
            reason_category_code: "growth_regulator".into(),
            problem_code: "3".into(),
        },
        NewTreatmentProblem {
            reason_category_code: "other".into(),
            problem_code: "3".into(),
        },
    ];
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let pf = &treatment_activities(&doc)[0]["ProblematicaFito"];
    assert_eq!(
        pf["Enfermedades"]["TipoEnfermedad"],
        serde_json::json!([254])
    );
    assert_eq!(
        pf["ArtropodosGasteropodos"]["TipoPlaga"],
        serde_json::json!([135])
    );
    assert_eq!(
        pf["MalasHierbas"]["TipoMalaHierba"],
        serde_json::json!([10])
    );
    assert_eq!(
        pf["ReguladoresOtros"]["TipoRegulador"],
        serde_json::json!([3])
    );
}

#[test]
fn absent_problem_buckets_are_omitted() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"), // disease only
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let pf = &treatment_activities(&doc)[0]["ProblematicaFito"];
    assert!(pf.get("Enfermedades").is_some());
    assert!(pf.get("ArtropodosGasteropodos").is_none());
    assert!(pf.get("MalasHierbas").is_none());
    assert!(pf.get("ReguladoresOtros").is_none());
}

#[test]
fn dose_units_convert_with_their_exact_factor() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    // SIEX has no ml/ha: 1500 ml/ha exports as 1.5 L/ha (UNIDADES_MEDIDA 18).
    new.dose_value = Some(1500.0);
    new.dose_unit_code = Some("ml_ha".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let p = &treatment_activities(&doc)[0]["ProductosFito"][0];
    assert_eq!(p["Unidad"], 18);
    let dose = p["Dosis"].as_f64().unwrap();
    assert!((dose - 1.5).abs() < 1e-9, "expected 1.5 L/ha, got {dose}");
    assert!(
        p.get("Cantidad").is_none(),
        "Dosis XOR Cantidad — rate units emit Dosis (descriptor: 'nunca ambas')"
    );
}

/// The descriptor is a machine exchange format and carries NO locale.
///
/// Every figure in it is a JSON number, which RFC 8259 spells with a dot and a
/// dot only — so nothing here may pass through the display formatters, whose
/// job is the opposite. The UI's region setting cannot reach this file, and the
/// test says so at the boundary rather than trusting that nobody wires it up:
/// a serialized "1,5" would be a string where the schema declares a number, and
/// the authority's own validator would reject the submission.
#[test]
fn the_descriptor_carries_numbers_not_localised_text() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut new = treatment(&fx, "2026-05-01");
    // A fractional dose and a fractional surface: the two places a decimal
    // comma would appear first if a display formatter ever leaked in.
    new.dose_value = Some(1.5);
    new.dose_unit_code = Some("l_ha".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let activity = &treatment_activities(&doc)[0];
    let dose = &activity["ProductosFito"][0]["Dosis"];
    assert!(dose.is_number(), "Dosis must be a JSON number, got {dose}");
    assert!(
        (dose.as_f64().unwrap() - 1.5).abs() < 1e-9,
        "the value must survive serialization unrounded, got {dose}"
    );

    // And the same over the WHOLE document, so a block added later cannot
    // quietly start emitting a formatted string.
    let raw = serde_json::to_string(&doc).unwrap();
    assert!(
        !raw.contains("\"1,5\""),
        "a decimal comma reached the descriptor as text: {raw}"
    );
    assert!(
        raw.contains("1.5"),
        "the dot-decimal figure should be present verbatim"
    );
}

#[test]
fn exceptional_authorisations_emit_their_substance_code() {
    let mut conn = db();
    let fx = fixture(&mut conn);

    let product_id = repo::insert_product(
        &mut conn,
        NewProduct {
            commercial_name: "Excepcional X".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: Some(7),
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        &mut conn,
        NewProductAuthorisation {
            product_id: product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "EXC-2026-01".into(),
            kind_code: Some("exceptional".into()),
            exceptional_substance_code: Some("73".into()),
            status: None,
            valid_from: Some("2026-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    let mut new = treatment(&fx, "2026-05-01");
    new.product_id = Some(product_id);
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let p = &treatment_activities(&doc)[0]["ProductosFito"][0];
    // TIPO_PRODFITO 4 = autorización excepcional; MateriaActiva is mandatory
    // exactly there (AUTORIZACION_EXCP catalogue code).
    assert_eq!(p["TipoProducto"], 4);
    assert_eq!(p["MateriaActiva"], 73);
    assert_schema_valid(&doc);
}

#[test]
fn applicator_equipment_emits_exactly_one_identifier() {
    let mut conn = db();
    let fx = fixture(&mut conn);

    // Manual application: no machinery on the record. The schema's oneOf
    // still demands an equipment identifier, so a fixed sentinel is emitted.
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-04-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    // Machinery registered in both ROMA and REGANIP: ROMA wins ("nunca ambos").
    let both = insert_machinery(&mut conn, &fx.farm_id, Some("47-00123"), Some("RG-9"));
    let mut with_both = treatment(&fx, "2026-04-02");
    with_both.machinery_id = Some(both);
    repo::insert_treatment_record(
        &mut conn,
        with_both,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    // Machinery in REGANIP only: aircraft and fixed/semi-mobile installations,
    // which is the whole reason both columns exist — ROMA registers the mobile
    // sprayer, REGANIP does not overlap with it.
    let aerial = insert_machinery(&mut conn, &fx.farm_id, None, Some("RG-4471"));
    let mut with_reganip = treatment(&fx, "2026-04-03");
    with_reganip.machinery_id = Some(aerial);
    repo::insert_treatment_record(
        &mut conn,
        with_reganip,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    // Machinery in neither registry: identified by IdEquipoAplicador.
    let unregistered = insert_machinery(&mut conn, &fx.farm_id, None, None);
    let mut with_unregistered = treatment(&fx, "2026-04-04");
    with_unregistered.machinery_id = Some(unregistered.clone());
    repo::insert_treatment_record(
        &mut conn,
        with_unregistered,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let ts = treatment_activities(&doc);
    assert_eq!(ts.len(), 4);

    let equipment = |i: usize| &ts[i]["IdentificadorAplicador"][0]["EquipoAplicador"];
    let manual = equipment(0);
    assert_eq!(manual["AplicacionManual"], true);
    assert_eq!(manual["IdEquipoAplicador"], "manual");
    assert!(manual.get("NumROMA").is_none() && manual.get("NumREGANIP").is_none());

    let roma = equipment(1);
    assert_eq!(roma["AplicacionManual"], false);
    assert_eq!(roma["NumROMA"], "47-00123");
    assert!(roma.get("NumREGANIP").is_none() && roma.get("IdEquipoAplicador").is_none());

    let reganip = equipment(2);
    assert_eq!(reganip["AplicacionManual"], false);
    assert_eq!(reganip["NumREGANIP"], "RG-4471");
    assert!(reganip.get("NumROMA").is_none() && reganip.get("IdEquipoAplicador").is_none());

    let free = equipment(3);
    assert_eq!(free["AplicacionManual"], false);
    assert_eq!(free["IdEquipoAplicador"], unregistered.as_str());
    assert!(free.get("NumROMA").is_none() && free.get("NumREGANIP").is_none());

    for t in ts {
        assert_eq!(
            t["IdentificadorAplicador"][0]["AplicadorEmpresa"]["NumROPO"],
            "ROPO-4700123"
        );
    }
    assert_schema_valid(&doc);
}

// ---------------------------------------------------------------------------
// Splits and aliases
// ---------------------------------------------------------------------------

#[test]
fn multi_crop_treatments_split_into_one_tratamfito_per_crop() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5),
            on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0),
        ],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let ts = treatment_activities(&doc);
    // 3.11.4 descriptor rule: all DGCs in one TratamFito share the crop, so
    // the record splits — each half carries its own frozen integer alias and
    // exactly its crop's plots.
    assert_eq!(ts.len(), 2);
    let aliases: Vec<i64> = ts
        .iter()
        .map(|t| t["IdAjenaTratamFito"].as_i64().unwrap())
        .collect();
    assert_ne!(aliases[0], aliases[1]);
    for t in ts {
        assert_eq!(t["DGCs"].as_array().unwrap().len(), 1);
    }
    let surfaces: Vec<f64> = ts
        .iter()
        .map(|t| t["DGCs"][0]["Superficie"].as_f64().unwrap())
        .collect();
    assert_eq!(surfaces.len(), 2);
    assert!(surfaces.contains(&2.5) && surfaces.contains(&3.0));
    assert_schema_valid(&doc);
}

#[test]
fn re_exporting_reuses_every_alias() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![
            on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 2.5),
            on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0),
        ],
        None,
    )
    .unwrap();

    let first = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let second = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    // SIEX keys edits/deletes on the alias: a re-export must be identical,
    // never renumbered.
    assert_eq!(first, second);
}

#[test]
fn dgc_aliases_are_shared_across_treatments_of_the_same_crop() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    for date in ["2026-04-01", "2026-05-01"] {
        repo::insert_treatment_record(
            &mut conn,
            treatment(&fx, date),
            vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
            None,
        )
        .unwrap();
    }

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let ts = treatment_activities(&doc);
    assert_eq!(ts.len(), 2);
    // A DGC is the plot+crop+season unit — both treatments reference the SAME
    // ajena code, so the authority sees one crop unit, not two.
    assert_eq!(
        ts[0]["DGCs"][0]["CodigoDGCAjena"],
        ts[1]["DGCs"][0]["CodigoDGCAjena"]
    );
    // But the activities themselves have distinct aliases.
    assert_ne!(ts[0]["IdAjenaTratamFito"], ts[1]["IdAjenaTratamFito"]);
}

// ---------------------------------------------------------------------------
// Deletions
// ---------------------------------------------------------------------------

#[test]
fn deleted_records_export_a_deletion_entry_only_if_previously_exported() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let exported = repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-04-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let first = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let alias = treatment_activities(&first)[0]["IdAjenaTratamFito"]
        .as_i64()
        .unwrap();

    // A record deleted before it was ever exported must simply vanish…
    let never_exported = repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();
    repo::soft_delete_treatment_record(&mut conn, &never_exported.id, None).unwrap();
    // …while the exported one becomes a deletion entry under its frozen alias.
    repo::soft_delete_treatment_record(&mut conn, &exported.id, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let ts = treatment_activities(&doc);
    assert_eq!(ts.len(), 1, "the never-exported deletion leaves no trace");
    assert_eq!(ts[0]["IdAjenaTratamFito"].as_i64().unwrap(), alias);
    assert_eq!(ts[0]["Borrar"], true);
    assert_schema_valid(&doc);
}

#[test]
fn active_entries_do_not_carry_the_deletion_flag() {
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
    assert!(treatment_activities(&doc)[0].get("Borrar").is_none());
}

// ---------------------------------------------------------------------------
// Precheck
// ---------------------------------------------------------------------------

#[test]
fn precheck_is_clean_on_an_export_ready_farm() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-01"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.is_clean(), "{check:?}");
}

#[test]
fn precheck_lists_missing_farm_identity_fields() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bare_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Sin papeles".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &bare_farm.id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(
        check.farm_missing_fields,
        vec!["owner_tax_id", "rea_code", "province_code"]
    );
}

#[test]
fn precheck_flags_an_unmappable_province() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Provincia rara".into(),
            owner_name: None,
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("ES249900000001".into()),
                siex_code: None,
                province_code: Some("99".into()), // no such INE province
            }),
        },
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &farm.id).unwrap();
    assert_eq!(check.farm_missing_fields, vec!["province_code"]);
}

#[test]
fn precheck_flags_a_malformed_rea_code() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "REA corto".into(),
            owner_name: None,
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                // Present but not the national 14-character code the schema
                // demands (minLength = maxLength = 14).
                rea_code: Some("12345".into()),
                siex_code: None,
                province_code: Some("47".into()),
            }),
        },
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &farm.id).unwrap();
    assert_eq!(check.farm_missing_fields, vec!["rea_code"]);
}

#[test]
fn precheck_lists_records_missing_efficacy_or_licence_and_plots_missing_crop() {
    let mut conn = db();
    let fx = fixture(&mut conn);

    let mut no_efficacy = treatment(&fx, "2026-04-01");
    no_efficacy.efficacy_code = None;
    let no_efficacy = repo::insert_treatment_record(
        &mut conn,
        no_efficacy,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let unlicensed = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Sin carnet".into(),
            tax_id: None,
            licence_number: None,
            licence_level_code: None,
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap()
    .id;
    let mut bad_operator = treatment(&fx, "2026-04-02");
    bad_operator.operator_id = unlicensed;
    let bad_operator = repo::insert_treatment_record(
        &mut conn,
        bad_operator,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let no_crop = repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-04-03"),
        vec![on_plot(&fx.barley_plot_id, None, 3.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.records_missing_efficacy.len(), 1);
    assert_eq!(
        check.records_missing_efficacy[0].treatment_record_id,
        no_efficacy.id
    );
    assert_eq!(
        check.records_missing_efficacy[0].product_name.as_deref(),
        Some("Fungitop")
    );
    assert_eq!(check.records_missing_operator_licence.len(), 1);
    assert_eq!(
        check.records_missing_operator_licence[0].treatment_record_id,
        bad_operator.id
    );
    assert_eq!(check.plots_missing_crop.len(), 1);
    assert_eq!(check.plots_missing_crop[0].treatment_record_id, no_crop.id);
    assert_eq!(check.plots_missing_crop[0].plot_name, "La Loma");
}

#[test]
fn precheck_ignores_deleted_records() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut no_efficacy = treatment(&fx, "2026-04-01");
    no_efficacy.efficacy_code = None;
    let record = repo::insert_treatment_record(
        &mut conn,
        no_efficacy,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();
    repo::soft_delete_treatment_record(&mut conn, &record.id, None).unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(
        check.is_clean(),
        "a deleted record cannot demand fixes: {check:?}"
    );
}
