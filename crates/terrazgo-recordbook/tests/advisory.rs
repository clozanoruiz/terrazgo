// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The book completeness advisory: what the record book is missing, reported
//! and never enforced.
//!
//! The rules pinned here come from the two decrees the book answers to —
//! RD 1311/2012 Anexo III Parte I (A.1's identity, B.e's crop, B.j's efficacy)
//! and RD 1051/2022 art. 4-5 (the fertilisation and irrigation duty, and the
//! exemption that may excuse it). Every finding is advisory: `report.rs`
//! forbids a gate on the printed book, because a farmer must be able to print
//! for an inspection while some registry data is still incomplete.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;
use terrazgo_core::models::{NewGeoFeature, PlotEsFields};
use terrazgo_recordbook::advisory::Duty;
use terrazgo_recordbook::{book_advisory, open_in_memory};

struct Fixture {
    season_id: String,
    farm_id: String,
    operator_id: String,
    product_id: String,
}

/// A complete holding: identity filled, one licensed applicator, one authorised
/// product. Tests take things away from it.
fn fixture(conn: &mut Connection) -> Fixture {
    let season = repo::insert_season(
        conn,
        NewSeason {
            campaign_year: 2026,
            label: "2025/2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let farm = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    // The postal address is A.1.a and is not on the create form.
    let detail = terrazgo_core::repository::get_farm(conn, &farm.id).unwrap();
    terrazgo_core::repository::update_farm(
        conn,
        &farm.id,
        terrazgo_core::models::UpdateFarm {
            name: detail.farm.name.clone(),
            owner_name: detail.farm.owner_name.clone(),
            owner_tax_id: detail.farm.owner_tax_id.clone(),
            location_text: None,
            address: Some("Camino de la Vega, 1".into()),
            postal_code: None,
            phone_fixed: None,
            phone_mobile: None,
            email: None,
            opened_on: None,
            latitude: None,
            longitude: None,
            country_code: detail.farm.country_code.clone(),
            es: None,
            representative: None,
        },
        None,
    )
    .unwrap();

    let operator_id = repo::insert_operator(
        conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: None,
            licence_number: Some("CYL-2018-04567".into()),
            licence_level_code: Some("qualified".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap()
    .id;
    let product_id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: "Fungitop".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: Some(21),
        },
        None,
    )
    .unwrap()
    .id;
    repo::add_product_authorisation(
        conn,
        NewProductAuthorisation {
            product_id: product_id.clone(),
            country_code: "es".into(),
            authorisation_number: "ES-25.123".into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: Some("2024-01-01".into()),
            valid_until: None,
        },
        None,
    )
    .unwrap();

    Fixture {
        season_id: season.id,
        farm_id: farm.id,
        operator_id,
        product_id,
    }
}

/// A plot with a SIGPAC boundary, which is the only thing that gives the
/// advisory a land use to measure the exemption with.
fn insert_verified_plot(
    conn: &mut Connection,
    farm_id: &str,
    name: &str,
    area_ha: f64,
    land_use: &str,
) -> String {
    let plot_id = repo::insert_plot(
        conn,
        NewPlot {
            farm_id: farm_id.into(),
            name: name.into(),
            area_ha: Some(area_ha),
            es: Some(PlotEsFields {
                sigpac_province: Some("47".into()),
                sigpac_municipality: Some("182".into()),
                sigpac_aggregate: Some("0".into()),
                sigpac_zone: Some("0".into()),
                sigpac_polygon: Some("7".into()),
                sigpac_parcel: Some("14".into()),
                sigpac_enclosure: Some("1".into()),
            }),
        },
        None,
    )
    .unwrap()
    .id;
    terrazgo_core::repository::save_geo_feature(
        conn,
        NewGeoFeature {
            plot_id: Some(plot_id.clone()),
            farm_id: None,
            role: "boundary".into(),
            source: "sigpac".into(),
            geometry: r#"{"type":"Polygon","coordinates":[[[-4.72,41.65],[-4.71,41.65],[-4.71,41.66],[-4.72,41.65]]]}"#.into(),
            properties: Some(format!(r#"{{"uso_sigpac":"{land_use}"}}"#)),
            official_area_ha: Some(area_ha),
            campaign: Some(2026),
            fetched_at: None,
        },
        None,
    )
    .unwrap();
    plot_id
}

fn insert_crop(
    conn: &mut Connection,
    plot_id: &str,
    season_id: &str,
    irrigation: Option<&str>,
) -> String {
    repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_id.into(),
            season_id: season_id.into(),
            species_name: "trigo blando".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: irrigation.map(Into::into),
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap()
    .id
}

fn treatment(
    fx: &Fixture,
    plot_id: &str,
    crop_id: Option<&str>,
) -> (NewTreatmentRecord, Vec<NewTreatmentPlot>) {
    (
        NewTreatmentRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            application_date: "2026-05-01".into(),
            application_end_date: None,
            application_time: None,
            product_id: Some(fx.product_id.clone()),
            country_code: None,
            dose_value: Some(1.0),
            dose_unit_code: Some("l_ha".into()),
            total_quantity_value: None,
            total_quantity_unit_code: None,
            problems: vec![NewTreatmentProblem {
                reason_category_code: "disease".into(),
                problem_code: "1".into(),
            }],
            justifications: vec!["monitoring".into()],
            efficacy_code: Some("good".into()),
            target_organism: None,
            operator_id: fx.operator_id.clone(),
            machinery_id: None,
            advisor_id: None,
            measure_code: None,
            measure_intensity_value: None,
            measure_intensity_unit_code: None,
            measure_registration_number: None,
            phi_days_used: Some(14),
            notes: None,
        },
        vec![NewTreatmentPlot {
            plot_id: plot_id.into(),
            crop_id: crop_id.map(Into::into),
            surface_treated_ha: 3.0,
            growth_stage_code: None,
        }],
    )
}

#[test]
fn a_complete_small_holding_reports_only_what_it_cannot_know() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "La Vega", 3.0, "TA");
    let crop = insert_crop(&mut conn, &plot, &fx.season_id, Some("rainfed"));
    let (record, plots) = treatment(&fx, &plot, Some(&crop));
    repo::insert_treatment_record(&mut conn, record, plots, None).unwrap();

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(advisory.farm_missing_fields.is_empty());
    assert!(advisory.treatments_missing_crop.is_empty());
    assert!(advisory.treatments_missing_efficacy.is_empty());
    assert!(advisory.operators_missing_licence.is_empty());
    // Nothing has been said about the four conditional registers yet, and the
    // second decree's two sections hold nothing — all five are reported.
    assert_eq!(advisory.registers_undeclared.len(), 4);
    assert_eq!(
        advisory.fertilisation_absent.as_ref().map(|gap| gap.duty),
        Some(Duty::PossiblyExempt)
    );
    assert_eq!(
        advisory.irrigation_absent.as_ref().map(|gap| gap.duty),
        Some(Duty::PossiblyExempt)
    );
    assert!(!advisory.is_clean());
}

#[test]
fn missing_identity_fields_are_named_one_by_one() {
    // Anexo III Parte I A.1.a-b. They print blank in a binding section, which
    // is exactly what an advisory exists to point at.
    let mut conn = open_in_memory().unwrap();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Sin datos".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    let season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2026,
            label: "2025/2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let advisory = book_advisory(&conn, &season.id, &farm.id).unwrap();
    assert_eq!(
        advisory.farm_missing_fields,
        vec!["address", "owner_name", "owner_tax_id"]
    );
}

#[test]
fn a_treated_plot_without_a_crop_is_reported_with_its_plot_name() {
    // Anexo III Parte I B.e: "cultivo, indicando especie y variedad".
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "El Páramo", 2.0, "TA");
    let (record, plots) = treatment(&fx, &plot, None);
    repo::insert_treatment_record(&mut conn, record, plots, None).unwrap();

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(advisory.treatments_missing_crop.len(), 1);
    assert_eq!(advisory.treatments_missing_crop[0].plot_name, "El Páramo");
    assert_eq!(
        advisory.treatments_missing_crop[0].application_date,
        "2026-05-01"
    );
}

#[test]
fn an_unassessed_efficacy_is_reported_and_a_recorded_one_is_not() {
    // B.j is binding, but efficacy is observed AFTER the application — which is
    // why this is advisory here and refused at export instead.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "La Vega", 2.0, "TA");
    let crop = insert_crop(&mut conn, &plot, &fx.season_id, None);
    let (mut record, plots) = treatment(&fx, &plot, Some(&crop));
    record.efficacy_code = None;
    let stored = repo::insert_treatment_record(&mut conn, record, plots, None).unwrap();

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(advisory.treatments_missing_efficacy.len(), 1);
    assert_eq!(
        advisory.treatments_missing_efficacy[0]
            .product_name
            .as_deref(),
        Some("Fungitop")
    );

    repo::set_treatment_efficacy(&mut conn, &stored.id, Some("good".into()), None).unwrap();
    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(advisory.treatments_missing_efficacy.is_empty());
}

#[test]
fn an_applicator_without_a_licence_is_reported_once_however_many_records() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    conn.execute(
        "UPDATE operator SET licence_number = NULL WHERE id = ?1",
        [&fx.operator_id],
    )
    .unwrap();
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "La Vega", 2.0, "TA");
    let crop = insert_crop(&mut conn, &plot, &fx.season_id, None);
    for _ in 0..3 {
        let (record, plots) = treatment(&fx, &plot, Some(&crop));
        repo::insert_treatment_record(&mut conn, record, plots, None).unwrap();
    }

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(advisory.operators_missing_licence.len(), 1);
    assert_eq!(
        advisory.operators_missing_licence[0].full_name,
        "Carlos Pérez"
    );
}

#[test]
fn a_register_answers_with_rows_or_with_a_stated_no() {
    // Three states, one finding: silence. A register that holds records has
    // answered, and so has one whose "APLICA TRATAMIENTO: NO" is stored.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "La Vega", 2.0, "TA");

    let before = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(before.registers_undeclared.len(), 4);

    repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "transport",
        "2026-09-01",
        None,
    )
    .unwrap();
    repo::insert_seed_treatment(
        &mut conn,
        NewSeedTreatment {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            sown_on: "2025-10-20".into(),
            species_name: "trigo blando".into(),
            variety: None,
            crop_code: None,
            seed_quantity_kg: None,
            seed_lot: None,
            treatment_kind_code: None,
            product_name: "Celest Trio".into(),
            product_registration_number: None,
            product_active_substance: None,
            product_id: None,
            efficacy_code: None,
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: plot,
                surface_sown_ha: 2.0,
            }],
        },
        None,
    )
    .unwrap();

    let after = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(
        after.registers_undeclared,
        vec!["postharvest".to_string(), "storage_premises".to_string()]
    );
}

#[test]
fn the_second_decrees_sections_are_reported_only_while_empty() {
    // RD 1051/2022 art. 5.d and 5.e, in force since 1 Jan 2026.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let plot = insert_verified_plot(&mut conn, &fx.farm_id, "La Vega", 40.0, "TA");
    let crop = insert_crop(&mut conn, &plot, &fx.season_id, Some("drip"));

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let gap = advisory.fertilisation_absent.as_ref().unwrap();
    // 40 ha of arable, all of it irrigated: nothing about this holding is
    // exempt.
    assert_eq!(gap.duty, Duty::Binding);
    assert_eq!(gap.arable_permanent_ha, 40.0);
    assert_eq!(gap.irrigated_ha, 40.0);

    module_fertilisation::repository::insert_irrigation_record(
        &mut conn,
        module_fertilisation::models::NewIrrigationRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            irrigated_on: "2026-06-14".into(),
            irrigation_end_date: None,
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: None,
            water_soluble_p2o5_mg_l: None,
            energy_type_code: None,
            meter_number: None,
            notes: None,
            plots: vec![module_fertilisation::models::NewIrrigationPlot {
                plot_id: plot.clone(),
                crop_id: Some(crop.clone()),
                irrigated_area_ha: Some(40.0),
            }],
            water_origins: vec!["groundwater".into()],
        },
        None,
    )
    .unwrap();

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    // Section 8 has answered; section 6 still has not.
    assert!(advisory.irrigation_absent.is_none());
    assert!(advisory.fertilisation_absent.is_some());
}

#[test]
fn a_plot_never_verified_leaves_the_exemption_undetermined() {
    // No SIGPAC boundary, so no land use — the advisory says it cannot judge
    // rather than excusing a holding it cannot measure.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Sin verificar".into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap();

    let advisory = book_advisory(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let gap = advisory.fertilisation_absent.as_ref().unwrap();
    assert_eq!(gap.duty, Duty::Undetermined);
    assert_eq!(gap.plots_without_land_use, 1);
}
