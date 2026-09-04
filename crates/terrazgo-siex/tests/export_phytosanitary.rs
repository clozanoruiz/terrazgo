// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! `TratamFito` and its three sub-blocks: `ProductosFito` (the spray),
//! `OtrasActuacionesFito` (the non-chemical measure, which Anexo V grades
//! *excluyente* with the spray) and `AsesorValidacion` (the advisor), plus the
//! drying date `FechaSeca`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use terrazgo_siex::{SiexError, build_cuaderno, export_precheck};

// ---------------------------------------------------------------------------
// `TratamFito`'s three sub-blocks (seam 5): the non-chemical measure, the
// advisor, and the drying date.
// ---------------------------------------------------------------------------

/// A purely non-chemical actuation ready to insert: no product, a measure and
/// its intensity. `TIPO_MEDIDA_FITOSANITARIA` 12 is "Captura masiva a base de
/// trampas luminosas" — chosen because it is NOT one of the three kinds Anexo V
/// demands an MDF number for, so the intensity rules can be exercised alone.
fn non_chemical(fx: &Fixture, application_date: &str) -> NewTreatmentRecord {
    let mut record = treatment(fx, application_date);
    record.product_id = None;
    record.dose_value = None;
    record.dose_unit_code = None;
    record.measure_code = Some("12".into());
    record.measure_intensity_value = Some(24.0);
    record.measure_intensity_unit_code = Some("traps".into());
    record
}

/// The seam's headline: an actuation that applied no product is a first-class
/// `TratamFito`. `TratamFito`'s required set is `IdAjenaTratamFito`,
/// `FechaInicio`, `FechaFin`, `DGCs`, `ProblematicaFito`, `Justificaciones`,
/// `IdentificadorAplicador` and `Eficacia` — it pointedly omits `ProductosFito`,
/// which is what makes this shape schema-valid.
#[test]
fn a_purely_non_chemical_actuation_exports_as_its_own_treatment() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        non_chemical(&fx, "2026-05-04"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.is_clean(), "{check:?}");

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &treatment_activities(&doc)[0];
    let measure = &entry["OtrasActuacionesFito"];
    assert_eq!(measure["TipoMedida"], 12);
    assert_eq!(measure["Cantidad"], 24.0);
    // UNIDADES_MEDIDA 27 = "Trampas". The exact count's unit travels, not the
    // generic "Unidades" (11) — dropping WHAT was counted would lose the one
    // thing model 3.1 bis asks for by name.
    assert_eq!(measure["Unidad"], 27);
    assert!(measure.get("NumRegistroMDF").is_none());
    // Anexo V: the two sub-blocks are "excluyente". The member is omitted
    // rather than sent empty, so the entry states one thing, not two.
    assert!(
        entry.get("ProductosFito").is_none(),
        "a measure-only entry must not carry ProductosFito: {entry}"
    );
}

/// The mixed record — a spray AND a measure on one row, which the register
/// allows because model 3.1 bis prints both column groups side by side.
///
/// Anexo V grades all five members of `OtrasActuacionesFito` "excluyente con el
/// subbloque siguiente de «Productos fitosanitarios»", and the decree agrees
/// from the other side: RD 1311/2012 Anexo III Parte I B opens "Para cada
/// tratamiento… especificar la información siguiente" and lists no non-chemical
/// member at all, so one row carrying both is one row carrying two treatments.
#[test]
fn precheck_refuses_a_record_that_carries_both_a_product_and_a_measure() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut mixed = treatment(&fx, "2026-05-06");
    mixed.measure_code = Some("12".into());
    mixed.measure_intensity_value = Some(24.0);
    mixed.measure_intensity_unit_code = Some("traps".into());
    let record = repo::insert_treatment_record(
        &mut conn,
        mixed,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean(), "a dropped half must block: {check:?}");
    assert_eq!(check.records_mixing_product_and_measure.len(), 1);
    let listed = &check.records_mixing_product_and_measure[0];
    assert_eq!(listed.treatment_record_id, record.id);
    assert_eq!(listed.product_name.as_deref(), Some("Fungitop"));
    // Only the mixing blocks it: the measure itself is perfectly sendable.
    assert!(check.records_with_unsendable_measure.is_empty());

    let err = build_cuaderno(&mut conn, &fx.season_id, &fx.farm_id, None).unwrap_err();
    assert!(
        matches!(err, SiexError::Invalid("export_precheck_failed")),
        "got {err:?}"
    );
}

/// Anexo V grades `Cantidad` (field 17) and `Unidad` (field 18) Obligatorio
/// inside the block, while the register keeps the pair nullable — a farmer may
/// record that traps were hung before counting them, and the book prints that
/// without complaint. The JSON Schema requires `TipoMedida` alone, so this is
/// the grading deciding over `required`: the seam-4 cover-widths case again.
#[test]
fn precheck_refuses_a_measure_whose_intensity_was_never_stated() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut uncounted = non_chemical(&fx, "2026-05-08");
    uncounted.measure_intensity_value = None;
    uncounted.measure_intensity_unit_code = None;
    let record = repo::insert_treatment_record(
        &mut conn,
        uncounted,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.records_with_unsendable_measure.len(), 1);
    assert_eq!(
        check.records_with_unsendable_measure[0].treatment_record_id,
        record.id
    );
    // The record applied no product, so it is not the mixed case.
    assert!(check.records_mixing_product_and_measure.is_empty());
}

/// Anexo V field 19 grades `Registro MDF` Obligatorio for "suelta de OCB,
/// trampas y otros y feromonas y atrayentes para monitoreo" —
/// `TIPO_MEDIDA_FITOSANITARIA` 1, 14 and 15.
#[test]
fn precheck_demands_an_mdf_number_only_for_the_kinds_anexo_v_names() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    // 15 = "Feromonas y atrayentes para monitoreo": in scope, and unregistered.
    let mut pheromones = non_chemical(&fx, "2026-05-09");
    pheromones.measure_code = Some("15".into());
    pheromones.measure_intensity_unit_code = Some("diffusers".into());
    let demanded = repo::insert_treatment_record(
        &mut conn,
        pheromones,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();
    // 12 = "Captura masiva a base de trampas luminosas": out of scope, so its
    // silence is not a gap.
    repo::insert_treatment_record(
        &mut conn,
        non_chemical(&fx, "2026-05-10"),
        vec![on_plot(&fx.barley_plot_id, Some(&fx.barley_crop_id), 3.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(check.records_missing_measure_registration.len(), 1);
    assert_eq!(
        check.records_missing_measure_registration[0].treatment_record_id,
        demanded.id
    );
}

/// The other half of Anexo V field 19 — "en caso contrario el campo debe ir
/// vacío" — is deliberately NOT enforced: Anexo VI names a different set of
/// kinds, no decree names the field at all, and honouring either list would
/// silently discard a number the farmer recorded.
#[test]
fn an_mdf_number_travels_whatever_the_measure_kind() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut registered = non_chemical(&fx, "2026-05-11"); // kind 12, out of scope
    registered.measure_registration_number = Some("MDF-00871".into());
    repo::insert_treatment_record(
        &mut conn,
        registered,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(
        treatment_activities(&doc)[0]["OtrasActuacionesFito"]["NumRegistroMDF"],
        "MDF-00871"
    );
}

/// The counterpart, so the refusals cannot creep: an ordinary spray with no
/// measure stays exportable and carries none of the new members.
#[test]
fn precheck_leaves_an_ordinary_product_application_exportable() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-05-07"),
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.records_mixing_product_and_measure.is_empty());
    assert!(check.records_with_unsendable_measure.is_empty());
    assert!(check.records_missing_measure_registration.is_empty());
    assert!(check.records_missing_advisor_ropo.is_empty());
    assert!(check.is_clean(), "{check:?}");

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let entry = &treatment_activities(&doc)[0];
    assert!(entry.get("OtrasActuacionesFito").is_none());
    assert!(entry.get("AsesorValidacion").is_none());
    assert!(entry.get("FechaSeca").is_none());
    assert_eq!(entry["ProductosFito"].as_array().unwrap().len(), 1);
}

/// Anexo III Parte I B.d asks for "Identificación del aplicador **y, en su
/// caso, del asesor**" — one sentence, two identifications, which is why the
/// advisor is a field on the treatment and `AsesorValidacion` hangs off
/// `TratamFito`. Only `NumROPO` is sent: `Validacion`, `Confirmacion`,
/// `Contrato`, `Fecha` and `Observaciones` describe a sign-off model 3.1 bis
/// collects as a handwritten signature, which this app has no way to hold.
#[test]
fn a_named_advisor_reaches_the_descriptor_as_a_ropo_number_and_nothing_else() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "Agroasesores del Duero SL".into(),
            tax_id: Some("B47123456".into()),
            registration_number: Some("ROPO-AS-47-0912".into()),
        },
        None,
    )
    .unwrap();
    let mut advised = treatment(&fx, "2026-05-12");
    advised.advisor_id = Some(advisor.id.clone());
    repo::insert_treatment_record(
        &mut conn,
        advised,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &treatment_activities(&doc)[0];
    assert_eq!(entry["AsesorValidacion"]["NumROPO"], "ROPO-AS-47-0912");
    for unattestable in ["Validacion", "Confirmacion", "Contrato", "Fecha"] {
        assert!(
            entry["AsesorValidacion"].get(unattestable).is_none(),
            "{unattestable} claims a sign-off the book cannot hold"
        );
    }
}

/// Anexo V grades the advisor's ROPO (field 50) Obligatorio on this block,
/// where blocks 1.2 and 1.3 grade the same field Voluntario — which is why the
/// three non-field registers omit the block and this one refuses. `NumROPO` is
/// the block's only carriable member, so omitting it would drop B.d's
/// identification with nothing on screen saying so.
#[test]
fn precheck_refuses_a_record_naming_an_advisor_with_no_ropo_number() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let advisor = terrazgo_core::repository::insert_advisor(
        &mut conn,
        terrazgo_core::models::NewAdvisor {
            name: "Cooperativa del Cerrato".into(),
            tax_id: None,
            registration_number: None,
        },
        None,
    )
    .unwrap();
    let mut advised = treatment(&fx, "2026-05-13");
    advised.advisor_id = Some(advisor.id.clone());
    let record = repo::insert_treatment_record(
        &mut conn,
        advised,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(!check.is_clean());
    assert_eq!(check.records_missing_advisor_ropo.len(), 1);
    assert_eq!(
        check.records_missing_advisor_ropo[0].treatment_record_id,
        record.id
    );
}

/// `FechaSeca`: model 9.3's fourth column and one of RD 1048/2022 art. 45.2's
/// five dates. It sits on the treatment because Anexo V reads "fecha en la que
/// se realiza el secado **para la realización del tratamiento**" — the field is
/// dried in order to spray it.
#[test]
fn a_drying_date_travels_as_fecha_seca() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut dried = treatment(&fx, "2026-06-02");
    dried.drying_date = Some("2026-05-28".into());
    repo::insert_treatment_record(
        &mut conn,
        dried,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(treatment_activities(&doc)[0]["FechaSeca"], "28/05/2026");
}

/// Anexo V grades `FechaSeca` Obligatorio "cuando se trate de cultivos bajo
/// agua", and the export deliberately does not gate on it. The condition is not
/// that the crop is flooded but that the field was dried FOR this treatment,
/// and a rice herbicide applied on water is a lawful record with no drying date
/// to state — so a gate keyed on `sowing_record.flooded_on` would refuse
/// records the decree permits.
#[test]
fn a_treatment_on_a_flooded_crop_needs_no_drying_date() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_sowing_record(
        &mut conn,
        terrazgo_core::models::NewSowingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-20".into(),
            sowing_end_date: None,
            flooded_on: Some("2026-05-15".into()),
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![terrazgo_core::models::NewSowingPlot {
                plot_id: fx.wheat_plot_id.clone(),
                crop_id: Some(fx.wheat_crop_id.clone()),
            }],
        },
        None,
    )
    .unwrap();
    repo::insert_treatment_record(
        &mut conn,
        treatment(&fx, "2026-06-05"), // no drying_date
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let check = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(check.is_clean(), "{check:?}");
    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert!(treatment_activities(&doc)[0].get("FechaSeca").is_none());
}

#[test]
fn build_refuses_while_the_precheck_is_not_clean() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let mut no_efficacy = treatment(&fx, "2026-05-01");
    no_efficacy.efficacy_code = None;
    repo::insert_treatment_record(
        &mut conn,
        no_efficacy,
        vec![on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)],
        None,
    )
    .unwrap();

    let err = build_cuaderno(&mut conn, &fx.season_id, &fx.farm_id, None).unwrap_err();
    assert!(
        matches!(err, SiexError::Invalid("export_precheck_failed")),
        "got {err:?}"
    );
}

/// Reglamento (UE) 2023/564's two conditional fields on the one twin the
/// serializer already emits. Both are optional in the format, so an ordinary
/// record omits them entirely rather than sending a placeholder.
#[test]
fn the_annex_fields_serialize_in_the_shapes_the_format_wants() {
    let mut conn = db_with_catalogues();
    let fx = fixture(&mut conn);

    let mut new = treatment(&fx, "2026-05-01");
    new.application_time = Some("20:30".into());
    repo::insert_treatment_record(
        &mut conn,
        new,
        vec![NewTreatmentPlot {
            growth_stage_code: Some("6".into()),
            ..on_plot(&fx.wheat_plot_id, Some(&fx.wheat_crop_id), 4.0)
        }],
        None,
    )
    .unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let activity = &treatment_activities(&doc)[0];

    // Anexo VI types the hour string(8) as HH:MM:SS, so the stored HH:MM is
    // padded at export — never stored padded, a farmer records no seconds.
    assert_eq!(activity["HoraTratamiento"], "20:30:00");
    // An INTEGER, and the catalogue's own code rather than the BBCH stage the
    // book prints: the schema validates EstadoFenologico against EST_FENOLOGICO.
    assert_eq!(activity["DGCs"][0]["EstadoFenologico"], 6);
}

#[test]
fn a_record_stating_neither_annex_field_omits_both_keys() {
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
    let activity = &treatment_activities(&doc)[0];
    // Absent, not "" and not 0 — the schema's own defaults would read as
    // midnight and as a stage the crop was never at.
    assert!(activity.get("HoraTratamiento").is_none());
    assert!(activity["DGCs"][0].get("EstadoFenologico").is_none());
}
