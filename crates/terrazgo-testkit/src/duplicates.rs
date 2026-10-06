// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The check a save runs, held to the list it serves (docs/sync.md →
//! Duplicate suspects).
//!
//! After a form saves, the pairs its record is in are found by a fetch of their
//! own — the records near that one, never its whole book
//! (`terrazgo_core::duplicates::record_candidates_sql`). The list a book's page
//! shows is found by another. Two fetches for one question can drift, so every
//! crate that owns a register runs this over its fixtures: for every live
//! record, the two must name the same pairs.
//!
//! Here rather than in each crate's tests because it needs core alone, and
//! every register-owning crate needs it.

use std::collections::BTreeSet;

use rusqlite::Connection;
use terrazgo_core::duplicates::{DuplicatePolicy, Scope};
use terrazgo_core::merge::RowCaption;
use terrazgo_core::repository as repo;

/// For every live record of every register `policies` compare: the pairs the
/// check after a save finds it in are exactly the pairs its book's page lists
/// it in. Returns how many records were checked, so a caller can assert its
/// fixture was not empty — a check over nothing passes.
pub fn assert_saved_pairs_match_the_book(
    conn: &Connection,
    policies: &[DuplicatePolicy],
    captions: &[RowCaption],
) -> usize {
    let mut checked = 0;
    for policy in policies.iter().filter(|policy| !policy.rules().is_empty()) {
        let records: Vec<(String, String)> = conn
            .prepare(&format!(
                "SELECT id, season_id FROM {} WHERE deleted_at IS NULL ORDER BY id",
                policy.table
            ))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for (id, season_id) in records {
            let saved =
                repo::list_saved_duplicates(conn, policies, captions, policy.table, &id).unwrap();
            let book = repo::list_duplicates(
                conn,
                policies,
                captions,
                Scope::Book {
                    season_id: &season_id,
                },
            )
            .unwrap();
            let pairs_of = |suspects: &[repo::SuspectedDuplicate]| -> BTreeSet<(String, String)> {
                suspects
                    .iter()
                    .filter(|pair| pair.register == policy.table)
                    .map(|pair| (pair.records[0].id.clone(), pair.records[1].id.clone()))
                    .filter(|(first, second)| *first == id || *second == id)
                    .collect()
            };
            assert_eq!(
                pairs_of(&saved.suspects),
                pairs_of(&book.suspects),
                "`{}` record {id}: the check after a save and its book's page name different pairs",
                policy.table
            );
            checked += 1;
        }
    }
    checked
}
