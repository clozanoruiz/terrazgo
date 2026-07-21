// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integer export aliases (docs/siex-export.md → gap 1). The contract under
//! test: an alias is minted once per (target, entity, split_key), is stable
//! across calls, monotonic per target, and never reuses or renumbers — SIEX
//! keys edits and deletions on it, so instability would corrupt the
//! authority's view of the cuaderno.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::open_in_memory;
use module_cue::repository::{ensure_export_alias, find_export_alias};

#[test]
fn aliases_are_stable_and_monotonic_per_target() {
    let mut conn = open_in_memory().unwrap();

    let a = ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "", None).unwrap();
    let b = ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-b", "", None).unwrap();
    assert_eq!(a, 1, "first alias starts the sequence at 1");
    assert_eq!(b, 2, "aliases are minted monotonically");

    // Re-exporting must reuse the alias, never mint a new one.
    let again =
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "", None).unwrap();
    assert_eq!(again, a);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM export_alias", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn split_keys_discriminate_within_one_record() {
    // A multi-crop treatment splits into one TratamFito per crop (3.11.4
    // descriptor rule), each with its own IdAjena — the alias keys on
    // (record, split), not the record alone.
    let mut conn = open_in_memory().unwrap();

    let wheat =
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "1", None).unwrap();
    let barley =
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "5", None).unwrap();
    assert_ne!(wheat, barley);
    assert_eq!(
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "1", None).unwrap(),
        wheat
    );
}

#[test]
fn find_never_mints() {
    // Deletion entries must reference only what was actually exported: the
    // lookup answers None for unknown tuples and leaves no row behind.
    let mut conn = open_in_memory().unwrap();

    assert_eq!(
        find_export_alias(&conn, "siex", "treatment_record", "uuid-a", "").unwrap(),
        None
    );
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM export_alias", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);

    let minted =
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "", None).unwrap();
    assert_eq!(
        find_export_alias(&conn, "siex", "treatment_record", "uuid-a", "").unwrap(),
        Some(minted)
    );
    // The same entity under a different split key is a different tuple.
    assert_eq!(
        find_export_alias(&conn, "siex", "treatment_record", "uuid-a", "x").unwrap(),
        None
    );
}

#[test]
fn targets_number_independently() {
    let mut conn = open_in_memory().unwrap();

    let siex =
        ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "", None).unwrap();
    let other =
        ensure_export_alias(&mut conn, "other", "treatment_record", "uuid-a", "", None).unwrap();
    assert_eq!(siex, 1);
    assert_eq!(other, 1, "each export regime has its own sequence");
}

#[test]
fn alias_inserts_are_audit_logged_with_full_row_images() {
    // Aliases are synced user data: not re-derivable, must survive backups and
    // roam at sync — so the insert lands in record_change like any other.
    let mut conn = open_in_memory().unwrap();
    ensure_export_alias(&mut conn, "siex", "treatment_record", "uuid-a", "", None).unwrap();

    let payload: String = conn
        .query_row(
            "SELECT payload FROM record_change WHERE entity_table = 'export_alias'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let doc: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(doc["after"]["target"], "siex");
    assert_eq!(doc["after"]["entity_id"], "uuid-a");
    assert_eq!(doc["after"]["alias"], 1);
    assert!(doc["after"].get("created_at").is_some());
}

#[test]
fn minting_an_alias_stamps_the_actor() {
    // First-time exports write (mint aliases), so they are attributed like
    // any other write; re-exports reuse the alias and log nothing new.
    let mut conn = open_in_memory().unwrap();
    ensure_export_alias(
        &mut conn,
        "siex",
        "treatment_record",
        "uuid-a",
        "",
        Some("profile-ana"),
    )
    .unwrap();
    let (count, actor): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(actor) FROM record_change WHERE entity_table = 'export_alias'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(actor.as_deref(), Some("profile-ana"));
}
