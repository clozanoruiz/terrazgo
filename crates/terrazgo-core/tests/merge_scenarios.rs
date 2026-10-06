// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The merge layer at the hard cases, against simulated devices.
//!
//! docs/sync.md → "Conflicts as the person sees them" asks for exactly this
//! and says why: **"A rare path with no test is a path that is wrong."** Two
//! people editing one register at once should be uncommon, so nothing else in
//! the suite will ever reach these states by accident.
//!
//! Every device here is a real database at the core schema with its own device
//! id, and every write goes through the real repositories. What is simulated is
//! only the *transport*: [`sync`] copies log rows from one device to another
//! the way a bundle will in slice 3. That split is the design's own — the merge
//! rules are defined without reference to how deltas travel — so testing them
//! against a function that copies rows is not a shortcut, it is the seam.
//!
//! The properties asserted throughout, from that section of the doc:
//!
//!   * **the book is never broken and never blocks** — every device shows the
//!     same thing after syncing, before anybody resolves anything;
//!   * **nothing is lost** — the loser stays in the log;
//!   * **resolving is an ordinary write** — and it closes the conflict
//!     everywhere, with no special message.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use common::*;
use rusqlite::params;
use terrazgo_core::CoreError;
use terrazgo_core::merge::{live_head, settle};
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

#[test]
fn two_devices_editing_one_register_agree_on_what_is_live() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);

    assert!(
        phone.conflicted("farm", &farm_id),
        "a conflict is two heads"
    );
    assert!(laptop.conflicted("farm", &farm_id));
    // And it reaches the review queue, on both devices, naming the branch that
    // is NOT live — the one a person has to choose or discard.
    assert_eq!(phone.conflict_rows("farm", &farm_id), [(A.to_string(), 2)]);
    assert_eq!(
        phone.conflict_rows("farm", &farm_id),
        laptop.conflict_rows("farm", &farm_id),
        "each device derives the same queue from the same log"
    );
    let live = phone.farm_name(&farm_id);
    assert_eq!(
        live,
        laptop.farm_name(&farm_id),
        "the book is never broken: both devices show the same version"
    );
    // Named explicitly, so the assertion above cannot pass on two devices that
    // both lost the row or both kept the pre-conflict name.
    //
    // Not flaky, and worth saying why: the laptop edits second, so either its
    // clock is genuinely later, or the two land in the same millisecond with
    // counter 0 each (separate logs) and the device-id tie-break applies. B
    // sorts after A, so B wins either way.
    assert_eq!(
        live.as_deref(),
        Some("El Soto"),
        "B's edit carries the later clock, and both devices must show it"
    );
    // And nothing is lost — both versions are still in the log.
    let names: BTreeSet<String> = phone
        .conn
        .prepare(
            "SELECT json_extract(payload, '$.after.name') FROM record_change
                  WHERE entity_table = 'farm' AND operation = 'update'",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(names.contains("La Vega") && names.contains("El Soto"));
}

#[test]
fn an_edit_concurrent_with_a_soft_delete_still_settles() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    rename(&mut phone, &farm_id, "La Vega");
    repo::soft_delete_farm(&mut laptop.conn, &farm_id, None).unwrap();
    sync_both(&mut phone, &mut laptop);

    assert!(phone.conflicted("farm", &farm_id));
    // Whichever side went live, both devices agree — including on whether the
    // farm is deleted, which is an ordinary column here and not a special case.
    let deleted_on = |device: &Device| -> Option<String> {
        device
            .conn
            .query_row(
                "SELECT deleted_at FROM farm WHERE id = ?1",
                [&farm_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap()
    };
    assert_eq!(deleted_on(&phone), deleted_on(&laptop));
    assert_eq!(phone.farm_name(&farm_id), laptop.farm_name(&farm_id));
}

#[test]
fn three_devices_editing_at_once_still_agree() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let tablet = &mut Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, tablet]);

    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    rename(tablet, &farm_id, "Las Eras");

    sync_both(&mut phone, &mut laptop);
    sync_both(&mut laptop, tablet);
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut phone, tablet);

    assert_eq!(phone.heads_of("farm", &farm_id).len(), 3);
    assert_eq!(
        phone.conflict_rows("farm", &farm_id).len(),
        2,
        "three heads are two branches waiting, not one conflict of three"
    );
    let shown = phone.farm_name(&farm_id);
    assert_eq!(shown, laptop.farm_name(&farm_id));
    assert_eq!(
        shown,
        tablet.farm_name(&farm_id),
        "three-way disagreement still shows one book"
    );
    assert!(
        ["La Vega", "El Soto", "Las Eras"].contains(&shown.unwrap_or_default().as_str()),
        "and it shows one of the three edits, not the name they all replaced"
    );
}

#[test]
fn resolving_closes_the_conflict_everywhere_including_a_device_that_was_away() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut away = Device::new(D);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, &mut away]);

    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);
    assert!(phone.conflicted("farm", &farm_id));

    // A person resolves on the phone: an ordinary correction, no special call.
    // Its stamp merges both heads, because `register` computes the vector from
    // every version the register has been written with.
    rename(&mut phone, &farm_id, "La Vega");
    assert!(
        !phone.conflicted("farm", &farm_id),
        "an ordinary write closes it where it was made"
    );

    assert!(
        phone.conflict_rows("farm", &farm_id).is_empty(),
        "and the review queue empties with it — nothing to patch, it is derived"
    );

    sync_both(&mut phone, &mut laptop);
    assert!(!laptop.conflicted("farm", &farm_id));
    assert!(laptop.conflict_rows("farm", &farm_id).is_empty());

    // The fourth device was offline throughout and never saw the conflict at
    // all — it receives the resolution and the branches together.
    sync(&laptop, &mut away);
    assert!(!away.conflicted("farm", &farm_id));
    assert!(
        away.conflict_rows("farm", &farm_id).is_empty(),
        "a device that never saw the conflict is never shown it"
    );
    assert_eq!(away.farm_name(&farm_id).as_deref(), Some("La Vega"));
    assert_eq!(away.farm_name(&farm_id), phone.farm_name(&farm_id));
}

#[test]
fn the_same_bundle_applied_twice_changes_nothing() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    rename(&mut phone, &farm_id, "La Vega");

    sync(&phone, &mut laptop);
    let once = laptop.farm_name(&farm_id);
    let rows_once: i64 = laptop
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();

    sync(&phone, &mut laptop);
    let twice = laptop.farm_name(&farm_id);
    let rows_twice: i64 = laptop
        .conn
        .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
        .unwrap();

    assert_eq!(
        once, twice,
        "applying a bundle twice is not a second change"
    );
    assert_eq!(rows_once, rows_twice);
}

#[test]
fn a_register_a_device_never_held_arrives_whole() {
    // The plain case the conflict machinery must not complicate: nothing to
    // rewind, so the incoming branch is replayed from nothing.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    sync(&phone, &mut laptop);

    assert_eq!(laptop.farm_name(&farm.id).as_deref(), Some("La Vega"));
    assert!(!laptop.conflicted("farm", &farm.id));
}

/// One campaign, two devices, bounds guessed a month apart.
///
/// The gap the derived season id cannot close (docs/sync.md → Seasons created
/// on two devices): the id needs the exact dates, while the LABEL comes from
/// the years, so two books collide on `(farm, label)` without colliding on the
/// id. The UNIQUE stays — every way of enforcing it in the merge would either
/// fuse two different campaigns or invent a name for a document that gets
/// printed — so the apply stops and a person decides.
///
/// What slice 2 owes that decision is a refusal it can act on. The resolution
/// itself is later: merging two books is slice 8, and until then the refusal
/// offers the rename.
fn one_campaign_dated_from(device: &mut Device, farm_id: &str, starts_on: &str) -> Season {
    repo::insert_season(
        &mut device.conn,
        NewSeason {
            farm_id: farm_id.into(),
            starts_on: starts_on.into(),
            ends_on: "2027-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap()
}

#[test]
fn two_books_for_one_campaign_are_refused_in_words_a_person_can_act_on() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    let theirs = one_campaign_dated_from(&mut phone, &farm_id, "2026-09-01");
    let ours = one_campaign_dated_from(&mut laptop, &farm_id, "2026-10-01");
    assert_ne!(theirs.id, ours.id, "different dates, so different ids");
    assert_eq!(theirs.label, ours.label, "but one name, from the years");

    let refused = try_sync(&phone, &mut laptop);
    // Named down to the book: the other one is still in the file, so nothing on
    // this device's screens could say which book to rename, or on which farm.
    match &refused {
        Err(CoreError::SeasonLabelCollision { farm, label }) => {
            assert_eq!(farm, "Los Llanos");
            assert_eq!(label, &ours.label);
        }
        other => panic!("expected a named refusal a form can translate, got {other:?}"),
    }

    // Refused whole: nothing of the bundle is left behind, so retrying after
    // the farmer decides starts from the same place.
    let books: i64 = laptop
        .conn
        .query_row("SELECT COUNT(*) FROM season", [], |r| r.get(0))
        .unwrap();
    assert_eq!(books, 1, "the apply left nothing half-done");
    assert!(laptop.conn.query_row(
        "SELECT COUNT(*) FROM record_change WHERE entity_table = 'season' AND entity_id = ?1",
        [&theirs.id],
        |r| r.get::<_, i64>(0)
    ).unwrap() == 0);
}

#[test]
fn renaming_one_book_lets_the_sync_through() {
    // The farmer's way out, and the reason the refusal is worth making legible:
    // one edit and the import goes through, with no re-transfer.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    one_campaign_dated_from(&mut phone, &farm_id, "2026-09-01");
    let ours = one_campaign_dated_from(&mut laptop, &farm_id, "2026-10-01");
    assert!(try_sync(&phone, &mut laptop).is_err());

    repo::update_season(
        &mut laptop.conn,
        &ours.id,
        UpdateSeason {
            starts_on: "2026-10-01".into(),
            ends_on: "2027-08-31".into(),
            custom_label: Some("2026/2027 Vega".into()),
        },
        None,
    )
    .unwrap();

    try_sync(&phone, &mut laptop).expect("the same bundle applies once the names differ");
    let books: i64 = laptop
        .conn
        .query_row("SELECT COUNT(*) FROM season", [], |r| r.get(0))
        .unwrap();
    assert_eq!(books, 2, "both campaigns are now here, told apart by name");
}

/// Two devices filling one UNIQUE child slot — the case slice 1's slot design
/// exists for, tested end to end here for the first time.
///
/// `farm_advisor` allows one live row per `(farm, advisor)` and replaces it
/// when rewritten. **Keyed by row id these would be two unrelated registers**:
/// nothing concurrent, nothing detected, and then an apply failing on the
/// UNIQUE index with the bundle refused whole, for ever. Keyed by the SLOT they
/// are two versions of one register, so they take the ordinary conflict path
/// (docs/sync.md → The aggregate map).
#[test]
fn two_devices_filling_one_slot_write_one_register() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    let advisor = repo::insert_advisor(
        &mut phone.conn,
        NewAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: None,
            registration_number: Some("CL-0042".into()),
        },
        None,
    )
    .unwrap();
    sync(&phone, &mut laptop);

    // Each device links the same advisor to the same farm, under a different
    // GIP framework — one slot, two contents, neither having seen the other.
    repo::set_farm_advisor(
        &mut phone.conn,
        &farm_id,
        &advisor.id,
        Some("atria".into()),
        None,
    )
    .unwrap();
    repo::set_farm_advisor(
        &mut laptop.conn,
        &farm_id,
        &advisor.id,
        Some("integrated_production".into()),
        None,
    )
    .unwrap();

    sync_both(&mut phone, &mut laptop);

    let slot: String = phone
        .conn
        .query_row(
            "SELECT DISTINCT root_id FROM record_change WHERE root_table = 'farm_advisor'",
            [],
            |r| r.get(0),
        )
        .expect("one slot, not two — both devices addressed the same register");

    assert_eq!(
        phone.heads_of("farm_advisor", &slot).len(),
        2,
        "two versions of ONE register, which is a conflict a person can see"
    );

    let framework = |device: &Device| -> Vec<String> {
        device
            .conn
            .prepare("SELECT gip_system_code FROM farm_advisor")
            .unwrap()
            .query_map([], |r| {
                r.get::<_, Option<String>>(0).map(|c| c.unwrap_or_default())
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(framework(&phone).len(), 1, "one live row in the slot");
    assert_eq!(
        framework(&phone),
        framework(&laptop),
        "and both devices show the same one"
    );
}

#[test]
fn a_conflict_on_two_different_rows_of_one_register_still_shows_one_book() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut away = Device::new(D);
    let (farm_id, season_id, first, second) = one_book(&mut phone);
    sync(&phone, &mut laptop);
    sync(&phone, &mut away);

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
    sync(&phone, &mut away);

    // The phone adds a plot; the laptop annotates the record. Row by row these
    // do not collide at all — which is exactly why the register, not the row,
    // is the unit.
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
    // And a third device that was away for all of it, so it has nothing to
    // rewind and replays the winner from nothing — the other path into
    // `make_live`, which has to reach the same book as the two that rewound.
    sync(&phone, &mut away);

    assert!(
        phone.conflicted("sowing_record", &record.id),
        "two people stated two different things about one record"
    );
    assert_eq!(
        sown_plots(&phone),
        sown_plots(&laptop),
        "the book is never broken: the devices that rewound agree"
    );
    assert_eq!(
        sown_plots(&phone),
        sown_plots(&away),
        "and so does the one that replayed the winner from nothing"
    );
    assert_eq!(
        sown_plots(&phone),
        vec![first],
        "the winning statement's child set is what the tables hold, whole"
    );
    // Nothing is lost: the plot the losing branch added is still in the log,
    // which is what the review screen reads to show the other version.
    let logged: i64 = phone
        .conn
        .query_row(
            "SELECT COUNT(*) FROM record_change
             WHERE entity_table = 'sowing_plot' AND json_extract(payload, '$.after.plot_id') = ?1",
            [&second],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(logged, 1);
}

/// The register a conflict has been resolved on holds the same rows everywhere
/// — including on devices that only ever saw the resolution.
///
/// **A rewind is not written down** (docs/sync.md → A rewind is not written
/// down, so a replay has to infer it). The device that lost undid its rows and
/// the log kept no record of it, so once a resolution has seen both branches
/// the losing set stops being a head and becomes ordinary ancestry inside the
/// winner's lineage. A replay that took the whole lineage would put those rows
/// back on every device except the one that rewound them, and no bundle would
/// ever take them away again.
///
/// It has to be a register with children and two branches touching DIFFERENT
/// rows, because that is the only shape where the losing rows are not simply
/// overwritten by the winner's.
#[test]
fn a_resolution_leaves_every_device_holding_the_same_register() {
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

    // The laptop's version went live, so the phone's added plot was rewound out
    // of the phone's own tables. A person on the phone now resolves by keeping
    // what the book is showing — an ordinary write, which is the whole of what
    // resolving is.
    let showing: Vec<String> = sown_plots(&phone);
    assert_eq!(showing, vec![first.clone()], "the phone rewound its plot");
    repo::update_sowing_record(
        &mut phone.conn,
        &record.id,
        sowing_state(
            Some("2ª pasada"),
            &showing.iter().map(String::as_str).collect::<Vec<_>>(),
        ),
        None,
    )
    .unwrap();
    assert!(!phone.conflicted("sowing_record", &record.id));

    sync_both(&mut phone, &mut laptop);
    sync(&phone, &mut away);

    assert_eq!(
        sown_plots(&phone),
        vec![first],
        "the resolution stands on the device it was made on"
    );
    assert_eq!(
        sown_plots(&laptop),
        sown_plots(&phone),
        "and the losing branch does not come back on the device that never held it"
    );
    assert_eq!(
        sown_plots(&away),
        sown_plots(&phone),
        "nor on one that replays the whole lineage from nothing"
    );
}

/// **A change set both versions have seen can be part of one version only** —
/// beaten on one side, kept on the other — so it is shared history to neither
/// (docs/sync.md → Applying a winner needs rewind and replay).
///
/// The phone adds a plot while the laptop writes a note, and the laptop's note
/// goes live there; the laptop then edits again, closing the conflict on top of
/// its own version, while the phone edits again on top of its plot. Both last
/// versions have seen the plot, so a split by what both have seen touches it in
/// neither direction: the laptop, making the phone's later edit live, never
/// replays the plot it had never shown. *Found by the audit (2026-10-03)*:
/// one log, two books, for good.
#[test]
fn a_change_both_versions_saw_is_replayed_where_only_one_of_them_kept_it() {
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
        sowing_state(Some("2ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    sync(&phone, &mut laptop);
    assert!(laptop.conflicted("sowing_record", &record));
    assert_eq!(plots_sown(&laptop), vec![first.clone()], "the note is live");

    // The laptop edits again from the record's form; then the phone, later.
    later();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record,
        sowing_state(Some("3ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    later();
    repo::update_sowing_record(
        &mut phone.conn,
        &record,
        sowing_state(Some("del móvil"), &[&first, &second]),
        None,
    )
    .unwrap();

    sync(&phone, &mut laptop);
    sync(&laptop, &mut phone);
    sync(&phone, &mut away);
    let mut both = vec![first, second];
    both.sort();
    assert_eq!(plots_sown(&phone), both, "the phone's last version is live");
    assert_eq!(
        plots_sown(&laptop),
        both,
        "and the laptop replays the plot it never showed"
    );
    assert_eq!(
        plots_sown(&away),
        both,
        "as one replaying from nothing does"
    );
}

/// The other face of the same rule: the version this device shows lost where
/// the file was written. The laptop keeps its own version of a conflict and
/// sends one file holding its note and the choice; the phone, which had shown
/// its own plot, must take the plot back out — the commonest way to resolve,
/// two devices, one file.
#[test]
fn a_version_that_lost_where_the_file_was_written_is_rewound_here() {
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
        sowing_state(Some("2ª pasada"), &[&first]),
        None,
    )
    .unwrap();
    sync(&phone, &mut laptop);
    let live = live_head(&laptop.heads_of("sowing_record", &record))
        .cloned()
        .unwrap();
    assert_eq!(live.device, B, "the laptop's note is live there");
    terrazgo_core::merge::resolve(
        &mut laptop.conn,
        "sowing_record",
        &record,
        &live.device,
        live.seq,
        None,
    )
    .unwrap();

    // One file: the note and the choice, arriving together.
    sync(&laptop, &mut phone);
    sync(&laptop, &mut away);
    assert_eq!(plots_sown(&laptop), vec![first.clone()]);
    assert_eq!(
        plots_sown(&phone),
        vec![first.clone()],
        "the phone takes back out the plot the kept version never had"
    );
    assert_eq!(plots_sown(&away), vec![first]);
    assert!(!phone.conflicted("sowing_record", &record));
}

#[test]
fn a_deleted_book_frees_its_name_for_one_arriving_from_another_device() {
    // `idx_season_farm_label_active` is partial — `WHERE deleted_at IS NULL` —
    // so a book that arrives deleted collides with nothing. A check stricter
    // than the index it stands in for would refuse a whole bundle the schema
    // would have taken.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);

    let buried = repo::insert_season(
        &mut phone.conn,
        NewSeason {
            farm_id: farm_id.clone(),
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    repo::delete_book(&mut phone.conn, &buried.id, &[], None).unwrap();

    // The laptop keeps a LIVE book of the same campaign year — so the same name
    // from the years, and different dates, so a different derived id.
    let live = repo::insert_season(
        &mut laptop.conn,
        NewSeason {
            farm_id,
            starts_on: "2025-10-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(buried.label, live.label);
    assert_ne!(buried.id, live.id);

    try_sync(&phone, &mut laptop).expect("a book on its way out collides with nothing");
    let books: i64 = laptop
        .conn
        .query_row(
            "SELECT COUNT(*) FROM season WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(books, 1, "one live book, and the deleted one beside it");
}

#[test]
fn a_row_arriving_under_a_table_that_never_syncs_is_refused() {
    // `audit::write_change` holds every row a device WRITES to the aggregate
    // map. A row that ARRIVES has had none of that done to it, and the applier
    // splices its table name into SQL — so the same map has to answer for it on
    // the way in.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    rename(&mut phone, &farm_id, "La Vega");

    phone
        .conn
        .execute(
            "UPDATE record_change SET entity_table = 'catalogue', entity_id = 'smuggled',
                 payload = json('{\"before\":null,\"after\":{\"id\":\"smuggled\",
                     \"source\":\"siex\",\"source_updated_at\":null,\"source_digest\":null,
                     \"imported_by_version\":null,\"imported_at\":\"2026-09-20T10:00:00Z\"}}')
             WHERE entity_table = 'farm' AND operation = 'update'",
            [],
        )
        .unwrap();

    let refused = try_sync(&phone, &mut laptop);
    assert!(
        matches!(refused, Err(CoreError::ShapeViolation(_))),
        "expected the map to refuse it, got {refused:?}"
    );
    let smuggled: i64 = laptop
        .conn
        .query_row(
            "SELECT COUNT(*) FROM catalogue WHERE id = 'smuggled'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        smuggled, 0,
        "and the bundle is refused whole, so nothing lands"
    );
}

#[test]
fn three_devices_filling_one_slot_leave_one_live_row() {
    // `two_devices_filling_one_slot_write_one_register` is the pair; three is
    // where a rewind has to free a slot that a replay is about to fill, with a
    // third version in the log that must not be materialised at all.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let tablet = &mut Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, tablet]);
    let advisor = repo::insert_advisor(
        &mut phone.conn,
        NewAdvisor {
            name: "Ana Ruiz".into(),
            tax_id: None,
            registration_number: Some("CL-0042".into()),
        },
        None,
    )
    .unwrap();
    sync(&phone, &mut laptop);
    sync(&phone, tablet);

    for (device, framework) in [
        (&mut phone, "atria"),
        (&mut laptop, "integrated_production"),
        (&mut *tablet, "organic"),
    ] {
        repo::set_farm_advisor(
            &mut device.conn,
            &farm_id,
            &advisor.id,
            Some(framework.into()),
            None,
        )
        .unwrap();
    }

    sync_both(&mut phone, &mut laptop);
    sync_both(&mut laptop, tablet);
    sync_both(&mut phone, &mut laptop);
    sync_both(&mut phone, tablet);

    let live = |device: &Device| -> Vec<Option<String>> {
        device
            .conn
            .prepare("SELECT gip_system_code FROM farm_advisor")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(
        live(&phone).len(),
        1,
        "one link, whatever three devices say"
    );
    assert_eq!(live(&phone), live(&laptop));
    assert_eq!(live(&phone), live(tablet));

    let slot: String = phone
        .conn
        .query_row(
            "SELECT DISTINCT root_id FROM record_change WHERE root_table = 'farm_advisor'",
            [],
            |r| r.get(0),
        )
        .expect("one slot, three versions");
    assert_eq!(phone.heads_of("farm_advisor", &slot).len(), 3);
    assert_eq!(
        phone.conflict_rows("farm_advisor", &slot).len(),
        2,
        "three heads are two branches waiting"
    );
}

/// A row leaving a slot and another entering it, applied where the one leaving
/// still stands. Found by the whole-schema random histories
/// (src-tauri/tests/contracts/random_histories.rs), on a book's "no treated
/// seed" declaration; a plot's water declaration has the same shape.
///
/// The phone re-declares while the laptop, then the tablet, replace and
/// withdraw — so the phone's version and the tablet's are concurrent, the
/// tablet's goes live by the clock, and the phone resolves keeping its own:
/// its row in, the laptop's out. On the phone the laptop's row had already
/// been withdrawn by the tablet. On the laptop it still stands, and nothing
/// rewinds it: the tablet's withdrawal and the resolution both came after it.
/// A branch is applied a row at a time — each row's last touch — so the
/// tablet's withdrawal of the laptop's row is replaced by the resolution's,
/// which the resolution wrote after its own row. Applied in that order, two
/// rows stand in one slot for a statement, and the partial UNIQUE index
/// refused the laptop every file the phone wrote from then on.
#[test]
fn a_row_leaving_a_slot_frees_it_before_another_takes_it_on_every_device() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let mut tablet = Device::new(C);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop, &mut tablet]);
    let plot = repo::insert_plot(&mut phone.conn, new_plot(&farm_id, "El Prado"), None)
        .unwrap()
        .id;
    repo::set_water_declaration(&mut phone.conn, &plot, "2026-01-10", None).unwrap();
    sync(&phone, &mut laptop);
    sync(&phone, &mut tablet);

    later();
    repo::clear_water_declaration(&mut laptop.conn, &plot, None).unwrap();
    sync(&laptop, &mut phone);
    sync(&laptop, &mut tablet);
    later();
    repo::set_water_declaration(&mut phone.conn, &plot, "2026-02-01", None).unwrap();
    later();
    repo::set_water_declaration(&mut laptop.conn, &plot, "2026-02-02", None).unwrap();
    sync(&laptop, &mut tablet);
    later();
    repo::clear_water_declaration(&mut tablet.conn, &plot, None).unwrap();

    sync(&tablet, &mut phone);
    let slot: String = phone
        .conn
        .query_row(
            "SELECT DISTINCT root_id FROM record_change WHERE root_table = 'plot_water_declaration'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let heads = phone.heads_of("plot_water_declaration", &slot);
    assert_eq!(heads.len(), 2, "the phone's version against the tablet's");
    let own = heads.iter().find(|head| head.device == A).unwrap();
    terrazgo_core::merge::resolve(
        &mut phone.conn,
        "plot_water_declaration",
        &slot,
        &own.device,
        own.seq,
        None,
    )
    .unwrap();

    try_sync(&phone, &mut laptop).expect("the laptop takes the phone's file");
    sync(&phone, &mut tablet);
    let standing = |device: &Device| -> Vec<String> {
        device
            .conn
            .prepare(
                "SELECT declared_on FROM plot_water_declaration
                 WHERE deleted_at IS NULL ORDER BY id",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    assert_eq!(standing(&phone), vec!["2026-02-01".to_owned()]);
    assert_eq!(standing(&laptop), standing(&phone));
    assert_eq!(standing(&tablet), standing(&phone));
}

#[test]
fn settling_a_register_that_is_already_settled_changes_nothing() {
    // An importer may settle a register the delivery touched twice — two
    // bundles in one session, a retry after a refusal. `settle` is the whole
    // of what it does, so it has to be safe to run again: the tables must not
    // move and the review queue must be rewritten to the same rows, not
    // appended to.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    rename(&mut phone, &farm_id, "La Vega");
    rename(&mut laptop, &farm_id, "El Soto");
    sync_both(&mut phone, &mut laptop);

    let before_name = phone.farm_name(&farm_id);
    let before_rows = phone.conflict_rows("farm", &farm_id);
    assert_eq!(before_rows.len(), 1, "a conflict is waiting");

    let live = live_head(&phone.heads_of("farm", &farm_id)).cloned();
    let tx = phone.conn.transaction().unwrap();
    let after = settle(&tx, "farm", &farm_id, live.as_ref()).unwrap();
    tx.commit().unwrap();

    assert_eq!(after.len(), 2, "still two heads, live one first");
    assert_eq!(after.first(), live.as_ref());
    assert_eq!(phone.farm_name(&farm_id), before_name);
    assert_eq!(phone.conflict_rows("farm", &farm_id), before_rows);
}

#[test]
fn a_device_that_only_listens_ends_up_identical() {
    // Every column, not just the one the test renamed: a whole-register merge
    // has to bring the whole register.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    let farm_id = shared_farm(&mut phone, &mut [&mut laptop]);
    rename(&mut phone, &farm_id, "La Vega");
    sync(&phone, &mut laptop);

    let row_of = |device: &Device| -> Vec<String> {
        device
            .conn
            .prepare("SELECT * FROM farm WHERE id = ?1")
            .unwrap()
            .query_map(params![&farm_id], |row| {
                (0..row.as_ref().column_count())
                    .map(|index| Ok(format!("{:?}", row.get_ref(index)?)))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
    };
    assert_eq!(row_of(&phone), row_of(&laptop));
}
