// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module's duplicate rules (docs/sync.md → Duplicate suspects): what each
//! raises and what it leaves alone — and the one register whose delete reaches
//! into others, a soil cover withdrawing its maintenance lines, removed as a
//! duplicate and restored again. The machinery the rules go through is pinned
//! in core.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{CoreFixture, FarmWithPlots, farm_with_plots};
use module_ecoscheme::duplicates::{
    CULTURAL_OPERATION_DUPLICATES, GRAZING_DUPLICATES, SOIL_COVER_DUPLICATES,
};
use module_ecoscheme::models::*;
use module_ecoscheme::open_in_memory;
use module_ecoscheme::repository as repo;
use rusqlite::Connection;
use terrazgo_core::duplicates::{DuplicatePolicy, Scope};
use terrazgo_core::repository as core_repo;
use terrazgo_testkit::sync::send;

const TODAY: &str = "2026-06-11";

const POLICIES: [DuplicatePolicy; 3] = [
    GRAZING_DUPLICATES,
    CULTURAL_OPERATION_DUPLICATES,
    SOIL_COVER_DUPLICATES,
];

fn fixture() -> (Connection, CoreFixture) {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    (conn, fx)
}

fn listed(conn: &Connection) -> core_repo::DuplicateList {
    core_repo::list_duplicates(
        conn,
        &POLICIES,
        module_ecoscheme::ROW_CAPTIONS,
        Scope::Current { today: TODAY },
    )
    .unwrap()
}

/// The pairs the Status view lists, as `(register, first, second)`.
fn suspected(conn: &Connection) -> Vec<(&'static str, String, String)> {
    listed(conn)
        .suspects
        .into_iter()
        .map(|pair| {
            let [first, second] = pair.records;
            (pair.register, first.id, second.id)
        })
        .collect()
}

fn pair_of(register: &'static str, a: &str, b: &str) -> Vec<(&'static str, String, String)> {
    let (first, second) = if a < b { (a, b) } else { (b, a) };
    vec![(register, first.to_owned(), second.to_owned())]
}

// ---------------------------------------------------------------------------
// Grazing
// ---------------------------------------------------------------------------

fn flock(rega: &str) -> GrazingAnimal {
    GrazingAnimal {
        id: String::new(),
        grazing_record_id: String::new(),
        species_code: "03".into(),
        rega_code: rega.into(),
        animal_count: 120,
    }
}

fn grazing(
    conn: &mut Connection,
    fx: &CoreFixture,
    from: &str,
    to: Option<&str>,
    on: &[&str],
    rega: &str,
) -> String {
    repo::insert_grazing_record(
        conn,
        NewGrazingRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "extensive_grazing".into(),
            plot_group_ref: None,
            soil_cover_id: None,
            started_on: from.into(),
            ended_on: to.map(str::to_owned),
            notes: None,
            plot_ids: on.iter().map(|plot| (*plot).to_owned()).collect(),
            animals: vec![flock(rega)],
        },
        None,
    )
    .unwrap()
    .record
    .id
}

const FLOCK: &str = "ES071234560001";
const NEIGHBOURS_FLOCK: &str = "ES079999990001";

#[test]
fn one_flock_recorded_twice_on_one_pasture_is_a_suspect() {
    let (mut conn, fx) = fixture();
    let first = grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        &[&fx.plot_a],
        FLOCK,
    );
    let second = grazing(
        &mut conn,
        &fx,
        "2026-05-10",
        Some("2026-05-30"),
        &[&fx.plot_a, &fx.plot_b],
        FLOCK,
    );
    assert_eq!(suspected(&conn), pair_of("grazing_record", &first, &second));
}

#[test]
fn two_flocks_on_one_pasture_at_once_are_not_a_suspect() {
    let (mut conn, fx) = fixture();
    grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        &[&fx.plot_a],
        FLOCK,
    );
    grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        &[&fx.plot_a],
        NEIGHBOURS_FLOCK,
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_grazing_after_the_flock_left_is_not_a_suspect() {
    let (mut conn, fx) = fixture();
    grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-04-30"),
        &[&fx.plot_a],
        FLOCK,
    );
    grazing(
        &mut conn,
        &fx,
        "2026-05-01",
        Some("2026-05-30"),
        &[&fx.plot_a],
        FLOCK,
    );
    assert!(suspected(&conn).is_empty());
}

#[test]
fn a_grazing_still_under_way_meets_every_later_one() {
    // No end yet means the flock is still there, not that the day is unknown.
    let (mut conn, fx) = fixture();
    let open = grazing(&mut conn, &fx, "2026-02-01", None, &[&fx.plot_a], FLOCK);
    let later = grazing(
        &mut conn,
        &fx,
        "2026-05-20",
        Some("2026-05-25"),
        &[&fx.plot_a],
        FLOCK,
    );
    assert_eq!(suspected(&conn), pair_of("grazing_record", &open, &later));
}

#[test]
fn a_pair_of_grazings_lines_its_herds_up_by_code_and_species() {
    // A herd is a grazing's child keyed by two columns — its REGA code and its
    // species — and the review puts one record's herd beside the other's by
    // that key, never by row id: each record's herd rows have ids of their own.
    let (mut conn, fx) = fixture();
    let mut insert = |animals: Vec<GrazingAnimal>| {
        repo::insert_grazing_record(
            &mut conn,
            NewGrazingRecord {
                season_id: fx.season_id.clone(),
                farm_id: fx.farm_id.clone(),
                practice_code: "extensive_grazing".into(),
                plot_group_ref: None,
                soil_cover_id: None,
                started_on: "2026-04-01".into(),
                ended_on: Some("2026-04-20".into()),
                notes: None,
                plot_ids: vec![fx.plot_a.clone()],
                animals,
            },
            None,
        )
        .unwrap()
        .record
        .id
    };
    let counted_less = insert(vec![flock(FLOCK)]);
    let mut more = flock(FLOCK);
    more.animal_count = 150;
    let mut goats = flock(FLOCK);
    goats.species_code = "02".into();
    let counted_more = insert(vec![more, goats]);

    let review = core_repo::review_pair(
        &conn,
        &GRAZING_DUPLICATES,
        &counted_less,
        &counted_more,
        module_ecoscheme::ROW_CAPTIONS,
    )
    .unwrap();
    let counts: Vec<Vec<Option<i64>>> = review
        .lines
        .iter()
        .filter(|line| line.table == "grazing_animal" && line.column == "animal_count")
        .map(|line| {
            line.values
                .iter()
                .map(|value| value.as_ref().and_then(|v| v.value.as_i64()))
                .collect()
        })
        .collect();
    // The pair comes smaller id first, and every line's values follow it.
    let less_first = counted_less < counted_more;
    let side = |less: Option<i64>, more: Option<i64>| {
        if less_first {
            vec![less, more]
        } else {
            vec![more, less]
        }
    };
    // The sheep on one line, counted 120 on one side and 150 on the other; the
    // goats only one record lists, alone.
    assert_eq!(counts.len(), 2, "{counts:?}");
    assert!(counts.contains(&side(Some(120), Some(150))), "{counts:?}");
    assert!(counts.contains(&side(None, Some(120))), "{counts:?}");
}

// ---------------------------------------------------------------------------
// Cultural operations
// ---------------------------------------------------------------------------

fn operation(
    conn: &mut Connection,
    fx: &CoreFixture,
    kind: &str,
    day: &str,
    on: &[&str],
) -> String {
    repo::insert_cultural_operation(
        conn,
        NewCulturalOperation {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "sustainable_mowing".into(),
            operation_kind_code: kind.into(),
            performed_on: day.into(),
            performed_end_date: None,
            activity_description: None,
            residue_destination_code: None,
            soil_cover_id: None,
            notes: None,
            plot_ids: on.iter().map(|plot| (*plot).to_owned()).collect(),
        },
        None,
    )
    .unwrap()
    .record
    .id
}

#[test]
fn one_mowing_recorded_a_day_apart_is_a_suspect_and_another_kind_of_work_is_not() {
    let (mut conn, fx) = fixture();
    let first = operation(&mut conn, &fx, "mowing", "2026-05-12", &[&fx.plot_a]);
    let second = operation(&mut conn, &fx, "mowing", "2026-05-13", &[&fx.plot_a]);
    operation(&mut conn, &fx, "brush_cutting", "2026-05-12", &[&fx.plot_a]);
    operation(&mut conn, &fx, "mowing", "2026-05-12", &[&fx.plot_b]);
    assert_eq!(
        suspected(&conn),
        pair_of("cultural_operation", &first, &second)
    );
}

// ---------------------------------------------------------------------------
// Soil covers
// ---------------------------------------------------------------------------

fn cover(
    conn: &mut Connection,
    fx: &CoreFixture,
    day: &str,
    on: &[&str],
    mowed: Option<&str>,
) -> SoilCoverDetail {
    repo::insert_soil_cover(
        conn,
        NewSoilCover {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            practice_code: "plant_cover".into(),
            cover_type_code: "2".into(),
            established_on: day.into(),
            width_m: None,
            free_canopy_width_m: None,
            widths_stated_on: None,
            notes: None,
            plot_ids: on.iter().map(|plot| (*plot).to_owned()).collect(),
            maintenance: mowed
                .map(|day| {
                    vec![CoverMaintenanceLine {
                        id: String::new(),
                        kind_code: "mowing".into(),
                        performed_on: day.into(),
                        performed_end_date: None,
                        animals: Vec::new(),
                    }]
                })
                .unwrap_or_default(),
        },
        None,
    )
    .unwrap()
}

#[test]
fn one_cover_established_twice_in_a_book_is_a_suspect_whatever_the_dates() {
    // A cover is established once per plot and campaign; two devices giving it
    // two dates are still one cover.
    let (mut conn, fx) = fixture();
    let first = cover(&mut conn, &fx, "2026-03-15", &[&fx.plot_a], None)
        .record
        .id;
    let second = cover(
        &mut conn,
        &fx,
        "2026-04-02",
        &[&fx.plot_a, &fx.plot_b],
        None,
    )
    .record
    .id;
    assert_eq!(suspected(&conn), pair_of("soil_cover", &first, &second));
}

#[test]
fn covers_on_other_plots_are_not_a_suspect() {
    let (mut conn, fx) = fixture();
    cover(&mut conn, &fx, "2026-03-15", &[&fx.plot_a], None);
    cover(&mut conn, &fx, "2026-03-15", &[&fx.plot_b], None);
    assert!(suspected(&conn).is_empty());
}

// ---------------------------------------------------------------------------
// A delete that reaches into other registers, undone
// ---------------------------------------------------------------------------

fn live_operation(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT deleted_at IS NULL FROM cultural_operation WHERE id = ?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

fn live_cover(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT deleted_at IS NULL FROM soil_cover WHERE id = ?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn a_cover_removed_twice_over_comes_back_with_its_maintenance() {
    // A cover's mowing is a record in another register, and withdrawing the
    // cover withdraws it. Two devices each keep a different copy offline, both
    // covers are removed, and restoring one must bring its mowing back too —
    // which clearing one `deleted_at` would not.
    let mut phone = open_in_memory().unwrap();
    let mut laptop = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut phone, FarmWithPlots::default());
    let first = cover(
        &mut phone,
        &fx,
        "2026-03-15",
        &[&fx.plot_a],
        Some("2026-05-12"),
    );
    let second = cover(&mut phone, &fx, "2026-03-20", &[&fx.plot_a], None);
    let mowing = first.maintenance[0].id.clone();
    let group = terrazgo_core::sync::ensure_sync_group(&phone).unwrap();
    terrazgo_core::sync::join_sync_group(&laptop, &group).unwrap();
    send(&phone, &mut laptop);

    let keep = |conn: &mut Connection, kept: &str, removed: &str| {
        core_repo::keep_duplicate(
            conn,
            &SOIL_COVER_DUPLICATES,
            kept,
            removed,
            None,
            repo::soft_delete_soil_cover_tx,
        )
        .unwrap();
    };
    keep(&mut phone, &second.record.id, &first.record.id);
    keep(&mut laptop, &first.record.id, &second.record.id);
    send(&phone, &mut laptop);
    send(&laptop, &mut phone);

    for device in [&phone, &laptop] {
        assert!(!live_cover(device, &first.record.id) && !live_cover(device, &second.record.id));
        assert!(!live_operation(device, &mowing), "withdrawn with its cover");
        assert_eq!(listed(device).both_removed.len(), 1);
    }

    core_repo::restore_removed_duplicate(
        &mut laptop,
        &SOIL_COVER_DUPLICATES,
        &first.record.id,
        None,
    )
    .unwrap();
    send(&laptop, &mut phone);
    for device in [&phone, &laptop] {
        assert!(live_cover(device, &first.record.id));
        assert!(
            live_operation(device, &mowing),
            "the mowing came back with it"
        );
        assert!(!live_cover(device, &second.record.id));
        let list = listed(device);
        assert!(list.both_removed.is_empty() && list.suspects.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Right after a save
// ---------------------------------------------------------------------------

#[test]
fn a_save_that_writes_several_registers_is_asked_about_all_of_them() {
    // A cover's save writes its maintenance too — a mowing is a cultural
    // operation, a register with a rule of its own. The same cover entered
    // twice, each with its mowing, is two pairs, and the check after the
    // second save finds both.
    let (mut conn, fx) = fixture();
    let first = cover(
        &mut conn,
        &fx,
        "2026-03-15",
        &[&fx.plot_a],
        Some("2026-05-01"),
    );
    let second = cover(
        &mut conn,
        &fx,
        "2026-03-20",
        &[&fx.plot_a],
        Some("2026-05-01"),
    );
    let mowing_of = |cover: &SoilCoverDetail| -> String {
        conn.query_row(
            "SELECT id FROM cultural_operation WHERE soil_cover_id = ?1",
            [&cover.record.id],
            |r| r.get(0),
        )
        .unwrap()
    };
    let (first_mowing, second_mowing) = (mowing_of(&first), mowing_of(&second));

    let found = core_repo::list_saved_duplicates(
        &conn,
        &POLICIES,
        module_ecoscheme::ROW_CAPTIONS,
        "soil_cover",
        &second.record.id,
    )
    .unwrap();
    let mut saved = found.saved.clone();
    saved.sort();
    let mut expected = vec![second.record.id.clone(), second_mowing.clone()];
    expected.sort();
    assert_eq!(saved, expected, "the cover and the mowing it wrote");
    let mut pairs: Vec<(&str, String, String)> = found
        .suspects
        .iter()
        .map(|pair| {
            (
                pair.register,
                pair.records[0].id.clone(),
                pair.records[1].id.clone(),
            )
        })
        .collect();
    pairs.sort();
    let mut expected = [
        pair_of("cultural_operation", &first_mowing, &second_mowing),
        pair_of("soil_cover", &first.record.id, &second.record.id),
    ]
    .concat();
    expected.sort();
    assert_eq!(pairs, expected);
}

#[test]
fn what_a_save_finds_is_what_the_book_page_lists() {
    // Every shape this module's rules read, in one farm — grazings still under
    // way and closed, mowings a day apart and of another kind, covers with and
    // without maintenance — and every live record checked: the check a form
    // runs after saving and the book's page must name the same pairs
    // (docs/sync.md → The same rule, right after the form saves).
    let (mut conn, fx) = fixture();
    grazing(&mut conn, &fx, "2026-02-01", None, &[&fx.plot_a], FLOCK);
    grazing(
        &mut conn,
        &fx,
        "2026-04-01",
        Some("2026-06-15"),
        &[&fx.plot_a],
        FLOCK,
    );
    grazing(
        &mut conn,
        &fx,
        "2026-05-10",
        Some("2026-05-30"),
        &[&fx.plot_b],
        FLOCK,
    );
    grazing(
        &mut conn,
        &fx,
        "2026-05-10",
        Some("2026-05-30"),
        &[&fx.plot_a],
        NEIGHBOURS_FLOCK,
    );
    operation(&mut conn, &fx, "mowing", "2026-05-01", &[&fx.plot_a]);
    operation(
        &mut conn,
        &fx,
        "mowing",
        "2026-05-02",
        &[&fx.plot_a, &fx.plot_b],
    );
    operation(&mut conn, &fx, "brush_cutting", "2026-05-01", &[&fx.plot_a]);
    cover(
        &mut conn,
        &fx,
        "2026-03-15",
        &[&fx.plot_a],
        Some("2026-05-01"),
    );
    cover(&mut conn, &fx, "2026-03-20", &[&fx.plot_a], None);
    cover(&mut conn, &fx, "2026-03-15", &[&fx.plot_b], None);

    let checked = terrazgo_testkit::duplicates::assert_saved_pairs_match_the_book(
        &conn,
        &POLICIES,
        module_ecoscheme::ROW_CAPTIONS,
    );
    assert_eq!(checked, 4 + 4 + 3, "the cover's mowing is an operation too");
    // The flock still grazing and the one after it, the two mowings, and the
    // two covers: the check compared something.
    assert_eq!(suspected(&conn).len(), 3);
}
