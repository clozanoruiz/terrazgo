// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core's two registers: `SiembraPlantacion` — how a crop began, and the
//! stated link from a sowing to the seed treatment that dressed its seed — and
//! `ComercializacionVD`, what left the holding.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use module_cue::models::*;
use module_cue::repository as repo;
use terrazgo_siex::export_precheck;

// ---------------------------------------------------------------------------
// Seam 2 — core's two registers: what left the holding, and how a crop began
// ---------------------------------------------------------------------------

/// A complete sowing: dates, the seed weight the twin requires, and a crop on
/// every plot.
fn sowing(fx: &Fixture) -> terrazgo_core::models::NewSowingRecord {
    terrazgo_core::models::NewSowingRecord {
        season_id: fx.season_id.clone(),
        farm_id: fx.farm_id.clone(),
        kind_code: "sowing".into(),
        sown_on: "2025-10-15".into(),
        sowing_end_date: Some("2025-10-17".into()),
        flooded_on: None,
        seed_quantity_kg: Some(1800.0),
        notes: None,
        plots: vec![terrazgo_core::models::NewSowingPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: Some(fx.wheat_crop_id.clone()),
        }],
    }
}

#[test]
fn a_sale_exports_the_produce_its_amount_and_its_own_unit() {
    // Unlike TratamientosPostCosecha, this block carries a `Unidad` member, so
    // the stored tonnes travel as tonnes and nothing is converted. One stored
    // date fills both ends: the model prints a single "Fecha" column.
    let mut conn = db();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_harvest_record(&mut conn, harvest(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "ComercializacionVD");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["ProductoVegetal"], 85);
    assert_eq!(entries[0]["Cantidad"], 42.5);
    assert_eq!(entries[0]["Unidad"], 6); // UNIDADES_MEDIDA 6 = t, not converted
    assert_eq!(entries[0]["FechaInicio"], "24/07/2026");
    assert_eq!(entries[0]["FechaFin"], "24/07/2026");
}

#[test]
fn a_sale_sends_neither_the_sale_kind_nor_the_papers_the_schema_omits() {
    // `TipoVenta` is optional, Voluntario and unstored — the printed model draws
    // no comercializada/directa distinction, so claiming one would be inventing
    // it. `NumFactura` and `NumLote` are in the descriptor SHEET and not in the
    // JSON Schema, and the schema wins (the 3.11.4 re-diff rule), so the stored
    // albarán and lot stay printed-only.
    let mut conn = db();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_harvest_record(&mut conn, harvest(&fx), None).unwrap();

    let entry = block(
        &export_json(&mut conn, &fx.season_id, &fx.farm_id),
        "ComercializacionVD",
    )[0]
    .clone();
    assert!(entry.get("TipoVenta").is_none());
    assert!(entry.get("NumFactura").is_none());
    assert!(entry.get("NumLote").is_none());
}

#[test]
fn precheck_lists_a_sale_missing_the_three_members_the_schema_requires() {
    // All three are nullable in the register, because the printed model leaves
    // those cells to be filled by hand.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bare = terrazgo_core::models::NewHarvestRecord {
        plant_product_code: None,
        quantity_value: None,
        quantity_unit_code: None,
        ..harvest(&fx)
    };
    terrazgo_core::repository::insert_harvest_record(&mut conn, bare, None).unwrap();

    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.harvest_missing_fields.len(), 1);
    assert_eq!(
        report.harvest_missing_fields[0].product_name,
        "Trigo blando"
    );
    assert!(!report.is_clean());
}

#[test]
fn a_sowing_states_whether_it_was_sown_or_planted() {
    // The WS descriptor types the `SiembraPlantacion` member `number(1)`,
    // "1 Siembra 0 Plantación" — it is NOT the crop, which rides per-DGC as
    // `CodigoCultivo`. The register's form is titled "Siembra y plantación", so
    // both answers are its documented use and a constant would state a falsehood
    // about every orchard.
    let mut conn = db();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    let planted = terrazgo_core::models::NewSowingRecord {
        kind_code: "planting".into(),
        sown_on: "2026-02-20".into(),
        sowing_end_date: None,
        plots: vec![terrazgo_core::models::NewSowingPlot {
            plot_id: fx.barley_plot_id.clone(),
            crop_id: Some(fx.barley_crop_id.clone()),
        }],
        ..sowing(&fx)
    };
    terrazgo_core::repository::insert_sowing_record(&mut conn, planted, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entries = block(&doc, "SiembraPlantacion");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["SiembraPlantacion"], 1);
    assert_eq!(entries[1]["SiembraPlantacion"], 0);
    // A stated end date travels; an absent one means one day's work, so the
    // start date is the honest end rather than a fallback.
    assert_eq!(entries[0]["FechaInicio"], "15/10/2025");
    assert_eq!(entries[0]["FechaFin"], "17/10/2025");
    assert_eq!(entries[1]["FechaInicio"], "20/02/2026");
    assert_eq!(entries[1]["FechaFin"], "20/02/2026");
    // The DGC names the crop through its frozen alias.
    assert!(entries[0]["DGCs"][0]["CodigoDGCAjena"].is_i64());
}

#[test]
fn an_unlinked_sowing_claims_no_treated_or_acquired_material() {
    // Nothing is inferred from a date coincidence: the farmer states the link or
    // there is none. `FechaAdquisicion` is required by the schema and carries no
    // `type`, so a null satisfies it — which is what an own-seed sowing sends,
    // rather than a date it does not have.
    let mut conn = db();
    let fx = fixture(&mut conn);
    terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    // A treated-seed record on the same date and the same plot, deliberately
    // NOT linked — under a matching heuristic this would have flipped the flag.
    repo::insert_seed_treatment(&mut conn, seed_treatment(&fx), None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "SiembraPlantacion")[0];
    assert_eq!(entry["MaterialTratado"], false);
    assert_eq!(entry["MaterialAdquirido"], false);
    assert!(entry["FechaAdquisicion"].is_null());
    assert!(entry.get("NumLote").is_none());
}

#[test]
fn a_linked_sowing_reads_its_provenance_from_the_treated_seed_register() {
    // `MaterialAdquirido` needs no column: TIPO_TRATAMIENTO 4 and 5 are literally
    // "adquisición de semilla tratada", so `treatment_kind_code` IS the
    // distinction the member asks about.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let sown =
        terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    let linked = NewSeedTreatment {
        sowing_record_id: Some(sown.record.id.clone()),
        ..seed_treatment(&fx)
    };
    repo::insert_seed_treatment(&mut conn, linked, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "SiembraPlantacion")[0];
    assert_eq!(entry["MaterialTratado"], true);
    assert_eq!(entry["MaterialAdquirido"], true);
    assert_eq!(entry["FechaAdquisicion"], "30/09/2025");
    assert_eq!(entry["NumLote"], "L-2025-4471");
}

#[test]
fn seed_treated_on_the_holding_is_treated_material_that_was_never_bought() {
    // TIPO_TRATAMIENTO 2 — treated in the farm's own store. The two booleans are
    // separate questions, and only one of them is yes.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let sown =
        terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    let own = NewSeedTreatment {
        sowing_record_id: Some(sown.record.id.clone()),
        treatment_kind_code: Some("on_farm".into()),
        seed_lot: None,
        acquired_on: None,
        ..seed_treatment(&fx)
    };
    repo::insert_seed_treatment(&mut conn, own, None).unwrap();

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "SiembraPlantacion")[0];
    assert_eq!(entry["MaterialTratado"], true);
    assert_eq!(entry["MaterialAdquirido"], false);
    assert!(entry["FechaAdquisicion"].is_null());
}

#[test]
fn several_lots_on_one_sowing_send_the_earliest_purchase_and_no_lot_number() {
    // One sowing can use several sacks — the 3.2 register is one row per product
    // — and the block has room for one date and one lot. The earliest purchase is
    // when material for this sowing started being acquired; naming one of two
    // lots would be a false statement about the other, and the member is
    // optional, so silence is available. Each lot still travels on its own
    // UsoSemillaTratada entry.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let sown =
        terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    for (lot, bought_on) in [("L-2025-4471", "2025-09-30"), ("L-2025-5518", "2025-08-11")] {
        let linked = NewSeedTreatment {
            sowing_record_id: Some(sown.record.id.clone()),
            seed_lot: Some(lot.into()),
            acquired_on: Some(bought_on.into()),
            ..seed_treatment(&fx)
        };
        repo::insert_seed_treatment(&mut conn, linked, None).unwrap();
    }

    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    let entry = &block(&doc, "SiembraPlantacion")[0];
    assert_eq!(entry["FechaAdquisicion"], "11/08/2025");
    assert!(entry.get("NumLote").is_none());
    // Both lots are still reported, one activity each.
    assert_eq!(block(&doc, "UsoSemillaTratada").len(), 2);
}

#[test]
fn a_withdrawn_seed_record_stops_claiming_the_sowing_used_treated_material() {
    // The link states something about a LIVE register: a withdrawn 3.2 row no
    // longer asserts that the material was treated, so the sowing's boolean has
    // to follow it back.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let sown =
        terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    let linked = NewSeedTreatment {
        sowing_record_id: Some(sown.record.id.clone()),
        ..seed_treatment(&fx)
    };
    let seed = repo::insert_seed_treatment(&mut conn, linked, None).unwrap();
    assert_eq!(
        block(
            &export_json(&mut conn, &fx.season_id, &fx.farm_id),
            "SiembraPlantacion"
        )[0]["MaterialTratado"],
        true
    );

    repo::soft_delete_seed_treatment(&mut conn, &seed.record.id, None).unwrap();
    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(
        block(&doc, "SiembraPlantacion")[0]["MaterialTratado"],
        false
    );
}

#[test]
fn a_link_may_not_reach_another_farm_or_another_campaign() {
    // The export reads the link to state MaterialTratado on that sowing, so a
    // cross-farm link would put one holding's treated seed in another's
    // descriptor. The foreign key alone would allow it.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let other_farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Otra finca".into(),
            country_code: "es".into(),
            owner_name: None,
            owner_tax_id: None,
            es: None,
        },
        None,
    )
    .unwrap()
    .id;
    let other_plot = insert_plot(&mut conn, &other_farm, "Parcela ajena", 3.0);
    let elsewhere = terrazgo_core::models::NewSowingRecord {
        farm_id: other_farm.clone(),
        plots: vec![terrazgo_core::models::NewSowingPlot {
            plot_id: other_plot,
            crop_id: None,
        }],
        ..sowing(&fx)
    };
    let elsewhere =
        terrazgo_core::repository::insert_sowing_record(&mut conn, elsewhere, None).unwrap();

    let crossed = NewSeedTreatment {
        sowing_record_id: Some(elsewhere.record.id.clone()),
        ..seed_treatment(&fx)
    };
    assert!(matches!(
        repo::insert_seed_treatment(&mut conn, crossed, None).unwrap_err(),
        module_cue::error::CueError::Invalid("sowing_not_on_farm")
    ));
}

#[test]
fn precheck_lists_a_sowing_with_no_seed_weight_or_a_plot_with_no_crop() {
    // `Cantidad` is required by the twin and shown on no printed page, so it is
    // the one field of this register a farmer can leave blank without noticing.
    // A plot with no crop would serialize as a DGC stating nothing at all.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bare = terrazgo_core::models::NewSowingRecord {
        seed_quantity_kg: None,
        plots: vec![terrazgo_core::models::NewSowingPlot {
            plot_id: fx.wheat_plot_id.clone(),
            crop_id: None,
        }],
        ..sowing(&fx)
    };
    terrazgo_core::repository::insert_sowing_record(&mut conn, bare, None).unwrap();

    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.sowing_missing_seed_quantity.len(), 1);
    assert_eq!(report.sowing_missing_seed_quantity[0].kind_code, "sowing");
    assert_eq!(report.sowing_plots_missing_crop.len(), 1);
    assert!(!report.is_clean());
}

#[test]
fn precheck_demands_lot_and_purchase_date_only_of_acquired_seed() {
    // The descriptor's own cross-field rule ("NumeroLote obligatorio si
    // Tratamiento es 4 o 5") plus FechaAdquisicion, which is what makes "the
    // earliest purchase" well defined. Seed treated on the holding is subject to
    // neither, so the rules cannot block a farmer who bought nothing.
    let mut conn = db();
    let fx = fixture(&mut conn);
    let bought = NewSeedTreatment {
        seed_lot: None,
        acquired_on: None,
        ..seed_treatment(&fx)
    };
    repo::insert_seed_treatment(&mut conn, bought, None).unwrap();
    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert_eq!(report.seed_acquired_missing_lot.len(), 1);
    assert_eq!(report.seed_acquired_missing_date.len(), 1);

    let mut conn = db();
    let fx = fixture(&mut conn);
    let own = NewSeedTreatment {
        treatment_kind_code: Some("on_farm".into()),
        seed_lot: None,
        acquired_on: None,
        ..seed_treatment(&fx)
    };
    repo::insert_seed_treatment(&mut conn, own, None).unwrap();
    let report = export_precheck(&conn, &fx.season_id, &fx.farm_id).unwrap();
    assert!(report.seed_acquired_missing_lot.is_empty());
    assert!(report.seed_acquired_missing_date.is_empty());
    assert!(report.is_clean());
}

#[test]
fn withdrawn_sales_and_sowings_become_deletion_entries_under_their_aliases() {
    let mut conn = db();
    let fx = fixture(&mut conn);
    let sale =
        terrazgo_core::repository::insert_harvest_record(&mut conn, harvest(&fx), None).unwrap();
    let sown =
        terrazgo_core::repository::insert_sowing_record(&mut conn, sowing(&fx), None).unwrap();
    let first = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    let sale_alias = block(&first, "ComercializacionVD")[0]["IdAjenaVenta"].clone();
    let sowing_alias = block(&first, "SiembraPlantacion")[0]["IdAjenaSiembraPlant"].clone();

    terrazgo_core::repository::soft_delete_harvest_record(&mut conn, &sale.record.id, None)
        .unwrap();
    terrazgo_core::repository::soft_delete_sowing_record(&mut conn, &sown.record.id, None).unwrap();
    let doc = export_json(&mut conn, &fx.season_id, &fx.farm_id);
    assert_schema_valid(&doc);
    assert_eq!(block(&doc, "ComercializacionVD")[0]["Borrar"], true);
    assert_eq!(
        block(&doc, "ComercializacionVD")[0]["IdAjenaVenta"],
        sale_alias
    );
    assert_eq!(block(&doc, "SiembraPlantacion")[0]["Borrar"], true);
    assert_eq!(
        block(&doc, "SiembraPlantacion")[0]["IdAjenaSiembraPlant"],
        sowing_alias
    );
}

#[test]
fn a_campaign_with_neither_register_omits_both_blocks() {
    // No block is ever obligatory: the container declares no required properties
    // and every block is 0..n, so absence says "none happened" correctly.
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
    assert!(activities.get("ComercializacionVD").is_none());
    assert!(activities.get("SiembraPlantacion").is_none());
}
