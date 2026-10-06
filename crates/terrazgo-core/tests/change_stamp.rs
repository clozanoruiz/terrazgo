// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The change stamp every write opens (docs/sync.md → The change stamp): what
//! each `record_change` row says about the device and change set it came from,
//! and which register it belongs to.
//!
//! Nothing merges yet. These rows are what the merge engine will read, and a
//! row written wrong today stays wrong in the log forever — so their shape is
//! pinned from the first write, through the real repositories, including the
//! writes that touch more than one register in one transaction.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use serde_json::json;
use terrazgo_core::CoreError;
use terrazgo_core::audit;
use terrazgo_core::models::*;
use terrazgo_core::open_in_memory;
use terrazgo_core::repository as repo;
use terrazgo_core::sync::{Hlc, VersionVector, installed_device, slot_id};
use terrazgo_testkit::{FarmWithPlots, farm_with_plots};

/// One `record_change` row, with the stamp columns decoded.
#[derive(Debug)]
struct Logged {
    entity_table: String,
    entity_id: String,
    root_table: String,
    root_id: String,
    device: String,
    seq: i64,
    vector: VersionVector,
    hlc: Hlc,
    changed_at: String,
    actor: Option<String>,
    season_id: Option<String>,
}

/// The whole log, in the order it was written (ids are UUIDv7, ordered by
/// creation within a process).
fn log(conn: &Connection) -> Vec<Logged> {
    let mut stmt = conn
        .prepare(
            "SELECT entity_table, entity_id, root_table, root_id, origin_device, origin_seq,
                    version_vector, hlc, changed_at, actor, season_id
             FROM record_change ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        Ok(Logged {
            entity_table: r.get(0)?,
            entity_id: r.get(1)?,
            root_table: r.get(2)?,
            root_id: r.get(3)?,
            device: r.get(4)?,
            seq: r.get(5)?,
            vector: VersionVector::from_json(&r.get::<_, String>(6)?).unwrap(),
            hlc: r.get(7)?,
            changed_at: r.get(8)?,
            actor: r.get(9)?,
            season_id: r.get(10)?,
        })
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

/// The rows the last change set wrote.
fn last_set(conn: &Connection) -> Vec<Logged> {
    let all = log(conn);
    let me = installed_device(conn).unwrap();
    let last = all
        .iter()
        .filter(|row| row.device == me)
        .map(|row| row.seq)
        .max()
        .unwrap();
    all.into_iter()
        .filter(|row| row.device == me && row.seq == last)
        .collect()
}

fn vv(entries: &[(&str, i64)]) -> VersionVector {
    let mut vector = VersionVector::default();
    for (device, seq) in entries {
        vector.observe(device, *seq);
    }
    vector
}

/// A row another device wrote about `root`, as a merge would have stored it.
fn foreign_row(
    conn: &Connection,
    device: &str,
    seq: i64,
    root: (&str, &str),
    vector: &str,
    hlc: u64,
) {
    conn.execute(
        "INSERT INTO record_change
           (id, entity_table, entity_id, operation, changed_at, payload,
            root_table, root_id, origin_device, origin_seq, version_vector, hlc)
         VALUES (?1, ?2, ?3, 'update', '2026-09-18T10:00:00Z', '{}', ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            format!("{device}-{seq}"),
            root.0,
            root.1,
            device,
            seq,
            vector,
            i64::try_from(hlc).unwrap()
        ],
    )
    .unwrap();
}

const DEVICE_B: &str = "0192f3a4-0000-7000-8000-0000000000b0";
const DEVICE_C: &str = "0192f3a4-0000-7000-8000-0000000000c0";

// ---------------------------------------------------------------------------
// The change set
// ---------------------------------------------------------------------------

#[test]
fn a_connection_without_a_device_cannot_write_at_all() {
    // Loud, and before anything lands: a row with no origin could never be
    // merged, and a write that half-committed would be worse than none.
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    terrazgo_core::migrations().to_latest(&mut conn).unwrap();

    let refused = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            country_code: "es".into(),
            ..NewFarm::default()
        },
        None,
    );
    assert!(matches!(
        refused,
        Err(CoreError::Stamp("no_device_identity"))
    ));
    let farms: i64 = conn
        .query_row("SELECT COUNT(*) FROM farm", [], |r| r.get(0))
        .unwrap();
    assert_eq!(farms, 0, "the transaction rolled back with the refusal");
}

#[test]
fn every_row_of_one_write_shares_one_change_set() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());

    let record = repo::insert_harvest_record(
        &mut conn,
        NewHarvestRecord {
            season_id: fx.season_id.clone(),
            farm_id: fx.farm_id.clone(),
            harvested_on: "2026-07-01".into(),
            product_name: "Trigo".into(),
            plant_product_code: None,
            quantity_value: None,
            quantity_unit_code: None,
            delivery_note_ref: None,
            lot_number: None,
            buyer_name: "Cooperativa".into(),
            buyer_tax_id: None,
            buyer_address: None,
            buyer_registry_number: None,
            notes: None,
            plots: vec![
                NewHarvestPlot {
                    plot_id: fx.plot_a.clone(),
                    crop_id: None,
                },
                NewHarvestPlot {
                    plot_id: fx.plot_b.clone(),
                    crop_id: None,
                },
            ],
        },
        Some("profile-1"),
    )
    .unwrap();

    let set = last_set(&conn);
    assert_eq!(set.len(), 3, "two treated plots and the record");
    let first = &set[0];
    for row in &set {
        assert_eq!(row.seq, first.seq);
        assert_eq!(row.hlc, first.hlc);
        assert_eq!(row.changed_at, first.changed_at);
        assert_eq!(row.vector, first.vector);
        // The register is the record, children included.
        assert_eq!(
            (row.root_table.as_str(), row.root_id.as_str()),
            ("harvest_record", record.record.id.as_str())
        );
        assert_eq!(row.season_id.as_deref(), Some(fx.season_id.as_str()));
        assert_eq!(row.actor.as_deref(), Some("profile-1"));
    }
    assert_eq!(
        set.iter()
            .filter(|row| row.entity_table == "harvest_plot")
            .count(),
        2
    );
}

#[test]
fn each_write_is_this_devices_next_change_set_and_the_clock_only_rises() {
    let mut conn = open_in_memory().unwrap();
    farm_with_plots(&mut conn, FarmWithPlots::default());

    let mut sets: Vec<(i64, Hlc)> = log(&conn).iter().map(|row| (row.seq, row.hlc)).collect();
    sets.dedup();
    assert!(sets.len() > 3, "the fixture is several writes");
    for (index, (seq, _)) in sets.iter().enumerate() {
        assert_eq!(
            *seq,
            i64::try_from(index).unwrap() + 1,
            "1, 2, 3 … with no gap"
        );
    }
    assert!(sets.windows(2).all(|pair| pair[0].1 < pair[1].1));
}

#[test]
fn another_devices_numbers_do_not_move_this_ones() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let before = last_set(&conn)[0].seq;
    foreign_row(
        &conn,
        DEVICE_B,
        900,
        ("farm", "elsewhere"),
        r#"{"b":900}"#,
        1,
    );

    rename_plot(&mut conn, &fx.plot_a, "El Prado Alto");
    assert_eq!(last_set(&conn)[0].seq, before + 1);
}

#[test]
fn logging_one_row_twice_in_one_change_set_is_refused() {
    // A change set states each row once: two images of one row in one set
    // would leave a merge two answers for what the set said.
    let mut conn = open_in_memory().unwrap();
    let tx = audit::begin(&mut conn, None).unwrap();
    let stamp = tx.register("farm", "f1", None).unwrap();
    audit::log_insert(&tx, &stamp, "farm", "f1", &json!({ "id": "f1" })).unwrap();
    let row = json!({ "id": "f1" });
    let twice = audit::log_update(&tx, &stamp, "farm", "f1", &row, &row);
    assert!(matches!(twice, Err(CoreError::Sqlite(_))));
}

// ---------------------------------------------------------------------------
// Version vectors
// ---------------------------------------------------------------------------

fn rename_plot(conn: &mut Connection, plot_id: &str, name: &str) {
    repo::update_plot(
        conn,
        plot_id,
        UpdatePlot {
            name: name.into(),
            area_ha: Some(4.0),
            es: None,
        },
        None,
    )
    .unwrap();
}

#[test]
fn a_registers_vector_counts_its_own_history_and_nothing_else() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let me = installed_device(&conn).unwrap();
    let created = log(&conn)
        .into_iter()
        .find(|row| row.root_id == fx.plot_a)
        .unwrap();
    assert_eq!(created.vector, vv(&[(&me, created.seq)]));

    rename_plot(&mut conn, &fx.plot_a, "El Prado Alto");
    let renamed = last_set(&conn).remove(0);
    assert_eq!(
        renamed.vector,
        vv(&[(&me, renamed.seq)]),
        "a vector only grows"
    );

    // A write to ANOTHER register starts from that register's own history.
    rename_plot(&mut conn, &fx.plot_b, "La Loma Baja");
    let other = last_set(&conn).remove(0);
    assert_eq!(other.root_id, fx.plot_b);
    let plot_b_created = log(&conn)
        .into_iter()
        .find(|row| row.root_id == fx.plot_b)
        .unwrap();
    assert!(plot_b_created.seq < renamed.seq);
    assert_eq!(other.vector, vv(&[(&me, other.seq)]));
}

#[test]
fn a_write_dominates_every_version_this_device_holds_of_the_register() {
    // Two other devices edited plot A concurrently and both edits have
    // arrived. A write here merges BOTH into its vector, which is what lets a
    // resolution close the conflict on every device (docs/sync.md → Conflicts
    // as the person sees them). Their clocks were ahead of this one; the
    // stamp still follows them.
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let me = installed_device(&conn).unwrap();
    let mine = last_set(&conn)[0].seq;
    let ahead = (4_000_000_000_000_u64) << Hlc::COUNTER_BITS; // year 2096
    foreign_row(
        &conn,
        DEVICE_B,
        3,
        ("plot", &fx.plot_a),
        &format!(r#"{{"{DEVICE_B}":3}}"#),
        ahead,
    );
    foreign_row(
        &conn,
        DEVICE_C,
        5,
        ("plot", &fx.plot_a),
        &format!(r#"{{"{DEVICE_C}":5}}"#),
        ahead + 7,
    );

    rename_plot(&mut conn, &fx.plot_a, "El Prado Alto");
    let written = last_set(&conn).remove(0);
    assert_eq!(written.seq, mine + 1);
    assert_eq!(
        written.vector,
        vv(&[(DEVICE_B, 3), (DEVICE_C, 5), (&me, written.seq)])
    );
    assert!(
        written.hlc.raw() > ahead + 7,
        "the clock followed what it saw"
    );
}

// ---------------------------------------------------------------------------
// Registers: children, slots, and several registers in one transaction
// ---------------------------------------------------------------------------

#[test]
fn a_child_row_is_logged_under_its_register() {
    let mut conn = open_in_memory().unwrap();
    let farm = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            country_code: "es".into(),
            representative: Some(FarmRepresentativeFields {
                full_name: "Ana Pérez".into(),
                tax_id: None,
                representation_kind: None,
                address: None,
                locality: None,
                province: None,
                postal_code: None,
                phone: None,
                email: None,
            }),
            ..NewFarm::default()
        },
        None,
    )
    .unwrap();

    let set = last_set(&conn);
    let child = set
        .iter()
        .find(|row| row.entity_table == "farm_representative")
        .unwrap();
    assert_eq!(
        (child.root_table.as_str(), child.root_id.as_str()),
        ("farm", farm.id.as_str())
    );
    assert_eq!(set.len(), 2);
}

#[test]
fn a_slot_register_spans_every_row_that_fills_the_slot() {
    // Declared, withdrawn, declared again: three writes, two rows, ONE
    // register — so a device that declared offline and one that withdrew are
    // two versions of one thing, not two unrelated rows.
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let slot = slot_id(&[json!(fx.plot_a)]);

    let first = repo::set_water_declaration(&mut conn, &fx.plot_a, "2026-03-01", None).unwrap();
    repo::clear_water_declaration(&mut conn, &fx.plot_a, None).unwrap();
    let second = repo::set_water_declaration(&mut conn, &fx.plot_a, "2026-04-01", None).unwrap();
    assert_ne!(first.id, second.id, "a fresh row");

    let rows: Vec<Logged> = log(&conn)
        .into_iter()
        .filter(|row| row.entity_table == "plot_water_declaration")
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|row| row.root_table == "plot_water_declaration" && row.root_id == slot)
    );
    let me = installed_device(&conn).unwrap();
    assert_eq!(rows[2].vector.get(&me), rows[2].seq);
    assert!(rows[2].seq > rows[0].seq);
}

#[test]
fn recording_a_water_point_withdraws_the_declaration_in_the_same_change_set() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    repo::set_water_declaration(&mut conn, &fx.plot_a, "2026-03-01", None).unwrap();
    // Another device has also touched the declaration, so its register has a
    // history the point's does not.
    let slot = slot_id(&[json!(fx.plot_a)]);
    foreign_row(
        &conn,
        DEVICE_B,
        4,
        ("plot_water_declaration", &slot),
        &format!(r#"{{"{DEVICE_B}":4}}"#),
        1,
    );

    let point = repo::insert_water_point(
        &mut conn,
        NewWaterPoint {
            plot_id: fx.plot_a.clone(),
            denomination: "Pozo".into(),
            inside_plot: true,
            distance_m: None,
            latitude: None,
            longitude: None,
        },
        None,
    )
    .unwrap();

    let set = last_set(&conn);
    assert_eq!(set.len(), 2);
    let withdrawal = set
        .iter()
        .find(|row| row.entity_table == "plot_water_declaration")
        .unwrap();
    let recorded = set
        .iter()
        .find(|row| row.entity_table == "plot_water_point")
        .unwrap();
    assert_eq!(withdrawal.root_id, slot);
    assert_eq!(recorded.root_id, point.id);
    // One change set, two registers — each with its own history.
    assert_eq!(
        (withdrawal.seq, withdrawal.hlc),
        (recorded.seq, recorded.hlc)
    );
    let me = recorded.device.as_str();
    assert_eq!(withdrawal.vector, vv(&[(DEVICE_B, 4), (me, recorded.seq)]));
    assert_eq!(recorded.vector, vv(&[(me, recorded.seq)]));
}

#[test]
fn withdrawing_an_advisor_withdraws_each_link_as_a_register_of_its_own() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let advisor = repo::insert_advisor(
        &mut conn,
        NewAdvisor {
            name: "Asesoría del Duero".into(),
            tax_id: None,
            registration_number: None,
        },
        None,
    )
    .unwrap();
    for farm in [&fx.farm_id, &fx.other_farm_id] {
        repo::set_farm_advisor(&mut conn, farm, &advisor.id, None, None).unwrap();
    }

    repo::soft_delete_advisor(&mut conn, &advisor.id, None).unwrap();

    let set = last_set(&conn);
    assert_eq!(set.len(), 3);
    let mut roots: Vec<(String, String)> = set
        .iter()
        .map(|row| (row.root_table.clone(), row.root_id.clone()))
        .collect();
    roots.sort();
    let mut expected = vec![
        ("advisor".to_string(), advisor.id.clone()),
        (
            "farm_advisor".to_string(),
            slot_id(&[json!(fx.farm_id), json!(advisor.id)]),
        ),
        (
            "farm_advisor".to_string(),
            slot_id(&[json!(fx.other_farm_id), json!(advisor.id)]),
        ),
    ];
    expected.sort();
    assert_eq!(roots, expected);
}

#[test]
fn a_zone_check_writes_one_register_per_zone_type() {
    // The slot is the UNIQUE index's key: a conflict over one zone type must
    // not roll back another device's fresh result for a different one.
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let flag = |code: &str| NewZoneFlag {
        zone_type_code: code.into(),
        status: "outside".into(),
        coverage_pct: None,
        detail: None,
    };
    let check = |conn: &mut Connection| {
        repo::replace_zone_flags(
            conn,
            &fx.plot_a,
            2026,
            "sigpac",
            vec![flag("nitrate_vulnerable"), flag("natura_2000")],
            None,
        )
        .unwrap();
    };

    check(&mut conn);
    let first = last_set(&conn);
    assert_eq!(first.len(), 2);
    let nitrate = slot_id(&[
        json!(fx.plot_a),
        json!("nitrate_vulnerable"),
        json!(2026),
        json!("sigpac"),
    ]);
    let natura = slot_id(&[
        json!(fx.plot_a),
        json!("natura_2000"),
        json!(2026),
        json!("sigpac"),
    ]);
    let mut roots: Vec<&str> = first.iter().map(|row| row.root_id.as_str()).collect();
    roots.sort_unstable();
    let mut expected = vec![nitrate.as_str(), natura.as_str()];
    expected.sort_unstable();
    assert_eq!(roots, expected);

    // A re-check replaces each type's row: the retired row and its
    // replacement are two versions of that type's register.
    check(&mut conn);
    let second = last_set(&conn);
    assert_eq!(second.len(), 4);
    for root in [&nitrate, &natura] {
        assert_eq!(second.iter().filter(|row| &row.root_id == root).count(), 2);
    }
}

#[test]
fn replacing_a_geometry_is_a_second_version_of_one_register() {
    let mut conn = open_in_memory().unwrap();
    let fx = farm_with_plots(&mut conn, FarmWithPlots::default());
    let square = |corner: f64| NewGeoFeature {
        plot_id: Some(fx.plot_a.clone()),
        farm_id: None,
        role: "boundary".into(),
        geometry: format!(
            r#"{{"type":"Polygon","coordinates":[[[{corner},41.65],[-4.71,41.65],[-4.71,41.66],[{corner},41.66],[{corner},41.65]]]}}"#
        ),
        source: "drawn".into(),
        campaign: None,
        official_area_ha: None,
        properties: None,
        fetched_at: None,
    };

    let first = repo::save_geo_feature(&mut conn, square(-4.72), None).unwrap();
    let second = repo::save_geo_feature(&mut conn, square(-4.73), None).unwrap();

    let slot = slot_id(&[
        json!(null),
        json!(fx.plot_a),
        json!("boundary"),
        json!("drawn"),
    ]);
    let set = last_set(&conn);
    assert_eq!(set.len(), 2, "the retired row and the new one");
    assert!(
        set.iter()
            .all(|row| row.root_table == "geo_feature" && row.root_id == slot)
    );
    let ids: Vec<&str> = set.iter().map(|row| row.entity_id.as_str()).collect();
    assert!(ids.contains(&first.id.as_str()) && ids.contains(&second.id.as_str()));
}

#[test]
fn an_export_alias_is_keyed_by_the_entry_it_aliases() {
    let mut conn = open_in_memory().unwrap();
    repo::ensure_export_alias(&mut conn, "siex", "crop", "c1", "", None).unwrap();
    let row = last_set(&conn).remove(0);
    assert_eq!(row.root_table, "export_alias");
    assert_eq!(
        row.root_id,
        slot_id(&[json!("siex"), json!("crop"), json!("c1"), json!("")])
    );
}

#[test]
fn registering_this_device_is_one_logged_row_however_often_it_runs() {
    let mut conn = open_in_memory().unwrap();
    let me = installed_device(&conn).unwrap();

    let first = repo::register_this_device(&mut conn, None).unwrap();
    let again = repo::register_this_device(&mut conn, None).unwrap();
    assert_eq!(first, again);
    assert_eq!(first.id, me);
    assert_eq!(
        first.label, None,
        "a device cannot know what people call it"
    );

    let rows = log(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].root_table.as_str(), rows[0].root_id.as_str()),
        ("sync_peer", me.as_str())
    );
    assert_eq!(repo::list_sync_peers(&conn).unwrap(), vec![first]);
}

#[test]
fn naming_a_device_is_a_logged_write_like_any_other() {
    let mut conn = open_in_memory().unwrap();
    let me = installed_device(&conn).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();

    let named = repo::rename_sync_peer(&mut conn, &me, Some("  Móvil de María  "), None).unwrap();
    assert_eq!(
        named.label.as_deref(),
        Some("Móvil de María"),
        "trimmed, like every other name the app stores"
    );
    let row = last_set(&conn).remove(0);
    assert_eq!(
        (row.root_table.as_str(), row.root_id.as_str()),
        ("sync_peer", me.as_str()),
        "so the name reaches every other device at the next sync"
    );

    // Unnamed is a state the table already has, and an empty string would be a
    // second spelling of it that prints differently in a list.
    let cleared = repo::rename_sync_peer(&mut conn, &me, Some("   "), None).unwrap();
    assert_eq!(cleared.label, None);
    assert!(matches!(
        repo::rename_sync_peer(
            &mut conn,
            "0192f3a4-0000-7000-8000-00000000beef",
            None,
            None
        ),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn retiring_a_device_is_soft_and_reversible_and_never_this_device() {
    let mut conn = open_in_memory().unwrap();
    let me = installed_device(&conn).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();

    assert!(
        matches!(
            repo::retire_sync_peer(&mut conn, &me, true, None),
            Err(CoreError::Invalid("sync_peer_is_this_device"))
        ),
        "the device making the change is not one to stop expecting changes from"
    );

    // A device the book has heard of, as an arriving bundle would leave it.
    let other = "0192f3a4-0000-7000-8000-0000000000b2";
    conn.execute(
        "INSERT INTO sync_peer (id, label, created_at, updated_at)
         VALUES (?1, 'Portátil', '2026-09-01T10:00:00Z', '2026-09-01T10:00:00Z')",
        [other],
    )
    .unwrap();

    let retired = repo::retire_sync_peer(&mut conn, other, true, None).unwrap();
    assert!(retired.deleted_at.is_some());
    assert_eq!(
        retired.label.as_deref(),
        Some("Portátil"),
        "a retired phone's changes stay in the book and stay attributed to it"
    );
    assert_eq!(
        repo::list_sync_peers(&conn).unwrap().len(),
        2,
        "so it is still listed"
    );
    let back = repo::retire_sync_peer(&mut conn, other, false, None).unwrap();
    assert_eq!(back.deleted_at, None, "and it can come back");
}

#[test]
fn a_restored_device_takes_over_from_the_replica_it_replaced() {
    // What a backup import does after minting a new id. The old replica can
    // never write again — settings.json holds a different id now — so its row
    // is retired, and the name people gave the machine follows the machine.
    let mut conn = open_in_memory().unwrap();
    let replaced = installed_device(&conn).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();
    repo::rename_sync_peer(&mut conn, &replaced, Some("Portátil de la oficina"), None).unwrap();

    // The import: a new identity on this same database.
    let restored = terrazgo_core::sync::mint_device_id();
    terrazgo_core::sync::install_device(&conn, &restored).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();

    let mine = repo::succeed_sync_peer(&mut conn, &replaced, None)
        .unwrap()
        .expect("there was a replica to take over from");
    assert_eq!(mine.id, restored);
    assert_eq!(
        mine.label.as_deref(),
        Some("Portátil de la oficina"),
        "the name follows the machine, not the replica"
    );

    let peers = repo::list_sync_peers(&conn).unwrap();
    let old = peers.iter().find(|peer| peer.id == replaced).unwrap();
    assert!(old.deleted_at.is_some(), "the replica is retired");
    assert_eq!(
        old.label.as_deref(),
        Some("Portátil de la oficina"),
        "and keeps its name, so a version it wrote still reads as that machine"
    );

    // One change set, two registers: a device receiving it sees both halves or
    // neither.
    let set = last_set(&conn);
    assert_eq!(set.len(), 2);
    let roots: Vec<&str> = set.iter().map(|row| row.root_id.as_str()).collect();
    assert!(roots.contains(&replaced.as_str()) && roots.contains(&restored.as_str()));
    assert!(set.iter().all(|row| row.root_table == "sync_peer"));
}

#[test]
fn restoring_another_devices_backup_has_nothing_to_take_over_from() {
    // The laptop's database restored onto the phone: that book has never heard
    // of the phone's previous id, so there is no row to retire and no name to
    // inherit — and nothing at all is written.
    let mut conn = open_in_memory().unwrap();
    repo::register_this_device(&mut conn, None).unwrap();
    let before = log(&conn).len();

    let stranger = "0192f3a4-0000-7000-8000-0000000000f0";
    assert_eq!(
        repo::succeed_sync_peer(&mut conn, stranger, None).unwrap(),
        None
    );
    assert_eq!(log(&conn).len(), before, "a no-op consumes no change set");
}

#[test]
fn a_second_import_changes_nothing_once_the_handover_is_done() {
    let mut conn = open_in_memory().unwrap();
    let replaced = installed_device(&conn).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();
    repo::rename_sync_peer(&mut conn, &replaced, Some("Móvil"), None).unwrap();

    let restored = terrazgo_core::sync::mint_device_id();
    terrazgo_core::sync::install_device(&conn, &restored).unwrap();
    repo::register_this_device(&mut conn, None).unwrap();
    repo::succeed_sync_peer(&mut conn, &replaced, None).unwrap();
    let retired_at = repo::list_sync_peers(&conn)
        .unwrap()
        .into_iter()
        .find(|peer| peer.id == replaced)
        .and_then(|peer| peer.deleted_at);
    let after_first = log(&conn).len();

    assert_eq!(
        repo::succeed_sync_peer(&mut conn, &replaced, None).unwrap(),
        None,
        "the handover already happened"
    );
    assert_eq!(log(&conn).len(), after_first);
    assert_eq!(
        repo::list_sync_peers(&conn)
            .unwrap()
            .into_iter()
            .find(|peer| peer.id == replaced)
            .and_then(|peer| peer.deleted_at),
        retired_at,
        "and the retirement keeps its own date rather than being restamped"
    );
}

// ---------------------------------------------------------------------------
// The aggregate map, checked on every write
// ---------------------------------------------------------------------------
//
// Every other test in the workspace exercises the check by passing it. These
// make sure it can fail: a guard that never refuses anything is
// indistinguishable from one that is not there.

/// Open a transaction, stamp it for `(root_table, root_id)`, log `row` as a
/// `table` insert, and hand back what the log call said. The transaction is
/// dropped uncommitted either way.
fn log_under(
    conn: &mut Connection,
    root: (&str, &str),
    table: &str,
    id: &str,
    row: serde_json::Value,
) -> terrazgo_core::Result<()> {
    let tx = audit::begin(conn, None).unwrap();
    let stamp = tx.register(root.0, root.1, None).unwrap();
    audit::log_insert(&tx, &stamp, table, id, &row)
}

fn refused(result: terrazgo_core::Result<()>, needle: &str) {
    match result {
        Err(CoreError::ShapeViolation(message)) => {
            assert!(
                message.contains(needle),
                "{message:?} should mention {needle:?}"
            )
        }
        other => panic!("expected a ShapeViolation, got {other:?}"),
    }
}

#[test]
fn a_row_stamped_for_another_register_is_refused() {
    let mut conn = open_in_memory().unwrap();
    refused(
        log_under(
            &mut conn,
            ("plot", "plot-b"),
            "plot",
            "plot-a",
            json!({ "id": "plot-a" }),
        ),
        "plot-a belongs to plot plot-a",
    );
}

#[test]
fn a_child_under_the_wrong_register_is_refused() {
    // The mistake the refactor nearly shipped: a row written in a helper,
    // logged with the caller's stamp for a different register.
    let mut conn = open_in_memory().unwrap();
    let plot_row = json!({ "id": "hp1", "harvest_record_id": "harvest-2", "plot_id": "p" });
    refused(
        log_under(
            &mut conn,
            ("harvest_record", "harvest-1"),
            "harvest_plot",
            "hp1",
            plot_row.clone(),
        ),
        "belongs to harvest_record harvest-2",
    );
    // …and the same row under its own register is fine.
    log_under(
        &mut conn,
        ("harvest_record", "harvest-2"),
        "harvest_plot",
        "hp1",
        plot_row,
    )
    .unwrap();
}

#[test]
fn a_child_image_without_its_parent_column_is_refused() {
    let mut conn = open_in_memory().unwrap();
    refused(
        log_under(
            &mut conn,
            ("harvest_record", "h"),
            "harvest_plot",
            "hp1",
            json!({ "id": "hp1" }),
        ),
        "has no harvest_record_id",
    );
}

#[test]
fn a_slot_row_under_another_slot_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let declaration = json!({ "id": "d1", "plot_id": "plot-a", "declared_on": "2026-03-01" });
    refused(
        log_under(
            &mut conn,
            ("plot_water_declaration", &slot_id(&[json!("plot-b")])),
            "plot_water_declaration",
            "d1",
            declaration.clone(),
        ),
        "belongs to plot_water_declaration [\"plot-a\"]",
    );
    // Keyed by its row id instead of its slot, it is refused too.
    refused(
        log_under(
            &mut conn,
            ("plot_water_declaration", "d1"),
            "plot_water_declaration",
            "d1",
            declaration,
        ),
        "belongs to plot_water_declaration [\"plot-a\"]",
    );
}

#[test]
fn a_row_logged_under_another_rows_id_is_refused() {
    // The log's address and its image must name the same row: a receiving
    // device materialises the image under the address.
    let mut conn = open_in_memory().unwrap();
    refused(
        log_under(
            &mut conn,
            ("plot", "plot-a"),
            "plot",
            "plot-a",
            json!({ "id": "plot-b" }),
        ),
        "plot row logged as plot-a, but its image names Some(\"plot-b\")",
    );
    // An extension row has no `id`: its key is its parent's.
    refused(
        log_under(
            &mut conn,
            ("farm", "farm-1"),
            "farm_es_extension",
            "farm-2",
            json!({ "farm_id": "farm-1" }),
        ),
        "farm_es_extension row logged as farm-2",
    );
    log_under(
        &mut conn,
        ("farm", "farm-1"),
        "farm_es_extension",
        "farm-1",
        json!({ "farm_id": "farm-1" }),
    )
    .unwrap();
}

#[test]
fn a_row_stamped_for_another_campaign_is_refused() {
    let mut conn = open_in_memory().unwrap();
    let tx = audit::begin(&mut conn, None).unwrap();
    let stamp = tx.register("crop", "c1", Some("season-2025")).unwrap();
    let crop = json!({ "id": "c1", "season_id": "season-2026" });
    refused(
        audit::log_insert(&tx, &stamp, "crop", "c1", &crop),
        "is in season \"season-2026\", but was stamped for Some(\"season-2025\")",
    );
}

#[test]
fn a_table_that_never_syncs_cannot_be_logged() {
    let mut conn = open_in_memory().unwrap();
    refused(
        log_under(
            &mut conn,
            ("catalogue", "c"),
            "catalogue",
            "c",
            json!({ "id": "c" }),
        ),
        "never synced",
    );
}

#[test]
fn a_table_nobody_declared_cannot_be_logged() {
    let mut conn = open_in_memory().unwrap();
    refused(
        log_under(&mut conn, ("tank", "t"), "tank", "t", json!({ "id": "t" })),
        "tank is not in the aggregate map",
    );
}

#[test]
fn a_refused_row_takes_its_whole_write_with_it() {
    // Through a real repository: the write fails, and nothing of it lands —
    // not the row, not the log. Removing the map's entry for `farm` stands in
    // for a module that forgot to declare a table.
    let mut conn = open_in_memory().unwrap();
    conn.execute("DELETE FROM temp.sync_shape WHERE table_name = 'farm'", [])
        .unwrap();
    let result = repo::insert_farm(
        &mut conn,
        NewFarm {
            name: "Finca".into(),
            country_code: "es".into(),
            ..NewFarm::default()
        },
        None,
    );
    assert!(matches!(result, Err(CoreError::ShapeViolation(_))));
    let (farms, logged): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM farm), (SELECT COUNT(*) FROM record_change)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((farms, logged), (0, 0));
}

#[test]
fn a_connection_without_the_map_cannot_log() {
    let mut conn = Connection::open_in_memory().unwrap();
    terrazgo_core::migrations().to_latest(&mut conn).unwrap();
    terrazgo_core::sync::install_device(&conn, &terrazgo_core::sync::mint_device_id()).unwrap();
    assert!(matches!(
        audit::begin(&mut conn, None),
        Err(CoreError::Stamp("no_sync_shape"))
    ));
}

#[test]
fn declaring_one_table_twice_is_refused() {
    use terrazgo_core::sync::{CORE_SYNC_SHAPE, TableSync, install_shape};
    let conn = Connection::open_in_memory().unwrap();
    let again: &[TableSync] = &[TableSync::root("farm")];
    match install_shape(&conn, &[CORE_SYNC_SHAPE, again]) {
        Err(CoreError::ShapeViolation(message)) => assert_eq!(message, "farm is declared twice"),
        other => panic!("expected a ShapeViolation, got {other:?}"),
    }
}
