// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! **A device that holds only the log can rebuild the tables.** That is what
//! the whole merge rests on, and it is a claim about every register at once, so
//! it is checked the only way it can be: write a real campaign through the real
//! repositories, throw the tables away, replay the log into an empty database
//! through the generic applier, and compare the two row for row.
//!
//! What it actually pins is the payload↔column contract that lets ONE applier
//! serve sixty tables (docs/sync.md → The generic row applier): the logged
//! payload's key set must be the table's column set exactly. That holds by
//! design — `Machinery.kind` carries `#[serde(rename = "type")]` so the payload
//! uses the real column name — but it holds because somebody remembered, in
//! every register, in every crate. A serde attribute added next year breaks it
//! silently: the app keeps working, every test passes, and only a device
//! receiving that row finds out. Here, it fails on the next run.
//!
//! It lives in the shell for the reason `index_contract` does: only the shell
//! sees core's tables and every module's in one schema.
//!
//! # Every synced table, or it fails by name
//!
//! The seed is the demo campaign plus one of every register the demo does not
//! write. The demo lives in `module-phytosanitary`, which may not depend on
//! another module, so on its own it reaches only core's registers and its own.
//! `every_synced_table_is_rebuilt_from_its_log` holds the seed to the whole
//! aggregate map: a table added anywhere, or one the demo stops writing, fails
//! here with its name until something below writes it. That is what makes this
//! the check a crate written afterwards cannot opt out of (docs/sync.md → A
//! crate written afterwards pays none of this) — a helper each crate had to
//! remember to call would cover only the crates that remembered.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use rusqlite::Connection;
use serde_json::Value;
use terrazgo_core::merge::Applier;
use terrazgo_lib::db::composed_migrations;

/// An empty database at the composed schema, able to log writes.
fn fresh_migrated_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    terrazgo_core::sync::install_device(&conn, &terrazgo_core::sync::mint_device_id()).unwrap();
    let shape = terrazgo_lib::registry::composed_sync_shape();
    terrazgo_core::sync::install_shape(&conn, &[&shape]).unwrap();
    conn
}

/// A database holding the demo campaign and one of every register the demo
/// does not write, all through the real repositories.
fn seeded_db() -> Connection {
    let mut conn = fresh_migrated_db();
    let summary = module_phytosanitary::demo::seed_demo(&mut conn).unwrap();
    assert!(summary.seeded, "the fixture seeded nothing");
    the_registers_the_demo_does_not_write(&mut conn);
    conn
}

fn first(conn: &Connection, sql: &str) -> String {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// The other modules' registers, the acts people make on derived state, and
/// the sync layer's own rows — each written once, on the demo's farm and book.
fn the_registers_the_demo_does_not_write(conn: &mut Connection) {
    use module_ecoscheme::models::*;
    use module_ecoscheme::repository as eco;
    use module_fertilisation::models::*;
    use module_fertilisation::repository as fert;
    use terrazgo_core::models::{NewSeason, NewUserProfile, NewZoneFlag};
    use terrazgo_core::repository as core;

    let farm = first(conn, "SELECT id FROM farm ORDER BY id LIMIT 1");
    let book = first(
        conn,
        &format!("SELECT id FROM season WHERE farm_id = '{farm}' ORDER BY id LIMIT 1"),
    );
    let crop = first(
        conn,
        &format!("SELECT id FROM crop WHERE season_id = '{book}' ORDER BY id LIMIT 1"),
    );
    let plot = first(
        conn,
        &format!("SELECT plot_id FROM crop WHERE id = '{crop}'"),
    );

    // Sections 6, 7.1 and 8 of the book: module-fertilisation.
    let material = fert::insert_fertiliser_material(
        conn,
        NewFertiliserMaterial {
            name: "Purín de porcino".into(),
            material_code: "5".into(),
            material_detail_code: None,
            supplier_name: Some("Ganadería del Duero S.L.".into()),
            supplier_rega: Some("ES471820000123".into()),
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: Some("composting".into()),
            density_kg_l: Some(1.03),
            notes: None,
            nutrients: vec![MaterialNutrient {
                id: String::new(),
                kind_code: "macro".into(),
                nutrient_code: "3".into(),
                percentage: 0.4,
            }],
        },
        None,
    )
    .unwrap()
    .material
    .id;
    fert::insert_irrigation_record(
        conn,
        NewIrrigationRecord {
            season_id: book.clone(),
            farm_id: farm.clone(),
            irrigated_on: "2026-06-14".into(),
            irrigation_end_date: Some("2026-06-28".into()),
            irrigation_method_code: "drip".into(),
            volume_value: 320.0,
            volume_unit_code: "m3_ha".into(),
            water_nitric_n_mg_l: Some(12.5),
            water_soluble_p2o5_mg_l: Some(1.8),
            energy_type_code: Some("2".into()),
            meter_number: Some("C-4471".into()),
            notes: None,
            plots: vec![NewIrrigationPlot {
                plot_id: plot.clone(),
                crop_id: Some(crop.clone()),
                irrigated_area_ha: Some(3.5),
            }],
            water_origins: vec!["groundwater".into()],
            // BUENAS_PRACTICAS_AMBITOS, "SI" under "Ámbito Riego".
            practices: vec!["23".into()],
        },
        None,
    )
    .unwrap();
    fert::insert_fertilisation_record(
        conn,
        NewFertilisationRecord {
            season_id: book.clone(),
            farm_id: farm.clone(),
            applied_on: "2026-03-12".into(),
            application_end_date: None,
            fertilisation_type_code: "top_dressing".into(),
            application_method_code: "broadcast".into(),
            dose_value: 250.0,
            dose_unit_code: "kg_ha".into(),
            fertiliser_material_id: material,
            sludge_application: false,
            sustainable_input_management: true,
            irrigation_record_id: None,
            machinery_id: None,
            service_company: None,
            service_regfer_number: None,
            delivery_note_ref: None,
            yield_estimated_kg_ha: None,
            yield_final_kg_ha: None,
            notes: None,
            plots: vec![NewFertilisationPlot {
                plot_id: plot.clone(),
                crop_id: Some(crop.clone()),
                fertilised_area_ha: Some(3.5),
            }],
            // BUENAS_PRACTICAS_AMBITOS, "SI" under "Ámbito Fertilización".
            practices: vec!["4".into()],
        },
        None,
    )
    .unwrap();
    fert::insert_fertilisation_plan(
        conn,
        NewFertilisationPlan {
            season_id: book.clone(),
            farm_id: farm.clone(),
            needs_n_kg_ha: 120.0,
            needs_p2o5_kg_ha: 40.0,
            needs_k2o_kg_ha: 30.0,
            expected_yield_kg_ha: 5000.0,
            preceding_crop_code: None,
            drawn_up_on: "2026-01-10".into(),
            tool_generated: false,
            notes: None,
            crop_ids: vec![crop.clone()],
        },
        None,
    )
    .unwrap();

    // Section 9 of the book: module-ecoscheme. The cover's mowing is a row in
    // the cultural-operations register, so it writes that one too.
    eco::insert_soil_cover(
        conn,
        NewSoilCover {
            season_id: book.clone(),
            farm_id: farm.clone(),
            practice_code: "plant_cover".into(),
            cover_type_code: "2".into(),
            established_on: "2026-02-01".into(),
            width_m: Some(2.0),
            free_canopy_width_m: Some(1.0),
            widths_stated_on: Some("2026-03-01".into()),
            notes: None,
            plot_ids: vec![plot.clone()],
            maintenance: vec![CoverMaintenanceLine {
                id: String::new(),
                kind_code: "mowing".into(),
                performed_on: "2026-04-01".into(),
                performed_end_date: None,
                animals: Vec::new(),
            }],
        },
        None,
    )
    .unwrap();
    eco::insert_grazing_record(
        conn,
        NewGrazingRecord {
            season_id: book.clone(),
            farm_id: farm.clone(),
            practice_code: "extensive_grazing".into(),
            plot_group_ref: None,
            soil_cover_id: None,
            started_on: "2026-05-01".into(),
            ended_on: Some("2026-05-20".into()),
            notes: None,
            plot_ids: vec![plot.clone()],
            animals: vec![GrazingAnimal {
                id: String::new(),
                grazing_record_id: String::new(),
                species_code: "03".into(),
                rega_code: "ES071234560001".into(),
                animal_count: 120,
            }],
        },
        None,
    )
    .unwrap();

    // Core: a zone check, a person, this device, an act on an alert and one on
    // a pair of records, and the number the exporter mints.
    core::replace_zone_flags(
        conn,
        &plot,
        2026,
        "sigpac",
        vec![NewZoneFlag {
            zone_type_code: "nitrate_vulnerable".into(),
            status: "inside".into(),
            coverage_pct: Some(40.0),
            detail: None,
        }],
        None,
    )
    .unwrap();
    core::insert_user_profile(
        conn,
        NewUserProfile {
            display_name: "María".into(),
            operator_id: None,
        },
        None,
    )
    .unwrap();
    core::register_this_device(conn, None).unwrap();
    let operator = first(conn, "SELECT id FROM operator ORDER BY id LIMIT 1");
    core::acknowledge_alert(
        conn,
        module_phytosanitary::alerts::LICENCE_EXPIRY.kind(),
        &operator,
        Some("2026-07-15"),
        None,
    )
    .unwrap();
    let treatments: Vec<String> = conn
        .prepare("SELECT id FROM treatment_record ORDER BY id LIMIT 2")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    core::mark_distinct(
        conn,
        &module_phytosanitary::duplicates::TREATMENT_DUPLICATES,
        &treatments[0],
        &treatments[1],
        None,
    )
    .unwrap();
    core::ensure_export_alias(
        conn,
        terrazgo_siex::SIEX_TARGET,
        "treatment_record",
        &treatments[0],
        "",
        None,
    )
    .unwrap();

    // The purge's marker: a book deleted, and what can go of it erased a
    // month on — a declaration, since the book itself never goes.
    let erased = core::insert_season(
        conn,
        NewSeason {
            farm_id: farm.clone(),
            starts_on: "2019-09-01".into(),
            ends_on: "2020-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    module_phytosanitary::repository::set_register_declaration(
        conn,
        &farm,
        &erased.id,
        "seed_treatment",
        "2020-03-01",
        None,
    )
    .unwrap();
    core::delete_book(conn, &erased.id, &[], None).unwrap();
    let a_month_on = terrazgo_core::date::add_days(
        &terrazgo_core::date::today_utc(),
        core::REMOVED_BOOK_DAYS + 1,
    )
    .unwrap();
    let purged = core::purge_due(conn, &a_month_on, None).unwrap();
    assert!(purged.registers > 0, "the purge erased nothing");
}

/// One logged change, in the order it was written.
struct Change {
    entity_table: String,
    entity_id: String,
    operation: String,
    payload: Value,
}

/// Every change the log holds, oldest first. `record_change.id` is a UUIDv7
/// minted in one process, so ordering by it is the write order — the same rule
/// the applier will use when it replays a change set (docs/sync.md).
fn log_of(conn: &Connection) -> Vec<Change> {
    conn.prepare(
        "SELECT entity_table, entity_id, operation, payload FROM record_change ORDER BY id",
    )
    .unwrap()
    .query_map([], |row| {
        let payload: String = row.get("payload")?;
        Ok(Change {
            entity_table: row.get("entity_table")?,
            entity_id: row.get("entity_id")?,
            operation: row.get("operation")?,
            payload: serde_json::from_str(&payload).unwrap(),
        })
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

/// Replay `changes` into an empty database through the applier alone.
///
/// Foreign keys are DEFERRED rather than ordered, which is the design's own
/// answer (docs/sync.md → Foreign keys are deferred, not ordered) and is
/// exercised here rather than merely asserted: a register may be logged before
/// the row it references — a treated plot is logged before its treatment
/// record — so no apply order satisfies them row by row. Enforcement moves to
/// the commit, where an incomplete replay fails whole.
fn replay(changes: &[Change]) -> Connection {
    let (conn, refused) = try_replay(changes);
    assert!(
        refused.is_empty(),
        "these registers cannot be materialised from their own log payloads:\n{}\n\n\
         The payload's keys must be the table's columns exactly — that is what lets one \
         applier serve every register. A #[serde(rename)], a skipped field, a nested block \
         or a column added without a matching field breaks it. See docs/sync.md → The \
         generic row applier.",
        refused.join("\n")
    );
    conn
}

/// The replay, reporting every register it could not apply rather than the
/// first: whoever fixes one of these wants the whole list, and the failures
/// come in families — one nested block breaks every row of its register.
fn try_replay(changes: &[Change]) -> (Connection, Vec<String>) {
    let mut conn = fresh_migrated_db();
    let mut refused: BTreeSet<String> = BTreeSet::new();
    {
        let tx = conn.transaction().unwrap();
        tx.execute_batch("PRAGMA defer_foreign_keys = ON").unwrap();
        // The applier's own scope: its statements borrow the transaction, so it
        // has to be dropped before `commit()` can consume it.
        {
            let mut applier = Applier::new(&tx);
            for change in changes {
                let applied = match &change.payload["after"] {
                    Value::Null => applier.delete(&change.entity_table, &change.entity_id),
                    after => applier.upsert(&change.entity_table, after),
                };
                if let Err(err) = applied {
                    refused.insert(format!(
                        "  {} (on {}) — {err}",
                        change.entity_table, change.operation
                    ));
                }
            }
        }
        if refused.is_empty() {
            tx.commit().unwrap();
        }
        // Otherwise the transaction rolls back as it drops: a replay missing
        // rows would fail the deferred foreign-key check at commit anyway, and
        // that error would bury the mismatches this is trying to report.
    }
    (conn, refused.into_iter().collect())
}

/// The user tables of the composed schema, in name order.
fn user_tables(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .unwrap()
    .query_map([], |r| r.get::<_, String>(0))
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

/// The tables the aggregate map keeps on each device: never logged, so never
/// rebuilt from a log.
fn device_local_tables(conn: &Connection) -> BTreeSet<String> {
    conn.prepare("SELECT table_name FROM temp.sync_shape WHERE role = 'local'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// One table's contents as comparable text, ordered so two databases that hold
/// the same rows produce the same answer whatever order they were written in.
fn rows_of(conn: &Connection, table: &str) -> Vec<String> {
    let sql = format!("SELECT * FROM \"{table}\"");
    let mut stmt = conn.prepare(&sql).unwrap();
    let width = stmt.column_count();
    let mut rows: Vec<String> = stmt
        .query_map([], |row| {
            let mut cells = Vec::with_capacity(width);
            for index in 0..width {
                cells.push(format!("{:?}", row.get_ref(index)?));
            }
            Ok(cells.join("\u{1f}"))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    rows.sort();
    rows
}

#[test]
fn the_log_alone_rebuilds_every_register_it_recorded() {
    let seeded = seeded_db();
    let rebuilt = replay(&log_of(&seeded));

    let local = device_local_tables(&seeded);
    let mut compared = 0usize;
    for table in user_tables(&seeded) {
        // The log itself is replicated as a set union, not applied; and what
        // the aggregate map calls local — the catalogues, what this file has
        // held, the conflicts it works out — never travels in it.
        if table == "record_change" || local.contains(&table) {
            continue;
        }
        let expected = rows_of(&seeded, &table);
        if expected.is_empty() {
            continue;
        }
        compared += 1;
        assert_eq!(
            rows_of(&rebuilt, &table),
            expected,
            "{table} did not come back the same from its own log"
        );
    }
    assert!(
        compared > 0,
        "nothing was compared — the seed or the table filter is wrong"
    );
}

#[test]
fn every_synced_table_is_rebuilt_from_its_log() {
    // Without this, a seed that stopped writing a register — or a register the
    // seed never knew about — would leave the test above passing on whatever
    // was left. A table counts once the seed both logged it and left rows in
    // it, since the test above compares only tables that hold something.
    let seeded = seeded_db();
    let logged: BTreeSet<String> = log_of(&seeded)
        .into_iter()
        .map(|change| change.entity_table)
        .collect();
    let synced: Vec<String> = seeded
        .prepare("SELECT table_name FROM temp.sync_shape WHERE role <> 'local' ORDER BY table_name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(!synced.is_empty(), "the aggregate map is not installed");

    let unchecked: Vec<&String> = synced
        .iter()
        .filter(|table| !logged.contains(*table) || rows_of(&seeded, table).is_empty())
        .collect();
    assert!(
        unchecked.is_empty(),
        "nothing here writes these synced tables, so nothing checks that their \
         log can rebuild them: {unchecked:?}\n\nWrite one in \
         `the_registers_the_demo_does_not_write`, through its repository."
    );
}
