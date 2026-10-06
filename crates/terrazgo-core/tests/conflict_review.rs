// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a person is shown when two devices wrote one register, and what
//! happens when they choose (docs/sync.md → Conflicts as the person sees them).
//!
//! The engine's own matrix is `merge_scenarios.rs`; this file is the half a
//! person reaches. Both run against the same simulated devices, because the
//! property that matters is not that a screen renders — it is that **a choice
//! made on one device is the book on every device**, including one that was
//! offline while it was made.
//!
//! The three load-bearing properties of that section, restated as what a test
//! may not let through:
//!
//!   * the book is never broken and never blocks — a review writes nothing,
//!     and the losing version is read without ever being materialised;
//!   * nothing is lost — the version not kept stays in the log;
//!   * a person may merge, an algorithm may not — so a stale choice, made
//!     against a version somebody else has already resolved away, is refused
//!     rather than applied.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use terrazgo_core::CoreError;
use terrazgo_core::merge::{CORE_ROW_CAPTIONS, ConflictReview, ReviewLine, resolve, review};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_testkit::query_cost;

/// The review of a register, as a screen would ask for it.
fn read(device: &Device, root_table: &str, root_id: &str) -> ConflictReview {
    review(&device.conn, root_table, root_id, CORE_ROW_CAPTIONS).unwrap()
}

/// One line's values, in the versions' order, as text.
///
/// `None` for the two ways a version says nothing here — it does not hold the
/// row at all, or it holds the row with that column unset. A screen prints a
/// dash for both, and a test that told them apart would be asserting on the
/// difference between two kinds of blank.
fn values(line: &ReviewLine) -> Vec<Option<String>> {
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

/// The line about one column, wherever it is in the list.
fn line<'a>(review: &'a ConflictReview, column: &str) -> &'a ReviewLine {
    review
        .lines
        .iter()
        .find(|line| line.column == column)
        .unwrap_or_else(|| panic!("no line for {column}: {:?}", columns(review)))
}

fn columns(review: &ConflictReview) -> Vec<&str> {
    review
        .lines
        .iter()
        .map(|line| line.column.as_str())
        .collect()
}

/// Two devices renaming one holding without seeing each other. B's edit carries
/// the later clock and the later device id, so B is live either way — the same
/// reasoning `merge_scenarios.rs` sets out.
fn conflicted_farm(phone: &mut Device, laptop: &mut Device) -> String {
    let farm_id = shared_farm(phone, &mut [laptop]);
    rename(phone, &farm_id, "La Vega");
    rename(laptop, &farm_id, "El Soto");
    sync_both(phone, laptop);
    assert!(phone.conflicted("farm", &farm_id));
    farm_id
}

// ---------------------------------------------------------------------------
// Reading the two versions
// ---------------------------------------------------------------------------

#[test]
fn the_review_shows_both_versions_with_the_live_one_first() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);

    let review = read(&phone, "farm", &farm_id);
    assert_eq!(review.versions.len(), 2);
    assert!(review.versions[0].live, "the live version leads");
    assert!(!review.versions[1].live);
    assert_eq!(review.versions[0].device, B, "B's edit is what is showing");
    assert_eq!(review.versions[1].device, A);
    assert_eq!(
        values(line(&review, "name")),
        [Some("El Soto".into()), Some("La Vega".into())],
        "the disagreement, in the versions' order"
    );
    assert_eq!(
        review.caption.as_deref(),
        Some("El Soto"),
        "the register is named by the version the book is showing"
    );
    // Both devices compute the same review from the same log: the screen is a
    // function of the log, like everything else in the merge.
    let there = read(&laptop, "farm", &farm_id);
    assert_eq!(values(line(&there, "name")), values(line(&review, "name")));
    assert_eq!(there.versions[0].device, review.versions[0].device);
}

#[test]
fn the_review_lists_only_what_the_versions_disagree_about() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);

    let review = read(&phone, "farm", &farm_id);
    assert_eq!(
        columns(&review),
        ["name"],
        "a farm has seventeen columns and the two versions differ on one"
    );
    // `updated_at` is the trap: two branches nearly always differ on it and
    // never differ ABOUT it.
    assert!(!columns(&review).contains(&"updated_at"));
    // Two records compared side by side show what they share as well, marked
    // as agreeing; a conflict never does.
    assert!(review.lines.iter().all(|line| line.differs));
}

#[test]
fn the_review_writes_nothing_and_never_materialises_the_losing_version() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);

    let before: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();
    let showing = phone.farm_name(&farm_id);
    read(&phone, "farm", &farm_id);
    let after: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();

    assert_eq!(before, after, "reading a conflict logs nothing");
    assert_eq!(
        phone.farm_name(&farm_id),
        showing,
        "and the book goes on showing what it was showing"
    );
}

#[test]
fn a_register_with_one_version_has_nothing_to_review() {
    // The race the screen has to survive: somebody resolves the conflict on
    // another device and the bundle lands while this one's dialog is open.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    rename(&mut phone, &farm_id, "La Vega");

    assert!(matches!(
        review(&phone.conn, "farm", &farm_id, CORE_ROW_CAPTIONS),
        Err(CoreError::Invalid("register_not_in_conflict"))
    ));
}

#[test]
fn a_child_only_one_version_has_is_a_line_of_its_own() {
    // The design's worked example, in the shape core can build it: one device
    // adds a plot to a sowing while the other edits the note. Nothing is
    // concurrent row by row, which is exactly why the register is the unit.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (farm_id, season_id, first, second) = one_book(&mut phone);
    sync(&phone, &mut laptop);

    let record = repo::insert_sowing_record(
        &mut phone.conn,
        NewSowingRecord {
            season_id,
            farm_id,
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: first.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record;
    sync(&phone, &mut laptop);

    repo::update_sowing_record(
        &mut phone.conn,
        &record.id,
        sowing_state(None, &[&first, &second]),
        None,
    )
    .unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record.id,
        sowing_state(Some("2ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    let review = read(&phone, "sowing_record", &record.id);
    assert_eq!(
        values(line(&review, "notes")),
        [Some("2ª pasada".into()), None],
        "the laptop's note against a version that has none"
    );

    // The added plot: a line about a row one version does not hold at all, and
    // it says LA LOMA rather than a UUID — the whole point of the naming map.
    let added = review
        .lines
        .iter()
        .find(|line| line.table == "sowing_plot" && line.column == "plot_id")
        .expect("the plot only one version has");
    assert!(
        !added.root,
        "it is a child of the register, not the register"
    );
    assert_eq!(values(added), [None, Some("La Loma".into())]);
}

#[test]
fn a_line_is_never_about_the_column_that_files_a_row_under_its_register() {
    // A child row only one version holds shows its own fields — and NOT its
    // link to the register being compared, which is the same value on both
    // sides by construction and says nothing a person is choosing between.
    // Worse than noise: `sowing_plot.sowing_record_id` refers to a named table,
    // so it would render the sowing's own DATE under a raw column name.
    //
    // This is the other half of syncFields.js's coverage rule — a column the
    // comparison can emit is a column that must have a label, and
    // sync_fields_contract.rs exempts exactly what `compare` skips.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (farm_id, season_id, first, second) = one_book(&mut phone);
    sync(&phone, &mut laptop);

    let record = repo::insert_sowing_record(
        &mut phone.conn,
        NewSowingRecord {
            season_id,
            farm_id,
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: first.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record;
    sync(&phone, &mut laptop);

    repo::update_sowing_record(
        &mut phone.conn,
        &record.id,
        sowing_state(None, &[&first, &second]),
        None,
    )
    .unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record.id,
        sowing_state(Some("2ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    let review = read(&phone, "sowing_record", &record.id);
    let added: Vec<&str> = review
        .lines
        .iter()
        .filter(|line| line.table == "sowing_plot")
        .map(|line| line.column.as_str())
        .collect();
    assert!(
        added.contains(&"plot_id"),
        "the plot one version added is the point of the line: {added:?}"
    );
    assert!(
        !added.contains(&"sowing_record_id"),
        "but not the column saying which sowing it belongs to: {added:?}"
    );
    // Nor the two that place the register itself, on any row.
    for column in ["season_id", "farm_id", "id", "created_at", "updated_at"] {
        assert!(
            !review.lines.iter().any(|line| line.column == column),
            "{column} is not something two versions disagree ABOUT"
        );
    }
}

#[test]
fn a_version_is_named_by_the_device_and_the_person_that_wrote_it() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    // The phone knows itself and has been told what the laptop is called.
    repo::register_this_device(&mut phone.conn, None).unwrap();
    repo::rename_sync_peer(&mut phone.conn, A, Some("Móvil de María"), None).unwrap();
    let profile = repo::insert_user_profile(
        &mut phone.conn,
        NewUserProfile {
            display_name: "María".into(),
            operator_id: None,
        },
        None,
    )
    .unwrap();

    repo::update_farm(
        &mut phone.conn,
        &farm_id,
        farm_named("La Vega"),
        Some(&profile.id),
    )
    .unwrap();
    rename(&mut laptop, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);

    let review = read(&phone, "farm", &farm_id);
    let mine = review
        .versions
        .iter()
        .find(|version| version.device == A)
        .unwrap();
    assert_eq!(mine.label.as_deref(), Some("Móvil de María"));
    assert_eq!(mine.actor.as_deref(), Some(profile.id.as_str()));
    assert_eq!(mine.actor_name.as_deref(), Some("María"));
    assert!(
        mine.changed_at.ends_with('Z'),
        "the instant an inspector reads, never the clock the merge orders by"
    );

    let theirs = review
        .versions
        .iter()
        .find(|version| version.device == B)
        .unwrap();
    assert_eq!(
        theirs.label, None,
        "a device nobody has named yet is still a version"
    );
}

// ---------------------------------------------------------------------------
// Choosing
// ---------------------------------------------------------------------------

#[test]
fn keeping_the_version_the_book_is_not_showing_makes_it_the_book_everywhere() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut away = Device::new(D);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, &mut away]);
    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);
    assert_eq!(phone.farm_name(&farm_id).as_deref(), Some("El Soto"));

    // A person keeps the phone's version — the one the book is NOT showing,
    // which is the case the whole rewind exists for.
    let keep = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    assert_eq!(phone.farm_name(&farm_id).as_deref(), Some("La Vega"));
    assert!(!phone.conflicted("farm", &farm_id));
    assert!(phone.conflict_rows("farm", &farm_id).is_empty());

    sync_both(&mut phone, &mut laptop);
    assert_eq!(
        laptop.farm_name(&farm_id).as_deref(),
        Some("La Vega"),
        "the device that was showing the other version adopts the choice"
    );
    assert!(!laptop.conflicted("farm", &farm_id));

    // And a device that never saw the conflict receives branches and choice
    // together, replaying the lot from nothing.
    sync(&laptop, &mut away);
    assert_eq!(away.farm_name(&farm_id).as_deref(), Some("La Vega"));
    assert!(away.conflict_rows("farm", &farm_id).is_empty());
}

#[test]
fn keeping_the_live_version_closes_the_conflict_and_leaves_the_book_alone() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);

    let live = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == B)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &live.device,
        live.seq,
        None,
    )
    .unwrap();

    assert_eq!(phone.farm_name(&farm_id).as_deref(), Some("El Soto"));
    assert!(!phone.conflicted("farm", &farm_id));
    sync_both(&mut phone, &mut laptop);
    assert!(!laptop.conflicted("farm", &farm_id));
    assert_eq!(laptop.farm_name(&farm_id).as_deref(), Some("El Soto"));
}

#[test]
fn nothing_is_lost_when_a_version_is_not_kept() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);

    let keep = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    let names: Vec<String> = phone
        .conn
        .prepare(
            "SELECT json_extract(payload, '$.after.name') FROM record_change
             WHERE entity_table = 'farm' ORDER BY hlc",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(
        names.contains(&"El Soto".to_string()),
        "the version nobody kept is still in the log, which is the audit trail"
    );
    assert_eq!(
        names.last().map(String::as_str),
        Some("La Vega"),
        "and the resolution states the kept version, so a replay lands on it"
    );
}

#[test]
fn keeping_a_version_takes_the_other_branch_s_children_with_it() {
    // The deletion half, and the one a naive resolution gets wrong: the losing
    // branch added a row, so the choice has to say that row is gone — or a
    // device replaying the log would apply the addition and never undo it.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut away = Device::new(D);
    let (farm_id, season_id, first, second) = one_book(&mut phone);
    sync(&phone, &mut laptop);
    sync(&phone, &mut away);

    let record = repo::insert_sowing_record(
        &mut phone.conn,
        NewSowingRecord {
            season_id,
            farm_id,
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: first.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record;
    sync(&phone, &mut laptop);
    sync(&phone, &mut away);

    // The phone adds a plot; the laptop writes a note. The laptop's version
    // goes live, so the phone's plot is already rewound out of its tables.
    repo::update_sowing_record(
        &mut phone.conn,
        &record.id,
        sowing_state(None, &[&first, &second]),
        None,
    )
    .unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record.id,
        sowing_state(Some("2ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    assert_eq!(sown_plots(&phone), vec![first.clone()]);

    // A person keeps the phone's version: both plots come back, and the note
    // goes with the version that is not kept.
    let keep = phone
        .heads_of("sowing_record", &record.id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "sowing_record",
        &record.id,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    let sown = |device: &Device| {
        let mut plots = sown_plots(device);
        plots.sort();
        plots
    };
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(sown(&phone), expected, "the kept version's plots");
    let note: Option<String> = phone
        .conn
        .query_row(
            "SELECT notes FROM sowing_record WHERE id = ?1",
            [&record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(note, None, "and the note the other version added is gone");

    sync_both(&mut phone, &mut laptop);
    sync(&phone, &mut away);
    assert_eq!(sown(&laptop), expected, "on the device that lost");
    assert_eq!(
        sown(&away),
        expected,
        "and on one replaying the whole lineage from nothing"
    );
    let away_note: Option<String> = away
        .conn
        .query_row(
            "SELECT notes FROM sowing_record WHERE id = ?1",
            [&record.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(away_note, None);
}

/// The row `writer` added for `plot` to a sowing, as its log names it.
fn plot_row_added_by(device: &Device, writer: &str, plot: &str) -> String {
    device
        .conn
        .query_row(
            "SELECT entity_id FROM record_change
             WHERE entity_table = 'sowing_plot' AND operation = 'insert'
               AND origin_device = ?1 AND json_extract(payload, '$.after.plot_id') = ?2",
            [writer, plot],
            |r| r.get(0),
        )
        .unwrap()
}

/// Two versions can hold one child slot under rows of their own: both devices
/// added the same plot. Keeping the one whose row sorts first, a resolution
/// written in key order inserted the arriving row while the leaving one still
/// held the slot, and the choice failed on a raw `UNIQUE` error. The apply
/// frees a slot before filling it (`merge::make_live`), whatever order a
/// change set was written in. *Found by the audit (2026-10-03).*
#[test]
fn keeping_a_version_that_holds_a_plot_under_another_row_frees_the_slot_first() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut away = Device::new(D);
    let (record, first, second) = a_sown_record(&mut phone, &mut [&mut laptop, &mut away]);

    later();
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(None, &[&first, &second]),
        None,
    )
    .unwrap();
    later();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("2ª pasada"), &[&first, &second]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    // The laptop's version is live, and the phone's row for the plot was made
    // first, so its id sorts first: the order in which the arriving row came
    // before the leaving one.
    let phones = plot_row_added_by(&phone, A, &second);
    let laptops = plot_row_added_by(&phone, B, &second);
    assert!(
        phones < laptops,
        "the case under test: the kept row sorts first"
    );
    let keep = phone
        .heads_of("sowing_record", &record)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "sowing_record",
        &record,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    sync_both(&mut phone, &mut laptop);
    sync(&phone, &mut away);
    let held = |device: &Device| -> Vec<String> {
        device
            .conn
            .prepare("SELECT id FROM sowing_plot WHERE plot_id = ?1")
            .unwrap()
            .query_map([&second], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    for device in [&phone, &laptop, &away] {
        assert_eq!(held(device), vec![phones.clone()], "{}", device.id);
        assert!(!device.conflicted("sowing_record", &record));
    }
}

/// The registers one device's newest change set wrote, as `(table, rows)`.
fn newest_set_wrote(device: &Device) -> Vec<(String, i64)> {
    device
        .conn
        .prepare(
            "SELECT entity_table, COUNT(*) FROM record_change
             WHERE origin_device = ?1
               AND origin_seq = (SELECT MAX(origin_seq) FROM record_change WHERE origin_device = ?1)
             GROUP BY entity_table ORDER BY entity_table",
        )
        .unwrap()
        .query_map([&device.id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// How many plots a device's sowing record covers.
fn plots_of(device: &Device, record: &str) -> i64 {
    device
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sowing_plot WHERE sowing_record_id = ?1",
            [record],
            |r| r.get(0),
        )
        .unwrap()
}

/// A choice where either version is a removal writes what differs, as any
/// other choice does: every device the change set reaches holds both branches
/// to rebuild the rest from (docs/sync.md → Bringing a register back writes
/// only what changes). Keeping a removal over a correction is the record's own
/// row — and the record stays removed on both devices, its plots as they were.
#[test]
fn keeping_a_removal_over_a_correction_writes_only_what_differs() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (record, first, second) = a_sown_record(&mut phone, &mut [&mut laptop]);
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(None, &[&first, &second]),
        None,
    )
    .unwrap();
    sync(&phone, &mut laptop);

    later();
    repo::soft_delete_sowing_record(&mut phone.conn, &record, None).unwrap();
    later();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("2ª pasada"), &[&first, &second]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);
    assert!(laptop.conflicted("sowing_record", &record));

    let removal = laptop
        .heads_of("sowing_record", &record)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut laptop.conn,
        "sowing_record",
        &record,
        &removal.device,
        removal.seq,
        None,
    )
    .unwrap();

    assert_eq!(
        newest_set_wrote(&laptop),
        vec![("sowing_record".to_owned(), 1)],
        "the record's own row — none of its plots, which both versions agree on"
    );
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        let removed: bool = device
            .conn
            .query_row(
                "SELECT deleted_at IS NOT NULL FROM sowing_record WHERE id = ?1",
                [&record],
                |r| r.get(0),
            )
            .unwrap();
        assert!(removed, "{}: and it stays removed", device.id);
        assert_eq!(plots_of(device, &record), 2, "{}", device.id);
        assert!(!device.conflicted("sowing_record", &record));
    }
}

/// The other way round: keeping a correction over the removal the book shows
/// brings the record back with its own row alone, and it comes back whole on
/// both devices.
#[test]
fn keeping_a_correction_over_a_removal_writes_only_what_differs() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (record, first, _) = a_sown_record(&mut phone, &mut [&mut laptop]);

    // The correction first, the removal after it: the removal carries the later
    // clock, so it is the version the book shows.
    later();
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(Some("corrected"), &[&first]),
        None,
    )
    .unwrap();
    later();
    repo::soft_delete_sowing_record(&mut laptop.conn, &record, None).unwrap();
    sync_both(&mut phone, &mut laptop);
    let heads = laptop.heads_of("sowing_record", &record);
    assert_eq!(
        terrazgo_core::merge::live_head(&heads).unwrap().device,
        B,
        "the removal shows"
    );
    let correction = heads.into_iter().find(|head| head.device == A).unwrap();

    resolve(
        &mut laptop.conn,
        "sowing_record",
        &record,
        &correction.device,
        correction.seq,
        None,
    )
    .unwrap();

    assert_eq!(
        newest_set_wrote(&laptop),
        vec![("sowing_record".to_owned(), 1)],
        "the record's own row, its plot unchanged"
    );
    sync_both(&mut phone, &mut laptop);
    for device in [&phone, &laptop] {
        let notes: Option<String> = device
            .conn
            .query_row(
                "SELECT notes FROM sowing_record WHERE id = ?1 AND deleted_at IS NULL",
                [&record],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notes.as_deref(), Some("corrected"), "{}", device.id);
        assert_eq!(plots_of(device, &record), 1, "{}", device.id);
        assert!(!device.conflicted("sowing_record", &record));
    }
}

#[test]
fn keeping_one_of_three_versions_settles_all_of_them() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut tablet = Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, &mut tablet]);

    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    rename(&mut tablet, &farm_id, "Las Eras");
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut laptop, &mut tablet);
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut phone, &mut tablet);
    assert_eq!(phone.heads_of("farm", &farm_id).len(), 3);

    let review = read(&phone, "farm", &farm_id);
    assert_eq!(review.versions.len(), 3, "three versions, one screen");
    assert_eq!(
        values(line(&review, "name")).len(),
        3,
        "and a value from each on the one line they disagree about"
    );

    let keep = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    assert!(
        !phone.conflicted("farm", &farm_id),
        "one decision settles a three-way conflict, not two of three"
    );
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut phone, &mut tablet);
    for device in [&phone, &laptop, &tablet] {
        assert_eq!(device.farm_name(&farm_id).as_deref(), Some("La Vega"));
        assert!(device.conflict_rows("farm", &farm_id).is_empty());
    }
}

#[test]
fn a_version_somebody_has_already_resolved_away_is_refused() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);
    let stale = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();

    // The laptop decides first, and its decision reaches the phone.
    let theirs = laptop
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == B)
        .unwrap();
    resolve(
        &mut laptop.conn,
        "farm",
        &farm_id,
        &theirs.device,
        theirs.seq,
        None,
    )
    .unwrap();
    sync(&laptop, &mut phone);

    // A person on the phone now clicks the choice their screen still shows.
    assert!(
        matches!(
            resolve(
                &mut phone.conn,
                "farm",
                &farm_id,
                &stale.device,
                stale.seq,
                None
            ),
            Err(CoreError::Invalid("conflict_version_gone"))
        ),
        "quietly applying it would overwrite a decision somebody already made"
    );
    assert_eq!(phone.farm_name(&farm_id).as_deref(), Some("El Soto"));
}

#[test]
fn resolving_a_register_that_is_no_longer_in_conflict_does_nothing() {
    // Two people can decide the same way at the same time; the second one to be
    // asked has nothing left to do, and that is not an error.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = conflicted_farm(&mut phone, &mut laptop);
    let live = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == B)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &live.device,
        live.seq,
        None,
    )
    .unwrap();

    let before: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();
    let head = phone.heads_of("farm", &farm_id).into_iter().next().unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &head.device,
        head.seq,
        None,
    )
    .unwrap();
    let after: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();

    assert_eq!(before, after, "a settled register is not written again");
}

#[test]
fn two_versions_that_agree_in_content_still_need_the_choice_written_down() {
    // Two devices naming one phone the same thing, offline. Nothing differs, so
    // a resolution that only logged differences would log nothing — and the
    // conflict would outlive the decision.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    repo::register_this_device(&mut phone.conn, None).unwrap();
    sync(&phone, &mut laptop);

    repo::rename_sync_peer(&mut phone.conn, A, Some("Móvil"), None).unwrap();
    repo::rename_sync_peer(&mut laptop.conn, A, Some("Móvil"), None).unwrap();
    sync_both(&mut phone, &mut laptop);
    assert!(phone.conflicted("sync_peer", A), "concurrent, though equal");

    let keep = phone
        .heads_of("sync_peer", A)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "sync_peer",
        A,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();

    assert!(!phone.conflicted("sync_peer", A));
    sync_both(&mut phone, &mut laptop);
    assert!(!laptop.conflicted("sync_peer", A));
    let label: Option<String> = laptop
        .conn
        .query_row("SELECT label FROM sync_peer WHERE id = ?1", [A], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(label.as_deref(), Some("Móvil"));
}

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

#[test]
fn the_queue_lists_one_entry_per_register_with_every_version_named() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut tablet = Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, &mut tablet]);
    // A device's own row is what tells the others it exists, so the laptop can
    // only be NAMED on the phone once the laptop has written itself down and
    // that row has travelled.
    repo::register_this_device(&mut phone.conn, None).unwrap();
    repo::register_this_device(&mut laptop.conn, None).unwrap();
    sync(&laptop, &mut phone);
    repo::rename_sync_peer(&mut phone.conn, B, Some("Portátil"), None).unwrap();

    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    rename(&mut tablet, &farm_id, "Las Eras");
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut laptop, &mut tablet);
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut phone, &mut tablet);

    let queue = repo::list_sync_conflicts(&phone.conn, CORE_ROW_CAPTIONS).unwrap();
    assert_eq!(
        queue.len(),
        1,
        "three versions of one register is one entry"
    );
    let entry = &queue[0];
    assert_eq!(entry.root_table, "farm");
    assert_eq!(entry.root_id, farm_id);
    assert_eq!(entry.devices.len(), 3);
    assert!(entry.devices[0].live, "the live version leads");
    assert_eq!(
        entry.caption,
        phone.farm_name(&farm_id),
        "the queue names the register as the book shows it"
    );
    let named = entry
        .devices
        .iter()
        .find(|device| device.device == B)
        .unwrap();
    assert_eq!(named.label.as_deref(), Some("Portátil"));
    assert_eq!(
        entry.season_id, None,
        "a holding's register belongs to no book"
    );

    // And it empties with the decision, since it is derived from the log.
    let keep = phone
        .heads_of("farm", &farm_id)
        .into_iter()
        .find(|head| head.device == A)
        .unwrap();
    resolve(
        &mut phone.conn,
        "farm",
        &farm_id,
        &keep.device,
        keep.seq,
        None,
    )
    .unwrap();
    assert!(
        repo::list_sync_conflicts(&phone.conn, CORE_ROW_CAPTIONS)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_conflicted_record_is_listed_under_the_book_it_is_in() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let (farm_id, season_id, first, _) = one_book(&mut phone);
    sync(&phone, &mut laptop);

    let record = repo::insert_sowing_record(
        &mut phone.conn,
        NewSowingRecord {
            season_id: season_id.clone(),
            farm_id,
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: first.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record;
    sync(&phone, &mut laptop);

    repo::update_sowing_record(
        &mut phone.conn,
        &record.id,
        sowing_state(Some("mañana"), &[&first]),
        None,
    )
    .unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record.id,
        sowing_state(Some("tarde"), &[&first]),
        None,
    )
    .unwrap();
    sync_both(&mut phone, &mut laptop);

    let queue = repo::list_sync_conflicts(&phone.conn, CORE_ROW_CAPTIONS).unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].season_id.as_deref(), Some(season_id.as_str()));
    assert_eq!(
        queue[0].season_label.as_deref(),
        Some("2025/2026"),
        "named, so a queue read across books says which one"
    );
    assert_eq!(
        queue[0].farm_name.as_deref(),
        Some("Los Llanos"),
        "and whose: two farms may each keep a 2025/2026"
    );
    assert_eq!(
        queue[0].caption.as_deref(),
        Some("2026-04-10"),
        "a sowing is known by the day it was sown"
    );
}

/// A farm update that changes only the name — the form submits the whole state.
fn farm_named(name: &str) -> UpdateFarm {
    UpdateFarm {
        name: name.into(),
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
        es: None,
        representative: None,
    }
}

// ---------------------------------------------------------------------------
// What the queue costs
// ---------------------------------------------------------------------------

#[test]
fn the_queue_costs_a_statement_per_table_rather_than_per_conflict() {
    // The review screen reads this list, and a list that runs a query per row
    // is the defect docs/data-model.md → "Indexes and query scope" exists to
    // stop. The names come from three batched reads — the devices, the
    // registers' own tables, the campaigns — so more conflicts cost more ROWS
    // and not more statements.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    // Six conflicted plots of one farm, all in the same table.
    let mut plots = Vec::new();
    for index in 0..6 {
        let plot = repo::insert_plot(
            &mut phone.conn,
            new_plot(&farm_id, &format!("Parcela {index}")),
            None,
        )
        .unwrap();
        plots.push(plot.id);
    }
    sync(&phone, &mut laptop);
    for (index, plot_id) in plots.iter().enumerate() {
        let rename = |device: &mut Device, name: &str| {
            repo::update_plot(
                &mut device.conn,
                plot_id,
                UpdatePlot {
                    name: name.into(),
                    area_ha: Some(2.0),
                    es: None,
                },
                None,
            )
            .unwrap();
        };
        rename(&mut phone, &format!("El Prado {index}"));
        rename(&mut laptop, &format!("La Loma {index}"));
    }
    sync_both(&mut phone, &mut laptop);

    let (queue, cost) = query_cost(&mut phone.conn, |conn| {
        repo::list_sync_conflicts(conn, CORE_ROW_CAPTIONS).unwrap()
    });
    assert_eq!(queue.len(), 6, "six registers waiting");
    assert_eq!(
        cost.statements, 3,
        "the queue itself, the devices' labels and the plots' names — and \
         nothing per conflict. A season lookup runs only when a conflict is in \
         a book, and none of these is"
    );
}
