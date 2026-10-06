// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Simulated devices, and the transport between them.
//!
//! **What is simulated is only the transport.** Every device here is a real
//! database at the core schema with its own device id, and every write goes
//! through the real repositories; [`sync`] copies log rows from one to another
//! the way a bundle does. That split is the design's own — the merge rules are
//! defined without reference to how deltas travel (docs/sync.md → "Two layers,
//! and only one of them has policy") — so a test that exercises them against a
//! function copying rows is testing at the seam rather than around it.
//!
//! Shared by the two files that need more than one device: the merge engine's
//! scenario matrix, and the review a person reads before choosing.

use std::collections::BTreeSet;

use rusqlite::Connection;
use terrazgo_core::CoreError;
use terrazgo_core::merge::{Head, heads, live_head, settle};
use terrazgo_core::models::{NewSowingPlot, NewSowingRecord, UpdateFarm, UpdateSowingRecord};
use terrazgo_core::repository as repo;

use super::{new_farm, new_plot, new_season};

/// One simulated device: its own database, its own id, its own clock.
pub struct Device {
    pub conn: Connection,
    pub id: String,
}

impl Device {
    pub fn new(id: &str) -> Self {
        let conn = terrazgo_core::open_in_memory().unwrap();
        terrazgo_core::sync::install_device(&conn, id).unwrap();
        Self {
            conn,
            id: id.to_string(),
        }
    }

    pub fn farm_name(&self, farm_id: &str) -> Option<String> {
        self.conn
            .query_row("SELECT name FROM farm WHERE id = ?1", [farm_id], |r| {
                r.get(0)
            })
            .ok()
    }

    pub fn heads_of(&self, root_table: &str, root_id: &str) -> Vec<Head> {
        heads(&self.conn, root_table, root_id).unwrap()
    }

    /// Whether this device is showing a conflict on that register.
    pub fn conflicted(&self, root_table: &str, root_id: &str) -> bool {
        self.heads_of(root_table, root_id).len() > 1
    }

    /// The review queue's rows for a register: one per branch waiting.
    pub fn conflict_rows(&self, root_table: &str, root_id: &str) -> Vec<(String, i64)> {
        self.conn
            .prepare(
                "SELECT other_device, other_seq FROM sync_conflict
                 WHERE root_table = ?1 AND root_id = ?2 ORDER BY other_device",
            )
            .unwrap()
            .query_map([root_table, root_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }
}

/// Let the clock move on, so the next write on any device is stamped after
/// everything any device holds.
///
/// For a scenario where WHICH version goes live is part of what it sets up.
/// The hybrid clock takes the wall clock's millisecond when it is past
/// everything a device has seen, and every simulated device reads one clock —
/// so two milliseconds later, the next stamp is later than every stamp so far,
/// whatever each device's counter stood at.
pub fn later() {
    std::thread::sleep(std::time::Duration::from_millis(2));
}

/// Device ids that sort A < B < C < D, so the device-id tie-break is legible.
pub const A: &str = "0192f3a4-0000-7000-8000-00000000000a";
pub const B: &str = "0192f3a4-0000-7000-8000-00000000000b";
pub const C: &str = "0192f3a4-0000-7000-8000-00000000000c";
pub const D: &str = "0192f3a4-0000-7000-8000-00000000000d";

/// One log row, carried between devices exactly as it was written.
struct Carried {
    values: Vec<rusqlite::types::Value>,
    root_table: String,
    root_id: String,
}

/// Every column of `record_change`, in a fixed order, so a carried row is
/// rebuilt on the far side without naming them twice.
const LOG_COLUMNS: &str = "id, entity_table, entity_id, season_id, operation, changed_at, actor,
     payload, root_table, root_id, origin_device, origin_seq, version_vector, hlc";

/// Copy whatever `from` has logged that `to` has not, then settle every
/// register those rows touched.
///
/// **This is the whole of what a transport owes the merge layer**: deliver the
/// rows, then ask each affected register to bring its tables in line. The
/// before-state each register was showing is read BEFORE the new rows land,
/// which is how `settle` knows what to rewind — nothing records it.
///
/// Log rows are carried verbatim. A change set's identity is
/// `(origin_device, origin_seq)` everywhere and forever, so a receiving device
/// that re-stamped anything would be inventing a different history.
pub fn sync(from: &Device, to: &mut Device) {
    try_sync(from, to).expect("the bundle applied");
}

/// The same delivery, returning what the apply said.
///
/// **A bundle is refused whole**: every register settles inside one
/// transaction, so a refusal anywhere rolls back the rows that were already
/// inserted. Half a campaign is not a state anything downstream could reason
/// about, and the farmer's retry has to start from where they were.
pub fn try_sync(from: &Device, to: &mut Device) -> Result<(), CoreError> {
    let held: BTreeSet<String> = to
        .conn
        .prepare("SELECT id FROM record_change")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();

    let carried: Vec<Carried> = from
        .conn
        .prepare(&format!(
            "SELECT {LOG_COLUMNS} FROM record_change ORDER BY hlc, id"
        ))
        .unwrap()
        .query_map([], |row| {
            let mut values = Vec::new();
            for index in 0..14 {
                values.push(row.get::<_, rusqlite::types::Value>(index)?);
            }
            Ok(Carried {
                root_table: row.get("root_table")?,
                root_id: row.get("root_id")?,
                values,
            })
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
        .into_iter()
        .filter(|carried| match &carried.values[0] {
            rusqlite::types::Value::Text(id) => !held.contains(id),
            _ => false,
        })
        .collect();

    if carried.is_empty() {
        return Ok(());
    }

    let touched: BTreeSet<(String, String)> = carried
        .iter()
        .map(|row| (row.root_table.clone(), row.root_id.clone()))
        .collect();

    // What each register was showing before the delivery — the head `settle`
    // has to rewind out of.
    let was_live: Vec<Option<Head>> = touched
        .iter()
        .map(|(table, id)| live_head(&to.heads_of(table, id)).cloned())
        .collect();

    let tx = to.conn.transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    {
        let mut insert = tx
            .prepare(&format!(
                "INSERT INTO record_change ({LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"
            ))
            .unwrap();
        for row in &carried {
            insert
                .execute(rusqlite::params_from_iter(row.values.iter()))
                .unwrap();
        }
    }
    for ((table, id), before) in touched.iter().zip(was_live) {
        // `?` drops the transaction, which rolls it back — the log rows this
        // delivery inserted go with it.
        settle(&tx, table, id, before.as_ref())?;
    }
    tx.commit()?;
    Ok(())
}

/// Sync both ways, which is what two file copies amount to.
pub fn sync_both(left: &mut Device, right: &mut Device) {
    let left_snapshot = Device {
        conn: clone_log(&left.conn),
        id: left.id.clone(),
    };
    sync(&left_snapshot, right);
    let right_snapshot = Device {
        conn: clone_log(&right.conn),
        id: right.id.clone(),
    };
    sync(&right_snapshot, left);
}

/// A throwaway database holding a copy of one device's log — a stand-in for the
/// bundle file, so a two-way sync does not need two mutable borrows at once.
fn clone_log(source: &Connection) -> Connection {
    let copy = terrazgo_core::open_in_memory().unwrap();
    let rows: Vec<Vec<rusqlite::types::Value>> = source
        .prepare(&format!("SELECT {LOG_COLUMNS} FROM record_change"))
        .unwrap()
        .query_map([], |row| {
            (0..14)
                .map(|index| row.get::<_, rusqlite::types::Value>(index))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    {
        let mut insert = copy
            .prepare(&format!(
                "INSERT INTO record_change ({LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"
            ))
            .unwrap();
        for row in rows {
            insert
                .execute(rusqlite::params_from_iter(row.iter()))
                .unwrap();
        }
    }
    copy
}

/// A farm on `origin`, carried to every other device, so the scenarios start
/// from shared history rather than from nothing.
pub fn shared_farm(origin: &mut Device, others: &mut [&mut Device]) -> String {
    let farm = repo::insert_farm(&mut origin.conn, new_farm("Los Llanos"), None).unwrap();
    for other in others {
        let snapshot = Device {
            conn: clone_log(&origin.conn),
            id: origin.id.clone(),
        };
        sync(&snapshot, other);
    }
    farm.id
}

/// A farmer correcting the holding's name — the smallest real edit there is,
/// and the one every scenario below makes concurrently.
pub fn rename(device: &mut Device, farm_id: &str, name: &str) {
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
// A register with children, shared by the files that need one
// ---------------------------------------------------------------------------

/// A register with children, so a conflict can land on two DIFFERENT rows of
/// one statement — which is the case the whole-register unit of merge exists
/// for (docs/sync.md → The unit of merge is the whole register, whose worked
/// example is "one device corrects the dose while another adds a treated
/// plot").
///
/// Every other scenario in this file conflicts on one row, where the winner's
/// image simply overwrites the loser's. Here the two branches touch rows that
/// never meet, so nothing overwrites anything and what the devices end up
/// holding is decided by the rewind alone.
pub fn one_book(device: &mut Device) -> (String, String, String, String) {
    let farm = repo::insert_farm(&mut device.conn, new_farm("Los Llanos"), None).unwrap();
    let season = repo::insert_season(
        &mut device.conn,
        new_season(&farm.id, 2026, "2025/2026"),
        None,
    )
    .unwrap();
    let first = repo::insert_plot(&mut device.conn, new_plot(&farm.id, "El Prado"), None).unwrap();
    let second = repo::insert_plot(&mut device.conn, new_plot(&farm.id, "La Loma"), None).unwrap();
    (farm.id, season.id, first.id, second.id)
}

/// A sowing record's state as a form would submit it: the whole register,
/// plots included, which is how `update_sowing_record` reconciles children.
pub fn sowing_state(notes: Option<&str>, plots: &[&str]) -> UpdateSowingRecord {
    UpdateSowingRecord {
        kind_code: "sowing".into(),
        sown_on: "2026-04-10".into(),
        sowing_end_date: None,
        flooded_on: None,
        seed_quantity_kg: Some(180.0),
        notes: notes.map(str::to_string),
        plots: plots
            .iter()
            .map(|plot_id| NewSowingPlot {
                plot_id: (*plot_id).into(),
                crop_id: None,
            })
            .collect(),
    }
}

/// The plots a device currently has on its sowing records.
pub fn sown_plots(device: &Device) -> Vec<String> {
    device
        .conn
        .prepare("SELECT plot_id FROM sowing_plot ORDER BY plot_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

/// A book on `origin` holding one sowing on its first plot, carried to every
/// other device. Returns the record and the book's two plots.
pub fn a_sown_record(origin: &mut Device, others: &mut [&mut Device]) -> (String, String, String) {
    let (farm_id, season_id, first, second) = one_book(origin);
    let record = repo::insert_sowing_record(
        &mut origin.conn,
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
    for other in others {
        sync(origin, other);
    }
    (record.id, first, second)
}

/// The plots on a device's sowing records, in one order whatever the rows'.
pub fn plots_sown(device: &Device) -> Vec<String> {
    let mut plots = sown_plots(device);
    plots.sort();
    plots
}
