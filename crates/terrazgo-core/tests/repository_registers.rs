// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The registers core owns because they are whole-farm data rather than one
//! module's: the fields the printed model asks of the holding itself (model
//! 1.1's contact details and the rest of slice 5), `harvest_record` (model
//! section 5 — what left the holding and to whom) and `sowing_record` (which
//! feeds model sections 9.2 and 9.3).
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::CoreError;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// Slice 5: the fields the printed model asks for
// ---------------------------------------------------------------------------

/// Model 1.1 asks for postal contact details of the holding; they are universal
/// (every country's book wants them) so they live on `farm`, not the regional
/// extension. The create form does not offer them — 1.1 is set up once, in the
/// edit form — so a new farm carries them as NULL until updated.
#[test]
fn farm_contact_details_round_trip_and_are_audited() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    assert_eq!(farm.address, None, "not on the create form");

    let detail = repo::update_farm(
        &mut conn,
        &farm.id,
        UpdateFarm {
            name: "Finca".into(),
            owner_name: Some("María García".into()),
            owner_tax_id: Some("12345678Z".into()),
            location_text: Some("Medina del Campo".into()),
            address: Some("Camino de la Vega, 4".into()),
            postal_code: Some("47400".into()),
            phone_fixed: Some("983000000".into()),
            phone_mobile: Some("600000000".into()),
            email: Some("maria@example.es".into()),
            opened_on: None,
            latitude: None,
            longitude: None,
            country_code: "es".into(),
            es: Some(FarmEsFields {
                rega_code: None,
                rea_code: Some("ES244700000123".into()),
                siex_code: Some("ES470000000123".into()),
                province_code: Some("47".into()),
            }),
            representative: None,
        },
        None,
    )
    .unwrap();

    assert_eq!(detail.farm.address.as_deref(), Some("Camino de la Vega, 4"));
    assert_eq!(detail.farm.postal_code.as_deref(), Some("47400"));
    assert_eq!(detail.farm.email.as_deref(), Some("maria@example.es"));
    // The national and autonómico registry numbers are separate columns: the
    // model prints them side by side, so one field could never serve both.
    let es = detail.es.expect("extension");
    assert_eq!(es.siex_code.as_deref(), Some("ES470000000123"));
    assert_eq!(es.rea_code.as_deref(), Some("ES244700000123"));

    let (_, before, after) = last_change(&conn, "farm", &farm.id);
    assert_eq!(before["address"], Value::Null);
    assert_eq!(after["address"], "Camino de la Vega, 4");
    assert_eq!(after["phone_mobile"], "600000000");
}

/// The representative follows the extension contract exactly: absent block
/// means none, present means insert-or-update, and removing it hard-deletes
/// the row with a null after-image.
#[test]
fn farm_representative_is_reconciled_from_the_submitted_state() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let update = |rep: Option<FarmRepresentativeFields>| UpdateFarm {
        name: "Finca".into(),
        owner_name: None,
        owner_tax_id: None,
        location_text: None,
        address: None,
        postal_code: None,
        phone_fixed: None,
        phone_mobile: None,
        email: None,
        opened_on: None,
        latitude: None,
        longitude: None,
        country_code: "es".into(),
        es: None,
        representative: rep,
    };
    let fields = |name: &str| FarmRepresentativeFields {
        full_name: name.into(),
        tax_id: Some("87654321X".into()),
        representation_kind: Some("Administrador único".into()),
        address: None,
        locality: None,
        province: None,
        postal_code: None,
        phone: None,
        email: None,
    };

    // None → nothing stored, nothing logged.
    let detail = repo::update_farm(&mut conn, &farm.id, update(None), None).unwrap();
    assert!(detail.representative.is_none());

    // Insert.
    let detail =
        repo::update_farm(&mut conn, &farm.id, update(Some(fields("Ana Ruiz"))), None).unwrap();
    let rep = detail.representative.expect("representative stored");
    assert_eq!(rep.full_name, "Ana Ruiz");
    assert_eq!(
        rep.representation_kind.as_deref(),
        Some("Administrador único")
    );
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    assert_eq!(after["full_name"], "Ana Ruiz");

    // Update in place.
    repo::update_farm(
        &mut conn,
        &farm.id,
        update(Some(fields("Ana Ruiz Pérez"))),
        None,
    )
    .unwrap();
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "update");
    assert_eq!(before["full_name"], "Ana Ruiz");
    assert_eq!(after["full_name"], "Ana Ruiz Pérez");

    // Removing the block hard-deletes the row, logged with a null after-image
    // (the farm_es_extension precedent).
    let detail = repo::update_farm(&mut conn, &farm.id, update(None), None).unwrap();
    assert!(detail.representative.is_none());
    let (op, before, after) = last_change(&conn, "farm_representative", &farm.id);
    assert_eq!(op, "delete");
    assert_eq!(before["full_name"], "Ana Ruiz Pérez");
    assert!(after.is_null());

    // And a blank name is rejected like every other user-entered name.
    let blank = repo::update_farm(&mut conn, &farm.id, update(Some(fields("  "))), None);
    assert!(matches!(blank, Err(CoreError::Invalid("empty_name"))));
}

/// Anexo III A.2.e: "secano o regadío (indicando en su caso el sistema de
/// riego)" and "al aire libre o protegido". Both are coded lists, not booleans
/// — the official model prints four siglas for each.
#[test]
fn crop_carries_its_own_surface_and_agronomic_codes() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();

    let crop = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(1.25),
            irrigation_code: Some("sprinkler".into()),
            growing_environment_code: Some("open_air".into()),
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(crop.area_ha, Some(1.25));
    assert_eq!(crop.irrigation_code.as_deref(), Some("sprinkler"));

    let updated = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(1.5),
            irrigation_code: Some("drip".into()),
            growing_environment_code: Some("greenhouse".into()),
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(updated.area_ha, Some(1.5));
    assert_eq!(
        updated.growing_environment_code.as_deref(),
        Some("greenhouse")
    );

    let (_, before, after) = last_change(&conn, "crop", &crop.id);
    assert_eq!(before["irrigation_code"], "sprinkler");
    assert_eq!(after["irrigation_code"], "drip");
    assert_eq!(after["area_ha"], 1.5);

    // The codes are real foreign keys — a typo cannot reach the book.
    let bad = repo::update_crop(
        &mut conn,
        &crop.id,
        UpdateCrop {
            species_name: "maíz".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: Some("no-such-system".into()),
            growing_environment_code: None,
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    );
    assert!(bad.is_err(), "unknown irrigation code must be refused");
}

/// Model 1.2 prints a NIF beside every applicator, 1.3 an acquisition date
/// beside every machine (Anexo III A.1.c and A.1.h).
#[test]
fn operator_tax_id_and_machinery_acquisition_date_round_trip() {
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();

    let operator = repo::insert_operator(
        &mut conn,
        NewOperator {
            full_name: "Carlos Pérez".into(),
            tax_id: Some("11111111H".into()),
            licence_number: Some("ROPO-1".into()),
            licence_level_code: Some("pilot".into()),
            licence_expiry_date: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(operator.tax_id.as_deref(), Some("11111111H"));
    // 'pilot' is the aerial carné the model prints as a fourth column.
    assert_eq!(operator.licence_level_code.as_deref(), Some("pilot"));

    let machinery = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Atomizador".into(),
            kind: None,
            acquired_on: Some("2018-03-15".into()),
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(machinery.acquired_on.as_deref(), Some("2018-03-15"));

    let (_, _, after) = last_change(&conn, "machinery", &machinery.id);
    assert_eq!(after["acquired_on"], "2018-03-15");
    let (_, _, after) = last_change(&conn, "operator", &operator.id);
    assert_eq!(after["tax_id"], "11111111H");
}

// --- commercialised harvest (model section 5) -------------------------------
//
// In core rather than in the CUE module: what leaves the holding and to whom is
// whole-farm data the costs and analytics modules will want. Fully correctable,
// like the treated-seed register — the record holds no snapshot of another
// row's identity, so there is nothing a later edit elsewhere could rewrite.

struct HarvestFixture {
    season_id: String,
    farm_id: String,
    plot_a: String,
    plot_b: String,
    crop_a: String,
}

fn harvest_fixture(conn: &mut Connection) -> HarvestFixture {
    let season = repo::insert_season(conn, new_season(2026, "2025/2026"), None).unwrap();
    let farm = repo::insert_farm(conn, new_farm("Finca La Vega"), None).unwrap();
    let plot_a = repo::insert_plot(conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let plot_b = repo::insert_plot(conn, new_plot(&farm.id, "La Loma"), None).unwrap();
    let crop_a = repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot_a.id.clone(),
            season_id: season.id.clone(),
            species_name: "trigo blando".into(),
            variety: Some("Nogal".into()),
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            crop_code: None,
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    HarvestFixture {
        season_id: season.id,
        farm_id: farm.id,
        plot_a: plot_a.id,
        plot_b: plot_b.id,
        crop_a: crop_a.id,
    }
}

fn new_harvest(fx: &HarvestFixture) -> NewHarvestRecord {
    NewHarvestRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        harvested_on: "2026-07-24".into(),
        product_name: "trigo blando".into(),
        plant_product_code: Some("1".into()),
        quantity_value: Some(42.5),
        quantity_unit_code: Some("t".into()),
        delivery_note_ref: Some("ALB-2026/318".into()),
        lot_number: Some("L-26-07".into()),
        buyer_name: "Cooperativa Cerealista del Duero".into(),
        buyer_tax_id: Some("F47008123".into()),
        buyer_address: Some("Ctra. Palencia km 4, Valladolid".into()),
        buyer_registry_number: Some("21.0012345/VA".into()),
        notes: None,
        plots: vec![NewHarvestPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: Some(fx.crop_a.clone()),
        }],
    }
}

/// The model's field list for section 5: date, product, quantity, the parcels
/// of origin, the delivery-note and lot references, and the buyer block down to
/// the "Nº de RGSEAA" — which core stores under a neutral name because core
/// tables carry no regional identifiers.
#[test]
fn a_harvest_records_what_left_the_holding_and_to_whom() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    assert_eq!(saved.record.harvested_on, "2026-07-24");
    assert_eq!(saved.record.quantity_value, Some(42.5));
    assert_eq!(saved.record.quantity_unit_code.as_deref(), Some("t"));
    assert_eq!(
        saved.record.buyer_registry_number.as_deref(),
        Some("21.0012345/VA")
    );
    assert_eq!(saved.plots.len(), 1);
    // The harvested crop is frozen, so renaming it later cannot rewrite what
    // the printed book said was sold.
    assert_eq!(
        saved.plots[0].crop_name_snapshot.as_deref(),
        Some("trigo blando")
    );
    assert_eq!(saved.plots[0].variety_snapshot.as_deref(), Some("Nogal"));
}

/// A quantity is a value AND its unit or neither: an amount with no unit is not
/// a statement, and the set is {kg, t} because that is what the model measures a
/// sold harvest in. Enforced here rather than by a foreign key — `unit` is a
/// module-cue lookup and core may never reference a module's table.
#[test]
fn a_harvest_quantity_is_a_value_and_a_unit_or_neither() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    // Both absent: the printed cell is left to be filled by hand.
    let mut blank = new_harvest(&fx);
    blank.quantity_value = None;
    blank.quantity_unit_code = None;
    let saved = repo::insert_harvest_record(&mut conn, blank, None).unwrap();
    assert!(saved.record.quantity_value.is_none());

    for (value, unit) in [(Some(1200.0), Some("kg")), (Some(1.2), Some("t"))] {
        let mut ok = new_harvest(&fx);
        ok.quantity_value = value;
        ok.quantity_unit_code = unit.map(str::to_string);
        assert!(repo::insert_harvest_record(&mut conn, ok, None).is_ok());
    }

    // A litre of wheat is a different claim, not a unit slip; and a value with
    // no unit, or a unit with no value, says nothing.
    for (value, unit) in [
        (Some(1200.0), Some("l")),
        (Some(1200.0), Some("m3")),
        (Some(1200.0), None),
        (None, Some("kg")),
        (Some(0.0), Some("kg")),
        (Some(-3.0), Some("t")),
    ] {
        let mut bad = new_harvest(&fx);
        bad.quantity_value = value;
        bad.quantity_unit_code = unit.map(str::to_string);
        assert!(
            matches!(
                repo::insert_harvest_record(&mut conn, bad, None).unwrap_err(),
                CoreError::Invalid("invalid_harvest_quantity")
            ),
            "accepted {value:?} {unit:?}"
        );
    }
}

#[test]
fn a_harvest_needs_a_product_a_buyer_and_at_least_one_plot() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut blank_product = new_harvest(&fx);
    blank_product.product_name = "  ".into();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, blank_product, None).unwrap_err(),
        CoreError::Invalid("empty_name")
    ));

    let mut blank_buyer = new_harvest(&fx);
    blank_buyer.buyer_name = String::new();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, blank_buyer, None).unwrap_err(),
        CoreError::Invalid("empty_buyer_name")
    ));

    let mut no_plots = new_harvest(&fx);
    no_plots.plots.clear();
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, no_plots, None).unwrap_err(),
        CoreError::Invalid("no_plots")
    ));
}

/// A parcel on another holding would put foreign land in this farm's book.
#[test]
fn a_harvest_plot_must_be_on_the_same_farm() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let other = repo::insert_farm(&mut conn, new_farm("Finca ajena"), None).unwrap();
    let foreign = repo::insert_plot(&mut conn, new_plot(&other.id, "El Soto"), None).unwrap();

    let mut record = new_harvest(&fx);
    record.plots = vec![NewHarvestPlot {
        plot_id: foreign.id,
        crop_id: None,
    }];
    assert!(matches!(
        repo::insert_harvest_record(&mut conn, record, None).unwrap_err(),
        CoreError::Invalid("plot_not_on_farm")
    ));
}

#[test]
fn every_harvest_row_is_logged_with_a_complete_image_and_the_actor() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), Some("carlos")).unwrap();

    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "insert");
    assert!(before.is_null());
    // Complete row image: the log is the future sync delta source.
    for column in [
        "id",
        "season_id",
        "farm_id",
        "harvested_on",
        "product_name",
        "plant_product_code",
        "quantity_value",
        "quantity_unit_code",
        "delivery_note_ref",
        "lot_number",
        "buyer_name",
        "buyer_tax_id",
        "buyer_address",
        "buyer_registry_number",
        "notes",
        "created_at",
        "updated_at",
        "deleted_at",
    ] {
        assert!(
            after.get(column).is_some(),
            "after-image is missing column '{column}'"
        );
    }

    let (_, _, plot) = last_change(&conn, "harvest_plot", &saved.plots[0].id);
    assert_eq!(plot["crop_name_snapshot"], "trigo blando");

    let actor: String = conn
        .query_row(
            "SELECT actor FROM record_change WHERE entity_id = ?1",
            [&saved.record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "carlos");
}

#[test]
fn a_harvest_can_be_corrected_in_full_and_logs_both_images() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    let updated = repo::update_harvest_record(
        &mut conn,
        &saved.record.id,
        UpdateHarvestRecord {
            harvested_on: "2026-07-26".into(),
            product_name: "trigo blando".into(),
            plant_product_code: Some("1".into()),
            quantity_value: Some(44.0),
            quantity_unit_code: Some("t".into()),
            delivery_note_ref: Some("ALB-2026/322".into()),
            lot_number: None,
            buyer_name: "Harinera del Pisuerga S.L.".into(),
            buyer_tax_id: Some("B47999000".into()),
            buyer_address: None,
            buyer_registry_number: None,
            notes: Some("Albarán corregido tras el pesaje definitivo.".into()),
            plots: vec![NewHarvestPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.record.quantity_value, Some(44.0));
    assert_eq!(updated.record.buyer_name, "Harinera del Pisuerga S.L.");
    // Clearing an optional field really clears it.
    assert!(updated.record.lot_number.is_none());
    // The campaign and the holding are not the form's to move.
    assert_eq!(updated.record.season_id, saved.record.season_id);
    assert_eq!(updated.record.farm_id, saved.record.farm_id);

    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "update");
    assert_eq!(before["quantity_value"], 42.5);
    assert_eq!(after["quantity_value"], 44.0);
}

/// The origin plots are reconciled from the submitted state — added, dropped
/// and changed rows each get their own audit entry, so the log stays
/// rebuildable.
#[test]
fn correcting_the_harvest_plots_reconciles_them_and_logs_each_change() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();
    let original_plot_row = saved.plots[0].id.clone();

    let updated = repo::update_harvest_record(
        &mut conn,
        &saved.record.id,
        UpdateHarvestRecord {
            harvested_on: saved.record.harvested_on.clone(),
            product_name: saved.record.product_name.clone(),
            plant_product_code: saved.record.plant_product_code.clone(),
            quantity_value: saved.record.quantity_value,
            quantity_unit_code: saved.record.quantity_unit_code.clone(),
            delivery_note_ref: saved.record.delivery_note_ref.clone(),
            lot_number: saved.record.lot_number.clone(),
            buyer_name: saved.record.buyer_name.clone(),
            buyer_tax_id: saved.record.buyer_tax_id.clone(),
            buyer_address: saved.record.buyer_address.clone(),
            buyer_registry_number: saved.record.buyer_registry_number.clone(),
            notes: None,
            // El Prado goes, La Loma arrives.
            plots: vec![NewHarvestPlot {
                plot_id: fx.plot_b.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    assert_eq!(updated.plots.len(), 1);
    assert_eq!(updated.plots[0].plot_id, fx.plot_b);

    // The dropped row is hard-deleted (it is a pure child, like an extension
    // row) and logged with a null after-image.
    let (op, _, after) = last_change(&conn, "harvest_plot", &original_plot_row);
    assert_eq!(op, "delete");
    assert!(after.is_null(), "a removed child logs a null after-image");

    let (op, _, _) = last_change(&conn, "harvest_plot", &updated.plots[0].id);
    assert_eq!(op, "insert");
}

#[test]
fn a_deleted_harvest_leaves_the_book_but_keeps_both_images() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();

    repo::soft_delete_harvest_record(&mut conn, &saved.record.id, None).unwrap();

    assert!(
        repo::list_harvest_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    let (op, before, after) = last_change(&conn, "harvest_record", &saved.record.id);
    assert_eq!(op, "delete");
    assert_eq!(before["buyer_name"], "Cooperativa Cerealista del Duero");
    assert!(
        after["deleted_at"].is_string(),
        "a soft delete logs the deleted row as its after-image"
    );
}

#[test]
fn harvests_list_per_farm_and_campaign_oldest_first() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut later = new_harvest(&fx);
    later.harvested_on = "2026-08-02".into();
    repo::insert_harvest_record(&mut conn, later, None).unwrap();
    let mut earlier = new_harvest(&fx);
    earlier.harvested_on = "2026-07-11".into();
    repo::insert_harvest_record(&mut conn, earlier, None).unwrap();

    let rows = repo::list_harvest_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].record.harvested_on, "2026-07-11");
    assert_eq!(rows[1].record.harvested_on, "2026-08-02");
}

/// The core half of the season-deletion guard. Every record-book view is read
/// through its season, so hiding one would hide the sale it holds — including a
/// soft-deleted one, whose audit history is reachable only that way.
#[test]
fn a_season_holding_a_harvest_cannot_be_deleted() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let empty = repo::insert_season(&mut conn, new_season(2028, "2027/2028"), None).unwrap();

    let saved = repo::insert_harvest_record(&mut conn, new_harvest(&fx), None).unwrap();
    assert!(matches!(
        repo::soft_delete_season(&mut conn, &fx.season_id, None).unwrap_err(),
        CoreError::Invalid("season_in_use")
    ));
    // A season with nothing in it is still deletable.
    assert!(repo::soft_delete_season(&mut conn, &empty.id, None).is_ok());

    repo::soft_delete_harvest_record(&mut conn, &saved.record.id, None).unwrap();
    assert!(
        matches!(
            repo::soft_delete_season(&mut conn, &fx.season_id, None).unwrap_err(),
            CoreError::Invalid("season_in_use")
        ),
        "a soft-deleted sale still pins its season"
    );
}

// --- sowing and planting (feeds model sections 9.2 and 9.3) -----------------
//
// Harvest's mirror image, and in core for the same reason: the two bracket a
// crop. It is a register in its own right AND the source of two columns of the
// record book's third decree — but it carries no eco-scheme practice code,
// because core may not reference a module's lookup and a sowing is a farm event
// under no decree in particular.

fn new_sowing(fx: &HarvestFixture) -> NewSowingRecord {
    NewSowingRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        kind_code: "sowing".into(),
        sown_on: "2026-04-10".into(),
        sowing_end_date: None,
        flooded_on: None,
        seed_quantity_kg: Some(180.0),
        notes: None,
        plots: vec![NewSowingPlot {
            plot_id: fx.plot_a.clone(),
            crop_id: Some(fx.crop_a.clone()),
        }],
    }
}

#[test]
fn a_sowing_records_how_a_crop_began_and_freezes_it() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_sowing_record(&mut conn, new_sowing(&fx), None).unwrap();
    assert_eq!(saved.record.sown_on, "2026-04-10");
    assert_eq!(saved.record.seed_quantity_kg, Some(180.0));
    assert_eq!(saved.plots.len(), 1);
    // Frozen like the harvest's, so renaming the crop later cannot rewrite what
    // the book said was sown.
    assert_eq!(
        saved.plots[0].crop_name_snapshot.as_deref(),
        Some("trigo blando")
    );
    assert_eq!(saved.plots[0].variety_snapshot.as_deref(), Some("Nogal"));

    let read_back = repo::get_sowing_record(&conn, &saved.record.id).unwrap();
    assert_eq!(read_back.plots.len(), 1);
}

#[test]
fn a_sowing_states_whether_it_was_sown_or_planted() {
    // The register's form is titled "Siembra y plantación" and asks how each
    // crop began, so both are its documented use — and the column is NOT NULL
    // because SIEX `SiembraPlantacion` has to say which, with no "unstated"
    // answer available. The lookup is what makes an invented value impossible.
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let sown = repo::insert_sowing_record(&mut conn, new_sowing(&fx), None).unwrap();
    assert_eq!(sown.record.kind_code, "sowing");

    let planted = repo::insert_sowing_record(
        &mut conn,
        NewSowingRecord {
            kind_code: "planting".into(),
            ..new_sowing(&fx)
        },
        None,
    )
    .unwrap();
    assert_eq!(planted.record.kind_code, "planting");

    // A correction can restate it: a row entered as a sowing and meant as a
    // planting is a typo like any other.
    let corrected = repo::update_sowing_record(
        &mut conn,
        &sown.record.id,
        UpdateSowingRecord {
            kind_code: "planting".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();
    assert_eq!(corrected.record.kind_code, "planting");

    // And a value outside the lookup is refused by the foreign key.
    assert!(
        repo::insert_sowing_record(
            &mut conn,
            NewSowingRecord {
                kind_code: "transplant".into(),
                ..new_sowing(&fx)
            },
            None,
        )
        .is_err()
    );
}

#[test]
fn a_dry_sowing_is_flooded_later_by_correcting_the_same_record() {
    // RD 1048/2022 art. 45.2 names siembra and inundación as separate
    // annotations, each within a month of its own activity — and a rice grower
    // dry-sows in April and floods in May. So `flooded_on` is NULL at insert
    // and filled by a CORRECTION weeks later: one act, one row. That is the
    // whole reason the two dates can share a record.
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_sowing_record(&mut conn, new_sowing(&fx), None).unwrap();
    assert_eq!(
        saved.record.flooded_on, None,
        "not flooded yet, not unknown"
    );

    let corrected = repo::update_sowing_record(
        &mut conn,
        &saved.record.id,
        UpdateSowingRecord {
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: Some("2026-05-05".into()),
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: fx.plot_a.clone(),
                crop_id: Some(fx.crop_a.clone()),
            }],
        },
        None,
    )
    .unwrap();
    assert_eq!(corrected.record.flooded_on.as_deref(), Some("2026-05-05"));
    assert_eq!(corrected.record.id, saved.record.id);
}

#[test]
fn a_field_cannot_be_flooded_before_it_is_sown() {
    // The register is about *siembra en seco*: the seed goes into dry ground
    // and the water follows, which is the order model 9.3 prints its columns in
    // and the order art. 45.2 names the activities.
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut backwards = new_sowing(&fx);
    backwards.flooded_on = Some("2026-04-09".into());
    assert!(matches!(
        repo::insert_sowing_record(&mut conn, backwards, None).unwrap_err(),
        CoreError::Invalid("flooded_before_sown")
    ));

    // The same day is fine — sown in the morning, flooded in the afternoon.
    let mut same_day = new_sowing(&fx);
    same_day.flooded_on = Some("2026-04-10".into());
    assert!(repo::insert_sowing_record(&mut conn, same_day, None).is_ok());
}

#[test]
fn a_sowing_cannot_end_before_it_starts() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut backwards = new_sowing(&fx);
    backwards.sowing_end_date = Some("2026-04-01".into());
    assert!(matches!(
        repo::insert_sowing_record(&mut conn, backwards, None).unwrap_err(),
        CoreError::Invalid("invalid_date_interval")
    ));
}

#[test]
fn a_stated_seed_quantity_must_be_a_real_weight() {
    // The column exists only because the SIEX twin requires `Cantidad`; the
    // decree asks for dates. So an unstated amount is fine, and a stated zero
    // is not a sowing.
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let mut unstated = new_sowing(&fx);
    unstated.seed_quantity_kg = None;
    assert!(repo::insert_sowing_record(&mut conn, unstated, None).is_ok());

    for bad in [0.0, -5.0] {
        let mut invalid = new_sowing(&fx);
        invalid.seed_quantity_kg = Some(bad);
        assert!(matches!(
            repo::insert_sowing_record(&mut conn, invalid, None).unwrap_err(),
            CoreError::Invalid("invalid_seed_quantity")
        ));
    }
}

#[test]
fn a_sowing_needs_a_plot_and_it_must_be_on_the_farm() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);
    let other_farm = repo::insert_farm(&mut conn, new_farm("Finca del Vecino"), None).unwrap();
    let other_plot = repo::insert_plot(&mut conn, new_plot(&other_farm.id, "Ajena"), None).unwrap();

    let mut no_plots = new_sowing(&fx);
    no_plots.plots = vec![];
    assert!(matches!(
        repo::insert_sowing_record(&mut conn, no_plots, None).unwrap_err(),
        CoreError::Invalid("no_plots")
    ));

    let mut foreign = new_sowing(&fx);
    foreign.plots = vec![NewSowingPlot {
        plot_id: other_plot.id,
        crop_id: None,
    }];
    assert!(matches!(
        repo::insert_sowing_record(&mut conn, foreign, None).unwrap_err(),
        CoreError::Invalid("plot_not_on_farm")
    ));
}

#[test]
fn a_correction_reconciles_the_sown_plots_and_keeps_the_ones_that_stayed() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_sowing_record(&mut conn, new_sowing(&fx), None).unwrap();
    let kept_id = saved.plots[0].id.clone();

    let corrected = repo::update_sowing_record(
        &mut conn,
        &saved.record.id,
        UpdateSowingRecord {
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: Some("2026-04-12".into()),
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![
                NewSowingPlot {
                    plot_id: fx.plot_a.clone(),
                    crop_id: Some(fx.crop_a.clone()),
                },
                NewSowingPlot {
                    plot_id: fx.plot_b.clone(),
                    crop_id: None,
                },
            ],
        },
        None,
    )
    .unwrap();

    assert_eq!(corrected.plots.len(), 2);
    assert!(
        corrected.plots.iter().any(|p| p.id == kept_id),
        "a plot that stayed keeps its row, so the audit reads as a correction"
    );
    assert_eq!(
        corrected.record.sowing_end_date.as_deref(),
        Some("2026-04-12")
    );
}

#[test]
fn every_sowing_write_is_audited_with_a_complete_row_image() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_sowing_record(&mut conn, new_sowing(&fx), Some("carlos")).unwrap();
    let (operation, before, after) = last_change(&conn, "sowing_record", &saved.record.id);
    assert_eq!(operation, "insert");
    assert!(before.is_null());
    assert_eq!(after["sown_on"], "2026-04-10");
    assert!(after.get("created_at").is_some(), "complete row image");

    // The junction is logged as an entity of its own, so a delta can rebuild it.
    let (plot_op, _, plot_after) = last_change(&conn, "sowing_plot", &saved.plots[0].id);
    assert_eq!(plot_op, "insert");
    assert_eq!(plot_after["plot_id"], fx.plot_a);
}

#[test]
fn a_deleted_sowing_leaves_the_register_but_keeps_its_history() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    let saved = repo::insert_sowing_record(&mut conn, new_sowing(&fx), None).unwrap();
    repo::soft_delete_sowing_record(&mut conn, &saved.record.id, None).unwrap();

    assert!(
        repo::list_sowing_records(&conn, &fx.season_id, &fx.farm_id)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        repo::get_sowing_record(&conn, &saved.record.id).unwrap_err(),
        CoreError::NotFound
    ));
    let (operation, _, _) = last_change(&conn, "sowing_record", &saved.record.id);
    assert_eq!(operation, "delete");
}

#[test]
fn sowings_list_oldest_first_within_their_own_season_and_farm() {
    let mut conn = db();
    let fx = harvest_fixture(&mut conn);

    for date in ["2026-05-01", "2026-03-15", "2026-04-10"] {
        let mut new = new_sowing(&fx);
        new.sown_on = date.into();
        repo::insert_sowing_record(&mut conn, new, None).unwrap();
    }

    let listed = repo::list_sowing_records(&conn, &fx.season_id, &fx.farm_id).unwrap();
    let dates: Vec<&str> = listed.iter().map(|d| d.record.sown_on.as_str()).collect();
    assert_eq!(dates, ["2026-03-15", "2026-04-10", "2026-05-01"]);

    assert!(
        repo::list_sowing_records(&conn, &fx.season_id, "no-such-farm")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_season_holding_a_sowing_cannot_be_deleted() {
    // Every register scoped to a season has to be in this guard: a season
    // holding nothing but a sowing would otherwise be deletable, and its
    // records would vanish from a book that is read season by season.
    let mut conn = db();
    let season = repo::insert_season(&mut conn, new_season(2026, "2025/2026"), None).unwrap();
    let farm = repo::insert_farm(&mut conn, new_farm("Arrozal"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "Tabla 1"), None).unwrap();

    repo::insert_sowing_record(
        &mut conn,
        NewSowingRecord {
            season_id: season.id.clone(),
            farm_id: farm.id.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: Some("2026-05-05".into()),
            seed_quantity_kg: None,
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: plot.id,
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap();

    assert!(matches!(
        repo::soft_delete_season(&mut conn, &season.id, None).unwrap_err(),
        CoreError::Invalid("season_in_use")
    ));
}

#[test]
fn crops_on_plot_returns_every_live_unit_and_says_nothing_about_choosing() {
    // The DGC question, and it deliberately answers with a list: a plot carrying
    // two crops is two units, and which of them a caller may assume is the
    // caller's rule. The SIEX export refuses such a plot rather than choosing.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let bare = repo::insert_plot(&mut conn, new_plot(&farm.id, "El Erial"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let other_season = repo::insert_season(&mut conn, new_season(2027, "2027"), None).unwrap();

    let crop = |plot_id: &str, season_id: &str, species: &str| NewCrop {
        plot_id: plot_id.into(),
        season_id: season_id.into(),
        species_name: species.into(),
        variety: None,
        production_system_code: None,
        area_ha: None,
        irrigation_code: None,
        growing_environment_code: None,
        gip_system_code: None,
        crop_code: None,
        source: None,
        source_campaign: None,
        declared_area_ha: None,
    };

    // A plot with nothing on it this season.
    assert!(
        repo::crops_on_plot(&conn, &bare.id, &season.id)
            .unwrap()
            .is_empty()
    );

    let wheat = repo::insert_crop(&mut conn, crop(&plot.id, &season.id, "trigo"), None).unwrap();
    // Another season's crop on the same plot must not appear.
    repo::insert_crop(&mut conn, crop(&plot.id, &other_season.id, "cebada"), None).unwrap();
    let found = repo::crops_on_plot(&conn, &plot.id, &season.id).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, wheat.id);

    // A second crop on the same plot and season: two units, both returned.
    let vetch = repo::insert_crop(&mut conn, crop(&plot.id, &season.id, "veza"), None).unwrap();
    assert_eq!(
        repo::crops_on_plot(&conn, &plot.id, &season.id)
            .unwrap()
            .len(),
        2
    );

    // A withdrawn crop is not a unit — counting one would make a plot look
    // ambiguous over a row the farmer has already retracted.
    repo::soft_delete_crop(&mut conn, &vetch.id, None).unwrap();
    let found = repo::crops_on_plot(&conn, &plot.id, &season.id).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, wheat.id);
}

#[test]
fn find_crop_for_export_resolves_a_withdrawn_crop() {
    // Crop deletion is always allowed — the registers that print a crop froze
    // its name and variety at write time — so a record written years ago
    // routinely names a crop that is no longer live, and the SIEX descriptor
    // still has to state that crop's PRODUCTOS code. The soft-delete filter is
    // left off for exactly that, which is why this has a name.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let plot = repo::insert_plot(&mut conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let season = repo::insert_season(&mut conn, new_season(2026, "2026"), None).unwrap();
    let wheat = repo::insert_crop(
        &mut conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: season.id.clone(),
            species_name: "trigo".into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            crop_code: Some("21".into()),
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap();

    repo::soft_delete_crop(&mut conn, &wheat.id, None).unwrap();
    // The book's own lister hides it, as it should.
    assert!(
        repo::list_crops(&conn, &season.id, &farm.id)
            .unwrap()
            .is_empty()
    );

    let found = repo::find_crop_for_export(&conn, &wheat.id)
        .unwrap()
        .unwrap();
    assert_eq!(found.id, wheat.id);
    assert_eq!(found.crop_code.as_deref(), Some("21"));
    assert!(found.deleted_at.is_some());

    // A missing row is None rather than an error — the `find_export_alias`
    // convention. Every crop_id that reaches it carries a real foreign key, so
    // this is unreachable in practice and the caller simply names no crop code.
    assert!(
        repo::find_crop_for_export(&conn, "no-such-crop")
            .unwrap()
            .is_none()
    );
}

#[test]
fn find_machinery_es_answers_by_id_and_says_nothing_when_neither_registry_applies() {
    // The SIEX export reads a ROMA number LIVE for a record that froze none of
    // its own, so it needs the extension by id rather than by farm.
    let mut conn = db();
    let farm = repo::insert_farm(&mut conn, new_farm("Finca"), None).unwrap();
    let sprayer = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Pulverizador".into(),
            kind: None,
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: Some("ROMA-4471".into()),
            reganip_number: None,
        },
        None,
    )
    .unwrap();
    let hand_tool = repo::insert_machinery(
        &mut conn,
        NewMachinery {
            farm_id: farm.id.clone(),
            name: "Mochila".into(),
            kind: None,
            acquired_on: None,
            last_inspection_date: None,
            next_inspection_due_date: None,
            roma_number: None,
            reganip_number: None,
        },
        None,
    )
    .unwrap();

    let found = repo::find_machinery_es(&conn, &sprayer.id)
        .unwrap()
        .unwrap();
    assert_eq!(found.roma_number.as_deref(), Some("ROMA-4471"));
    assert!(found.reganip_number.is_none());

    // Registered in neither ROMA nor REGANIP: no extension row at all, which is
    // a `None` rather than a row of nulls.
    assert!(
        repo::find_machinery_es(&conn, &hand_tool.id)
            .unwrap()
            .is_none()
    );
}
