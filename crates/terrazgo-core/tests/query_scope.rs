// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Does listing core's registers and registries cost more as they fill up?
//!
//! The statements a listing runs must be bounded by the number of CHILD TABLES
//! it hydrates, never by the number of rows. That holds for the two registers
//! that bracket a crop — sowing and harvest — and equally for the three
//! registries that hang a Spanish extension off each row, where the extension
//! read was one query per plot, per machine, per premises.
//!
//! A per-row child query returns exactly the right answer while doing it, which
//! is why no correctness test in this directory can fail on it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rusqlite::Connection;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::query_cost;

/// Few rows against four times as many. Both are small: the defect is a count
/// that MOVES, and four times nothing is still four times.
const FEW: usize = 3;
const MANY: usize = 12;

struct Land {
    season_id: String,
    farm_id: String,
    plot_id: String,
}

fn land(conn: &mut Connection) -> Land {
    let farm = repo::insert_farm(conn, new_farm("Finca La Vega"), None).unwrap();
    let season = repo::insert_season(conn, new_season(&farm.id, 2026, "2025/2026"), None).unwrap();
    let plot = repo::insert_plot(conn, new_plot(&farm.id, "Parcela 1"), None).unwrap();
    Land {
        season_id: season.id,
        farm_id: farm.id,
        plot_id: plot.id,
    }
}

/// Build `count` rows with `insert`, then report what listing them costs.
fn statements_to_list<T>(
    count: usize,
    insert: impl Fn(&mut Connection, &Land, usize),
    list: impl Fn(&Connection, &Land) -> Vec<T>,
) -> (usize, usize) {
    let mut conn = db();
    let fx = land(&mut conn);
    for n in 0..count {
        insert(&mut conn, &fx, n);
    }
    let (listed, cost) = query_cost(&mut conn, |conn| list(conn, &fx));
    (listed.len(), cost.statements)
}

fn day(n: usize) -> String {
    format!("2026-05-{:02}", n + 1)
}

#[test]
fn listing_sowings_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_sowing_record(
            conn,
            NewSowingRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                kind_code: "sowing".into(),
                sown_on: day(n),
                sowing_end_date: None,
                flooded_on: None,
                seed_quantity_kg: Some(180.0),
                notes: None,
                plots: vec![NewSowingPlot {
                    plot_id: fx.plot_id.clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &Land| {
        repo::list_sowing_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(
        few_statements, many_statements,
        "sowing: {few_statements} statements for {FEW} records, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_harvests_costs_the_same_at_four_times_the_records() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_harvest_record(
            conn,
            NewHarvestRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                harvested_on: day(n),
                product_name: "trigo blando".into(),
                plant_product_code: Some("1".into()),
                quantity_value: Some(42.5),
                quantity_unit_code: Some("t".into()),
                delivery_note_ref: None,
                lot_number: None,
                buyer_name: "Cooperativa Cerealista del Duero".into(),
                buyer_tax_id: None,
                buyer_address: None,
                buyer_registry_number: None,
                notes: None,
                plots: vec![NewHarvestPlot {
                    plot_id: fx.plot_id.clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
    };
    let list = |conn: &Connection, fx: &Land| {
        repo::list_harvest_records(conn, &fx.season_id, &fx.farm_id).unwrap()
    };

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

// --- the registries, where the child is an extension row --------------------
//
// An extension is at most one row per parent, so the per-parent read looked
// harmless. It is the same defect: a farm with 400 plots ran 400 point queries
// to draw one list, and a cooperative-sized holding is exactly where the
// registry views are largest.

#[test]
fn listing_plots_costs_the_same_at_four_times_the_plots() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        let mut plot = new_plot(&fx.farm_id, &format!("Recinto {n}"));
        plot.es = Some(PlotEsFields {
            sigpac_province: Some("47".into()),
            sigpac_municipality: Some("186".into()),
            sigpac_aggregate: None,
            sigpac_zone: None,
            sigpac_polygon: Some("12".into()),
            sigpac_parcel: Some(format!("{}", 100 + n)),
            sigpac_enclosure: Some("1".into()),
        });
        repo::insert_plot(conn, plot, None).unwrap();
    };
    let list = |conn: &Connection, fx: &Land| repo::list_plots(conn, &fx.farm_id).unwrap();

    // `land` already made one plot, so both counts carry the same offset.
    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW + 1, MANY + 1));
    assert_eq!(
        few_statements, many_statements,
        "plots: {few_statements} statements for {FEW} extensions, \
         {many_statements} for {MANY}"
    );
}

#[test]
fn listing_machinery_details_costs_the_same_at_four_times_the_machines() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_machinery(
            conn,
            NewMachinery {
                farm_id: fx.farm_id.clone(),
                name: format!("Atomizador {n}"),
                kind: None,
                acquired_on: None,
                last_inspection_date: None,
                next_inspection_due_date: None,
                roma_number: Some(format!("470012345{n}")),
                reganip_number: None,
            },
            None,
        )
        .unwrap();
    };
    let list =
        |conn: &Connection, fx: &Land| repo::list_machinery_details(conn, &fx.farm_id).unwrap();

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

#[test]
fn listing_premises_details_costs_the_same_at_four_times_the_premises() {
    let insert = |conn: &mut Connection, fx: &Land, n: usize| {
        repo::insert_premises(
            conn,
            NewPremises {
                farm_id: fx.farm_id.clone(),
                kind_code: "building".into(),
                name: format!("Almacén {n}"),
                address: Some("Camino de la Vega, 1".into()),
                vehicle_model: None,
                plate: None,
                // EDIFICACIONES_INSTALACIONES 2 = "Almacén de maquinaria".
                class_code: Some("2".into()),
                volume_m3: Some(420.0),
                notes: None,
                cadastral_reference: Some(format!("123456{n:02}AB1234C0001XY")),
                rea_installation_code: None,
            },
            None,
        )
        .unwrap();
    };
    let list =
        |conn: &Connection, fx: &Land| repo::list_premises_details(conn, &fx.farm_id).unwrap();

    let (few, few_statements) = statements_to_list(FEW, insert, list);
    let (many, many_statements) = statements_to_list(MANY, insert, list);
    assert_eq!((few, many), (FEW, MANY));
    assert_eq!(few_statements, many_statements);
}

/// The record book list is the one list that grows with farms × years, so it
/// pages — and a page must cost what a page costs, however many books exist.
/// Counted in ROWS as well as statements: a list that read every book and cut
/// the page in Rust would run the same two statements and produce every row.
#[test]
fn a_page_of_record_books_costs_the_same_at_four_times_the_books() {
    const PAGE: i64 = 10;
    let cost_at = |farms: usize| {
        let mut conn = db();
        for f in 0..farms {
            let farm =
                repo::insert_farm(&mut conn, new_farm(&format!("Finca {f:03}")), None).unwrap();
            for year in 2020..2025 {
                repo::insert_season(
                    &mut conn,
                    new_season(&farm.id, year, &year.to_string()),
                    None,
                )
                .unwrap();
            }
        }
        let (page, cost) = query_cost(&mut conn, |conn| repo::list_seasons(conn, PAGE, 0).unwrap());
        assert_eq!(page.total, (farms * 5) as i64);
        assert_eq!(page.seasons.len(), PAGE as usize);
        cost
    };
    let few = cost_at(FEW);
    let many = cost_at(MANY);
    assert_eq!(few.statements, many.statements, "{few:?} against {many:?}");
    assert_eq!(few.rows, many.rows, "{few:?} against {many:?}");
}

/// One farm's books are read off its own rows: other farms' books — a
/// cooperative's hundreds — add nothing to what picking one of them costs.
#[test]
fn a_farms_books_cost_the_same_at_four_times_the_other_farms() {
    let cost_at = |others: usize| {
        let mut conn = db();
        let farm = repo::insert_farm(&mut conn, new_farm("Finca La Vega"), None).unwrap();
        for year in 2024..2027 {
            repo::insert_season(
                &mut conn,
                new_season(&farm.id, year, &year.to_string()),
                None,
            )
            .unwrap();
        }
        for f in 0..others {
            let other =
                repo::insert_farm(&mut conn, new_farm(&format!("Finca {f:03}")), None).unwrap();
            for year in 2020..2025 {
                repo::insert_season(
                    &mut conn,
                    new_season(&other.id, year, &year.to_string()),
                    None,
                )
                .unwrap();
            }
        }
        let (books, cost) = query_cost(&mut conn, |conn| {
            repo::list_farm_seasons(conn, &farm.id).unwrap()
        });
        assert_eq!(books.len(), 3);
        cost
    };
    let few = cost_at(FEW);
    let many = cost_at(MANY);
    assert_eq!(few.statements, many.statements, "{few:?} against {many:?}");
    assert_eq!(few.rows, many.rows, "{few:?} against {many:?}");
}

/// `count` plots in a nitrate zone, each an alert, plus `history` acts about
/// conditions no alert names any more — what years of use leave behind.
fn zoned(count: usize, history: usize) -> Connection {
    let mut conn = db();
    let fx = land(&mut conn);
    for n in 0..count {
        let plot = repo::insert_plot(
            &mut conn,
            new_plot(&fx.farm_id, &format!("Parcela Z{n}")),
            None,
        )
        .unwrap();
        repo::replace_zone_flags(
            &mut conn,
            &plot.id,
            2026,
            "sigpac",
            vec![NewZoneFlag {
                zone_type_code: "nitrate_vulnerable".into(),
                status: "inside".into(),
                coverage_pct: Some(50.0),
                detail: None,
            }],
            None,
        )
        .unwrap();
    }
    if history > 0 {
        conn.execute_batch(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {history})
             INSERT INTO alert_acknowledgement
               (id, alert_type_code, subject_table, subject_id, due_date, status, created_at)
             SELECT printf('old-%d', i), 'phi_window', 'treatment_record',
                    printf('gone-%d', i), '2024-07-01', 'dismissed', '2024-06-11T09:00:00Z'
             FROM n;"
        ))
        .unwrap();
    }
    conn
}

/// Assembling the alert list: core's own alerts raised, their plots named, the
/// acts on them read — a statement for each, never one per alert.
fn alert_list_cost(conn: &mut Connection) -> (usize, terrazgo_testkit::QueryCost) {
    let (alerts, cost) = query_cost(conn, |conn| {
        let raised = repo::current_alerts(conn).unwrap().raised;
        repo::list_alerts(conn, raised, "2026-06-11").unwrap()
    });
    (alerts.len(), cost)
}

#[test]
fn listing_alerts_costs_the_same_at_four_times_the_alerts() {
    let (few, few_cost) = alert_list_cost(&mut zoned(FEW, 0));
    let (many, many_cost) = alert_list_cost(&mut zoned(MANY, 0));
    // Without this the rest proves nothing: naming no alerts costs nothing.
    assert_eq!((few, many), (FEW, MANY), "the scales must differ in alerts");
    assert_eq!(
        few_cost.statements, many_cost.statements,
        "{few_cost:?} against {many_cost:?}"
    );
}

#[test]
fn acts_about_conditions_long_gone_add_nothing_to_what_listing_reads() {
    // Acts are never pruned, so the table grows for as long as the farmer uses
    // the app. The list reads only the acts on today's subjects: rows and
    // statements alike must not notice the history.
    let (fresh_alerts, fresh) = alert_list_cost(&mut zoned(MANY, 0));
    let (old_alerts, years) = alert_list_cost(&mut zoned(MANY, 2000));
    assert_eq!(fresh_alerts, old_alerts);
    assert_eq!(fresh, years, "{fresh:?} against {years:?}");
}

// ---------------------------------------------------------------------------
// The purge
// ---------------------------------------------------------------------------

#[test]
fn finding_nothing_due_costs_the_same_at_four_times_what_is_kept() {
    // The purge runs at every start and after every import, so what it costs
    // when nothing is due is paid every day. Each register of a book is asked
    // for its removed rows only, on an index holding nothing else: what is
    // kept is never read (docs/sync.md → What it costs a farmer in time).
    //
    // One database measured before and after its history grows, as every
    // counting test here: two databases built apart differ in every id.
    let mut conn = db();
    let fx = land(&mut conn);
    let sow = |conn: &mut Connection, n: usize| {
        repo::insert_sowing_record(
            conn,
            NewSowingRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                kind_code: "sowing".into(),
                sown_on: day(n),
                sowing_end_date: None,
                flooded_on: None,
                seed_quantity_kg: Some(180.0),
                notes: None,
                plots: vec![NewSowingPlot {
                    plot_id: fx.plot_id.clone(),
                    crop_id: None,
                }],
            },
            None,
        )
        .unwrap();
    };
    for n in 0..FEW {
        sow(&mut conn, n);
    }
    let later = terrazgo_core::date::add_days(
        &terrazgo_core::date::today_utc(),
        repo::REMOVED_BOOK_DAYS + 1,
    )
    .unwrap();
    let (erased, few) = query_cost(&mut conn, |conn| {
        repo::purge_due(conn, &later, None).unwrap()
    });
    assert_eq!(erased.registers, 0);
    for n in FEW..MANY {
        sow(&mut conn, n);
    }
    let (_, many) = query_cost(&mut conn, |conn| {
        repo::purge_due(conn, &later, None).unwrap()
    });
    assert_eq!(few, many, "nothing due reads nothing more for what is kept");
}
