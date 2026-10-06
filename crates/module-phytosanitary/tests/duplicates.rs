// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! This module's duplicate rules (docs/sync.md → Duplicate suspects): what each
//! raises and what it leaves alone, a copy removed through its register's own
//! delete, and what the Status view's list costs as the books behind the
//! current ones pile up.
//!
//! The machinery the rules go through — scopes, verdicts, two devices — is
//! pinned in core; what is pinned here is each rule's shape.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::treatment::{Fixture, add_es_authorisation, add_plot, base_fixture, sample_treatment};
use module_phytosanitary::duplicates::{
    ANALYSIS_DUPLICATES, NON_FIELD_DUPLICATES, SEED_TREATMENT_DUPLICATES, TREATMENT_DUPLICATES,
};
use module_phytosanitary::models::*;
use module_phytosanitary::repository as repo;
use rusqlite::{Connection, StatementStatus};
use terrazgo_core::duplicates::{DuplicatePolicy, Scope, candidates_sql, overlap_sql};

const TODAY: &str = "2026-06-11";

const POLICIES: [DuplicatePolicy; 4] = [
    TREATMENT_DUPLICATES,
    NON_FIELD_DUPLICATES,
    SEED_TREATMENT_DUPLICATES,
    ANALYSIS_DUPLICATES,
];

/// The pairs the Status view lists, as `(register, first, second)`.
fn suspected(conn: &Connection) -> Vec<(&'static str, String, String)> {
    terrazgo_core::repository::list_duplicates(
        conn,
        &POLICIES,
        module_phytosanitary::ROW_CAPTIONS,
        Scope::Current { today: TODAY },
    )
    .unwrap()
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

struct Farm {
    fx: Fixture,
    plot_a: String,
    plot_b: String,
}

fn farm() -> (Connection, Farm) {
    let mut conn = module_phytosanitary::open_in_memory().unwrap();
    let fx = base_fixture(&mut conn);
    add_es_authorisation(&mut conn, &fx.product_id);
    let plot_a = add_plot(&mut conn, &fx.farm_id, "El Prado");
    let plot_b = add_plot(&mut conn, &fx.farm_id, "La Loma");
    (conn, Farm { fx, plot_a, plot_b })
}

fn plots(ids: &[&str]) -> Vec<NewTreatmentPlot> {
    ids.iter()
        .map(|id| NewTreatmentPlot {
            plot_id: (*id).into(),
            crop_id: None,
            surface_treated_ha: 1.0,
            growth_stage_code: None,
        })
        .collect()
}

/// A spray of `product_id` on `day`.
fn spray(conn: &mut Connection, fx: &Fixture, product_id: &str, day: &str, on: &[&str]) -> String {
    let mut new = sample_treatment(fx, None, Some(21));
    new.product_id = Some(product_id.into());
    new.application_date = day.into();
    repo::insert_treatment_record(conn, new, plots(on), None)
        .unwrap()
        .id
}

/// A product the farmer added, carrying `number` as its Spanish authorisation.
fn product(conn: &mut Connection, name: &str, number: &str) -> String {
    let id = repo::insert_product(
        conn,
        NewProduct {
            commercial_name: name.into(),
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
            product_id: id.clone(),
            country_code: "es".into(),
            authorisation_number: number.into(),
            kind_code: None,
            exceptional_substance_code: None,
            status: None,
            valid_from: None,
            valid_until: None,
        },
        None,
    )
    .unwrap();
    id
}

// ---------------------------------------------------------------------------
// Treatments
// ---------------------------------------------------------------------------

#[test]
fn one_spray_recorded_by_two_operators_is_a_suspect() {
    // The design's own case: one typed the next day, the other listed only the
    // plot they covered.
    let (mut conn, f) = farm();
    let first = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_a, &f.plot_b],
    );
    let second = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-02",
        &[&f.plot_b],
    );
    assert_eq!(
        suspected(&conn),
        pair_of("treatment_record", &first, &second)
    );
}

#[test]
fn one_product_added_twice_offline_is_still_one_product() {
    // Two phones each added the product from the catalogue: two product rows,
    // one authorisation number — which is what each record froze.
    let (mut conn, f) = farm();
    let twin = product(&mut conn, "FUNGITOP", "ES-25.123");
    let first = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_a],
    );
    let second = spray(&mut conn, &f.fx, &twin, "2026-05-01", &[&f.plot_a]);
    assert_eq!(
        suspected(&conn),
        pair_of("treatment_record", &first, &second)
    );
}

#[test]
fn another_product_or_other_plots_or_two_days_apart_is_not_a_suspect() {
    let (mut conn, f) = farm();
    let other = product(&mut conn, "Insectix", "ES-99.001");
    spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_a],
    );
    spray(&mut conn, &f.fx, &other, "2026-05-01", &[&f.plot_a]);
    spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_b],
    );
    spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-03",
        &[&f.plot_a],
    );
    assert!(suspected(&conn).is_empty(), "{:?}", suspected(&conn));
}

#[test]
fn a_treatment_over_several_days_meets_one_recorded_inside_them() {
    let (mut conn, f) = farm();
    let mut spread = sample_treatment(&f.fx, None, Some(21));
    spread.application_date = "2026-05-01".into();
    spread.application_end_date = Some("2026-05-06".into());
    let spread = repo::insert_treatment_record(&mut conn, spread, plots(&[&f.plot_a]), None)
        .unwrap()
        .id;
    let inside = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-05",
        &[&f.plot_a],
    );
    assert_eq!(
        suspected(&conn),
        pair_of("treatment_record", &spread, &inside)
    );
}

fn measure(conn: &mut Connection, fx: &Fixture, code: &str, day: &str, on: &[&str]) -> String {
    let mut new = sample_treatment(fx, None, None);
    new.product_id = None;
    new.dose_value = None;
    new.dose_unit_code = None;
    new.phi_days_used = None;
    new.measure_code = Some(code.into());
    new.application_date = day.into();
    repo::insert_treatment_record(conn, new, plots(on), None)
        .unwrap()
        .id
}

#[test]
fn two_non_chemical_measures_of_one_kind_are_a_suspect_and_a_spray_is_not_one() {
    // Neither names a product, and two empty products agree: the measure is
    // what tells them apart.
    let (mut conn, f) = farm();
    let first = measure(&mut conn, &f.fx, "15", "2026-04-10", &[&f.plot_a]);
    let second = measure(&mut conn, &f.fx, "15", "2026-04-10", &[&f.plot_a]);
    measure(&mut conn, &f.fx, "7", "2026-04-10", &[&f.plot_a]);
    spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-04-10",
        &[&f.plot_a],
    );
    assert_eq!(
        suspected(&conn),
        pair_of("treatment_record", &first, &second)
    );
}

#[test]
fn keeping_one_spray_removes_the_other_through_the_registers_own_delete() {
    let (mut conn, f) = farm();
    let first = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_a],
    );
    let second = spray(
        &mut conn,
        &f.fx,
        &f.fx.product_id,
        "2026-05-01",
        &[&f.plot_a],
    );
    terrazgo_core::repository::keep_duplicate(
        &mut conn,
        &TREATMENT_DUPLICATES,
        &first,
        &second,
        None,
        repo::soft_delete_treatment_record_tx,
    )
    .unwrap();
    assert!(suspected(&conn).is_empty());
    let withdrawn = |id: &str| {
        repo::get_treatment_record(&conn, id)
            .unwrap()
            .record
            .deleted_at
            .is_some()
    };
    assert!(!withdrawn(&first));
    assert!(withdrawn(&second), "withdrawn, never dropped");
}

// ---------------------------------------------------------------------------
// Non-field treatments, seed treatments, analyses
// ---------------------------------------------------------------------------

fn store_treatment(conn: &mut Connection, f: &Farm, day: &str, produce: Option<&str>) -> String {
    let new = NewNonFieldTreatment {
        premises_id: None,
        season_id: f.fx.season_id.clone(),
        farm_id: f.fx.farm_id.clone(),
        country_code: None,
        subject_kind_code: "postharvest".into(),
        treated_on: day.into(),
        subject_description: "Trigo blando de la cosecha 2026".into(),
        subject_product_code: produce.map(str::to_owned),
        treated_quantity_value: None,
        treated_quantity_unit_code: None,
        product_id: f.fx.product_id.clone(),
        product_quantity_value: None,
        product_quantity_unit_code: None,
        operator_id: f.fx.operator_id.clone(),
        machinery_id: None,
        advisor_id: None,
        problems: vec![NewTreatmentProblem {
            reason_category_code: "pest".into(),
            problem_code: "135".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        notes: None,
    };
    repo::insert_non_field_treatment(conn, new, None)
        .unwrap()
        .record
        .id
}

#[test]
fn one_store_treatment_recorded_twice_is_a_suspect_and_another_produce_is_not() {
    let (mut conn, f) = farm();
    let first = store_treatment(&mut conn, &f, "2026-06-01", Some("1"));
    let second = store_treatment(&mut conn, &f, "2026-06-02", Some("1"));
    store_treatment(&mut conn, &f, "2026-06-01", Some("5"));
    assert_eq!(
        suspected(&conn),
        pair_of("non_field_treatment", &first, &second)
    );
}

fn treated_seed(conn: &mut Connection, f: &Farm, day: &str, number: &str, on: &[&str]) -> String {
    let new = NewSeedTreatment {
        season_id: f.fx.season_id.clone(),
        farm_id: f.fx.farm_id.clone(),
        sown_on: day.into(),
        species_name: "trigo blando".into(),
        variety: Some("Nogal".into()),
        crop_code: Some("1".into()),
        seed_quantity_kg: Some(680.0),
        seed_lot: Some("L-2025-4471".into()),
        treatment_kind_code: Some("purchased_es".into()),
        acquired_on: None,
        sowing_record_id: None,
        product_name: "Celest Trio".into(),
        product_registration_number: Some(number.into()),
        product_active_substance: None,
        product_id: None,
        efficacy_code: None,
        notes: None,
        plots: on
            .iter()
            .map(|plot| NewSeedTreatmentPlot {
                plot_id: (*plot).into(),
                surface_sown_ha: 1.0,
            })
            .collect(),
    };
    repo::insert_seed_treatment(conn, new, None)
        .unwrap()
        .record
        .id
}

#[test]
fn one_seed_treatment_recorded_twice_is_a_suspect_and_another_product_is_not() {
    let (mut conn, f) = farm();
    let first = treated_seed(&mut conn, &f, "2025-11-10", "ES-24.876", &[&f.plot_a]);
    let second = treated_seed(
        &mut conn,
        &f,
        "2025-11-11",
        "ES-24.876",
        &[&f.plot_a, &f.plot_b],
    );
    treated_seed(&mut conn, &f, "2025-11-10", "ES-11.111", &[&f.plot_a]);
    treated_seed(&mut conn, &f, "2025-11-10", "ES-24.876", &[&f.plot_b]);
    // The last one overlaps `second` on plot_b within a day: also a suspect.
    let found = suspected(&conn);
    assert!(found.contains(&pair_of("seed_treatment", &first, &second)[0]));
    assert_eq!(found.len(), 2, "{found:?}");
}

fn analysis(conn: &mut Connection, f: &Farm, day: &str, bulletin: Option<&str>) -> String {
    let new = NewAnalysisRecord {
        season_id: f.fx.season_id.clone(),
        farm_id: f.fx.farm_id.clone(),
        sampled_on: day.into(),
        material_kind_code: "crop".into(),
        bulletin_number: bulletin.map(str::to_owned),
        lab_name: None,
        lab_address: None,
        lab_tax_id: None,
        substances_detected: None,
        soil: Default::default(),
        notes: None,
        plots: vec![NewAnalysisPlot {
            plot_id: f.plot_a.clone(),
            crop_id: None,
        }],
        analysis_type_codes: vec!["pesticide_residues".into()],
        substance_codes: vec![],
    };
    repo::insert_analysis_record(conn, new, None)
        .unwrap()
        .record
        .id
}

#[test]
fn one_bulletin_entered_twice_is_a_suspect_and_two_without_a_number_are_not() {
    let (mut conn, f) = farm();
    let first = analysis(&mut conn, &f, "2026-06-18", Some("B-2026/1187"));
    let second = analysis(&mut conn, &f, "2026-06-25", Some("B-2026/1187"));
    analysis(&mut conn, &f, "2026-06-18", None);
    analysis(&mut conn, &f, "2026-06-18", None);
    assert_eq!(
        suspected(&conn),
        pair_of("analysis_record", &first, &second)
    );
}

// ---------------------------------------------------------------------------
// What the Status view's list costs
// ---------------------------------------------------------------------------

/// Virtual-machine steps SQLite took to run `sql`, asked about `scope`, to
/// the end — the work done, whatever it returned.
fn steps(conn: &Connection, sql: &str, scope: Scope) -> i32 {
    let mut stmt = conn.prepare(sql).unwrap();
    let mut rows = stmt.query([scope.parameter()]).unwrap();
    while rows.next().unwrap().is_some() {}
    drop(rows);
    stmt.get_status(StatementStatus::VmStep)
}

/// Sprays per campaign in [`history`].
const RECORDS_PER_BOOK: usize = 40;

/// A farm whose history is written a campaign at a time, each of
/// [`RECORDS_PER_BOOK`] sprays, the newest running to August 2026.
///
/// **A cost test measures ONE of these before and after its history grows**
/// ([`History::reach_back_to`]), never two built apart. Every id is random, so
/// two builds differ in more than their history: which of the current books
/// SQLite visits first follows the farm's id, and that moved the count by a
/// book's worth of records from run to run — noise that had to be allowed
/// for, and that once in fifteen runs exceeded the allowance. Grown in place,
/// the current books and every id in them are the same in both counts, so the
/// history is the only thing that changed and the counts can be compared
/// exactly.
///
/// Every campaign treats the plots in the same pattern. (The scaled farm in
/// `common::scale` walks its plots across campaigns on purpose, for tests that
/// must tell recent plots from old ones.)
struct History {
    conn: Connection,
    fx: Fixture,
    plot_ids: Vec<String>,
    /// The year the oldest campaign written so far ends in.
    oldest: i64,
}

impl History {
    /// The campaigns ending in `since` to 2026.
    fn since(since: i64) -> History {
        let mut conn = module_phytosanitary::open_in_memory().unwrap();
        let fx = base_fixture(&mut conn);
        add_es_authorisation(&mut conn, &fx.product_id);
        let plot_ids: Vec<String> = (0..20)
            .map(|i| add_plot(&mut conn, &fx.farm_id, &format!("Parcela {i}")))
            .collect();
        let mut history = History {
            conn,
            fx,
            plot_ids,
            oldest: 2027,
        };
        history.reach_back_to(since);
        history
    }

    /// Write the campaigns from the one before the oldest back to the one
    /// ending in `year`.
    fn reach_back_to(&mut self, year: i64) {
        for year in (year..self.oldest).rev() {
            self.campaign(year);
        }
        self.oldest = self.oldest.min(year);
    }

    /// The campaign ending in August of `year`: its book — the fixture's own
    /// for 2026 — and its sprays.
    fn campaign(&mut self, year: i64) {
        let season_id = if year == 2026 {
            self.fx.season_id.clone()
        } else {
            repo::insert_season(
                &mut self.conn,
                NewSeason {
                    farm_id: self.fx.farm_id.clone(),
                    starts_on: format!("{}-09-01", year - 1),
                    ends_on: format!("{year}-08-31"),
                    custom_label: None,
                },
                None,
            )
            .unwrap()
            .id
        };
        for r in 0..RECORDS_PER_BOOK {
            let mut new = sample_treatment(&self.fx, None, Some(21));
            new.season_id = season_id.clone();
            // Three days apart from March, so no two sprays share a window.
            let day = 60 + r * 3;
            new.application_date = format!("{year}-{:02}-{:02}", 3 + day / 30 - 2, 1 + day % 30);
            let on = [
                self.plot_ids[(2 * r) % 20].as_str(),
                self.plot_ids[(2 * r + 1) % 20].as_str(),
            ];
            repo::insert_treatment_record(&mut self.conn, new, plots(&on), None).unwrap();
        }
    }
}

#[test]
fn the_status_views_list_does_not_grow_with_the_books_behind_the_current_ones() {
    // The list is worked out on every read, and a farm's books pile up for as
    // long as it keeps using the app. One farm holds the current books — the
    // campaign under way and the one before — and two campaigns behind them;
    // then eight more are written behind those. The two fetches the list runs
    // for this register — its records, and their plots — must do the same work
    // before and after; a fetch that reached past the current books would give
    // the same pairs and cost the history.
    let current = Scope::Current { today: TODAY };
    // The control: the same fetches asked on a day in 2000, when every book
    // counts as current, do grow with the history.
    let everything = Scope::Current {
        today: "2000-01-01",
    };
    let fetches = |scope| {
        let overlap = TREATMENT_DUPLICATES.overlaps()[0];
        [
            candidates_sql(&TREATMENT_DUPLICATES, scope),
            overlap_sql(&TREATMENT_DUPLICATES, &overlap, scope),
        ]
    };
    let work = |conn: &Connection, scope| -> i32 {
        fetches(scope)
            .iter()
            .map(|sql| steps(conn, sql, scope))
            .sum()
    };

    let mut farm = History::since(2023);
    let (short, short_all) = (work(&farm.conn, current), work(&farm.conn, everything));
    farm.reach_back_to(2015);
    let (long, long_all) = (work(&farm.conn, current), work(&farm.conn, everything));

    assert!(
        long_all > 2 * short_all,
        "the counter must be able to fail ({short_all} against {long_all})"
    );
    // The same farm, books and ids in both counts, so the only step the
    // history can add is a BOUNDARY READ: each fetch walks the current books'
    // records one book at a time on `idx_treatment_record_book`, and a walk
    // reads one entry past its book to see it has ended — unless that book is
    // the last in the index, where it meets the end instead. Book ids are
    // random, so a current book is sometimes last; a book written behind it
    // may then sort after it, and the fetch pays that one read. Only one book
    // can be last, so it is one step per fetch at most, and never fewer —
    // measured 2026-09-30: +2 in exactly the runs where a current book stopped
    // being last, 0 in every other. A fetch that leaked would add a step per
    // record behind the current books: 320 here.
    let boundary = fetches(current).len() as i32;
    assert!(
        (0..=boundary).contains(&(long - short)),
        "{short} steps with two books behind the current ones, {long} with ten"
    );
}

// ---------------------------------------------------------------------------
// Right after a save
// ---------------------------------------------------------------------------

#[test]
fn what_a_save_finds_is_what_the_book_page_lists() {
    // Every shape this module's rules read, in one farm, and every live record
    // checked: the check a form runs after saving and the book's page must
    // name the same pairs (docs/sync.md → The same rule, right after the form saves).
    let (mut conn, f) = farm();
    let other = product(&mut conn, "Otro", "ES-00001");
    let mut spread = sample_treatment(&f.fx, None, Some(21));
    spread.application_date = "2026-05-01".into();
    spread.application_end_date = Some("2026-05-12".into());
    repo::insert_treatment_record(&mut conn, spread, plots(&[&f.plot_a]), None).unwrap();
    for (product_id, day, on) in [
        (&f.fx.product_id, "2026-05-11", vec![&f.plot_a]),
        (&f.fx.product_id, "2026-05-13", vec![&f.plot_a, &f.plot_b]),
        (&f.fx.product_id, "2026-05-15", vec![&f.plot_b]),
        (&other, "2026-05-11", vec![&f.plot_a]),
        (&f.fx.product_id, "2026-04-20", vec![&f.plot_b]),
    ] {
        let on: Vec<&str> = on.iter().map(|plot| plot.as_str()).collect();
        spray(&mut conn, &f.fx, product_id, day, &on);
    }
    measure(&mut conn, &f.fx, "15", "2026-04-10", &[&f.plot_a]);
    measure(&mut conn, &f.fx, "15", "2026-04-11", &[&f.plot_a]);
    store_treatment(&mut conn, &f, "2026-06-01", Some("1"));
    store_treatment(&mut conn, &f, "2026-06-02", Some("1"));
    store_treatment(&mut conn, &f, "2026-06-01", Some("5"));
    treated_seed(&mut conn, &f, "2025-11-10", "ES-24.876", &[&f.plot_a]);
    treated_seed(
        &mut conn,
        &f,
        "2025-11-11",
        "ES-24.876",
        &[&f.plot_a, &f.plot_b],
    );
    analysis(&mut conn, &f, "2026-06-18", Some("B-2026/1187"));
    analysis(&mut conn, &f, "2026-06-25", Some("B-2026/1187"));
    analysis(&mut conn, &f, "2026-06-18", None);

    let checked = terrazgo_testkit::duplicates::assert_saved_pairs_match_the_book(
        &conn,
        &POLICIES,
        module_phytosanitary::ROW_CAPTIONS,
    );
    assert_eq!(checked, 8 + 3 + 2 + 3);
    // The long spray with the two inside its reach, the two measures, and one
    // pair each of store, seed and analysis: the check compared something.
    assert_eq!(suspected(&conn).len(), 6);
}

/// Steps and rows of one of a record's fetches, run to the end.
fn record_work(conn: &Connection, sql: &str, record_id: &str) -> (i32, usize) {
    let mut stmt = conn.prepare(sql).unwrap();
    let mut rows = stmt.query([record_id]).unwrap();
    let mut returned = 0;
    while rows.next().unwrap().is_some() {
        returned += 1;
    }
    drop(rows);
    (stmt.get_status(StatementStatus::VmStep), returned)
}

#[test]
fn the_check_after_a_save_reads_near_the_record_and_never_the_history() {
    // Asked after every save, so what it reads must not grow with the farm's
    // past, and should not be the whole book either: one farm with two
    // campaigns behind the current book, then ten, and its latest spray asked
    // about both times.
    let fetches = || {
        let overlap = TREATMENT_DUPLICATES.overlaps()[0];
        [
            terrazgo_core::duplicates::record_candidates_sql(&TREATMENT_DUPLICATES),
            terrazgo_core::duplicates::record_overlap_sql(&TREATMENT_DUPLICATES, &overlap),
        ]
    };
    let latest = |conn: &Connection| -> String {
        conn.query_row(
            "SELECT id FROM treatment_record ORDER BY application_date DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    let work = |conn: &Connection| -> (i32, usize) {
        let id = latest(conn);
        fetches()
            .iter()
            .map(|sql| record_work(conn, sql, &id))
            .fold((0, 0), |(steps, rows), (s, r)| (steps + s, rows + r))
    };
    // The control: the list's own fetch asked on a day in 2000, when every
    // book counts as current, grows with the history on this same farm.
    let everything = Scope::Current {
        today: "2000-01-01",
    };
    let list_work = |conn: &Connection| {
        steps(
            conn,
            &candidates_sql(&TREATMENT_DUPLICATES, everything),
            everything,
        )
    };

    let mut farm = History::since(2023);
    let ((short_steps, short_rows), short_list) = (work(&farm.conn), list_work(&farm.conn));
    farm.reach_back_to(2015);
    let ((long_steps, long_rows), long_list) = (work(&farm.conn), list_work(&farm.conn));

    assert!(
        long_list > 2 * short_list,
        "the counter must be able to fail"
    );
    // The same farm and the same record asked about, so the history can add
    // only a boundary read per fetch — the list's test above says why.
    let boundary = fetches().len() as i32;
    assert!(
        (0..=boundary).contains(&(long_steps - short_steps)),
        "{short_steps} steps with two campaigns behind, {long_steps} with ten"
    );
    // Sprays three days apart and a day's slack: the record and its plots, and
    // no neighbour — where the book's page reads all forty.
    assert_eq!(
        (short_rows, long_rows),
        (3, 3),
        "the record's row and its two plots"
    );
}
