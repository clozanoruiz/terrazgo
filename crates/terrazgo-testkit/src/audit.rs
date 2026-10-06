// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading back the `record_change` row a write just logged: its images, and
//! where it sits in the sync log.
//!
//! Eight copies of this existed across five crates, in two shapes: core and
//! `module-phytosanitary` split the payload into `before`/`after`, `module-fertilisation`
//! returned the whole document. The three-value shape is the one kept, because
//! the payload contract *is* two row images (`docs/data-model.md`) and a helper
//! that hands back the envelope makes every caller re-index into it.

use rusqlite::Connection;
use serde_json::Value;

/// The latest `record_change` row for an entity: `(operation, before, after)`.
///
/// Panics if the entity has no logged change — which is the assertion most
/// callers want anyway: a write that logged nothing is the defect.
pub fn last_change(conn: &Connection, table: &str, id: &str) -> (String, Value, Value) {
    conn.query_row(
        "SELECT operation, payload FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map(|(op, payload)| {
        let mut doc: Value = serde_json::from_str(&payload).unwrap();
        (op, doc["before"].take(), doc["after"].take())
    })
    .unwrap()
}

/// Where an entity's latest change sits in the sync log: the change set it was
/// written in, and the register it was filed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamped {
    /// `(origin_device, origin_seq)` — the change-set key. Two rows written by
    /// one transaction share it.
    pub change_set: (String, i64),
    /// `(root_table, root_id)` — the register, which is the unit of merge.
    pub register: (String, String),
}

/// The latest `record_change` row for an entity, as its change set and its
/// register. For asserting that a transaction touching several registers
/// wrote them as ONE change set, each under its own register.
///
/// Panics if the entity has no logged change, like [`last_change`].
pub fn last_stamp(conn: &Connection, table: &str, id: &str) -> Stamped {
    conn.query_row(
        "SELECT origin_device, origin_seq, root_table, root_id FROM record_change
         WHERE entity_table = ?1 AND entity_id = ?2
         ORDER BY changed_at DESC, id DESC LIMIT 1",
        [table, id],
        |r| {
            Ok(Stamped {
                change_set: (r.get(0)?, r.get(1)?),
                register: (r.get(2)?, r.get(3)?),
            })
        },
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{FarmWithPlots, farm_with_plots};
    use terrazgo_core::models::UpdatePlot;
    use terrazgo_core::repository as repo;

    #[test]
    fn returns_the_newest_change_split_into_before_and_after() {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        let fx = farm_with_plots(&mut conn, FarmWithPlots::default());

        let (op, before, after) = last_change(&conn, "plot", &fx.plot_a);
        assert_eq!(op, "insert");
        assert!(before.is_null(), "an insert has no before image");
        assert_eq!(after["name"], "El Prado");

        repo::update_plot(
            &mut conn,
            &fx.plot_a,
            UpdatePlot {
                name: "El Prado Alto".into(),
                area_ha: Some(4.0),
                es: None,
            },
            None,
        )
        .unwrap();

        let (op, before, after) = last_change(&conn, "plot", &fx.plot_a);
        assert_eq!(op, "update");
        assert_eq!(before["name"], "El Prado");
        assert_eq!(after["name"], "El Prado Alto");
    }
}
