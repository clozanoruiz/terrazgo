// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The file two devices hand each other: what it contains, what it refuses,
//! and that a pair of them is a full sync.
//!
//! `merge_scenarios.rs` tests the merge against a function that copies log
//! rows; this tests the thing that will actually do the copying. Between them
//! the seam docs/sync.md draws — merge rules defined without reference to how
//! deltas travel — is covered from both sides.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use jiff::Timestamp;
use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::bundle;
use terrazgo_core::models::*;
use terrazgo_core::repository as repo;
use terrazgo_core::sync::VersionVector;

const A: &str = "0192f3a4-0000-7000-8000-00000000000a";
const B: &str = "0192f3a4-0000-7000-8000-00000000000b";
const C: &str = "0192f3a4-0000-7000-8000-00000000000c";

/// This device's clock, as `apply_bundle` takes it.
fn now_ms() -> i64 {
    Timestamp::now().as_millisecond()
}

struct Device {
    conn: Connection,
    id: String,
}

impl Device {
    fn new(id: &str) -> Self {
        let conn = terrazgo_core::open_in_memory().unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        Self {
            conn,
            id: id.to_string(),
        }
    }

    /// A bundle answering `seen` — what a reply carries, once this device has
    /// read the other's manifest.
    fn bundle_for(&self, seen: &VersionVector) -> Vec<u8> {
        let mut bytes = Vec::new();
        bundle::write_bundle(&self.conn, &self.id, seen, &mut bytes).unwrap();
        bytes
    }

    /// A first bundle, sent to a device whose state is unknown: everything.
    fn first_bundle(&self) -> Vec<u8> {
        self.bundle_for(&VersionVector::default())
    }

    fn apply(&mut self, bytes: &[u8]) -> Result<bundle::ImportSummary, CoreError> {
        let parsed = bundle::read_bundle(bytes)?;
        bundle::apply_bundle(&mut self.conn, &parsed, now_ms())
    }

    fn farm_name(&self, farm_id: &str) -> Option<String> {
        self.conn
            .query_row("SELECT name FROM farm WHERE id = ?1", [farm_id], |r| {
                r.get(0)
            })
            .ok()
    }

    fn log_rows(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))
            .unwrap()
    }
}

/// The two devices agree to sync: `sender`'s group is minted if it has none,
/// and `receiver` joins it.
///
/// This is the farmer confirming, once, the first time two devices meet — and
/// every scenario below that is not about pairing starts from it, because an
/// import refuses a bundle from a holding this device has not joined.
fn pair(sender: &Device, receiver: &Device) {
    let group = terrazgo_core::sync::ensure_sync_group(&sender.conn).unwrap();
    terrazgo_core::sync::join_sync_group(&receiver.conn, &group).unwrap();
}

fn rename(device: &mut Device, farm_id: &str, name: &str) {
    repo::update_farm(
        &mut device.conn,
        farm_id,
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
        },
        None,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// The container
// ---------------------------------------------------------------------------

#[test]
fn a_bundle_carries_the_log_and_reads_back_identical() {
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let parsed = bundle::read_bundle(&phone.first_bundle()[..]).unwrap();
    assert_eq!(parsed.manifest.device, A);
    assert_eq!(parsed.manifest.format, bundle::FORMAT_VERSION);
    assert_eq!(parsed.change_sets.len(), 2, "a create and an edit");
    assert_eq!(
        parsed
            .change_sets
            .iter()
            .map(|f| f.rows.len())
            .sum::<usize>() as i64,
        phone.log_rows()
    );
    // The stored text travels verbatim — the hash on the far side depends on it.
    let stored: String = phone
        .conn
        .query_row(
            "SELECT payload FROM record_change WHERE id = ?1",
            [&parsed.change_sets[0].rows[0].id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, parsed.change_sets[0].rows[0].payload);
}

#[test]
fn the_manifest_says_what_the_sender_holds() {
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let parsed = bundle::read_bundle(&phone.first_bundle()[..]).unwrap();
    assert_eq!(parsed.manifest.seen.get(A), 2, "two change sets so far");
    assert_eq!(parsed.manifest.seen.get(B), 0);
}

#[test]
fn a_truncated_bundle_is_refused() {
    let mut phone = Device::new(A);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let whole = phone.first_bundle();

    for cut in [whole.len() - 1, whole.len() / 2, 1] {
        let refused = bundle::read_bundle(&whole[..cut]);
        assert!(
            refused.is_err(),
            "a bundle cut at {cut} of {} bytes was accepted",
            whole.len()
        );
    }
}

#[test]
fn a_bundle_missing_its_last_line_is_refused() {
    // The failure the trailer exists for: every line parses, and the file
    // simply stops. File existence is not file readiness (docs/sync.md).
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let whole = phone.first_bundle();
    let mut lines = ungzip(&whole);
    lines.pop();
    let refused = bundle::read_bundle(&regzip(&lines)[..]);
    assert!(matches!(
        refused,
        Err(CoreError::Invalid("bundle_incomplete"))
    ));
}

#[test]
fn a_changed_byte_inside_a_frame_is_refused() {
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let mut lines = ungzip(&phone.first_bundle());
    // A frame's own bytes, altered so it still parses: the row count and the
    // line count both still match, and only the checksum can tell.
    let frame = String::from_utf8(lines[1].clone()).unwrap();
    assert!(frame.contains("Los Llanos"));
    lines[1] = frame.replace("Los Llanos", "Los Llanoz").into_bytes();

    let refused = bundle::read_bundle(&regzip(&lines)[..]);
    assert!(matches!(
        refused,
        Err(CoreError::Invalid("bundle_incomplete"))
    ));
}

#[test]
fn a_format_version_from_the_future_is_refused() {
    let mut phone = Device::new(A);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();

    let mut lines = ungzip(&phone.first_bundle());
    let manifest = String::from_utf8(lines[0].clone()).unwrap();
    lines[0] = manifest
        .replace(
            &format!("\"format\":{}", bundle::FORMAT_VERSION),
            &format!("\"format\":{}", bundle::FORMAT_VERSION + 1),
        )
        .into_bytes();

    assert!(matches!(
        bundle::read_bundle(&regzip(&lines)[..]),
        Err(CoreError::Invalid("bundle_format_unsupported"))
    ));
}

/// The uncompressed lines of a bundle, each still carrying its newline.
fn ungzip(bytes: &[u8]) -> Vec<Vec<u8>> {
    use std::io::Read;
    let mut plain = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut plain)
        .unwrap();
    plain
        .split_inclusive(|byte| *byte == b'\n')
        .map(<[u8]>::to_vec)
        .collect()
}

fn regzip(lines: &[Vec<u8>]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for line in lines {
        encoder.write_all(line).unwrap();
    }
    encoder.finish().unwrap()
}

// ---------------------------------------------------------------------------
// What goes in one
// ---------------------------------------------------------------------------

#[test]
fn a_peer_that_has_everything_is_sent_nothing() {
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let everything = bundle::seen_by(&phone.conn).unwrap();
    let parsed = bundle::read_bundle(&phone.bundle_for(&everything)[..]).unwrap();
    assert!(parsed.change_sets.is_empty());
    // And it is still a well-formed bundle, because a reply saying "I have
    // nothing for you" still has to say what it holds.
    assert_eq!(parsed.manifest.seen, everything);
}

#[test]
fn a_peer_gets_only_what_it_is_missing() {
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");
    rename(&mut phone, &farm.id, "El Soto");

    let mut seen = VersionVector::default();
    seen.observe(A, 2);
    let parsed = bundle::read_bundle(&phone.bundle_for(&seen)[..]).unwrap();
    assert_eq!(parsed.change_sets.len(), 1);
    assert_eq!(
        parsed.change_sets[0].seq, 3,
        "only the set past what it had"
    );
}

// ---------------------------------------------------------------------------
// Applying one
// ---------------------------------------------------------------------------

#[test]
fn two_file_copies_are_a_full_bidirectional_sync() {
    // The design's central transport claim: each side's bundle answers the
    // other's request and states its own, so one exchange each way is enough.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let phone_first = phone.first_bundle();
    laptop.apply(&phone_first).unwrap();

    rename(&mut phone, &farm.id, "La Vega");
    repo::insert_plot(&mut laptop.conn, new_plot(&farm.id, "El Prado"), None).unwrap();

    // Copy one: the phone sends what it has, and learns what the laptop holds.
    let outgoing = phone.first_bundle();
    let summary = laptop.apply(&outgoing).unwrap();
    assert_eq!(summary.peer, A);

    // Copy two: the laptop answers exactly that request.
    let reply = laptop.bundle_for(&bundle::seen_by(&phone.conn).unwrap());
    let back = phone.apply(&reply).unwrap();
    assert_eq!(back.peer, B);

    assert_eq!(phone.log_rows(), laptop.log_rows(), "one log on both");
    assert_eq!(phone.farm_name(&farm.id), laptop.farm_name(&farm.id));
    let plots: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM plot", [], |r| r.get(0))
        .unwrap();
    assert_eq!(plots, 1, "the laptop's plot reached the phone");
}

#[test]
fn an_import_names_the_books_its_new_change_sets_wrote_into() {
    // What the shell asks of an import before it says the catalogues may need
    // updating (docs/sync.md → What stays device-local): the books to read,
    // and only those this file changed — a whole-log file re-sends every book
    // the farm ever had, and reading all of them would cost a book each.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let plot = repo::insert_plot(&mut phone.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let old = repo::insert_season(
        &mut phone.conn,
        new_season(&farm.id, 2025, "2024/2025"),
        None,
    )
    .unwrap();
    let book = repo::insert_season(
        &mut phone.conn,
        new_season(&farm.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    let first = laptop.apply(&phone.first_bundle()).unwrap();
    let mut both = vec![old.id.clone(), book.id.clone()];
    both.sort();
    assert_eq!(first.books, both);

    repo::insert_crop(
        &mut phone.conn,
        NewCrop {
            plot_id: plot.id.clone(),
            season_id: book.id.clone(),
            species_name: "Trigo blando".into(),
            variety: None,
            production_system_code: None,
            area_ha: Some(2.0),
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
    let crop = laptop.apply(&phone.first_bundle()).unwrap();
    assert_eq!(
        crop.books,
        vec![book.id.clone()],
        "the old book was already held"
    );

    rename(&mut phone, &farm.id, "La Vega");
    let renamed = laptop.apply(&phone.first_bundle()).unwrap();
    assert!(renamed.books.is_empty(), "a farm is in no book");
}

#[test]
fn the_same_bundle_applied_twice_holds_what_it_already_has() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let outgoing = phone.first_bundle();
    let first = laptop.apply(&outgoing).unwrap();
    assert_eq!(first.change_sets_applied, 2);
    assert_eq!(first.change_sets_already_held, 0);

    let rows = laptop.log_rows();
    let again = laptop.apply(&outgoing).unwrap();
    assert_eq!(again.change_sets_applied, 0);
    assert_eq!(again.change_sets_already_held, 2);
    assert_eq!(laptop.log_rows(), rows, "nothing was written twice");
    assert_eq!(laptop.farm_name(&farm.id).as_deref(), Some("La Vega"));
}

#[test]
fn a_conflict_survives_the_round_trip_and_reaches_the_queue() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let shared = phone.first_bundle();
    laptop.apply(&shared).unwrap();

    rename(&mut phone, &farm.id, "La Vega");
    rename(&mut laptop, &farm.id, "El Soto");

    let outgoing = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    let summary = laptop.apply(&outgoing).unwrap();
    assert_eq!(
        summary.conflicts, 1,
        "two devices wrote without seeing each other"
    );

    let waiting: i64 = laptop
        .conn
        .query_row("SELECT COUNT(*) FROM sync_conflict", [], |r| r.get(0))
        .unwrap();
    assert_eq!(waiting, 1);
}

#[test]
fn two_versions_that_say_the_same_thing_are_not_counted_as_a_conflict() {
    // Two heads in the log, nothing in the queue — and the import's message
    // must agree with the queue, not with the log (docs/sync.md → Versions
    // that say the same thing are not listed).
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let shared = phone.first_bundle();
    laptop.apply(&shared).unwrap();

    rename(&mut phone, &farm.id, "La Vega");
    rename(&mut laptop, &farm.id, "La Vega");

    let outgoing = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    let summary = laptop.apply(&outgoing).unwrap();
    assert_eq!(
        terrazgo_core::merge::heads(&laptop.conn, "farm", &farm.id)
            .unwrap()
            .len(),
        2,
        "concurrent, though equal"
    );
    assert_eq!(summary.conflicts, 0, "nothing for a person to choose");
}

#[test]
fn a_bundle_from_a_different_schema_is_refused() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();

    // The pre-release hazard `schema_digest` exists for: the same
    // `user_version`, a different shape, because a migration file was edited
    // in place rather than added to.
    laptop
        .conn
        .execute_batch("CREATE TABLE later_addition (id TEXT PRIMARY KEY)")
        .unwrap();

    assert!(matches!(
        laptop.apply(&phone.first_bundle()),
        Err(CoreError::Invalid("bundle_schema_mismatch"))
    ));
}

#[test]
fn a_peer_whose_clock_is_far_ahead_is_refused_by_name() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let outgoing = phone.first_bundle();

    // The receiver's clock four hours behind the sender's is the same thing as
    // the sender's four hours ahead, and it is the arithmetic the refusal does.
    let parsed = bundle::read_bundle(&outgoing[..]).unwrap();
    let four_hours = 4 * 60 * 60 * 1000;
    let refused = bundle::apply_bundle(&mut laptop.conn, &parsed, now_ms() - four_hours);
    match refused {
        Err(CoreError::PeerClockAhead { peer, ahead_ms }) => {
            assert_eq!(peer, A, "the farmer has to be told which device");
            assert!(
                ahead_ms > 3 * 60 * 60 * 1000,
                "and by how much: got {ahead_ms} ms"
            );
        }
        other => panic!("expected a named clock refusal, got {other:?}"),
    }
    assert_eq!(laptop.log_rows(), 0, "and nothing of it was kept");
}

#[test]
fn an_hour_of_clock_skew_still_syncs() {
    // ε is two hours because a clock set to Spanish local time believing it is
    // UTC lands at exactly +1 or +2 (docs/sync.md → How far ahead a stamp may
    // be). That mistake must not stop a farmer syncing.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();

    let parsed = bundle::read_bundle(&phone.first_bundle()[..]).unwrap();
    let one_hour = 60 * 60 * 1000;
    bundle::apply_bundle(&mut laptop.conn, &parsed, now_ms() - one_hour)
        .expect("an hour ahead is absorbed, and heals by itself");
}

#[test]
fn two_devices_writing_under_one_identity_are_refused() {
    // The rewind path docs/sync.md records as unpreventable: the whole
    // app-data directory copied, so two live replicas share an id. The change
    // sets then disagree about what (A, 1) says.
    let mut phone = Device::new(A);
    let mut clone = Device::new(A);
    pair(&phone, &clone);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    repo::insert_farm(&mut clone.conn, new_farm("El Soto"), None).unwrap();

    let refused = clone.apply(&phone.first_bundle());
    assert!(
        matches!(refused, Err(CoreError::Invalid("device_identity_shared"))),
        "expected the hash to catch it, got {refused:?}"
    );
}

#[test]
fn a_change_set_that_matches_is_not_mistaken_for_a_clone() {
    // The other side of the same check, and the common case: a peer sends back
    // what we already gave it. Byte-identical, so it is simply already held.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let outgoing = phone.first_bundle();
    laptop.apply(&outgoing).unwrap();

    // The laptop now holds A's change set and sends it straight back.
    let echoed = laptop.first_bundle();
    let summary = phone.apply(&echoed).unwrap();
    assert_eq!(summary.change_sets_already_held, 1);
    assert_eq!(summary.change_sets_applied, 0);
}

// ---------------------------------------------------------------------------
// The season collision, which only the importer can see whole
// ---------------------------------------------------------------------------

/// A farm whose two campaign books derive ids in the order that makes the
/// apply hard: the book TAKING the name settles before the one releasing it.
///
/// The ids are `Uuid::new_v5` over farm and dates, so which order they fall in
/// is decided by the farm id — random per run. Picking a farm that produces the
/// awkward order makes the test about the retry rather than about luck.
fn farm_where_the_taker_sorts_first(device: &mut Device) -> (String, String, String) {
    for _ in 0..64 {
        let farm = repo::insert_farm(&mut device.conn, new_farm("Los Llanos"), None).unwrap();
        let releases = repo::season_id(&farm.id, "2025-09-01", "2026-08-31");
        let takes = repo::season_id(&farm.id, "2025-10-01", "2026-08-31");
        if takes < releases {
            return (farm.id, releases, takes);
        }
    }
    panic!("64 farms and never the awkward order — the id derivation changed");
}

#[test]
fn a_name_released_and_taken_in_one_bundle_applies_whatever_the_order() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let (farm_id, _releases, _takes) = farm_where_the_taker_sorts_first(&mut phone);

    // Both devices hold the first book.
    let shared = phone.first_bundle();
    laptop.apply(&shared).unwrap();
    let first = repo::insert_season(
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
    let catch_up = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    laptop.apply(&catch_up).unwrap();

    // The phone gives it a name of its own, freeing "2025/2026", then starts a
    // second book of the same campaign year — which takes that name.
    repo::update_season(
        &mut phone.conn,
        &first.id,
        UpdateSeason {
            starts_on: "2025-09-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("Secano".into()),
        },
        None,
    )
    .unwrap();
    let second = repo::insert_season(
        &mut phone.conn,
        NewSeason {
            farm_id,
            starts_on: "2025-10-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(second.label, "2025/2026");

    let outgoing = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    laptop
        .apply(&outgoing)
        .expect("the release and the taking arrived together, so it applies");

    let names: Vec<String> = laptop
        .conn
        .prepare("SELECT label FROM season WHERE deleted_at IS NULL ORDER BY label")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(names, ["2025/2026", "Secano"]);
}

#[test]
fn two_books_of_one_name_that_nothing_frees_are_still_refused() {
    // The retry must not turn a real collision into a silent acceptance: here
    // both books stay live and one of them has to be renamed by a person.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let shared = phone.first_bundle();
    laptop.apply(&shared).unwrap();

    let book = |device: &mut Device, starts_on: &str| {
        repo::insert_season(
            &mut device.conn,
            NewSeason {
                farm_id: farm.id.clone(),
                starts_on: starts_on.into(),
                ends_on: "2026-08-31".into(),
                custom_label: None,
            },
            None,
        )
        .unwrap()
    };
    let theirs = book(&mut phone, "2025-09-01");
    let ours = book(&mut laptop, "2025-10-01");
    assert_eq!(theirs.label, ours.label);

    let outgoing = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    assert!(matches!(
        laptop.apply(&outgoing),
        Err(CoreError::SeasonLabelCollision { label, .. }) if label == ours.label
    ));
}

#[test]
fn a_refused_bundle_leaves_nothing_behind() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let shared = phone.first_bundle();
    laptop.apply(&shared).unwrap();

    // A bundle carrying a real edit AND a collision: the edit must not survive
    // the refusal, or a retry after the farmer renames would start from a
    // place neither device chose.
    rename(&mut phone, &farm.id, "La Vega");
    for (device, starts_on) in [(&mut phone, "2025-09-01"), (&mut laptop, "2025-10-01")] {
        repo::insert_season(
            &mut device.conn,
            NewSeason {
                farm_id: farm.id.clone(),
                starts_on: starts_on.into(),
                ends_on: "2026-08-31".into(),
                custom_label: None,
            },
            None,
        )
        .unwrap();
    }
    // Counted after the laptop's own writes: what must not move is what the
    // refused delivery would have added.
    let before = laptop.log_rows();

    let outgoing = phone.bundle_for(&bundle::seen_by(&laptop.conn).unwrap());
    assert!(laptop.apply(&outgoing).is_err());
    assert_eq!(laptop.log_rows(), before, "the log is untouched");
    assert_eq!(
        laptop.farm_name(&farm.id).as_deref(),
        Some("Los Llanos"),
        "and so are the tables"
    );
}

// ---------------------------------------------------------------------------
// A file made for another device
// ---------------------------------------------------------------------------
//
// docs/sync.md → A trimmed reply is safe only for the device it answers. A
// reply leaves out what the device it answers already holds, so imported
// anywhere else it can leave out what THAT device does not. Every scenario
// here is the same three devices — a laptop and two phones, P and Q — and the
// same misroute: a reply trimmed for Q, imported by P.

/// A laptop and two phones, all in the laptop's group, all holding the
/// laptop's farm.
fn laptop_and_two_phones() -> (Device, Device, Device, String) {
    let mut laptop = Device::new(A);
    let mut p = Device::new(B);
    let mut q = Device::new(C);
    pair(&laptop, &p);
    pair(&laptop, &q);
    let farm = repo::insert_farm(&mut laptop.conn, new_farm("Los Llanos"), None).unwrap();
    let whole = laptop.first_bundle();
    p.apply(&whole).unwrap();
    q.apply(&whole).unwrap();
    (laptop, p, q, farm.id)
}

/// A reply to `peer`, as the laptop writes one once it has read that peer's
/// manifest.
fn reply_to(sender: &Device, peer: &Device) -> Vec<u8> {
    sender.bundle_for(&bundle::seen_by(&peer.conn).unwrap())
}

/// The refusal a file made for another device meets — asserted by name,
/// because the other refusals it could fall into instead are exactly what this
/// check exists to pre-empt.
fn assert_skips_changes(outcome: Result<bundle::ImportSummary, CoreError>) {
    assert!(
        matches!(outcome, Err(CoreError::Invalid("bundle_skips_changes"))),
        "expected the file to be refused as leaving changes out, got {outcome:?}"
    );
}

fn plots_of(device: &Device) -> Vec<String> {
    device
        .conn
        .prepare("SELECT id FROM plot ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn a_file_that_would_leave_a_hole_in_a_devices_numbers_is_refused() {
    // The laptop's set 2 reached Q and not P, and its set 3 reached nobody.
    // The reply trimmed for Q leaves 2 out, so P would hold 1 and 3 — and its
    // manifest, a MAX per device, would say "up to 3" for ever after, so no
    // reply to it would carry 2 again.
    let (mut laptop, mut p, mut q, farm_id) = laptop_and_two_phones();
    rename(&mut laptop, &farm_id, "La Vega");
    q.apply(&reply_to(&laptop, &q)).unwrap();
    repo::insert_plot(&mut laptop.conn, new_plot(&farm_id, "El Prado"), None).unwrap();

    let before = p.log_rows();
    assert_skips_changes(p.apply(&reply_to(&laptop, &q)));
    assert_eq!(p.log_rows(), before, "and nothing of it was kept");
}

#[test]
fn a_change_without_the_one_it_was_built_on_is_refused() {
    // The permanent one. Q adds a plot to a sowing; the laptop corrects the
    // record on top of that, logging only the row that changed. Taken without
    // Q's set, P would show the correction on one plot where the laptop shows
    // two — and when Q's set arrived later, the live head would already be a
    // set built on it, so nothing would move: one log, two different books.
    let (mut laptop, mut p, mut q, farm_id) = laptop_and_two_phones();
    let season = repo::insert_season(
        &mut laptop.conn,
        new_season(&farm_id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    let first = repo::insert_plot(&mut laptop.conn, new_plot(&farm_id, "El Prado"), None).unwrap();
    let second = repo::insert_plot(&mut laptop.conn, new_plot(&farm_id, "La Loma"), None).unwrap();
    let record = repo::insert_sowing_record(
        &mut laptop.conn,
        NewSowingRecord {
            season_id: season.id.clone(),
            farm_id: farm_id.clone(),
            kind_code: "sowing".into(),
            sown_on: "2026-04-10".into(),
            sowing_end_date: None,
            flooded_on: None,
            seed_quantity_kg: Some(180.0),
            notes: None,
            plots: vec![NewSowingPlot {
                plot_id: first.id.clone(),
                crop_id: None,
            }],
        },
        None,
    )
    .unwrap()
    .record;
    p.apply(&reply_to(&laptop, &p)).unwrap();
    q.apply(&reply_to(&laptop, &q)).unwrap();

    repo::update_sowing_record(
        &mut q.conn,
        &record.id,
        sowing_state(None, &[&first.id, &second.id]),
        None,
    )
    .unwrap();
    laptop.apply(&q.first_bundle()).unwrap();
    repo::update_sowing_record(
        &mut laptop.conn,
        &record.id,
        sowing_state(Some("corregido"), &[&first.id, &second.id]),
        None,
    )
    .unwrap();

    let misrouted = reply_to(&laptop, &q);
    assert_skips_changes(p.apply(&misrouted));

    // And the way forward the refusal points at does reach the laptop's book:
    // once P holds what the file left out, the same file applies.
    p.apply(&q.first_bundle()).unwrap();
    p.apply(&misrouted).unwrap();
    let sown = |device: &Device| -> Vec<String> {
        device
            .conn
            .prepare("SELECT plot_id FROM sowing_plot ORDER BY plot_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(sown(&p), sown(&laptop), "one log, one book");
    assert_eq!(sown(&p).len(), 2);
}

#[test]
fn a_change_built_on_another_registers_row_is_refused_by_name() {
    // A plot is its own register and names its farm, so the vector on the
    // laptop's plot counts the laptop alone: nothing in it says the farm is
    // Q's. Unrefused, the import fails at commit on SQLite's own foreign-key
    // message, which is nothing a farmer can act on.
    let (mut laptop, mut p, mut q, _) = laptop_and_two_phones();
    let second_farm = repo::insert_farm(&mut q.conn, new_farm("La Vega"), None).unwrap();
    laptop.apply(&q.first_bundle()).unwrap();
    repo::insert_plot(
        &mut laptop.conn,
        new_plot(&second_farm.id, "El Prado"),
        None,
    )
    .unwrap();

    let misrouted = reply_to(&laptop, &q);
    // The check the import screen runs before asking anything gives the same
    // answer as the import, having written nothing.
    let parsed = bundle::read_bundle(&misrouted[..]).unwrap();
    assert!(matches!(
        bundle::refuse_if_incomplete(&p.conn, &parsed),
        Err(CoreError::Invalid("bundle_skips_changes"))
    ));
    assert_skips_changes(p.apply(&misrouted));
    assert!(plots_of(&p).is_empty());
}

#[test]
fn a_name_freed_on_a_device_this_one_has_not_heard_from_is_not_called_a_collision() {
    // Q renames a book, and the laptop opens a new one under the name that
    // freed. P, still holding the old name, would refuse the file as two books
    // sharing a name — which on the sender they do not — and the screen would
    // ask the farmer to merge or rename books that do not collide.
    let (mut laptop, mut p, mut q, farm_id) = laptop_and_two_phones();
    let book =
        repo::insert_season(&mut laptop.conn, new_season(&farm_id, 2026, "Viña"), None).unwrap();
    p.apply(&reply_to(&laptop, &p)).unwrap();
    q.apply(&reply_to(&laptop, &q)).unwrap();

    repo::update_season(
        &mut q.conn,
        &book.id,
        UpdateSeason {
            starts_on: book.starts_on.clone(),
            ends_on: book.ends_on.clone(),
            custom_label: Some("Viña vieja".into()),
        },
        None,
    )
    .unwrap();
    laptop.apply(&q.first_bundle()).unwrap();
    repo::insert_season(&mut laptop.conn, new_season(&farm_id, 2027, "Viña"), None).unwrap();

    assert_skips_changes(p.apply(&reply_to(&laptop, &q)));
}

#[test]
fn a_reply_made_for_another_device_applies_where_nothing_is_missing() {
    // What the check does NOT refuse: P already holds everything Q held, so
    // the file leaves out nothing P lacks — whoever it was trimmed for.
    let (mut laptop, mut p, q, farm_id) = laptop_and_two_phones();
    rename(&mut laptop, &farm_id, "La Vega");

    let summary = p.apply(&reply_to(&laptop, &q)).unwrap();
    assert_eq!(summary.change_sets_applied, 1);
    assert_eq!(p.farm_name(&farm_id).as_deref(), Some("La Vega"));
}

#[test]
fn one_reply_to_two_phones_imported_in_one_session_applies_at_both() {
    // The consolidation laptop: it imports both phones, exports once, and the
    // one file goes to both. Trimmed for the phone read last it would leave
    // out that phone's work at the other; trimmed to what BOTH had seen it is
    // safe at each, by construction.
    let (mut laptop, mut p, mut q, farm_id) = laptop_and_two_phones();
    repo::insert_plot(&mut p.conn, new_plot(&farm_id, "El Prado"), None).unwrap();
    repo::insert_plot(&mut q.conn, new_plot(&farm_id, "La Loma"), None).unwrap();
    let from_p = laptop.apply(&p.first_bundle()).unwrap();
    let from_q = laptop.apply(&q.first_bundle()).unwrap();
    rename(&mut laptop, &farm_id, "La Vega");

    // The contrast: answering only the last import leaves Q's plot out at P.
    assert_skips_changes(p.apply(&laptop.bundle_for(&from_q.peer_seen)));

    let answering = VersionVector::common_ancestor_of(&[from_p.peer_seen, from_q.peer_seen]);
    let reply = laptop.bundle_for(&answering);
    p.apply(&reply).unwrap();
    q.apply(&reply).unwrap();

    for phone in [&p, &q] {
        assert_eq!(phone.log_rows(), laptop.log_rows(), "one log on all three");
        assert_eq!(plots_of(phone), plots_of(&laptop));
        assert_eq!(phone.farm_name(&farm_id).as_deref(), Some("La Vega"));
    }
}

#[test]
fn a_frame_past_its_manifests_own_seen_is_unreadable() {
    // A manifest states what its sender holds, and the frames are drawn from
    // that. A file carrying a set its own manifest says the sender does not
    // hold was not written by this app — and read as written, it would make
    // the completeness check reason from a claim the file itself contradicts.
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let mut lines = ungzip(&phone.first_bundle());
    let manifest = String::from_utf8(lines[0].clone()).unwrap();
    let claimed = format!("\"{A}\":2");
    assert!(manifest.contains(&claimed), "{manifest}");
    lines[0] = manifest
        .replace(&claimed, &format!("\"{A}\":1"))
        .into_bytes();

    assert!(matches!(
        bundle::read_bundle(&regzip(&lines)[..]),
        Err(CoreError::Invalid("bundle_unreadable"))
    ));
}

#[test]
fn a_file_says_what_it_starts_from_and_what_its_sender_knows() {
    // What it starts from is what the receiver checks it holds; what its
    // sender knows of other devices is what lets a phone learn, through the
    // laptop, what another phone holds (docs/sync.md → What each device knows
    // of the others).
    let (mut laptop, mut p, q, farm_id) = laptop_and_two_phones();
    repo::insert_plot(&mut p.conn, new_plot(&farm_id, "El Prado"), None).unwrap();
    let from_p = laptop.apply(&p.first_bundle()).unwrap();

    let to_q = bundle::read_bundle(&reply_to(&laptop, &q)[..]).unwrap();
    assert_eq!(to_q.manifest.since, bundle::seen_by(&q.conn).unwrap());
    assert_eq!(
        to_q.manifest.known.get(B),
        Some(&from_p.peer_seen),
        "the laptop passes on what P said it holds"
    );
    let whole = bundle::read_bundle(&laptop.first_bundle()[..]).unwrap();
    assert_eq!(whole.manifest.since, VersionVector::default());
}

#[test]
fn a_phone_learns_what_another_holds_through_the_laptop() {
    let (mut laptop, mut p, mut q, farm_id) = laptop_and_two_phones();
    repo::insert_plot(&mut p.conn, new_plot(&farm_id, "El Prado"), None).unwrap();
    let from_p = laptop.apply(&p.first_bundle()).unwrap();
    assert!(
        !terrazgo_core::sync::known(&q.conn).unwrap().contains_key(B),
        "Q has never heard of P"
    );
    q.apply(&reply_to(&laptop, &q)).unwrap();
    let known = terrazgo_core::sync::known(&q.conn).unwrap();
    assert_eq!(known.get(B), Some(&from_p.peer_seen));
    assert_eq!(
        known.get(A),
        Some(&bundle::seen_by(&laptop.conn).unwrap()),
        "and the laptop's own, heard directly"
    );
}

#[test]
fn a_frame_at_or_below_what_its_file_starts_from_is_unreadable() {
    // A file carries what its sender holds past what it starts from, and
    // nothing at or below it; one that does was not written by this app.
    let mut phone = Device::new(A);
    let farm = repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    rename(&mut phone, &farm.id, "La Vega");

    let mut lines = ungzip(&phone.first_bundle());
    let manifest = String::from_utf8(lines[0].clone()).unwrap();
    assert!(manifest.contains("\"since\":{}"), "{manifest}");
    lines[0] = manifest
        .replace("\"since\":{}", &format!("\"since\":{{\"{A}\":1}}"))
        .into_bytes();

    assert!(matches!(
        bundle::read_bundle(&regzip(&lines)[..]),
        Err(CoreError::Invalid("bundle_unreadable"))
    ));
}

#[test]
fn a_file_of_the_format_before_the_purge_is_refused_for_its_version() {
    // Format 1 had no `since` and no `known`. It reads far enough to be told
    // apart, and is refused for what it is rather than as unreadable.
    let mut phone = Device::new(A);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let mut lines = ungzip(&phone.first_bundle());
    let mut manifest: serde_json::Value = serde_json::from_slice(&lines[0]).unwrap();
    let object = manifest.as_object_mut().unwrap();
    object.remove("since");
    object.remove("known");
    object.insert("format".into(), serde_json::json!(1));
    lines[0] = format!("{manifest}\n").into_bytes();
    assert!(matches!(
        bundle::read_bundle(&regzip(&lines)[..]),
        Err(CoreError::Invalid("bundle_format_unsupported"))
    ));
}

// ---------------------------------------------------------------------------
// The hash itself
// ---------------------------------------------------------------------------

#[test]
fn the_change_set_hash_does_not_depend_on_row_order() {
    // Two replicas do hold a set's rows in one order, since the ids are
    // UUIDv7s carried verbatim — but the digest must not depend on a reader's
    // `ORDER BY`, or two correct implementations could disagree.
    let rows = [logged_row("b", "El Soto"), logged_row("a", "La Vega")];
    let forwards = bundle::change_set_hash(&rows);
    let mut backwards = rows.clone();
    backwards.reverse();
    assert_eq!(forwards, bundle::change_set_hash(&backwards));

    // And it still answers the question it is for.
    let different = [logged_row("b", "El Soto"), logged_row("a", "Las Eras")];
    assert_ne!(forwards, bundle::change_set_hash(&different));
}

/// One logged row, built by hand: the hash is a function of these fields and
/// nothing else, so it is tested on them rather than through a repository.
fn logged_row(id: &str, name: &str) -> bundle::RowChange {
    bundle::RowChange {
        id: id.into(),
        entity_table: "farm".into(),
        entity_id: "f1".into(),
        season_id: None,
        operation: "update".into(),
        root_table: "farm".into(),
        root_id: "f1".into(),
        version_vector: r#"{"a":1}"#.into(),
        payload: format!(r#"{{"before":null,"after":{{"name":"{name}"}}}}"#),
    }
}

#[test]
fn the_schema_digest_changes_with_the_schema_and_not_with_the_data() {
    let mut one = Device::new(A);
    let two = Device::new(B);
    assert_eq!(
        bundle::schema_digest(&one.conn).unwrap(),
        bundle::schema_digest(&two.conn).unwrap(),
        "two devices at one build have one schema"
    );

    repo::insert_farm(&mut one.conn, new_farm("Los Llanos"), None).unwrap();
    assert_eq!(
        bundle::schema_digest(&one.conn).unwrap(),
        bundle::schema_digest(&two.conn).unwrap(),
        "and rows are not schema"
    );

    one.conn
        .execute_batch("CREATE TABLE later_addition (id TEXT PRIMARY KEY)")
        .unwrap();
    assert_ne!(
        bundle::schema_digest(&one.conn).unwrap(),
        bundle::schema_digest(&two.conn).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Whose book this is
// ---------------------------------------------------------------------------

#[test]
fn a_group_is_minted_on_the_first_export_and_never_again() {
    // Not at first launch, which is the whole point: two devices set up
    // separately would each hold a group before anything had decided what
    // joining means (docs/sync.md → Device identity).
    let mut phone = Device::new(A);
    assert_eq!(
        terrazgo_core::sync::sync_group(&phone.conn).unwrap(),
        None,
        "a device that has never exported belongs to no holding"
    );
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();

    let first = bundle::read_bundle(&phone.first_bundle()[..])
        .unwrap()
        .manifest
        .group;
    assert_eq!(
        terrazgo_core::sync::sync_group(&phone.conn)
            .unwrap()
            .as_deref(),
        Some(first.as_str())
    );

    let second = bundle::read_bundle(&phone.first_bundle()[..])
        .unwrap()
        .manifest
        .group;
    assert_eq!(first, second, "exporting again does not re-mint it");
}

#[test]
fn a_device_that_has_joined_nothing_refuses_and_says_which() {
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let outgoing = phone.first_bundle();

    assert!(matches!(
        laptop.apply(&outgoing),
        Err(CoreError::Invalid("sync_not_paired"))
    ));
    assert_eq!(laptop.log_rows(), 0, "and nothing of it was kept");

    // Joining is the whole of what the farmer has to do, and the same file
    // then applies — nothing is re-exported or re-copied.
    let group = bundle::read_bundle(&outgoing[..]).unwrap().manifest.group;
    terrazgo_core::sync::join_sync_group(&laptop.conn, &group).unwrap();
    laptop
        .apply(&outgoing)
        .expect("the same bundle, now paired");
    assert_eq!(laptop.log_rows(), 1);
}

#[test]
fn a_bundle_from_another_holding_is_refused() {
    // The neighbour's file on the same memory stick. Both devices have a
    // group; they are simply not the same one.
    let mut phone = Device::new(A);
    let mut neighbour = Device::new(B);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    repo::insert_farm(&mut neighbour.conn, new_farm("El Soto"), None).unwrap();
    let theirs = neighbour.first_bundle();
    let _ = phone.first_bundle(); // mints the phone's own group

    assert!(matches!(
        phone.apply(&theirs),
        Err(CoreError::Invalid("sync_group_mismatch"))
    ));
    assert_eq!(
        phone.farm_name(&farm_id(&phone)).as_deref(),
        Some("Los Llanos"),
        "the neighbour's book did not land in ours"
    );
}

#[test]
fn two_devices_that_each_minted_a_group_can_still_be_joined() {
    // The farmer's own two devices, each of which exported before ever
    // importing. Recoverable by the same act, which is why the refusal above
    // is told apart from `sync_not_paired` rather than being the same code:
    // this one means leaving a group, not joining a first.
    let mut phone = Device::new(A);
    let mut laptop = Device::new(B);
    repo::insert_farm(&mut phone.conn, new_farm("Los Llanos"), None).unwrap();
    let outgoing = phone.first_bundle();
    let _ = laptop.first_bundle();
    assert!(matches!(
        laptop.apply(&outgoing),
        Err(CoreError::Invalid("sync_group_mismatch"))
    ));

    let group = bundle::read_bundle(&outgoing[..]).unwrap().manifest.group;
    terrazgo_core::sync::join_sync_group(&laptop.conn, &group).unwrap();
    laptop.apply(&outgoing).expect("joined, so it applies");
    assert_eq!(
        terrazgo_core::sync::sync_group(&laptop.conn).unwrap(),
        terrazgo_core::sync::sync_group(&phone.conn).unwrap()
    );
}

#[test]
fn a_device_belongs_to_at_most_one_group_and_the_schema_says_so() {
    // The CHECK, not a rule in Rust: nothing — no bug, no bad import, no hand
    // -written SQL — can leave this database in two holdings at once.
    let phone = Device::new(A);
    terrazgo_core::sync::ensure_sync_group(&phone.conn).unwrap();
    let second = phone.conn.execute(
        "INSERT INTO sync_group (row_id, group_id, joined_at)
         VALUES (2, '0192f3a4-0000-7000-8000-0000000000ff', '2026-09-22T10:00:00Z')",
        [],
    );
    assert!(second.is_err(), "a second group row must not be possible");

    // And joining replaces rather than adding.
    terrazgo_core::sync::join_sync_group(&phone.conn, "0192f3a4-0000-7000-8000-0000000000ff")
        .unwrap();
    let rows: i64 = phone
        .conn
        .query_row("SELECT COUNT(*) FROM sync_group", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(
        terrazgo_core::sync::sync_group(&phone.conn)
            .unwrap()
            .as_deref(),
        Some("0192f3a4-0000-7000-8000-0000000000ff")
    );
}

#[test]
fn a_group_id_off_a_bundle_has_to_be_a_canonical_uuid() {
    // It arrives from another device, like the device id off settings.json,
    // and is then written into this database — so it is checked in the one
    // spelling every id here uses.
    let phone = Device::new(A);
    for spelling in [
        "0192F3A4-0000-7000-8000-0000000000FF",
        "{0192f3a4-0000-7000-8000-0000000000ff}",
        "the neighbour's farm",
        "",
    ] {
        assert!(
            matches!(
                terrazgo_core::sync::join_sync_group(&phone.conn, spelling),
                Err(CoreError::Invalid("sync_group_invalid"))
            ),
            "{spelling:?} was accepted as a group"
        );
    }
}

#[test]
fn a_restored_backup_is_still_the_same_holding() {
    // Why the group is in the DATABASE and not beside `device_id` in
    // settings.json: a restored device is a different REPLICA — it is re-minted
    // a device id precisely so its change sets cannot collide — but it is the
    // same HOLDING, and having to re-join every peer after a restore would be
    // a cost with nothing behind it.
    let source = common::TempFile::reserve("group-src.db");
    let dest = common::TempFile::reserve("group-dest.db");
    let mut conn = Connection::open(source.path()).unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    terrazgo_core::db::migrations()
        .to_latest(&mut conn)
        .unwrap();
    let group = terrazgo_core::sync::ensure_sync_group(&conn).unwrap();

    terrazgo_core::backup::export_backup(&conn, dest.path()).unwrap();
    let restored = Connection::open(dest.path()).unwrap();
    assert_eq!(
        terrazgo_core::sync::sync_group(&restored)
            .unwrap()
            .as_deref(),
        Some(group.as_str())
    );
}

/// The one farm a device holds — enough for a test that only needs to name it.
fn farm_id(device: &Device) -> String {
    device
        .conn
        .query_row("SELECT id FROM farm LIMIT 1", [], |r| r.get(0))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Naming the sender
// ---------------------------------------------------------------------------

/// `device` registered as its own `sync_peer`, as opening the app does, and
/// given `label` there.
fn registered(device: &mut Device, label: Option<&str>) {
    repo::register_this_device(&mut device.conn, None).unwrap();
    if let Some(label) = label {
        let id = device.id.clone();
        repo::rename_sync_peer(&mut device.conn, &id, Some(label), None).unwrap();
    }
}

fn sender_of(receiver: &Device, bytes: &[u8]) -> Option<String> {
    let parsed = bundle::read_bundle(bytes).unwrap();
    bundle::sender_label(&receiver.conn, &parsed).unwrap()
}

#[test]
fn a_bundle_names_its_sender_by_the_name_it_was_given_there() {
    // Read before anything is applied: the screen asks whether to import a
    // file from "Móvil de Juan", not from a device id.
    let mut phone = Device::new(A);
    registered(&mut phone, Some("Móvil"));
    let id = phone.id.clone();
    repo::rename_sync_peer(&mut phone.conn, &id, Some("Móvil de Juan"), None).unwrap();
    let laptop = Device::new(B);
    assert_eq!(
        sender_of(&laptop, &phone.first_bundle()).as_deref(),
        Some("Móvil de Juan"),
        "the latest name the bundle carries"
    );
}

#[test]
fn the_name_this_device_gave_the_sender_comes_first() {
    let mut phone = Device::new(A);
    registered(&mut phone, Some("Móvil"));
    let mut laptop = Device::new(B);
    pair(&phone, &laptop);
    laptop.apply(&phone.first_bundle()).unwrap();
    repo::rename_sync_peer(&mut laptop.conn, A, Some("Móvil de Juan"), None).unwrap();
    // The phone has not heard of the new name yet, and its bundle still says
    // "Móvil".
    assert_eq!(
        sender_of(&laptop, &phone.first_bundle()).as_deref(),
        Some("Móvil de Juan")
    );
}

#[test]
fn a_sender_nobody_has_named_has_no_name() {
    let mut phone = Device::new(A);
    registered(&mut phone, None);
    let laptop = Device::new(B);
    assert_eq!(sender_of(&laptop, &phone.first_bundle()), None);
}
