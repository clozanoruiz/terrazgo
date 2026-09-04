// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Model section 3.2 — sowings made with seed the supplier already treated.
//!
//! Two things make this register different from every other treatment record,
//! and both are pinned here. The product is captured as FREE TEXT, because a
//! sack of treated seed names a product the farmer never bought as such and
//! demanding a registry row first would block a lawful record. And the record
//! is CORRECTABLE in full, because none of it is a snapshot of another row —
//! there is nothing a later edit elsewhere could silently rewrite.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::last_change;

use module_cue::models::*;
use module_cue::open_in_memory;
use module_cue::repository as repo;
use rusqlite::Connection;

struct Fixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
}

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
    let farm_id = repo::insert_farm(
        conn,
        NewFarm {
            name: "Finca La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let plot = |conn: &mut Connection, name: &str, area: f64| {
        repo::insert_plot(
            conn,
            NewPlot {
                farm_id: farm_id.clone(),
                name: name.into(),
                area_ha: Some(area),
                es: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let plot_a = plot(conn, "El Prado", 4.0);
    let plot_b = plot(conn, "La Loma", 3.0);

    Fixture {
        season_id: season.id,
        farm_id,
        plot_a,
        plot_b,
    }
}

fn sample(fx: &Fixture) -> NewSeedTreatment {
    NewSeedTreatment {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        sown_on: "2025-11-10".into(),
        species_name: "trigo blando".into(),
        variety: Some("Nogal".into()),
        crop_code: Some("1".into()),
        seed_quantity_kg: Some(680.0),
        seed_lot: Some("L-2025-4471".into()),
        treatment_kind_code: Some("purchased_es".into()),
        acquired_on: None,
        sowing_record_id: None,
        product_name: "Celest Trio".into(),
        product_registration_number: Some("ES-24.876".into()),
        product_active_substance: Some("fludioxonil + difenoconazol".into()),
        product_id: None,
        efficacy_code: None,
        notes: None,
        plots: vec![NewSeedTreatmentPlot {
            plot_id: fx.plot_a.clone(),
            surface_sown_ha: 3.2,
        }],
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// The product arrives as text off the sack's label, with no registry row
/// behind it — that is the ordinary case, not a degraded one.
#[test]
fn treated_seed_is_recorded_with_a_free_text_product() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    assert_eq!(saved.record.product_name, "Celest Trio");
    assert_eq!(
        saved.record.product_registration_number.as_deref(),
        Some("ES-24.876")
    );
    assert_eq!(saved.record.product_id, None, "no registry row is required");
    assert_eq!(saved.record.seed_lot.as_deref(), Some("L-2025-4471"));
    assert_eq!(saved.record.seed_quantity_kg, Some(680.0));
    assert_eq!(saved.plots.len(), 1);
    assert_eq!(saved.plots[0].surface_sown_ha, 3.2);
    // Efficacy is observed after emergence.
    assert_eq!(saved.record.efficacy_code, None);
}

/// When the product IS in the registry the link is kept, without displacing the
/// printed text: the label is what the register shows either way.
#[test]
fn an_optional_registry_link_does_not_replace_the_printed_text() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let product_id = repo::insert_product(
        &mut conn,
        NewProduct {
            commercial_name: "Celest Trio".into(),
            holder: None,
            formulation_type_code: None,
            default_phi_days: None,
        },
        None,
    )
    .unwrap()
    .id;

    let mut new = sample(&fx);
    new.product_id = Some(product_id.clone());
    let saved = repo::insert_seed_treatment(&mut conn, new, None).unwrap();
    assert_eq!(
        saved.record.product_id.as_deref(),
        Some(product_id.as_str())
    );
    assert_eq!(saved.record.product_name, "Celest Trio");
}

#[test]
fn a_sowing_needs_a_species_a_product_and_at_least_one_plot() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut blank_species = sample(&fx);
    blank_species.species_name = "  ".into();
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, blank_species, None).unwrap_err(),
        module_cue::CueError::Invalid("empty_name")
    ));

    let mut blank_product = sample(&fx);
    blank_product.product_name = "".into();
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, blank_product, None).unwrap_err(),
        module_cue::CueError::Invalid("empty_product_name")
    ));

    let mut no_plots = sample(&fx);
    no_plots.plots.clear();
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, no_plots, None).unwrap_err(),
        module_cue::CueError::Invalid("no_plots")
    ));
}

/// A sowing recorded on someone else's land would put the wrong holding's
/// ground in this book — the `treatment_record` rule.
#[test]
fn a_plot_on_another_farm_is_rejected() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    let foreign_plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: other_farm.id,
            name: "Ajena".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;

    let mut new = sample(&fx);
    new.plots = vec![NewSeedTreatmentPlot {
        plot_id: foreign_plot,
        surface_sown_ha: 1.0,
    }];
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, new, None).unwrap_err(),
        module_cue::CueError::PlotNotOnFarm { .. }
    ));
}

#[test]
fn a_sown_surface_must_be_positive() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut new = sample(&fx);
    new.plots[0].surface_sown_ha = 0.0;
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, new, None).unwrap_err(),
        module_cue::CueError::Invalid("nonpositive_area")
    ));
}

#[test]
fn the_seed_quantity_may_be_left_unstated_but_not_negative() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut unstated = sample(&fx);
    unstated.seed_quantity_kg = None;
    assert_eq!(
        repo::insert_seed_treatment(&mut conn, unstated, None)
            .unwrap()
            .record
            .seed_quantity_kg,
        None
    );

    let mut negative = sample(&fx);
    negative.seed_quantity_kg = Some(-1.0);
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, negative, None).unwrap_err(),
        module_cue::CueError::Invalid("invalid_seed_quantity")
    ));
}

// ---------------------------------------------------------------------------
// Correction
// ---------------------------------------------------------------------------

/// Unlike a treatment record, this one holds no snapshot of another row, so a
/// full-row correction is safe: nothing printed elsewhere depends on it.
#[test]
fn a_sowing_can_be_corrected_in_full_and_logs_both_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    let updated = repo::update_seed_treatment(
        &mut conn,
        &saved.record.id,
        UpdateSeedTreatment {
            sown_on: "2025-11-12".into(),
            species_name: "trigo duro".into(),
            variety: None,
            crop_code: Some("2".into()),
            seed_quantity_kg: Some(700.0),
            seed_lot: Some("L-2025-4472".into()),
            treatment_kind_code: Some("on_farm".into()),
            acquired_on: None,
            sowing_record_id: None,
            product_name: "Celest Trio Extra".into(),
            product_registration_number: None,
            product_active_substance: None,
            product_id: None,
            notes: Some("Lote corregido tras revisar el albarán.".into()),
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_a.clone(),
                surface_sown_ha: 3.4,
            }],
        },
        Some("carlos"),
    )
    .unwrap();

    assert_eq!(updated.record.sown_on, "2025-11-12");
    assert_eq!(updated.record.species_name, "trigo duro");
    assert_eq!(updated.record.variety, None, "a cleared field is cleared");
    assert_eq!(updated.plots[0].surface_sown_ha, 3.4);

    let (op, before, after) = last_change(&conn, "seed_treatment", &saved.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["species_name"], "trigo blando");
    assert_eq!(after["species_name"], "trigo duro");
}

/// The sown plots are reconciled from the submitted state — added, dropped and
/// changed rows each get their own audit entry, so the log stays rebuildable.
#[test]
fn correcting_the_sown_plots_reconciles_them_and_logs_each_change() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();
    let original_plot_row = saved.plots[0].id.clone();

    let updated = repo::update_seed_treatment(
        &mut conn,
        &saved.record.id,
        UpdateSeedTreatment {
            sown_on: saved.record.sown_on.clone(),
            species_name: saved.record.species_name.clone(),
            variety: saved.record.variety.clone(),
            crop_code: saved.record.crop_code.clone(),
            seed_quantity_kg: saved.record.seed_quantity_kg,
            seed_lot: saved.record.seed_lot.clone(),
            treatment_kind_code: saved.record.treatment_kind_code.clone(),
            acquired_on: None,
            sowing_record_id: None,
            product_name: saved.record.product_name.clone(),
            product_registration_number: saved.record.product_registration_number.clone(),
            product_active_substance: saved.record.product_active_substance.clone(),
            product_id: None,
            notes: None,
            // El Prado goes, La Loma arrives.
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_b.clone(),
                surface_sown_ha: 2.5,
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.plots.len(), 1);
    assert_eq!(updated.plots[0].plot_id, fx.plot_b);

    // The dropped row is hard-deleted (it is a pure child, like an extension
    // row) and logged with a null after-image.
    let (op, _, after) = last_change(&conn, "seed_treatment_plot", &original_plot_row);
    assert_eq!(op, "delete");
    assert!(after.is_null(), "a removed child logs a null after-image");
    // The new one is logged as its own insert.
    let (op, _, after) = last_change(&conn, "seed_treatment_plot", &updated.plots[0].id);
    assert_eq!(op, "insert");
    assert_eq!(after["surface_sown_ha"], 2.5);
}

/// Only the surface changed: the row keeps its identity rather than being
/// deleted and re-created, so the audit trail reads as a correction.
#[test]
fn changing_only_a_surface_updates_the_existing_plot_row() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();
    let row_id = saved.plots[0].id.clone();

    let updated = repo::update_seed_treatment(
        &mut conn,
        &saved.record.id,
        UpdateSeedTreatment {
            sown_on: saved.record.sown_on.clone(),
            species_name: saved.record.species_name.clone(),
            variety: saved.record.variety.clone(),
            crop_code: saved.record.crop_code.clone(),
            seed_quantity_kg: saved.record.seed_quantity_kg,
            seed_lot: saved.record.seed_lot.clone(),
            treatment_kind_code: saved.record.treatment_kind_code.clone(),
            acquired_on: None,
            sowing_record_id: None,
            product_name: saved.record.product_name.clone(),
            product_registration_number: saved.record.product_registration_number.clone(),
            product_active_substance: saved.record.product_active_substance.clone(),
            product_id: None,
            notes: None,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_a.clone(),
                surface_sown_ha: 3.9,
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.plots[0].id, row_id, "the same row, corrected");
    let (op, before, after) = last_change(&conn, "seed_treatment_plot", &row_id);
    assert_eq!(op, "update");
    assert_eq!(before["surface_sown_ha"], 3.2);
    assert_eq!(after["surface_sown_ha"], 3.9);
}

#[test]
fn efficacy_is_recorded_after_emergence_and_logged() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    let updated =
        repo::set_seed_treatment_efficacy(&mut conn, &saved.record.id, Some("good".into()), None)
            .unwrap();
    assert_eq!(updated.efficacy_code.as_deref(), Some("good"));

    let (op, before, after) = last_change(&conn, "seed_treatment", &saved.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["efficacy_code"], serde_json::Value::Null);
    assert_eq!(after["efficacy_code"], "good");
}

#[test]
fn soft_delete_hides_the_sowing_and_keeps_both_images() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    repo::soft_delete_seed_treatment(&mut conn, &saved.record.id, None).unwrap();
    assert!(
        repo::list_seed_treatments(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );

    let (op, before, after) = last_change(&conn, "seed_treatment", &saved.record.id);
    assert_eq!(op, "delete");
    assert_eq!(before["deleted_at"], serde_json::Value::Null);
    assert!(after["deleted_at"].is_string());
}

#[test]
fn every_inserted_row_is_logged_with_the_actor() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), Some("carlos")).unwrap();

    let (op, _, after) = last_change(&conn, "seed_treatment", &saved.record.id);
    assert_eq!(op, "insert");
    assert_eq!(after["seed_lot"], "L-2025-4471");
    assert_eq!(after["product_name"], "Celest Trio");

    let (_, _, plot) = last_change(&conn, "seed_treatment_plot", &saved.plots[0].id);
    assert_eq!(plot["surface_sown_ha"], 3.2);

    let actor: String = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1",
            [&saved.record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "carlos");
}

// ---------------------------------------------------------------------------
// The declaration guard now spans two tables
// ---------------------------------------------------------------------------

/// `register_declaration` is shared with the non-field registers, but the
/// `seed_treatment` register is backed by a DIFFERENT table. The guard has to
/// know that, or a farmer could declare "no treated seed" with sowings on file.
#[test]
fn the_seed_register_cannot_be_declared_empty_while_it_holds_sowings() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    assert!(matches!(
        repo::set_register_declaration(
            &mut conn,
            &fx.farm_id,
            &fx.season_id,
            "seed_treatment",
            "2026-09-01",
            None,
        )
        .unwrap_err(),
        module_cue::CueError::Invalid("register_has_rows")
    ));
}

/// And the other direction: a sowing recorded into a register already declared
/// empty withdraws that declaration, exactly as a non-field treatment does.
#[test]
fn recording_a_sowing_withdraws_a_standing_seed_declaration() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let declared = repo::set_register_declaration(
        &mut conn,
        &fx.farm_id,
        &fx.season_id,
        "seed_treatment",
        "2026-09-01",
        None,
    )
    .unwrap();

    repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    assert!(
        repo::list_register_declarations(&conn, &fx.farm_id, &fx.season_id)
            .unwrap()
            .is_empty()
    );
    let (op, _, _) = last_change(&conn, "register_declaration", &declared.id);
    assert_eq!(op, "delete");
}

/// A soft-deleted sowing no longer holds the register: the farmer may then
/// declare it empty, which is the honest state.
#[test]
fn deleting_the_last_sowing_frees_the_register_to_be_declared_empty() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();
    repo::soft_delete_seed_treatment(&mut conn, &saved.record.id, None).unwrap();

    assert!(
        repo::set_register_declaration(
            &mut conn,
            &fx.farm_id,
            &fx.season_id,
            "seed_treatment",
            "2026-09-01",
            None,
        )
        .is_ok()
    );
}

/// The registers stay independent: sowings say nothing about postharvest.
#[test]
fn a_sowing_does_not_block_declaring_another_register_empty() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    assert!(
        repo::set_register_declaration(
            &mut conn,
            &fx.farm_id,
            &fx.season_id,
            "postharvest",
            "2026-09-01",
            None,
        )
        .is_ok()
    );
}

#[test]
fn sowings_list_per_farm_and_campaign_oldest_first() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let mut later = sample(&fx);
    later.sown_on = "2025-11-20".into();
    repo::insert_seed_treatment(&mut conn, later, None).unwrap();
    repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();

    let listed = repo::list_seed_treatments(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record.sown_on, "2025-11-10");
    assert_eq!(listed[1].record.sown_on, "2025-11-20");

    // Another campaign's book is untouched.
    let other = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2026/2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    assert!(
        repo::list_seed_treatments(&conn, &other.id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
}

/// Where the seed was treated is the twin's required `Tratamiento`, and one of
/// FEGA's four `TIPO_TRATAMIENTO` values — but the printed model has no column
/// for it, so a book kept to the model alone leaves it unstated rather than
/// being blocked. A stated value must still be one the export can speak.
#[test]
fn the_seed_treatment_kind_is_optional_but_never_invented() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let mut unstated = sample(&fx);
    unstated.treatment_kind_code = None;
    let saved = repo::insert_seed_treatment(&mut conn, unstated, None).unwrap();
    assert!(saved.record.treatment_kind_code.is_none());

    for code in [
        "on_farm",
        "processing_centre",
        "purchased_es",
        "purchased_abroad",
    ] {
        let mut record = sample(&fx);
        record.treatment_kind_code = Some(code.into());
        let saved = repo::insert_seed_treatment(&mut conn, record, None).unwrap();
        assert_eq!(saved.record.treatment_kind_code.as_deref(), Some(code));
    }

    let mut invented = sample(&fx);
    invented.treatment_kind_code = Some("by_hand".into());
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, invented, None).unwrap_err(),
        module_cue::CueError::Invalid("unknown_seed_treatment_kind")
    ));
}

// ---------------------------------------------------------------------------
// The link to core's sowing register (`SiembraPlantacion.MaterialTratado`)
// ---------------------------------------------------------------------------
//
// The column lives on THIS side because a module may reference a core table and
// never the reverse — which happens to be the direction the exchange format
// points too (`UsoSemillaTratada.IdAjenaSiembraPlant`) and the right
// cardinality: one sowing can use several lots, each naming it.

fn insert_sowing(conn: &mut Connection, season_id: &str, farm_id: &str, plot_id: &str) -> String {
    terrazgo_core::repository::insert_sowing_record(
        conn,
        terrazgo_core::models::NewSowingRecord {
            season_id: season_id.into(),
            farm_id: farm_id.into(),
            kind_code: "sowing".into(),
            sown_on: "2025-11-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(680.0),
            notes: None,
            plots: vec![terrazgo_core::models::NewSowingPlot {
                plot_id: plot_id.into(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn a_record_may_name_the_sowing_that_used_its_seed() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let sowing_id = insert_sowing(&mut conn, &fx.season_id, &fx.farm_id, &fx.plot_a);

    let linked = NewSeedTreatment {
        sowing_record_id: Some(sowing_id.clone()),
        acquired_on: Some("2025-10-27".into()),
        ..sample(&fx)
    };
    let saved = repo::insert_seed_treatment(&mut conn, linked, None).unwrap();
    assert_eq!(saved.record.sowing_record_id, Some(sowing_id.clone()));
    assert_eq!(saved.record.acquired_on.as_deref(), Some("2025-10-27"));

    // Reachable from the sowing's side, which is how the export reads it.
    let from_sowing = repo::list_seed_treatments_for_sowing(&conn, &sowing_id).unwrap();
    assert_eq!(from_sowing.len(), 1);
    assert_eq!(from_sowing[0].id, saved.record.id);
}

#[test]
fn the_link_may_not_reach_another_holding_or_another_campaign() {
    // The foreign key alone would allow both. The export reads this link to
    // state `MaterialTratado` on that sowing, so a cross-farm link would put one
    // farmer's treated seed in another's descriptor — the `plot_not_on_farm`
    // rule, one table over.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);

    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra finca".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let other_plot = repo::insert_plot(
        &mut conn,
        NewPlot {
            farm_id: other_farm.clone(),
            name: "Parcela ajena".into(),
            area_ha: Some(2.0),
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let elsewhere = insert_sowing(&mut conn, &fx.season_id, &other_farm, &other_plot);
    assert!(matches!(
        repo::insert_seed_treatment(
            &mut conn,
            NewSeedTreatment {
                sowing_record_id: Some(elsewhere),
                ..sample(&fx)
            },
            None,
        )
        .unwrap_err(),
        module_cue::error::CueError::Invalid("sowing_not_on_farm")
    ));

    let next_season = repo::insert_season(
        &mut conn,
        NewSeason {
            campaign_year: 2027,
            label: "2026/2027".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();
    let next_year = insert_sowing(&mut conn, &next_season.id, &fx.farm_id, &fx.plot_a);
    assert!(matches!(
        repo::insert_seed_treatment(
            &mut conn,
            NewSeedTreatment {
                sowing_record_id: Some(next_year),
                ..sample(&fx)
            },
            None,
        )
        .unwrap_err(),
        module_cue::error::CueError::Invalid("sowing_not_on_farm")
    ));
}

#[test]
fn a_link_to_a_withdrawn_sowing_is_refused_and_a_withdrawn_record_stops_counting() {
    // A link is a statement about a LIVE register, in both directions: it cannot
    // be made to a soft-deleted sowing, and a soft-deleted seed record stops
    // asserting that the sowing used treated material.
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let sowing_id = insert_sowing(&mut conn, &fx.season_id, &fx.farm_id, &fx.plot_a);
    let saved = repo::insert_seed_treatment(
        &mut conn,
        NewSeedTreatment {
            sowing_record_id: Some(sowing_id.clone()),
            ..sample(&fx)
        },
        None,
    )
    .unwrap();

    repo::soft_delete_seed_treatment(&mut conn, &saved.record.id, None).unwrap();
    assert!(
        repo::list_seed_treatments_for_sowing(&conn, &sowing_id)
            .unwrap()
            .is_empty()
    );

    terrazgo_core::repository::soft_delete_sowing_record(&mut conn, &sowing_id, None).unwrap();
    assert!(matches!(
        repo::insert_seed_treatment(
            &mut conn,
            NewSeedTreatment {
                sowing_record_id: Some(sowing_id),
                ..sample(&fx)
            },
            None,
        )
        .unwrap_err(),
        module_cue::error::CueError::NotFound
    ));
}

#[test]
fn a_correction_can_add_or_drop_the_link_and_the_purchase_date() {
    let mut conn = open_in_memory().unwrap();
    let fx = fixture(&mut conn);
    let sowing_id = insert_sowing(&mut conn, &fx.season_id, &fx.farm_id, &fx.plot_a);
    let saved = repo::insert_seed_treatment(&mut conn, sample(&fx), None).unwrap();
    assert_eq!(saved.record.sowing_record_id, None);

    let base = sample(&fx);
    let linked = repo::update_seed_treatment(
        &mut conn,
        &saved.record.id,
        UpdateSeedTreatment {
            sown_on: base.sown_on.clone(),
            species_name: base.species_name.clone(),
            variety: base.variety.clone(),
            crop_code: base.crop_code.clone(),
            seed_quantity_kg: base.seed_quantity_kg,
            seed_lot: base.seed_lot.clone(),
            treatment_kind_code: base.treatment_kind_code.clone(),
            acquired_on: Some("2025-10-27".into()),
            sowing_record_id: Some(sowing_id.clone()),
            product_name: base.product_name.clone(),
            product_registration_number: base.product_registration_number.clone(),
            product_active_substance: base.product_active_substance.clone(),
            product_id: None,
            notes: base.notes.clone(),
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_a.clone(),
                surface_sown_ha: 3.2,
            }],
        },
        None,
    )
    .unwrap();
    assert_eq!(linked.record.sowing_record_id, Some(sowing_id));
    assert_eq!(linked.record.acquired_on.as_deref(), Some("2025-10-27"));

    let unlinked = repo::update_seed_treatment(
        &mut conn,
        &saved.record.id,
        UpdateSeedTreatment {
            sown_on: base.sown_on,
            species_name: base.species_name,
            variety: base.variety,
            crop_code: base.crop_code,
            seed_quantity_kg: base.seed_quantity_kg,
            seed_lot: base.seed_lot,
            treatment_kind_code: base.treatment_kind_code,
            acquired_on: None,
            sowing_record_id: None,
            product_name: base.product_name,
            product_registration_number: base.product_registration_number,
            product_active_substance: base.product_active_substance,
            product_id: None,
            notes: base.notes,
            plots: vec![NewSeedTreatmentPlot {
                plot_id: fx.plot_a.clone(),
                surface_sown_ha: 3.2,
            }],
        },
        None,
    )
    .unwrap();
    assert_eq!(unlinked.record.sowing_record_id, None);
    assert_eq!(unlinked.record.acquired_on, None);
}
