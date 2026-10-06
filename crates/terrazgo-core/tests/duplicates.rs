// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Duplicate suspects, against core's own registers (docs/sync.md → Duplicate
//! suspects): what each rule raises and what it leaves alone, which records a
//! list starts from, and what a person's verdict does — on one device and
//! across two.
//!
//! The rules of the modules' registers are tested in each module; what is
//! pinned here is the machinery every rule goes through.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    A, B, CoreFixture, Device, FarmWithPlots, farm_with_plots, one_book, sync, sync_both,
};
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::duplicates::{
    CROP_DUPLICATES, DuplicatePolicy, HARVEST_DUPLICATES, SOWING_DUPLICATES, Scope,
};
use terrazgo_core::merge::CORE_ROW_CAPTIONS;
use terrazgo_core::models::{
    NewCrop, NewHarvestPlot, NewHarvestRecord, NewSeason, NewSowingPlot, NewSowingRecord,
};
use terrazgo_core::repository::{self as repo, DuplicateList};

/// A day inside the fixture's campaign (September 2025 to August 2026).
const TODAY: &str = "2026-06-11";

const POLICIES: [DuplicatePolicy; 3] = [CROP_DUPLICATES, SOWING_DUPLICATES, HARVEST_DUPLICATES];

fn fixture() -> (Connection, CoreFixture) {
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    (conn, fx)
}

fn list(conn: &Connection, scope: Scope) -> DuplicateList {
    repo::list_duplicates(conn, &POLICIES, CORE_ROW_CAPTIONS, scope).unwrap()
}

/// The suspected pairs on the Status view, as `(register, first, second)`.
fn suspected(conn: &Connection) -> Vec<(&'static str, String, String)> {
    list(conn, Scope::Current { today: TODAY })
        .suspects
        .into_iter()
        .map(|pair| {
            let [first, second] = pair.records;
            (pair.register, first.id, second.id)
        })
        .collect()
}

fn pair_of(a: &str, b: &str, register: &'static str) -> (&'static str, String, String) {
    let (first, second) = if a < b { (a, b) } else { (b, a) };
    (register, first.to_owned(), second.to_owned())
}

fn load(
    season_id: &str,
    farm_id: &str,
    day: &str,
    kg: f64,
    note: Option<&str>,
    plots: &[&str],
) -> NewHarvestRecord {
    NewHarvestRecord {
        season_id: season_id.into(),
        farm_id: farm_id.into(),
        harvested_on: day.into(),
        product_name: "Trigo blando".into(),
        plant_product_code: Some("1".into()),
        quantity_value: Some(kg),
        quantity_unit_code: Some("kg".into()),
        delivery_note_ref: note.map(str::to_owned),
        lot_number: None,
        buyer_name: "Cooperativa del Páramo".into(),
        buyer_tax_id: None,
        buyer_address: None,
        buyer_registry_number: None,
        notes: None,
        plots: plots
            .iter()
            .map(|plot| NewHarvestPlot {
                plot_id: (*plot).into(),
                crop_id: None,
            })
            .collect(),
    }
}

fn harvest(conn: &mut Connection, record: NewHarvestRecord) -> String {
    repo::insert_harvest_record(conn, record, None)
        .unwrap()
        .record
        .id
}

fn sowing(
    conn: &mut Connection,
    fx: &CoreFixture,
    kind: &str,
    from: &str,
    to: Option<&str>,
    plots: &[&str],
) -> String {
    repo::insert_sowing_record(
        conn,
        NewSowingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            kind_code: kind.into(),
            sown_on: from.into(),
            sowing_end_date: to.map(str::to_owned),
            flooded_on: None,
            seed_quantity_kg: None,
            notes: None,
            plots: plots
                .iter()
                .map(|plot| NewSowingPlot {
                    plot_id: (*plot).into(),
                    crop_id: None,
                })
                .collect(),
        },
        None,
    )
    .unwrap()
    .record
    .id
}

fn crop(
    conn: &mut Connection,
    fx: &CoreFixture,
    plot: &str,
    species: &str,
    code: Option<&str>,
) -> String {
    repo::insert_crop(
        conn,
        NewCrop {
            plot_id: plot.into(),
            season_id: fx.season_id.clone(),
            species_name: species.into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            crop_code: code.map(str::to_owned),
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap()
    .id
}

// ---------------------------------------------------------------------------
// What a rule raises
// ---------------------------------------------------------------------------

#[test]
fn one_load_recorded_twice_is_listed_once_as_a_pair() {
    let (mut conn, fx) = fixture();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a, &fx.plot_b],
        ),
    );
    // Found from both records, listed once, named smaller id first.
    assert_eq!(
        suspected(&conn),
        vec![pair_of(&first, &second, "harvest_record")]
    );
}

#[test]
fn a_second_load_the_same_day_with_its_own_weight_is_not_a_suspect() {
    // Several loads a day off one plot are the ordinary harvest.
    let (mut conn, fx) = fixture();
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            Some("A-1"),
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            11_400.0,
            Some("A-2"),
            &[&fx.plot_a],
        ),
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_harvest_rule_with_no_slack_does_not_reach_the_next_day() {
    let (mut conn, fx) = fixture();
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-02",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn one_delivery_note_twice_is_one_load_whatever_the_day_or_the_weight() {
    let (mut conn, fx) = fixture();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            Some("A-1"),
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-09",
            9_000.0,
            Some("A-1"),
            &[&fx.plot_b],
        ),
    );
    assert_eq!(
        suspected(&conn),
        vec![pair_of(&first, &second, "harvest_record")]
    );
}

#[test]
fn two_harvests_without_a_delivery_note_are_not_thereby_one_load() {
    // A stated value only: two empty notes agree about nothing.
    let (mut conn, fx) = fixture();
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-09",
            9_000.0,
            None,
            &[&fx.plot_b],
        ),
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_sowing_typed_a_day_out_on_some_of_the_plots_is_a_suspect() {
    // The realistic duplicate: one operator typed the next day, the other
    // listed only the plot they covered.
    let (mut conn, fx) = fixture();
    let first = sowing(
        &mut conn,
        &fx,
        "sowing",
        "2025-10-20",
        None,
        &[&fx.plot_a, &fx.plot_b],
    );
    let second = sowing(&mut conn, &fx, "sowing", "2025-10-21", None, &[&fx.plot_b]);
    assert_eq!(
        suspected(&conn),
        vec![pair_of(&first, &second, "sowing_record")]
    );
}

#[test]
fn a_sowing_two_days_out_or_on_other_plots_or_of_another_kind_is_not() {
    let (mut conn, fx) = fixture();
    sowing(&mut conn, &fx, "sowing", "2025-10-20", None, &[&fx.plot_a]);
    sowing(&mut conn, &fx, "sowing", "2025-10-22", None, &[&fx.plot_a]);
    sowing(&mut conn, &fx, "sowing", "2025-10-20", None, &[&fx.plot_b]);
    sowing(
        &mut conn,
        &fx,
        "planting",
        "2025-10-20",
        None,
        &[&fx.plot_a],
    );
    assert!(suspected(&conn).is_empty(), "{:?}", suspected(&conn));
}

#[test]
fn a_sowing_over_several_days_meets_one_recorded_inside_them() {
    let (mut conn, fx) = fixture();
    let spread = sowing(
        &mut conn,
        &fx,
        "sowing",
        "2025-10-10",
        Some("2025-10-16"),
        &[&fx.plot_a],
    );
    let inside = sowing(&mut conn, &fx, "sowing", "2025-10-14", None, &[&fx.plot_a]);
    assert_eq!(
        suspected(&conn),
        vec![pair_of(&spread, &inside, "sowing_record")]
    );
}

#[test]
fn the_same_crop_added_to_one_plot_twice_is_a_suspect_by_code_or_by_name() {
    let (mut conn, fx) = fixture();
    let coded = crop(&mut conn, &fx, &fx.plot_a, "Trigo blando", Some("1"));
    let recoded = crop(&mut conn, &fx, &fx.plot_a, "trigo", Some("1"));
    // Typed without its code on one device: the name still says it.
    let named = crop(&mut conn, &fx, &fx.plot_b, "Cebada", Some("5"));
    let renamed = crop(&mut conn, &fx, &fx.plot_b, "Cebada", None);
    let mut expected = vec![
        pair_of(&coded, &recoded, "crop"),
        pair_of(&named, &renamed, "crop"),
    ];
    expected.sort_by(|a, b| a.1.cmp(&b.1));
    let mut found = suspected(&conn);
    found.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(found, expected);
}

#[test]
fn two_species_on_one_plot_or_one_species_on_two_plots_are_not() {
    let (mut conn, fx) = fixture();
    crop(&mut conn, &fx, &fx.plot_a, "Trigo blando", Some("1"));
    crop(&mut conn, &fx, &fx.plot_a, "Veza", Some("9"));
    crop(&mut conn, &fx, &fx.plot_b, "Trigo blando", Some("1"));
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_record_on_another_farm_is_never_half_of_a_pair() {
    let (mut conn, fx) = fixture();
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.other_season_id,
            &fx.other_farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.other_farm_plot],
        ),
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_deleted_record_is_never_half_of_a_pair() {
    let (mut conn, fx) = fixture();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    repo::soft_delete_harvest_record(&mut conn, &first, None).unwrap();
    assert!(suspected(&conn).is_empty());
}

// ---------------------------------------------------------------------------
// Which records a list starts from
// ---------------------------------------------------------------------------

#[test]
fn an_old_book_lists_its_pairs_on_its_page_and_not_on_the_status_view() {
    let (mut conn, fx) = fixture();
    let old = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: fx.farm_id.clone(),
            starts_on: "2023-09-01".into(),
            ends_on: "2024-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    let first = harvest(
        &mut conn,
        load(
            &old.id,
            &fx.farm_id,
            "2024-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &old.id,
            &fx.farm_id,
            "2024-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );

    assert!(
        suspected(&conn).is_empty(),
        "a campaign that ended two years ago is not what is waiting now"
    );
    let page = list(&conn, Scope::Book { season_id: &old.id });
    assert_eq!(page.suspects.len(), 1, "its own page still lists it");
    let [a, b] = &page.suspects[0].records;
    assert_eq!(
        (a.id.as_str(), b.id.as_str()),
        (
            pair_of(&first, &second, "").1.as_str(),
            pair_of(&first, &second, "").2.as_str()
        )
    );
}

#[test]
fn the_campaign_before_the_current_one_is_still_current() {
    // Late entries and delayed imports land in the book that just closed.
    let (mut conn, fx) = fixture();
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-07-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-07-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let later = list(
        &conn,
        Scope::Current {
            today: "2027-03-15",
        },
    );
    assert_eq!(later.suspects.len(), 1);
    let much_later = list(
        &conn,
        Scope::Current {
            today: "2027-09-15",
        },
    );
    assert!(much_later.suspects.is_empty());
}

#[test]
fn a_pair_across_two_books_of_one_campaign_is_listed() {
    // Two devices that each started a book for one campaign, with dates a
    // fortnight apart: exactly the pairs worth finding.
    let (mut conn, fx) = fixture();
    let twin = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: fx.farm_id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-09-14".into(),
            custom_label: Some("Campaña del móvil".into()),
        },
        None,
    )
    .unwrap();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &twin.id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    assert_eq!(
        suspected(&conn),
        vec![pair_of(&first, &second, "harvest_record")]
    );
}

// ---------------------------------------------------------------------------
// How a pair is named and ordered
// ---------------------------------------------------------------------------

fn profile(conn: &mut Connection, name: &str) -> String {
    repo::insert_user_profile(
        conn,
        terrazgo_core::models::NewUserProfile {
            display_name: name.into(),
            operator_id: None,
        },
        None,
    )
    .unwrap()
    .id
}

/// A harvest recorded by `author`, or by nobody named.
fn harvest_by(conn: &mut Connection, record: NewHarvestRecord, author: Option<&str>) -> String {
    repo::insert_harvest_record(conn, record, author)
        .unwrap()
        .record
        .id
}

#[test]
fn each_record_is_named_by_its_book_its_day_and_who_wrote_it() {
    let (mut conn, fx) = fixture();
    let maria = profile(&mut conn, "María");
    let juan = profile(&mut conn, "Juan");
    let day = "2026-06-01";
    let first = harvest_by(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
        Some(&maria),
    );
    harvest_by(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
        Some(&juan),
    );

    let listed = list(&conn, Scope::Current { today: TODAY });
    let named = listed.suspects[0]
        .records
        .iter()
        .find(|r| r.id == first)
        .unwrap();
    assert_eq!(named.season_id.as_deref(), Some(fx.season_id.as_str()));
    assert!(named.season_label.is_some());
    assert_eq!(
        named.farm_name.as_deref(),
        Some("Finca La Vega"),
        "the book's farm too: two farms may each keep a book of one name"
    );
    assert_eq!(
        named.caption.as_deref(),
        Some(day),
        "a harvest is known by its day"
    );
    assert_eq!(named.day.as_deref(), Some(day));
    assert_eq!(named.written_by.as_deref(), Some(maria.as_str()));
    assert_eq!(named.author_name.as_deref(), Some("María"));
    assert!(named.written_on.is_some() && named.written_at.is_some());
    assert!(listed.suspects[0].apart, "María and Juan are two people");
}

#[test]
fn a_record_nobody_is_named_on_is_not_a_second_person() {
    // Most writes made before profiles existed name nobody. Reading that as a
    // second author would rank every such pair as two people's.
    let (mut conn, fx) = fixture();
    let maria = profile(&mut conn, "María");
    let day = "2026-06-01";
    harvest_by(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
        Some(&maria),
    );
    harvest_by(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
        None,
    );
    let listed = list(&conn, Scope::Current { today: TODAY });
    assert!(!listed.suspects[0].apart);
}

#[test]
fn pairs_written_apart_come_first_then_the_latest() {
    let (mut conn, fx) = fixture();
    let maria = profile(&mut conn, "María");
    let juan = profile(&mut conn, "Juan");
    // One person, recent.
    for _ in 0..2 {
        harvest_by(
            &mut conn,
            load(
                &fx.season_id,
                &fx.farm_id,
                "2026-06-20",
                5_000.0,
                None,
                &[&fx.plot_a],
            ),
            Some(&maria),
        );
    }
    // Older, but written by two people.
    for author in [&maria, &juan] {
        harvest_by(
            &mut conn,
            load(
                &fx.season_id,
                &fx.farm_id,
                "2026-06-01",
                7_000.0,
                None,
                &[&fx.plot_b],
            ),
            Some(author),
        );
    }

    let listed = list(&conn, Scope::Current { today: TODAY });
    let order: Vec<(bool, Option<&str>)> = listed
        .suspects
        .iter()
        .map(|pair| (pair.apart, pair.records[0].day.as_deref()))
        .collect();
    assert_eq!(
        order,
        vec![(true, Some("2026-06-01")), (false, Some("2026-06-20"))]
    );
}

// ---------------------------------------------------------------------------
// "Both are real"
// ---------------------------------------------------------------------------

fn two_loads(conn: &mut Connection, fx: &CoreFixture) -> (String, String) {
    let first = harvest(
        conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    (first, second)
}

fn verdicts(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM duplicate_verdict", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_pair_judged_distinct_is_not_listed_again() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    // Either order: the act names the pair smaller id first.
    repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &second, &first, None).unwrap();
    assert!(suspected(&conn).is_empty());
    let (id, first_id, second_id): (String, String, String) = conn
        .query_row(
            "SELECT id, first_id, second_id FROM duplicate_verdict",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let (_, a, b) = pair_of(&first, &second, "");
    assert_eq!((first_id, second_id), (a, b), "named smaller id first");
    let (operation, _, after) = common::last_change(&conn, "duplicate_verdict", &id);
    assert_eq!(operation, "insert", "the act is logged, so it travels");
    assert_eq!(after["verdict"], "distinct");
}

#[test]
fn judging_a_pair_distinct_twice_writes_one_act() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &second, None).unwrap();
    repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &second, None).unwrap();
    assert_eq!(verdicts(&conn), 1);
}

#[test]
fn a_verdict_on_one_pair_says_nothing_about_another() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    let third = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &second, None).unwrap();
    let mut left = suspected(&conn);
    left.sort();
    let mut expected = vec![
        pair_of(&first, &third, "harvest_record"),
        pair_of(&second, &third, "harvest_record"),
    ];
    expected.sort();
    assert_eq!(left, expected);
}

#[test]
fn a_record_cannot_be_judged_against_itself_nor_one_that_is_not_there() {
    let (mut conn, fx) = fixture();
    let (first, _) = two_loads(&mut conn, &fx);
    assert!(matches!(
        repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &first, None),
        Err(CoreError::Invalid("duplicate_pair_one_record"))
    ));
    assert!(matches!(
        repo::mark_distinct(
            &mut conn,
            &HARVEST_DUPLICATES,
            &first,
            "no-such-record",
            None
        ),
        Err(CoreError::NotFound)
    ));
    // A record of another register is not in this one.
    let sown = sowing(&mut conn, &fx, "sowing", "2025-10-20", None, &[&fx.plot_a]);
    assert!(matches!(
        repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &sown, None),
        Err(CoreError::NotFound)
    ));
    assert_eq!(verdicts(&conn), 0);
}

// ---------------------------------------------------------------------------
// "Keep this one"
// ---------------------------------------------------------------------------

fn keep(conn: &mut Connection, kept: &str, removed: &str) -> Result<(), CoreError> {
    repo::keep_duplicate(
        conn,
        &HARVEST_DUPLICATES,
        kept,
        removed,
        None,
        repo::soft_delete_harvest_record_tx,
    )
}

fn is_deleted(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        &format!("SELECT deleted_at IS NOT NULL FROM {table} WHERE id = ?1"),
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn keeping_one_removes_the_other_in_one_change_set_with_the_act() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    keep(&mut conn, &first, &second).unwrap();

    assert!(!is_deleted(&conn, "harvest_record", &first));
    assert!(
        is_deleted(&conn, "harvest_record", &second),
        "soft-deleted, never dropped"
    );
    assert!(suspected(&conn).is_empty());

    // The audit trail reads the removal and its reason under one change-set
    // key: that is where the reason lives, since no register has a column
    // for one.
    let sets: Vec<(String, i64)> = conn
        .prepare(
            "SELECT DISTINCT origin_device, origin_seq FROM record_change
             WHERE (entity_table = 'harvest_record' AND entity_id = ?1 AND operation = 'delete')
                OR entity_table = 'duplicate_verdict'",
        )
        .unwrap()
        .query_map([&second], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(sets.len(), 1, "one change set: {sets:?}");
    let (verdict, kept): (String, String) = conn
        .query_row("SELECT verdict, kept_id FROM duplicate_verdict", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(
        (verdict.as_str(), kept.as_str()),
        ("duplicate", first.as_str())
    );
}

#[test]
fn keeping_a_record_that_is_gone_is_refused_and_removes_nothing() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    repo::soft_delete_harvest_record(&mut conn, &first, None).unwrap();
    assert!(matches!(
        keep(&mut conn, &first, &second),
        Err(CoreError::Invalid("duplicate_kept_gone"))
    ));
    assert!(!is_deleted(&conn, "harvest_record", &second));
    assert_eq!(verdicts(&conn), 0);
}

#[test]
fn removing_a_record_somebody_already_removed_writes_no_act() {
    // The register's own delete refuses a record it no longer holds, and the
    // act goes with it: nothing is committed.
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    repo::soft_delete_harvest_record(&mut conn, &second, None).unwrap();
    assert!(matches!(
        keep(&mut conn, &first, &second),
        Err(CoreError::NotFound)
    ));
    assert_eq!(verdicts(&conn), 0);
}

#[test]
fn a_removal_that_deletes_the_wrong_record_is_refused_whole() {
    // The pairing of a register and its delete is made by hand in the shell. A
    // delete that removed something other than the named record must not be
    // committed under an act saying it removed that record.
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    let bystander = sowing(&mut conn, &fx, "sowing", "2025-10-20", None, &[&fx.plot_a]);
    let refused = repo::keep_duplicate(
        &mut conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        None,
        |tx, _| repo::soft_delete_sowing_record_tx(tx, &bystander),
    );
    assert!(
        matches!(refused, Err(CoreError::ShapeViolation(_))),
        "{refused:?}"
    );
    assert!(
        !is_deleted(&conn, "sowing_record", &bystander),
        "rolled back"
    );
    assert_eq!(verdicts(&conn), 0);
}

// ---------------------------------------------------------------------------
// Two devices
// ---------------------------------------------------------------------------

/// A book on `phone`, two loads of one harvest in it, and the whole of it
/// carried to `laptop`.
fn two_loads_on_two_devices(phone: &mut Device, laptop: &mut Device) -> (String, String) {
    let (farm, season, plot, _) = one_book(phone);
    let first = harvest(
        &mut phone.conn,
        load(&season, &farm, "2026-06-01", 12_000.0, None, &[&plot]),
    );
    let second = harvest(
        &mut phone.conn,
        load(&season, &farm, "2026-06-01", 12_000.0, None, &[&plot]),
    );
    sync(phone, laptop);
    (first, second)
}

#[test]
fn a_verdict_travels_and_two_devices_judging_one_pair_do_not_conflict() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (first, second) = two_loads_on_two_devices(&mut phone, &mut laptop);
    assert_eq!(suspected(&laptop.conn).len(), 1);

    repo::mark_distinct(&mut phone.conn, &HARVEST_DUPLICATES, &first, &second, None).unwrap();
    repo::mark_distinct(&mut laptop.conn, &HARVEST_DUPLICATES, &second, &first, None).unwrap();
    sync_both(&mut phone, &mut laptop);

    for device in [&phone, &laptop] {
        assert!(suspected(&device.conn).is_empty());
        let waiting: i64 = device
            .conn
            .query_row("SELECT COUNT(*) FROM sync_conflict", [], |r| r.get(0))
            .unwrap();
        assert_eq!(waiting, 0, "two acts are two registers, never a conflict");
        assert_eq!(verdicts(&device.conn), 2);
    }
}

#[test]
fn a_pair_arriving_by_sync_is_listed_on_the_receiving_device() {
    // Nothing runs at import: the list is worked out when it is read.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (farm, season, plot, _) = one_book(&mut phone);
    sync(&phone, &mut laptop);
    harvest(
        &mut phone.conn,
        load(&season, &farm, "2026-06-01", 12_000.0, None, &[&plot]),
    );
    harvest(
        &mut laptop.conn,
        load(&season, &farm, "2026-06-02", 12_000.0, Some("X"), &[&plot]),
    );
    assert!(suspected(&laptop.conn).is_empty());
    sync(&phone, &mut laptop);
    assert!(
        suspected(&laptop.conn).is_empty(),
        "a day apart: the harvest rule forgives no day"
    );

    harvest(
        &mut phone.conn,
        load(&season, &farm, "2026-06-02", 12_000.0, Some("X"), &[&plot]),
    );
    sync(&phone, &mut laptop);
    let listed = list(&laptop.conn, Scope::Current { today: TODAY });
    assert_eq!(listed.suspects.len(), 1);
    assert!(
        listed.suspects[0].apart,
        "written on two devices, which ranks it first"
    );
}

/// Opposite verdicts on two devices before either hears of the other: the
/// phone keeps `first`, the laptop keeps `second`.
fn removed_twice(phone: &mut Device, laptop: &mut Device) -> (String, String) {
    let (first, second) = two_loads_on_two_devices(phone, laptop);
    repo::keep_duplicate(
        &mut phone.conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    repo::keep_duplicate(
        &mut laptop.conn,
        &HARVEST_DUPLICATES,
        &second,
        &first,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    sync_both(phone, laptop);
    (first, second)
}

#[test]
fn two_opposite_removals_leave_the_operation_nowhere_and_are_listed_as_such() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (first, second) = removed_twice(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert!(is_deleted(&device.conn, "harvest_record", &first));
        assert!(is_deleted(&device.conn, "harvest_record", &second));
        let listed = list(&device.conn, Scope::Current { today: TODAY });
        assert!(listed.suspects.is_empty());
        let removed: Vec<(String, String)> = listed
            .both_removed
            .iter()
            .map(|pair| (pair.records[0].id.clone(), pair.records[1].id.clone()))
            .collect();
        let (_, a, b) = pair_of(&first, &second, "");
        assert_eq!(removed, vec![(a, b)]);
    }
}

#[test]
fn restoring_one_of_two_opposite_removals_brings_it_back_everywhere() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (first, second) = removed_twice(&mut phone, &mut laptop);
    repo::restore_removed_duplicate(&mut phone.conn, &HARVEST_DUPLICATES, &first, None).unwrap();
    sync(&phone, &mut laptop);
    for device in [&phone, &laptop] {
        assert!(!is_deleted(&device.conn, "harvest_record", &first));
        assert!(is_deleted(&device.conn, "harvest_record", &second));
        let listed = list(&device.conn, Scope::Current { today: TODAY });
        assert!(listed.both_removed.is_empty() && listed.suspects.is_empty());
    }
    // The restored record is the one its first writer made, not a copy.
    let buyer: String = laptop
        .conn
        .query_row(
            "SELECT buyer_name FROM harvest_record WHERE id = ?1",
            [&first],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(buyer, "Cooperativa del Páramo");
}

#[test]
fn a_record_removed_once_is_not_restored_this_way() {
    // Removed by one verdict whose kept record is live: that is a decision,
    // not two decisions that cancelled each other out.
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    keep(&mut conn, &first, &second).unwrap();
    assert!(matches!(
        repo::restore_removed_duplicate(&mut conn, &HARVEST_DUPLICATES, &second, None),
        Err(CoreError::Invalid("duplicate_restore_refused"))
    ));
    assert!(matches!(
        repo::restore_removed_duplicate(&mut conn, &HARVEST_DUPLICATES, &first, None),
        Err(CoreError::Invalid("duplicate_restore_refused")),
    ));
}

#[test]
fn a_record_written_to_since_its_removal_is_not_restored_over_that_write() {
    // A third device corrected the record before hearing it was removed. The
    // register now has two versions — the removal live, the correction waiting
    // for a person — and a restore would pick between them without asking.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut tablet = Device::new(common::C);
    let (first, second) = two_loads_on_two_devices(&mut phone, &mut laptop);
    sync(&phone, &mut tablet);
    let stored = repo::get_harvest_record(&tablet.conn, &first).unwrap();
    repo::update_harvest_record(
        &mut tablet.conn,
        &first,
        terrazgo_core::models::UpdateHarvestRecord {
            harvested_on: stored.record.harvested_on,
            product_name: stored.record.product_name,
            plant_product_code: stored.record.plant_product_code,
            quantity_value: stored.record.quantity_value,
            quantity_unit_code: stored.record.quantity_unit_code,
            delivery_note_ref: None,
            lot_number: None,
            buyer_name: stored.record.buyer_name,
            buyer_tax_id: None,
            buyer_address: None,
            buyer_registry_number: None,
            notes: Some("corregido en la tableta".into()),
            plots: stored
                .plots
                .iter()
                .map(|plot| NewHarvestPlot {
                    plot_id: plot.plot_id.clone(),
                    crop_id: None,
                })
                .collect(),
        },
        None,
    )
    .unwrap();
    // After the correction, so the removal is the version the clock makes live.
    repo::keep_duplicate(
        &mut phone.conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    repo::keep_duplicate(
        &mut laptop.conn,
        &HARVEST_DUPLICATES,
        &second,
        &first,
        None,
        repo::soft_delete_harvest_record_tx,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    sync(&tablet, &mut phone);

    assert!(is_deleted(&phone.conn, "harvest_record", &first));
    assert!(phone.conflicted("harvest_record", &first));
    assert!(matches!(
        repo::restore_removed_duplicate(&mut phone.conn, &HARVEST_DUPLICATES, &first, None),
        Err(CoreError::Invalid("duplicate_restore_refused"))
    ));
    // The other one had no second version, and comes back.
    repo::restore_removed_duplicate(&mut phone.conn, &HARVEST_DUPLICATES, &second, None).unwrap();
    assert!(!is_deleted(&phone.conn, "harvest_record", &second));
}

// ---------------------------------------------------------------------------
// Looking at a pair
// ---------------------------------------------------------------------------

/// One line of a pair's review, found by its table and column; for a child
/// table, the line whose values include `named` on some side.
fn pair_line<'a>(
    review: &'a repo::PairReview,
    table: &str,
    column: &str,
    named: Option<&str>,
) -> &'a terrazgo_core::merge::ReviewLine {
    review
        .lines
        .iter()
        .find(|line| {
            line.table == table
                && line.column == column
                && named.is_none_or(|name| pair_values(line).contains(&Some(name.to_owned())))
        })
        .unwrap_or_else(|| panic!("no {table}.{column} line naming {named:?}"))
}

/// A line's values as text, in the order of the pair's records: what a
/// reference names where it names something, `None` for a side with nothing.
fn pair_values(line: &terrazgo_core::merge::ReviewLine) -> Vec<Option<String>> {
    line.values
        .iter()
        .map(|value| {
            let value = value.as_ref()?;
            if let Some(display) = &value.display {
                return Some(display.clone());
            }
            match &value.value {
                serde_json::Value::Null => None,
                serde_json::Value::String(text) => Some(text.clone()),
                other => Some(other.to_string()),
            }
        })
        .collect()
}

#[test]
fn a_pair_is_shown_side_by_side_with_what_it_shares_and_where_it_parts() {
    let (mut conn, fx) = fixture();
    let day = "2026-06-01";
    let mut one = load(
        &fx.season_id,
        &fx.farm_id,
        day,
        12_000.0,
        None,
        &[&fx.plot_a],
    );
    one.lot_number = Some("L-7".into());
    let mut other = load(
        &fx.season_id,
        &fx.farm_id,
        day,
        12_000.0,
        None,
        &[&fx.plot_a, &fx.plot_b],
    );
    other.notes = Some("segundo remolque".into());
    let with_lot = harvest(&mut conn, one);
    let with_notes = harvest(&mut conn, other);

    // Asked either way round, the records come smaller id first, as a verdict
    // names them, and every line's values follow them.
    let review = repo::review_pair(
        &conn,
        &HARVEST_DUPLICATES,
        &with_notes,
        &with_lot,
        CORE_ROW_CAPTIONS,
    )
    .unwrap();
    let (_, first, second) = pair_of(&with_lot, &with_notes, "");
    assert_eq!(review.register, "harvest_record");
    assert_eq!(
        [review.records[0].id.as_str(), review.records[1].id.as_str()],
        [first.as_str(), second.as_str()]
    );
    let side = |lot: Option<&str>, notes: Option<&str>| -> Vec<Option<String>> {
        let (a, b) = if first == with_lot {
            (lot, notes)
        } else {
            (notes, lot)
        };
        vec![a.map(str::to_owned), b.map(str::to_owned)]
    };

    // What they share is shown, and marked as shared: it is why they were put
    // side by side.
    let shared = pair_line(&review, "harvest_record", "harvested_on", None);
    assert!(!shared.differs && shared.root);
    assert_eq!(pair_values(shared), [Some(day.into()), Some(day.into())]);
    assert!(!pair_line(&review, "harvest_record", "quantity_value", None).differs);

    // Where they part is marked, the missing side left empty.
    let lot = pair_line(&review, "harvest_record", "lot_number", None);
    assert!(lot.differs);
    assert_eq!(pair_values(lot), side(Some("L-7"), None));
    let notes = pair_line(&review, "harvest_record", "notes", None);
    assert_eq!(pair_values(notes), side(None, Some("segundo remolque")));

    // A plot both list sits on one line, named; a plot only one lists stands
    // alone. Neither is matched by row id — the two records' plot rows have
    // ids of their own.
    let both = pair_line(&review, "harvest_plot", "plot_id", Some("El Prado"));
    assert!(!both.differs && !both.root);
    assert_eq!(
        pair_values(both),
        [Some("El Prado".into()), Some("El Prado".into())]
    );
    let alone = pair_line(&review, "harvest_plot", "plot_id", Some("La Loma"));
    assert!(alone.differs);
    assert_eq!(pair_values(alone), side(None, Some("La Loma")));
    let plot_lines = review
        .lines
        .iter()
        .filter(|line| line.table == "harvest_plot" && line.column == "plot_id")
        .count();
    assert_eq!(plot_lines, 2, "El Prado once, La Loma once");

    // Nothing that stamps a row, files it, or ties a child to its record.
    for column in [
        "id",
        "created_at",
        "updated_at",
        "season_id",
        "farm_id",
        "harvest_record_id",
        "deleted_at",
    ] {
        assert!(
            review.lines.iter().all(|line| line.column != column),
            "{column} is not something a person compares"
        );
    }

    // Each record is named the way the list names it.
    assert_eq!(review.records[0].day.as_deref(), Some(day));
    assert!(review.records[0].season_label.is_some());
    assert!(review.records[1].written_at.is_some());
}

#[test]
fn a_pair_removed_twice_over_is_shown_as_it_was() {
    // How a person looks at the two copies before restoring one of them.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (first, second) = removed_twice(&mut phone, &mut laptop);
    let review = repo::review_pair(
        &laptop.conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        CORE_ROW_CAPTIONS,
    )
    .unwrap();
    let removed = pair_line(&review, "harvest_record", "deleted_at", None);
    assert!(pair_values(removed).iter().all(Option::is_some));
    assert!(!pair_line(&review, "harvest_record", "harvested_on", None).differs);
}

#[test]
fn a_pair_is_two_records_of_the_register_it_names() {
    let (mut conn, fx) = fixture();
    let (first, _) = two_loads(&mut conn, &fx);
    let review = |a: &str, b: &str| {
        repo::review_pair(&conn, &HARVEST_DUPLICATES, a, b, CORE_ROW_CAPTIONS).map(|_| ())
    };
    assert!(matches!(
        review(&first, &first),
        Err(CoreError::Invalid("duplicate_pair_one_record"))
    ));
    assert!(matches!(
        review(&first, "no-such-record"),
        Err(CoreError::NotFound)
    ));
    let sown = sowing(&mut conn, &fx, "sowing", "2025-10-20", None, &[&fx.plot_a]);
    assert!(matches!(
        repo::review_pair(&conn, &HARVEST_DUPLICATES, &first, &sown, CORE_ROW_CAPTIONS),
        Err(CoreError::NotFound)
    ));
}

// ---------------------------------------------------------------------------
// Right after a save
// ---------------------------------------------------------------------------

fn saved(conn: &Connection, table: &str, id: &str) -> repo::SavedDuplicates {
    repo::list_saved_duplicates(conn, &POLICIES, CORE_ROW_CAPTIONS, table, id).unwrap()
}

fn saved_pairs(found: &repo::SavedDuplicates) -> Vec<(&'static str, String, String)> {
    found
        .suspects
        .iter()
        .map(|pair| {
            (
                pair.register,
                pair.records[0].id.clone(),
                pair.records[1].id.clone(),
            )
        })
        .collect()
}

/// Correct a harvest's weight, as its form would.
fn reweigh(conn: &mut Connection, id: &str, kg: f64, notes: Option<&str>) {
    let stored = repo::get_harvest_record(conn, id).unwrap();
    repo::update_harvest_record(
        conn,
        id,
        terrazgo_core::models::UpdateHarvestRecord {
            harvested_on: stored.record.harvested_on,
            product_name: stored.record.product_name,
            plant_product_code: stored.record.plant_product_code,
            quantity_value: Some(kg),
            quantity_unit_code: stored.record.quantity_unit_code,
            delivery_note_ref: stored.record.delivery_note_ref,
            lot_number: None,
            buyer_name: stored.record.buyer_name,
            buyer_tax_id: None,
            buyer_address: None,
            buyer_registry_number: None,
            notes: notes.map(str::to_owned),
            plots: stored
                .plots
                .iter()
                .map(|plot| NewHarvestPlot {
                    plot_id: plot.plot_id.clone(),
                    crop_id: None,
                })
                .collect(),
        },
        None,
    )
    .unwrap();
}

#[test]
fn a_record_saved_like_one_already_there_is_found_the_moment_it_is_saved() {
    let (mut conn, fx) = fixture();
    let day = "2026-06-01";
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let lighter = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            5_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    assert!(
        saved(&conn, "harvest_record", &lighter).suspects.is_empty(),
        "another load the same day, with its own weight, is not asked about"
    );

    let second = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            day,
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let found = saved(&conn, "harvest_record", &second);
    assert_eq!(found.saved, vec![second.clone()], "what the save wrote");
    assert_eq!(
        saved_pairs(&found),
        vec![pair_of(&first, &second, "harvest_record")]
    );
    // Named as the list names them, so the review shows who wrote the other.
    assert!(
        found.suspects[0]
            .records
            .iter()
            .all(|record| record.written_at.is_some() && record.day.as_deref() == Some(day))
    );
}

#[test]
fn an_edit_is_asked_too_and_a_pair_judged_distinct_stays_answered() {
    let (mut conn, fx) = fixture();
    let (first, second) = two_loads(&mut conn, &fx);
    repo::mark_distinct(&mut conn, &HARVEST_DUPLICATES, &first, &second, None).unwrap();
    reweigh(&mut conn, &second, 12_000.0, Some("corregido"));
    assert!(
        saved(&conn, "harvest_record", &second).suspects.is_empty(),
        "a correction does not reopen a question somebody answered"
    );

    // An edit that makes a record match is asked about, from the edited side.
    let third = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            5_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    assert!(saved(&conn, "harvest_record", &third).suspects.is_empty());
    reweigh(&mut conn, &third, 12_000.0, None);
    let mut found = saved_pairs(&saved(&conn, "harvest_record", &third));
    found.sort();
    let mut expected = vec![
        pair_of(&first, &third, "harvest_record"),
        pair_of(&second, &third, "harvest_record"),
    ];
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn a_sowing_saved_inside_a_longer_one_begun_weeks_before_is_found() {
    // What is read around a record reaches back to the records of its book
    // still going on when it began, not only to the ones starting near it.
    let (mut conn, fx) = fixture();
    let long = sowing(
        &mut conn,
        &fx,
        "sowing",
        "2025-10-01",
        Some("2025-10-31"),
        &[&fx.plot_a],
    );
    let inside = sowing(&mut conn, &fx, "sowing", "2025-10-25", None, &[&fx.plot_a]);
    assert_eq!(
        saved_pairs(&saved(&conn, "sowing_record", &inside)),
        vec![pair_of(&long, &inside, "sowing_record")]
    );
    let after = sowing(&mut conn, &fx, "sowing", "2025-11-02", None, &[&fx.plot_a]);
    assert!(
        saved(&conn, "sowing_record", &after).suspects.is_empty(),
        "two days after the long sowing ended: out of reach"
    );
}

#[test]
fn a_delivery_note_saved_twice_is_found_whatever_the_day() {
    // The harvest's book rule, which no window of days would reach.
    let (mut conn, fx) = fixture();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            Some("A-1"),
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-20",
            5_000.0,
            Some("A-1"),
            &[&fx.plot_b],
        ),
    );
    assert_eq!(
        saved_pairs(&saved(&conn, "harvest_record", &second)),
        vec![pair_of(&first, &second, "harvest_record")]
    );
}

#[test]
fn a_record_in_another_book_of_the_campaign_is_found() {
    let (mut conn, fx) = fixture();
    let twin = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: fx.farm_id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-09-14".into(),
            custom_label: Some("Campaña del móvil".into()),
        },
        None,
    )
    .unwrap();
    let first = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    let second = harvest(
        &mut conn,
        load(
            &twin.id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    assert_eq!(
        saved_pairs(&saved(&conn, "harvest_record", &second)),
        vec![pair_of(&first, &second, "harvest_record")]
    );
}

#[test]
fn what_a_save_finds_is_what_the_book_page_lists() {
    // Every shape core's rules read, in one farm: crops by code and by name,
    // sowings of periods that nest and chain and of another kind, loads on one
    // day and under one note, a record in a second book of the campaign, and a
    // removed one. For every live record, the check after a save and its
    // book's page must name the same pairs.
    let (mut conn, fx) = fixture();
    let twin = repo::insert_season(
        &mut conn,
        NewSeason {
            farm_id: fx.farm_id.clone(),
            starts_on: "2025-09-15".into(),
            ends_on: "2026-09-14".into(),
            custom_label: Some("Campaña del móvil".into()),
        },
        None,
    )
    .unwrap();
    crop(&mut conn, &fx, &fx.plot_a, "Trigo blando", Some("1"));
    crop(&mut conn, &fx, &fx.plot_a, "Trigo blando", None);
    crop(&mut conn, &fx, &fx.plot_a, "Cebada", Some("1"));
    crop(&mut conn, &fx, &fx.plot_b, "Trigo blando", Some("1"));
    for (kind, from, to, plots) in [
        ("sowing", "2025-10-01", Some("2025-10-31"), vec![&fx.plot_a]),
        ("sowing", "2025-10-25", None, vec![&fx.plot_a, &fx.plot_b]),
        ("sowing", "2025-11-01", Some("2025-11-03"), vec![&fx.plot_b]),
        ("sowing", "2025-11-04", None, vec![&fx.plot_b]),
        ("planting", "2025-10-02", None, vec![&fx.plot_a]),
        ("sowing", "2025-12-01", None, vec![&fx.plot_b]),
    ] {
        let plots: Vec<&str> = plots.iter().map(|plot| plot.as_str()).collect();
        sowing(&mut conn, &fx, kind, from, to, &plots);
    }
    for (season, day, kg, note, plots) in [
        (
            &fx.season_id,
            "2026-06-01",
            12_000.0,
            None,
            vec![&fx.plot_a],
        ),
        (
            &fx.season_id,
            "2026-06-01",
            12_000.0,
            None,
            vec![&fx.plot_a, &fx.plot_b],
        ),
        (
            &fx.season_id,
            "2026-06-01",
            9_000.0,
            Some("A-1"),
            vec![&fx.plot_b],
        ),
        (
            &fx.season_id,
            "2026-06-18",
            4_000.0,
            Some("A-1"),
            vec![&fx.plot_a],
        ),
        (&twin.id, "2026-06-01", 12_000.0, None, vec![&fx.plot_a]),
        (
            &twin.id,
            "2026-07-01",
            3_000.0,
            Some("A-1"),
            vec![&fx.plot_a],
        ),
    ] {
        let plots: Vec<&str> = plots.iter().map(|plot| plot.as_str()).collect();
        harvest(&mut conn, load(season, &fx.farm_id, day, kg, note, &plots));
    }
    let gone = harvest(
        &mut conn,
        load(
            &fx.season_id,
            &fx.farm_id,
            "2026-06-01",
            12_000.0,
            None,
            &[&fx.plot_a],
        ),
    );
    repo::soft_delete_harvest_record(&mut conn, &gone, None).unwrap();

    let checked = terrazgo_testkit::duplicates::assert_saved_pairs_match_the_book(
        &conn,
        &POLICIES,
        CORE_ROW_CAPTIONS,
    );
    assert_eq!(checked, 4 + 6 + 6, "every live record was checked");
    assert!(
        !suspected(&conn).is_empty(),
        "the farm has pairs, so the check compared something"
    );
}

#[test]
fn a_pair_removed_twice_over_says_who_removed_each_copy() {
    // The two copies are often identical — one operation typed twice — so what
    // tells them apart for a person is who removed which, and where. The phone
    // kept `first` (removing `second`); the laptop kept `second`.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (first, second) = removed_twice(&mut phone, &mut laptop);
    let listed = list(&laptop.conn, Scope::Current { today: TODAY });
    let pair = &listed.both_removed[0];
    let removed_on = |id: &str| {
        let record = pair.records.iter().find(|r| r.id == id).unwrap();
        assert!(record.removed_at.is_some());
        record.removed_on.clone()
    };
    assert_eq!(
        removed_on(&second).as_deref(),
        Some(A),
        "the phone removed it"
    );
    assert_eq!(
        removed_on(&first).as_deref(),
        Some(B),
        "the laptop removed it"
    );

    // The review of the pair says the same.
    let review = repo::review_pair(
        &laptop.conn,
        &HARVEST_DUPLICATES,
        &first,
        &second,
        CORE_ROW_CAPTIONS,
    )
    .unwrap();
    let on = |id: &str| {
        review
            .records
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .removed_on
            .clone()
    };
    assert_eq!(
        (on(&first).as_deref(), on(&second).as_deref()),
        (Some(B), Some(A))
    );

    // A pair that is merely suspected names no removal.
    let (mut conn, fx) = fixture();
    two_loads(&mut conn, &fx);
    let listed = list(&conn, Scope::Current { today: TODAY });
    assert!(
        listed.suspects[0]
            .records
            .iter()
            .all(|r| r.removed_on.is_none() && r.removed_at.is_none())
    );
}
