// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a startup integrity check costs as the record book grows.
//!
//! `#[ignore]` on purpose: this writes hundreds of megabytes and takes minutes,
//! so it is a measurement to re-run when the question comes up again, not a
//! gate. Run it with:
//!
//! ```text
//! cargo test -p terrazgo --test quick_check_cost -- --ignored --nocapture
//! ```
//!
//! It fills the REAL composed schema — every core and module migration, every
//! index — with `record_change` rows, because that is the table that grows
//! without bound: it is append-only, it logs every write in the app as a
//! complete before/after row image, and it has no retention policy yet (that
//! decision belongs to the Stage-2 sync design, and is also a regulatory one:
//! 3-year minimum, RD 1311/2012 art. 16.3).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use std::time::Instant;
use terrazgo::db::composed_migrations;
use terrazgo_testkit::files::TempFile;

/// Sizes to report at, in bytes. The top of the range is a cooperative-scale
/// holding after a decade, not a smallholder.
const CHECKPOINTS_MB: &[u64] = &[10, 50, 100, 250, 500];

/// A plausible `record_change` payload: a complete before/after image of a
/// treatment row, which is the shape the log actually stores.
fn payload(n: u64) -> String {
    format!(
        r#"{{"before":null,"after":{{"id":"0192f3a4-{n:012x}","farm_id":"0192f3a4-0000-7000-8000-000000000001","season_id":"0192f3a4-0000-7000-8000-000000000002","application_date":"2026-04-17","application_end_date":null,"application_time":"08:30","product_id":"0192f3a4-0000-7000-8000-00000000000a","product_name_snapshot":"Producto fitosanitario de ejemplo","registration_number_snapshot":"ES-00{n:05}","dose_value":1.75,"dose_unit_code":"l_ha","total_quantity_used":12.5,"total_quantity_unit_code":"l","operator_id":"0192f3a4-0000-7000-8000-00000000000b","operator_name_snapshot":"Nombre del aplicador","machinery_id":"0192f3a4-0000-7000-8000-00000000000c","roma_number_snapshot":"ROMA-{n:07}","reganip_number_snapshot":null,"efficacy_code":null,"phi_days":21,"phi_end_date":"2026-05-08","justification_code":"umbral","problem_code":"pulgon","notes":"Aplicacion realizada segun las condiciones de la etiqueta.","deleted_at":null}}}}"#
    )
}

fn size_mb(path: &std::path::Path) -> u64 {
    let mut total = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    for suffix in ["-wal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        total += std::fs::metadata(std::path::Path::new(&p))
            .map(|m| m.len())
            .unwrap_or(0);
    }
    total / (1024 * 1024)
}

#[test]
#[ignore = "writes hundreds of MB and takes minutes; run explicitly with --ignored"]
fn quick_check_cost_as_the_record_book_grows() {
    let file = TempFile::reserve("quick-check-cost.db");
    let mut conn = Connection::open(file.path()).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    composed_migrations().to_latest(&mut conn).unwrap();

    println!();
    println!("  MB |   rows | quick_check | integrity_check | ratio");
    println!("-----+--------+-------------+-----------------+------");

    let mut rows: u64 = 0;
    for target in CHECKPOINTS_MB {
        while size_mb(file.path()) < *target {
            let tx = conn.transaction().unwrap();
            {
                let mut stmt = tx
                    .prepare(
                        "INSERT INTO record_change
                           (id, entity_table, entity_id, season_id, operation,
                            changed_at, actor, payload)
                         VALUES (?1, 'treatment_record', ?2, ?3, 'insert', ?4, ?5, ?6)",
                    )
                    .unwrap();
                for _ in 0..20_000 {
                    rows += 1;
                    stmt.execute(rusqlite::params![
                        format!("0192f3a4-0000-7000-8000-{rows:012x}"),
                        format!("0192f3a4-1111-7000-8000-{rows:012x}"),
                        "0192f3a4-0000-7000-8000-000000000002",
                        "2026-04-17T08:30:00Z",
                        "0192f3a4-0000-7000-8000-00000000000f",
                        payload(rows),
                    ])
                    .unwrap();
                }
            }
            tx.commit().unwrap();
            // Fold the WAL back in so the reported size is the database.
            conn.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();
        }

        let t0 = Instant::now();
        let quick: String = conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .unwrap();
        let dq = t0.elapsed();

        let t1 = Instant::now();
        let full: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        let df = t1.elapsed();

        assert_eq!(quick, "ok");
        assert_eq!(full, "ok");
        println!(
            "{:4} | {:6} | {:8.0} ms | {:12.0} ms | {:.1}x",
            size_mb(file.path()),
            rows,
            dq.as_secs_f64() * 1000.0,
            df.as_secs_f64() * 1000.0,
            df.as_secs_f64() / dq.as_secs_f64().max(f64::EPSILON),
        );
    }
    println!();
}
