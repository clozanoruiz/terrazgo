// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The aggregate map, checked against the composed schema (docs/sync.md → The
//! aggregate map).
//!
//! Every write already checks its log rows against the map as it happens, so a
//! row can never be filed under the wrong register. What that cannot check is
//! the map itself: whether it names every table, and whether its registers are
//! drawn where the schema's constraints need them. That is this test.
//!
//! **There is no list of tables here: the schema is the expectation.** A user
//! table is any table not keyed by a lookup `code` — the seeded reference
//! tables, identical on two devices at one schema version — other than
//! `record_change`, which is the log rather than something logged. Each must be
//! declared exactly once, by core or by the module that owns it.
//!
//! The second half is the reason slot-keyed registers exist. Two devices can
//! each insert a row the other has not seen; if those rows collide on a UNIQUE
//! index but belong to different registers, the merge sees nothing and the
//! apply fails. So every UNIQUE index must fall inside one register:
//!
//!   * a register keyed by its row (`Root`) may carry no UNIQUE index that
//!     leaves out `id`;
//!   * a slot must be exactly the union of its table's UNIQUE keys — coarser
//!     and two registers share a key, finer and one key spans two registers;
//!   * a child's UNIQUE indexes must include the column naming its register,
//!     so they only ever collide within one register, which a merge replaces
//!     whole.
//!
//! The exceptions are collisions the design knows about and resolves some
//! other way; each names where.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;
use terrazgo_core::sync::{SyncRole, TableSync};
use terrazgo_lib::db::composed_migrations;
use terrazgo_lib::registry::composed_sync_shape;

/// UNIQUE keys two devices can fill independently that the merge does NOT see
/// as a conflict, and where each one's resolution is written down. Adding a
/// line here is a design decision, not a test fix.
const KNOWN_COLLISIONS: &[(&str, &[&str], &str)] = &[
    (
        "season",
        &["farm_id", "label"],
        "two devices creating one campaign's book offline. The UNIQUE STAYS and \
         the merge refuses the apply (`season_label_collision`), because every \
         way of enforcing it silently would either fuse two different campaigns \
         or invent a name for a document that is printed and submitted — so a \
         person decides. docs/sync.md → Seasons created on two devices",
    ),
    (
        "export_alias",
        &["target", "alias"],
        "two devices minting one integer — prevented by one submitting device \
         per farm, built with the exporter: docs/sync.md → `export_alias` \
         collisions",
    ),
];

fn composed_schema() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();
    conn
}

fn strings(conn: &Connection, sql: &str, param: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([param], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn tables(conn: &Connection) -> Vec<String> {
    strings(
        conn,
        "SELECT name FROM sqlite_schema
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != ?1
         ORDER BY name",
        "record_change",
    )
}

fn columns(conn: &Connection, table: &str) -> BTreeSet<String> {
    strings(conn, "SELECT name FROM pragma_table_info(?1)", table)
        .into_iter()
        .collect()
}

fn primary_key(conn: &Connection, table: &str) -> Vec<String> {
    strings(
        conn,
        "SELECT name FROM pragma_table_info(?1) WHERE pk > 0 ORDER BY pk",
        table,
    )
}

/// Every UNIQUE index that is not the primary key, as its column set —
/// `UNIQUE (…)` constraints and `CREATE UNIQUE INDEX`, partial ones included.
fn unique_keys(conn: &Connection, table: &str) -> Vec<BTreeSet<String>> {
    strings(
        conn,
        "SELECT name FROM pragma_index_list(?1) WHERE \"unique\" = 1 AND origin != 'pk'",
        table,
    )
    .iter()
    .map(|index| {
        strings(conn, "SELECT name FROM pragma_index_info(?1)", index)
            .into_iter()
            .collect()
    })
    .collect()
}

/// `(target table, on_delete)` of the foreign key on `column`, if any.
fn foreign_key(conn: &Connection, table: &str, column: &str) -> Option<(String, String)> {
    conn.prepare("SELECT \"table\", on_delete FROM pragma_foreign_key_list(?1) WHERE \"from\" = ?2")
        .unwrap()
        .query_row([table, column], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok()
}

fn is_known_collision(table: &str, key: &BTreeSet<String>) -> bool {
    KNOWN_COLLISIONS.iter().any(|(known_table, known_key, _)| {
        *known_table == table
            && known_key.len() == key.len()
            && known_key.iter().all(|column| key.contains(*column))
    })
}

fn declared() -> BTreeMap<&'static str, SyncRole> {
    let mut map = BTreeMap::new();
    for TableSync { table, role } in composed_sync_shape() {
        assert!(
            map.insert(table, role).is_none(),
            "{table} is declared twice in the aggregate map"
        );
    }
    map
}

#[test]
fn every_user_table_is_declared_exactly_once() {
    let conn = composed_schema();
    let map = declared();
    let mut undeclared = Vec::new();
    for table in tables(&conn) {
        let lookup = primary_key(&conn, &table) == ["code"];
        match (lookup, map.contains_key(table.as_str())) {
            (false, false) => undeclared.push(table),
            (true, true) => panic!(
                "{table} is a seeded lookup (keyed by `code`) and must not be in the \
                 aggregate map — every device already holds the same rows"
            ),
            _ => {}
        }
    }
    assert!(
        undeclared.is_empty(),
        "tables nobody has classified for the merge: {undeclared:?} — declare each \
         in its owning crate's sync shape (core's CORE_SYNC_SHAPE or the module's \
         SYNC_SHAPE)"
    );
    for table in map.keys() {
        assert!(
            tables(&conn).iter().any(|t| t == table),
            "the aggregate map declares {table}, which the schema does not have"
        );
    }
}

#[test]
fn every_declaration_matches_the_table_it_describes() {
    let conn = composed_schema();
    let map = declared();
    for (table, role) in &map {
        let present = columns(&conn, table);
        match role {
            SyncRole::Root => assert_eq!(
                primary_key(&conn, table),
                ["id"],
                "{table} is declared a register keyed by its row, so its key must be `id`"
            ),
            SyncRole::Child { root, fk } => {
                assert!(
                    matches!(map.get(root), Some(SyncRole::Root)),
                    "{table}'s register {root} must be declared a Root"
                );
                // It lives and dies with its register: that is what lets a
                // merge replace a register's children whole.
                assert_eq!(
                    foreign_key(&conn, table, fk),
                    Some(((*root).to_string(), "CASCADE".to_string())),
                    "{table}.{fk} must reference {root} ON DELETE CASCADE"
                );
                // Every logged row is checked to be the row its log call
                // names, read off its image as `id` — or, for a regional
                // extension, as the parent column that is its whole key.
                let key = primary_key(&conn, table);
                assert!(
                    key == ["id"] || key == [*fk],
                    "{table} must be keyed by `id` or by {fk} alone, not {key:?}"
                );
            }
            SyncRole::Slot { columns: slot } => {
                assert_eq!(
                    primary_key(&conn, table),
                    ["id"],
                    "{table} is a slot-keyed register; its rows are still keyed by `id`"
                );
                for column in *slot {
                    assert!(
                        present.contains(*column),
                        "{table} has no slot column {column}"
                    );
                }
            }
            SyncRole::Local => {}
        }
    }
}

/// Every UNIQUE key in the schema that two devices could fill without the merge
/// seeing a conflict, under `map`. Empty for a sound map.
fn unseen_collisions(conn: &Connection, map: &BTreeMap<&'static str, SyncRole>) -> Vec<String> {
    let mut unseen = Vec::new();
    for (table, role) in map {
        let keys: Vec<BTreeSet<String>> = unique_keys(conn, table)
            .into_iter()
            .filter(|key| !is_known_collision(table, key))
            .collect();
        match role {
            SyncRole::Root => {
                for key in keys.iter().filter(|key| !key.contains("id")) {
                    unseen.push(format!(
                        "{table} UNIQUE {key:?}: a register keyed by its row cannot see two \
                         devices fill this key — key the register on it (SyncRole::Slot)"
                    ));
                }
            }
            SyncRole::Slot { columns: slot } => {
                let slot: BTreeSet<String> = slot.iter().map(|c| (*c).to_string()).collect();
                let union: BTreeSet<String> = keys.iter().flatten().cloned().collect();
                if slot != union {
                    unseen.push(format!(
                        "{table}: slot {slot:?} is not the union of its UNIQUE keys {union:?}"
                    ));
                }
            }
            SyncRole::Child { fk, .. } => {
                for key in keys.iter().filter(|key| !key.contains(*fk)) {
                    unseen.push(format!(
                        "{table} UNIQUE {key:?} leaves out {fk}, so two registers' children \
                         can collide"
                    ));
                }
            }
            SyncRole::Local => {}
        }
    }
    unseen
}

#[test]
fn no_unique_key_can_be_filled_by_two_devices_unseen() {
    let conn = composed_schema();
    let unseen = unseen_collisions(&conn, &declared());
    assert!(unseen.is_empty(), "{unseen:#?}");
}

#[test]
fn the_collision_rule_catches_the_mistakes_it_exists_for() {
    // The control: the same rule against maps that are wrong in the ways this
    // arc nearly was. A rule that passes the real map and these alike would be
    // checking nothing.
    let conn = composed_schema();
    let wrong = |table: &'static str, role: SyncRole| {
        let mut map = declared();
        map.insert(table, role);
        unseen_collisions(&conn, &map)
    };
    // A replace-in-place table keyed by its row id.
    assert_eq!(
        wrong("geo_feature", SyncRole::Root).len(),
        2,
        "both arms of the arc"
    );
    // The first zone-flag slot written in this arc: coarser than the index, so
    // one check's zone types shared a register.
    assert_eq!(
        wrong(
            "plot_zone_flag",
            SyncRole::Slot {
                columns: &["plot_id", "campaign", "source"]
            }
        )
        .len(),
        1
    );
    // A child declared under a column its UNIQUE key leaves out: its rows
    // could then collide across registers.
    assert_eq!(
        wrong(
            "treatment_plot",
            SyncRole::Child {
                root: "treatment_record",
                fk: "crop_id"
            }
        )
        .len(),
        1
    );
}

#[test]
fn every_known_collision_is_still_real() {
    // A listed exception whose index has gone is an allowance nobody needs,
    // waiting to excuse something else.
    let conn = composed_schema();
    for (table, key, why) in KNOWN_COLLISIONS {
        let key: BTreeSet<String> = key.iter().map(|c| (*c).to_string()).collect();
        assert!(
            unique_keys(&conn, table).contains(&key),
            "{table} no longer has UNIQUE {key:?}; drop its entry ({why})"
        );
    }
}
